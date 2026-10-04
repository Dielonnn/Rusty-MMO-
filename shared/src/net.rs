//! Message framing over a non-blocking TCP stream.
//!
//! Each message is bincode, prefixed with its length as a little-endian u32.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

const MAX_MESSAGE: usize = 4 * 1024 * 1024;

pub struct Connection {
    stream: TcpStream,
    inbuf: Vec<u8>,
    outbuf: Vec<u8>,
    /// The other side hung up; reported once buffered messages are read.
    closed: bool,
}

impl Connection {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            inbuf: Vec::new(),
            outbuf: Vec::new(),
            closed: false,
        })
    }

    /// Connects to `addr` ("host" or "host:port"), giving up after a few seconds.
    pub fn connect(addr: &str, default_port: u16) -> io::Result<Self> {
        let addr = addr.trim();
        let addrs: Vec<SocketAddr> = if addr.contains(':') {
            addr.to_socket_addrs()?.collect()
        } else {
            (addr, default_port).to_socket_addrs()?.collect()
        };
        let mut last_err = io::Error::new(ErrorKind::NotFound, "no address found");
        for a in addrs {
            match TcpStream::connect_timeout(&a, Duration::from_secs(5)) {
                Ok(stream) => return Self::new(stream),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    pub fn send<T: Serialize>(&mut self, msg: &T) -> io::Result<()> {
        let bytes = bincode::serialize(msg).map_err(io::Error::other)?;
        self.outbuf
            .extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.outbuf.extend_from_slice(&bytes);
        self.flush()
    }

    /// Writes as much queued data as the socket takes right now.
    pub fn flush(&mut self) -> io::Result<()> {
        while !self.outbuf.is_empty() {
            match self.stream.write(&self.outbuf) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(n) => {
                    self.outbuf.drain(..n);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        if self.outbuf.len() > MAX_MESSAGE * 4 {
            return Err(io::Error::other("peer is not reading"));
        }
        Ok(())
    }

    /// Returns the next complete message, if one has arrived.
    pub fn poll<T: DeserializeOwned>(&mut self) -> io::Result<Option<T>> {
        if !self.closed {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match self.stream.read(&mut buf) {
                    Ok(0) => {
                        self.closed = true;
                        break;
                    }
                    Ok(n) => self.inbuf.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
        }
        if self.inbuf.len() >= 4 {
            let len = u32::from_le_bytes(self.inbuf[..4].try_into().unwrap()) as usize;
            if len > MAX_MESSAGE {
                return Err(io::Error::new(ErrorKind::InvalidData, "message too large"));
            }
            if self.inbuf.len() >= 4 + len {
                let msg = bincode::deserialize(&self.inbuf[4..4 + len])
                    .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
                self.inbuf.drain(..4 + len);
                return Ok(Some(msg));
            }
        }
        if self.closed {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ClientMsg, PROTOCOL_VERSION};
    use std::net::TcpListener;

    #[test]
    fn round_trip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let mut client = Connection::connect(&addr, 0).unwrap();
        let (stream, _) = listener.accept().unwrap();
        let mut server = Connection::new(stream).unwrap();

        client
            .send(&ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                account: "Tester".into(),
            })
            .unwrap();
        client.send(&ClientMsg::StartAttack).unwrap();

        let mut got = Vec::new();
        for _ in 0..1000 {
            if let Some(msg) = server.poll::<ClientMsg>().unwrap() {
                got.push(msg);
                if got.len() == 2 {
                    break;
                }
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        assert!(matches!(&got[0], ClientMsg::Hello { account, .. } if account == "Tester"));
        assert!(matches!(got[1], ClientMsg::StartAttack));
    }
}

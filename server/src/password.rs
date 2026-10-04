//! Account passwords, hashed with Argon2. Only the hash is saved, as a PHC
//! string that carries its own salt and settings.

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

pub const MIN_LEN: usize = 6;
pub const MAX_LEN: usize = 64;

/// Whether a password is acceptable for a new account.
pub fn check_new(password: &str) -> Result<(), String> {
    let len = password.chars().count();
    if !(MIN_LEN..=MAX_LEN).contains(&len) || password.chars().any(char::is_control) {
        return Err(format!(
            "Passwords must be {MIN_LEN} to {MAX_LEN} characters."
        ));
    }
    Ok(())
}

/// Hashes a password with a fresh random salt.
pub fn hash(password: &str) -> String {
    hasher()
        .hash_password(password.as_bytes())
        .expect("hashing a password")
        .to_string()
}

/// Whether `password` matches a hash made by [`hash`]. A hash that can't be
/// read matches nothing.
pub fn verify(password: &str, hash: &str) -> bool {
    hasher().verify_password(password.as_bytes(), hash).is_ok()
}

#[cfg(not(test))]
fn hasher() -> Argon2<'static> {
    Argon2::default()
}

/// Tests run unoptimized, where the real settings take seconds per hash.
#[cfg(test)]
fn hasher() -> Argon2<'static> {
    let params = argon2::Params::new(256, 1, 1, None).unwrap();
    Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let h = hash("hunter22");
        assert!(h.starts_with("$argon2id$"));
        assert!(!h.contains("hunter22"));
        assert!(verify("hunter22", &h));
        assert!(!verify("hunter23", &h));
        assert!(!verify("hunter22", "not a hash"));
        assert_ne!(h, hash("hunter22"), "salted");
    }

    #[test]
    fn new_password_rules() {
        assert!(check_new("abcdef").is_ok());
        assert!(check_new("abc").is_err());
        assert!(check_new(&"x".repeat(MAX_LEN + 1)).is_err());
        assert!(check_new("abc\ndef").is_err());
    }
}

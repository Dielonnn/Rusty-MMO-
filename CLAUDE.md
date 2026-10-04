# Rusty MMO

A 3D tab-target MMO in Rust: `shared/` (game data, protocol, terrain),
`server/` (authoritative simulation + TCP server), `client/` (macroquad game).
See README.md for how it fits together.

## Checks

Before committing, run `cargo fmt --all`, `cargo clippy --workspace --all-targets`
(keep it warning-free) and `cargo test --workspace`.

## Releases

Every iteration of work that lands is a new release. The first was v1.0; each
one after bumps the major version (v2.0, v3.0, ...).

For each release:

1. Set `version` in `[workspace.package]` in the root `Cargo.toml` to `N.0.0`
   (the game shows it as `vN.0`).
2. Bump `PROTOCOL_VERSION` in `shared/src/protocol.rs` if any message changed shape.
3. Add a `## vN.0` section at the top of CHANGELOG.md (below the intro), with a
   `Tag: \`vN.0\`.` line and a bullet list of player-facing changes.
4. Commit with a message starting `vN.0: ` and a short summary.
5. Tag the commit `vN.0` and push the branch and the tag.
6. Build the Windows exes with
   `cargo build --release --workspace --target x86_64-pc-windows-gnu`
   (needs `rustup target add x86_64-pc-windows-gnu` and `gcc-mingw-w64-x86-64`)
   and hand the user `RustyMMO-vN.0.exe` and `RustyMMO-Server-vN.0.exe`, copied from
   `target/x86_64-pc-windows-gnu/release/rusty_mmo.exe` and `server.exe`.

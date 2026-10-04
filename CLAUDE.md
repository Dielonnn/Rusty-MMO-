# Rusty MMO

A 3D tab-target MMO in Rust: `shared/` (game data, protocol, terrain),
`server/` (authoritative simulation + TCP server), `client/` (macroquad game).
See README.md for how it fits together.

## Working together

Two people (each with their own Claude) work on this repository. So:

- **Never commit or push to `main`.** Start every piece of work on a new
  branch from the latest `main` (`git fetch origin main` first), and push that
  branch.
- **Never merge.** Don't merge pull requests, enable auto-merge, or merge,
  rebase or fast-forward anything into `main`. A person reviews and merges.
- Only open a pull request when the user asks for one.
- Only push to branches you created in this session. Never force-push, rebase
  or rewrite someone else's branch.
- Before starting, check what's new on `main` and on open branches, so you
  don't redo or clash with the other person's work.

## Checks

Before committing, run `cargo fmt --all`, `cargo clippy --workspace --all-targets`
(keep it warning-free) and `cargo test --workspace`.

## Releases

Every iteration of work that lands is a new release. The first was v1.0; each
one after bumps the major version (v2.0, v3.0, ...).

The version bump and changelog entry go in the branch with the work. Two
branches can claim the same number: when you start, and again before you push,
check `main`, and if `main` already has your number, take the next one.

For each release:

1. Set `version` in `[workspace.package]` in the root `Cargo.toml` to `N.0.0`
   (the game shows it as `vN.0`).
2. Bump `PROTOCOL_VERSION` in `shared/src/protocol.rs` if any message changed shape.
3. Add a `## vN.0` section at the top of CHANGELOG.md (below the intro), with a
   `Commit: the one titled "vN.0: ...".` line and a bullet list of
   player-facing changes.
4. Commit to the work branch with a message starting `vN.0: ` and a short
   summary, and push the branch (never `main`).
5. Don't tag. Tags go on `main` after a person merges, so tell the user which
   commit to tag once it's merged.
6. Build the Windows exes with
   `cargo build --release --workspace --target x86_64-pc-windows-gnu`
   (needs `rustup target add x86_64-pc-windows-gnu` and `gcc-mingw-w64-x86-64`)
   and hand the user `RustyMMO-vN.0.exe` and `RustyMMO-Server-vN.0.exe`, copied from
   `target/x86_64-pc-windows-gnu/release/rusty_mmo.exe` and `server.exe`.
   Say they're built from the branch and not merged yet.

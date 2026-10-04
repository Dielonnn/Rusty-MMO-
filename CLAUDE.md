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
- **Always open a pull request** into `main` when the work is done and pushed,
  and post its link. That's how the work reaches `main`: a person reviews it
  and clicks Merge.
- Only push to branches you created in this session. Never force-push, rebase
  or rewrite someone else's branch.
- Before starting, check what's new on `main` and on open branches, so you
  don't redo or clash with the other person's work.
- **Before you push, catch up with `main`.** Run `git fetch origin main`. If
  `main` has moved, merge it into your branch (`git merge origin/main`, never
  rebase). If that brought in a new CLAUDE.md, re-read it and follow the new
  rules. Then rerun the checks, and push only if they pass.

## Checks

Before committing, run `cargo fmt --all`, `cargo clippy --workspace --all-targets`
(keep it warning-free) and `cargo test --workspace`.

GitHub CI (`.github/workflows/ci.yml`) runs the same checks on every pull request
and on `main`. Run them locally anyway so a broken branch never gets pushed,
and if CI goes red on your pull request, fix it before handing it over.

## Releases

Every iteration of work that lands is a new release, numbered `vN.M`. The first
was v1.0.

- A **major** release (v8.0, v9.0, ...) is a big step: a milestone of the
  graphics plan or a major new system (parties, dungeons, a new zone). It bumps
  N and resets M to 0.
- A **minor** release (v7.1, v7.2, ...) is anything smaller: a fix, a tweak, a
  few new items or recipes, one modest feature. It bumps M.

If it's unclear which one a piece of work is, ask the person you're working
with.

The version bump and changelog entry go in the branch with the work. Two
branches can claim the same number: when you start, and again before you push,
check `main`, and if `main` already has your number, take the next one of the
same kind (v7.1 taken: use v7.2; v8.0 taken: use v9.0). A minor release always
builds on the latest major on `main`: if `main` moved from v7.x to v8.0 while
you worked, a minor release becomes v8.1.

For each release:

1. Set `version` in `[workspace.package]` in the root `Cargo.toml` to `N.M.0`
   (the game shows it as `vN.M`).
2. Bump `PROTOCOL_VERSION` in `shared/src/protocol.rs` if any message changed shape.
3. Add a `## vN.M` section at the top of CHANGELOG.md (below the intro), with a
   `Commit: the one titled "vN.M: ...".` line and a bullet list of
   player-facing changes.
4. Commit to the work branch with a message starting `vN.M: ` and a short
   summary, and push the branch (never `main`).
5. Don't tag. Tags go on `main` after a person merges, so tell the user which
   commit to tag once it's merged.
6. Build the Windows exes with
   `cargo build --release --workspace --target x86_64-pc-windows-gnu`
   (needs `rustup target add x86_64-pc-windows-gnu` and `gcc-mingw-w64-x86-64`)
   and hand the user `RustyMMO-vN.M.exe` and `RustyMMO-Server-vN.M.exe`, copied from
   `target/x86_64-pc-windows-gnu/release/rusty_mmo.exe` and `server.exe`.
   Say they're built from the branch and not merged yet.

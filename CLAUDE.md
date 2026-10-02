# Working on MOTIX (notes for Claude sessions)

Read these first: `STATUS.md` (where things stand, what to do next), then `ARCHITECTURE.md`,
`DECISIONS.md`, `PHASES.md`. The owner (SlothGod1) isn't a programmer: explain in plain language,
never show milestone codes or jargon in the app's UI, and ask before big product decisions.

## Delivering work
**Now (ADR-036, GitHub set aside):** Claude builds and tests in its sandbox, writes the changed files into the
owner's clone at `B:\Family\Andrew Cardone\Creation\Editing Software\motix` through the file bridge, stages them
back and `cmp`s them, then writes `motix\.motix-build-ready` (its text = release notes). The owner's server
builds, signs and publishes the update within a few minutes; read `Editing Software\MOTIX build status.txt`
(and `MOTIX build log.txt` on failure) to confirm. No GitHub Desktop or screen control needed. Bridge limits:
it can't write inside `.github\` or delete files, and it adds C2PA metadata to media files (MP4, MOV, M4A,
WAV, PNG, JPEG) — never deliver byte-exact media through it.

**Later (back on GitHub):** commit + push from GitHub Desktop (ADR-030), or a Claude Code session with the repo
attached pushing `claude/...` branches (ADR-029); CI tests, builds and publishes signed releases.

Either way, only deliver finished, locally-verified work. Before delivering run:
`cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check`

## Conventions
- Rust 1.95 (pinned), edition 2024, clippy pedantic clean, `#![forbid(unsafe_code)]` outside platform/FFI crates.
- Every user-visible command is an `Action` in `crates/motix-app/src/actions.rs`; edits go through
  `AppState` methods so undo works.
- UI tests: `crates/motix-ui/tests/ui.rs` (egui_kittest; use `with_step_dt(1/60)`, `with_max_steps(120)`).
  Screenshots: `MOTIX_SCREENSHOT_DIR=… cargo test -p motix-ui -- --ignored` (needs a GPU or Mesa).
- Never commit secrets. The update-signing key lives only in the `MOTIX_UPDATE_SIGNING_KEY` Actions secret and the
  build server's private `%LOCALAPPDATA%\MOTIX-build` folder (ADR-036); never read, print or move it.
- Docs are part of the change: update REQUIREMENTS/DECISIONS/PHASES/STATUS with each feature.

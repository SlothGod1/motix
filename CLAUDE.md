# Working on MOTIX (notes for Claude sessions)

Read these first: `STATUS.md` (where things stand, what to do next), then `ARCHITECTURE.md`,
`DECISIONS.md`, `PHASES.md`. The owner (SlothGod1) isn't a programmer: explain in plain language,
never show milestone codes or jargon in the app's UI, and ask before big product decisions.

## Delivering work
Two routes, both ending in the same automatic CI → signed release (ADR-027, ADR-029):

- **Cowork chat with the owner (main route, ADR-030):** Claude builds and tests in its sandbox,
  then writes changed files into the owner's GitHub Desktop clone at
  `B:\Family\Andrew Cardone\Creation\Editing Software\motix` through the file bridge. The owner
  commits and clicks **Push origin** in GitHub Desktop. The bridge can't write into `.github\`: put
  workflow changes in `Editing Software\_github\` and ask the owner to copy them over. It can't delete
  files either: list deletions for the owner. It adds C2PA metadata to media files (MP4, MOV, M4A,
  WAV, PNG, JPEG) in transit, so never deliver byte-exact media through it. Before asking the
  owner to push, list the clone (`device_list_dir`) and check every file you meant to write arrived
  (a missing `Cargo.toml` once broke the build).
- **Claude Code session with the repo attached:** push only to the session's working branch
  (`claude/...`); CI's `promote` job fast-forwards `main` when every check passes.

Either way, only deliver finished, locally-verified work. Before delivering run:
`cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check`

## Conventions
- Rust 1.95 (pinned), edition 2024, clippy pedantic clean, `#![forbid(unsafe_code)]` outside platform/FFI crates.
- Every user-visible command is an `Action` in `crates/motix-app/src/actions.rs`; edits go through
  `AppState` methods so undo works.
- UI tests: `crates/motix-ui/tests/ui.rs` (egui_kittest; use `with_step_dt(1/60)`, `with_max_steps(120)`).
  Screenshots: `MOTIX_SCREENSHOT_DIR=… cargo test -p motix-ui -- --ignored` (needs a GPU or Mesa).
- Never commit secrets. The update-signing key lives only in the `MOTIX_UPDATE_SIGNING_KEY` Actions secret.
- Docs are part of the change: update REQUIREMENTS/DECISIONS/PHASES/STATUS with each feature.

# Development Guide

> Developer preview. What exists today: the Cargo workspace, `motix-core`, `motix-probe`, `motix-app`, `motix-ui`, `apps/motix`, CI workflows, `deny.toml`.
> Commands marked *(planned)* don't exist yet.
> Note the distinction: **developers** install toolchains; **users** never do (ADR-017).

## 0. Getting started today

1. Install Rust with [rustup](https://rustup.rs). The pinned version in `rust-toolchain.toml` installs itself on first use.
2. From the repository root:
   ```
   cargo test --workspace      # all tests (116 today)
   cargo clippy --workspace --all-targets
   cargo fmt --all
   cargo run -p motix          # open the MOTIX window
   cargo test -p motix-ui -- --ignored   # render UI screenshots to target/screenshots (needs a GPU)
   ```
3. Optional: `cargo install cargo-deny` then `cargo deny check` (the same license/advisory policy CI enforces).
4. Release helpers: `cargo run -p xtask -- keygen <file>` (new update-signing key pair), `manifest <version> <files…>`, `sign <file>` (reads `MOTIX_UPDATE_SIGNING_KEY`). CI uses these to publish a signed preview release for every green push to `main` (ADR-027). Local builds are "developer builds" (`0.1.0-dev`) and never update themselves; set `MOTIX_VERSION` at build time to stamp a version.

CI (`.github/workflows/ci.yml`) runs on every push to GitHub and publishes a Windows and a Linux preview build as downloadable artifacts.

> **Note for the local copy:** the Claude desktop bridge can't write into folders named `.github`, so in the
> Editing Software folder these files live in `_github/`. Rename `_github` → `.github` before the first push.

## 1. Toolchain

| Tool | Version policy | Why |
|------|---------------|-----|
| Rust | Pinned in `rust-toolchain.toml` (stable); bumped deliberately | Reproducible builds |
| Windows: Visual Studio Build Tools (MSVC, Windows SDK) | Latest supported | Linking, Windows APIs |
| Linux: GCC/Clang, pkg-config, Wayland/X11/ALSA/PipeWire dev headers | Distro LTS | Building winit/cpal |
| NASM, CMake, Meson/Ninja | Pinned in CI | Building FFmpeg, dav1d, SVT-AV1, aws-lc |
| Inno Setup (Windows packaging only) | Pinned | Installer |
| Python 3 (optional) | Any | *Only* for dev scripts like color reference generation. Never shipped. |
| macOS: Xcode command-line tools | From Phase 13 | Not needed until the macOS phase (OD-8). |

`cargo xtask doctor` *(planned)* checks the machine and prints exactly what's missing.

## 2. Repository layout

```
/                       docs (this file, ARCHITECTURE.md, ...)
crates/                 library crates (see ARCHITECTURE §4.2)
apps/                   binaries: motix, motix-launcher, motix-server, motix-media-worker, motix-ai-worker
spikes/                 Phase 0 experiments (throwaway, not linked into apps)
xtask/                  build/package/release automation (Rust, no shell scripts where avoidable)
third_party/            pinned source manifests + hashes for FFmpeg etc. (not the sources themselves)
assets/                 fonts (OFL), icons, presets, templates
tests/                  cross-crate integration tests, media fixtures manifest
fuzz/                   cargo-fuzz targets
packaging/              Inno Setup scripts, AppImage/Flatpak manifests, systemd units, Dockerfile
.github/workflows/      ci.yml, release.yml, nightly.yml, audit.yml, tuf-timestamp.yml
```

## 3. Common commands *(planned)*

```
cargo xtask doctor            # check toolchain
cargo xtask deps              # fetch/build pinned FFmpeg & C deps (cached)
cargo run -p motix           # run the client (dev)
cargo run -p motix-server    # run the server (dev, temp data-dir)
cargo test --workspace        # unit + integration tests
cargo xtask test-render       # golden-image render tests (software GPU)
cargo xtask fuzz <target>     # run a fuzz target
cargo xtask package           # build installers/AppImage locally
cargo deny check              # licenses, advisories, bans
```

## 4. Engineering conventions

- **Crate boundaries** follow ARCHITECTURE §4.2; dependency arrows point down only. `motix-ui` is the only crate that may depend on egui. Crates are created when their phase starts, not before.
- **Mac-ready code** (OD-8): no Windows- or Linux-only APIs outside `motix-platform`; prefer crates that already support macOS.
- **Crypto** lives only in `motix-identity` and `motix-e2ee`, uses reviewed libraries (OpenMLS, RustCrypto/dalek, rustls), and gets a second review on every change.
- **`unsafe`** is allowed only in `motix-platform`, FFI wrapper modules and GPU interop, each block with a `// SAFETY:` comment; `#![forbid(unsafe_code)]` everywhere else.
- **Errors:** `thiserror` in libraries, `anyhow`-style only at binary edges. User-facing errors are translated to plain language in `motix-app` (never show raw error chains to users; keep them in logs).
- **No panics across boundaries:** worker IPC, network handlers and file loaders return errors; `panic = "unwind"` in the main app with a top-level handler that saves state.
- **Limits are constants** in `motix-core::limits` (max dimensions, max message size, max nesting depth…), used by validators and tests.
- **Time** is always `motix_core::Time` (flicks) — no `f64` seconds in the model.
- **Secrets** use `Secret<T>`; never `Debug`-print keys.
- **Logging:** `tracing` with subsystem targets; no project content above `debug`.
- **Docs are part of the change:** a PR that changes architecture, a decision, a dependency or a requirement updates the matching doc in the same PR.

## 5. Adding a dependency

A PR adding a crate or C library must state: purpose, license (and that `cargo deny` passes), maintenance status (recent releases, maintainers), size/compile-time impact, security surface (does it parse untrusted input? `unsafe`?), and alternatives considered. Then add a row to [LICENSING_AND_COSTS.md](LICENSING_AND_COSTS.md).

## 6. Testing strategy

| Layer | Tooling | Runs |
|-------|---------|------|
| Unit | `cargo test` | Every PR |
| Property / convergence | `proptest` (commands, CRDT convergence, time math) | Every PR |
| Integration | Workspace tests with fixtures | Every PR |
| Render regression | Golden images on WARP (Win) / lavapipe (Linux), tolerance-based | Every PR |
| Media / codec | Fixture corpus (generated + licensed samples), decode/encode/round-trip | Every PR (small set), nightly (full) |
| Crash recovery | Randomized kill tests, fault-injecting SQLite VFS | Nightly |
| Networking | In-process simulated network (latency/loss/partition), then real iroh on loopback | Every PR / nightly |
| Security | Fuzzing (cargo-fuzz), TUF attack vectors, authz matrix tests | Nightly + before release |
| Update / rollback | VM-based install→update→rollback | Before release |
| Migration | Open fixture projects from every released format version | Every PR |
| Performance | Benchmarks (criterion) with thresholds; perf HUD captures | Nightly, tracked over time |
| Cross-platform | Windows + Linux runners | Every PR |
| Manual hardware matrix | NVIDIA / AMD / Intel GPUs, HDR display, Wayland/X11 | Before release |

Media fixtures with unclear licenses are never committed; the fixture manifest downloads only files with known, redistributable licenses or generates them. Today's fixtures (`crates/motix-probe/tests/fixtures`, ~230 KB) are synthetic FFmpeg test patterns made by `generate.sh`.

## 7. Branching and releases

- `main` is always releasable; feature branches + PRs; squash merge.
- Conventional commit messages (`feat:`, `fix:`, `docs:`…) → changelog generation.
- Versions: SemVer; tags `vX.Y.Z` trigger the release workflow; release requires owner approval in the `release` environment.
- Nightly builds from `main` publish to the `nightly` channel automatically.

## 8. Security practices for contributors

- 2FA required; signed commits encouraged, signed tags required for releases.
- **Developer Certificate of Origin**: every commit is signed off (`git commit -s`), certifying you have the right to contribute it under GPL-3.0-or-later. No CLA.
- Never commit secrets; CI secrets live in GitHub environments with required reviewers.
- Report vulnerabilities privately (see [SECURITY.md](SECURITY.md)).

## 9. Definition of done (per PR)

- [ ] Tests added/updated and passing on Windows and Linux
- [ ] `cargo clippy -D warnings`, `cargo fmt`, `cargo deny` clean
- [ ] Docs updated (ARCHITECTURE / DECISIONS / REQUIREMENTS / LICENSING as relevant)
- [ ] No new `unsafe` outside allowed crates
- [ ] User-facing text is plain language
- [ ] Security considerations noted in the PR description when touching untrusted input, networking, updates or keys

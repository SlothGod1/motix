# Development Phases

Rules:
- Every phase ends with something **testable by a person**, not just code.
- A phase is complete only when its completion criteria pass, docs are updated, and the security checklist is done.
- Later phases are intentionally less detailed; they are refined when the previous phase completes.
- Scope changes go through the owner. Ideas go to [IDEAS.md](IDEAS.md), not into the current phase.

| Phase | Name | One-line outcome |
|-------|------|------------------|
| 0 | Foundations & core spikes | Repo, CI, and the three experiments the app is built on (UI, media, project model) |
| 1 | Local core | Open media, cut it on a timeline, preview with sound, export an MP4 — never lose work |
| 2 | Distribution & updates | Real installers; signed, verified, rollback-safe auto-updates |
| 3 | Editing essentials | A social-video editor a beginner can actually use |
| 4 | Temporary collaboration | "Start Collaboration Session" → invite → edit together, P2P |
| 5 | Persistent server | Self-hosted MOTIX Server with ACLs, revisions, backups, migration |
| 6 | Motion graphics & compositing | Compositions, masks, blend modes, parenting, graph editor, color tools |
| 7 | Color management & HDR | HDR10/HLG in and out, accurate HDR/SDR preview, scopes |
| 8 | Professional audio | Mixer, buses, DSP, loudness, surround |
| 9 | Local AI | Captions, cleanup, detection, background removal, auto-reframe |
| 10 | Advanced effects | Optical flow, tracking, stabilization, stylization, glow, distortion |
| 11 | Extensibility & interchange | WASM plugins, scripting, OTIO/FCPXML/XML |
| 12 | Hardening & 1.0 | Sandboxing, localization, MSI, performance, security audit |
| 13 | macOS | Signed, notarized Mac client and server at feature parity (can move earlier — owner's call, OD-8) |

Phases 6–10 can be reordered by the owner based on user feedback; 0–5 are sequential because each builds infrastructure the next needs. Phase 13 (macOS) can be scheduled any time after Phase 2.

**Current status (2026-09-27):** Phase 0 → M1 in progress, with part of M2's timeline pulled forward at the owner's request.
- Done (preview 1): workspace, CI, `motix-core`; UI shell (`motix-app`, `motix-ui`, `apps/motix`): dockable Media / Viewer / Inspector / Timeline panels, dark theme, safe-area guides, frame-accurate playhead, transport and timecode, media bin with drag-and-drop and file picker, menus, shortcuts, command palette, accessibility labels. First Windows build from GitHub Actions ✅.
- Done (preview 2, owner feedback): `motix-probe` (instant, memory-safe inspection of MP4/MOV/MKV/WebM/WAV/PNG/JPEG — size, rotation, fps, VFR, duration, codecs, bit depth, HDR10/HLG/Dolby Vision flags, audio streams); **unlimited tracks** created automatically; **linked video + audio on import** (unlink/relink); drag from bin to timeline or double-click; move clips with frame + edge snapping; split, delete, add/remove/mute tracks; **undo/redo for everything**; project size typed in **pixels** and frame rate typed freely; **"match project to this video?"** on the first clip; per-clip **fit/fill/stretch/original** placement shown in the viewer; colour output (SDR/HDR10/HLG) and bit depth settings; no milestone codes in the UI. 82 automated tests plus a screenshot test.
- Done (preview 3, owner feedback): **signed automatic updates** (checks GitHub at start-up and every 10 minutes, downloads in the background, verifies the Ed25519 signature and checksum, "Restart now / Later", Help > Check for updates, auto-check switch) and a CI job that publishes a signed preview release for every green push; **tracks in the order media is added, named after files** ("Audio of …" right below each video), rename/reorder/lock/solo/hide; **Resolve-style timeline** (big timecode, Select/Blade tools, snapping and linked-selection toggles, edge trimming with linked clips, markers with colours, Up/Down edit jumps, track height). 108 automated tests.
- Done (owner request): **run MOTIX from a shared network folder** (Help > Share MOTIX on your network…); each new version is installed there once for every PC without disturbing PCs that have it open, from GitHub or a signed release copied into the folder's `updates` inbox (ADR-031). Fixed: the bottom of the Updates window couldn't be clicked after it grew. 116 automated tests.
- Done (owner request): **save and open projects** (Ctrl+S / Ctrl+Shift+S / Ctrl+O / Ctrl+N, "save changes?" prompts, `.bak` copy, crash-recovery copy every minute offered on the next start, open by double-click or drag-and-drop; ADR-032) and the owner-only **Creator Lab** tab with password lock and the example library for future AI features (ADR-033). 127 automated tests.
- Done (owner request): **real pictures and sound** in the viewer — exact frames while scrubbing, smooth playback, HDR tone-mapped, mixed audio with mute/solo; on Windows the free FFmpeg helper is downloaded once with one click (ADR-034). 138 automated tests.
- Done (owner request): **updates from the owner's server** while GitHub is set aside — the server builds and signs MOTIX and publishes to a network folder; own copies can pick that folder in the Updates window (ADR-036). 140 automated tests.
- Remaining for M1: layer compositing (more than the top picture), proxies for heavy footage, the libav worker of ADR-005. Then M2: export; then the effects/transitions engine and the Creator Lab's Scene Library and Effect Studio (ADR-033).

---

## Delivery order: core first, collaboration-ready underneath (OD-9)

The owner's rule: **the editor must work on its own before any collaboration ships**, but collaboration must never
require rebuilding the core. So Phases 0–3 build a complete single-user editor, and the pieces collaboration needs
are built *into* that core quietly (the "background track"). Phases 4–5 then add networking on top.

### Milestones you can see

| Milestone | Phase | What you can do | Rough effort after the previous milestone* |
|-----------|-------|-----------------|---------------------------------------------|
| **M0 · Unblocked** | 0 | — (setup: public GitHub repo + package downloads allowed) | ~15 minutes of owner setup |
| **M1 · First window** | 0 → 1 | Download a Windows zip from GitHub, open the app: dark UI with dockable panels, drag in a video, watch it play with sound. Also runs the HDR and GPU checks on your real hardware. | 2–4 working sessions |
| **M2 · First edit** | 1 | Timeline with cut, trim, split, undo; nothing lost if it crashes; export a vertical MP4. **First genuinely usable build.** | 6–12 sessions |
| **M3 · Real install** | 2 | One-click installer; the app updates itself securely and rolls back if an update breaks. | 3–6 sessions |
| **M4 · Beginner alpha** | 3 | Text, captions, keyframes, transitions, presets, proxies — a first-timer can make a TikTok without help. | 10–20 sessions |
| M5 · Edit together | 4 | Temporary end-to-end encrypted sessions. | after M4 |
| M6 · Your own server | 5 | Persistent self-hosted server. | after M5 |

\* A "working session" is one conversation like this one. These are honest ranges, not promises: they depend on how
quickly builds can be tested on your hardware, and will be re-estimated after M1.

### Background track — collaboration foundations built during Phases 1–3 (invisible to users)

| Foundation | Built in | Why it prevents a rebuild later |
|------------|----------|---------------------------------|
| Project file is a CRDT change journal (Loro) from day one | Phase 1 | Every local edit is already a mergeable change; sync later just ships these changes. |
| Every change carries an author/device ID (a local key made silently on first run; no UI, no network) | Phase 1 | Signing and permissions (ADR-022) plug into data that already exists. |
| All edits go through commands (`motix-commands`) | Phase 1 | Local UI, undo and future remote edits use one path. |
| Every change is validated regardless of source | Phase 1 | Today it protects against bad files; later the same code protects against bad peers. |
| Deterministic conflict normalization + **"two simulated editors" merge tests in CI** | Phase 1–3 | Proves concurrent editing works long before any network exists. |
| Media referenced by content hash, never by path alone | Phase 1 | Collaborators can find or fetch the same media later. |
| Selection, playhead and panels kept out of the project document | Phase 1 | They become "presence" in collaboration without touching the model. |
| Changes are opaque, sign-able, encrypt-able blobs (checked in S0-CRDT) | Phase 0 | MLS encryption wraps them without changing the format. |
| Networking/encryption spikes (S0-NET, S0-E2EE) | Run during Phase 3, off the critical path | De-risk Phase 4 without delaying any core milestone. |

**Not built until Phase 4:** network code, invites, relays, MLS encryption, the server. Core builds ship with no
networking except the update check — nothing to secure prematurely, nothing to slow the core down.

---

## Phase 0 — Foundations & spikes

**Objective:** Create the engineering foundation and retire the risks the core app is built on, with small experiments before committing to them. Only spikes that block the core app run here; the rest run later off the critical path.

**Features (deliverables):**
1. Cargo workspace (crates are added when their phase starts), `xtask`, rustfmt/clippy config, `deny.toml`, GPL-3.0 `LICENSE`, PR template with DCO. ✅ *Done except `xtask` (added with the FFmpeg build in S0-MEDIA).*
2. GitHub Actions: build + test on Windows and Linux for every PR; cargo-deny (licenses, advisories, bans, sources — also run daily), clippy (deny warnings), fmt check; Actions pinned by SHA. ✅ *Written; runs once the repo is on GitHub.*
3. `motix-core`: flicks time type, time ranges, exact rationals, frame and sample rates, limits — with exhaustive tests (first real code). ✅ *Done: 27 tests incl. 20,000-case randomized properties; IDs (UUIDv7) arrive with `motix-schema` in Phase 1.*
4. **M0 — unblock** (owner): public GitHub repository (free CI builds for Windows/Linux, required anyway for free code signing) and package downloads allowed for the build environment (`index.crates.io`, `static.crates.io`, `crates.io`).
5. **Spikes** (each in `spikes/<name>/`, each ending with a short written result appended to DECISIONS.md). **Core-blocking** spikes run now; the others are scheduled where marked:

| Spike | Question | Pass criteria |
|-------|----------|---------------|
| S0-UI *(core-blocking; becomes M1)* | Is egui on our own wgpu surface good enough? (ADR-003) | The 7 criteria in ADR-003 |
| S0-HDR *(folded into M1, run on the owner's display)* | Can we present scRGB/PQ on Windows HDR with correct SDR white? | Side-by-side with a reference SDR player matches; HDR test pattern shows > 203 nits values |
| S0-MEDIA *(core-blocking; becomes M1)* | FFmpeg built in CI (GPL config), decode in a worker process, frames via shared memory | 4K60 8-bit H.264 decode → GPU texture at ≥ 60 fps; overhead vs in-process < 10%; worker crash doesn't crash host |
| S0-CRDT *(core-blocking)* | Does Loro + a typed schema model a timeline well? Are changes opaque blobs that can later be signed and encrypted? (ADR-007, ADR-022) | Spike criteria in ADR-007; each change exportable as bytes with author ID, importable after a byte-level round trip |
| S0-NET *(background, during Phase 3)* | iroh connectivity across real networks + self-hosted relay | Direct connection between two home networks; relay fallback on a blocked-UDP network; LAN-only works offline |
| S0-UPD *(start of Phase 2)* | TUF (tough) + side-by-side launcher + rollback; Velopack comparison | Fake release: update, forced crash → rollback, expired metadata rejected, tampered file rejected |
| S0-E2EE *(background, during Phase 3)* | OpenMLS (X-Wing hybrid ciphersuite) + Loro updates: encrypt, sign, verify, add/remove member, new-member snapshot, recovery kit (ADR-022) | 3 devices + untrusted relay store converge; removed device can't decrypt; forged/altered update rejected by all; per-update overhead < 1 KB and < 1 ms; PQ iroh handshake works |

**Dependencies:** OD-1 decided (GPL-3.0 → GPL FFmpeg build with x264/x265); OD-2 decided by S0-UI's outcome.

**Architecture touched:** all top-level boundaries (crate graph, worker IPC, surface ownership, doc model, transport, updater).

**Tests:** unit tests for `motix-core`; each spike has a scripted, repeatable demo; CI green on both OSes.

**Completion criteria:**
- Core-blocking spikes (S0-UI, S0-MEDIA, S0-CRDT) and S0-HDR have written outcomes; ADR-003, 005, 007 moved to Accepted or revised. (S0-UPD, S0-NET, S0-E2EE complete in their scheduled phases.)
- OD-2 decided (OD-1 already decided: GPL-3.0-or-later).
- CI runs on every PR on Windows and Linux.

**Known limitations:** The M1 build is a developer preview (unsigned zip; Windows may show "Windows protected your PC" → *More info* → *Run anyway*). Spike code is reviewed before it becomes production code.

---

## Phase 1 — Local core ("open, cut, play, export, never lose work")

**Objective:** A developer-build editor that does the fundamental loop reliably and correctly — ends at **M2 · First edit**. Includes the Phase 1 items of the background track above.

**Features:**
- Project create/open/save as `.motix` (SQLite + Loro journal); continuous autosave; crash recovery; rolling backups; named versions; format versioning with migration framework.
- Media import via worker processes (probe, thumbnails, waveforms); content hashing; relink.
- Single composition: 2 video tracks + 2 audio tracks; add, move, trim, split, delete, ripple delete, snapping; undo/redo.
- Viewer with real-time playback, audio sync, adaptive resolution; frame cache.
- Render graph (sources, transform, opacity, over-blend) in scene-linear RGBA16F; SDR Rec.709/sRGB color path with correct tags.
- Export MP4 (H.264 via x264 "Best quality" or hardware "Fast"; AAC) and WebM (AV1/VP9 + Opus); 3 presets (Vertical 1080×1920, Square, 1080p landscape).
- UI shell: dark theme, docking, command palette, shortcuts, drag-and-drop import, context menus, tooltips, accessibility for standard widgets.
- GPU/driver detection with CPU fallback path; logging with redaction; local crash reports.

**Dependencies:** Phase 0 outcomes; OD-1 (GPL FFmpeg configuration); OD-5 direction (x264/x265 bundled in dev/alpha builds).

**Architecture:** `motix-core`, `motix-schema`, `motix-project`, `motix-commands`, `motix-media`, `motix-media-worker`, `motix-render` (basic nodes), `motix-color` (SDR), `motix-audio` (playback + mix), `motix-playback`, `motix-app`, `motix-ui`, `motix-platform` (paths, GPU info).

**Tests:**
- Unit: time math, schema validation, commands, migrations (fixture per format version).
- **Background-track tests:** two (and five) simulated editors apply random concurrent commands to copies of one project, exchange change blobs in random order, and must converge to identical, valid documents — no network involved.
- Crash-recovery tests: kill the process at random points during edits (automated, hundreds of iterations) → project always reopens with ≤ 1 s loss.
- Power-loss simulation for SQLite journal (fault-injecting VFS).
- Render regression: golden images compared with tolerance on WARP (Windows) / lavapipe (Linux) in CI.
- Media fixtures: VFR phone video, rotated video, 10-bit, odd frame rates (23.976, 29.97, 59.94), mono/stereo/5.1 audio, broken files.
- Color correctness: export → re-import → compare against reference values (ΔE thresholds); tag checks with ffprobe-equivalent.
- A/V sync test: generated beep/flash media, measure drift after export (≤ 1 audio sample, 0 frames).
- Fuzzing: project loader, worker IPC messages.

**Completion criteria:**
- A non-developer can build a 60-second vertical video from 5 phone clips and export it, on Windows and Linux.
- No data loss in 1,000 randomized kill tests.
- Performance targets R-PRF-1, R-PRF-3, R-PRF-6 met (R-PRF-2 may use proxies — proxies arrive in Phase 3; document status).
- Security checklist for Phase 1 (SECURITY.md §4 rows marked phase 1) passes.

**Known limitations:** No installer/updater (dev builds). No text, keyframes, effects, proxies, HDR. Workers are process-isolated but not yet OS-sandboxed.

---

## Phase 2 — Distribution & updates

**Objective:** Anyone can install MOTIX with one download, and it updates itself securely.

**Features:** Inno Setup per-user installer; portable zip; AppImage; launcher with side-by-side versions; TUF repo on GitHub Pages; release workflow (build → test → package → sign → approve → publish); channels (stable/beta/nightly); update preferences UI; health check + automatic rollback; staged rollout; SBOM and provenance attestations; Authenticode signing (per OD-3); file association and `motix://` URL handler; uninstall behavior (R-INS-5).

**Dependencies:** OD-3 (signing route); S0-UPD spike (runs first in this phase); TUF root key ceremony (owner).

**Architecture:** `motix-update`, `motix-launcher`, `motix-platform` (installer integration), `xtask release`, `.github/workflows/release.yml`.

**Tests:** TUF attack test vectors (SECURITY.md §4); install/upgrade/rollback/uninstall in clean Windows 10, Windows 11, Ubuntu LTS and Fedora VMs; interrupted download/power loss during staging; update of the launcher itself; downgrade protection; metadata expiry.

**Completion criteria:** A clean VM installs from the GitHub release, receives an update to the next version automatically, and survives a deliberately broken release by rolling back — on Windows and Linux.

**Known limitations:** SmartScreen warnings until signing reputation builds; server packaging not yet (Phase 5).

---

## Phase 3 — Editing essentials (first public alpha)

**Objective:** A social-video editor that a beginner can use without a manual (R-UX-1).

**Features:** unlimited tracks; ripple/roll/slip/slide; markers; keyframes (transform, opacity, volume) with easing; text layers and styled captions (manual entry, SRT import/export); transitions (cross dissolve, dip, slide, zoom, wipe); speed change and freeze frame; social presets and safe areas; automatic proxies; render-ahead cache with cache bar; export queue; social templates; Simple vs Advanced timeline views; welcome screen with "New vertical video" templates. **Ends at M4 · Beginner alpha.**

**Background (off the critical path):** S0-NET and S0-E2EE spikes, so Phase 4 starts with its riskiest questions already answered.

**Dependencies:** Phases 1–2. vello/cosmic-text integration (ADR-019).

**Architecture:** `motix-text`, expanded `motix-render` nodes (effect interface v1, versioned), proxy manager, template format.

**Tests:** usability test with 5 non-technical users (task: R-UX-1) — record where they get stuck; text shaping tests (Arabic, Hindi, CJK, emoji ZWJ sequences); keyframe interpolation golden tests; proxy/original parity tests on export.

**Completion criteria:** 4 of 5 test users complete R-UX-1 unaided within 15 minutes; R-PRF-2 and R-PRF-4 met.

**Known limitations:** No collaboration, compositing, HDR, AI.

---

## Phase 4 — Temporary collaboration (P2P)

**Objective:** Two or more people edit the same project live with zero network setup — end-to-end encrypted.

**Features:** identity creation, backup/restore, second-device link, safety codes; MLS group per session with hybrid post-quantum ciphersuite; signed, encrypted updates verified by every client; key rotation on removal; "Start Collaboration Session"; invite link/QR with expiry, single-use, role, host approval; join flow; host-authority sync; presence (avatars, selections, playheads); offline/reconnect; media request (proxy first); conflict indicators and soft locks; "save a copy" permission; end session.

**Dependencies:** Phase 3; S0-NET, S0-CRDT and S0-E2EE results; OD-4 (relay choice).

**Architecture:** `motix-identity`, `motix-e2ee`, `motix-net`, `motix-sync` (authority + peer roles), `motix/session/1` and `motix/blobs/1` protocols.

**Tests:** network simulation (latency, loss, reordering, partitions) with deterministic seeds; convergence property tests (random concurrent command sequences on N peers → identical documents); authentication/authorization/replay/malformed-packet tests (SECURITY.md §4); real-world test across two home networks, a mobile hotspot (CGNAT) and a UDP-blocked network.

**Completion criteria:** 4 people edit one project for 30 minutes across different networks with no divergence, no lost edits, and sub-250 ms remote edit latency on direct connections; a packet capture at the relay and the host's forwarded data contain no plaintext project content; all E2EE tests in SECURITY.md §4 pass.

**Known limitations:** Session ends when host leaves; no asynchronous collaboration (Phase 5).

---

## Phase 5 — Persistent self-hosted server

**Objective:** A team can run an always-on MOTIX Server at home, on a NAS or VPS, and move it between machines — and the server never needs to be trusted with project content.

**Features:** `motix-server` binary; Windows service installer; .deb/.rpm; tar.gz; OCI image + Unraid template; claim-code setup; local admin page; server identity + client pinning; MLS Delivery Service + KeyPackage directory; signed role logs; encrypted project hosting with revisions and retention; encrypted media store; recovery kits; opt-in per-project Helper (proxies/thumbnails/renders); embedded relay; scheduled/manual backups; export/import/migration; audit log; rate limiting; firewall helper with consent; server update policies with maintenance mode and rollback; client "Servers" panel and admin UI.

**Dependencies:** Phase 4.

**Architecture:** ARCHITECTURE §14; `motix-update` server mode.

**Tests:** migration test (Windows → Linux container, new IP) with clients reconnecting to the same Server ID; backup verify/restore; load and abuse tests; privilege-escalation tests; update with DB migration + forced failure → rollback without data loss; 7-day soak test.

**Completion criteria:** A non-expert installs the server on a spare PC following only on-screen instructions, invites two people, and later migrates it to another machine without users re-joining.

**Known limitations:** Server-side proxies/renders only for projects where the owner enabled Helper. Metadata (membership, sizes, timing) visible to the server.

---

## Phase 6 — Motion graphics & compositing
**Objective:** Layer-based motion design for social content. **Features:** compositions/precomps (nested), masks (bezier, feather, expansion), blend modes, track mattes, parenting, null/adjustment layers, shape layers, graph editor, motion presets, basic color correction (wheels, curves, HSL), LUTs. **Tests:** golden renders per blend mode/mask case; nesting depth and cycle protection; performance with 50-layer comps. **Completion:** recreate 5 reference social templates (lower thirds, animated captions, kinetic type) using only built-in tools.

## Phase 7 — Color management & HDR
**Objective:** Correct HDR and wide-gamut workflows for the formats creators actually shoot and platforms accept (ADR-023).
**Features:**
- HDR10 and HLG import/export with static metadata (mastering display, MaxCLL/MaxFALL).
- **HDR10+**: read dynamic metadata and use it for per-scene tone mapping; *generate* per-scene dynamic metadata on export from our own analysis (x265 for HEVC, T.35 for AV1). UI naming pending adopter-program check (RR-12).
- **Dolby Vision import**: iPhone profile 8.4 and profiles 5/8.1 decoded with RPU metadata applied (libdovi, MIT) so clips look as the camera intended; mixed DV/HLG/SDR timelines handled.
- **Dolby Vision export**: *not shipped* until legal review (RR-2); an experimental dev-only build flag may exist for research.
- Rec.2020/P3 working spaces; tone mapping (HDR→SDR, SDR-in-HDR); Windows HDR display transform; Linux HDR where supported; scopes (waveform, vectorscope, histogram, false color, nits readout); OCIO integration (Could).
**Tests:** measured test patterns with reference values; metadata round-trip (static, HDR10+, DV RPU parsing) against reference files; iPhone DV sample set compared with Apple's own rendering; display-transform validation on SDR and HDR monitors; YouTube-accepted HDR10+ upload check.
**Completion:** iPhone Dolby Vision and Samsung HDR10+ footage → SDR, HLG, HDR10 and HDR10+ exports that match reference renders within ΔE thresholds and are accepted by YouTube as HDR.

## Phase 8 — Professional audio
**Objective:** Audio good enough that users don't need a second app. **Features:** mixer, buses, sends, EQ/compressor/limiter/gate/de-esser, noise reduction, loudness metering and normalization to platform targets, surround 5.1/7.1, time-stretch/pitch. **Tests:** DSP reference comparisons; loudness accuracy against EBU test signals; RT-thread no-allocation checks. **Completion:** EBU R128 compliance test set passes; 100-track session plays without dropouts.

## Phase 9 — Local AI
**Objective:** AI features that save real editing time, running locally. **Features:** model manager; transcription → captions (word-timed, styled); translation; silence removal; scene detection; beat detection; background removal; auto-reframe; voice isolation. **Tests:** accuracy benchmarks on public datasets; performance per hardware tier; license review of every model. **Completion:** 10-minute talking-head video → captioned, silence-trimmed vertical cut in under 5 minutes of user time on a mid-range PC.

## Phase 10 — Advanced effects
**Objective:** Signature-quality effects, original implementations. **Features:** optical-flow retiming and interpolation; motion tracking and stabilization; glow, advanced blurs, distortion, stylization, sharpening, denoising; motion blur. **Tests:** quality metrics (PSNR/SSIM/VMAF vs references) and visual review. **Completion:** owner-approved quality bar on a curated test set.

## Phase 11 — Extensibility & interchange
**Objective:** Others can extend MOTIX safely; projects move between tools. **Features:** WASM plugin API (effects, importers/exporters, scripts), shader effects, CLAP hosting, OTIO/FCPXML/Premiere XML/EDL. **Tests:** plugin sandbox escape tests; interchange round-trip tests. **Completion:** 3 example plugins; round-trip a 100-cut timeline with a major NLE via OTIO/XML.

## Phase 12 — Hardening & 1.0
**Objective:** Production quality. **Features:** OS sandboxing of workers; zero-copy GPU paths; localization; MSI; offline update bundles; performance pass; accessibility audit; external security review (if affordable) or structured self-review; codec legal review completed (OD-5). **Completion:** all Must-have requirements met and verified; no open High-severity security issues.

## Phase 13 — macOS
**Objective:** MOTIX and MOTIX Server on macOS at feature parity (OD-8). Can be scheduled any time after Phase 2.
**Features:** macOS CI (free GitHub runners for public repos); Metal backend validation; EDR/HDR viewer; VideoToolbox decode/encode incl. ProRes via Apple's encoder; CoreML (ONNX Runtime) and Metal (whisper.cpp); Keychain; Seatbelt worker sandbox; `.app` + DMG, Developer ID signing and notarization; bundle-swap updater with rollback; server `.pkg` + launchd; Apple Silicon (macOS 14+ proposed).
**Dependencies:** Apple Developer Program (US$99/year) — owner decision at phase start.
**Tests:** full cross-platform test suite on macOS runners; Gatekeeper/notarization checks on a clean Mac; HDR viewer on an XDR/EDR display; cross-platform collaboration (Mac ↔ Windows ↔ Linux).
**Completion:** a clean Mac downloads the DMG, installs without warnings, edits, collaborates with Windows/Linux users, and updates/rolls back like other platforms.

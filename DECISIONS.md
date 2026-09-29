# Decision Log

Each decision records: **Problem · Options · Chosen approach · Reason · Consequences · Alternatives rejected**.
Technology decisions also record license, performance, cross-platform support, maintenance risk, security and long-term implications.

Statuses: **Accepted** · **Proposed** (engineering recommendation, not yet approved) · **Spike** (to be confirmed by a Phase 0 experiment with pass/fail criteria) · **Pending owner** (product owner must choose) · **Decided** (owner decision recorded) · **Deferred** · **Superseded**.

---

## Owner decisions

Decisions that belong to the product owner. Log: **decided** ones record the date and choice; **pending** ones carry a recommendation.

| ID | Topic | Status |
|----|-------|--------|
| OD-1 | Project license | ✅ Decided 2026-09-27: **GPL-3.0-or-later** |
| OD-2 | UI toolkit | ⏳ Pending Phase 0 spike (recommendation: egui + wgpu) |
| OD-3 | Windows code signing | ✅ Follows from OD-1: unsigned private alpha → **SignPath Foundation** (free) once the repo is public |
| OD-4 | Relay infrastructure | ⏳ Pending (recommendation below) |
| OD-5 | Patent-encumbered codecs | ⏳ Pending legal check before public 1.0; **dev/alpha builds bundle x264/x265** per "best features for free" |
| OD-6 | Product name | ✅ Decided 2026-09-27: **MOTIX** (trademark clearance recommended — RR-14) |
| OD-7 | Collaboration security model | ✅ Decided 2026-09-27 (delegated to architect): **end-to-end encrypted by default + opt-in server helper per project** (ADR-022) |
| OD-8 | macOS | ✅ Decided 2026-09-27: macOS **is a required platform**, delivered in a later phase; architecture stays Mac-ready; no Mac builds/tests until then. Apple Developer Program (US$99/year) decision deferred to that phase |
| OD-9 | Delivery order | ✅ Decided 2026-09-27: **core single-user editor first (Phases 0–3); collaboration foundations built invisibly inside it; networking ships from Phase 4** (PHASES.md → Delivery order) |

### OD-1 — Project license ✅
**Decided:** GPL-3.0-or-later, with DCO sign-off for contributions (no CLA).
- **Enables:** bundling x264/x265 (best-quality H.264/HEVC), Rubber Band, GPL-only AI models (e.g. Robust Video Matting); free Windows signing via SignPath Foundation (OSI license); protection against closed-source forks.
- **Consequences:** every distributed binary must come with source (or a written offer) — satisfied by the public GitHub repo and release source archives; all dependencies must be GPL-3.0-compatible (enforced by `cargo deny`); a proprietary edition later would require every contributor's permission.
- **Rejected:** permissive (loses x264/x265), proprietary (loses free signing and GPL components; contradicts $0 goal).

### OD-2 — UI toolkit ⏳
**Recommendation:** egui + wgpu, confirmed by the Phase 0 spike (ADR-003); Slint as the runner-up.
**Consequence:** switching later costs weeks-to-months of UI rewrite (the engine is unaffected by design).

### OD-3 — Windows code signing ✅
| Option | Cost | Notes |
|--------|------|-------|
| **SignPath Foundation** *(chosen)* | $0 | For OSI-licensed projects; SignPath verifies binaries are built from the public repo by CI and signs them with a certificate issued to the foundation. Application and approval required. |
| Azure Artifact Signing (formerly Trusted Signing) | ≈ US$10/month (Basic), paid Azure subscription required; eligibility rules apply | Fallback if SignPath declines. |
| OV code-signing certificate | ≈ US$200–400/year + hardware token | Traditional. |
| Unsigned | $0 | SmartScreen warnings; acceptable for a small invited alpha only. |

Note: since 2024, even EV certificates no longer skip SmartScreen reputation-building; any option needs time/downloads to build reputation.

### OD-4 — Relay infrastructure for temporary sessions ⏳
**Recommendation:** n0's free public relays for alpha; relay built into every MOTIX Server (teams with a server never need a third party); a project-run relay (~US$5–10/month) once there are real users; a custom relay list always available. Relays only ever see encrypted traffic (ADR-022), so the choice affects availability, not privacy. See ARCHITECTURE §12.2.

### OD-5 — Patent-encumbered codec policy ⏳
Code licenses (GPL, LGPL, BSD) do **not** grant patent rights. H.264, HEVC and AAC are covered by patent pools (Via LA, Access Advance); there are pool claims even against VP9/AV1. Many H.264 patents expire in the late 2020s, but this needs real legal review.

| Option | Description |
|--------|-------------|
| A. Hardware/OS first | H.264/HEVC export only via GPU encoders or OS frameworks (licensed by the hardware/OS vendor). |
| **B. Bundle x264/x265** *(current direction)* | Best quality for free. Patent exposure for the distributor in some jurisdictions — the same position as OBS, HandBrake, Shotcut and Kdenlive. |
| C. Optional "codec pack" managed component | Same encoders, downloaded on request; separates exposure from the base install. |

**Direction:** B in development and alpha builds (owner guidance: best working features for free). Confirm B vs C after a legal check before public 1.0. Hardware/OS encoders are always offered as "Fast" export. *Not legal advice.*

### OD-6 — Product name ✅
**Decided 2026-09-27:** **MOTIX**. Client `motix`, server **MOTIX Server** (`motix-server`), crates `motix-*`, project files `.motix`, invite links `motix://`, GitHub repository `motix`.
- **Trademark flag (not legal advice):** Infineon Technologies uses **MOTIX™** for motor-control chips and embedded software; a live US registration (Reg. No. 4846194, Technology Launch LLC) covers "MOTIX" for computer hardware and software (class 9 — the same class as video-editing software); an App Store developer also publishes as "MOTIX". Risk of an objection exists, particularly once the product is distributed widely or commercially. **Recommendation:** a trademark clearance check before spending on branding, domain or logo (RR-14). Renaming the code later is a small, mechanical change.

### OD-7 — Collaboration security model ✅
**Decided:** Option C — *strict end-to-end encryption by default, with an opt-in "server helper" per project* (ADR-022).
- Every collaboration (temporary session or persistent server) is end-to-end encrypted. Relays and servers store and forward only ciphertext.
- A project owner may deliberately add the server as a visible **Helper** member of *one* project so it can create proxies, thumbnails and renders; every member sees a "Server can read this project" badge. Off by default.
- **Consequences:** no server-side proxies/search/renders unless Helper is enabled; a project is unrecoverable if every member loses their keys *and* the owner's recovery kit — so key backup is a Must-have.

### OD-8 — macOS ✅
**Decided:** macOS is a required platform, scheduled as a later phase (PHASES.md, Phase 13, movable earlier). Until then the code must not preclude it (ADR-024), but there are no Mac builds or tests.
- **Cost flag:** shipping to Mac users without scary Gatekeeper blocks requires Apple's Developer Program (US$99/year; the fee waiver excludes individuals and open-source projects). Unsigned apps on macOS 15+ can only be opened through System Settings → Privacy & Security, and Homebrew no longer bypasses Gatekeeper for unsigned casks. Decide at the start of the macOS phase.
- **Target proposal:** Apple Silicon only, macOS 14+ (Apple's last Intel-capable release is macOS 26; building Intel doubles testing for a shrinking user base).

### OD-9 — Delivery order ✅
**Decided:** the editor must work fully on its own before any collaboration ships, but collaboration must not require rebuilding the core.
- **Consequences:** Phases 0–3 deliver milestones M1–M4 (first window → first edit → installer → beginner alpha). The "background track" (CRDT journal, author IDs on changes, command-only edits, validation of every change, deterministic merges tested with simulated editors, hash-referenced media, presence kept out of the document) is built inside those phases. S0-NET and S0-E2EE move off the critical path to run during Phase 3. No networking code ships before Phase 4 except the update check.

---

## Accepted decisions

### ADR-001 — Documentation-first, phased delivery
- **Problem:** A multi-year, high-ambition project with a small team risks building the wrong thing or an unmaintainable foundation.
- **Chosen:** Analyze, document architecture and decisions, then build in phases where each phase ends with something testable (per spec §55, §49).
- **Consequences:** Slower first code; far less rework. Docs must be kept in sync with code (enforced in PR checklist).
- **Status:** Accepted (by spec).

### ADR-017 — No managed language runtimes in shipped products
- **Problem:** Spec §3/§33 forbids requiring Python/Node/.NET/Java/Docker installs.
- **Chosen:** No such runtime is used in the shipped client or server, even bundled. AI runs via ONNX Runtime / whisper.cpp; build tooling may use anything.
- **Consequences:** Some AI models need conversion to ONNX/GGUF; some research models won't be usable. Much smaller installs and attack surface.
- **Status:** Accepted (by spec).

---

## Proposed technology decisions

### ADR-002 — Core language: Rust
- **Problem:** Choose the primary language for engine, client and server.
- **Options:** Rust · C++ (20/23) · C# (.NET) · Go · TypeScript (Electron/Node) · Zig.
- **Chosen:** **Rust**, with C/C++ libraries (FFmpeg, ONNX Runtime, whisper.cpp) through FFI in worker processes.

| Criterion | Assessment |
|-----------|-----------|
| Why | Memory safety without GC (the spec's threat model: malicious media, projects and packets); performance on par with C++; single static binaries (ideal for "no runtime" server); excellent cross-platform tooling (cargo); strong ecosystem for exactly our needs (wgpu, tokio, iroh, rustls, Loro, vello, tough). |
| Advantages | Eliminates the most common vulnerability class in parsers and network code; fearless concurrency for render/audio/network threads; reproducible builds; good AI-assisted development support. |
| Disadvantages | Steeper learning curve; longer compile times; fewer mature *media* libraries than C++ (OpenFX, OIIO, OCIO are C++); FFI boundaries require care (`unsafe`). |
| License | MIT/Apache-2.0 toolchain and most crates. |
| Performance | Native; SIMD available; no GC pauses (important for audio RT thread). |
| Cross-platform | Tier-1 on Windows x64, Linux x64 and macOS (Apple Silicon); ARM64 Windows/Linux available later. |
| Maintenance risk | Low: large, stable ecosystem; stable toolchain with 6-week releases and editions. |
| Security | Best available for a native app; `unsafe` confined to FFI/platform crates and reviewed. |
| Long-term | Rust adoption is growing in systems/graphics; not a niche bet. |

- **Alternatives rejected:** C++ (memory-safety risk in exactly the code that parses hostile input; build/packaging complexity across platforms); C#/.NET (needs a runtime or large self-contained bundle; weaker GPU/media ecosystem on Linux); Go (GC pauses, weak GPU ecosystem); Electron/TypeScript (heavy, poor for GPU video pipeline and HDR); Zig (pre-1.0).

### ADR-003 — UI toolkit: egui on our own wgpu surface *(Spike, OD-2)*
- **Problem:** The UI must be modern, dockable, accessible and high-DPI, and must show **HDR video** and a very large timeline at high frame rates. The viewer must display GPU textures produced by the engine without copying through the CPU.
- **Key constraint discovered:** HDR preview requires control of the *window swapchain's color space* (scRGB/PQ). Toolkits that own the swapchain and render SDR only make an accurate HDR viewer impossible without separate native child windows.
- **Options evaluated:**

| Option | HDR viewer | Engine texture sharing | Docking | Accessibility | Polish / beginner feel | Risk |
|--------|-----------|-------------------------|---------|---------------|------------------------|------|
| **egui + egui_tiles on our wgpu surface** | ✅ we own the surface; wgpu exposes `ExtendedSrgbLinear`/`Bt2100Pq` | ✅ same wgpu device | ✅ egui_tiles (used by Rerun) | ◐ AccessKit built in; custom widgets need work | ◐ needs a serious custom theme and custom widgets | Immediate-mode feel; must design polish deliberately |
| Slint (Rust) | ✗/◐ Slint owns the surface; HDR output not exposed | ✅ wgpu textures importable (feature-gated, *not stable*, pinned to an older wgpu major) | ✗ build our own | ✅ AccessKit | ✅ declarative, designer-friendly | wgpu version coupling; HDR |
| Qt 6 (QML) via cxx-qt | ✅ Qt RHI supports HDR swapchains | ✗ sharing wgpu textures with Qt RHI requires cross-API external memory | ✅ (KDDockWidgets is GPL/commercial) | ✅ best in class | ✅ | Three languages (Rust/C++/QML); LGPL compliance; heavy |
| Tauri / Electron (web UI) | ✗ no accurate HDR, no zero-copy video | ✗ | ✅ (JS libs) | ✅ | ✅ | Viewer would need an overlay native window; Linux WebKitGTK inconsistencies |
| Iced / Xilem / GPUI | ◐ | ◐ | ✗ | ◐ | ◐ | Less mature for this scale or not wgpu-based |

- **Chosen (proposed):** **egui** (MIT/Apache-2.0) with **egui_tiles** for docking and **AccessKit** for accessibility, drawn into a wgpu surface we configure ourselves. Timeline, viewer, graph editor and scopes are custom GPU widgets.
- **Reason:** The only option that satisfies HDR viewer + zero-copy engine textures + docking + pure Rust today. Polish is achievable with effort; the other options' gaps are structural.
- **Spike pass/fail criteria (Phase 0):** (1) HDR scRGB viewer on Windows HDR display, SDR UI correct at user's SDR brightness; (2) 5,000-clip timeline at ≥ 120 fps scroll/zoom on an integrated GPU; (3) docking + saved workspaces; (4) NVDA (Windows) and Orca (Linux) read menus, buttons, inspector fields; (5) IME text input (CJK) and emoji in text fields; (6) Wayland + X11 on Linux; (7) a designer-quality mock screen reproduced credibly.
- **Consequences:** We invest in a design system (theme, typography, spacing, animation helpers). UI stays behind `motix-app` so a later switch touches only `motix-ui`.
- **Progress (2026-09-27):** M1 shell built on egui 0.36 / eframe (wgpu 30 renderer) / egui_tiles 0.17. Verified so far: docking layout, custom GPU-drawn viewer and timeline, keyboard-driven command palette, AccessKit labels for custom widgets (tested by queries), headless GPU rendering for screenshot tests. Still to verify on real hardware: criteria 1 (HDR), 2 (5,000-clip timeline speed), 5 (IME), 6 (Wayland/X11 on a desktop).
- **Alternatives rejected:** see table. Slint remains the fallback if (7) fails badly and a native child-window HDR viewer proves workable.

### ADR-004 — GPU abstraction: wgpu
- **Options:** wgpu · raw Vulkan (ash) + DX12 · bgfx · Diligent · Qt RHI.
- **Chosen:** **wgpu** (MIT/Apache-2.0): DX12, Vulkan, Metal, GL backends; WGSL shaders (naga also accepts SPIR-V/GLSL); compute shaders; used widely (Firefox's WebGPU, Bevy, many tools).
- **Pros:** Vendor-neutral (NVIDIA/AMD/Intel/Qualcomm); safe API; surface color spaces for HDR; `wgpu-hal` escape hatch for platform features (external memory import, HDR metadata).
- **Cons:** Abstraction lags newest features (e.g. hardware video APIs, some 64-bit atomics); major releases frequently (API churn); some platform interop (D3D shared handles, Vulkan video) requires `unsafe` hal access.
- **Performance:** Close to native for our workloads; validation overhead minimal in release.
- **Maintenance risk:** Low–medium (active, well-funded ecosystem). Pin versions; upgrade deliberately.
- **Security:** Shader validation (naga) protects against malformed shaders — relevant for future shader plugins.
- **Rejected:** raw Vulkan+DX12 (2× backend work); bgfx/Diligent (C++; weaker compute story); Qt RHI (ties engine to Qt).

### ADR-005 — Media I/O: FFmpeg via FFI in worker processes
- **Options:** FFmpeg libraries · GStreamer · platform APIs (Media Foundation / VAAPI directly) · pure-Rust decoders.
- **Chosen:** **FFmpeg** (libavformat/libavcodec/libswresample/libavfilter selectively), pinned version, built from source in CI with an explicit configure line; accessed through thin Rust bindings; run in `motix-media-worker` processes.
- **Pros:** Broadest codec/container coverage by far; hardware acceleration for every vendor; industry standard.
- **Cons:** Large C codebase with regular CVEs (→ sandboxed workers); build complexity (cached CI artifact); license depends on configure flags (LGPL vs GPL).
- **License:** GPL build (x264/x265 enabled) → the FFmpeg libraries we ship are GPL-2.0-or-later, compatible with our GPL-3.0-or-later (OD-1). Configure line and source published with every release.
- **Performance:** Frames cross the process boundary via shared memory (one copy); later zero-copy GPU handles. Phase 0 measures overhead (target: < 10% at 4K60).
- **Rejected:** GStreamer (heavier, plugin sprawl, harder to sandbox and bundle on Windows); platform APIs only (2× work, fewer formats); pure Rust (coverage far too small today — revisit for specific codecs like dav1d-in-Rust, rav1d).

### ADR-006 — Time representation: integer flicks + rationals
- **Problem:** Floating-point time accumulates error; frame rates like 29.97 are rationals.
- **Chosen:** `i64` ticks at 705,600,000/s (≈ 13,000 years range) for all timeline positions; frame rates as exact rationals; conversion to frame/sample indices by exact integer math.
- **Consequences:** No drift, exact round-trip for every common frame and sample rate. Unusual rates (e.g. 1000/1001 × 120) still exact. Arbitrary VFR source timestamps are mapped with rounding at import, recorded.
- **Rejected:** `f64` seconds (drift, non-determinism across peers); per-sequence timebases (conversions everywhere).

### ADR-007 — Project/collaboration data model: Loro CRDT under a typed schema *(Spike)*
- **Problem:** The same model must support instant local edits, crash-safe persistence, undo/redo, history, offline edits and concurrent editing (spec §34–37).
- **Options:**

| Option | Offline / P2P | Needs central server | Semantic invariants | Complexity |
|--------|---------------|----------------------|---------------------|------------|
| Server-authoritative ops (Figma-style LWW per property) | ✗ weak offline | ✅ yes | Easy (server decides) | Low |
| Operational transformation | ◐ | ✅ yes | Hard to prove correct beyond text | High |
| Event sourcing (domain events, append-only) | ◐ (needs a sequencer) | ◐ | Easy per event | Medium |
| **CRDT (Loro)** + authority node for validation | ✅ | ✗ (authority optional: host or server) | Needs normalization layer | Medium |
| Automerge / Yrs (other CRDTs) | ✅ | ✗ | Same issue | Medium |

- **Chosen:** **Loro** (MIT) — Rust-native, fast, has **movable lists** (track/layer reordering without duplication) and a **movable tree** (bins/folders, nested groups) which Automerge/Yrs lack or handle less directly; built-in undo manager, version vectors, shallow snapshots, and time travel for history. Wrapped by `motix-schema` (typed API, validation, deterministic normalization). Edits are expressed as domain commands (`motix-commands`) that produce Loro transactions — keeping user intent visible for undo grouping and future scripting.
- **Why an authority node anyway:** permission enforcement and validation. In temporary sessions the host is authority; with a persistent server, the server.
- **Spike criteria:** 10,000-item project: open < 1 s, edit apply < 1 ms, snapshot < 50 ms; concurrent edit scenarios (split vs trim, ripple vs insert, move vs delete, reorder tracks) converge with acceptable results; undo only affects own changes; document growth over a simulated 40-hour project stays manageable with compaction.
- **Consequences:** We own a normalization layer and must design conflict UX. The CRDT library is a critical dependency (pinned; fallback is Automerge with a similar schema layer).
- **Rejected:** Pure server-authoritative model (breaks local-first/P2P); OT (complexity, central server); plain event sourcing (needs a total order = central sequencer, poor offline merge).

### ADR-008 — Project file container: SQLite with append-only journal
- **Options:** Single JSON/binary file rewritten on save · folder of files · zip package · **SQLite**.
- **Chosen:** SQLite (public domain, compiled in) holding the journal of CRDT updates, snapshots, named versions and asset index (ARCHITECTURE §5.3).
- **Pros:** Atomic, crash-safe transactions (WAL), incremental appends (no full rewrite on autosave), single file for users, mature tooling, well-studied robustness.
- **Cons:** Opening untrusted databases needs defensive configuration; a single file is less diff-friendly than text (acceptable; history is inside).
- **Rejected:** rewritten JSON (slow for big projects, corruption-prone on power loss); folder-based projects (users move/copy half of them); zip (no atomic incremental writes).

### ADR-009 — Networking: iroh
- **Options:** **iroh** (QUIC, dial-by-key, hole punching, relays) · WebRTC (webrtc-rs / libdatachannel) + STUN/TURN + signaling server · libp2p · raw QUIC (quinn) + our own NAT traversal · Tailscale/Headscale-style WireGuard mesh.
- **Chosen:** **iroh 1.x** (MIT/Apache-2.0; 1.0 released June 2026 with wire-protocol stability across 1.x) and **iroh-blobs** for verified content-addressed transfer.

| Criterion | Assessment |
|-----------|-----------|
| Why | Matches the spec almost exactly: identity = public key (not IP), direct-first with hole punching, blind relay fallback, relays self-hostable, LAN discovery, QUIC multiplexed streams. |
| Performance | QUIC with multipath; the project reports ~90% direct connections and ~95% of bytes flowing directly. |
| Cross-platform | Windows, Linux, macOS, mobile. |
| Security | TLS 1.3 with raw public keys; mutual authentication; relays can't decrypt. Post-quantum hybrid key exchange (X25519MLKEM768) supported as an opt-in using the aws-lc-rs provider — **we enable it** (ADR-022). Content is additionally end-to-end encrypted above the transport (ADR-022), so transport security is a second layer, not the only one. |
| Maintenance risk | Medium: a single company (n0) leads development; mitigated by permissive license, 1.0 stability commitment and the ability to self-host relays. |
| Long-term | Protocol stability within 1.x; our protocols sit on top via ALPN so a transport swap is contained in `motix-net`. |

- **Rejected:** WebRTC (needs a signaling service we would have to run; heavier stack; designed for browsers); libp2p (more complex, less focused on NAT traversal UX); raw QUIC (re-implementing hole punching and relays); WireGuard mesh (needs a coordination server and OS-level networking).

### ADR-010 — Identity: local Ed25519 keys and device certificates
- **Chosen:** user identity key + per-device keys certified by the user key (ARCHITECTURE §11). Keys in OS keystores (Windows Credential Manager/DPAPI, Linux Secret Service, macOS Keychain — via `keyring`), passphrase-encrypted file fallback. Backups with `age` (MIT/Apache-2.0 implementation `rage`).
- **Consequences:** No password resets; recovery depends on user backups or a second device — UX must make backup easy and prompted at the right moments.
- **Rejected:** centralized accounts (spec §10); OAuth with third parties (dependency + privacy); passwords per server (phishable, reused).

### ADR-011 — Persistent server is a trusted participant — **Superseded by ADR-022**
- Original proposal: the server decrypts and stores project data to validate, keep revisions and make backups.
- **Superseded 2026-09-27** after the owner asked for the most secure collaboration option (OD-7). The server is now *untrusted for confidentiality*: it stores and forwards ciphertext only, unless a project owner explicitly enables the Helper role for one project.

### ADR-012 — Update security: TUF + Authenticode + provenance
- **Options:** download latest `.exe` from GitHub (spec forbids) · single Ed25519-signed manifest (minisign-style) · **TUF** · Sigstore-only.
- **Chosen:** **TUF** metadata (via **tough**, MIT/Apache-2.0) hosted as static files (GitHub Pages), targets on GitHub Releases; Windows binaries also Authenticode-signed (OD-3); GitHub build-provenance attestations for transparency.
- **Why TUF over a single signed manifest:** handles the realistic failures — key compromise (rotation via root), freeze attacks (expiry), rollback attacks (version monotonicity), partial compromise (role separation, thresholds). A home-made manifest scheme would re-invent these badly.
- **Consequences:** Key ceremony and procedures to document (root key offline; timestamp re-signing workflow). Slightly more complex release pipeline.
- **Rejected:** minisign manifest alone (no rotation/freeze/rollback story); Sigstore-only (verification needs online transparency-log checks and is designed for provenance rather than update trust).

### ADR-013 — Install layout: per-user side-by-side versions + launcher *(Spike)*
- **Options:** MSI per-machine with in-place upgrades · MSIX · Squirrel/**Velopack** · own launcher with side-by-side versions.
- **Chosen (proposed):** own small **launcher** (`motix-launcher`) + `versions/` directories + `state.json` (ARCHITECTURE §15.3); initial install by **Inno Setup** (free) per-user; MSI (WiX) later for enterprises. **Velopack** (MIT; Rust core; delta updates; GitHub Releases support) is evaluated in the Phase 0 spike as an alternative for install + delta packaging, *with our TUF verification in front*.
- **Reason:** Update-without-admin, instant rollback, health checks and TUF integration are core requirements; owning the ~2–3k-line launcher keeps them under our control.
- **Rejected:** per-machine MSI (UAC prompt on every update, poor rollback); MSIX (sandbox restrictions for GPU/driver access and file associations, packaging friction on Linux parity).

### ADR-014 — Scene-linear floating-point working space from Phase 1
- **Chosen:** RGBA16F premultiplied, scene-linear working space; explicit color metadata on every image; built-in `motix-color` module (sRGB, Rec.709/BT.1886, Rec.2020, Display P3, PQ, HLG, BT.2390 tone mapping) with CPU reference + WGSL; OpenColorIO (BSD-3) later.
- **Reason:** HDR and quality requirements make an 8-bit sRGB pipeline a guaranteed rewrite.
- **Consequences:** ~2× memory bandwidth vs 8-bit; mitigated by region-of-interest rendering and proxies.

### ADR-015 — AI runtime: ONNX Runtime + whisper.cpp, local-first
- **Options:** ONNX Runtime · whisper.cpp/ggml · Burn (Rust, wgpu backend) · PyTorch/Python (rejected by ADR-017) · cloud APIs (privacy, cost).
- **Chosen:** **ONNX Runtime** (MIT) for vision/audio models: CPU everywhere, **DirectML** on Windows (any DX12 GPU; note: DirectML is in maintenance mode — Windows ML is Microsoft's successor path; monitor), optional managed CUDA/TensorRT for NVIDIA, MIGraphX/ROCm on Linux for AMD. **whisper.cpp** (MIT) with Vulkan for transcription on any GPU. **Burn** tracked as a future pure-Rust, wgpu-based option.
- **Consequences:** Runs in `motix-ai-worker`; each model individually license-reviewed; hardware-dependent speed, surfaced honestly in UI.

### ADR-016 — Plugins via WebAssembly sandbox *(Deferred)*
- **Chosen direction:** wasmtime (Apache-2.0 with LLVM exception) components with capability grants; WGSL shader effects validated by naga. Not built before Phase 11.

### ADR-018 — Server data directory as the unit of migration
- **Chosen:** all persistent server state under one data-dir with fixed internal layout (ARCHITECTURE §14.2); identity key travels with backups; application binaries elsewhere.
- **Consequences:** Migration = backup → install → import. IP/OS/hardware changes don't change the Server ID.

### ADR-019 — Vector and text rendering: vello + cosmic-text
- **Chosen:** **vello** (Apache-2.0/MIT, GPU compute 2D renderer; CPU fallback) for shapes, text and caption styles; **cosmic-text / swash / harfrust** (MIT/Apache-2.0) for shaping and fonts.
- **Risk:** vello is younger than Skia; if blocking issues arise, fallback is tiny-skia (CPU) for affected features. Tracked in Phase 3.
- **Rejected:** Skia via bindings (large C++ dependency, build complexity).

### ADR-020 — Audio stack
- **Chosen:** cpal (Apache-2.0) device I/O; rubato (MIT) or libsoxr (LGPL-2.1) resampling; ebur128 (MIT) loudness; Signalsmith Stretch (MIT) time-stretch/pitch; RNNoise (BSD-3) / DeepFilterNet (MIT/Apache-2.0) noise reduction.
- **Rejected:** JUCE (commercial/AGPL licensing), PortAudio (C, less idiomatic; cpal covers our backends).

### ADR-021 — Serialization for IPC and network protocols
- **Chosen:** serde + postcard (compact, no_std-friendly, non-self-describing) inside a versioned envelope `{type id, version, length}`; hard maximum sizes enforced *before* allocation; every decoded message validated. CRDT payloads use Loro's own binary encoding, decoded only in validated contexts.
- **Rejected:** JSON on hot paths (size, speed); Protobuf/Cap'n Proto (extra toolchain; acceptable alternative if cross-language clients become a goal).

### ADR-022 — End-to-end encrypted collaboration with MLS (supersedes ADR-011)
- **Problem:** The owner wants the most secure option for all co-op work (OD-7). Transport encryption alone leaves project content readable on the persistent server, its disks and its backups.
- **Options:**

| Option | Server can read content | Forward secrecy / post-compromise security | Member removal | Standardized | Cost to us |
|--------|------------------------|--------------------------------------------|----------------|--------------|-----------|
| Transport TLS only (ADR-011) | Yes | Transport only | ACL change | Yes | Low |
| Pairwise encryption (encrypt each update to each member) | No | Weak | Re-encrypt to remaining | No (custom) | High, O(n) per update |
| Shared project key (one symmetric key, rotated manually) | No | None | Manual rotation | No (custom) | Medium; fragile |
| **MLS — Messaging Layer Security (RFC 9420) via OpenMLS** | **No** | **Yes (epochs)** | **Automatic key rotation** | **IETF standard** | Medium |
| Research access-control CRDTs (e.g. Keyhive) | No | Varies | Yes | No (research) | High risk |

- **Chosen:** **MLS via OpenMLS** (MIT, Rust, maintained by Phoenix R&D / Cryspen). Each project and each temporary session is an MLS group whose members are *devices* (credential = device certificate chained to the user identity key, ADR-010).
  - **Content:** CRDT updates, snapshots and presence are encrypted with keys derived from the current MLS epoch; media is encrypted with a random per-asset key (chunked AEAD, streamable, seekable), and asset keys live inside the encrypted project document.
  - **Signed edits:** every update is signed by its author's device key; every client verifies author + role before applying, deterministically, so a compromised server cannot forge or alter edits.
  - **Roles over E2EE:** the membership/role log is a signed, append-only log (owners sign changes). The server can read it (it needs to know who may fetch/append) but cannot change it. Viewers hold read keys but their edits are rejected by every client.
  - **Post-quantum:** transport uses iroh's X25519MLKEM768 hybrid handshake. For stored MLS handshake messages ("harvest now, decrypt later"), we use OpenMLS's hybrid **X-Wing** ciphersuite (X25519 + ML-KEM). Interop risk is irrelevant because only our software speaks our protocol; the ciphersuite is draft, so we keep a ciphersuite-migration path (MLS re-init) — *spike item*.
  - **Server role:** Delivery Service + encrypted storage + KeyPackage directory + transport ACL + relay. It sees metadata only: member public keys, roles, sizes, timing.
  - **Opt-in Helper (OD-7):** an owner can add the server's device key as a member with role *Helper* (can read; may publish only derived artifacts such as proxies, thumbnails and renders; cannot edit). Visible to all members; removing it rotates keys.
  - **Recovery kit:** each project owner has a recovery public key (private key printed as words / saved file). Snapshot keys are periodically sealed to it, so a project survives the loss of every member device.
  - **Verification:** safety codes (short comparable fingerprints / QR) between collaborators; "verified" badges; loud warnings on key changes.
- **Consequences:**
  - Server cannot generate proxies, thumbnails, search indexes or renders unless Helper is enabled; clients do this work and upload encrypted results.
  - New members see the project from the snapshot at which they joined; earlier history is visible only if an owner shares history keys (explicit action).
  - Removed members keep whatever they already downloaded (true of any system); they cannot read anything new.
  - Joining a persistent-server project needs an owner/manager device to come online once to complete the MLS add (automatic if the invite was pre-approved).
  - A malicious server can still withhold or delay updates (availability); per-author signed hash chains and version-vector gossip let clients *detect* this.
  - Server backups contain only ciphertext → safe to store off-site.
- **Rejected:** ADR-011 (server reads everything); custom schemes (never invent cryptography); research CRDT access control (not production-ready).
- **Status:** Accepted in direction (OD-7); details validated in the Phase 0 spike **S0-E2EE** and built in Phases 4–5.

### ADR-023 — HDR formats: HDR10, HLG, HDR10+ and Dolby Vision
- **Problem:** Phase 7 covered HDR10 and HLG only. Creators' phones record Dolby Vision (iPhone default: Profile 8.4, HLG-compatible) and HDR10+ (many Samsung/Android phones); platforms accept different formats (YouTube documents PQ/HLG with optional HDR10+ dynamic metadata; it does not list Dolby Vision).
- **Chosen:**

| Format | Import & correct preview | Export | Notes |
|--------|------------------------|--------|-------|
| HDR10 (PQ + static metadata) | ✅ | ✅ | Royalty-free. |
| HLG | ✅ | ✅ | Royalty-free. Best for phone-to-social HDR. |
| **HDR10+** (dynamic metadata, SMPTE ST 2094-40) | ✅ metadata read (FFmpeg) and used by our tone mapper | ✅ *auto-generated* per-scene metadata from our own analysis, written via x265 (HEVC) and AV1 T.35 metadata | No per-unit royalties. Using the name/logo needs the HDR10+ adopter program (**Verify** fees/terms); until then the UI says "dynamic HDR metadata (ST 2094-40)". |
| **Dolby Vision** | ✅ decode base layer + parse and apply RPU metadata (profiles 5, 8.1, 8.4) via **libdovi** (MIT) or libplacebo (LGPL-2.1) | ⚠️ **Research + legal** — open tools (libdovi/dovi_tool, MIT) can generate profile 8.1/8.4 RPUs and x265 can inject them, but Dolby Vision is a licensed proprietary technology (patents, trademark) | Import matters most (iPhone footage). Export only after legal review; HDR10/HLG/HDR10+ cover platform needs meanwhile. |

- **Consequences:** adds libdovi dependency (Phase 7); HDR10+ metadata generation becomes a quality feature ("scene-by-scene analysis"); Dolby Vision export stays out of shipped builds until cleared.
- **Rejected:** ignoring DV metadata (iPhone clips would look wrong in profile 5/8.1 cases and lose intent in 8.4).

### ADR-024 — macOS support approach (Deferred build, Mac-ready now)
- **Problem:** macOS is required (OD-8) but scheduled later; later work must not require a rewrite.
- **Chosen now (no Mac builds):** stay on portable choices that already support macOS — Rust, wgpu (Metal), egui/winit, AccessKit, cpal (CoreAudio), iroh, OpenMLS, SQLite, FFmpeg (VideoToolbox), ONNX Runtime (CoreML EP), whisper.cpp (Metal), `keyring` (Keychain). Everything OS-specific goes through `motix-platform` traits. Avoid Windows- or Linux-only crates outside `motix-platform`.
- **Planned for the macOS phase:** `.app` bundle + DMG; Developer ID signing + notarization; update by atomic bundle swap with the previous bundle kept for rollback; EDR/HDR preview via Metal (`ExtendedSrgbLinear`/`ExtendedDisplayP3` surfaces — macOS has the most mature HDR desktop pipeline); VideoToolbox hardware encode including **ProRes via Apple's own encoder**; Seatbelt sandbox profile for workers; server as a launchd service.
- **Costs:** US$99/year Apple Developer Program at Mac launch (OD-8); GitHub macOS runners are free for public repositories.

### ADR-025 — In-process, memory-safe header probe (`motix-probe`)
- **Problem:** the owner wants MOTIX to ask "match the project to this video?" and to place clips of different sizes sensibly *on import*. That needs each file's size, rotation, frame rate, duration, audio streams and HDR signalling immediately — before the FFmpeg worker (Phase 1) exists, and ideally without starting a process per file even after it does.
- **Chosen:** a small pure-Rust crate, `motix-probe`, that reads **container headers only** (ISO-BMFF `moov`, Matroska `Info`/`Tracks`, RIFF `fmt `/`data`, PNG `IHDR`, JPEG SOF) and never decodes. `#![forbid(unsafe_code)]`, no dependencies beyond `motix-core`, bounded reads (64 MiB header cap, 64 tracks, depth 16), errors instead of panics, and a randomized corruption test (4,400 mutated files per run) in CI; a cargo-fuzz target follows with the fuzzing work in Phase 1.
- **Why in-process is acceptable here:** ARCHITECTURE §6 puts untrusted-media *decoding* in sandboxed workers because FFmpeg is C. This parser is memory-safe Rust with hard limits, so the worst case is a wrong answer or an error, not memory corruption. FFmpeg remains the authority once it runs: the worker's probe result overrides this one when they differ.
- **Consequences:** the import UI is instant; files the probe can't read (AVI, MXF, HEIC…) are still accepted by extension and get details when the media engine arrives. HDR10+ (in-stream SEI) and Dolby Vision RPUs are only *flagged* here (DV via `dvcC`/`dvvC`), not parsed.
- **Rejected:** shelling out to `ffprobe` (a separate install — violates "no extra installs"); the `mp4`/`matroska` crates (partial coverage, extra dependencies, still need our own HDR logic).

### ADR-026 — Interim timeline model and snapshot undo
- **Problem:** the owner asked for an editable timeline now (unlimited tracks, linked A/V, undo). The final model is the Loro-backed typed schema (ADR-007), which needs its spike first.
- **Chosen:** a plain Rust model in `motix-app::timeline` (tracks and clips with stable ids, link groups) whose **operations are named methods that map 1:1 to future document operations** (place, move group, split, delete, link/unlink, add/remove track, set fit). Undo/redo takes a **snapshot of media bin + timeline + settings** per edit (200 steps).
- **Consequences:** simple, fully tested behaviour today; snapshots are memory-heavy for huge projects, which is acceptable for previews and disappears when ADR-007 lands (undo becomes CRDT-history based). UI code does not change when the model's backing store changes.
- **Rejected:** building the CRDT schema before the spike; command-object undo that would be thrown away.

### ADR-027 — Preview-channel updater: signed GitHub releases, checked every 10 minutes
- **Problem:** the owner wants MOTIX to check GitHub at start-up and every 10 minutes, download new versions by itself, ask before restarting, and offer a manual check. ADR-012 (TUF + launcher, ADR-013) is the long-term design but needs the launcher and key ceremony first.
- **Chosen (now):** `motix-update` — a background thread that polls `GET /repos/SlothGod1/motix/releases` with `If-None-Match` (unchanged lists cost nothing and don't count against GitHub's limits), picks the newest newer release, downloads `SHA256SUMS` + `SHA256SUMS.sig`, **verifies the Ed25519 signature against keys compiled into MOTIX**, checks the manifest's `# motix-version` line matches the release (no downgrade by re-tagging), streams the zip to `%LOCALAPPDATA%\MOTIX\updates` while hashing it, and deletes anything that doesn't match. "Restart now" / "Later (install on close)". Install unpacks inside the MOTIX folder (same drive, so moves are atomic), moves the current files to `.motix-update/previous` (renaming a running `.exe` is allowed on Windows) and moves the new ones in, **rolling back on any error**. HTTPS via rustls with the operating system's certificate store (rustls-platform-verifier), HTTPS-only, bounded sizes, safe zip paths.
- **Release side:** the CI `release` job (every green push to `main`) stamps `MOTIX_VERSION = <workspace version>-preview.<run>`, zips each platform, writes `SHA256SUMS` and signs it with `cargo run -p xtask -- sign` using the `MOTIX_UPDATE_SIGNING_KEY` repository secret. Without the secret the release is published but installed copies refuse it.
- **Key custody:** the secret key exists only in that GitHub secret (and the owner's offline backup). Rotation: ship a release that trusts old + new keys, then switch.
- **Limits (accepted for previews):** no automatic health-check rollback yet (the previous version is kept on disk for manual rollback); developer builds (no `MOTIX_VERSION`) never update themselves; per-user install location must be writable.
- **Later:** TUF metadata (ADR-012) and the launcher (ADR-013) replace the manifest and file swap; the key, UI and release flow stay.

### ADR-028 — Track order follows the order media is added; tracks named after files
- **Problem:** the owner found "Video 1…4 / Audio 1…2" grouping confusing and asked for tracks named after their files, in the order added, with each video's audio right below it.
- **Chosen:** one ordered list of tracks (mixed kinds). A new file gets new tracks at the bottom: `<file>` and `Audio of <file>` (`Audio 2 of <file>` for extra streams; audio-only files use the file name). Dropping onto a track reuses it (and the tracks below it) when free, otherwise inserts new tracks right there. **Higher video tracks are in front** (After Effects-style layer order); tracks can be reordered. Rename, lock, solo, mute/hide per track. New clips start at the playhead (owner's choice) or at a drop point; "Add at end" appends.
- **Consequences:** compositing order = list order of video tracks (top wins); audio is mixed from all audible tracks (solo-aware). The CRDT schema (ADR-007) stores tracks as an ordered sequence, which this matches.

### ADR-029 — Automatic delivery from Claude's sessions: push → checks → main → signed release
- **Problem:** the owner wants Claude to publish work to GitHub by itself. Claude's cloud sessions reach GitHub through a proxy that only lets a session push to **its own working branch** (named `claude/...`) of a repository attached to the session; it can't push to `main` directly.
- **Chosen:** CI runs on pushes to `claude/**`. When tests (Windows + Linux), cargo-deny and the builds all pass, a `promote` job **fast-forwards `main`** to that commit using the workflow's own token, and the `release` job publishes the signed preview release (ADR-027) — so installed copies of MOTIX update within about 10 minutes. If `main` moved on in the meantime, promotion refuses (no merge commits, no overwritten work) and nothing is released.
- **Owner setup (one time):** install the Claude GitHub App on `SlothGod1/motix`, start sessions with that repository attached, and add the `MOTIX_UPDATE_SIGNING_KEY` secret.
- **Safety:** only accounts with write access can push `claude/...` branches, so this grants nothing beyond what pushing to `main` would; failing checks block promotion; the owner can turn it off by deleting the `promote` job or protecting `main`.

### ADR-030 — Delivery from the Cowork chat through GitHub Desktop
- **Problem:** the owner wants to keep building MOTIX with Claude in the Cowork chat, which can't push to GitHub (Cowork sessions don't attach repositories), and still get changes onto GitHub with little effort.
- **Chosen:** the owner clones `SlothGod1/motix` with **GitHub Desktop** into `Editing Software\motix` (inside the folder already shared with Claude). Claude builds and tests in its sandbox, writes the changed files straight into that clone, and gives a one-line summary; the owner reviews the list in GitHub Desktop, clicks **Commit to main** and **Push origin**. CI then tests, builds and publishes the signed release (ADR-027) as usual.
- **Limits:** the file bridge can't write inside `.github\` or delete files, so workflow edits (rare) and deletions are done by the owner from Claude's instructions. The owner's click is also a useful human check before anything ships.
- **Rejected:** the GitHub web uploader (drops dotfiles and folders like `.github`, can't delete, many clicks); giving Claude a personal access token (secrets must not pass through the chat).


# Requirements

Derived from the product specification and [ANALYSIS.md](ANALYSIS.md). IDs are stable; don't renumber.
"Phase" is the phase in [PHASES.md](PHASES.md) where the requirement is first delivered.

**Tags:** 🔒 security implications · ⚖️ licensing concern · ⚡ performance implications · 🖥️ platform limitation

---

## Must Have

### Installation and runtime
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-INS-1 | Client installs on Windows with one installer, no admin rights, no separate dependency installs. | 🔒🖥️ | 2 |
| R-INS-2 | Client runs on Linux from one AppImage without installing dependencies (GPU driver excepted). | 🖥️ | 2 |
| R-INS-9 | Client installs on macOS from one signed, notarized DMG with no Gatekeeper workarounds. | 🖥️⚖️ | 13 |
| R-INS-3 | No Python, Node.js, Java, .NET, Docker, database, web server or SDK required on user machines (ADR-017). | ⚖️ | 1 |
| R-INS-4 | All dependencies version-pinned and loaded only from the app's own directories. | 🔒 | 1 |
| R-INS-5 | Uninstall removes app, runtimes and caches; asks about downloaded models; never deletes projects or identity keys. | | 2 |
| R-INS-6 | Large optional components (AI models, CUDA, codec packs) are downloaded by the app on demand with consent and progress, verified by signed metadata. | 🔒⚖️ | 9 |
| R-INS-7 | Server installs as a service on Windows and Linux (macOS in Phase 13) with a setup wizard; no manual config file editing required. | 🔒🖥️ | 5 |
| R-INS-8 | Detect GPU/driver capability at startup; plain-language guidance if the driver is too old; CPU fallback where practical. | 🖥️⚡ | 1 |

### Local editing core
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-EDT-1 | Works fully offline: create, edit, render, export, save, recover. | | 1 |
| R-EDT-2 | Import common media (MP4/MOV/MKV/WebM; H.264/HEVC/AV1/VP9/ProRes/DNx decode; AAC/MP3/Opus/FLAC/WAV; PNG/JPEG/WebP/TIFF/EXR/HEIC stills; image sequences), including phone HDR footage (HLG, HDR10+, Dolby Vision). | ⚖️🔒 | 1–3, 7 |
| R-EDT-3 | Timeline with multiple video and audio tracks, trim, split, move, ripple delete, snapping, undo/redo. | | 1–3 |
| R-EDT-4 | Real-time preview with audio sync, adaptive quality and proxies; UI never freezes during renders. | ⚡ | 1–3 |
| R-EDT-5 | Keyframes for transform, opacity and effect parameters with easing. | | 3 |
| R-EDT-6 | Text layers and captions with full Unicode shaping, RTL, emoji and styling. | ⚡ | 3 |
| R-EDT-7 | Vertical, square, landscape and custom resolutions; safe-area overlays for major platforms. | | 3 |
| R-EDT-8 | Export presets for TikTok, Reels, Shorts, YouTube, X, Discord; custom export with precise codec/container/bitrate/color/audio settings. | ⚖️ | 1–3 |
| R-EDT-9 | Speed changes and freeze frames. | | 3 |
| R-EDT-10 | Variable-frame-rate media handled correctly (no A/V drift). | | 1 |

### Quality and color
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-QLT-1 | Floating-point, scene-linear processing with explicit color metadata on every image (ADR-014). | ⚡ | 1 |
| R-QLT-2 | High-quality scaling (linear-light filtering), chroma upsampling and audio resampling in export. | ⚡ | 1 |
| R-QLT-3 | Versioned effect implementations: old projects render identically after updates. | | 3 |
| R-QLT-4 | SDR Rec.709 correct end-to-end (import tags → display → export tags). | | 1 |
| R-QLT-5 | Windows HDR on + SDR project: preview matches SDR appearance via our display transform. | 🖥️ | 7 (spike in 0) |

### Project safety
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-PRJ-1 | Continuous journaled autosave; ≤ 1 s of edits at risk on crash/power loss. | ⚡ | 1 |
| R-PRJ-2 | Crash recovery on next start; integrity verification; fallback to rolling backups. | | 1 |
| R-PRJ-3 | Project format versioned with tested migrations; newer files open read-only in older apps. | | 1 |
| R-PRJ-4 | Pre-migration backup before any format upgrade. | | 1 |
| R-PRJ-5 | Named versions and restore. | | 1 |
| R-PRJ-6 | Media referenced by content hash + relative/absolute hints; relinking; missing-media handling. | | 1 |
| R-PRJ-7 | Caches, proxies and temp files stored separately and always deletable. | | 1 |
| R-PRJ-8 | Project files treated as untrusted input (validation, limits, no executable content). | 🔒 | 1 |

### Updates and distribution
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-UPD-1 | GitHub Actions builds, tests, packages, signs and publishes Windows and Linux releases (macOS from Phase 13) for client and server. | ⚖️ | 2 |
| R-UPD-2 | Update metadata signed (TUF); rollback and freeze attack protection; key rotation. | 🔒 | 2 |
| R-UPD-3 | Client update: check → notify → download → verify → stage → switch → health check → auto-rollback. | 🔒 | 2 |
| R-UPD-4 | User chooses Automatic / Notify only / Manual; channels Stable (default) / Beta / Nightly. | | 2 |
| R-UPD-5 | Update source abstracted behind `UpdateProvider`; GitHub is the first implementation. | | 2 |
| R-UPD-6 | Server updates: optional automatic, scheduled window or admin approval; pre-update snapshot; rollback. | 🔒 | 5 |
| R-UPD-7 | Windows binaries Authenticode-signed before public release (OD-3). | ⚖️🖥️ | 2 |

### Collaboration, identity, server
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-COL-1 | No developer-hosted accounts; local cryptographic identities. | 🔒 | 4 |
| R-COL-2 | Identity backup/restore and second-device linking. | 🔒 | 4 |
| R-COL-3 | "Start Collaboration Session" → copyable invite → guest joins without port forwarding or network config. | 🔒🖥️ | 4 |
| R-COL-4 | Direct P2P preferred; blind relay fallback; transport always encrypted with post-quantum hybrid handshakes. | 🔒⚡ | 4 |
| R-COL-5 | Concurrent editing with convergence, presence, offline edits and reconnection; local edits never wait for the network. | ⚡ | 4 |
| R-COL-6 | Project roles (Owner/Editor/Commenter/Viewer) enforced by the authority for every operation. | 🔒 | 4 |
| R-COL-7 | Invites expire, are single-use by default, revocable, and can require host approval. | 🔒 | 4 |
| R-COL-8 | Media shared on demand (proxy first), verified, permission-checked. | 🔒⚡ | 4 |
| R-COL-9 | **All collaboration content is end-to-end encrypted** (MLS): relays and servers store/forward ciphertext only (OD-7, ADR-022). | 🔒 | 4–5 |
| R-COL-10 | Every edit is signed by its author and verified (signature + role) by every receiving client; forged or out-of-role edits are rejected identically by all. | 🔒 | 4 |
| R-COL-11 | Removing a member rotates keys; removed devices can't read new content. | 🔒 | 4 |
| R-COL-12 | Safety-code verification between collaborators and with servers; hard warnings on key changes. | 🔒 | 4 |
| R-COL-13 | Project recovery kit so a project survives loss of all member devices. | 🔒 | 5 |
| R-COL-14 | Opt-in, per-project, visible "server Helper" that may read one project to produce proxies/thumbnails/renders; off by default. | 🔒⚡ | 5 |
| R-SRV-1 | Headless server hosts **encrypted** projects, revisions, membership/roles and backups; single binary, no external runtime/DB. | 🔒 | 5 |
| R-SRV-2 | Server identity is a keypair independent of IP/host; clients pin it and warn on change. | 🔒 | 5 |
| R-SRV-3 | Backup/export/import across machines and OSes preserves server identity. | 🔒 | 5 |
| R-SRV-4 | Clean data-dir separation (config, db, projects, media, backups, identity, cache, logs). | | 5 |
| R-SRV-5 | Audit log of security-relevant events. | 🔒 | 5 |
| R-SRV-6 | Rate limiting and abuse protection suitable for internet exposure. | 🔒⚡ | 5 |
| R-SRV-7 | Firewall changes only with explicit, explained consent; manual instructions available. | 🔒🖥️ | 5 |

### Security (cross-cutting)
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-SEC-1 | Media parsing/decoding isolated in worker processes (sandbox hardening later). | 🔒⚡ | 1 |
| R-SEC-2 | No secrets or project content in logs; redacted diagnostic reports. | 🔒 | 1 |
| R-SEC-3 | Supply-chain controls in CI (cargo-deny/audit, pinned Actions, SBOM, provenance). | 🔒 | 0 |
| R-SEC-4 | Fuzzing of all parsers of untrusted input (media probe outputs, project loader, protocol messages). | 🔒 | 1+ |
| R-SEC-5 | Established crypto libraries and protocols only. | 🔒 | all |

### UX and platform
| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| R-UX-1 | A first-time user can import a clip, trim it, add a caption and export a vertical video without instructions. | | 3 |
| R-UX-2 | Dark mode, high-DPI, keyboard shortcuts, drag and drop, context menus, tooltips. | 🖥️ | 1 |
| R-UX-3 | Command palette and search over all actions. | | 1 |
| R-UX-4 | Dockable panels and saved workspaces. | | 1 |
| R-UX-5 | Screen-reader support for standard UI; full keyboard operation. | 🖥️ | 1–3 |
| R-PLT-1 | Windows 10 22H2+/11 x64 and Linux x86-64 (glibc ≥ 2.35) for client and server. | 🖥️ | 1 |
| R-PLT-4 | **macOS** (proposed: Apple Silicon, macOS 14+) for client and server, feature-parity with Windows/Linux (OD-8). No dependency or design choice may preclude it before then. | 🖥️⚖️ | 13 |
| R-PLT-2 | Platform-specific code isolated behind `motix-platform` traits. | 🖥️ | 0 |
| R-PLT-3 | Hardware acceleration across NVIDIA, AMD and Intel; no vendor lock-in. | 🖥️⚡⚖️ | 1–3 |

### Performance targets (proposed, measured in CI or release checklist)
| ID | Target | Tags |
|----|--------|------|
| R-PRF-1 | 1080p60 H.264, 3 layers + transform + text: real-time playback at full res on an Intel Iris Xe–class iGPU. | ⚡🖥️ |
| R-PRF-2 | 4K60 10-bit HEVC: real-time at ½ res (or via proxy) on a mid-range discrete GPU (RTX 3060 / RX 6600 class). | ⚡ |
| R-PRF-3 | UI input-to-photon < 50 ms; no UI stall > 50 ms during any background work. | ⚡ |
| R-PRF-4 | Open a 1,000-clip project < 2 s; 10,000-item timeline scroll/zoom ≥ 60 fps. | ⚡ |
| R-PRF-5 | Collaboration: remote edit visible < 250 ms on direct connections (same continent). | ⚡ |
| R-PRF-6 | Cold start to usable UI < 3 s on SSD. | ⚡ |

---

## Should Have

| ID | Requirement | Tags | Phase |
|----|-------------|------|-------|
| S-1 | Compositions/precompositions, masks, blend modes, parenting, adjustment layers. | ⚡ | 6 |
| S-2 | Graph editor for keyframe curves. | | 6 |
| S-3 | Time remapping with speed ramps; frame blending; optical-flow interpolation. | ⚡ | 6/10 |
| S-4 | Transitions library. | | 3 |
| S-5 | Color correction (wheels, curves, HSL), LUT import (.cube), scopes. | ⚖️ | 6/7 |
| S-6 | HDR10, HLG and HDR10+ import, grading and export (incl. auto-generated HDR10+ dynamic metadata); Dolby Vision import with correct RPU handling; tone mapping. | 🖥️⚡⚖️ | 7 |
| S-7 | Audio mixer with buses, EQ, compressor, limiter, noise reduction, loudness meters, surround 5.1/7.1. | ⚡ | 8 |
| S-8 | Local AI: transcription/captions, translation, silence/scene/beat detection, background removal, auto-reframe, voice isolation. | ⚖️⚡🔒 | 9 |
| S-9 | Motion tracking (point/planar) and stabilization. | ⚡ | 10 |
| S-10 | Background render queue and export queue. | ⚡ | 3 |
| S-11 | Interchange: OpenTimelineIO, FCPXML, Premiere XML, EDL import/export. | ⚖️ | 11 |
| S-12 | Social templates (caption styles, intro/outro, aspect variants) stored as project snippets. | ⚖️ | 3 |
| S-13 | OS sandboxing of media and AI workers (restricted token/AppContainer; seccomp/Landlock). | 🔒🖥️ | 12 |
| S-14 | Linux Flatpak, server .deb/.rpm, OCI image, Unraid template. | 🖥️ | 2/5 |
| S-15 | MSI for managed Windows deployment; offline/air-gapped update bundles. | 🖥️ | 12 |
| S-16 | Localization framework (Fluent) and at least one additional language. | | 12 |
| S-17 | Server revision retention policies and trash with grace period. | | 5 |
| S-18 | Staged rollouts (percentage) for updates. | | 2 |

## Could Have

| ID | Requirement | Tags |
|----|-------------|------|
| C-1 | WebAssembly plugin system and scripting (ADR-016). | 🔒 |
| C-2 | CLAP/VST3 audio plugin hosting (separate process). | 🔒⚖️ |
| C-3 | OpenColorIO / ACES configs. | ⚖️ |
| C-4 | Timecoded comments and review/approval. | |
| C-5 | Distributed rendering on collaborators' machines or the server. | ⚡🔒 |
| C-6 | Short human-typeable invite codes via a rendezvous service with PAKE. | 🔒⚖️ |
| C-7 | ARM64 Windows/Linux builds. | 🖥️ |
| C-9 | Zero-copy GPU frame sharing between worker and main process. | ⚡🖥️ |
| C-10 | Optional external (cloud) AI providers, explicitly opt-in. | 🔒⚖️ |

## Research Required

| ID | Topic | Why research | Tags |
|----|-------|--------------|------|
| RR-1 | Codec patent exposure for bundled H.264/HEVC/AAC encoders in target markets (OD-5). | Legal | ⚖️ |
| RR-2 | Dolby Vision **export** (profile 8.1/8.4 RPU generation with open tools) — legality without a Dolby license. Import is planned (S-6). | Legal | ⚖️ |
| RR-3 | ProRes encoding legality/licensing. | Legal | ⚖️ |
| RR-4 | Fine-grained (per-track/per-clip) permissions over a CRDT. | No proven approach | 🔒 |
| RR-5 | ~~Zero-knowledge server mode~~ — resolved: adopted as the default (OD-7, ADR-022). | — | 🔒 |
| RR-6 | Linux HDR output (Wayland color management) maturity per compositor. | Moving target | 🖥️ |
| RR-7 | DirectML maintenance mode → Windows ML migration path and minimum Windows versions. | Vendor roadmap | 🖥️ |
| RR-8 | Zero-copy decode interop (D3D11/12 shared handles, Vulkan external memory, Vulkan Video) through wgpu-hal. | Technical risk | ⚡🖥️ |
| RR-9 | Semantic merge rules for timeline operations (ripple vs insert, etc.) — covered by the Phase 0 spike. | Technical risk | |
| RR-10 | Original high-quality optical-flow retiming competitive with commercial plug-ins. | Algorithmic | ⚡ |
| RR-11 | Post-quantum MLS ciphersuite (X-Wing) stability and migration path; history-key sharing for new members. | Draft standard | 🔒 |
| RR-12 | HDR10+ adopter program terms/fees for using the name in UI and export presets. | Legal | ⚖️ |
| RR-13 | GPL-3.0 compatibility of every crypto provider (e.g. aws-lc-rs components) and C library in the final build. | Legal | ⚖️ |
| RR-14 | Trademark clearance for the name MOTIX (existing MOTIX™ marks in software/hardware, class 9). | Legal | ⚖️ |

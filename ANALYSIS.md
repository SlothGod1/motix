# Specification Analysis

> Step 1 of the initial task. This document reviews the product specification as written, before any code exists.
> It is a point-in-time analysis (2026-09-27), revised the same day after the first owner decisions
> (GPL-3.0, macOS required, end-to-end encrypted collaboration, HDR10+/Dolby Vision). Findings that turn into work are tracked in
> [REQUIREMENTS.md](REQUIREMENTS.md), [DECISIONS.md](DECISIONS.md), [SECURITY.md](SECURITY.md) and
> [LICENSING_AND_COSTS.md](LICENSING_AND_COSTS.md).

Product name (OD-6, decided 2026-09-27): **MOTIX** (client) and **MOTIX Server** (headless server).
Binaries: `motix`, `motix-server`. Crate prefix: `motix-`. Project files: `.motix`. Invite links: `motix://`.

---

## 0. Executive summary

The specification is coherent and unusually thorough. Its principles (local-first, no accounts, P2P-first,
self-hostable, secure updates, bundle everything) reinforce each other well. The main problems are not
contradictions in intent, but **five places where the stated goals collide with physics, law or money**:

| # | Collision | Why it matters | Proposed resolution |
|---|-----------|----------------|---------------------|
| 1 | **"Bundle everything" vs. codec licensing and patents** | The best H.264/HEVC encoders (x264, x265) are GPL. H.264/HEVC/AAC carry patent-pool claims regardless of code license. | Decide the project license first (OD-1). Prefer hardware/OS encoders + royalty-free codecs by default; treat patent-encumbered software encoders as a separately reviewed component (OD-5). |
| 2 | **"$0 and no central infrastructure" vs. "collaborate without port forwarding"** | Roughly 1 in 10 peer connections cannot be made direct and needs a relay. *Someone* must run that relay. | Use the iroh stack (dial-by-key, hole punching, relay fallback). Relays: free public relays for alpha, any MOTIX Server doubles as a relay for its team, optional project relay later (OD-4). |
| 3 | **"End-to-end encrypted" vs. "server stores revisions, enforces permissions, makes backups"** | A server that validates and backs up project content must be able to read it. | **Revised (OD-7):** content is end-to-end encrypted with MLS; the server stores ciphertext, enforces transport-level roles from a signed role log, and every client verifies edits. Server-side proxies/renders only where a project owner enables the visible Helper role. |
| 4 | **"Windows installer, professional, no warnings" vs. "$0"** | Unsigned Windows installers trigger SmartScreen warnings — the opposite of "Download → Install → Done". | Free signing through SignPath Foundation *requires an OSI open-source license*. Otherwise ~US$10/month (Azure Artifact Signing) or an OV certificate. Tied to OD-1 and OD-3. |
| 5 | **"Rival After Effects" vs. "small team, $0"** | After Effects represents hundreds of engineer-years. | Accept it as a multi-year trajectory. Architecture must allow it; phases must each ship something usable. The social-video focus is the realistic wedge. |

**Owner decisions** (status in [DECISIONS.md → Owner decisions](DECISIONS.md#owner-decisions)):

1. **OD-1 Project license** — ✅ GPL-3.0-or-later.
2. **OD-2 UI toolkit** — ⏳ egui + wgpu recommended, pending Phase 0 spike.
3. **OD-3 Windows code signing** — ✅ SignPath Foundation (free) once public.
4. **OD-4 Relay infrastructure** — ⏳ recommendation recorded.
5. **OD-5 Patent-encumbered codecs** — ⏳ x264/x265 bundled in dev/alpha; legal check before public 1.0.
6. **OD-6 Product name** — ✅ MOTIX (trademark check recommended — RR-14).
7. **OD-7 Collaboration security** — ✅ end-to-end encrypted by default + opt-in server Helper.
8. **OD-8 macOS** — ✅ required platform, later phase; US$99/year Apple fee decided then.

---

## 1. Missing requirements

Things the specification does not state but that the architecture depends on. Each has a proposed default in
[REQUIREMENTS.md](REQUIREMENTS.md); items marked **(owner)** need a product decision.

### Product and scope
- **Product name and project license (owner).** The license decides which encoders can be bundled and whether free code signing is available.
- **macOS.** Not in the original spec. **Now required (OD-8)**, delivered in Phase 13; no architectural choice may rule it out meanwhile. Honest cost: US$99/year Apple Developer Program for a normal install experience.
- **Mobile.** Creators often edit on phones. *Default:* out of scope. A "preview on phone" companion is listed in IDEAS.md.
- **Minimum hardware.** *Proposed:* Windows 10 22H2+ / 11 x64; Linux x86-64 with glibc ≥ 2.35; a GPU with DirectX 12 (feature level 11_0) or Vulkan 1.2; 8 GB RAM minimum, 16 GB recommended. A CPU-only fallback exists but is not expected to play 4K in real time.
  - Note: Windows 10 reached end of mainstream support in October 2025. Supporting it costs testing effort; dropping it loses a large user share. **(owner, low urgency)**
- **Performance targets.** The spec says "fast" without numbers. *Proposed* measurable targets are listed in REQUIREMENTS.md (e.g. 1080p60 three-layer playback on an integrated GPU; UI never blocks for more than 50 ms).
- **Maximum sizes.** Resolution (8K?), project duration, number of clips, number of collaborators per session. *Proposed:* design for 8K and 10,000+ timeline items; test at 4K and 8 concurrent collaborators.
- **Localization and text.** Languages for the UI, right-to-left scripts, complex scripts and **emoji in captions** (critical for social content). *Proposed:* full Unicode shaping from day one; English UI first with translation infrastructure.
- **Accessibility target.** *Proposed:* screen-reader support for all non-canvas UI, full keyboard operation, WCAG 2.2 AA contrast for UI chrome.
- **Interchange.** Import/export with other editors (Premiere XML, Final Cut XML, OpenTimelineIO, EDL). Not requested but expected by professionals. Listed as a Should-have.
- **Fonts, templates and stock assets.** Where they come from and under what license. Social templates are a core workflow but have licensing implications.
- **Direct publishing** to TikTok/YouTube/etc. Not requested. Platform APIs need registered developer apps and are subject to platform terms (and sometimes review). Listed in IDEAS.md only.
- **Review and comments.** Timecoded comments, approvals. Common in collaborative editing; not requested. Listed in IDEAS.md.

### Operations and support
- **Crash reporting and telemetry policy.** The spec wants logs and privacy but does not say whether anything leaves the machine. *Proposed:* no telemetry. Crash reports are written locally; the user can choose to attach one to a GitHub issue. A self-hosted crash endpoint can come later.
- **Uninstall behaviour.** What happens to projects, caches, identity keys and downloaded AI models. *Proposed:* uninstall removes the application, runtimes and caches; it asks before removing downloaded models; it **never** removes projects or identity keys.
- **Corporate environments.** HTTP proxies, silent/managed installs (MSI), offline or air-gapped installation and updates.
- **Server data governance.** Retention of audit logs and revisions, how an operator deletes a user's data, backup schedule defaults.
- **Identity recovery.** With no accounts, losing a device's key means losing that identity. The recovery story (backup file, recovery phrase, second device) must be specified.

### Collaboration semantics
- **Who holds the media in a collaboration?** Project *state* is small; media is huge. The spec does not say whether collaborators must already have the media, receive proxies, or receive originals.
  - *Proposed:* content-addressed media transfer on demand, proxies first, originals on request, subject to permissions.
- **Offline peers in temporary sessions.** If the host goes offline, a temporary session has no one to sync with. The relay must not store data (spec §12), so there is no store-and-forward.
  - *Proposed:* this is acceptable. Persistent servers exist for asynchronous work.
- **Permission granularity.** Project-level roles are straightforward. Per-clip or per-track permissions are much harder with a CRDT. *Proposed:* start with project roles (Owner/Editor/Commenter/Viewer).

---

## 2. Contradictions and tensions

| # | Tension | Resolution |
|---|---------|-----------|
| C1 | "Bundle every dependency" (§3, §33) vs. "identify what cannot legally be bundled" (§6). GPL encoders, patent-encumbered codecs, CUDA (≈ 1 GB+), AI models (hundreds of MB to GBs each) cannot or should not go into one installer. | Three tiers: **Bundled** (core), **Managed components** (downloaded by the app on first need, pinned, hash-verified and signed, uninstalled with the app), and **System-provided** (GPU drivers, OS codecs). The user never runs a separate installer. See [ARCHITECTURE.md §9](ARCHITECTURE.md#9-dependency-strategy). |
| C2 | "E2E encryption" (§12) vs. server-side permissions, revisions and backups (§13). | **Revised:** MLS end-to-end encryption; server stores encrypted revisions/backups, enforces roles at transport level; clients verify signed edits. Cost: no server-side proxies/search/renders without the opt-in Helper. ADR-022 (supersedes ADR-011). |
| C3 | "No central infrastructure / $0" (§6, §10) vs. NAT traversal without configuration (§11). | Relays and address discovery are unavoidable for some connections. Use free public relays at first, make relays configurable, and let every MOTIX Server act as a relay. OD-4. |
| C4 | "No accounts" (§10) vs. persistent identity, revocation and multiple devices. | Local Ed25519 identities, device certificates and exportable backups. Revocation is per-server or per-project, not global. ADR-010. |
| C5 | "Automatic updates" (§17) vs. per-machine installs that need administrator approval (UAC) for every update. | Install the client **per-user** so updates need no elevation. The server installs as a system service; its updates run inside the service. ADR-013. |
| C6 | "Installer configures the firewall" (§43) vs. "never silently open network access". | Firewall changes only with explicit consent, shown in plain language. The client needs no inbound rule for P2P in most cases, because hole punching uses outbound UDP. |
| C7 | "Hardware acceleration" (§32) vs. "quality first" (§24). Hardware encoders give lower quality per bit than slow software presets. | Two export modes: **Fast** (hardware) and **Best quality** (software). Social presets default to the best-quality encoder permitted by the codec policy (OD-5) at the platform's bitrate. |
| C8 | "Accurate HDR preview on all platforms" (§26) vs. Linux. HDR output on Linux needs a Wayland compositor with color-management support; X11 has none. | Full HDR preview on Windows first; Linux HDR on supported Wayland compositors; SDR tone-mapped preview elsewhere. |
| C9 | "Dolby Vision workflows" (§26) vs. $0 and legal. Dolby Vision authoring requires Dolby licensing. | **Revised:** Dolby Vision *import* (iPhone footage) with correct RPU handling via libdovi (MIT) is planned in Phase 7; HDR10+ import and auto-generated export are planned; Dolby Vision *export* waits for legal review (ADR-023, RR-2). |
| C10 | "Beginners without a manual" vs. professional depth (§1, §30). | Progressive disclosure: one data model, several views (Simple timeline ↔ Advanced timeline/graph editor). Never two separate products. |
| C11 | "Server updates itself" (§17) vs. Docker, where images are immutable. | In containers the server only notifies. Admins update by pulling the new image tag. |
| C12 | "Relay must not store data" (§12) vs. "offline changes sync later" (§36). | Offline sync is guaranteed only against a persistent server. Temporary sessions resync only while both sides are online. |
| C13 | "Token expiration / rotation" (§16) vs. key-based authentication. With mutually authenticated QUIC, long-lived bearer tokens are unnecessary for the main protocol. | Use tokens only where they fit (admin web UI, invites, resumption). Connection authentication is key-based. |

---

## 3. Risk register

Severity: **H** = can sink the project or a phase, **M** = significant rework or cost, **L** = manageable.

### 3.1 Security risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| Malicious media exploits a decoder (FFmpeg has a steady stream of memory-safety CVEs) | H | Decode in separate, low-privilege **media worker processes** (crash isolation first, OS sandbox added in stages); keep FFmpeg updated; fuzz our demux/probe wrappers. |
| Malicious project file (oversized values, recursion bombs, path traversal in media references, SQLite tricks) | H | Strict schema validation, resource limits, no code in projects, SQLite defensive mode, relative paths resolved inside allowed roots only. |
| Internet-exposed server attacked (DoS, auth bypass, resource exhaustion) | H | Rust (memory safety), key-based mutual authentication, per-key and per-IP rate limits, message size caps, fuzzed protocol decoders, no default open admin surface. |
| Update channel compromise (stolen signing key, compromised GitHub account, compromised CI) | H | TUF: offline root key, key rotation, threshold signing later, expiring metadata, rollback and freeze protection; protected release environment with manual approval; provenance attestations. |
| Identity key theft from a user's machine | M | OS keystore where available; passphrase-encrypted fallback; per-device keys so one device can be revoked. |
| Supply-chain attack through crates or GitHub Actions | M | `cargo-deny` and `cargo-audit` in CI, lockfile committed, Actions pinned by commit SHA, minimal-permission tokens, dependency review before adding a crate. |
| AI models as an attack surface (malicious model files, pickle-style formats) | M | Use only ONNX/GGUF formats (no pickle), pinned hashes, signed model manifest, inference in a worker process. |
| Plugins/scripts (future) | M | WebAssembly sandbox with explicit capabilities; no native plugins without an explicit trust prompt. |

### 3.2 Architecture risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| CRDT doesn't fit timeline semantics (overlapping clips after a merge, broken ripple edits) | H | Typed schema layer over the CRDT, deterministic post-merge normalization, conflict indicators in the UI; Phase 0 spike with realistic operations. |
| UI toolkit hits a wall 2 years in (HDR, docking, accessibility, performance) | H | Phase 0 spike with hard pass/fail criteria; UI kept thin over an `motix-app` view-model layer so it can be replaced without touching the engine. |
| Rendering engine designed for 8-bit SDR, later needs HDR — full rewrite | H | Scene-linear half-float working space and explicit color metadata from Phase 1 (ADR-014). |
| Process isolation for decoding costs too much performance (copying frames) | M | Shared-memory frame transport first; zero-copy GPU handle sharing as a later optimization. Measured in Phase 0. |
| Single developer / small team bus factor | M | Documentation-first process (this set of docs), ADRs, tests as specification. |

### 3.3 Networking risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| Direct connections fail (symmetric NAT, carrier-grade NAT, corporate firewalls blocking UDP) | M | Relay fallback over HTTPS/443. Reported direct-connection rate for iroh is about 90%. |
| Dependence on a third party's public relays (n0) — availability, rate limits, terms can change | M | Configurable relay list, self-hostable relay (bundled into MOTIX Server), optional project relay (OD-4). |
| Large media transfer over relays is slow and costly for the relay operator | M | Prefer direct paths for bulk data; proxies first; relay bandwidth limits; the team's own server as relay. |
| Invite links leak (pasted into a public channel) | M | Short expiry, single-use by default, optional host approval ("Alex wants to join — Allow?"), revoke button. |

### 3.4 Licensing and legal risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| GPL obligations (x264, x265, Rubber Band) if the project is not GPL-compatible | H | OD-1. A GPL project may bundle them; a permissive or proprietary one may not. |
| Codec patents (H.264, HEVC, AAC; claims even against AV1/VP9) | H | Not solvable by code license. Prefer OS/hardware codecs (licensed by the OS or GPU vendor) and royalty-free formats; get legal review before public 1.0. OD-5. |
| AI model licenses (many popular models are non-commercial or GPL) | M | License review per model before adding it to the model catalog; tracked in LICENSING_AND_COSTS.md. |
| Trademark/trade dress ("After Effects", "Twixtor", "Sapphire") | L | Never use competitor names in UI or marketing claims; original implementations only. |
| ProRes encoding (reverse-engineered encoders, Apple licensing program) | L | Decode support is common; encode marked Research Required. |

### 3.5 Performance risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| Real-time playback of long-GOP 4K/HDR on mid-range machines | H | Hardware decode, proxies generated automatically in the background, adaptive preview resolution, render-ahead cache. |
| Optical-flow retiming, AI and denoise are far from real time | M | Background rendering with a visible cache bar; quality tiers; never block the UI. |
| Immediate-mode UI cost with huge timelines | M | Custom GPU-rendered timeline widget with virtualization; measured in Phase 0 spike. |
| CRDT document growth over long projects | M | Periodic snapshot compaction, history trimming policy for servers. |

### 3.6 Cross-platform risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| Linux fragmentation (glibc versions, Wayland vs X11, Mesa vs NVIDIA drivers) | M | Build on the oldest supported glibc; AppImage + Flatpak; test on at least Ubuntu LTS and Fedora with both Wayland and X11. |
| GPU vendor driver bugs | M | wgpu abstracts APIs, but bugs still differ. A GPU blocklist/workaround table, CPU fallback, and software-rasterizer tests in CI (WARP on Windows, lavapipe on Linux). |
| No GPUs in free CI runners | M | Deterministic rendering tests run on software rasterizers in CI; a manual hardware test matrix before each release. |
| Windows-only assumptions creeping in (paths, case sensitivity, file locking) | L | Platform code behind `motix-platform` traits; Linux CI on every commit. |

### 3.7 Installation risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| SmartScreen warnings on unsigned installers | H | OD-3. |
| Antivirus false positives (self-updating executables, unsigned DLLs, new binaries without reputation) | M | Sign every binary; submit false positives to vendors; avoid packers. |
| Large download size (FFmpeg + AI runtime + fonts) | M | Managed components download on first need; the base installer stays small (target < 150 MB). |
| Missing GPU driver features | M | Detect at startup; plain-language message with a link to the vendor's driver page (the one dependency we cannot bundle). |

### 3.8 Update risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| A bad update bricks the app | H | Side-by-side versions, launcher health check, automatic rollback, staged rollouts. |
| Project format migrated by a new version, then the user rolls back | H | Automatic pre-migration project backup; older versions open newer projects read-only with a clear message. |
| Server rollback after a database migration loses writes | M | Pre-update snapshot; the server stays in maintenance mode (no writes) until its post-update health check passes. |
| Updater itself has a bug and can't update | M | The launcher is minimal, updated rarely, with a separate update path and tests. |

### 3.9 Collaboration risks
| Risk | Sev | Mitigation |
|------|-----|-----------|
| Semantic conflicts (two people ripple-edit the same track) | H | Normalization rules, conflict markers, optional soft locks, presence showing who is where. |
| Permission enforcement over CRDT updates | M | Authority node (session host or server) validates and relays; viewers cannot publish updates. |
| Version skew between collaborators | M | Protocol version negotiation; same document-schema major version required to join a session. |
| Clock skew | L | CRDT uses logical clocks; wall-clock time is only for display. |

---

## 4. What this analysis changes in the spec

Nothing in the spec is rejected. The following are **recommended adjustments** for owner approval:

1. Add a license decision (OD-1) and a product name (OD-6) as prerequisites.
2. Reword "bundle everything" as "the user never installs anything separately": bundled + managed components + system drivers.
3. Reword "E2E encrypted" as "encrypted in transit end to end; relays are blind; a self-hosted server is trusted by its owner".
4. Add measurable performance targets.
5. Treat Dolby Vision authoring, ProRes encoding and per-object permissions as Research Required rather than roadmap items.
6. Add identity backup/recovery as a Must-have in the collaboration phase.

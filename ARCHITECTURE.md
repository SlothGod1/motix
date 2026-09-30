# Architecture

> Status: **Proposed** (Phase 0 starting). Revised 2026-09-27: macOS added as a required platform (later phase), end-to-end encrypted collaboration (ADR-022), HDR10+/Dolby Vision (ADR-023), GPL-3.0 license.
> Items marked *(spike)* are validated in Phase 0 before being marked Accepted in
> [DECISIONS.md](DECISIONS.md). This document describes the target architecture; [PHASES.md](PHASES.md) describes
> the order in which it is built.

## Contents
1. [Principles](#1-principles)
2. [Technology stack summary](#2-technology-stack-summary)
3. [System context](#3-system-context)
4. [Client architecture](#4-client-architecture)
5. [Project model and storage](#5-project-model-and-storage)
6. [Media pipeline](#6-media-pipeline)
7. [Rendering and preview](#7-rendering-and-preview)
8. [Color management and HDR](#8-color-management-and-hdr)
9. [Dependency strategy](#9-dependency-strategy)
10. [Audio](#10-audio)
11. [Identity](#11-identity)
12. [Networking](#12-networking)
13. [Collaboration](#13-collaboration)
14. [Persistent server](#14-persistent-server)
15. [Updates and release pipeline](#15-updates-and-release-pipeline)
16. [Packaging and installation](#16-packaging-and-installation)
17. [AI subsystem](#17-ai-subsystem)
18. [Extensibility](#18-extensibility)
19. [Cross-platform strategy](#19-cross-platform-strategy)
20. [Logging, diagnostics and crash handling](#20-logging-diagnostics-and-crash-handling)
21. [Security architecture (summary)](#21-security-architecture-summary)

---

## 1. Principles

1. **One engine, many surfaces.** The editing/rendering engine is a UI-independent Rust library. The GUI, the headless server (proxy generation, server-side renders) and test harnesses all use the same engine.
2. **Local-first.** Every edit applies to a local document first. The network only synchronizes. Losing the network never loses work and never blocks editing.
3. **Untrusted by default.** Media, project files, network peers and plugins are all hostile until validated. Parsing of complex formats happens in separate processes.
4. **Quality-preserving pipeline.** Scene-linear floating-point working space, explicit color metadata on every image, high-quality resamplers. No silent 8-bit round-trips.
5. **Everything disposable is separate.** Caches, proxies and temporary files live apart from projects and can be deleted at any time.
6. **Owned runtime.** The app ships with or manages everything it needs. The only thing the user may ever have to update is their GPU driver.
7. **Boring where possible, novel only where necessary.** SQLite, QUIC, TUF, FFmpeg are proven. Novelty is spent on UX and the editing model.

---

## 2. Technology stack summary

Full evaluations (alternatives, pros/cons, license, risks) are in [DECISIONS.md](DECISIONS.md).

| Area | Choice | ADR | Status |
|------|--------|-----|--------|
| Language (client, server, tools) | **Rust** (stable toolchain, pinned) | ADR-002 | Proposed |
| GPU abstraction | **wgpu** (DX12 / Vulkan / Metal; GL fallback) | ADR-004 | Proposed |
| UI toolkit | **egui** (+ egui_tiles docking, AccessKit) rendering through our own wgpu surface; Slint as fallback candidate | ADR-003 | *(spike)* — OD-2 |
| 2D vector / text rendering | **vello** (GPU vector renderer) + **cosmic-text / swash** (shaping, fonts) | ADR-019 | Proposed |
| Media decode/encode | **FFmpeg** libraries via FFI, pinned version built in CI, run in worker processes | ADR-005 | Proposed |
| Time representation | Integer ticks at 705,600,000 per second ("flicks") + rational frame rates | ADR-006 | Proposed |
| Project document / collaboration model | **Loro** CRDT under a typed schema layer, with an authority node for sessions | ADR-007 | *(spike)* |
| Project file container | **SQLite** (bundled) with append-only change journal | ADR-008 | Proposed |
| Networking | **iroh** 1.x (QUIC, dial-by-public-key, hole punching, relay fallback) + **iroh-blobs** | ADR-009 | *(spike)* |
| Identity | Local **Ed25519** keys; device certificates; safety codes; no accounts | ADR-010 | Proposed |
| Collaboration encryption | **MLS** (RFC 9420) via **OpenMLS**, hybrid post-quantum ciphersuite; signed edits; server stores ciphertext only | ADR-022 | Accepted direction; *(spike)* |
| Async runtime | **tokio** | ADR-002 | Proposed |
| TLS / crypto | **rustls** (aws-lc-rs provider for post-quantum X25519MLKEM768), RustCrypto / dalek crates, **BLAKE3**, **age**. No custom crypto. | ADR-010, ADR-022 | Proposed |
| Update security | **TUF** metadata via the **tough** crate + Authenticode + GitHub build provenance | ADR-012 | Proposed |
| Install/update layout | Per-user side-by-side versions + small launcher; Velopack evaluated | ADR-013 | *(spike)* |
| Color | Own color module (WGSL + CPU reference) from Phase 1; OpenColorIO for pro configs later | ADR-014 | Proposed |
| HDR formats | HDR10, HLG, HDR10+ (read + auto-generated dynamic metadata), Dolby Vision import via **libdovi**; DV export pending legal | ADR-023 | Proposed |
| Audio I/O & DSP | **cpal**, **rubato**/soxr resampling, **ebur128**, Signalsmith Stretch | ADR-020 | Proposed |
| AI inference | **ONNX Runtime** (DirectML / CPU / optional CUDA; CoreML on macOS) + **whisper.cpp** (Vulkan; Metal on macOS) in a worker process | ADR-015 | Proposed |
| Plugins / scripting (later) | **WebAssembly** (wasmtime) sandbox; WGSL shader effects | ADR-016 | Deferred |
| Serialization (IPC, protocol) | **serde** + **postcard** with explicit versioned envelopes and size limits | ADR-021 | Proposed |
| Build / CI / release | Cargo workspace + `xtask`; **GitHub Actions**; **GitHub Releases**; TUF repo on GitHub Pages | ADR-012 | Proposed |
| Windows installer | Inno Setup (per-user) initially; MSI (WiX) later for managed deployments | ADR-013 | Proposed |
| Linux packaging | AppImage (self-updating) + Flatpak; .deb/.rpm for the server | ADR-013 | Proposed |
| macOS packaging (later phase) | Signed + notarized `.app` in a DMG; bundle-swap updates with rollback; launchd for the server | ADR-024 | Deferred (OD-8) |
| Project license | **GPL-3.0-or-later** | OD-1 | Decided |

---

## 3. System context

```
                           ┌──────────────────────────────────────────────┐
                           │           GitHub (public infrastructure)     │
                           │  Releases (binaries) · Pages (TUF metadata)  │
                           └───────────────▲──────────────────────────────┘
                                           │ HTTPS: update checks & downloads
                                           │ (signed; TUF verified)
 ┌──────────────────────┐    QUIC (E2E)    │     ┌──────────────────────┐
 │  MOTIX (client)   A   │◄────────────────────────►  MOTIX (client)   B    │
 │  local project state │   direct (hole punched)  │  local project state │
 └─────────┬────────────┘   or via blind relay     └─────────┬────────────┘
           │                        ▲                        │
           │                ┌───────┴────────┐               │
           │                │  Relay (blind) │               │
           │                │ public / team  │               │
           │                └───────▲────────┘               │
           │                        │                        │
           │     QUIC (mutually authenticated by key)        │
           └────────────────►┌──────┴───────────────┐◄───────┘
                             │   MOTIX Server       │  (optional, self-hosted)
                             │ encrypted projects &  │
                             │ media · role logs ·   │
                             │ relay · backups       │
                             └───────────────────────┘
```

- **No component is operated by the developer that the core editor depends on.** GitHub is used only for updates, and updates can come from any mirror because they are signed.
- Relays see only encrypted QUIC packets. **Servers see only end-to-end encrypted project data** (ADR-022) unless a project owner enables the Helper role for that project.

---

## 4. Client architecture

### 4.1 Process model

```
┌──────────────────────────────── motix (main process) ─────────────────────────────────┐
│                                                                                        │
│  UI thread ─── egui frame loop, input, docking, view-models (motix-app / motix-ui)          │
│      │                                                                                 │
│      ▼ commands                                                                        │
│  Document ─── Loro doc + typed schema (motix-schema) · undo manager · change events      │
│      │                                  │                                              │
│      ├── Journal writer (motix-project) ───┘  append-only, fsync'd, crash-safe           │
│      │                                                                                 │
│  Playback / render scheduler (motix-playback) ── render graph (motix-render) ── wgpu device │
│      │                                                                                 │
│  Audio engine (motix-audio) ── real-time thread, lock-free, cpal output                  │
│      │                                                                                 │
│  Async runtime (tokio) ── networking (motix-net), sync (motix-sync), updates (motix-update)    │
│  CPU pool (rayon) ── thumbnails compositing, waveform reduction, hashing              │
└───────────┬─────────────────────────────┬──────────────────────────────┬──────────────┘
            │ IPC (bounded, validated)    │                              │
   ┌────────▼─────────┐        ┌──────────▼─────────┐          ┌─────────▼─────────┐
   │ motix-media-worker  │  ...N  │ motix-media-worker    │          │  motix-ai-worker     │
   │ probe / decode   │        │ encode / export    │          │  ONNX / whisper   │
   │ (sandboxed)      │        │ (sandboxed)        │          │  (sandboxed)      │
   └──────────────────┘        └────────────────────┘          └───────────────────┘
          frames via shared memory (later: shared GPU handles)
```

**Why separate worker processes?** FFmpeg and ML runtimes are large C/C++ codebases that parse untrusted input.
A crash or exploit in a worker must not take down the editor or access the user's files. Workers:
- receive only an already-open file handle or a read-only path grant, never arbitrary filesystem access;
- run with reduced privileges (Windows: restricted token + job object, later AppContainer; Linux: seccomp-bpf + Landlock + no-new-privs; macOS: Seatbelt sandbox profile);
- are restarted automatically if they crash; the clip shows "Couldn't decode this frame" instead of the app dying.

The OS sandbox is introduced incrementally (process isolation in Phase 1, sandbox hardening in a later phase — see [PHASES.md](PHASES.md)). The *process boundary* is there from day one because retrofitting it is expensive.

### 4.2 Crate layout

```
crates/
  motix-core         ids (UUIDv7), time (flicks, rationals), errors, limits
  motix-probe        memory-safe header inspection on import (MP4/MOV/MKV/WebM/WAV/PNG/JPEG) — ADR-025
  motix-schema       typed project schema over Loro, validation, normalization, migrations
  motix-project      project container (SQLite), journal, autosave, recovery, named versions
  motix-commands     edit operations (split, trim, ripple, keyframe...), undo grouping
  motix-media        media types, probe results, frame/audio buffers, worker IPC protocol
  motix-render       render graph, GPU evaluation, built-in effects, frame cache
  motix-color        color spaces, transfer functions, tone mapping (CPU reference + WGSL)
  motix-text         shaping, font management, text layout for titles and captions
  motix-audio        audio graph, mixing, DSP, metering, loudness
  motix-playback     transport, A/V sync, adaptive quality, render-ahead
  motix-ai           model manager, task API (client side of motix-ai-worker)
  motix-identity     keys, device certificates, keystore integration, safety codes, recovery kits
  motix-e2ee         MLS groups (OpenMLS), content/asset encryption, signed role log
  motix-net          iroh endpoint, ALPN protocols, invites/tickets, rate limiting
  motix-sync         collaboration engine: authority + peer roles, presence
  motix-update       TUF client, download/stage/verify/apply/rollback (client & server)
  motix-platform     OS-specific services behind traits (paths, keystore, sandbox, HDR, firewall)
  motix-app          UI-agnostic application state, view-models, action registry
  motix-ui           egui front end (panels, timeline widget, viewer)
apps/
  motix           client binary
  motix-launcher launcher / version selector / rollback
  motix-server   headless server
  motix-media-worker media worker binary
  motix-ai-worker    AI worker binary
xtask/            build, package, FFmpeg build, release helpers
```

Dependency rule: **arrows point down only**. `motix-ui` → `motix-app` → engine crates → `motix-core`. Engine crates never depend on `motix-ui`; `motix-server` never links `motix-ui`. This is what makes the UI toolkit replaceable.

### 4.3 UI structure

- **Action registry** (`motix-app`): every user-visible operation is a named action with an id, label, default shortcut, enablement predicate and handler. Menus, the command palette, context menus, shortcuts and (later) scripting all call the same registry.
- **Workspaces**: layout of dockable panels (egui_tiles) saved per user. Presets: *Simple* (viewer + one timeline + inspector), *Edit*, *Motion*, *Color*, *Audio*, *Captions*.
- **Progressive disclosure**: the same composition can be shown in the Simple timeline (tracks, clips, big handles) or the Advanced timeline (properties, keyframes, graph editor). Switching views never changes data.
- **Viewer and timeline** are custom GPU widgets drawn with wgpu inside the egui frame (the viewer shows the render engine's output texture directly; no CPU copy).
- **Accessibility**: AccessKit tree for all standard widgets; custom widgets (timeline, viewer) expose an accessible structure (tracks → clips with names, times and actions).

---

## 5. Project model and storage

### 5.1 Domain model

```
Project
 ├── settings (default fps, resolution, working color space, audio sample rate)
 ├── Assets            (media files, images, audio, fonts; content hash + locator hints)
 ├── Compositions      (a "sequence" is just a composition with tracks)
 │     ├── settings (width, height, fps (rational), duration, color space, audio layout)
 │     └── Tracks  (video | audio | caption), ordered
 │           └── Items   (clip | text | shape | solid | adjustment | nested composition | generator)
 │                 ├── source ref, source in/out, timeline start, speed/time-remap curve
 │                 ├── transform, opacity, blend mode, parent (item id)
 │                 ├── Masks[]
 │                 ├── Effects[]  (effect type id + version + parameters)
 │                 └── Properties → Keyframes[] (time, value, interpolation, easing)
 ├── Markers, Captions (styled, word-timed)
 ├── Bins / folders (movable tree)
 └── Named versions (pointers into history)
```

- **One concept for sequences and compositions.** Premiere-style editing and After-Effects-style layering are two *views* over the same model: a track holding one long item behaves like an AE layer.
- **Stable IDs** (UUIDv7) for every entity; references are by id, never by array index.
- **Time** is `i64` ticks at 705,600,000/s (flicks), which divides evenly into all common frame rates (23.976 = 24000/1001, 25, 29.97, 30, 50, 59.94, 60, 120) and audio rates (44.1k, 48k, 96k). Frame rates are stored as exact rationals. No floating-point time in the model.
- **Effects are versioned** (`effect_id@version`), so an improved algorithm never silently changes an old project's look. Upgrading an effect is an explicit action.

### 5.2 Document representation *(spike)*

The canonical in-memory state is a **Loro** CRDT document (maps, movable lists, movable tree, text). `motix-schema`
provides typed accessors (`Composition`, `Track`, `Item`…) that read and write the Loro document, and enforces:

- **Validation** on every import/receive: types, ranges, sizes, reference integrity.
- **Deterministic normalization**: invariants a CRDT cannot guarantee (e.g. no two items overlap on one track) are resolved by a pure function of the merged state, so every peer computes the same result. Example: if two concurrent edits produce overlapping clips, the clip with the later logical timestamp is pushed to a newly visible "conflict lane" and marked for the user, rather than silently deleted.
- **Undo/redo** through Loro's undo manager, which undoes *my* changes without undoing collaborators' changes.

Why a CRDT even for local-only use: the same representation gives crash-safe incremental persistence (append updates), history/named versions, offline collaboration and merging — one model instead of two.

### 5.3 Project file (`.motix`)

A single SQLite database file (the "SQLite as an application file format" pattern):

| Table | Contents |
|-------|----------|
| `meta` | format version, min reader version, project id, created/modified, app version |
| `journal` | append-only Loro update blobs (seq, peer, timestamp, bytes, checksum) |
| `snapshots` | periodic compacted Loro snapshots (journal before a snapshot can be trimmed) |
| `versions` | named versions: name, author, time, Loro frontier |
| `assets` | asset id, BLAKE3 content hash, size, media fingerprint, relative path, last-known absolute path, volume id |
| `blobs` | small embedded assets (e.g. a logo PNG the user chose to embed) |

- **Autosave = journaling.** Every committed edit is appended in a WAL transaction; fsync is batched (≤ 1 s) or immediate on idle. After a crash or power loss, at most the last ~1 s is lost. There is no "Save" required; "Save" creates a named checkpoint.
- **Rolling backups.** Every 10 minutes of activity a compact copy goes to `<project>.motix-backups/` (keep last N). Protects against file-system level corruption and user mistakes.
- **Crash recovery.** On start, projects with an unclean-shutdown marker are verified (`PRAGMA integrity_check`, journal checksums) and reopened; if verification fails, the newest good backup is offered.
- **Untrusted file handling.** Projects are opened with SQLite defensive settings (`SQLITE_DBCONFIG_DEFENSIVE`, `trusted_schema=OFF`, no extensions, size limits), then validated by `motix-schema` before any object reaches the engine.

### 5.4 Schema versioning and migration

- `format_version` (integer) in `meta`. Each version has a **pure migration function** `vN → vN+1` over the document, tested with fixture projects from every released version.
- **Forward compatibility:** unknown fields are preserved on round-trip. A project whose `min_reader_version` is newer than the app opens **read-only** with a clear message ("Made with a newer version — update to edit").
- **Before migrating**, the original file is copied to the backup folder so a rollback of the app never strands the project.

### 5.5 Assets and relinking

- Assets are identified by **content hash + fingerprint** (size, duration, codec, first-frame hash), not by path.
- Locator hints, tried in order: path relative to the project file → last absolute path → same volume id with different drive letter/mount point → user search folders → hash index of known media.
- Missing media shows a non-blocking banner with "Locate…" (finding one file relinks all siblings in the same folder).
- "Package project" creates a portable folder/zip with the project and all used media (optionally trimmed).

### 5.6 Storage locations

| Kind | Windows | Linux | macOS (later phase) | Disposable |
|------|---------|-------|---------------------|-----------|
| Projects | wherever the user saves them (default `Documents\MOTIX Projects`) | `~/Videos/MOTIX Projects` | `~/Movies/MOTIX Projects` | **No** |
| Settings, identity keys | `%APPDATA%\MOTIX\` (keys in Credential Manager/DPAPI) | `$XDG_CONFIG_HOME/motix/` (keys in Secret Service) | `~/Library/Application Support/MOTIX/` (keys in Keychain) | No |
| Proxies | `%LOCALAPPDATA%\MOTIX\Proxies\` or user-chosen fast drive | `$XDG_CACHE_HOME/motix/proxies` | `~/Library/Caches/MOTIX/Proxies` | Yes |
| Render/preview cache | `%LOCALAPPDATA%\MOTIX\Cache\` | `$XDG_CACHE_HOME/motix/cache` | `~/Library/Caches/MOTIX/Cache` | Yes |
| Managed components (FFmpeg variants, AI runtimes, models) | `%LOCALAPPDATA%\MOTIX\Components\` | `$XDG_DATA_HOME/motix/components` | `~/Library/Application Support/MOTIX/Components` | Re-downloadable |
| Logs, crash reports | `%LOCALAPPDATA%\MOTIX\Logs\` | `$XDG_STATE_HOME/motix/logs` | `~/Library/Logs/MOTIX` | Yes |

Caches are keyed by content hashes and can be deleted at any time; the UI has "Clean cache" with sizes shown.

---

## 6. Media pipeline

```
 File ──► [worker] probe ──► MediaInfo (validated) ──► Asset
                    │
 Playback request ──► [worker] demux ─► decode (HW or SW) ─► convert to standard layout
                                                              │ shared memory ring (Phase 1)
                                                              │ shared GPU texture (later)
                                                              ▼
                                                   upload/import into wgpu texture
                                                   + color metadata (primaries, transfer, range, matrix)
```

- **Quick probe on import** (`motix-probe`, ADR-025): pure-Rust header reading in-process — size, rotation, frame rate, VFR, duration, codecs, bit depth, colour/HDR flags, audio streams. Drives the "match project?" question and clip placement instantly.
- **Full probe** in a worker, with timeouts and memory limits; overrides the quick probe when they disagree. Output is a validated `MediaInfo` (streams, codecs, color metadata, VFR detection, rotation, HDR metadata).
- **Decode**: FFmpeg with hardware acceleration where available — Windows: D3D11VA/D3D12VA (all vendors), NVDEC, QSV; Linux: VAAPI, NVDEC, Vulkan Video; macOS: VideoToolbox. Automatic fallback to software decoding.
- **HDR metadata**: static (mastering display, MaxCLL/MaxFALL), HDR10+ dynamic metadata and Dolby Vision RPUs are extracted and kept with the frames (ADR-023).
- **Variable frame rate** media is conformed through a timestamp map, never by assuming constant frame rate.
- **Frame format**: decoded frames keep native bit depth (8/10/12/16-bit, YUV 4:2:0/4:2:2/4:4:4) until the GPU converts them to the working space with the correct matrix, range and transfer function. Chroma upsampling happens on the GPU with a proper filter.
- **Proxies**: generated automatically in the background for heavy media (long-GOP 4K+, 10-bit HEVC, high-bitrate), stored as intra-frame codecs with explicit color tags. Proxy use is automatic and visible ("Proxy" badge), and export always uses originals.
- **Thumbnails & waveforms**: produced by workers, cached by content hash.
- **Image sequences**: treated as a single asset with a frame pattern.
- **Export**: the render graph produces frames at full quality → encode worker → mux. Two modes: **Best quality** (software encoders) and **Fast** (hardware encoders). Social presets are data files describing container, codec, resolution, frame rate, bitrate, loudness target and platform limits.

### Encoders by license class (OD-1 = GPL-3.0, OD-5)

| Output | Royalty-free / permissive | Hardware / OS provided | GPL (usable — project is GPL-3.0) |
|--------|--------------------------|------------------------|------------------------------------------|
| AV1 | SVT-AV1, libaom | NVENC, AMF, QSV, VAAPI (newer GPUs) | — |
| VP9 | libvpx | QSV/VAAPI (some) | — |
| H.264 | OpenH264 (Cisco binary model) | NVENC, AMF, QSV, VAAPI, Media Foundation, VideoToolbox (macOS) | **x264** (default "Best quality") |
| HEVC | — | NVENC, AMF, QSV, VAAPI, Media Foundation (with OS extension), VideoToolbox (macOS) | **x265** (default "Best quality"; writes HDR10+ metadata) |
| Audio | Opus, FLAC, FFmpeg native AAC (patent status: see LICENSING) | — | — |
| Intermediate | FFV1, DNxHR (FFmpeg native), CineForm | ProRes via VideoToolbox (macOS — Apple's own licensed encoder) | — |

---

## 7. Rendering and preview

### 7.1 Render graph

- For a composition at time *t*, the engine builds a **DAG**: sources → per-item transforms, masks, effects → blends into parent → adjustment layers → output transforms.
- Nodes are evaluated on the GPU (wgpu compute/fragment shaders written in WGSL). Every node declares its **region of definition** and **region of interest**, so only needed pixels are computed (important for blurs, crops and 8K).
- **Working format:** premultiplied RGBA, 16-bit float by default (32-bit float per project option), in a scene-linear working color space (see §8).
- **Resampling** (scaling, rotation) uses high-quality filters (Lanczos/bicubic with correct linear-light filtering), with a draft mode for interaction.
- **Node versioning:** each effect node implementation is keyed by `effect_id@version`; old versions stay available so old projects render identically.
- **CPU reference path:** every built-in node has a CPU reference implementation used in tests (and as a last-resort fallback). GPU output is compared against it within tolerances.

### 7.2 Caching

- **Frame cache** (RAM → disk) keyed by `hash(node subtree parameters, source content hashes, time, resolution, quality)`. Editing one parameter invalidates only downstream frames.
- Timeline shows a **cache bar** (green = rendered, grey = not yet).
- **Render-ahead**: while idle or playing, a background task renders upcoming frames at the current preview quality.

### 7.3 Playback

- Audio clock is master; video frames are presented against it.
- **Adaptive quality**: if frames can't be delivered in time, drop to ½ or ¼ resolution, then to proxies, then to showing cached frames only — and tell the user ("Preview at ½ resolution to keep real time").
- The UI thread never waits on rendering; the viewer shows the most recent completed frame.

### 7.4 Text and vector graphics

- **cosmic-text / swash / harfrust** for Unicode shaping (complex scripts, RTL, emoji, fallback fonts).
- **vello** renders text, shapes and caption styles as vectors on the GPU at any scale (crisp at 8K, animatable). CPU fallback via vello's CPU renderer.

---

## 8. Color management and HDR

- Every image in the pipeline carries **color metadata**: primaries, transfer function, matrix, range, and HDR mastering metadata when present.
- **Working space**: scene-linear, default **linear Rec.709/sRGB primaries** for SDR projects and **linear Rec.2020** for HDR projects (ACEScg as an option once OpenColorIO is integrated). Selected per project; changing it is non-destructive.
- **Input transforms** are derived automatically from media metadata (with user override for mistagged files — a very common real-world problem).
- **Output transforms**: SDR Rec.709 (BT.1886), sRGB, Display P3; HDR10 (PQ, Rec.2020) and HLG.
- **HDR formats** (ADR-023):
  - **HDR10+**: dynamic metadata from sources is read and drives per-scene tone mapping; on export we can *generate* ST 2094-40 metadata by analyzing each scene (brightness histograms/percentiles) and write it through x265 (HEVC) or AV1 T.35 metadata. YouTube accepts this.
  - **Dolby Vision**: sources (iPhone profile 8.4, profiles 5 and 8.1) are decoded and their RPU metadata applied via libdovi (or libplacebo) so they look as intended; profile 8.4's HLG base layer is also usable directly. **Export** of Dolby Vision is technically possible with open tools but is licensed technology — kept out of shipped builds until a legal review (RR-2).
- **Tone mapping** (HDR → SDR and SDR-in-HDR placement) with selectable operators (BT.2390 EETF, a filmic curve), used both for export and for preview on SDR displays.
- **Display transform** is separate from output transform: the viewer knows what the *display* can show.
  - **Windows HDR on, SDR project:** the swapchain uses `scRGB` (linear, extended range) and the display transform maps SDR reference white to Windows' *SDR content brightness* setting, so the preview matches what an SDR viewer sees rather than relying on OS auto-conversion. HDR projects are shown with true HDR values up to the display's reported peak, with tone mapping above it.
  - wgpu exposes the needed surface color spaces (`ExtendedSrgbLinear`, `Bt2100Pq`) on DX12 and Vulkan. Validated in the Phase 0 HDR spike.
  - **Linux:** HDR preview on Wayland compositors that support the color-management protocol; SDR tone-mapped preview otherwise.
  - **macOS (later phase):** EDR via Metal (`ExtendedSrgbLinear` / `ExtendedDisplayP3` surfaces) — the most mature desktop HDR pipeline.
- **Scopes** (waveform, vectorscope, histogram, false color) computed on the GPU from the output-transformed image.
- **OpenColorIO** (BSD-3) is integrated later for studio configs and ACES; the built-in module covers the common cases without a C++ dependency in Phase 1.

### 8.1 Mixed sources in one project

A project can freely mix sizes, frame rates, bit depths and dynamic ranges. The **project settings decide the output**; each clip is converted on the way in:

| Property | How a differing clip is handled |
|----------|--------------------------------|
| Size / shape | Placement mode per clip: **Scale to fit** (default), **Scale to fill** (crop), **Stretch**, **Original size**. Scaling runs on the GPU in linear light with a high-quality filter (Lanczos/bicubic for upscaling, area-averaging for downscaling). Phone rotation metadata is applied first. |
| Frame rate | Clips play in real time at the project rate: frames are repeated or skipped by timestamp (nearest frame). Frame blending and optical-flow retiming are later options per clip. VFR sources use their timestamp map. |
| Bit depth | Decoded at native depth (8/10/12-bit), converted to 16-bit float on the GPU. Output depth is a project setting (8 or 10; HDR forces 10). |
| Colour / dynamic range | Each clip gets an input transform from its metadata into the linear working space (Rec.709 primaries for SDR projects, Rec.2020 for HDR projects). **SDR in an HDR project:** Rec.709 → Rec.2020 gamut conversion and SDR white placed at **203 nits (ITU-R BT.2408 reference white)** so it sits naturally next to HDR footage; optional inverse tone mapping ("expand to HDR") later. **HDR in an SDR project:** tone-mapped (BT.2390 EETF by default). |

**Worked example (owner question):** an 8-bit SDR 1920×1080 clip and a 10-bit HDR10 3840×2160 clip, exported as 10-bit HDR10 UHD. Project: 3840×2160, HDR10, 10-bit. The HDR10 clip passes through untouched (PQ → linear → PQ). The SDR clip is upscaled 2× with a high-quality filter, converted from Rec.709 to Rec.2020, and mapped so its white is 203 nits. Export: HEVC Main10 (x265) or AV1 10-bit with PQ transfer, Rec.2020 primaries and HDR10 static metadata (mastering display, MaxCLL/MaxFALL computed from the actual frames). The SDR footage looks exactly as it did — it can't gain highlight detail it never recorded — while the HDR footage keeps its full range.


---

## 9. Dependency strategy

**Goal:** Download → Install → Open → Use. The user never installs anything separately.

### 9.1 Tiers

| Tier | Meaning | Examples |
|------|---------|----------|
| **Built-in** | Compiled into our binaries (static linking) | Rust std, tokio, wgpu, egui, SQLite, rustls, Loro, iroh, vello, audio DSP, color module |
| **Bundled** | Separate files shipped inside the installer, pinned, loaded from the app folder only | FFmpeg shared libraries (GPL build incl. x264/x265, dav1d, SVT-AV1 — OD-1/OD-5), ONNX Runtime CPU + DirectML (Windows), default fonts (OFL) |
| **Managed component** | Downloaded **by the app** on first need, from a pinned URL, verified against a TUF-signed manifest (hash + size), stored in the Components folder, removed on uninstall | AI models, CUDA/TensorRT execution provider for NVIDIA users, additional font packs, optional codec packs (OD-5), Linux Vulkan-video extras |
| **System-provided** | Cannot legally or technically be bundled; detected, never required to be installed manually except drivers | GPU drivers (and their Vulkan/DX12 implementations, NVENC/AMF/QSV encoders), Windows Media Foundation codecs, Linux glibc/Wayland/X11/Mesa, macOS Metal/VideoToolbox/CoreML |

Managed components use the same TUF-signed metadata as app updates. Their UX is: "Transcription needs a 150 MB speech model. Download now?" → progress bar → done. Never "go install X".

### 9.2 Component-by-component

| Dependency | Tier | Notes |
|------------|------|-------|
| C/C++ runtime (MSVC CRT) | Built-in | Static CRT (`+crt-static`), so no VC++ Redistributable installer. |
| .NET, Java, Python, Node.js, Docker | **Not used** | Explicit non-goal (ADR-017). AI models are run through ONNX Runtime / whisper.cpp, not Python. |
| WebView2 / browser engine | Not used | UI is native wgpu. |
| Database | Built-in | SQLite compiled in (client and server). No database server. |
| TLS | Built-in | rustls; no OpenSSL DLLs. |
| FFmpeg | Bundled | GPL build (x264, x265, dav1d, SVT-AV1, libvpx, opus…) from pinned source in CI with an explicit configure line (recorded in LICENSING_AND_COSTS.md). Loaded only from the app directory (DLL search path hardened). |
| libdovi | Built-in (Phase 7) | Dolby Vision RPU parsing/application (MIT, Rust). |
| OpenMLS + aws-lc-rs | Built-in | End-to-end encryption and post-quantum handshakes (ADR-022). |
| GPU acceleration SDKs | Built-in / System | NVENC/AMF/QSV headers are compiled into FFmpeg; the runtime parts ship with GPU drivers. No SDK installs for users. |
| Vulkan loader | System (Windows: ships with GPU drivers); bundled in AppImage if needed | DX12 is part of Windows 10/11. |
| ONNX Runtime | Bundled (CPU; DirectML on Windows) + Managed (CUDA/TensorRT for NVIDIA, MIGraphX/ROCm on Linux) | CUDA libraries are large (hundreds of MB to > 1 GB) and only downloaded for users who opt in. |
| whisper.cpp | Built-in (static) | Vulkan backend = GPU transcription on any vendor. |
| AI models | Managed | Pinned by hash; license-vetted per model. |
| Fonts | Bundled (UI + a small starter set) / Managed (larger packs) | OFL-licensed. Users' system fonts are also available. |
| Patent-encumbered encoders | Bundled in dev/alpha builds (OD-5 direction B); final form after legal check | Hardware/OS encoders always offered as "Fast" export. |
| Update/installer tooling | Built-in | Our own launcher + TUF client. |
| Server: web server, DB, runtime | Built-in | Single `motix-server` binary. |

### 9.3 What cannot be bundled (must be system-provided or user-owned)

- **GPU drivers.** The one thing a user may need to update. The app detects outdated/broken drivers and shows a plain-language message with a link to the vendor's download page.
- **OS HEVC decoding/encoding on Windows via Media Foundation** requires Microsoft's HEVC extension on some systems; we don't depend on it (FFmpeg decodes HEVC in software/with D3D11VA).
- **Dolby Vision tooling, Apple ProRes official SDKs, fdk-aac** (non-free license) — not bundled.
- **CUDA** is redistributable under NVIDIA's terms but is delivered as a managed component, not in the base installer, because of size.

### 9.4 Isolation rules

- Libraries are loaded only from our own directories (Windows: `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32)`; Linux: `RPATH=$ORIGIN`).
- We never read `PATH` to find tools, never call an external `ffmpeg.exe`, and never write into system folders (except the server's service registration, with consent).

---

## 10. Audio

- **Engine**: a real-time audio thread with a lock-free graph (no allocations, no locks, no I/O on the RT thread). 32-bit float internally at the project sample rate.
- **Layouts**: mono, stereo, 5.1, 7.1 (channel layouts are explicit metadata, not implied by channel count). Ambisonics listed as an idea.
- **Resampling**: high-quality windowed-sinc (rubato or soxr) for mismatched sources; quality tiers for preview vs export.
- **Time-stretch / pitch**: Signalsmith Stretch (MIT) by default; Rubber Band (GPL, usable since OD-1) as a higher-quality option, chosen by listening tests.
- **Processing**: EQ, compressor, limiter, gate, de-esser, noise reduction (RNNoise/DeepFilterNet), voice isolation (AI, §17).
- **Metering**: peak/true-peak, RMS, EBU R128 / ITU-R BS.1770 loudness (integrated, short-term, momentary, LRA). Social presets carry loudness targets.
- **Automation**: keyframes on any audio parameter with sample-accurate interpolation.
- **Device I/O**: cpal (WASAPI on Windows; PipeWire/PulseAudio/ALSA on Linux). Device loss is handled without stopping the app.

---

## 11. Identity

No accounts. Identity is a keypair the user's app creates on first run.

```
User identity key (Ed25519)  ── "who I am" across devices; kept in the OS keystore; backed up by the user
   │ signs
   ├── Device certificate #1 (device key = iroh NodeId, name "Andrew's desktop", created, expiry)
   └── Device certificate #2 ("Andrew's laptop")

Public profile (self-signed): display name, avatar hash, user public key
```

- **First run**: generate keys silently. The user picks a display name. No email, no password.
- **Keystores**: Windows Credential Manager/DPAPI, Linux Secret Service, macOS Keychain; passphrase-encrypted file fallback (Argon2id + age).
- **Safety codes**: every pair of people can compare a short code (words or a QR scan) derived from both identity keys. Once compared, the contact shows as **Verified**. A key change for a verified contact or a known server produces a hard warning.
- **Backup**: "Back up my identity" exports a passphrase-encrypted file (age), plus an optional printed recovery phrase. The app prompts for it at the moments that matter: first collaboration, first server join, becoming a project owner.
- **Second device**: link by scanning/pasting a one-time link from the first device; the first device signs the new device certificate. (Each device is a separate MLS member, §13.)
- **Revocation**: project owners and server admins remove a device or user key; for E2EE projects this triggers an MLS key rotation (§13). There is no global revocation list (no central authority by design).
- **Server identity** uses the same construction: `Server ID` = the server's Ed25519 public key (its iroh NodeId) + a self-signed server profile. **IP addresses and hostnames are only hints.** If the key at a known address changes, the client refuses to connect until the user confirms (SSH-style pinning).

---

## 12. Networking

### 12.1 Transport

All peer-to-peer and client–server traffic uses **iroh** (QUIC over UDP; TLS 1.3 with raw public keys):

- **Dial by key**: connect to a NodeId (public key), not an IP. Both sides are authenticated by their keys in the handshake — mutual authentication without passwords or certificate authorities.
- **Post-quantum handshake**: iroh's hybrid X25519MLKEM768 key exchange is enabled (aws-lc-rs provider), protecting recorded traffic against future quantum attacks.
- **Direct first**: local addresses, public addresses and UDP hole punching; a relayed connection upgrades to direct when possible.
- **Relay fallback**: when direct fails (symmetric NAT, CGNAT, UDP blocked) traffic flows through a relay over HTTPS. Relays forward encrypted packets, **cannot read them, and store nothing**.
- **LAN**: local discovery (mDNS) so machines on the same network connect with no internet at all.
- **Two layers of encryption**: transport encryption (QUIC/TLS) protects each hop; **content** is additionally end-to-end encrypted between project members with MLS (§13). A misconfigured or compromised relay/server still sees only ciphertext.

Protocols are separated by ALPN:

| ALPN | Purpose |
|------|---------|
| `motix/session/1` | Collaboration: handshake, membership, encrypted document sync, encrypted presence |
| `motix/blobs/1` | Encrypted media/proxy transfer (iroh-blobs, BLAKE3-verified streaming of ciphertext) |
| `motix/admin/1` | Server administration from a client (server admins only) |

### 12.2 Relays (OD-4)

| Option | Cost | Trade-off |
|--------|------|-----------|
| n0's public relays (default in iroh) | $0 | Third-party infrastructure; fair-use limits; can change. Fine for alpha. |
| Relay built into every MOTIX Server | $0 (the team's own machine) | Teams with a server never need a third party. |
| Project-operated relay | ~US$5–10/month VPS + bandwidth | Independence; small ongoing cost. |
| User-configured relay list | $0 | Advanced setting; enterprises can run their own. |

Because content is end-to-end encrypted, relay choice affects **availability and speed, not privacy**.

### 12.3 Mode 1 — Local
No network activity except the optional update check. The collaboration subsystem is not started.

### 12.4 Mode 2 — Temporary session

```
Host                                              Guest
────                                              ─────
[Start Collaboration Session]
  create MLS group for the session (host device = first member)
  create invite: {host NodeId, relay URL, direct addrs, session id,
                  invite secret (256-bit), role, expiry, single/multi-use}
  → "Copy invite link" / QR
                                         paste link → [Join]
                                         connect(host NodeId) via iroh (PQ hybrid handshake)
        ◄──── QUIC, mutually authenticated by device keys ────►
                                         send: invite id + proof + MLS KeyPackage
                                              proof = HMAC(invite secret, TLS channel binding ‖ guest key)
  verify proof, expiry, single-use
  "Maya wants to join as Editor — Allow / Deny"   (on by default)
  MLS Commit(add Maya) + Welcome → Maya
  send current snapshot encrypted under the new epoch
        ◄──── MLS-encrypted, author-signed Loro updates + presence ────►
  optional: compare safety codes → "Verified"
[End Session] → group closed; keys discarded; nothing persists on any relay
```

- The invite secret never crosses the wire (only an HMAC bound to this TLS connection), so a captured handshake can't be replayed.
- Topology is a **star through the host**: the host orders updates, checks roles and serves media. Content stays end-to-end encrypted between members even though it passes through relays.
- Guests keep a local copy only if the host granted "can save a copy".

### 12.5 Mode 3 — Persistent server
- The client stores a **server entry**: server NodeId (the identity), name, optional address hints and relay hints.
- **Joining the server**: an admin creates a server invite; accepting registers the user's identity key and device certificates in the server's membership list.
- **Joining a project** on the server: a project owner/manager invites the user; the invitee's device uploads an MLS KeyPackage to the server; the next time any owner/manager device is online, it completes the MLS add automatically (if the invite was pre-approved) and uploads an encrypted snapshot for the newcomer. The server **cannot** add members to a project by itself — that is the point.
- **Authentication** on every connection is the QUIC handshake (device key) + certificate chain to a registered user key. No passwords, no bearer tokens on the main protocol.
- **Server behind NAT**: works without port forwarding (hole punching / relay); forwarding one UDP port is an *optional* reliability/performance improvement the setup wizard explains.

### 12.6 Authorization and permissions

| Role (per project) | Read | Comment | Edit | Manage members | Delete project |
|--------------------|------|---------|------|----------------|----------------|
| Viewer | ✓ | | | | |
| Commenter | ✓ | ✓ | | | |
| Editor | ✓ | ✓ | ✓ | | |
| Owner | ✓ | ✓ | ✓ | ✓ | ✓ |
| *Helper* (server, opt-in) | ✓ | | derived artifacts only (proxies, thumbnails, renders) | | |

Server-level roles: **Server Admin** (manage server, storage, backups, updates, server membership — *not* project content) and **Member**.

Enforcement happens in **two places**, because under end-to-end encryption the server can't read edits:
1. **Every client** verifies each update's author signature and the author's role in the project's **signed role log** before applying it. Invalid updates are rejected identically by all clients (deterministic), so no one can forge or exceed a role, even through a compromised server.
2. **The authority** (session host or server) enforces transport-level access from the same signed role log: who may fetch the encrypted project, who may append, rate limits. Viewers' append attempts are refused before they reach anyone.

The role log is signed by owners and readable by the server (it must know who may connect); the server cannot alter it. All role changes appear in the audit log.

### 12.7 Abuse protection
Per-NodeId and per-IP connection rate limits, handshake limits, maximum message sizes enforced before allocation, per-connection memory budgets, bounded queues with backpressure, slow-peer disconnection, per-member storage quotas on servers. Protocol decoders are fuzzed.

### 12.8 Version compatibility
The session handshake exchanges `{app version, protocol version range, document schema major version, MLS ciphersuite, capabilities}`. Peers pick the highest common protocol version; joining requires the same schema major version and ciphersuite. Mismatches produce a plain-language message ("Maya needs to update to join this session").

---

## 13. Collaboration

### 13.1 Data flow

```
 Local edit ─► Loro transaction ─► local doc (instant) ─► local journal (disk, plaintext on the user's own machine)
                                         │
                                         └─► sign (device key) ─► encrypt (MLS epoch key) ─► outbound queue
                                                                                               │
                                             authority (host/server): check sender's role,     │
                                             store ciphertext, forward  ◄──────────────────────┘
                                                                                               │
 other members ◄─ decrypt ◄─ verify signature + role ◄─ validate schema ◄─ import ◄─ normalize ◄┘
```

- **Local-first**: every edit is applied and saved locally before any network I/O. Latency never affects editing.
- **Offline**: the outbound queue persists. On reconnect, members exchange version vectors and send only missing updates. Against a persistent server: arbitrary offline periods. Temporary sessions: only while the host is online (ANALYSIS C12).
- **Validation on every receiving client**: decrypt → verify signature → check role → apply to a scratch fork → schema validation and limits → accept, or reject deterministically (all clients reject the same updates). The sender is told and rolls back locally.
- **Semantic conflicts**: deterministic normalization (§5.2), conflict indicators, optional soft locks ("Maya is editing this clip").
- **Presence** (ephemeral, encrypted): who's here, selections, playheads, current panel.
- **History**: every change has an author and logical time; named versions; "restore this version" creates a new change (never rewrites history).

### 13.2 End-to-end encryption (ADR-022)

| Element | Mechanism |
|---------|-----------|
| Group key agreement | MLS (RFC 9420) via OpenMLS; one group per project / temporary session; members are devices |
| Ciphersuite | Hybrid post-quantum **X-Wing** (X25519 + ML-KEM) + ChaCha20-Poly1305 + Ed25519 signatures; migration path via MLS re-init *(Phase 0 spike)* |
| Document updates & snapshots | Encrypted with keys exported from the current MLS epoch; tagged with epoch id |
| Media, proxies, thumbnails | Random 256-bit key per asset; chunked AEAD (e.g. 64 KiB chunks, STREAM-style nonces) so files stream and seek; content addressed by BLAKE3 of the **ciphertext**; asset keys stored inside the encrypted document |
| Author authenticity | Ed25519 signature over each update by the author's device key; per-author sequence numbers and hash chains |
| Membership & roles | Signed append-only role log (owners sign); MLS Commits add/remove devices; removal rotates keys immediately |
| Forward secrecy / post-compromise security | MLS epochs; periodic self-updates by each member |
| New members | Receive a snapshot encrypted under the current epoch; earlier history only if an owner shares history keys |
| Recovery | Owner's **recovery kit** (printed words / file): snapshot keys are periodically sealed to the owner's recovery public key |
| Server Helper (opt-in) | Server device added as MLS member with role Helper; badge visible to all; removal rotates keys |
| Server misbehavior | Cannot read or forge; *can* withhold/delay — detected through hash-chain gaps and version-vector gossip between members |

What the server/relays **can** see (metadata): which device keys are in which project, role log, sizes and timing of encrypted blobs, IP addresses of connecting devices. Padding of update sizes is a later hardening item.

### 13.3 Media sharing
Assets are referenced by hash inside the encrypted document. A member missing an asset requests the ciphertext (proxy first, original on demand, only if their role permits) from the host, the server or any online member; transfers are resumable and verified; decryption uses the asset key from the document. Because the server can't transcode encrypted media, **proxies are produced by the uploader's machine** (or by the Helper if enabled) and uploaded encrypted.

---

## 14. Persistent server

### 14.1 Shape
A single binary, `motix-server`, with no GUI requirement:
- runs as a **Windows Service**, **systemd service**, **launchd service** (macOS phase) or in a container;
- embeds: iroh endpoint and relay, SQLite, encrypted blob store, MLS **Delivery Service** (orders and stores encrypted handshake/application messages, hosts KeyPackages), backup engine, updater, and a small **local-only admin web UI** for setup (bound to localhost, one-time token);
- **never holds project keys** unless a project owner enables Helper for that project; Helper work (proxies, thumbnails, renders) runs in the same sandboxed worker processes as the client.

### 14.2 Data directory (migratable by design)

```
<data-dir>/                     (chosen during setup; everything important is here)
  server.toml                   configuration (human-readable, optional to edit)
  identity/
    server.key                  server private key (encrypted at rest; key in OS keystore or passphrase)
    server.pub
  db/
    server.sqlite               server membership, device certs, project directory, signed role logs,
                                invites, KeyPackages, audit log  (metadata — no project content)
  projects/<project-id>/
    project.sqlite              MLS messages + encrypted update journal + encrypted snapshots
  media/<ab>/<cd>/<blake3>      encrypted media, proxies and thumbnails (content-addressed ciphertext)
  helper/                       only if Helper is enabled: Helper's MLS state (encrypted at rest)
  backups/                      local backup archives (can point elsewhere)
  cache/                        disposable
  logs/                         rotating logs
```

The **application** (binaries) lives elsewhere (Program Files / `/opt` / app bundle / container image). The data-dir path is the only thing the service config points to.

### 14.3 Setup

```
Install → service starts in "unclaimed" state
       → prints / shows a one-time **claim code** (console, log, and local admin page)
       → Setup wizard (local web page or CLI prompts):
            choose storage location → server name → network check (relays reachable? LAN only?)
            → optional firewall rule (explained, consented) → backup schedule
       → owner opens MOTIX → "Connect to a server" → enters claim code
       → owner's identity becomes Server Admin → compare the server's safety code → done
```

### 14.4 Backup, export, import, migration
- `Backup` = consistent snapshot (SQLite online backup API + media manifest) packed into `*.motixbackup` (tar + zstd) with BLAKE3 checksums; incremental media. Since project content is already end-to-end encrypted, **backups are safe to store off-site** (another disk, a friend's server, cloud storage). The server identity key inside the backup is additionally passphrase-encrypted (age).
- **Scheduled backups** with retention (e.g. 7 daily, 4 weekly).
- **Restore/import** on any OS: install server → "Import backup" → verify checksums → start. The identity key travels with the backup, so **clients reconnect to the same Server ID** even if IP, OS, hardware or storage path changed. Project keys never lived on the server, so nothing about E2EE changes during migration.
- Verification: `motix-server backup verify <file>` and a periodic automatic restore test into a temp dir.

### 14.5 Revisions and retention
Encrypted journal + periodic encrypted snapshots per project (members produce snapshots; the server stores them). Retention is by count/age of encrypted objects (the server can't inspect content). Deleted projects go to a server trash with a grace period.

### 14.6 Audit log
Append-only table of security-relevant events (server/project joins, role changes, invites created/used/revoked, Helper enabled/disabled, admin actions, update installs, backup/restore) with actor key, time and outcome. Never contains project content or secrets.

---

## 15. Updates and release pipeline

### 15.1 Release pipeline (GitHub Actions)

```
push tag vX.Y.Z (protected, signed tag)
   │
   ├─ build matrix: windows-x64, linux-x64 (oldest supported glibc container)
   │     cargo build --locked --release · FFmpeg from cache or pinned source
   ├─ tests: unit · integration · render regression (WARP/lavapipe) · media fixtures
   ├─ audit: cargo-deny (licenses, bans, advisories) · cargo-audit · SBOM (CycloneDX)
   ├─ package: Inno Setup .exe · portable .zip · AppImage · Flatpak bundle
   │           server: .zip (Windows service installer) · .tar.gz · .deb · .rpm · OCI image
   ├─ sign binaries: Authenticode (OD-3) · GitHub build-provenance attestations
   │
   ├─ ── environment "release" (manual approval by owner) ──
   ├─ TUF: add targets (hash, length, custom metadata) · sign targets/snapshot/timestamp
   ├─ publish: GitHub Release (assets + notes + SHA-256 list) · TUF repo → GitHub Pages
   └─ post-release: smoke test by installing from the public channel in a clean VM
```

Nightly builds come from a scheduled workflow to the `nightly` channel without manual approval (different, lower-privilege targets delegation).

### 15.2 Update metadata (TUF)

- **Roles**: `root` (offline; owner-held; eventually 2-of-3 threshold), `targets` (release signing; used only in the approved release environment), delegated `targets/nightly`, `snapshot` and `timestamp` (online; timestamp re-signed daily by a scheduled workflow so clients detect freeze attacks).
- **Targets custom metadata** per artifact:
  `product (client|server)`, `channel`, `version`, `platform`, `min_os`, `min_launcher_version`, `protocol_range`, `project_format_max`, `server_compat_range`, `rollout_percent`, `critical`, `release_notes_url`.
- The TUF client (`tough`) gives: signature thresholds, **rollback protection** (versions never go backwards unless the user explicitly downgrades), **freeze protection** (expiry), **key rotation** without reinstalling, and mix-and-match protection.
- Hosting is dumb static files (GitHub Pages + Releases). Mirrors or a self-hosted update server need no trust: an `UpdateProvider` only supplies bytes; TUF supplies trust.

```rust
trait UpdateProvider {            // where bytes come from — not whether to trust them
    fn fetch_metadata(&self, role: &str) -> Result<Bytes>;
    fn fetch_target(&self, target: &TargetPath, range: Option<Range<u64>>) -> Result<ByteStream>;
}
// GitHubUpdateProvider (Pages + Releases), SelfHostedUpdateProvider (any HTTPS base URL),
// EnterpriseUpdateProvider (file share / offline bundle)
```

### 15.3 Client updater

> **Implemented now (preview channel, ADR-027):** `motix-update` checks GitHub Releases at start-up and every 10 minutes, downloads in the background, verifies an Ed25519-signed `SHA256SUMS`, and installs on restart (or on close) by swapping files with rollback. The TUF/launcher design below supersedes it before public 1.0.
>
> **Shared network folder (ADR-031):** a shared MOTIX already uses the `versions\` + launcher layout below (the launcher is a copy of `motix.exe` at the top of the folder; `versions\current` plays the role of `state.json`). Updates are installed into the share once for every PC.


Install layout (per user, no admin rights needed to update):

```
%LOCALAPPDATA%\Programs\MOTIX\          (Linux: ~/.local/share/motix/app/ or AppImage)
  motix-launcher.exe      tiny, stable; shortcuts point here
  state.json               {current, previous, pending, attempts}
  versions\
    1.4.2\  motix.exe, ffmpeg dlls, ...
    1.5.0\  (new)
```

Flow:
1. **Check** (on start + every 24 h, respecting the user's choice: Automatic / Notify only / Manual): fetch TUF timestamp → snapshot → targets; verify.
2. **Decide**: newer version on my channel, compatible OS, within rollout percentage.
3. **Download** in the background (resumable, rate-limited) to `staging\`; verify length + hash from signed targets; on Windows also verify the Authenticode signature chain.
4. **Stage**: extract to `versions\<new>\` ; mark `pending`.
5. **Switch**: on next launch (or "Restart now"), the launcher starts the new version with a health-check flag.
6. **Health check**: the app must reach "healthy" within 60 s: UI up, GPU initialized, engine self-test, can open a bundled test project. It then writes `healthy` → launcher commits (`current = new`, `previous = old`).
7. **Rollback**: crash or timeout twice → launcher reverts to `previous`, shows "The update didn't start correctly; we went back to 1.4.2", and marks the version bad (won't be re-offered until a newer one exists).
8. **Cleanup**: keep current + previous; delete older.

Linux AppImage uses the same logic with AppImage files as versions. Distro packages and Flatpak defer to the package manager (notify only).

### 15.4 Server updater
- Policies: **Automatic**, **Automatic in maintenance window**, **Notify only** (default), **Manual**. Security-critical releases can be flagged for faster adoption but never force-install.
- Flow: download + verify (same TUF client) → notify admins → at the approved time: take a **pre-update snapshot** (DB + project DBs, not media) → install side-by-side → restart service → server starts in **maintenance mode** (no client writes) → migrations → self-test → open for clients.
- Rollback: if the self-test fails, restore the previous binary *and* the pre-update snapshot (no writes happened in maintenance mode, so nothing is lost).
- **Containers**: never self-update. Notify only; the admin pulls the new tag (`:stable`, `:beta`, `:X.Y`).

### 15.5 Version compatibility policy

| Axis | Rule |
|------|------|
| App version | SemVer. |
| Project format | Integer `format_version`; newer app always migrates older files (with backup); older app opens newer files read-only. |
| Collaboration protocol | Integer range negotiation; a server supports ≥ 2 previous protocol versions. |
| Client ↔ server | Server advertises supported client range; client warns "server is older/newer" and what won't work. |
| Launcher ↔ app | `min_launcher_version` in targets metadata; launcher updated first when needed. |

---

## 16. Packaging and installation

### Client
| Platform | Format | Notes |
|----------|--------|-------|
| Windows | Per-user installer (`MOTIX-Setup-x.y.z.exe`, Inno Setup) | No admin prompt; Start menu + file associations (`.motix`) + URL handler for invite links (`motix://`). |
| Windows | Portable zip | Same launcher; data in `./data` when a `portable.flag` file exists. |
| Windows | MSI (later) | Per-machine for managed/enterprise deployment. |
| Linux | AppImage | Self-updating; built on the oldest supported glibc; desktop integration on first run (asks). |
| Linux | Flatpak (Flathub) | Updates through Flatpak; sandbox permissions for media folders via portals. |
| macOS *(later phase)* | Signed + notarized `.app` in a DMG (Apple Silicon) | Updates by atomic bundle swap with previous bundle kept for rollback; requires Apple Developer Program (OD-8). |

### Server
| Platform | Format | Notes |
|----------|--------|-------|
| Windows | Installer (per-machine, admin) | Registers Windows Service, opens setup page, optional firewall rule with consent. |
| Linux | `.deb`, `.rpm` | systemd unit, dedicated `motix-server` user, data in `/var/lib/motix-server` by default. |
| Linux/NAS | Portable `.tar.gz` | Single binary + workers; `motix-server install-service` helper. |
| macOS *(later phase)* | Signed `.pkg` | launchd service. |
| Any | OCI image (GHCR, free) | Volume for data-dir; Unraid Community Applications template; Docker optional, never required. |

### Firewall
- Client: normally no inbound rule needed (outbound UDP for hole punching). If the user enables "Allow direct connections on my network", Windows asks through the standard firewall prompt, and we explain it first.
- Server: setup explains "Allow MOTIX Server through the firewall on UDP port N? This lets collaborators connect directly and faster." Consent required; rule scoped to our program and port; an equivalent command is shown for manual setups.

---

## 17. AI subsystem

- **Local by default.** Nothing is uploaded unless the user explicitly configures an external provider (later, optional).
- **Architecture**: `motix-ai` (task API in the main process) → `motix-ai-worker` (separate process) → runtimes: ONNX Runtime (DirectML/CPU/CUDA…; CoreML on macOS), whisper.cpp (Vulkan/CPU; Metal on macOS).
- **Model manager**: catalog of models (task, size, license, hash, runtime, minimum hardware) in TUF-signed metadata; download on first use; delete from settings. Only ONNX/GGUF formats (no pickle).
- **Replaceable models**: tasks are defined by interface (e.g. `transcribe(audio) → words with timestamps`); models are implementations.
- **Initial tasks** (Phase 9): transcription + captions, silence detection, scene detection, beat detection, background removal / matting, auto-reframe (face/subject tracking), voice isolation/noise reduction.
- **Honest performance**: every AI task runs as a background job with progress and cancel; never on the UI thread.

---

## 18. Extensibility

Designed now, built later (no marketplace in early phases):

- **Stable internal interfaces** first: `Effect` (GPU node with typed params), `Importer`, `Exporter`, `AudioProcessor`, `AiTask`, `UpdateProvider`, `StorageBackend`. Built-in features are implemented through these same interfaces.
- **Plugins** (later): WebAssembly components (wasmtime) with explicit capabilities (no filesystem/network unless granted), plus **shader effects** as WGSL validated by naga with resource limits. Native plugins, if ever, require an explicit trust prompt.
- **Audio plugins** (later): CLAP hosting (MIT), possibly VST3 — in a separate process.
- **Scripting** (later): the same action registry exposed to sandboxed WASM scripts.

---

## 19. Cross-platform strategy

- All OS-specific code lives in `motix-platform` behind traits (`Paths`, `Keystore`, `Sandbox`, `DisplayInfo`, `Firewall`, `ServiceManager`, `FileAssociations`). Other crates are platform-neutral.
- CI builds and tests **Windows and Linux on every pull request**; macOS joins in Phase 13 (or earlier as a build-only check if I-35 is approved).
- Paths: always `PathBuf`; never assume case-insensitivity, drive letters or `/`.
- GPU: wgpu picks DX12 on Windows, Vulkan on Linux (DX12/Vulkan selectable for troubleshooting), GL as last resort. A driver blocklist handles known bad drivers.
- **macOS is a required platform delivered in a later phase** (OD-8, ADR-024). Until then: no Mac builds, but no choice may preclude it — every dependency above already supports macOS, and OS-specific code stays in `motix-platform`.

---

## 20. Logging, diagnostics and crash handling

- `tracing` with structured fields; per-subsystem targets (`render`, `decode`, `encode`, `net`, `sync`, `server`, `update`, `media.missing`, `perf`).
- Rotating files; size-capped; one-click "Copy diagnostic report" (versions, GPU/driver, OS, recent logs — **redacted**).
- **Redaction** is enforced by types: secrets are wrapped in types (`Secret<T>`) whose `Debug`/`Display` print `[redacted]`; project text/media paths are logged only at debug level and redacted in reports.
- **Crash handling**: minidumps written locally (crash handler in the launcher/out-of-process); on next start, the user may view the report and choose to attach it to a GitHub issue. No automatic upload.
- **Performance HUD** (developer toggle): frame times, decode queue depth, cache hit rate, GPU memory.

---

## 21. Security architecture (summary)

Full threat model and controls: [SECURITY.md](SECURITY.md).

| Boundary | Control |
|----------|---------|
| Media files → engine | Worker processes, sandbox, validation of all worker outputs, fuzzing |
| Project files → engine | SQLite defensive mode, schema validation, limits, no executable content |
| Network peers → client/server | Key-authenticated QUIC with post-quantum hybrid handshake, transport ACL at authority, rate limits |
| Project content ↔ relays/servers | End-to-end encryption (MLS), signed edits verified by every client, signed role log; servers hold ciphertext only (ADR-022) |
| Update channel → install | TUF (signatures, expiry, rollback protection), Authenticode, provenance |
| Plugins (future) → app | WASM capability sandbox |
| Local secrets | OS keystore, encrypted-at-rest fallbacks, redacted logs |

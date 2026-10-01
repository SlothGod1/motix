# Licensing and Costs

> **Not legal advice.** This file tracks what we know and what must be checked. Anything marked **Verify** must be
> confirmed against the dependency's current license text before release. Codec patent questions need a qualified
> lawyer before a public 1.0 (see OD-5 in [DECISIONS.md](DECISIONS.md)).
>
> "Open source" does not automatically mean "no legal considerations": copyleft obligations, patent claims,
> trademark rules and model-weight licenses all apply independently.

Last reviewed: 2026-09-27 (pre-Phase 0).

## 1. Project license
**Decided (OD-1, 2026-09-27): GPL-3.0-or-later**, contributions under DCO sign-off. Every dependency must be GPL-3.0-compatible — enforced by `cargo deny` (see `deny.toml`). GPL components (x264, x265, Rubber Band, GPL models) are usable. **NC** (non-commercial) and AGPL-with-separate-commercial-terms components are not used.

GPL obligations we must meet on every release: ship or link the complete corresponding source (public repo + source archive attached to each GitHub Release, including our FFmpeg configure line and patches), keep license notices in the app's "About → Licenses" screen, and never add restrictions (e.g. DRM on the app itself).

## 2. Dependencies

### 2.1 Core (built-in)
| Dependency | License | Intended use | Commercial-use implications | Patent concerns | Cost | Alternatives |
|------------|---------|--------------|-----------------------------|-----------------|------|--------------|
| Rust toolchain & std | MIT / Apache-2.0 | Language | None | None | $0 | — |
| OpenMLS | MIT | End-to-end group encryption (MLS, RFC 9420; hybrid PQ X-Wing ciphersuite) | None | None | $0 | Custom scheme (rejected) |
| aws-lc-rs / AWS-LC | Apache-2.0 / ISC (some parts derived from OpenSSL — **Verify GPL-3.0 compatibility**, RR-13) | Post-quantum TLS key exchange for iroh/rustls | Notices | None | $0 | RustCrypto ML-KEM via a custom rustls provider |
| wgpu, naga | MIT / Apache-2.0 | GPU abstraction | None | None | $0 | Vulkan/DX12 direct |
| egui, egui-wgpu, egui-winit, egui_tiles | MIT / Apache-2.0 | UI (proposed) | None | None | $0 | Slint (GPL-3.0 / royalty-free / commercial) |
| winit | Apache-2.0 | Windowing | Notice file | None | $0 | SDL3 (zlib) |
| AccessKit | MIT / Apache-2.0 | Accessibility | None | None | $0 | — |
| eframe 0.36 (egui host) | MIT / Apache-2.0 | App window + wgpu renderer | None | None | $0 | — |
| egui_kittest | MIT / Apache-2.0 | UI tests and screenshots (dev only) | None | None | $0 | — |
| rfd | MIT | Native file dialogs (Windows common dialog, Linux XDG portal) | None | None | $0 | — |
| egui default fonts (Ubuntu, Hack, Noto Emoji) | OFL-1.1, Ubuntu Font Licence 1.0 (+ MIT/Apache for the crate) | UI text | Fonts may be bundled with GPL software; keep license notices | None | $0 | Inter/Noto (OFL) later |
| tokio, rayon, tracing | MIT (tokio, tracing), MIT/Apache (rayon) | Runtime, parallelism, logging | None | None | $0 | — |
| iroh, iroh-blobs | MIT / Apache-2.0 | P2P networking | None | None | $0 | quinn + custom NAT traversal |
| rustls | Apache-2.0 / MIT / ISC | TLS | None | None | $0 | — |
| ring / aws-lc-rs (rustls crypto provider) | ring: Apache-2.0 AND ISC; aws-lc-rs: ISC AND (Apache-2.0 OR ISC) — **Verify** | Crypto primitives | Notices | None | $0 | RustCrypto |
| ed25519-dalek, x25519-dalek | BSD-3-Clause | Signatures (update signing in use since preview 3) | Notice | None | $0 | RustCrypto |
| ureq 3 | MIT / Apache-2.0 | HTTPS client for update checks and downloads | None | None | $0 | reqwest |
| rustls-platform-verifier | MIT / Apache-2.0 | Uses the operating system's trusted certificates | None | None | $0 | webpki-roots |
| webpki-root-certs (via the verifier) | CDLA-Permissive-2.0 (data) | Mozilla CA list fallback | Keep the licence text; data, not code — allowed as a cargo-deny exception | None | $0 | — |
| sha2, semver, serde_json | MIT / Apache-2.0 | Checksums, version comparison, GitHub API JSON | None | None | $0 | — |
| zip (deflate only) | MIT | Unpacking update archives | None | None | $0 | tar + flate2 |
| argon2 (RustCrypto) | MIT / Apache-2.0 | Creator Lab owner-password check (Argon2id) | None | None | $0 | scrypt |
| dirs | MIT / Apache-2.0 | Per-user settings/download folders | None | None | $0 | directories |
| blake3 | CC0-1.0 OR Apache-2.0 | Content hashing | None | None | $0 | SHA-256 |
| age / rage, argon2, zeroize | MIT / Apache-2.0 | Backup & key-file encryption | None | None | $0 | — |
| keyring | MIT / Apache-2.0 | OS keystore access | None | None | $0 | — |
| Loro | MIT | CRDT document model | None | None | $0 | Automerge (MIT), Yrs (MIT) |
| SQLite (via rusqlite, bundled) | Public domain (rusqlite: MIT) | Project files, server DB | None | None | $0 | — |
| serde, postcard | MIT / Apache-2.0 | Serialization | None | None | $0 | Protobuf |
| tough | MIT / Apache-2.0 | TUF update client | None | None | $0 | rust-tuf |
| vello | Apache-2.0 / MIT | Vector/text GPU rendering | None | None | $0 | tiny-skia, Skia |
| cosmic-text, swash, harfrust, ttf-parser | MIT / Apache-2.0 (harfrust: MIT) | Text shaping & fonts | None | None | $0 | HarfBuzz (MIT) |
| image (Rust crate) | MIT / Apache-2.0 | Still image decoding | None | Some formats (HEIF/HEIC) are patent-encumbered — not via this crate | $0 | FFmpeg |

### 2.2 Media
| Dependency | License | Intended use | Commercial-use implications | Patent concerns | Cost | Alternatives |
|------------|---------|--------------|-----------------------------|-----------------|------|--------------|
| FFmpeg programs (ffmpeg.exe, ffplay.exe; Gyan "essentials" 9.0.2, GPLv3) | GPL-3.0 | Preview decoding and sound now (ADR-034); downloaded once from the publisher's GitHub release on the user's request (pinned SHA-256) | Licence + source pointers saved beside it | As below | $0 | libav worker (ADR-005) |
| FFmpeg (libav*) | LGPL-2.1-or-later; **GPL-2.0-or-later if built with GPL parts** | Demux, decode, encode, mux | LGPL: dynamic link, ship source/offer, allow relinking, list configure flags. GPL build makes the combined work GPL. | FFmpeg implements patented codecs (H.264, HEVC, AAC…). Distributing binaries may require patent licenses in some jurisdictions. | $0 | GStreamer |
| dav1d | BSD-2-Clause | AV1 decode | Notice | AOM royalty-free patent license; third-party pool claims exist | $0 | libaom |
| SVT-AV1 | BSD-3-Clause-Clear + AOM Patent License 1.0 (**Verify** current terms) | AV1 encode | Notice | As above | $0 | libaom, rav1e (BSD-2) |
| libaom | BSD-2-Clause + AOM patent license | AV1 | Notice | As above | $0 | — |
| libvpx | BSD-3-Clause + Google patent grant | VP8/VP9 | Notice | Pool claims exist against VP9 | $0 | — |
| libopus, FLAC | BSD-3-Clause | Audio codecs | Notice | Opus: royalty-free licenses from contributors | $0 | — |
| FFmpeg native AAC encoder | LGPL (part of FFmpeg) | AAC audio for MP4 | — | AAC patent pool (Via LA) — some patents expired; **Verify** remaining claims | $0 | Opus (not accepted by all platforms in MP4) |
| OpenH264 | BSD-2-Clause | H.264 encode (fallback) | Notice | Cisco pays H.264 royalties **only for Cisco-distributed binaries** downloaded from Cisco at runtime — using that model means a managed download, not bundling | $0 | HW encoders |
| x264 | **GPL-2.0-or-later** (commercial license available from x264 LLC) | Best-quality H.264 encode | ✅ Usable (project is GPL-3.0) | H.264 patents (many expire late 2020s — **Verify**) | $0 (GPL) | OpenH264, HW encoders |
| x265 | **GPL-2.0-or-later** (commercial license from MulticoreWare) | Best-quality HEVC encode; HDR10+ metadata; DV RPU injection (research) | ✅ Usable (project is GPL-3.0) | HEVC pools (Access Advance, Via LA) — significant | $0 (GPL) | HW encoders |
| NVENC/NVDEC headers (nv-codec-headers) | MIT | NVIDIA HW encode/decode | Runtime in driver | Covered by NVIDIA hardware licensing (**Verify**) | $0 | — |
| AMF headers | MIT | AMD HW encode | Runtime in driver | As above | $0 | — |
| oneVPL/libvpl (QSV) | MIT | Intel HW encode/decode | Runtime in driver | As above | $0 | — |
| LAME | LGPL-2.0-or-later | MP3 encode | LGPL terms | MP3 patents expired | $0 | — |
| zimg | WTFPL | High-quality scaling/colorspace (via FFmpeg zscale) | None | None | $0 | own GPU scalers |
| libsoxr | LGPL-2.1-or-later | Audio resampling | LGPL terms | None | $0 | rubato (MIT) |
| ProRes encoding (FFmpeg prores_ks) | LGPL (FFmpeg) | Intermediate export | — | Apple licensing program for ProRes; **Research Required** | $0 | DNxHR, CineForm |
| libdovi / dovi_tool | MIT | Dolby Vision RPU parsing and application (import); RPU generation for research only | Notice | **Dolby Vision is licensed proprietary technology** — import/playback of existing metadata planned; export needs legal review (RR-2) | $0 | libplacebo |
| libplacebo | LGPL-2.1-or-later | Alternative DV/HDR rendering reference, tone mapping | LGPL terms | As above for DV | $0 | own shaders |
| HDR10+ (SMPTE ST 2094-40) | Royalty-free (no per-unit royalties); name/logo via HDR10+ adopter program — **Verify** terms/fees (RR-12) | Dynamic HDR metadata read/generate | Trademark use needs adopter agreement | No per-unit royalties per HDR10+ LLC | $0 (verify admin fee) | Generic "dynamic HDR metadata" wording |
| Dolby Vision export / certification | Proprietary | — | Requires Dolby license | Yes | Not feasible at $0 without legal clearance | HDR10+/HLG/HDR10 |
| fdk-aac | Non-free (FDK license, incompatible with GPL) | — | **Not used** | — | — | FFmpeg AAC |

### 2.3 Color, audio, text assets
| Dependency | License | Intended use | Implications | Patent | Cost | Alternatives |
|------------|---------|--------------|--------------|--------|------|--------------|
| OpenColorIO (later) | BSD-3-Clause | Studio color configs | Notice | None | $0 | built-in `motix-color` |
| ACES OCIO configs | Permissive (Academy) — **Verify** | ACES workflows | Notice | None | $0 | — |
| cpal | Apache-2.0 | Audio device I/O | Notice | None | $0 | — |
| rubato | MIT | Resampling | None | None | $0 | soxr |
| ebur128 (Rust) | MIT | Loudness metering | None | None | $0 | — |
| Signalsmith Stretch | MIT | Time-stretch / pitch | None | None | $0 | Rubber Band |
| Rubber Band | **GPL-2.0-or-later** / commercial | Time-stretch (alt.; highest quality option) | ✅ Usable (GPL) | None | $0 (GPL) | Signalsmith |
| RNNoise | BSD-3-Clause | Noise suppression | Notice | None | $0 | DeepFilterNet |
| Fonts: Inter, Noto Sans (+ CJK), Noto Color Emoji, JetBrains Mono | SIL OFL-1.1 | UI & starter fonts | May bundle; can't sell fonts alone; keep license | None | $0 | — |

### 2.4 AI runtimes and models
Each model is reviewed individually before being added to the catalog.

| Dependency | License | Intended use | Implications | Cost | Notes |
|------------|---------|--------------|--------------|------|-------|
| ONNX Runtime | MIT | Inference runtime | Notice | $0 | |
| DirectML (redistributable) | Microsoft redistribution terms — **Verify** | Windows GPU inference (any vendor) | Redistributable with app | $0 | In maintenance mode; watch Windows ML |
| CUDA runtime / cuDNN / TensorRT | NVIDIA EULAs (redistributable components with conditions) — **Verify** | Optional NVIDIA acceleration | Managed component only; attribution & EULA terms | $0 | Very large downloads |
| whisper.cpp / ggml | MIT | Transcription runtime | Notice | $0 | Vulkan backend |
| Whisper model weights | MIT | Transcription | Notice | $0 | |
| Silero VAD | MIT | Voice activity / silence | Notice | $0 | |
| Demucs (HT) | MIT | Voice isolation / stems | Notice | $0 | |
| DeepFilterNet | MIT / Apache-2.0 | Speech enhancement | Notice | $0 | |
| SAM 2 | Apache-2.0 | Interactive masks/tracking | Notice | $0 | |
| BiRefNet | MIT | Background removal | Notice | $0 | |
| RIFE | MIT | Frame interpolation | Notice | $0 | |
| RT-DETR / YOLOX | Apache-2.0 | Object detection | Notice | $0 | |
| ~~Ultralytics YOLO~~ | AGPL-3.0 | — | **Avoid** (network copyleft, commercial license sold separately) | — | |
| ~~RMBG-1.4 / 2.0~~ | CC BY-NC 4.0 | — | **NC — not used** | — | |
| Robust Video Matting | GPL-3.0 | Video matting (alternative) | ✅ Usable (project is GPL-3.0) | $0 | |

### 2.5 Build, packaging, distribution
| Item | License / terms | Use | Cost |
|------|-----------------|-----|------|
| GitHub (public repo), Actions, Releases, Pages, GHCR, private vulnerability reporting | GitHub ToS | Source, CI, distribution | **$0 for public repos** (including Windows, Linux and macOS runners). Private repos have limited free minutes (Windows ×2, macOS ×10 multipliers) — a strong reason to keep the repo public. |
| Inno Setup | Inno Setup License (free incl. commercial) | Windows installer | $0 |
| WiX Toolset (later) | MS-RL | MSI | $0 |
| appimagetool / linuxdeploy | MIT | AppImage | $0 |
| Flatpak / Flathub | LGPL tooling; Flathub policies | Linux distribution | $0 |
| cargo-deny, cargo-audit, cargo-fuzz, cargo-cyclonedx | MIT / Apache-2.0 | Supply chain & testing | $0 |
| wasmtime (later) | Apache-2.0 WITH LLVM-exception | Plugin sandbox | $0 |
| OpenTimelineIO (later) | Apache-2.0 | Interchange | $0 |
| CLAP (later) | MIT | Audio plugin hosting | $0 |
| VST3 SDK (later) | Reported relicensed to MIT in 2025 — **Verify** | Audio plugin hosting | $0 |

## 3. Costs

### 3.1 Zero-cost baseline (achievable)
Everything required to build, test, release and update the client and server can be done for **$0** with a public GitHub repository, *except* the items in 3.2 that affect user experience or legal certainty.

### 3.2 Costs that may be unavoidable or strongly advisable
| Item | Cost | Needed for | $0 alternative |
|------|------|-----------|----------------|
| Windows code signing (OD-3) | **$0 via SignPath Foundation** (eligible: GPL-3.0); fallback ≈ $10/month Azure Artifact Signing | No SmartScreen warnings | Unsigned alpha only |
| **Apple Developer Program** (OD-8, Phase 13) | **US$99/year** — no waiver for individuals or open-source projects | Signing + notarization so Mac users can install normally (macOS 15+ blocks unsigned apps behind System Settings; Homebrew no longer bypasses this) | None acceptable for mass-market; unsigned only for testers |
| Relay server for temporary sessions (OD-4) | ≈ $5–10/month VPS + bandwidth | Independence from third-party relays | n0 public relays; team servers as relays |
| Legal review of codec patents (OD-5) | Varies (hundreds to thousands of USD) | Confidence before public 1.0 | Hardware/OS encoders + royalty-free formats only |
| Test hardware: AMD, Intel, NVIDIA GPUs; an HDR monitor | One-time, hundreds of USD (may already be owned) | Cross-vendor and HDR testing (CI runners have no GPUs) | Community testers; software rasterizers in CI |
| Hardware security key for TUF root | ≈ $50 one-time | Stronger offline root key | Encrypted offline USB + paper backup |
| Domain name | ≈ $12/year | Website, update mirror, trademark presence | GitHub Pages subdomain |
| Trademark search/registration | Varies (a USPTO filing is a few hundred USD per class) | Clearing and protecting the name MOTIX (OD-6, RR-14) — existing MOTIX™ marks in software exist | Free USPTO/EUIPO searches first |

### 3.3 Features with cost or legal risk (flagged per spec §59)
- **Direct publishing to social platforms** — platform developer programs, API quotas, app review; possible ongoing compliance cost.
- **Cloud AI providers** (optional) — per-use API fees; privacy implications. Local models are the default.
- **Stock media / template library** — licensing per asset; hosting bandwidth.
- **Dolby Vision export** — licensed technology; import is planned, export only after legal review.
- **HDR10+ naming** — royalty-free, but using the name/logo requires the adopter program.
- **macOS distribution** — US$99/year (above).
- **ProRes encode** — licensing uncertainty; research.

# MOTIX

**MOTIX** is a modern, local-first video editor and motion-graphics application for social-media creators — simple enough to
use without a manual, powerful enough to grow into professional compositing — with private, peer-to-peer
collaboration and an optional self-hosted server.

> **Status: developer preview.** The architecture is documented and the app runs on Windows and Linux: dockable
> panels, a DaVinci Resolve-style timeline (tracks named after your files, linked video/audio, move, trim,
> blade, markers, undo), instant media inspection (size, frame rate, HDR), typed project size and frame rate,
> and signed automatic updates from GitHub. Video playback is next.

## What it will be

- **Download → Install → Open → Edit.** No separate installs of FFmpeg, Python, databases, runtimes or SDKs — ever.
- **Local-first.** Works fully offline. Your projects live on your machine. No accounts.
- **Built for social video.** Vertical/square/landscape, captions, safe areas, platform export presets.
- **Quality first.** Floating-point, color-managed pipeline; HDR; high-quality scaling and resampling.
- **Hardware accelerated** on NVIDIA, AMD and Intel, with CPU fallbacks.
- **Collaborate in one click.** Start a session, send an invite, edit together — direct peer-to-peer, no port forwarding.
- **End-to-end encrypted collaboration.** Relays and even your own server store only data they can't read (MLS, post-quantum hybrid). Every edit is signed by its author.
- **Self-host if you want.** MOTIX Server keeps projects, revisions and backups on your own hardware and moves between machines without losing its identity.
- **Secure, automatic updates** with cryptographic verification and automatic rollback.

## Documentation

| Document | Purpose |
|----------|---------|
| [ANALYSIS.md](ANALYSIS.md) | Review of the specification: missing requirements, contradictions, risks |
| [ARCHITECTURE.md](ARCHITECTURE.md) | System design: client, server, media, rendering, color, networking, collaboration, updates, packaging |
| [DECISIONS.md](DECISIONS.md) | Architecture decision records and **owner decisions** (decided and pending) |
| [REQUIREMENTS.md](REQUIREMENTS.md) | Must / Should / Could / Research Required, with security, licensing, performance and platform tags |
| [PHASES.md](PHASES.md) | Phased roadmap with objectives, tests and completion criteria |
| [SECURITY.md](SECURITY.md) | Threat model, controls, security testing plan, vulnerability reporting |
| [LICENSING_AND_COSTS.md](LICENSING_AND_COSTS.md) | Every dependency's license, patent concerns and costs |
| [IDEAS.md](IDEAS.md) | Proposed features awaiting owner approval (approve by ID in chat, e.g. "approve I-01") |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Toolchain, repo layout, conventions, testing strategy |

## Technology at a glance

Rust · wgpu · egui (pending spike) · FFmpeg (sandboxed workers) · Loro CRDT · SQLite · iroh (QUIC P2P) · OpenMLS (end-to-end encryption) · TUF updates · ONNX Runtime & whisper.cpp (local AI)

## Platforms

Windows 10 22H2+/11 (x64), Linux (x86-64) and **macOS** (Apple Silicon — later phase) for both MOTIX and
MOTIX Server. The server also runs in Docker/OCI containers (optional, never required).

## License

[GPL-3.0-or-later](LICENSE). Contributions are accepted under the Developer Certificate of Origin (`git commit -s`).

# Ideas

Proposed features waiting for the product owner's decision. Nothing here is built automatically.

**How to decide:** tell Claude in chat, for example *"approve I-01 and I-07, decline I-20"*. Approved ideas move
to **Approved**, get a requirement ID in [REQUIREMENTS.md](REQUIREMENTS.md) and a phase in [PHASES.md](PHASES.md).
Declined ideas move to **Declined** so they aren't proposed again.

⭐ = the architect thinks it's unusually high value. Each idea gives *why* it helps and its *cost/risk*.

---

## Approved
*(none yet)*

## Adopted through owner decisions
- **I-00a · macOS client and server** — now a required platform (OD-8, Phase 13).
- **I-00b · Zero-knowledge server** — now the default collaboration model (OD-7, ADR-022).

## Declined
*(none yet)*

---

## Proposed

### Social-media workflow
- **I-01 ⭐ Preview on your phone** — scan a QR code and the current timeline streams (P2P, end-to-end encrypted) to your phone at real size. *Why:* vertical video is judged on a phone. *Cost:* reuses iroh; needs a tiny viewer app or web page.
- **I-02 ⭐ Platform compliance checker** — before export, check duration, resolution, bitrate, file size, loudness (e.g. −14 LUFS), safe areas and caption readability for the chosen platform. *Why:* avoids re-uploads and bad platform re-compression. *Cost:* low.
- **I-03 ⭐ Animated word-by-word captions** — per-word highlight/pop/bounce styles driven by transcription timing. *Why:* the dominant short-form caption style. *Cost:* builds on Phase 3 text + Phase 9 transcription.
- **I-04 · One edit, many aspect ratios** — keep 9:16, 1:1 and 16:9 variants of one timeline with per-variant reframing. *Cost:* model support for variant overrides.
- **I-05 · Beat-synced cutting** — snap cuts and effects to detected beats. *Cost:* Phase 9 beat detection.
- **I-06 · Hook analyzer** — guidance for the first 3 seconds (motion, text, audio). *Cost:* low, heuristic.
- **I-07 · Direct publishing** to YouTube/TikTok/etc. *Risk:* platform developer programs, API review, ongoing maintenance.
- **I-08 · Thumbnail/cover designer** with frame picker and text templates.

### Collaboration and security
- **I-09 ⭐ Timecoded comments and review mode** — commenters mark frames/regions; editors resolve. *Why:* most collaboration is feedback. *Cost:* Commenter role already planned.
- **I-10 ⭐ Encrypt projects on your own disk** — `.motix` files and caches encrypted with a key held in the OS keystore. *Why:* protects work on a lost or stolen laptop, completing the end-to-end story. *Cost:* small performance cost; recovery tied to identity backup.
- **I-11 · Encrypted review links** — send a view-only, end-to-end encrypted link that expires. *Cost:* needs a viewer (phone/web).
- **I-12 · Encrypted project hand-off file** — export a project + media as one encrypted package to send by USB/email/cloud; only chosen people can open it.
- **I-13 · Metadata hiding** — pad update sizes and batch timing so servers/relays learn less about activity. *Cost:* bandwidth.
- **I-14 · Short invite codes** (e.g. `7-orange-piano`) via a rendezvous service using a PAKE. *Why:* easy to read aloud. *Cost:* a small hosted service.
- **I-15 · LAN auto-discovery** — "Maya's MOTIX is on your network — join?" *Cost:* low (mDNS).
- **I-16 · Follow mode** — follow a collaborator's playhead and view.
- **I-17 · Distributed rendering** — split exports across trusted members' idle machines (they are already key holders). *Risk:* complexity.

### Editing and effects
- **I-18 · Expressions** in a small sandboxed, deterministic language for linking properties. *Risk:* must never become a general scripting hole.
- **I-19 · Motion templates** with exposed parameters (reusable branded graphics).
- **I-20 · Audio-reactive animation** — drive properties from audio levels/bands.
- **I-21 · Multicam editing** with audio-waveform sync.
- **I-22 · Stem separation** (vocals/music/drums). *Cost:* Demucs (MIT).
- **I-23 · 360°/VR video.** *Cost:* high.
- **I-24 · Ambisonic audio.**

### Quality and media
- **I-25 ⭐ Mistagged-color detector** — warn when a clip's color tags look wrong (full vs limited range, BT.601 vs BT.709) and offer a fix. *Why:* the most common cause of washed-out or crushed video. *Cost:* heuristic analysis.
- **I-26 · Upload-aware encoding** — settings known to survive platform re-compression well.
- **I-27 · VMAF quality preview** of the export vs the timeline.
- **I-28 · Content Credentials (C2PA)** — optionally sign exports with provenance. *Cost:* SDK license to verify; key management.
- **I-29 · Dolby Vision export** — profile 8.1/8.4 via open tools, *only* after legal review (RR-2).

### AI
- **I-30 · Natural-language command bar** with a small local LLM ("remove silences, add captions, make it 9:16"), mapped to the action registry only. *Risk:* model size, reliability.
- **I-31 · Text-based editing** — edit the video by editing its transcript.
- **I-32 · AI media search** — "the clip where I'm holding the red mug" via local embeddings.
- **I-33 · Auto B-roll suggestions** from your own library.
- **I-34 · Pure-Rust GPU inference (Burn on wgpu)** — could remove vendor AI runtimes entirely if fast enough.
- **I-35 · Style profile from "Edits I love"** (owner request) — measure shot lengths, cut-to-beat alignment, transition types, motion and colour of the owner's favourites; suggest edits and score drafts against that profile.
- **I-36 · Upscale preference tuning** (owner request) — run several upscalers/settings (Real-ESRGAN-style ONNX models) on the owner's comparisons and learn which he prefers for faces, text, anime, low light.
- **I-37 · Scene packs** (owner request) — shot detection + face re-identification to export "every shot of this person", pacing shots, close-ups, and clean transition points from films/episodes, as named clips ready to drop on the timeline. *Verify* face-model licences.
- **I-38 · Transition finder** — find matching motion/colour between two shots for seamless match cuts and whip-pan transitions.
- **I-39 · Beat-synced auto edit** — given a song and a scene pack, propose a first cut in the owner's style.

### Platform, UX, operations
- **I-35 ⭐ Build-check macOS in CI from now on** — compile (not test or ship) on free macOS runners every PR. *Why:* catches Mac-breaking code years before the Mac phase, for $0. *Cost:* ~5 minutes of CI per PR.
- **I-36 · Mobile companion app** (review, comment, preview).
- **I-37 · Project health check** — missing media, huge caches, unused assets, format warnings.
- **I-38 · Interactive first-run tutorial project** instead of a manual.
- **I-39 · Color-blind-friendly palettes** and scalable UI.
- **I-40 · Self-hosted crash-report endpoint** in MOTIX Server (reports end-to-end encrypted to the team's admins).
- **I-41 · Font manager** with on-demand download of OFL fonts.
- **I-42 · Sticker/emoji library** (e.g. OpenMoji — CC BY-SA, attribution required).

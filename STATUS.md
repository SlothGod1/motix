# MOTIX — Current status and hand-off

Last updated: 2026-10-02

## Where things stand
- **Playback** (ADR-034): `motix-media` runs the FFmpeg programs. Windows: a banner offers a one-time download of Gyan
  essentials 9.0.2 (pinned URL + SHA-256 in `motix_media::helper`) into the shared folder's `tools\` or
  `%LOCALAPPDATA%\MOTIX\tools`; Linux: system ffmpeg. Viewer shows real frames; sound mixed by ffmpeg, played by ffplay.
  (CI was deliberately left unchanged — no `.github` edit needed.)
- **Updates now come from the owner's server (ADR-036):** deliver = write files + `.motix-build-ready`; check
  `Editing Software\MOTIX build status.txt`. Workspace version 0.2.0 (`-server.<timestamp>` builds).
- Releases came from GitHub CI (last: preview.8, which carries the update-folder option) (Claude's sandbox can't build Windows programs). Latest pushed: shared network
  folder (ADR-031, preview 4).
- This change: **save / open projects** (ADR-032: JSON `.motix`, atomic save + `.bak`, unsaved-changes prompts,
  crash-recovery copy every minute) and the owner-only **Creator Lab** (ADR-033: Edits I love, Upscale
  comparisons, Scene packs; password lock). 127 automated tests.
- Creator Lab password: the owner's check (created 2026-10-01) is built in as `motix_app::owner::BUILT_IN`.
  To change it, he creates a new check file and Claude replaces `BUILT_IN`. Never ask for the password itself.
- Update-signing public key compiled into MOTIX: `7764ad35…526b`; secret only in the Actions secret + owner backup.

## Next steps
1. **Scene Library** (owner's order: playback → Scene Library → effects engine → Effect Studio; upscaler training
   later). Owner-only, in the Creator Lab, separate from editor projects. Add many videos/seasons → analyse once in
   the background (shots, faces, speech, camera motion, look) on his RTX 4070 Ti Super → text prompts (people &
   actions, mood & look, camera moves, dialogue/quotes) → results → packs (in MOTIX + export files). Person by
   clicking a face, a photo, or a name (name needs the optional online AI, decided later).
2. Effects/transitions engine with a starter set for everyone; then **Effect Studio**: edit + prompt ("what I like,
   what to add") → draft effect with sliders → Publish ships it in the next update to everyone's Effects panel.
3. Export (vertical MP4); layer compositing; FFmpeg as a separate download so updates stay small.

## Environment notes
- Local builds are developer builds (`0.1.0-dev`, updates off). Set `MOTIX_VERSION` at build time to stamp.
- Probe fixtures: `crates/motix-probe/tests/fixtures/generate.sh` (FFmpeg 6+).

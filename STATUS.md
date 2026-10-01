# MOTIX — Current status and hand-off

Last updated: 2026-10-01

## Where things stand
- Releases come from GitHub CI (Claude's sandbox can't build Windows programs). Latest pushed: shared network
  folder (ADR-031, preview 4).
- This change: **save / open projects** (ADR-032: JSON `.motix`, atomic save + `.bak`, unsaved-changes prompts,
  crash-recovery copy every minute) and the owner-only **Creator Lab** (ADR-033: Edits I love, Upscale
  comparisons, Scene packs; password lock). 127 automated tests.
- Creator Lab password: the owner's check (created 2026-10-01) is built in as `motix_app::owner::BUILT_IN`.
  To change it, he creates a new check file and Claude replaces `BUILT_IN`. Never ask for the password itself.
- Update-signing public key compiled into MOTIX: `7764ad35…526b`; secret only in the Actions secret + owner backup.

## Next steps
1. **Video + audio playback** (M1 remainder). Plan: ship a pinned GPL FFmpeg build next to `motix.exe` (CI change in
   `.github` → owner copies the file), decode in a separate process into the viewer (interim for ADR-005's
   libav worker), audio via cpal; GPL source offer for the FFmpeg build (Verify).
2. Creator Lab analysis (ADR-033 AI plan): shot detection + beats first, then faces/scene packs (Verify model licences).
3. M2: export (vertical MP4).

## Environment notes
- Local builds are developer builds (`0.1.0-dev`, updates off). Set `MOTIX_VERSION` at build time to stamp.
- Probe fixtures: `crates/motix-probe/tests/fixtures/generate.sh` (FFmpeg 6+).

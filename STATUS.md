# MOTIX — Current status and hand-off

Last updated: 2026-09-28

## Where things stand
- On GitHub `main`: this commit (preview 3 + automatic delivery), once pushed from a Claude Code session.
- Preview 3: tracks named after files in the order added (ADR-028); DaVinci Resolve-style timeline
  (Select/Blade, snapping, linked selection, edge trims, markers, edit jumps, track height, lock/solo/mute);
  signed automatic updates `motix-update` + `xtask` (ADR-027); CI promotes green `claude/**` branches to
  `main` and publishes a signed preview release (ADR-029). 108 automated tests.
- Update-signing public key compiled into MOTIX: `7764ad35…526b`. The secret key is only in the
  `MOTIX_UPDATE_SIGNING_KEY` Actions secret (owner adds it) and the owner's private backup.

## Next steps
1. Confirm the first push: CI green → `promote` → first signed release. If the secret is missing, the
   release step warns and installed copies refuse the release — ask the owner to add the secret.
2. Owner downloads MOTIX once from the Releases page; later versions install themselves. Confirm an
   end-to-end self-update on the owner's Windows PC.
3. M1 remainder: FFmpeg media worker → real video frames in the viewer + audio playback (+ waveforms);
   HDR display check.
4. M2: project save + crash recovery, export (vertical MP4), S0-CRDT spike.

## Environment notes
- Local builds are developer builds (`0.1.0-dev`, updates off). Set `MOTIX_VERSION` at build time to stamp.
- Probe fixtures: `crates/motix-probe/tests/fixtures/generate.sh` (FFmpeg 6+).

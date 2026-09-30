# MOTIX — Current status and hand-off

Last updated: 2026-09-29

## Where things stand
- On GitHub `main`: preview 3 + fixes; first signed release `v0.1.0-preview.3` exists (the signing secret is set).
- This change: **shared network folder** (ADR-031). Help > *Share MOTIX on your network…* copies MOTIX into
  an empty folder with a launcher (`MOTIX.exe` = a copy of MOTIX that starts `versions\<current>\motix.exe`).
  PCs run `\\PC\<share>\MOTIX.exe`; updates (GitHub, or a signed release in the share's `updates\`) are
  installed into the share once and picked up on each PC's next start. 116 automated tests.
- Update-signing public key compiled into MOTIX: `7764ad35…526b`. The secret key is only in the
  `MOTIX_UPDATE_SIGNING_KEY` Actions secret and the owner's private backup.
- Claude's sandbox can't build Windows programs (Rust's Windows parts and linkers are blocked by its network
  policy), so Windows builds come from GitHub's CI.

## Next steps
1. Owner: download the newest release once, run `motix.exe`, Help > Share MOTIX on your network…, pick an
   empty folder, share it in Windows (Read/Write for trusted people), start `\\PC\<share>\MOTIX.exe` on the
   other PCs.
2. M1 remainder: FFmpeg media worker → real video frames in the viewer + audio playback (+ waveforms);
   HDR display check.
3. M2: project save + crash recovery, export (vertical MP4), S0-CRDT spike.

## Environment notes
- Local builds are developer builds (`0.1.0-dev`, updates off). Set `MOTIX_VERSION` at build time to stamp.
- Probe fixtures: `crates/motix-probe/tests/fixtures/generate.sh` (FFmpeg 6+).

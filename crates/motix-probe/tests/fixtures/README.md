# Probe test fixtures

Tiny synthetic files (FFmpeg test patterns and sine tones, well under 0.5 s) created for
MOTIX's tests. They contain no third-party content and are covered by the repository's
license. Regenerate with FFmpeg 6+ using the commands in `generate.sh`.

The MP4, MOV, M4A, WAV, PNG and JPEG files here also carry a small "content credentials"
(C2PA) metadata block that the file-transfer route added on the way to GitHub (ADR-030).
MOTIX must read such files anyway (many phones and editors add the same metadata), so they
are kept as a realistic extra case; freshly generated files won't have it.

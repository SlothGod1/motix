//! Hard limits on values that can arrive from untrusted sources.
//!
//! Validators, decoders and tests all use these constants, so that a malicious
//! project file, media file or network peer cannot make the application allocate
//! absurd amounts of memory or overflow arithmetic. Change them only with a
//! matching update to `SECURITY.md` and the tests.

/// Largest accepted frame rate, in frames per second.
pub const MAX_FRAME_RATE_FPS: i64 = 1_000;

/// Largest accepted denominator for frame rates (e.g. the `1001` in `24000/1001`).
///
/// Bounding the denominator keeps all time conversions inside `i128` arithmetic.
pub const MAX_RATE_DENOMINATOR: i64 = 1_000_000;

/// Largest accepted audio sample rate, in hertz.
pub const MAX_SAMPLE_RATE_HZ: u32 = 768_000;

/// Largest accepted image width or height, in pixels (16K).
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;

/// Largest accepted number of audio channels in one stream.
pub const MAX_AUDIO_CHANNELS: u32 = 64;

/// Longest accepted composition, in seconds (100 hours).
pub const MAX_COMPOSITION_SECONDS: i64 = 100 * 60 * 60;

/// Deepest accepted nesting of compositions inside compositions.
pub const MAX_NESTING_DEPTH: u32 = 64;

/// Longest accepted user-visible string (names, caption text), in bytes.
pub const MAX_STRING_BYTES: usize = 64 * 1024;

/// Largest accepted control message between processes, in bytes.
///
/// Video frames travel through shared memory, not through messages.
pub const MAX_IPC_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Largest accepted single network message, in bytes.
pub const MAX_NET_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

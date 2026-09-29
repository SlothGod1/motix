//! Fast, memory-safe inspection of media files.
//!
//! `motix-probe` answers the questions MOTIX needs the moment a file is imported —
//! *how big is the picture, what frame rate, how long, how many audio streams, is it
//! HDR?* — without decoding anything. It reads container headers only:
//!
//! | Format | What is read |
//! |--------|--------------|
//! | MP4 / MOV / M4A / 3GP (ISO-BMFF) | `moov`: tracks, sample descriptions, timing, `colr`, HDR boxes, rotation |
//! | Matroska / `WebM` | `Info`, `Tracks` (incl. `Colour`) |
//! | WAV (RIFF) | `fmt `, `data` |
//! | PNG, JPEG | Image size |
//!
//! # Security
//!
//! Every byte comes from an untrusted file. This crate is `#![forbid(unsafe_code)]`,
//! never recurses without a depth limit, never allocates based on an unchecked size
//! (see the `MAX_*` constants), and returns [`ProbeError`] instead of panicking.
//! A randomized corruption test runs on every build. Actual *decoding* is done by
//! FFmpeg in a sandboxed worker process (ARCHITECTURE §6); see ADR-026 for why this
//! header-only reader runs in-process.

#![forbid(unsafe_code)]
// Binary parsers name fields after the spec (w, h, a, b, c, d) and convert
// bounded values between integer and float types on purpose.
#![allow(clippy::many_single_char_names, clippy::similar_names, clippy::cast_precision_loss)]

mod bytes;
mod image;
mod isobmff;
mod matroska;
mod wav;

use motix_core::{FrameRate, Time};
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Largest `moov` (MP4 header) or Matroska header element read into memory.
pub const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;
/// Most tracks/streams accepted in one file.
pub const MAX_TRACKS: usize = 64;
/// Deepest box / element nesting followed.
pub const MAX_DEPTH: usize = 16;

/// Why a file could not be inspected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeError {
    /// The file could not be read.
    Io(String),
    /// Not a format this reader understands (it may still be playable later via FFmpeg).
    UnknownFormat,
    /// The file claims to be a known format but its structure is broken or truncated.
    Malformed(&'static str),
    /// A header is larger than [`MAX_HEADER_BYTES`].
    TooLarge,
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read the file ({e})"),
            Self::UnknownFormat => f.write_str("this file type can't be inspected yet"),
            Self::Malformed(what) => write!(f, "the file looks damaged ({what})"),
            Self::TooLarge => f.write_str("the file's header is unusually large"),
        }
    }
}

impl std::error::Error for ProbeError {}

impl From<std::io::Error> for ProbeError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Self::Malformed("ends too early")
        } else {
            Self::Io(e.kind().to_string())
        }
    }
}

/// The container format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    /// MPEG-4 (`.mp4`, `.m4v`, `.m4a`, `.3gp`).
    Mp4,
    /// `QuickTime` (`.mov`).
    QuickTime,
    /// Matroska (`.mkv`).
    Matroska,
    /// `WebM` (`.webm`).
    WebM,
    /// RIFF WAVE (`.wav`).
    Wav,
    /// PNG image.
    Png,
    /// JPEG image.
    Jpeg,
}

impl Container {
    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mp4 => "MP4",
            Self::QuickTime => "QuickTime MOV",
            Self::Matroska => "Matroska MKV",
            Self::WebM => "WebM",
            Self::Wav => "WAV",
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
        }
    }
}

/// How the picture's brightness is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// Standard dynamic range (BT.709 / BT.1886 / sRGB).
    Sdr,
    /// HDR, Perceptual Quantizer (SMPTE ST 2084) — HDR10, HDR10+, Dolby Vision.
    Pq,
    /// HDR, Hybrid Log-Gamma (ARIB STD-B67) — common on phones and broadcast.
    Hlg,
    /// Not signalled in the file (treated as SDR).
    Unspecified,
}

/// Colour primaries (the gamut).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primaries {
    /// BT.709 / sRGB.
    Bt709,
    /// BT.2020 wide gamut.
    Bt2020,
    /// Display P3.
    P3,
    /// BT.601 (standard-definition).
    Bt601,
    /// Not signalled.
    Unspecified,
    /// Something else (ITU-T H.273 code point).
    Other(u16),
}

/// Colour signalling of a video stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorInfo {
    /// Gamut.
    pub primaries: Primaries,
    /// Brightness encoding.
    pub transfer: Transfer,
    /// Full-range (0–255) instead of video-range (16–235) levels, when known.
    pub full_range: Option<bool>,
}

impl Default for ColorInfo {
    fn default() -> Self {
        Self {
            primaries: Primaries::Unspecified,
            transfer: Transfer::Unspecified,
            full_range: None,
        }
    }
}

impl ColorInfo {
    /// Builds from ITU-T H.273 code points (as stored in `colr`, `vpcC`, Matroska `Colour`).
    #[must_use]
    pub fn from_h273(primaries: u16, transfer: u16, full_range: Option<bool>) -> Self {
        Self {
            primaries: match primaries {
                1 => Primaries::Bt709,
                9 => Primaries::Bt2020,
                11 | 12 => Primaries::P3,
                5 | 6 => Primaries::Bt601,
                0 | 2 => Primaries::Unspecified,
                other => Primaries::Other(other),
            },
            transfer: match transfer {
                16 => Transfer::Pq,
                18 => Transfer::Hlg,
                0 | 2 => Transfer::Unspecified,
                _ => Transfer::Sdr,
            },
            full_range,
        }
    }
}

/// The dynamic-range format, as a person would name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicRange {
    /// Standard dynamic range.
    Sdr,
    /// HDR10 (PQ, BT.2020). HDR10+ dynamic metadata lives inside the video stream and
    /// is detected when decoding arrives.
    Hdr10,
    /// HLG.
    Hlg,
    /// Dolby Vision (with the profile number when known).
    DolbyVision(Option<u8>),
}

impl DynamicRange {
    /// Plain-language label, e.g. `"HDR10"`.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Sdr => "SDR".to_owned(),
            Self::Hdr10 => "HDR10".to_owned(),
            Self::Hlg => "HLG".to_owned(),
            Self::DolbyVision(Some(p)) => format!("Dolby Vision (profile {p})"),
            Self::DolbyVision(None) => "Dolby Vision".to_owned(),
        }
    }

    /// `true` for any HDR format.
    #[must_use]
    pub const fn is_hdr(self) -> bool {
        !matches!(self, Self::Sdr)
    }
}

/// Dolby Vision signalling found in the container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DolbyVision {
    /// Dolby Vision profile (5, 7, 8…), when the configuration record was readable.
    pub profile: Option<u8>,
}

/// The video stream of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoInfo {
    /// Codec, e.g. `"H.264"`, `"HEVC"`, `"AV1"`.
    pub codec: String,
    /// Stored picture width in pixels.
    pub coded_width: u32,
    /// Stored picture height in pixels.
    pub coded_height: u32,
    /// Clockwise rotation to apply when displaying (0, 90, 180, 270). Phones record
    /// portrait video as landscape pixels plus a rotation.
    pub rotation: u16,
    /// Frame rate, if it could be determined.
    pub frame_rate: Option<FrameRate>,
    /// `true` if frame durations vary (common on phones and screen recordings).
    pub variable_frame_rate: bool,
    /// Bits per colour sample (8, 10, 12), when known.
    pub bit_depth: Option<u8>,
    /// Colour signalling.
    pub color: ColorInfo,
    /// Dolby Vision configuration, when present.
    pub dolby_vision: Option<DolbyVision>,
    /// Static HDR metadata present (mastering display / content light level).
    pub hdr_metadata: bool,
}

impl VideoInfo {
    /// Width as displayed (after rotation).
    #[must_use]
    pub fn display_width(&self) -> u32 {
        if self.rotation % 180 == 90 {
            self.coded_height
        } else {
            self.coded_width
        }
    }

    /// Height as displayed (after rotation).
    #[must_use]
    pub fn display_height(&self) -> u32 {
        if self.rotation % 180 == 90 {
            self.coded_width
        } else {
            self.coded_height
        }
    }

    /// The dynamic-range format.
    #[must_use]
    pub fn dynamic_range(&self) -> DynamicRange {
        if let Some(dv) = self.dolby_vision {
            return DynamicRange::DolbyVision(dv.profile);
        }
        match self.color.transfer {
            Transfer::Pq => DynamicRange::Hdr10,
            Transfer::Hlg => DynamicRange::Hlg,
            Transfer::Sdr | Transfer::Unspecified => DynamicRange::Sdr,
        }
    }
}

/// One audio stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioInfo {
    /// Codec, e.g. `"AAC"`, `"PCM 24-bit"`.
    pub codec: String,
    /// Channel count.
    pub channels: u16,
    /// Samples per second.
    pub sample_rate: u32,
}

/// Everything the probe learned about a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaInfo {
    /// Container format.
    pub container: Container,
    /// Total duration, when known (stills have none).
    pub duration: Option<Time>,
    /// The main video stream (or the picture, for stills).
    pub video: Option<VideoInfo>,
    /// Audio streams in file order.
    pub audio: Vec<AudioInfo>,
    /// `true` for single still images.
    pub still: bool,
}

/// Inspects the file at `path`.
///
/// # Errors
/// See [`ProbeError`].
pub fn probe_path(path: &Path) -> Result<MediaInfo, ProbeError> {
    let file = File::open(path)?;
    probe(BufReader::with_capacity(64 * 1024, file))
}

/// Inspects media from any seekable reader.
///
/// # Errors
/// See [`ProbeError`].
pub fn probe<R: Read + Seek>(mut r: R) -> Result<MediaInfo, ProbeError> {
    let len = r.seek(SeekFrom::End(0))?;
    r.seek(SeekFrom::Start(0))?;
    let mut magic = [0_u8; 12];
    let n = read_up_to(&mut r, &mut magic)?;
    r.seek(SeekFrom::Start(0))?;
    let magic = &magic[..n];

    if magic.starts_with(b"RIFF") && magic.get(8..12) == Some(b"WAVE") {
        wav::probe(&mut r, len)
    } else if magic.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        matroska::probe(&mut r, len)
    } else if magic.starts_with(b"\x89PNG\r\n\x1a\n") {
        image::probe_png(&mut r)
    } else if magic.starts_with(&[0xFF, 0xD8, 0xFF]) {
        image::probe_jpeg(&mut r, len)
    } else if magic.len() >= 8 && isobmff::looks_like(&magic[4..8]) {
        isobmff::probe(&mut r, len)
    } else {
        Err(ProbeError::UnknownFormat)
    }
}

fn read_up_to<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<usize, ProbeError> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(k) => filled += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(filled)
}

/// Converts seconds (from a float field in a file) to [`Time`], rejecting nonsense.
pub(crate) fn time_from_seconds_f64(seconds: f64) -> Option<Time> {
    let max = motix_core::limits::MAX_COMPOSITION_SECONDS as f64 * 10.0;
    if !seconds.is_finite() || seconds < 0.0 || seconds > max {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let flicks = (seconds * motix_core::FLICKS_PER_SECOND as f64).round() as i64;
    Some(Time::from_flicks(flicks))
}

/// `value / timescale` seconds as exact [`Time`].
pub(crate) fn time_from_ticks(value: u64, timescale: u32) -> Option<Time> {
    if timescale == 0 {
        return None;
    }
    let flicks = i128::from(value) * i128::from(motix_core::FLICKS_PER_SECOND) / i128::from(timescale);
    i64::try_from(flicks).ok().map(Time::from_flicks)
}

//! Matroska and `WebM`: reads the `Info` and `Tracks` elements.

use crate::bytes::{Bytes, read_vec};
use crate::{
    AudioInfo, ColorInfo, Container, DolbyVision, MAX_TRACKS, MediaInfo, ProbeError, VideoInfo, time_from_seconds_f64,
};
use motix_core::FrameRate;
use std::io::{Read, Seek, SeekFrom};

const EBML: u32 = 0x1A45_DFA3;
const DOC_TYPE: u32 = 0x4282;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const DURATION: u32 = 0x4489;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_TYPE: u32 = 0x83;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const DEFAULT_DURATION: u32 = 0x23_E383;
const VIDEO: u32 = 0xE0;
const PIXEL_WIDTH: u32 = 0xB0;
const PIXEL_HEIGHT: u32 = 0xBA;
const COLOUR: u32 = 0x55B0;
const BITS_PER_CHANNEL: u32 = 0x55B2;
const RANGE: u32 = 0x55B9;
const TRANSFER: u32 = 0x55BA;
const PRIMARIES: u32 = 0x55BB;
const MAX_CLL: u32 = 0x55BC;
const MASTERING: u32 = 0x55D0;
const AUDIO: u32 = 0xE1;
const SAMPLING_FREQUENCY: u32 = 0xB5;
const CHANNELS: u32 = 0x9F;
const BIT_DEPTH: u32 = 0x6264;
const BLOCK_ADDITION_MAPPING: u32 = 0x41E4;
const BLOCK_ADD_ID_TYPE: u32 = 0x41E7;
const BLOCK_ADD_ID_EXTRA: u32 = 0x41ED;
const CLUSTER: u32 = 0x1F43_B675;

const MAX_TOP_LEVEL: usize = 100_000;

/// Reads an EBML variable-length integer. With `keep_marker` it is an element ID.
fn vint(b: &mut Bytes<'_>, keep_marker: bool) -> Result<(u64, bool), ProbeError> {
    let first = b.u8()?;
    if first == 0 {
        return Err(ProbeError::Malformed("bad EBML number"));
    }
    let len = first.leading_zeros() as usize + 1;
    let mut value = if keep_marker {
        u64::from(first)
    } else {
        u64::from(first) & ((1_u64 << (8 - len)) - 1)
    };
    for &byte in b.take(len - 1)? {
        value = (value << 8) | u64::from(byte);
    }
    let all_ones = !keep_marker && value == (1_u64 << (7 * len)) - 1;
    Ok((value, all_ones))
}

/// Iterates child elements: `(id, body)`.
fn elements(data: &[u8]) -> impl Iterator<Item = Result<(u32, &[u8]), ProbeError>> {
    let mut b = Bytes::new(data);
    std::iter::from_fn(move || {
        if b.remaining() == 0 {
            return None;
        }
        Some((|| {
            let (id, _) = vint(&mut b, true)?;
            let (size, unknown) = vint(&mut b, false)?;
            let size = if unknown {
                b.remaining()
            } else {
                usize::try_from(size).map_err(|_| ProbeError::TooLarge)?
            };
            let id = u32::try_from(id).map_err(|_| ProbeError::Malformed("bad element ID"))?;
            Ok((id, b.take(size)?))
        })())
    })
}

fn uint(body: &[u8]) -> u64 {
    body.iter().take(8).fold(0, |acc, &x| (acc << 8) | u64::from(x))
}

fn float(body: &[u8]) -> Option<f64> {
    match body.len() {
        4 => Some(f64::from(f32::from_be_bytes(body.try_into().ok()?))),
        8 => Some(f64::from_be_bytes(body.try_into().ok()?)),
        _ => None,
    }
}

/// Reads the header of the element at `pos`: `(id, body_start, body_len or None if unknown)`.
fn header_at<R: Read + Seek>(r: &mut R, pos: u64, len: u64) -> Result<(u32, u64, Option<u64>), ProbeError> {
    r.seek(SeekFrom::Start(pos))?;
    let avail = (len - pos).min(12);
    let head = read_vec(r, avail)?;
    let mut b = Bytes::new(&head);
    let (id, _) = vint(&mut b, true)?;
    let (size, unknown) = vint(&mut b, false)?;
    let consumed = (head.len() - b.remaining()) as u64;
    let id = u32::try_from(id).map_err(|_| ProbeError::Malformed("bad element ID"))?;
    Ok((id, pos + consumed, (!unknown).then_some(size)))
}

pub(crate) fn probe<R: Read + Seek>(r: &mut R, len: u64) -> Result<MediaInfo, ProbeError> {
    // EBML header → doc type.
    let (id, body, size) = header_at(r, 0, len)?;
    if id != EBML {
        return Err(ProbeError::UnknownFormat);
    }
    let size = size.ok_or(ProbeError::Malformed("EBML header size unknown"))?;
    r.seek(SeekFrom::Start(body))?;
    let ebml = read_vec(r, size.min(4096))?;
    let mut container = Container::Matroska;
    for el in elements(&ebml) {
        let (id, value) = el?;
        if id == DOC_TYPE && value.starts_with(b"webm") {
            container = Container::WebM;
        }
    }

    // Segment.
    let seg_pos = body + size;
    if seg_pos >= len {
        return Err(ProbeError::Malformed("no segment"));
    }
    let (id, seg_body, seg_size) = header_at(r, seg_pos, len)?;
    if id != SEGMENT {
        return Err(ProbeError::Malformed("no segment"));
    }
    let seg_end = seg_size.map_or(len, |s| seg_body.saturating_add(s).min(len));

    let mut info = None;
    let mut tracks = None;
    let mut pos = seg_body;
    for _ in 0..MAX_TOP_LEVEL {
        if pos + 2 > seg_end || (info.is_some() && tracks.is_some()) {
            break;
        }
        let (id, body, size) = header_at(r, pos, len)?;
        let Some(size) = size else { break };
        match id {
            INFO | TRACKS => {
                r.seek(SeekFrom::Start(body))?;
                let data = read_vec(r, size)?;
                if id == INFO {
                    info = Some(data);
                } else {
                    tracks = Some(data);
                }
            }
            CLUSTER if tracks.is_some() => break,
            _ => {}
        }
        pos = body.saturating_add(size);
    }

    let mut scale = 1_000_000_u64;
    let mut duration_units = None;
    if let Some(info) = &info {
        for el in elements(info) {
            let (id, v) = el?;
            match id {
                TIMESTAMP_SCALE => scale = uint(v).max(1),
                DURATION => duration_units = float(v),
                _ => {}
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let duration = duration_units.and_then(|d| time_from_seconds_f64(d * scale as f64 / 1.0e9));

    let tracks = tracks.ok_or(ProbeError::Malformed("no track list"))?;
    let mut video = None;
    let mut audio = Vec::new();
    let mut count = 0;
    for el in elements(&tracks) {
        let (id, entry) = el?;
        if id != TRACK_ENTRY {
            continue;
        }
        count += 1;
        if count > MAX_TRACKS {
            return Err(ProbeError::Malformed("too many tracks"));
        }
        match parse_track(entry)? {
            Parsed::Video(v) if video.is_none() => video = Some(v),
            Parsed::Audio(a) => audio.push(a),
            _ => {}
        }
    }
    if video.is_none() && audio.is_empty() {
        return Err(ProbeError::Malformed("no video or audio tracks"));
    }
    Ok(MediaInfo {
        container,
        duration,
        video,
        audio,
        still: false,
    })
}

enum Parsed {
    Video(VideoInfo),
    Audio(AudioInfo),
    Other,
}

fn parse_track(entry: &[u8]) -> Result<Parsed, ProbeError> {
    let mut kind = 0;
    let mut codec_id = String::new();
    let mut private: &[u8] = &[];
    let mut default_duration = None;
    let mut video_el: &[u8] = &[];
    let mut audio_el: &[u8] = &[];
    let mut dolby_vision = None;
    for el in elements(entry) {
        let (id, v) = el?;
        match id {
            TRACK_TYPE => kind = uint(v),
            CODEC_ID => codec_id = String::from_utf8_lossy(&v[..v.len().min(64)]).into_owned(),
            CODEC_PRIVATE => private = v,
            DEFAULT_DURATION => default_duration = Some(uint(v)),
            VIDEO => video_el = v,
            AUDIO => audio_el = v,
            BLOCK_ADDITION_MAPPING => {
                let mut is_dv = false;
                let mut extra: &[u8] = &[];
                for m in elements(v) {
                    let (mid, mv) = m?;
                    match mid {
                        BLOCK_ADD_ID_TYPE => is_dv = matches!(uint(mv), 0x6476_6343 | 0x6476_7643 | 0x6476_7743),
                        BLOCK_ADD_ID_EXTRA => extra = mv,
                        _ => {}
                    }
                }
                if is_dv {
                    dolby_vision = Some(DolbyVision {
                        profile: extra.get(2).map(|p| p >> 1),
                    });
                }
            }
            _ => {}
        }
    }
    match kind {
        1 => Ok(Parsed::Video(parse_video(
            &codec_id,
            private,
            default_duration,
            video_el,
            dolby_vision,
        )?)),
        2 => Ok(Parsed::Audio(parse_audio(&codec_id, audio_el)?)),
        _ => Ok(Parsed::Other),
    }
}

fn parse_video(
    codec_id: &str,
    private: &[u8],
    default_duration: Option<u64>,
    video_el: &[u8],
    dolby_vision: Option<DolbyVision>,
) -> Result<VideoInfo, ProbeError> {
    let codec = match codec_id {
        "V_MPEG4/ISO/AVC" => "H.264",
        "V_MPEGH/ISO/HEVC" => "HEVC",
        "V_AV1" => "AV1",
        "V_VP9" => "VP9",
        "V_VP8" => "VP8",
        "V_PRORES" => "ProRes",
        "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" => "MPEG-4 Part 2",
        "V_MJPEG" => "Motion JPEG",
        "V_FFV1" => "FFV1",
        "V_MPEG2" => "MPEG-2",
        other => other.strip_prefix("V_").unwrap_or(other),
    }
    .to_owned();
    let mut bit_depth = match codec_id {
        "V_MPEGH/ISO/HEVC" => private.get(17).map(|d| (d & 0x07) + 8),
        "V_AV1" => private.get(2).map(|f| match (f & 0x40 != 0, f & 0x20 != 0) {
            (true, true) => 12,
            (true, false) => 10,
            _ => 8,
        }),
        "V_MPEG4/ISO/AVC" => crate::isobmff::avc_bit_depth(private),
        "V_VP8" => Some(8),
        _ => None,
    };
    let (mut w, mut h) = (0_u64, 0_u64);
    let mut color = ColorInfo::default();
    let mut hdr_metadata = false;
    for el in elements(video_el) {
        let (id, v) = el?;
        match id {
            PIXEL_WIDTH => w = uint(v),
            PIXEL_HEIGHT => h = uint(v),
            COLOUR => {
                let (mut p, mut t, mut range) = (2_u64, 2_u64, None);
                for c in elements(v) {
                    let (cid, cv) = c?;
                    match cid {
                        BITS_PER_CHANNEL => {
                            let bits = uint(cv);
                            if (8..=16).contains(&bits) {
                                bit_depth = u8::try_from(bits).ok();
                            }
                        }
                        RANGE => range = Some(uint(cv) == 2),
                        TRANSFER => t = uint(cv),
                        PRIMARIES => p = uint(cv),
                        MAX_CLL | MASTERING => hdr_metadata = true,
                        _ => {}
                    }
                }
                color = ColorInfo::from_h273(u16::try_from(p).unwrap_or(2), u16::try_from(t).unwrap_or(2), range);
            }
            _ => {}
        }
    }
    let (Ok(coded_width), Ok(coded_height)) = (u32::try_from(w), u32::try_from(h)) else {
        return Err(ProbeError::Malformed("impossible picture size"));
    };
    let frame_rate = default_duration
        .filter(|&ns| ns > 0)
        .and_then(|ns| i64::try_from(ns).ok())
        .and_then(|ns| FrameRate::from_measured(1_000_000_000, ns).ok());
    Ok(VideoInfo {
        codec,
        coded_width,
        coded_height,
        rotation: 0,
        frame_rate,
        variable_frame_rate: false,
        bit_depth,
        color,
        dolby_vision,
        hdr_metadata,
    })
}

fn parse_audio(codec_id: &str, audio_el: &[u8]) -> Result<AudioInfo, ProbeError> {
    let (mut rate, mut channels, mut bits) = (8_000.0_f64, 1_u64, 0_u64);
    for el in elements(audio_el) {
        let (id, v) = el?;
        match id {
            SAMPLING_FREQUENCY => rate = float(v).unwrap_or(rate),
            CHANNELS => channels = uint(v),
            BIT_DEPTH => bits = uint(v),
            _ => {}
        }
    }
    let codec = match codec_id {
        c if c.starts_with("A_AAC") => "AAC".to_owned(),
        "A_OPUS" => "Opus".to_owned(),
        "A_VORBIS" => "Vorbis".to_owned(),
        "A_FLAC" => "FLAC".to_owned(),
        "A_AC3" => "Dolby Digital (AC-3)".to_owned(),
        "A_EAC3" => "Dolby Digital Plus".to_owned(),
        "A_TRUEHD" => "Dolby TrueHD".to_owned(),
        c if c.starts_with("A_DTS") => "DTS".to_owned(),
        "A_MPEG/L3" => "MP3".to_owned(),
        c if c.starts_with("A_PCM") => format!("PCM {bits}-bit"),
        other => other.strip_prefix("A_").unwrap_or(other).to_owned(),
    };
    let sample_rate = if rate.is_finite() && rate > 0.0 && rate <= f64::from(motix_core::limits::MAX_SAMPLE_RATE_HZ) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            rate.round() as u32
        }
    } else {
        return Err(ProbeError::Malformed("impossible sample rate"));
    };
    Ok(AudioInfo {
        codec,
        channels: u16::try_from(channels.clamp(1, u64::from(motix_core::limits::MAX_AUDIO_CHANNELS))).unwrap_or(1),
        sample_rate,
    })
}

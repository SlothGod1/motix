//! ISO Base Media File Format (MP4, MOV, M4A, 3GP): reads the `moov` box only.

use crate::bytes::{Bytes, read_vec};
use crate::{
    AudioInfo, ColorInfo, Container, DolbyVision, MAX_DEPTH, MAX_TRACKS, MediaInfo, ProbeError, VideoInfo,
    time_from_ticks,
};
use motix_core::{FrameRate, Time};
use std::io::{Read, Seek, SeekFrom};

const MAX_TOP_LEVEL_BOXES: usize = 10_000;

/// Box types that can start a MOV/MP4 file.
pub(crate) fn looks_like(fourcc: &[u8]) -> bool {
    matches!(
        fourcc,
        b"ftyp" | b"moov" | b"mdat" | b"wide" | b"free" | b"skip" | b"pnot" | b"uuid"
    )
}

pub(crate) fn probe<R: Read + Seek>(r: &mut R, len: u64) -> Result<MediaInfo, ProbeError> {
    let mut pos = 0_u64;
    let mut container = Container::QuickTime;
    for _ in 0..MAX_TOP_LEVEL_BOXES {
        if pos.checked_add(8).is_none_or(|end| end > len) {
            break;
        }
        r.seek(SeekFrom::Start(pos))?;
        let mut head = [0_u8; 8];
        r.read_exact(&mut head)?;
        let size32 = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
        let kind = [head[4], head[5], head[6], head[7]];
        let (size, header) = match size32 {
            0 => (len - pos, 8),
            1 => {
                let mut large = [0_u8; 8];
                r.read_exact(&mut large)?;
                (u64::from_be_bytes(large), 16)
            }
            s => (u64::from(s), 8),
        };
        if size < header || pos.checked_add(size).is_none_or(|end| end > len) {
            return Err(ProbeError::Malformed("a box is larger than the file"));
        }
        match &kind {
            b"ftyp" => {
                let body = read_vec(r, (size - header).min(4096))?;
                if body.get(0..4) != Some(b"qt  ".as_slice()) {
                    container = Container::Mp4;
                }
            }
            b"moov" => {
                let body = read_vec(r, size - header)?;
                return parse_moov(&body, container);
            }
            _ => {}
        }
        pos += size;
    }
    Err(ProbeError::Malformed("no movie header (moov) found"))
}

/// Iterates the child boxes of a box body.
struct Boxes<'a> {
    b: Bytes<'a>,
}

impl<'a> Iterator for Boxes<'a> {
    type Item = Result<([u8; 4], &'a [u8]), ProbeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.b.remaining() < 8 {
            return None;
        }
        Some((|| {
            let size = self.b.be_u32()?;
            let kind = self.b.fourcc()?;
            let body_len = match size {
                0 => self.b.remaining(),
                1 => {
                    let large = self.b.be_u64()?;
                    usize::try_from(large.checked_sub(16).ok_or(ProbeError::Malformed("bad box size"))?)
                        .map_err(|_| ProbeError::TooLarge)?
                }
                s => usize::try_from(s.checked_sub(8).ok_or(ProbeError::Malformed("bad box size"))?)
                    .map_err(|_| ProbeError::TooLarge)?,
            };
            Ok((kind, self.b.take(body_len)?))
        })())
    }
}

fn boxes(data: &[u8]) -> Boxes<'_> {
    Boxes { b: Bytes::new(data) }
}

fn find(data: &[u8], kind: [u8; 4]) -> Result<Option<&[u8]>, ProbeError> {
    for item in boxes(data) {
        let (k, body) = item?;
        if k == kind {
            return Ok(Some(body));
        }
    }
    Ok(None)
}

/// Follows a path of nested boxes, e.g. `[b"mdia", b"minf", b"stbl"]`.
fn find_path<'a>(mut data: &'a [u8], path: &[[u8; 4]]) -> Result<Option<&'a [u8]>, ProbeError> {
    if path.len() > MAX_DEPTH {
        return Err(ProbeError::Malformed("nesting too deep"));
    }
    for kind in path {
        match find(data, *kind)? {
            Some(body) => data = body,
            None => return Ok(None),
        }
    }
    Ok(Some(data))
}

struct Track {
    handler: [u8; 4],
    timescale: u32,
    duration: u64,
    rotation: u16,
    video: Option<VideoInfo>,
    audio: Option<AudioInfo>,
}

fn parse_moov(moov: &[u8], container: Container) -> Result<MediaInfo, ProbeError> {
    let mut movie_duration = None;
    let mut tracks = Vec::new();
    for item in boxes(moov) {
        let (kind, body) = item?;
        match &kind {
            b"mvhd" => movie_duration = parse_mvhd(body)?,
            b"trak" => {
                if tracks.len() >= MAX_TRACKS {
                    return Err(ProbeError::Malformed("too many tracks"));
                }
                if let Some(t) = parse_trak(body)? {
                    tracks.push(t);
                }
            }
            _ => {}
        }
    }
    let track_duration = tracks
        .iter()
        .filter(|t| t.video.is_some() || t.audio.is_some())
        .filter_map(|t| time_from_ticks(t.duration, t.timescale))
        .max();
    let duration = track_duration.or(movie_duration).filter(|d| *d > Time::ZERO);
    let mut video = None;
    let mut audio = Vec::new();
    for t in tracks {
        match &t.handler {
            b"vide" if video.is_none() => {
                video = t.video.map(|mut v| {
                    v.rotation = t.rotation;
                    v
                });
            }
            b"soun" => audio.extend(t.audio),
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

fn parse_mvhd(body: &[u8]) -> Result<Option<Time>, ProbeError> {
    let mut b = Bytes::new(body);
    let version = b.u8()?;
    b.skip(3)?;
    let (timescale, duration) = if version == 1 {
        b.skip(16)?;
        (b.be_u32()?, b.be_u64()?)
    } else {
        b.skip(8)?;
        (b.be_u32()?, u64::from(b.be_u32()?))
    };
    Ok(time_from_ticks(duration, timescale))
}

fn parse_trak(trak: &[u8]) -> Result<Option<Track>, ProbeError> {
    let rotation = match find(trak, *b"tkhd")? {
        Some(tkhd) => parse_tkhd_rotation(tkhd)?,
        None => 0,
    };
    let Some(mdia) = find(trak, *b"mdia")? else {
        return Ok(None);
    };
    let Some(mdhd) = find(mdia, *b"mdhd")? else {
        return Ok(None);
    };
    let (timescale, duration) = {
        let mut b = Bytes::new(mdhd);
        let version = b.u8()?;
        b.skip(3)?;
        if version == 1 {
            b.skip(16)?;
            (b.be_u32()?, b.be_u64()?)
        } else {
            b.skip(8)?;
            (b.be_u32()?, u64::from(b.be_u32()?))
        }
    };
    let handler = match find(mdia, *b"hdlr")? {
        Some(h) => {
            let mut b = Bytes::new(h);
            b.skip(8)?;
            b.fourcc()?
        }
        None => return Ok(None),
    };
    let Some(stbl) = find_path(mdia, &[*b"minf", *b"stbl"])? else {
        return Ok(None);
    };
    let entry = match find(stbl, *b"stsd")? {
        Some(stsd) => {
            let mut b = Bytes::new(stsd);
            b.skip(8)?; // version/flags, entry_count
            boxes(b.rest()).next().transpose()?
        }
        None => None,
    };
    let mut result = Track {
        handler,
        timescale,
        duration,
        rotation,
        video: None,
        audio: None,
    };
    let Some((fourcc, body)) = entry else {
        return Ok(Some(result));
    };
    match &handler {
        b"vide" => {
            let mut v = parse_visual_entry(fourcc, body)?;
            if let Some(stts) = find(stbl, *b"stts")? {
                let (rate, vfr) = frame_rate_from_stts(stts, timescale)?;
                v.frame_rate = rate;
                v.variable_frame_rate = vfr;
            }
            result.video = Some(v);
        }
        b"soun" => result.audio = Some(parse_audio_entry(fourcc, body, timescale)?),
        _ => {}
    }
    Ok(Some(result))
}

fn parse_tkhd_rotation(tkhd: &[u8]) -> Result<u16, ProbeError> {
    let mut b = Bytes::new(tkhd);
    let version = b.u8()?;
    b.skip(3)?;
    b.skip(if version == 1 { 32 } else { 20 })?;
    b.skip(16)?; // reserved, layer, alternate group, volume, reserved
    let a = b.be_i32()?;
    let bb = b.be_i32()?;
    b.skip(4)?;
    let c = b.be_i32()?;
    let d = b.be_i32()?;
    let one = 0x1_0000;
    Ok(match (a, bb, c, d) {
        (0, x, y, 0) if x == one && y == -one => 90,
        (x, 0, 0, y) if x == -one && y == -one => 180,
        (0, x, y, 0) if x == -one && y == one => 270,
        _ => 0,
    })
}

fn codec_name(fourcc: [u8; 4]) -> (String, Option<u8>, bool) {
    // (name, implied bit depth, is Dolby Vision sample entry)
    let s = |n: &str| n.to_owned();
    match &fourcc {
        b"avc1" | b"avc3" => (s("H.264"), None, false),
        b"hvc1" | b"hev1" => (s("HEVC"), None, false),
        b"dvh1" | b"dvhe" => (s("HEVC"), None, true),
        b"dva1" | b"dvav" => (s("H.264"), None, true),
        b"av01" => (s("AV1"), None, false),
        b"dav1" => (s("AV1"), None, true),
        b"vp09" => (s("VP9"), None, false),
        b"vp08" => (s("VP8"), Some(8), false),
        b"apch" => (s("ProRes 422 HQ"), Some(10), false),
        b"apcn" => (s("ProRes 422"), Some(10), false),
        b"apcs" => (s("ProRes 422 LT"), Some(10), false),
        b"apco" => (s("ProRes 422 Proxy"), Some(10), false),
        b"ap4h" => (s("ProRes 4444"), Some(12), false),
        b"ap4x" => (s("ProRes 4444 XQ"), Some(12), false),
        b"AVdn" | b"AVdh" => (s("DNxHD/DNxHR"), None, false),
        b"mp4v" => (s("MPEG-4 Part 2"), Some(8), false),
        b"jpeg" | b"mjpa" | b"mjpb" => (s("Motion JPEG"), Some(8), false),
        other => (String::from_utf8_lossy(other).trim().to_owned(), None, false),
    }
}

fn parse_visual_entry(fourcc: [u8; 4], body: &[u8]) -> Result<VideoInfo, ProbeError> {
    let mut b = Bytes::new(body);
    b.skip(24)?;
    let width = b.be_u16()?;
    let height = b.be_u16()?;
    b.skip(50)?;
    let (codec, mut bit_depth, dv_entry) = codec_name(fourcc);
    let mut v = VideoInfo {
        codec,
        coded_width: u32::from(width),
        coded_height: u32::from(height),
        rotation: 0,
        frame_rate: None,
        variable_frame_rate: false,
        bit_depth: None,
        color: ColorInfo::default(),
        dolby_vision: dv_entry.then_some(DolbyVision { profile: None }),
        hdr_metadata: false,
    };
    let mut color_from_colr = false;
    for item in boxes(b.rest()) {
        let Ok((kind, child)) = item else { break };
        let mut c = Bytes::new(child);
        match &kind {
            b"colr" => {
                let kind = c.fourcc()?;
                if &kind == b"nclx" || &kind == b"nclc" {
                    let p = c.be_u16()?;
                    let t = c.be_u16()?;
                    let _matrix = c.be_u16()?;
                    let full = if &kind == b"nclx" {
                        Some(c.u8()? & 0x80 != 0)
                    } else {
                        None
                    };
                    v.color = ColorInfo::from_h273(p, t, full);
                    color_from_colr = true;
                }
            }
            b"hvcC" => {
                if let Some(depth) = child.get(17) {
                    bit_depth = Some((depth & 0x07) + 8);
                }
            }
            b"avcC" => {
                bit_depth = bit_depth.or(avc_bit_depth(child));
            }
            b"av1C" => {
                if let Some(flags) = child.get(2) {
                    let high = flags & 0x40 != 0;
                    let twelve = flags & 0x20 != 0;
                    bit_depth = Some(match (high, twelve) {
                        (true, true) => 12,
                        (true, false) => 10,
                        _ => 8,
                    });
                }
            }
            // FullBox: version/flags(4) profile(1) level(1) depth|chroma|range(1) primaries transfer matrix
            b"vpcC" if child.len() >= 10 => {
                bit_depth = Some(child[6] >> 4);
                if !color_from_colr {
                    v.color = ColorInfo::from_h273(u16::from(child[7]), u16::from(child[8]), Some(child[6] & 1 != 0));
                }
            }
            b"dvcC" | b"dvvC" | b"dvwC" => {
                v.dolby_vision = Some(DolbyVision {
                    profile: child.get(2).map(|p| p >> 1),
                });
            }
            b"mdcv" | b"clli" | b"SmDm" | b"CoLL" => v.hdr_metadata = true,
            _ => {}
        }
    }
    v.bit_depth = bit_depth.filter(|d| (8..=16).contains(d));
    Ok(v)
}

/// Bit depth from an `avcC` record: only High profiles carry it; others are 8-bit.
pub(crate) fn avc_bit_depth(avcc: &[u8]) -> Option<u8> {
    let mut b = Bytes::new(avcc);
    b.skip(1).ok()?;
    let profile = b.u8().ok()?;
    b.skip(3).ok()?; // compat, level, length size
    let sps_count = b.u8().ok()? & 0x1F;
    for _ in 0..sps_count {
        let n = usize::from(b.be_u16().ok()?);
        b.skip(n).ok()?;
    }
    let pps_count = b.u8().ok()?;
    for _ in 0..pps_count {
        let n = usize::from(b.be_u16().ok()?);
        b.skip(n).ok()?;
    }
    if matches!(profile, 100 | 110 | 122 | 244) && b.remaining() >= 4 {
        b.skip(1).ok()?; // chroma format
        return Some((b.u8().ok()? & 0x07) + 8);
    }
    Some(8)
}

fn parse_audio_entry(fourcc: [u8; 4], body: &[u8], timescale: u32) -> Result<AudioInfo, ProbeError> {
    let mut b = Bytes::new(body);
    b.skip(8)?;
    let version = b.be_u16()?;
    b.skip(6)?;
    let mut channels = b.be_u16()?;
    let sample_size = b.be_u16()?;
    b.skip(4)?;
    let mut rate = b.be_u32()? >> 16;
    if version == 2 {
        // QuickTime sound description v2: sizeOfStructOnly, then f64 rate and u32 channels.
        b.skip(4)?;
        let r = f64::from_bits(b.be_u64()?);
        let ch = b.be_u32()?;
        if r.is_finite() && r > 0.0 && r < 1.0e7 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                rate = r.round() as u32;
            }
        }
        channels = u16::try_from(ch).unwrap_or(0);
    }
    if (8_000..=motix_core::limits::MAX_SAMPLE_RATE_HZ).contains(&timescale) {
        rate = timescale;
    }
    let codec = match &fourcc {
        b"mp4a" => "AAC".to_owned(),
        b"ac-3" => "Dolby Digital (AC-3)".to_owned(),
        b"ec-3" => "Dolby Digital Plus".to_owned(),
        b"Opus" => "Opus".to_owned(),
        b"fLaC" => "FLAC".to_owned(),
        b"alac" => "Apple Lossless".to_owned(),
        b".mp3" => "MP3".to_owned(),
        b"samr" => "AMR".to_owned(),
        b"sowt" | b"twos" | b"lpcm" | b"ipcm" | b"in24" | b"in32" | b"raw " => {
            let bits = match &fourcc {
                b"in24" => 24,
                b"in32" => 32,
                _ => sample_size,
            };
            format!("PCM {bits}-bit")
        }
        b"fl32" | b"fl64" | b"fpcm" => "PCM float".to_owned(),
        other => String::from_utf8_lossy(other).trim().to_owned(),
    };
    Ok(AudioInfo {
        codec,
        channels: channels.min(u16::try_from(motix_core::limits::MAX_AUDIO_CHANNELS).unwrap_or(64)),
        sample_rate: rate,
    })
}

/// Frame rate from the time-to-sample table. Returns `(rate, variable)`.
fn frame_rate_from_stts(stts: &[u8], timescale: u32) -> Result<(Option<FrameRate>, bool), ProbeError> {
    let mut b = Bytes::new(stts);
    b.skip(4)?;
    let count = b.be_u32()? as usize;
    let count = count.min(b.remaining() / 8);
    let mut entries = Vec::with_capacity(count.min(4096));
    let mut total_samples = 0_u64;
    let mut total_delta = 0_u64;
    for _ in 0..count {
        let n = b.be_u32()?;
        let delta = b.be_u32()?;
        total_samples = total_samples.saturating_add(u64::from(n));
        total_delta = total_delta.saturating_add(u64::from(n) * u64::from(delta));
        entries.push((n, delta));
    }
    if timescale == 0 || total_samples == 0 || total_delta == 0 {
        return Ok((None, false));
    }
    // Most common delta, weighted by sample count.
    entries.sort_by_key(|e| e.1);
    let mut best = (0_u64, 0_u32);
    let mut i = 0;
    while i < entries.len() {
        let delta = entries[i].1;
        let mut n = 0_u64;
        while i < entries.len() && entries[i].1 == delta {
            n += u64::from(entries[i].0);
            i += 1;
        }
        if n > best.0 {
            best = (n, delta);
        }
    }
    // Variable if more than 2% of frames differ from the common duration.
    let variable = total_samples - best.0 > total_samples / 50 + 1;
    let (num, den) = if variable || best.1 == 0 {
        (
            i64::try_from(total_samples.saturating_mul(u64::from(timescale))).unwrap_or(i64::MAX),
            i64::try_from(total_delta).unwrap_or(i64::MAX),
        )
    } else {
        (i64::from(timescale), i64::from(best.1))
    };
    Ok((FrameRate::from_measured(num, den).ok(), variable))
}

//! RIFF WAVE audio.

use crate::bytes::{Bytes, read_vec};
use crate::{AudioInfo, Container, MediaInfo, ProbeError, time_from_ticks};
use std::io::{Read, Seek, SeekFrom};

const MAX_CHUNKS: usize = 10_000;

pub(crate) fn probe<R: Read + Seek>(r: &mut R, len: u64) -> Result<MediaInfo, ProbeError> {
    let mut pos = 12_u64;
    let mut fmt: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data_len = None;
    for _ in 0..MAX_CHUNKS {
        if pos + 8 > len {
            break;
        }
        r.seek(SeekFrom::Start(pos))?;
        let head = read_vec(r, 8)?;
        let mut h = Bytes::new(&head);
        let id = h.fourcc()?;
        let size = u64::from(h.le_u32()?);
        match &id {
            b"fmt " => {
                let body = read_vec(r, size.min(64))?;
                let mut b = Bytes::new(&body);
                let mut tag = b.le_u16()?;
                let channels = b.le_u16()?;
                let rate = b.le_u32()?;
                b.skip(4)?;
                let block_align = b.le_u16()?;
                let bits = b.le_u16()?;
                if tag == 0xFFFE && b.remaining() >= 10 {
                    b.skip(8)?; // cbSize, valid bits, channel mask
                    tag = b.le_u16()?;
                }
                fmt = Some((tag, channels, rate, block_align, bits));
            }
            b"data" => {
                // 0xFFFFFFFF means "unknown / RF64": use the rest of the file.
                let available = len - (pos + 8);
                data_len = Some(if size == 0xFFFF_FFFF {
                    available
                } else {
                    size.min(available)
                });
                if fmt.is_some() {
                    break;
                }
            }
            _ => {}
        }
        pos = pos.saturating_add(8 + size + (size & 1));
    }
    let (tag, channels, rate, block_align, bits) = fmt.ok_or(ProbeError::Malformed("no audio format (fmt) chunk"))?;
    if channels == 0 || rate == 0 || rate > motix_core::limits::MAX_SAMPLE_RATE_HZ {
        return Err(ProbeError::Malformed("impossible audio format"));
    }
    let codec = match tag {
        1 => format!("PCM {bits}-bit"),
        3 => "PCM float".to_owned(),
        6 => "A-law".to_owned(),
        7 => "µ-law".to_owned(),
        0x55 => "MP3".to_owned(),
        other => format!("WAV format {other:#06x}"),
    };
    let duration = match (data_len, block_align) {
        (Some(n), a) if a > 0 => time_from_ticks(n / u64::from(a), rate),
        _ => None,
    };
    Ok(MediaInfo {
        container: Container::Wav,
        duration,
        video: None,
        audio: vec![AudioInfo {
            codec,
            channels,
            sample_rate: rate,
        }],
        still: false,
    })
}

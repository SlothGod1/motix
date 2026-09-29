//! Still images: PNG and JPEG picture size.

use crate::bytes::{Bytes, read_vec};
use crate::{ColorInfo, Container, MediaInfo, ProbeError, VideoInfo};
use std::io::{Read, Seek, SeekFrom};

/// JPEG markers are scanned in the first 16 MiB only.
const MAX_JPEG_SCAN: u64 = 16 * 1024 * 1024;

fn still(
    container: Container,
    codec: &str,
    width: u32,
    height: u32,
    bit_depth: Option<u8>,
) -> Result<MediaInfo, ProbeError> {
    let max = motix_core::limits::MAX_IMAGE_DIMENSION * 4;
    if width == 0 || height == 0 || width > max || height > max {
        return Err(ProbeError::Malformed("impossible image size"));
    }
    Ok(MediaInfo {
        container,
        duration: None,
        video: Some(VideoInfo {
            codec: codec.to_owned(),
            coded_width: width,
            coded_height: height,
            rotation: 0,
            frame_rate: None,
            variable_frame_rate: false,
            bit_depth,
            color: ColorInfo::default(),
            dolby_vision: None,
            hdr_metadata: false,
        }),
        audio: Vec::new(),
        still: true,
    })
}

pub(crate) fn probe_png<R: Read + Seek>(r: &mut R) -> Result<MediaInfo, ProbeError> {
    let head = read_vec(r, 25)?;
    let mut b = Bytes::new(&head);
    b.skip(8)?;
    b.skip(4)?; // chunk length
    if b.fourcc()? != *b"IHDR" {
        return Err(ProbeError::Malformed("PNG without IHDR"));
    }
    let w = b.be_u32()?;
    let h = b.be_u32()?;
    let depth = b.u8()?;
    still(Container::Png, "PNG", w, h, Some(depth.max(8)))
}

pub(crate) fn probe_jpeg<R: Read + Seek>(r: &mut R, len: u64) -> Result<MediaInfo, ProbeError> {
    let mut pos = 2_u64;
    let limit = len.min(MAX_JPEG_SCAN);
    while pos + 4 <= limit {
        r.seek(SeekFrom::Start(pos))?;
        let head = read_vec(r, 4)?;
        if head[0] != 0xFF {
            return Err(ProbeError::Malformed("JPEG marker expected"));
        }
        let marker = head[1];
        if marker == 0xFF {
            pos += 1; // fill byte
            continue;
        }
        if matches!(marker, 0x01 | 0xD0..=0xD9) {
            pos += 2;
            continue;
        }
        let seg_len = u64::from(u16::from_be_bytes([head[2], head[3]]));
        if seg_len < 2 {
            return Err(ProbeError::Malformed("bad JPEG segment"));
        }
        let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if is_sof {
            let body = read_vec(r, 5)?;
            let mut b = Bytes::new(&body);
            let precision = b.u8()?;
            let h = b.be_u16()?;
            let w = b.be_u16()?;
            return still(Container::Jpeg, "JPEG", u32::from(w), u32::from(h), Some(precision));
        }
        if marker == 0xDA {
            break; // image data started without a frame header
        }
        pos += 2 + seg_len;
    }
    Err(ProbeError::Malformed("JPEG without a frame header"))
}

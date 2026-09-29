//! Probes real files generated with FFmpeg (see `fixtures/README.md`) and checks the
//! results against `ffprobe`'s answers. Also feeds corrupted copies to make sure
//! damaged files produce errors, never panics.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use motix_core::{FrameRate, Time};
use motix_probe::{Container, DynamicRange, Primaries, ProbeError, Transfer, probe, probe_path};
use std::io::Cursor;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn ms(t: Option<Time>) -> i64 {
    (t.expect("duration").as_seconds_f64() * 1000.0).round() as i64
}

#[test]
fn h264_aac_mp4() {
    let m = probe_path(&fixture("h264_aac_2997.mp4")).unwrap();
    assert_eq!(m.container, Container::Mp4);
    let v = m.video.as_ref().unwrap();
    assert_eq!((v.codec.as_str(), v.coded_width, v.coded_height), ("H.264", 64, 36));
    assert_eq!(v.frame_rate, Some(FrameRate::FPS_29_97));
    assert!(!v.variable_frame_rate);
    assert_eq!(v.bit_depth, Some(8));
    assert_eq!(v.dynamic_range(), DynamicRange::Sdr);
    assert_eq!(v.rotation, 0);
    assert_eq!(m.audio.len(), 1);
    assert_eq!(
        (m.audio[0].codec.as_str(), m.audio[0].channels, m.audio[0].sample_rate),
        ("AAC", 2, 48_000)
    );
    assert!((490..=530).contains(&ms(m.duration)), "{:?}", m.duration);
    assert!(!m.still);
}

#[test]
fn hevc_10_bit_hdr10_mp4() {
    let m = probe_path(&fixture("hevc10_pq.mp4")).unwrap();
    let v = m.video.unwrap();
    assert_eq!(v.codec, "HEVC");
    assert_eq!(v.bit_depth, Some(10));
    assert_eq!(v.frame_rate, Some(FrameRate::FPS_60));
    assert_eq!(v.color.transfer, Transfer::Pq);
    assert_eq!(v.color.primaries, Primaries::Bt2020);
    assert_eq!(v.dynamic_range(), DynamicRange::Hdr10);
    assert!(m.audio.is_empty());
}

#[test]
fn hevc_10_bit_hlg_mov() {
    let m = probe_path(&fixture("hevc10_hlg.mov")).unwrap();
    assert_eq!(m.container, Container::QuickTime);
    let v = m.video.unwrap();
    assert_eq!(v.frame_rate, Some(FrameRate::FPS_50));
    assert_eq!(v.dynamic_range(), DynamicRange::Hlg);
    assert_eq!(v.bit_depth, Some(10));
}

#[test]
fn av1_10_bit() {
    let v = probe_path(&fixture("av1_10bit.mp4")).unwrap().video.unwrap();
    assert_eq!(v.codec, "AV1");
    assert_eq!(v.bit_depth, Some(10));
    assert_eq!(v.frame_rate, Some(FrameRate::FPS_30));
}

#[test]
fn phone_rotation_swaps_display_size() {
    let v = probe_path(&fixture("rotated_phone.mp4")).unwrap().video.unwrap();
    assert!(v.rotation == 90 || v.rotation == 270, "rotation {}", v.rotation);
    assert_eq!((v.display_width(), v.display_height()), (36, 64));
    assert_eq!((v.coded_width, v.coded_height), (64, 36));
}

#[test]
fn audio_only_m4a() {
    let m = probe_path(&fixture("audio_only.m4a")).unwrap();
    assert!(m.video.is_none());
    assert_eq!((m.audio[0].channels, m.audio[0].sample_rate), (1, 44_100));
}

#[test]
fn wav_files() {
    let m = probe_path(&fixture("stereo_48k.wav")).unwrap();
    assert_eq!(m.container, Container::Wav);
    assert_eq!(m.audio[0].codec, "PCM 16-bit");
    assert_eq!((m.audio[0].channels, m.audio[0].sample_rate), (2, 48_000));
    assert_eq!(ms(m.duration), 500);
    let m = probe_path(&fixture("mono_96k_24bit.wav")).unwrap();
    assert_eq!(m.audio[0].codec, "PCM 24-bit");
    assert_eq!((m.audio[0].channels, m.audio[0].sample_rate), (1, 96_000));
    assert_eq!(ms(m.duration), 250);
}

#[test]
fn matroska_and_webm() {
    let m = probe_path(&fixture("h264_flac_25.mkv")).unwrap();
    assert_eq!(m.container, Container::Matroska);
    let v = m.video.as_ref().unwrap();
    assert_eq!((v.codec.as_str(), v.coded_width, v.coded_height), ("H.264", 64, 36));
    assert_eq!(v.frame_rate, Some(FrameRate::FPS_25));
    assert_eq!(v.bit_depth, Some(8));
    assert_eq!((m.audio[0].codec.as_str(), m.audio[0].sample_rate), ("FLAC", 48_000));
    assert!((190..=210).contains(&ms(m.duration)));

    let m = probe_path(&fixture("vp9_opus.webm")).unwrap();
    assert_eq!(m.container, Container::WebM);
    assert_eq!(m.video.as_ref().unwrap().codec, "VP9");
    assert_eq!(m.audio[0].codec, "Opus");
    assert!((240..=270).contains(&ms(m.duration)));
}

#[test]
fn still_images() {
    let m = probe_path(&fixture("still.png")).unwrap();
    assert!(m.still);
    let v = m.video.unwrap();
    assert_eq!((v.coded_width, v.coded_height), (48, 64));
    let v = probe_path(&fixture("still.jpg")).unwrap().video.unwrap();
    assert_eq!((v.coded_width, v.coded_height), (72, 40));
}

#[test]
fn unknown_and_missing_files() {
    assert_eq!(
        probe(Cursor::new(b"hello world, not media".to_vec())),
        Err(ProbeError::UnknownFormat)
    );
    assert_eq!(probe(Cursor::new(Vec::new())), Err(ProbeError::UnknownFormat));
    assert!(matches!(
        probe_path(Path::new("definitely/missing.mp4")),
        Err(ProbeError::Io(_))
    ));
}

/// Deterministic xorshift so failures are reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn corrupted_files_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let names = [
        "h264_aac_2997.mp4",
        "hevc10_pq.mp4",
        "hevc10_hlg.mov",
        "rotated_phone.mp4",
        "audio_only.m4a",
        "stereo_48k.wav",
        "h264_flac_25.mkv",
        "vp9_opus.webm",
        "still.png",
        "still.jpg",
        "av1_10bit.mp4",
    ];
    for name in names {
        let original = std::fs::read(fixture(name)).unwrap();
        for round in 0..400 {
            let mut data = original.clone();
            match round % 4 {
                // Flip random bytes.
                0 | 1 => {
                    for _ in 0..=rng.below(8) {
                        let i = rng.below(data.len());
                        data[i] = rng.next() as u8;
                    }
                }
                // Truncate.
                2 => data.truncate(rng.below(data.len())),
                // Write a huge size field somewhere.
                _ => {
                    let i = rng.below(data.len().saturating_sub(4).max(1));
                    for b in data.iter_mut().skip(i).take(4) {
                        *b = 0xFF;
                    }
                }
            }
            // Must return (Ok or Err) without panicking.
            let _ = probe(Cursor::new(data));
        }
    }
}

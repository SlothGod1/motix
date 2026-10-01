//! Real decoding with the FFmpeg on this machine (skipped when it isn't installed).
#![allow(clippy::cast_precision_loss)] // byte counts to seconds

use motix_app::{AppState, ClipId};
use motix_core::{FrameRate, Time};
use motix_media::plan::{VideoTarget, audio_plan, frame_size, mix_args, video_target};
use motix_media::{Tools, VideoEngine, VideoRequest, command};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../motix-probe/tests/fixtures")
        .join(name)
}

fn tools() -> Option<Tools> {
    let t = Tools::find(&[]);
    if t.is_none() {
        eprintln!("FFmpeg not installed here; skipping");
    }
    t
}

fn wait_for_frame(engine: &VideoEngine, after: u64) -> Option<std::sync::Arc<motix_media::Frame>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let (generation, frame) = engine.latest();
        if generation > after {
            return frame;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no frame arrived");
}

fn target(name: &str, at: f64, hdr: bool) -> VideoTarget {
    VideoTarget {
        clip: ClipId(1),
        path: fixture(name),
        source_seconds: at,
        still: Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png")),
        hdr,
        size: frame_size(None),
    }
}

#[test]
fn single_frames_scrubbing_and_hdr() {
    let Some(tools) = tools() else { return };
    assert!(motix_media::has_filter(&tools, "scale"));
    let engine = VideoEngine::start(tools);
    for (name, hdr) in [
        ("h264_aac_2997.mp4", false),
        ("hevc10_pq.mp4", true),
        ("still.png", false),
    ] {
        let (before, _) = engine.latest();
        engine.show(Some(VideoRequest {
            target: VideoTarget {
                size: (320, 180),
                ..target(name, 0.1, hdr)
            },
            playing: false,
            fps: FrameRate::FPS_30,
        }));
        let frame = wait_for_frame(&engine, before).unwrap_or_else(|| panic!("{name}: no picture"));
        assert_eq!((frame.width, frame.height), (320, 180));
        assert_eq!(frame.rgba.len(), 320 * 180 * 4);
        assert!(
            frame.rgba.chunks(4).any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16),
            "{name}: not black"
        );
    }
    assert!(engine.problem().is_none());
    // Nothing on screen clears the picture.
    let (before, _) = engine.latest();
    engine.show(None);
    assert!(wait_for_frame(&engine, before).is_none());
}

#[test]
fn playback_streams_frames() {
    let Some(tools) = tools() else { return };
    let engine = VideoEngine::start(tools);
    let mut seen = 0;
    let mut generation = engine.latest().0;
    for i in 0..10 {
        engine.show(Some(VideoRequest {
            target: VideoTarget {
                size: (160, 90),
                ..target("h264_aac_2997.mp4", f64::from(i) / 30.0, false)
            },
            playing: true,
            fps: FrameRate::FPS_30,
        }));
        let frame = wait_for_frame(&engine, generation);
        generation = engine.latest().0;
        if frame.is_some() {
            seen += 1;
        }
    }
    assert!(seen >= 5, "frames kept coming while playing ({seen})");
}

#[test]
fn the_timeline_mix_has_the_right_length() {
    let Some(tools) = tools() else { return };
    let mut s = AppState::default();
    s.match_asked = true;
    s.import(vec![fixture("stereo_48k.wav"), fixture("h264_aac_2997.mp4")]);
    let wav = s.media.items()[0].id;
    let video = s.media.items()[1].id;
    s.add_to_timeline(video, None, None).unwrap();
    s.playhead = Time::from_seconds(0).unwrap();
    s.add_to_timeline(wav, None, None).unwrap();
    assert!(video_target(&s.timeline, &s.media, Time::ZERO).is_some());

    let plan = audio_plan(&s.timeline, &s.media, Time::ZERO);
    assert_eq!(plan.len(), 2, "the video's sound and the WAV");
    let longest = plan.iter().map(|i| i.delay + i.duration).fold(0.0, f64::max);
    let out = command(&tools.ffmpeg)
        .args(mix_args(&plan).unwrap())
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    assert!(out.status.success());
    let seconds = out.stdout.len() as f64 / (48_000.0 * 2.0 * 4.0);
    assert!(
        (seconds - longest).abs() < 0.1,
        "mix is {seconds:.2}s, clips last {longest:.2}s"
    );
    let loud = out
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs())
        .fold(0.0_f32, f32::max);
    assert!(loud > 0.01, "the mix isn't silent");
}

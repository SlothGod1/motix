//! What to decode: which picture is on screen at the playhead, which sounds play from
//! there, and the FFmpeg command lines that produce them. Pure functions, no processes.

use motix_app::probe::DynamicRange;
use motix_app::{ClipId, ClipSource, MediaBin, MediaKind, Resolution, Timeline, TrackKind};
use motix_core::{FLICKS_PER_SECOND, FrameRate, Time};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;

/// Largest preview frame side, in pixels (the viewer is rarely bigger; smaller frames
/// keep playback light).
pub const MAX_PREVIEW_SIDE: u32 = 1280;
/// Sample rate of preview audio.
pub const AUDIO_RATE: u32 = 48_000;
/// How far ahead one audio mix reaches; playback restarts the mix after this.
pub const AUDIO_WINDOW_SECONDS: f64 = 600.0;
/// Most sounds mixed at once.
pub const MAX_AUDIO_INPUTS: usize = 48;

/// Seconds as a float (for FFmpeg arguments and frame maths).
#[must_use]
pub fn seconds(t: Time) -> f64 {
    t.flicks() as f64 / FLICKS_PER_SECOND as f64
}

/// The picture to show at a moment of the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoTarget {
    /// The clip.
    pub clip: ClipId,
    /// Media file.
    pub path: PathBuf,
    /// Position in the file, seconds.
    pub source_seconds: f64,
    /// A still image (same picture at every moment).
    pub still: bool,
    /// HDR (PQ/HLG) source, tone-mapped for the preview.
    pub hdr: bool,
    /// Preview frame size.
    pub size: (u32, u32),
}

/// The preview frame size for a picture of `source` size: fits in
/// [`MAX_PREVIEW_SIDE`], keeps the shape, even numbers.
#[must_use]
pub fn frame_size(source: Option<Resolution>) -> (u32, u32) {
    let (w, h) = source.map_or((1920.0, 1080.0), |r| (f64::from(r.width), f64::from(r.height)));
    let scale = (f64::from(MAX_PREVIEW_SIDE) / w.max(h)).min(1.0);
    let even = |v: f64| ((v * scale / 2.0).round() as u32).max(1) * 2;
    (even(w), even(h))
}

/// The topmost visible picture at `t`, and where in its file to look.
#[must_use]
pub fn video_target(timeline: &Timeline, media: &MediaBin, t: Time) -> Option<VideoTarget> {
    let clip = timeline.top_picture_at(t)?;
    let item = media.get(clip.media)?;
    item.size_bytes?; // a missing file shows nothing
    let offset = t.saturating_sub(clip.start);
    let hdr = item
        .info
        .as_ref()
        .and_then(|i| i.video.as_ref())
        .is_some_and(|v| !matches!(v.dynamic_range(), DynamicRange::Sdr));
    Some(VideoTarget {
        clip: clip.id,
        path: item.path.clone(),
        source_seconds: seconds(clip.source_in.saturating_add(offset)),
        still: item.kind == MediaKind::Image,
        hdr,
        size: frame_size(clip.source_size.or_else(|| item.resolution())),
    })
}

/// One sound in a preview mix.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioInput {
    /// Media file.
    pub path: PathBuf,
    /// Which audio stream of the file.
    pub stream: usize,
    /// Where in the file to start, seconds.
    pub source_start: f64,
    /// How long after playback starts it begins, seconds.
    pub delay: f64,
    /// How long it plays, seconds.
    pub duration: f64,
}

/// Every audible sound from `from` onwards (up to [`AUDIO_WINDOW_SECONDS`]),
/// respecting mute and solo.
#[must_use]
pub fn audio_plan(timeline: &Timeline, media: &MediaBin, from: Time) -> Vec<AudioInput> {
    let from_s = seconds(from);
    let mut out: Vec<AudioInput> = timeline
        .clips()
        .iter()
        .filter(|c| timeline.is_audible(c.track))
        .filter(|c| timeline.track(c.track).is_some_and(|t| t.kind == TrackKind::Audio))
        .filter_map(|c| {
            let ClipSource::Audio(stream) = c.source else {
                return None;
            };
            let item = media.get(c.media).filter(|m| m.size_bytes.is_some())?;
            let start = seconds(c.start);
            let end = seconds(c.end());
            if end <= from_s || start >= from_s + AUDIO_WINDOW_SECONDS {
                return None;
            }
            let delay = (start - from_s).max(0.0);
            let skipped = (from_s - start).max(0.0);
            let duration = (end - start - skipped).min(AUDIO_WINDOW_SECONDS - delay);
            (duration > 0.001).then(|| AudioInput {
                path: item.path.clone(),
                stream,
                source_start: seconds(c.source_in) + skipped,
                delay,
                duration,
            })
        })
        .collect();
    out.sort_by(|a, b| a.delay.total_cmp(&b.delay));
    out.truncate(MAX_AUDIO_INPUTS);
    out
}

fn arg(s: impl Into<OsString>) -> OsString {
    s.into()
}

fn secs_arg(s: f64) -> OsString {
    arg(format!("{:.6}", s.max(0.0)))
}

/// The picture filter: (tone-map HDR), scale into the frame keeping its shape, RGBA.
fn video_filter(target: &VideoTarget, fps: Option<FrameRate>) -> String {
    let (w, h) = target.size;
    let mut chain = Vec::new();
    if let Some(fps) = fps {
        let r = fps.as_rational();
        chain.push(format!("fps={}/{}", r.num(), r.den()));
    }
    if target.hdr {
        chain.push(
            "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=hable:desat=0,\
             zscale=t=bt709:m=bt709:r=tv"
                .to_owned(),
        );
    }
    chain.push(format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,format=rgba"
    ));
    chain.join(",")
}

/// FFmpeg arguments that write RGBA frames of `target.size` to standard output: one
/// frame (`stream_fps` = `None`) or a continuous stream at the project's frame rate.
#[must_use]
pub fn video_args(target: &VideoTarget, stream_fps: Option<FrameRate>) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-nostdin"].map(arg).to_vec();
    if !target.still && target.source_seconds > 0.0 {
        a.extend([arg("-ss"), secs_arg(target.source_seconds)]);
    }
    a.extend([
        arg("-i"),
        target.path.clone().into_os_string(),
        arg("-map"),
        arg("0:v:0"),
    ]);
    if stream_fps.is_none() || target.still {
        a.extend([arg("-frames:v"), arg("1")]);
    }
    a.extend([
        arg("-vf"),
        arg(video_filter(target, if target.still { None } else { stream_fps })),
        arg("-f"),
        arg("rawvideo"),
        arg("-pix_fmt"),
        arg("rgba"),
        arg("pipe:1"),
    ]);
    a
}

/// FFmpeg arguments that mix `inputs` into 48 kHz stereo 32-bit float on standard
/// output. `None` when there's nothing to play.
#[must_use]
pub fn mix_args(inputs: &[AudioInput]) -> Option<Vec<OsString>> {
    if inputs.is_empty() {
        return None;
    }
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-nostdin"].map(arg).to_vec();
    let mut filter = String::new();
    for (i, input) in inputs.iter().enumerate() {
        a.extend([
            arg("-ss"),
            secs_arg(input.source_start),
            arg("-t"),
            secs_arg(input.duration),
            arg("-i"),
            input.path.clone().into_os_string(),
        ]);
        let delay_ms = (input.delay * 1000.0).round() as u64;
        let _ = write!(
            filter,
            "[{i}:a:{}]aresample={AUDIO_RATE},aformat=sample_fmts=fltp:channel_layouts=stereo,\
             adelay=delays={delay_ms}:all=1[a{i}];",
            input.stream
        );
    }
    for i in 0..inputs.len() {
        let _ = write!(filter, "[a{i}]");
    }
    let _ = write!(
        filter,
        "amix=inputs={}:normalize=0:dropout_transition=0[out]",
        inputs.len()
    );
    a.extend([
        arg("-filter_complex"),
        arg(filter),
        arg("-map"),
        arg("[out]"),
        arg("-f"),
        arg("f32le"),
        arg("-ar"),
        arg(AUDIO_RATE.to_string()),
        arg("-ac"),
        arg("2"),
        arg("pipe:1"),
    ]);
    Some(a)
}

/// `ffplay` arguments that play the mix from standard input with no window.
#[must_use]
pub fn ffplay_args() -> Vec<OsString> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nodisp",
        "-autoexit",
        "-f",
        "f32le",
        "-ar",
        "48000",
        "-ch_layout",
        "stereo",
        "-i",
        "pipe:0",
    ]
    .map(arg)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use motix_app::{AppState, MediaItem, MediaKind};

    fn fake(state: &mut AppState, name: &str, size: Option<(u32, u32)>, audio: usize, secs: i64) -> motix_app::MediaId {
        use motix_app::probe::{AudioInfo, ColorInfo, Container, MediaInfo, VideoInfo};
        // Synthetic media that "exists" (has a size) without a real file.
        state.media.insert_with(|id| MediaItem {
            id,
            path: PathBuf::from(name),
            name: name.to_owned(),
            kind: if size.is_some() {
                MediaKind::Video
            } else {
                MediaKind::Audio
            },
            size_bytes: Some(1),
            info: Some(MediaInfo {
                container: Container::Mp4,
                duration: Some(Time::from_seconds(secs).unwrap()),
                video: size.map(|(w, h)| VideoInfo {
                    codec: "H.264".into(),
                    coded_width: w,
                    coded_height: h,
                    rotation: 0,
                    frame_rate: Some(FrameRate::FPS_30),
                    variable_frame_rate: false,
                    bit_depth: Some(8),
                    color: ColorInfo::default(),
                    dolby_vision: None,
                    hdr_metadata: false,
                }),
                audio: (0..audio)
                    .map(|_| AudioInfo {
                        codec: "AAC".into(),
                        channels: 2,
                        sample_rate: 48_000,
                    })
                    .collect(),
                still: false,
            }),
            probe_note: None,
        })
    }

    #[test]
    fn frame_sizes_keep_shape_and_are_even() {
        assert_eq!(
            frame_size(Some(Resolution {
                width: 3840,
                height: 2160
            })),
            (1280, 720)
        );
        assert_eq!(
            frame_size(Some(Resolution {
                width: 1080,
                height: 1920
            })),
            (720, 1280)
        );
        assert_eq!(
            frame_size(Some(Resolution {
                width: 641,
                height: 359
            })),
            (642, 360)
        );
        assert_eq!(frame_size(None), (1280, 720));
    }

    #[test]
    fn picks_the_top_picture_and_the_right_moment() {
        let mut s = AppState::default();
        s.match_asked = true;
        let a = fake(&mut s, "a.mp4", Some((1920, 1080)), 1, 10);
        let b = fake(&mut s, "b.mp4", Some((1080, 1920)), 0, 10);
        s.add_to_timeline(a, None, None).unwrap();
        s.playhead = Time::from_seconds(2).unwrap();
        s.add_to_timeline(b, None, None).unwrap();
        let t = video_target(&s.timeline, &s.media, Time::from_seconds(1).unwrap()).unwrap();
        assert_eq!(t.path, PathBuf::from("a.mp4"));
        assert!((t.source_seconds - 1.0).abs() < 1e-9);
        let t = video_target(&s.timeline, &s.media, Time::from_seconds(3).unwrap()).unwrap();
        assert_eq!(
            t.path,
            PathBuf::from("a.mp4"),
            "the first file's video is the top track"
        );
        assert!(video_target(&s.timeline, &s.media, Time::from_seconds(30).unwrap()).is_none());
    }

    #[test]
    fn audio_plan_offsets_delays_and_respects_mute() {
        let mut s = AppState::default();
        s.match_asked = true;
        let a = fake(&mut s, "a.mp4", Some((1920, 1080)), 1, 10);
        let song = fake(&mut s, "song.wav", None, 1, 30);
        s.add_to_timeline(a, None, None).unwrap();
        s.playhead = Time::from_seconds(4).unwrap();
        s.add_to_timeline(song, None, None).unwrap();
        let plan = audio_plan(&s.timeline, &s.media, Time::from_seconds(2).unwrap());
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].path, PathBuf::from("a.mp4"));
        assert!((plan[0].source_start - 2.0).abs() < 1e-9 && plan[0].delay == 0.0);
        assert!((plan[0].duration - 8.0).abs() < 1e-9);
        assert_eq!(plan[1].path, PathBuf::from("song.wav"));
        assert!((plan[1].delay - 2.0).abs() < 1e-9 && plan[1].source_start == 0.0);
        // Mute the song's track: only the video's sound remains.
        let song_track = s.timeline.clips().iter().find(|c| c.media == song).unwrap().track;
        s.set_track_muted(song_track, true);
        assert_eq!(
            audio_plan(&s.timeline, &s.media, Time::from_seconds(2).unwrap()).len(),
            1
        );
        assert!(mix_args(&[]).is_none());
    }

    #[test]
    fn command_lines() {
        let t = VideoTarget {
            clip: ClipId(1),
            path: PathBuf::from("clip one.mov"),
            source_seconds: 1.5,
            still: false,
            hdr: true,
            size: (640, 360),
        };
        let one: Vec<String> = video_args(&t, None)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(one.windows(2).any(|w| w == ["-ss", "1.500000"]));
        assert!(one.windows(2).any(|w| w == ["-frames:v", "1"]));
        assert!(one.iter().any(|a| a.contains("tonemap") && a.contains("pad=640:360")));
        let stream: Vec<String> = video_args(&t, Some(FrameRate::FPS_30))
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(!stream.iter().any(|a| a == "-frames:v"));
        assert!(stream.iter().any(|a| a.starts_with("fps=30/1,")));
    }
}

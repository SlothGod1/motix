//! Saving and opening projects (`.motix` files), and crash recovery copies.
//!
//! A project file is UTF-8 JSON with a format name and version (ADR-032). It stores
//! the project settings, the media list (where each file is — absolute and relative to
//! the project, so a folder with the project and its media can be moved), every
//! track, clip and marker, and the playhead. Media details (size, frame rate…) are not
//! stored: files are inspected again when the project opens, so they're always current.
//!
//! Opening a file treats it as untrusted input (SECURITY §3): size and count limits,
//! names cleaned, times range-checked, ids checked for duplicates, and clips that don't
//! fit (missing track, overlapping another clip, wrong kind of track) dropped and
//! reported instead of crashing. Missing media files keep their clips and are listed.
//!
//! This JSON format is the stand-in until the collaborative document (ADR-007) lands;
//! the importer for these files will stay.

use crate::media::{MediaBin, MediaId, MediaItem};
use crate::project::{ColorOutput, FitMode, ProjectSettings, Resolution};
use crate::timeline::{
    Clip, ClipId, ClipSource, LinkId, MAX_NAME_CHARS, Marker, MarkerColor, MarkerId, Timeline, Track, TrackId,
    TrackKind,
};
use motix_core::limits::{MAX_COMPOSITION_SECONDS, MAX_IMAGE_DIMENSION};
use motix_core::{FLICKS_PER_SECOND, FrameRate, Time};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// File extension of MOTIX projects.
pub const EXTENSION: &str = "motix";
/// The `format` field of every project file.
pub const FORMAT: &str = "motix-project";
/// Newest file version this build writes and reads.
pub const VERSION: u32 = 1;
/// Largest project file accepted.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Most tracks in a project.
pub const MAX_TRACKS: usize = 1_000;
/// Most clips in a project.
pub const MAX_CLIPS: usize = 200_000;
/// Most markers in a project.
pub const MAX_MARKERS: usize = 20_000;
/// Most media files in a project.
pub const MAX_MEDIA: usize = 100_000;
/// Longest stored path, in bytes.
const MAX_PATH_BYTES: usize = 4096;

/// Why a project couldn't be opened, in plain language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// The file couldn't be read.
    Read(String),
    /// It's not a MOTIX project.
    NotAProject,
    /// It was saved by a newer MOTIX.
    TooNew(u32),
    /// It's damaged or exceeds a limit.
    Damaged(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(why) => write!(f, "Couldn't read the project ({why})."),
            Self::NotAProject => f.write_str("That file isn't a MOTIX project."),
            Self::TooNew(v) => write!(
                f,
                "This project was saved by a newer MOTIX (file version {v}). Update MOTIX to open it."
            ),
            Self::Damaged(why) => write!(f, "The project file is damaged ({why})."),
        }
    }
}

impl std::error::Error for OpenError {}

// ----- the file format (version 1) -----

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileV1 {
    format: String,
    version: u32,
    #[serde(default)]
    saved_by: String,
    /// For crash-recovery copies: the project the unsaved changes belong to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recovery_of: Option<String>,
    settings: SettingsV1,
    #[serde(default)]
    playhead: i64,
    media: Vec<MediaV1>,
    tracks: Vec<TrackV1>,
    clips: Vec<ClipV1>,
    markers: Vec<MarkerV1>,
    next_media_id: u64,
    next_timeline_id: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsV1 {
    width: u32,
    height: u32,
    fps_num: i64,
    fps_den: i64,
    color: ColorV1,
    bit_depth: u8,
    default_fit: FitV1,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ColorV1 {
    Sdr,
    Hdr10,
    Hlg,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum FitV1 {
    Fit,
    Fill,
    Stretch,
    Original,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaV1 {
    id: u64,
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relative: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum KindV1 {
    Video,
    Audio,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackV1 {
    id: u64,
    kind: KindV1,
    name: String,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    solo: bool,
    #[serde(default)]
    locked: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipV1 {
    id: u64,
    track: u64,
    media: u64,
    /// `None` for the picture, `Some(n)` for audio stream `n`.
    audio_stream: Option<usize>,
    name: String,
    start: i64,
    duration: i64,
    source_in: i64,
    source_duration: Option<i64>,
    link: Option<u64>,
    fit: FitV1,
    source_size: Option<(u32, u32)>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum MarkerColorV1 {
    Blue,
    Cyan,
    Green,
    Yellow,
    Red,
    Pink,
    Purple,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkerV1 {
    id: u64,
    time: i64,
    name: String,
    color: MarkerColorV1,
}

const fn color_out(c: ColorOutput) -> ColorV1 {
    match c {
        ColorOutput::Sdr => ColorV1::Sdr,
        ColorOutput::Hdr10 => ColorV1::Hdr10,
        ColorOutput::Hlg => ColorV1::Hlg,
    }
}

const fn color_in(c: ColorV1) -> ColorOutput {
    match c {
        ColorV1::Sdr => ColorOutput::Sdr,
        ColorV1::Hdr10 => ColorOutput::Hdr10,
        ColorV1::Hlg => ColorOutput::Hlg,
    }
}

const fn fit_out(f: FitMode) -> FitV1 {
    match f {
        FitMode::Fit => FitV1::Fit,
        FitMode::Fill => FitV1::Fill,
        FitMode::Stretch => FitV1::Stretch,
        FitMode::Original => FitV1::Original,
    }
}

const fn fit_in(f: FitV1) -> FitMode {
    match f {
        FitV1::Fit => FitMode::Fit,
        FitV1::Fill => FitMode::Fill,
        FitV1::Stretch => FitMode::Stretch,
        FitV1::Original => FitMode::Original,
    }
}

const fn marker_out(c: MarkerColor) -> MarkerColorV1 {
    match c {
        MarkerColor::Blue => MarkerColorV1::Blue,
        MarkerColor::Cyan => MarkerColorV1::Cyan,
        MarkerColor::Green => MarkerColorV1::Green,
        MarkerColor::Yellow => MarkerColorV1::Yellow,
        MarkerColor::Red => MarkerColorV1::Red,
        MarkerColor::Pink => MarkerColorV1::Pink,
        MarkerColor::Purple => MarkerColorV1::Purple,
    }
}

const fn marker_in(c: MarkerColorV1) -> MarkerColor {
    match c {
        MarkerColorV1::Blue => MarkerColor::Blue,
        MarkerColorV1::Cyan => MarkerColor::Cyan,
        MarkerColorV1::Green => MarkerColor::Green,
        MarkerColorV1::Yellow => MarkerColor::Yellow,
        MarkerColorV1::Red => MarkerColor::Red,
        MarkerColorV1::Pink => MarkerColor::Pink,
        MarkerColorV1::Purple => MarkerColor::Purple,
    }
}

// ----- saving -----

/// What gets saved.
pub struct Contents<'a> {
    /// Media bin.
    pub media: &'a MediaBin,
    /// Tracks, clips, markers.
    pub timeline: &'a Timeline,
    /// Project settings.
    pub project: &'a ProjectSettings,
    /// Playhead.
    pub playhead: Time,
}

/// `path` relative to `base`, when `path` is inside it.
fn relative_to(path: &Path, base: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    rel.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| rel.to_string_lossy().replace('\\', "/"))
}

/// Encodes a project for saving to `project_path` (used to store media paths
/// relative to the project). `recovery_of` marks a crash-recovery copy.
#[must_use]
pub fn encode(contents: &Contents<'_>, project_path: &Path, recovery_of: Option<&Path>) -> Vec<u8> {
    let base = project_path.parent().unwrap_or(Path::new(""));
    let p = contents.project;
    let rate = p.frame_rate.as_rational();
    let file = FileV1 {
        format: FORMAT.to_owned(),
        version: VERSION,
        saved_by: format!("MOTIX {}", env!("CARGO_PKG_VERSION")),
        recovery_of: recovery_of.map(|r| r.display().to_string()),
        settings: SettingsV1 {
            width: p.resolution.width,
            height: p.resolution.height,
            fps_num: rate.num(),
            fps_den: rate.den(),
            color: color_out(p.color),
            bit_depth: p.bit_depth,
            default_fit: fit_out(p.default_fit),
        },
        playhead: contents.playhead.flicks(),
        media: contents
            .media
            .items()
            .iter()
            .map(|m| MediaV1 {
                id: m.id.0,
                path: m.path.display().to_string(),
                relative: relative_to(&m.path, base),
            })
            .collect(),
        tracks: contents
            .timeline
            .tracks()
            .iter()
            .map(|t| TrackV1 {
                id: t.id.0,
                kind: match t.kind {
                    TrackKind::Video => KindV1::Video,
                    TrackKind::Audio => KindV1::Audio,
                },
                name: t.name.clone(),
                muted: t.muted,
                solo: t.solo,
                locked: t.locked,
            })
            .collect(),
        clips: contents
            .timeline
            .clips()
            .iter()
            .map(|c| ClipV1 {
                id: c.id.0,
                track: c.track.0,
                media: c.media.0,
                audio_stream: match c.source {
                    ClipSource::Picture => None,
                    ClipSource::Audio(n) => Some(n),
                },
                name: c.name.clone(),
                start: c.start.flicks(),
                duration: c.duration.flicks(),
                source_in: c.source_in.flicks(),
                source_duration: c.source_duration.map(Time::flicks),
                link: c.link.map(|l| l.0),
                fit: fit_out(c.fit),
                source_size: c.source_size.map(|r| (r.width, r.height)),
            })
            .collect(),
        markers: contents
            .timeline
            .markers()
            .iter()
            .map(|m| MarkerV1 {
                id: m.id.0,
                time: m.time.flicks(),
                name: m.name.clone(),
                color: marker_out(m.color),
            })
            .collect(),
        next_media_id: contents.media.next_id(),
        next_timeline_id: contents.timeline.next_id(),
    };
    let mut out = serde_json::to_vec_pretty(&file).unwrap_or_default();
    out.push(b'\n');
    out
}

/// Writes `bytes` to `path` safely: to a temporary file next to it first, then
/// renamed over the old file, so a crash or full disk never leaves a half-written
/// project. The previous version is kept as `<name>.motix.bak`.
///
/// # Errors
/// File-system errors (the old file is untouched).
pub fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.saving-{}", std::process::id()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        if path.exists() {
            let _ = fs::copy(path, dir.join(format!("{name}.bak")));
        }
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

// ----- opening -----

/// Just enough of a file to tell what it is.
#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
}

/// A project read from a file.
#[derive(Debug)]
pub struct Loaded {
    /// Media bin (files inspected again).
    pub media: MediaBin,
    /// Tracks, clips, markers.
    pub timeline: Timeline,
    /// Settings.
    pub project: ProjectSettings,
    /// Playhead.
    pub playhead: Time,
    /// Names of media files that weren't found (their clips are kept).
    pub missing: Vec<String>,
    /// How many damaged clips or markers were left out.
    pub dropped: usize,
    /// For a crash-recovery copy: the project it belongs to.
    pub recovery_of: Option<PathBuf>,
}

fn damaged(why: &str) -> OpenError {
    OpenError::Damaged(why.to_owned())
}

fn clean(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    if s.is_empty() { "Untitled".to_owned() } else { s }
}

fn time(flicks: i64) -> Option<Time> {
    let max = MAX_COMPOSITION_SECONDS * FLICKS_PER_SECOND;
    (0..=max).contains(&flicks).then(|| Time::from_flicks(flicks))
}

/// Reads a project file from disk.
///
/// # Errors
/// See [`OpenError`].
pub fn open(path: &Path) -> Result<Loaded, OpenError> {
    let meta = fs::metadata(path).map_err(|e| OpenError::Read(e.to_string()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(damaged("it is far larger than any project could be"));
    }
    let bytes = fs::read(path).map_err(|e| OpenError::Read(e.to_string()))?;
    decode(&bytes, path)
}

/// Decodes a project that was stored at `project_path`, inspecting its media again.
///
/// # Errors
/// See [`OpenError`].
#[allow(clippy::too_many_lines)] // one linear validation pass reads best in one place
pub fn decode(bytes: &[u8], project_path: &Path) -> Result<Loaded, OpenError> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(damaged("it is far larger than any project could be"));
    }
    // Check the header first so other files get a clear message.
    let header: Header = serde_json::from_slice(bytes).map_err(|_| OpenError::NotAProject)?;
    if header.format != FORMAT {
        return Err(OpenError::NotAProject);
    }
    if header.version > VERSION {
        return Err(OpenError::TooNew(header.version));
    }
    let file: FileV1 = serde_json::from_slice(bytes).map_err(|e| OpenError::Damaged(e.to_string()))?;
    if file.media.len() > MAX_MEDIA
        || file.tracks.len() > MAX_TRACKS
        || file.clips.len() > MAX_CLIPS
        || file.markers.len() > MAX_MARKERS
    {
        return Err(damaged("it has more items than MOTIX allows"));
    }

    // Settings.
    let s = &file.settings;
    let resolution = Resolution::new(s.width, s.height).map_err(|e| OpenError::Damaged(e.0))?;
    let frame_rate = FrameRate::from_fraction(s.fps_num, s.fps_den).map_err(|_| damaged("bad frame rate"))?;
    if !matches!(s.bit_depth, 8 | 10 | 12) {
        return Err(damaged("bad bit depth"));
    }
    let project = ProjectSettings {
        resolution,
        frame_rate,
        color: color_in(s.color),
        bit_depth: s.bit_depth,
        default_fit: fit_in(s.default_fit),
    };

    // Media: find each file (where it was, or next to the project if the folder moved).
    let base = project_path.parent().unwrap_or(Path::new(""));
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut missing = Vec::new();
    for m in &file.media {
        if !seen.insert(m.id) || m.id >= file.next_media_id {
            return Err(damaged("two media files share an id"));
        }
        if m.path.len() > MAX_PATH_BYTES || m.relative.as_ref().is_some_and(|r| r.len() > MAX_PATH_BYTES) {
            return Err(damaged("a file path is too long"));
        }
        let stored = PathBuf::from(&m.path);
        let beside = m
            .relative
            .as_deref()
            .map(Path::new)
            .filter(|r| r.components().all(|c| matches!(c, Component::Normal(_))))
            .map(|r| base.join(r));
        let path = if stored.is_file() {
            stored
        } else if let Some(b) = beside.filter(|b| b.is_file()) {
            b
        } else {
            stored
        };
        let item = MediaItem::from_path(MediaId(m.id), path);
        if item.size_bytes.is_none() {
            missing.push(item.name.clone());
        }
        items.push(item);
    }
    let media = MediaBin::from_parts(items, file.next_media_id);

    // Tracks.
    let mut dropped = 0;
    let mut track_ids = HashSet::new();
    let mut tracks = Vec::new();
    for t in &file.tracks {
        if !track_ids.insert(t.id) || t.id > file.next_timeline_id {
            return Err(damaged("two tracks share an id"));
        }
        tracks.push(Track {
            id: TrackId(t.id),
            kind: match t.kind {
                KindV1::Video => TrackKind::Video,
                KindV1::Audio => TrackKind::Audio,
            },
            name: clean(&t.name),
            muted: t.muted,
            solo: t.solo,
            locked: t.locked,
        });
    }

    // Clips: keep the ones that make sense; drop (and count) the rest.
    let mut clip_ids = HashSet::new();
    let mut clips: Vec<Clip> = Vec::new();
    for c in &file.clips {
        if !clip_ids.insert(c.id) || c.id > file.next_timeline_id {
            return Err(damaged("two clips share an id"));
        }
        let Some(track) = tracks.iter().find(|t| t.id.0 == c.track) else {
            dropped += 1;
            continue;
        };
        let source = c.audio_stream.map_or(ClipSource::Picture, ClipSource::Audio);
        let fits_track = matches!(
            (track.kind, source),
            (TrackKind::Video, ClipSource::Picture) | (TrackKind::Audio, ClipSource::Audio(_))
        );
        let times = (
            time(c.start),
            time(c.duration).filter(|d| *d > Time::ZERO),
            time(c.source_in),
            c.source_duration.map(time),
        );
        let (Some(start), Some(duration), Some(source_in), source_duration) = times else {
            dropped += 1;
            continue;
        };
        let source_duration = match source_duration {
            None => None,
            Some(Some(d)) => Some(d),
            Some(None) => {
                dropped += 1;
                continue;
            }
        };
        let Some(end) = time(start.flicks().saturating_add(duration.flicks())) else {
            dropped += 1;
            continue;
        };
        let source_size = match c.source_size {
            None => None,
            Some((w, h)) if (1..=MAX_IMAGE_DIMENSION).contains(&w) && (1..=MAX_IMAGE_DIMENSION).contains(&h) => {
                Some(Resolution { width: w, height: h })
            }
            Some(_) => {
                dropped += 1;
                continue;
            }
        };
        let overlaps = clips
            .iter()
            .any(|o| o.track == track.id && o.start < end && start < o.end());
        if !fits_track || overlaps || media.get(MediaId(c.media)).is_none() {
            dropped += 1;
            continue;
        }
        clips.push(Clip {
            id: ClipId(c.id),
            track: track.id,
            media: MediaId(c.media),
            source,
            name: clean(&c.name),
            start,
            duration,
            source_in,
            source_duration,
            link: c.link.map(LinkId),
            fit: fit_in(c.fit),
            source_size,
        });
    }

    let mut marker_ids = HashSet::new();
    let mut markers = Vec::new();
    for m in &file.markers {
        if !marker_ids.insert(m.id) || m.id > file.next_timeline_id {
            return Err(damaged("two markers share an id"));
        }
        let Some(t) = time(m.time) else {
            dropped += 1;
            continue;
        };
        markers.push(Marker {
            id: MarkerId(m.id),
            time: t,
            name: clean(&m.name),
            color: marker_in(m.color),
        });
    }
    markers.sort_by_key(|m| m.time);
    let timeline = Timeline::from_parts(tracks, clips, markers, file.next_timeline_id);
    Ok(Loaded {
        media,
        timeline,
        project,
        playhead: time(file.playhead).unwrap_or(Time::ZERO),
        missing,
        dropped,
        recovery_of: file.recovery_of.map(PathBuf::from),
    })
}

// ----- crash recovery -----

/// How often unsaved changes are copied to the recovery folder.
pub const RECOVERY_INTERVAL_SECONDS: u64 = 60;
/// A recovery copy that hasn't been touched for this long belongs to a MOTIX that
/// didn't close properly (a running MOTIX touches its copy every minute).
pub const RECOVERY_STALE_SECONDS: u64 = 3 * 60;

/// A crash-recovery copy found on start-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryCopy {
    /// The copy.
    pub path: PathBuf,
    /// When it was last written.
    pub saved: std::time::SystemTime,
}

/// Recovery copies left behind by a MOTIX that didn't close properly, newest first.
#[must_use]
pub fn find_recovery(dir: &Path) -> Vec<RecoveryCopy> {
    let now = std::time::SystemTime::now();
    let mut out: Vec<RecoveryCopy> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == EXTENSION))
        .filter_map(|e| {
            let saved = e.metadata().ok()?.modified().ok()?;
            let age = now.duration_since(saved).unwrap_or_default();
            (age.as_secs() >= RECOVERY_STALE_SECONDS).then(|| RecoveryCopy { path: e.path(), saved })
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.saved));
    out
}

/// This MOTIX's own recovery copy in `dir`.
#[must_use]
pub fn recovery_path(dir: &Path) -> PathBuf {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    dir.join(format!("recovery-{}-{started}.{EXTENSION}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("motix-doc-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample(dir: &Path) -> AppState {
        let media = dir.join("media");
        fs::create_dir_all(&media).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../motix-probe/tests/fixtures");
        for f in ["h264_aac_2997.mp4", "stereo_48k.wav"] {
            fs::copy(fixtures.join(f), media.join(f)).unwrap();
        }
        let mut s = AppState::default();
        s.match_asked = true;
        s.import(vec![media.join("h264_aac_2997.mp4"), media.join("stereo_48k.wav")]);
        let v = s.media.items()[0].id;
        let a = s.media.items()[1].id;
        s.add_to_timeline(v, None, None).unwrap();
        s.add_to_timeline(a, None, None).unwrap();
        s.playhead = Time::from_seconds(1).unwrap();
        s.perform(crate::Action::AddMarker);
        s.set_resolution(1080, 1080).unwrap();
        s
    }

    fn contents(s: &AppState) -> Contents<'_> {
        Contents {
            media: &s.media,
            timeline: &s.timeline,
            project: &s.project,
            playhead: s.playhead,
        }
    }

    #[test]
    fn round_trip_keeps_everything() {
        let dir = temp("round");
        let s = sample(&dir);
        let path = dir.join("My edit.motix");
        write_atomically(&path, &encode(&contents(&s), &path, None)).unwrap();
        let loaded = open(&path).unwrap();
        assert_eq!(loaded.timeline, s.timeline);
        assert_eq!(loaded.project, s.project);
        assert_eq!(loaded.playhead, s.playhead);
        assert_eq!(loaded.media.items(), s.media.items());
        assert!(loaded.missing.is_empty());
        assert_eq!(loaded.dropped, 0);
        // Saving again keeps the previous version as a backup.
        write_atomically(&path, &encode(&contents(&s), &path, None)).unwrap();
        assert!(dir.join("My edit.motix.bak").is_file());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_moved_project_folder_still_finds_its_media() {
        let dir = temp("moved");
        let s = sample(&dir);
        let path = dir.join("edit.motix");
        let bytes = encode(&contents(&s), &path, None);
        // Move the whole folder somewhere else.
        let moved = temp("moved-to");
        fs::rename(dir.join("media"), moved.join("media")).unwrap();
        let loaded = decode(&bytes, &moved.join("edit.motix")).unwrap();
        assert!(loaded.missing.is_empty(), "found via the relative path");
        assert!(loaded.media.items()[0].path.starts_with(&moved));
        // And with the media gone, clips stay and the files are listed as missing.
        let _ = fs::remove_dir_all(moved.join("media"));
        let loaded = decode(&bytes, &moved.join("edit.motix")).unwrap();
        assert_eq!(loaded.missing, ["h264_aac_2997.mp4", "stereo_48k.wav"]);
        assert_eq!(loaded.timeline.clips().len(), s.timeline.clips().len());
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(moved);
    }

    #[test]
    fn hostile_or_foreign_files_are_refused_cleanly() {
        let p = Path::new("x.motix");
        assert_eq!(decode(b"not json", p).unwrap_err(), OpenError::NotAProject);
        assert_eq!(
            decode(br#"{"format":"something-else","version":1}"#, p).unwrap_err(),
            OpenError::NotAProject
        );
        assert_eq!(
            decode(br#"{"format":"motix-project","version":99}"#, p).unwrap_err(),
            OpenError::TooNew(99)
        );
        assert!(matches!(
            decode(br#"{"format":"motix-project","version":1}"#, p),
            Err(OpenError::Damaged(_))
        ));

        let dir = temp("hostile");
        let s = sample(&dir);
        let path = dir.join("e.motix");
        let good: serde_json::Value = serde_json::from_slice(&encode(&contents(&s), &path, None)).unwrap();
        let tweak = |f: &dyn Fn(&mut serde_json::Value)| {
            let mut v = good.clone();
            f(&mut v);
            decode(&serde_json::to_vec(&v).unwrap(), &path)
        };
        // Overlapping, negative and wrong-track clips are dropped, not fatal.
        let r = tweak(&|v| {
            let first = v["clips"][0].clone();
            let mut copy = first.clone();
            copy["id"] = 999.into();
            v["clips"].as_array_mut().unwrap().push(copy);
            v["clips"][1]["start"] = (-5).into();
            v["next_timeline_id"] = 1000.into();
        })
        .unwrap();
        assert_eq!(r.dropped, 2);
        // Duplicate ids and absurd sizes are refused.
        assert!(
            tweak(&|v| {
                let t = v["tracks"][0].clone();
                v["tracks"].as_array_mut().unwrap().push(t);
            })
            .is_err()
        );
        assert!(tweak(&|v| v["settings"]["width"] = 99_999.into()).is_err());
        assert!(tweak(&|v| v["settings"]["fps_den"] = 0.into()).is_err());
        // Control characters and huge names are cleaned.
        let r = tweak(&|v| v["tracks"][0]["name"] = format!("\u{7}{}", "x".repeat(5000)).into()).unwrap();
        assert_eq!(r.timeline.tracks()[0].name.chars().count(), MAX_NAME_CHARS);
        // Unknown fields mean a different (or tampered) format.
        assert!(tweak(&|v| v["surprise"] = 1.into()).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stale_recovery_copies_are_found() {
        let dir = temp("recovery");
        let mine = recovery_path(&dir);
        fs::write(&mine, b"{}").unwrap();
        assert!(
            find_recovery(&dir).is_empty(),
            "a fresh copy belongs to a running MOTIX"
        );
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(RECOVERY_STALE_SECONDS + 5);
        fs::File::options()
            .write(true)
            .open(&mine)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(find_recovery(&dir).len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saving_tracks_unsaved_changes_and_recovery_restores_them() {
        use crate::{Action, AfterSave, Outcome};
        let dir = temp("state");
        let mut s = sample(&dir);
        assert!(s.is_dirty());
        assert_eq!(s.project_name(), "Untitled project");
        assert_eq!(s.perform(Action::SaveProject), Outcome::PickSavePath { then: None });
        assert_eq!(
            s.perform(Action::Quit),
            Outcome::ConfirmUnsaved { then: AfterSave::Quit }
        );
        s.save_to(&dir.join("Beach day")).unwrap();
        assert_eq!(
            s.file.as_deref(),
            Some(dir.join("Beach day.motix").as_path()),
            "adds .motix"
        );
        assert!(!s.is_dirty());
        assert_eq!(s.project_name(), "Beach day");
        assert_eq!(s.perform(Action::Quit), Outcome::Quit);

        // An edit after saving, then a crash: the recovery copy brings it back.
        s.playhead = Time::from_seconds(3).unwrap();
        s.perform(Action::AddMarker);
        assert!(s.is_dirty());
        assert_eq!(
            s.perform(Action::SaveProject),
            Outcome::Done,
            "saves straight to its file"
        );
        assert!(!s.is_dirty());
        s.playhead = Time::from_seconds(2).unwrap();
        s.perform(Action::AddMarker);
        let copy = recovery_path(&dir);
        fs::write(&copy, s.recovery_bytes(&copy)).unwrap();

        let mut after_crash = crate::AppState::default();
        after_crash.restore_recovery(&copy).unwrap();
        assert_eq!(after_crash.timeline.markers().len(), 3);
        assert_eq!(after_crash.file, s.file, "still belongs to Beach day.motix");
        assert!(after_crash.is_dirty());

        let mut reopened = crate::AppState::default();
        reopened.open_file(&dir.join("Beach day.motix")).unwrap();
        assert_eq!(reopened.timeline.markers().len(), 2, "the saved version");
        assert!(!reopened.is_dirty());
        assert!(reopened.open_file(&dir.join("nope.motix")).is_err());
        assert_eq!(reopened.timeline.markers().len(), 2, "a failed open keeps the project");

        reopened.new_project();
        assert!(reopened.timeline.is_empty() && reopened.file.is_none() && !reopened.is_dirty());
        let _ = fs::remove_dir_all(dir);
    }
}

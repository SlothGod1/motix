//! The media bin: files the user has brought into the project.
//!
//! Each file is inspected on import with `motix-probe` (size, frame rate, duration,
//! audio streams, HDR). Files the probe can't read yet are still accepted if their
//! extension is a known media type; decoding them arrives with the media engine.

use crate::project::Resolution;
use motix_core::{FrameRate, Time};
use motix_probe::{MediaInfo, probe_path};
use std::path::{Path, PathBuf};

/// Broad category of a media file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    /// Video (may include audio).
    Video,
    /// Audio only.
    Audio,
    /// Still image.
    Image,
    /// Not a format MOTIX recognizes.
    Unsupported,
}

impl MediaKind {
    /// Guess from a file extension (case-insensitive).
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi" | "mts" | "m2ts" | "mxf" | "hevc" | "3gp" => Self::Video,
            "wav" | "mp3" | "aac" | "m4a" | "flac" | "ogg" | "opus" | "aif" | "aiff" => Self::Audio,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "tif" | "tiff" | "exr" | "heic" | "heif" | "bmp" => Self::Image,
            _ => Self::Unsupported,
        }
    }

    /// From what the probe found, falling back to the extension.
    #[must_use]
    pub fn from_info(info: &MediaInfo) -> Self {
        if info.still {
            Self::Image
        } else if info.video.is_some() {
            Self::Video
        } else if info.audio.is_empty() {
            Self::Unsupported
        } else {
            Self::Audio
        }
    }

    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Image => "Image",
            Self::Unsupported => "Unsupported",
        }
    }
}

/// Stable identifier of a media item within a project (never reused).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MediaId(pub u64);

/// How long a still image lasts when placed on the timeline.
pub const STILL_DURATION_SECONDS: i64 = 5;
/// Length used when a file's duration couldn't be read.
pub const UNKNOWN_DURATION_SECONDS: i64 = 10;

/// One file in the media bin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaItem {
    /// Stable identifier.
    pub id: MediaId,
    /// Where the file is.
    pub path: PathBuf,
    /// File name shown to the user.
    pub name: String,
    /// Kind of media.
    pub kind: MediaKind,
    /// Size on disk, if known.
    pub size_bytes: Option<u64>,
    /// What the probe found, if it could read the file.
    pub info: Option<MediaInfo>,
    /// Why the file couldn't be inspected, in plain language.
    pub probe_note: Option<String>,
}

impl MediaItem {
    /// Builds an item from a path: reads its size and inspects its contents.
    #[must_use]
    pub fn from_path(id: MediaId, path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let size_bytes = std::fs::metadata(&path)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .map(|m| m.len());
        let guessed = MediaKind::from_path(&path);
        let (info, probe_note) = if size_bytes.is_some() {
            match probe_path(&path) {
                Ok(info) => (Some(info), None),
                Err(e) => (None, Some(format!("Details unavailable: {e}."))),
            }
        } else {
            (None, Some("File not found.".to_owned()))
        };
        let kind = match &info {
            Some(i) => match MediaKind::from_info(i) {
                MediaKind::Unsupported => guessed,
                k => k,
            },
            None => guessed,
        };
        Self {
            id,
            path,
            name,
            kind,
            size_bytes,
            info,
            probe_note,
        }
    }

    /// Human-readable size, e.g. `"12.4 MB"`.
    #[must_use]
    pub fn size_label(&self) -> String {
        match self.size_bytes {
            None => "—".to_owned(),
            Some(b) => human_size(b),
        }
    }

    /// Picture size as displayed (after phone rotation).
    #[must_use]
    pub fn resolution(&self) -> Option<Resolution> {
        let v = self.info.as_ref()?.video.as_ref()?;
        Some(Resolution {
            width: v.display_width(),
            height: v.display_height(),
        })
    }

    /// Frame rate, for video.
    #[must_use]
    pub fn frame_rate(&self) -> Option<FrameRate> {
        let info = self.info.as_ref()?;
        if info.still {
            return None;
        }
        info.video.as_ref()?.frame_rate
    }

    /// `true` if the file has a picture (video or still).
    #[must_use]
    pub fn has_picture(&self) -> bool {
        match &self.info {
            Some(i) => i.video.is_some(),
            None => matches!(self.kind, MediaKind::Video | MediaKind::Image),
        }
    }

    /// Number of audio streams. Files that couldn't be inspected: an audio file
    /// counts as one; for videos it's unknown, so none are assumed.
    #[must_use]
    pub fn audio_streams(&self) -> usize {
        match &self.info {
            Some(i) => i.audio.len(),
            None => usize::from(self.kind == MediaKind::Audio),
        }
    }

    /// How long the item lasts when placed on the timeline.
    #[must_use]
    pub fn timeline_duration(&self) -> Time {
        let fallback = |s| Time::from_seconds(s).unwrap_or(Time::SECOND);
        if self.kind == MediaKind::Image {
            return fallback(STILL_DURATION_SECONDS);
        }
        self.info
            .as_ref()
            .and_then(|i| i.duration)
            .filter(|d| *d > Time::ZERO)
            .unwrap_or_else(|| fallback(UNKNOWN_DURATION_SECONDS))
    }

    /// One-line technical summary, e.g. `"3840×2160 · 59.94 fps · HEVC 10-bit · HDR10"`.
    #[must_use]
    pub fn summary(&self) -> String {
        let Some(info) = &self.info else {
            return self.kind.label().to_owned();
        };
        let mut parts = Vec::new();
        if let Some(v) = &info.video {
            parts.push(format!("{}×{}", v.display_width(), v.display_height()));
            if let Some(rate) = v.frame_rate.filter(|_| !info.still) {
                parts.push(format!("{} fps", rate.short_label()));
            }
            match v.bit_depth {
                Some(d) if d > 8 => parts.push(format!("{} {d}-bit", v.codec)),
                _ => parts.push(v.codec.clone()),
            }
            if !info.still {
                parts.push(v.dynamic_range().label());
            }
        }
        if !info.audio.is_empty() {
            parts.push(match info.audio.len() {
                1 => "1 audio stream".to_owned(),
                n => format!("{n} audio streams"),
            });
        }
        parts.join(" · ")
    }
}

/// Formats a byte count with decimal units.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Result of adding files to the bin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddReport {
    /// Newly added, in order.
    pub added: Vec<MediaId>,
    /// Already in the bin.
    pub duplicates: usize,
    /// Not a recognized media format (not added).
    pub unsupported: usize,
}

/// The project's media bin.
#[derive(Clone, Debug, Default)]
pub struct MediaBin {
    items: Vec<MediaItem>,
    next_id: u64,
}

impl MediaBin {
    /// Items in the order they were added.
    #[must_use]
    pub fn items(&self) -> &[MediaItem] {
        &self.items
    }

    /// Looks up an item.
    #[must_use]
    pub fn get(&self, id: MediaId) -> Option<&MediaItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Adds files, skipping duplicates and unsupported formats.
    pub fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> AddReport {
        let mut report = AddReport::default();
        for path in paths {
            if self.items.iter().any(|i| i.path == path) {
                report.duplicates += 1;
                continue;
            }
            let id = MediaId(self.next_id);
            let item = MediaItem::from_path(id, path);
            if item.kind == MediaKind::Unsupported {
                report.unsupported += 1;
            } else {
                self.next_id += 1;
                report.added.push(id);
                self.items.push(item);
            }
        }
        report
    }

    /// Adds an item that was already inspected (for example on a background thread,
    /// or synthetic media in tests). `make` receives the new item's id.
    pub fn insert_with(&mut self, make: impl FnOnce(MediaId) -> MediaItem) -> MediaId {
        let id = MediaId(self.next_id);
        self.next_id += 1;
        let mut item = make(id);
        item.id = id;
        self.items.push(item);
        id
    }

    /// Rebuilds a bin from a saved project.
    #[must_use]
    pub(crate) fn from_parts(items: Vec<MediaItem>, next_id: u64) -> Self {
        let next_id = items.iter().map(|i| i.id.0 + 1).max().unwrap_or(0).max(next_id);
        Self { items, next_id }
    }

    /// The id the next item will get (saved so ids are never reused).
    #[must_use]
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Removes an item, if it exists.
    pub fn remove(&mut self, id: MediaId) -> Option<MediaItem> {
        let index = self.items.iter().position(|i| i.id == id)?;
        Some(self.items.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_from_extensions() {
        assert_eq!(MediaKind::from_path(Path::new("clip.MP4")), MediaKind::Video);
        assert_eq!(MediaKind::from_path(Path::new("iphone.mov")), MediaKind::Video);
        assert_eq!(MediaKind::from_path(Path::new("song.m4a")), MediaKind::Audio);
        assert_eq!(MediaKind::from_path(Path::new("photo.HEIC")), MediaKind::Image);
        assert_eq!(MediaKind::from_path(Path::new("notes.txt")), MediaKind::Unsupported);
        assert_eq!(MediaKind::from_path(Path::new("no_extension")), MediaKind::Unsupported);
    }

    #[test]
    fn bin_dedupes_and_rejects_unsupported() {
        let mut bin = MediaBin::default();
        let r = bin.add_paths(["a.mp4", "b.wav", "a.mp4", "c.txt"].map(PathBuf::from));
        assert_eq!(r.added, vec![MediaId(0), MediaId(1)]);
        assert_eq!((r.duplicates, r.unsupported), (1, 1));
        assert_eq!(bin.items().len(), 2);
        let a = &bin.items()[0];
        assert_eq!(a.name, "a.mp4");
        assert_eq!(a.size_bytes, None, "missing file has no size");
        assert_eq!(a.probe_note.as_deref(), Some("File not found."));
        assert_eq!(a.audio_streams(), 0);
        assert_eq!(bin.items()[1].audio_streams(), 1);
        assert_eq!(
            a.timeline_duration(),
            Time::from_seconds(UNKNOWN_DURATION_SECONDS).unwrap()
        );
        assert!(bin.remove(MediaId(9)).is_none());
        assert_eq!(bin.remove(MediaId(0)).map(|i| i.name), Some("a.mp4".to_owned()));
        // Ids are never reused.
        assert_eq!(bin.add_paths([PathBuf::from("d.mp4")]).added, vec![MediaId(2)]);
    }

    #[test]
    fn sizes() {
        assert_eq!(human_size(999), "999 B");
        assert_eq!(human_size(12_400_000), "12.4 MB");
        assert_eq!(human_size(3_000_000_000), "3.0 GB");
    }
}

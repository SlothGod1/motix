//! The media bin: files the user has brought into the project.
//!
//! In M1 the kind of file is guessed from its extension. Real inspection of the
//! file's contents (codec, duration, color) happens in the sandboxed media worker
//! (ARCHITECTURE §6) and replaces this guess.

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

/// One file in the media bin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaItem {
    /// Where the file is.
    pub path: PathBuf,
    /// File name shown to the user.
    pub name: String,
    /// Guessed kind.
    pub kind: MediaKind,
    /// Size on disk, if known.
    pub size_bytes: Option<u64>,
}

impl MediaItem {
    /// Builds an item from a path, reading the size from disk if possible.
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let kind = MediaKind::from_path(&path);
        let size_bytes = std::fs::metadata(&path)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .map(|m| m.len());
        Self {
            path,
            name,
            kind,
            size_bytes,
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddReport {
    /// Newly added.
    pub added: usize,
    /// Already in the bin.
    pub duplicates: usize,
    /// Not a recognized media format (not added).
    pub unsupported: usize,
}

/// The project's media bin.
#[derive(Clone, Debug, Default)]
pub struct MediaBin {
    items: Vec<MediaItem>,
}

impl MediaBin {
    /// Items in the order they were added.
    #[must_use]
    pub fn items(&self) -> &[MediaItem] {
        &self.items
    }

    /// Adds files, skipping duplicates and unsupported formats.
    pub fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> AddReport {
        let mut report = AddReport::default();
        for path in paths {
            let item = MediaItem::from_path(path);
            if item.kind == MediaKind::Unsupported {
                report.unsupported += 1;
            } else if self.items.iter().any(|i| i.path == item.path) {
                report.duplicates += 1;
            } else {
                self.items.push(item);
                report.added += 1;
            }
        }
        report
    }

    /// Removes the item at `index`, if it exists.
    pub fn remove(&mut self, index: usize) -> Option<MediaItem> {
        (index < self.items.len()).then(|| self.items.remove(index))
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
        assert_eq!(
            r,
            AddReport {
                added: 2,
                duplicates: 1,
                unsupported: 1
            }
        );
        assert_eq!(bin.items().len(), 2);
        assert_eq!(bin.items()[0].name, "a.mp4");
        assert_eq!(bin.items()[0].size_bytes, None, "missing file has no size");
        assert!(bin.remove(5).is_none());
        assert_eq!(bin.remove(0).map(|i| i.name), Some("a.mp4".to_owned()));
    }

    #[test]
    fn sizes() {
        assert_eq!(human_size(999), "999 B");
        assert_eq!(human_size(12_400_000), "12.4 MB");
        assert_eq!(human_size(3_000_000_000), "3.0 GB");
    }
}

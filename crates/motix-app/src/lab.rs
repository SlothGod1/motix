//! The Creator Lab library (owner only, ADR-033): the examples MOTIX's AI will learn
//! the owner's taste from.
//!
//! Three collections, kept per user on this PC (not inside projects):
//!
//! * **Edits I love** — finished edits plus what the owner likes about them (pacing,
//!   transitions, colour…), optionally pinned to a moment;
//! * **Upscale comparisons** — before/after pairs and which looks better;
//! * **Scene sources** — films and episodes to search, who to follow, and what kind of
//!   clips to collect (scene packs).
//!
//! Nothing here sends anything anywhere: the analysis runs on this PC when it arrives.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Most entries per collection.
pub const MAX_ENTRIES: usize = 10_000;
/// Longest note, in characters.
pub const MAX_NOTE_CHARS: usize = 2_000;
/// Largest library file read.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Something the owner likes about an edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Cut rhythm and timing.
    Pacing,
    /// Cuts landing on the beat.
    BeatSync,
    /// Transitions between shots.
    Transitions,
    /// Colour and grading.
    Color,
    /// Camera and clip motion (zooms, shakes, speed ramps).
    Motion,
    /// Text and titles.
    Text,
    /// Sound design.
    Sound,
    /// Which shots were chosen.
    ShotChoice,
}

impl Quality {
    /// Every option, in the order shown.
    pub const ALL: [Self; 8] = [
        Self::Pacing,
        Self::BeatSync,
        Self::Transitions,
        Self::Color,
        Self::Motion,
        Self::Text,
        Self::Sound,
        Self::ShotChoice,
    ];

    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pacing => "Pacing",
            Self::BeatSync => "Cuts on the beat",
            Self::Transitions => "Transitions",
            Self::Color => "Colour",
            Self::Motion => "Motion & speed ramps",
            Self::Text => "Text",
            Self::Sound => "Sound",
            Self::ShotChoice => "Shot choice",
        }
    }
}

/// A finished edit the owner loves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LovedEdit {
    /// The video file.
    pub path: PathBuf,
    /// What's good about it.
    #[serde(default)]
    pub qualities: Vec<Quality>,
    /// The owner's own words.
    #[serde(default)]
    pub notes: String,
}

/// Which side of an upscale comparison looks better.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Better {
    /// The original.
    Before,
    /// The upscaled version.
    After,
}

/// A before/after upscale comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscalePair {
    /// The original.
    pub before: PathBuf,
    /// The upscaled version (empty until chosen).
    #[serde(default)]
    pub after: Option<PathBuf>,
    /// Which looks better, once decided.
    #[serde(default)]
    pub better: Option<Better>,
    /// What's better or worse (sharpness, faces, skin, noise…).
    #[serde(default)]
    pub notes: String,
}

/// Kinds of clips to collect from a film or episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipWish {
    /// Every shot of the chosen person.
    Person,
    /// The person walking or pacing.
    Pacing,
    /// Close-ups.
    CloseUps,
    /// Action and fast movement.
    Action,
    /// Cuts that would make good transitions.
    Transitions,
    /// Calm, wide establishing shots.
    Scenery,
}

impl ClipWish {
    /// Every option, in the order shown.
    pub const ALL: [Self; 6] = [
        Self::Person,
        Self::Pacing,
        Self::CloseUps,
        Self::Action,
        Self::Transitions,
        Self::Scenery,
    ];

    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Person => "Every shot of them",
            Self::Pacing => "Walking / pacing",
            Self::CloseUps => "Close-ups",
            Self::Action => "Action",
            Self::Transitions => "Good transition moments",
            Self::Scenery => "Scenery",
        }
    }
}

/// A film or episode to make scene packs from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneSource {
    /// The video file.
    pub path: PathBuf,
    /// Who to follow (a character or actor name, as the owner calls them).
    #[serde(default)]
    pub person: String,
    /// What to collect.
    #[serde(default)]
    pub wishes: Vec<ClipWish>,
}

/// Which collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collection {
    /// Edits I love.
    LovedEdits,
    /// Upscale comparisons (adds "before" files).
    Upscales,
    /// Films and episodes.
    SceneSources,
}

/// The whole library.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabLibrary {
    /// Edits I love.
    #[serde(default)]
    pub loved: Vec<LovedEdit>,
    /// Upscale comparisons.
    #[serde(default)]
    pub upscales: Vec<UpscalePair>,
    /// Films and episodes.
    #[serde(default)]
    pub sources: Vec<SceneSource>,
}

fn clip_note(s: &mut String) {
    if s.chars().count() > MAX_NOTE_CHARS {
        *s = s.chars().take(MAX_NOTE_CHARS).collect();
    }
}

impl LabLibrary {
    /// Loads the library (an empty one if there's none yet or it's unreadable).
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let ok = fs::metadata(path).is_ok_and(|m| m.len() <= MAX_FILE_BYTES);
        let mut lib: Self = ok
            .then(|| fs::read(path).ok())
            .flatten()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        lib.tidy();
        lib
    }

    /// Saves the library.
    ///
    /// # Errors
    /// File-system errors.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        crate::document::write_atomically(path, &bytes)
    }

    fn tidy(&mut self) {
        self.loved.truncate(MAX_ENTRIES);
        self.upscales.truncate(MAX_ENTRIES);
        self.sources.truncate(MAX_ENTRIES);
        for e in &mut self.loved {
            clip_note(&mut e.notes);
            e.qualities.dedup();
        }
        for e in &mut self.upscales {
            clip_note(&mut e.notes);
        }
        for e in &mut self.sources {
            clip_note(&mut e.person);
        }
    }

    /// Adds files to a collection, skipping ones already there. Returns how many were added.
    pub fn add(&mut self, collection: Collection, paths: Vec<PathBuf>) -> usize {
        let mut added = 0;
        for path in paths {
            let exists = match collection {
                Collection::LovedEdits => self.loved.iter().any(|e| e.path == path),
                Collection::Upscales => self.upscales.iter().any(|e| e.before == path),
                Collection::SceneSources => self.sources.iter().any(|e| e.path == path),
            };
            let full = match collection {
                Collection::LovedEdits => self.loved.len(),
                Collection::Upscales => self.upscales.len(),
                Collection::SceneSources => self.sources.len(),
            } >= MAX_ENTRIES;
            if exists || full {
                continue;
            }
            match collection {
                Collection::LovedEdits => self.loved.push(LovedEdit {
                    path,
                    qualities: Vec::new(),
                    notes: String::new(),
                }),
                Collection::Upscales => self.upscales.push(UpscalePair {
                    before: path,
                    after: None,
                    better: None,
                    notes: String::new(),
                }),
                Collection::SceneSources => self.sources.push(SceneSource {
                    path,
                    person: String::new(),
                    wishes: vec![ClipWish::Person],
                }),
            }
            added += 1;
        }
        added
    }

    /// How many examples there are in total.
    #[must_use]
    pub fn total(&self) -> usize {
        self.loved.len() + self.upscales.len() + self.sources.len()
    }
}

/// File name, for display.
#[must_use]
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_round_trips_and_ignores_duplicates() {
        let dir = std::env::temp_dir().join(format!("motix-lab-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let file = dir.join("lab.json");
        assert_eq!(LabLibrary::load(&file), LabLibrary::default(), "nothing yet");

        let mut lib = LabLibrary::default();
        assert_eq!(lib.add(Collection::LovedEdits, vec!["a.mp4".into(), "a.mp4".into()]), 1);
        assert_eq!(lib.add(Collection::Upscales, vec!["old.png".into()]), 1);
        assert_eq!(lib.add(Collection::SceneSources, vec!["film.mkv".into()]), 1);
        lib.loved[0].qualities.push(Quality::Transitions);
        lib.loved[0].notes = "x".repeat(MAX_NOTE_CHARS + 50);
        lib.upscales[0].better = Some(Better::After);
        lib.sources[0].person = "The detective".into();
        lib.save(&file).unwrap();

        let back = LabLibrary::load(&file);
        assert_eq!(back.total(), 3);
        assert_eq!(
            back.loved[0].notes.chars().count(),
            MAX_NOTE_CHARS,
            "long notes trimmed"
        );
        assert_eq!(back.sources[0].wishes, [ClipWish::Person]);
        fs::write(&file, b"garbage").unwrap();
        assert_eq!(
            LabLibrary::load(&file),
            LabLibrary::default(),
            "unreadable means empty, not a crash"
        );
        let _ = fs::remove_dir_all(dir);
    }
}

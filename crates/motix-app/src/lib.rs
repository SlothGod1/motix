//! UI-independent application layer for MOTIX: project settings, media bin, timeline,
//! undo, and the action registry.
//!
//! Everything the user can *do* is an [`actions::Action`] in one registry, so menus,
//! keyboard shortcuts, the command palette and (later) scripting and collaboration
//! all go through the same path (ARCHITECTURE §4.3, background track in PHASES.md).
//!
//! This crate deliberately does not depend on any UI toolkit, so the UI can be
//! replaced (ADR-003) and all behaviour here is unit-testable.

#![forbid(unsafe_code)]
// Status messages are short strings replaced often; `clone_into` would hurt readability.
#![allow(clippy::assigning_clones)]

pub mod actions;
pub mod media;
pub mod project;
pub mod state;
pub mod timeline;
pub mod updates;

pub use actions::{Action, ActionInfo, Availability, Key, Shortcut};
pub use media::{MediaBin, MediaId, MediaItem, MediaKind};
pub use motix_probe as probe;
pub use project::{ColorOutput, FitMode, ProjectSettings, Resolution, SIZE_PRESETS, SettingsError, SizePreset};
pub use state::Tool;
pub use state::{AppState, MatchOffer, Outcome};
pub use timeline::{
    Clip, ClipId, ClipSource, Edge, EditError, Marker, MarkerColor, MarkerId, Timeline, Track, TrackId, TrackKind,
};
pub use updates::{SharingInfo, UpdateInfo, UpdatePhase};

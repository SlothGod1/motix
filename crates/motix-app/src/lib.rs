//! UI-independent application layer for MOTIX.
//!
//! Everything the user can *do* is an [`actions::Action`] in one registry, so menus,
//! keyboard shortcuts, the command palette and (later) scripting and collaboration
//! all go through the same path (ARCHITECTURE §4.3, background track in PHASES.md).
//!
//! This crate deliberately does not depend on any UI toolkit, so the UI can be
//! replaced (ADR-003) and all behaviour here is unit-testable.

#![forbid(unsafe_code)]

pub mod actions;
pub mod canvas;
pub mod media;
pub mod state;

pub use actions::{Action, ActionInfo, Availability, Key, Shortcut};
pub use canvas::CanvasPreset;
pub use media::{MediaBin, MediaItem, MediaKind};
pub use state::{AppState, Outcome};

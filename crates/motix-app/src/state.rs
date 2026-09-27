//! Application state and the single entry point for performing actions.

use crate::actions::{self, Action, Availability};
use crate::{CanvasPreset, MediaBin};
use motix_core::{FrameRate, Time};

/// What happened when an action was performed. The UI reacts to these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Done; nothing else for the UI to do.
    Done,
    /// The UI should open a native file picker for media.
    PickMediaFiles,
    /// The UI should open the command palette.
    OpenCommandPalette,
    /// The UI should restore the default panel layout.
    ResetLayout,
    /// The UI should show the About box.
    ShowAbout,
    /// The UI should close the window.
    Quit,
    /// Not in this build yet; tells the user when it arrives.
    NotYet {
        /// Action label.
        label: &'static str,
        /// Milestone that delivers it.
        milestone: &'static str,
    },
}

/// Everything the UI shows, independent of the UI toolkit.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Imported media.
    pub media: MediaBin,
    /// Output canvas.
    pub canvas: CanvasPreset,
    /// Timeline frame rate.
    pub frame_rate: FrameRate,
    /// Current playhead position.
    pub playhead: Time,
    /// Whether playback is running.
    pub playing: bool,
    /// Whether safe-area guides are shown.
    pub show_safe_areas: bool,
    /// Index of the selected media item, if any.
    pub selected_media: Option<usize>,
    /// Last message for the status bar.
    pub status: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            media: MediaBin::default(),
            canvas: CanvasPreset::VERTICAL,
            frame_rate: FrameRate::FPS_30,
            playhead: Time::ZERO,
            playing: false,
            show_safe_areas: true,
            selected_media: None,
            status: "Welcome to MOTIX — drag videos, audio or images into the window to start.".to_owned(),
        }
    }
}

impl AppState {
    /// Performs an action. This is the only way the UI changes application state,
    /// so menus, shortcuts and the command palette always behave identically.
    pub fn perform(&mut self, action: Action) -> Outcome {
        let info = actions::info_of(action);
        if let Availability::Planned(milestone) = info.availability {
            self.status = format!("\u{201c}{}\u{201d} is coming in milestone {milestone}.", info.label);
            return Outcome::NotYet {
                label: info.label,
                milestone,
            };
        }
        match action {
            Action::ImportMedia => Outcome::PickMediaFiles,
            Action::CommandPalette => Outcome::OpenCommandPalette,
            Action::ResetLayout => Outcome::ResetLayout,
            Action::About => Outcome::ShowAbout,
            Action::Quit => Outcome::Quit,
            Action::TogglePlayback => {
                self.playing = !self.playing;
                Outcome::Done
            }
            Action::GoToStart => {
                self.playhead = Time::ZERO;
                Outcome::Done
            }
            Action::PreviousFrame => {
                self.step_frames(-1);
                Outcome::Done
            }
            Action::NextFrame => {
                self.step_frames(1);
                Outcome::Done
            }
            Action::CanvasVertical => self.set_canvas(CanvasPreset::VERTICAL),
            Action::CanvasPortrait => self.set_canvas(CanvasPreset::PORTRAIT),
            Action::CanvasSquare => self.set_canvas(CanvasPreset::SQUARE),
            Action::CanvasLandscape => self.set_canvas(CanvasPreset::LANDSCAPE),
            Action::ToggleSafeAreas => {
                self.show_safe_areas = !self.show_safe_areas;
                Outcome::Done
            }
            // Planned actions returned above; listed for exhaustiveness.
            Action::NewProject
            | Action::OpenProject
            | Action::SaveVersion
            | Action::Undo
            | Action::Redo
            | Action::Split
            | Action::Export
            | Action::StartCollaboration => Outcome::Done,
        }
    }

    fn set_canvas(&mut self, canvas: CanvasPreset) -> Outcome {
        self.canvas = canvas;
        self.status = format!(
            "Canvas set to {} ({}×{}) — {}.",
            canvas.name, canvas.width, canvas.height, canvas.used_for
        );
        Outcome::Done
    }

    /// Moves the playhead by whole frames, never before zero.
    pub fn step_frames(&mut self, frames: i64) {
        let current = self.frame_rate.time_to_frame(self.playhead);
        let target = current.saturating_add(frames).max(0);
        if let Ok(t) = self.frame_rate.frame_to_time(target) {
            self.playhead = t;
        }
    }

    /// Advances the playhead during playback by `elapsed`, never before zero.
    pub fn advance(&mut self, elapsed: Time) {
        if self.playing {
            self.playhead = self.playhead.saturating_add(elapsed).max(Time::ZERO);
        }
    }

    /// Timecode `HH:MM:SS:FF` for the playhead at the timeline frame rate.
    #[must_use]
    pub fn timecode(&self) -> String {
        timecode(self.playhead, self.frame_rate)
    }
}

/// Formats `t` as `HH:MM:SS:FF` using the nominal (rounded-up) frames per second.
#[must_use]
pub fn timecode(t: Time, rate: FrameRate) -> String {
    let frame = rate.time_to_frame(t).max(0);
    let nominal = rate.as_rational().ceil().max(1);
    let ff = frame % nominal;
    let total_seconds = frame / nominal;
    let (h, m, s) = (total_seconds / 3600, (total_seconds / 60) % 60, total_seconds % 60);
    format!("{h:02}:{m:02}:{s:02}:{ff:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_actions_explain_themselves() {
        let mut s = AppState::default();
        assert_eq!(
            s.perform(Action::Undo),
            Outcome::NotYet {
                label: "Undo",
                milestone: "M2"
            }
        );
        assert!(s.status.contains("M2"));
        assert_eq!(
            s.perform(Action::StartCollaboration),
            Outcome::NotYet {
                label: "Start collaboration session",
                milestone: "M5"
            }
        );
    }

    #[test]
    fn playback_and_stepping() {
        let mut s = AppState::default();
        s.perform(Action::PreviousFrame);
        assert_eq!(s.playhead, Time::ZERO, "cannot go before zero");
        s.perform(Action::NextFrame);
        s.perform(Action::NextFrame);
        assert_eq!(s.timecode(), "00:00:00:02");
        s.advance(Time::SECOND);
        assert_eq!(s.timecode(), "00:00:00:02", "no movement while paused");
        s.perform(Action::TogglePlayback);
        s.advance(Time::SECOND);
        assert_eq!(s.timecode(), "00:00:01:02");
        s.perform(Action::GoToStart);
        assert_eq!(s.playhead, Time::ZERO);
    }

    #[test]
    fn canvas_and_ui_outcomes() {
        let mut s = AppState::default();
        assert_eq!(s.perform(Action::CanvasLandscape), Outcome::Done);
        assert_eq!(s.canvas, CanvasPreset::LANDSCAPE);
        assert_eq!(s.perform(Action::ImportMedia), Outcome::PickMediaFiles);
        assert_eq!(s.perform(Action::CommandPalette), Outcome::OpenCommandPalette);
        let before = s.show_safe_areas;
        s.perform(Action::ToggleSafeAreas);
        assert_ne!(before, s.show_safe_areas);
    }

    #[test]
    fn timecodes() {
        assert_eq!(
            timecode(Time::from_seconds(3_723).unwrap(), FrameRate::FPS_25),
            "01:02:03:00"
        );
        let t = FrameRate::FPS_29_97.frame_to_time(31).unwrap();
        assert_eq!(timecode(t, FrameRate::FPS_29_97), "00:00:01:01");
    }
}

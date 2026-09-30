//! Application state and the single entry point for changing it.
//!
//! Every change to the project (media, timeline, settings) goes through a method
//! here that records an undo step, so undo/redo covers everything and — later —
//! every change maps onto one collaboration operation.

use crate::actions::{self, Action, Availability};
use crate::media::{AddReport, MediaBin, MediaId, MediaItem};
use crate::project::{ColorOutput, FitMode, ProjectSettings, Resolution, SIZE_PRESETS, SettingsError};
use crate::timeline::{ClipId, Edge, EditError, MarkerColor, MarkerId, Timeline, TrackId, TrackKind};
use motix_core::{FrameRate, Time};
use motix_probe::DynamicRange;
use std::path::PathBuf;

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
    /// The host should check for updates now and show the updates window.
    CheckForUpdates,
    /// The host should ask for a folder and set up a shared copy of MOTIX there.
    ShareOnNetwork,
    /// Not in this build yet.
    NotYet {
        /// Action label.
        label: &'static str,
    },
}

/// The timeline's editing tool (DaVinci Resolve keys: A and B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Select, move and trim clips (A).
    #[default]
    Select,
    /// Click a clip to cut it in two at that point (B).
    Blade,
}

/// A suggestion to change the project to match a clip, shown as a question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchOffer {
    /// The media that prompted it.
    pub media: MediaId,
    /// Its name.
    pub name: String,
    /// New size, if different.
    pub resolution: Option<Resolution>,
    /// New frame rate, if different.
    pub frame_rate: Option<FrameRate>,
    /// New colour output, if different (HDR footage).
    pub color: Option<ColorOutput>,
}

impl MatchOffer {
    /// Builds an offer for `item`, or `None` if the project already matches.
    #[must_use]
    pub fn for_media(item: &MediaItem, project: &ProjectSettings) -> Option<Self> {
        let resolution = item
            .resolution()
            .map(|r| Resolution {
                width: (r.width + 1) & !1,
                height: (r.height + 1) & !1,
            })
            .and_then(|r| Resolution::new(r.width, r.height).ok())
            .filter(|r| *r != project.resolution);
        let frame_rate = item.frame_rate().filter(|r| *r != project.frame_rate);
        let color = item
            .info
            .as_ref()
            .and_then(|i| i.video.as_ref())
            .filter(|_| item.info.as_ref().is_some_and(|i| !i.still))
            .map(|v| match v.dynamic_range() {
                DynamicRange::Sdr => ColorOutput::Sdr,
                DynamicRange::Hdr10 | DynamicRange::DolbyVision(_) => ColorOutput::Hdr10,
                DynamicRange::Hlg => ColorOutput::Hlg,
            })
            .filter(|c| c.is_hdr() && *c != project.color);
        (resolution.is_some() || frame_rate.is_some() || color.is_some()).then(|| Self {
            media: item.id,
            name: item.name.clone(),
            resolution,
            frame_rate,
            color,
        })
    }

    /// Each difference as `(setting, project now, clip)`.
    #[must_use]
    pub fn differences(&self, project: &ProjectSettings) -> Vec<(&'static str, String, String)> {
        let mut d = Vec::new();
        if let Some(r) = self.resolution {
            d.push(("Size", project.resolution.to_string(), r.to_string()));
        }
        if let Some(f) = self.frame_rate {
            d.push((
                "Frame rate",
                format!("{} fps", project.frame_rate.short_label()),
                format!("{} fps", f.short_label()),
            ));
        }
        if let Some(c) = self.color {
            d.push(("Colour", project.color.label().to_owned(), c.label().to_owned()));
        }
        d
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    label: String,
    media: MediaBin,
    timeline: Timeline,
    project: ProjectSettings,
}

/// Undo steps kept.
pub const MAX_UNDO: usize = 200;

/// Everything the UI shows, independent of the UI toolkit.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // independent on/off view settings
pub struct AppState {
    /// Imported media.
    pub media: MediaBin,
    /// Project settings.
    pub project: ProjectSettings,
    /// Tracks and clips.
    pub timeline: Timeline,
    /// Current playhead position.
    pub playhead: Time,
    /// Whether playback is running.
    pub playing: bool,
    /// Whether safe-area guides are shown.
    pub show_safe_areas: bool,
    /// Selected media item, if any.
    pub selected_media: Option<MediaId>,
    /// Selected clips.
    pub selected_clips: Vec<ClipId>,
    /// Last message for the status bar.
    pub status: String,
    /// A pending "match project to this clip?" question.
    pub match_offer: Option<MatchOffer>,
    /// Whether the project has already asked (it asks once, for the first video).
    pub match_asked: bool,
    /// Current timeline tool.
    pub tool: Tool,
    /// Clips snap to the playhead, markers and other clips' edges while dragging (N).
    pub snapping: bool,
    /// Clicking a clip also selects the clips linked to it (Ctrl+Shift+L).
    pub linked_selection: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            media: MediaBin::default(),
            project: ProjectSettings::default(),
            timeline: Timeline::default(),
            playhead: Time::ZERO,
            playing: false,
            show_safe_areas: true,
            selected_media: None,
            selected_clips: Vec::new(),
            status: "Welcome to MOTIX — drag videos, audio or images into the window to start.".to_owned(),
            match_offer: None,
            match_asked: false,
            tool: Tool::Select,
            snapping: true,
            linked_selection: true,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl AppState {
    // ----- undo -----

    fn snapshot(&self, label: &str) -> Snapshot {
        Snapshot {
            label: label.to_owned(),
            media: self.media.clone(),
            timeline: self.timeline.clone(),
            project: self.project,
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.media = s.media;
        self.timeline = s.timeline;
        self.project = s.project;
        self.selected_clips.retain(|&c| self.timeline.clip(c).is_some());
        if self.selected_media.is_some_and(|m| self.media.get(m).is_none()) {
            self.selected_media = None;
        }
    }

    /// Runs an edit as one undo step. On error nothing changes and the status says why.
    fn edit<T, E: std::fmt::Display>(
        &mut self,
        label: &str,
        f: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        let before = self.snapshot(label);
        match f(self) {
            Ok(v) => {
                self.undo.push(before);
                if self.undo.len() > MAX_UNDO {
                    self.undo.remove(0);
                }
                self.redo.clear();
                Ok(v)
            }
            Err(e) => {
                self.restore(before);
                self.status = e.to_string();
                Err(e)
            }
        }
    }

    /// `true` if there is something to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// `true` if there is something to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn undo(&mut self) {
        let Some(prev) = self.undo.pop() else {
            self.status = "Nothing to undo.".to_owned();
            return;
        };
        let label = prev.label.clone();
        let now = self.snapshot(&label);
        self.restore(prev);
        self.redo.push(now);
        self.status = format!("Undid: {label}.");
    }

    fn redo(&mut self) {
        let Some(next) = self.redo.pop() else {
            self.status = "Nothing to redo.".to_owned();
            return;
        };
        let label = next.label.clone();
        let now = self.snapshot(&label);
        self.restore(next);
        self.undo.push(now);
        self.status = format!("Redid: {label}.");
    }

    // ----- actions -----

    /// Performs an action. Menus, shortcuts and the command palette all come here,
    /// so they always behave identically.
    pub fn perform(&mut self, action: Action) -> Outcome {
        let info = actions::info_of(action);
        if info.availability == Availability::Soon {
            self.status = format!(
                "\u{201c}{}\u{201d} isn't available yet — it's coming in a future update.",
                info.label
            );
            return Outcome::NotYet { label: info.label };
        }
        match action {
            Action::ImportMedia => return Outcome::PickMediaFiles,
            Action::CommandPalette => return Outcome::OpenCommandPalette,
            Action::ResetLayout => return Outcome::ResetLayout,
            Action::About => return Outcome::ShowAbout,
            Action::Quit => return Outcome::Quit,
            Action::CheckForUpdates => return Outcome::CheckForUpdates,
            Action::ShareOnNetwork => return Outcome::ShareOnNetwork,
            Action::ToolSelect => {
                self.tool = Tool::Select;
                self.status = "Selection tool — click to select, drag to move, drag clip edges to trim.".to_owned();
            }
            Action::ToolBlade => {
                self.tool = Tool::Blade;
                self.status = "Blade tool — click a clip to cut it there. Press A to go back to selecting.".to_owned();
            }
            Action::ToggleSnapping => {
                self.snapping = !self.snapping;
                self.status = format!("Snapping {}.", if self.snapping { "on" } else { "off" });
            }
            Action::ToggleLinkedSelection => {
                self.linked_selection = !self.linked_selection;
                self.status = if self.linked_selection {
                    "Linked selection on — clicking a video also selects its audio.".to_owned()
                } else {
                    "Linked selection off — clips are selected and moved on their own.".to_owned()
                };
            }
            Action::AddMarker => self.add_marker_at_playhead(),
            Action::PreviousEdit => self.jump_edit(false),
            Action::NextEdit => self.jump_edit(true),
            Action::PreviousMarker => self.jump_marker(false),
            Action::NextMarker => self.jump_marker(true),
            Action::AppendToTimeline => match self.selected_media {
                Some(m) => {
                    let _ = self.append_to_timeline(m);
                }
                None => self.status = "Select something in the Media panel first.".to_owned(),
            },
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::TogglePlayback => {
                if !self.playing && !self.timeline.is_empty() && self.playhead >= self.timeline.end() {
                    self.playhead = Time::ZERO;
                }
                self.playing = !self.playing;
            }
            Action::GoToStart => self.playhead = Time::ZERO,
            Action::PreviousFrame => self.step_frames(-1),
            Action::NextFrame => self.step_frames(1),
            Action::ToggleSafeAreas => self.show_safe_areas = !self.show_safe_areas,
            Action::Split => self.split_at_playhead(),
            Action::Delete => self.delete_selection(),
            Action::ToggleLink => self.toggle_link(),
            Action::AddToTimeline => match self.selected_media {
                Some(m) => {
                    let _ = self.add_to_timeline(m, None, None);
                }
                None => self.status = "Select something in the Media panel first.".to_owned(),
            },
            Action::AddVideoTrack => self.add_track(TrackKind::Video),
            Action::AddAudioTrack => self.add_track(TrackKind::Audio),
            Action::MatchProjectToMedia => match self.selected_media {
                Some(m) => self.match_project_to(m),
                None => self.status = "Select a video in the Media panel first.".to_owned(),
            },
            Action::SizeVertical => self.set_size_preset(0),
            Action::SizePortrait => self.set_size_preset(2),
            Action::SizeSquare => self.set_size_preset(3),
            Action::SizeHorizontal => self.set_size_preset(4),
            Action::SwapOrientation => {
                let r = self.project.resolution.swapped();
                let _ = self.set_resolution(r.width, r.height);
            }
            // Unavailable actions returned above; listed for exhaustiveness.
            Action::NewProject
            | Action::OpenProject
            | Action::SaveVersion
            | Action::Export
            | Action::StartCollaboration => {}
        }
        Outcome::Done
    }

    // ----- media -----

    /// Adds files (from a dialog or drag and drop) to the media bin.
    pub fn import(&mut self, paths: Vec<PathBuf>) -> AddReport {
        let mut report = AddReport::default();
        let _ = self.edit("Import media", |s| {
            report = s.media.add_paths(paths);
            if report.added.is_empty() {
                Err("Nothing new was imported.")
            } else {
                Ok(())
            }
        });
        let a = report.added.len();
        let mut parts = Vec::new();
        if a > 0 {
            parts.push(format!("Imported {a} file{}", if a == 1 { "" } else { "s" }));
        }
        if report.duplicates > 0 {
            parts.push(format!("{} already in the project", report.duplicates));
        }
        if report.unsupported > 0 {
            parts.push(format!("{} not a supported media file", report.unsupported));
        }
        if !parts.is_empty() {
            self.status = parts.join(" · ");
        }
        if a > 0 {
            if let Some(&first) = report.added.first() {
                self.selected_media = Some(first);
            }
            self.status
                .push_str(" — double-click or drag onto the timeline to use it.");
        }
        report
    }

    /// Removes media from the project, along with its clips.
    pub fn remove_media(&mut self, id: MediaId) {
        let result = self.edit("Remove media", |s| {
            let removed = s.timeline.delete_media(id);
            let item = s.media.remove(id).ok_or(EditError::NoSuchMedia)?;
            Ok::<_, EditError>((item.name, removed))
        });
        if let Ok((name, clips)) = result {
            self.selected_clips.retain(|&c| self.timeline.clip(c).is_some());
            if self.selected_media == Some(id) {
                self.selected_media = self.media.items().first().map(|i| i.id);
            }
            self.status = if clips > 0 {
                format!("Removed {name} and its {clips} clip(s). The file itself was not touched.")
            } else {
                format!("Removed {name} from the project. The file itself was not touched.")
            };
        }
    }

    // ----- timeline -----

    /// Snaps a time to the start of its frame at the project frame rate.
    #[must_use]
    pub fn snap_to_frame(&self, t: Time) -> Time {
        let rate = self.project.frame_rate;
        rate.frame_to_time(rate.time_to_frame(t.max(Time::ZERO))).unwrap_or(t)
    }

    /// Snaps a time to the first frame boundary at or after it.
    #[must_use]
    pub fn snap_up_to_frame(&self, t: Time) -> Time {
        let rate = self.project.frame_rate;
        let t = t.max(Time::ZERO);
        let mut frame = rate.time_to_frame(t);
        if rate.frame_to_time(frame).is_ok_and(|ft| ft < t) {
            frame += 1;
        }
        rate.frame_to_time(frame).unwrap_or(t)
    }

    /// Places media on the timeline at `at` (or at the playhead), optionally dropped
    /// onto a track. Without a track, the media gets new tracks at the bottom, named
    /// after the file. A video with sound becomes linked video and audio clips.
    ///
    /// # Errors
    /// See [`EditError`]; the status bar explains it too.
    pub fn add_to_timeline(
        &mut self,
        media: MediaId,
        at: Option<Time>,
        track: Option<TrackId>,
    ) -> Result<Vec<ClipId>, EditError> {
        let start = self.snap_to_frame(at.unwrap_or(self.playhead));
        self.place(media, start, track)
    }

    /// Places media right after the end of everything on the timeline.
    ///
    /// # Errors
    /// See [`EditError`].
    pub fn append_to_timeline(&mut self, media: MediaId) -> Result<Vec<ClipId>, EditError> {
        let start = self.snap_up_to_frame(self.timeline.end());
        self.place(media, start, None)
    }

    fn place(&mut self, media: MediaId, start: Time, track: Option<TrackId>) -> Result<Vec<ClipId>, EditError> {
        let item = self.media.get(media).cloned().ok_or(EditError::NoSuchMedia)?;
        let was_empty = self.timeline.is_empty();
        let fit = self.project.default_fit;
        let ids = self.edit("Add clip", |s| s.timeline.place_media(&item, start, track, fit))?;
        self.selected_clips.clone_from(&ids);
        let audio = ids.len() - usize::from(item.has_picture());
        self.status = match (item.has_picture(), audio) {
            (true, n) if n > 0 => format!(
                "Added {} as linked video and {n} audio clip{} — Ctrl+L unlinks them.",
                item.name,
                if n == 1 { "" } else { "s" }
            ),
            _ => format!("Added {} to the timeline.", item.name),
        };
        if was_empty && !self.match_asked && item.has_picture() && item.kind == crate::MediaKind::Video {
            self.match_asked = true;
            self.match_offer = MatchOffer::for_media(&item, &self.project);
        }
        Ok(ids)
    }

    /// The clips that move or trim together with `clip`: the selection if `clip` is
    /// in it, otherwise its linked group (or just the clip, with linked selection off).
    #[must_use]
    pub fn edit_group(&self, clip: ClipId) -> Vec<ClipId> {
        if self.selected_clips.contains(&clip) {
            self.selected_clips.clone()
        } else if self.linked_selection {
            self.timeline.linked_group(clip)
        } else {
            vec![clip]
        }
    }

    /// One frame at the project rate (the shortest a clip can be).
    #[must_use]
    pub fn one_frame(&self) -> Time {
        let rate = self.project.frame_rate;
        rate.frame_to_time(1)
            .unwrap_or(Time::from_flicks(1))
            .max(Time::from_flicks(1))
    }

    /// Moves a clip (and everything linked to it) so it starts at `new_start`,
    /// optionally onto another track of the same kind. The caller snaps `new_start`
    /// (to frames, the playhead or clip edges) — see [`AppState::snap_to_frame`].
    ///
    /// # Errors
    /// See [`EditError`].
    pub fn move_clip(&mut self, clip: ClipId, new_start: Time, track: Option<TrackId>) -> Result<(), EditError> {
        let current = self.timeline.clip(clip).ok_or(EditError::NoSuchClip)?;
        let track = track.filter(|t| *t != current.track);
        let offset = new_start.max(Time::ZERO).saturating_sub(current.start);
        if offset == Time::ZERO && track.is_none() {
            return Ok(());
        }
        let group = self.edit_group(clip);
        self.edit("Move clip", |s| s.timeline.move_clips(&group, clip, offset, track))
    }

    /// How far `edge` of `clip` can be dragged: `(earliest, latest)` offsets.
    ///
    /// # Errors
    /// See [`EditError`].
    pub fn trim_limits(&self, clip: ClipId, edge: Edge) -> Result<(Time, Time), EditError> {
        let group = self.edit_group(clip);
        self.timeline.trim_limits(&group, clip, edge, self.one_frame())
    }

    /// Trims `edge` of `clip` (and the linked clips whose edge lines up) by `delta`.
    ///
    /// # Errors
    /// See [`EditError`].
    pub fn trim_clip(&mut self, clip: ClipId, edge: Edge, delta: Time) -> Result<Time, EditError> {
        if delta == Time::ZERO {
            return Ok(Time::ZERO);
        }
        let group = self.edit_group(clip);
        let min = self.one_frame();
        let applied = self.edit("Trim", |s| s.timeline.trim(&group, clip, edge, delta, min))?;
        self.status = format!(
            "Trimmed the {} by {}.",
            if edge == Edge::Start { "start" } else { "end" },
            timecode(applied.max(Time::ZERO.saturating_sub(applied)), self.project.frame_rate)
        );
        Ok(applied)
    }

    /// Blade tool: cuts `clip` (and its linked clips, with linked selection on) at `at`.
    pub fn blade(&mut self, clip: ClipId, at: Time) {
        let at = self.snap_to_frame(at);
        let targets = if self.linked_selection {
            self.timeline.linked_group(clip)
        } else {
            vec![clip]
        };
        let result = self.edit("Cut", |s| {
            let made = s.timeline.split(&targets, at);
            if made.is_empty() {
                Err(EditError::Nothing("Can't cut exactly at the edge of a clip."))
            } else {
                Ok(made)
            }
        });
        if result.is_ok() {
            self.status = format!("Cut at {}.", timecode(at, self.project.frame_rate));
        }
    }

    /// Selects a clip and everything linked to it (`additive` keeps the current selection).
    pub fn select_clip(&mut self, clip: ClipId, additive: bool) {
        if !additive {
            self.selected_clips.clear();
        }
        let group = if self.linked_selection {
            self.timeline.linked_group(clip)
        } else {
            vec![clip]
        };
        for c in group {
            if !self.selected_clips.contains(&c) {
                self.selected_clips.push(c);
            }
        }
        if let Some(c) = self.timeline.clip(clip) {
            self.selected_media = Some(c.media);
        }
    }

    fn split_at_playhead(&mut self) {
        let t = self.playhead;
        let targets: Vec<ClipId> = if self.selected_clips.is_empty() {
            self.timeline.clips_at(t).map(|c| c.id).collect()
        } else {
            self.selected_clips.clone()
        };
        let result = self.edit("Split", |s| {
            let made = s.timeline.split(&targets, t);
            if made.is_empty() {
                Err(EditError::Nothing(
                    "Nothing to split here — move the playhead over a clip (or select one).",
                ))
            } else {
                Ok(made)
            }
        });
        if let Ok(made) = result {
            self.status = format!("Split {} clip(s) at {}.", made.len(), self.timecode());
        }
    }

    fn delete_selection(&mut self) {
        let targets = self.selected_clips.clone();
        let result = self.edit("Delete", |s| match s.timeline.delete(&targets) {
            0 => Err(EditError::Nothing("Select clips on the timeline to delete them.")),
            n => Ok(n),
        });
        if let Ok(n) = result {
            self.selected_clips.clear();
            self.status = format!("Deleted {n} clip(s).");
        }
    }

    fn toggle_link(&mut self) {
        let targets = self.selected_clips.clone();
        if let Ok(linked) = self.edit("Link / unlink", |s| s.timeline.toggle_link(&targets)) {
            self.status = if linked {
                "Linked — these clips now move together.".to_owned()
            } else {
                "Unlinked — audio and video can now be moved and edited separately.".to_owned()
            };
            if !linked && let Some(&first) = self.selected_clips.first() {
                self.selected_clips = vec![first];
            }
        }
    }

    fn add_track(&mut self, kind: TrackKind) {
        let _ = self.edit("Add track", |s| Ok::<_, EditError>(s.timeline.add_track(kind)));
        self.status = format!("Added a {} track.", kind.label().to_lowercase());
    }

    /// Removes a track and its clips.
    pub fn remove_track(&mut self, track: TrackId) {
        let label = self.timeline.track_label(track);
        if let Ok(n) = self.edit("Delete track", |s| s.timeline.remove_track(track)) {
            self.selected_clips.retain(|&c| self.timeline.clip(c).is_some());
            self.status = format!("Deleted {label} and its {n} clip(s).");
        }
    }

    /// Renames a track.
    pub fn rename_track(&mut self, track: TrackId, name: &str) {
        let _ = self.edit("Rename track", |s| s.timeline.rename_track(track, name));
    }

    /// Moves a track up (`-1`) or down (`+1`).
    pub fn move_track(&mut self, track: TrackId, delta: isize) {
        let _ = self.edit("Move track", |s| s.timeline.move_track(track, delta));
    }

    /// Solos an audio track.
    pub fn set_track_solo(&mut self, track: TrackId, solo: bool) {
        let _ = self.edit(if solo { "Solo track" } else { "Unsolo track" }, |s| {
            s.timeline.set_track_solo(track, solo)
        });
    }

    /// Locks or unlocks a track.
    pub fn set_track_locked(&mut self, track: TrackId, locked: bool) {
        let _ = self.edit(if locked { "Lock track" } else { "Unlock track" }, |s| {
            s.timeline.set_track_locked(track, locked)
        });
        if locked {
            self.selected_clips
                .retain(|&c| self.timeline.clip(c).is_some_and(|c| c.track != track));
        }
    }

    // ----- markers -----

    fn add_marker_at_playhead(&mut self) {
        let t = self.playhead;
        let n = self.timeline.markers().len() + 1;
        if self
            .edit("Add marker", |s| {
                s.timeline.add_marker(t, &format!("Marker {n}"), MarkerColor::Blue)
            })
            .is_ok()
        {
            self.status = format!(
                "Added Marker {n} at {} — right-click it to rename or recolour.",
                self.timecode()
            );
        }
    }

    /// Renames or recolours a marker.
    pub fn update_marker(&mut self, marker: MarkerId, name: &str, color: MarkerColor) {
        let _ = self.edit("Edit marker", |s| s.timeline.update_marker(marker, name, color));
    }

    /// Removes a marker.
    pub fn remove_marker(&mut self, marker: MarkerId) {
        let _ = self.edit("Delete marker", |s| s.timeline.remove_marker(marker));
    }

    fn jump_edit(&mut self, forward: bool) {
        let points = self.timeline.edit_points();
        self.jump_to(points, forward);
    }

    fn jump_marker(&mut self, forward: bool) {
        let points = self.timeline.markers().iter().map(|m| m.time).collect();
        self.jump_to(points, forward);
    }

    fn jump_to(&mut self, points: Vec<Time>, forward: bool) {
        let now = self.playhead;
        let target = if forward {
            points.into_iter().find(|&p| p > now)
        } else {
            points.into_iter().rev().find(|&p| p < now)
        };
        if let Some(t) = target {
            self.playhead = t;
        }
    }

    /// Mutes an audio track or hides a video track.
    pub fn set_track_muted(&mut self, track: TrackId, muted: bool) {
        let _ = self.edit(if muted { "Mute track" } else { "Unmute track" }, |s| {
            s.timeline.set_track_muted(track, muted)
        });
    }

    /// Sets how a clip's picture is placed when its size differs from the project's.
    pub fn set_clip_fit(&mut self, clip: ClipId, fit: FitMode) {
        let _ = self.edit("Change fit", |s| s.timeline.set_fit(clip, fit));
    }

    // ----- project settings -----

    fn set_size_preset(&mut self, index: usize) {
        let r = SIZE_PRESETS[index].resolution;
        let _ = self.set_resolution(r.width, r.height);
    }

    /// Sets the output size.
    ///
    /// # Errors
    /// [`SettingsError`] with a plain explanation (also put in the status bar).
    pub fn set_resolution(&mut self, width: u32, height: u32) -> Result<(), SettingsError> {
        let r = self.edit("Change size", |s| {
            let r = Resolution::new(width, height)?;
            s.project.resolution = r;
            Ok::<_, SettingsError>(r)
        })?;
        self.status = format!(
            "Project size is now {r} ({} {}).",
            r.orientation().to_lowercase(),
            r.aspect_label()
        );
        Ok(())
    }

    /// Sets the frame rate from text the user typed ("30", "29.97", "24000/1001").
    ///
    /// # Errors
    /// [`SettingsError`] with a plain explanation.
    pub fn set_frame_rate_text(&mut self, text: &str) -> Result<(), SettingsError> {
        let rate = FrameRate::parse_user(text).map_err(|_| {
            let e = SettingsError(format!(
                "\u{201c}{}\u{201d} isn't a frame rate. Type a number like 30, 29.97 or 60 (up to 1000).",
                text.trim()
            ));
            self.status = e.0.clone();
            e
        })?;
        self.set_frame_rate(rate);
        Ok(())
    }

    /// Sets the frame rate.
    pub fn set_frame_rate(&mut self, rate: FrameRate) {
        let _ = self.edit("Change frame rate", |s| {
            s.project.frame_rate = rate;
            Ok::<_, EditError>(())
        });
        self.playhead = self.snap_to_frame(self.playhead);
        self.status = format!("Frame rate is now {} fps.", rate.short_label());
    }

    /// Sets the colour output. HDR switches the bit depth to 10 if needed.
    pub fn set_color(&mut self, color: ColorOutput) {
        let _ = self.edit("Change colour output", |s| {
            s.project.color = color;
            if color.is_hdr() && s.project.bit_depth < 10 {
                s.project.bit_depth = 10;
            }
            Ok::<_, EditError>(())
        });
        self.status = format!("Colour output: {} · {}-bit.", color.label(), self.project.bit_depth);
    }

    /// Sets the output bit depth (8 or 10).
    ///
    /// # Errors
    /// [`SettingsError`] if HDR is selected with 8-bit, or the depth is unsupported.
    pub fn set_bit_depth(&mut self, depth: u8) -> Result<(), SettingsError> {
        self.edit("Change bit depth", |s| {
            if !matches!(depth, 8 | 10) {
                return Err(SettingsError("Choose 8-bit or 10-bit.".to_owned()));
            }
            if depth == 8 && s.project.color.is_hdr() {
                return Err(SettingsError(
                    "HDR needs 10-bit — switch colour output to SDR first to use 8-bit.".to_owned(),
                ));
            }
            s.project.bit_depth = depth;
            Ok(())
        })
    }

    /// Sets the default placement for clips whose size differs from the project's.
    /// `apply_to_existing` also changes every clip already on the timeline.
    pub fn set_default_fit(&mut self, fit: FitMode, apply_to_existing: bool) {
        let _ = self.edit("Change default fit", |s| {
            s.project.default_fit = fit;
            if apply_to_existing {
                let ids: Vec<ClipId> = s.timeline.clips().iter().map(|c| c.id).collect();
                for id in ids {
                    s.timeline.set_fit(id, fit)?;
                }
            }
            Ok::<_, EditError>(())
        });
    }

    fn apply_offer(&mut self, offer: &MatchOffer) {
        let _ = self.edit("Match project to clip", |s| {
            if let Some(r) = offer.resolution {
                s.project.resolution = r;
            }
            if let Some(f) = offer.frame_rate {
                s.project.frame_rate = f;
            }
            if let Some(c) = offer.color {
                s.project.color = c;
                s.project.bit_depth = s.project.bit_depth.max(10);
            }
            Ok::<_, EditError>(())
        });
        self.playhead = self.snap_to_frame(self.playhead);
        let p = &self.project;
        self.status = format!(
            "Project now matches {}: {} at {} fps, {}.",
            offer.name,
            p.resolution,
            p.frame_rate.short_label(),
            p.color.label()
        );
    }

    /// Accepts the pending "match project?" question.
    pub fn accept_match(&mut self) {
        if let Some(offer) = self.match_offer.take() {
            self.apply_offer(&offer);
        }
    }

    /// Declines the pending "match project?" question.
    pub fn decline_match(&mut self) {
        if self.match_offer.take().is_some() {
            self.status = format!(
                "Kept the project at {} · {} fps. Clips of other sizes use \u{201c}{}\u{201d}.",
                self.project.resolution,
                self.project.frame_rate.short_label(),
                self.project.default_fit.label()
            );
        }
    }

    /// Changes the project to match a media item right away.
    pub fn match_project_to(&mut self, media: MediaId) {
        let Some(item) = self.media.get(media) else { return };
        match MatchOffer::for_media(item, &self.project) {
            Some(offer) => self.apply_offer(&offer),
            None => {
                self.status = if item.resolution().is_some() {
                    format!("The project already matches {}.", item.name)
                } else {
                    format!("{} has no picture size to match.", item.name)
                };
            }
        }
    }

    // ----- playback -----

    /// Moves the playhead by whole frames, never before zero.
    pub fn step_frames(&mut self, frames: i64) {
        let rate = self.project.frame_rate;
        let current = rate.time_to_frame(self.playhead);
        let target = current.saturating_add(frames).max(0);
        if let Ok(t) = rate.frame_to_time(target) {
            self.playhead = t;
        }
    }

    /// Advances the playhead during playback by `elapsed`; stops at the end of the timeline.
    pub fn advance(&mut self, elapsed: Time) {
        if !self.playing {
            return;
        }
        self.playhead = self.playhead.saturating_add(elapsed).max(Time::ZERO);
        let end = self.timeline.end();
        if !self.timeline.is_empty() && self.playhead >= end {
            self.playhead = end;
            self.playing = false;
        }
    }

    /// Timecode `HH:MM:SS:FF` for the playhead at the project frame rate.
    #[must_use]
    pub fn timecode(&self) -> String {
        timecode(self.playhead, self.project.frame_rate)
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
    use crate::media::MediaKind;
    use motix_probe::{AudioInfo, ColorInfo, Container, MediaInfo, Primaries, Transfer, VideoInfo};

    fn secs(s: i64) -> Time {
        Time::from_seconds(s).unwrap()
    }

    /// Puts a synthetic media item straight into the bin (no file needed).
    fn add_fake(s: &mut AppState, name: &str, size: (u32, u32), rate: FrameRate, hdr: bool, audio: usize) -> MediaId {
        s.media.insert_with(|id| MediaItem {
            id,
            path: PathBuf::from(name),
            name: name.to_owned(),
            kind: MediaKind::Video,
            size_bytes: Some(1_000_000),
            info: Some(MediaInfo {
                container: Container::Mp4,
                duration: Some(secs(8)),
                video: Some(VideoInfo {
                    codec: "HEVC".into(),
                    coded_width: size.0,
                    coded_height: size.1,
                    rotation: 0,
                    frame_rate: Some(rate),
                    variable_frame_rate: false,
                    bit_depth: Some(if hdr { 10 } else { 8 }),
                    color: if hdr {
                        ColorInfo {
                            primaries: Primaries::Bt2020,
                            transfer: Transfer::Pq,
                            full_range: Some(false),
                        }
                    } else {
                        ColorInfo::default()
                    },
                    dolby_vision: None,
                    hdr_metadata: hdr,
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
    fn unavailable_actions_explain_themselves_without_jargon() {
        let mut s = AppState::default();
        assert_eq!(
            s.perform(Action::Export),
            Outcome::NotYet {
                label: "Export video…"
            }
        );
        assert!(s.status.contains("coming in a future update"));
        assert!(!s.status.contains('M'), "no milestone codes: {}", s.status);
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
    fn typed_frame_rates_and_sizes() {
        let mut s = AppState::default();
        s.set_frame_rate_text("29.97").unwrap();
        assert_eq!(s.project.frame_rate, FrameRate::FPS_29_97);
        s.set_frame_rate_text("12.5").unwrap();
        assert_eq!(s.project.frame_rate.short_label(), "12.5");
        assert!(s.set_frame_rate_text("fast").is_err());
        assert!(s.status.contains("isn't a frame rate"));
        s.set_resolution(2160, 3840).unwrap();
        assert_eq!(
            s.project.resolution,
            Resolution {
                width: 2160,
                height: 3840
            }
        );
        assert!(s.set_resolution(2161, 3840).is_err());
        assert_eq!(s.project.resolution.width, 2160, "unchanged after error");
        s.perform(Action::SwapOrientation);
        assert_eq!(
            s.project.resolution,
            Resolution {
                width: 3840,
                height: 2160
            }
        );
        // All of that is undoable.
        s.perform(Action::Undo);
        assert_eq!(
            s.project.resolution,
            Resolution {
                width: 2160,
                height: 3840
            }
        );
        s.perform(Action::Redo);
        assert_eq!(s.project.resolution.width, 3840);
    }

    #[test]
    fn hdr_needs_ten_bit() {
        let mut s = AppState::default();
        s.set_color(ColorOutput::Hdr10);
        assert_eq!(s.project.bit_depth, 10);
        assert!(s.set_bit_depth(8).is_err());
        s.set_color(ColorOutput::Sdr);
        s.set_bit_depth(8).unwrap();
        assert!(s.set_bit_depth(9).is_err());
    }

    #[test]
    fn first_video_asks_to_match_project_once() {
        let mut s = AppState::default();
        let uhd = add_fake(&mut s, "uhd.mp4", (3840, 2160), FrameRate::FPS_59_94, true, 1);
        let fhd = add_fake(&mut s, "fhd.mp4", (1920, 1080), FrameRate::FPS_30, false, 1);
        let ids = s.add_to_timeline(uhd, None, None).unwrap();
        assert_eq!(ids.len(), 2, "video with sound becomes linked video + audio");
        let offer = s.match_offer.clone().expect("asks");
        assert_eq!(
            offer.resolution,
            Some(Resolution {
                width: 3840,
                height: 2160
            })
        );
        assert_eq!(offer.frame_rate, Some(FrameRate::FPS_59_94));
        assert_eq!(offer.color, Some(ColorOutput::Hdr10));
        assert_eq!(offer.differences(&s.project).len(), 3);
        s.accept_match();
        assert_eq!(s.project.resolution.width, 3840);
        assert_eq!(s.project.frame_rate, FrameRate::FPS_59_94);
        assert_eq!((s.project.color, s.project.bit_depth), (ColorOutput::Hdr10, 10));
        // Second video (appended): not asked again; placed after the first, scaled by the default fit.
        s.append_to_timeline(fhd).unwrap();
        assert!(s.match_offer.is_none());
        let clip = s.timeline.clip(s.selected_clips[0]).unwrap();
        assert_eq!(
            clip.start,
            FrameRate::FPS_59_94.frame_to_time(480).unwrap(),
            "next frame after 8 s"
        );
        assert_ne!(
            clip.track,
            s.timeline.clip(ids[0]).unwrap().track,
            "each file gets its own tracks"
        );
        assert_eq!(s.timeline.tracks().len(), 4);
        assert_eq!(clip.fit, FitMode::Fit);
        // Undo the match (and the second clip).
        s.perform(Action::Undo);
        s.perform(Action::Undo);
        assert_eq!(s.project.resolution.width, 1080);
    }

    #[test]
    fn declining_keeps_settings_and_matching_later_works() {
        let mut s = AppState::default();
        let v = add_fake(&mut s, "v.mp4", (1920, 1080), FrameRate::FPS_24, false, 0);
        s.add_to_timeline(v, None, None).unwrap();
        s.decline_match();
        assert_eq!(s.project.resolution.width, 1080);
        assert!(s.status.contains("Scale to fit"));
        s.selected_media = Some(v);
        s.perform(Action::MatchProjectToMedia);
        assert_eq!(
            s.project.resolution,
            Resolution {
                width: 1920,
                height: 1080
            }
        );
        assert_eq!(s.project.frame_rate, FrameRate::FPS_24);
        s.perform(Action::MatchProjectToMedia);
        assert!(s.status.contains("already matches"));
    }

    #[test]
    fn edit_split_delete_link_undo() {
        let mut s = AppState::default();
        let v = add_fake(&mut s, "v.mp4", (1080, 1920), FrameRate::FPS_30, false, 1);
        let ids = s.add_to_timeline(v, None, None).unwrap();
        assert!(s.match_offer.is_none(), "already matches the default vertical project");
        s.playhead = secs(3);
        s.selected_clips.clear();
        s.perform(Action::Split);
        assert_eq!(s.timeline.clips().len(), 4);
        s.select_clip(ids[0], false);
        assert_eq!(s.selected_clips.len(), 2, "linked partner selected too");
        s.perform(Action::ToggleLink);
        assert_eq!(s.selected_clips.len(), 1);
        s.perform(Action::Delete);
        assert_eq!(s.timeline.clips().len(), 3);
        s.move_clip(ids[1], secs(20), None).unwrap();
        assert_eq!(s.timeline.clip(ids[1]).unwrap().start, secs(20));
        for _ in 0..4 {
            s.perform(Action::Undo);
        }
        assert_eq!(s.timeline.clips().len(), 2);
        s.perform(Action::Undo);
        assert!(s.timeline.is_empty());
    }

    #[test]
    fn playback_stops_at_the_end() {
        let mut s = AppState::default();
        let v = add_fake(&mut s, "v.mp4", (1080, 1920), FrameRate::FPS_30, false, 0);
        s.add_to_timeline(v, None, None).unwrap();
        s.perform(Action::TogglePlayback);
        s.advance(secs(20));
        assert!(!s.playing);
        assert_eq!(s.playhead, secs(8));
        s.perform(Action::TogglePlayback);
        assert_eq!(s.playhead, Time::ZERO, "play from the end restarts");
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

    #[test]
    fn tools_markers_and_jumps() {
        let mut s = AppState::default();
        let v = add_fake(&mut s, "v.mp4", (1080, 1920), FrameRate::FPS_30, false, 1);
        let ids = s.add_to_timeline(v, None, None).unwrap();
        // Blade cuts both linked clips.
        s.perform(Action::ToolBlade);
        assert_eq!(s.tool, Tool::Blade);
        s.blade(ids[0], secs(3));
        assert_eq!(s.timeline.clips().len(), 4);
        // Markers and jumping.
        s.playhead = secs(2);
        s.perform(Action::AddMarker);
        s.playhead = Time::ZERO;
        s.perform(Action::NextMarker);
        assert_eq!(s.playhead, secs(2));
        s.perform(Action::NextEdit);
        assert_eq!(s.playhead, secs(3));
        s.perform(Action::NextEdit);
        assert_eq!(s.playhead, secs(8));
        s.perform(Action::PreviousEdit);
        assert_eq!(s.playhead, secs(3));
        // Linked selection off: selecting and moving one clip only.
        s.perform(Action::ToggleLinkedSelection);
        s.select_clip(ids[0], false);
        assert_eq!(s.selected_clips, vec![ids[0]]);
        s.move_clip(ids[0], secs(20), None).unwrap();
        assert_eq!(s.timeline.clip(ids[1]).unwrap().start, Time::ZERO);
        // Trim the end of the moved clip by -1 s.
        let before = s.timeline.clip(ids[0]).unwrap().duration;
        s.trim_clip(ids[0], Edge::End, Time::ZERO.saturating_sub(secs(1)))
            .unwrap();
        assert_eq!(
            s.timeline.clip(ids[0]).unwrap().duration,
            before.saturating_sub(secs(1))
        );
    }

    #[test]
    fn add_goes_to_the_playhead_on_new_tracks() {
        let mut s = AppState::default();
        let a = add_fake(&mut s, "a.mp4", (1080, 1920), FrameRate::FPS_30, false, 1);
        let b = add_fake(&mut s, "b.mp4", (1080, 1920), FrameRate::FPS_30, false, 1);
        s.add_to_timeline(a, None, None).unwrap();
        s.playhead = secs(2);
        let ids = s.add_to_timeline(b, None, None).unwrap();
        assert_eq!(s.timeline.clip(ids[0]).unwrap().start, secs(2));
        let names: Vec<_> = s.timeline.tracks().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["a.mp4", "Audio of a.mp4", "b.mp4", "Audio of b.mp4"]);
        let t = s.timeline.tracks()[0].id;
        s.rename_track(t, "Intro");
        s.move_track(t, 2);
        assert_eq!(s.timeline.tracks()[2].name, "Intro");
        s.perform(Action::Undo);
        s.perform(Action::Undo);
        assert_eq!(s.timeline.tracks()[0].name, "a.mp4");
    }
}

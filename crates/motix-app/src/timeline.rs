//! The timeline model: any number of video and audio tracks holding clips, plus markers.
//!
//! There is no fixed track layout. Tracks are listed **top to bottom in the order you
//! add media**, and each new track is named after the file it was made for — a video
//! with sound gives a track named after the video and, right below it, one named
//! "Audio of …". Tracks can be renamed, reordered, hidden/muted, soloed and locked.
//! **Higher tracks are in front**: where two pictures overlap in time, the one on the
//! track nearer the top is seen.
//!
//! A video file with sound becomes a **linked group** — one picture clip plus one audio
//! clip per audio stream — that moves, trims, splits and deletes together until the user
//! unlinks it.
//!
//! This is a plain in-memory model. In the collaboration phase it is backed by the
//! CRDT document (ARCHITECTURE §7, ADR-026); every change goes through a named method
//! that maps one-to-one onto a document operation.

use crate::media::{MediaId, MediaItem};
use crate::project::{FitMode, Resolution};
use motix_core::{Time, TimeRange};
use std::fmt;

/// Longest track or marker name accepted, in characters.
pub const MAX_NAME_CHARS: usize = 200;

/// Stable identifier of a track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u64);
/// Stable identifier of a clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipId(pub u64);
/// Identifier shared by clips that move together (video + its audio).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LinkId(pub u64);
/// Stable identifier of a marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkerId(pub u64);

/// What a track holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackKind {
    /// Pictures: video and stills. Tracks nearer the top are in front.
    Video,
    /// Sound. All audible audio tracks are mixed together.
    Audio,
}

impl TrackKind {
    /// `"Video"` or `"Audio"`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
        }
    }
}

/// A track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    /// Identifier.
    pub id: TrackId,
    /// What it holds.
    pub kind: TrackKind,
    /// Name shown in the track header (the file name unless the user renamed it).
    pub name: String,
    /// Audio tracks: silenced. Video tracks: hidden.
    pub muted: bool,
    /// Audio tracks: when any track is soloed, only soloed tracks are heard.
    pub solo: bool,
    /// Locked tracks can't be edited (clips can't be moved, trimmed, split or deleted).
    pub locked: bool,
}

/// Which part of the media file a clip plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipSource {
    /// The picture (video or still image).
    Picture,
    /// The audio stream with this index.
    Audio(usize),
}

/// A piece of media placed on a track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    /// Identifier.
    pub id: ClipId,
    /// The track it's on.
    pub track: TrackId,
    /// Media it plays.
    pub media: MediaId,
    /// Which part of the media.
    pub source: ClipSource,
    /// Name shown on the clip.
    pub name: String,
    /// Where it starts on the timeline.
    pub start: Time,
    /// How long it plays.
    pub duration: Time,
    /// Where in the media file playback starts.
    pub source_in: Time,
    /// Length of the media, when known (stills have no limit).
    pub source_duration: Option<Time>,
    /// Group of clips that move together, if linked.
    pub link: Option<LinkId>,
    /// Placement when the picture's size differs from the project's.
    pub fit: FitMode,
    /// Picture size of the media, when known.
    pub source_size: Option<Resolution>,
}

impl Clip {
    /// Where it ends on the timeline (exclusive).
    #[must_use]
    pub fn end(&self) -> Time {
        self.start.saturating_add(self.duration)
    }

    /// The span it covers.
    #[must_use]
    pub fn range(&self) -> TimeRange {
        TimeRange::new(self.start, self.end()).unwrap_or(TimeRange::EMPTY)
    }
}

/// Which end of a clip is being trimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The start (left edge).
    Start,
    /// The end (right edge).
    End,
}

/// Marker colours (the same set DaVinci Resolve uses).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkerColor {
    /// Blue (default).
    Blue,
    /// Cyan.
    Cyan,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Red.
    Red,
    /// Pink.
    Pink,
    /// Purple.
    Purple,
}

impl MarkerColor {
    /// Every colour.
    pub const ALL: [Self; 7] = [
        Self::Blue,
        Self::Cyan,
        Self::Green,
        Self::Yellow,
        Self::Red,
        Self::Pink,
        Self::Purple,
    ];

    /// Plain-language name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Cyan => "Cyan",
            Self::Green => "Green",
            Self::Yellow => "Yellow",
            Self::Red => "Red",
            Self::Pink => "Pink",
            Self::Purple => "Purple",
        }
    }
}

/// A named point in time on the timeline (press M).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// Identifier.
    pub id: MarkerId,
    /// Where it is.
    pub time: Time,
    /// Its label.
    pub name: String,
    /// Its colour.
    pub color: MarkerColor,
}

/// Why an edit couldn't be made, in plain language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// Another clip is in the way.
    Occupied,
    /// The clip no longer exists.
    NoSuchClip,
    /// The track no longer exists.
    NoSuchTrack,
    /// Audio can't go on a video track and vice versa.
    WrongTrackKind,
    /// The media is not in the project.
    NoSuchMedia,
    /// The media has nothing that can go on a timeline.
    NothingToPlace,
    /// The track is locked.
    Locked,
    /// The edit would do nothing (with a hint).
    Nothing(&'static str),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Occupied => "There's already a clip there.",
            Self::NoSuchClip => "That clip no longer exists.",
            Self::NoSuchTrack => "That track no longer exists.",
            Self::WrongTrackKind => "Audio clips go on audio tracks and pictures on video tracks.",
            Self::NoSuchMedia => "That media is no longer in the project.",
            Self::NothingToPlace => "This file has no picture or sound that can be placed yet.",
            Self::Locked => "That track is locked — unlock it to edit its clips.",
            Self::Nothing(hint) => hint,
        })
    }
}

impl std::error::Error for EditError {}

fn clean_name(name: &str) -> Result<String, EditError> {
    let name: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    if name.is_empty() {
        Err(EditError::Nothing("A name can't be empty."))
    } else {
        Ok(name)
    }
}

/// All tracks, clips and markers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    /// Top to bottom.
    tracks: Vec<Track>,
    clips: Vec<Clip>,
    /// Sorted by time.
    markers: Vec<Marker>,
    next_id: u64,
}

impl Timeline {
    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    // ----- lookup -----

    /// Every track, top to bottom.
    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Every clip.
    #[must_use]
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    /// Markers, earliest first.
    #[must_use]
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// `true` when there are no clips.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    /// Looks up a clip.
    #[must_use]
    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    /// Looks up a track.
    #[must_use]
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    fn track_mut(&mut self, id: TrackId) -> Result<&mut Track, EditError> {
        self.tracks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(EditError::NoSuchTrack)
    }

    fn index_of(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == id)
    }

    /// Tracks of one kind, top to bottom.
    pub fn tracks_of(&self, kind: TrackKind) -> impl Iterator<Item = &Track> {
        self.tracks.iter().filter(move |t| t.kind == kind)
    }

    /// Track ids top to bottom (the order they're shown).
    #[must_use]
    pub fn display_order(&self) -> Vec<TrackId> {
        self.tracks.iter().map(|t| t.id).collect()
    }

    /// The track's name.
    #[must_use]
    pub fn track_label(&self, id: TrackId) -> String {
        self.track(id).map(|t| t.name.clone()).unwrap_or_default()
    }

    /// `true` if the clip's track is locked (or the clip is gone).
    #[must_use]
    pub fn is_locked(&self, clip: ClipId) -> bool {
        self.clip(clip)
            .and_then(|c| self.track(c.track))
            .is_none_or(|t| t.locked)
    }

    /// Whether an audio track is heard, taking mute and solo into account.
    #[must_use]
    pub fn is_audible(&self, id: TrackId) -> bool {
        let any_solo = self.tracks_of(TrackKind::Audio).any(|t| t.solo);
        self.track(id)
            .is_some_and(|t| t.kind == TrackKind::Audio && !t.muted && (!any_solo || t.solo))
    }

    /// End of the last clip.
    #[must_use]
    pub fn end(&self) -> Time {
        self.clips.iter().map(Clip::end).max().unwrap_or(Time::ZERO)
    }

    /// `true` if nothing but `ignoring` overlaps `range` on `track`.
    #[must_use]
    pub fn is_free(&self, track: TrackId, range: TimeRange, ignoring: &[ClipId]) -> bool {
        !self
            .clips
            .iter()
            .any(|c| c.track == track && !ignoring.contains(&c.id) && c.range().overlaps(range))
    }

    // ----- tracks -----

    fn insert_track(&mut self, kind: TrackKind, name: String, after: Option<TrackId>) -> TrackId {
        let id = TrackId(self.new_id());
        let track = Track {
            id,
            kind,
            name,
            muted: false,
            solo: false,
            locked: false,
        };
        match after.and_then(|a| self.index_of(a)) {
            Some(i) => self.tracks.insert(i + 1, track),
            None => self.tracks.push(track),
        }
        id
    }

    /// Adds an empty track at the bottom, named e.g. "Video track 2".
    pub fn add_track(&mut self, kind: TrackKind) -> TrackId {
        let n = self.tracks_of(kind).count() + 1;
        self.insert_track(kind, format!("{} track {n}", kind.label()), None)
    }

    /// Renames a track.
    ///
    /// # Errors
    /// [`EditError::NoSuchTrack`], or [`EditError::Nothing`] for an empty name.
    pub fn rename_track(&mut self, id: TrackId, name: &str) -> Result<(), EditError> {
        let name = clean_name(name)?;
        self.track_mut(id)?.name = name;
        Ok(())
    }

    /// Moves a track up (`-1`) or down (`+1`) in the list. Moving a video track up
    /// brings its pictures in front of the tracks below it.
    ///
    /// # Errors
    /// [`EditError::NoSuchTrack`], or [`EditError::Nothing`] at the top/bottom.
    pub fn move_track(&mut self, id: TrackId, delta: isize) -> Result<(), EditError> {
        let from = self.index_of(id).ok_or(EditError::NoSuchTrack)?;
        let to = from
            .checked_add_signed(delta)
            .filter(|&t| t < self.tracks.len())
            .ok_or(EditError::Nothing("The track can't move any further."))?;
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        Ok(())
    }

    /// Removes a track and every clip on it. Returns how many clips were removed.
    ///
    /// # Errors
    /// [`EditError::NoSuchTrack`], [`EditError::Locked`].
    pub fn remove_track(&mut self, id: TrackId) -> Result<usize, EditError> {
        let index = self.index_of(id).ok_or(EditError::NoSuchTrack)?;
        if self.tracks[index].locked {
            return Err(EditError::Locked);
        }
        self.tracks.remove(index);
        let before = self.clips.len();
        self.clips.retain(|c| c.track != id);
        self.tidy_links();
        Ok(before - self.clips.len())
    }

    /// Mutes (audio) or hides (video) a track.
    ///
    /// # Errors
    /// [`EditError::NoSuchTrack`].
    pub fn set_track_muted(&mut self, id: TrackId, muted: bool) -> Result<(), EditError> {
        self.track_mut(id)?.muted = muted;
        Ok(())
    }

    /// Solos an audio track.
    ///
    /// # Errors
    /// [`EditError::NoSuchTrack`].
    pub fn set_track_solo(&mut self, id: TrackId, solo: bool) -> Result<(), EditError> {
        self.track_mut(id)?.solo = solo;
        Ok(())
    }

    /// Locks or unlocks a track.
    ///
    /// # Errors
    /// [`EditError::NoSuchTrack`].
    pub fn set_track_locked(&mut self, id: TrackId, locked: bool) -> Result<(), EditError> {
        self.track_mut(id)?.locked = locked;
        Ok(())
    }

    // ----- placing media -----

    /// Places media at `start`: its picture and each audio stream get their own track.
    ///
    /// With no `target`, new tracks are added **at the bottom**, named after the file
    /// ("clip.mp4", then "Audio of clip.mp4"). With a `target` (media dropped onto a
    /// track), the first part goes on that track if it fits there, and the rest on the
    /// tracks directly below it — any that don't fit get new tracks inserted right
    /// there. Placing never fails for lack of room.
    ///
    /// # Errors
    /// [`EditError::NothingToPlace`] if the media has neither picture nor sound.
    pub fn place_media(
        &mut self,
        media: &MediaItem,
        start: Time,
        target: Option<TrackId>,
        fit: FitMode,
    ) -> Result<Vec<ClipId>, EditError> {
        let has_picture = media.has_picture();
        let audio = media.audio_streams();
        if !has_picture && audio == 0 {
            return Err(EditError::NothingToPlace);
        }
        let start = start.max(Time::ZERO);
        let duration = media.timeline_duration();
        let range = TimeRange::from_start_duration(start, duration).map_err(|_| EditError::Nothing("Too long."))?;
        let source_duration = media.info.as_ref().filter(|i| !i.still).and_then(|i| i.duration);

        let name = &media.name;
        let mut parts: Vec<(TrackKind, ClipSource, String)> = Vec::new();
        if has_picture {
            parts.push((TrackKind::Video, ClipSource::Picture, name.clone()));
        }
        for stream in 0..audio {
            let track_name = match (has_picture, audio) {
                (true, 1) => format!("Audio of {name}"),
                (true, _) => format!("Audio {} of {name}", stream + 1),
                (false, 1) => name.clone(),
                (false, _) => format!("{name} (stream {})", stream + 1),
            };
            parts.push((TrackKind::Audio, ClipSource::Audio(stream), track_name));
        }

        let target = target.filter(|t| self.track(*t).is_some());
        let link = (parts.len() > 1).then(|| LinkId(self.new_id()));
        let mut placed = Vec::new();
        let mut last: Option<TrackId> = None;
        for (i, (kind, source, track_name)) in parts.into_iter().enumerate() {
            let candidate = if i == 0 {
                target
            } else {
                last.and_then(|l| self.index_of(l))
                    .and_then(|ix| self.tracks.get(ix + 1))
                    .map(|t| t.id)
            };
            let usable = candidate
                .filter(|&c| self.track(c).is_some_and(|t| t.kind == kind && !t.locked) && self.is_free(c, range, &[]));
            // New tracks go right below the previous part, below the drop target,
            // or at the very bottom.
            let after = if i == 0 { target } else { last };
            let on = usable.unwrap_or_else(|| self.insert_track(kind, track_name, after));
            let id = ClipId(self.new_id());
            self.clips.push(Clip {
                id,
                track: on,
                media: media.id,
                source,
                name: media.name.clone(),
                start,
                duration,
                source_in: Time::ZERO,
                source_duration,
                link,
                fit,
                source_size: media.resolution(),
            });
            placed.push(id);
            last = Some(on);
        }
        Ok(placed)
    }

    // ----- moving -----

    /// Every clip that moves with `id` when linked selection is on (itself included).
    #[must_use]
    pub fn linked_group(&self, id: ClipId) -> Vec<ClipId> {
        match self.clip(id) {
            None => Vec::new(),
            Some(Clip { link: None, .. }) => vec![id],
            Some(Clip { link: Some(l), .. }) => {
                self.clips.iter().filter(|c| c.link == Some(*l)).map(|c| c.id).collect()
            }
        }
    }

    /// Checks whether moving `group` by `offset` is possible (the `grabbed` clip may
    /// also change to `to_track`), without changing anything. Returns the adjusted
    /// offset (never moving a clip before zero).
    ///
    /// # Errors
    /// [`EditError::Occupied`], [`EditError::Locked`], [`EditError::WrongTrackKind`],
    /// [`EditError::NoSuchClip`], [`EditError::NoSuchTrack`].
    pub fn can_move(
        &self,
        group: &[ClipId],
        grabbed: ClipId,
        offset: Time,
        to_track: Option<TrackId>,
    ) -> Result<Time, EditError> {
        let grabbed_clip = self.clip(grabbed).ok_or(EditError::NoSuchClip)?;
        if let Some(t) = to_track {
            let target = self.track(t).ok_or(EditError::NoSuchTrack)?;
            if Some(target.kind) != self.track(grabbed_clip.track).map(|t| t.kind) {
                return Err(EditError::WrongTrackKind);
            }
            if target.locked {
                return Err(EditError::Locked);
            }
        }
        let mut earliest = Time::MAX;
        for &c in group {
            if self.is_locked(c) {
                return Err(EditError::Locked);
            }
            earliest = earliest.min(self.clip(c).ok_or(EditError::NoSuchClip)?.start);
        }
        let offset = offset.max(Time::ZERO.saturating_sub(earliest));
        for &c in group {
            let clip = self.clip(c).ok_or(EditError::NoSuchClip)?;
            let track = if c == grabbed {
                to_track.unwrap_or(clip.track)
            } else {
                clip.track
            };
            let range = clip
                .range()
                .shifted(offset)
                .map_err(|_| EditError::Nothing("Too far."))?;
            if !self.is_free(track, range, group) {
                return Err(EditError::Occupied);
            }
        }
        Ok(offset)
    }

    /// Moves `group` by `offset`; the `grabbed` clip may also change to another track of
    /// the same kind.
    ///
    /// # Errors
    /// As [`Timeline::can_move`].
    pub fn move_clips(
        &mut self,
        group: &[ClipId],
        grabbed: ClipId,
        offset: Time,
        to_track: Option<TrackId>,
    ) -> Result<(), EditError> {
        let offset = self.can_move(group, grabbed, offset, to_track)?;
        for &c in group {
            let clip = self.clip_mut(c).ok_or(EditError::NoSuchClip)?;
            clip.start = clip.start.saturating_add(offset);
            if c == grabbed
                && let Some(t) = to_track
            {
                clip.track = t;
            }
        }
        Ok(())
    }

    /// Moves a clip and everything linked to it (convenience for [`Timeline::move_clips`]).
    ///
    /// # Errors
    /// As [`Timeline::can_move`].
    pub fn move_group(&mut self, id: ClipId, offset: Time, to_track: Option<TrackId>) -> Result<(), EditError> {
        let group = self.linked_group(id);
        self.move_clips(&group, id, offset, to_track)
    }

    // ----- trimming -----

    /// The range of `delta` allowed when dragging `edge` of `grabbed`, applied to every
    /// clip in `group` whose same edge lines up with it. Trims stop at the media's
    /// length, at neighbouring clips and at `min_len`.
    ///
    /// # Errors
    /// [`EditError::NoSuchClip`], [`EditError::Locked`].
    pub fn trim_limits(
        &self,
        group: &[ClipId],
        grabbed: ClipId,
        edge: Edge,
        min_len: Time,
    ) -> Result<(Time, Time), EditError> {
        let g = self.clip(grabbed).ok_or(EditError::NoSuchClip)?;
        let edge_time = match edge {
            Edge::Start => g.start,
            Edge::End => g.end(),
        };
        let (mut lo, mut hi) = (Time::MIN, Time::MAX);
        for c in self.trim_members(group, edge, edge_time) {
            if self.is_locked(c.id) {
                return Err(EditError::Locked);
            }
            let others = self
                .clips
                .iter()
                .filter(|o| o.track == c.track && !group.contains(&o.id) && o.id != c.id);
            match edge {
                Edge::Start => {
                    // Not before the media's first frame, not before zero, not past the end.
                    lo = lo.max(Time::ZERO.saturating_sub(c.source_in));
                    lo = lo.max(Time::ZERO.saturating_sub(c.start));
                    hi = hi.min(c.duration.saturating_sub(min_len));
                    if let Some(prev) = others.map(Clip::end).filter(|&e| e <= c.start).max() {
                        lo = lo.max(prev.saturating_sub(c.start));
                    }
                }
                Edge::End => {
                    lo = lo.max(min_len.saturating_sub(c.duration));
                    if let Some(sd) = c.source_duration {
                        hi = hi.min(sd.saturating_sub(c.source_in).saturating_sub(c.duration));
                    }
                    if let Some(next) = others.map(|o| o.start).filter(|&s| s >= c.end()).min() {
                        hi = hi.min(next.saturating_sub(c.end()));
                    }
                }
            }
        }
        Ok((lo, hi.max(lo)))
    }

    fn trim_members<'a>(&'a self, group: &'a [ClipId], edge: Edge, edge_time: Time) -> impl Iterator<Item = &'a Clip> {
        self.clips.iter().filter(move |c| {
            group.contains(&c.id)
                && match edge {
                    Edge::Start => c.start == edge_time,
                    Edge::End => c.end() == edge_time,
                }
        })
    }

    /// Trims `edge` of `grabbed` (and of the clips in `group` whose edge lines up with it)
    /// by `delta`, clamped to [`Timeline::trim_limits`]. Returns the delta applied.
    ///
    /// # Errors
    /// As [`Timeline::trim_limits`].
    pub fn trim(
        &mut self,
        group: &[ClipId],
        grabbed: ClipId,
        edge: Edge,
        delta: Time,
        min_len: Time,
    ) -> Result<Time, EditError> {
        let (lo, hi) = self.trim_limits(group, grabbed, edge, min_len)?;
        let delta = delta.clamp(lo, hi);
        let g = self.clip(grabbed).ok_or(EditError::NoSuchClip)?;
        let edge_time = match edge {
            Edge::Start => g.start,
            Edge::End => g.end(),
        };
        let ids: Vec<ClipId> = self.trim_members(group, edge, edge_time).map(|c| c.id).collect();
        for id in ids {
            let c = self.clip_mut(id).ok_or(EditError::NoSuchClip)?;
            match edge {
                Edge::Start => {
                    c.start = c.start.saturating_add(delta);
                    c.source_in = c.source_in.saturating_add(delta);
                    c.duration = c.duration.saturating_sub(delta);
                }
                Edge::End => c.duration = c.duration.saturating_add(delta),
            }
        }
        Ok(delta)
    }

    // ----- other edits -----

    /// Deletes clips (clips on locked tracks are kept). Returns how many were removed.
    pub fn delete(&mut self, ids: &[ClipId]) -> usize {
        let deletable: Vec<ClipId> = ids.iter().copied().filter(|&c| !self.is_locked(c)).collect();
        let before = self.clips.len();
        self.clips.retain(|c| !deletable.contains(&c.id));
        self.tidy_links();
        before - self.clips.len()
    }

    /// Deletes every clip of a media item, locked or not. Returns how many were removed.
    pub fn delete_media(&mut self, media: MediaId) -> usize {
        let before = self.clips.len();
        self.clips.retain(|c| c.media != media);
        self.tidy_links();
        before - self.clips.len()
    }

    /// Splits clips at `at` (clips on locked tracks are skipped). Linked clips split
    /// together and the right-hand parts stay linked to each other. Returns the new
    /// right-hand clips.
    pub fn split(&mut self, ids: &[ClipId], at: Time) -> Vec<ClipId> {
        let mut new_links: Vec<(LinkId, LinkId)> = Vec::new();
        let mut created = Vec::new();
        let targets: Vec<ClipId> = self
            .clips
            .iter()
            .filter(|c| ids.contains(&c.id) && c.start < at && at < c.end())
            .map(|c| c.id)
            .filter(|&c| !self.is_locked(c))
            .collect();
        for id in targets {
            let Some(index) = self.clips.iter().position(|c| c.id == id) else {
                continue;
            };
            let mut right = self.clips[index].clone();
            let left_len = at.saturating_sub(right.start);
            right.id = ClipId(self.new_id());
            right.start = at;
            right.duration = right.duration.saturating_sub(left_len);
            right.source_in = right.source_in.saturating_add(left_len);
            if let Some(old) = right.link {
                let new = if let Some((_, n)) = new_links.iter().find(|(o, _)| *o == old) {
                    *n
                } else {
                    let n = LinkId(self.new_id());
                    new_links.push((old, n));
                    n
                };
                right.link = Some(new);
            }
            self.clips[index].duration = left_len;
            created.push(right.id);
            self.clips.push(right);
        }
        self.tidy_links();
        created
    }

    /// Unlinks the groups of `ids` if any is linked; otherwise links `ids` together.
    /// Returns `true` if the clips are now linked.
    ///
    /// # Errors
    /// [`EditError::Nothing`] when there is nothing to link or unlink.
    pub fn toggle_link(&mut self, ids: &[ClipId]) -> Result<bool, EditError> {
        let links: Vec<LinkId> = ids.iter().filter_map(|&i| self.clip(i)?.link).collect();
        if !links.is_empty() {
            for c in &mut self.clips {
                if c.link.is_some_and(|l| links.contains(&l)) {
                    c.link = None;
                }
            }
            return Ok(false);
        }
        let existing: Vec<ClipId> = ids.iter().copied().filter(|&i| self.clip(i).is_some()).collect();
        if existing.len() < 2 {
            return Err(EditError::Nothing("Select two or more clips to link them."));
        }
        let link = LinkId(self.new_id());
        for c in &mut self.clips {
            if existing.contains(&c.id) {
                c.link = Some(link);
            }
        }
        Ok(true)
    }

    /// Sets how a clip's picture is placed.
    ///
    /// # Errors
    /// [`EditError::NoSuchClip`].
    pub fn set_fit(&mut self, id: ClipId, fit: FitMode) -> Result<(), EditError> {
        self.clip_mut(id).ok_or(EditError::NoSuchClip)?.fit = fit;
        Ok(())
    }

    // ----- markers -----

    /// Adds a marker.
    ///
    /// # Errors
    /// [`EditError::Nothing`] if a marker is already at that time or the name is empty.
    pub fn add_marker(&mut self, time: Time, name: &str, color: MarkerColor) -> Result<MarkerId, EditError> {
        if self.markers.iter().any(|m| m.time == time) {
            return Err(EditError::Nothing("There's already a marker here."));
        }
        let name = clean_name(name)?;
        let id = MarkerId(self.new_id());
        let at = self.markers.partition_point(|m| m.time < time);
        self.markers.insert(
            at,
            Marker {
                id,
                time: time.max(Time::ZERO),
                name,
                color,
            },
        );
        Ok(id)
    }

    /// Changes a marker's name and colour.
    ///
    /// # Errors
    /// [`EditError::Nothing`] if the marker is gone or the name is empty.
    pub fn update_marker(&mut self, id: MarkerId, name: &str, color: MarkerColor) -> Result<(), EditError> {
        let name = clean_name(name)?;
        let m = self
            .markers
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or(EditError::Nothing("That marker no longer exists."))?;
        m.name = name;
        m.color = color;
        Ok(())
    }

    /// Removes a marker.
    ///
    /// # Errors
    /// [`EditError::Nothing`] if it's already gone.
    pub fn remove_marker(&mut self, id: MarkerId) -> Result<(), EditError> {
        let i = self
            .markers
            .iter()
            .position(|m| m.id == id)
            .ok_or(EditError::Nothing("That marker no longer exists."))?;
        self.markers.remove(i);
        Ok(())
    }

    // ----- queries -----

    /// The picture clip seen at time `t`: the one on the visible video track nearest the top.
    #[must_use]
    pub fn top_picture_at(&self, t: Time) -> Option<&Clip> {
        self.tracks_of(TrackKind::Video)
            .filter(|track| !track.muted)
            .find_map(|track| self.clips.iter().find(|c| c.track == track.id && c.range().contains(t)))
    }

    /// Clips (on any track) under time `t`.
    pub fn clips_at(&self, t: Time) -> impl Iterator<Item = &Clip> {
        self.clips.iter().filter(move |c| c.range().contains(t))
    }

    /// Every clip start and end (and zero), sorted, without duplicates.
    #[must_use]
    pub fn edit_points(&self) -> Vec<Time> {
        let mut v: Vec<Time> = std::iter::once(Time::ZERO)
            .chain(self.clips.iter().flat_map(|c| [c.start, c.end()]))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Removes links that now join fewer than two clips.
    fn tidy_links(&mut self) {
        let mut counts: Vec<(LinkId, usize)> = Vec::new();
        for l in self.clips.iter().filter_map(|c| c.link) {
            match counts.iter_mut().find(|(k, _)| *k == l) {
                Some((_, n)) => *n += 1,
                None => counts.push((l, 1)),
            }
        }
        for c in &mut self.clips {
            if let Some(l) = c.link
                && counts.iter().any(|(k, n)| *k == l && *n < 2)
            {
                c.link = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaKind;
    use motix_probe::{AudioInfo, ColorInfo, Container, MediaInfo, VideoInfo};
    use std::path::PathBuf;

    fn secs(s: i64) -> Time {
        Time::from_seconds(s).unwrap()
    }

    fn media(id: u64, picture: bool, audio: usize, seconds: i64) -> MediaItem {
        let video = picture.then(|| VideoInfo {
            codec: "H.264".into(),
            coded_width: 3840,
            coded_height: 2160,
            rotation: 0,
            frame_rate: None,
            variable_frame_rate: false,
            bit_depth: Some(8),
            color: ColorInfo::default(),
            dolby_vision: None,
            hdr_metadata: false,
        });
        let audio = (0..audio)
            .map(|_| AudioInfo {
                codec: "AAC".into(),
                channels: 2,
                sample_rate: 48_000,
            })
            .collect();
        let ext = if picture { "mp4" } else { "wav" };
        MediaItem {
            id: MediaId(id),
            path: PathBuf::from(format!("m{id}.{ext}")),
            name: format!("m{id}.{ext}"),
            kind: if picture { MediaKind::Video } else { MediaKind::Audio },
            size_bytes: None,
            info: Some(MediaInfo {
                container: Container::Mp4,
                duration: Some(secs(seconds)),
                video,
                audio,
                still: false,
            }),
            probe_note: None,
        }
    }

    fn names(tl: &Timeline) -> Vec<String> {
        tl.tracks().iter().map(|t| t.name.clone()).collect()
    }

    #[test]
    fn tracks_follow_the_order_media_is_added_and_use_file_names() {
        let mut tl = Timeline::default();
        assert!(tl.tracks().is_empty(), "no fixed tracks");
        tl.place_media(&media(1, true, 1, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        tl.place_media(&media(2, false, 1, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        tl.place_media(&media(3, true, 1, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        assert_eq!(
            names(&tl),
            ["m1.mp4", "Audio of m1.mp4", "m2.wav", "m3.mp4", "Audio of m3.mp4"]
        );
        // Higher is in front.
        assert_eq!(tl.top_picture_at(secs(1)).unwrap().media, MediaId(1));
        let first = tl.tracks()[0].id;
        tl.set_track_muted(first, true).unwrap();
        assert_eq!(
            tl.top_picture_at(secs(1)).unwrap().media,
            MediaId(3),
            "hidden track skipped"
        );
    }

    #[test]
    fn several_audio_streams_get_numbered_tracks() {
        let mut tl = Timeline::default();
        let ids = tl
            .place_media(&media(1, true, 2, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(names(&tl), ["m1.mp4", "Audio 1 of m1.mp4", "Audio 2 of m1.mp4"]);
        assert_eq!(tl.linked_group(ids[1]).len(), 3);
        tl.place_media(&media(2, false, 2, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        assert_eq!(names(&tl)[3..], ["m2.wav (stream 1)", "m2.wav (stream 2)"]);
    }

    #[test]
    fn dropping_on_a_track_uses_it_and_the_ones_below() {
        let mut tl = Timeline::default();
        let a = tl
            .place_media(&media(1, true, 1, 4), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        let (vt, at) = (tl.clip(a[0]).unwrap().track, tl.clip(a[1]).unwrap().track);
        // Later on the same tracks: reuses them.
        let b = tl
            .place_media(&media(2, true, 1, 4), secs(4), Some(vt), FitMode::Fill)
            .unwrap();
        assert_eq!(tl.clip(b[0]).unwrap().track, vt);
        assert_eq!(tl.clip(b[1]).unwrap().track, at);
        assert_eq!(tl.clip(b[0]).unwrap().fit, FitMode::Fill);
        assert_eq!(tl.tracks().len(), 2);
        // Overlapping: new tracks inserted right below the drop target.
        tl.place_media(&media(3, true, 1, 4), secs(1), Some(vt), FitMode::Fit)
            .unwrap();
        assert_eq!(names(&tl), ["m1.mp4", "m3.mp4", "Audio of m3.mp4", "Audio of m1.mp4"]);
        // Audio-only dropped on a video track: gets its own track right below it.
        tl.place_media(&media(4, false, 1, 2), secs(20), Some(vt), FitMode::Fit)
            .unwrap();
        assert_eq!(names(&tl)[1], "m4.wav");
    }

    #[test]
    fn rename_reorder_lock() {
        let mut tl = Timeline::default();
        let ids = tl
            .place_media(&media(1, true, 1, 4), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        let v = tl.tracks()[0].id;
        tl.rename_track(v, "  Intro  ").unwrap();
        assert_eq!(tl.track_label(v), "Intro");
        assert!(tl.rename_track(v, "   ").is_err());
        tl.move_track(v, 1).unwrap();
        assert_eq!(names(&tl), ["Audio of m1.mp4", "Intro"]);
        assert!(tl.move_track(v, 1).is_err());
        tl.set_track_locked(v, true).unwrap();
        assert_eq!(tl.move_group(ids[1], secs(1), None), Err(EditError::Locked));
        assert_eq!(tl.delete(&ids), 1, "only the unlocked clip is deleted");
        assert_eq!(tl.remove_track(v), Err(EditError::Locked));
    }

    #[test]
    fn solo_and_mute() {
        let mut tl = Timeline::default();
        tl.place_media(&media(1, false, 1, 4), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        tl.place_media(&media(2, false, 1, 4), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        let (a, b) = (tl.tracks()[0].id, tl.tracks()[1].id);
        assert!(tl.is_audible(a) && tl.is_audible(b));
        tl.set_track_solo(b, true).unwrap();
        assert!(!tl.is_audible(a) && tl.is_audible(b));
        tl.set_track_muted(b, true).unwrap();
        assert!(!tl.is_audible(b));
    }

    #[test]
    fn linked_clips_move_together_and_unlink() {
        let mut tl = Timeline::default();
        let ids = tl
            .place_media(&media(1, true, 1, 4), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        tl.move_group(ids[1], secs(3), None).unwrap();
        assert_eq!(tl.clip(ids[0]).unwrap().start, secs(3));
        assert_eq!(tl.clip(ids[1]).unwrap().start, secs(3));
        tl.move_group(ids[0], Time::ZERO.saturating_sub(secs(100)), None)
            .unwrap();
        assert_eq!(tl.clip(ids[0]).unwrap().start, Time::ZERO, "clamped at zero");
        assert_eq!(tl.toggle_link(&[ids[0]]), Ok(false));
        tl.move_group(ids[1], secs(2), None).unwrap();
        assert_eq!(tl.clip(ids[0]).unwrap().start, Time::ZERO);
        assert_eq!(tl.clip(ids[1]).unwrap().start, secs(2));
        assert_eq!(tl.toggle_link(&ids), Ok(true));
        assert_eq!(tl.linked_group(ids[0]).len(), 2);
        // Moving just one clip of a linked pair (linked selection off).
        tl.move_clips(&[ids[0]], ids[0], secs(10), None).unwrap();
        assert_eq!(tl.clip(ids[1]).unwrap().start, secs(2));
    }

    #[test]
    fn moves_are_refused_when_blocked_or_wrong_kind() {
        let mut tl = Timeline::default();
        let v = tl.add_track(TrackKind::Video);
        let a = tl
            .place_media(&media(1, true, 0, 4), Time::ZERO, Some(v), FitMode::Fit)
            .unwrap()[0];
        let b = tl
            .place_media(&media(2, true, 0, 4), secs(4), Some(v), FitMode::Fit)
            .unwrap()[0];
        assert_eq!(tl.tracks().len(), 1);
        assert_eq!(
            tl.move_group(b, Time::ZERO.saturating_sub(secs(1)), None),
            Err(EditError::Occupied)
        );
        let audio_track = tl.add_track(TrackKind::Audio);
        assert_eq!(
            tl.move_group(a, Time::ZERO, Some(audio_track)),
            Err(EditError::WrongTrackKind)
        );
        let v2 = tl.add_track(TrackKind::Video);
        assert_eq!(tl.track_label(v2), "Video track 2");
        tl.move_group(b, Time::ZERO.saturating_sub(secs(1)), Some(v2)).unwrap();
        assert_eq!(tl.clip(b).unwrap().track, v2);
    }

    #[test]
    fn trimming_respects_media_length_neighbours_and_links() {
        let mut tl = Timeline::default();
        let frame = Time::from_flicks(motix_core::FLICKS_PER_SECOND / 30);
        let ids = tl
            .place_media(&media(1, true, 1, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        let group = tl.linked_group(ids[0]);
        // Trim the start in by 2 s: both linked clips follow.
        assert_eq!(tl.trim(&group, ids[0], Edge::Start, secs(2), frame), Ok(secs(2)));
        for &c in &ids {
            let c = tl.clip(c).unwrap();
            assert_eq!((c.start, c.duration, c.source_in), (secs(2), secs(8), secs(2)));
        }
        // Can't extend past the media's start or end.
        assert_eq!(
            tl.trim(&group, ids[0], Edge::Start, Time::ZERO.saturating_sub(secs(5)), frame),
            Ok(Time::ZERO.saturating_sub(secs(2)))
        );
        assert_eq!(tl.trim(&group, ids[0], Edge::End, secs(5), frame), Ok(Time::ZERO));
        // Shorten the end, then a neighbour limits how far it can grow back.
        tl.trim(&group, ids[0], Edge::End, Time::ZERO.saturating_sub(secs(4)), frame)
            .unwrap();
        assert_eq!(tl.clip(ids[0]).unwrap().end(), secs(6));
        let vt = tl.clip(ids[0]).unwrap().track;
        tl.place_media(&media(2, true, 0, 3), secs(7), Some(vt), FitMode::Fit)
            .unwrap();
        assert_eq!(tl.trim(&group, ids[0], Edge::End, secs(4), frame), Ok(secs(1)));
        // Never shorter than one frame.
        let d = tl.clip(ids[0]).unwrap().duration;
        tl.trim(&group, ids[0], Edge::End, Time::ZERO.saturating_sub(secs(100)), frame)
            .unwrap();
        assert_eq!(tl.clip(ids[0]).unwrap().duration, frame);
        assert!(d > frame);
    }

    #[test]
    fn split_keeps_right_halves_linked() {
        let mut tl = Timeline::default();
        let ids = tl
            .place_media(&media(1, true, 1, 10), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        let right = tl.split(&tl.linked_group(ids[0]), secs(4));
        assert_eq!(right.len(), 2);
        assert_eq!(tl.clip(ids[0]).unwrap().duration, secs(4));
        let r = tl.clip(right[0]).unwrap();
        assert_eq!((r.start, r.duration, r.source_in), (secs(4), secs(6), secs(4)));
        assert_ne!(r.link, tl.clip(ids[0]).unwrap().link);
        assert_eq!(tl.linked_group(right[0]).len(), 2);
        assert!(
            tl.split(&[ids[0]], Time::ZERO).is_empty(),
            "splitting at an edge does nothing"
        );
    }

    #[test]
    fn deleting_one_of_a_pair_leaves_the_other_unlinked() {
        let mut tl = Timeline::default();
        let ids = tl
            .place_media(&media(1, true, 1, 3), Time::ZERO, None, FitMode::Fit)
            .unwrap();
        assert_eq!(tl.delete(&[ids[0]]), 1);
        assert_eq!(tl.clip(ids[1]).unwrap().link, None);
        let track = tl.clip(ids[1]).unwrap().track;
        assert_eq!(tl.remove_track(track), Ok(1));
        assert!(tl.is_empty());
    }

    #[test]
    fn markers_and_edit_points() {
        let mut tl = Timeline::default();
        let m = tl.add_marker(secs(5), "Beat drop", MarkerColor::Red).unwrap();
        tl.add_marker(secs(2), "Intro", MarkerColor::Blue).unwrap();
        assert_eq!(tl.markers()[0].name, "Intro", "sorted by time");
        assert!(tl.add_marker(secs(2), "again", MarkerColor::Blue).is_err());
        tl.update_marker(m, "Drop", MarkerColor::Green).unwrap();
        assert_eq!(tl.markers()[1].color, MarkerColor::Green);
        tl.remove_marker(m).unwrap();
        assert_eq!(tl.markers().len(), 1);
        tl.place_media(&media(1, true, 1, 3), secs(1), None, FitMode::Fit)
            .unwrap();
        assert_eq!(tl.edit_points(), [Time::ZERO, secs(1), secs(4)]);
        assert_eq!(
            tl.place_media(&media(9, false, 0, 3), Time::ZERO, None, FitMode::Fit),
            Err(EditError::NothingToPlace)
        );
    }
}

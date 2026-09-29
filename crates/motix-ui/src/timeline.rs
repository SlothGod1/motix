//! The timeline panel, modelled on DaVinci Resolve's Edit page: a tool bar, a large
//! timecode above the track headers, a ruler with markers, and one row per track —
//! listed top to bottom in the order media was added and named after its file.
//!
//! Clips can be selected, dragged (with snapping), trimmed by their edges, and cut with
//! the blade tool. Media dragged from the Media panel drops where it's released.

use crate::{UiState, theme};
use motix_app::state::timecode;
use motix_app::{
    Action, AppState, ClipId, ClipSource, Edge, MarkerColor, MarkerId, MediaId, MediaKind, Tool, TrackId, TrackKind,
};
use motix_core::{Rational, Rounding, Time};

const HEADER_W: f32 = 250.0;
const TOOLBAR_H: f32 = 30.0;
const RULER_H: f32 = 30.0;
const TRACK_GAP: f32 = 2.0;
/// Pointer distance within which clip edges snap to the playhead, markers and other clips.
const SNAP_PX: f32 = 8.0;
/// How close to a clip's edge the pointer must be to trim instead of move.
const EDGE_PX: f32 = 6.0;
/// Default and allowed track heights.
pub(crate) const TRACK_H_DEFAULT: f32 = 44.0;
const TRACK_H_RANGE: std::ops::RangeInclusive<f32> = 28.0..=96.0;

/// A clip being moved.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClipDrag {
    clip: ClipId,
    grab_x: f32,
    original_start: Time,
}

/// A clip edge being trimmed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TrimDrag {
    clip: ClipId,
    edge: Edge,
    grab_x: f32,
    limits: (Time, Time),
}

/// Timeline-only UI state.
pub(crate) struct TimelineView {
    pub px_per_second: f32,
    pub scroll_seconds: f32,
    pub track_scroll: f32,
    pub track_height: f32,
    pub clip_drag: Option<ClipDrag>,
    pub trim_drag: Option<TrimDrag>,
    pub renaming: Option<(TrackId, String, bool)>,
    pub marker_edit: Option<(MarkerId, String)>,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            px_per_second: 60.0,
            scroll_seconds: 0.0,
            track_scroll: 0.0,
            track_height: TRACK_H_DEFAULT,
            clip_drag: None,
            trim_drag: None,
            renaming: None,
            marker_edit: None,
        }
    }
}

/// Seconds between labelled ruler ticks so that labels are at least ~90 px apart.
pub(crate) fn tick_step_seconds(px_per_second: f32) -> f32 {
    const STEPS: [f32; 10] = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 300.0, 600.0];
    STEPS
        .iter()
        .copied()
        .find(|s| s * px_per_second >= 90.0)
        .unwrap_or(600.0)
}

fn time_from_seconds(seconds: f32) -> Time {
    let ms = (f64::from(seconds.max(0.0)) * 1000.0).round() as i64;
    Time::from_seconds_rational(Rational::new(ms, 1000).unwrap_or(Rational::ZERO), Rounding::Nearest)
        .unwrap_or(Time::ZERO)
}

fn signed_seconds_to_time(seconds: f32) -> Time {
    let t = time_from_seconds(seconds.abs());
    if seconds < 0.0 { Time::ZERO.saturating_sub(t) } else { t }
}

fn seconds(t: Time) -> f32 {
    t.as_seconds_f64() as f32
}

fn clip_colors(kind: TrackKind, media: MediaKind) -> (egui::Color32, egui::Color32) {
    match (kind, media) {
        (TrackKind::Audio, _) => (theme::CLIP_AUDIO, theme::CLIP_AUDIO_BAR),
        (TrackKind::Video, MediaKind::Image) => (theme::CLIP_IMAGE, theme::CLIP_IMAGE_BAR),
        (TrackKind::Video, _) => (theme::CLIP_VIDEO, theme::CLIP_VIDEO_BAR),
    }
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState, actions: &mut Vec<Action>) {
    let view = &mut ui_state.timeline;
    toolbar(ui, state, view, actions);

    let full = ui.available_rect_before_wrap();
    let lanes_left = full.min.x + HEADER_W;
    let ruler = egui::Rect::from_min_max(
        egui::pos2(lanes_left, full.min.y),
        egui::pos2(full.max.x, full.min.y + RULER_H),
    );
    let tc_box = egui::Rect::from_min_max(full.min, egui::pos2(lanes_left - 4.0, ruler.max.y));
    let tracks_area = egui::Rect::from_min_max(egui::pos2(full.min.x, ruler.max.y + 3.0), full.max);
    let lanes_area = egui::Rect::from_min_max(egui::pos2(lanes_left, tracks_area.min.y), tracks_area.max);
    let track_h = view.track_height;

    let background = ui.interact(full, ui.id().with("timeline_bg"), egui::Sense::click_and_drag());
    let painter = ui.painter_at(full);
    let pps = view.px_per_second;

    let rows: Vec<(TrackId, f32, TrackKind)> = state
        .timeline
        .tracks()
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id, i as f32 * (track_h + TRACK_GAP), t.kind))
        .collect();
    let content_h = rows.last().map_or(0.0, |r| r.1 + track_h);
    let max_scroll = (content_h - tracks_area.height() + 8.0).max(0.0);

    // Scrolling: wheel pans time; over the track headers (or with Alt) it scrolls tracks;
    // Ctrl+wheel zooms around the pointer.
    if background.contains_pointer() {
        let (scroll, zoom, alt, pointer) = ui.input(|i| {
            (
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.modifiers.alt,
                i.pointer.hover_pos(),
            )
        });
        let over_headers = pointer.is_some_and(|p| p.x < lanes_left);
        if (zoom - 1.0).abs() > f32::EPSILON {
            zoom_around(view, zoom, pointer.map_or(lanes_left, |p| p.x), lanes_left);
        } else if over_headers || alt {
            view.track_scroll -= scroll.y + scroll.x;
        } else {
            let delta = if scroll.x.abs() > scroll.y.abs() {
                scroll.x
            } else {
                scroll.y
            };
            view.scroll_seconds = (view.scroll_seconds - delta / pps).max(0.0);
        }
    }
    let mut scrollbar = None;
    if max_scroll > 0.0 {
        let track_rect = egui::Rect::from_min_max(
            egui::pos2(full.max.x - 7.0, tracks_area.min.y),
            egui::pos2(full.max.x - 1.0, tracks_area.max.y),
        );
        let visible = tracks_area.height() / (content_h + 8.0);
        let thumb_h = (track_rect.height() * visible).max(24.0);
        let travel = (track_rect.height() - thumb_h).max(1.0);
        let thumb = egui::Rect::from_min_size(
            egui::pos2(
                track_rect.min.x,
                track_rect.min.y + travel * (view.track_scroll / max_scroll),
            ),
            egui::vec2(track_rect.width(), thumb_h),
        );
        let bar = ui.interact(track_rect, ui.id().with("track_scrollbar"), egui::Sense::drag());
        if bar.dragged() {
            view.track_scroll += bar.drag_delta().y * max_scroll / travel;
        }
        let active = bar.hovered() || bar.dragged();
        bar.on_hover_text("Scroll tracks (or scroll over the track names)");
        scrollbar = Some((track_rect, thumb, active));
    }
    view.track_scroll = view.track_scroll.clamp(0.0, max_scroll);
    let scroll_s = view.scroll_seconds;
    let track_scroll = view.track_scroll;
    let x_of = |s: f32| lanes_left + (s - scroll_s) * pps;
    let time_at_x = |x: f32| time_from_seconds(scroll_s + (x - lanes_left).max(0.0) / pps);
    let track_top = |rel: f32| tracks_area.min.y + rel - track_scroll;
    let track_at_y = |y: f32| -> Option<(TrackId, TrackKind)> {
        rows.iter()
            .find(|&&(_, rel, _)| {
                let top = track_top(rel);
                y >= top && y < top + track_h + TRACK_GAP
            })
            .map(|&(id, _, k)| (id, k))
    };

    // Big timecode above the headers, like Resolve.
    painter.rect_filled(tc_box, 4.0, theme::RAISED);
    painter.text(
        tc_box.left_center() + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        state.timecode(),
        egui::FontId::monospace(19.0),
        theme::PLAYHEAD,
    );
    draw_ruler(&painter, ruler, state, scroll_s, pps);
    markers_on_ruler(ui, state, view, ruler, scroll_s, pps);

    // Track headers and lanes.
    let clip_area_painter = painter.with_clip_rect(tracks_area);
    for &(id, rel, kind) in &rows {
        let top = track_top(rel);
        if top + track_h < tracks_area.min.y || top > tracks_area.max.y {
            continue;
        }
        let lane = egui::Rect::from_min_max(egui::pos2(lanes_left, top), egui::pos2(full.max.x, top + track_h));
        clip_area_painter.rect_filled(lane, 2.0, theme::LANE);
        track_header(
            ui,
            state,
            view,
            actions,
            id,
            kind,
            top,
            full.min.x,
            track_h,
            tracks_area,
        );
    }

    // Empty timeline hint.
    if state.timeline.is_empty() {
        let top = rows
            .last()
            .map_or(lanes_area.min.y + 8.0, |r| track_top(r.1) + track_h + 8.0);
        let centre = egui::pos2(lanes_area.center().x, top.max(lanes_area.min.y) + 24.0);
        if centre.y < full.max.y - 10.0 {
            painter.text(
                centre,
                egui::Align2::CENTER_CENTER,
                "Drag media here, or double-click it in the Media panel",
                egui::FontId::proportional(14.0),
                theme::TEXT,
            );
            painter.text(
                centre + egui::vec2(0.0, 20.0),
                egui::Align2::CENTER_CENTER,
                "Each file gets its own tracks, named after the file — as many as you need.",
                egui::FontId::proportional(12.0),
                theme::TEXT_WEAK,
            );
        }
    }

    // Clips.
    let modifiers = ui.input(|i| i.modifiers);
    let pointer_now = ui.input(|i| i.pointer.hover_pos());
    let clip_painter = painter.with_clip_rect(lanes_area);
    let moving_group: Vec<ClipId> = view.clip_drag.map(|d| state.edit_group(d.clip)).unwrap_or_default();
    let mut clicked_clip = None;
    let mut blade_hover: Option<Time> = None;
    let clips: Vec<_> = state.timeline.clips().to_vec();
    for clip in &clips {
        let Some(&(_, rel, kind)) = rows.iter().find(|r| r.0 == clip.track) else {
            continue;
        };
        let top = track_top(rel);
        let x0 = x_of(seconds(clip.start));
        let x1 = x_of(seconds(clip.end())).max(x0 + 3.0);
        let rect = egui::Rect::from_min_max(egui::pos2(x0, top + 1.0), egui::pos2(x1, top + track_h - 1.0));
        if rect.max.x < lanes_left
            || rect.min.x > full.max.x
            || rect.max.y < tracks_area.min.y
            || rect.min.y > full.max.y
        {
            continue;
        }
        let visible = rect.intersect(lanes_area);
        let response = ui.interact(
            visible,
            ui.id().with(("clip", clip.id.0)),
            egui::Sense::click_and_drag(),
        );
        let media_kind = state.media.get(clip.media).map_or(MediaKind::Video, |m| m.kind);
        let (body, band) = clip_colors(kind, media_kind);
        let track = state.timeline.track(clip.track);
        let dim = track.is_some_and(|t| t.muted) || moving_group.contains(&clip.id);
        let locked = track.is_some_and(|t| t.locked);
        let selected = state.selected_clips.contains(&clip.id);
        let alpha = if dim { 0.35 } else { 1.0 };
        clip_painter.rect_filled(rect, 3.0, body.linear_multiply(alpha));
        let band_h = 15.0_f32.min(rect.height() * 0.45);
        let band_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), band_h));
        clip_painter.rect_filled(
            band_rect,
            egui::CornerRadius {
                nw: 3,
                ne: 3,
                sw: 0,
                se: 0,
            },
            band.linear_multiply(alpha),
        );
        if kind == TrackKind::Audio {
            // A calm centre line (the real waveform arrives with audio decoding).
            let y = f32::midpoint(band_rect.max.y, rect.max.y);
            clip_painter.line_segment(
                [egui::pos2(rect.min.x + 4.0, y), egui::pos2(rect.max.x - 4.0, y)],
                egui::Stroke::new(1.0, band.linear_multiply(alpha)),
            );
        }
        if locked {
            // Diagonal hatching on locked tracks, like Resolve.
            let hatch = painter.with_clip_rect(rect.intersect(lanes_area));
            let mut x = rect.min.x - rect.height();
            while x < rect.max.x {
                hatch.line_segment(
                    [egui::pos2(x, rect.max.y), egui::pos2(x + rect.height(), rect.min.y)],
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(70)),
                );
                x += 8.0;
            }
        }
        clip_painter.rect_stroke(
            rect,
            3.0,
            if selected {
                egui::Stroke::new(2.0, theme::SELECTED)
            } else {
                egui::Stroke::new(1.0, egui::Color32::from_black_alpha(140))
            },
            egui::StrokeKind::Inside,
        );
        let text_clip = painter.with_clip_rect(rect.shrink(2.0).intersect(lanes_area));
        let name = match clip.source {
            ClipSource::Audio(n) if state.media.get(clip.media).is_some_and(|m| m.audio_streams() > 1) => {
                format!("{}  (audio {})", clip.name, n + 1)
            }
            _ => clip.name.clone(),
        };
        let text_x = rect.min.x.max(lanes_left) + 5.0;
        let galley = text_clip.layout_no_wrap(name.clone(), egui::FontId::proportional(11.0), theme::TEXT);
        let text_pos = egui::pos2(text_x, band_rect.center().y - galley.size().y / 2.0);
        let text_w = galley.size().x;
        text_clip.galley(text_pos, galley, theme::TEXT);
        if clip.link.is_some() {
            // Resolve underlines the names of linked clips.
            text_clip.line_segment(
                [
                    egui::pos2(text_x, band_rect.max.y - 1.5),
                    egui::pos2(text_x + text_w, band_rect.max.y - 1.5),
                ],
                egui::Stroke::new(1.0, theme::TEXT.linear_multiply(0.7)),
            );
        }
        if rect.height() > 30.0
            && let Some(size) = clip.source_size
            && kind == TrackKind::Video
            && size != state.project.resolution
        {
            text_clip.text(
                egui::pos2(text_x, rect.max.y - 3.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{size} · {}", clip.fit.label().to_lowercase()),
                egui::FontId::proportional(10.0),
                theme::TEXT.linear_multiply(0.75),
            );
        }
        let described = format!(
            "{} clip {name} at {}, {} long{}{}",
            kind.label(),
            timecode(clip.start, state.project.frame_rate),
            timecode(clip.duration, state.project.frame_rate),
            if clip.link.is_some() { ", linked" } else { "" },
            if locked { ", locked" } else { "" }
        );
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &described));

        // What the pointer would do here.
        let near_edge = pointer_now.and_then(|p| {
            if !rect.contains(p) || locked || state.tool != Tool::Select || rect.width() < EDGE_PX * 3.0 {
                None
            } else if (p.x - rect.min.x).abs() <= EDGE_PX {
                Some(Edge::Start)
            } else if (rect.max.x - p.x).abs() <= EDGE_PX {
                Some(Edge::End)
            } else {
                None
            }
        });
        let response = if state.tool == Tool::Blade && !locked {
            if response.hovered()
                && let Some(p) = pointer_now
            {
                blade_hover = Some(state.snap_to_frame(time_at_x(p.x)));
            }
            response.on_hover_cursor(egui::CursorIcon::Crosshair)
        } else if near_edge.is_some() {
            response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        } else {
            response
        };

        if response.drag_started()
            && state.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            if let Some(edge) = near_edge {
                if !selected {
                    state.select_clip(clip.id, false);
                }
                match state.trim_limits(clip.id, edge) {
                    Ok(limits) => {
                        view.trim_drag = Some(TrimDrag {
                            clip: clip.id,
                            edge,
                            grab_x: p.x,
                            limits,
                        });
                    }
                    Err(e) => state.status = e.to_string(),
                }
            } else if locked {
                state.status = motix_app::EditError::Locked.to_string();
            } else {
                if !selected {
                    state.select_clip(clip.id, false);
                }
                view.clip_drag = Some(ClipDrag {
                    clip: clip.id,
                    grab_x: p.x,
                    original_start: clip.start,
                });
            }
        }
        if response.clicked() {
            if state.tool == Tool::Blade {
                if let Some(p) = response.interact_pointer_pos() {
                    state.blade(clip.id, time_at_x(p.x));
                }
            } else {
                clicked_clip = Some(clip.id);
            }
        }
        response.on_hover_text(&described).context_menu(|ui| {
            if !state.selected_clips.contains(&clip.id) {
                state.select_clip(clip.id, false);
            }
            let linked = clip.link.is_some();
            if ui
                .button(if linked {
                    "Unlink audio and video"
                } else {
                    "Link selected clips"
                })
                .clicked()
            {
                actions.push(Action::ToggleLink);
                ui.close();
            }
            if ui.button("Split at playhead").clicked() {
                actions.push(Action::Split);
                ui.close();
            }
            if ui.button("Delete").clicked() {
                actions.push(Action::Delete);
                ui.close();
            }
        });
    }
    if let Some(id) = clicked_clip {
        state.select_clip(id, modifiers.command || modifiers.shift);
    }
    if let Some(t) = blade_hover {
        let x = x_of(seconds(t));
        clip_painter.line_segment(
            [egui::pos2(x, lanes_area.min.y), egui::pos2(x, lanes_area.max.y)],
            egui::Stroke::new(1.0, theme::SELECTED),
        );
    }

    // Snap targets: zero, the playhead, markers and clip edges (not those being edited).
    let snap = |state: &AppState, t: Time, extra_edges: &[Time], exclude: &[ClipId]| -> Option<(Time, Time)> {
        if !state.snapping {
            return None;
        }
        let mut best: Option<(f32, Time, Time)> = None;
        let targets = std::iter::once(Time::ZERO)
            .chain(std::iter::once(state.playhead))
            .chain(state.timeline.markers().iter().map(|m| m.time))
            .chain(
                state
                    .timeline
                    .clips()
                    .iter()
                    .filter(|c| !exclude.contains(&c.id))
                    .flat_map(|c| [c.start, c.end()]),
            );
        for target in targets {
            for &edge in std::iter::once(&t).chain(extra_edges) {
                let d = (x_of(seconds(target)) - x_of(seconds(edge))).abs();
                if d < SNAP_PX && best.is_none_or(|b| d < b.0) {
                    best = Some((d, target, edge));
                }
            }
        }
        best.map(|(_, target, edge)| (target, edge))
    };

    // Moving clips: ghost preview while dragging, commit on release.
    if let Some(drag) = view.clip_drag {
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let released = ui.input(|i| i.pointer.any_released());
        if let (Some(p), Some(clip)) = (pointer, state.timeline.clip(drag.clip).cloned()) {
            let raw = seconds(drag.original_start) + (p.x - drag.grab_x) / pps;
            let mut start = state.snap_to_frame(time_from_seconds(raw));
            let group = state.edit_group(drag.clip);
            let end = start.saturating_add(clip.duration);
            if let Some((target, edge)) = snap(state, start, &[end], &group) {
                start = if edge == start {
                    target
                } else {
                    target.saturating_sub(clip.duration)
                }
                .max(Time::ZERO);
            }
            let own_kind = state.timeline.track(clip.track).map(|t| t.kind);
            let target_track = track_at_y(p.y).filter(|(_, k)| Some(*k) == own_kind).map(|(id, _)| id);
            let offset = start.saturating_sub(clip.start);
            let result = state
                .timeline
                .can_move(&group, drag.clip, offset, target_track.filter(|t| *t != clip.track));
            let ok = result.is_ok();
            for c in state.timeline.clips().iter().filter(|c| group.contains(&c.id)) {
                let track = if c.id == drag.clip {
                    target_track.unwrap_or(c.track)
                } else {
                    c.track
                };
                let Some(&(_, rel, _)) = rows.iter().find(|r| r.0 == track) else {
                    continue;
                };
                let top = track_top(rel);
                let s0 = c.start.saturating_add(offset);
                let gx0 = x_of(seconds(s0));
                let gx1 = x_of(seconds(s0.saturating_add(c.duration))).max(gx0 + 3.0);
                let r = egui::Rect::from_min_max(egui::pos2(gx0, top + 1.0), egui::pos2(gx1, top + track_h - 1.0));
                let color = if ok { theme::SELECTED } else { theme::PLAYHEAD };
                clip_painter.rect_filled(r, 3.0, color.linear_multiply(0.15));
                clip_painter.rect_stroke(r, 3.0, egui::Stroke::new(1.5, color), egui::StrokeKind::Inside);
            }
            clip_painter.text(
                egui::pos2(x_of(seconds(start)) + 4.0, lanes_area.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                match &result {
                    Ok(_) => timecode(start, state.project.frame_rate),
                    Err(e) => e.to_string(),
                },
                egui::FontId::monospace(10.5),
                if ok { theme::TEXT } else { theme::PLAYHEAD },
            );
            if released {
                view.clip_drag = None;
                if ok {
                    let _ = state.move_clip(drag.clip, start, target_track);
                } else if let Err(e) = result {
                    state.status = format!("{e} The clip went back where it was.");
                }
            }
        } else {
            view.clip_drag = None;
        }
    }

    // Trimming: preview the new edge, commit on release.
    if let Some(trim) = view.trim_drag {
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let released = ui.input(|i| i.pointer.any_released());
        if let (Some(p), Some(clip)) = (pointer, state.timeline.clip(trim.clip).cloned()) {
            let edge_time = if trim.edge == Edge::Start {
                clip.start
            } else {
                clip.end()
            };
            let raw = signed_seconds_to_time((p.x - trim.grab_x) / pps);
            let mut new_edge = state.snap_to_frame(edge_time.saturating_add(raw).max(Time::ZERO));
            if let Some((target, _)) = snap(state, new_edge, &[], &state.edit_group(trim.clip)) {
                new_edge = target;
            }
            let delta = new_edge.saturating_sub(edge_time).clamp(trim.limits.0, trim.limits.1);
            let shown = edge_time.saturating_add(delta);
            if let Some(&(_, rel, _)) = rows.iter().find(|r| r.0 == clip.track) {
                let top = track_top(rel);
                let (a, b) = if trim.edge == Edge::Start {
                    (shown, clip.end())
                } else {
                    (clip.start, shown)
                };
                let r = egui::Rect::from_min_max(
                    egui::pos2(x_of(seconds(a)), top + 1.0),
                    egui::pos2(x_of(seconds(b)).max(x_of(seconds(a)) + 2.0), top + track_h - 1.0),
                );
                clip_painter.rect_stroke(
                    r,
                    3.0,
                    egui::Stroke::new(2.0, theme::SELECTED),
                    egui::StrokeKind::Inside,
                );
                let x = x_of(seconds(shown));
                clip_painter.line_segment(
                    [egui::pos2(x, lanes_area.min.y), egui::pos2(x, lanes_area.max.y)],
                    egui::Stroke::new(1.0, theme::SELECTED),
                );
                let sign = if delta < Time::ZERO { "-" } else { "+" };
                let magnitude = delta.max(Time::ZERO.saturating_sub(delta));
                clip_painter.text(
                    egui::pos2(x + 4.0, top - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{sign}{}", timecode(magnitude, state.project.frame_rate)),
                    egui::FontId::monospace(10.5),
                    theme::TEXT,
                );
            }
            if released {
                view.trim_drag = None;
                let _ = state.trim_clip(trim.clip, trim.edge, delta);
            }
        } else {
            view.trim_drag = None;
        }
    }

    // Media dragged from the Media panel.
    if background.dnd_hover_payload::<MediaId>().is_some()
        && let Some(p) = ui.input(|i| i.pointer.hover_pos())
    {
        let x = x_of(seconds(state.snap_to_frame(time_at_x(p.x))));
        painter.line_segment(
            [egui::pos2(x, lanes_area.min.y), egui::pos2(x, lanes_area.max.y)],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
        let hint = match track_at_y(p.y) {
            Some((track, _)) => {
                if let Some(&(_, rel, _)) = rows.iter().find(|r| r.0 == track) {
                    let top = track_top(rel);
                    painter.rect_stroke(
                        egui::Rect::from_min_max(egui::pos2(lanes_left, top), egui::pos2(full.max.x, top + track_h)),
                        2.0,
                        egui::Stroke::new(1.5, theme::ACCENT),
                        egui::StrokeKind::Inside,
                    );
                }
                format!("Drop on {}", state.timeline.track_label(track))
            }
            None => "Drop here for new tracks".to_owned(),
        };
        painter.text(
            egui::pos2(x + 6.0, lanes_area.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            hint,
            egui::FontId::proportional(11.5),
            theme::ACCENT,
        );
    }
    if let Some(id) = background.dnd_release_payload::<MediaId>()
        && let Some(p) = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()))
    {
        let track = track_at_y(p.y).map(|(id, _)| id);
        state.selected_media = Some(*id);
        let _ = state.add_to_timeline(*id, Some(time_at_x(p.x)), track);
    }

    // Click or drag on empty space (or the ruler) moves the playhead and clears the selection.
    if view.clip_drag.is_none()
        && view.trim_drag.is_none()
        && let Some(pos) = background.interact_pointer_pos()
        && (background.clicked() || background.dragged())
        && pos.x >= lanes_left
    {
        if background.clicked() && !modifiers.command && !modifiers.shift {
            state.selected_clips.clear();
        }
        state.playhead = state.snap_to_frame(time_at_x(pos.x));
    }

    // Keep the playhead visible while playing.
    let playhead_s = seconds(state.playhead);
    let visible_s = (full.max.x - lanes_left) / pps;
    if state.playing && playhead_s > view.scroll_seconds + visible_s * 0.9 {
        view.scroll_seconds = playhead_s - visible_s * 0.1;
    }

    if let Some((track_rect, thumb, active)) = scrollbar {
        painter.rect_filled(track_rect, 3.0, theme::RAISED);
        painter.rect_filled(thumb, 3.0, if active { theme::TEXT } else { theme::TEXT_WEAK });
    }

    // Playhead: a red line with a handle in the ruler.
    let x = lanes_left + (playhead_s - view.scroll_seconds) * pps;
    if x >= lanes_left && x <= full.max.x {
        painter.line_segment(
            [egui::pos2(x, ruler.min.y + 10.0), egui::pos2(x, full.max.y)],
            egui::Stroke::new(1.5, theme::PLAYHEAD),
        );
        let head = [
            egui::pos2(x - 6.0, ruler.min.y + 2.0),
            egui::pos2(x + 6.0, ruler.min.y + 2.0),
            egui::pos2(x + 6.0, ruler.min.y + 10.0),
            egui::pos2(x, ruler.min.y + 16.0),
            egui::pos2(x - 6.0, ruler.min.y + 10.0),
        ];
        painter.add(egui::Shape::convex_polygon(
            head.to_vec(),
            theme::PLAYHEAD,
            egui::Stroke::NONE,
        ));
    }
}

/// A button in a track header.
#[derive(Clone, Copy)]
enum Toggle {
    Solo,
    Mute,
    Hide,
    Lock,
}

#[allow(clippy::too_many_arguments)]
fn track_header(
    ui: &mut egui::Ui,
    state: &mut AppState,
    view: &mut TimelineView,
    actions: &mut Vec<Action>,
    id: TrackId,
    kind: TrackKind,
    top: f32,
    left: f32,
    track_h: f32,
    tracks_area: egui::Rect,
) {
    let Some(track) = state.timeline.track(id).cloned() else {
        return;
    };
    let header = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(HEADER_W - 6.0, track_h));
    let painter = ui.painter().with_clip_rect(tracks_area);
    let accent = if kind == TrackKind::Video {
        theme::VIDEO
    } else {
        theme::AUDIO
    };
    painter.rect_filled(header, 3.0, theme::RAISED);
    painter.rect_filled(
        egui::Rect::from_min_size(header.min, egui::vec2(3.0, track_h)),
        1.0,
        accent,
    );
    let badge_text = if kind == TrackKind::Video { "🎬" } else { "🎵" };
    let badge = egui::Rect::from_min_size(header.min + egui::vec2(8.0, 5.0), egui::vec2(22.0, 18.0));
    painter.rect_filled(badge, 3.0, accent.linear_multiply(0.35));
    painter.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        badge_text,
        egui::FontId::proportional(12.0),
        theme::TEXT,
    );

    // Buttons on the right: lock, and eye (video) or M/S (audio).
    let button_w = 22.0;
    let mut bx = header.max.x - 4.0;
    let mut place = |w: f32| {
        bx -= w;
        let r = egui::Rect::from_min_size(egui::pos2(bx, header.min.y + 4.0), egui::vec2(w - 2.0, 20.0));
        bx -= 1.0;
        r
    };
    let visible = |r: egui::Rect| tracks_area.contains_rect(r);
    let mut toggles: Vec<(egui::Rect, &str, bool, &str, Toggle)> = Vec::new();
    if kind == TrackKind::Audio {
        toggles.push((
            place(button_w),
            "S",
            track.solo,
            "Solo: hear only soloed tracks",
            Toggle::Solo,
        ));
        toggles.push((place(button_w), "M", track.muted, "Mute this track", Toggle::Mute));
    } else {
        let tip = if track.muted {
            "Show this track (hidden now)"
        } else {
            "Hide this track in the viewer and export"
        };
        // Lit while the track is visible, like Resolve's eye.
        toggles.push((place(button_w), "👁", !track.muted, tip, Toggle::Hide));
    }
    let lock_tip = if track.locked {
        "Unlock this track"
    } else {
        "Lock this track so its clips can't be changed"
    };
    toggles.push((
        place(button_w),
        if track.locked { "🔒" } else { "🔓" },
        track.locked,
        lock_tip,
        Toggle::Lock,
    ));
    let name_right = bx - 2.0;
    for (rect, text, on, tip, which) in toggles {
        if !visible(rect) {
            continue;
        }
        let size = if text.len() > 1 { 14.0 } else { 11.5 };
        let button = egui::Button::new(egui::RichText::new(text).size(size))
            .selected(on)
            .small()
            .min_size(rect.size());
        let described = format!("{tip} ({})", track.name);
        let r = ui.put(rect, button);
        r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, &described));
        if r.on_hover_text(tip).clicked() {
            match which {
                Toggle::Solo => state.set_track_solo(id, !track.solo),
                Toggle::Mute | Toggle::Hide => state.set_track_muted(id, !track.muted),
                Toggle::Lock => state.set_track_locked(id, !track.locked),
            }
        }
    }

    // Name (double-click to rename).
    let name_rect = egui::Rect::from_min_max(
        egui::pos2(badge.max.x + 6.0, header.min.y + 2.0),
        egui::pos2(name_right, header.min.y + 26.0),
    );
    let renaming_this = view.renaming.as_ref().is_some_and(|(t, _, _)| *t == id);
    if renaming_this && visible(name_rect) {
        let mut commit = None;
        if let Some((_, text, focus)) = view.renaming.as_mut() {
            let edit = ui.put(
                name_rect,
                egui::TextEdit::singleline(text).font(egui::FontId::proportional(12.0)),
            );
            edit.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Track name"));
            if *focus {
                edit.request_focus();
                *focus = false;
            }
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if escape {
                commit = Some(None);
            } else if edit.lost_focus() {
                commit = Some(Some(text.clone()));
            }
        }
        if let Some(result) = commit {
            view.renaming = None;
            if let Some(new_name) = result
                && new_name.trim() != track.name
            {
                state.rename_track(id, &new_name);
            }
        }
    } else {
        let text_painter = painter.with_clip_rect(name_rect.intersect(tracks_area));
        text_painter.text(
            name_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            &track.name,
            egui::FontId::proportional(12.0),
            if track.muted { theme::TEXT_WEAK } else { theme::TEXT },
        );
    }
    if track_h >= 40.0 {
        let clips = state.timeline.clips().iter().filter(|c| c.track == id).count();
        painter.text(
            egui::pos2(badge.min.x, header.max.y - 5.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{} · {clips} clip{}", kind.label(), if clips == 1 { "" } else { "s" }),
            egui::FontId::proportional(10.0),
            theme::TEXT_WEAK,
        );
    }

    let response = ui.interact(
        egui::Rect::from_min_max(header.min, egui::pos2(name_right, header.max.y)).intersect(tracks_area),
        ui.id().with(("track_header", id.0)),
        egui::Sense::click(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &track.name));
    if response.double_clicked() {
        view.renaming = Some((id, track.name.clone(), true));
    }
    response
        .on_hover_text(format!("{}\nDouble-click to rename · right-click for more", track.name))
        .context_menu(|ui| {
            if ui.button("Rename…").clicked() {
                view.renaming = Some((id, track.name.clone(), true));
                ui.close();
            }
            if ui.button("Move up (in front)").clicked() {
                state.move_track(id, -1);
                ui.close();
            }
            if ui.button("Move down (behind)").clicked() {
                state.move_track(id, 1);
                ui.close();
            }
            if ui
                .button(if track.locked { "Unlock track" } else { "Lock track" })
                .clicked()
            {
                state.set_track_locked(id, !track.locked);
                ui.close();
            }
            ui.separator();
            if ui.button("Add video track").clicked() {
                actions.push(Action::AddVideoTrack);
                ui.close();
            }
            if ui.button("Add audio track").clicked() {
                actions.push(Action::AddAudioTrack);
                ui.close();
            }
            ui.separator();
            if ui
                .button(format!("Delete track \u{201c}{}\u{201d} and its clips", track.name))
                .clicked()
            {
                state.remove_track(id);
                ui.close();
            }
        });
}

fn markers_on_ruler(
    ui: &mut egui::Ui,
    state: &mut AppState,
    view: &mut TimelineView,
    ruler: egui::Rect,
    scroll_s: f32,
    pps: f32,
) {
    let markers = state.timeline.markers().to_vec();
    for m in markers {
        let x = ruler.min.x + (seconds(m.time) - scroll_s) * pps;
        if x < ruler.min.x - 6.0 || x > ruler.max.x + 6.0 {
            continue;
        }
        let color = theme::marker(m.color);
        let r = egui::Rect::from_min_size(egui::pos2(x - 5.0, ruler.max.y - 13.0), egui::vec2(10.0, 12.0));
        let flag = [
            egui::pos2(r.min.x, r.min.y),
            egui::pos2(r.max.x, r.min.y),
            egui::pos2(r.max.x, r.max.y - 4.0),
            egui::pos2(x, r.max.y),
            egui::pos2(r.min.x, r.max.y - 4.0),
        ];
        ui.painter()
            .with_clip_rect(ruler)
            .add(egui::Shape::convex_polygon(flag.to_vec(), color, egui::Stroke::NONE));
        let response = ui.interact(r, ui.id().with(("marker", m.id.0)), egui::Sense::click());
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Marker {}", m.name)));
        if response.clicked() {
            state.playhead = m.time;
        }
        let editing = view.marker_edit.as_ref().is_some_and(|(id, _)| *id == m.id);
        response
            .on_hover_text(format!("{} · {}", m.name, timecode(m.time, state.project.frame_rate)))
            .context_menu(|ui| {
                if !editing {
                    view.marker_edit = Some((m.id, m.name.clone()));
                }
                if let Some((_, text)) = view.marker_edit.as_mut() {
                    ui.label("Name");
                    let edit = ui.text_edit_singleline(text);
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let name = text.clone();
                        state.update_marker(m.id, &name, m.color);
                        view.marker_edit = None;
                        ui.close();
                    }
                }
                ui.horizontal(|ui| {
                    for c in MarkerColor::ALL {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                        ui.painter().rect_filled(rect, 3.0, theme::marker(c));
                        if c == m.color {
                            ui.painter().rect_stroke(
                                rect,
                                3.0,
                                egui::Stroke::new(2.0, theme::TEXT),
                                egui::StrokeKind::Outside,
                            );
                        }
                        if resp.on_hover_text(c.label()).clicked() {
                            let name = view
                                .marker_edit
                                .as_ref()
                                .map_or_else(|| m.name.clone(), |(_, t)| t.clone());
                            state.update_marker(m.id, &name, c);
                        }
                    }
                });
                if ui.button("Save name").clicked() {
                    if let Some((_, text)) = view.marker_edit.take() {
                        state.update_marker(m.id, &text, m.color);
                    }
                    ui.close();
                }
                if ui.button("Delete marker").clicked() {
                    view.marker_edit = None;
                    state.remove_marker(m.id);
                    ui.close();
                }
            });
    }
}

fn zoom_around(view: &mut TimelineView, factor: f32, pointer_x: f32, lanes_left: f32) {
    let before = view.px_per_second;
    let after = (before * factor).clamp(4.0, 2_000.0);
    let anchor_s = view.scroll_seconds + (pointer_x - lanes_left).max(0.0) / before;
    view.px_per_second = after;
    view.scroll_seconds = (anchor_s - (pointer_x - lanes_left).max(0.0) / after).max(0.0);
}

fn toolbar(ui: &mut egui::Ui, state: &AppState, view: &mut TimelineView, actions: &mut Vec<Action>) {
    let rect = ui.available_rect_before_wrap();
    let bar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), TOOLBAR_H - 4.0));
    let mut bar_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let text = |t: &str| egui::RichText::new(t).size(12.0);
    let mut toggle = |ui: &mut egui::Ui, label: &str, on: bool, tip: &str, action: Action| {
        if ui
            .add(egui::Button::new(text(label)).selected(on))
            .on_hover_text(tip)
            .clicked()
        {
            actions.push(action);
        }
    };
    toggle(
        &mut bar_ui,
        "Select",
        state.tool == Tool::Select,
        "Selection tool (A): select, move, and drag clip edges to trim",
        Action::ToolSelect,
    );
    toggle(
        &mut bar_ui,
        "✂ Blade",
        state.tool == Tool::Blade,
        "Blade tool (B): click a clip to cut it at that point",
        Action::ToolBlade,
    );
    bar_ui.separator();
    toggle(
        &mut bar_ui,
        "Snap",
        state.snapping,
        "Snapping (N): clips snap to the playhead, markers and other clips",
        Action::ToggleSnapping,
    );
    toggle(
        &mut bar_ui,
        "Linked",
        state.linked_selection,
        "Linked selection (Ctrl+Shift+L): clicking a video also selects its audio",
        Action::ToggleLinkedSelection,
    );
    bar_ui.separator();
    let has_selection = !state.selected_clips.is_empty();
    let mut button = |ui: &mut egui::Ui, label: &str, enabled: bool, tip: &str, action: Action| {
        if ui
            .add_enabled(enabled, egui::Button::new(text(label)))
            .on_hover_text(tip)
            .clicked()
        {
            actions.push(action);
        }
    };
    button(
        &mut bar_ui,
        "Split",
        !state.timeline.is_empty(),
        "Split clips at the playhead (Ctrl+K)",
        Action::Split,
    );
    let linked = state
        .selected_clips
        .iter()
        .any(|&c| state.timeline.clip(c).is_some_and(|c| c.link.is_some()));
    button(
        &mut bar_ui,
        if linked { "Unlink" } else { "Link" },
        has_selection,
        "Unlink a video from its audio to edit them separately, or link selected clips (Ctrl+L)",
        Action::ToggleLink,
    );
    button(
        &mut bar_ui,
        "Delete",
        has_selection,
        "Delete selected clips (Delete)",
        Action::Delete,
    );
    button(
        &mut bar_ui,
        "Marker",
        true,
        "Add a marker at the playhead (M)",
        Action::AddMarker,
    );
    bar_ui.separator();
    button(
        &mut bar_ui,
        "+ Video track",
        true,
        "Add an empty video track at the bottom",
        Action::AddVideoTrack,
    );
    button(
        &mut bar_ui,
        "+ Audio track",
        true,
        "Add an empty audio track at the bottom",
        Action::AddAudioTrack,
    );
    bar_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui
            .add(egui::Button::new(text("Fit")))
            .on_hover_text("Show the whole timeline")
            .clicked()
        {
            let end = seconds(state.timeline.end()).max(5.0);
            let width = (bar.width() - HEADER_W - 20.0).max(100.0);
            view.px_per_second = (width / end).clamp(4.0, 2_000.0);
            view.scroll_seconds = 0.0;
        }
        if ui
            .add(egui::Button::new(text("+")))
            .on_hover_text("Zoom in (Ctrl+scroll)")
            .clicked()
        {
            view.px_per_second = (view.px_per_second * 1.5).min(2_000.0);
        }
        if ui
            .add(egui::Button::new(text("-")))
            .on_hover_text("Zoom out (Ctrl+scroll)")
            .clicked()
        {
            view.px_per_second = (view.px_per_second / 1.5).max(4.0);
        }
        ui.add(
            egui::Slider::new(&mut view.track_height, TRACK_H_RANGE)
                .show_value(false)
                .text(text("Track height")),
        )
        .on_hover_text("Make tracks taller or shorter");
    });
    ui.advance_cursor_after_rect(bar);
    ui.add_space(4.0);
}

fn draw_ruler(painter: &egui::Painter, ruler: egui::Rect, state: &AppState, scroll_s: f32, pps: f32) {
    painter.rect_filled(ruler, 0.0, theme::RAISED);
    let x_of = |s: f32| ruler.min.x + (s - scroll_s) * pps;
    let step = tick_step_seconds(pps);
    let mut s = (scroll_s / step).floor() * step;
    while x_of(s) < ruler.max.x {
        let x = x_of(s);
        if x >= ruler.min.x {
            painter.line_segment(
                [egui::pos2(x, ruler.max.y - 10.0), egui::pos2(x, ruler.max.y)],
                egui::Stroke::new(1.0, theme::TEXT_WEAK),
            );
            let label = timecode(time_from_seconds(s), state.project.frame_rate);
            painter.text(
                egui::pos2(x + 4.0, ruler.min.y + 3.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::monospace(10.0),
                theme::TEXT_WEAK,
            );
            for minor in 1..5 {
                let mx = x + step * pps * minor as f32 / 5.0;
                painter.line_segment(
                    [egui::pos2(mx, ruler.max.y - 4.0), egui::pos2(mx, ruler.max.y)],
                    egui::Stroke::new(1.0, theme::LINE),
                );
            }
        }
        s += step;
    }
    // Shade the part after the end of the timeline.
    if !state.timeline.is_empty() {
        let end_x = x_of(seconds(state.timeline.end()));
        if end_x < ruler.max.x {
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(end_x.max(ruler.min.x), ruler.min.y), ruler.max),
                0.0,
                egui::Color32::from_black_alpha(90),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{signed_seconds_to_time, tick_step_seconds};
    use motix_core::Time;

    #[test]
    fn ticks_stay_readable_at_any_zoom() {
        for pps in [5.0_f32, 20.0, 80.0, 300.0, 2000.0] {
            let step = tick_step_seconds(pps);
            assert!(
                step * pps >= 90.0 || (step - 600.0).abs() < f32::EPSILON,
                "{pps} px/s → {step}s"
            );
        }
        assert!((tick_step_seconds(80.0) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn signed_offsets() {
        assert!(signed_seconds_to_time(-1.5) < Time::ZERO);
        assert_eq!(signed_seconds_to_time(2.0), Time::from_seconds(2).unwrap());
    }
}

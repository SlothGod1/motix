//! The timeline: ruler, tracks and a draggable playhead.
//!
//! In M1 the tracks are empty; clips arrive with M2 (First edit).

use crate::{UiState, theme};
use motix_app::AppState;
use motix_app::state::timecode;
use motix_core::{Rational, Rounding, Time};

const HEADER_W: f32 = 64.0;
const RULER_H: f32 = 26.0;
const TRACK_H: f32 = 46.0;
const TRACKS: [(&str, egui::Color32); 4] = [
    ("V2", theme::VIDEO),
    ("V1", theme::VIDEO),
    ("A1", theme::AUDIO),
    ("A2", theme::AUDIO),
];

/// Seconds between labelled ruler ticks so that labels are at least ~90 px apart.
pub(crate) fn tick_step_seconds(px_per_second: f32) -> f32 {
    const STEPS: [f32; 10] = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 300.0, 600.0];
    STEPS
        .iter()
        .copied()
        .find(|s| s * px_per_second >= 90.0)
        .unwrap_or(600.0)
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut AppState, view: &mut UiState) {
    let full = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(full, egui::Sense::click_and_drag());
    let painter = ui.painter_at(full);

    // Zoom with Ctrl+scroll, pan with scroll.
    if response.hovered() {
        let (scroll, zoom, command) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.command));
        if (zoom - 1.0).abs() > f32::EPSILON {
            view.timeline_px_per_second = (view.timeline_px_per_second * zoom).clamp(5.0, 2_000.0);
        } else if !command {
            let delta = if scroll.x.abs() > scroll.y.abs() {
                scroll.x
            } else {
                scroll.y
            };
            view.timeline_scroll_seconds =
                (view.timeline_scroll_seconds - delta / view.timeline_px_per_second).max(0.0);
        }
    }

    let lanes_left = full.min.x + HEADER_W;
    let x_of = |seconds: f32| lanes_left + (seconds - view.timeline_scroll_seconds) * view.timeline_px_per_second;

    // Ruler.
    let ruler = egui::Rect::from_min_max(
        egui::pos2(lanes_left, full.min.y),
        egui::pos2(full.max.x, full.min.y + RULER_H),
    );
    painter.rect_filled(ruler, 0.0, theme::RAISED);
    let step = tick_step_seconds(view.timeline_px_per_second);
    let first = (view.timeline_scroll_seconds / step).floor() * step;
    let mut s = first;
    while x_of(s) < full.max.x {
        let x = x_of(s);
        if x >= lanes_left {
            painter.line_segment(
                [egui::pos2(x, ruler.max.y - 8.0), egui::pos2(x, ruler.max.y)],
                egui::Stroke::new(1.0, theme::TEXT_WEAK),
            );
            let t = Time::from_seconds_rational(
                Rational::new((s * 1000.0) as i64, 1000).unwrap_or(Rational::ZERO),
                Rounding::Nearest,
            )
            .unwrap_or(Time::ZERO);
            let label = timecode(t, state.frame_rate);
            painter.text(
                egui::pos2(x + 4.0, ruler.min.y + 4.0),
                egui::Align2::LEFT_TOP,
                &label[..8],
                egui::FontId::monospace(10.5),
                theme::TEXT_WEAK,
            );
            for minor in 1..5 {
                let mx = x + step * view.timeline_px_per_second * minor as f32 / 5.0;
                painter.line_segment(
                    [egui::pos2(mx, ruler.max.y - 4.0), egui::pos2(mx, ruler.max.y)],
                    egui::Stroke::new(1.0, theme::LINE),
                );
            }
        }
        s += step;
    }

    // Tracks.
    for (i, (name, color)) in TRACKS.iter().enumerate() {
        let top = ruler.max.y + 4.0 + i as f32 * (TRACK_H + 4.0);
        if top > full.max.y {
            break;
        }
        let header = egui::Rect::from_min_size(egui::pos2(full.min.x, top), egui::vec2(HEADER_W - 6.0, TRACK_H));
        painter.rect_filled(header, 6.0, theme::RAISED);
        painter.rect_filled(
            egui::Rect::from_min_size(header.min, egui::vec2(3.0, TRACK_H)),
            2.0,
            *color,
        );
        painter.text(
            header.left_center() + egui::vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            *name,
            egui::FontId::proportional(13.0),
            theme::TEXT,
        );
        let lane = egui::Rect::from_min_max(egui::pos2(lanes_left, top), egui::pos2(full.max.x, top + TRACK_H));
        painter.rect_filled(lane, 4.0, theme::BG);
        if i == 1 && state.media.items().is_empty() {
            painter.text(
                lane.left_center() + egui::vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "Clips will appear here (M2 · First edit)",
                egui::FontId::proportional(12.0),
                theme::TEXT_WEAK,
            );
        }
    }

    // Click or drag anywhere in the lanes to move the playhead.
    if let Some(pos) = response.interact_pointer_pos()
        && (response.clicked() || response.dragged())
        && pos.x >= lanes_left
    {
        let seconds = view.timeline_scroll_seconds + (pos.x - lanes_left) / view.timeline_px_per_second;
        let ms = (seconds.max(0.0) * 1000.0) as i64;
        if let Ok(t) = Time::from_seconds_rational(Rational::new(ms, 1000).unwrap_or(Rational::ZERO), Rounding::Floor) {
            // Snap to the frame grid so the viewer shows whole frames.
            let frame = state.frame_rate.time_to_frame(t);
            state.playhead = state.frame_rate.frame_to_time(frame).unwrap_or(t);
        }
    }

    // Keep the playhead visible while playing.
    let playhead_s = state.playhead.as_seconds_f64() as f32;
    let visible_s = (full.max.x - lanes_left) / view.timeline_px_per_second;
    if state.playing && playhead_s > view.timeline_scroll_seconds + visible_s * 0.9 {
        view.timeline_scroll_seconds = playhead_s - visible_s * 0.1;
    }

    let x = lanes_left + (playhead_s - view.timeline_scroll_seconds) * view.timeline_px_per_second;
    if x >= lanes_left && x <= full.max.x {
        painter.line_segment(
            [egui::pos2(x, full.min.y), egui::pos2(x, full.max.y)],
            egui::Stroke::new(1.5, theme::PLAYHEAD),
        );
        let head = [
            egui::pos2(x - 6.0, full.min.y),
            egui::pos2(x + 6.0, full.min.y),
            egui::pos2(x, full.min.y + 9.0),
        ];
        painter.add(egui::Shape::convex_polygon(
            head.to_vec(),
            theme::PLAYHEAD,
            egui::Stroke::NONE,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::tick_step_seconds;

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
}

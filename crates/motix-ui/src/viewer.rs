//! The viewer: the project frame at its true shape, what's on the timeline at the
//! playhead (placed with its fit mode), safe-area guides and transport controls.
//!
//! The picture comes from the FFmpeg helper (`motix-media`); until a frame arrives
//! (or without the helper) a labelled placeholder shows exactly where it will go.

use crate::theme;
use motix_app::{Action, AppState, ClipId, MediaKind};

/// The decoded picture for the viewer.
#[derive(Default)]
pub(crate) struct Preview {
    /// The newest frame.
    pub texture: Option<egui::TextureHandle>,
    /// The clip that frame belongs to.
    pub clip: Option<ClipId>,
    /// Why there's no picture (no helper, or it failed), in plain language.
    pub note: Option<String>,
}

pub(crate) fn show(ui: &mut egui::Ui, state: &AppState, preview: &Preview, actions: &mut Vec<Action>) {
    let full = ui.available_rect_before_wrap();
    let transport_h = 40.0;
    let backdrop = egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, full.max.y - transport_h));
    let painter = ui.painter_at(full);
    painter.rect_filled(backdrop, 6.0, theme::BG);

    let project = state.project.resolution;
    let (canvas_w, canvas_h) = project.fit_within(backdrop.width() - 24.0, backdrop.height() - 24.0);
    let canvas = egui::Rect::from_center_size(backdrop.center(), egui::vec2(canvas_w, canvas_h));
    painter.rect_filled(canvas, 2.0, egui::Color32::BLACK);

    let clip = state.timeline.top_picture_at(state.playhead);
    let (headline, detail) = if let Some(clip) = clip {
        let scale = canvas.width() / project.width as f32;
        let source = clip.source_size.unwrap_or(project);
        let (x, y, w, h) = clip.fit.place(source, project);
        let pic = egui::Rect::from_min_size(
            canvas.min + egui::vec2(x as f32 * scale, y as f32 * scale),
            egui::vec2(w as f32 * scale, h as f32 * scale),
        );
        let item = state.media.get(clip.media);
        let kind = item.map_or(MediaKind::Video, |m| m.kind);
        let picture = preview.texture.as_ref().filter(|_| preview.clip == Some(clip.id));
        let color = if kind == MediaKind::Image {
            theme::IMAGE
        } else {
            theme::VIDEO
        };
        // Parts outside the frame (cropped by "fill" / "original size") are drawn faintly.
        painter.rect_stroke(
            pic,
            0.0,
            egui::Stroke::new(1.0, color.linear_multiply(0.35)),
            egui::StrokeKind::Inside,
        );
        let inside = painter.with_clip_rect(canvas);
        if let Some(texture) = picture {
            inside.image(
                texture.id(),
                pic,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            inside.rect_filled(pic, 0.0, color.linear_multiply(0.28));
        }
        // A simple grid so scaling and cropping are visible (placeholder only).
        for i in (1..4).filter(|_| picture.is_none()) {
            let fx = pic.min.x + pic.width() * i as f32 / 4.0;
            let fy = pic.min.y + pic.height() * i as f32 / 4.0;
            let stroke = egui::Stroke::new(1.0, color.linear_multiply(0.25));
            inside.line_segment([egui::pos2(fx, pic.min.y), egui::pos2(fx, pic.max.y)], stroke);
            inside.line_segment([egui::pos2(pic.min.x, fy), egui::pos2(pic.max.x, fy)], stroke);
        }
        if picture.is_none() {
            inside.rect_stroke(pic, 0.0, egui::Stroke::new(1.5, color), egui::StrokeKind::Inside);
        }
        let placement = if source == project {
            format!("{source} · same size as the project")
        } else {
            format!("{source} · {}", clip.fit.label().to_lowercase())
        };
        let status = if picture.is_some() {
            String::new()
        } else if item.is_some_and(|m| m.size_bytes.is_none()) {
            "This file can't be found. Put it back (or reopen the project next to it) to see it.".to_owned()
        } else if let Some(note) = &preview.note {
            note.clone()
        } else {
            "Loading the picture…".to_owned()
        };
        (clip.name.clone(), format!("{placement}\n{status}"))
    } else if state.timeline.is_empty() {
        match state.selected_media.and_then(|m| state.media.get(m)) {
            Some(item) => (
                item.name.clone(),
                format!("{}\nDouble-click it or drag it onto the timeline.", item.summary()),
            ),
            None => (
                "Drop a video here".to_owned(),
                "or use File > Import media (Ctrl+I)".to_owned(),
            ),
        }
    } else {
        (
            "Nothing here".to_owned(),
            "There's no picture on the timeline at this point.".to_owned(),
        )
    };
    painter.rect_stroke(
        canvas,
        2.0,
        egui::Stroke::new(1.0, theme::LINE),
        egui::StrokeKind::Outside,
    );

    let canvas_response = ui.interact(canvas, ui.id().with("motix_canvas"), egui::Sense::hover());
    let described = format!("Viewer, {project} frame: {headline}. {detail}");
    canvas_response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &described));
    // Centred, wrapped text so it never spills over narrow (vertical) frames.
    let wrap = (canvas.width() - 28.0).max(40.0);
    let centred = |text: &str, size: f32, color: egui::Color32| {
        let mut job = egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::proportional(size), color, wrap);
        job.halign = egui::Align::Center;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let showing_picture = clip.is_some_and(|c| preview.clip == Some(c.id) && preview.texture.is_some());
    if !showing_picture {
        let head = centred(&headline, 18.0, theme::TEXT);
        let sub = centred(&detail, 12.5, theme::TEXT_WEAK);
        let top = canvas.center().y - (head.size().y + 6.0 + sub.size().y) / 2.0;
        let head_h = head.size().y;
        painter.galley(egui::pos2(canvas.center().x, top), head, theme::TEXT);
        painter.galley(egui::pos2(canvas.center().x, top + head_h + 6.0), sub, theme::TEXT_WEAK);
    }

    if state.show_safe_areas {
        let (left, top, right, bottom) = project.safe_insets();
        let safe = egui::Rect::from_min_max(
            egui::pos2(
                canvas.min.x + canvas.width() * left,
                canvas.min.y + canvas.height() * top,
            ),
            egui::pos2(
                canvas.max.x - canvas.width() * right,
                canvas.max.y - canvas.height() * bottom,
            ),
        );
        let stroke = egui::Stroke::new(1.0, theme::ACCENT.linear_multiply(0.8));
        let corners = [
            safe.left_top(),
            safe.right_top(),
            safe.right_bottom(),
            safe.left_bottom(),
            safe.left_top(),
        ];
        painter.extend(egui::Shape::dashed_line(&corners, stroke, 6.0, 4.0));
        painter.text(
            safe.left_top() + egui::vec2(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            "Safe area",
            egui::FontId::proportional(10.5),
            theme::ACCENT,
        );
    }

    // Transport row.
    let row = egui::Rect::from_min_max(egui::pos2(full.min.x, full.max.y - transport_h + 6.0), full.max);
    let mut row_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(row)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let play_label = if state.playing { "⏸" } else { "▶" };
    for (label, tip, action) in [
        ("⏮", "Go to start (Home)", Action::GoToStart),
        ("◀", "Previous frame (Left arrow)", Action::PreviousFrame),
        (play_label, "Play / pause (Space)", Action::TogglePlayback),
        ("▶|", "Next frame (Right arrow)", Action::NextFrame),
    ] {
        if row_ui
            .add(egui::Button::new(egui::RichText::new(label).size(15.0)).min_size(egui::vec2(34.0, 26.0)))
            .on_hover_text(tip)
            .clicked()
        {
            actions.push(action);
        }
    }
    row_ui.add_space(10.0);
    row_ui.label(
        egui::RichText::new(state.timecode())
            .monospace()
            .size(15.0)
            .color(theme::TEXT),
    );
    row_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let p = &state.project;
        ui.label(
            egui::RichText::new(format!(
                "{} · {} fps · {} · {}-bit",
                p.resolution,
                p.frame_rate.short_label(),
                if p.color.is_hdr() { p.color.label() } else { "SDR" },
                p.bit_depth
            ))
            .color(theme::TEXT_WEAK),
        );
    });
}

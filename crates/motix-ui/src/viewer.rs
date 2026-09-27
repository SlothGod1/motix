//! The viewer: the canvas at its true aspect ratio, safe-area guides and transport controls.

use crate::theme;
use motix_app::{Action, AppState, MediaKind};

pub(crate) fn show(ui: &mut egui::Ui, state: &AppState, actions: &mut Vec<Action>) {
    let full = ui.available_rect_before_wrap();
    let transport_h = 40.0;
    let backdrop = egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, full.max.y - transport_h));
    let painter = ui.painter_at(full);
    painter.rect_filled(backdrop, 6.0, theme::BG);

    // Canvas at its real aspect ratio, centred in the stage.
    let (canvas_w, canvas_h) = state
        .canvas
        .fit_within(backdrop.width() - 24.0, backdrop.height() - 24.0);
    let canvas = egui::Rect::from_center_size(backdrop.center(), egui::vec2(canvas_w, canvas_h));
    painter.rect_filled(canvas, 2.0, egui::Color32::BLACK);
    painter.rect_stroke(
        canvas,
        2.0,
        egui::Stroke::new(1.0, theme::LINE),
        egui::StrokeKind::Outside,
    );

    let selected = state.selected_media.and_then(|i| state.media.items().get(i));
    let (headline, detail) = match selected {
        None => (
            "Drop a video here".to_owned(),
            "or use File > Import media (Ctrl+I)".to_owned(),
        ),
        Some(item) if item.kind == MediaKind::Video => (
            item.name.clone(),
            "Video playback arrives in the next M1 update.".to_owned(),
        ),
        Some(item) => (
            item.name.clone(),
            format!("{} · preview arrives in the next M1 update.", item.kind.label()),
        ),
    };
    let canvas_response = ui.interact(canvas, ui.id().with("motix_canvas"), egui::Sense::hover());
    let described = format!("Viewer, {} canvas: {headline}. {detail}", state.canvas.name);
    canvas_response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &described));
    // Centred, wrapped text inside the canvas so it never spills over narrow (vertical) canvases.
    let wrap = (canvas.width() - 28.0).max(40.0);
    let centred = |text: &str, size: f32, color: egui::Color32| {
        let mut job = egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::proportional(size), color, wrap);
        job.halign = egui::Align::Center;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let head = centred(&headline, 18.0, theme::TEXT);
    let sub = centred(&detail, 12.5, theme::TEXT_WEAK);
    let top = canvas.center().y - (head.size().y + 6.0 + sub.size().y) / 2.0;
    let head_h = head.size().y;
    painter.galley(egui::pos2(canvas.center().x, top), head, theme::TEXT);
    painter.galley(egui::pos2(canvas.center().x, top + head_h + 6.0), sub, theme::TEXT_WEAK);

    if state.show_safe_areas {
        let (left, top, right, bottom) = state.canvas.safe_insets();
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
        ui.label(
            egui::RichText::new(format!(
                "{} · {}×{} · {}",
                state.canvas.name, state.canvas.width, state.canvas.height, state.frame_rate
            ))
            .color(theme::TEXT_WEAK),
        );
    });
}

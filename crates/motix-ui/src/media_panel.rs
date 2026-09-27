//! The media bin panel.

use crate::theme;
use motix_app::{Action, AppState, MediaKind};

fn badge(kind: MediaKind) -> (&'static str, egui::Color32) {
    match kind {
        MediaKind::Video => ("VID", theme::VIDEO),
        MediaKind::Audio => ("AUD", theme::AUDIO),
        MediaKind::Image => ("IMG", theme::IMAGE),
        MediaKind::Unsupported => ("???", theme::TEXT_WEAK),
    }
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut AppState, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} item{}",
                state.media.items().len(),
                if state.media.items().len() == 1 { "" } else { "s" }
            ))
            .color(theme::TEXT_WEAK),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ Import").on_hover_text("Import media (Ctrl+I)").clicked() {
                actions.push(Action::ImportMedia);
            }
        });
    });
    ui.add_space(4.0);

    if state.media.items().is_empty() {
        let rect = ui.available_rect_before_wrap().shrink2(egui::vec2(0.0, 4.0));
        let response = ui.allocate_rect(rect, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        painter.extend(egui::Shape::dashed_line(
            &corners,
            egui::Stroke::new(1.0, theme::LINE),
            6.0,
            4.0,
        ));
        painter.text(
            rect.center() - egui::vec2(0.0, 9.0),
            egui::Align2::CENTER_CENTER,
            "Drag files here",
            egui::FontId::proportional(15.0),
            theme::TEXT,
        );
        painter.text(
            rect.center() + egui::vec2(0.0, 12.0),
            egui::Align2::CENTER_CENTER,
            "videos · audio · images",
            egui::FontId::proportional(12.0),
            theme::TEXT_WEAK,
        );
        // Painted text is invisible to screen readers, so describe the drop zone explicitly.
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                "Drag files here, or click to import media",
            )
        });
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            actions.push(Action::ImportMedia);
        }
        return;
    }

    let mut remove = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, item) in state.media.items().iter().enumerate() {
            let selected = state.selected_media == Some(index);
            let (tag, color) = badge(item.kind);
            let row = ui
                .horizontal(|ui| {
                    ui.label(egui::RichText::new(tag).monospace().size(10.5).color(color));
                    ui.add(egui::Label::new(egui::RichText::new(&item.name).color(theme::TEXT)).truncate());
                })
                .response;
            let row = ui.interact(row.rect, ui.id().with(("media_row", index)), egui::Sense::click());
            if selected {
                ui.painter()
                    .rect_filled(row.rect.expand(2.0), 4.0, theme::ACCENT.linear_multiply(0.18));
            }
            if row.clicked() {
                state.selected_media = Some(index);
            }
            row.on_hover_text(format!("{}\n{}", item.path.display(), item.size_label()))
                .context_menu(|ui| {
                    if ui.button("Remove from project").clicked() {
                        remove = Some(index);
                        ui.close();
                    }
                });
        }
    });
    if let Some(index) = remove
        && let Some(item) = state.media.remove(index)
    {
        state.status = format!(
            "Removed {} from the project (the file itself was not touched).",
            item.name
        );
        state.selected_media = if state.media.items().is_empty() {
            None
        } else {
            Some(index.min(state.media.items().len() - 1))
        };
    }
}

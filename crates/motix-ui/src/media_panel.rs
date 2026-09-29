//! The media bin panel. Drag an item onto the timeline, or double-click it to add it
//! to the end.

use crate::theme;
use motix_app::{Action, AppState, MediaId, MediaKind};

pub(crate) fn badge(kind: MediaKind) -> (&'static str, egui::Color32) {
    match kind {
        MediaKind::Video => ("VID", theme::VIDEO),
        MediaKind::Audio => ("AUD", theme::AUDIO),
        MediaKind::Image => ("IMG", theme::IMAGE),
        MediaKind::Unsupported => ("???", theme::TEXT_WEAK),
    }
}

enum RowAction {
    Select(MediaId),
    AddToTimeline(MediaId),
    Match(MediaId),
    Remove(MediaId),
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut AppState, actions: &mut Vec<Action>) {
    let count = state.media.items().len();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{count} item{}", if count == 1 { "" } else { "s" })).color(theme::TEXT_WEAK),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ Import").on_hover_text("Import media (Ctrl+I)").clicked() {
                actions.push(Action::ImportMedia);
            }
        });
    });
    ui.add_space(4.0);

    if count == 0 {
        empty_drop_zone(ui, actions);
        return;
    }

    let mut todo = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for item in state.media.items() {
            let selected = state.selected_media == Some(item.id);
            let (tag, color) = badge(item.kind);
            let width = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 38.0), egui::Sense::hover());
            let row = ui.interact(
                rect,
                ui.id().with(("media_row", item.id.0)),
                egui::Sense::click_and_drag(),
            );
            row.dnd_set_drag_payload(item.id);
            if selected {
                ui.painter().rect_filled(rect, 5.0, theme::ACCENT.linear_multiply(0.18));
            } else if row.hovered() {
                ui.painter().rect_filled(rect, 5.0, theme::RAISED);
            }
            let painter = ui.painter_at(rect);
            painter.text(
                rect.left_top() + egui::vec2(6.0, 5.0),
                egui::Align2::LEFT_TOP,
                tag,
                egui::FontId::monospace(10.5),
                color,
            );
            painter.text(
                rect.left_top() + egui::vec2(38.0, 3.0),
                egui::Align2::LEFT_TOP,
                &item.name,
                egui::FontId::proportional(13.0),
                theme::TEXT,
            );
            painter.text(
                rect.left_top() + egui::vec2(38.0, 20.0),
                egui::Align2::LEFT_TOP,
                item.summary(),
                egui::FontId::proportional(11.0),
                theme::TEXT_WEAK,
            );
            let label = format!("{}, {}", item.name, item.summary());
            row.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if row.double_clicked() {
                todo = Some(RowAction::AddToTimeline(item.id));
            } else if row.clicked() || row.drag_started() {
                todo = Some(RowAction::Select(item.id));
            }
            row.on_hover_text(format!(
                "{}\n{}\nDouble-click or drag onto the timeline to use it.",
                item.path.display(),
                item.size_label()
            ))
            .context_menu(|ui| {
                if ui.button("Add to timeline").clicked() {
                    todo = Some(RowAction::AddToTimeline(item.id));
                    ui.close();
                }
                if item.resolution().is_some() && ui.button("Match project settings to this").clicked() {
                    todo = Some(RowAction::Match(item.id));
                    ui.close();
                }
                ui.separator();
                if ui.button("Remove from project").clicked() {
                    todo = Some(RowAction::Remove(item.id));
                    ui.close();
                }
            });
        }
    });

    // While dragging, show what's being carried next to the pointer.
    if let Some(id) = egui::DragAndDrop::payload::<MediaId>(ui.ctx())
        && let Some(item) = state.media.get(*id)
        && let Some(pos) = ui.ctx().pointer_interact_pos()
    {
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("motix_drag_media"),
        ));
        let text = format!("{}  ·  drop on the timeline", item.name);
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(12.5), theme::TEXT);
        let r = egui::Rect::from_min_size(pos + egui::vec2(14.0, 10.0), galley.size() + egui::vec2(14.0, 8.0));
        painter.rect_filled(r, 6.0, theme::RAISED);
        painter.rect_stroke(r, 6.0, egui::Stroke::new(1.0, theme::ACCENT), egui::StrokeKind::Inside);
        painter.galley(r.min + egui::vec2(7.0, 4.0), galley, theme::TEXT);
    }

    match todo {
        Some(RowAction::Select(id)) => state.selected_media = Some(id),
        Some(RowAction::AddToTimeline(id)) => {
            state.selected_media = Some(id);
            let _ = state.add_to_timeline(id, None, None);
        }
        Some(RowAction::Match(id)) => state.match_project_to(id),
        Some(RowAction::Remove(id)) => state.remove_media(id),
        None => {}
    }
}

fn empty_drop_zone(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
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
}

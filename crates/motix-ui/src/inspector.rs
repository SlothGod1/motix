//! The inspector: details of the selected media, or project settings.

use crate::theme;
use motix_app::{Action, AppState, CanvasPreset};
use motix_core::FrameRate;

pub(crate) fn show(ui: &mut egui::Ui, state: &mut AppState, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if let Some(item) = state.selected_media.and_then(|i| state.media.items().get(i)) {
            section(ui, "Selected media");
            row(ui, "Name", &item.name);
            row(ui, "Type", item.kind.label());
            row(ui, "Size", &item.size_label());
            ui.label(
                egui::RichText::new(item.path.display().to_string())
                    .small()
                    .color(theme::TEXT_WEAK),
            );
            ui.add_space(10.0);
        }

        section(ui, "Project");
        ui.label(egui::RichText::new("Canvas").color(theme::TEXT_WEAK));
        for preset in CanvasPreset::ALL {
            let label = format!("{}  ·  {}", preset.name, preset.used_for);
            if ui.selectable_label(state.canvas == preset, label).clicked() {
                actions.push(match preset {
                    p if p == CanvasPreset::VERTICAL => Action::CanvasVertical,
                    p if p == CanvasPreset::PORTRAIT => Action::CanvasPortrait,
                    p if p == CanvasPreset::SQUARE => Action::CanvasSquare,
                    _ => Action::CanvasLandscape,
                });
            }
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Frame rate").color(theme::TEXT_WEAK));
        egui::ComboBox::from_id_salt("motix_frame_rate")
            .selected_text(state.frame_rate.to_string())
            .show_ui(ui, |ui| {
                for rate in FrameRate::COMMON {
                    ui.selectable_value(&mut state.frame_rate, rate, rate.to_string());
                }
            });
        ui.add_space(6.0);
        let mut safe = state.show_safe_areas;
        if ui
            .checkbox(&mut safe, "Show safe areas")
            .on_hover_text("Where social apps put captions and buttons (Ctrl+G)")
            .changed()
        {
            actions.push(Action::ToggleSafeAreas);
        }
    });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).strong().size(13.5));
    ui.separator();
}

fn row(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).color(theme::TEXT_WEAK));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(value).truncate());
        });
    });
}

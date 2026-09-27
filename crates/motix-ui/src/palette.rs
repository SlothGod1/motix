//! The command palette: type part of any command's name and press Enter.

use crate::theme;
use motix_app::Action;
use motix_app::actions::{self, Availability};

#[derive(Default)]
pub(crate) struct Palette {
    pub open: bool,
    query: String,
    selected: usize,
    focus_pending: bool,
}

impl Palette {
    pub(crate) fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus_pending = true;
    }

    /// Draws the palette if open; returns the action chosen this frame.
    pub(crate) fn show(&mut self, ctx: &egui::Context) -> Option<Action> {
        if !self.open {
            return None;
        }
        let results = actions::search(&self.query);
        let (down, up, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Enter),
            )
        });
        if down {
            self.selected = (self.selected + 1).min(results.len().saturating_sub(1));
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        let mut chosen = None;
        let modal = egui::Modal::new(egui::Id::new("motix_palette")).show(ctx, |ui| {
            ui.set_width(460.0);
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Type a command…")
                    .desired_width(f32::INFINITY),
            );
            if self.focus_pending {
                edit.request_focus();
                self.focus_pending = false;
            }
            if edit.changed() {
                self.selected = 0;
            }
            ui.add_space(4.0);
            if results.is_empty() {
                ui.label(egui::RichText::new("No matching command").color(theme::TEXT_WEAK));
            }
            for (i, info) in results.iter().take(12).enumerate() {
                let mut text = egui::RichText::new(info.label);
                if let Availability::Planned(m) = info.availability {
                    text = egui::RichText::new(format!("{}  ({m})", info.label)).color(theme::TEXT_WEAK);
                }
                let mut button = egui::Button::new(text)
                    .selected(i == self.selected)
                    .min_size(egui::vec2(ui.available_width(), 24.0));
                if let Some(s) = info.shortcut {
                    button = button.shortcut_text(s.label());
                }
                if ui.add(button).clicked() {
                    chosen = Some(info.action);
                }
            }
        });
        if enter && chosen.is_none() {
            chosen = results.get(self.selected).map(|i| i.action);
        }
        if chosen.is_some() || modal.should_close() {
            self.open = false;
        }
        chosen
    }
}

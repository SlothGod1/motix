//! Questions about the project file: "Save changes before…?" and "Restore your
//! unsaved changes?" after MOTIX didn't close properly.

use crate::theme;
use motix_app::AfterSave;
use motix_app::document::RecoveryCopy;
use std::time::SystemTime;

/// What the user chose in a prompt.
pub(crate) enum Choice {
    /// Save first, then continue.
    Save(AfterSave),
    /// Continue without saving.
    DontSave(AfterSave),
    /// Restore this recovery copy.
    Restore(std::path::PathBuf),
    /// Throw the recovery copies away.
    DiscardRecovery,
}

fn modal(ctx: &egui::Context, id: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Modal::new(egui::Id::new(id)).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.heading(title);
        ui.add_space(6.0);
        add(ui);
    });
}

const fn doing(then: AfterSave) -> &'static str {
    match then {
        AfterSave::NewProject => "starting a new project",
        AfterSave::OpenProject | AfterSave::OpenDropped => "opening another project",
        AfterSave::Quit => "closing MOTIX",
    }
}

/// "Save changes to “name” before …?" Returns the choice; `Cancel` clears `pending`.
pub(crate) fn unsaved(ctx: &egui::Context, pending: &mut Option<AfterSave>, name: &str) -> Option<Choice> {
    let then = (*pending)?;
    let mut choice = None;
    let mut cancel = false;
    modal(ctx, "motix_unsaved", "Save your changes?", |ui| {
        ui.label(format!(
            "\u{201c}{name}\u{201d} has changes that aren't saved. Save them before {}?",
            doing(then)
        ));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new(egui::RichText::new("Save").strong()).fill(theme::ACCENT))
                .clicked()
            {
                choice = Some(Choice::Save(then));
            }
            if ui.button("Don't save").clicked() {
                choice = Some(Choice::DontSave(then));
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    });
    if cancel || choice.is_some() {
        *pending = None;
    }
    choice
}

fn ago(t: SystemTime) -> String {
    let mins = SystemTime::now().duration_since(t).map_or(0, |d| d.as_secs() / 60);
    match mins {
        0 => "just now".to_owned(),
        1 => "1 minute ago".to_owned(),
        m if m < 120 => format!("{m} minutes ago"),
        m if m < 48 * 60 => format!("{} hours ago", m / 60),
        m => format!("{} days ago", m / (24 * 60)),
    }
}

/// "MOTIX didn't close properly — restore your unsaved changes?"
pub(crate) fn recovery(ctx: &egui::Context, copies: &[RecoveryCopy]) -> Option<Choice> {
    let newest = copies.first()?;
    let mut choice = None;
    modal(ctx, "motix_recovery", "Restore your unsaved changes?", |ui| {
        ui.label(format!(
            "MOTIX didn't close properly last time. Your unsaved changes from {} were kept safe.",
            ago(newest.saved)
        ));
        if copies.len() > 1 {
            ui.label(
                egui::RichText::new(format!(
                    "{} older copies were also found; restoring keeps the newest.",
                    copies.len() - 1
                ))
                .color(theme::TEXT_WEAK),
            );
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new(egui::RichText::new("Restore").strong()).fill(theme::ACCENT))
                .clicked()
            {
                choice = Some(Choice::Restore(newest.path.clone()));
            }
            if ui
                .button("Discard")
                .on_hover_text("Throw the unsaved changes away")
                .clicked()
            {
                choice = Some(Choice::DiscardRecovery);
            }
        });
    });
    choice
}

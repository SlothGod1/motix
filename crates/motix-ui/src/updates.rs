//! The "Updates" window (Help > Check for updates…) and the "update ready" banner.

use crate::{Request, theme};
use motix_app::{UpdateInfo, UpdatePhase};

/// UI-only state for updates.
#[derive(Default)]
pub(crate) struct UpdatesView {
    pub open: bool,
    /// The version whose "ready" banner the user dismissed with "Later".
    pub banner_dismissed_for: Option<String>,
}

fn phase_text(info: &UpdateInfo) -> (String, egui::Color32) {
    match &info.phase {
        UpdatePhase::Disabled { reason } => (reason.clone(), theme::TEXT_WEAK),
        UpdatePhase::Idle => ("Not checked yet.".to_owned(), theme::TEXT_WEAK),
        UpdatePhase::Checking => ("Checking GitHub for a newer version…".to_owned(), theme::TEXT),
        UpdatePhase::UpToDate => ("You have the newest version.".to_owned(), theme::AUDIO),
        UpdatePhase::Downloading { version, .. } => {
            (format!("Downloading version {version} in the background…"), theme::TEXT)
        }
        UpdatePhase::Ready { version, .. } => (
            format!("Version {version} is downloaded, checked and ready to install."),
            theme::AUDIO,
        ),
        UpdatePhase::Installing => ("Installing…".to_owned(), theme::TEXT),
        UpdatePhase::Failed { message } => (message.clone(), theme::PLAYHEAD),
    }
}

/// Draws the updates window if open.
pub(crate) fn window(ctx: &egui::Context, view: &mut UpdatesView, info: &UpdateInfo, requests: &mut Vec<Request>) {
    if !view.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Updates")
        .id(egui::Id::new("motix_updates"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(format!("You're running MOTIX {}.", info.current_version));
            ui.add_space(4.0);
            let (text, color) = phase_text(info);
            ui.label(egui::RichText::new(text).color(color));
            if let UpdatePhase::Downloading { done, total, .. } = &info.phase {
                let fraction = total.filter(|t| *t > 0).map_or(0.0, |t| *done as f32 / t as f32);
                ui.add(egui::ProgressBar::new(fraction).show_percentage());
            }
            if let UpdatePhase::Ready { notes, .. } = &info.phase
                && !notes.trim().is_empty()
            {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("What's new").strong());
                egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(notes).color(theme::TEXT_WEAK));
                });
            }
            if let Some(when) = &info.last_checked {
                ui.label(
                    egui::RichText::new(format!("Last checked {when}."))
                        .small()
                        .color(theme::TEXT_WEAK),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let busy = matches!(
                    info.phase,
                    UpdatePhase::Checking | UpdatePhase::Downloading { .. } | UpdatePhase::Installing
                );
                let disabled = matches!(info.phase, UpdatePhase::Disabled { .. });
                if ui
                    .add_enabled(!busy && !disabled, egui::Button::new("Check now"))
                    .clicked()
                {
                    requests.push(Request::CheckForUpdates);
                }
                if matches!(info.phase, UpdatePhase::Ready { .. }) {
                    if ui
                        .add(egui::Button::new(egui::RichText::new("Restart and install").strong()).fill(theme::ACCENT))
                        .clicked()
                    {
                        requests.push(Request::InstallUpdateNow);
                    }
                    if !info.install_on_exit && ui.button("Install when I close MOTIX").clicked() {
                        requests.push(Request::InstallUpdateOnExit);
                    }
                }
            });
            ui.add_space(6.0);
            let mut auto = info.auto_check;
            if ui
                .checkbox(
                    &mut auto,
                    "Check automatically (when MOTIX starts and every 10 minutes)",
                )
                .on_hover_text("New versions download in the background; nothing is installed until you restart.")
                .changed()
            {
                requests.push(Request::SetAutoCheck(auto));
            }
            ui.label(
                egui::RichText::new(
                    "Updates come from the official MOTIX releases on GitHub and are checked against MOTIX's \
                     signing key before anything is installed.",
                )
                .small()
                .color(theme::TEXT_WEAK),
            );
        });
    if !open {
        view.open = false;
    }
}

/// The banner shown at the top of the window when an update is ready.
pub(crate) fn banner(ui: &mut egui::Ui, view: &mut UpdatesView, info: &UpdateInfo, requests: &mut Vec<Request>) {
    let UpdatePhase::Ready { version, .. } = &info.phase else {
        return;
    };
    if view.banner_dismissed_for.as_deref() == Some(version.as_str()) || info.install_on_exit {
        return;
    }
    egui::Frame::new()
        .fill(theme::ACCENT.linear_multiply(0.25))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("MOTIX {version} is ready to install.")).strong());
                if ui.button("Restart now").clicked() {
                    requests.push(Request::InstallUpdateNow);
                }
                if ui
                    .button("Later")
                    .on_hover_text("It will install automatically when you close MOTIX")
                    .clicked()
                {
                    view.banner_dismissed_for = Some(version.clone());
                    requests.push(Request::InstallUpdateOnExit);
                }
                if ui.button("What's new").clicked() {
                    view.open = true;
                }
            });
        });
}

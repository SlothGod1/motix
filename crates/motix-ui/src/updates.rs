//! The "Updates" window (Help > Check for updates…, and Help > Share MOTIX on your
//! network…) and the "update ready" banner.

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
        UpdatePhase::Checking => ("Checking for a newer version…".to_owned(), theme::TEXT),
        UpdatePhase::UpToDate => ("You have the newest version.".to_owned(), theme::AUDIO),
        UpdatePhase::Downloading { version, .. } => {
            (format!("Downloading version {version} in the background…"), theme::TEXT)
        }
        UpdatePhase::Ready { version, .. } => (
            format!("Version {version} is downloaded, checked and ready to install."),
            theme::AUDIO,
        ),
        UpdatePhase::Installed { version, .. } => (
            format!("Version {version} is installed in the shared folder. Restart MOTIX to start using it."),
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
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 90.0))
        .show(ctx, |ui| {
            ui.label(format!("You're running MOTIX {}.", info.current_version));
            ui.add_space(4.0);
            let (text, color) = phase_text(info);
            ui.label(egui::RichText::new(text).color(color));
            if let UpdatePhase::Downloading { done, total, .. } = &info.phase {
                let fraction = total.filter(|t| *t > 0).map_or(0.0, |t| *done as f32 / t as f32);
                ui.add(egui::ProgressBar::new(fraction).show_percentage());
            }
            if let UpdatePhase::Ready { notes, .. } | UpdatePhase::Installed { notes, .. } = &info.phase
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
                if matches!(info.phase, UpdatePhase::Installed { .. })
                    && ui
                        .add(egui::Button::new(egui::RichText::new("Restart now").strong()).fill(theme::ACCENT))
                        .clicked()
                {
                    requests.push(Request::InstallUpdateNow);
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
            if !info.source_text.is_empty() {
                ui.label(egui::RichText::new(&info.source_text).small().color(theme::TEXT_WEAK));
            }
            ui.add_space(8.0);
            ui.separator();
            sharing(ui, info, requests);
        });
    if !open {
        view.open = false;
    }
}

/// The "Share on your network" part of the window.
fn sharing(ui: &mut egui::Ui, info: &UpdateInfo, requests: &mut Vec<Request>) {
    ui.label(egui::RichText::new("Use MOTIX on other PCs").strong());
    if let Some(folder) = &info.sharing.running_from {
        ui.label(format!(
            "This copy runs from the shared folder {folder}. Every PC that starts MOTIX from there gets \
             new versions automatically."
        ));
        return;
    }
    ui.label(
        "Put MOTIX in a shared folder and start it from any PC on your network. New versions are \
         installed there once, for every PC.",
    );
    match &info.sharing.last_result {
        Some(Ok(folder)) => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("MOTIX is ready in {folder}.")).color(theme::AUDIO));
            let name = folder.rsplit(['\\', '/']).find(|s| !s.is_empty()).unwrap_or("MOTIX");
            let pc = info.sharing.pc_name.as_deref().unwrap_or("THIS-PC");
            ui.label(
                egui::RichText::new(format!(
                    "Next, share that folder: right-click it in File Explorer > Properties > Sharing > \
                     Share…, add the people who use the other PCs and give them Read/Write (so any PC can \
                     install updates). Then on each PC open \\\\{pc}\\{name}\\MOTIX.exe \
                     (right-click it > Pin to Start makes it easy to find next time)."
                ))
                .color(theme::TEXT_WEAK),
            );
        }
        Some(Err(why)) => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("Couldn't share MOTIX: {why}")).color(theme::PLAYHEAD));
        }
        None => {}
    }
    ui.add_space(4.0);
    if ui
        .button("Choose a folder to share MOTIX from…")
        .on_hover_text("Pick an empty folder on this PC. MOTIX copies itself there with a launcher, MOTIX.exe.")
        .clicked()
    {
        requests.push(Request::ShareOnNetwork);
    }
}

/// The banner shown at the top of the window when an update is ready.
pub(crate) fn banner(ui: &mut egui::Ui, view: &mut UpdatesView, info: &UpdateInfo, requests: &mut Vec<Request>) {
    if let UpdatePhase::Installed { version, .. } = &info.phase {
        installed_banner(ui, view, version, requests);
        return;
    }
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

/// The banner for a shared copy: the new version is already installed.
fn installed_banner(ui: &mut egui::Ui, view: &mut UpdatesView, version: &str, requests: &mut Vec<Request>) {
    if view.banner_dismissed_for.as_deref() == Some(version) {
        return;
    }
    egui::Frame::new()
        .fill(theme::ACCENT.linear_multiply(0.25))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("MOTIX {version} is installed. Restart to use it.")).strong());
                if ui.button("Restart now").clicked() {
                    requests.push(Request::InstallUpdateNow);
                }
                if ui
                    .button("Later")
                    .on_hover_text("You'll get it the next time you start MOTIX")
                    .clicked()
                {
                    view.banner_dismissed_for = Some(version.to_owned());
                }
                if ui.button("What's new").clicked() {
                    view.open = true;
                }
            });
        });
}

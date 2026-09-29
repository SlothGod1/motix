//! MOTIX desktop application entry point.
//!
//! Thin host around [`motix_ui::MotixUi`]: creates the window with a GPU renderer (wgpu),
//! performs the operating-system services the UI asks for (file pickers, quitting), and
//! runs the background updater ([`motix_update::Updater`]).

// No console window behind the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use motix_app::{UpdateInfo, UpdatePhase};
use motix_ui::{MotixUi, Request};
use motix_update::{Config, Phase, Updater, UreqHttp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// The version stamped by the release build (`MOTIX_VERSION`, e.g. `0.1.0-preview.15`).
/// Local developer builds have none and don't update themselves.
const RELEASE_VERSION: Option<&str> = option_env!("MOTIX_VERSION");
/// Where official releases are published.
const RELEASE_REPO: &str = "SlothGod1/motix";

fn version_text() -> String {
    RELEASE_VERSION.map_or_else(|| format!("{}-dev", env!("CARGO_PKG_VERSION")), str::to_owned)
}

/// Per-user settings that outlive a session.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    auto_check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_check_updates: true,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("MOTIX").join("settings.json"))
}

impl Settings {
    fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Some(path) = settings_path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(json) = serde_json::to_vec_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

fn start_updater(settings: &Settings) -> Result<Updater, String> {
    let version = RELEASE_VERSION.ok_or("This is a developer build, so it doesn't update itself.")?;
    let current = semver::Version::parse(version)
        .map_err(|_| "This build's version number is unusual, so automatic updates are off.")?;
    let exe =
        std::env::current_exe().map_err(|_| "MOTIX couldn't find its own folder, so automatic updates are off.")?;
    let install_dir = exe.parent().ok_or("MOTIX couldn't find its own folder.")?.to_path_buf();
    let download_dir = dirs::data_local_dir()
        .map(|d| d.join("MOTIX").join("updates"))
        .ok_or("MOTIX couldn't find a place to keep downloads.")?;
    let config = Config {
        repo: RELEASE_REPO.to_owned(),
        current,
        archive_name: motix_update::platform_archive_name().to_owned(),
        include_prereleases: true,
        download_dir,
        install_dir,
        trusted_keys: motix_update::TRUSTED_KEYS.to_vec(),
        auto_check: settings.auto_check_updates,
        interval: motix_update::CHECK_INTERVAL,
    };
    let http = UreqHttp::new(&format!("MOTIX/{version} (+https://github.com/{RELEASE_REPO})"));
    Ok(Updater::start(config, http))
}

fn ago(t: SystemTime) -> String {
    match SystemTime::now().duration_since(t).map(|d| d.as_secs() / 60) {
        Ok(0) | Err(_) => "just now".to_owned(),
        Ok(1) => "1 minute ago".to_owned(),
        Ok(m) => format!("{m} minutes ago"),
    }
}

struct MotixApp {
    ui: MotixUi,
    settings: Settings,
    updater: Result<Updater, String>,
    install_on_exit: bool,
    install_error: Option<String>,
}

impl MotixApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let gpu_info = cc.wgpu_render_state.as_ref().map_or_else(
            || "GPU: unavailable".to_owned(),
            |rs| {
                let info = rs.adapter.get_info();
                format!("{} · {:?}", info.name, info.backend)
            },
        );
        let settings = Settings::load();
        let updater = start_updater(&settings);
        let mut ui = MotixUi::new(gpu_info);
        if std::env::args().any(|a| a == "--updated") {
            ui.state_mut().status = format!("MOTIX was updated to version {}.", version_text());
        }
        Self {
            ui,
            settings,
            updater,
            install_on_exit: false,
            install_error: None,
        }
    }

    fn update_info(&self) -> UpdateInfo {
        let mut info = UpdateInfo {
            current_version: version_text(),
            auto_check: self.settings.auto_check_updates,
            install_on_exit: self.install_on_exit,
            ..UpdateInfo::default()
        };
        match &self.updater {
            Err(reason) => info.phase = UpdatePhase::Disabled { reason: reason.clone() },
            Ok(updater) => {
                let status = updater.status();
                info.last_checked = status.last_checked.map(ago);
                info.phase = match status.phase {
                    Phase::Idle => UpdatePhase::Idle,
                    Phase::Checking => UpdatePhase::Checking,
                    Phase::UpToDate => UpdatePhase::UpToDate,
                    Phase::Downloading { version, done, total } => UpdatePhase::Downloading { version, done, total },
                    Phase::Ready { version, notes } => UpdatePhase::Ready { version, notes },
                    Phase::Installing => UpdatePhase::Installing,
                    Phase::Failed(e) => UpdatePhase::Failed { message: e.to_string() },
                };
                if let Some(e) = &self.install_error {
                    info.phase = UpdatePhase::Failed { message: e.clone() };
                }
            }
        }
        info
    }

    fn pick_media(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Import media into MOTIX")
            .add_filter(
                "Media",
                &[
                    "mp4", "mov", "m4v", "mkv", "webm", "avi", "mts", "m2ts", "mxf", "3gp", "wav", "mp3", "aac", "m4a",
                    "flac", "ogg", "opus", "aif", "aiff", "png", "jpg", "jpeg", "webp", "gif", "tif", "tiff", "exr",
                    "heic", "heif", "bmp",
                ],
            )
            .add_filter("All files", &["*"])
            .pick_files();
        if let Some(files) = picked {
            self.ui.import(files);
        }
    }
}

impl eframe::App for MotixApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui.set_update_info(self.update_info());
        self.ui.show(ui);
        // Background download progress and "update ready" need a refresh now and then.
        ui.ctx().request_repaint_after(Duration::from_secs(1));
        for request in self.ui.take_requests() {
            match request {
                Request::PickMediaFiles => self.pick_media(),
                Request::Quit => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                Request::CheckForUpdates => {
                    if let Ok(u) = &self.updater {
                        self.install_error = None;
                        u.check_now();
                    }
                }
                Request::SetAutoCheck(on) => {
                    self.settings.auto_check_updates = on;
                    self.settings.save();
                    if let Ok(u) = &self.updater {
                        u.set_auto_check(on);
                    }
                }
                Request::InstallUpdateOnExit => self.install_on_exit = true,
                Request::InstallUpdateNow => {
                    if let Ok(u) = &self.updater {
                        match u.install_and_restart() {
                            Ok(()) => {
                                self.install_on_exit = false;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            Err(e) => self.install_error = Some(e.to_string()),
                        }
                    }
                }
            }
        }
    }

    fn on_exit(&mut self) {
        if self.install_on_exit
            && let Ok(u) = &self.updater
            && u.has_ready_update()
        {
            let _ = u.install();
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MOTIX")
            .with_app_id("motix")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("MOTIX", options, Box::new(|cc| Ok(Box::new(MotixApp::new(cc)))))
}

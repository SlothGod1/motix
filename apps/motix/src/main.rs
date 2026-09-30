//! MOTIX desktop application entry point.
//!
//! Thin host around [`motix_ui::MotixUi`]: creates the window with a GPU renderer (wgpu),
//! performs the operating-system services the UI asks for (file pickers, quitting), and
//! runs the background updater ([`motix_update::Updater`]). MOTIX can run from its own
//! folder or from a shared folder that several PCs use ([`motix_update::shared`]); the
//! copy at the top of a shared folder (`MOTIX.exe`) is the launcher, which only starts
//! the newest installed version.

// No console window behind the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use motix_app::{SharingInfo, UpdateInfo, UpdatePhase};
use motix_ui::{MotixUi, Request};
use motix_update::{Config, Layout, Phase, Source, UpdateError, Updater, UreqHttp, shared};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
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

/// How this copy is installed (its own folder, or a shared folder).
fn layout() -> Option<Layout> {
    std::env::current_exe().ok().map(|exe| Layout::detect(&exe))
}

/// Where updates come from for `layout`.
fn sources(layout: &Layout) -> Vec<Source> {
    let github = Source::GitHub {
        repo: RELEASE_REPO.to_owned(),
    };
    match layout {
        Layout::Shared { root } => vec![
            Source::Folder {
                path: root.join(shared::UPDATES),
            },
            github,
        ],
        Layout::Portable { .. } => vec![github],
    }
}

fn source_text(layout: Option<&Layout>) -> String {
    match layout {
        Some(Layout::Shared { .. }) => {
            "New versions come from the official MOTIX releases on GitHub, or from a release \
                                        copied into the shared folder's 'updates' folder. Each is checked against \
                                        MOTIX's signing key and installed once, for every PC."
                .to_owned()
        }
        _ => "Updates come from the official MOTIX releases on GitHub and are checked against MOTIX's signing key \
              before anything is installed."
            .to_owned(),
    }
}

fn pc_name() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
}

fn start_updater(settings: &Settings, layout: Option<&Layout>) -> Result<Updater, String> {
    let version = RELEASE_VERSION.ok_or("This is a developer build, so it doesn't update itself.")?;
    let current = semver::Version::parse(version)
        .map_err(|_| "This build's version number is unusual, so automatic updates are off.")?;
    let layout = layout
        .cloned()
        .ok_or("MOTIX couldn't find its own folder, so automatic updates are off.")?;
    let download_dir = dirs::data_local_dir()
        .map(|d| d.join("MOTIX").join("updates"))
        .ok_or("MOTIX couldn't find a place to keep downloads.")?;
    let config = Config {
        sources: sources(&layout),
        current,
        archive_name: motix_update::platform_archive_name().to_owned(),
        include_prereleases: true,
        download_dir,
        layout,
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
    layout: Option<Layout>,
    share_result: Option<Result<String, String>>,
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
        let layout = layout();
        let updater = start_updater(&settings, layout.as_ref());
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
            layout,
            share_result: None,
        }
    }

    fn update_info(&self) -> UpdateInfo {
        let mut info = UpdateInfo {
            current_version: version_text(),
            auto_check: self.settings.auto_check_updates,
            install_on_exit: self.install_on_exit,
            source_text: source_text(self.layout.as_ref()),
            sharing: SharingInfo {
                running_from: match &self.layout {
                    Some(Layout::Shared { root }) => Some(root.display().to_string()),
                    _ => None,
                },
                last_result: self.share_result.clone(),
                pc_name: pc_name(),
            },
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
                    Phase::Installed { version, notes } => UpdatePhase::Installed { version, notes },
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

    /// Help > Share MOTIX on your network…: copies this MOTIX into a folder the user
    /// picks, with the launcher, so PCs on the network can run it from there.
    fn share_on_network(&mut self) {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose an empty folder to share MOTIX from")
            .pick_folder()
        else {
            return;
        };
        let version = semver::Version::parse(&version_text()).unwrap_or_else(|_| semver::Version::new(0, 0, 0));
        let from = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(std::path::Path::to_path_buf));
        let result = match from {
            None => Err("MOTIX couldn't find its own folder".to_owned()),
            Some(from) => shared::create(&folder, &from, &version).map_err(|e| match e {
                UpdateError::Disk(why) => why,
                other => other.to_string(),
            }),
        };
        self.share_result = Some(result.map(|()| folder.display().to_string()));
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
                Request::ShareOnNetwork => self.share_on_network(),
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

/// Started as `MOTIX.exe` at the top of a shared folder: start the newest installed
/// version (passing on anything we were given, such as `--updated`) and exit.
fn run_as_launcher(exe: &Path) -> ExitCode {
    let fail = |message: &str| {
        eprintln!("{message}");
        rfd::MessageDialog::new()
            .set_title("MOTIX")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .show();
        ExitCode::FAILURE
    };
    let Some(root) = exe.parent() else {
        return fail("MOTIX couldn't find the folder it was started from.");
    };
    let Some((version, program)) = shared::pick(root) else {
        return fail(
            "MOTIX isn't installed in this shared folder any more (its 'versions' folder is missing or empty).\n\n\
             Open MOTIX from its download folder and choose Help > Share MOTIX on your network to set it up again.",
        );
    };
    match std::process::Command::new(&program)
        .args(std::env::args_os().skip(1))
        .current_dir(program.parent().unwrap_or(root))
        .spawn()
    {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("MOTIX {version} couldn't be started: {e}")),
    }
}

fn main() -> ExitCode {
    if let Ok(exe) = std::env::current_exe()
        && shared::is_launcher(&exe)
    {
        return run_as_launcher(&exe);
    }
    match run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("MOTIX couldn't open its window: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_app() -> eframe::Result {
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

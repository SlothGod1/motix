//! The MOTIX user interface.
//!
//! Built with egui (ADR-003). All behaviour goes through [`motix_app::AppState::perform`],
//! so this crate only draws and routes input. Native services that need the operating
//! system (file pickers, closing the window) are returned to the host as [`Request`]s,
//! which keeps this crate testable without a window.

#![forbid(unsafe_code)]
// Pixel math mixes integer sizes and f32 screen coordinates by nature, and
// immediate-mode drawing functions are long by nature.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::assigning_clones,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names
)]

mod inspector;
mod lab;
mod media_panel;
mod palette;
mod panes;
mod project_prompts;
pub mod theme;
mod timeline;
mod updates;
mod viewer;

use motix_app::actions::{self, Action, Key};
use motix_app::document::RecoveryCopy;
use motix_app::{AfterSave, AppState, Outcome, Shortcut};
use motix_core::{FLICKS_PER_SECOND, Time};
use std::path::PathBuf;

pub use panes::Pane;

/// Something the host application must do on the UI's behalf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Show a native "open files" dialog and pass the result to [`MotixUi::import`].
    PickMediaFiles,
    /// Close the window.
    Quit,
    /// Check for updates now (Help > Check for updates…, or the Check now button).
    CheckForUpdates,
    /// Install the downloaded update and restart MOTIX.
    InstallUpdateNow,
    /// Install the downloaded update when MOTIX closes.
    InstallUpdateOnExit,
    /// Turn automatic update checks on or off.
    SetAutoCheck(bool),
    /// Ask for a folder and set up a shared copy of MOTIX in it (Help > Share MOTIX
    /// on your network…); report back through [`MotixUi::set_update_info`].
    ShareOnNetwork,
    /// Show a "save as" dialog for a `.motix` file named `suggested`, then call
    /// [`MotixUi::save_as`] (or [`MotixUi::cancel_save`] if the user cancels).
    PickSavePath {
        /// Suggested file name, e.g. `"Untitled project.motix"`.
        suggested: String,
    },
    /// Show an "open" dialog for `.motix` files, then call [`MotixUi::open_project`].
    PickProjectFile,
    /// Creator Lab: save this owner-password check file (it contains no password)
    /// where the user chooses.
    SaveOwnerSetup {
        /// The file's contents.
        text: String,
    },
    /// Creator Lab: pick video/image files for a collection, then call
    /// [`MotixUi::lab_add`].
    PickLabFiles {
        /// Which collection.
        collection: motix_app::lab::Collection,
    },
    /// Creator Lab: pick the upscaled version for comparison `index`, then call
    /// [`MotixUi::lab_set_upscaled`].
    PickUpscaleAfter {
        /// Which comparison.
        index: usize,
    },
    /// Ask for a network folder to get updates from (the home build server).
    PickUpdateFolder,
    /// Stop getting updates from the network folder.
    ClearUpdateFolder,
    /// Download the video helper (FFmpeg) in the background; report with
    /// [`MotixUi::set_helper_status`] and finish with [`MotixUi::attach_media`].
    DownloadVideoHelper,
}

/// Where getting the video helper stands (set by the host).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HelperStatus {
    /// Nothing to show (the helper is there, or can't be downloaded on this system).
    #[default]
    Hidden,
    /// Missing; offer to download it.
    Offer,
    /// Downloading: bytes so far, total if known.
    Downloading(u64, Option<u64>),
    /// The last try failed (why, in plain language).
    Failed(String),
}

/// Transient UI-only state (never part of the project document).
pub(crate) struct UiState {
    pub timeline: timeline::TimelineView,
    pub settings: inspector::SettingsFields,
    pub palette: palette::Palette,
    pub show_about: bool,
    pub updates: updates::UpdatesView,
    /// A "save changes?" question waiting for an answer.
    pub unsaved: Option<AfterSave>,
    /// What to do once the pending "save as" completes.
    pub after_save: Option<AfterSave>,
    /// A project dropped on the window, waiting for the "save changes?" answer.
    pub dropped_project: Option<PathBuf>,
    /// Crash-recovery copies offered on start-up.
    pub recovery: Vec<RecoveryCopy>,
    /// The user agreed to close (so a close request isn't intercepted again).
    pub allow_close: bool,
    /// The owner-only Creator Lab.
    pub lab: lab::LabView,
    /// The decoded picture the viewer shows.
    pub preview: viewer::Preview,
}

/// The whole MOTIX window.
pub struct MotixUi {
    state: AppState,
    ui_state: UiState,
    tree: egui_tiles::Tree<Pane>,
    requests: Vec<Request>,
    update_info: motix_app::UpdateInfo,
    gpu_info: String,
    themed: bool,
    media: MediaPlayback,
    helper: HelperStatus,
}

/// Video and sound for the viewer (absent in tests and when FFmpeg is missing).
#[derive(Default)]
struct MediaPlayback {
    tools: Option<motix_media::Tools>,
    video: Option<motix_media::VideoEngine>,
    audio: motix_media::AudioPlayer,
    wired_repaint: bool,
    last_request: Option<motix_media::VideoRequest>,
    generation: u64,
    /// Where sound started (timeline time, wall clock) and the project revision then.
    audio_anchor: Option<(motix_core::Time, std::time::Instant, u64)>,
}

impl MotixUi {
    /// Creates the UI. `gpu_info` is shown in the status bar (e.g. "NVIDIA RTX 3060 · Dx12").
    #[must_use]
    pub fn new(gpu_info: impl Into<String>) -> Self {
        Self {
            state: AppState::default(),
            ui_state: UiState {
                timeline: timeline::TimelineView::default(),
                settings: inspector::SettingsFields::default(),
                palette: palette::Palette::default(),
                show_about: false,
                updates: updates::UpdatesView::default(),
                unsaved: None,
                after_save: None,
                dropped_project: None,
                recovery: Vec::new(),
                allow_close: false,
                lab: lab::LabView::default(),
                preview: viewer::Preview::default(),
            },
            tree: panes::default_layout(),
            requests: Vec::new(),
            update_info: motix_app::UpdateInfo::default(),
            gpu_info: gpu_info.into(),
            themed: false,
            media: MediaPlayback::default(),
            helper: HelperStatus::Hidden,
        }
    }

    /// Read access to application state (for tests and the host).
    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Tells the UI where the updater is (called by the host every frame or on change).
    pub fn set_update_info(&mut self, info: motix_app::UpdateInfo) {
        self.update_info = info;
    }

    /// Whether the updates window is open.
    #[must_use]
    pub fn updates_window_open(&self) -> bool {
        self.ui_state.updates.open
    }

    /// Write access to application state (for tests and scripted demos).
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Whether the command palette is open.
    #[must_use]
    pub fn palette_open(&self) -> bool {
        self.ui_state.palette.open
    }

    /// Adds files chosen by the user (from a dialog or drag and drop). A MOTIX
    /// project among them is opened instead (asking about unsaved changes first).
    pub fn import(&mut self, paths: Vec<PathBuf>) {
        let (projects, media): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(motix_app::document::EXTENSION))
        });
        if !media.is_empty() {
            self.state.import(media);
        }
        if let Some(project) = projects.into_iter().next() {
            if self.state.is_dirty() {
                self.ui_state.dropped_project = Some(project);
                self.ui_state.unsaved = Some(AfterSave::OpenDropped);
            } else {
                let _ = self.state.open_file(&project);
            }
        }
    }

    /// Saves to the path the user chose in the "save as" dialog, then carries on with
    /// whatever was waiting for the save (new project, open, quit).
    pub fn save_as(&mut self, path: &std::path::Path) {
        let then = self.ui_state.after_save.take();
        if self.state.save_to(path).is_ok()
            && let Some(then) = then
        {
            self.continue_after(then);
        }
    }

    /// The user cancelled the "save as" dialog: nothing else happens.
    pub fn cancel_save(&mut self) {
        self.ui_state.after_save = None;
        self.ui_state.dropped_project = None;
    }

    /// Opens a project chosen in the "open" dialog (or given when MOTIX started).
    pub fn open_project(&mut self, path: &std::path::Path) {
        let _ = self.state.open_file(path);
    }

    /// Tells the UI whether to offer, or show progress of, the video helper download.
    pub fn set_helper_status(&mut self, status: HelperStatus) {
        self.helper = status;
    }

    fn helper_banner(&mut self, ui: &mut egui::Ui) {
        let text = match &self.helper {
            HelperStatus::Hidden => return,
            HelperStatus::Offer => format!(
                "To see and hear your clips, MOTIX needs its free video helper (FFmpeg, about {} MB, downloaded once).",
                motix_media::helper::DOWNLOAD_MB
            ),
            HelperStatus::Downloading(done, total) => match total {
                Some(t) if *t > 0 => format!("Downloading the video helper… {}%", done.saturating_mul(100) / t),
                _ => format!("Downloading the video helper… {} MB", done / 1_000_000),
            },
            HelperStatus::Failed(why) => format!("Couldn't get the video helper: {why}."),
        };
        egui::Frame::new()
            .fill(theme::ACCENT.linear_multiply(0.25))
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(text).strong());
                    let label = match self.helper {
                        HelperStatus::Offer => Some("Download"),
                        HelperStatus::Failed(_) => Some("Try again"),
                        _ => None,
                    };
                    if let Some(label) = label
                        && ui.button(label).clicked()
                    {
                        self.helper = HelperStatus::Downloading(0, None);
                        self.requests.push(Request::DownloadVideoHelper);
                    }
                });
            });
    }

    /// Gives the UI the FFmpeg helpers for real pictures and sound (`None`: they
    /// weren't found, and the viewer says so).
    pub fn attach_media(&mut self, tools: Option<motix_media::Tools>) {
        self.media.audio.stop();
        self.media.video = tools.clone().map(motix_media::VideoEngine::start);
        self.media.wired_repaint = false;
        self.media.last_request = None;
        self.ui_state.preview.note = tools.is_none().then(|| {
            if motix_media::helper::can_download() {
                "Video preview needs MOTIX's video helper (FFmpeg). Use the Download button at the top of \
                 the window — it's free and only needed once."
                    .to_owned()
            } else {
                "Video preview needs FFmpeg. Install it with your system's package manager (for example \
                 \u{201c}sudo apt install ffmpeg\u{201d}) and restart MOTIX."
                    .to_owned()
            }
        });
        self.media.tools = tools;
    }

    /// The clip whose decoded picture the viewer is showing, if any.
    #[must_use]
    pub fn preview_clip(&self) -> Option<motix_app::ClipId> {
        self.ui_state.preview.texture.as_ref().and(self.ui_state.preview.clip)
    }

    /// Keeps the viewer's picture and the sound in step with the playhead.
    fn update_preview(&mut self, ctx: &egui::Context) {
        let Some(engine) = &self.media.video else { return };
        if !self.media.wired_repaint {
            let c = ctx.clone();
            engine.on_new_frame(move || c.request_repaint());
            self.media.wired_repaint = true;
        }
        let s = &self.state;
        let request = motix_media::plan::video_target(&s.timeline, &s.media, s.playhead).map(|target| {
            motix_media::VideoRequest {
                target,
                playing: s.playing,
                fps: s.project.frame_rate,
            }
        });
        if request != self.media.last_request {
            engine.show(request.clone());
            self.media.last_request = request;
        }
        let (generation, frame) = engine.latest();
        if generation != self.media.generation {
            self.media.generation = generation;
            let preview = &mut self.ui_state.preview;
            match frame {
                Some(f) => {
                    let image =
                        egui::ColorImage::from_rgba_unmultiplied([f.width as usize, f.height as usize], &f.rgba);
                    match &mut preview.texture {
                        Some(t) => t.set(image, egui::TextureOptions::LINEAR),
                        None => {
                            preview.texture =
                                Some(ctx.load_texture("motix_preview", image, egui::TextureOptions::LINEAR));
                        }
                    }
                    preview.clip = Some(f.clip);
                }
                None => preview.clip = None,
            }
            if let Some(why) = engine.problem() {
                preview.note = Some(why);
            }
        }
        self.update_sound();
    }

    fn update_sound(&mut self) {
        let Some(tools) = self.media.tools.clone() else { return };
        let s = &self.state;
        if !s.playing {
            if self.media.audio_anchor.take().is_some() {
                self.media.audio.stop();
            }
            return;
        }
        let restart = match self.media.audio_anchor {
            None => true,
            Some((t0, started, revision)) => {
                let expected = motix_media::plan::seconds(t0) + started.elapsed().as_secs_f64();
                let now = motix_media::plan::seconds(s.playhead);
                revision != s.revision()
                    || (now - expected).abs() > 0.5
                    || now - motix_media::plan::seconds(t0) > motix_media::plan::AUDIO_WINDOW_SECONDS - 5.0
            }
        };
        if restart {
            let plan = motix_media::plan::audio_plan(&s.timeline, &s.media, s.playhead);
            if let Err(why) = self.media.audio.play(&tools, &plan) {
                self.ui_state.preview.note = Some(why);
            }
            self.media.audio_anchor = Some((s.playhead, std::time::Instant::now(), s.revision()));
        }
    }

    /// Tells the Creator Lab where to keep its library and remember the owner on this
    /// PC (a per-user settings folder); opens it if this PC remembers the owner.
    pub fn set_lab_storage(&mut self, dir: &std::path::Path) {
        self.ui_state.lab.set_storage(dir);
    }

    /// Adds files picked for a Creator Lab collection.
    pub fn lab_add(&mut self, collection: motix_app::lab::Collection, paths: Vec<PathBuf>) {
        self.ui_state.lab.add(collection, paths);
    }

    /// Sets the upscaled file of a Creator Lab comparison.
    pub fn lab_set_upscaled(&mut self, index: usize, path: PathBuf) {
        self.ui_state.lab.set_after(index, path);
    }

    /// Shows the Creator Lab page (`true`) or the editor (`false`).
    pub fn show_lab(&mut self, open: bool) {
        self.ui_state.lab.open = open;
    }

    /// Whether the Creator Lab is unlocked on this PC.
    #[must_use]
    pub fn lab_unlocked(&self) -> bool {
        self.ui_state.lab.unlocked
    }

    /// The Creator Lab's library (for tests and the host).
    #[must_use]
    pub fn lab_library(&self) -> &motix_app::lab::LabLibrary {
        self.ui_state.lab.library()
    }

    /// Replaces the owner check (tests use a quick one; the app uses the built-in one).
    pub fn set_owner_check(&mut self, check: Option<motix_app::owner::OwnerCheck>) {
        self.ui_state.lab.check = check;
    }

    /// Offers to restore crash-recovery copies (newest first) found on start-up.
    pub fn offer_recovery(&mut self, copies: Vec<RecoveryCopy>) {
        self.ui_state.recovery = copies;
    }

    /// Whether a "save changes?" question is showing.
    #[must_use]
    pub fn asking_to_save(&self) -> bool {
        self.ui_state.unsaved.is_some()
    }

    fn suggested_file_name(&self) -> String {
        format!("{}.{}", self.state.project_name(), motix_app::document::EXTENSION)
    }

    fn continue_after(&mut self, then: AfterSave) {
        match then {
            AfterSave::NewProject => self.state.new_project(),
            AfterSave::OpenProject => self.requests.push(Request::PickProjectFile),
            AfterSave::OpenDropped => {
                if let Some(p) = self.ui_state.dropped_project.take() {
                    let _ = self.state.open_file(&p);
                }
            }
            AfterSave::Quit => {
                self.ui_state.allow_close = true;
                self.requests.push(Request::Quit);
            }
        }
    }

    fn project_prompts(&mut self, ctx: &egui::Context) {
        let name = self.state.project_name();
        let choice = project_prompts::unsaved(ctx, &mut self.ui_state.unsaved, &name)
            .or_else(|| project_prompts::recovery(ctx, &self.ui_state.recovery));
        match choice {
            None => {
                if self.ui_state.unsaved.is_none() && self.ui_state.after_save.is_none() {
                    // Cancelled: forget a dropped project that was waiting.
                    self.ui_state.dropped_project = None;
                }
            }
            Some(project_prompts::Choice::Save(then)) => {
                if let Some(path) = self.state.file.clone() {
                    if self.state.save_to(&path).is_ok() {
                        self.continue_after(then);
                    }
                } else {
                    self.ui_state.after_save = Some(then);
                    self.requests.push(Request::PickSavePath {
                        suggested: self.suggested_file_name(),
                    });
                }
            }
            Some(project_prompts::Choice::DontSave(then)) => self.continue_after(then),
            Some(project_prompts::Choice::Restore(path)) => {
                let _ = self.state.restore_recovery(&path);
                self.discard_recovery();
            }
            Some(project_prompts::Choice::DiscardRecovery) => {
                self.discard_recovery();
                self.state.status = "Unsaved changes from last time were discarded.".to_owned();
            }
        }
    }

    fn discard_recovery(&mut self) {
        for copy in std::mem::take(&mut self.ui_state.recovery) {
            let _ = std::fs::remove_file(copy.path);
        }
    }

    /// Requests produced since the last call, for the host to handle.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Performs an action exactly as a menu click or shortcut would.
    pub fn perform(&mut self, action: Action) {
        match self.state.perform(action) {
            Outcome::Done | Outcome::NotYet { .. } => {}
            Outcome::PickMediaFiles => self.requests.push(Request::PickMediaFiles),
            Outcome::OpenCommandPalette => self.ui_state.palette.open(),
            Outcome::ResetLayout => {
                self.tree = panes::default_layout();
                self.state.status = "Panel layout restored.".to_owned();
            }
            Outcome::ShowAbout => self.ui_state.show_about = true,
            Outcome::Quit => {
                self.ui_state.allow_close = true;
                self.requests.push(Request::Quit);
            }
            Outcome::CheckForUpdates => {
                self.ui_state.updates.open = true;
                self.requests.push(Request::CheckForUpdates);
            }
            Outcome::ShareOnNetwork => {
                self.ui_state.updates.open = true;
                self.requests.push(Request::ShareOnNetwork);
            }
            Outcome::PickSavePath { then } => {
                self.ui_state.after_save = then;
                self.requests.push(Request::PickSavePath {
                    suggested: self.suggested_file_name(),
                });
            }
            Outcome::PickProjectToOpen => self.requests.push(Request::PickProjectFile),
            Outcome::ConfirmUnsaved { then } => self.ui_state.unsaved = Some(then),
        }
    }

    /// Draws one frame. Call from the host's per-frame UI callback.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.themed {
            theme::apply(&ctx);
            self.themed = true;
        }
        // Closing the window with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested()) && self.state.is_dirty() && !self.ui_state.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.ui_state.unsaved = Some(AfterSave::Quit);
        }
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);
        self.tick_playback(&ctx);
        self.update_preview(&ctx);

        egui::Panel::top("motix_top_bar").show(ui, |ui| {
            self.top_bar(ui);
            updates::banner(ui, &mut self.ui_state.updates, &self.update_info, &mut self.requests);
            self.helper_banner(ui);
        });
        egui::Panel::bottom("motix_status_bar").show(ui, |ui| self.status_bar(ui));
        if self.ui_state.lab.open {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(theme::BG))
                .show(ui, |ui| lab::page(ui, &mut self.ui_state.lab, &mut self.requests));
        } else {
            self.editor(ui);
        }

        Self::drop_overlay(&ctx);
        if let Some(action) = self.ui_state.palette.show(&ctx) {
            self.perform(action);
        }
        self.match_modal(&ctx);
        self.project_prompts(&ctx);
        updates::window(&ctx, &mut self.ui_state.updates, &self.update_info, &mut self.requests);
        self.about_modal(&ctx);
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(6.0))
            .show(ui, |ui| {
                let mut behavior = panes::Behavior {
                    state: &mut self.state,
                    ui_state: &mut self.ui_state,
                    actions: Vec::new(),
                };
                self.tree.ui(&mut behavior, ui);
                let queued = std::mem::take(&mut behavior.actions);
                for a in queued {
                    self.perform(a);
                }
            });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let mut chosen = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(egui::RichText::new("MOTIX").strong().color(theme::ACCENT).size(16.0));
            ui.add_space(8.0);
            let lab_open = self.ui_state.lab.open;
            if ui.selectable_label(!lab_open, "Editor").clicked() {
                self.ui_state.lab.open = false;
            }
            let lab_label = if self.ui_state.lab.unlocked {
                "Creator Lab"
            } else {
                "\u{1f512} Creator Lab"
            };
            if ui
                .selectable_label(lab_open, lab_label)
                .on_hover_text("Teach MOTIX your style (owner only)")
                .clicked()
            {
                self.ui_state.lab.open = true;
            }
            ui.separator();
            for menu in actions::MENUS {
                ui.menu_button(menu, |ui| {
                    for info in actions::ALL.iter().filter(|i| i.menu == menu) {
                        let mut text = egui::RichText::new(info.label);
                        if let Some(note) = info.availability.note() {
                            text = egui::RichText::new(format!("{}  ({note})", info.label)).color(theme::TEXT_WEAK);
                        }
                        let mut button = egui::Button::new(text);
                        if let Some(s) = info.shortcut {
                            button = button.shortcut_text(s.label());
                        }
                        if ui.add(button).clicked() {
                            chosen = Some(info.action);
                            ui.close();
                        }
                    }
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new("🔍 Search commands").shortcut_text("Ctrl+Shift+P"))
                    .on_hover_text("Find any command by typing part of its name")
                    .clicked()
                {
                    chosen = Some(Action::CommandPalette);
                }
                let name = self.state.project_name();
                if self.state.is_dirty() {
                    ui.label(egui::RichText::new(format!("{name} \u{2022}")).color(theme::TEXT))
                        .on_hover_text("Unsaved changes — press Ctrl+S to save");
                } else {
                    ui.label(egui::RichText::new(name).color(theme::TEXT_WEAK));
                }
            });
        });
        if let Some(a) = chosen {
            self.perform(a);
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.state.status).color(theme::TEXT_WEAK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("MOTIX {}", self.update_info.current_version)).color(theme::ACCENT),
                );
                if let Some(line) = self.update_info.status_line() {
                    ui.separator();
                    ui.label(egui::RichText::new(line).color(theme::AUDIO));
                }
                ui.separator();
                ui.label(egui::RichText::new(&self.gpu_info).color(theme::TEXT_WEAK));
            });
        });
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            self.import(dropped);
        }
    }

    fn drop_overlay(ctx: &egui::Context) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !hovering {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("motix_drop")));
        let rect = ctx.content_rect();
        painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(170));
        painter.rect_stroke(
            rect.shrink(12.0),
            12.0,
            egui::Stroke::new(2.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Drop to import into MOTIX",
            egui::FontId::proportional(26.0),
            theme::TEXT,
        );
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || self.ui_state.palette.open {
            return;
        }
        let mut fired = Vec::new();
        ctx.input(|input| {
            for info in actions::ALL {
                let Some(s) = info.shortcut else { continue };
                if input.modifiers.command != s.command || input.modifiers.shift != s.shift {
                    continue;
                }
                if input.key_pressed(egui_key(s)) {
                    fired.push(info.action);
                }
            }
        });
        for a in fired {
            self.perform(a);
        }
    }

    fn tick_playback(&mut self, ctx: &egui::Context) {
        if self.state.playing {
            let dt = f64::from(ctx.input(|i| i.stable_dt).min(0.1));
            let flicks = (dt * FLICKS_PER_SECOND as f64) as i64;
            self.state.advance(Time::from_flicks(flicks));
            ctx.request_repaint();
        }
    }

    fn match_modal(&mut self, ctx: &egui::Context) {
        let Some(offer) = self.state.match_offer.clone() else {
            return;
        };
        let mut choice = None;
        let r = egui::Modal::new(egui::Id::new("motix_match")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading("Match the project to this video?");
            ui.add_space(4.0);
            ui.label(format!(
                "\u{201c}{}\u{201d} is different from your project settings:",
                offer.name
            ));
            ui.add_space(6.0);
            egui::Grid::new("motix_match_grid")
                .num_columns(3)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.label(egui::RichText::new("Project now").color(theme::TEXT_WEAK));
                    ui.label(egui::RichText::new("This video").color(theme::TEXT_WEAK));
                    ui.end_row();
                    for (what, now, clip) in offer.differences(&self.state.project) {
                        ui.label(what);
                        ui.label(now);
                        ui.label(egui::RichText::new(clip).strong());
                        ui.end_row();
                    }
                });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!(
                    "If you keep your settings, clips of other sizes are placed with \u{201c}{}\u{201d}. \
                     You can change any setting later in the Inspector.",
                    self.state.project.default_fit.label()
                ))
                .color(theme::TEXT_WEAK),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new(egui::RichText::new("Match project").strong()).fill(theme::ACCENT))
                    .clicked()
                {
                    choice = Some(true);
                }
                if ui.button("Keep current settings").clicked() {
                    choice = Some(false);
                }
            });
        });
        if r.should_close() && choice.is_none() {
            choice = Some(false);
        }
        match choice {
            Some(true) => self.state.accept_match(),
            Some(false) => self.state.decline_match(),
            None => {}
        }
    }

    fn about_modal(&mut self, ctx: &egui::Context) {
        if !self.ui_state.show_about {
            return;
        }
        let r = egui::Modal::new(egui::Id::new("motix_about")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading(egui::RichText::new("MOTIX").color(theme::ACCENT));
            ui.label(format!(
                "Version {} — developer preview",
                self.update_info.current_version
            ));
            ui.add_space(6.0);
            ui.label("Local-first video editing and motion graphics for social media.");
            ui.label(egui::RichText::new("Free software under the GNU GPL v3 or later.").color(theme::TEXT_WEAK));
            ui.add_space(8.0);
            ui.button("Close").clicked()
        });
        if r.inner || r.should_close() {
            self.ui_state.show_about = false;
        }
    }
}

/// Maps a toolkit-independent key to egui's key.
pub(crate) fn egui_key(s: Shortcut) -> egui::Key {
    match s.key {
        Key::Space => egui::Key::Space,
        Key::ArrowLeft => egui::Key::ArrowLeft,
        Key::ArrowRight => egui::Key::ArrowRight,
        Key::Home => egui::Key::Home,
        Key::Escape => egui::Key::Escape,
        Key::Delete => egui::Key::Delete,
        Key::ArrowUp => egui::Key::ArrowUp,
        Key::ArrowDown => egui::Key::ArrowDown,
        Key::Char(c) => egui::Key::from_name(&c.to_string()).unwrap_or(egui::Key::F35),
    }
}

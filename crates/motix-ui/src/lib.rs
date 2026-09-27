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
    clippy::too_many_lines
)]

mod inspector;
mod media_panel;
mod palette;
mod panes;
pub mod theme;
mod timeline;
mod viewer;

use motix_app::actions::{self, Action, Availability, Key};
use motix_app::{AppState, Outcome, Shortcut};
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
}

/// Transient UI-only state (never part of the project document).
pub(crate) struct UiState {
    pub timeline_px_per_second: f32,
    pub timeline_scroll_seconds: f32,
    pub palette: palette::Palette,
    pub show_about: bool,
}

/// The whole MOTIX window.
pub struct MotixUi {
    state: AppState,
    ui_state: UiState,
    tree: egui_tiles::Tree<Pane>,
    requests: Vec<Request>,
    gpu_info: String,
    themed: bool,
}

impl MotixUi {
    /// Creates the UI. `gpu_info` is shown in the status bar (e.g. "NVIDIA RTX 3060 · Dx12").
    #[must_use]
    pub fn new(gpu_info: impl Into<String>) -> Self {
        Self {
            state: AppState::default(),
            ui_state: UiState {
                timeline_px_per_second: 80.0,
                timeline_scroll_seconds: 0.0,
                palette: palette::Palette::default(),
                show_about: false,
            },
            tree: panes::default_layout(),
            requests: Vec::new(),
            gpu_info: gpu_info.into(),
            themed: false,
        }
    }

    /// Read access to application state (for tests and the host).
    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Whether the command palette is open.
    #[must_use]
    pub fn palette_open(&self) -> bool {
        self.ui_state.palette.open
    }

    /// Adds files chosen by the user (from a dialog or drag and drop).
    pub fn import(&mut self, paths: Vec<PathBuf>) {
        let r = self.state.media.add_paths(paths);
        self.state.status = match (r.added, r.duplicates, r.unsupported) {
            (0, 0, 0) => "Nothing was imported.".to_owned(),
            (a, d, u) => {
                let mut parts = vec![format!("Imported {a} file{}", if a == 1 { "" } else { "s" })];
                if d > 0 {
                    parts.push(format!("{d} already in the project"));
                }
                if u > 0 {
                    parts.push(format!("{u} not a supported media file"));
                }
                parts.join(" · ")
            }
        };
        if self.state.selected_media.is_none() && !self.state.media.items().is_empty() {
            self.state.selected_media = Some(0);
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
            Outcome::Quit => self.requests.push(Request::Quit),
        }
    }

    /// Draws one frame. Call from the host's per-frame UI callback.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.themed {
            theme::apply(&ctx);
            self.themed = true;
        }
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);
        self.tick_playback(&ctx);

        egui::Panel::top("motix_top_bar").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("motix_status_bar").show(ui, |ui| self.status_bar(ui));
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

        Self::drop_overlay(&ctx);
        if let Some(action) = self.ui_state.palette.show(&ctx) {
            self.perform(action);
        }
        self.about_modal(&ctx);
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let mut chosen = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(egui::RichText::new("MOTIX").strong().color(theme::ACCENT).size(16.0));
            ui.add_space(8.0);
            for menu in ["File", "Edit", "Playback", "View", "Collaborate", "Help"] {
                ui.menu_button(menu, |ui| {
                    for info in actions::ALL.iter().filter(|i| i.menu == menu) {
                        let mut text = egui::RichText::new(info.label);
                        if let Availability::Planned(m) = info.availability {
                            text = egui::RichText::new(format!("{}  ({m})", info.label)).color(theme::TEXT_WEAK);
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
                ui.label(egui::RichText::new("Untitled project").color(theme::TEXT_WEAK));
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
                ui.label(egui::RichText::new("Developer preview · M1").color(theme::ACCENT));
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

    fn about_modal(&mut self, ctx: &egui::Context) {
        if !self.ui_state.show_about {
            return;
        }
        let r = egui::Modal::new(egui::Id::new("motix_about")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading(egui::RichText::new("MOTIX").color(theme::ACCENT));
            ui.label(format!(
                "Version {} — developer preview (milestone M1)",
                env!("CARGO_PKG_VERSION")
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
        Key::Char(c) => egui::Key::from_name(&c.to_string()).unwrap_or(egui::Key::F35),
    }
}

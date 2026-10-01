//! The Creator Lab page: owner-only (password, ADR-033). Collects the examples MOTIX's
//! AI will learn the owner's taste from: edits they love, upscale comparisons, and
//! films/episodes to make scene packs from.

use crate::{Request, theme};
use motix_app::lab::{Better, ClipWish, Collection, LabLibrary, Quality, display_name};
use motix_app::owner::{self, OwnerCheck};
use std::path::{Path, PathBuf};

/// UI state of the Lab.
pub(crate) struct LabView {
    /// The Lab page is showing (instead of the editor).
    pub open: bool,
    /// The owner password was accepted on this PC.
    pub unlocked: bool,
    /// The owner check this MOTIX accepts (normally [`owner::BUILT_IN`]).
    pub check: Option<OwnerCheck>,
    password: String,
    again: String,
    message: Option<(String, bool)>,
    tab: Collection,
    library: LabLibrary,
    /// Where the library is kept (set by the host).
    pub library_path: Option<PathBuf>,
    /// Where this PC remembers the unlock (set by the host).
    pub remember_path: Option<PathBuf>,
}

impl Default for LabView {
    fn default() -> Self {
        Self {
            open: false,
            unlocked: false,
            check: owner::BUILT_IN,
            password: String::new(),
            again: String::new(),
            message: None,
            tab: Collection::LovedEdits,
            library: LabLibrary::default(),
            library_path: None,
            remember_path: None,
        }
    }
}

impl LabView {
    /// Sets where things are stored and opens the Lab if this PC remembers the owner.
    pub(crate) fn set_storage(&mut self, dir: &Path) {
        self.library_path = Some(dir.join("creator-lab-library.json"));
        self.remember_path = Some(dir.join("creator-lab-unlock.txt"));
        let remembered = self
            .remember_path
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|t| owner::remembered_ok(&t, self.check.as_ref()));
        if remembered {
            self.unlock_now();
        }
    }

    fn unlock_now(&mut self) {
        self.unlocked = true;
        if let Some(p) = &self.library_path {
            self.library = LabLibrary::load(p);
        }
    }

    fn save(&mut self) {
        if let Some(p) = &self.library_path
            && let Err(e) = self.library.save(p)
        {
            self.message = Some((format!("Couldn't save the Lab library: {e}."), true));
        }
    }

    /// Adds files picked for a collection.
    pub(crate) fn add(&mut self, collection: Collection, paths: Vec<PathBuf>) {
        if !self.unlocked {
            return;
        }
        let n = self.library.add(collection, paths);
        self.tab = collection;
        self.message = Some((format!("Added {n} file{}.", if n == 1 { "" } else { "s" }), false));
        self.save();
    }

    /// Sets the upscaled side of comparison `index`.
    pub(crate) fn set_after(&mut self, index: usize, path: PathBuf) {
        if self.unlocked
            && let Some(pair) = self.library.upscales.get_mut(index)
        {
            pair.after = Some(path);
            self.save();
        }
    }

    /// The library (for tests).
    pub(crate) fn library(&self) -> &LabLibrary {
        &self.library
    }
}

fn message(ui: &mut egui::Ui, msg: Option<&(String, bool)>) {
    if let Some((text, error)) = msg {
        ui.label(egui::RichText::new(text).color(if *error { theme::PLAYHEAD } else { theme::AUDIO }));
    }
}

/// Draws the Lab page.
pub(crate) fn page(ui: &mut egui::Ui, view: &mut LabView, requests: &mut Vec<Request>) {
    egui::Frame::new()
        .fill(theme::BG)
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.heading(egui::RichText::new("Creator Lab").strong());
            ui.label(
                egui::RichText::new(
                    "Teach MOTIX your style: show it edits you love, upscales you like, and the films you \
                     want scene packs from. Everything stays on this PC.",
                )
                .color(theme::TEXT_WEAK),
            );
            ui.add_space(10.0);
            if view.unlocked {
                library(ui, view, requests);
            } else {
                lock_screen(ui, view, requests);
            }
        });
}

fn lock_screen(ui: &mut egui::Ui, view: &mut LabView, requests: &mut Vec<Request>) {
    ui.set_max_width(520.0);
    ui.label(egui::RichText::new("🔒 Only the owner can open the Creator Lab.").strong());
    ui.add_space(6.0);
    if let Some(check) = view.check {
        ui.label("Owner password");
        let field = ui.add(
            egui::TextEdit::singleline(&mut view.password)
                .password(true)
                .hint_text("Type your owner password")
                .desired_width(320.0),
        );
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Unlock").clicked() || enter {
            match owner::unlock(&view.password, Some(&check)) {
                Ok(key) => {
                    if let Some(p) = &view.remember_path {
                        if let Some(dir) = p.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        let _ = std::fs::write(p, owner::remembered_text(&key));
                    }
                    view.password.clear();
                    view.message = Some(("Welcome back. This PC will remember you.".to_owned(), false));
                    view.unlock_now();
                }
                Err(e) => view.message = Some((e.to_string(), true)),
            }
        }
        message(ui, view.message.as_ref());
        return;
    }
    ui.label(
        "This version of MOTIX doesn't have an owner password yet. If you're the owner, create one \
         below. MOTIX saves a small file that proves the password without containing it; send that \
         file to Claude to build into the next version. Then type your password here to open the Lab.",
    );
    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::singleline(&mut view.password)
            .password(true)
            .hint_text(format!(
                "New password (at least {} characters)",
                owner::MIN_PASSWORD_CHARS
            ))
            .desired_width(320.0),
    );
    ui.add(
        egui::TextEdit::singleline(&mut view.again)
            .password(true)
            .hint_text("Type it again")
            .desired_width(320.0),
    );
    ui.label(
        egui::RichText::new(
            "Tip: a few unrelated words (like \u{201c}purple ladder sunset river\u{201d}) is long, \
             memorable and very hard to guess. Never paste it into a chat.",
        )
        .small()
        .color(theme::TEXT_WEAK),
    );
    if ui.button("Create owner password…").clicked() {
        match owner::create(&view.password, &view.again) {
            Ok(check) => {
                view.password.clear();
                view.again.clear();
                requests.push(Request::SaveOwnerSetup {
                    text: owner::setup_file_text(&check),
                });
                view.message = Some((
                    "Choose where to save the file (for example your Editing Software folder), then tell \
                     Claude where it is."
                        .to_owned(),
                    false,
                ));
            }
            Err(e) => view.message = Some((e.to_string(), true)),
        }
    }
    message(ui, view.message.as_ref());
}

fn chips<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    all: &[T],
    chosen: &mut Vec<T>,
    label: impl Fn(T) -> &'static str,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for &q in all {
            let on = chosen.contains(&q);
            if ui.selectable_label(on, label(q)).clicked() {
                if on {
                    chosen.retain(|x| *x != q);
                } else {
                    chosen.push(q);
                }
                changed = true;
            }
        }
    });
    changed
}

fn library(ui: &mut egui::Ui, view: &mut LabView, requests: &mut Vec<Request>) {
    let lib = &view.library;
    let tabs = [
        (Collection::LovedEdits, format!("Edits I love ({})", lib.loved.len())),
        (
            Collection::Upscales,
            format!("Upscale comparisons ({})", lib.upscales.len()),
        ),
        (Collection::SceneSources, format!("Scene packs ({})", lib.sources.len())),
    ];
    ui.horizontal(|ui| {
        for (c, label) in tabs {
            if ui.selectable_label(view.tab == c, label).clicked() {
                view.tab = c;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Lock")
                .on_hover_text("Forget the password on this PC")
                .clicked()
            {
                if let Some(p) = &view.remember_path {
                    let _ = std::fs::remove_file(p);
                }
                view.unlocked = false;
                view.library = LabLibrary::default();
                view.message = None;
            }
        });
    });
    ui.separator();
    let (intro, add_label) = match view.tab {
        Collection::LovedEdits => (
            "Add edits you love and mark what's great about them. MOTIX will learn your taste in pacing, \
             transitions and style from these.",
            "Add edits…",
        ),
        Collection::Upscales => (
            "Add an original, then its upscaled version, and say which looks better and why. MOTIX will use \
             these to tune its upscaling to what you like.",
            "Add originals…",
        ),
        Collection::SceneSources => (
            "Add films or episodes, say who to follow and what to collect. MOTIX will cut them into scene \
             packs: every shot of that person, pacing shots, close-ups, transition moments.",
            "Add films or episodes…",
        ),
    };
    ui.label(egui::RichText::new(intro).color(theme::TEXT_WEAK));
    ui.label(
        egui::RichText::new(
            "Coming next: MOTIX analyses these on this PC (shot changes, beats, faces, motion) and suggests \
             edits, upscale settings and scene packs in your style.",
        )
        .small()
        .color(theme::TEXT_WEAK),
    );
    ui.horizontal(|ui| {
        if ui.button(add_label).clicked() {
            requests.push(Request::PickLabFiles { collection: view.tab });
        }
        message(ui, view.message.as_ref());
    });
    ui.add_space(6.0);

    let mut changed = false;
    let mut remove = None;
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match view.tab {
        Collection::LovedEdits => {
            for (i, e) in view.library.loved.iter_mut().enumerate() {
                card(ui, |ui| {
                    title_row(ui, &display_name(&e.path), &mut remove, i);
                    changed |= chips(ui, &Quality::ALL, &mut e.qualities, Quality::label);
                    changed |= ui
                        .add(
                            egui::TextEdit::multiline(&mut e.notes)
                                .hint_text("What do you love about it? (e.g. \u{201c}the whip pan at 0:12 into the slow-mo\u{201d})")
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        )
                        .changed();
                });
            }
        }
        Collection::Upscales => {
            for (i, p) in view.library.upscales.iter_mut().enumerate() {
                card(ui, |ui| {
                    title_row(ui, &display_name(&p.before), &mut remove, i);
                    ui.horizontal(|ui| {
                        let after = p.after.as_deref().map_or_else(|| "not chosen yet".to_owned(), display_name);
                        ui.label(format!("Upscaled: {after}"));
                        if ui.small_button("Choose…").clicked() {
                            requests.push(Request::PickUpscaleAfter { index: i });
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Better:");
                        for (v, label) in [(Some(Better::Before), "Original"), (Some(Better::After), "Upscaled"), (None, "Not sure")] {
                            if ui.selectable_label(p.better == v, label).clicked() {
                                p.better = v;
                                changed = true;
                            }
                        }
                    });
                    changed |= ui
                        .add(
                            egui::TextEdit::multiline(&mut p.notes)
                                .hint_text("What's better or worse? (sharpness, faces, skin, noise, artefacts…)")
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        )
                        .changed();
                });
            }
        }
        Collection::SceneSources => {
            for (i, s) in view.library.sources.iter_mut().enumerate() {
                card(ui, |ui| {
                    title_row(ui, &display_name(&s.path), &mut remove, i);
                    ui.horizontal(|ui| {
                        ui.label("Follow:");
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut s.person)
                                    .hint_text("Character or actor name")
                                    .desired_width(260.0),
                            )
                            .changed();
                    });
                    changed |= chips(ui, &ClipWish::ALL, &mut s.wishes, ClipWish::label);
                });
            }
        }
    });
    if let Some(i) = remove {
        match view.tab {
            Collection::LovedEdits => drop(view.library.loved.remove(i)),
            Collection::Upscales => drop(view.library.upscales.remove(i)),
            Collection::SceneSources => drop(view.library.sources.remove(i)),
        }
        changed = true;
    }
    if changed {
        view.save();
    }
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.add_space(6.0);
}

fn title_row(ui: &mut egui::Ui, name: &str, remove: &mut Option<usize>, i: usize) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(name).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Remove").clicked() {
                *remove = Some(i);
            }
        });
    });
}

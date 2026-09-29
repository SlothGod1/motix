//! The inspector: the selected clip, the selected media's details, and project settings
//! (typed size and frame rate, colour output, and how different-sized clips are placed).

use crate::theme;
use motix_app::probe::{Primaries, Transfer};
use motix_app::state::timecode;
use motix_app::{Action, AppState, ColorOutput, FitMode, MediaItem, ProjectSettings, SIZE_PRESETS, TrackKind};

/// Text-field contents for project settings, kept between frames while typing.
#[derive(Default)]
pub(crate) struct SettingsFields {
    width: String,
    height: String,
    fps: String,
    synced: Option<ProjectSettings>,
    error: Option<String>,
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut AppState, fields: &mut SettingsFields, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        clip_section(ui, state);
        if let Some(item) = state.selected_media.and_then(|m| state.media.get(m)).cloned() {
            media_section(ui, state, &item);
        }
        project_section(ui, state, fields, actions);
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

fn note(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(egui::RichText::new(text.into()).small().color(theme::TEXT_WEAK));
}

fn clip_section(ui: &mut egui::Ui, state: &mut AppState) {
    let Some(&first) = state.selected_clips.first() else {
        return;
    };
    let Some(clip) = state.timeline.clip(first).cloned() else {
        return;
    };
    let group = state.timeline.linked_group(first);
    let kind = state.timeline.track(clip.track).map_or(TrackKind::Video, |t| t.kind);
    let rate = state.project.frame_rate;
    section(
        ui,
        if state.selected_clips.len() > group.len() {
            "Selected clips"
        } else {
            "Selected clip"
        },
    );
    row(ui, "Name", &clip.name);
    row(ui, "Track", &state.timeline.track_label(clip.track));
    row(ui, "Starts at", &timecode(clip.start, rate));
    row(ui, "Length", &timecode(clip.duration, rate));
    if clip.link.is_some() {
        row(ui, "Linked", &format!("with {} other clip(s)", group.len() - 1));
    }
    if kind == TrackKind::Video {
        let project = state.project.resolution;
        if let Some(size) = clip.source_size {
            if size == project {
                note(ui, format!("{size} — same size as the project."));
            } else {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Different size from the project").color(theme::IMAGE));
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Placement").color(theme::TEXT_WEAK));
                    let mut fit = clip.fit;
                    egui::ComboBox::from_id_salt(("clip_fit", clip.id.0))
                        .selected_text(fit.label())
                        .show_ui(ui, |ui| {
                            for m in FitMode::ALL {
                                ui.selectable_value(&mut fit, m, m.label())
                                    .on_hover_text(m.explanation());
                            }
                        });
                    if fit != clip.fit {
                        state.set_clip_fit(clip.id, fit);
                    }
                });
                let (_, _, w, h) = clip.fit.place(size, project);
                note(
                    ui,
                    format!(
                        "{size} shown at {:.0}×{:.0} inside {project}. {}",
                        w,
                        h,
                        clip.fit.explanation()
                    ),
                );
            }
        }
        if let Some(src) = state.media.get(clip.media).and_then(MediaItem::frame_rate)
            && src != rate
        {
            let how = if src.as_rational() > rate.as_rational() {
                "some frames are skipped"
            } else {
                "some frames are shown twice"
            };
            note(
                ui,
                format!(
                    "Recorded at {} fps; plays at the project's {} fps ({how}). Smooth frame blending comes later.",
                    src.short_label(),
                    rate.short_label()
                ),
            );
        }
    }
    ui.horizontal(|ui| {
        if ui
            .button(if clip.link.is_some() { "Unlink" } else { "Link" })
            .on_hover_text("Ctrl+L")
            .clicked()
        {
            state.perform(Action::ToggleLink);
        }
        if ui.button("Delete").on_hover_text("Delete key").clicked() {
            state.perform(Action::Delete);
        }
    });
    ui.add_space(10.0);
}

fn color_label(item: &MediaItem) -> Option<String> {
    let v = item.info.as_ref()?.video.as_ref()?;
    if item.info.as_ref().is_some_and(|i| i.still) {
        return None;
    }
    let gamut = match v.color.primaries {
        Primaries::Bt2020 => "Rec.2020",
        Primaries::Bt709 => "Rec.709",
        Primaries::P3 => "Display P3",
        Primaries::Bt601 => "Rec.601",
        Primaries::Unspecified | Primaries::Other(_) => "",
    };
    let curve = match v.color.transfer {
        Transfer::Pq => "PQ",
        Transfer::Hlg => "HLG",
        Transfer::Sdr | Transfer::Unspecified => "",
    };
    let detail: Vec<&str> = [curve, gamut].into_iter().filter(|s| !s.is_empty()).collect();
    Some(if detail.is_empty() {
        v.dynamic_range().label()
    } else {
        format!("{} ({})", v.dynamic_range().label(), detail.join(" · "))
    })
}

fn media_section(ui: &mut egui::Ui, state: &mut AppState, item: &MediaItem) {
    section(ui, "Selected media");
    row(ui, "Name", &item.name);
    row(ui, "Type", item.kind.label());
    if let Some(info) = &item.info {
        if let Some(r) = item.resolution() {
            row(ui, "Size", &format!("{r} px"));
        }
        if let Some(v) = info.video.as_ref().filter(|_| !info.still) {
            if let Some(rate) = v.frame_rate {
                let vfr = if v.variable_frame_rate { " (variable)" } else { "" };
                row(ui, "Frame rate", &format!("{} fps{vfr}", rate.short_label()));
            }
            let depth = v.bit_depth.map(|d| format!(" · {d}-bit")).unwrap_or_default();
            row(ui, "Video", &format!("{}{depth}", v.codec));
            if let Some(c) = color_label(item) {
                row(ui, "Colour", &c);
            }
        }
        if let Some(d) = info.duration.filter(|_| !info.still) {
            row(ui, "Length", &timecode(d, state.project.frame_rate));
        }
        for (i, a) in info.audio.iter().enumerate() {
            let key = if info.audio.len() > 1 {
                format!("Audio {}", i + 1)
            } else {
                "Audio".to_owned()
            };
            let ch = match a.channels {
                1 => "mono".to_owned(),
                2 => "stereo".to_owned(),
                n => format!("{n} ch"),
            };
            row(
                ui,
                &key,
                &format!("{} · {ch} · {:.1} kHz", a.codec, f64::from(a.sample_rate) / 1000.0),
            );
        }
        row(
            ui,
            "File",
            &format!("{} · {}", info.container.label(), item.size_label()),
        );
    } else {
        row(ui, "File size", &item.size_label());
    }
    if let Some(n) = &item.probe_note {
        note(ui, n.clone());
    }
    ui.label(
        egui::RichText::new(item.path.display().to_string())
            .small()
            .color(theme::TEXT_WEAK),
    );
    ui.horizontal(|ui| {
        if ui
            .button("Add to timeline")
            .on_hover_text("Or double-click / drag it")
            .clicked()
        {
            let _ = state.add_to_timeline(item.id, None, None);
        }
        if item.resolution().is_some()
            && ui
                .button("Match project to this")
                .on_hover_text("Set the project's size, frame rate and colour to match this")
                .clicked()
        {
            state.match_project_to(item.id);
        }
    });
    ui.add_space(10.0);
}

fn project_section(ui: &mut egui::Ui, state: &mut AppState, f: &mut SettingsFields, actions: &mut Vec<Action>) {
    section(ui, "Project");
    let p = state.project;

    // Refresh the text fields when settings changed elsewhere (undo, presets, matching)
    // — but never while the user is typing.
    let typing = ui.ctx().egui_wants_keyboard_input();
    if f.synced != Some(p) && !typing {
        f.width = p.resolution.width.to_string();
        f.height = p.resolution.height.to_string();
        f.fps = p.frame_rate.short_label();
        f.synced = Some(p);
    }

    ui.label(egui::RichText::new("Size (pixels)").color(theme::TEXT_WEAK));
    let mut commit_size = false;
    ui.horizontal(|ui| {
        let w = ui.add(
            egui::TextEdit::singleline(&mut f.width)
                .desired_width(56.0)
                .hint_text("width"),
        );
        w.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Width in pixels"));
        ui.label("×");
        let h = ui.add(
            egui::TextEdit::singleline(&mut f.height)
                .desired_width(56.0)
                .hint_text("height"),
        );
        h.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Height in pixels"));
        commit_size = w.lost_focus() || h.lost_focus();
        if ui.button("Swap").on_hover_text("Turn vertical ↔ horizontal").clicked() {
            actions.push(Action::SwapOrientation);
        }
        ui.menu_button("Presets", |ui| {
            for preset in SIZE_PRESETS {
                let text = format!("{}  ·  {}  ·  {}", preset.resolution, preset.name, preset.used_for);
                if ui.button(text).clicked() {
                    let r = preset.resolution;
                    f.error = state.set_resolution(r.width, r.height).err().map(|e| e.0);
                    ui.close();
                }
            }
        });
    });
    if commit_size {
        let parsed = (f.width.trim().parse::<u32>(), f.height.trim().parse::<u32>());
        f.error = match parsed {
            (Ok(w), Ok(h)) if w != p.resolution.width || h != p.resolution.height => {
                state.set_resolution(w, h).err().map(|e| e.0)
            }
            (Ok(_), Ok(_)) => None,
            _ => Some("Type whole numbers of pixels, like 1080 and 1920.".to_owned()),
        };
        f.synced = None;
    }
    let r = state.project.resolution;
    let mut described = format!("{} · {}", r.aspect_label(), r.orientation());
    if let Some(q) = r.quality_name() {
        described.push_str(" · ");
        described.push_str(q);
    }
    note(ui, described);

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Frame rate").color(theme::TEXT_WEAK));
    ui.horizontal(|ui| {
        let fps = ui.add(
            egui::TextEdit::singleline(&mut f.fps)
                .desired_width(70.0)
                .hint_text("e.g. 30"),
        );
        fps.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Frame rate"));
        ui.label("fps");
        if fps.lost_focus() {
            if f.fps.trim() != p.frame_rate.short_label() {
                f.error = state.set_frame_rate_text(&f.fps).err().map(|e| e.0);
            }
            f.synced = None;
        }
    });
    note(ui, "Type any rate — 24, 25, 29.97, 30, 50, 60, 120…");

    if let Some(e) = &f.error {
        ui.label(egui::RichText::new(e).color(theme::PLAYHEAD));
    }

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Colour output").color(theme::TEXT_WEAK));
    let mut color = p.color;
    egui::ComboBox::from_id_salt("motix_color_output")
        .selected_text(color.label())
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            for c in ColorOutput::ALL {
                ui.selectable_value(&mut color, c, c.label())
                    .on_hover_text(c.explanation());
            }
        });
    if color != p.color {
        state.set_color(color);
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Bit depth").color(theme::TEXT_WEAK));
        let mut depth = state.project.bit_depth;
        egui::ComboBox::from_id_salt("motix_bit_depth")
            .selected_text(format!("{depth}-bit"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut depth, 8, "8-bit");
                ui.selectable_value(&mut depth, 10, "10-bit");
            });
        if depth != state.project.bit_depth {
            f.error = state.set_bit_depth(depth).err().map(|e| e.0);
        }
    });
    note(ui, state.project.color.explanation());

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Clips of a different size").color(theme::TEXT_WEAK));
    let mut fit = p.default_fit;
    egui::ComboBox::from_id_salt("motix_default_fit")
        .selected_text(fit.label())
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            for m in FitMode::ALL {
                ui.selectable_value(&mut fit, m, m.label())
                    .on_hover_text(m.explanation());
            }
        });
    if fit != p.default_fit {
        state.set_default_fit(fit, false);
    }
    note(ui, state.project.default_fit.explanation());
    if !state.timeline.is_empty()
        && ui
            .small_button("Apply to all clips")
            .on_hover_text("Use this placement for every clip already on the timeline")
            .clicked()
    {
        state.set_default_fit(state.project.default_fit, true);
    }

    ui.add_space(6.0);
    let mut safe = state.show_safe_areas;
    if ui
        .checkbox(&mut safe, "Show safe areas")
        .on_hover_text("Where social apps put captions and buttons (Ctrl+G)")
        .changed()
    {
        actions.push(Action::ToggleSafeAreas);
    }
}

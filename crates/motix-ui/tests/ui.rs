//! Interaction tests for the MOTIX window, driven like a user would (no GPU needed),
//! plus an opt-in screenshot test.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use motix_app::{Action, CanvasPreset};
use motix_ui::{MotixUi, Request};
use std::path::PathBuf;

fn harness() -> Harness<'static, MotixUi> {
    Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_ui_state(|ui, app: &mut MotixUi| app.show(ui), MotixUi::new("Test GPU"))
}

#[test]
fn starts_with_welcome_and_empty_bin() {
    let mut h = harness();
    h.run();
    h.get_by_label("Drag files here, or click to import media");
    assert!(h.query_all_by_label_contains("Drop a video here").count() >= 1);
    assert!(h.state().state().media.items().is_empty());
}

#[test]
fn import_button_asks_host_for_file_picker() {
    let mut h = harness();
    h.run();
    h.get_by_label("+ Import").click();
    h.run();
    assert_eq!(h.state_mut().take_requests(), vec![Request::PickMediaFiles]);
}

#[test]
fn imported_files_show_in_bin_and_inspector() {
    let mut h = harness();
    h.state_mut()
        .import(vec![PathBuf::from("holiday.mp4"), PathBuf::from("notes.txt")]);
    h.run();
    assert_eq!(h.state().state().media.items().len(), 1);
    assert!(h.state().state().status.contains("1 not a supported media file"));
    // Name appears in the bin, the inspector and the viewer.
    assert!(h.query_all_by_label("holiday.mp4").count() >= 2);
}

#[test]
fn space_toggles_playback_and_playhead_moves() {
    let mut h = harness();
    h.run();
    h.key_press(egui::Key::Space);
    h.run_steps(10);
    assert!(h.state().state().playing);
    assert!(h.state().state().playhead > motix_core::Time::ZERO);
    h.key_press(egui::Key::Home);
    h.key_press(egui::Key::Space);
    h.run();
    assert!(!h.state().state().playing);
}

#[test]
fn command_palette_runs_actions() {
    let mut h = harness();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P);
    h.run();
    assert!(h.state().palette_open());
    h.get_by_role(egui::accesskit::Role::TextInput).type_text("land");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(!h.state().palette_open());
    assert_eq!(h.state().state().canvas, CanvasPreset::LANDSCAPE);
}

#[test]
fn planned_features_explain_when_they_arrive() {
    let mut h = harness();
    h.state_mut().perform(Action::Export);
    h.run();
    assert!(h.state().state().status.contains("M2"));
}

/// Renders the window to PNG files for visual review.
/// Needs a GPU or software renderer: `cargo test -p motix-ui -- --ignored`.
#[test]
#[ignore = "needs a GPU adapter; run explicitly"]
fn screenshots() {
    let out = PathBuf::from(std::env::var("MOTIX_SCREENSHOT_DIR").unwrap_or_else(|_| "target/screenshots".to_owned()));
    std::fs::create_dir_all(&out).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .wgpu()
        .build_ui_state(
            |ui, app: &mut MotixUi| app.show(ui),
            MotixUi::new("Software renderer (test)"),
        );
    h.run();
    h.render().unwrap().save(out.join("01-empty.png")).unwrap();

    h.state_mut().import(vec![
        PathBuf::from("beach_day.mp4"),
        PathBuf::from("voiceover.wav"),
        PathBuf::from("logo.png"),
    ]);
    for _ in 0..3 {
        h.state_mut().perform(Action::NextFrame);
    }
    h.run();
    h.render().unwrap().save(out.join("02-media.png")).unwrap();

    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P);
    h.run();
    h.get_by_role(egui::accesskit::Role::TextInput).type_text("canvas");
    h.run();
    h.render().unwrap().save(out.join("03-palette.png")).unwrap();
}

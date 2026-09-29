//! Interaction tests for the MOTIX window, driven like a user would (no GPU needed),
//! plus an opt-in screenshot test. Real media files come from the probe's fixtures.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use motix_app::{Action, ColorOutput, MediaId, MediaItem, MediaKind, Resolution, TrackKind, UpdateInfo, UpdatePhase};
use motix_core::FrameRate;
use motix_ui::{MotixUi, Request};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../motix-probe/tests/fixtures")
        .join(name)
}

fn harness() -> Harness<'static, MotixUi> {
    Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(120)
        .build_ui_state(|ui, app: &mut MotixUi| app.show(ui), MotixUi::new("Test GPU"))
}

#[test]
fn starts_with_welcome_and_empty_bin() {
    let mut h = harness();
    h.run();
    h.get_by_label("Drag files here, or click to import media");
    assert!(h.query_all_by_label_contains("Drop a video here").count() >= 1);
    assert!(h.state().state().media.items().is_empty());
    assert!(h.state().state().timeline.tracks().is_empty(), "no fixed tracks");
    assert!(
        h.query_by_label_contains("M1").is_none(),
        "no milestone jargon in the UI"
    );
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
fn imported_files_are_inspected() {
    let mut h = harness();
    h.state_mut()
        .import(vec![fixture("h264_aac_2997.mp4"), PathBuf::from("notes.txt")]);
    h.run();
    let s = h.state().state();
    assert_eq!(s.media.items().len(), 1);
    assert!(s.status.contains("1 not a supported media file"));
    // The bin row describes what the probe found.
    assert!(
        h.query_all_by_label_contains("64×36 · 29.97 fps · H.264 · SDR · 1 audio stream")
            .count()
            >= 1
    );
    // The inspector shows the details.
    h.get_by_label_contains("stereo");
    assert!(h.query_all_by_label_contains("AAC").count() >= 1);
}

#[test]
fn double_click_adds_linked_video_and_audio_then_offers_to_match() {
    let mut h = harness();
    h.state_mut().import(vec![fixture("h264_aac_2997.mp4")]);
    h.run();
    // Double-click the bin row.
    h.get_by_label_contains("h264_aac_2997.mp4, 64×36").click();
    h.step();
    h.get_by_label_contains("h264_aac_2997.mp4, 64×36").click();
    h.run();
    let s = h.state().state();
    assert_eq!(s.timeline.clips().len(), 2, "video and its audio as separate clips");
    assert_eq!(s.timeline.tracks_of(TrackKind::Video).count(), 1);
    assert_eq!(s.timeline.tracks_of(TrackKind::Audio).count(), 1);
    assert!(s.timeline.clips()[0].link.is_some());
    // Asked whether to match the project to the clip.
    h.get_by_label("Match the project to this video?");
    h.get_by_label("Match project").click();
    h.run();
    let p = h.state().state().project;
    assert_eq!(p.resolution, Resolution { width: 64, height: 36 });
    assert_eq!(p.frame_rate, FrameRate::FPS_29_97);
    assert!(h.query_by_label("Match the project to this video?").is_none());
}

#[test]
fn keeping_settings_uses_fit_for_other_sizes() {
    let mut h = harness();
    h.state_mut().import(vec![fixture("hevc10_pq.mp4")]);
    h.state_mut().perform(Action::AddToTimeline);
    h.run();
    h.get_by_label("Keep current settings").click();
    h.run();
    let s = h.state().state();
    assert_eq!(
        s.project.resolution,
        Resolution {
            width: 1080,
            height: 1920
        }
    );
    assert!(s.status.contains("Scale to fit"));
    // Viewer and timeline both say the clip is being scaled.
    assert!(h.query_all_by_label_contains("scale to fit").count() >= 1);
}

#[test]
fn hdr_clip_offers_hdr_output() {
    let mut h = harness();
    h.state_mut().import(vec![fixture("hevc10_pq.mp4")]);
    h.state_mut().perform(Action::AddToTimeline);
    h.run();
    h.get_by_label("HDR10 (PQ, Rec.2020)");
    h.get_by_label("Match project").click();
    h.run();
    let p = h.state().state().project;
    assert_eq!((p.color, p.bit_depth), (ColorOutput::Hdr10, 10));
    assert_eq!(p.frame_rate, FrameRate::FPS_60);
}

#[test]
fn drag_media_onto_timeline() {
    let mut h = harness();
    h.state_mut().import(vec![fixture("stereo_48k.wav")]);
    h.run();
    let from = h.get_by_label_contains("stereo_48k.wav,").rect().center();
    let to = egui::pos2(700.0, 800.0);
    h.drag_at(from);
    h.run();
    for step in [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
        h.hover_at(from + (to - from) * (step / 6.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
    let s = h.state().state();
    assert_eq!(s.timeline.clips().len(), 1, "dropped: {}", s.status);
    assert_eq!(s.timeline.tracks_of(TrackKind::Audio).count(), 1);
    assert!(
        s.timeline.clips()[0].start > motix_core::Time::ZERO,
        "placed where it was dropped"
    );
}

#[test]
fn typed_frame_rate_and_size() {
    let mut h = harness();
    h.run();
    let fps = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Frame rate");
    fps.click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Frame rate")
        .type_text("59.94");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().state().project.frame_rate, FrameRate::FPS_59_94);

    let width = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Width in pixels");
    width.click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Width in pixels")
        .type_text("2160");
    h.key_press(egui::Key::Tab);
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Height in pixels")
        .type_text("3840");
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert_eq!(
        h.state().state().project.resolution,
        Resolution {
            width: 2160,
            height: 3840
        }
    );
    h.get_by_label_contains("4K UHD");
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
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search commands")
        .type_text("size horiz");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(!h.state().palette_open());
    assert_eq!(
        h.state().state().project.resolution,
        Resolution {
            width: 1920,
            height: 1080
        }
    );
}

#[test]
fn tracks_can_be_added_freely() {
    let mut h = harness();
    h.run();
    h.get_by_label("+ Video track").click();
    h.run();
    h.get_by_label("+ Video track").click();
    h.run();
    h.get_by_label("+ Audio track").click();
    h.run();
    h.get_by_label("Video track 2");
    h.get_by_label("Audio track 1");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert!(
        h.query_by_label("Audio track 1").is_none(),
        "undo removed the audio track"
    );
}

/// Adds synthetic media (no file needed) so tests can use realistic lengths and sizes.
fn fake_media(app: &mut MotixUi, name: &str, size: Option<(u32, u32)>, audio: usize, seconds: i64) -> MediaId {
    use motix_app::probe::{AudioInfo, ColorInfo, Container, MediaInfo, VideoInfo};
    app.state_mut().media.insert_with(|id| MediaItem {
        id,
        path: PathBuf::from(name),
        name: name.to_owned(),
        kind: if size.is_some() {
            MediaKind::Video
        } else {
            MediaKind::Audio
        },
        size_bytes: Some(10_000_000),
        info: Some(MediaInfo {
            container: Container::Mp4,
            duration: Some(motix_core::Time::from_seconds(seconds).unwrap()),
            video: size.map(|(w, h)| VideoInfo {
                codec: "H.264".into(),
                coded_width: w,
                coded_height: h,
                rotation: 0,
                frame_rate: Some(FrameRate::FPS_30),
                variable_frame_rate: false,
                bit_depth: Some(8),
                color: ColorInfo::default(),
                dolby_vision: None,
                hdr_metadata: false,
            }),
            audio: (0..audio)
                .map(|_| AudioInfo {
                    codec: "AAC".into(),
                    channels: 2,
                    sample_rate: 48_000,
                })
                .collect(),
            still: false,
        }),
        probe_note: None,
    })
}

#[test]
fn tracks_are_named_after_files_in_the_order_added() {
    let mut h = harness();
    let v1 = fake_media(h.state_mut(), "Beach day.mp4", Some((1080, 1920)), 1, 10);
    let a1 = fake_media(h.state_mut(), "Song.mp3", None, 1, 30);
    let v2 = fake_media(h.state_mut(), "Selfie.mov", Some((1080, 1920)), 1, 6);
    for m in [v1, a1, v2] {
        h.state_mut().state_mut().add_to_timeline(m, None, None).unwrap();
    }
    h.run();
    let names: Vec<_> = h
        .state()
        .state()
        .timeline
        .tracks()
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert_eq!(
        names,
        [
            "Beach day.mp4",
            "Audio of Beach day.mp4",
            "Song.mp3",
            "Selfie.mov",
            "Audio of Selfie.mov"
        ]
    );
    for n in &names {
        assert!(h.query_all_by_label(n).count() >= 1, "header for {n}");
    }
    // Double-click a header to rename it.
    let header = h.get_by_label("Song.mp3").rect().center();
    h.hover_at(header);
    h.run();
    h.get_by_label("Song.mp3").click();
    h.step();
    h.get_by_label("Song.mp3").click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Track name")
        .type_text("Music");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().state().timeline.tracks()[2].name, "Music");
    // Lock and solo buttons.
    h.get_by_label("Solo: hear only soloed tracks (Music)").click();
    h.run();
    assert!(h.state().state().timeline.tracks()[2].solo);
}

#[test]
fn blade_markers_and_edit_keys() {
    let mut h = harness();
    let v = fake_media(h.state_mut(), "Clip.mp4", Some((1080, 1920)), 1, 10);
    h.state_mut().state_mut().add_to_timeline(v, None, None).unwrap();
    h.run();
    h.key_press(egui::Key::B);
    h.run();
    assert_eq!(h.state().state().tool, motix_app::Tool::Blade);
    let clip = h.get_by_label_contains("Video clip Clip.mp4").rect();
    h.hover_at(clip.center());
    h.run();
    h.get_by_label_contains("Video clip Clip.mp4").click();
    h.run();
    assert_eq!(h.state().state().timeline.clips().len(), 4, "video and audio both cut");
    h.key_press(egui::Key::A);
    h.key_press(egui::Key::M);
    h.run();
    assert_eq!(h.state().state().timeline.markers().len(), 1);
    h.key_press(egui::Key::ArrowDown);
    h.run();
    assert!(
        h.state().state().playhead > motix_core::Time::ZERO,
        "jumped to the next edit"
    );
}

#[test]
fn help_menu_opens_updates_and_banner_offers_restart() {
    let mut h = harness();
    h.state_mut().perform(Action::CheckForUpdates);
    h.run();
    assert!(h.state().updates_window_open());
    assert_eq!(h.state_mut().take_requests(), vec![Request::CheckForUpdates]);
    h.get_by_label("Updates");
    let info = UpdateInfo {
        phase: UpdatePhase::Ready {
            version: "0.2.0-preview.9".into(),
            notes: "Faster timeline".into(),
        },
        ..UpdateInfo::default()
    };
    h.state_mut().set_update_info(info);
    h.run();
    h.get_by_label("MOTIX 0.2.0-preview.9 is ready to install.");
    h.get_by_label("Restart now").click();
    h.run();
    assert_eq!(h.state_mut().take_requests(), vec![Request::InstallUpdateNow]);
    h.get_by_label("Later").click();
    h.run();
    assert_eq!(h.state_mut().take_requests(), vec![Request::InstallUpdateOnExit]);
    assert!(h.query_by_label("MOTIX 0.2.0-preview.9 is ready to install.").is_none());
}

#[test]
fn unavailable_features_say_so_plainly() {
    let mut h = harness();
    h.state_mut().perform(Action::Export);
    h.run();
    assert!(h.state().state().status.contains("coming in a future update"));
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
        fixture("hevc10_pq.mp4"),
        fixture("h264_aac_2997.mp4"),
        fixture("rotated_phone.mp4"),
        fixture("stereo_48k.wav"),
        fixture("still.png"),
    ]);
    h.run();
    h.render().unwrap().save(out.join("02-media.png")).unwrap();

    let first = h.state().state().media.items()[1].id;
    let _ = h.state_mut().state_mut().add_to_timeline(first, None, None);
    h.run();
    h.render().unwrap().save(out.join("03-match-question.png")).unwrap();

    h.get_by_label("Keep current settings").click();
    h.run();

    // The owner's example: a video with sound, then a song, then another video with sound.
    let mut h2 = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .wgpu()
        .build_ui_state(
            |ui, app: &mut MotixUi| app.show(ui),
            MotixUi::new("Software renderer (test)"),
        );
    let v1 = fake_media(h2.state_mut(), "Beach day.mp4", Some((3840, 2160)), 1, 12);
    let a1 = fake_media(h2.state_mut(), "Summer song.mp3", None, 1, 30);
    let v2 = fake_media(h2.state_mut(), "Selfie.mov", Some((1080, 1920)), 1, 8);
    let s2 = h2.state_mut().state_mut();
    s2.match_asked = true;
    s2.add_to_timeline(v1, None, None).unwrap();
    s2.add_to_timeline(a1, None, None).unwrap();
    s2.playhead = motix_core::Time::from_seconds(9).unwrap();
    s2.add_to_timeline(v2, None, None).unwrap();
    s2.playhead = motix_core::Time::from_seconds(5).unwrap();
    s2.perform(Action::AddMarker);
    s2.playhead = motix_core::Time::from_seconds(11).unwrap();
    s2.perform(Action::AddMarker);
    s2.playhead = motix_core::Time::from_seconds(4).unwrap();
    let first_track = s2.timeline.tracks()[0].id;
    let clip = s2.timeline.clips()[0].id;
    s2.select_clip(clip, false);
    let _ = first_track;
    h2.run();
    h2.render().unwrap().save(out.join("04-timeline.png")).unwrap();
    let info = UpdateInfo {
        current_version: "0.1.0-preview.14".into(),
        phase: UpdatePhase::Ready {
            version: "0.1.0-preview.15".into(),
            notes: "Resolve-style timeline\nSigned automatic updates".into(),
        },
        last_checked: Some("2 minutes ago".into()),
        ..UpdateInfo::default()
    };
    h2.state_mut().set_update_info(info);
    h2.state_mut().perform(Action::CheckForUpdates);
    h2.run();
    h2.render().unwrap().save(out.join("05-updates.png")).unwrap();
}

//! Interaction tests for the MOTIX window, driven like a user would (no GPU needed),
//! plus an opt-in screenshot test. Real media files come from the probe's fixtures.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use motix_app::{
    Action, ColorOutput, MediaId, MediaItem, MediaKind, Resolution, SharingInfo, TrackKind, UpdateInfo, UpdatePhase,
};
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
fn sharing_on_the_network_and_shared_updates() {
    let mut h = harness();
    h.state_mut().perform(Action::ShareOnNetwork);
    h.run();
    assert!(h.state().updates_window_open());
    assert_eq!(h.state_mut().take_requests(), vec![Request::ShareOnNetwork]);
    let mut info = UpdateInfo {
        sharing: SharingInfo {
            last_result: Some(Ok(r"B:\Family\MOTIX".into())),
            pc_name: Some("ANDREWSNEWSERVER".into()),
            ..SharingInfo::default()
        },
        ..UpdateInfo::default()
    };
    h.state_mut().set_update_info(info.clone());
    h.run();
    h.get_by_label(r"MOTIX is ready in B:\Family\MOTIX.");
    assert!(
        h.query_by_label_contains(r"\\ANDREWSNEWSERVER\MOTIX\MOTIX.exe")
            .is_some(),
        "tells the user the exact network path"
    );
    h.get_by_label("Choose a folder to share MOTIX from…").click();
    h.run();
    assert_eq!(h.state_mut().take_requests(), vec![Request::ShareOnNetwork]);

    // A copy running from the shared folder: a new version was installed there.
    info.sharing = SharingInfo {
        running_from: Some(r"\\ANDREWSNEWSERVER\MOTIX".into()),
        ..SharingInfo::default()
    };
    info.phase = UpdatePhase::Installed {
        version: "0.2.0-preview.9".into(),
        notes: String::new(),
    };
    h.state_mut().set_update_info(info);
    h.run();
    assert!(h.query_by_label("Choose a folder to share MOTIX from…").is_none());
    h.get_by_label("MOTIX 0.2.0-preview.9 is installed. Restart to use it.");
    h.get_all_by_label("Restart now").next().unwrap().click();
    h.run();
    assert_eq!(h.state_mut().take_requests(), vec![Request::InstallUpdateNow]);
    h.get_by_label("Later").click();
    h.run();
    assert!(
        h.query_by_label("MOTIX 0.2.0-preview.9 is installed. Restart to use it.")
            .is_none()
    );
}

#[test]
fn save_open_and_unsaved_changes() {
    let dir = std::env::temp_dir().join(format!("motix-ui-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness();
    h.state_mut().import(vec![fixture("h264_aac_2997.mp4")]);
    h.run();
    h.get_by_label("Untitled project \u{2022}");

    // New project with unsaved changes asks first; Save asks where (first time).
    h.state_mut().perform(Action::NewProject);
    h.run();
    h.get_by_label("Save your changes?");
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(
        h.state_mut().take_requests(),
        vec![Request::PickSavePath {
            suggested: "Untitled project.motix".into()
        }]
    );
    h.state_mut().save_as(&dir.join("Trip.motix"));
    h.run();
    assert!(
        h.state().state().media.items().is_empty(),
        "then the new project started"
    );
    assert!(dir.join("Trip.motix").is_file());

    // Open it again (from the dialog), change it, and choose "Don't save" on New.
    h.state_mut().open_project(&dir.join("Trip.motix"));
    h.run();
    assert_eq!(h.state().state().media.items().len(), 1);
    h.get_by_label("Trip");
    let id = h.state().state().media.items()[0].id;
    h.state_mut().state_mut().add_to_timeline(id, None, None).unwrap();
    h.state_mut().perform(Action::NewProject);
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert!(h.state().state().timeline.is_empty());

    // Ctrl+S on a saved project saves straight away; Cancel keeps everything.
    h.state_mut().open_project(&dir.join("Trip.motix"));
    h.state_mut().state_mut().add_to_timeline(id, None, None).unwrap();
    h.state_mut().perform(Action::Quit);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state_mut().take_requests().is_empty(), "cancel means nothing happens");
    h.state_mut().perform(Action::SaveProject);
    h.run();
    assert!(!h.state().state().is_dirty());
    h.state_mut().perform(Action::Quit);
    assert_eq!(h.state_mut().take_requests(), vec![Request::Quit]);

    // Dropping a project file on the window opens it.
    let mut h2 = harness();
    h2.state_mut().import(vec![dir.join("Trip.motix")]);
    h2.run();
    assert_eq!(h2.state().state().timeline.clips().len(), 2, "video + its audio");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn creator_lab_is_owner_only() {
    let dir = std::env::temp_dir().join(format!("motix-ui-lab-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut h = harness();
    h.state_mut().set_owner_check(None);
    h.state_mut().set_lab_storage(&dir);
    h.get_by_label("\u{1f512} Creator Lab").click();
    h.run();
    h.get_by_label("\u{1f512} Only the owner can open the Creator Lab.");
    h.get_by_label("Create owner password…");
    assert!(!h.state().lab_unlocked());

    // With an owner password built in: wrong password stays locked, right one opens.
    let check =
        motix_app::owner::create_with("purple ladder sunset river", "purple ladder sunset river", 8 * 1024, 1).unwrap();
    let mut h = harness();
    h.state_mut().set_owner_check(Some(check));
    h.state_mut().set_lab_storage(&dir);
    h.state_mut().show_lab(true);
    h.run();
    h.get_by_label("Owner password");
    h.state_mut().show_lab(true);
    let field = h.get_by_role(egui::accesskit::Role::PasswordInput);
    field.click();
    h.run();
    h.get_by_role(egui::accesskit::Role::PasswordInput)
        .type_text("wrong guess entirely");
    h.run();
    h.get_by_label("Unlock").click();
    h.run();
    h.get_by_label("That's not the owner password.");
    assert!(!h.state().lab_unlocked());

    // The right password (typed into the same field after clearing it).
    let mut h = harness();
    h.state_mut().set_owner_check(Some(check));
    h.state_mut().set_lab_storage(&dir);
    h.state_mut().show_lab(true);
    h.run();
    h.get_by_role(egui::accesskit::Role::PasswordInput).click();
    h.run();
    h.get_by_role(egui::accesskit::Role::PasswordInput)
        .type_text("purple ladder sunset river");
    h.run();
    h.get_by_label("Unlock").click();
    h.run();
    assert!(h.state().lab_unlocked());
    h.get_by_label("Add edits…").click();
    h.run();
    assert_eq!(
        h.state_mut().take_requests(),
        vec![Request::PickLabFiles {
            collection: motix_app::lab::Collection::LovedEdits
        }]
    );
    h.state_mut().lab_add(
        motix_app::lab::Collection::LovedEdits,
        vec![PathBuf::from("my favourite.mp4")],
    );
    h.run();
    h.get_by_label("my favourite.mp4");
    h.get_by_label("Transitions").click();
    h.run();

    // Next start on this PC: remembered, and the library is still there.
    let mut h = harness();
    h.state_mut().set_owner_check(Some(check));
    h.state_mut().set_lab_storage(&dir);
    assert!(h.state().lab_unlocked());
    assert_eq!(
        h.state().lab_library().loved[0].qualities,
        [motix_app::lab::Quality::Transitions]
    );
    // A different owner password (another build) doesn't accept the remembered key.
    let other = motix_app::owner::create_with("another long password", "another long password", 8 * 1024, 1).unwrap();
    let mut h = harness();
    h.state_mut().set_owner_check(Some(other));
    h.state_mut().set_lab_storage(&dir);
    assert!(!h.state().lab_unlocked());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_viewer_shows_real_pictures() {
    let Some(tools) = motix_media::Tools::find(&[]) else {
        eprintln!("FFmpeg not installed here; skipping");
        return;
    };
    let mut h = harness();
    h.state_mut().attach_media(Some(tools));
    h.state_mut().state_mut().match_asked = true;
    h.state_mut().import(vec![fixture("h264_aac_2997.mp4")]);
    let id = h.state().state().media.items()[0].id;
    h.state_mut().state_mut().add_to_timeline(id, None, None).unwrap();
    let clip = h.state().state().timeline.clips()[0].id;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while h.state().preview_clip() != Some(clip) && std::time::Instant::now() < deadline {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        h.state().preview_clip(),
        Some(clip),
        "a decoded frame reached the viewer"
    );

    // Without the helper, the viewer explains what's missing.
    let mut h = harness();
    h.state_mut().attach_media(None);
    h.state_mut().state_mut().match_asked = true;
    h.state_mut().import(vec![fixture("h264_aac_2997.mp4")]);
    let id = h.state().state().media.items()[0].id;
    h.state_mut().state_mut().add_to_timeline(id, None, None).unwrap();
    h.run();
    assert!(
        h.query_by_label_contains("Video preview needs").is_some(),
        "explains the missing helper"
    );
    // The download offer.
    h.state_mut().set_helper_status(motix_ui::HelperStatus::Offer);
    h.run();
    h.get_by_label("Download").click();
    h.run();
    assert_eq!(h.state_mut().take_requests(), vec![Request::DownloadVideoHelper]);
    h.get_by_label_contains("Downloading the video helper");
    h.state_mut()
        .set_helper_status(motix_ui::HelperStatus::Failed("no internet connection".into()));
    h.run();
    h.get_by_label("Try again");
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
#[allow(clippy::too_many_lines)] // one walkthrough of every screen, top to bottom
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

    let info = UpdateInfo {
        current_version: "0.1.0-preview.14".into(),
        phase: UpdatePhase::Installed {
            version: "0.1.0-preview.15".into(),
            notes: "Run MOTIX from a shared folder on your network".into(),
        },
        last_checked: Some("just now".into()),
        source_text: "New versions come from the official MOTIX releases on GitHub, or from a release copied into \
                      the shared folder's 'updates' folder."
            .into(),
        sharing: SharingInfo {
            running_from: Some(r"\\ANDREWSNEWSERVER\MOTIX".into()),
            ..SharingInfo::default()
        },
        ..UpdateInfo::default()
    };
    h2.state_mut().set_update_info(info);
    h2.run();
    h2.render().unwrap().save(out.join("06-shared-updates.png")).unwrap();

    let info = UpdateInfo {
        current_version: "0.1.0-preview.14".into(),
        phase: UpdatePhase::UpToDate,
        sharing: SharingInfo {
            last_result: Some(Ok(r"B:\Family\Andrew Cardone\MOTIX".into())),
            pc_name: Some("ANDREWSNEWSERVER".into()),
            ..SharingInfo::default()
        },
        ..UpdateInfo::default()
    };
    h2.state_mut().set_update_info(info);
    h2.run();
    h2.render().unwrap().save(out.join("07-share-done.png")).unwrap();

    // The Creator Lab: locked, then with a few examples.
    let lab_dir = std::env::temp_dir().join(format!("motix-shot-lab-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lab_dir);
    std::fs::create_dir_all(&lab_dir).unwrap();
    let check =
        motix_app::owner::create_with("purple ladder sunset river", "purple ladder sunset river", 8 * 1024, 1).unwrap();
    let mut h3 = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .wgpu()
        .build_ui_state(
            |ui, app: &mut MotixUi| app.show(ui),
            MotixUi::new("Software renderer (test)"),
        );
    h3.state_mut().set_owner_check(Some(check));
    h3.state_mut().set_lab_storage(&lab_dir);
    h3.state_mut().show_lab(true);
    h3.run();
    h3.render().unwrap().save(out.join("08-lab-locked.png")).unwrap();
    let key = motix_app::owner::unlock("purple ladder sunset river", Some(&check)).unwrap();
    std::fs::write(
        lab_dir.join("creator-lab-unlock.txt"),
        motix_app::owner::remembered_text(&key),
    )
    .unwrap();
    h3.state_mut().set_lab_storage(&lab_dir);
    h3.state_mut().lab_add(
        motix_app::lab::Collection::LovedEdits,
        vec![PathBuf::from("Night drive edit.mp4"), PathBuf::from("Summer recap.mp4")],
    );
    h3.run();
    h3.render().unwrap().save(out.join("09-lab-library.png")).unwrap();
    let _ = std::fs::remove_dir_all(lab_dir);

    // Real pictures in the viewer (needs FFmpeg on this machine).
    if let Some(tools) = motix_media::Tools::find(&[]) {
        let mut h4 = Harness::builder()
            .with_size(egui::vec2(1440.0, 900.0))
            .wgpu()
            .build_ui_state(
                |ui, app: &mut MotixUi| app.show(ui),
                MotixUi::new("Software renderer (test)"),
            );
        h4.state_mut().attach_media(Some(tools));
        h4.state_mut()
            .import(vec![fixture("h264_aac_2997.mp4"), fixture("stereo_48k.wav")]);
        let first = h4.state().state().media.items()[0].id;
        let _ = h4.state_mut().state_mut().add_to_timeline(first, None, None);
        h4.run();
        if h4.query_by_label("Match project").is_some() {
            h4.get_by_label("Match project").click();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while h4.state().preview_clip().is_none() && std::time::Instant::now() < deadline {
            h4.step();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        h4.run();
        h4.render().unwrap().save(out.join("10-real-picture.png")).unwrap();
    }
}

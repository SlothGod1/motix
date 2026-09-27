//! MOTIX desktop application entry point.
//!
//! Thin host around [`motix_ui::MotixUi`]: creates the window with a GPU renderer (wgpu),
//! and performs the operating-system services the UI asks for (file pickers, quitting).

// No console window behind the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use motix_ui::{MotixUi, Request};

struct MotixApp {
    ui: MotixUi,
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
        Self {
            ui: MotixUi::new(gpu_info),
        }
    }
}

impl eframe::App for MotixApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui.show(ui);
        for request in self.ui.take_requests() {
            match request {
                Request::PickMediaFiles => {
                    let picked = rfd::FileDialog::new()
                        .set_title("Import media into MOTIX")
                        .add_filter(
                            "Media",
                            &[
                                "mp4", "mov", "m4v", "mkv", "webm", "avi", "mts", "m2ts", "mxf", "3gp", "wav", "mp3",
                                "aac", "m4a", "flac", "ogg", "opus", "aif", "aiff", "png", "jpg", "jpeg", "webp",
                                "gif", "tif", "tiff", "exr", "heic", "heif", "bmp",
                            ],
                        )
                        .add_filter("All files", &["*"])
                        .pick_files();
                    if let Some(files) = picked {
                        self.ui.import(files);
                    }
                }
                Request::Quit => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
            }
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

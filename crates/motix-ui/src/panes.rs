//! Dockable panels (`egui_tiles`) and the default workspace layout.

use crate::{UiState, inspector, media_panel, theme, timeline, viewer};
use motix_app::{Action, AppState};

/// A dockable panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// Imported media.
    Media,
    /// Preview of the canvas.
    Viewer,
    /// Properties of the selection or project.
    Inspector,
    /// Tracks and playhead.
    Timeline,
}

impl Pane {
    fn title(self) -> &'static str {
        match self {
            Self::Media => "Media",
            Self::Viewer => "Viewer",
            Self::Inspector => "Inspector",
            Self::Timeline => "Timeline",
        }
    }
}

/// The default "Edit" workspace: media · viewer · inspector on top, timeline below.
pub(crate) fn default_layout() -> egui_tiles::Tree<Pane> {
    let mut tiles = egui_tiles::Tiles::default();
    let media = tiles.insert_pane(Pane::Media);
    let viewer = tiles.insert_pane(Pane::Viewer);
    let inspector = tiles.insert_pane(Pane::Inspector);
    let timeline = tiles.insert_pane(Pane::Timeline);
    let media_tabs = tiles.insert_tab_tile(vec![media]);
    let viewer_tabs = tiles.insert_tab_tile(vec![viewer]);
    let inspector_tabs = tiles.insert_tab_tile(vec![inspector]);
    let timeline_tabs = tiles.insert_tab_tile(vec![timeline]);
    let top = tiles.insert_horizontal_tile(vec![media_tabs, viewer_tabs, inspector_tabs]);
    let root = tiles.insert_vertical_tile(vec![top, timeline_tabs]);
    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(l))) = tiles.get_mut(top) {
        l.shares.set_share(media_tabs, 0.9);
        l.shares.set_share(viewer_tabs, 2.2);
        l.shares.set_share(inspector_tabs, 0.9);
    }
    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(l))) = tiles.get_mut(root) {
        l.shares.set_share(top, 1.8);
        l.shares.set_share(timeline_tabs, 1.0);
    }
    egui_tiles::Tree::new("motix_workspace", root, tiles)
}

pub(crate) struct Behavior<'a> {
    pub state: &'a mut AppState,
    pub ui_state: &'a mut UiState,
    /// Actions requested by panels this frame (performed after drawing).
    pub actions: Vec<Action>,
}

impl egui_tiles::Behavior<Pane> for Behavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: egui_tiles::TileId, pane: &mut Pane) -> egui_tiles::UiResponse {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, theme::PANEL);
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)));
        match pane {
            Pane::Media => media_panel::show(&mut content, self.state, &mut self.actions),
            Pane::Viewer => viewer::show(&mut content, self.state, &mut self.actions),
            Pane::Inspector => inspector::show(&mut content, self.state, &mut self.actions),
            Pane::Timeline => timeline::show(&mut content, self.state, self.ui_state),
        }
        egui_tiles::UiResponse::None
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        theme::BG
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        4.0
    }
}

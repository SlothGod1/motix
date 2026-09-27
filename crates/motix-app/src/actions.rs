//! The action registry: every user-visible command, with label, shortcut and availability.

/// A user-visible command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Start a new project.
    NewProject,
    /// Open an existing project.
    OpenProject,
    /// Save a named version of the project.
    SaveVersion,
    /// Add media files to the media bin.
    ImportMedia,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Split the clip under the playhead.
    Split,
    /// Start or pause playback.
    TogglePlayback,
    /// Jump to the start of the timeline.
    GoToStart,
    /// Step one frame back.
    PreviousFrame,
    /// Step one frame forward.
    NextFrame,
    /// 9:16 vertical canvas (TikTok, Reels, Shorts).
    CanvasVertical,
    /// 1:1 square canvas.
    CanvasSquare,
    /// 4:5 portrait canvas (Instagram feed).
    CanvasPortrait,
    /// 16:9 landscape canvas (YouTube).
    CanvasLandscape,
    /// Show or hide the safe-area guides in the viewer.
    ToggleSafeAreas,
    /// Restore the default panel layout.
    ResetLayout,
    /// Open the command palette.
    CommandPalette,
    /// Export the video.
    Export,
    /// Start a collaboration session.
    StartCollaboration,
    /// Show version and license information.
    About,
    /// Quit MOTIX.
    Quit,
}

/// Whether an action works in this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Works now.
    Now,
    /// Planned; the string names the milestone that delivers it (e.g. `"M2"`).
    Planned(&'static str),
}

/// A keyboard key, independent of any UI toolkit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A letter or digit key (uppercase for letters).
    Char(char),
    /// Space bar.
    Space,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Home.
    Home,
    /// Escape.
    Escape,
}

/// A keyboard shortcut. `command` means Ctrl on Windows/Linux and ⌘ on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// Ctrl / ⌘.
    pub command: bool,
    /// Shift.
    pub shift: bool,
    /// The key.
    pub key: Key,
}

impl Shortcut {
    const fn plain(key: Key) -> Self {
        Self {
            command: false,
            shift: false,
            key,
        }
    }
    const fn cmd(key: Key) -> Self {
        Self {
            command: true,
            shift: false,
            key,
        }
    }
    const fn cmd_shift(key: Key) -> Self {
        Self {
            command: true,
            shift: true,
            key,
        }
    }

    /// Human-readable form, e.g. `Ctrl+Shift+Z`.
    #[must_use]
    pub fn label(self) -> String {
        let mut s = String::new();
        if self.command {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        match self.key {
            Key::Char(c) => s.push(c),
            Key::Space => s.push_str("Space"),
            Key::ArrowLeft => s.push_str("Left"),
            Key::ArrowRight => s.push_str("Right"),
            Key::Home => s.push_str("Home"),
            Key::Escape => s.push_str("Esc"),
        }
        s
    }
}

/// Static description of an action.
#[derive(Clone, Copy, Debug)]
pub struct ActionInfo {
    /// The action.
    pub action: Action,
    /// Menu / palette label in plain language.
    pub label: &'static str,
    /// Menu the action belongs to.
    pub menu: &'static str,
    /// Default shortcut, if any.
    pub shortcut: Option<Shortcut>,
    /// Whether it works in this build.
    pub availability: Availability,
}

const fn info(
    action: Action,
    label: &'static str,
    menu: &'static str,
    shortcut: Option<Shortcut>,
    availability: Availability,
) -> ActionInfo {
    ActionInfo {
        action,
        label,
        menu,
        shortcut,
        availability,
    }
}

use Availability::{Now, Planned};

/// Every action, in menu order.
pub const ALL: &[ActionInfo] = &[
    info(
        Action::NewProject,
        "New project",
        "File",
        Some(Shortcut::cmd(Key::Char('N'))),
        Planned("M2"),
    ),
    info(
        Action::OpenProject,
        "Open project…",
        "File",
        Some(Shortcut::cmd(Key::Char('O'))),
        Planned("M2"),
    ),
    info(
        Action::SaveVersion,
        "Save version",
        "File",
        Some(Shortcut::cmd(Key::Char('S'))),
        Planned("M2"),
    ),
    info(
        Action::ImportMedia,
        "Import media…",
        "File",
        Some(Shortcut::cmd(Key::Char('I'))),
        Now,
    ),
    info(
        Action::Export,
        "Export video…",
        "File",
        Some(Shortcut::cmd(Key::Char('E'))),
        Planned("M2"),
    ),
    info(Action::Quit, "Quit", "File", Some(Shortcut::cmd(Key::Char('Q'))), Now),
    info(
        Action::Undo,
        "Undo",
        "Edit",
        Some(Shortcut::cmd(Key::Char('Z'))),
        Planned("M2"),
    ),
    info(
        Action::Redo,
        "Redo",
        "Edit",
        Some(Shortcut::cmd_shift(Key::Char('Z'))),
        Planned("M2"),
    ),
    info(
        Action::Split,
        "Split clip at playhead",
        "Edit",
        Some(Shortcut::cmd(Key::Char('K'))),
        Planned("M2"),
    ),
    info(
        Action::TogglePlayback,
        "Play / pause",
        "Playback",
        Some(Shortcut::plain(Key::Space)),
        Now,
    ),
    info(
        Action::GoToStart,
        "Go to start",
        "Playback",
        Some(Shortcut::plain(Key::Home)),
        Now,
    ),
    info(
        Action::PreviousFrame,
        "Previous frame",
        "Playback",
        Some(Shortcut::plain(Key::ArrowLeft)),
        Now,
    ),
    info(
        Action::NextFrame,
        "Next frame",
        "Playback",
        Some(Shortcut::plain(Key::ArrowRight)),
        Now,
    ),
    info(Action::CanvasVertical, "Canvas: Vertical 9:16", "View", None, Now),
    info(Action::CanvasPortrait, "Canvas: Portrait 4:5", "View", None, Now),
    info(Action::CanvasSquare, "Canvas: Square 1:1", "View", None, Now),
    info(Action::CanvasLandscape, "Canvas: Landscape 16:9", "View", None, Now),
    info(
        Action::ToggleSafeAreas,
        "Show safe areas",
        "View",
        Some(Shortcut::cmd(Key::Char('G'))),
        Now,
    ),
    info(Action::ResetLayout, "Reset panel layout", "View", None, Now),
    info(
        Action::CommandPalette,
        "Command palette…",
        "View",
        Some(Shortcut::cmd_shift(Key::Char('P'))),
        Now,
    ),
    info(
        Action::StartCollaboration,
        "Start collaboration session",
        "Collaborate",
        None,
        Planned("M5"),
    ),
    info(Action::About, "About MOTIX", "Help", None, Now),
];

/// Looks up an action's description.
///
/// # Panics
/// Never for actions in [`ALL`]; a unit test checks every action is registered.
#[must_use]
pub fn info_of(action: Action) -> &'static ActionInfo {
    ALL.iter()
        .find(|i| i.action == action)
        .expect("every action is registered")
}

/// Finds the action bound to a shortcut.
#[must_use]
pub fn by_shortcut(shortcut: Shortcut) -> Option<Action> {
    ALL.iter().find(|i| i.shortcut == Some(shortcut)).map(|i| i.action)
}

/// Fuzzy search for the command palette.
///
/// Every query character must appear in the label in order (case-insensitive).
/// Results are ranked: prefix matches first, then word-start matches, then by
/// how tightly the characters cluster. An empty query returns every action.
#[must_use]
pub fn search(query: &str) -> Vec<&'static ActionInfo> {
    let q: Vec<char> = query
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut scored: Vec<(i64, usize, &ActionInfo)> = ALL
        .iter()
        .enumerate()
        .filter_map(|(order, i)| score(&q, i.label).map(|s| (s, order, i)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, i)| i).collect()
}

fn score(query: &[char], label: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let label: Vec<char> = label.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0usize;
    let mut first = None;
    let mut last = 0usize;
    for &qc in query {
        let found = label[pos..].iter().position(|&c| c == qc)? + pos;
        first.get_or_insert(found);
        last = found;
        pos = found + 1;
    }
    let first = first.unwrap_or(0);
    let spread = i64::try_from(last - first).unwrap_or(i64::MAX);
    let prefix_bonus = if first == 0 { 1_000 } else { 0 };
    Some(prefix_bonus - spread * 10 - i64::try_from(first).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_action_is_registered_once() {
        let all = [
            Action::NewProject,
            Action::OpenProject,
            Action::SaveVersion,
            Action::ImportMedia,
            Action::Undo,
            Action::Redo,
            Action::Split,
            Action::TogglePlayback,
            Action::GoToStart,
            Action::PreviousFrame,
            Action::NextFrame,
            Action::CanvasVertical,
            Action::CanvasSquare,
            Action::CanvasPortrait,
            Action::CanvasLandscape,
            Action::ToggleSafeAreas,
            Action::ResetLayout,
            Action::CommandPalette,
            Action::Export,
            Action::StartCollaboration,
            Action::About,
            Action::Quit,
        ];
        assert_eq!(all.len(), ALL.len());
        let ids: HashSet<_> = ALL.iter().map(|i| i.action).collect();
        assert_eq!(ids.len(), ALL.len(), "duplicate registration");
        for a in all {
            assert_eq!(info_of(a).action, a);
        }
    }

    #[test]
    fn shortcuts_are_unique() {
        let mut seen = HashSet::new();
        for i in ALL {
            if let Some(s) = i.shortcut {
                assert!(seen.insert(s), "shortcut {} used twice", s.label());
            }
        }
    }

    #[test]
    fn shortcut_lookup_and_labels() {
        let redo = Shortcut::cmd_shift(Key::Char('Z'));
        assert_eq!(by_shortcut(redo), Some(Action::Redo));
        assert_eq!(redo.label(), "Ctrl+Shift+Z");
        assert_eq!(by_shortcut(Shortcut::plain(Key::Space)), Some(Action::TogglePlayback));
        assert_eq!(by_shortcut(Shortcut::plain(Key::Escape)), None);
    }

    #[test]
    fn search_ranks_sensibly() {
        assert_eq!(search("").len(), ALL.len());
        assert_eq!(search("imp")[0].action, Action::ImportMedia);
        assert_eq!(search("play")[0].action, Action::TogglePlayback);
        assert_eq!(search("vert")[0].action, Action::CanvasVertical);
        assert_eq!(search("SAFE")[0].action, Action::ToggleSafeAreas);
        assert!(search("zzzz").is_empty());
        // Subsequence match: "npj" finds "New project".
        assert!(search("npj").iter().any(|i| i.action == Action::NewProject));
    }
}

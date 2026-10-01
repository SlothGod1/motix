//! Video frames and sound for the MOTIX viewer.
//!
//! Decoding is done by **FFmpeg running as a separate program** (shipped next to
//! `motix.exe`), never inside MOTIX itself: a damaged or malicious video can at worst
//! crash that helper, not MOTIX (ARCHITECTURE §6, ADR-005, interim ADR-034).
//!
//! * [`VideoEngine`] — a background thread that keeps the newest preview frame for the
//!   playhead: exact single frames while scrubbing, a continuous stream while playing.
//! * [`AudioPlayer`] — mixes every audible clip from the playhead with FFmpeg and plays
//!   it through `ffplay`.
//! * [`plan`] — which picture and sounds are needed, and the command lines (pure, tested).

#![forbid(unsafe_code)]
// Seconds as f64 and pixel sizes as u32 convert both ways by nature.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

pub mod audio;
pub mod helper;
pub mod plan;
pub mod video;

pub use audio::AudioPlayer;
pub use plan::{AudioInput, VideoTarget};
pub use video::{Frame, VideoEngine, VideoRequest};

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The FFmpeg programs MOTIX uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tools {
    /// `ffmpeg` — decodes and mixes.
    pub ffmpeg: PathBuf,
    /// `ffplay` — plays the mixed sound (sound is off without it).
    pub ffplay: Option<PathBuf>,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn find_one(name: &str, dirs: &[&Path]) -> Option<PathBuf> {
    let file = exe(name);
    let local = dirs.iter().map(|d| d.join(&file));
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(&file)).collect::<Vec<_>>())
        .unwrap_or_default();
    local.chain(on_path).find(|p| p.is_file())
}

impl Tools {
    /// Looks in `dirs` (in order: next to MOTIX, the downloaded helper's folders),
    /// then on the system `PATH`.
    #[must_use]
    pub fn find(dirs: &[&Path]) -> Option<Self> {
        let ffmpeg = find_one("ffmpeg", dirs)?;
        // Prefer the ffplay that sits with the chosen ffmpeg.
        let ffplay = ffmpeg
            .parent()
            .map(|d| d.join(exe("ffplay")))
            .filter(|p| p.is_file())
            .or_else(|| find_one("ffplay", dirs));
        Some(Self { ffmpeg, ffplay })
    }
}

/// `true` if this FFmpeg has the named filter.
#[must_use]
pub fn has_filter(tools: &Tools, name: &str) -> bool {
    command(&tools.ffmpeg)
        .args(["-hide_banner", "-filters"])
        .stdout(Stdio::piped())
        .output()
        .is_ok_and(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.split_whitespace().nth(1) == Some(name))
        })
}

/// A command for one of the helper programs: no console window on Windows, no
/// keyboard input, error messages discarded.
#[must_use]
pub fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW`: don't flash a console window.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

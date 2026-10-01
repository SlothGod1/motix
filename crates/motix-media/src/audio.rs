//! Preview sound: FFmpeg mixes every audible clip from the playhead into one stereo
//! stream, which `ffplay` plays (no window). Stopping or seeking ends both programs.

use crate::plan::{AudioInput, ffplay_args, mix_args};
use crate::{Tools, command};
use std::process::{Child, Stdio};

/// Plays the timeline's sound while MOTIX plays.
#[derive(Default)]
pub struct AudioPlayer {
    mixer: Option<Child>,
    player: Option<Child>,
}

impl AudioPlayer {
    /// Starts playing `inputs` (from [`crate::plan::audio_plan`]), replacing anything
    /// already playing. Does nothing when there's no sound or no `ffplay`.
    ///
    /// # Errors
    /// A plain-language reason when the helpers couldn't start.
    pub fn play(&mut self, tools: &Tools, inputs: &[AudioInput]) -> Result<(), String> {
        self.stop();
        let (Some(args), Some(ffplay)) = (mix_args(inputs), tools.ffplay.as_ref()) else {
            return Ok(());
        };
        let fail = |e: std::io::Error| format!("MOTIX couldn't start its sound helper (FFmpeg): {e}.");
        let mut mixer = command(&tools.ffmpeg)
            .args(args)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(fail)?;
        let Some(pcm) = mixer.stdout.take() else {
            let _ = mixer.kill();
            return Ok(());
        };
        match command(ffplay).args(ffplay_args()).stdin(Stdio::from(pcm)).spawn() {
            Ok(player) => {
                self.mixer = Some(mixer);
                self.player = Some(player);
                Ok(())
            }
            Err(e) => {
                let _ = mixer.kill();
                let _ = mixer.wait();
                Err(fail(e))
            }
        }
    }

    /// `true` while sound is playing.
    #[must_use]
    pub fn is_playing(&mut self) -> bool {
        self.player.as_mut().is_some_and(|p| matches!(p.try_wait(), Ok(None)))
    }

    /// Stops the sound.
    pub fn stop(&mut self) {
        for child in [self.player.take(), self.mixer.take()].into_iter().flatten() {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

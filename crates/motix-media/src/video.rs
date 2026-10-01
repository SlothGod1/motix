//! The preview frame for the playhead, decoded by FFmpeg on a background thread.
//!
//! The UI says what it wants every frame ([`VideoEngine::show`]); only the newest wish
//! counts. While scrubbing or paused MOTIX decodes exactly that one frame. While
//! playing it keeps one FFmpeg running that streams frames at the project's frame
//! rate, and skips ahead (dropping frames) if the screen falls behind; a jump on the
//! timeline restarts the stream at the new spot.

use crate::plan::{VideoTarget, video_args};
use crate::{Tools, command};
use motix_app::ClipId;
use motix_core::FrameRate;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// A decoded preview picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The clip it belongs to.
    pub clip: ClipId,
    /// The media file.
    pub path: PathBuf,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA bytes, row by row.
    pub rgba: Vec<u8>,
}

/// What the viewer needs right now.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoRequest {
    /// The picture at the playhead.
    pub target: VideoTarget,
    /// Playback is running (stream) or not (exact single frame).
    pub playing: bool,
    /// The project's frame rate.
    pub fps: FrameRate,
}

enum Msg {
    Show(Option<VideoRequest>),
    Stop,
}

type Repaint = Box<dyn Fn() + Send>;

#[derive(Default)]
struct Shared {
    frame: Mutex<Option<Arc<Frame>>>,
    generation: AtomicU64,
    repaint: Mutex<Option<Repaint>>,
    problem: Mutex<Option<String>>,
}

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn publish(&self, frame: Option<Frame>) {
        *locked(&self.frame) = frame.map(Arc::new);
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(r) = locked(&self.repaint).as_ref() {
            r();
        }
    }
}

/// Keeps the newest preview frame, decoding in the background.
pub struct VideoEngine {
    tx: Sender<Msg>,
    shared: Arc<Shared>,
}

impl VideoEngine {
    /// Starts the background thread.
    #[must_use]
    pub fn start(tools: Tools) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let worker_shared = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("motix-video".to_owned())
            .spawn(move || Worker::new(tools, worker_shared).run(&rx));
        if let Err(e) = spawned {
            *locked(&shared.problem) = Some(format!("MOTIX couldn't start video preview ({e})."));
        }
        Self { tx, shared }
    }

    /// Asks for the picture the viewer needs now (`None`: nothing on screen).
    pub fn show(&self, request: Option<VideoRequest>) {
        let _ = self.tx.send(Msg::Show(request));
    }

    /// The newest frame and a number that changes whenever it does.
    #[must_use]
    pub fn latest(&self) -> (u64, Option<Arc<Frame>>) {
        let frame = locked(&self.shared.frame).clone();
        (self.shared.generation.load(Ordering::SeqCst), frame)
    }

    /// Called (from the background thread) whenever a new frame is ready.
    pub fn on_new_frame(&self, f: impl Fn() + Send + 'static) {
        *locked(&self.shared.repaint) = Some(Box::new(f));
    }

    /// Why preview isn't working, if it isn't.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        locked(&self.shared.problem).clone()
    }
}

impl Drop for VideoEngine {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

/// A running FFmpeg streaming frames.
struct Stream {
    path: PathBuf,
    size: (u32, u32),
    hdr: bool,
    clip: ClipId,
    start: f64,
    fps: f64,
    next: u64,
    frames: Receiver<Vec<u8>>,
    child: Child,
    ended: bool,
}

impl Stream {
    fn time_of(&self, index: u64) -> f64 {
        self.start + index as f64 / self.fps
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Worker {
    tools: Tools,
    /// This FFmpeg can tone-map HDR (has the `zscale` filter).
    can_tonemap: bool,
    shared: Arc<Shared>,
    stream: Option<Stream>,
    /// What's on screen now: (file, size, hdr, frame number) for single frames.
    shown: Option<(PathBuf, (u32, u32), bool, i64)>,
}

fn frame_bytes(size: (u32, u32)) -> usize {
    size.0 as usize * size.1 as usize * 4
}

impl Worker {
    fn new(tools: Tools, shared: Arc<Shared>) -> Self {
        let can_tonemap = crate::has_filter(&tools, "zscale");
        Self {
            can_tonemap,
            tools,
            shared,
            stream: None,
            shown: None,
        }
    }

    fn run(mut self, rx: &Receiver<Msg>) {
        while let Ok(mut msg) = rx.recv() {
            // Only the newest wish matters.
            while let Ok(newer) = rx.try_recv() {
                msg = newer;
            }
            match msg {
                Msg::Stop => return,
                Msg::Show(None) => {
                    self.stream = None;
                    if self.shown.take().is_some() || locked(&self.shared.frame).is_some() {
                        self.shared.publish(None);
                    }
                }
                Msg::Show(Some(mut req)) => {
                    // Without the tone-mapping filter HDR shows untone-mapped (flat).
                    req.target.hdr &= self.can_tonemap;
                    if req.playing && !req.target.still {
                        self.play(&req);
                    } else {
                        self.stream = None;
                        self.single(&req);
                    }
                }
            }
        }
    }

    fn problem(&self, why: String) {
        *locked(&self.shared.problem) = Some(why);
    }

    fn single(&mut self, req: &VideoRequest) {
        let t = &req.target;
        let fps = req.fps.fps_f64().max(1.0);
        let index = if t.still {
            0
        } else {
            (t.source_seconds * fps).floor() as i64
        };
        let key = (t.path.clone(), t.size, t.hdr, index);
        if self.shown.as_ref() == Some(&key) {
            return;
        }
        let output = command(&self.tools.ffmpeg)
            .args(video_args(t, None))
            .stdout(Stdio::piped())
            .output();
        match output {
            Ok(out) if out.stdout.len() == frame_bytes(t.size) => {
                self.shown = Some(key);
                self.shared.publish(Some(Frame {
                    clip: t.clip,
                    path: t.path.clone(),
                    width: t.size.0,
                    height: t.size.1,
                    rgba: out.stdout,
                }));
            }
            // Past the end of the file or unreadable: show nothing for this moment.
            Ok(_) => {
                self.shown = Some(key);
                self.shared.publish(None);
            }
            Err(e) => self.problem(format!("MOTIX couldn't run its video helper (FFmpeg): {e}.")),
        }
    }

    fn start_stream(&mut self, req: &VideoRequest) -> Option<Stream> {
        let t = &req.target;
        let mut child = command(&self.tools.ffmpeg)
            .args(video_args(t, Some(req.fps)))
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| self.problem(format!("MOTIX couldn't run its video helper (FFmpeg): {e}.")))
            .ok()?;
        let mut stdout = child.stdout.take()?;
        let (tx, frames): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::sync_channel(4);
        let n = frame_bytes(t.size);
        let _ = std::thread::Builder::new()
            .name("motix-video-reader".to_owned())
            .spawn(move || {
                loop {
                    let mut buf = vec![0_u8; n];
                    if stdout.read_exact(&mut buf).is_err() || tx.send(buf).is_err() {
                        return;
                    }
                }
            });
        Some(Stream {
            path: t.path.clone(),
            size: t.size,
            hdr: t.hdr,
            clip: t.clip,
            start: t.source_seconds,
            fps: req.fps.fps_f64().max(1.0),
            next: 0,
            frames,
            child,
            ended: false,
        })
    }

    fn play(&mut self, req: &VideoRequest) {
        let t = &req.target;
        let frame_dur = 1.0 / req.fps.fps_f64().max(1.0);
        let reusable = self.stream.as_ref().is_some_and(|s| {
            let next = s.time_of(s.next);
            s.path == t.path
                && s.size == t.size
                && s.hdr == t.hdr
                && !s.ended
                && t.source_seconds >= next - 2.0 * frame_dur
                && t.source_seconds <= next + 1.0
        });
        if !reusable {
            self.stream = None;
            self.stream = self.start_stream(req);
        }
        let Some(stream) = self.stream.as_mut() else { return };
        let mut newest = None;
        while stream.time_of(stream.next) <= t.source_seconds + frame_dur * 0.5 {
            // A fresh FFmpeg needs a moment to open the file and seek.
            let wait = if stream.next == 0 { 5_000 } else { 400 };
            match stream.frames.recv_timeout(Duration::from_millis(wait)) {
                Ok(buf) => {
                    newest = Some(buf);
                    stream.next += 1;
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    stream.ended = true;
                    break;
                }
            }
        }
        if let Some(rgba) = newest {
            self.shown = None;
            self.shared.publish(Some(Frame {
                clip: stream.clip,
                path: stream.path.clone(),
                width: stream.size.0,
                height: stream.size.1,
                rgba,
            }));
        }
    }
}

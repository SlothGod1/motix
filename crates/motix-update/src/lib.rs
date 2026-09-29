//! Signed automatic updates for MOTIX.
//!
//! The [`Updater`] runs on a background thread. When MOTIX starts, and then every
//! [`CHECK_INTERVAL`] (10 minutes) while automatic checks are on, it:
//!
//! 1. asks GitHub for the list of releases (a conditional request, so an unchanged
//!    list costs almost nothing and doesn't count against GitHub's limits);
//! 2. picks the newest release newer than the running version ([`github::choose`]);
//! 3. downloads its `SHA256SUMS` and `SHA256SUMS.sig`, and **verifies the signature**
//!    against the public keys built into MOTIX ([`TRUSTED_KEYS`]) and that the
//!    manifest names that exact version (no downgrades);
//! 4. downloads the program archive in the background, checking its SHA-256 against
//!    the signed manifest — a file that doesn't match is deleted, never installed;
//! 5. reports [`Phase::Ready`]. Nothing is installed until the user restarts (or
//!    closes MOTIX, if they chose "Later").
//!
//! Installing ([`Updater::install`]) unpacks the archive next to the program and
//! swaps the files in with rollback on failure ([`install`]). The previous version
//! is kept in `.motix-update/previous`.
//!
//! This is the preview-channel updater described in ARCHITECTURE §15 and ADR-027. The
//! full TUF-based design (ADR-012) replaces the manifest format later; the signing
//! key and the user-facing behaviour stay the same.

#![forbid(unsafe_code)]

pub mod github;
pub mod http;
pub mod install;
pub mod verify;

use semver::Version;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

pub use http::{Http, UreqHttp};

/// How often MOTIX checks for updates while it's open (with automatic checks on).
pub const CHECK_INTERVAL: Duration = Duration::from_mins(10);
/// Largest release list accepted.
pub const MAX_LIST_BYTES: u64 = 8 * 1024 * 1024;
/// Largest program archive accepted.
pub const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;

/// Public keys whose signatures MOTIX accepts on updates (Ed25519, 32 bytes each).
///
/// The matching secret key is stored only as the `MOTIX_UPDATE_SIGNING_KEY` secret of
/// the GitHub repository. To rotate keys, ship a release that trusts both the old and
/// the new key, then sign with the new one.
pub const TRUSTED_KEYS: &[[u8; 32]] = &[TRUSTED_KEY_1];

/// MOTIX update key #1 (hex `7764ad35235271f778c92cdcaa24e73d79894ee127f1aa42aafe8687cd96526b`).
const TRUSTED_KEY_1: [u8; 32] = [
    0x77, 0x64, 0xad, 0x35, 0x23, 0x52, 0x71, 0xf7, 0x78, 0xc9, 0x2c, 0xdc, 0xaa, 0x24, 0xe7, 0x3d, 0x79, 0x89, 0x4e,
    0xe1, 0x27, 0xf1, 0xaa, 0x42, 0xaa, 0xfe, 0x86, 0x87, 0xcd, 0x96, 0x52, 0x6b,
];

/// Why an update couldn't be checked, downloaded or installed, in plain language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    /// Network or GitHub trouble.
    Network(String),
    /// The release isn't signed (or no key is configured).
    Unsigned,
    /// The signature doesn't match MOTIX's key.
    BadSignature,
    /// The signed checksum list is wrong.
    BadManifest(&'static str),
    /// The download doesn't match its signed checksum.
    ChecksumMismatch,
    /// The download is larger than allowed.
    TooLarge,
    /// The archive is unusable or unsafe.
    BadArchive(&'static str),
    /// A file-system problem.
    Disk(String),
    /// There's nothing downloaded to install.
    NothingToInstall,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(why) => write!(f, "Couldn't check for updates: {why}."),
            Self::Unsigned => f.write_str("The newest release isn't signed, so MOTIX won't install it."),
            Self::BadSignature => f.write_str(
                "The newest release's signature doesn't match MOTIX's key, so it was not installed. \
                 This protects you from tampered downloads.",
            ),
            Self::BadManifest(why) => write!(f, "The release's checksum list can't be trusted ({why})."),
            Self::ChecksumMismatch => {
                f.write_str("The download was damaged or changed, so it was deleted. MOTIX will try again later.")
            }
            Self::TooLarge => f.write_str("The update is unexpectedly large, so it was not downloaded."),
            Self::BadArchive(why) => write!(f, "The update file is unusable ({why})."),
            Self::Disk(why) => write!(
                f,
                "Couldn't write the update to disk ({why}). If MOTIX is in a protected folder, move it to a folder you own."
            ),
            Self::NothingToInstall => f.write_str("There's no downloaded update to install."),
        }
    }
}

impl std::error::Error for UpdateError {}

/// Where the updater is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Hasn't checked yet.
    Idle,
    /// Asking GitHub.
    Checking,
    /// The running version is the newest.
    UpToDate,
    /// Downloading `version` in the background.
    Downloading {
        /// Version.
        version: String,
        /// Bytes received.
        done: u64,
        /// Total bytes, when known.
        total: Option<u64>,
    },
    /// Downloaded and verified.
    Ready {
        /// Version.
        version: String,
        /// Release notes.
        notes: String,
    },
    /// Swapping files.
    Installing,
    /// The last attempt failed (the current version keeps working).
    Failed(UpdateError),
}

/// A snapshot of the updater, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// Current phase.
    pub phase: Phase,
    /// When the last check finished.
    pub last_checked: Option<SystemTime>,
    /// Whether automatic checks are on.
    pub auto_check: bool,
}

/// How the updater is set up.
#[derive(Clone, Debug)]
pub struct Config {
    /// GitHub repository, `owner/name`.
    pub repo: String,
    /// The running version.
    pub current: Version,
    /// This platform's archive name, e.g. `motix-windows-x64.zip`.
    pub archive_name: String,
    /// Accept preview releases.
    pub include_prereleases: bool,
    /// Where downloads are kept until installed.
    pub download_dir: PathBuf,
    /// The folder MOTIX runs from (files here are replaced on install).
    pub install_dir: PathBuf,
    /// Accepted signing keys (normally [`TRUSTED_KEYS`]).
    pub trusted_keys: Vec<[u8; 32]>,
    /// Check automatically at start-up and every [`CHECK_INTERVAL`].
    pub auto_check: bool,
    /// Override of [`CHECK_INTERVAL`] (tests).
    pub interval: Duration,
}

/// The archive name for the platform MOTIX was built for.
#[must_use]
pub fn platform_archive_name() -> &'static str {
    if cfg!(windows) {
        "motix-windows-x64.zip"
    } else if cfg!(target_os = "macos") {
        "motix-macos-arm64.zip"
    } else {
        "motix-linux-x64.zip"
    }
}

#[derive(Clone, Debug)]
struct ReadyUpdate {
    version: Version,
    archive: PathBuf,
}

struct Shared {
    status: Status,
    ready: Option<ReadyUpdate>,
    etag: Option<String>,
    releases: Vec<github::Release>,
}

enum Command {
    Check,
    SetAuto(bool),
    Stop,
}

/// The background updater.
pub struct Updater {
    shared: Arc<Mutex<Shared>>,
    tx: mpsc::Sender<Command>,
    config: Config,
}

fn lock(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Updater {
    /// Starts the background thread. With automatic checks on, it checks right away.
    #[must_use]
    pub fn start(config: Config, http: impl Http) -> Self {
        install::clean_staging(&config.install_dir);
        let shared = Arc::new(Mutex::new(Shared {
            status: Status {
                phase: Phase::Idle,
                last_checked: None,
                auto_check: config.auto_check,
            },
            ready: None,
            etag: None,
            releases: Vec::new(),
        }));
        let (tx, rx) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let worker_config = config.clone();
        let spawned = std::thread::Builder::new()
            .name("motix-updater".to_owned())
            .spawn(move || worker(&worker_config, &http, &worker_shared, &rx));
        if let Err(e) = spawned {
            lock(&shared).status.phase = Phase::Failed(UpdateError::Disk(e.to_string()));
        }
        Self { shared, tx, config }
    }

    /// The current status.
    #[must_use]
    pub fn status(&self) -> Status {
        lock(&self.shared).status.clone()
    }

    /// Checks now (in the background).
    pub fn check_now(&self) {
        let _ = self.tx.send(Command::Check);
    }

    /// Turns automatic checks on or off.
    pub fn set_auto_check(&self, on: bool) {
        lock(&self.shared).status.auto_check = on;
        let _ = self.tx.send(Command::SetAuto(on));
    }

    /// Installs the downloaded update. Returns the program to start afterwards.
    ///
    /// # Errors
    /// [`UpdateError::NothingToInstall`], or unpacking/disk errors (the current
    /// version is left intact).
    pub fn install(&self) -> Result<PathBuf, UpdateError> {
        let ready = lock(&self.shared).ready.clone().ok_or(UpdateError::NothingToInstall)?;
        lock(&self.shared).status.phase = Phase::Installing;
        let result = (|| -> Result<Vec<PathBuf>, UpdateError> {
            let (staging, backup) = install::work_paths(&self.config.install_dir);
            let files = install::extract(&ready.archive, &staging)?;
            install::apply(&staging, &files, &self.config.install_dir, &backup)?;
            install::clean_staging(&self.config.install_dir);
            Ok(files)
        })();
        match result {
            Ok(files) => {
                let _ = fs::remove_dir_all(self.config.download_dir.join(ready.version.to_string()));
                lock(&self.shared).ready = None;
                let exe = files
                    .iter()
                    .find(|f| f.file_stem().is_some_and(|s| s == "motix") && f.components().count() == 1)
                    .map_or_else(
                        || self.config.install_dir.join(exe_name()),
                        |f| self.config.install_dir.join(f),
                    );
                Ok(exe)
            }
            Err(e) => {
                lock(&self.shared).status.phase = Phase::Failed(e.clone());
                Err(e)
            }
        }
    }

    /// Installs the downloaded update and starts the new version. The caller should
    /// then close MOTIX.
    ///
    /// # Errors
    /// As [`Updater::install`], or if the new version can't be started.
    pub fn install_and_restart(&self) -> Result<(), UpdateError> {
        let exe = self.install()?;
        std::process::Command::new(&exe)
            .arg("--updated")
            .spawn()
            .map(|_| ())
            .map_err(|e| UpdateError::Disk(format!("couldn't start the new version: {e}")))
    }

    /// `true` when an update is downloaded and ready.
    #[must_use]
    pub fn has_ready_update(&self) -> bool {
        lock(&self.shared).ready.is_some()
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
    }
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "motix.exe" } else { "motix" }
}

fn worker(config: &Config, http: &dyn Http, shared: &Mutex<Shared>, rx: &mpsc::Receiver<Command>) {
    let mut auto = config.auto_check;
    let mut next = Instant::now();
    loop {
        let wait = if auto {
            next.saturating_duration_since(Instant::now())
        } else {
            Duration::from_hours(1)
        };
        let run = match rx.recv_timeout(wait) {
            Ok(Command::Check) => true,
            Ok(Command::SetAuto(on)) => {
                auto = on;
                next = Instant::now() + config.interval;
                false
            }
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => auto,
        };
        if run {
            check(config, http, shared);
            next = Instant::now() + config.interval;
        }
    }
}

fn set_phase(shared: &Mutex<Shared>, phase: Phase) {
    lock(shared).status.phase = phase;
}

/// One complete check (and download, if there is something new).
fn check(config: &Config, http: &dyn Http, shared: &Mutex<Shared>) {
    // Don't interrupt a finished download with a "Checking" flicker.
    let had_ready = lock(shared).ready.clone();
    if had_ready.is_none() {
        set_phase(shared, Phase::Checking);
    }
    let result = check_inner(config, http, shared, had_ready.as_ref());
    let mut s = lock(shared);
    s.status.last_checked = Some(SystemTime::now());
    match result {
        Ok(()) => {}
        Err(e) if had_ready.is_some() => {
            // Keep offering the verified update we already have.
            let _ = e;
        }
        Err(e) => s.status.phase = Phase::Failed(e),
    }
}

fn check_inner(
    config: &Config,
    http: &dyn Http,
    shared: &Mutex<Shared>,
    had_ready: Option<&ReadyUpdate>,
) -> Result<(), UpdateError> {
    let url = format!("https://api.github.com/repos/{}/releases?per_page=30", config.repo);
    let etag = lock(shared).etag.clone();
    let resp = http.get(&url, etag.as_deref(), MAX_LIST_BYTES)?;
    let releases = if resp.not_modified {
        lock(shared).releases.clone()
    } else {
        let list = github::parse_releases(&resp.body)
            .map_err(|_| UpdateError::Network("GitHub's answer wasn't a release list".to_owned()))?;
        let mut s = lock(shared);
        s.etag = resp.etag;
        s.releases.clone_from(&list);
        list
    };
    let Some(candidate) = github::choose(
        &releases,
        &config.current,
        config.include_prereleases,
        &config.archive_name,
    ) else {
        if had_ready.is_none() {
            set_phase(shared, Phase::UpToDate);
        }
        return Ok(());
    };
    if had_ready.is_some_and(|r| r.version >= candidate.version) {
        return Ok(());
    }
    let version = candidate.version.to_string();

    // Verify the signed manifest before downloading anything big.
    let sums = http.get(&candidate.sums.browser_download_url, None, 1024 * 1024)?.body;
    let sig = http.get(&candidate.signature.browser_download_url, None, 4096)?.body;
    verify::verify_signature(&sums, &sig, &config.trusted_keys)?;
    let expected = verify::expected_hash(&sums, &version, &config.archive_name)?;

    let dir = config.download_dir.join(&version);
    fs::create_dir_all(&dir).map_err(|e| UpdateError::Disk(e.to_string()))?;
    let final_path = dir.join(&config.archive_name);
    if !(final_path.exists() && sha256_file(&final_path).is_ok_and(|h| h == expected)) {
        let part = dir.join(format!("{}.part", config.archive_name));
        let total_hint = Some(candidate.archive.size).filter(|s| *s > 0);
        set_phase(
            shared,
            Phase::Downloading {
                version: version.clone(),
                done: 0,
                total: total_hint,
            },
        );
        let file = fs::File::create(&part).map_err(|e| UpdateError::Disk(e.to_string()))?;
        let mut hasher = HashingWriter {
            inner: BufWriter::new(file),
            hasher: Sha256::new(),
        };
        let mut last_report = Instant::now();
        let mut progress = |done: u64, total: Option<u64>| {
            if last_report.elapsed() > Duration::from_millis(200) {
                last_report = Instant::now();
                set_phase(
                    shared,
                    Phase::Downloading {
                        version: version.clone(),
                        done,
                        total: total.or(total_hint),
                    },
                );
            }
        };
        let downloaded = http.download(
            &candidate.archive.browser_download_url,
            MAX_ARCHIVE_BYTES,
            &mut hasher,
            &mut progress,
        );
        let flushed = hasher.inner.flush().map_err(|e| UpdateError::Disk(e.to_string()));
        if let Err(e) = downloaded.and(flushed) {
            let _ = fs::remove_file(&part);
            return Err(e);
        }
        let digest: [u8; 32] = hasher.hasher.finalize().into();
        drop(hasher.inner);
        if digest != expected {
            let _ = fs::remove_file(&part);
            return Err(UpdateError::ChecksumMismatch);
        }
        fs::rename(&part, &final_path).map_err(|e| UpdateError::Disk(e.to_string()))?;
    }
    let mut s = lock(shared);
    s.ready = Some(ReadyUpdate {
        version: candidate.version.clone(),
        archive: final_path,
    });
    s.status.phase = Phase::Ready {
        version,
        notes: candidate.notes,
    };
    Ok(())
}

struct HashingWriter<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn sha256_file(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0_u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tests::{make_zip, temp_dir};
    use crate::verify::tests::{sign, test_key};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// An in-memory GitHub.
    struct FakeHttp {
        files: HashMap<String, Vec<u8>>,
        list_calls: AtomicUsize,
    }

    impl Http for FakeHttp {
        fn get(&self, url: &str, etag: Option<&str>, max: u64) -> Result<http::Response, UpdateError> {
            if url.contains("/releases?") {
                self.list_calls.fetch_add(1, Ordering::SeqCst);
                if etag == Some("\"v1\"") {
                    return Ok(http::Response {
                        not_modified: true,
                        ..Default::default()
                    });
                }
            }
            let body = self.files.get(url).cloned().ok_or(UpdateError::Network("404".into()))?;
            assert!(body.len() as u64 <= max);
            Ok(http::Response {
                not_modified: false,
                body,
                etag: Some("\"v1\"".into()),
            })
        }
        fn download(
            &self,
            url: &str,
            _max: u64,
            out: &mut dyn Write,
            progress: &mut dyn FnMut(u64, Option<u64>),
        ) -> Result<(), UpdateError> {
            let body = self.files.get(url).ok_or(UpdateError::Network("404".into()))?;
            out.write_all(body).unwrap();
            progress(body.len() as u64, Some(body.len() as u64));
            Ok(())
        }
    }

    /// Shares one fake GitHub between the test and the updater thread.
    struct Shared(Arc<FakeHttp>);
    impl Http for Shared {
        fn get(&self, u: &str, e: Option<&str>, m: u64) -> Result<http::Response, UpdateError> {
            self.0.get(u, e, m)
        }
        fn download(
            &self,
            u: &str,
            m: u64,
            o: &mut dyn Write,
            p: &mut dyn FnMut(u64, Option<u64>),
        ) -> Result<(), UpdateError> {
            self.0.download(u, m, o, p)
        }
    }
    struct Setup {
        dir: PathBuf,
        config: Config,
        http: FakeHttp,
    }

    fn setup(tamper: bool, signed_by_us: bool) -> Setup {
        let dir = temp_dir("updater");
        let install_dir = dir.join("MOTIX");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join(exe_name()), b"old program").unwrap();
        let zip_path = dir.join("release.zip");
        make_zip(
            &zip_path,
            &[
                (&format!("MOTIX/{}", exe_name()), b"new program"),
                ("MOTIX/READ-ME-FIRST.txt", b"hi"),
            ],
        );
        let zip_bytes = fs::read(&zip_path).unwrap();
        let archive = platform_archive_name();
        let hash = verify::to_hex(&Sha256::digest(&zip_bytes));
        let sums = format!("# motix-version 0.2.0-preview.3\n{hash}  {archive}\n");
        let sig = if signed_by_us {
            sign(sums.as_bytes())
        } else {
            use ed25519_dalek::Signer;
            let other = ed25519_dalek::SigningKey::from_bytes(&[1; 32]);
            verify::to_hex(&other.sign(sums.as_bytes()).to_bytes()).into_bytes()
        };
        let base = "https://github.com/o/r/releases/download/v0.2.0-preview.3";
        let list = format!(
            r#"[{{"tag_name":"v0.2.0-preview.3","prerelease":true,"body":"New timeline","assets":[
                {{"name":"{archive}","browser_download_url":"{base}/{archive}","size":{}}},
                {{"name":"SHA256SUMS","browser_download_url":"{base}/SHA256SUMS"}},
                {{"name":"SHA256SUMS.sig","browser_download_url":"{base}/SHA256SUMS.sig"}}]}},
               {{"tag_name":"v0.1.0-preview.1","prerelease":true,"assets":[]}}]"#,
            zip_bytes.len()
        );
        let mut files = HashMap::new();
        files.insert(
            "https://api.github.com/repos/o/r/releases?per_page=30".to_owned(),
            list.into_bytes(),
        );
        let mut served = zip_bytes;
        if tamper {
            served.push(0);
        }
        files.insert(format!("{base}/{archive}"), served);
        files.insert(format!("{base}/SHA256SUMS"), sums.into_bytes());
        files.insert(format!("{base}/SHA256SUMS.sig"), sig);
        let config = Config {
            repo: "o/r".into(),
            current: Version::parse("0.1.0-preview.2").unwrap(),
            archive_name: archive.into(),
            include_prereleases: true,
            download_dir: dir.join("downloads"),
            install_dir,
            trusted_keys: vec![test_key().verifying_key().to_bytes()],
            auto_check: true,
            interval: Duration::from_millis(100),
        };
        Setup {
            dir,
            config,
            http: FakeHttp {
                files,
                list_calls: AtomicUsize::new(0),
            },
        }
    }

    fn wait_for(updater: &Updater, what: impl Fn(&Phase) -> bool) -> Phase {
        for _ in 0..500 {
            let p = updater.status().phase;
            if what(&p) {
                return p;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out; phase is {:?}", updater.status().phase);
    }

    #[test]
    fn checks_downloads_verifies_and_installs() {
        let s = setup(false, true);
        let install_dir = s.config.install_dir.clone();
        let updater = Updater::start(s.config, s.http);
        let phase = wait_for(&updater, |p| matches!(p, Phase::Ready { .. } | Phase::Failed(_)));
        assert_eq!(
            phase,
            Phase::Ready {
                version: "0.2.0-preview.3".into(),
                notes: "New timeline".into()
            }
        );
        assert!(updater.has_ready_update());
        assert!(updater.status().last_checked.is_some());
        let exe = updater.install().unwrap();
        assert_eq!(exe, install_dir.join(exe_name()));
        assert_eq!(fs::read(&exe).unwrap(), b"new program");
        assert_eq!(
            fs::read(install_dir.join(".motix-update/previous").join(exe_name())).unwrap(),
            b"old program"
        );
        assert!(!updater.has_ready_update());
        let _ = fs::remove_dir_all(s.dir);
    }

    #[test]
    fn tampered_downloads_are_rejected() {
        let s = setup(true, true);
        let updater = Updater::start(s.config, s.http);
        let phase = wait_for(&updater, |p| matches!(p, Phase::Ready { .. } | Phase::Failed(_)));
        assert_eq!(phase, Phase::Failed(UpdateError::ChecksumMismatch));
        assert_eq!(updater.install(), Err(UpdateError::NothingToInstall));
        let _ = fs::remove_dir_all(s.dir);
    }

    #[test]
    fn wrong_signing_key_is_rejected() {
        let s = setup(false, false);
        let updater = Updater::start(s.config, s.http);
        let phase = wait_for(&updater, |p| matches!(p, Phase::Ready { .. } | Phase::Failed(_)));
        assert_eq!(phase, Phase::Failed(UpdateError::BadSignature));
        let _ = fs::remove_dir_all(s.dir);
    }

    #[test]
    fn up_to_date_and_periodic_checks_use_etags() {
        let mut s = setup(false, true);
        s.config.current = Version::parse("0.2.0-preview.3").unwrap();
        let http = Arc::new(s.http);
        let updater = Updater::start(s.config, Shared(Arc::clone(&http)));
        assert_eq!(wait_for(&updater, |p| *p == Phase::UpToDate), Phase::UpToDate);
        // The 100 ms test interval triggers more checks (answered "not modified").
        std::thread::sleep(Duration::from_millis(350));
        assert!(http.list_calls.load(Ordering::SeqCst) >= 2);
        assert_eq!(updater.status().phase, Phase::UpToDate);
        updater.set_auto_check(false);
        assert!(!updater.status().auto_check);
        let _ = fs::remove_dir_all(s.dir);
    }

    #[test]
    fn no_trusted_key_means_nothing_installs() {
        let mut s = setup(false, true);
        s.config.trusted_keys.clear();
        let updater = Updater::start(s.config, s.http);
        let phase = wait_for(&updater, |p| matches!(p, Phase::Ready { .. } | Phase::Failed(_)));
        assert_eq!(phase, Phase::Failed(UpdateError::Unsigned));
        let _ = fs::remove_dir_all(s.dir);
    }

    #[test]
    fn built_in_key_is_valid() {
        for k in TRUSTED_KEYS {
            assert!(ed25519_dalek::VerifyingKey::from_bytes(k).is_ok());
            assert_ne!(k, &[0_u8; 32]);
        }
    }

    #[test]
    fn messages_are_plain() {
        assert!(UpdateError::BadSignature.to_string().contains("tampered"));
        assert!(
            UpdateError::Network("no internet connection".into())
                .to_string()
                .starts_with("Couldn't check")
        );
    }
}

//! MOTIX installed in a **shared folder** that several PCs run it from.
//!
//! Layout (ADR-031):
//!
//! ```text
//! <shared folder>\                e.g. B:\MOTIX, shared on the network as \\PC\MOTIX
//!   MOTIX.exe                     a copy of MOTIX that acts as the launcher: it starts
//!                                 the newest installed version
//!   motix-shared.json             marks the folder as a shared MOTIX
//!   READ-ME.txt
//!   versions\
//!     current                     text file naming the version the launcher starts
//!     0.1.0-preview.4\motix.exe   one folder per version (never changed once written)
//!     0.1.0-preview.5\motix.exe
//!   updates\                      optional inbox: a signed release dropped here by hand
//! ```
//!
//! A running copy is never touched: a new version goes into its own folder, and
//! `current` is switched to it in one step. PCs that already have MOTIX open keep
//! running their version and pick up the new one next time they start it. Old
//! versions are removed only when no PC is using them.

use crate::UpdateError;
use crate::install::extract;
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// File that marks a folder as a shared MOTIX.
pub const MARKER: &str = "motix-shared.json";
/// Folder holding one sub-folder per installed version.
pub const VERSIONS: &str = "versions";
/// File (in [`VERSIONS`]) naming the version to start.
pub const CURRENT: &str = "current";
/// The optional inbox for signed releases copied in by hand.
pub const UPDATES: &str = "updates";
/// How many versions to keep (older ones are removed when nobody uses them).
pub const KEEP_VERSIONS: usize = 3;

fn disk(e: &std::io::Error) -> UpdateError {
    UpdateError::Disk(e.to_string())
}

/// The program's file name.
#[must_use]
pub fn app_exe_name() -> &'static str {
    if cfg!(windows) { "motix.exe" } else { "motix" }
}

/// The launcher's name at the top of the shared folder. It's a copy of the program:
/// started from there, MOTIX only starts the newest version ([`is_launcher`]).
#[must_use]
pub fn root_launcher_name() -> &'static str {
    if cfg!(windows) { "MOTIX.exe" } else { "MOTIX" }
}

/// `true` when `exe` is the launcher at the top of a shared folder (so it should just
/// start the newest version, see [`pick`]).
#[must_use]
pub fn is_launcher(exe: &Path) -> bool {
    exe.parent().is_some_and(|dir| dir.join(MARKER).is_file())
}

/// If `exe` (the running program) lives in a shared folder's `versions\<version>\`,
/// returns the shared folder.
#[must_use]
pub fn root_of(exe: &Path) -> Option<PathBuf> {
    let version_dir = exe.parent()?;
    let versions = version_dir.parent()?;
    if versions.file_name()? != VERSIONS {
        return None;
    }
    let root = versions.parent()?;
    root.join(MARKER).is_file().then(|| root.to_path_buf())
}

/// The version `current` names, if its program is actually there.
#[must_use]
pub fn current(root: &Path) -> Option<Version> {
    let text = fs::read_to_string(root.join(VERSIONS).join(CURRENT)).ok()?;
    let v = Version::parse(text.trim()).ok()?;
    version_exe(root, &v).is_file().then_some(v)
}

/// Where a version's program is.
#[must_use]
pub fn version_exe(root: &Path, v: &Version) -> PathBuf {
    root.join(VERSIONS).join(v.to_string()).join(app_exe_name())
}

/// Every complete installed version, newest first.
#[must_use]
pub fn installed(root: &Path) -> Vec<Version> {
    let mut out: Vec<Version> = fs::read_dir(root.join(VERSIONS))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Version::parse(e.file_name().to_str()?).ok())
        .filter(|v| version_exe(root, v).is_file())
        .collect();
    out.sort_by(|a, b| b.cmp(a));
    out
}

/// The version the launcher should start: `current`, or else the newest complete
/// version (if `current` is missing or damaged).
#[must_use]
pub fn pick(root: &Path) -> Option<(Version, PathBuf)> {
    let v = current(root).or_else(|| installed(root).into_iter().next())?;
    let exe = version_exe(root, &v);
    Some((v, exe))
}

fn unique(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(".{tag}-{}-{nanos}", std::process::id())
}

/// Points `current` at `v`, in one step, unless it already names something newer.
fn set_current(root: &Path, v: &Version) -> Result<(), UpdateError> {
    if current(root).is_some_and(|c| c >= *v) {
        return Ok(());
    }
    let versions = root.join(VERSIONS);
    let tmp = versions.join(unique("current"));
    fs::write(&tmp, format!("{v}\n")).map_err(|e| disk(&e))?;
    fs::rename(&tmp, versions.join(CURRENT)).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        disk(&e)
    })
}

/// Unpacks a verified update archive as version `v` of the shared folder at `root`
/// and makes it the one the launcher starts. Safe to run from several PCs at once:
/// the first to finish wins and the others' copies are discarded.
///
/// # Errors
/// Unpacking or disk errors; nothing that's running is affected.
pub fn install(archive: &Path, root: &Path, v: &Version) -> Result<(), UpdateError> {
    let versions = root.join(VERSIONS);
    fs::create_dir_all(&versions).map_err(|e| disk(&e))?;
    let target = versions.join(v.to_string());
    if !version_exe(root, v).is_file() {
        let incoming = versions.join(unique("incoming"));
        let files = extract(archive, &incoming)?;
        if !files.iter().any(|f| f.as_os_str() == app_exe_name()) {
            let _ = fs::remove_dir_all(&incoming);
            return Err(UpdateError::BadArchive("MOTIX itself is missing from it"));
        }
        if let Err(e) = fs::rename(&incoming, &target) {
            let _ = fs::remove_dir_all(&incoming);
            // Another PC may have installed the same version a moment ago.
            if !version_exe(root, v).is_file() {
                return Err(disk(&e));
            }
        }
    }
    set_current(root, v)?;
    refresh_launcher(root, &target);
    tidy(root);
    Ok(())
}

/// Copies a version's program to the top of the shared folder as the launcher.
/// Best effort: the old launcher keeps working if it can't be replaced.
fn refresh_launcher(root: &Path, version_dir: &Path) {
    let new = version_dir.join(app_exe_name());
    let dest = root.join(root_launcher_name());
    let Ok(bytes) = fs::read(&new) else { return };
    if fs::read(&dest).is_ok_and(|old| old == bytes) {
        return;
    }
    let tmp = root.join(unique("launcher"));
    if fs::write(&tmp, &bytes).is_err() {
        let _ = fs::remove_file(&tmp);
        return;
    }
    copy_permissions(&new, &tmp);
    if fs::rename(&tmp, &dest).is_err() {
        // In use right now (Windows): move it aside first, then retry.
        let aside = root.join(unique("old-launcher"));
        if fs::rename(&dest, &aside).is_ok() && fs::rename(&tmp, &dest).is_err() {
            let _ = fs::rename(&aside, &dest);
        }
    }
    let _ = fs::remove_file(&tmp);
}

fn copy_permissions(from: &Path, to: &Path) {
    if let Ok(meta) = fs::metadata(from) {
        let _ = fs::set_permissions(to, meta.permissions());
    }
}

/// Removes versions beyond the newest [`KEEP_VERSIONS`] (never `current`) and
/// leftovers of interrupted installs. A version that some PC is still running
/// can't be renamed on Windows, so it's skipped and removed on a later try.
pub fn tidy(root: &Path) {
    let versions = root.join(VERSIONS);
    let keep_current = current(root);
    for (i, v) in installed(root).into_iter().enumerate() {
        if i < KEEP_VERSIONS || Some(&v) == keep_current.as_ref() {
            continue;
        }
        // Renaming first means a folder in use is left completely intact.
        let trash = versions.join(unique("trash"));
        if fs::rename(versions.join(v.to_string()), &trash).is_ok() {
            let _ = fs::remove_dir_all(&trash);
        }
    }
    let an_hour_ago = SystemTime::now() - std::time::Duration::from_hours(1);
    for dir in [&versions, &root.to_path_buf()] {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t < an_hour_ago);
            let path = entry.path();
            let leftover = name.starts_with(".trash-") || name.starts_with(".old-launcher-");
            // Another PC may be in the middle of writing these, so only old ones go.
            let in_progress =
                name.starts_with(".incoming-") || name.starts_with(".current-") || name.starts_with(".launcher-");
            if leftover || (stale && in_progress) {
                let _ = fs::remove_dir_all(&path);
                let _ = fs::remove_file(&path);
            }
        }
    }
}

const ROOT_README: &str = "\
MOTIX (shared copy)

Start MOTIX by double-clicking MOTIX.exe in this folder, on this PC or on any PC on
your network (for example \\\\THIS-PC-NAME\\MOTIX\\MOTIX.exe). It always starts the
newest version.

How updates work
- Any PC running MOTIX from here checks for new versions when it starts and every
  10 minutes. A new, signed version is installed into the 'versions' folder; PCs
  that have MOTIX open keep going and get the new version the next time they
  start it (or right away with 'Restart now').
- You can also copy a release (motix-windows-x64.zip, SHA256SUMS and
  SHA256SUMS.sig) into the 'updates' folder. MOTIX checks the signature and
  ignores anything that isn't signed with MOTIX's key.

Please don't rename or edit files in 'versions'. Your projects and settings are
not stored here.
";

const UPDATES_README: &str = "\
Put a MOTIX release here to install it on every PC that runs MOTIX from this
shared folder: the program archive (for example motix-windows-x64.zip) plus
SHA256SUMS and SHA256SUMS.sig from the same release. Files that aren't signed
with MOTIX's key are ignored.
";

/// Sets up (or adds this version to) a shared MOTIX at `root`, copying the program
/// files from `from_dir` (the folder the running MOTIX is in).
///
/// # Errors
/// If `root` is inside `from_dir` or not empty (and not already a shared MOTIX), or
/// on disk errors.
pub fn create(root: &Path, from_dir: &Path, v: &Version) -> Result<(), UpdateError> {
    let not_ok = |why: &str| Err(UpdateError::Disk(why.to_owned()));
    if root.starts_with(from_dir) {
        return not_ok("pick a folder outside the one MOTIX is running from");
    }
    let is_shared = root.join(MARKER).is_file();
    if !is_shared && fs::read_dir(root).is_ok_and(|mut d| d.next().is_some()) {
        return not_ok("pick an empty folder, or one that already holds a shared MOTIX");
    }
    if !from_dir.join(app_exe_name()).is_file() {
        return not_ok("MOTIX couldn't find its own program file");
    }
    let versions = root.join(VERSIONS);
    fs::create_dir_all(&versions).map_err(|e| disk(&e))?;
    let target = versions.join(v.to_string());
    if !version_exe(root, v).is_file() {
        let incoming = versions.join(unique("incoming"));
        fs::create_dir_all(&incoming).map_err(|e| disk(&e))?;
        let copied = (|| {
            for entry in fs::read_dir(from_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    let dest = incoming.join(entry.file_name());
                    fs::copy(entry.path(), &dest)?;
                }
            }
            fs::rename(&incoming, &target)
        })();
        if let Err(e) = copied {
            let _ = fs::remove_dir_all(&incoming);
            return Err(disk(&e));
        }
    }
    fs::create_dir_all(root.join(UPDATES)).map_err(|e| disk(&e))?;
    let write_new = |path: PathBuf, text: &str| -> Result<(), UpdateError> {
        if path.exists() {
            return Ok(());
        }
        fs::write(path, text).map_err(|e| disk(&e))
    };
    write_new(root.join("READ-ME.txt"), ROOT_README)?;
    write_new(root.join(UPDATES).join("READ-ME.txt"), UPDATES_README)?;
    write_new(root.join(MARKER), "{ \"format\": 1 }\n")?;
    set_current(root, v)?;
    refresh_launcher(root, &target);
    if !root.join(root_launcher_name()).is_file() {
        return not_ok("couldn't place MOTIX.exe in the shared folder");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tests::{make_zip, temp_dir};

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn portable(dir: &Path, program: &[u8]) -> PathBuf {
        let from = dir.join("download").join("MOTIX");
        fs::create_dir_all(&from).unwrap();
        fs::write(from.join(app_exe_name()), program).unwrap();
        fs::write(from.join("LICENSE.txt"), b"gpl").unwrap();
        from
    }

    fn release_zip(dir: &Path, name: &str, program: &[u8]) -> PathBuf {
        let zip = dir.join(name);
        make_zip(
            &zip,
            &[
                (&format!("MOTIX/{}", app_exe_name()), program),
                ("MOTIX/READ-ME-FIRST.txt", b"hi"),
            ],
        );
        zip
    }

    #[test]
    fn create_then_launch_the_right_version() {
        let dir = temp_dir("shared-create");
        let from = portable(&dir, b"program 1");
        let root = dir.join("Shared MOTIX");
        fs::create_dir_all(&root).unwrap();
        create(&root, &from, &v("0.1.0-preview.4")).unwrap();

        assert!(root.join(MARKER).is_file());
        assert_eq!(fs::read(root.join(root_launcher_name())).unwrap(), b"program 1");
        assert!(is_launcher(&root.join(root_launcher_name())));
        assert!(root.join(UPDATES).join("READ-ME.txt").is_file());
        assert_eq!(current(&root), Some(v("0.1.0-preview.4")));
        let (picked, exe) = pick(&root).unwrap();
        assert_eq!(picked, v("0.1.0-preview.4"));
        assert_eq!(fs::read(&exe).unwrap(), b"program 1");
        assert_eq!(root_of(&exe).as_deref(), Some(root.as_path()));
        assert_eq!(root_of(&from.join(app_exe_name())), None, "a plain folder isn't shared");
        assert!(!is_launcher(&exe) && !is_launcher(&from.join(app_exe_name())));

        // Running it again (same version) is harmless; a non-empty folder is refused.
        create(&root, &from, &v("0.1.0-preview.4")).unwrap();
        let other = dir.join("busy");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("holiday.mp4"), b"x").unwrap();
        assert!(create(&other, &from, &v("0.1.0-preview.4")).is_err());
        assert!(create(&from.join("inside"), &from, &v("0.1.0-preview.4")).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn install_adds_a_version_switches_current_and_updates_the_launcher() {
        let dir = temp_dir("shared-install");
        let from = portable(&dir, b"program 1");
        let root = dir.join("shared");
        create(&root, &from, &v("0.1.0-preview.4")).unwrap();

        let zip = release_zip(&dir, "u.zip", b"program 2");
        install(&zip, &root, &v("0.1.0-preview.5")).unwrap();
        assert_eq!(current(&root), Some(v("0.1.0-preview.5")));
        assert_eq!(
            fs::read(version_exe(&root, &v("0.1.0-preview.5"))).unwrap(),
            b"program 2"
        );
        assert_eq!(
            fs::read(version_exe(&root, &v("0.1.0-preview.4"))).unwrap(),
            b"program 1",
            "the running version is untouched"
        );
        assert_eq!(fs::read(root.join(root_launcher_name())).unwrap(), b"program 2");

        // A second PC installing the same version, or an older one, changes nothing.
        install(&zip, &root, &v("0.1.0-preview.5")).unwrap();
        install(&zip, &root, &v("0.1.0-preview.3")).unwrap();
        assert_eq!(current(&root), Some(v("0.1.0-preview.5")));

        // Archives without MOTIX in them are refused.
        let bad = dir.join("bad.zip");
        make_zip(&bad, &[("MOTIX/notes.txt", b"x")]);
        assert!(matches!(
            install(&bad, &root, &v("0.1.0-preview.9")),
            Err(UpdateError::BadArchive(_))
        ));
        assert_eq!(current(&root), Some(v("0.1.0-preview.5")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn old_versions_are_tidied_and_a_broken_current_falls_back() {
        let dir = temp_dir("shared-tidy");
        let from = portable(&dir, b"program 1");
        let root = dir.join("shared");
        create(&root, &from, &v("0.1.0-preview.1")).unwrap();
        for n in 2..=5 {
            let zip = release_zip(&dir, &format!("u{n}.zip"), b"p");
            install(&zip, &root, &v(&format!("0.1.0-preview.{n}"))).unwrap();
        }
        let left: Vec<String> = installed(&root).iter().map(ToString::to_string).collect();
        assert_eq!(left, ["0.1.0-preview.5", "0.1.0-preview.4", "0.1.0-preview.3"]);

        fs::write(root.join(VERSIONS).join(CURRENT), "garbage").unwrap();
        assert_eq!(pick(&root).unwrap().0, v("0.1.0-preview.5"));
        fs::write(root.join(VERSIONS).join(CURRENT), "9.9.9").unwrap();
        assert_eq!(pick(&root).unwrap().0, v("0.1.0-preview.5"), "names a missing version");
        let _ = fs::remove_dir_all(dir);
    }
}

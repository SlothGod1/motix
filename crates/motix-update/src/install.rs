//! Unpacking a verified update and swapping it in, with rollback.
//!
//! MOTIX is a portable folder (the program plus a few text files). An update is
//! unpacked into `<install>/.motix-update/staging`, then each file is swapped in:
//! the current file is **moved** to `<install>/.motix-update/previous` (Windows allows
//! renaming a running program) and the new file moved into place. If any step fails,
//! everything already swapped is put back. The previous version stays in `previous`
//! until the next update.

use crate::UpdateError;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// Most files an update archive may contain.
pub const MAX_ENTRIES: usize = 10_000;
/// Largest total unpacked size.
pub const MAX_UNPACKED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Folder inside the install folder used by the updater.
pub const WORK_DIR: &str = ".motix-update";

fn disk(e: &std::io::Error) -> UpdateError {
    UpdateError::Disk(e.to_string())
}

/// Staging and backup folders for an install folder.
#[must_use]
pub fn work_paths(install_dir: &Path) -> (PathBuf, PathBuf) {
    let work = install_dir.join(WORK_DIR);
    (work.join("staging"), work.join("previous"))
}

/// Unpacks `archive` into `dest` (emptied first). A single top-level folder (for
/// example `MOTIX/`) is removed so files land directly in `dest`. Returns the
/// unpacked files relative to `dest`.
///
/// # Errors
/// [`UpdateError::BadArchive`] for unsafe paths, too many files or too much data;
/// [`UpdateError::Disk`] for file-system errors.
pub fn extract(archive: &Path, dest: &Path) -> Result<Vec<PathBuf>, UpdateError> {
    let file = fs::File::open(archive).map_err(|e| disk(&e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| UpdateError::BadArchive("it isn't a valid zip file"))?;
    if zip.len() > MAX_ENTRIES {
        return Err(UpdateError::BadArchive("it has too many files"));
    }
    // Collect safe relative names first.
    let mut names = Vec::new();
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|_| UpdateError::BadArchive("an entry is damaged"))?;
        let path = entry
            .enclosed_name()
            .ok_or(UpdateError::BadArchive("it contains an unsafe path"))?;
        if entry.is_dir() {
            continue;
        }
        if !path
            .components()
            .all(|c| matches!(c, Component::Normal(n) if n != WORK_DIR))
        {
            return Err(UpdateError::BadArchive("it contains an unsafe path"));
        }
        names.push((i, path));
    }
    if names.is_empty() {
        return Err(UpdateError::BadArchive("it is empty"));
    }
    let first = |p: &Path| p.components().next().map(|c| c.as_os_str().to_owned());
    let common = first(&names[0].1).filter(|f| {
        names
            .iter()
            .all(|(_, p)| first(p).as_ref() == Some(f) && p.components().count() > 1)
    });

    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| disk(&e))?;
    }
    fs::create_dir_all(dest).map_err(|e| disk(&e))?;
    let mut total = 0_u64;
    let mut out = Vec::new();
    for (i, path) in names {
        let rel: PathBuf = match &common {
            Some(_) => path.components().skip(1).collect(),
            None => path,
        };
        let mut entry = zip
            .by_index(i)
            .map_err(|_| UpdateError::BadArchive("an entry is damaged"))?;
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| disk(&e))?;
        }
        let mut f = fs::File::create(&target).map_err(|e| disk(&e))?;
        let budget = MAX_UNPACKED_BYTES - total;
        let copied = std::io::copy(&mut (&mut entry).take(budget + 1), &mut f).map_err(|e| disk(&e))?;
        total += copied;
        if total > MAX_UNPACKED_BYTES {
            return Err(UpdateError::BadArchive("it unpacks to too much data"));
        }
        f.flush().map_err(|e| disk(&e))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o755));
        }
        out.push(rel);
    }
    Ok(out)
}

/// Swaps the files in `staging` into `install_dir`, moving the current ones to `backup`.
/// On any failure, every change is undone.
///
/// # Errors
/// [`UpdateError::Disk`] (after rolling back).
pub fn apply(staging: &Path, files: &[PathBuf], install_dir: &Path, backup: &Path) -> Result<(), UpdateError> {
    if backup.exists() {
        fs::remove_dir_all(backup).map_err(|e| disk(&e))?;
    }
    fs::create_dir_all(backup).map_err(|e| disk(&e))?;
    let mut moved_old: Vec<&PathBuf> = Vec::new();
    let mut placed_new: Vec<&PathBuf> = Vec::new();
    let result = (|| {
        for rel in files {
            let target = install_dir.join(rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| disk(&e))?;
            }
            if target.exists() {
                let keep = backup.join(rel);
                if let Some(parent) = keep.parent() {
                    fs::create_dir_all(parent).map_err(|e| disk(&e))?;
                }
                fs::rename(&target, &keep).map_err(|e| disk(&e))?;
                moved_old.push(rel);
            }
            fs::rename(staging.join(rel), &target).map_err(|e| disk(&e))?;
            placed_new.push(rel);
        }
        Ok(())
    })();
    if result.is_err() {
        for rel in placed_new {
            let _ = fs::remove_file(install_dir.join(rel));
        }
        for rel in moved_old {
            let _ = fs::rename(backup.join(rel), install_dir.join(rel));
        }
    }
    result
}

/// Removes leftovers of an interrupted update (the staging folder). The previous
/// version in `previous/` is kept as a manual fallback.
pub fn clean_staging(install_dir: &Path) {
    let (staging, _) = work_paths(install_dir);
    let _ = fs::remove_dir_all(staging);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    pub(crate) fn temp_dir(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "motix-update-test-{}-{tag}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let f = fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in files {
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn extract_strips_the_top_folder_and_rejects_escapes() {
        let dir = temp_dir("extract");
        let zip = dir.join("u.zip");
        make_zip(&zip, &[("MOTIX/motix.exe", b"new"), ("MOTIX/LICENSE.txt", b"gpl")]);
        let dest = dir.join("out");
        let mut files = extract(&zip, &dest).unwrap();
        files.sort();
        assert_eq!(files, [PathBuf::from("LICENSE.txt"), PathBuf::from("motix.exe")]);
        assert_eq!(fs::read(dest.join("motix.exe")).unwrap(), b"new");

        let evil = dir.join("evil.zip");
        make_zip(&evil, &[("../../outside.txt", b"x")]);
        assert!(matches!(extract(&evil, &dest), Err(UpdateError::BadArchive(_))));
        assert!(!dir.join("outside.txt").exists());

        let sneaky = dir.join("sneaky.zip");
        make_zip(&sneaky, &[(".motix-update/previous/x", b"x")]);
        assert!(matches!(extract(&sneaky, &dest), Err(UpdateError::BadArchive(_))));

        fs::write(dir.join("junk.zip"), b"not a zip").unwrap();
        assert!(matches!(
            extract(&dir.join("junk.zip"), &dest),
            Err(UpdateError::BadArchive(_))
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_swaps_files_and_keeps_the_previous_version() {
        let dir = temp_dir("apply");
        let install = dir.join("MOTIX");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("motix.exe"), b"old").unwrap();
        fs::write(install.join("my-notes.txt"), b"user file").unwrap();
        let zip = dir.join("u.zip");
        make_zip(&zip, &[("MOTIX/motix.exe", b"new"), ("MOTIX/READ-ME-FIRST.txt", b"hi")]);
        let (staging, backup) = work_paths(&install);
        let files = extract(&zip, &staging).unwrap();
        apply(&staging, &files, &install, &backup).unwrap();
        assert_eq!(fs::read(install.join("motix.exe")).unwrap(), b"new");
        assert_eq!(fs::read(install.join("READ-ME-FIRST.txt")).unwrap(), b"hi");
        assert_eq!(
            fs::read(install.join("my-notes.txt")).unwrap(),
            b"user file",
            "other files untouched"
        );
        assert_eq!(fs::read(backup.join("motix.exe")).unwrap(), b"old");
        clean_staging(&install);
        assert!(!staging.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_rolls_back_on_failure() {
        let dir = temp_dir("rollback");
        let install = dir.join("MOTIX");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("a.txt"), b"old a").unwrap();
        fs::write(install.join("b.txt"), b"old b").unwrap();
        let (staging, backup) = work_paths(&install);
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("a.txt"), b"new a").unwrap();
        // b.txt is listed but missing from staging, so the second swap fails.
        let files = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        assert!(apply(&staging, &files, &install, &backup).is_err());
        assert_eq!(fs::read(install.join("a.txt")).unwrap(), b"old a", "rolled back");
        assert_eq!(fs::read(install.join("b.txt")).unwrap(), b"old b");
        let _ = fs::remove_dir_all(dir);
    }
}

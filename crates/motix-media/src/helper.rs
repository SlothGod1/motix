//! Getting the video helper (FFmpeg) onto a PC that doesn't have it (ADR-034).
//!
//! The Windows build of MOTIX doesn't carry FFmpeg inside every update (that would make
//! each update tens of megabytes bigger). Instead MOTIX downloads one **pinned** FFmpeg
//! build once, when the user agrees, and checks it against a SHA-256 built into MOTIX
//! before using it — a changed or damaged download is thrown away. A MOTIX running from
//! a shared network folder keeps the helper in that folder, so every PC uses one copy.

use crate::Tools;
use motix_update::Http;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The FFmpeg build MOTIX uses (Gyan Doshi's "essentials" build, GPL v3).
pub const VERSION: &str = "9.0.2";
/// Where it's downloaded from.
pub const URL: &str = "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip";
/// Its SHA-256, as published with the release.
pub const SHA256: &str = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba";
/// About how big the download is, for the "download?" question.
pub const DOWNLOAD_MB: u32 = 109;
/// Largest download accepted.
const MAX_BYTES: u64 = 400 * 1024 * 1024;
/// Largest single file unpacked from it.
const MAX_FILE_BYTES: u64 = 400 * 1024 * 1024;

/// The folder (inside `base`) that holds this version of the helper.
#[must_use]
pub fn folder(base: &Path) -> PathBuf {
    base.join(format!("ffmpeg-{VERSION}"))
}

/// `true` when MOTIX can download the helper on this platform (Windows); elsewhere
/// people install FFmpeg with their system's package manager.
#[must_use]
pub const fn can_download() -> bool {
    cfg!(windows)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Progress of a download: `(bytes received, total if known)`.
pub type Progress<'a> = &'a mut dyn FnMut(u64, Option<u64>);

struct Hashing<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Downloads, checks and unpacks the helper into [`folder`]`(base)`.
///
/// # Errors
/// A plain-language reason; nothing half-finished is left behind.
pub fn download(http: &dyn Http, base: &Path, progress: Progress<'_>) -> Result<Tools, String> {
    fs::create_dir_all(base).map_err(|e| format!("couldn't create {} ({e})", base.display()))?;
    let part = base.join(format!(".ffmpeg-{VERSION}-{}.zip.part", std::process::id()));
    let result = (|| {
        let file = fs::File::create(&part).map_err(|e| format!("couldn't save the download ({e})"))?;
        let mut out = Hashing {
            inner: std::io::BufWriter::new(file),
            hasher: Sha256::new(),
        };
        http.download(URL, MAX_BYTES, &mut out, progress)
            .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| format!("couldn't save the download ({e})"))?;
        if hex(&out.hasher.finalize()) != SHA256 {
            return Err("the download didn't match MOTIX's checksum, so it was thrown away".to_owned());
        }
        unpack(&part, base)
    })();
    let _ = fs::remove_file(&part);
    result
}

/// Unpacks `ffmpeg.exe`, `ffplay.exe` and the licence from a verified archive.
///
/// # Errors
/// A plain-language reason.
pub fn unpack(archive: &Path, base: &Path) -> Result<Tools, String> {
    let bad = |why: &str| format!("the helper's archive is unusable ({why})");
    let file = fs::File::open(archive).map_err(|e| bad(&e.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| bad(&e.to_string()))?;
    let staging = base.join(format!(".ffmpeg-{VERSION}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| bad(&e.to_string()))?;
    let wanted = [
        ("ffmpeg.exe", "ffmpeg.exe"),
        ("ffplay.exe", "ffplay.exe"),
        ("ffmpeg", "ffmpeg"),
        ("ffplay", "ffplay"),
        ("LICENSE", "FFMPEG-LICENSE.txt"),
    ];
    let mut found = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| bad(&e.to_string()))?;
        let Some(name) = entry
            .enclosed_name()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        else {
            continue;
        };
        let Some((_, save_as)) = wanted.iter().find(|(n, _)| *n == name) else {
            continue;
        };
        if entry.is_dir() || found.contains(save_as) {
            continue;
        }
        let target = staging.join(save_as);
        let mut f = fs::File::create(&target).map_err(|e| bad(&e.to_string()))?;
        let copied =
            std::io::copy(&mut (&mut entry).take(MAX_FILE_BYTES + 1), &mut f).map_err(|e| bad(&e.to_string()))?;
        if copied > MAX_FILE_BYTES {
            let _ = fs::remove_dir_all(&staging);
            return Err(bad("a file is far too large"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o755));
        }
        found.push(*save_as);
    }
    let exe = |n: &str| {
        if cfg!(windows) {
            format!("{n}.exe")
        } else {
            n.to_owned()
        }
    };
    if !found.contains(&exe("ffmpeg").as_str()) {
        let _ = fs::remove_dir_all(&staging);
        return Err(bad("FFmpeg itself is missing from it"));
    }
    fs::write(
        staging.join("FFMPEG-SOURCE.txt"),
        format!(
            "ffmpeg and ffplay here are FFmpeg {VERSION} \"essentials\" builds by Gyan Doshi, licensed under the\n\
             GNU GPL version 3 (see FFMPEG-LICENSE.txt). MOTIX runs them as separate programs to show\n\
             video and play sound.\nDownloaded from: {URL}\nSHA-256: {SHA256}\n\
             FFmpeg source: https://github.com/FFmpeg/FFmpeg/releases/tag/n{VERSION}\n\
             Build recipe: https://www.gyan.dev/ffmpeg/builds/\n"
        ),
    )
    .map_err(|e| bad(&e.to_string()))?;
    let dest = folder(base);
    let _ = fs::remove_dir_all(&dest);
    if fs::rename(&staging, &dest).is_err() && !dest.join(exe("ffmpeg")).is_file() {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("couldn't put the helper in {}", dest.display()));
    }
    let _ = fs::remove_dir_all(&staging);
    Tools::find(&[dest.as_path()]).ok_or_else(|| bad("the programs didn't unpack"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use motix_update::UpdateError;
    use motix_update::http::Response;

    struct Fake(Vec<u8>);
    impl Http for Fake {
        fn get(&self, _: &str, _: Option<&str>, _: u64) -> Result<Response, UpdateError> {
            Err(UpdateError::Network("not used".into()))
        }
        fn download(
            &self,
            url: &str,
            _max: u64,
            out: &mut dyn Write,
            progress: &mut dyn FnMut(u64, Option<u64>),
        ) -> Result<(), UpdateError> {
            assert_eq!(url, URL);
            out.write_all(&self.0).unwrap();
            progress(self.0.len() as u64, Some(self.0.len() as u64));
            Ok(())
        }
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("motix-helper-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn zip_with(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in files {
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
        buf.into_inner()
    }

    #[test]
    fn a_tampered_download_is_thrown_away() {
        let base = temp("tampered");
        let fake = Fake(zip_with(&[("x/bin/ffmpeg.exe", b"evil")]));
        let err = download(&fake, &base, &mut |_, _| {}).unwrap_err();
        assert!(err.contains("checksum"));
        assert!(!folder(&base).exists());
        assert_eq!(fs::read_dir(&base).unwrap().count(), 0, "nothing left behind");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn unpacks_only_the_programs_and_licence() {
        let base = temp("unpack");
        fs::create_dir_all(&base).unwrap();
        let exe = |n: &str| {
            if cfg!(windows) {
                format!("{n}.exe")
            } else {
                n.to_owned()
            }
        };
        let archive = base.join("a.zip");
        fs::write(
            &archive,
            zip_with(&[
                (&format!("ffmpeg-9/bin/{}", exe("ffmpeg")), b"ffmpeg program"),
                (&format!("ffmpeg-9/bin/{}", exe("ffplay")), b"ffplay program"),
                ("ffmpeg-9/LICENSE", b"GPL"),
                ("ffmpeg-9/doc/huge.html", b"docs"),
                ("../../escape.exe", b"nope"),
            ]),
        )
        .unwrap();
        let tools = unpack(&archive, &base).unwrap();
        assert_eq!(tools.ffmpeg, folder(&base).join(exe("ffmpeg")));
        assert!(tools.ffplay.is_some());
        assert!(folder(&base).join("FFMPEG-LICENSE.txt").is_file());
        assert!(folder(&base).join("FFMPEG-SOURCE.txt").is_file());
        assert!(!folder(&base).join("huge.html").exists());
        assert!(!base.parent().unwrap().join("escape.exe").exists());
        let empty = base.join("b.zip");
        fs::write(&empty, zip_with(&[("readme.txt", b"hi")])).unwrap();
        assert!(unpack(&empty, &base).unwrap_err().contains("missing"));
        let _ = fs::remove_dir_all(base);
    }
}

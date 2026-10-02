//! Publishing a build from the home build server to a folder (ADR-036).
//!
//! Makes exactly what a GitHub release has — the program archive, `SHA256SUMS` (pinned to
//! the version) and its signature — and puts them in an updates folder that MOTIX checks
//! (the shared folder's `updates`, or any folder PCs are pointed at). Files are written
//! under temporary names and renamed into place, signature last.

use crate::{hex, parse_seed};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const READ_ME: &str = "MOTIX developer preview (built on the home build server)

- Windows: double-click motix.exe. If Windows shows \"Windows protected your PC\",
  click \"More info\" then \"Run anyway\" (developer builds are not code-signed yet).
- Linux: run ./motix
MOTIX updates itself from the folder it was set to use (Help > Check for updates).
MOTIX is free software under the GNU GPL v3 or later (see LICENSE.txt).
";

/// What to publish.
pub(crate) struct Job<'a> {
    pub version: &'a str,
    pub exe: &'a Path,
    pub license: &'a Path,
    pub key_seed: [u8; 32],
    pub to: &'a Path,
    pub notes: String,
    pub trusted: &'a [[u8; 32]],
}

/// `xtask publish <version> <exe> <license> <key-file> <to-folder> [notes-file]`.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let key_text = fs::read_to_string(&args[3]).map_err(|e| format!("couldn't read the key file: {e}"))?;
    let key_seed = parse_seed(&key_text).ok_or("the key file must hold 64 hex digits")?;
    let notes = args.get(5).and_then(|p| fs::read_to_string(p).ok()).unwrap_or_default();
    let archive = publish(&Job {
        version: &args[0],
        exe: Path::new(&args[1]),
        license: Path::new(&args[2]),
        key_seed,
        to: Path::new(&args[4]),
        notes,
        trusted: motix_update::TRUSTED_KEYS,
    })?;
    println!("Published MOTIX {} to {}", args[0], archive.display());
    Ok(())
}

fn zip_release(exe: &Path, license: &Path, out: &Path) -> Result<(), String> {
    let err = |e: &dyn std::fmt::Display| format!("couldn't make the archive: {e}");
    let file = fs::File::create(out).map_err(|e| err(&e))?;
    let mut z = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    let exe_name = exe.file_name().and_then(|n| n.to_str()).ok_or("bad program name")?;
    for (name, data) in [
        (
            exe_name.to_owned(),
            fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?,
        ),
        (
            "LICENSE.txt".to_owned(),
            fs::read(license).map_err(|e| format!("{}: {e}", license.display()))?,
        ),
        ("READ-ME-FIRST.txt".to_owned(), READ_ME.as_bytes().to_vec()),
    ] {
        z.start_file(format!("MOTIX/{name}"), opts).map_err(|e| err(&e))?;
        z.write_all(&data).map_err(|e| err(&e))?;
    }
    z.finish().map_err(|e| err(&e))?;
    Ok(())
}

/// Builds the archive, checksum list and signature, and puts them in `job.to`.
/// Returns the published archive.
pub(crate) fn publish(job: &Job<'_>) -> Result<PathBuf, String> {
    let version = semver::Version::parse(job.version).map_err(|_| format!("\"{}\" isn't a version", job.version))?;
    let key = SigningKey::from_bytes(&job.key_seed);
    if !job.trusted.contains(key.verifying_key().as_bytes()) {
        return Err("that signing key isn't one MOTIX trusts, so installed copies would refuse this build".to_owned());
    }
    fs::create_dir_all(job.to).map_err(|e| format!("couldn't use {}: {e}", job.to.display()))?;
    let archive_name = if job.exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        "motix-windows-x64.zip"
    } else {
        "motix-linux-x64.zip"
    };
    let tmp = |name: &str| job.to.join(format!(".{name}.incoming"));
    zip_release(job.exe, job.license, &tmp(archive_name))?;
    let zip_hash = hex(&Sha256::digest(fs::read(tmp(archive_name)).map_err(|e| e.to_string())?));
    let sums = format!("# motix-version {version}\n{zip_hash}  {archive_name}\n");
    let sig = format!("{}\n", hex(&key.sign(sums.as_bytes()).to_bytes()));
    let files = [
        (archive_name, None),
        ("NOTES.txt", Some(job.notes.clone())),
        ("SHA256SUMS", Some(sums)),
        ("SHA256SUMS.sig", Some(sig)),
    ];
    for (name, text) in &files {
        if let Some(text) = text {
            fs::write(tmp(name), text).map_err(|e| e.to_string())?;
        }
    }
    // Into place, signature last, so a PC checking mid-way sees an older or incomplete
    // (refused) release, never a mismatched one it would install.
    for (name, _) in &files {
        fs::rename(tmp(name), job.to.join(name)).map_err(|e| format!("couldn't publish {name}: {e}"))?;
    }
    Ok(job.to.join(archive_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_folder_is_a_valid_signed_release() {
        let dir = std::env::temp_dir().join(format!("motix-publish-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("motix.exe");
        fs::write(&exe, b"program").unwrap();
        let license = dir.join("LICENSE");
        fs::write(&license, b"GPL").unwrap();
        let seed = [5_u8; 32];
        let public = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        let out = dir.join("updates");
        let job = Job {
            version: "0.2.0-server.202610021530",
            exe: &exe,
            license: &license,
            key_seed: seed,
            to: &out,
            notes: "Scene Library".into(),
            trusted: &[public],
        };
        let archive = publish(&job).unwrap();
        assert_eq!(archive, out.join("motix-windows-x64.zip"));
        let sums = fs::read(out.join("SHA256SUMS")).unwrap();
        let sig = fs::read(out.join("SHA256SUMS.sig")).unwrap();
        motix_update::verify::verify_signature(&sums, &sig, &[public]).unwrap();
        let expected =
            motix_update::verify::expected_hash(&sums, "0.2.0-server.202610021530", "motix-windows-x64.zip").unwrap();
        let actual: [u8; 32] = Sha256::digest(fs::read(&archive).unwrap()).into();
        assert_eq!(expected, actual);
        assert_eq!(fs::read_to_string(out.join("NOTES.txt")).unwrap(), "Scene Library");
        assert!(
            fs::read_dir(&out)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().ends_with(".incoming")),
            "no temporary files left"
        );
        // An untrusted key and a bad version are refused.
        assert!(publish(&Job { trusted: &[], ..job }).unwrap_err().contains("trusts"));
        let _ = fs::remove_dir_all(dir);
    }
}

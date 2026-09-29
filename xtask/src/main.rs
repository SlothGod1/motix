//! MOTIX build and release helpers.
//!
//! ```text
//! cargo run -p xtask -- keygen <secret-key-file>     # new update-signing key pair
//! cargo run -p xtask -- manifest <version> <files…>  # write SHA256SUMS for a release
//! cargo run -p xtask -- sign <file>                  # sign with $MOTIX_UPDATE_SIGNING_KEY → <file>.sig
//! ```
//!
//! The signing key never appears on the command line or in logs: `sign` reads it from
//! the environment (a GitHub Actions secret in CI).

#![forbid(unsafe_code)]

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::process::ExitCode;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn parse_seed(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (i, pair) in text.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

fn keygen(secret_path: &str) -> Result<(), String> {
    let mut seed = [0_u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no secure randomness: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    if std::path::Path::new(secret_path).exists() {
        return Err(format!("{secret_path} already exists; refusing to overwrite a key"));
    }
    std::fs::write(secret_path, format!("{}\n", hex(&seed))).map_err(|e| e.to_string())?;
    println!("Secret key written to {secret_path} (keep it private).");
    println!(
        "Public key (put in motix-update TRUSTED_KEYS): {}",
        hex(key.verifying_key().as_bytes())
    );
    Ok(())
}

fn manifest(version: &str, files: &[String]) -> Result<(), String> {
    let mut out = format!("# motix-version {version}\n");
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("{path}: bad file name"))?;
        let _ = writeln!(out, "{}  {name}", hex(&Sha256::digest(&data)));
    }
    std::fs::write("SHA256SUMS", out).map_err(|e| e.to_string())?;
    println!("Wrote SHA256SUMS for {} file(s).", files.len());
    Ok(())
}

fn sign(path: &str) -> Result<(), String> {
    let secret =
        std::env::var("MOTIX_UPDATE_SIGNING_KEY").map_err(|_| "MOTIX_UPDATE_SIGNING_KEY is not set".to_owned())?;
    let seed = parse_seed(&secret).ok_or("MOTIX_UPDATE_SIGNING_KEY must be 64 hex digits")?;
    let key = SigningKey::from_bytes(&seed);
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let signature = key.sign(&data);
    std::fs::write(format!("{path}.sig"), format!("{}\n", hex(&signature.to_bytes()))).map_err(|e| e.to_string())?;
    println!("Signed {path} with key {}.", hex(key.verifying_key().as_bytes()));
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("keygen") if args.len() == 2 => keygen(&args[1]),
        Some("manifest") if args.len() >= 3 => manifest(&args[1], &args[2..]),
        Some("sign") if args.len() == 2 => sign(&args[1]),
        _ => Err("usage: xtask keygen <secret-file> | manifest <version> <files…> | sign <file>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

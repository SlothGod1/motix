//! Signature and checksum verification.
//!
//! Every release carries two small files next to the downloads:
//!
//! * `SHA256SUMS` — `sha256sum`-style lines (`<hex>  <file name>`) plus a first line
//!   `# motix-version <version>` that pins the version being released, so an old
//!   (validly signed) release can't be passed off as a newer one;
//! * `SHA256SUMS.sig` — the Ed25519 signature of `SHA256SUMS`, as 128 hex digits.
//!
//! The signature must verify against one of the public keys built into MOTIX
//! ([`crate::TRUSTED_KEYS`]); only then are the hashes trusted.

use crate::UpdateError;
use ed25519_dalek::{Signature, VerifyingKey};

/// Decodes lowercase or uppercase hex.
pub(crate) fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != N * 2 {
        return None;
    }
    let mut out = [0_u8; N];
    for (i, chunk) in text.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

/// Encodes bytes as lowercase hex.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Checks `sig_file` is a valid signature of `sums` by one of `keys`.
///
/// # Errors
/// [`UpdateError::Unsigned`] when no key is configured, [`UpdateError::BadSignature`]
/// when the signature is malformed or doesn't match.
pub fn verify_signature(sums: &[u8], sig_file: &[u8], keys: &[[u8; 32]]) -> Result<(), UpdateError> {
    if keys.is_empty() {
        return Err(UpdateError::Unsigned);
    }
    let text = std::str::from_utf8(sig_file).map_err(|_| UpdateError::BadSignature)?;
    let bytes: [u8; 64] = from_hex(text).ok_or(UpdateError::BadSignature)?;
    let signature = Signature::from_bytes(&bytes);
    for key in keys {
        if let Ok(vk) = VerifyingKey::from_bytes(key)
            && vk.verify_strict(sums, &signature).is_ok()
        {
            return Ok(());
        }
    }
    Err(UpdateError::BadSignature)
}

/// Reads the signed manifest: checks it's for `version` and returns the SHA-256 of `file`.
///
/// # Errors
/// [`UpdateError::BadManifest`] if the version line is missing or different, or the
/// file isn't listed.
pub fn expected_hash(sums: &[u8], version: &str, file: &str) -> Result<[u8; 32], UpdateError> {
    let text = std::str::from_utf8(sums).map_err(|_| UpdateError::BadManifest("not text"))?;
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default().trim();
    if first.strip_prefix("# motix-version ").map(str::trim) != Some(version) {
        return Err(UpdateError::BadManifest("it is for a different version"));
    }
    for line in lines {
        let line = line.trim();
        let Some((hash, name)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let name = name.trim().trim_start_matches('*');
        if name == file {
            return from_hex(hash).ok_or(UpdateError::BadManifest("a checksum is malformed"));
        }
    }
    Err(UpdateError::BadManifest("the download isn't listed"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    pub(crate) fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    pub(crate) fn sign(data: &[u8]) -> Vec<u8> {
        to_hex(&test_key().sign(data).to_bytes()).into_bytes()
    }

    #[test]
    fn hex_round_trip() {
        let b = [0_u8, 1, 0xAB, 0xFF];
        assert_eq!(to_hex(&b), "0001abff");
        assert_eq!(from_hex::<4>("0001ABff"), Some(b));
        assert_eq!(from_hex::<4>("0001ab"), None);
        assert_eq!(from_hex::<2>("zz00"), None);
    }

    #[test]
    fn signatures() {
        let key = test_key().verifying_key().to_bytes();
        let sums = b"# motix-version 1.0.0\nabc  x.zip\n";
        let sig = sign(sums);
        assert_eq!(verify_signature(sums, &sig, &[key]), Ok(()));
        assert_eq!(
            verify_signature(b"tampered", &sig, &[key]),
            Err(UpdateError::BadSignature)
        );
        assert_eq!(
            verify_signature(sums, b"nonsense", &[key]),
            Err(UpdateError::BadSignature)
        );
        assert_eq!(verify_signature(sums, &sig, &[]), Err(UpdateError::Unsigned));
        let other = SigningKey::from_bytes(&[9_u8; 32]).verifying_key().to_bytes();
        assert_eq!(verify_signature(sums, &sig, &[other]), Err(UpdateError::BadSignature));
        assert_eq!(verify_signature(sums, &sig, &[other, key]), Ok(()), "any trusted key");
    }

    #[test]
    fn manifests() {
        let h = "a".repeat(64);
        let sums = format!("# motix-version 0.2.0-preview.3\n{h}  motix-windows-x64.zip\n{h} *other.zip\n");
        assert_eq!(
            expected_hash(sums.as_bytes(), "0.2.0-preview.3", "motix-windows-x64.zip"),
            Ok([0xAA; 32])
        );
        assert_eq!(
            expected_hash(sums.as_bytes(), "0.2.0-preview.3", "other.zip"),
            Ok([0xAA; 32])
        );
        assert!(
            expected_hash(sums.as_bytes(), "0.2.0-preview.4", "other.zip").is_err(),
            "version pinned"
        );
        assert!(expected_hash(sums.as_bytes(), "0.2.0-preview.3", "missing.zip").is_err());
    }
}

//! The owner password that opens the Creator Lab (ADR-033).
//!
//! MOTIX's code is public on GitHub, so the password itself is never stored there —
//! not even scrambled in a way that could be reversed. What's built into MOTIX is an
//! [`OwnerCheck`]: a random salt and the SHA-256 of a key that **Argon2id** (a
//! deliberately slow, memory-hungry password hash) derives from the password. Checking
//! a guess means redoing that slow derivation, so trying millions of guesses is
//! impractically slow — as long as the password is long (at least
//! [`MIN_PASSWORD_CHARS`] characters; a few unrelated words is ideal).
//!
//! After the right password is typed once, the derived key is remembered for that
//! Windows user on that PC, so the Lab opens without retyping.
//!
//! Honest limit: because MOTIX is open source, someone could build their own copy
//! with the lock removed. The lock keeps the Lab closed in the official MOTIX; it
//! doesn't hide the code.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Shortest owner password accepted.
pub const MIN_PASSWORD_CHARS: usize = 12;

/// Argon2id settings used for new passwords: 64 MiB of memory, 3 passes.
pub const DEFAULT_MEMORY_KIB: u32 = 64 * 1024;
/// Argon2id passes.
pub const DEFAULT_PASSES: u32 = 3;

/// What MOTIX keeps to recognise the owner password (safe to publish).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerCheck {
    /// Random salt.
    pub salt: [u8; 16],
    /// Argon2id memory, KiB.
    pub memory_kib: u32,
    /// Argon2id passes.
    pub passes: u32,
    /// SHA-256 of the 32-byte key Argon2id derives from the password.
    pub key_hash: [u8; 32],
}

/// The owner check built into this MOTIX. `None` until the owner has created a
/// password with "Set up the owner password" and sent Claude the resulting file;
/// until then the Lab stays locked for everyone.
pub const BUILT_IN: Option<OwnerCheck> = None;

/// Why a password couldn't be set or checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerError {
    /// Too short.
    TooShort,
    /// The two entries differ.
    Mismatch,
    /// Wrong password.
    Wrong,
    /// The Lab's password hasn't been built into MOTIX yet.
    NotSetUp,
    /// The setup file is unreadable.
    BadFile,
    /// The computer couldn't produce random numbers or memory for the hash.
    Internal(String),
}

impl std::fmt::Display for OwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(
                f,
                "Use at least {MIN_PASSWORD_CHARS} characters — a few unrelated words works well."
            ),
            Self::Mismatch => f.write_str("The two passwords don't match."),
            Self::Wrong => f.write_str("That's not the owner password."),
            Self::NotSetUp => f.write_str("The owner password isn't set up in this version of MOTIX yet."),
            Self::BadFile => f.write_str("That isn't an owner-password file made by MOTIX."),
            Self::Internal(why) => write!(f, "Something went wrong: {why}."),
        }
    }
}

impl std::error::Error for OwnerError {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != N * 2 || !text.is_ascii() {
        return None;
    }
    let mut out = [0_u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The key Argon2id derives from `password` with `check`'s settings.
///
/// # Errors
/// [`OwnerError::Internal`] if the settings are unusable.
pub fn derive(password: &str, check: &OwnerCheck) -> Result<[u8; 32], OwnerError> {
    let params = argon2::Params::new(check.memory_kib, check.passes, 1, Some(32))
        .map_err(|e| OwnerError::Internal(e.to_string()))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = [0_u8; 32];
    argon
        .hash_password_into(password.as_bytes(), &check.salt, &mut key)
        .map_err(|e| OwnerError::Internal(e.to_string()))?;
    Ok(key)
}

fn key_matches(key: &[u8; 32], check: &OwnerCheck) -> bool {
    let digest: [u8; 32] = Sha256::digest(key).into();
    // Constant-time comparison.
    digest
        .iter()
        .zip(check.key_hash)
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Checks a typed password. Returns the key to remember on this PC.
///
/// # Errors
/// [`OwnerError::Wrong`], or [`OwnerError::NotSetUp`] when `check` is `None`.
pub fn unlock(password: &str, check: Option<&OwnerCheck>) -> Result<[u8; 32], OwnerError> {
    let check = check.ok_or(OwnerError::NotSetUp)?;
    let key = derive(password, check)?;
    if key_matches(&key, check) {
        Ok(key)
    } else {
        Err(OwnerError::Wrong)
    }
}

/// Text to store on this PC so the Lab opens without retyping.
#[must_use]
pub fn remembered_text(key: &[u8; 32]) -> String {
    format!("{}\n", hex(key))
}

/// `true` if a remembered key (from [`remembered_text`]) still matches `check`.
#[must_use]
pub fn remembered_ok(text: &str, check: Option<&OwnerCheck>) -> bool {
    match (unhex::<32>(text), check) {
        (Some(key), Some(check)) => key_matches(&key, check),
        _ => false,
    }
}

/// Creates the check for a new owner password (typed twice).
///
/// # Errors
/// [`OwnerError::TooShort`], [`OwnerError::Mismatch`], or [`OwnerError::Internal`].
pub fn create(password: &str, again: &str) -> Result<OwnerCheck, OwnerError> {
    create_with(password, again, DEFAULT_MEMORY_KIB, DEFAULT_PASSES)
}

/// Like [`create`] with chosen Argon2id settings (smaller ones only make sense in tests).
///
/// # Errors
/// As [`create`].
pub fn create_with(password: &str, again: &str, memory_kib: u32, passes: u32) -> Result<OwnerCheck, OwnerError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(OwnerError::TooShort);
    }
    if password != again {
        return Err(OwnerError::Mismatch);
    }
    let mut salt = [0_u8; 16];
    getrandom::fill(&mut salt).map_err(|e| OwnerError::Internal(e.to_string()))?;
    let mut check = OwnerCheck {
        salt,
        memory_kib,
        passes,
        key_hash: [0; 32],
    };
    let key = derive(password, &check)?;
    check.key_hash = Sha256::digest(key).into();
    Ok(check)
}

/// The setup file the owner sends to Claude (contains no password).
#[must_use]
pub fn setup_file_text(check: &OwnerCheck) -> String {
    format!(
        "MOTIX owner password check (safe to share: it does not contain the password)\n\
         argon2id-v19\nsalt {}\nmemory-kib {}\npasses {}\nkey-sha256 {}\n",
        hex(&check.salt),
        check.memory_kib,
        check.passes,
        hex(&check.key_hash)
    )
}

/// Reads a setup file made by [`setup_file_text`].
///
/// # Errors
/// [`OwnerError::BadFile`].
pub fn parse_setup_file(text: &str) -> Result<OwnerCheck, OwnerError> {
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(name).map(str::trim))
            .ok_or(OwnerError::BadFile)
    };
    if !text.lines().any(|l| l.trim() == "argon2id-v19") {
        return Err(OwnerError::BadFile);
    }
    let memory_kib: u32 = field("memory-kib ")?.parse().map_err(|_| OwnerError::BadFile)?;
    let passes: u32 = field("passes ")?.parse().map_err(|_| OwnerError::BadFile)?;
    if !(8 * 1024..=4 * 1024 * 1024).contains(&memory_kib) || !(1..=64).contains(&passes) {
        return Err(OwnerError::BadFile);
    }
    Ok(OwnerCheck {
        salt: unhex(field("salt ")?).ok_or(OwnerError::BadFile)?,
        memory_kib,
        passes,
        key_hash: unhex(field("key-sha256 ")?).ok_or(OwnerError::BadFile)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Small settings keep the tests fast; real passwords use the defaults.
    fn quick(pw: &str) -> OwnerCheck {
        create_with(pw, pw, 8 * 1024, 1).unwrap()
    }

    #[test]
    fn right_password_unlocks_and_is_remembered() {
        let check = quick("purple ladder sunset river");
        let key = unlock("purple ladder sunset river", Some(&check)).unwrap();
        let remembered = remembered_text(&key);
        assert!(remembered_ok(&remembered, Some(&check)));
        assert!(!remembered_ok("00", Some(&check)));
        assert!(!remembered_ok(&remembered, None), "no password built in: stays locked");
        assert_eq!(
            unlock("purple ladder sunset rivers", Some(&check)),
            Err(OwnerError::Wrong)
        );
        assert_eq!(unlock("anything", None), Err(OwnerError::NotSetUp));
    }

    #[test]
    fn new_passwords_are_checked_and_salted() {
        assert_eq!(create("short", "short"), Err(OwnerError::TooShort));
        assert_eq!(
            create_with("long enough words", "long enough wordz", 8 * 1024, 1),
            Err(OwnerError::Mismatch)
        );
        let a = quick("same password here");
        let b = quick("same password here");
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.key_hash, b.key_hash, "same password, different salts");
    }

    #[test]
    fn setup_file_round_trips_without_the_password() {
        let check = quick("correct horse battery staple");
        let text = setup_file_text(&check);
        assert!(!text.contains("horse"));
        assert_eq!(parse_setup_file(&text), Ok(check));
        assert_eq!(parse_setup_file("hello"), Err(OwnerError::BadFile));
        let weakened = text.replace("memory-kib 8192", "memory-kib 1");
        assert_eq!(parse_setup_file(&weakened), Err(OwnerError::BadFile));
    }
}

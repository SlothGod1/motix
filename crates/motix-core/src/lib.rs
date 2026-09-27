//! Core value types shared by every MOTIX crate.
//!
//! This crate sits at the bottom of the dependency graph and is linked into the
//! client, the server and the worker processes. It has **no dependencies** and
//! **no `unsafe` code**, and every constructor that accepts outside input
//! validates it.
//!
//! * [`Time`] / [`TimeRange`] — exact timeline time in integer flicks
//!   (705,600,000 per second). No floating-point time anywhere in the model.
//! * [`Rational`] — exact fractions (frame rates, aspect ratios, speeds).
//! * [`FrameRate`] / [`SampleRate`] — validated rates with exact, round-trip-safe
//!   conversion between frame/sample indices and time.
//! * [`limits`] — hard caps on untrusted values.
//!
//! See `ARCHITECTURE.md` §5.1 and `DECISIONS.md` ADR-006.

#![forbid(unsafe_code)]

mod error;
pub mod limits;
mod rate;
mod rational;
mod time;

pub use error::CoreError;
pub use rate::{FrameRate, SampleRate};
pub use rational::Rational;
pub use time::{FLICKS_PER_SECOND, Rounding, Time, TimeRange};

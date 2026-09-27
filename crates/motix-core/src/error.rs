//! Error type for `motix-core`.

use std::fmt;

/// Errors produced by core value types.
///
/// Every constructor that accepts outside input (project files, network peers,
/// media metadata) validates it and returns one of these instead of panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// A rational number was given a zero denominator.
    ZeroDenominator,
    /// An arithmetic result does not fit in the target type.
    Overflow,
    /// A value was outside its allowed range.
    OutOfRange {
        /// What was being validated, e.g. `"frame rate"`.
        what: &'static str,
        /// Human-readable description of the allowed range.
        allowed: &'static str,
    },
    /// A time range whose end is before its start.
    InvertedRange {
        /// Start, in flicks.
        start: i64,
        /// End, in flicks.
        end: i64,
    },
    /// Text could not be parsed.
    Parse {
        /// What was being parsed.
        what: &'static str,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => f.write_str("denominator must not be zero"),
            Self::Overflow => f.write_str("value is too large"),
            Self::OutOfRange { what, allowed } => {
                write!(f, "{what} is out of range (allowed: {allowed})")
            }
            Self::InvertedRange { start, end } => {
                write!(f, "time range ends ({end}) before it starts ({start})")
            }
            Self::Parse { what } => write!(f, "could not parse {what}"),
        }
    }
}

impl std::error::Error for CoreError {}

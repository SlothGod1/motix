//! Exact timeline time.
//!
//! All positions and durations in a project are integer **flicks**:
//! 1 second = 705,600,000 flicks. That number divides evenly into every common
//! video frame duration (23.976, 24, 25, 29.97, 30, 48, 50, 59.94, 60, 119.88,
//! 120, 240 fps) and every common audio sample period (8 kHz … 192 kHz), so
//! frame and sample boundaries are exact integers and nothing drifts.
//! `i64` flicks cover roughly ±414 years.

use crate::{CoreError, Rational};
use std::fmt;

/// Flicks per second.
pub const FLICKS_PER_SECOND: i64 = 705_600_000;

/// How to round when a value falls between two representable values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// Toward negative infinity.
    Floor,
    /// Toward positive infinity.
    Ceil,
    /// To the nearest value; exact halves round toward positive infinity.
    Nearest,
}

/// Divides `a` by a **positive** `b` with the given rounding.
pub(crate) fn div_round(a: i128, b: i128, rounding: Rounding) -> i128 {
    debug_assert!(b > 0, "divisor must be positive");
    match rounding {
        Rounding::Floor => a.div_euclid(b),
        Rounding::Ceil => -((-a).div_euclid(b)),
        // floor((2a + b) / 2b) == floor(a/b + 1/2)
        Rounding::Nearest => (2 * a + b).div_euclid(2 * b),
    }
}

/// A point on, or a length of, the timeline, in flicks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(i64);

impl Time {
    /// Zero.
    pub const ZERO: Self = Self(0);
    /// Largest representable time.
    pub const MAX: Self = Self(i64::MAX);
    /// Smallest representable time.
    pub const MIN: Self = Self(i64::MIN);
    /// Exactly one second.
    pub const SECOND: Self = Self(FLICKS_PER_SECOND);

    /// From a raw flick count.
    #[must_use]
    pub const fn from_flicks(flicks: i64) -> Self {
        Self(flicks)
    }

    /// The raw flick count.
    #[must_use]
    pub const fn flicks(self) -> i64 {
        self.0
    }

    /// From whole seconds.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if out of range.
    pub fn from_seconds(seconds: i64) -> Result<Self, CoreError> {
        seconds
            .checked_mul(FLICKS_PER_SECOND)
            .map(Self)
            .ok_or(CoreError::Overflow)
    }

    /// From an exact number of seconds, rounding to a whole flick if necessary.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if out of range.
    pub fn from_seconds_rational(seconds: Rational, rounding: Rounding) -> Result<Self, CoreError> {
        let flicks = div_round(
            i128::from(seconds.num()) * i128::from(FLICKS_PER_SECOND),
            i128::from(seconds.den()),
            rounding,
        );
        i64::try_from(flicks).map(Self).map_err(|_| CoreError::Overflow)
    }

    /// Exact value in seconds.
    #[must_use]
    pub fn to_seconds_rational(self) -> Rational {
        // Cannot fail: the denominator is a positive constant and the reduced
        // numerator's magnitude is at most |self.0|.
        Rational::new(self.0, FLICKS_PER_SECOND).unwrap_or(Rational::ZERO)
    }

    /// Approximate seconds, for display only.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_seconds_f64(self) -> f64 {
        self.0 as f64 / FLICKS_PER_SECOND as f64
    }

    /// `self + rhs`, or `None` on overflow.
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// `self - rhs`, or `None` on overflow.
    #[must_use]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// `self * n`, or `None` on overflow.
    #[must_use]
    pub const fn checked_mul(self, n: i64) -> Option<Self> {
        match self.0.checked_mul(n) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// `self + rhs`, clamped to the representable range.
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// `self - rhs`, clamped to the representable range.
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// `true` if before zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }
}

impl fmt::Display for Time {
    /// Formats as `[-]H:MM:SS.mmm` (milliseconds truncated toward zero).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let total_ms = abs / (FLICKS_PER_SECOND.unsigned_abs() / 1000);
        let ms = total_ms % 1000;
        let s = (total_ms / 1000) % 60;
        let m = (total_ms / 60_000) % 60;
        let h = total_ms / 3_600_000;
        write!(f, "{sign}{h}:{m:02}:{s:02}.{ms:03}")
    }
}

/// A half-open span of time `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeRange {
    start: Time,
    end: Time,
}

impl TimeRange {
    /// Creates a range.
    ///
    /// # Errors
    /// [`CoreError::InvertedRange`] if `end < start`.
    pub const fn new(start: Time, end: Time) -> Result<Self, CoreError> {
        if end.0 < start.0 {
            return Err(CoreError::InvertedRange {
                start: start.0,
                end: end.0,
            });
        }
        Ok(Self { start, end })
    }

    /// Creates a range from a start and a non-negative duration.
    ///
    /// # Errors
    /// [`CoreError::InvertedRange`] for a negative duration; [`CoreError::Overflow`]
    /// if the end is not representable.
    pub fn from_start_duration(start: Time, duration: Time) -> Result<Self, CoreError> {
        let end = start.checked_add(duration).ok_or(CoreError::Overflow)?;
        Self::new(start, end)
    }

    /// Inclusive start.
    #[must_use]
    pub const fn start(self) -> Time {
        self.start
    }

    /// Exclusive end.
    #[must_use]
    pub const fn end(self) -> Time {
        self.end
    }

    /// Length of the range. Saturates at [`Time::MAX`] for ranges wider than `i64`.
    #[must_use]
    pub const fn duration(self) -> Time {
        self.end.saturating_sub(self.start)
    }

    /// `true` if the range contains no time.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }

    /// `true` if `t` is inside `[start, end)`.
    #[must_use]
    pub const fn contains(self, t: Time) -> bool {
        self.start.0 <= t.0 && t.0 < self.end.0
    }

    /// `true` if the two ranges share any time.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.start.0 < other.end.0 && other.start.0 < self.end.0
    }

    /// The shared part of two ranges, if any.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (start < end).then_some(Self { start, end })
    }

    /// The range moved by `offset`.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if either end is not representable.
    pub fn shifted(self, offset: Time) -> Result<Self, CoreError> {
        Ok(Self {
            start: self.start.checked_add(offset).ok_or(CoreError::Overflow)?,
            end: self.end.checked_add(offset).ok_or(CoreError::Overflow)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div_round_modes() {
        assert_eq!(div_round(7, 2, Rounding::Floor), 3);
        assert_eq!(div_round(7, 2, Rounding::Ceil), 4);
        assert_eq!(div_round(7, 2, Rounding::Nearest), 4);
        assert_eq!(div_round(-7, 2, Rounding::Floor), -4);
        assert_eq!(div_round(-7, 2, Rounding::Ceil), -3);
        assert_eq!(div_round(-7, 2, Rounding::Nearest), -3); // -3.5 rounds toward +inf
        assert_eq!(div_round(5, 3, Rounding::Nearest), 2);
        assert_eq!(div_round(4, 3, Rounding::Nearest), 1);
        assert_eq!(div_round(6, 3, Rounding::Nearest), 2);
    }

    #[test]
    fn seconds_conversions() {
        assert_eq!(Time::from_seconds(2).unwrap().flicks(), 1_411_200_000);
        assert!(Time::from_seconds(i64::MAX / 2).is_err());
        let third = Rational::new_const(1, 3);
        assert_eq!(
            Time::from_seconds_rational(third, Rounding::Floor).unwrap().flicks(),
            235_200_000
        );
        let tiny = Rational::new_const(1, 1_000_000_000_000);
        assert_eq!(Time::from_seconds_rational(tiny, Rounding::Floor).unwrap(), Time::ZERO);
        assert_eq!(Time::from_seconds_rational(tiny, Rounding::Ceil).unwrap().flicks(), 1);
        assert_eq!(Time::SECOND.to_seconds_rational(), Rational::ONE);
        assert_eq!(
            Time::from_flicks(352_800_000).to_seconds_rational(),
            Rational::new_const(1, 2)
        );
    }

    #[test]
    fn checked_arithmetic() {
        assert_eq!(Time::MAX.checked_add(Time::from_flicks(1)), None);
        assert_eq!(Time::MIN.checked_sub(Time::from_flicks(1)), None);
        assert_eq!(Time::MAX.saturating_add(Time::SECOND), Time::MAX);
        assert_eq!(
            Time::SECOND.checked_mul(3),
            Some(Time::from_flicks(3 * FLICKS_PER_SECOND))
        );
        assert_eq!(Time::MAX.checked_mul(2), None);
    }

    #[test]
    fn display() {
        assert_eq!(Time::ZERO.to_string(), "0:00:00.000");
        assert_eq!(Time::from_seconds(3_723).unwrap().to_string(), "1:02:03.000");
        assert_eq!(Time::from_flicks(-FLICKS_PER_SECOND / 2).to_string(), "-0:00:00.500");
        assert_eq!(Time::MIN.to_string().chars().next(), Some('-'));
    }

    #[test]
    fn ranges() {
        let sec = Time::SECOND;
        let r = TimeRange::from_start_duration(sec, sec.checked_mul(2).unwrap()).unwrap();
        assert_eq!(r.end(), sec.checked_mul(3).unwrap());
        assert!(r.contains(sec));
        assert!(!r.contains(r.end()));
        assert!(TimeRange::new(sec, Time::ZERO).is_err());
        assert!(TimeRange::from_start_duration(sec, Time::from_flicks(-1)).is_err());
        assert!(TimeRange::from_start_duration(Time::MAX, sec).is_err());

        let a = TimeRange::new(Time::ZERO, sec).unwrap();
        let b = TimeRange::new(sec, sec.checked_mul(2).unwrap()).unwrap();
        assert!(!a.overlaps(b), "touching ranges do not overlap");
        assert_eq!(a.intersection(b), None);
        let c = TimeRange::new(Time::from_flicks(10), Time::from_flicks(FLICKS_PER_SECOND + 10)).unwrap();
        assert!(a.overlaps(c) && c.overlaps(b));
        assert_eq!(
            a.intersection(c),
            Some(TimeRange::new(Time::from_flicks(10), sec).unwrap())
        );
        let empty = TimeRange::new(sec, sec).unwrap();
        assert!(empty.is_empty() && !empty.contains(sec) && !empty.overlaps(a));
        assert_eq!(a.shifted(sec).unwrap(), b);
        assert!(a.shifted(Time::MAX).is_err());
        assert_eq!(TimeRange::new(Time::MIN, Time::MAX).unwrap().duration(), Time::MAX);
    }
}

//! Frame rates and sample rates, and exact conversion between indices and time.
//!
//! # Conversion rule
//!
//! For a rate of `num/den` events per second, event `k` happens exactly at
//! `k · den/num` seconds. When that instant is not a whole flick, we use the
//! **first flick at or after it** (`ceil`). Converting a time back to an index
//! uses `floor`. Together these guarantee, for every rate up to 705,600,000 Hz:
//!
//! * `time_to_index(index_to_time(k)) == k` for every `k`, and
//! * every time belongs to exactly one index: frame `k` covers
//!   `[index_to_time(k), index_to_time(k + 1))`.
//!
//! For all common rates the instants are whole flicks anyway, so no rounding
//! ever happens ([`FrameRate::is_exact`]).

use crate::limits::{MAX_FRAME_RATE_FPS, MAX_RATE_DENOMINATOR, MAX_SAMPLE_RATE_HZ};
use crate::time::{FLICKS_PER_SECOND, Rounding, div_round};
use crate::{CoreError, Rational, Time, TimeRange};
use std::fmt;

/// Time of event `index` for a rate of `num/den` per second (`num, den > 0`).
fn index_to_time(index: i64, num: i64, den: i64) -> Result<Time, CoreError> {
    let flicks = div_round(
        i128::from(index) * i128::from(den) * i128::from(FLICKS_PER_SECOND),
        i128::from(num),
        Rounding::Ceil,
    );
    i64::try_from(flicks)
        .map(Time::from_flicks)
        .map_err(|_| CoreError::Overflow)
}

/// Index of the event whose span contains `t`, for a rate of `num/den` per second.
fn time_to_index(t: Time, num: i64, den: i64) -> i64 {
    let index = div_round(
        i128::from(t.flicks()) * i128::from(num),
        i128::from(den) * i128::from(FLICKS_PER_SECOND),
        Rounding::Floor,
    );
    // |result| <= |t| because num/den <= FLICKS_PER_SECOND (enforced by the
    // validated constructors), so this conversion cannot fail.
    i64::try_from(index).unwrap_or(if index < 0 { i64::MIN } else { i64::MAX })
}

fn is_exact(num: i64, den: i64) -> bool {
    (i128::from(den) * i128::from(FLICKS_PER_SECOND)) % i128::from(num) == 0
}

/// A video frame rate: a positive exact fraction of frames per second.
///
/// Validated on construction: `0 < fps <= 1000` and denominator `<= 1,000,000`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameRate(Rational);

impl FrameRate {
    /// 23.976 fps (24000/1001) — film transferred to NTSC.
    pub const FPS_23_976: Self = Self(Rational::new_const(24_000, 1_001));
    /// 24 fps — cinema.
    pub const FPS_24: Self = Self(Rational::new_const(24, 1));
    /// 25 fps — PAL.
    pub const FPS_25: Self = Self(Rational::new_const(25, 1));
    /// 29.97 fps (30000/1001) — NTSC; common on phones.
    pub const FPS_29_97: Self = Self(Rational::new_const(30_000, 1_001));
    /// 30 fps.
    pub const FPS_30: Self = Self(Rational::new_const(30, 1));
    /// 47.952 fps (48000/1001).
    pub const FPS_47_952: Self = Self(Rational::new_const(48_000, 1_001));
    /// 48 fps.
    pub const FPS_48: Self = Self(Rational::new_const(48, 1));
    /// 50 fps.
    pub const FPS_50: Self = Self(Rational::new_const(50, 1));
    /// 59.94 fps (60000/1001).
    pub const FPS_59_94: Self = Self(Rational::new_const(60_000, 1_001));
    /// 60 fps.
    pub const FPS_60: Self = Self(Rational::new_const(60, 1));
    /// 90 fps.
    pub const FPS_90: Self = Self(Rational::new_const(90, 1));
    /// 100 fps.
    pub const FPS_100: Self = Self(Rational::new_const(100, 1));
    /// 119.88 fps (120000/1001).
    pub const FPS_119_88: Self = Self(Rational::new_const(120_000, 1_001));
    /// 120 fps — phone slow motion.
    pub const FPS_120: Self = Self(Rational::new_const(120, 1));
    /// 240 fps — phone slow motion.
    pub const FPS_240: Self = Self(Rational::new_const(240, 1));

    /// The frame rates offered in presets. All are exact in flicks.
    pub const COMMON: [Self; 15] = [
        Self::FPS_23_976,
        Self::FPS_24,
        Self::FPS_25,
        Self::FPS_29_97,
        Self::FPS_30,
        Self::FPS_47_952,
        Self::FPS_48,
        Self::FPS_50,
        Self::FPS_59_94,
        Self::FPS_60,
        Self::FPS_90,
        Self::FPS_100,
        Self::FPS_119_88,
        Self::FPS_120,
        Self::FPS_240,
    ];

    /// Validates a frame rate.
    ///
    /// # Errors
    /// [`CoreError::OutOfRange`] unless `0 < fps <= 1000` with denominator `<= 1,000,000`.
    pub fn new(fps: Rational) -> Result<Self, CoreError> {
        let out_of_range = CoreError::OutOfRange {
            what: "frame rate",
            allowed: "greater than 0 and at most 1000 fps, denominator at most 1000000",
        };
        if !fps.is_positive() || fps.den() > MAX_RATE_DENOMINATOR || fps > Rational::from_integer(MAX_FRAME_RATE_FPS) {
            return Err(out_of_range);
        }
        Ok(Self(fps))
    }

    /// Validates a frame rate given as `num/den`.
    ///
    /// # Errors
    /// As [`FrameRate::new`], plus [`CoreError::ZeroDenominator`].
    pub fn from_fraction(num: i64, den: i64) -> Result<Self, CoreError> {
        Self::new(Rational::new(num, den)?)
    }

    /// The exact rate in frames per second.
    #[must_use]
    pub const fn as_rational(self) -> Rational {
        self.0
    }

    /// Approximate frames per second, for display.
    #[must_use]
    pub fn fps_f64(self) -> f64 {
        self.0.to_f64()
    }

    /// Start time of frame `frame` (see the module docs for the rounding rule).
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if the time is not representable.
    pub fn frame_to_time(self, frame: i64) -> Result<Time, CoreError> {
        index_to_time(frame, self.0.num(), self.0.den())
    }

    /// The frame being shown at time `t`.
    #[must_use]
    pub fn time_to_frame(self, t: Time) -> i64 {
        time_to_index(t, self.0.num(), self.0.den())
    }

    /// The span of time during which `frame` is shown.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if either end is not representable.
    pub fn frame_range(self, frame: i64) -> Result<TimeRange, CoreError> {
        let next = frame.checked_add(1).ok_or(CoreError::Overflow)?;
        TimeRange::new(self.frame_to_time(frame)?, self.frame_to_time(next)?)
    }

    /// `true` if every frame boundary is a whole number of flicks.
    #[must_use]
    pub fn is_exact(self) -> bool {
        is_exact(self.0.num(), self.0.den())
    }

    /// Duration of one frame, when it is a whole number of flicks.
    #[must_use]
    pub fn frame_duration(self) -> Option<Time> {
        self.is_exact().then(|| {
            // Exact, so the quotient is an integer below FLICKS_PER_SECOND.
            let d = i128::from(self.0.den()) * i128::from(FLICKS_PER_SECOND) / i128::from(self.0.num());
            Time::from_flicks(i64::try_from(d).unwrap_or(i64::MAX))
        })
    }

    /// Parses a frame rate the way a person types it.
    ///
    /// Accepts whole numbers (`"30"`), decimals (`"12.5"`, `"29.97"`), fractions
    /// (`"30000/1001"`) and an optional `fps` suffix. Decimals that are the usual
    /// shorthand for the NTSC family (`23.976`, `23.98`, `29.97`, `47.952`, `59.94`,
    /// `119.88`, `239.76`) become the exact `N×1000/1001` rate that cameras and phones
    /// actually record, so "29.97" matches phone footage frame-for-frame.
    ///
    /// # Errors
    /// [`CoreError::Parse`] for text that is not a number, and the errors of
    /// [`FrameRate::new`] for rates outside the allowed range.
    pub fn parse_user(text: &str) -> Result<Self, CoreError> {
        let parse_err = CoreError::Parse { what: "frame rate" };
        let t = text.trim();
        let t = t
            .strip_suffix("fps")
            .or_else(|| t.strip_suffix("FPS"))
            .unwrap_or(t)
            .trim();
        if t.is_empty() || t.len() > 32 {
            return Err(parse_err);
        }
        if t.contains('/') {
            return Self::new(t.parse::<Rational>().map_err(|_| parse_err)?);
        }
        let (whole, frac) = t.split_once(['.', ',']).unwrap_or((t, ""));
        let digits_ok = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if (whole.is_empty() && frac.is_empty()) || !digits_ok(whole) || !digits_ok(frac) || frac.len() > 6 {
            return Err(parse_err);
        }
        let scale = 10_i64.pow(u32::try_from(frac.len()).map_err(|_| parse_err.clone())?);
        let whole_v: i64 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| parse_err.clone())?
        };
        let frac_v: i64 = if frac.is_empty() {
            0
        } else {
            frac.parse().map_err(|_| parse_err.clone())?
        };
        let typed = Rational::from_i128(
            i128::from(whole_v) * i128::from(scale) + i128::from(frac_v),
            i128::from(scale),
        )?;
        if frac.len() >= 2 {
            for base in [24_i64, 30, 48, 60, 120, 240] {
                let ntsc = Rational::new_const(base * 1000, 1001);
                // The typed decimal is the NTSC rate rounded to the digits typed.
                let diff = typed.checked_sub(ntsc)?;
                let scaled = diff.checked_mul(Rational::from_integer(2 * scale))?;
                if scaled.num().unsigned_abs() < scaled.den().unsigned_abs() {
                    return Self::new(ntsc);
                }
            }
        }
        Self::new(typed)
    }

    /// Interprets a rate measured from a media file (`num/den` frames per second).
    ///
    /// Containers often store slightly rounded timing (for example 33,366,667 ns per
    /// frame for 29.97 fps). A measurement within 0.05 % of a [common](Self::COMMON)
    /// rate is taken to be that rate; anything else is kept as measured (with the
    /// denominator reduced to at most 1000 if needed).
    ///
    /// # Errors
    /// The errors of [`FrameRate::new`] for rates outside the allowed range.
    pub fn from_measured(num: i64, den: i64) -> Result<Self, CoreError> {
        let measured = Rational::new(num, den)?;
        if !measured.is_positive() {
            return Err(CoreError::OutOfRange {
                what: "frame rate",
                allowed: "greater than 0",
            });
        }
        let m = measured.to_f64();
        for rate in Self::COMMON {
            let s = rate.fps_f64();
            if ((m - s) / s).abs() < 0.0005 {
                return Ok(rate);
            }
        }
        if measured.den() <= MAX_RATE_DENOMINATOR {
            return Self::new(measured);
        }
        let thousandths = div_round(
            i128::from(measured.num()) * 1000,
            i128::from(measured.den()),
            Rounding::Nearest,
        );
        Self::new(Rational::from_i128(thousandths, 1000)?)
    }

    /// Short plain form for text fields: `"30"`, `"29.97"`, `"23.976"`, `"12.5"`.
    #[must_use]
    pub fn short_label(self) -> String {
        if self.0.den() == 1 {
            return self.0.num().to_string();
        }
        let s = format!("{:.3}", self.fps_f64());
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

impl fmt::Display for FrameRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.den() == 1 {
            write!(f, "{} fps", self.0.num())
        } else {
            write!(f, "{:.3} fps ({})", self.fps_f64(), self.0)
        }
    }
}

/// An audio sample rate in hertz.
///
/// Validated on construction: `1 <= hz <= 768,000`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SampleRate(u32);

impl SampleRate {
    /// 44.1 kHz — CD audio.
    pub const HZ_44_100: Self = Self(44_100);
    /// 48 kHz — the video standard.
    pub const HZ_48_000: Self = Self(48_000);
    /// 88.2 kHz.
    pub const HZ_88_200: Self = Self(88_200);
    /// 96 kHz.
    pub const HZ_96_000: Self = Self(96_000);
    /// 176.4 kHz.
    pub const HZ_176_400: Self = Self(176_400);
    /// 192 kHz.
    pub const HZ_192_000: Self = Self(192_000);

    /// The sample rates offered in presets. All are exact in flicks.
    pub const COMMON: [Self; 6] = [
        Self::HZ_44_100,
        Self::HZ_48_000,
        Self::HZ_88_200,
        Self::HZ_96_000,
        Self::HZ_176_400,
        Self::HZ_192_000,
    ];

    /// Validates a sample rate.
    ///
    /// # Errors
    /// [`CoreError::OutOfRange`] unless `1 <= hz <= 768,000`.
    pub fn new(hz: u32) -> Result<Self, CoreError> {
        if hz == 0 || hz > MAX_SAMPLE_RATE_HZ {
            return Err(CoreError::OutOfRange {
                what: "sample rate",
                allowed: "1 to 768000 Hz",
            });
        }
        Ok(Self(hz))
    }

    /// Rate in hertz.
    #[must_use]
    pub const fn hz(self) -> u32 {
        self.0
    }

    /// Start time of sample `sample`.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if the time is not representable.
    pub fn sample_to_time(self, sample: i64) -> Result<Time, CoreError> {
        index_to_time(sample, i64::from(self.0), 1)
    }

    /// The sample playing at time `t`.
    #[must_use]
    pub fn time_to_sample(self, t: Time) -> i64 {
        time_to_index(t, i64::from(self.0), 1)
    }

    /// `true` if every sample boundary is a whole number of flicks.
    #[must_use]
    pub fn is_exact(self) -> bool {
        is_exact(i64::from(self.0), 1)
    }
}

impl fmt::Display for SampleRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hz", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_people_type() {
        let p = |s: &str| FrameRate::parse_user(s).unwrap();
        assert_eq!(p("30"), FrameRate::FPS_30);
        assert_eq!(p(" 60 fps "), FrameRate::FPS_60);
        assert_eq!(p("29.97"), FrameRate::FPS_29_97);
        assert_eq!(p("29,97"), FrameRate::FPS_29_97);
        assert_eq!(p("23.976"), FrameRate::FPS_23_976);
        assert_eq!(p("23.98"), FrameRate::FPS_23_976);
        assert_eq!(p("59.94"), FrameRate::FPS_59_94);
        assert_eq!(p("119.88"), FrameRate::FPS_119_88);
        assert_eq!(p("30000/1001"), FrameRate::FPS_29_97);
        assert_eq!(p("60.00"), FrameRate::FPS_60);
        assert_eq!(p("12.5"), FrameRate::from_fraction(25, 2).unwrap());
        assert_eq!(
            p("29.9"),
            FrameRate::from_fraction(299, 10).unwrap(),
            "one digit is not NTSC shorthand"
        );
        assert_eq!(p("15"), FrameRate::from_fraction(15, 1).unwrap());
        for bad in [
            "",
            "abc",
            "-30",
            "0",
            "1.2.3",
            "1e3",
            "30 frames",
            "1001",
            "0.0000001",
            ".",
        ] {
            assert!(FrameRate::parse_user(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn measured_rates_snap_to_standards() {
        // Matroska stores 29.97 fps as 33,366,667 ns per frame.
        assert_eq!(
            FrameRate::from_measured(1_000_000_000, 33_366_667).unwrap(),
            FrameRate::FPS_29_97
        );
        assert_eq!(
            FrameRate::from_measured(1_000_000_000, 41_708_333).unwrap(),
            FrameRate::FPS_23_976
        );
        assert_eq!(FrameRate::from_measured(90_000, 3_000).unwrap(), FrameRate::FPS_30);
        assert_eq!(
            FrameRate::from_measured(15, 1).unwrap(),
            FrameRate::from_fraction(15, 1).unwrap()
        );
        // Far from any standard and with a huge denominator: kept, rounded to 1/1000.
        let odd = FrameRate::from_measured(1_000_000_000, 70_000_001).unwrap();
        assert_eq!(odd.as_rational(), Rational::new_const(7_143, 500));
        assert!(FrameRate::from_measured(0, 1).is_err());
        assert!(FrameRate::from_measured(5_000, 1).is_err());
    }

    #[test]
    fn short_labels() {
        assert_eq!(FrameRate::FPS_30.short_label(), "30");
        assert_eq!(FrameRate::FPS_29_97.short_label(), "29.97");
        assert_eq!(FrameRate::FPS_23_976.short_label(), "23.976");
        assert_eq!(FrameRate::from_fraction(25, 2).unwrap().short_label(), "12.5");
    }

    #[test]
    fn all_common_rates_are_exact() {
        for r in FrameRate::COMMON {
            assert!(r.is_exact(), "{r} should be exact");
            assert!(FrameRate::new(r.as_rational()).is_ok(), "{r} should validate");
        }
        for r in SampleRate::COMMON {
            assert!(r.is_exact(), "{r} should be exact");
        }
        for hz in [8_000, 11_025, 16_000, 22_050, 32_000] {
            assert!(SampleRate::new(hz).unwrap().is_exact());
        }
    }

    #[test]
    fn known_frame_durations() {
        assert_eq!(FrameRate::FPS_24.frame_duration(), Some(Time::from_flicks(29_400_000)));
        assert_eq!(
            FrameRate::FPS_23_976.frame_duration(),
            Some(Time::from_flicks(29_429_400))
        );
        assert_eq!(
            FrameRate::FPS_29_97.frame_duration(),
            Some(Time::from_flicks(23_543_520))
        );
        assert_eq!(
            FrameRate::FPS_59_94.frame_duration(),
            Some(Time::from_flicks(11_771_760))
        );
        // One hour of 29.97 fps is 107,892.107... frames; frame 107,892 starts just before 1 h.
        let hour = Time::from_seconds(3_600).unwrap();
        assert_eq!(FrameRate::FPS_29_97.time_to_frame(hour), 107_892);
    }

    #[test]
    fn validation() {
        assert!(FrameRate::from_fraction(0, 1).is_err());
        assert!(FrameRate::from_fraction(-24, 1).is_err());
        assert!(FrameRate::from_fraction(1_001, 1).is_err());
        assert!(FrameRate::from_fraction(1_000, 1).is_ok());
        assert!(FrameRate::from_fraction(1, 1_000_001).is_err());
        assert!(FrameRate::from_fraction(24, 0).is_err());
        assert!(SampleRate::new(0).is_err());
        assert!(SampleRate::new(768_001).is_err());
        assert!(SampleRate::new(768_000).is_ok());
    }

    #[test]
    fn inexact_rate_still_round_trips() {
        let odd = FrameRate::from_fraction(11, 1).unwrap(); // 11 does not divide 705,600,000
        assert!(!odd.is_exact());
        assert_eq!(odd.frame_duration(), None);
        for k in -1_000..1_000 {
            let t = odd.frame_to_time(k).unwrap();
            assert_eq!(odd.time_to_frame(t), k);
            assert_eq!(odd.time_to_frame(Time::from_flicks(t.flicks() - 1)), k - 1);
        }
    }

    #[test]
    fn frame_range_partitions_time() {
        let r = FrameRate::FPS_29_97;
        let a = r.frame_range(10).unwrap();
        let b = r.frame_range(11).unwrap();
        assert_eq!(a.end(), b.start());
        assert_eq!(a.duration(), r.frame_duration().unwrap());
        assert!(r.frame_range(i64::MAX).is_err());
    }

    #[test]
    fn extremes_do_not_panic() {
        for r in FrameRate::COMMON {
            let _ = r.time_to_frame(Time::MAX);
            let _ = r.time_to_frame(Time::MIN);
            assert!(r.frame_to_time(i64::MAX).is_err());
            assert!(r.frame_to_time(i64::MIN).is_err());
        }
        let fastest = FrameRate::from_fraction(1_000, 1).unwrap();
        let slowest = FrameRate::from_fraction(1, 1_000_000).unwrap();
        for r in [fastest, slowest] {
            let _ = r.time_to_frame(Time::MAX);
            let _ = r.frame_to_time(i64::MAX);
        }
        let s = SampleRate::new(768_000).unwrap();
        let _ = s.time_to_sample(Time::MIN);
        assert!(s.sample_to_time(i64::MAX).is_err());
    }

    #[test]
    fn display() {
        assert_eq!(FrameRate::FPS_25.to_string(), "25 fps");
        assert_eq!(FrameRate::FPS_23_976.to_string(), "23.976 fps (24000/1001)");
        assert_eq!(SampleRate::HZ_48_000.to_string(), "48000 Hz");
    }
}

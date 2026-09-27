//! Randomized property tests for exact time conversion.
//!
//! Uses a small deterministic generator (no external crates) so failures are
//! reproducible: every assertion message includes the seed and inputs.

use motix_core::{FLICKS_PER_SECOND, FrameRate, Rational, Rounding, SampleRate, Time};

/// xorshift64* — deterministic, fast, good enough for test inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(i128::from(hi) - i128::from(lo) + 1).expect("span fits");
        let offset = i64::try_from(self.next() % span).expect("offset fits");
        lo + offset
    }
}

const SEED: u64 = 0x05EE_D0FE_D170_A11F;
const CASES: usize = 20_000;

fn random_frame_rate(rng: &mut Rng) -> FrameRate {
    loop {
        let den = rng.range_i64(1, 1_000_000);
        let num = rng.range_i64(1, 1_000 * den);
        if let Ok(r) = FrameRate::from_fraction(num, den) {
            return r;
        }
    }
}

#[test]
fn frame_round_trip_for_random_rates() {
    let mut rng = Rng(SEED);
    for case in 0..CASES {
        let rate = random_frame_rate(&mut rng);
        // Stay inside the representable time range for this rate.
        let max_frame = rate.time_to_frame(Time::MAX) - 1;
        let min_frame = rate.time_to_frame(Time::MIN) + 1;
        let frame = rng.range_i64(min_frame.max(-10_000_000_000), max_frame.min(10_000_000_000));
        let t = rate.frame_to_time(frame).expect("representable");
        assert_eq!(rate.time_to_frame(t), frame, "case {case}: rate {rate:?} frame {frame}");
    }
}

#[test]
fn frames_partition_time_without_gaps() {
    let mut rng = Rng(SEED ^ 1);
    for case in 0..CASES {
        let rate = random_frame_rate(&mut rng);
        let frame = rng.range_i64(-1_000_000, 1_000_000);
        let start = rate.frame_to_time(frame).expect("start");
        let end = rate.frame_to_time(frame + 1).expect("end");
        assert!(start < end, "case {case}: frames must have positive length ({rate:?})");
        // First and last flick of the span belong to this frame; the next flick does not.
        assert_eq!(rate.time_to_frame(start), frame, "case {case}");
        assert_eq!(
            rate.time_to_frame(Time::from_flicks(end.flicks() - 1)),
            frame,
            "case {case}"
        );
        assert_eq!(rate.time_to_frame(end), frame + 1, "case {case}");
    }
}

#[test]
fn time_to_frame_is_monotonic() {
    let mut rng = Rng(SEED ^ 2);
    for case in 0..CASES {
        let rate = random_frame_rate(&mut rng);
        let a = rng.range_i64(i64::MIN / 2, i64::MAX / 2);
        let b = rng.range_i64(a, i64::MAX / 2);
        assert!(
            rate.time_to_frame(Time::from_flicks(a)) <= rate.time_to_frame(Time::from_flicks(b)),
            "case {case}: {rate:?} {a} {b}"
        );
    }
}

#[test]
fn common_rates_are_exact_multiples_over_long_spans() {
    for rate in FrameRate::COMMON {
        let d = rate.frame_duration().expect("common rates are exact").flicks();
        for frame in [0_i64, 1, 2, 1_000, 107_892, 10_000_000, -5] {
            assert_eq!(
                rate.frame_to_time(frame).unwrap().flicks(),
                frame * d,
                "{rate} frame {frame}"
            );
        }
    }
}

#[test]
fn sample_round_trip() {
    let mut rng = Rng(SEED ^ 3);
    for case in 0..CASES {
        let hz = u32::try_from(rng.range_i64(1, 768_000)).unwrap();
        let rate = SampleRate::new(hz).unwrap();
        let sample = rng.range_i64(-100_000_000_000, 100_000_000_000);
        let t = rate.sample_to_time(sample).unwrap();
        assert_eq!(rate.time_to_sample(t), sample, "case {case}: {hz} Hz sample {sample}");
    }
}

#[test]
fn audio_and_video_stay_in_sync_over_ten_hours() {
    // At every whole video frame of 29.97 fps, the 48 kHz sample index must
    // match the exact rational value — no drift after 10 hours.
    let video = FrameRate::FPS_29_97;
    let audio = SampleRate::HZ_48_000;
    let ten_hours = video.time_to_frame(Time::from_seconds(36_000).unwrap());
    for frame in (0..ten_hours).step_by(9_973) {
        let t = video.frame_to_time(frame).unwrap();
        let exact = Rational::new(frame * 48_000 * 1_001, 30_000).unwrap().floor();
        assert_eq!(audio.time_to_sample(t), exact, "frame {frame}");
    }
}

#[test]
fn rational_arithmetic_matches_reference() {
    let mut rng = Rng(SEED ^ 4);
    for case in 0..CASES {
        let (an, ad) = (rng.range_i64(-1_000_000, 1_000_000), rng.range_i64(1, 1_000_000));
        let (bn, bd) = (rng.range_i64(-1_000_000, 1_000_000), rng.range_i64(1, 1_000_000));
        let a = Rational::new(an, ad).unwrap();
        let b = Rational::new(bn, bd).unwrap();
        let sum = a.checked_add(b).unwrap();
        // Cross-check: sum * ad * bd == an*bd + bn*ad
        let lhs = i128::from(sum.num()) * i128::from(ad) * i128::from(bd);
        let rhs = (i128::from(an) * i128::from(bd) + i128::from(bn) * i128::from(ad)) * i128::from(sum.den());
        assert_eq!(lhs, rhs, "case {case}");
        assert_eq!(a.checked_sub(b).unwrap().checked_add(b).unwrap(), a, "case {case}");
        if !b.is_zero() {
            assert_eq!(a.checked_mul(b).unwrap().checked_div(b).unwrap(), a, "case {case}");
        }
        assert_eq!(
            a.cmp(&b),
            (i128::from(an) * i128::from(bd)).cmp(&(i128::from(bn) * i128::from(ad)))
        );
        assert!(a.floor() <= a.ceil() && a.ceil() - a.floor() <= 1);
    }
}

#[test]
fn seconds_rounding_brackets_exact_value() {
    let mut rng = Rng(SEED ^ 5);
    for case in 0..CASES {
        let s = Rational::new(
            rng.range_i64(-1_000_000_000, 1_000_000_000),
            rng.range_i64(1, 1_000_000_000),
        )
        .unwrap();
        let lo = Time::from_seconds_rational(s, Rounding::Floor).unwrap().flicks();
        let hi = Time::from_seconds_rational(s, Rounding::Ceil).unwrap().flicks();
        let near = Time::from_seconds_rational(s, Rounding::Nearest).unwrap().flicks();
        assert!(hi - lo <= 1, "case {case}");
        assert!(near == lo || near == hi, "case {case}");
        let exact_times_den = i128::from(s.num()) * i128::from(FLICKS_PER_SECOND);
        assert!(i128::from(lo) * i128::from(s.den()) <= exact_times_den, "case {case}");
        assert!(i128::from(hi) * i128::from(s.den()) >= exact_times_den, "case {case}");
    }
}

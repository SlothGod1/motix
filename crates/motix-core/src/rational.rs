//! Exact rational numbers.

use crate::CoreError;
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// An exact fraction `num / den`, always stored in lowest terms with `den > 0`.
///
/// Used for frame rates (`24000/1001`), pixel aspect ratios and speed factors,
/// where floating point would drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rational {
    num: i64,
    den: i64,
}

const fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Rational {
    /// Zero.
    pub const ZERO: Self = Self { num: 0, den: 1 };
    /// One.
    pub const ONE: Self = Self { num: 1, den: 1 };

    /// Creates a fraction, reducing it to lowest terms.
    ///
    /// # Errors
    /// [`CoreError::ZeroDenominator`] if `den == 0`; [`CoreError::Overflow`] if the
    /// normalized value does not fit (only possible with `i64::MIN`).
    pub fn new(num: i64, den: i64) -> Result<Self, CoreError> {
        if den == 0 {
            return Err(CoreError::ZeroDenominator);
        }
        Self::from_i128(i128::from(num), i128::from(den))
    }

    /// Const constructor for fractions written in source code.
    ///
    /// # Panics
    /// If `den <= 0` or the fraction is not already in lowest terms. Because it is
    /// `const`, misuse in a `const` item fails at compile time.
    #[must_use]
    pub const fn new_const(num: i64, den: i64) -> Self {
        assert!(den > 0, "denominator must be positive");
        assert!(
            gcd_u128(num.unsigned_abs() as u128, den.unsigned_abs() as u128) == 1,
            "fraction must be in lowest terms"
        );
        Self { num, den }
    }

    /// An integer as a fraction.
    #[must_use]
    pub const fn from_integer(n: i64) -> Self {
        Self { num: n, den: 1 }
    }

    /// Numerator (sign carrier).
    #[must_use]
    pub const fn num(self) -> i64 {
        self.num
    }

    /// Denominator (always positive).
    #[must_use]
    pub const fn den(self) -> i64 {
        self.den
    }

    /// `true` if the value is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.num == 0
    }

    /// `true` if the value is strictly greater than zero.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.num > 0
    }

    /// `true` if the value is strictly less than zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.num < 0
    }

    /// Reduces an `i128` fraction and converts it back to `i64` parts.
    pub(crate) fn from_i128(num: i128, den: i128) -> Result<Self, CoreError> {
        if den == 0 {
            return Err(CoreError::ZeroDenominator);
        }
        let g = gcd_u128(num.unsigned_abs(), den.unsigned_abs());
        // g >= 1 because den != 0; the casts cannot truncate because g divides both.
        let g = i128::try_from(g).map_err(|_| CoreError::Overflow)?;
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            n = n.checked_neg().ok_or(CoreError::Overflow)?;
            d = d.checked_neg().ok_or(CoreError::Overflow)?;
        }
        Ok(Self {
            num: i64::try_from(n).map_err(|_| CoreError::Overflow)?,
            den: i64::try_from(d).map_err(|_| CoreError::Overflow)?,
        })
    }

    /// Exact addition.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if the reduced result does not fit.
    pub fn checked_add(self, rhs: Self) -> Result<Self, CoreError> {
        let n = i128::from(self.num) * i128::from(rhs.den) + i128::from(rhs.num) * i128::from(self.den);
        let d = i128::from(self.den) * i128::from(rhs.den);
        Self::from_i128(n, d)
    }

    /// Exact subtraction.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if the reduced result does not fit.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, CoreError> {
        let n = i128::from(self.num) * i128::from(rhs.den) - i128::from(rhs.num) * i128::from(self.den);
        let d = i128::from(self.den) * i128::from(rhs.den);
        Self::from_i128(n, d)
    }

    /// Exact multiplication.
    ///
    /// # Errors
    /// [`CoreError::Overflow`] if the reduced result does not fit.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, CoreError> {
        Self::from_i128(
            i128::from(self.num) * i128::from(rhs.num),
            i128::from(self.den) * i128::from(rhs.den),
        )
    }

    /// Exact division.
    ///
    /// # Errors
    /// [`CoreError::ZeroDenominator`] when dividing by zero; [`CoreError::Overflow`]
    /// if the reduced result does not fit.
    pub fn checked_div(self, rhs: Self) -> Result<Self, CoreError> {
        Self::from_i128(
            i128::from(self.num) * i128::from(rhs.den),
            i128::from(self.den) * i128::from(rhs.num),
        )
    }

    /// `1 / self`.
    ///
    /// # Errors
    /// [`CoreError::ZeroDenominator`] if `self` is zero.
    pub fn recip(self) -> Result<Self, CoreError> {
        Self::ONE.checked_div(self)
    }

    /// Largest integer `<= self`.
    #[must_use]
    pub const fn floor(self) -> i64 {
        // With den > 0, Euclidean division is floor division and never overflows.
        self.num.div_euclid(self.den)
    }

    /// Smallest integer `>= self`.
    #[must_use]
    #[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
    pub const fn ceil(self) -> i64 {
        // ceil(x) = -floor(-x), computed in i128 so that `-i64::MIN` cannot overflow.
        // The result lies between num/den and num/den + 1, so it always fits in i64.
        (-((-(self.num as i128)).div_euclid(self.den as i128))) as i64
    }

    /// Approximate value, for display and non-critical math only.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        // Denominators are positive, so cross-multiplication preserves order.
        (i128::from(self.num) * i128::from(other.den)).cmp(&(i128::from(other.num) * i128::from(self.den)))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl FromStr for Rational {
    type Err = CoreError;

    /// Parses `"n"` or `"n/d"` (whitespace around parts allowed).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parse = |t: &str| {
            t.trim()
                .parse::<i64>()
                .map_err(|_| CoreError::Parse { what: "fraction" })
        };
        match s.split_once('/') {
            None => Ok(Self::from_integer(parse(s)?)),
            Some((n, d)) => Self::new(parse(n)?, parse(d)?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_sign_and_terms() {
        assert_eq!(Rational::new(2, 4).unwrap(), Rational::new_const(1, 2));
        assert_eq!(Rational::new(3, -6).unwrap(), Rational::new_const(-1, 2));
        assert_eq!(Rational::new(-3, -6).unwrap(), Rational::new_const(1, 2));
        assert_eq!(Rational::new(0, -5).unwrap(), Rational::ZERO);
    }

    #[test]
    fn rejects_zero_denominator() {
        assert_eq!(Rational::new(1, 0), Err(CoreError::ZeroDenominator));
        assert_eq!(
            Rational::ONE.checked_div(Rational::ZERO),
            Err(CoreError::ZeroDenominator)
        );
        assert_eq!(Rational::ZERO.recip(), Err(CoreError::ZeroDenominator));
    }

    #[test]
    fn extreme_values_overflow_cleanly() {
        // -i64::MIN is not representable.
        assert_eq!(Rational::new(i64::MIN, -1), Err(CoreError::Overflow));
        assert_eq!(Rational::new(i64::MIN, 1).unwrap().num(), i64::MIN);
        let big = Rational::from_integer(i64::MAX);
        assert_eq!(big.checked_add(Rational::ONE), Err(CoreError::Overflow));
        assert_eq!(big.checked_mul(Rational::from_integer(2)), Err(CoreError::Overflow));
    }

    #[test]
    fn arithmetic() {
        let a = Rational::new_const(24000, 1001);
        let b = Rational::new_const(1001, 24000);
        assert_eq!(a.checked_mul(b).unwrap(), Rational::ONE);
        assert_eq!(a.recip().unwrap(), b);
        assert_eq!(
            Rational::new_const(1, 3)
                .checked_add(Rational::new_const(1, 6))
                .unwrap(),
            Rational::new_const(1, 2)
        );
        assert_eq!(
            Rational::new_const(1, 3)
                .checked_sub(Rational::new_const(1, 2))
                .unwrap(),
            Rational::new_const(-1, 6)
        );
    }

    #[test]
    fn floor_and_ceil() {
        assert_eq!(Rational::new_const(7, 2).floor(), 3);
        assert_eq!(Rational::new_const(7, 2).ceil(), 4);
        assert_eq!(Rational::new_const(-7, 2).floor(), -4);
        assert_eq!(Rational::new_const(-7, 2).ceil(), -3);
        assert_eq!(Rational::from_integer(-3).floor(), -3);
        assert_eq!(Rational::from_integer(-3).ceil(), -3);
        assert_eq!(Rational::from_integer(i64::MIN).ceil(), i64::MIN);
    }

    #[test]
    fn ordering() {
        assert!(Rational::new_const(24000, 1001) < Rational::from_integer(24));
        assert!(Rational::new_const(-1, 2) < Rational::ZERO);
        assert_eq!(
            Rational::new_const(30000, 1001).cmp(&Rational::new_const(30000, 1001)),
            Ordering::Equal
        );
    }

    #[test]
    fn display_and_parse_round_trip() {
        for r in [
            Rational::new_const(24000, 1001),
            Rational::from_integer(25),
            Rational::new_const(-3, 7),
        ] {
            assert_eq!(r.to_string().parse::<Rational>().unwrap(), r);
        }
        assert_eq!(
            " 30000 / 1001 ".parse::<Rational>().unwrap(),
            Rational::new_const(30000, 1001)
        );
        assert!("1/0".parse::<Rational>().is_err());
        assert!("abc".parse::<Rational>().is_err());
        assert!("1/2/3".parse::<Rational>().is_err());
    }
}

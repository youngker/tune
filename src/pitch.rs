//! Linear and logarithmic operations on pitches, frequencies and frequency ratios.

use std::{
    cmp::Ordering,
    fmt::{self, Display, Formatter},
    ops::{Div, Mul},
    str::FromStr,
};

use crate::{
    math, parse,
    tuning::{Approximation, Tuning},
};

/// Struct representing the frequency of a pitch.
///
///
/// You can retrieve the absolute frequency of a [`Pitch`] in Hz via [`Pitch::as_hz`].
/// Alternatively, [`Pitch`]es can interact with [`Ratio`]s using [`Ratio::between_pitches`] or the [`Mul`]/[`Div`] operators.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Pitch {
    hz: f64,
}

impl Pitch {
    /// A more intuitive replacement for [`Pitched::pitch`].
    ///
    /// # Examples
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// use tune::pitch::Pitched;
    ///
    /// let note = NoteLetter::C.in_octave(4);
    /// assert_approx_eq!(Pitch::of(note).as_hz(), note.pitch().as_hz());
    /// ```
    pub fn of(pitched: impl Pitched) -> Pitch {
        pitched.pitch()
    }

    pub fn from_hz(hz: f64) -> Pitch {
        Pitch { hz }
    }

    pub fn as_hz(self) -> f64 {
        self.hz
    }
}

impl FromStr for Pitch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with("Hz") || s.ends_with("hz") {
            let freq = &s[..s.len() - 2];
            let freq = freq
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid frequency: '{freq}': {e}"))?;
            Ok(Pitch::from_hz(freq.as_float()))
        } else {
            Err("Must end with Hz or hz".to_string())
        }
    }
}

/// Lower a [`Pitch`] by a given [`Ratio`].
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::pitch::Pitch;
/// # use tune::pitch::Ratio;
/// assert_approx_eq!((Pitch::from_hz(330.0) / Ratio::from_float(1.5)).as_hz(), 220.0);
/// ```
impl Div<Ratio> for Pitch {
    type Output = Pitch;

    fn div(self, rhs: Ratio) -> Self::Output {
        Pitch::from_hz(self.as_hz() / rhs.as_float())
    }
}

/// Raise a [`Pitch`] by a given [`Ratio`].
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::pitch::Pitch;
/// # use tune::pitch::Ratio;
/// assert_approx_eq!((Pitch::from_hz(220.0) * Ratio::from_float(1.5)).as_hz(), 330.0);
/// ```
impl Mul<Ratio> for Pitch {
    type Output = Pitch;

    fn mul(self, rhs: Ratio) -> Self::Output {
        Pitch::from_hz(self.as_hz() * rhs.as_float())
    }
}

/// Objects which have a [`Pitch`] assigned.
pub trait Pitched {
    /// Retrieves the [`Pitch`] of the [`Pitched`] object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// use tune::pitch::Pitched;
    ///
    /// assert_approx_eq!(Pitch::from_hz(123.456).pitch().as_hz(), 123.456);
    /// assert_approx_eq!(NoteLetter::A.in_octave(5).pitch().as_hz(), 880.0);
    /// ```
    fn pitch(&self) -> Pitch;

    /// Finds a key or note for any [`Pitched`] object in the given `tuning`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// # use tune::tuning::ConcertPitch;
    /// use tune::pitch::Pitched;
    ///
    /// let a4 = NoteLetter::A.in_octave(4);
    /// let tuning = ConcertPitch::from_a4_pitch(Pitch::from_hz(432.0));
    ///
    /// let approximation = a4.find_in_tuning(tuning);
    /// assert_eq!(approximation.approx_value, a4);
    /// assert_approx_eq!(approximation.deviation.as_cents(), 31.766654);
    /// ```
    fn find_in_tuning<K, T: Tuning<K>>(&self, tuning: T) -> Approximation<K> {
        tuning.find_by_pitch(self.pitch())
    }
}

impl Pitched for Pitch {
    fn pitch(&self) -> Pitch {
        *self
    }
}

/// Struct representing the relative distance between two [`Pitch`]es.
///
/// Mathematically, this distance can be interpreted as the factor between the two pitches in
/// linear frequency space or as the offset between them in logarithmic frequency space.
///
/// The [`Ratio`] struct offers both linear and logarithmic accessors to the encapsulated distance.
/// It is possible to convert between the different representations by using `from_<repr1>` and `as_<repr2>` in
/// combination where `<reprN>` can be a linear (`float`) or logarithmic (`cents`, `semitones`, `octaves`) quantity.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::pitch::Ratio;
/// assert_approx_eq!(Ratio::from_float(1.5).as_cents(), 701.955);
/// assert_approx_eq!(Ratio::from_cents(400.0).as_semitones(), 4.0);
/// assert_approx_eq!(Ratio::from_semitones(3.0).as_octaves(), 0.25);
/// assert_approx_eq!(Ratio::from_octaves(3.0).as_float(), 8.0);
/// ```
///
/// # Invalid Values
///
/// [`Ratio`] can contain non-finite values if the *linear* value is not a finite positive number.
///
/// ```
/// # use tune::pitch::Ratio;
/// assert!(Ratio::from_cents(0.0).as_cents().is_finite());
/// assert!(Ratio::from_cents(-3.0).as_cents().is_finite());
/// assert!(Ratio::from_float(0.0).as_cents() == f64::NEG_INFINITY);
/// assert!(Ratio::from_float(-3.0).as_cents().is_nan());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Ratio {
    float_value: f64,
}

impl Ratio {
    pub fn from_float(float_value: f64) -> Self {
        Self { float_value }
    }

    pub fn from_cents(cents_value: f64) -> Self {
        Self::from_octaves(cents_value / 1200.0)
    }

    pub fn from_semitones(semitones: impl Into<f64>) -> Self {
        Self::from_octaves(semitones.into() / 12.0)
    }

    pub fn from_octaves(octaves: impl Into<f64>) -> Self {
        Self::from_float(octaves.into().exp2())
    }

    pub fn octave() -> Self {
        Self::from_float(2.0)
    }

    /// Creates a new [`Ratio`] instance based on the relative distance between two [`Pitched`] entities.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Pitch;
    /// # use tune::pitch::Ratio;
    /// let pitch_330_hz = Pitch::from_hz(330.0);
    /// let pitch_440_hz = Pitch::from_hz(440.0);
    /// assert_approx_eq!(Ratio::between_pitches(pitch_330_hz, pitch_440_hz).as_float(), 4.0 / 3.0);
    /// ```
    pub fn between_pitches(pitch_a: impl Pitched, pitch_b: impl Pitched) -> Self {
        Ratio::from_float(pitch_b.pitch().as_hz() / pitch_a.pitch().as_hz())
    }

    /// Stretches `self` by the provided `stretch`.
    ///
    /// This reverses [`Ratio::deviation_from`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_approx_eq!(Ratio::octave().stretched_by(Ratio::from_cents(10.0)).as_cents(), 1210.0);
    /// ```
    pub fn stretched_by(self, stretch: Ratio) -> Ratio {
        Ratio::from_float(self.as_float() * stretch.as_float())
    }

    /// Calculates the difference between the provided `reference` and `self`.
    ///
    /// This reverses [`Ratio::stretched_by`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_approx_eq!(Ratio::from_cents(1210.0).deviation_from(Ratio::octave()).as_cents(), 10.0);
    /// ```
    pub fn deviation_from(self, reference: Ratio) -> Ratio {
        Ratio::from_float(self.as_float() / reference.as_float())
    }

    /// Creates a new [`Ratio`] instance by applying `self` `num_repetitions` times.
    ///
    /// This reverses [`Ratio::divided_into_equal_steps`] or [`Ratio::num_equal_steps_of_size`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_approx_eq!(Ratio::from_semitones(2.0).repeated(3).as_semitones(), 6.0);
    /// ```
    pub fn repeated(self, num_repetitions: impl Into<f64>) -> Ratio {
        Ratio::from_octaves(self.as_octaves() * num_repetitions.into())
    }

    /// Returns the [`Ratio`] resulting from dividing `self` into `num_steps` equal steps.
    ///
    /// This reverses [`Ratio::repeated`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_approx_eq!(Ratio::octave().divided_into_equal_steps(15).as_cents(), 80.0);
    /// ```
    pub fn divided_into_equal_steps(self, num_steps: impl Into<f64>) -> Ratio {
        Ratio::from_octaves(self.as_octaves() / num_steps.into())
    }

    /// Determines how many equal steps of size `step_size` fit into `self`.
    ///
    /// This reverses [`Ratio::repeated`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_approx_eq!(Ratio::octave().num_equal_steps_of_size(Ratio::from_cents(80.0)), 15.0);
    /// ```
    pub fn num_equal_steps_of_size(self, step_size: Ratio) -> f64 {
        self.as_octaves() / step_size.as_octaves()
    }

    pub fn as_float(self) -> f64 {
        self.float_value
    }

    pub fn as_cents(self) -> f64 {
        self.as_semitones() * 100.0
    }

    pub fn as_semitones(self) -> f64 {
        self.as_octaves() * 12.0
    }

    pub fn as_octaves(self) -> f64 {
        self.float_value.log2()
    }

    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_approx_eq!(Ratio::from_float(4.0).inv().as_float(), 0.25);
    /// assert_approx_eq!(Ratio::from_cents(150.0).inv().as_cents(), -150.0);
    /// ```
    pub fn inv(self) -> Ratio {
        Self {
            float_value: 1.0 / self.float_value,
        }
    }

    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// assert_eq!(Ratio::from_float(f64::INFINITY).abs().as_float(), f64::INFINITY);
    /// assert_approx_eq!(Ratio::from_float(2.0).abs().as_float(), 2.0);
    /// assert_approx_eq!(Ratio::from_float(1.0).abs().as_float(), 1.0);
    /// assert_approx_eq!(Ratio::from_float(0.5).abs().as_float(), 2.0);
    /// assert_eq!(Ratio::from_float(0.0).abs().as_float(), f64::INFINITY);
    ///
    /// // Pathological cases, documented for completeness
    /// assert_eq!(Ratio::from_float(-0.0).abs().as_float(), f64::NEG_INFINITY);
    /// assert_approx_eq!(Ratio::from_float(-0.5).abs().as_float(), -2.0);
    /// assert_approx_eq!(Ratio::from_float(-1.0).abs().as_float(), -1.0);
    /// assert_approx_eq!(Ratio::from_float(-2.0).abs().as_float(), -2.0);
    /// assert_eq!(Ratio::from_float(f64::NEG_INFINITY).abs().as_float(), f64::NEG_INFINITY);
    /// assert!(Ratio::from_float(f64::NAN).abs().as_float().is_nan());
    /// ```
    pub fn abs(self) -> Ratio {
        Self {
            float_value: if self.float_value > -1.0 && self.float_value < 1.0 {
                self.float_value.recip()
            } else {
                self.float_value
            },
        }
    }

    /// Check whether the given [`Ratio`] is negligible.
    ///
    /// The threshold is around a 500th of a cent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// assert!(!Ratio::from_cents(0.002).is_negligible());
    /// assert!(Ratio::from_cents(0.001).is_negligible());
    /// assert!(Ratio::from_cents(0.000).is_negligible());
    /// assert!(Ratio::from_cents(-0.001).is_negligible());
    /// assert!(!Ratio::from_cents(-0.002).is_negligible());
    /// ```
    pub fn is_negligible(self) -> bool {
        (0.999999..1.000001).contains(&self.float_value)
    }

    /// `impl` stolen from <https://doc.rust-lang.org/std/primitive.f64.html#method.total_cmp>.
    pub fn total_cmp(&self, other: &Self) -> Ordering {
        let mut left = self.as_float().to_bits() as i64;
        let mut right = other.as_float().to_bits() as i64;
        left ^= (((left >> 63) as u64) >> 1) as i64;
        right ^= (((right >> 63) as u64) >> 1) as i64;
        left.cmp(&right)
    }

    /// Finds a rational number approximation of the current [`Ratio`] instance.
    ///
    /// The largest acceptable numerator or denominator can be controlled using the `odd_limit` parameter.
    /// Only odd factors are compared against the `odd_limit` which means that 12 is 3, effectively, while 11 stays 11.
    /// Read the documentation of [`math::odd_factors_u16`] for more examples.
    ///
    /// # Examples
    ///
    /// A minor seventh can be approximated by 16/9.
    ///
    ///```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// let minor_seventh = Ratio::from_semitones(10);
    /// let odd_limit = 9;
    /// let f = minor_seventh.nearest_fraction(odd_limit);
    /// assert_eq!((f.numer, f.denom), (16, 9));
    /// assert_eq!(f.num_octaves, 0);
    /// assert_approx_eq!(f.deviation.as_cents(), 3.910002); // Quite good!
    /// ```
    ///
    /// Reducing the `odd_limit` saves computation time but may lead to a bad approximation.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # let minor_seventh = Ratio::from_semitones(10);
    /// let odd_limit = 5;
    /// let f = minor_seventh.nearest_fraction(odd_limit);
    /// assert_eq!((f.numer, f.denom), (5, 3));
    /// assert_eq!(f.num_octaves, 0);
    /// assert_approx_eq!(f.deviation.as_cents(), 115.641287); // Pretty bad!
    /// ```
    ///
    /// The approximation is normalized to values within an octave. The number of octaves is reported separately.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// let lower_than_an_octave = Ratio::from_float(3.0 / 4.0);
    /// let odd_limit = 11;
    /// let f = lower_than_an_octave.nearest_fraction(odd_limit);
    /// assert_eq!((f.numer, f.denom), (3, 2));
    /// assert_eq!(f.num_octaves, -1);
    /// assert_approx_eq!(f.deviation.as_cents(), 0.0);
    /// ```
    pub fn nearest_fraction(self, odd_limit: u16) -> NearestFraction {
        NearestFraction::for_ratio(self, odd_limit)
    }
}

/// The default [`Ratio`] is the ratio that represents equivalence of two frequencies, i.e. no distance at all.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::pitch::Ratio;
/// assert_approx_eq!(Ratio::default().as_float(), 1.0); // Neutral element for multiplication
/// assert_approx_eq!(Ratio::default().as_cents(), 0.0); // Neutral element for addition
/// ```
impl Default for Ratio {
    fn default() -> Self {
        Self::from_float(1.0)
    }
}

/// [`Ratio`]s can be formatted as float or cents.
///
/// # Examples
//
/// ```
/// # use tune::pitch::Ratio;
/// // As float
/// assert_eq!(format!("{}", Ratio::from_float(1.5)), "1.5000");
/// assert_eq!(format!("{}", Ratio::from_float(1.0 / 1.5)), "0.6667");
/// assert_eq!(format!("{:.2}", Ratio::from_float(1.0 / 1.5)), "0.67");
///
/// // As cents
/// assert_eq!(format!("{:#}", Ratio::from_float(1.5)), "+702.0c");
/// assert_eq!(format!("{:#}", Ratio::from_float(1.0 / 1.5)), "-702.0c");
/// assert_eq!(format!("{:#.2}", Ratio::from_float(1.0 / 1.5)), "-701.96c");
///
/// // With padding
/// assert_eq!(format!("{:=^#14.2}", Ratio::from_float(1.5)), "===+701.96c===");
/// ```
impl Display for Ratio {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let formatted = if f.alternate() {
            format!(
                "{:+.precision$}c",
                self.as_cents(),
                precision = f.precision().unwrap_or(1)
            )
        } else {
            format!(
                "{:.precision$}",
                self.as_float(),
                precision = f.precision().unwrap_or(4)
            )
        };
        f.pad_integral(true, "", &formatted)
    }
}

/// [`Ratio`]s can be parsed using `tune`'s built-in expression language.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::pitch::Ratio;
/// assert_approx_eq!("1.5".parse::<Ratio>().unwrap().as_float(), 1.5);
/// assert_approx_eq!("3/2".parse::<Ratio>().unwrap().as_float(), 1.5);
/// assert_approx_eq!("7:12:2".parse::<Ratio>().unwrap().as_semitones(), 7.0);
/// assert_approx_eq!("702c".parse::<Ratio>().unwrap().as_cents(), 702.0);
/// assert_eq!("foo".parse::<Ratio>().unwrap_err(), "Invalid expression \'foo\': Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)");
impl FromStr for Ratio {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<RatioExpression>().map(RatioExpression::ratio)
    }
}

/// Target type for successfully parsed and validated ratio expressions.
#[derive(Copy, Clone, Debug)]
pub struct RatioExpression {
    ratio: Ratio,
    representation: RatioExpressionVariant,
}

impl RatioExpression {
    pub fn ratio(self) -> Ratio {
        self.ratio
    }

    pub fn variant(self) -> RatioExpressionVariant {
        self.representation
    }
}

/// The only way to construct a [`RatioExpression`] is via the [`FromStr`] trait.
impl FromStr for RatioExpression {
    type Err = String;

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        s = s.trim();
        parse_ratio(s)
            .and_then(|representation| {
                representation.as_ratio().map(|ratio| Self {
                    ratio,
                    representation,
                })
            })
            .map_err(|e| format!("Invalid expression '{s}': {e}"))
    }
}

/// Type used to distinguish which particular outer expression was given as string input before parsing.
#[derive(Copy, Clone, Debug)]
pub enum RatioExpressionVariant {
    Float {
        float_value: f64,
    },
    Fraction {
        numer: f64,
        denom: f64,
    },
    IntervalFraction {
        numer: f64,
        denom: f64,
        interval: f64,
    },
    Cents {
        cents_value: f64,
    },
}

impl RatioExpressionVariant {
    pub fn as_ratio(self) -> Result<Ratio, String> {
        let float_value = self.as_float()?;
        if float_value > 0.0 {
            Ok(Ratio { float_value })
        } else {
            Err(format!("Evaluates to {float_value} but should be positive"))
        }
    }

    fn as_float(self) -> Result<f64, String> {
        let as_float = match self {
            Self::Float { float_value } => float_value,
            Self::Fraction { numer, denom } => numer / denom,
            Self::IntervalFraction {
                numer,
                denom,
                interval,
            } => interval.powf(numer / denom),
            Self::Cents { cents_value } => Ratio::from_cents(cents_value).as_float(),
        };
        if as_float.is_finite() {
            Ok(as_float)
        } else {
            Err(format!("Evaluates to {as_float}"))
        }
    }
}

fn parse_ratio(s: &str) -> Result<RatioExpressionVariant, String> {
    let s = s.trim();
    if let [numer, denom, interval] = parse::split_balanced(s, ':').as_slice() {
        Ok(RatioExpressionVariant::IntervalFraction {
            numer: parse_ratio_as_float(numer, "interval numerator")?,
            denom: parse_ratio_as_float(denom, "interval denominator")?,
            interval: parse_ratio_as_float(interval, "interval")?,
        })
    } else if let [numer, denom] = parse::split_balanced(s, '/').as_slice() {
        Ok(RatioExpressionVariant::Fraction {
            numer: parse_ratio_as_float(numer, "numerator")?,
            denom: parse_ratio_as_float(denom, "denominator")?,
        })
    } else if let [cents_value, ""] = parse::split_balanced(s, 'c').as_slice() {
        Ok(RatioExpressionVariant::Cents {
            cents_value: parse_ratio_as_float(cents_value, "cents value")?,
        })
    } else if s.starts_with('(') && s.ends_with(')') {
        parse_ratio(&s[1..s.len() - 1])
    } else {
        Ok(RatioExpressionVariant::Float {
            float_value: s.parse().map_err(|_| {
                "Must be a float (e.g. 1.5), fraction (e.g. 3/2), \
                 interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)"
                    .to_string()
            })?,
        })
    }
}

fn parse_ratio_as_float(s: &str, name: &str) -> Result<f64, String> {
    parse_ratio(s)
        .and_then(RatioExpressionVariant::as_float)
        .map_err(|e| format!("Invalid {name} '{s}': {e}"))
}

/// An odd-limit nearest-fraction approximation fo a given [`Ratio`].
#[derive(Copy, Clone, Debug)]
pub struct NearestFraction {
    /// The numerator of the approximation.
    pub numer: u16,
    /// The denominator of the approximation.
    pub denom: u16,
    /// The deviation of the target value from the approximation.
    pub deviation: Ratio,
    /// The number of even factors that have been removed from the approximation to account for octave equivalence.
    pub num_octaves: i32,
}

impl NearestFraction {
    fn for_ratio(ratio: Ratio, odd_limit: u16) -> Self {
        let num_octaves = ratio.as_octaves().floor() as i32;
        let target_ratio = ratio.deviation_from(Ratio::from_octaves(num_octaves));

        let mut left = (0, 1);
        let mut right = (1, 0);

        let mut best = (0, 0);
        let mut best_deviation = Ratio::from_float(f64::INFINITY);

        while let Some(mid) =
            u16::checked_add(left.0, right.0).zip(u16::checked_add(left.1, right.1))
        {
            let odd_factors_numer = math::odd_factors_u16(mid.0);
            let odd_factors_denom = math::odd_factors_u16(mid.1);

            if odd_factors_numer > odd_limit && odd_factors_denom > odd_limit {
                break;
            }

            let mid_ratio = Ratio::from_float(f64::from(mid.0) / f64::from(mid.1));

            if odd_factors_numer <= odd_limit && odd_factors_denom <= odd_limit {
                let mid_deviation = target_ratio.deviation_from(mid_ratio);
                if mid_deviation.abs() < best_deviation.abs() {
                    best = mid;
                    best_deviation = mid_deviation;
                }
            }

            match target_ratio.partial_cmp(&mid_ratio) {
                Some(Ordering::Less) => {
                    right = mid;
                }
                Some(Ordering::Greater) => {
                    left = mid;
                }
                Some(Ordering::Equal) | None => break,
            }
        }

        NearestFraction {
            numer: best.0,
            denom: best.1,
            deviation: best_deviation,
            num_octaves,
        }
    }
}

impl Display for NearestFraction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let formatted = format!(
            "{}/{} [{:+.0}c] ({:+}o)",
            self.numer,
            self.denom,
            self.deviation.as_cents(),
            self.num_octaves
        );
        f.pad(&formatted)
    }
}

#[cfg(test)]
mod test {
    use std::iter;

    use super::*;

    #[test]
    fn parses_successfully() {
        let test_cases = [
            ("1", 1.0000),
            ("99.9", 99.9000),
            ("(1.25)", 1.2500),
            ("(1.25)", 1.2500),
            ("10/3", 3.3333),
            ("10/(10/3)", 3.0000),
            ("(10/3)/10", 0.3333),
            ("(3/4)/(5/6)", 0.9000),
            ("(3/4)/(5/6)", 0.9000),
            ("0:12:2", 1.000),
            ("7:12:2", 1.4983),   // 2^(7/12) - 12-edo perfect fifth
            ("7/12:1:2", 1.4983), // 2^(7/12) - 12-edo perfect fifth
            ("12:12:2", 2.000),
            ("-12:12:2", 0.500),
            ("4:1:3/2", 5.0625),   // (3/2)^4 - pythagorean major third
            ("1:1/4:3/2", 5.0625), // (3/2)^4 - pythagorean major third
            ("1/2:3/2:(1:2:64)", 2.0000),
            ("((1/2):(3/2):(1:2:64))", 2.0000),
            (" (    (1 /2)  :(3 /2):   (1: 2:   64  ))     ", 2.0000),
            ("12:7:700c", 2.000),
            ("0c", 1.0000),
            ("(0/3)c", 1.0000),
            ("702c", 1.5000),  // 2^(702/1200) - pythagorean fifth
            ("-702c", 0.6666), // 2^(-702/1200) - pythagorean fifth downwards
            ("1200c", 2.0000),
            ("702c/3", 0.5000),    // 2^(702/1200)/3 - 702 cents divided by 3
            ("3/702c", 2.0000),    // 3/2^(702/1200) - 3 divided by 702 cents
            ("(1404/2)c", 1.5000), // 2^(702/1200) - 1402/2 cents
        ];

        for (input, expected) in test_cases.iter() {
            let parsed = input.parse::<Ratio>().unwrap().as_float();
            assert!(
                (parsed - expected).abs() < 0.0001,
                "`{input}` should evaluate to {expected} but was {parsed:.4}"
            );
        }
    }

    #[test]
    fn parses_with_error() {
        let test_cases = [
            (
                "0.0",
                "Invalid expression '0.0': Evaluates to 0 but should be positive",
            ),
            (
                "-1.2345",
                "Invalid expression '-1.2345': Evaluates to -1.2345 but should be positive",
            ),
            ("1/0", "Invalid expression '1/0': Evaluates to inf"),
            (
                "(1/0)c",
                "Invalid expression '(1/0)c': Invalid cents value '(1/0)': Evaluates to inf",
            ),
            (
                "(1/x)c",
                "Invalid expression '(1/x)c': Invalid cents value '(1/x)': Invalid denominator 'x': \
                 Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)",
            ),
            (
                "   (1   /x )c ",
                "Invalid expression '(1   /x )c': Invalid cents value '(1   /x )': Invalid denominator 'x': \
                 Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)",
            ),
        ];

        for (input, expected) in test_cases.iter() {
            let parse_error = input.parse::<Ratio>().unwrap_err();
            assert_eq!(parse_error, *expected);
        }
    }

    #[test]
    fn parse_variant() {
        assert!(matches!(
            "1".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Float { .. }
        ));
        assert!(matches!(
            "10/3".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Fraction { .. }
        ));
        assert!(matches!(
            "(3/4)/(5/6)".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Fraction { .. }
        ));
        assert!(matches!(
            "12:7:700c".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::IntervalFraction { .. }
        ));
        assert!(matches!(
            "(0/3)c".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Cents { .. }
        ));
    }

    #[test]
    fn find_nearest_fraction() {
        let nearest_fractions: Vec<_> = iter::successors(Some(0.5), |prev| Some(prev * 1.05))
            .take(50)
            .map(|ratio| {
                format!(
                    "ratio = {:.2}, nearest_fraction = {}",
                    ratio,
                    Ratio::from_float(ratio).nearest_fraction(11)
                )
            })
            .collect();

        assert_eq!(
            nearest_fractions,
            [
                "ratio = 0.50, nearest_fraction = 1/1 [+0c] (-1o)",
                "ratio = 0.53, nearest_fraction = 12/11 [-66c] (-1o)",
                "ratio = 0.55, nearest_fraction = 11/10 [+4c] (-1o)",
                "ratio = 0.58, nearest_fraction = 7/6 [-13c] (-1o)",
                "ratio = 0.61, nearest_fraction = 11/9 [-10c] (-1o)",
                "ratio = 0.64, nearest_fraction = 14/11 [+5c] (-1o)",
                "ratio = 0.67, nearest_fraction = 4/3 [+9c] (-1o)",
                "ratio = 0.70, nearest_fraction = 7/5 [+9c] (-1o)",
                "ratio = 0.74, nearest_fraction = 3/2 [-26c] (-1o)",
                "ratio = 0.78, nearest_fraction = 14/9 [-5c] (-1o)",
                "ratio = 0.81, nearest_fraction = 18/11 [-8c] (-1o)",
                "ratio = 0.86, nearest_fraction = 12/7 [-4c] (-1o)",
                "ratio = 0.90, nearest_fraction = 9/5 [-4c] (-1o)",
                "ratio = 0.94, nearest_fraction = 11/6 [+49c] (-1o)",
                "ratio = 0.99, nearest_fraction = 2/1 [-17c] (-1o)",
                "ratio = 1.04, nearest_fraction = 1/1 [+67c] (+0o)",
                "ratio = 1.09, nearest_fraction = 12/11 [+1c] (+0o)",
                "ratio = 1.15, nearest_fraction = 8/7 [+5c] (+0o)",
                "ratio = 1.20, nearest_fraction = 6/5 [+5c] (+0o)",
                "ratio = 1.26, nearest_fraction = 14/11 [-13c] (+0o)",
                "ratio = 1.33, nearest_fraction = 4/3 [-9c] (+0o)",
                "ratio = 1.39, nearest_fraction = 7/5 [-9c] (+0o)",
                "ratio = 1.46, nearest_fraction = 16/11 [+10c] (+0o)",
                "ratio = 1.54, nearest_fraction = 14/9 [-22c] (+0o)",
                "ratio = 1.61, nearest_fraction = 8/5 [+14c] (+0o)",
                "ratio = 1.69, nearest_fraction = 12/7 [-21c] (+0o)",
                "ratio = 1.78, nearest_fraction = 16/9 [+0c] (+0o)",
                "ratio = 1.87, nearest_fraction = 11/6 [+31c] (+0o)",
                "ratio = 1.96, nearest_fraction = 2/1 [-35c] (+0o)",
                "ratio = 2.06, nearest_fraction = 1/1 [+50c] (+1o)",
                "ratio = 2.16, nearest_fraction = 12/11 [-17c] (+1o)",
                "ratio = 2.27, nearest_fraction = 8/7 [-13c] (+1o)",
                "ratio = 2.38, nearest_fraction = 6/5 [-13c] (+1o)",
                "ratio = 2.50, nearest_fraction = 5/4 [+1c] (+1o)",
                "ratio = 2.63, nearest_fraction = 4/3 [-26c] (+1o)",
                "ratio = 2.76, nearest_fraction = 11/8 [+5c] (+1o)",
                "ratio = 2.90, nearest_fraction = 16/11 [-8c] (+1o)",
                "ratio = 3.04, nearest_fraction = 3/2 [+23c] (+1o)",
                "ratio = 3.19, nearest_fraction = 8/5 [-4c] (+1o)",
                "ratio = 3.35, nearest_fraction = 5/3 [+10c] (+1o)",
                "ratio = 3.52, nearest_fraction = 7/4 [+10c] (+1o)",
                "ratio = 3.70, nearest_fraction = 11/6 [+14c] (+1o)",
                "ratio = 3.88, nearest_fraction = 2/1 [-52c] (+1o)",
                "ratio = 4.07, nearest_fraction = 1/1 [+32c] (+2o)",
                "ratio = 4.28, nearest_fraction = 12/11 [-34c] (+2o)",
                "ratio = 4.49, nearest_fraction = 9/8 [-3c] (+2o)",
                "ratio = 4.72, nearest_fraction = 7/6 [+19c] (+2o)",
                "ratio = 4.95, nearest_fraction = 5/4 [-16c] (+2o)",
                "ratio = 5.20, nearest_fraction = 9/7 [+19c] (+2o)",
                "ratio = 5.46, nearest_fraction = 11/8 [-12c] (+2o)"
            ]
        );
    }
}

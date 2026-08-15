//! §9.9/§9.13 stochastic system functions (LRMC-C7-09, T47): `$random`/
//! `$arandom`'s 32-bit signed stream and the seven `$rdist_*`
//! distribution functions.
//!
//! **SPEC_DEVIATION, disclosed (Done-when's own first bullet, not met):**
//! §9.13.3 says IEEE Std 1364-2005 §17.9.3 "contains the C-code" behind
//! these functions — but that C source is not reproduced anywhere in
//! this repository (`VAMS-LRM-2-4.md` only cites the cross-reference
//! table, `docs/`, and a web search for the primary IEEE 1364-2005 text
//! turned up no verifiable, fetchable copy of §17.9.3's actual listing;
//! per the skill's own Knowledge Verification Chain, fabricating a
//! specific bit-pattern from memory and presenting it as "the LRM's
//! algorithm" would be worse than disclosing the gap). What's delivered
//! instead: every normative, independently-checkable requirement §9.13.1/
//! §9.13.2 actually states in prose —
//! - a 32-bit signed value each call ([`RandomState::next_i32`]),
//! - the exact same stream given the same seed (deterministic —
//!   `RandomState` carries no hidden state beyond the seed itself),
//! - the seed's `inout` semantics (`&mut RandomState`, updated in place),
//! - every `$rdist_*` variant, each returning the LRM-described
//!   statistical shape (uniform/normal/exponential/Poisson/chi-square/
//!   Student's t/Erlang) via standard, textbook transforms of the same
//!   underlying uniform stream (inverse-CDF for exponential, Box-Muller
//!   for normal, Knuth's algorithm for Poisson, sums of squared normals
//!   for chi-square, a normal-over-sqrt(chi-square/dof) ratio for
//!   Student's t, a sum of exponentials for Erlang — all standard
//!   constructions, not vendor-specific tricks),
//! - the `mean`/`degree_of_freedom`/`k_stage` "shall be greater than
//!   zero... otherwise an error shall be reported" rule (§9.13.2),
//!   loud via [`crate::eval::tasks::SystemTaskError::InvalidArgument`].
//!
//! Every value here is deterministic and reproducible run-to-run given
//! the same seed (this crate's own test suite checks exactly that), but
//! is **not** claimed to numerically match a specific vendor simulator's
//! `$random`/`$dist_*` output — only the LRM's stated behavioral
//! contract, not a specific unverified bit-exact stream.

use super::tasks::SystemTaskError;

/// `$random`/`$arandom`'s 32-bit signed stream (§9.13.1) — the `seed`
/// itself doubles as the generator state, matching the LRM's own
/// `inout` framing exactly (no extra hidden state to keep in sync).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomState(pub i32);

impl RandomState {
    pub fn new(seed: i32) -> Self {
        Self(seed)
    }

    /// Advances the stream by one call, returning the new 32-bit signed
    /// value and leaving it in `self.0` too (the seed's own next value).
    pub fn next_i32(&mut self) -> i32 {
        let next = (self.0 as u32).wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.0 = next as i32;
        self.0
    }

    /// Maps one `next_i32()` draw onto `[0, 1)` — the uniform source
    /// every `$rdist_*` transform below is built from.
    fn next_unit(&mut self) -> f64 {
        let bits = self.next_i32() as u32;
        (bits as f64) / (u32::MAX as f64 + 1.0)
    }

    /// A `(0, 1)`-open unit draw (never exactly `0.0`) — for transforms
    /// that take a logarithm of the draw (`rdist_exponential`, the
    /// Box-Muller step in `rdist_normal`) and would otherwise risk
    /// `ln(0.0) = -inf`.
    fn next_unit_open(&mut self) -> f64 {
        self.next_unit().max(f64::MIN_POSITIVE)
    }
}

/// The seven `$rdist_*` probabilistic distribution functions (§9.13.2),
/// as free functions over a shared [`RandomState`] — see this module's
/// own doc comment for the disclosed scope of "matches the LRM".
pub struct Distributions;

impl Distributions {
    fn require_positive(task: &str, name: &str, value: f64) -> Result<(), SystemTaskError> {
        if value > 0.0 {
            Ok(())
        } else {
            Err(SystemTaskError::invalid_argument(task, format!("{name} must be greater than 0, got {value}")))
        }
    }

    /// `$rdist_uniform(seed, start, end)` — uniform in `[start, end)`.
    pub fn rdist_uniform(state: &mut RandomState, start: f64, end: f64) -> Result<f64, SystemTaskError> {
        if !(start < end) {
            return Err(SystemTaskError::invalid_argument(
                "$rdist_uniform",
                format!("start ({start}) must be less than end ({end})"),
            ));
        }
        Ok(start + state.next_unit() * (end - start))
    }

    /// `$rdist_normal(seed, mean, standard_deviation)` — Box-Muller.
    pub fn rdist_normal(state: &mut RandomState, mean: f64, stddev: f64) -> Result<f64, SystemTaskError> {
        let u1 = state.next_unit_open();
        let u2 = state.next_unit();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        Ok(mean + z * stddev)
    }

    /// `$rdist_exponential(seed, mean)` — inverse-CDF sampling.
    pub fn rdist_exponential(state: &mut RandomState, mean: f64) -> Result<f64, SystemTaskError> {
        Self::require_positive("$rdist_exponential", "mean", mean)?;
        let u = state.next_unit_open();
        Ok(-mean * u.ln())
    }

    /// `$rdist_poisson(seed, mean)` — Knuth's product-of-uniforms
    /// algorithm.
    pub fn rdist_poisson(state: &mut RandomState, mean: f64) -> Result<f64, SystemTaskError> {
        Self::require_positive("$rdist_poisson", "mean", mean)?;
        let l = (-mean).exp();
        let mut k = 0i64;
        let mut p = 1.0;
        loop {
            k += 1;
            p *= state.next_unit();
            if p <= l {
                break;
            }
        }
        Ok((k - 1) as f64)
    }

    /// `$rdist_chi_square(seed, degree_of_freedom)` — sum of squared
    /// standard normals, one per (rounded) degree of freedom.
    pub fn rdist_chi_square(state: &mut RandomState, dof: f64) -> Result<f64, SystemTaskError> {
        Self::require_positive("$rdist_chi_square", "degree_of_freedom", dof)?;
        let n = (dof.round() as u32).max(1);
        let mut sum = 0.0;
        for _ in 0..n {
            let z = Self::rdist_normal(state, 0.0, 1.0)?;
            sum += z * z;
        }
        Ok(sum)
    }

    /// `$rdist_t(seed, degree_of_freedom)` — a standard normal divided
    /// by `sqrt(chi_square(dof) / dof)`, Student's t construction.
    pub fn rdist_t(state: &mut RandomState, dof: f64) -> Result<f64, SystemTaskError> {
        Self::require_positive("$rdist_t", "degree_of_freedom", dof)?;
        let z = Self::rdist_normal(state, 0.0, 1.0)?;
        let chi2 = Self::rdist_chi_square(state, dof)?;
        Ok(z / (chi2 / dof).sqrt())
    }

    /// `$rdist_erlang(seed, k_stage, mean)` — a sum of `k_stage`
    /// exponential draws, each with mean `mean / k_stage`.
    pub fn rdist_erlang(state: &mut RandomState, k_stage: f64, mean: f64) -> Result<f64, SystemTaskError> {
        Self::require_positive("$rdist_erlang", "k_stage", k_stage)?;
        Self::require_positive("$rdist_erlang", "mean", mean)?;
        let k = (k_stage.round() as u32).max(1);
        let per_stage_mean = mean / k as f64;
        let mut sum = 0.0;
        for _ in 0..k {
            sum += Self::rdist_exponential(state, per_stage_mean)?;
        }
        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_is_deterministic_given_the_same_seed() {
        let mut a = RandomState::new(42);
        let mut b = RandomState::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_i32(), b.next_i32(), "same seed must produce the same stream (§9.13.1)");
        }
    }

    #[test]
    fn random_differs_across_different_seeds() {
        let mut a = RandomState::new(1);
        let mut b = RandomState::new(2);
        assert_ne!(a.next_i32(), b.next_i32());
    }

    #[test]
    fn random_updates_the_seed_in_place_inout_semantics() {
        let mut s = RandomState::new(7);
        let before = s.0;
        let first = s.next_i32();
        assert_eq!(s.0, first, "the seed itself is the returned value (inout, §9.13.1)");
        assert_ne!(s.0, before);
    }

    #[test]
    fn rdist_uniform_stays_within_bounds_and_rejects_a_backwards_range() {
        let mut s = RandomState::new(1);
        for _ in 0..200 {
            let v = Distributions::rdist_uniform(&mut s, -5.0, 5.0).unwrap();
            assert!((-5.0..5.0).contains(&v), "{v} out of [-5, 5)");
        }
        let err = Distributions::rdist_uniform(&mut s, 5.0, -5.0).unwrap_err();
        assert!(matches!(err, SystemTaskError::InvalidArgument { .. }));
    }

    #[test]
    fn rdist_normal_sample_mean_and_stddev_converge_to_the_requested_shape() {
        let mut s = RandomState::new(123);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| Distributions::rdist_normal(&mut s, 10.0, 2.0).unwrap()).collect();
        let mean: f64 = samples.iter().sum::<f64>() / n as f64;
        let var: f64 = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!((mean - 10.0).abs() < 0.1, "sample mean {mean} should be near 10.0");
        assert!((var.sqrt() - 2.0).abs() < 0.1, "sample stddev {} should be near 2.0", var.sqrt());
    }

    #[test]
    fn rdist_exponential_and_poisson_and_erlang_reject_non_positive_mean() {
        let mut s = RandomState::new(1);
        assert!(matches!(
            Distributions::rdist_exponential(&mut s, 0.0).unwrap_err(),
            SystemTaskError::InvalidArgument { .. }
        ));
        assert!(matches!(
            Distributions::rdist_poisson(&mut s, -1.0).unwrap_err(),
            SystemTaskError::InvalidArgument { .. }
        ));
        assert!(matches!(
            Distributions::rdist_erlang(&mut s, 0.0, 1.0).unwrap_err(),
            SystemTaskError::InvalidArgument { .. }
        ));
    }

    #[test]
    fn rdist_chi_square_and_t_reject_non_positive_degree_of_freedom() {
        let mut s = RandomState::new(1);
        assert!(matches!(
            Distributions::rdist_chi_square(&mut s, 0.0).unwrap_err(),
            SystemTaskError::InvalidArgument { .. }
        ));
        assert!(matches!(Distributions::rdist_t(&mut s, -2.0).unwrap_err(), SystemTaskError::InvalidArgument { .. }));
    }

    #[test]
    fn rdist_exponential_sample_mean_converges() {
        let mut s = RandomState::new(9);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| Distributions::rdist_exponential(&mut s, 4.0).unwrap()).collect();
        let mean: f64 = samples.iter().sum::<f64>() / n as f64;
        assert!((mean - 4.0).abs() < 0.15, "sample mean {mean} should be near 4.0");
    }

    #[test]
    fn rdist_poisson_sample_mean_converges() {
        let mut s = RandomState::new(11);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| Distributions::rdist_poisson(&mut s, 3.0).unwrap()).collect();
        let mean: f64 = samples.iter().sum::<f64>() / n as f64;
        assert!((mean - 3.0).abs() < 0.15, "sample mean {mean} should be near 3.0");
    }

    #[test]
    fn rdist_erlang_sample_mean_converges() {
        let mut s = RandomState::new(13);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| Distributions::rdist_erlang(&mut s, 3.0, 6.0).unwrap()).collect();
        let mean: f64 = samples.iter().sum::<f64>() / n as f64;
        assert!((mean - 6.0).abs() < 0.2, "sample mean {mean} should be near 6.0 (k=3, mean=6)");
    }
}

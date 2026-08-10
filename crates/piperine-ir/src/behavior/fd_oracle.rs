//! `verify_derivatives` — the finite-difference oracle every hand-written
//! `BehaviorKind::jacobian` must pass (`ideal.md` §3.8/§12: DVT-12).
//!
//! A wrong analytic derivative does not crash — it converges slowly or
//! converges to the wrong place. This is the only thing standing between a
//! new `BehaviorKind` and a plausible wrong answer, the same oracle
//! pattern `dv-gradients` used to validate the adjoint against `.sens`
//! (central finite difference: exact to `O(h²)`, cheap, and independent of
//! however the analytic Jacobian was derived).

/// Where a comparison against central finite difference disagreed.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivativeMismatch {
    /// The name the caller gave the kind under test (for the message —
    /// this crate has no `type_name` machinery of its own).
    pub kind: String,
    /// The sample point (the full input vector) where the mismatch fired.
    pub point: Vec<f64>,
    /// Which input's partial derivative disagreed.
    pub input: usize,
    /// The kind's own `jacobian` value.
    pub analytic: f64,
    /// The central finite-difference estimate.
    pub finite_diff: f64,
}

impl std::fmt::Display for DerivativeMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "verify_derivatives: {} disagrees with finite difference at input[{}], point {:?}: \
             analytic = {}, finite-difference = {}",
            self.kind, self.input, self.point, self.analytic, self.finite_diff
        )
    }
}

impl std::error::Error for DerivativeMismatch {}

/// Compare `kind`'s analytic Jacobian against a central finite difference
/// at every point in `points`, for every input dimension. `name` is
/// whatever the caller wants attributed in a failure (the kind's own type
/// has no runtime name available to a trait object).
///
/// Returns the first disagreement found — not a `Vec` of all of them; a
/// single named point is enough to reproduce and fix the bug, and this
/// crate does not want a "collect all findings" posture duplicated from
/// `dv-core`.
pub fn verify_derivatives(
    kind: &dyn crate::behavior::BehaviorKind,
    points: &[Vec<f64>],
    name: &str,
) -> Result<(), DerivativeMismatch> {
    for point in points {
        let n = point.len();
        let mut analytic = vec![0.0; n];
        kind.jacobian(point, &mut analytic);
        for i in 0..n {
            let h = 1e-6 * point[i].abs().max(1.0);
            let mut plus = point.clone();
            plus[i] += h;
            let mut minus = point.clone();
            minus[i] -= h;
            let fd = (kind.value(&plus) - kind.value(&minus)) / (2.0 * h);
            let tol = 1e-4 + 1e-4 * fd.abs();
            if (analytic[i] - fd).abs() > tol {
                return Err(DerivativeMismatch {
                    kind: name.to_string(),
                    point: point.clone(),
                    input: i,
                    analytic: analytic[i],
                    finite_diff: fd,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::kinds::{Comparator, Composed, Hysteresis, Limiter, Scaled, Sum, Switch, Vccs, Vco, Vcvs};
    use crate::behavior::table::Table;
    use crate::behavior::BehaviorKind;

    /// Sample grids for every shipped kind, chosen to avoid the exact
    /// transition point of a genuinely discontinuous kind (`Hysteresis`,
    /// `Switch`): FD probes `h` away from a jump would straddle it and
    /// disagree with the flat-elsewhere analytic Jacobian by design, which
    /// is not a bug — see the comment at each such kind's own test.
    fn grid_1d(points: &[f64]) -> Vec<Vec<f64>> {
        points.iter().map(|&x| vec![x]).collect()
    }
    fn grid_2d(points: &[(f64, f64)]) -> Vec<Vec<f64>> {
        points.iter().map(|&(a, b)| vec![a, b]).collect()
    }

    #[test]
    fn comparator_passes() {
        let k = Comparator { lo: 0.0, hi: 5.0, gain: 4.0 };
        let pts = grid_2d(&[(-1.0, 0.0), (0.0, 0.0), (0.3, -0.2), (2.0, 1.0)]);
        assert_eq!(verify_derivatives(&k, &pts, "comparator"), Ok(()));
    }

    #[test]
    fn hysteresis_passes_away_from_its_own_thresholds() {
        // Flat away from the flip: sample points that don't straddle
        // vth_hi=0.6 or vth_lo=0.4 within the FD step.
        let k = Hysteresis::new(0.6, 0.4);
        let pts = grid_2d(&[(0.5, 0.0), (0.5, 0.0), (0.5, 0.0)]);
        assert_eq!(verify_derivatives(&k, &pts, "hysteresis"), Ok(()));
    }

    #[test]
    fn switch_passes_away_from_its_own_threshold() {
        let k = Switch { ron: 1.0, roff: 1e6, vth: 0.5 };
        let pts = grid_1d(&[0.1, 0.9, 2.0]);
        assert_eq!(verify_derivatives(&k, &pts, "switch"), Ok(()));
    }

    #[test]
    fn limiter_passes_in_the_linear_region() {
        // Avoid the exact corners at lo/hi themselves (a genuine kink, not
        // a bug — same convention as Table's clamp region).
        let k = Limiter { lo: -1.0, hi: 1.0 };
        let pts = grid_1d(&[-0.9, -0.3, 0.0, 0.5, 0.9]);
        assert_eq!(verify_derivatives(&k, &pts, "limiter"), Ok(()));
    }

    #[test]
    fn vcvs_passes() {
        let k = Vcvs { gain: 3.0 };
        assert_eq!(verify_derivatives(&k, &grid_2d(&[(1.0, 0.4), (0.0, 0.0), (-2.0, 3.0)]), "vcvs"), Ok(()));
    }

    #[test]
    fn vccs_passes() {
        let k = Vccs { gm: 0.02 };
        assert_eq!(verify_derivatives(&k, &grid_2d(&[(1.0, 0.4), (0.0, 0.0), (-2.0, 3.0)]), "vccs"), Ok(()));
    }

    #[test]
    fn vco_passes() {
        let k = Vco { f0: 1e3, kvco: 500.0 };
        assert_eq!(verify_derivatives(&k, &grid_1d(&[-1.0, 0.0, 2.0]), "vco"), Ok(()));
    }

    #[test]
    fn table_passes_inside_a_segment() {
        // Away from the breakpoints themselves — same clamp/kink caveat.
        let k = Table::new(vec![0.0, 2.0, 4.0], vec![0.0, 10.0, 0.0]).unwrap();
        assert_eq!(verify_derivatives(&k, &grid_1d(&[0.5, 1.0, 3.0]), "table"), Ok(()));
    }

    #[test]
    fn composition_kinds_pass() {
        let sum = Sum { a: Box::new(Vcvs { gain: 2.0 }), b: Box::new(Vccs { gm: 3.0 }) };
        assert_eq!(verify_derivatives(&sum, &grid_2d(&[(1.0, 0.5), (0.0, 0.0)]), "sum"), Ok(()));

        let scaled = Scaled { inner: Box::new(Vco { f0: 1.0, kvco: 2.0 }), k: 5.0 };
        assert_eq!(verify_derivatives(&scaled, &grid_1d(&[0.5, 1.5]), "scaled"), Ok(()));

        // composed(f, g): f must take the single scalar g reduces to — a
        // vco (1 input, smooth) driven by a vcvs (2 inputs, smooth).
        let composed = Composed { f: Box::new(Vco { f0: 1.0, kvco: 2.0 }), g: Box::new(Vcvs { gain: 1.0 }) };
        assert_eq!(verify_derivatives(&composed, &grid_2d(&[(0.3, 0.1), (-0.2, 0.4)]), "composed"), Ok(()));
    }

    // ---- the guard proven able to fail (per DVT-12 and MD-31's spirit) ----

    /// A deliberately wrong kind, living only here as the discrimination
    /// proof: `value = x^2` but `jacobian` reports the constant `1.0`
    /// instead of `2x`. `verify_derivatives` must catch it, not converge
    /// slowly and stay silent.
    struct WrongSquare;

    impl BehaviorKind for WrongSquare {
        fn value(&self, inputs: &[f64]) -> f64 {
            inputs[0] * inputs[0]
        }
        fn jacobian(&self, _inputs: &[f64], out: &mut [f64]) {
            out[0] = 1.0; // wrong on purpose: should be 2*x
        }
    }

    #[test]
    fn a_wrong_jacobian_is_caught_and_the_failure_names_kind_point_and_both_values() {
        let wrong = WrongSquare;
        let err = verify_derivatives(&wrong, &grid_1d(&[3.0]), "wrong-square")
            .expect_err("a constant-1.0 jacobian on x^2 must disagree with FD (which is ~6.0 at x=3)");
        assert_eq!(err.kind, "wrong-square");
        assert_eq!(err.point, vec![3.0]);
        assert_eq!(err.input, 0);
        assert_eq!(err.analytic, 1.0);
        assert!((err.finite_diff - 6.0).abs() < 1e-3, "FD near the true derivative 2*3=6");
        let msg = err.to_string();
        assert!(msg.contains("wrong-square"), "message names the kind");
        assert!(msg.contains('3'), "message names the point");
        assert!(msg.contains('1'), "message carries the analytic value");
    }

    #[test]
    fn a_correct_jacobian_at_the_same_point_passes() {
        struct RightSquare;
        impl BehaviorKind for RightSquare {
            fn value(&self, inputs: &[f64]) -> f64 {
                inputs[0] * inputs[0]
            }
            fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
                out[0] = 2.0 * inputs[0];
            }
        }
        assert_eq!(verify_derivatives(&RightSquare, &grid_1d(&[3.0]), "right-square"), Ok(()));
    }
}

//! The secant step of crossing location: one prediction of the instant an
//! analog quantity crosses zero, from a bracketing pair of accepted samples.
//! The locator of `solver-crossings` retakes steps with it; the digital
//! engine and `piperine-ir` are not involved.

/// Which sign changes a crossing detector fires on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossDir {
    Rising,
    Falling,
    Either,
}

/// A predicted crossing instant with its error bound — `t_cross` is inside
/// `[t0, t1]`, the bracket that produced it, and `error_bound` is the width
/// of that bracket (never smaller than what the caller actually resolved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediction {
    /// The interpolated (secant) crossing instant.
    pub t_cross: f64,
    /// How far `t_cross` could still be off — the bracket width. Shrinks
    /// as the bracket narrows; never claims better than it has.
    pub error_bound: f64,
}

/// One secant prediction from a bracketing pair of accepted samples whose
/// sign change matches `dir` (`Rising`: `v0 <= 0` and `v1 > 0`; `Falling`:
/// `v0 >= 0` and `v1 < 0`; `Either`: any sign change). `None` when
/// `(t0, v0)`-`(t1, v1)` does not actually bracket a crossing in that
/// direction.
pub fn predict_crossing(dir: CrossDir, t0: f64, v0: f64, t1: f64, v1: f64) -> Option<Prediction> {
    let brackets = match dir {
        CrossDir::Rising => v0 <= 0.0 && v1 > 0.0,
        CrossDir::Falling => v0 >= 0.0 && v1 < 0.0,
        CrossDir::Either => (v0 <= 0.0 && v1 > 0.0) || (v0 >= 0.0 && v1 < 0.0),
    };
    if !brackets || v1 == v0 {
        return None;
    }
    let t_cross = t0 + (t1 - t0) * (-v0) / (v1 - v0);
    Some(Prediction { t_cross, error_bound: (t1 - t0).abs() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predict_crossing_interpolates_a_linear_bracket_exactly() {
        // v(t) = t - 0.5, bracketed by [0, 1]: the secant is exact for a
        // linear function.
        let p = predict_crossing(CrossDir::Rising, 0.0, -0.5, 1.0, 0.5).unwrap();
        assert!((p.t_cross - 0.5).abs() < 1e-15);
        assert_eq!(p.error_bound, 1.0, "bracket width, unrefined");
    }

    #[test]
    fn predict_crossing_rejects_a_non_bracketing_pair() {
        assert!(predict_crossing(CrossDir::Rising, 0.0, 0.5, 1.0, 1.0).is_none(), "no sign change");
        assert!(predict_crossing(CrossDir::Falling, 0.0, -0.5, 1.0, 0.5).is_none(), "wrong direction");
    }
}

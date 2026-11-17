//! Which unknowns make a Jacobian singular (§17.1, `E0503`): the null vector
//! of the dense matrix of the stamps, by singular value decomposition. It runs
//! only on failure, and not above [`MAX_DENSE`] unknowns.
use crate::error::{Error, RunCode, SolverDomain};
use crate::math::linear::{AsIndex, Stamp};
use crate::math::num::Scalar;
use faer::Mat;

/// The largest system the dense diagnosis is attempted on.
pub const MAX_DENSE: usize = 2000;

/// A component of the null vector counts when it exceeds this fraction of the
/// largest one.
const NULL_COMPONENT: f64 = 1e-6;

/// The diagnosis of one singular Jacobian.
#[derive(Debug, Clone, PartialEq)]
pub struct SingularityReport {
    /// The unknowns in the null vector of the smallest singular value, in index order.
    pub unknowns: Vec<usize>,
    /// `Some(size)` when the system was above [`MAX_DENSE`] and not diagnosed.
    pub skipped: Option<usize>,
}

impl SingularityReport {
    pub fn of<A: AsIndex, E: Scalar>(size: usize, stamps: &[Stamp<A, E>]) -> Self {
        if size > MAX_DENSE {
            return Self { unknowns: Vec::new(), skipped: Some(size) };
        }
        let mut dense = Mat::<f64>::zeros(size, size);
        for stamp in stamps {
            if let Stamp::Matrix(r, c, value) = stamp
                && let (Some(row), Some(col)) = (r.as_index(), c.as_index())
                && row < size
                && col < size
            {
                dense[(row, col)] += value.diagnostic_value();
            }
        }
        let unknowns = match dense.svd() {
            Ok(svd) if size > 0 => {
                let null = svd.V().col(size - 1);
                let largest = (0..size).map(|i| null[i].abs()).fold(0.0, f64::max);
                (0..size).filter(|&i| null[i].abs() > NULL_COMPONENT * largest).collect()
            }
            _ => Vec::new(),
        };
        Self { unknowns, skipped: None }
    }

    /// The run error of a linear solve that failed (`what` says how): `E0507`
    /// when a stamp itself is non-finite, else `E0503` with the null-vector
    /// unknowns (or the note that the diagnosis was skipped).
    pub fn error<A: AsIndex, E: Scalar>(
        domain: SolverDomain,
        size: usize,
        stamps: &[Stamp<A, E>],
        what: &str,
    ) -> Error {
        let non_finite = stamps.iter().any(|s| match s {
            Stamp::Matrix(_, _, v) | Stamp::Rhs(_, v) => !v.is_finite(),
        });
        if non_finite {
            return Error::run(RunCode::E0507, domain, format!("{what}: a stamp is non-finite"));
        }
        let report = Self::of(size, stamps);
        match report.skipped {
            Some(n) => Error::run(
                RunCode::E0503,
                domain,
                format!("{what}; {n} unknowns are above the {MAX_DENSE} the diagnosis covers, so none are named"),
            ),
            None => Error::run(RunCode::E0503, domain, what).with_unknowns(report.unknowns),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Ix(Option<usize>);
    impl AsIndex for Ix {
        fn as_index(&self) -> Option<usize> {
            self.0
        }
    }

    fn m(r: usize, c: usize, v: f64) -> Stamp<Ix, f64> {
        Stamp::Matrix(Ix(Some(r)), Ix(Some(c)), v)
    }

    #[test]
    fn a_loop_of_two_potential_sources_is_singular_in_both_flows() {
        // Unknowns: 0 = node a, 1 = flow of v1, 2 = flow of v2. Both sources
        // tie a to ground: the two branch rows are the same equation `a = V`,
        // so the flows are only determined through their sum (the null vector).
        let stamps = vec![m(0, 1, 1.0), m(0, 2, 1.0), m(1, 0, 1.0), m(2, 0, 1.0)];
        let report = SingularityReport::of(3, &stamps);
        assert_eq!(report.unknowns, vec![1, 2], "both flows, never the node");
        assert_eq!(report.skipped, None);
    }

    #[test]
    fn a_node_reached_only_through_capacitors_is_singular_at_dc() {
        // Unknowns: 0 = a driven node (1 S to ground), 1 = a floating node whose
        // only connections are capacitors, which stamp nothing at DC.
        let stamps = vec![m(0, 0, 1.0), m(1, 0, 0.0), m(0, 1, 0.0), m(1, 1, 0.0)];
        let report = SingularityReport::of(2, &stamps);
        assert_eq!(report.unknowns, vec![1]);
    }

    #[test]
    fn a_singular_system_above_the_dense_bound_says_it_skipped() {
        let size = MAX_DENSE + 1;
        let stamps: Vec<Stamp<Ix, f64>> = (1..size).map(|i| m(i, i, 1.0)).collect();
        let report = SingularityReport::of(size, &stamps);
        assert_eq!(report.skipped, Some(size));
        assert!(report.unknowns.is_empty());
    }
}

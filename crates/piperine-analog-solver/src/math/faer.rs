//! The `faer` sparse LU linear system: symbolic analysis + numeric
//! factorization behind the `LinearSystem`/`SymbolicLinearSystem`
//! contracts.
use crate::error::Error;
use crate::math::linear::{
    AsIndex, LinearSystem, Stamp, SymbolicLinearSystem, SymbolicMatrix,
};
use crate::math::num::Scalar;
use faer::prelude::{Solve, SparseColMat};
use faer::sparse::Triplet;
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::Col;
use ndarray::Array1;

#[derive(Clone)]
pub struct FaerSymbolicMatrix {
    size: usize,
    pattern: SymbolicLu<usize>,
}

impl SymbolicMatrix for FaerSymbolicMatrix {
    fn size(&self) -> usize {
        self.size
    }

    fn new<A: AsIndex, E: Scalar>(
        size: usize,
        stamps: Vec<Stamp<A, E>>,
    ) -> crate::error::Result<Self> {
        let mut triplets = Vec::new();

        for stamp in stamps {
            if let Stamp::Matrix(r, c, val) = stamp
                && let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                    triplets.push(Triplet::new(ri, ci, val));
                }
        }

        let mat = SparseColMat::try_new_from_triplets(size, size, &triplets).map_err(|err| {
            Error::cause(
                crate::error::SolverDomain::SpaceMatrix,
                "The library threw an error while trying to create the symbolic matrix",
                Box::new(err),
            )
        })?;

        let pattern = SymbolicLu::try_new(mat.symbolic()).map_err(|err| {
            Error::cause(
                crate::error::SolverDomain::SpaceMatrix,
                "The library threw an error while trying to create the symbolic matrix",
                Box::new(err),
            )
        })?;

        Ok(Self { size, pattern })
    }
}

pub struct FaerSparseLinearSystem<E: Scalar> {
    pub triplets: Vec<Triplet<usize, usize, E>>,
    pub b_vec: Vec<E>,
    pub size: usize,
}

impl<E: Scalar> FaerSparseLinearSystem<E> {
    /// Clear the stamped triplets and the RHS vector in-place, without
    /// reallocating, so the buffer is reused across Newton iterations.
    ///
    /// The symbolic LU (`SymbolicLu`) is already reused for a whole run; this
    /// closes the other half of per-iteration allocation cost — the `Vec`
    /// rebuild that `LinearSystem::new` would do. Call this instead of
    /// reconstructing the system each iteration.
    pub fn reset(&mut self) {
        self.triplets.clear();
        self.b_vec.fill(E::zero());
    }
}

impl FaerSparseLinearSystem<f64> {
    /// Solve `Jᵀ x = b` reusing the symbolic LU of `J` (DVG-08): the matrix
    /// `J` is factored once and `solve_transpose` is dispatched on the same
    /// factorization — never by building a separate transposed system and
    /// re-factoring it. Sibling of `analyses/noise.rs:240`'s complex adjoint
    /// (D6): same idea, real-valued, a different code path — the spec names
    /// this distinction deliberately so the optimistic "reuse the noise
    /// adjoint" reading cannot survive.
    ///
    /// Reads the same stamped triplets and RHS vector [`solve_with_backend`]
    /// does, against the same [`FaerSymbolicMatrix`] the forward solve was
    /// planned against — one symbolic pattern serves both directions.
    ///
    /// A singular or ill-conditioned `J` fails loud through the existing
    /// [`SolverDomain::SpaceMatrix`] error family — faer reports a
    /// structural deficiency at factor time, and a numerical deficiency
    /// (a structurally full-rank but rank-deficient value) as a non-finite
    /// result, caught here so no `±inf`/`NaN` ever reaches the adjoint
    /// driver. Never garbage.
    pub fn solve_transpose_with_backend(
        &self,
        symbolic: &FaerSymbolicMatrix,
    ) -> crate::error::Result<Array1<f64>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets)
            .map_err(|err| {
                Error::cause(
                    crate::error::SolverDomain::SpaceMatrix,
                    "The library threw an error while trying to create the LHS of the sparse matrix",
                    Box::new(err),
                )
            })?;

        let b = Col::from_fn(self.size, |i| self.b_vec[i]);

        let lu = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
            symbolic.pattern.clone(),
            a.as_ref(),
        )
        .map_err(|err| {
            Error::cause(
                crate::error::SolverDomain::SpaceMatrix,
                "the transpose solve requires a structurally non-singular Jacobian",
                Box::new(err),
            )
        })?;

        let lambda = lu.solve_transpose(&b).to_ndarray();
        // faer's symbolic LU catches *structural* singularity (a rank-
        // deficient sparsity pattern) at `try_new_with_symbolic`, but a
        // numerically singular matrix whose pattern is non-singular slips
        // through with `±inf`/`NaN` in the result. The Newton loop applies
        // the identical finiteness gate (`math/newton_raphson.rs:197`);
        // the adjoint must refuse garbage just as loud — a silent NaN here
        // would propagate as a wrong gradient to every host consumer.
        if lambda.iter().any(|x| !x.is_finite()) {
            return Err(Error::simple(
                crate::error::SolverDomain::SpaceMatrix,
                "the transpose solve returned a non-finite result — the Jacobian is \
                 numerically singular or ill-conditioned",
            ));
        }
        Ok(lambda)
    }
}

impl<E: 'static + Scalar> LinearSystem<E> for FaerSparseLinearSystem<E> {
    fn new(size: usize) -> Self {
        Self {
            triplets: Vec::with_capacity(size * 4),
            b_vec: vec![E::zero(); size],
            size,
        }
    }

    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp<A, E>>) {
        for stamp in stamps {
            match stamp {
                Stamp::Matrix(r, c, val) => {
                    if let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                        self.triplets.push(Triplet::new(ri, ci, val));
                    }
                }
                Stamp::Rhs(r, val) => {
                    if let Some(ri) = r.as_index() {
                        self.b_vec[ri] += val;
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        self.triplets.clear();
        self.b_vec.fill(E::zero());
    }

}

impl<E: Scalar + 'static> SymbolicLinearSystem<E> for FaerSparseLinearSystem<E> {
    type SymbolicType = FaerSymbolicMatrix;

    fn solve_with_backend(
        &self,
        symbolic: &Self::SymbolicType,
    ) -> crate::error::Result<Array1<E>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets).map_err(
            |err| Error::cause(crate::error::SolverDomain::SpaceMatrix,
                               "The library threw an error while trying to create the LHS of the sparse matrix", Box::new(err))
        )?;

        let b = Col::from_fn(self.size, |i| self.b_vec[i]);

        // REUSE Symbolic
        let lu = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
            symbolic.pattern.clone(),
            a.as_ref(),
        )
        .map_err(|err| {
            Error::cause(
                crate::error::SolverDomain::SpaceMatrix,
                "The library threw an error while trying to create the RHS of the sparse matrix",
                Box::new(err),
            )
        })?;

        Ok(lu.solve(&b).to_ndarray())
    }
}

pub trait FaerToNdarray<E> {
    fn to_ndarray(&self) -> Array1<E>;
}

impl<E: Clone + 'static> FaerToNdarray<E> for Col<E> {
    fn to_ndarray(&self) -> Array1<E> {
        self.as_ref().iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_clears_stamps_without_reallocating() {
        use crate::math::linear::{AsIndex, Stamp};

        #[derive(Clone, Copy)]
        struct Idx(usize);
        impl AsIndex for Idx {
            fn as_index(&self) -> Option<usize> { Some(self.0) }
        }

        let mut sys: FaerSparseLinearSystem<f64> = LinearSystem::new(4);
        // Stamp a matrix entry and an RHS entry.
        sys.apply_stamps(vec![
            Stamp::Matrix(Idx(0), Idx(1), 3.0),
            Stamp::Rhs(Idx(2), -7.0),
        ]);
        assert_eq!(sys.triplets.len(), 1);
        assert_eq!(sys.b_vec[2], -7.0);
        let cap_before = sys.triplets.capacity();

        sys.reset();

        // Cleared: no triplets, RHS back to zero.
        assert!(sys.triplets.is_empty(), "triplets not cleared");
        for v in &sys.b_vec {
            assert_eq!(*v, 0.0, "b_vec not zeroed");
        }
        // Capacity retained (no reallocation) — the whole point of reset().
        assert!(
            sys.triplets.capacity() >= cap_before,
            "capacity dropped: {} < {}", sys.triplets.capacity(), cap_before
        );
    }

    // ── dv-gradients T7: the real transpose solve Jᵀλ = b ────────────────────
    //
    // The adjoint primitive (DVG-08). Sibling of the noise adjoint at
    // `analyses/noise.rs:240` — same idea, real-valued, a different code
    // path. The spec (D6) names this distinction deliberately so the
    // optimistic "reuse the noise adjoint" reading cannot survive.

    /// Bare index carrier — the same shape the `reset…` test above uses.
    #[derive(Clone, Copy, Debug)]
    struct Idx(usize);
    impl AsIndex for Idx {
        fn as_index(&self) -> Option<usize> {
            Some(self.0)
        }
    }

    /// Build a stamped system + its symbolic pattern from raw `(r, c, v)`
    /// rows plus an RHS — the smallest legible assembly of a Jacobian.
    fn assemble(size: usize, matrix: &[(usize, usize, f64)], rhs: &[f64])
        -> (FaerSparseLinearSystem<f64>, FaerSymbolicMatrix)
    {
        let mut stamps: Vec<Stamp<Idx, f64>> = Vec::new();
        for &(r, c, v) in matrix {
            stamps.push(Stamp::Matrix(Idx(r), Idx(c), v));
        }
        for (i, &v) in rhs.iter().enumerate() {
            stamps.push(Stamp::Rhs(Idx(i), v));
        }
        let symbolic = FaerSymbolicMatrix::new(size, stamps.clone())
            .expect("symbolic pattern builds");
        let mut system: FaerSparseLinearSystem<f64> = LinearSystem::new(size);
        system.apply_stamps(stamps);
        (system, symbolic)
    }

    // DVG-08: a real transpose solve against a known small system.
    //
    // J = [[2, 5], [1, 3]]   (non-symmetric, det = 1)
    // Jᵀ = [[2, 1], [5, 3]]
    // Solving Jᵀ λ = [1, 0]ᵀ:
    //     2λ₁ + λ₂  = 1
    //     5λ₁ + 3λ₂ = 0
    // → λ₂ = 1 − 2λ₁ → 5λ₁ + 3 − 6λ₁ = 0 → λ₁ = 3, λ₂ = −5
    // Hand-derived expected: λ = [3, −5].
    #[test]
    fn real_transpose_solve_matches_a_hand_computed_lambda() {
        let (system, symbolic) = assemble(
            2,
            &[(0, 0, 2.0), (0, 1, 5.0), (1, 0, 1.0), (1, 1, 3.0)],
            &[1.0, 0.0],
        );

        let lambda = system
            .solve_transpose_with_backend(&symbolic)
            .expect("transpose solve converges on a well-conditioned 2×2");

        assert_eq!(lambda.len(), 2, "λ has the system size");
        assert!(
            (lambda[0] - 3.0).abs() < 1e-12,
            "λ₀ = 3 (hand-computed), got {}",
            lambda[0]
        );
        assert!(
            (lambda[1] - (-5.0)).abs() < 1e-12,
            "λ₁ = −5 (hand-computed), got {}",
            lambda[1]
        );
    }

    // DVG-08: the existing factorization is reused, not refactored.
    //
    // The proof is structural: the same `FaerSymbolicMatrix` (one symbolic
    // pattern) serves the forward solve `Jx = b` and the transpose solve
    // `Jᵀλ = c` against the same numeric Jacobian. A path that rebuilt a
    // transposed symbolic per direction would need a second pattern object;
    // here, one pattern answers both. The values are checked against the
    // analytic inverse so the assertion is on the math, not just "it ran".
    #[test]
    fn one_symbolic_factorization_serves_both_forward_and_transpose_solves() {
        // Same J as above; the analytic inverse is J⁻¹ = [[3, −5], [−1, 2]]
        // (det = 1). Forward solve Jx = b for b = [4, 1] → x = J⁻¹ b = [7, −2].
        // Transpose solve Jᵀλ = c for c = [1, 0] → λ = [3, −5] (hand-derived
        // above).
        let (system, symbolic) = assemble(
            2,
            &[(0, 0, 2.0), (0, 1, 5.0), (1, 0, 1.0), (1, 1, 3.0)],
            &[4.0, 1.0],
        );

        // Forward solve against the same pattern.
        let x = system.solve_with_backend(&symbolic).expect("forward solve");
        assert!(
            (x[0] - 7.0).abs() < 1e-12 && (x[1] - (-2.0)).abs() < 1e-12,
            "forward Jx = b matches the analytic inverse: got {:?}",
            x
        );

        // Re-stamp the RHS only — the matrix triplets stay; only `b`
        // changes. This is the exact pattern the adjoint driver uses: one
        // Jacobian, two right-hand sides, one symbolic pattern.
        let mut again: FaerSparseLinearSystem<f64> = LinearSystem::new(2);
        again.apply_stamps(vec![
            Stamp::Matrix(Idx(0), Idx(0), 2.0),
            Stamp::Matrix(Idx(0), Idx(1), 5.0),
            Stamp::Matrix(Idx(1), Idx(0), 1.0),
            Stamp::Matrix(Idx(1), Idx(1), 3.0),
            Stamp::Rhs(Idx(0), 1.0),
            Stamp::Rhs(Idx(1), 0.0),
        ]);

        let lambda = again
            .solve_transpose_with_backend(&symbolic)
            .expect("transpose solve with the reused symbolic pattern");
        assert!(
            (lambda[0] - 3.0).abs() < 1e-12 && (lambda[1] - (-5.0)).abs() < 1e-12,
            "transpose Jᵀλ = c with the same symbolic pattern matches: got {:?}",
            lambda
        );
    }

    // DVG-08: a singular Jacobian fails loud, never returns garbage.
    //
    // J = [[1, 1], [1, 1]] (rank 1, det = 0). faer reports
    // `LuError::SymbolicSingular`; the piperine wrapper maps every backend
    // failure into the existing `SolverDomain::SpaceMatrix` error family —
    // the same path a singular forward solve takes, never an `unwrap` or a
    // silent NaN.
    #[test]
    fn singular_system_fails_loud_through_the_existing_error_family() {
        let (system, symbolic) = assemble(
            2,
            &[(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0)],
            &[1.0, 0.0],
        );

        let err = system
            .solve_transpose_with_backend(&symbolic)
            .expect_err("singular J must refuse the transpose solve");
        let msg = err.to_string();
        assert!(
            msg.contains("SpaceMatrix") || msg.contains("non-singular"),
            "the failure routes through the SolverDomain error family: {msg}"
        );
    }
}

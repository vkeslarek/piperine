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
    pub size: usize,
    pub pattern: SymbolicLu<usize>,
}

impl SymbolicMatrix for FaerSymbolicMatrix {
    fn size(&self) -> usize {
        self.size
    }

    fn new<A: AsIndex, E: Scalar>(
        size: usize,
        stamps: Vec<Stamp<A, E>>,
    ) -> crate::result::Result<Self> {
        let mut triplets = Vec::new();

        for stamp in stamps {
            if let Stamp::Matrix(r, c, val) = stamp
                && let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                    triplets.push(Triplet::new(ri, ci, val));
                }
        }

        let mat = SparseColMat::try_new_from_triplets(size, size, &triplets).map_err(|err| {
            Error::cause(
                "Problem assembling the space matrix",
                "The library threw an error while trying to create the symbolic matrix",
                Box::new(err),
            )
        })?;

        let pattern = SymbolicLu::try_new(mat.symbolic()).map_err(|err| {
            Error::cause(
                "Problem assembling the space matrix",
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

}

impl<E: Scalar + 'static> SymbolicLinearSystem<E> for FaerSparseLinearSystem<E> {
    type SymbolicType = FaerSymbolicMatrix;

    fn solve_with_backend(
        &self,
        symbolic: &Self::SymbolicType,
    ) -> crate::result::Result<Array1<E>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets).map_err(
            |err| Error::cause("Problem assembling the space matrix",
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
                "Problem assembling the space matrix",
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

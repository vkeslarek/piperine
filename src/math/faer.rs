use crate::error::Error;
use crate::math::linear::{
    AsIndex, LinearSystem, NoSymbolic, Stamp, SymbolicLinearSystem, SymbolicMatrix,
};
use crate::math::num::Field;
use faer::prelude::{Solve, SparseColMat};
use faer::sparse::Triplet;
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::{Col, Mat};
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

    fn new<A: AsIndex, E: Field>(
        size: usize,
        stamps: Vec<Stamp<A, E>>,
    ) -> crate::result::Result<Self> {
        let mut triplets = Vec::new();

        for stamp in stamps {
            if let Stamp::Matrix(r, c, val) = stamp {
                if let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                    triplets.push(Triplet::new(ri, ci, val));
                }
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

pub struct FaerSparseLinearSystem<E: Field> {
    pub triplets: Vec<Triplet<usize, usize, E>>,
    pub b_vec: Vec<E>,
    pub size: usize,
}

impl<E: 'static + Field> LinearSystem<E> for FaerSparseLinearSystem<E> {
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

    fn solve(&self) -> crate::result::Result<Array1<E>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets).map_err(
            |err| Error::cause("Problem assembling the space matrix", "The library threw an error while trying to create the LHS of the sparse matrix", Box::new(err))
        )?;

        let b = Col::from_fn(self.size, |i| self.b_vec[i]);

        let symbolic_pattern = SymbolicLu::try_new(a.symbolic()).map_err(|err| {
            Error::cause(
                "Linear Solve Error",
                "Symbolic analysis (LU) failed",
                Box::new(err),
            )
        })?;

        let lu =
            faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(symbolic_pattern, a.as_ref())
                .map_err(|err| {
                    Error::cause(
                        "Linear Solve Error",
                        "LU Factorization failed",
                        Box::new(err),
                    )
                })?;

        Ok(lu.solve(&b).to_ndarray())
    }
}

impl<E: Field + 'static> SymbolicLinearSystem<E> for FaerSparseLinearSystem<E> {
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

pub struct FaerDenseLinearSystem<E: Field> {
    pub matrix: Mat<E>,
    pub rhs: Col<E>,
    pub size: usize,
}

impl<E: 'static + Field> LinearSystem<E> for FaerDenseLinearSystem<E> {
    fn new(size: usize) -> Self {
        Self {
            matrix: Mat::zeros(size, size),
            rhs: Col::zeros(size),
            size,
        }
    }

    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp<A, E>>) {
        for stamp in stamps {
            match stamp {
                Stamp::Matrix(r, c, val) => {
                    if let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                        self.matrix[(ri, ci)] = self.matrix[(ri, ci)] + val;
                    }
                }
                Stamp::Rhs(r, val) => {
                    if let Some(ri) = r.as_index() {
                        self.rhs[ri] = self.rhs[ri] + val;
                    }
                }
            }
        }
    }

    fn solve(&self) -> crate::result::Result<Array1<E>> {
        let lu = self.matrix.partial_piv_lu();

        let solution = lu.solve(&self.rhs);

        let mut out = Array1::zeros(self.size);
        for i in 0..self.size {
            out[i] = solution[i];
        }

        Ok(out)
    }
}

impl<E: Field + 'static> SymbolicLinearSystem<E> for FaerDenseLinearSystem<E> {
    type SymbolicType = NoSymbolic;

    fn solve_with_backend(
        &self,
        _symbolic: &Self::SymbolicType,
    ) -> crate::result::Result<Array1<E>> {
        self.solve()
    }
}

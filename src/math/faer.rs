use crate::error::Error;
use crate::math::linear::{AsIndex, DenseLinearSystem, SparseLinearSystem, Stamp2, SymbolicMatrix};
use crate::math::num::Field;
use faer::prelude::{Solve, SparseColMat};
use faer::sparse::Triplet;
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::{Col, Mat};
use ndarray::{Array1, Array2};

#[derive(Clone)]
pub struct FaerSymbolicMatrix {
    pub size: usize,
    pub pattern: SymbolicLu<usize>,
}

impl SymbolicMatrix for FaerSymbolicMatrix {
    fn new<A: AsIndex, E: Field>(
        size: usize,
        stamps: Vec<Stamp2<A, E>>,
    ) -> crate::result::Result<Self> {
        let mut triplets = Vec::new();

        for stamp in stamps {
            if let Stamp2::Matrix(r, c, val) = stamp {
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

    fn size(&self) -> usize {
        self.size
    }
}

pub struct FaerSparseLinearSystem<E: Field> {
    pub triplets: Vec<Triplet<usize, usize, E>>,
    pub b_vec: Vec<E>,
    pub size: usize,
}

impl<E: 'static + Field> SparseLinearSystem<E> for FaerSparseLinearSystem<E> {
    type SymbolicType = FaerSymbolicMatrix;

    fn new(size: usize) -> Self {
        Self {
            triplets: Vec::with_capacity(size * 4),
            b_vec: vec![E::zero(); size],
            size,
        }
    }

    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp2<A, E>>) {
        for stamp in stamps {
            match stamp {
                Stamp2::Matrix(r, c, val) => {
                    if let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                        self.triplets.push(Triplet::new(ri, ci, val));
                    }
                }
                Stamp2::Rhs(r, val) => {
                    if let Some(ri) = r.as_index() {
                        self.b_vec[ri] += val;
                    }
                }
            }
        }
    }

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

pub trait FaerToNdarray<E> {
    fn to_ndarray(&self) -> Array1<E>;
}

impl<E: Clone + 'static> FaerToNdarray<E> for Col<E> {
    fn to_ndarray(&self) -> Array1<E> {
        self.as_ref().iter().cloned().collect()
    }
}

pub struct FaerDenseSolver<E: Field> {
    matrix: Mat<E>,
    rhs: Mat<E>,
}

impl<E: Field> FaerDenseSolver<E> {
    pub fn new(size: usize) -> Self {
        Self {
            matrix: Mat::<E>::zeros(size, size),
            rhs: Mat::<E>::zeros(size, 1),
        }
    }
}

impl<E: Field> DenseLinearSystem<E> for FaerDenseSolver<E> {
    fn set_matrix(&mut self, matrix: &Array2<E>) {
        assert_eq!(matrix.nrows(), self.matrix.nrows());
        assert_eq!(matrix.ncols(), self.matrix.ncols());

        for r in 0..matrix.nrows() {
            for c in 0..matrix.ncols() {
                let val = matrix[[r, c]];
                self.matrix[(r, c)] = val;
            }
        }
    }

    fn set_rhs(&mut self, rhs: &Array1<E>) {
        assert_eq!(rhs.len(), self.rhs.nrows());

        for r in 0..rhs.ndim() {
            self.rhs[(r, 0)] = rhs[r];
        }
    }

    fn solve(&self) -> crate::result::Result<Array1<E>> {
        let lu = self.matrix.partial_piv_lu();
        let solution_mat = lu.solve(&self.rhs);

        let vec_size = self.rhs.nrows();
        let mut solution_array = Array1::<E>::zeros(vec_size);
        for i in 0..vec_size {
            solution_array[i] = solution_mat[(i, 0)];
        }

        Ok(solution_array)
    }
}

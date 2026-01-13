use crate::error::Error;
use crate::math::linear::{DenseLinearSystem, SparseLinearSystem, SymbolicMatrix};
use crate::math::num::Field;
use crate::math::{Stamp, Symbol};
use faer::prelude::{Solve, SparseColMat};
use faer::sparse::Triplet;
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::traits::ComplexField;
use faer::{Col, Mat};
use ndarray::{Array1, Array2};
use std::collections::HashMap;
use std::marker::PhantomData;

pub struct FaerSparseLinearSystem<S: Symbol, E: Field> {
    pub triplets: Vec<Triplet<usize, usize, E>>,
    pub b_vec: Vec<E>,
    pub size: usize,
    _phantom: PhantomData<S>,
}

impl<S: Symbol, E: Field + ComplexField + 'static> SparseLinearSystem<S, E>
    for FaerSparseLinearSystem<S, E>
{
    type SymbolicType = FaerSymbolicMatrix<S>;

    fn new(size: usize) -> Self {
        Self {
            triplets: Vec::with_capacity(size * 4),
            b_vec: vec![E::zero(); size],
            size,
            _phantom: PhantomData,
        }
    }

    fn apply_stamps(&mut self, symbolic: &Self::SymbolicType, stamps: Vec<Stamp<S, E>>) {
        for stamp in stamps {
            match stamp {
                Stamp::Matrix(r, c, val) => {
                    if let (Some(&ri), Some(&ci)) =
                        (symbolic.mapping().get(&r), symbolic.mapping().get(&c))
                    {
                        self.triplets.push(Triplet::new(ri, ci, val));
                    }
                }
                Stamp::Rhs(r, val) => {
                    if let Some(&ri) = symbolic.mapping().get(&r) {
                        self.b_vec[ri] += val;
                    }
                }
            }
        }
    }

    fn solve_with_backend(self, symbolic: &Self::SymbolicType) -> crate::result::Result<Array1<E>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets).map_err(
            |err| Error::cause("Problem assembling the space matrix", "The library threw an error while trying to create the LHS of the sparse matrix", Box::new(err))
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

#[derive(Clone)]
pub struct FaerSymbolicMatrix<S: Symbol> {
    pub mapping: HashMap<S, usize>,
    pub size: usize,
    pub pattern: SymbolicLu<usize>,
}

impl<S: Symbol> FaerSymbolicMatrix<S> {
    pub fn new<D: Field + ComplexField>(
        symbols: Vec<S>,
        stamps: Vec<Stamp<S, D>>,
    ) -> crate::result::Result<Self>
    where
        Self: Sized,
    {
        let mut mapping = HashMap::new();
        let mut index = 0;

        for symbol in symbols {
            mapping.insert(symbol, index);
            index += 1;
        }

        let mut triplets = Vec::new();

        for stamp in stamps {
            if let Stamp::Matrix(a, b, val) = stamp {
                let a_mapped = mapping.get(&a).unwrap();
                let b_mapped = mapping.get(&b).unwrap();
                triplets.push(Triplet::new(*a_mapped, *b_mapped, val));
            }
        }

        let size = mapping.len();
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

        Ok(Self {
            mapping,
            size,
            pattern,
        })
    }
}

impl<S: Symbol> SymbolicMatrix<S> for FaerSymbolicMatrix<S> {
    fn size(&self) -> usize {
        self.size
    }

    fn mapping(&self) -> &HashMap<S, usize> {
        &self.mapping
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

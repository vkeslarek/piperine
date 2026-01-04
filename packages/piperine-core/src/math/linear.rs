use crate::error::{ErrorDetail, Problem};
use faer::Col;
use faer::prelude::{Solve, SparseColMat};
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::traits::ComplexField;
use num_complex::Complex;
use num_traits::Zero;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::AddAssign;

pub use faer::sparse::Triplet;

pub trait Symbol: Clone + Eq + Hash {}

pub trait Element: Copy + Zero + AddAssign + ComplexField {}

impl Element for f64 {}
impl Element for Complex<f64> {}

pub enum Stamp<S: Symbol, E: Element> {
    Matrix(S, S, E),
    Rhs(S, E),
}

pub struct LinearSystem<E: Element> {
    pub triplets: Vec<Triplet<usize, usize, E>>,
    pub b_vec: Vec<E>,
    pub size: usize,
}

impl<E: Element> LinearSystem<E> {
    pub fn new(size: usize) -> Self {
        Self {
            triplets: Vec::with_capacity(size * 4),
            b_vec: vec![E::zero(); size],
            size,
        }
    }

    pub fn apply_stamps<S: Symbol>(
        &mut self,
        symbolic: &SymbolicMatrix<S>,
        stamps: Vec<Stamp<S, E>>,
    ) {
        for stamp in stamps {
            match stamp {
                Stamp::Matrix(r, c, val) => {
                    if let (Some(&ri), Some(&ci)) =
                        (symbolic.mapping.get(&r), symbolic.mapping.get(&c))
                    {
                        self.triplets.push(Triplet::new(ri, ci, val));
                    }
                }
                Stamp::Rhs(r, val) => {
                    if let Some(&ri) = symbolic.mapping.get(&r) {
                        self.b_vec[ri] += val;
                    }
                }
            }
        }
    }

    pub fn solve_with_backend<S: Symbol>(
        self,
        symbolic: &SymbolicMatrix<S>,
    ) -> crate::error::Result<HashMap<S, E>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets).map_err(
            |err| ErrorDetail {
                title: "Problem assembling the space matrix".to_string(),
                detail:
                    "The library threw an error while trying to create the LHS of the sparse matrix"
                        .to_string(),
                problems: vec![Problem::FaerCreationProblem(err)],
            },
        )?;

        let b = Col::from_fn(self.size, |i| self.b_vec[i]);

        // REUSE Symbolic
        let lu = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
            symbolic.pattern.clone(),
            a.as_ref(),
        )
        .map_err(|err| ErrorDetail {
            title: "Problem assembling the space matrix".to_string(),
            detail:
                "The library threw an error while trying to create the RHS of the sparse matrix"
                    .to_string(),
            problems: vec![Problem::FaerLuError(err)],
        })?;

        let sol = lu.solve(&b);

        let mut ret: HashMap<S, E> = HashMap::new();
        for (reference, index) in &symbolic.mapping {
            ret.insert(reference.clone(), sol[*index]);
        }

        Ok(ret)
    }
}

pub struct SymbolicMatrix<S: Symbol> {
    pub mapping: HashMap<S, usize>,
    pub size: usize,
    pub pattern: SymbolicLu<usize>,
}

impl<T: Symbol> SymbolicMatrix<T> {
    pub fn new<D: Element>(
        symbols: Vec<T>,
        stamps: Vec<Stamp<T, D>>,
    ) -> crate::error::Result<Self> {
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
            ErrorDetail {
                title: "Problem assembling the space matrix".to_string(),
                detail: "The library threw an error while trying to create the symbolic matrix"
                    .to_string(),
                problems: vec![Problem::FaerCreationProblem(err)],
            }
        })?;

        let pattern = SymbolicLu::try_new(mat.symbolic()).map_err(|err| ErrorDetail {
            title: "Problem assembling the space matrix".to_string(),
            detail: "The library threw an error while trying to create the symbolic matrix"
                .to_string(),
            problems: vec![Problem::FaerGenericError(err)],
        })?;

        Ok(Self {
            mapping,
            size,
            pattern,
        })
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

use crate::math::Symbol;
use crate::math::deriv::{BdfCoefficientGenerator, DifferentiableIndependentScalar};
use crate::math::num::{Field, ScalableByReal};
use ndarray::{Array1, Array2, ArrayView1, ArrayViewMut1, Zip};
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

pub struct InitialValue<S: Symbol, E: Field> {
    pub reference: S,
    pub value: E,
}

#[derive(Debug, Clone)]
pub struct SymbolicVector1<S: Symbol, E: Field> {
    pub values: Array1<E>,
    pub mapping: HashMap<S, usize>,
}

impl<S: Symbol, E: Field> SymbolicVector1<S, E> {
    pub fn new(symbolic_mapping: HashMap<S, usize>) -> Self {
        let size = symbolic_mapping.len();
        Self {
            values: Array1::zeros(size),
            mapping: symbolic_mapping,
        }
    }

    pub fn from_values(values: Array1<E>, mapping: HashMap<S, usize>) -> Self {
        assert_eq!(
            values.len(),
            mapping.len(),
            "Data size does not match mapping"
        );
        Self { values, mapping }
    }

    pub fn view(&self) -> SymbolicVectorView<'_, S, E> {
        SymbolicVectorView {
            values: self.values.view(),
            mapping: &self.mapping,
        }
    }

    pub fn view_mut(&mut self) -> SymbolicVectorViewMut<'_, S, E> {
        SymbolicVectorViewMut {
            values: self.values.view_mut(),
            mapping: &self.mapping,
        }
    }

    pub fn get(&self, symbol: &S) -> Option<&E> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get(idx)
    }

    pub fn get_mut(&mut self, symbol: &S) -> Option<&mut E> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get_mut(idx)
    }

    fn apply_initial_values(&mut self, initial_conditions: Vec<InitialValue<S, E>>) {
        for initial_value in initial_conditions {
            let idx = self
                .mapping
                .get(&initial_value.reference)
                .expect("Initial value references an invalid symbol");
            self.values[*idx] = initial_value.value;
        }
    }
}

impl<S: Symbol, E: Field> Index<&S> for SymbolicVector1<S, E> {
    type Output = E;
    fn index(&self, symbol: &S) -> &Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &self.values[*idx]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SymbolicVectorView<'a, S: Symbol, E: Field> {
    pub values: ArrayView1<'a, E>,
    pub mapping: &'a HashMap<S, usize>,
}

impl<'a, S: Symbol, E: Field> SymbolicVectorView<'a, S, E> {
    pub fn get(&self, symbol: &S) -> Option<&E> {
        self.mapping
            .get(symbol)
            .and_then(|&idx| self.values.get(idx))
    }

    pub fn to_owned(&self) -> SymbolicVector1<S, E> {
        SymbolicVector1 {
            values: self.values.to_owned(),
            mapping: self.mapping.clone(),
        }
    }
}

impl<'a, S: Symbol, E: Field> Index<&S> for SymbolicVectorView<'a, S, E> {
    type Output = E;
    fn index(&self, symbol: &S) -> &Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &self.values[*idx]
    }
}

#[derive(Debug)]
pub struct SymbolicVectorViewMut<'a, S: Symbol, E: Field> {
    pub values: ArrayViewMut1<'a, E>,
    pub mapping: &'a HashMap<S, usize>,
}

impl<'a, S: Symbol, E: Field> Index<&S> for SymbolicVectorViewMut<'a, S, E> {
    type Output = E;
    fn index(&self, symbol: &S) -> &Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &self.values[*idx]
    }
}

impl<'a, S: Symbol, E: Field> IndexMut<&S> for SymbolicVectorViewMut<'a, S, E> {
    fn index_mut(&mut self, symbol: &S) -> &mut Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &mut self.values[*idx]
    }
}

impl<'a, S: Symbol, E: Field> SymbolicVectorViewMut<'a, S, E> {
    pub fn get(&self, symbol: &S) -> Option<&E> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get(idx)
    }

    pub fn get_mut(&mut self, symbol: &S) -> Option<&mut E> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get_mut(idx)
    }

    pub fn apply_initial_values(&mut self, initial_conditions: Vec<InitialValue<S, E>>) {
        for initial_value in initial_conditions {
            let idx = self
                .mapping
                .get(&initial_value.reference)
                .expect("Initial value references an invalid symbol");
            self.values[*idx] = initial_value.value;
        }
    }

    pub fn assign(&mut self, values: &ArrayView1<E>, mapping: &HashMap<S, usize>) {
        for (symbol, idx_foreign) in mapping {
            let idx = self.mapping.get(symbol).expect("Symbol not found");
            let value = values[*idx_foreign];

            self.values[*idx] = value;
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymbolicVector2<S: Symbol, E: Field> {
    pub values: Array2<E>,
    pub mapping: HashMap<S, usize>,
    pub current_index: usize,
    pub filled: usize,
    pub capacity: usize,
}

impl<S: Symbol, E: Field> SymbolicVector2<S, E> {
    pub fn new(symbolic_mapping: HashMap<S, usize>, capacity: usize) -> Self {
        let var_count = symbolic_mapping.len();
        Self {
            values: Array2::zeros((capacity, var_count)),
            mapping: symbolic_mapping,
            current_index: 0,
            filled: 0,
            capacity,
        }
    }

    pub fn push(&mut self, vector: &SymbolicVector1<S, E>) {
        debug_assert_eq!(
            vector.values.len(),
            self.values.ncols(),
            "Dimension mismatch"
        );
        self.push_raw(vector.values.view());
    }

    pub fn pop(&mut self) -> SymbolicVector1<S, E> {
        SymbolicVector1::from_values(
            self.pop_raw()
                .expect("The Vector doesn't have enough items to pop")
                .to_owned(),
            self.mapping.clone(),
        )
    }

    pub fn push_raw(&mut self, values: ArrayView1<E>) {
        let write_idx = if self.filled == 0 {
            0
        } else {
            (self.current_index + 1) % self.capacity
        };

        self.values.row_mut(write_idx).assign(&values);
        self.current_index = write_idx;

        if self.filled < self.capacity {
            self.filled += 1;
        }
    }

    pub fn pop_raw(&mut self) -> Option<ArrayView1<E>> {
        let index = self
            .get_physical_index(0)
            .expect("To pop a new Vector, you must have capacity of at least 1.");

        if self.filled == 0 {
            return None;
        }

        self.current_index = (self.current_index - 1 + self.capacity) % self.capacity;
        self.filled -= 1;

        Some(self.values.row(index))
    }

    fn get_physical_index(&self, steps_back: usize) -> Option<usize> {
        if steps_back >= self.filled {
            return None;
        }

        Some((self.current_index + self.capacity - steps_back) % self.capacity)
    }

    pub fn latest(&self) -> Option<SymbolicVectorView<'_, S, E>> {
        self.view(0)
    }

    pub fn latest_mut(&mut self) -> Option<SymbolicVectorViewMut<'_, S, E>> {
        self.view_mut(0)
    }

    pub fn previous(&self) -> Option<SymbolicVectorView<'_, S, E>> {
        self.view(1)
    }

    pub fn view(&self, steps_back: usize) -> Option<SymbolicVectorView<'_, S, E>> {
        let phys_idx = self.get_physical_index(steps_back)?;
        Some(SymbolicVectorView {
            values: self.values.row(phys_idx),
            mapping: &self.mapping,
        })
    }

    pub fn view_mut(&mut self, steps_back: usize) -> Option<SymbolicVectorViewMut<'_, S, E>> {
        let phys_idx = self.get_physical_index(steps_back)?;

        Some(SymbolicVectorViewMut {
            values: self.values.row_mut(phys_idx),
            mapping: &self.mapping,
        })
    }
}

impl<S: Symbol, E: Field> Index<usize> for SymbolicVector2<S, E> {
    type Output = [E];

    fn index(&self, steps_back: usize) -> &Self::Output {
        let phys_idx = self
            .get_physical_index(steps_back)
            .expect("SymbolicVector2: History index out of bounds");

        self.values
            .row(phys_idx)
            .to_slice()
            .expect("Memory non-contiguous")
    }
}

impl<S: Symbol, E: Field + DifferentiableIndependentScalar> SymbolicVector2<S, E> {
    pub fn integration_parameters(&self, dx_symbol: &S) -> Option<(E, Array1<E>)> {
        let dx_idx = *self.mapping.get(dx_symbol)?;

        let order = self.filled.saturating_sub(1).min(3);

        let mut time_points = Vec::with_capacity(order + 1);

        for i in 0..=order {
            let phys_idx = self.get_physical_index(i)?;
            let t_val = self.values[[phys_idx, dx_idx]];
            time_points.push(t_val);
        }

        if time_points[0] <= time_points[1] {
            return None;
        }

        let coeffs = BdfCoefficientGenerator::generate(order, time_points)?;

        let mut history_sum = Array1::<E>::zeros(self.values.ncols());

        for (i, &beta) in coeffs.history_coeffs.iter().enumerate() {
            let steps_back = i + 1;

            let view = self.view(steps_back)?;

            Zip::from(&mut history_sum)
                .and(&view.values)
                .for_each(|sum, &val| {
                    *sum += val * E::from(beta);
                });
        }

        Some((coeffs.alpha, history_sum))
    }
}

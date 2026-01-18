use crate::math::Symbol;
use crate::math::deriv::{BdfCoefficientGenerator, DifferentiableIndependentScalar};
use crate::math::iv::InitialValue;
use crate::math::num::Field;
use ndarray::{Array1, Array2, ArrayView1, ArrayViewMut1, Zip};
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone)]
pub struct IndexedArray1<S: Symbol, E: Field> {
    pub values: Array1<E>,
    pub mapping: HashMap<S, usize>,
}

impl<S: Symbol, E: Field> IndexedArray1<S, E> {
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

    pub fn from_iv(
        initial_values: Vec<InitialValue<S, E>>,
        mapping: HashMap<S, usize>,
    ) -> IndexedArray1<S, E> {
        let mut val = IndexedArray1::new(mapping);
        val.apply_initial_values(initial_values);
        val
    }

    pub fn view(&self) -> IndexedArrayView1<'_, S, E> {
        IndexedArrayView1 {
            values: self.values.view(),
            mapping: &self.mapping,
        }
    }

    pub fn view_mut(&mut self) -> IndexedArrayViewMut1<'_, S, E> {
        IndexedArrayViewMut1 {
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

impl<S: Symbol, E: Field> Index<&S> for IndexedArray1<S, E> {
    type Output = E;
    fn index(&self, symbol: &S) -> &Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &self.values[*idx]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedArrayView1<'a, S: Symbol, E: Field> {
    pub values: ArrayView1<'a, E>,
    pub mapping: &'a HashMap<S, usize>,
}

impl<'a, S: Symbol, E: Field> IndexedArrayView1<'a, S, E> {
    pub fn get(&self, symbol: &S) -> Option<&E> {
        self.mapping
            .get(symbol)
            .and_then(|&idx| self.values.get(idx))
    }

    pub fn to_owned(&self) -> IndexedArray1<S, E> {
        IndexedArray1 {
            values: self.values.to_owned(),
            mapping: self.mapping.clone(),
        }
    }
}

impl<'a, S: Symbol, E: Field> Index<&S> for IndexedArrayView1<'a, S, E> {
    type Output = E;
    fn index(&self, symbol: &S) -> &Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &self.values[*idx]
    }
}

#[derive(Debug)]
pub struct IndexedArrayViewMut1<'a, S: Symbol, E: Field> {
    pub values: ArrayViewMut1<'a, E>,
    pub mapping: &'a HashMap<S, usize>,
}

impl<'a, S: Symbol, E: Field> Index<&S> for IndexedArrayViewMut1<'a, S, E> {
    type Output = E;
    fn index(&self, symbol: &S) -> &Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &self.values[*idx]
    }
}

impl<'a, S: Symbol, E: Field> IndexMut<&S> for IndexedArrayViewMut1<'a, S, E> {
    fn index_mut(&mut self, symbol: &S) -> &mut Self::Output {
        let idx = self.mapping.get(symbol).expect("Symbol not found");
        &mut self.values[*idx]
    }
}

impl<'a, S: Symbol, E: Field> IndexedArrayViewMut1<'a, S, E> {
    pub fn get(&self, symbol: &S) -> Option<&E> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get(idx)
    }

    pub fn get_mut(&mut self, symbol: &S) -> Option<&mut E> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get_mut(idx)
    }

    pub fn set(&mut self, symbol: &S, value: &E) {
        let idx = *self.mapping.get(symbol).expect("Symbol not found");
        self.values[idx] = *value;
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
pub struct IndexedArray2<S: Symbol, E: Field> {
    pub values: Array2<E>,
    pub mapping: HashMap<S, usize>,
    pub current_index: usize,
    pub filled: usize,
    pub capacity: usize,
}

impl<S: Symbol, E: Field> IndexedArray2<S, E> {
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

    pub fn push_align(&mut self, vector: &IndexedArray1<S, E>) {
        let write_idx = if self.filled == 0 {
            0
        } else {
            (self.current_index + 1) % self.capacity
        };

        self.values.row_mut(write_idx).fill(E::zero());

        for (symbol, &source_idx) in &vector.mapping {
            if let Some(&target_idx) = self.mapping.get(symbol) {
                self.values.row_mut(write_idx)[target_idx] = vector.values[source_idx];
            }
        }

        self.current_index = write_idx;
        if self.filled < self.capacity {
            self.filled += 1;
        }
    }

    pub fn push(&mut self, vector: &IndexedArray1<S, E>) {
        debug_assert_eq!(
            vector.values.len(),
            self.values.ncols(),
            "Dimension mismatch"
        );
        self.push_raw(vector.values.view());
    }

    pub fn pop(&mut self) -> IndexedArray1<S, E> {
        IndexedArray1::from_values(
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

    pub fn pop_raw(&mut self) -> Option<ArrayView1<'_, E>> {
        if self.filled == 0 {
            return None;
        }

        let remove_idx = self.current_index;

        self.current_index = (self.current_index + self.capacity - 1) % self.capacity;
        self.filled -= 1;

        Some(self.values.row(remove_idx))
    }

    fn get_physical_index(&self, steps_back: usize) -> Option<usize> {
        if steps_back >= self.filled {
            return None;
        }

        Some((self.current_index + self.capacity - steps_back) % self.capacity)
    }

    pub fn latest(&self) -> Option<IndexedArrayView1<'_, S, E>> {
        self.view(0)
    }

    pub fn latest_mut(&mut self) -> Option<IndexedArrayViewMut1<'_, S, E>> {
        self.view_mut(0)
    }

    pub fn previous(&self) -> Option<IndexedArrayView1<'_, S, E>> {
        self.view(1)
    }

    pub fn view(&self, steps_back: usize) -> Option<IndexedArrayView1<'_, S, E>> {
        let phys_idx = self.get_physical_index(steps_back)?;
        Some(IndexedArrayView1 {
            values: self.values.row(phys_idx),
            mapping: &self.mapping,
        })
    }

    pub fn view_mut(&mut self, steps_back: usize) -> Option<IndexedArrayViewMut1<'_, S, E>> {
        let phys_idx = self.get_physical_index(steps_back)?;

        Some(IndexedArrayViewMut1 {
            values: self.values.row_mut(phys_idx),
            mapping: &self.mapping,
        })
    }
}

impl<S: Symbol, E: Field> Index<usize> for IndexedArray2<S, E> {
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

impl<S: Symbol, E: Field + DifferentiableIndependentScalar> IndexedArray2<S, E> {
    pub fn integration_parameters(&self, dx_symbol: &S) -> Option<(E, Array1<E>)> {
        let dx_idx = *self.mapping.get(dx_symbol)?;
        let order = self.filled.saturating_sub(1).min(3).max(0);
        let mut time_points = Vec::with_capacity(order + 1);
        
        for i in 0..=order {
            let phys_idx = self.get_physical_index(i)?;
            let t_val = self.values[[phys_idx, dx_idx]];
            time_points.push(t_val);
        }

        if time_points.len() > 1 && time_points[0] <= time_points[1] {
            return None;
        }

        time_points.reverse();

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

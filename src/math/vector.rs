use crate::math::Symbol;
use crate::math::array::{IndexedArray1, IndexedArrayView1};
use crate::math::num::Field;
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct IndexedVec1<S: Symbol, E: Field> {
    pub values: Vec<Array1<E>>,
    pub mapping: HashMap<S, usize>,
}

impl<S: Symbol, E: Field> IndexedVec1<S, E> {
    pub fn new(mapping: HashMap<S, usize>) -> Self {
        Self {
            values: vec![],
            mapping,
        }
    }

    pub fn get(&self, symbol: &S) -> Option<ArrayView1<E>> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get(idx).map(|arr| arr.view())
    }

    pub fn get_mut(&mut self, symbol: &S) -> Option<ArrayViewMut1<E>> {
        let idx = *self.mapping.get(symbol)?;
        self.values.get_mut(idx).map(|arr| arr.view_mut())
    }

    pub fn push(&mut self, vector: &IndexedArray1<S, E>) {
        let mut new_value = Array1::zeros(vector.values.len());

        for symbol in vector.mapping.keys() {
            let idx = self.mapping.get(symbol).expect("Symbol not found");
            let value = vector.get(symbol).expect("Symbol not found");

            new_value[*idx] = *value;
        }

        self.values.push(new_value);
    }

    pub fn push_view(&mut self, vector: &IndexedArrayView1<S, E>) {
        let mut new_value = Array1::zeros(vector.values.len());

        for symbol in vector.mapping.keys() {
            let idx = self.mapping.get(symbol).expect("Symbol not found");
            let value = vector.get(symbol).expect("Symbol not found");

            new_value[*idx] = *value;
        }

        self.values.push(new_value);
    }

    pub fn push_raw(&mut self, arr: &ArrayView1<E>) {
        assert_eq!(arr.len(), self.mapping.len());

        self.values.push(arr.to_owned());
    }

    pub fn pop(&mut self) -> IndexedArray1<S, E> {
        let array = self.values.pop().expect("No vectors to pop");

        IndexedArray1::from_values(array, self.mapping.clone())
    }
}

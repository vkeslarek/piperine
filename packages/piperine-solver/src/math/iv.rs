use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::AsIndex;
use crate::math::num::Scalar;
use ndarray::Array1;

#[derive(Clone)]
pub struct InitialValue<A: AsIndex, E: Scalar> {
    pub reference: A,
    pub value: E,
}

pub trait InitialValueApplyExt<A: AsIndex, E: Scalar> {
    fn apply_iv(&mut self, initial_values: Vec<InitialValue<A, E>>);
}

impl<A: AsIndex, E: Scalar> InitialValueApplyExt<A, E> for CircularArrayBuffer2<E> {
    fn apply_iv(&mut self, initial_values: Vec<InitialValue<A, E>>) {
        self.push(&Array1::zeros(self.size()).view());

        if let Some(mut row) = self.latest_mut() {
            for iv in initial_values {
                if let Some(idx) = iv.reference.as_index() {
                    if let Some(cell) = row.get_mut(idx) {
                        *cell = iv.value;
                    } else {
                        panic!(
                            "Initial Value index {} is out of bounds for buffer size {}",
                            idx,
                            self.size()
                        );
                    }
                }
            }
        }
    }
}

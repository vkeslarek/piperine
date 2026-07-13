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
        // Overlay the initial values on the latest state (e.g. user `ic`
        // on top of the DC point) rather than zeroing unrelated nodes.
        let base = self
            .latest()
            .map(|row| row.to_owned())
            .unwrap_or_else(|| Array1::zeros(self.size()));
        self.push(&base.view());

        let size = self.size();
        if let Some(mut row) = self.latest_mut() {
            for iv in initial_values {
                if let Some(idx) = iv.reference.as_index() {
                    // Indices out of bounds here mean the circuit builder passed
                    // an initial value referencing a variable the buffer does
                    // not know about — a programming invariant, not a runtime
                    // user-input error. `debug_assert!` keeps the loud failure
                    // in tests and dev builds while staying out of the way in
                    // release (AGENTS.md: never panic on user input).
                    debug_assert!(
                        idx < size,
                        "Initial Value index {} is out of bounds for buffer size {}",
                        idx,
                        size
                    );
                    if let Some(cell) = row.get_mut(idx) {
                        *cell = iv.value;
                    }
                }
            }
        }
    }
}

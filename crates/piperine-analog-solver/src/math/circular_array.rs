//! `CircularArrayBuffer2` — a fixed-capacity circular history of solution
//! rows: the integration scheme's time window with zero hot-path
//! allocation.
use crate::math::iv::InitialValue;
use crate::math::linear::AsIndex;
use crate::math::num::Scalar;
use ndarray::{Array1, Array2, ArrayView1, ArrayViewMut1};
use num_traits::Zero;

#[derive(Debug, Clone)]
pub struct CircularArrayBuffer2<V> {
    buffer: Array2<V>,
    cursor: usize,
    count: usize,
}

impl<V: Zero + Clone> CircularArrayBuffer2<V> {
    pub fn new(capacity: usize, size: usize) -> Self {
        assert!(capacity > 0, "Capacity must be non-zero");
        Self {
            buffer: Array2::zeros((capacity, size)),
            cursor: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, values: &ArrayView1<V>) {
        assert_eq!(
            values.len(),
            self.buffer.ncols(),
            "Push size does not match number of variables"
        );

        let mut row = self.buffer.row_mut(self.cursor);
        row.assign(values);

        self.cursor = (self.cursor + 1) % self.buffer.nrows();

        if self.count < self.buffer.nrows() {
            self.count += 1;
        }
    }

    pub fn view(&self, lookback: usize) -> Option<ArrayView1<'_, V>> {
        if lookback > self.count {
            None
        } else {
            let idx = self.get_physical_index(lookback);
            Some(self.buffer.row(idx?))
        }
    }

    pub fn view_mut(&mut self, lookback: usize) -> Option<ArrayViewMut1<'_, V>> {
        let idx = self.get_physical_index(lookback);
        Some(self.buffer.row_mut(idx?))
    }

    pub fn latest(&self) -> Option<ArrayView1<'_, V>> {
        if self.count == 0 { None } else { self.view(0) }
    }

    pub fn latest_mut(&mut self) -> Option<ArrayViewMut1<'_, V>> {
        if self.count == 0 {
            None
        } else {
            self.view_mut(0)
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn size(&self) -> usize {
        self.buffer.ncols()
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn get_physical_index(&self, lookback: usize) -> Option<usize> {
        if lookback >= self.count || self.count == 0 {
            None
        } else {
            let capacity = self.buffer.nrows();
            // Safe: lookback < count, and cursor is always < capacity
            Some((self.cursor + capacity - 1 - lookback) % capacity)
        }
    }
}

impl<V: Scalar> CircularArrayBuffer2<V> {
    /// Overlays `initial_values` on the latest row of the history, pushed as a new row.
    pub fn apply_iv<A: AsIndex>(&mut self, initial_values: Vec<InitialValue<A, V>>) {
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

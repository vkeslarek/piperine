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

    pub fn view(&self, lookback: usize) -> Option<ArrayView1<V>> {
        if lookback > self.count {
            None
        } else {
            let idx = self.get_physical_index(lookback);
            Some(self.buffer.row(idx?))
        }
    }

    pub fn view_mut(&mut self, lookback: usize) -> Option<ArrayViewMut1<V>> {
        let idx = self.get_physical_index(lookback);
        Some(self.buffer.row_mut(idx?))
    }

    pub fn latest(&self) -> Option<ArrayView1<V>> {
        if self.count == 0 { None } else { self.view(0) }
    }

    pub fn latest_mut(&mut self) -> Option<ArrayViewMut1<V>> {
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
        if lookback > self.count {
            None
        } else {
            let capacity = self.buffer.nrows();
            Some((self.cursor + capacity - 1 - lookback) % capacity)
        }
    }
}

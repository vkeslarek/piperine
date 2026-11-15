//! A transient result read at other times: ngspice's print grid is not the solver's step grid, so
//! a parity test evaluates the solver's waveform at the exported times by linear interpolation.

/// The values of one quantity at the solver's accepted steps.
#[derive(Clone, Debug)]
pub struct Waveform {
    times: Vec<f64>,
    values: Vec<f64>,
}

impl Waveform {
    /// A waveform over strictly increasing `times`. Panics when the lengths differ or a time does
    /// not increase: a malformed result is a harness failure, not a value to compare.
    pub fn new(times: Vec<f64>, values: Vec<f64>) -> Self {
        assert_eq!(times.len(), values.len(), "a waveform holds one value per time");
        assert!(times.windows(2).all(|pair| pair[1] > pair[0]), "the times of a waveform increase strictly");
        Waveform { times, values }
    }

    /// The value at `t`, linear between the two bracketing steps; `None` outside the first and
    /// last time of the result (nothing is extrapolated).
    pub fn at(&self, t: f64) -> Option<f64> {
        let (first, last) = (*self.times.first()?, *self.times.last()?);
        if t < first || t > last {
            return None;
        }
        let upper = self.times.partition_point(|time| *time < t);
        if self.times[upper] == t {
            return Some(self.values[upper]);
        }
        let (t0, t1) = (self.times[upper - 1], self.times[upper]);
        let (v0, v1) = (self.values[upper - 1], self.values[upper]);
        Some(v0 + (v1 - v0) * (t - t0) / (t1 - t0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_outside_the_result_is_none() {
        let waveform = Waveform::new(vec![0.0, 1.0, 3.0], vec![0.0, 10.0, 30.0]);
        assert_eq!(waveform.at(-1e-12), None);
        assert_eq!(waveform.at(3.0 + 1e-12), None);
        assert_eq!(waveform.at(0.0), Some(0.0));
        assert_eq!(waveform.at(3.0), Some(30.0));
        assert_eq!(waveform.at(0.5), Some(5.0));
        assert_eq!(waveform.at(2.0), Some(20.0));
    }
}

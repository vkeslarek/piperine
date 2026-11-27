//! The frequency grid of an `ac` or a `noise` run.

/// The frequency grid of an `ac` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    /// Logarithmic, `n` points per decade (ngspice `dec`).
    PerDecade(u32),
    /// Linear, `n` points in all.
    Points(u32),
}

impl Grid {
    /// The number of frequencies from `from` to `to`: `round(n * log10(to / from)) + 1` per
    /// decade (CLI-09 AC 4), `n` in all (AC 6).
    pub fn steps(&self, from: f64, to: f64) -> usize {
        match self {
            Grid::PerDecade(n) => (f64::from(*n) * (to / from).log10()).round() as usize + 1,
            Grid::Points(n) => *n as usize,
        }
    }
}

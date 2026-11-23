//! `TableModel`: the interpolant of a `table_model` (§22.2.15, PI-10, Appendix C.2 rule on the end
//! conditions of the splines). The coefficients of every segment are computed once, when the table is
//! built; `value` and `slope` only pick the segment and evaluate. The slope of the interpolant at a
//! sample is the slope of the segment on its right, at the last sample that of the segment on its left.
//! An `x` outside the table under extrapolation `E` is a panic coded `E0705`.

use super::control::Panic;

/// The interpolation of a table: the closest sample, a line, a quadratic or a cubic spline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Degree {
    /// `D`.
    Closest,
    /// `1`.
    Linear,
    /// `2`.
    Quadratic,
    /// `3`.
    Cubic,
}

/// What a table does outside its samples at one end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extrapolation {
    /// `C`: the end value.
    Constant,
    /// `L`: the line of the end slope.
    Linear,
    /// `E`: `E0705`.
    Error,
}

/// A table of `(abscissa, value)` samples with strictly increasing abscissae, and its interpolant.
#[derive(Debug)]
pub struct TableModel {
    /// The abscissae.
    xs: Vec<f64>,
    /// The values.
    ys: Vec<f64>,
    /// The interpolation.
    degree: Degree,
    /// The behavior below the first abscissa.
    low: Extrapolation,
    /// The behavior above the last abscissa.
    high: Extrapolation,
    /// Per segment `[x_i, x_{i+1}]`, the polynomial `a + b·t + c·t² + d·t³` in `t = x − x_i`.
    segments: Vec<[f64; 4]>,
}

impl TableModel {
    /// The model of `samples` under `control` (the text the elaborator checked: `D`, `1`, `2` or `3`, then
    /// at most two of `C`, `L`, `E`; none is `L` at both ends, one applies to both).
    pub fn new(samples: &[(f64, f64)], control: &str) -> TableModel {
        let mut chars = control.chars();
        let degree = match chars.next() {
            Some('D') => Degree::Closest,
            Some('2') => Degree::Quadratic,
            Some('3') => Degree::Cubic,
            _ => Degree::Linear,
        };
        let ends: Vec<Extrapolation> = chars.map(|c| match c { 'C' => Extrapolation::Constant, 'E' => Extrapolation::Error, _ => Extrapolation::Linear }).collect();
        let (low, high) = match ends.as_slice() {
            [] => (Extrapolation::Linear, Extrapolation::Linear),
            [both] => (*both, *both),
            [low, high, ..] => (*low, *high),
        };
        let xs: Vec<f64> = samples.iter().map(|sample| sample.0).collect();
        let ys: Vec<f64> = samples.iter().map(|sample| sample.1).collect();
        let segments = match degree {
            Degree::Closest => Vec::new(),
            Degree::Linear => Self::lines(&xs, &ys),
            Degree::Quadratic => Self::parabolas(&xs, &ys),
            Degree::Cubic => Self::not_a_knot(&xs, &ys),
        };
        TableModel { xs, ys, degree, low, high, segments }
    }

    /// One line per segment.
    fn lines(xs: &[f64], ys: &[f64]) -> Vec<[f64; 4]> {
        (0..xs.len() - 1).map(|i| [ys[i], (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]), 0.0, 0.0]).collect()
    }

    /// The quadratic spline: the first segment is the parabola through the first three samples; each later segment
    /// takes the value and the slope of the one before at their shared sample and passes through the next.
    fn parabolas(xs: &[f64], ys: &[f64]) -> Vec<[f64; 4]> {
        let (h0, h1) = (xs[1] - xs[0], xs[2] - xs[1]);
        let (d0, d1) = ((ys[1] - ys[0]) / h0, (ys[2] - ys[1]) / h1);
        let curvature = (d1 - d0) / (h0 + h1);
        let mut segments = vec![[ys[0], d0 - curvature * h0, curvature, 0.0]];
        for i in 1..xs.len() - 1 {
            let h = xs[i + 1] - xs[i];
            let previous = segments[i - 1];
            let h_before = xs[i] - xs[i - 1];
            let slope = previous[1] + 2.0 * previous[2] * h_before;
            segments.push([ys[i], slope, ((ys[i + 1] - ys[i]) / h - slope) / h, 0.0]);
        }
        segments
    }

    /// The cubic spline with the not-a-knot end conditions: the third derivative is continuous at the second and the
    /// last-but-one sample. The second derivatives `m` satisfy a tridiagonal system once the end ones are eliminated.
    fn not_a_knot(xs: &[f64], ys: &[f64]) -> Vec<[f64; 4]> {
        let n = xs.len();
        let h: Vec<f64> = xs.windows(2).map(|pair| pair[1] - pair[0]).collect();
        let delta: Vec<f64> = (0..n - 1).map(|i| (ys[i + 1] - ys[i]) / h[i]).collect();
        // unknowns m[1..n-1): row i is h[i-1]·m[i-1] + 2(h[i-1]+h[i])·m[i] + h[i]·m[i+1] = 6(delta[i] − delta[i−1])
        let unknowns = n - 2;
        let (mut lower, mut diagonal, mut upper, mut right) = (vec![0.0; unknowns], vec![0.0; unknowns], vec![0.0; unknowns], vec![0.0; unknowns]);
        for k in 0..unknowns {
            let i = k + 1;
            lower[k] = h[i - 1];
            diagonal[k] = 2.0 * (h[i - 1] + h[i]);
            upper[k] = h[i];
            right[k] = 6.0 * (delta[i] - delta[i - 1]);
        }
        // not-a-knot at the low end: m[0] = m[1](1 + h0/h1) − m[2]·h0/h1; at the high end likewise
        diagonal[0] += h[0] * (1.0 + h[0] / h[1]);
        upper[0] -= h[0] * h[0] / h[1];
        let last = unknowns - 1;
        let (last_width, before_width) = (h[n - 2], h[n - 3]);
        diagonal[last] += last_width * (1.0 + last_width / before_width);
        lower[last] -= last_width * last_width / before_width;
        // Thomas elimination
        for k in 1..unknowns {
            let factor = lower[k] / diagonal[k - 1];
            diagonal[k] -= factor * upper[k - 1];
            right[k] -= factor * right[k - 1];
        }
        let mut inner = vec![0.0; unknowns];
        inner[last] = right[last] / diagonal[last];
        for k in (0..last).rev() {
            inner[k] = (right[k] - upper[k] * inner[k + 1]) / diagonal[k];
        }
        let mut m = vec![0.0; n];
        m[1..n - 1].copy_from_slice(&inner);
        m[0] = m[1] * (1.0 + h[0] / h[1]) - m[2] * h[0] / h[1];
        m[n - 1] = m[n - 2] * (1.0 + last_width / before_width) - m[n - 3] * last_width / before_width;
        (0..n - 1).map(|i| [ys[i], delta[i] - h[i] * (2.0 * m[i] + m[i + 1]) / 6.0, m[i] / 2.0, (m[i + 1] - m[i]) / (6.0 * h[i])]).collect()
    }

    /// The segment `x` falls in, the last for `x` at the last abscissa.
    fn segment_of(&self, x: f64) -> usize {
        self.xs.partition_point(|abscissa| *abscissa <= x).saturating_sub(1).min(self.xs.len() - 2)
    }

    /// The value of the interpolant at `x`; outside the table the extrapolation of that end decides (`E0705` for `E`).
    pub fn value(&self, x: f64) -> f64 {
        let last = self.xs.len() - 1;
        if x < self.xs[0] {
            return match self.low {
                Extrapolation::Constant => self.ys[0],
                Extrapolation::Linear => self.ys[0] + self.slope_inside(0) * (x - self.xs[0]),
                Extrapolation::Error => self.outside(x),
            };
        }
        if x > self.xs[last] {
            return match self.high {
                Extrapolation::Constant => self.ys[last],
                Extrapolation::Linear => self.ys[last] + self.slope_inside(last) * (x - self.xs[last]),
                Extrapolation::Error => self.outside(x),
            };
        }
        match self.degree {
            Degree::Closest => self.closest(x),
            _ => {
                let i = self.segment_of(x);
                let [a, b, c, d] = self.segments[i];
                let t = x - self.xs[i];
                a + t * (b + t * (c + t * d))
            }
        }
    }

    /// The derivative of the interpolant at `x`: the segment on the right of a sample, on the left at the last one;
    /// outside the table `0` for `C`, the end slope for `L`, `E0705` for `E`.
    pub fn slope(&self, x: f64) -> f64 {
        let last = self.xs.len() - 1;
        if x < self.xs[0] {
            return match self.low {
                Extrapolation::Constant => 0.0,
                Extrapolation::Linear => self.slope_inside(0),
                Extrapolation::Error => self.outside(x),
            };
        }
        if x > self.xs[last] {
            return match self.high {
                Extrapolation::Constant => 0.0,
                Extrapolation::Linear => self.slope_inside(last),
                Extrapolation::Error => self.outside(x),
            };
        }
        self.slope_at(x)
    }

    /// The slope at an `x` inside the table.
    fn slope_at(&self, x: f64) -> f64 {
        if self.degree == Degree::Closest {
            return 0.0;
        }
        let i = self.segment_of(x);
        let [_, b, c, d] = self.segments[i];
        let t = x - self.xs[i];
        b + t * (2.0 * c + 3.0 * d * t)
    }

    /// The slope at the sample `index` (the end slope of an extrapolation `L`).
    fn slope_inside(&self, index: usize) -> f64 {
        self.slope_at(self.xs[index])
    }

    /// The sample closest to `x`, the lower one on a tie.
    fn closest(&self, x: f64) -> f64 {
        let i = self.segment_of(x);
        if x - self.xs[i] <= self.xs[i + 1] - x { self.ys[i] } else { self.ys[i + 1] }
    }

    /// An `x` outside the table under extrapolation `E`: the panic coded `E0705` (§22.2.15 rule 6).
    fn outside(&self, x: f64) -> f64 {
        Panic::raise(format!("E0705: `table_model` was called at x = {x:e}, outside its table [{:e}, {:e}] under extrapolation `E`", self.xs[0], self.xs[self.xs.len() - 1]))
    }
}

use num_complex::Complex;

pub trait SolverNumber {}

impl SolverNumber for f64 {}

impl SolverNumber for Complex<f64> {}

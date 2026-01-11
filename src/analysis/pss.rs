use crate::circuit::netlist::CircuitReference;
use ndarray::Array1;
use std::collections::HashMap;

pub struct PssAnalysisOptions {
    pub period: f64,         // T
    pub dt: f64,             // Transient step size
    pub max_pss_iter: usize, // Newton iterations for shooting
    pub pss_reltol: f64,
    pub t_stab: f64,
}

#[derive(Debug)]
pub struct PssAnalysisResult {
    pub values: Array1<f64>,
    pub mapping: HashMap<CircuitReference, usize>,
}

use crate::circuit::netlist::CircuitReference;
use crate::math::unit::{Ohm, Siemens, UnitExt};
use ndarray::ArrayView1;
use std::collections::HashMap;

pub mod ac;
pub mod dc;
pub mod noise;
pub mod transient;

#[derive(Debug, Clone)]
pub struct Context {
    pub gmin: Siemens,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub max_iter: usize,
    pub min_res: Ohm,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            gmin: 1.0.pS(),
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            max_iter: 500,
            min_res: 1.0.uOhms(),
        }
    }
}
impl Context {
    pub fn has_converged(
        &self,
        old_values: &ArrayView1<f64>,
        new_values: &ArrayView1<f64>,
        mapping: &HashMap<CircuitReference, usize>,
    ) -> bool {
        mapping.iter().all(|(reference, &index)| {
            if index >= old_values.len() || index >= new_values.len() {
                return true;
            }

            let old_v = old_values[index];
            let new_v = new_values[index];

            let abs_limit = if matches!(reference, CircuitReference::Branch(_)) {
                self.abstol
            } else {
                self.vntol
            };

            let magnitude = old_v.abs().max(new_v.abs());
            let allowed_error = self.reltol * magnitude + abs_limit;
            let diff = (new_v - old_v).abs();

            diff <= allowed_error
        })
    }
}

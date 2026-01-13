use crate::circuit::netlist::CircuitReference;
use crate::math::deriv::DifferentiableIndependentScalar;
use crate::math::faer::FaerToNdarray;
use crate::math::linear::{SparseLinearSystem, SymbolicMatrix};
use crate::math::num::{Field, ScalableByReal};
use crate::math::unit::{Conductance, Resistance, UnitExt};
use faer::traits::ComplexField;
use ndarray::ArrayView1;
use num_traits::real::Real;
use std::collections::HashMap;
use std::hash::Hash;
use crate::math::Symbol;

pub mod ac;
pub mod dc;
pub mod pss;
pub mod transient;
pub mod noise;

#[derive(Debug, Clone)]
pub struct Context {
    pub gmin: Conductance,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub max_iter: usize,
    pub min_res: Resistance,
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
                return false;
            }

            let old_v = old_values[index];
            let new_v = new_values[index];

            let abs_limit = if matches!(reference, CircuitReference::Branch(_)) {
                self.abstol // Current (Amps)
            } else {
                self.vntol // Voltage (Volts)
            };

            let magnitude = old_v.abs().max(new_v.abs());
            let allowed_error = self.reltol * magnitude + abs_limit;
            let diff = (new_v - old_v).abs();

            diff <= allowed_error
        })
    }
}

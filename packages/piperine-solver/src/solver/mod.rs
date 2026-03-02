use crate::circuit::netlist::Netlist;
use crate::math::unit::{Ohm, Siemens, UnitExt};
use faer::{set_global_parallelism, Par};
use ndarray::ArrayView1;
use std::num::NonZeroUsize;
use std::sync::Once;

pub mod ac;
pub mod dc;
pub mod noise;
pub mod transient;

static INIT: Once = Once::new();

pub fn init_solver_configuration() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_thread_ids(true)
            .with_thread_names(true)
            .init();

        set_global_parallelism(Par::Rayon(NonZeroUsize::new(1).unwrap()));
    });
}

#[derive(Debug, Clone)]
pub struct Context {
    pub gmin: Siemens,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub time: f64,
    pub max_iter: usize,
    pub min_res: Ohm,
    pub dc_damp_tolerance: f64,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            gmin: 1.0.pS(),
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            time: 0.0,
            max_iter: 500,
            min_res: 1e-12,
            dc_damp_tolerance: 0.5,
        }
    }
}

impl Context {
    pub fn has_converged(
        &self,
        old_values_opt: Option<ArrayView1<f64>>,
        new_values: &ArrayView1<f64>,
        netlist: &Netlist,
    ) -> bool {
        if old_values_opt.is_none() {
            return false;
        }

        let old_values = old_values_opt.unwrap();

        netlist
            .all_references()
            .iter()
            .filter(|s| s.idx().is_some())
            .all(|reference| {
                let index = reference.idx().unwrap();

                if index >= old_values.len() || index >= new_values.len() {
                    return true;
                }

                let old_v = old_values[index];
                let new_v = new_values[index];

                let abs_limit = if reference.is_branch() {
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

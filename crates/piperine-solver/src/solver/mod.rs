use crate::analog::Netlist;
use crate::core::element::Element;
use crate::math::unit::{Ohm, Siemens};
use faer::{Par, set_global_parallelism};
use ndarray::{ArrayView1, ArrayViewMut1};
use std::num::NonZeroUsize;
use std::sync::Once;

pub mod ac;
pub mod convergence;
pub mod dc;
pub mod noise;
pub mod tf;
pub mod transient;

static INIT: Once = Once::new();

pub(crate) fn check_convergence(
    devices: &[Box<dyn Element>],
    state: &crate::math::circular_array::CircularArrayBuffer2<f64>,
    new_guess: &ArrayView1<f64>,
    context: &Context,
    netlist: &Netlist,
) -> bool {
    for device in devices {
        if device.limiting_active() {
            return false;
        }
    }
    context.has_converged(state.view(0), new_guess, netlist)
}

/// ngspice `NIconvTest`: every node's current imbalance (and every branch
/// row's equation residual) must be within tolerance. Node rows use the
/// current tolerance `abstol`, branch rows the voltage tolerance `vntol`;
/// both add the relative term `reltol · scale`. Shared by DC and transient.
pub(crate) fn residual_converged(
    netlist: &Netlist,
    context: &Context,
    residual: &[f64],
    scale: &[f64],
) -> bool {
    use crate::math::linear::AsIndex;
    for r in netlist.all_references() {
        let Some(i) = r.as_index() else { continue };
        if i >= residual.len() {
            continue;
        }
        let abs_limit = if r.variable().is_branch() { context.vntol } else { context.abstol };
        let tol = abs_limit + context.reltol * scale[i];
        if residual[i].abs() > tol {
            return false;
        }
    }
    true
}

pub(crate) fn apply_damping(
    state: &crate::math::circular_array::CircularArrayBuffer2<f64>,
    mut current_guess: ArrayViewMut1<f64>,
    dc_damp_tolerance: f64,
) {
    let last_guess = match state.latest() {
        Some(guess) => guess,
        None => return,
    };
    let diff_norm_sq: f64 = current_guess
        .iter()
        .zip(last_guess.iter())
        .fold(0.0, |acc, (curr, prev)| acc + (curr - prev).powi(2));
    let diff_norm = diff_norm_sq.sqrt();
    if diff_norm >= dc_damp_tolerance {
        for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
            *curr = (*curr + *prev) * 0.5;
        }
    }
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
    /// Truncation error tolerance for adaptive timestep (default: 7.0)
    pub trtol: f64,
    /// Charge tolerance in Coulombs for truncation error (default: 1e-14)
    pub chgtol: f64,
    pub temperature: f64,
    pub tnom: f64,
    /// Transient integration method for the reactive companion model. Default
    /// **Gear order 2** (BDF2): 2nd-order accurate *and* strongly stable —
    /// it damps the numerical ringing that trapezoidal shows on stiff/LC
    /// circuits, at the cost of a little extra artificial damping. Order ramps
    /// 1 → 2 over the first steps.
    pub integration: crate::analysis::truncation::IntegrationMethod,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            gmin: 1e-12,
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            time: 0.0,
            max_iter: 500,
            min_res: 1e-12,
            dc_damp_tolerance: 0.5,
            trtol: 7.0,
            chgtol: 1e-14,
            temperature: 300.15,
            tnom: 300.15,
            integration: crate::analysis::truncation::IntegrationMethod::Gear { order: 2 },
        }
    }
}

impl Context {
    pub fn init_global() {
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_thread_ids(true)
                .with_thread_names(true)
                .init();

            set_global_parallelism(Par::Rayon(NonZeroUsize::new(1).unwrap()));
        });
    }

    pub fn has_converged(
        &self,
        old_values_opt: Option<ArrayView1<f64>>,
        new_values: &ArrayView1<f64>,
        netlist: &Netlist,
    ) -> bool {
        let Some(old_values) = old_values_opt else { return false; };

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

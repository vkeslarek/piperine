use crate::analog::Netlist;
use crate::math::integration::IntegrationMethod;
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

// ── Tolerances (immutable, Copy) ───────────────────────────────────────────

/// Immutable per-run numerical tolerances. `Copy`. Shared across every analysis
/// through `Context`. Extracted from the old flat `Context` fields (MD-04).
#[derive(Debug, Clone, Copy)]
pub struct Tolerances {
    pub gmin: Siemens,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub min_res: Ohm,
    /// Truncation error tolerance for adaptive timestep (default: 7.0)
    pub trtol: f64,
    /// Charge tolerance in Coulombs for truncation error (default: 1e-14)
    pub chgtol: f64,
    pub temperature: f64,
    pub tnom: f64,
    pub integration: IntegrationMethod,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            gmin: 1e-12,
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            min_res: 1e-12,
            trtol: 7.0,
            chgtol: 1e-14,
            temperature: 300.15,
            tnom: 300.15,
            integration: IntegrationMethod::Gear { order: 2 },
        }
    }
}

impl Tolerances {
    /// The convergence test that used to be `Context::has_converged` — moved
    /// here because it only reads tolerance fields. Same logic, same output.
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

    /// ngspice `NIconvTest`: every node's current imbalance (and every branch
    /// row's equation residual) must be within tolerance.
    pub fn residual_test(
        &self,
        netlist: &Netlist,
        residual: &[f64],
        scale: &[f64],
    ) -> bool {
        use crate::math::linear::AsIndex;
        for r in netlist.all_references() {
            let Some(i) = r.as_index() else { continue };
            if i >= residual.len() {
                continue;
            }
            let abs_limit = if r.variable().is_branch() { self.abstol } else { self.vntol };
            let tol = abs_limit + self.reltol * scale[i];
            if residual[i].abs() > tol {
                return false;
            }
        }
        true
    }
}

// ── Policy (mutable, owned by drivers/plan) ────────────────────────────────

/// Mutable per-run state that used to be flat fields on `Context`. Owned by
/// the driver or `ConvergencePlan`, never by the shared `Context` (MD-04).
#[derive(Debug, Clone)]
pub struct Policy {
    pub time: f64,
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            time: 0.0,
            max_iter: 500,
            dc_damp_tolerance: 0.5,
        }
    }
}

impl Policy {
    /// Damp a Newton update: halve it in-place when the vector norm exceeds
    /// `self.dc_damp_tolerance`. Body moved from the old free fn `apply_damping`.
    pub fn damp_update(
        &self,
        state: &crate::math::circular_array::CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
    ) {
        let Some(last_guess) = state.latest() else { return };
        let diff_norm_sq: f64 = current_guess
            .iter()
            .zip(last_guess.iter())
            .fold(0.0, |acc, (curr, prev)| acc + (curr - prev).powi(2));
        let diff_norm = diff_norm_sq.sqrt();
        if diff_norm >= self.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }
        }
    }
}

// ── Context (shared, immutable) ────────────────────────────────────────────

/// The shared context every analysis receives. Carries only `Tolerances`.
/// Mutable plan state lives on `Policy`, owned by the driver.
/// `time`, `dc_damp_tolerance`, and `max_iter` are temporary — they will move
/// to `Policy` once `NewtonStrategy` is wired and can provide them through the
/// plan (T5).
#[derive(Debug, Clone)]
pub struct Context {
    pub tolerances: Tolerances,
    pub time: f64,
    pub dc_damp_tolerance: f64,
    pub max_iter: usize,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            tolerances: Tolerances::default(),
            time: 0.0,
            dc_damp_tolerance: 0.5,
            max_iter: 500,
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
}

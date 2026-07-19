//! The analyses layer (design §1/§2 — Scheme B): one module per analysis,
//! each holding both its request/state types (what element and host
//! exchange) and its driver (how it runs). Shared here: the run
//! configuration every driver speaks — `Context` (immutable `Tolerances`)
//! and `Policy` (per-analysis convergence tunables, MD-04) — plus the
//! `Solver` host entry that hands out the analyses. The config home lives
//! in `config.rs`, the Newton/homotopy/stepper machinery in
//! `convergence.rs`. A driver may call down into `analog`, `digital`,
//! `math`, and read config — never sideways into another analysis, never
//! up into the host.

use crate::analog::Netlist;
use crate::analyses::config::TraceFlags;
use faer::{Par, set_global_parallelism};
use ndarray::ArrayView1;
use std::num::NonZeroUsize;
use std::sync::Once;

pub mod ac;
pub mod config;
pub mod convergence;
pub mod dc;
pub mod noise;
pub mod pss;
pub mod pz;
pub mod sens;
pub mod sp;
pub mod tf;
pub mod transient;

static INIT: Once = Once::new();

// ── Tolerances (immutable, Copy) ───────────────────────────────────────────

/// Immutable per-run numerical tolerances. `Copy`. Shared across every analysis
/// through `Context`. Extracted from the old flat `Context` fields (MD-04).
#[derive(Debug, Clone, Copy)]
pub struct Tolerances {
    pub gmin: f64,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub min_res: f64,
    /// Truncation error tolerance for adaptive timestep (default: 7.0)
    pub trtol: f64,
    /// Charge tolerance in Coulombs for truncation error (default: 1e-14)
    pub chgtol: f64,
    pub temperature: f64,
    pub tnom: f64,
    /// Circuit-wide diagonal conductance to ground on every node (default 0).
    /// Helps convergence on floating/poorly-damped topologies (ngspice gshunt).
    pub gshunt: f64,
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
            gshunt: 0.0,
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

/// Convergence tunables the Newton loop consults each solve. Owned by the
/// driver (each analysis solver carries its own), never by the shared
/// immutable `Context` (MD-04). Hosts configure it per analysis.
#[derive(Debug, Clone)]
pub struct Policy {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
    /// Diagnostic trace toggles (SS-08). `Default` seeds them from the
    /// `PIPERINE_TRACE_{GMIN,SRC,TRAN}` env vars — the single env read left;
    /// every trace site reads these typed fields.
    pub trace: TraceFlags,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_iter: 500,
            dc_damp_tolerance: 0.5,
            trace: TraceFlags::from_env(),
        }
    }
}

// ── Context (shared, immutable) ────────────────────────────────────────────

/// The shared context every analysis receives: only the immutable
/// [`Tolerances`]. Mutable convergence state lives on [`Policy`], owned by
/// the driver; simulation time reaches elements through their analysis
/// context or as an explicit argument (MD-04).
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub tolerances: Tolerances,
}

impl Context {
    pub fn init_global() {
        INIT.call_once(|| {
            // Keep piperine's own diagnostics at INFO, but silence the Cranelift
            // JIT backend (it logs every compiled function's CL IR at INFO via
            // the `log` crate, which drowns real output). `cranelift*=off` keeps
            // the log-compat shim on for any other crate without the IR spam.
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::new("info,cranelift_jit=off,cranelift_codegen=off"),
                )
                .with_thread_ids(true)
                .with_thread_names(true)
                .init();

            set_global_parallelism(Par::Rayon(NonZeroUsize::new(1).unwrap()));
        });
    }
}

// ── host entry ───────────────────────────────────────────────────────────

/// The host entry point: owns the circuit + run configuration, initializes
/// the process globals once (MD-06), and hands out the five analyses.
pub struct Solver {
    circuit: crate::core::circuit::CircuitInstance,
    context: Context,
    policy: Policy,
    tran_opts: transient::TransientAnalysisOptions,
}

impl Solver {
    pub fn new(circuit: crate::core::circuit::CircuitInstance) -> Self {
        Self {
            circuit,
            context: Context::default(),
            policy: Policy::default(),
            tran_opts: transient::TransientAnalysisOptions::new(1e-3, 1e-6),
        }
    }

    pub fn with_context(mut self, ctx: Context) -> Self {
        self.context = ctx;
        self
    }

    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_tran_opts(mut self, opts: transient::TransientAnalysisOptions) -> Self {
        self.tran_opts = opts;
        self
    }

    /// Initializes process globals (tracing, faer) via `Context::init_global`
    /// — guarded by `Once`, so repeated builds are free. Returns self (moves on).
    pub fn build(self) -> Self {
        Context::init_global();
        self
    }

    pub fn dc(&mut self) -> crate::result::Result<dc::DcSolver<'_>> {
        let mut solver = self.circuit.dc(self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn tran(&mut self) -> crate::result::Result<transient::TransientSolver<'_>> {
        let mut solver = self.circuit.transient(self.tran_opts.clone(), self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn ac(&mut self) -> crate::result::Result<ac::AcSolver<'_>> {
        let mut solver = self.circuit.ac(self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn noise(&mut self, opts: noise::NoiseAnalysisOptions) -> crate::result::Result<noise::NoiseSolver<'_>> {
        self.circuit.noise(opts, self.context.clone())
    }

    pub fn tf(&mut self, opts: tf::TransferFunctionAnalysisOptions) -> crate::result::Result<tf::TransferFunctionSolver<'_>> {
        self.circuit.transfer_function(opts, self.context.clone())
    }

    pub fn circuit(&self) -> &crate::core::circuit::CircuitInstance {
        &self.circuit
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

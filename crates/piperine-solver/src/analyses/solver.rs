//! The host entry point into the analyses layer: `Solver` owns the circuit and
//! the run configuration (`Context`/`Policy`/transient options) and hands out
//! the analyses, initializing the process globals once on `build` (MD-06).

use crate::analyses::context::{Context, Policy};
use crate::analyses::{ac, dc, noise, tf, transient};

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

    pub fn dc(&mut self) -> crate::core::result::Result<dc::DcSolver<'_>> {
        let mut solver = self.circuit.dc(self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn tran(&mut self) -> crate::core::result::Result<transient::TransientSolver<'_>> {
        let mut solver = self.circuit.transient(self.tran_opts.clone(), self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn ac(&mut self) -> crate::core::result::Result<ac::AcSolver<'_>> {
        let mut solver = self.circuit.ac(self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn noise(&mut self, opts: noise::NoiseAnalysisOptions) -> crate::core::result::Result<noise::NoiseSolver<'_>> {
        self.circuit.noise(opts, self.context.clone())
    }

    pub fn tf(&mut self, opts: tf::TransferFunctionAnalysisOptions) -> crate::core::result::Result<tf::TransferFunctionSolver<'_>> {
        self.circuit.transfer_function(opts, self.context.clone())
    }

    pub fn circuit(&self) -> &crate::core::circuit::CircuitInstance {
        &self.circuit
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

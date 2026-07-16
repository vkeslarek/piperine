use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::tf::TransferFunctionAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::core::circuit::CircuitInstance;
use crate::result::Result;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::tf::TransferFunctionSolver;
use crate::solver::transient::TransientSolver;
use crate::solver::{Context, Policy};

/// The host entry point: owns the circuit + run configuration, initializes
/// the process globals once (MD-06), and hands out the five analyses.
pub struct Solver {
    circuit: CircuitInstance,
    context: Context,
    policy: Policy,
    tran_opts: TransientAnalysisOptions,
}

impl Solver {
    pub fn new(circuit: CircuitInstance) -> Self {
        Self {
            circuit,
            context: Context::default(),
            policy: Policy::default(),
            tran_opts: TransientAnalysisOptions::new(1e-3, 1e-6),
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

    pub fn with_tran_opts(mut self, opts: TransientAnalysisOptions) -> Self {
        self.tran_opts = opts;
        self
    }

    /// Initializes process globals (tracing, faer) via `Context::init_global`
    /// — guarded by `Once`, so repeated builds are free. Returns self (moves on).
    pub fn build(self) -> Self {
        Context::init_global();
        self
    }

    pub fn dc(&mut self) -> Result<DcSolver<'_>> {
        let mut solver = self.circuit.dc(self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn tran(&mut self) -> Result<TransientSolver<'_>> {
        let mut solver = self.circuit.transient(self.tran_opts.clone(), self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn ac(&mut self) -> Result<AcSolver<'_>> {
        let mut solver = self.circuit.ac(self.context.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn noise(&mut self, opts: NoiseAnalysisOptions) -> Result<NoiseSolver<'_>> {
        self.circuit.noise(opts, self.context.clone())
    }

    pub fn tf(&mut self, opts: TransferFunctionAnalysisOptions) -> Result<TransferFunctionSolver<'_>> {
        self.circuit.transfer_function(opts, self.context.clone())
    }

    pub fn circuit(&self) -> &CircuitInstance {
        &self.circuit
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

//! The host entry point into the analyses layer: `Solver` owns the circuit and
//! the run configuration (`Context`/`Policy`/transient options) and hands out
//! the analyses, initializing the process globals once on `build` (MD-06).

use crate::analyses::Context;
use piperine_analog_solver::tolerances::Policy;
use crate::analyses::{ac, dc, noise, tf, transient};

// ── host entry ───────────────────────────────────────────────────────────

/// The host entry point: owns the circuit + run configuration, initializes
/// the process globals once (MD-06), and hands out the five analyses.
pub struct Solver {
    circuit: crate::core::circuit::CircuitInstance,
    context: Context,
    policy: Policy,
    tran_opts: transient::TransientAnalysisOptions,
    host: crate::analyses::adapters::HostHooks,
}

impl Solver {
    pub fn new(circuit: crate::core::circuit::CircuitInstance) -> Self {
        Self {
            circuit,
            context: Context::default(),
            policy: Policy::default(),
            tran_opts: transient::TransientAnalysisOptions::new(1e-3, 1e-6),
            host: Default::default(),
        }
    }

    pub fn with_context(mut self, ctx: Context) -> Self {
        self.context = ctx;
        self
    }

    /// Kept for a host that fixes the policy of a solver (`tests/checkpoint_restore.rs`).
    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_tran_opts(mut self, opts: transient::TransientAnalysisOptions) -> Self {
        self.tran_opts = opts;
        self
    }

    /// Sends every observation of the analyses of a circuit of devices to
    /// `sink` (abi.md C5, A5): changes, commits, fired crossings, records and
    /// traces, in instant order, each with its path, analysis kind and time.
    pub fn with_sink(mut self, sink: Box<dyn crate::protocol::Sink>) -> Self {
        self.host.sink = Some(std::rc::Rc::new(std::cell::RefCell::new(sink)));
        self
    }

    /// Hands every accepted step of a transient of devices to `observer`, by reference, once the
    /// sink has every observation up to its time; the start is observed too. A run that keeps
    /// nothing per step (`Keep::Last`) is read through it.
    pub fn with_step_observer(mut self, observer: Box<dyn crate::protocol::StepObserver>) -> Self {
        self.host.observer = Some(std::rc::Rc::new(std::cell::RefCell::new(observer)));
        self
    }

    /// Calls `waiter` once each time the event net `net` fires at an accepted
    /// instant of an analysis of a circuit of devices (abi.md §5 R7): the
    /// owner lands there, and what the waiter writes is due at the next
    /// microstep.
    pub fn on_event(&mut self, net: &str, waiter: crate::protocol::Waiter) -> &mut Self {
        self.host.waiters.push((net.to_string(), std::rc::Rc::new(std::cell::RefCell::new(waiter))));
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
        solver.host = self.host.clone();
        Ok(solver)
    }

    pub fn tran(&mut self) -> crate::core::result::Result<transient::TransientSolver<'_>> {
        let mut solver = self.circuit.transient(self.tran_opts.clone(), self.context.clone())?;
        solver.policy = self.policy.clone();
        solver.host = self.host.clone();
        Ok(solver)
    }

    pub fn ac(&mut self) -> crate::core::result::Result<ac::AcSolver<'_>> {
        let mut solver = ac::AcSolver::observed_by(&mut self.circuit, self.context.clone(), self.host.clone())?;
        solver.policy = self.policy.clone();
        Ok(solver)
    }

    pub fn noise(&mut self, opts: noise::NoiseAnalysisOptions) -> crate::core::result::Result<noise::NoiseSolver<'_>> {
        noise::NoiseSolver::observed_by(&mut self.circuit, opts, self.context.clone(), self.host.clone())
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

//! DC operating point (`.op`): the Newton driver (`DcSystem`/`DcSolver`) over
//! the device assembly, with gmin/source-stepping homotopy escalation, and the
//! operating point of a mixed design through the owner protocol.
use piperine_analog_solver::device::{Ambient, AssemblySnapshot};
use piperine_analog_solver::netlist::AnalogReference;
use piperine_analog_solver::convergence::{ConvergencePlan, HomotopyDriver};
use crate::analyses::Context;
use piperine_analog_solver::tolerances::Policy;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::math::linear::Stamp;
use piperine_analog_solver::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::DcAnalysisResult;

use ndarray::ArrayView1;
use std::collections::HashMap;

// ── driver ───────────────────────────────────────────────────────────────

/// Non-linear system representation for DC analysis.
///
/// This structure implements the [`NonLinearSystem`] trait to enable Newton-Raphson
/// iteration for DC operating point calculation. It manages circuit state updates,
/// convergence checking, damping, and Safe Operating Area (SOA) violation detection.
pub struct DcSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    /// Extra node-to-ground conductance for **gmin stepping** (SPICE homotopy):
    /// 0 in normal operation, ramped large → 0 by [`DcSolver::solve_gmin_stepping`]
    /// so each intermediate problem is diagonally dominant. Owned here, not in
    /// the shared immutable `Context`.
    pub gmin_extra: f64,
    /// Forced-source scale for **source stepping** (SPICE homotopy): 1.0 in
    /// normal operation, ramped 0 → 1 by [`DcSolver::solve_source_stepping`].
    /// Read by the devices as the `SOURCE_SCALE` ambient input.
    pub src_scale: f64,
    /// UIC hold clamps (ngspice `CKTsetIC`): stamped only by the transient
    /// driver's internal t=0 solve — empty for a standalone DC analysis.
    pub uic_clamps: Vec<crate::analyses::transient::UicClamp>,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for DcSystem<'a> {
    /// Assembles the system matrix and RHS vector for DC analysis from the
    /// device assembly. Called by the Newton-Raphson solver at each iteration.
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        self.assemble_device(state)
    }

    fn netlist(&self) -> &piperine_analog_solver::netlist::Netlist {
        self.circuit.netlist()
    }

    fn any_limiting_report(&self) -> bool {
        self.circuit.abi.as_ref().is_some_and(|bound| !bound.analog.limiting_active().is_empty())
    }

    /// Every Newton iteration but the first ends here: each limit's `LAST`
    /// becomes the `x̂` of the iteration just assembled (abi.md A2).
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<f64>, iteration_number: usize) {
        if iteration_number > 0
            && let Some(bound) = self.circuit.abi.as_mut()
        {
            bound.analog.end_iteration();
        }
    }

    /// Called after successful convergence.
    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        _: &ArrayView1<f64>,
    ) {
        // The converged iteration ends like the others.
        if let Some(bound) = self.circuit.abi.as_mut() {
            bound.analog.end_iteration();
        }
    }
}

impl DcSystem<'_> {
    /// The stamps of an equilibrium solve of a circuit of devices (abi.md §10):
    /// the assembly's `FLOW`/`DELAY` rows at the iterate, with the homotopy's
    /// continuation values as the `GMIN` and `SOURCE_SCALE` ambient inputs,
    /// then the solver's own shunts and the hold clamps. There is no stamp
    /// cache: a function whose reads did not change is not called (F3).
    fn assemble_device(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        let x: Vec<f64> = match state.latest() {
            Some(row) => row.to_vec(),
            None => vec![0.0; state.size()],
        };
        let amb = self.ambient();
        let bound = self.circuit.abi.as_mut().expect("a circuit of devices");
        let mut all_stamps = bound.analog.assemble_dc(&x, amb)?;
        if let Some(err) = piperine_analog_solver::stamp_validation::non_finite_stamp_error("devices", &all_stamps) {
            return Err(err);
        }
        self.push_solver_stamps(&mut all_stamps);
        Ok(all_stamps)
    }

    /// The coordinates of an equilibrium solve (abi.md §4.2): `STATIC`, with
    /// the homotopy's continuation values as `GMIN` and `SOURCE_SCALE`.
    pub(crate) fn ambient(&self) -> Ambient {
        Ambient {
            time: 0.0,
            freq: 0.0,
            is_static: true,
            gmin: self.context.tolerances.gmin + self.gmin_extra,
            source_scale: self.src_scale,
        }
    }

    /// What the solver adds to every circuit: the gmin-stepping shunt, the
    /// user's `gshunt` and the hold clamps.
    fn push_solver_stamps(&self, all_stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        if self.gmin_extra > 0.0 {
            let g = self.gmin_extra;
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), g));
                }
            }
        }
        let gshunt = self.context.tolerances.gshunt;
        if gshunt > 0.0 {
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), gshunt));
                }
            }
        }
        for clamp in &self.uic_clamps {
            clamp.stamp(all_stamps);
        }
    }
}

pub struct DcSolver<'a> {
    pub system: DcSystem<'a>,
    pub solver: NewtonRaphsonSolver<AnalogReference, f64, FaerSparseLinearSystem<f64>>,
    /// Convergence tunables for this analysis (MD-04). Defaults on
    /// construction; hosts override before [`solve`](Self::solve).
    pub policy: Policy,
    /// How many plain-Newton attempts the convergence plan drove (1 = no
    /// homotopy). `SolverStats::homotopy_levels` is this minus the first.
    newton_calls: usize,
    /// One-deep checkpoint (ABI-07), taken before each homotopy attempt and
    /// restored on strategy fallthrough: the assembly's term histories and
    /// limit states start clean on the next retry.
    assembly_checkpoint: Option<AssemblySnapshot>,
    /// The homotopy that converged the first equilibrium of a circuit of devices.
    homotopy_strategy: Option<&'static str>,
    /// The host's sink and waiters, for a circuit of devices.
    pub(crate) host: crate::analyses::adapters::HostHooks,
    /// The analysis global (`global::DC`, `TRAN`, `AC` or `NOISE`) the operating point's
    /// digital side carries: the analysis the operating point is for.
    analysis: u32,
}

impl<'a> DcSolver<'a> {
    pub fn new(circuit: &'a mut CircuitInstance, mut context: Context) -> crate::core::result::Result<Self> {
        Context::init_global();
        if let Some(bound) = &circuit.abi {
            bound.tighten(&mut context.tolerances);
        }
        circuit.setup_all(&context)?;
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = DcSystem {
            circuit,
            context,
            gmin_extra: 0.0,
            src_scale: 1.0,
            uic_clamps: Vec::new(),
        };

        let solver = match NewtonRaphsonSolver::new(&mut system, size, 1) {
            Ok(solver) => solver,
            Err(error) => return Err(system.circuit.name_error(error)),
        };

        Ok(Self {
            system,
            solver,
            policy: Policy::default(),
            newton_calls: 0,
            assembly_checkpoint: None,
            homotopy_strategy: None,
            host: Default::default(),
            analysis: piperine_abi::raw::global::DC,
        })
    }

    /// The operating point of another analysis (`global::AC` or `global::NOISE`): the same solve,
    /// its digital side carrying the caller's analysis global instead of `DC` (§11.1.1, FIX-11).
    pub fn under(circuit: &'a mut CircuitInstance, context: Context, analysis: u32) -> crate::core::result::Result<Self> {
        let mut dc = Self::new(circuit, context)?;
        dc.analysis = analysis;
        Ok(dc)
    }

    /// Seed the DC Newton initial guess with node-voltage hints (the host
    /// §5.1 `OpConfig.nodeset`). Applied before [`solve`](Self::solve); the
    /// solver still converges to the operating point — this only changes the
    /// starting guess, useful for nonlinear circuits with multiple solutions.
    pub fn apply_initial_conditions(&mut self, ivs: Vec<InitialValue<AnalogReference, f64>>) {
        if !ivs.is_empty() {
            self.solver.push_initial_conditions(ivs);
        }
    }

    /// One equilibrium solve of a circuit of devices: the first runs the
    /// convergence plan (plain Newton, then the homotopies), a later round
    /// re-solves from the last solution with damped Newton.
    pub(crate) fn equilibrium(&mut self, first: bool) -> crate::core::result::Result<ndarray::Array1<f64>> {
        if first {
            let plan = ConvergencePlan::default().with_trace(self.policy.trace);
            let outcome = plan.solve(self)?;
            self.homotopy_strategy = outcome.strategy;
            return Ok(outcome.solution);
        }
        let strategy = piperine_analog_solver::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances.clone();
        self.solver.solve_with_strategy(&mut self.system, &strategy, &tolerances, &policy)
    }

    /// The operating point of a circuit of devices, through the protocol
    /// (abi.md §10): the digital engine leaves the circuit for the run and
    /// comes back after it, whatever the outcome.
    pub fn solve(&mut self) -> crate::core::result::Result<DcAnalysisResult> {
        use crate::analyses::adapters::{DigitalSide, EquilibriumSide, empty_digital};
        use crate::protocol::{Protocol, ProtocolOptions};
        self.solver.reset_iteration_counter();
        self.newton_calls = 0;
        let bound = self.system.circuit.abi.as_mut().expect("a circuit of devices");
        let spt = bound.digital.plan().resolution.seconds_per_tick;
        let mut digital = std::mem::replace(&mut bound.digital, empty_digital(spt)?);
        // Reactions and discrete functions read the continuation values at rest: every
        // instant runs after the last solve of its sequence (abi.md §4.2).
        digital.set_continuation(self.system.context.tolerances.gmin, 1.0);
        let side = DigitalSide::new(digital, bound, self.analysis);
        let hooks = self.host.clone();
        let nets = hooks.nets(bound);
        let protocol =
            Protocol::new(side, EquilibriumSide { dc: self, solution: None }, ProtocolOptions::new(spt));
        let mut protocol = hooks.install(protocol, nets.as_deref().unwrap_or(&[]));
        let ran = nets.and_then(|_| protocol.operating_point());
        let (side, analog) = protocol.into_sides();
        let solution = analog.solution;
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            bound.digital = side.into_digital();
        }
        ran.map_err(|error| self.system.circuit.name_error(error))?;
        // A design with no continuous pin runs its instants with no solve.
        let mut values = HashMap::new();
        if let Some(solution) = solution {
            for reference in self.system.circuit.netlist().all_references() {
                if let Some(i) = reference.idx() {
                    values.insert(reference.variable().clone(), solution[i]);
                }
            }
        }
        let mut result = DcAnalysisResult::new(values);
        result.stats.newton_iterations = self.solver.total_iterations();
        result.stats.converged = true;
        result.stats.homotopy_strategy = self.homotopy_strategy.map(str::to_string);
        result.stats.homotopy_levels = self.newton_calls.saturating_sub(1);
        result.stats.assembly_time_ns = self.solver.assembly_time_ns();
        result.stats.solve_time_ns = self.solver.solve_time_ns();
        Ok(result)
    }

}

impl HomotopyDriver for DcSolver<'_> {
    fn newton(&mut self) -> crate::core::result::Result<ndarray::Array1<f64>> {
        self.newton_calls += 1;
        let strategy = piperine_analog_solver::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances.clone();
        self.solver.solve_with_strategy(
            &mut self.system,
            &strategy,
            &tolerances,
            &policy,
        )
    }

    fn set_gmin_extra(&mut self, g: f64) {
        self.system.gmin_extra = g;
    }

    fn set_src_scale(&mut self, s: f64) {
        self.system.src_scale = s;
    }

    fn gmin_floor(&self) -> f64 {
        self.system.context.tolerances.gmin.max(1e-12)
    }

    fn checkpoint_devices(&mut self) {
        if let Some(bound) = &self.system.circuit.abi {
            self.assembly_checkpoint = Some(bound.analog.snapshot());
        }
    }

    fn restore_devices(&mut self) {
        if let Some(bound) = self.system.circuit.abi.as_mut()
            && let Some(snapshot) = &self.assembly_checkpoint
        {
            bound.analog.restore(snapshot);
        }
    }
}

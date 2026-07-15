use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult,
    TransientAnalysisState, TransientStep,
};
use crate::core::circuit::CircuitInstance;
use crate::analog::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::integration::{TrBdf2, TrBdf2Phase};
use crate::math::iv::InitialValue;
use crate::math::linear::{AsIndex, Stamp};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::convergence::StepperStrategy;
use crate::solver::dc::DcSolver;
use crate::solver::Context;
use log::debug;
use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

pub struct TransientSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    /// Absolute time at the point this phase solves (t_{n+γ} for the TR stage,
    /// t_{n+1} for the BDF2 stage).
    pub time: f64,
    /// Which TR-BDF2 sub-step the next assemble stamps.
    pub phase: TrBdf2Phase,
    /// Full step size `h = t_{n+1} − t_n`.
    pub h: f64,
    /// Last accepted step size, so the TR stage can re-derive the previous
    /// capacitor current. Updated after each accepted BDF2 phase.
    pub prev_h: f64,
    pub tfinal: f64,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha_hint: f64,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        // TR-BDF2 is the sole integration scheme — no Gear order ramp. The
        // kernel derives the companion from `phase` + `h` via the centralised
        // `TrBdf2::phase_coeffs` (MD-07); the TR stage's previous-current
        // term is re-derived from `prev_h` inside the kernel.
        let tran_ctx = TransientAnalysisContext {
            time: self.time,
            tfinal: self.tfinal,
            phase: self.phase,
            h: self.h,
            prev_h: self.prev_h,
        };

        let mut all_stamps = Vec::new();

        self.context.time = self.time;
        self.circuit.update_all(state, &self.context);
        let CircuitInstance { devices, digital_state, .. } = &mut *self.circuit;
        let tran_state = TransientAnalysisState::new(state, &digital_state.nets);
        for tran in devices.iter_mut() {
            all_stamps.extend(tran.load_transient(&tran_state, &tran_ctx, &self.context));
        }
        Ok(all_stamps)
    }

    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        let netlist = self.circuit.netlist();
        { if self.circuit.devices.iter().any(|d| d.limiting_active()) { return false; } self.context.tolerances.has_converged(state.view(0), new_guess, netlist) }
    }

    fn residual_converged(&self, residual: &[f64], scale: &[f64]) -> bool {
        self.context.tolerances.residual_test(self.circuit.netlist(), residual, scale)
    }

    fn any_limiting(&self) -> bool {
        self.circuit.devices.iter().any(|d| d.limiting_active())
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        current_guess: ArrayViewMut1<f64>,
    ) {
        crate::solver::Policy::default().damp_update(state, current_guess);
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>) {}

    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        _: &ArrayView1<f64>,
    ) {
    }
}

pub struct TransientSolver<'a> {
    pub system: TransientSystem<'a>,
    pub solver: NewtonRaphsonSolver<AnalogReference, f64, FaerSparseLinearSystem<f64>>,
    pub options: TransientAnalysisOptions,
    /// User-supplied initial node voltages (piperine-bench/docs/SPEC.md §5.1
    /// `TranConfig.ic`), pushed after the DC operating point so the t=0
    /// state reflects them. Milestone-1: a seed (the companion model's
    /// first step may show a transient); full enforced-hold is deferred.
    initial_conditions: Vec<InitialValue<AnalogReference, f64>>,
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        Context::init_global();

        // Build DAG topology once before simulation begins
        circuit.rebuild_digital_topology();
        circuit.init_digital()?;

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            context,
            time: 0.0,
            phase: TrBdf2Phase::Trapezoidal,
            h: options.dt,
            prev_h: 0.0,
            tfinal: options.stop_time,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 4)?;

        Ok(Self {
            system,
            solver,
            options,
            initial_conditions: Vec::new(),
        })
    }

    /// Seed the transient's t=0 state with user initial node voltages
    /// (piperine-bench/docs/SPEC.md §5.1 `TranConfig.ic`). Applied after the DC operating
    /// point in `compute_initial_conditions`.
    pub fn apply_initial_conditions(&mut self, ivs: Vec<InitialValue<AnalogReference, f64>>) {
        self.initial_conditions = ivs;
    }

    fn compute_initial_conditions(&mut self) -> crate::result::Result<TransientStep> {
        debug!("Calculating DC Operating Point...");
        let mut dc_solver = DcSolver::new(self.system.circuit, Context::default())?;
        let dc_result = dc_solver.solve()?;

        let netlist = self.system.circuit.netlist();
        let iv_dc = dc_result.as_iv(netlist);

        // Element `@initial { V(p,n) <- ic }` UIC seeds: set the t=0 branch
        // voltage `v(plus) = v(minus) + ic` (cap/ind/dio initial condition,
        // SPICE `.ic`). Overlaid on the DC point so unconstrained nodes keep
        // their operating-point values.
        let mut device_ic: Vec<InitialValue<AnalogReference, f64>> = Vec::new();
        for dev in &self.system.circuit.devices {
            for (plus, minus, ic) in dev.initial_conditions() {
                let Some(plus_ref) = plus else { continue };
                let v_minus = minus
                    .as_ref()
                    .and_then(|m| m.as_index())
                    .map_or(0.0, |i| iv_dc.iter().find(|iv| iv.reference.as_index() == Some(i)).map_or(0.0, |iv| iv.value));
                device_ic.push(InitialValue { reference: plus_ref, value: v_minus + ic });
            }
        }

        self.solver.push_initial_conditions(iv_dc.clone());
        self.solver.push_initial_conditions(iv_dc);
        if !device_ic.is_empty() {
            self.solver.push_initial_conditions(device_ic.clone());
            self.solver.push_initial_conditions(device_ic);
        }
        // User `ic` seeds the t=0 state. Pushed twice so both rows of the
        // companion's history buffer see the ic values (avoids a
        // discontinuity that would spike the first transient step) —
        // milestone-1 seed; full enforced-hold is deferred.
        if !self.initial_conditions.is_empty() {
            self.solver.push_initial_conditions(self.initial_conditions.clone());
            self.solver.push_initial_conditions(self.initial_conditions.clone());
        }

        Ok(self.snapshot(0.0))
    }

    /// Advance one TR-BDF2 step over `[t_n, t_n + dt]`. Two Newton solves:
    /// phase 1 (Trapezoidal over `γ·dt`) → `x_{n+γ}`; phase 2 (BDF2 over
    /// `(1−γ)·dt`) → `x_{n+1}`, warm-started from `x_{n+γ}`. Either phase
    /// failing rejects the whole step (TRB-05). The Newton buffer's push
    /// semantics line the history up naturally: phase 1 reads `x_n` at
    /// `view(1)`; phase 2 reads `x_{n+γ}` at `view(1)` and `x_n` at `view(2)`.
    fn execute_timestep(
        &mut self,
        t_n: f64,
        dt: f64,
    ) -> crate::result::Result<Option<TransientStep>> {
        let strategy = crate::solver::convergence::DampedNewton;
        let policy = crate::solver::Policy::default();
        let tolerances = self.system.context.tolerances;
        // Borrow-checker workaround: the netlist lives behind `&mut system`
        // (through `circuit`), but `solve_with_strategy` borrows the system
        // mutably while only reading the netlist. The raw-pointer aliasing is
        // sound because the solve never mutates the netlist.
        let netlist = self.system.circuit.netlist() as *const crate::analog::Netlist;
        let netlist: &crate::analog::Netlist = unsafe { &*netlist };
        let t_next = t_n + dt;

        // Phase 1 — Trapezoidal over γ·dt → intermediate point x_{n+γ}.
        self.system.phase = TrBdf2Phase::Trapezoidal;
        self.system.h = dt;
        self.system.time = t_n + TrBdf2::GAMMA * dt;
        self.solver.solve_with_strategy(
            &mut self.system, &strategy, &tolerances, &policy, netlist,
        )?;

        // Phase 2 — BDF2 over (1−γ)·dt → final point x_{n+1} (warm-start x_{n+γ}).
        self.system.phase = TrBdf2Phase::Bdf2;
        self.system.h = dt;
        self.system.time = t_next;
        self.solver.solve_with_strategy(
            &mut self.system, &strategy, &tolerances, &policy, netlist,
        )?;

        // Step accepted: this dt becomes `prev_h` for the next step's TR-stage
        // previous-current re-derivation.
        self.system.prev_h = dt;
        Ok(Some(self.snapshot(t_next)))
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let stop_time: f64 = self.options.stop_time;
        let record_from: f64 = self.options.record_from;
        let mut dt: f64 = self.options.dt;
        let dt_min = self.options.dt_min;

        let initial_snapshot = self.compute_initial_conditions()?;
        let mut steps = Vec::new();
        // The t=0 DC operating point is only part of the recorded output when
        // recording starts at (or before) t=0; a delayed start drops it but
        // still computes it — the initial state seeds the integration.
        if 0.0 >= record_from {
            steps.push(initial_snapshot);
        }

        let mut current_time = 0.0;
        // Previous accepted step size, tracked locally for the LTE stepper
        // (TR-BDF2 has no Gear order ramp, so it no longer lives on the system).
        let mut dt_prev = 0.0;

        while current_time < stop_time {
            let dt_proposed = dt;
            
            let t_next_event = self.system.circuit.digital_state.peek_next_event_time();
            let mut t_next = (current_time + dt_proposed).min(stop_time);
            if t_next_event < t_next {
                t_next = t_next_event;
            }
            
            let dt_actual = t_next - current_time;
            
            // Checkpoint digital state
            self.system.circuit.digital_state.checkpoint();

            // Process digital events EXACTLY at t_next BEFORE analog solve.
            self.system.circuit.run_digital_at(t_next)?;

            // Solve the TR-BDF2 step [current_time, t_next] (two phases).
            let analog_result = self.execute_timestep(current_time, dt_actual);

            if let Ok(Some(snapshot)) = analog_result {
                // Post-convergence: run digital with the updated analog
                // voltages (A2D bridge).
                let solution = self.solver.current_guess().unwrap().to_owned();
                let _changed = self.system.circuit.accept_and_run_digital(
                    solution.as_slice().unwrap(),
                    &self.system.context,
                    t_next,
                )?;
                self.system.circuit.digital_state.commit();
                // A delayed-start transient still solves every step (state
                // evolution matters); only the recording is gated.
                if t_next >= record_from {
                    steps.push(snapshot);
                }
                current_time = t_next;

                // LTE-driven timestep via StepperStrategy (MD-05). Per-device
                // LTE here; the global Milne LTE + PI controller land next.
                let stepper = crate::solver::convergence::LteStepper;
                dt = stepper.propose_dt(
                    current_time,
                    dt_actual,
                    dt_proposed,
                    dt_prev,
                    self.system.circuit,
                    self.solver.state(),
                    &self.system.context.tolerances,
                    &self.options,
                );
                dt_prev = dt_actual;
            } else {
                // Rollback and retry with smaller step
                self.system.circuit.digital_state.rollback();
                let stepper = crate::solver::convergence::LteStepper;
                dt = stepper.reject_dt(dt_proposed, &self.options);
                
                if dt <= dt_min && let Err(e) = analog_result {
                    return Err(e);
                }
                continue;
            }
        }

        Ok(TransientAnalysisResult::new(
            steps,
        ))
    }

    fn snapshot(&self, time: f64) -> TransientStep {
        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();
        let latest_state = self.solver.current_guess().unwrap();

        for reference in netlist.all_references() {
            if let Some(idx) = reference.idx() {
                values.insert(reference.variable().clone(), latest_state[idx]);
            }
        }

        TransientStep::new(time, values)
            .with_digital(self.system.circuit.digital_state.nets.clone())
    }
}

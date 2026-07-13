use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult, TransientStep,
};
use crate::core::circuit::CircuitInstance;
use crate::analog::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::iv::InitialValue;
use crate::math::linear::{AsIndex, Stamp};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::dc::DcSolver;
use crate::solver::Context;
use log::debug;
use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

pub struct TransientSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub time: f64,
    pub dt: f64,
    pub tfinal: f64,
    /// Previous accepted step size, and how many steps have been accepted —
    /// together they set the usable BDF order (Gear ramps 1 → 2).
    pub dt_prev: f64,
    pub step_index: usize,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha_hint: f64,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        // Gear ramps order 1 → 2 as history accumulates: the first accepted
        // step has no `t_{n-2}` for BDF2, so it uses backward-Euler.
        let order = if self.step_index >= 2 && self.dt_prev > 0.0 {
            self.context.integration.order().min(2)
        } else {
            1
        };
        let tran_ctx = TransientAnalysisContext {
            time: self.time,
            dt: self.dt,
            tfinal: self.tfinal,
            dt_prev: self.dt_prev,
            order,
        };

        let mut all_stamps = Vec::new();

        self.context.time = self.time;
        self.circuit.update_all(state, &self.context);
        for tran in &mut self.circuit.devices {
            if let Some(a) = tran.as_analog() {
                all_stamps.extend(a.load_transient(state, &tran_ctx, &self.context));
            }
        }
        Ok(all_stamps)
    }

    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        let netlist = self.circuit.netlist();
        super::check_convergence(&self.circuit.devices, state, new_guess, &self.context, netlist)
    }

    fn residual_converged(&self, residual: &[f64], scale: &[f64]) -> bool {
        super::residual_converged(self.circuit.netlist(), &self.context, residual, scale)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        current_guess: ArrayViewMut1<f64>,
    ) {
        super::apply_damping(state, current_guess, self.context.dc_damp_tolerance);
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
        circuit.init_digital();

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            context,
            time: 0.0,
            dt: options.dt,
            tfinal: options.stop_time,
            dt_prev: 0.0,
            step_index: 0,
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

        // Device `@initial { V(p,n) <- ic }` UIC seeds: set the t=0 branch
        // voltage `v(plus) = v(minus) + ic` (cap/ind/dio initial condition,
        // SPICE `.ic`). Overlaid on the DC point so unconstrained nodes keep
        // their operating-point values.
        let mut device_ic: Vec<InitialValue<AnalogReference, f64>> = Vec::new();
        for dev in &self.system.circuit.devices {
            if let Some(a) = dev.as_analog_ref() {
                for (plus, minus, ic) in a.initial_conditions() {
                    let Some(plus_ref) = plus else { continue };
                    let v_minus = minus
                        .as_ref()
                        .and_then(|m| m.as_index())
                        .map_or(0.0, |i| iv_dc.iter().find(|iv| iv.reference.as_index() == Some(i)).map_or(0.0, |iv| iv.value));
                    device_ic.push(InitialValue { reference: plus_ref, value: v_minus + ic });
                }
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

    fn execute_timestep(
        &mut self,
        current_time: f64,
        dt: f64,
    ) -> crate::result::Result<Option<TransientStep>> {
        self.system.time = current_time;
        self.system.dt = dt;

        debug!(
            "Solving Transient Step: t = {:.6}s, dt = {:.3e}s",
            current_time, dt
        );

        let max_iter = self.system.context.max_iter;
        let result = self.solver.solve(&mut self.system, 1.0 / dt, max_iter);

        result.map(|_| Some(self.snapshot(current_time)))
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let stop_time: f64 = self.options.stop_time;
        let record_from: f64 = self.options.record_from;
        let mut dt: f64 = self.options.dt;
        let max_step: f64 = dt;

        let initial_snapshot = self.compute_initial_conditions()?;
        let mut steps = Vec::new();
        // The t=0 DC operating point is only part of the recorded output when
        // recording starts at (or before) t=0; a delayed start drops it but
        // still computes it — the initial state seeds the integration.
        if 0.0 >= record_from {
            steps.push(initial_snapshot);
        }

        let mut current_time = 0.0;
        let min_step = 1e-15;

        while current_time < stop_time {
            let dt_proposed = dt; // Stepper logic normally here
            
            let t_next_event = self.system.circuit.digital_state.peek_next_event_time();
            let mut t_next = (current_time + dt_proposed).min(stop_time);
            if t_next_event < t_next {
                t_next = t_next_event;
            }
            
            let dt_actual = t_next - current_time;
            
            // Checkpoint digital state
            self.system.circuit.digital_state.checkpoint();

            // Process digital events EXACTLY at t_next BEFORE analog solve.
            self.system.circuit.run_digital_at(t_next);

            // Solve analog timestep [current_time, t_next]
            let analog_result = self.execute_timestep(t_next, dt_actual);

            if let Ok(Some(snapshot)) = analog_result {
                // Post-convergence: run digital with the updated analog
                // voltages (A2D bridge). If digital outputs changed, the
                // D2A bridge may require re-solving, but for now we accept
                // the digital state as-is (one evaluation per timestep).
                let solution = self.solver.current_guess().unwrap().to_owned();
                let _changed = self.system.circuit.accept_and_run_digital(
                    solution.as_slice().unwrap(),
                    &self.system.context,
                    t_next,
                );
                self.system.circuit.digital_state.commit();
                // A delayed-start transient still solves every step (state
                // evolution matters); only the recording is gated.
                if t_next >= record_from {
                    steps.push(snapshot);
                }
                // Advance BDF history: this step's size becomes `dt_prev` and
                // the accepted-step count drives the order ramp (1 → 2).
                self.system.dt_prev = dt_actual;
                self.system.step_index += 1;
                current_time = t_next;
                // Use dt_proposed for growth so an event-clamped step doesn't shrink dt permanently
                dt = f64::min(f64::max(dt_proposed * 2.0, min_step), max_step);
            } else {
                // Rollback and retry with smaller step
                self.system.circuit.digital_state.rollback();
                // Scale from dt_proposed as well
                dt = f64::max(dt_proposed * 0.5, min_step);
                
                if dt <= min_step && let Err(e) = analog_result {
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

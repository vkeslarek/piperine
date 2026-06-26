use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult, TransientStep,
};
use crate::circuit::CircuitInstance;
use crate::analog::netlist::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::dc::DcSolver;
use crate::solver::{Context, init_solver_configuration};
use log::debug;
use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

pub struct TransientSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub time: f64,
    pub dt: f64,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha_hint: f64,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        let tran_ctx = TransientAnalysisContext {
            time: self.time.into(),
            dt: self.dt.into(),
        };

        let mut all_stamps = Vec::new();

        self.context.time = self.time;
        self.circuit.update_all(state, &self.context);
        for tran in self.circuit.all_runtimes_mut() {
            all_stamps.extend(tran.load_transient(state, &tran_ctx, &self.context));
        }
        Ok(all_stamps)
    }

    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        for runtime in self.circuit.all_runtimes() {
            if runtime.limiting_active() {
                debug!("Device {} requested limiting reiteration", runtime.device_name());
                return false;
            }
        }
        let netlist = self.circuit.netlist();
        self.context
            .has_converged(state.view(0), new_guess, netlist)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
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

        if diff_norm >= self.context.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }
        }
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
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        init_solver_configuration();

        // Build DAG topology once before simulation begins
        circuit.rebuild_digital_topology();

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            context,
            time: 0.0,
            dt: options.dt,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 4)?;

        Ok(Self {
            system,
            solver,
            options,
        })
    }

    fn compute_initial_conditions(&mut self) -> crate::result::Result<TransientStep> {
        debug!("Calculating DC Operating Point...");
        let mut dc_solver = DcSolver::new(self.system.circuit, Context::default())?;
        let dc_result = dc_solver.solve()?;

        let netlist = self.system.circuit.netlist();
        let iv_dc = dc_result.as_iv(netlist);

        self.solver.push_initial_conditions(iv_dc.clone());
        self.solver.push_initial_conditions(iv_dc);

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

        if result.is_ok() {
            let snapshot = self.snapshot(current_time);
            Ok(Some(snapshot))
        } else {
            Err(result.unwrap_err())
        }
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let stop_time: f64 = self.options.stop_time.into();
        let mut dt: f64 = self.options.dt.into();
        let max_step: f64 = dt;

        let initial_snapshot = self.compute_initial_conditions()?;
        let mut steps = vec![initial_snapshot];

        let mut current_time = 0.0;
        let min_step = 1e-15;

        while current_time < stop_time {
            let dt_proposed = dt; // Stepper logic normally here
            
            let t_next_event = self.system.circuit.digital_state.peek_next_event_time();
            // TODO: Breakpoint queue peek_next() would go here
            let mut t_next = (current_time + dt_proposed).min(stop_time);
            if t_next_event < t_next {
                t_next = t_next_event;
            }
            
            let dt_actual = t_next - current_time;
            
            // Checkpoint digital state
            self.system.circuit.digital_state.checkpoint();

            // Process digital events EXACTLY at t_next BEFORE analog solve.
            // Split borrow: digital_state/topology and digital_runtimes are separate fields.
            {
                let digital_state = &mut self.system.circuit.digital_state;
                let digital_runtimes = &mut self.system.circuit.digital_runtimes;
                let digital_topology = &self.system.circuit.digital_topology;
                let mut devices: Vec<&mut dyn crate::digital::state::DigitalDevice> =
                    digital_runtimes.iter_mut().map(|d| -> &mut dyn crate::digital::state::DigitalDevice { &mut **d }).collect();
                match digital_topology {
                    Some(topo) => digital_state.evaluate_dag_ordered(t_next, &mut devices, topo),
                    None => digital_state.evaluate_until_stable(t_next, &mut devices),
                }
            }

            // Solve analog timestep [current_time, t_next]
            let analog_result = self.execute_timestep(t_next, dt_actual);

            if let Ok(Some(snapshot)) = analog_result {
                // Post-convergence
                let _state = self.solver.current_guess().unwrap().clone();
                let _ctx = &self.system.context;
                let c = &mut self.system.circuit;
                for _runtime in c.all_runtimes_mut() {
                    // TODO: call runtime.accept_timestep() once we can access
                    // the state buffer and context from here.
                }

                self.system.circuit.digital_state.commit();
                steps.push(snapshot);
                current_time = t_next;
                // Use dt_proposed for growth so an event-clamped step doesn't shrink dt permanently
                dt = f64::min(f64::max(dt_proposed * 2.0, min_step), max_step);
            } else {
                // Rollback and retry with smaller step
                self.system.circuit.digital_state.rollback();
                // Scale from dt_proposed as well
                dt = f64::max(dt_proposed * 0.5, min_step);
                
                if dt <= min_step && analog_result.is_err() {
                    return Err(analog_result.unwrap_err());
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
    }
}

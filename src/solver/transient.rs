use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult,
};
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, CircuitVariable, GND};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::deriv::Integrable;
use crate::math::faer::FaerSparseLinearSystem2;
use crate::math::linear::{AsIndex, Stamp2};
use crate::math::newton_raphson2::{NewtonRaphsonSolver2, NonLinearSystem};
use crate::math::unit::UnitExt;
use crate::solver::dc::DcSolver;
use crate::solver::{Context, init_solver_configuration};
use log::debug;
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use num_traits::Zero;
use std::collections::HashMap;
use std::sync::Arc;
use crate::devices::voltage_source::Waveform::Step;

pub struct TransientSystem<'a> {
    pub circuit: &'a mut Circuit,
    pub time: f64,
    pub dt: f64,
    pub time_history: Vec<f64>,
}

impl<'a> TransientSystem<'a> {
    pub fn map_dynamic_stamps(
        alpha: f64,
        history: &Array1<f64>,
        dynamic_stamps: Vec<Stamp2<CircuitReference, f64>>,
    ) -> Vec<Stamp2<CircuitReference, f64>> {
        let mut stamps = Vec::with_capacity(dynamic_stamps.len() * 2);

        for s in dynamic_stamps {
            match s {
                Stamp2::Matrix(row, col, val) => {
                    stamps.push(Stamp2::Matrix(row.clone(), col.clone(), val * alpha));

                    if let Some(idx) = col.idx() {
                        let rhs_contribution = val * history[idx];

                        stamps.push(Stamp2::Rhs(row, -rhs_contribution));
                    }
                }
                Stamp2::Rhs(row, val) => {
                    stamps.push(Stamp2::Rhs(row, val));
                }
            }
        }

        stamps
    }
}

impl<'a> NonLinearSystem<CircuitReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha_hint: f64,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp2<CircuitReference, f64>>> {
        let tran_ctx = TransientAnalysisContext {
            time: self.time.into(),
            dt: self.dt.into(),
        };

        let (alpha, history) = state
            .integration_parameters(self.time_history.clone())
            .unwrap_or((f64::zero(), Array1::zeros(state.size())));

        let mut all_stamps = Vec::new();

        for (name, comp) in self.circuit.components_mut() {
            if let Some(tran) = comp.as_transient() {
                tran.update_transient(state, &tran_ctx, context)?;

                all_stamps.extend(tran.load_transient(state, &tran_ctx, context));

                let raw_dynamic = tran.load_transient_dynamic(state, &tran_ctx, context);
                all_stamps.extend(Self::map_dynamic_stamps(alpha, &history, raw_dynamic));
            } else {
                debug!("Component '{}' ignored in transient", name);
            }
        }
        Ok(all_stamps)
    }

    fn converged(
        &self,
        state: &CircularArrayBuffer2<f64>,
        new_guess: &ArrayView1<f64>,
        context: &Context,
    ) -> bool {
        let netlist = self.circuit.netlist();
        context.has_converged(state.latest(), new_guess, netlist)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
        context: &Context,
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

        if diff_norm >= context.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }
        }
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>, _context: &Context) {}
}

#[derive(Debug, Clone)]
pub struct TransientStep {
    pub time: f64,
    pub values: HashMap<Arc<CircuitVariable>, f64>,
}

pub struct TransientSolver<'a> {
    pub system: TransientSystem<'a>,
    pub solver: NewtonRaphsonSolver2<CircuitReference, f64, FaerSparseLinearSystem2<f64>>,
    pub options: TransientAnalysisOptions,
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut Circuit,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        init_solver_configuration();
        let netlist = circuit.netlist();

        let mut mapped_vars: Vec<_> = netlist
            .all_references()
            .into_iter()
            .filter(|id| id.idx().is_some())
            .collect();
        mapped_vars.sort_by_key(|id| id.idx().unwrap());

        let size = mapped_vars
            .last()
            .map(|id| id.idx().unwrap() + 1)
            .unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            time: 0.0,
            dt: options.dt,
            time_history: Vec::with_capacity(16),
        };

        let solver = NewtonRaphsonSolver2::new(&mut system, size, 4, context)?;

        Ok(Self {
            system,
            solver,
            options,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let mut steps = Vec::new();
        let stop_time = self.options.stop_time;
        let dt = self.options.dt;

        debug!("Calculaing DC Operating Point...");
        let mut dc_solver = DcSolver::new(self.system.circuit, Context::default())?;
        let dc_result = dc_solver.solve()?;

        let mut initial_vector = ndarray::Array1::<f64>::zeros(self.solver.state.size());
        let netlist = self.system.circuit.netlist();

        for (var, val) in &dc_result.values {
            if let Some(id) = netlist.reference_for(var) {
                if let Some(idx) = id.idx() {
                    if idx < initial_vector.len() {
                        initial_vector[idx] = *val;
                    }
                }
            }
        }

        self.solver.state.push(&initial_vector.view());
        self.solver.state.push(&initial_vector.view());

        self.system.time_history.insert(0, 0.0);
        self.system.time_history.insert(0, 0.0 - dt);

        steps.push(self.snapshot(0.0));

        let mut current_time = 0.0;

        while current_time < stop_time {
            current_time += dt;

            self.system.time = current_time;
            self.system.dt = dt;

            self.system.time_history.insert(0, current_time);

            debug!("Solving Transient Step: t = {:.6}s", current_time);

            let result = self.solver.solve(&mut self.system, 1.0 / dt);

            if result.is_ok() {
                steps.push(self.snapshot(current_time));

                if self.system.time_history.len() > 10 {
                    self.system.time_history.truncate(10);
                }
            } else {
                return Err(result.unwrap_err());
            }
        }

        Ok(TransientAnalysisResult { values: steps })
    }

    fn snapshot(&self, time: f64) -> TransientStep {
        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();
        let latest_state = self.solver.state.latest().unwrap();

        for reference in netlist.all_references() {
            if let Some(idx) = reference.idx() {
                values.insert(reference.variable().clone(), latest_state[idx]);
            }
        }

        TransientStep { time, values }
    }
}

#[test]
fn test_transient_rc_charging() {
    let mut circuit = Circuit::new("RC Transient Demo");

    circuit.voltage_source(
        "V1",
        "in",
        GND,
        Step {
            initial: 0.0.V(),
            final_value: 5.0.V(),
            delay: 0.0,
            rise_time: 1.0.us(),
        },
    );

    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.capacitor("C1", "out", GND, 1.0.uF());

    let options = TransientAnalysisOptions {
        stop_time: 5.0.ms(),
        dt: 100.0.us(),
    };

    let result = circuit
        .transient(options, Context::default())
        .unwrap()
        .solve()
        .unwrap();

    let out_key = circuit
        .netlist()
        .reference_for(&CircuitVariable::Node("out".into()))
        .expect("Node 'out' not found in netlist")
        .variable();

    let one_tau_step = result
        .values
        .iter()
        .find(|step| (step.time - 0.001).abs() < 1e-6)
        .expect("Time point 1.0ms not found in simulation results");

    let v_at_1ms = *one_tau_step
        .values
        .get(out_key)
        .expect("Variable 'out' missing in result step");

    println!("At 1ms (1 Tau): {:.4} V", v_at_1ms);
    assert!((v_at_1ms - 3.16).abs() < 0.1);

    // D. Check Final State (t = 5ms)
    let final_step = result.values.last().unwrap();
    let final_v = *final_step.values.get(out_key).unwrap();

    println!("At 5ms (Final): {:.4} V", final_v);
    assert!((final_v - 5.0).abs() < 0.05);
}

#[test]
fn test_transient_rc_step() {
    let mut circuit = Circuit::new("RC Step Response");

    circuit.voltage_source(
        "V1",
        "in",
        GND,
        Step {
            initial: 0.0.V(),
            final_value: 1.0.V(),
            delay: 0.0,
            rise_time: 1e-9,
        },
    );

    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.capacitor("C1", "out", GND, 1.0.uF());

    let result = circuit
        .transient(
            TransientAnalysisOptions {
                stop_time: 5.0.ms(),
                dt: 100.0.us(),
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();

    let out_key = circuit
        .netlist()
        .reference_for(&CircuitVariable::Node("out".into()))
        .expect("Output node not found in netlist")
        .variable();

    let final_snapshot = result.values.last().expect("Simulation returned no data");

    let v_final = *final_snapshot
        .values
        .get(out_key)
        .expect("Voltage value for 'out' missing");

    println!("Transient Final Voltage: {:.4} V", v_final);
    assert!(
        (v_final - 1.0).abs() < 0.01,
        "Capacitor did not charge to 1V. Got {}",
        v_final
    );
}

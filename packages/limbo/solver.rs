mod op;

use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::transient::{TransientAnalysisContext, TransientAnalysisOptions};
use crate::circuit::Circuit;
use crate::error::ErrorDetail;
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::unit::{Conductance, Frequency, Resistance, UnitExt};
use crate::netlist::CircuitReference;
use crate::state::CircuitState;
use num_complex::Complex;
use num_traits::real::Real;
use std::collections::HashMap;

pub struct Solver {
    circuit: Circuit,
    context: Context,
    symbolic: SymbolicMatrix<CircuitReference>,
}

impl Solver {
    pub fn build(mut circuit: Circuit, context: Context) -> crate::error::Result<Self> {
        // 1. Gather ALL possible symbols (nodes + branches)
        let symbols = circuit.netlist().all_references();

        let mut stamps = circuit.load_dc(&context);

        let dummy_state = CircuitState::new(HashMap::new(), HashMap::new(), 1);
        let dummy_tran = TransientAnalysisContext {
            time: 0.0.Sec(),
            dt: 1.0.Sec(),
        };
        stamps.extend(circuit.load_transient(&dummy_state, &dummy_tran, &context));

        Ok(Self {
            circuit,
            context,
            symbolic: SymbolicMatrix::new(symbols, stamps)?,
        })
    }

    /// Newton-Raphson Engine: The heart of the solver.
    /// Used by both OP and Transient analysis to find convergence for non-linearities.
    fn solve_nr(
        &mut self,
        circuit_state: &mut CircuitState<f64>,
        time_ctx: &TransientAnalysisContext,
    ) -> crate::error::Result<()> {
        let max_iters = 5000;

        for i in 0..max_iters {
            // 1. Prepare stamps based on current guess in circuit_state
            let stamps = if time_ctx.dt != 0.0.Sec() {
                self.circuit.update_dc(&self.context)?;
                self.circuit.update()?;
                self.circuit.load_dc(&self.context)
            } else {
                self.circuit
                    .update_transient(circuit_state, time_ctx, &self.context)?;
                self.circuit.update()?;
                self.circuit
                    .load_transient(circuit_state, time_ctx, &self.context)
            };

            // 2. Solve the linear system
            let mut system = LinearSystem::new(self.symbolic.size());
            system.apply_stamps(&self.symbolic, stamps, self.context.gmin.value);
            let next_solution = system.solve_with_backend(&self.symbolic)?;

            // 3. Update the guess in the state
            circuit_state.push_guess(time_ctx.time.value, next_solution);

            // 4. Check Convergence
            if self
                .circuit
                .check_convergence(circuit_state, time_ctx, &self.context)?
            {
                circuit_state.commit_guess();
                return Ok(());
            }
        }

        Err(ErrorDetail::simple(
            "Convergence Failure",
            "NR failed to converge.",
        ))
    }

    /// Operating Point: Solves at t=0 with dt=0 (no reactive effects)
    pub fn op(&mut self) -> crate::error::Result<HashMap<CircuitReference, f64>> {
        // Create an empty state for DC
        let mut state = CircuitState::new(HashMap::new(), HashMap::new(), 1);
        let dc_ctx = TransientAnalysisContext {
            time: 0.0.Sec(),
            dt: 0.0.Sec(),
        };

        // Seed the first guess with zeros if empty
        state.push_guess(0.0, HashMap::new());

        self.solve_nr(&mut state, &dc_ctx)?;

        Ok(state.get_last_vector().clone())
    }

    /// Transient: Steps through time, running NR at every step.
    pub fn transient(
        &mut self,
        options: TransientAnalysisOptions,
    ) -> crate::error::Result<Vec<(f64, HashMap<CircuitReference, f64>)>> {
        let mut history = Vec::new();

        // 1. Initial Operating Point
        let mut circuit_state = CircuitState::new(self.op()?, HashMap::new(), 5);
        history.push((0.0, circuit_state.get_last_vector().clone()));

        let mut t = options.dt;
        while t <= options.stop_time {
            let step_ctx = TransientAnalysisContext {
                time: t.Sec(),
                dt: options.dt.Sec(),
            };

            // Use the previous solution as the starting guess for the new time step
            let last_val = circuit_state.get_last_vector().clone();
            circuit_state.push_guess(t, last_val);

            // 2. Converge non-linearities for this time step
            self.solve_nr(&mut circuit_state, &step_ctx)?;

            history.push((t, circuit_state.get_last_vector().clone()));
            t += options.dt;
        }

        Ok(history)
    }

    pub fn solve_ac(
        &mut self,
        frequency: Frequency,
    ) -> crate::error::Result<HashMap<CircuitReference, Complex<f64>>> {
        let op = self.op()?;
        self.solve_ac_on_point(&op, frequency)
    }

    pub fn solve_ac_on_point(
        &mut self,
        dc_state: &HashMap<CircuitReference, f64>,
        frequency: Frequency,
    ) -> crate::error::Result<HashMap<CircuitReference, Complex<f64>>> {
        let ac_ctx = AcAnalysisContext { frequency };

        // 1. Prepare the Complex Circuit State
        // We initialize it with the DC operating point values (as the real part).
        let mut complex_values = HashMap::new();
        for (k, v) in dc_state {
            complex_values.insert(k.clone(), Complex::new(*v, 0.0));
        }

        let ac_state = CircuitState::new(complex_values, HashMap::new(), 1);

        // 2. Load the complex stamps
        // Components use the DC bias point from ac_state to determine small-signal parameters.
        let stamps = self.circuit.load_ac(&ac_state, &ac_ctx, &self.context);

        // 3. Solve the complex system
        let mut system = LinearSystem::new(self.symbolic.size());
        system.apply_stamps(&self.symbolic, stamps, self.context.gmin.value);

        system.solve_with_backend(&self.symbolic)
    }

    /// Performs a frequency sweep over a specified range.
    pub fn ac_sweep(
        &mut self,
        start_freq: Frequency,
        stop_freq: Frequency,
        steps: usize,
        logarithmic: bool,
    ) -> crate::error::Result<Vec<(Frequency, HashMap<CircuitReference, Complex<f64>>)>> {
        // 1. Mandatory step: Find the DC bias point first
        let dc_bias = self.op()?;

        let mut results = Vec::with_capacity(steps);

        for i in 0..steps {
            let t = (i as f64) / ((steps - 1) as f64).max(1.0);

            let freq = if logarithmic {
                // For log sweeps, we work with the raw values to use log10/powf
                let log_start = start_freq.value.log10();
                let log_stop = stop_freq.value.log10();
                let val = 10.0f64.powf(log_start + (log_stop - log_start) * t);
                val.Hz() // Re-wrap into a Frequency unit
            } else {
                // For linear sweeps, use unit arithmetic: start + (delta * ratio)
                start_freq + (stop_freq - start_freq) * t.ratio()
            };

            let solution = self.solve_ac_on_point(&dc_bias, freq)?;
            results.push((freq, solution));
        }

        Ok(results)
    }
}

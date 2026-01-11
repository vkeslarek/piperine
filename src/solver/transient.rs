use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult,
    TransientCircuitState, TransientSolver,
};
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::error::Error;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use std::ops::Sub;

pub struct TransientSolverImpl {
    circuit: Circuit,
    context: Context,
    options: TransientAnalysisOptions,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: TransientCircuitState,
}

impl TransientSolver for TransientSolverImpl {
    fn build(
        mut circuit: Circuit,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        let symbols = Self::get_active_symbols(&circuit);

        let mut dummy_state = TransientCircuitState::new(std::collections::HashMap::new(), 0, 1);
        let initial_context = TransientAnalysisContext {
            time: 0.0.Sec(),
            dt: options.dt.Sec(),
        };

        let mut all_stamps = Vec::new();
        all_stamps.extend(Self::linearize_static(
            &mut circuit,
            &mut dummy_state,
            &initial_context,
            &context,
        )?);
        all_stamps.extend(Self::linearize_dynamic(
            &mut circuit,
            &mut dummy_state,
            &initial_context,
            &context,
        )?);

        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, all_stamps)?;

        let state =
            TransientCircuitState::new(symbolic_matrix.mapping.clone(), symbolic_matrix.size, 3);

        Ok(Self {
            circuit,
            context,
            options,
            symbolic_matrix,
            state,
        })
    }

    fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let mut result = TransientAnalysisResult::new(self.symbolic_matrix.mapping.clone());

        let mut initial_context = TransientAnalysisContext {
            time: 0.0.Sec(),
            dt: self.options.dt.Sec(),
        };

        self.solve_newton_raphson(&initial_context, 0.0, 0.0)?;

        result.push(0.0, self.state.history.row(0).view());
        self.state.push_timestep(self.options.dt);

        initial_context.time = self.options.dt.Sec();

        while initial_context.time.value <= self.options.stop_time {
            self.solve_newton_raphson(
                &initial_context,
                initial_context.time.value,
                self.options.dt,
            )?;

            let converged_values = self.state.history.row(0).to_owned();
            result.push(initial_context.time.value, converged_values.view());

            let next_time = initial_context.time.value + self.options.dt;
            if next_time <= self.options.stop_time {
                self.state.push_timestep(next_time);
            }

            initial_context.time = next_time.Sec();
        }

        Ok(result)
    }
}

impl TransientSolverImpl {
    /// The Core Newton-Raphson Loop
    /// Handles both Static Physics (R) and Dynamic Physics (C * dv/dt)
    fn solve_newton_raphson(
        &mut self,
        transient_analysis_context: &TransientAnalysisContext,
        time: f64,
        dt: f64,
    ) -> crate::result::Result<()> {
        let (alpha, history_vec) = if dt > 0.0 {
            self.state.integration_parameters(2) // Order 2 (BDF2)
        } else {
            (0.0, ndarray::Array1::zeros(self.symbolic_matrix.size()))
        };

        for _iteration in 0..self.context.max_iter {
            let mut stamps = Self::linearize_static(
                &mut self.circuit,
                &self.state,
                transient_analysis_context,
                &self.context,
            )?;

            let dynamic_stamps = Self::linearize_dynamic(
                &mut self.circuit,
                &self.state,
                transient_analysis_context,
                &self.context,
            )?;

            for s in dynamic_stamps {
                match s {
                    Stamp::Matrix(r, c, val) => {
                        stamps.push(Stamp::Matrix(r.clone(), c.clone(), val * alpha));
                        if let Some(&col_idx) = self.symbolic_matrix.mapping.get(&c) {
                            let h_val = history_vec[col_idx];
                            stamps.push(Stamp::Rhs(r, -val * h_val));
                        }
                    }
                    _ => {}
                }
            }

            let mut linear_system = FaerLinearSystem::new(self.symbolic_matrix.size());
            linear_system.apply_stamps(&self.symbolic_matrix, stamps);
            let solution = linear_system.solve_with_backend(&self.symbolic_matrix)?;

            let converged = self.context.has_converged(
                self.state.history.row(0),
                solution.view(),
                &self.symbolic_matrix.mapping,
            );

            self.state.update_guess(solution);

            if converged {
                return Ok(());
            }
        }

        Err(Error::simple(
            "Convergence Failure",
            format!("Transient step at t={} failed to converge.", time),
        ))
    }

    fn linearize_static(
        circuit: &mut Circuit,
        state: &TransientCircuitState,
        transient_analysis_context: &TransientAnalysisContext,
        ctx: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();
        for (_, comp) in circuit.components_mut() {
            if let Some(t_comp) = comp.as_transient() {
                t_comp.update_transient(state, transient_analysis_context, ctx)?;

                stamps.extend(
                    t_comp
                        .load_transient(state, transient_analysis_context, ctx)
                        .into_iter()
                        .filter(|s| !s.has_ground_node()),
                );
            }
        }

        Ok(stamps)
    }

    fn linearize_dynamic(
        circuit: &mut Circuit,
        state: &TransientCircuitState,
        transient_analysis_context: &TransientAnalysisContext,
        ctx: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();
        for (_, comp) in circuit.components_mut() {
            if let Some(t_comp) = comp.as_transient() {
                stamps.extend(
                    t_comp
                        .load_dynamic(state, transient_analysis_context, ctx)
                        .into_iter()
                        .filter(|s| !s.has_ground_node()),
                );
            }
        }
        Ok(stamps)
    }

    fn get_active_symbols(circuit: &Circuit) -> Vec<CircuitReference> {
        circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| !s.is_ground())
            .collect()
    }
}

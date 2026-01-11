use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult,
};
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::circuit::state::CircuitState;
use crate::error::Error;
use crate::map;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use ndarray::Array1;
use std::collections::HashMap;

pub struct TransientSolver {
    circuit: Circuit,
    context: Context,
    options: TransientAnalysisOptions,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: CircuitState<f64>,
}

impl TransientSolver {
    pub fn build(
        mut circuit: Circuit,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        let symbols = Self::get_active_symbols(&circuit);
        let mut dummy_state = CircuitState::new(HashMap::new(), HashMap::new(), 0);
        let init_ctx = TransientAnalysisContext {
            time: 0.0.Sec(),
            dt: options.dt.Sec(),
        };

        // Collect stamps for symbolic analysis
        let stamps = [Self::linearize_static, Self::linearize_dynamic]
            .iter()
            .flat_map(|f| {
                f(&mut circuit, &mut dummy_state, &init_ctx, &context).unwrap_or_default()
            })
            .collect();

        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, stamps)?;
        let mut state = CircuitState::new(
            symbolic_matrix.mapping.clone(),
            map![IndependentVariable::Time => 0],
            3,
        );

        // Apply initial conditions
        circuit.components_mut().values_mut().for_each(|comp| {
            if let Some(tran) = comp.as_transient() {
                state.apply_initial_conditions(tran.initial_transient_values(&context));
            }
        });

        Ok(Self {
            circuit,
            context,
            options,
            symbolic_matrix,
            state,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let mut result = TransientAnalysisResult::new(self.symbolic_matrix.mapping.clone());
        let mut t = 0.0;

        while t <= self.options.stop_time {
            let dt = if t == 0.0 { 0.0 } else { self.options.dt };
            let ctx = TransientAnalysisContext {
                time: t.Sec(),
                dt: dt.Sec(),
            };

            if let Err(e) = self.solve_newton_raphson(&ctx, t, dt) {
                self.state.rollback();
                return Err(e);
            }

            self.state.commit();
            result.push(t, self.state.get_dependent_column(0));
            t += self.options.dt;
        }
        Ok(result)
    }

    fn solve_newton_raphson(
        &mut self,
        ctx: &TransientAnalysisContext,
        time: f64,
        dt: f64,
    ) -> crate::result::Result<()> {
        self.state.prepare_next(&Array1::from_elem(1, time).view());

        let (alpha, history) = self
            .state
            .integration_parameters(IndependentVariable::Time)
            .unwrap_or((0.0, Array1::zeros(self.symbolic_matrix.size())));

        for iter in 0..self.context.max_iter {
            let mut stamps =
                Self::linearize_static(&mut self.circuit, &self.state, ctx, &self.context)?;
            let dynamic =
                Self::linearize_dynamic(&mut self.circuit, &self.state, ctx, &self.context)?;

            // Apply BDF dynamic stamps
            for s in dynamic {
                if let Stamp::Matrix(r, c, val) = s {
                    stamps.push(Stamp::Matrix(r.clone(), c.clone(), val * alpha));
                    if let Some(&idx) = self.symbolic_matrix.mapping.get(&c) {
                        stamps.push(Stamp::Rhs(r, -val * history[idx]));
                    }
                }
            }

            let solution = self.solve_linear_system(stamps)?;
            let converged = self.context.has_converged(
                self.state.get_dependent_column(0),
                solution.view(),
                &self.symbolic_matrix.mapping,
            );

            self.state.get_current_dependent_column().assign(&solution);

            if converged {
                println!("t={:.3e} iters={}", time, iter);
                return Ok(());
            }
        }
        Err(Error::simple(
            "Convergence Failure",
            format!("Failed at t={}", time),
        ))
    }
    
    fn solve_linear_system(
        &self,
        stamps: Vec<Stamp<CircuitReference, f64>>,
    ) -> crate::result::Result<Array1<f64>> {
        let mut system = FaerLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(&self.symbolic_matrix, stamps);
        system.solve_with_backend(&self.symbolic_matrix)
    }

    fn linearize_static(
        circuit: &mut Circuit,
        state: &CircuitState<f64>,
        ctx: &TransientAnalysisContext,
        solver_ctx: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();
        for comp in circuit.components_mut().values_mut() {
            if let Some(t_comp) = comp.as_transient() {
                t_comp.update_transient(state, ctx, solver_ctx)?;
                stamps.extend(
                    t_comp
                        .load_transient(state, ctx, solver_ctx)
                        .into_iter()
                        .filter(|s| !s.has_ground_node()),
                );
            }
        }
        Ok(stamps)
    }

    fn linearize_dynamic(
        circuit: &mut Circuit,
        state: &CircuitState<f64>,
        ctx: &TransientAnalysisContext,
        solver_ctx: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();
        for comp in circuit.components_mut().values_mut() {
            if let Some(t_comp) = comp.as_transient() {
                stamps.extend(
                    t_comp
                        .load_transient_dynamic(state, ctx, solver_ctx)
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
            .filter(|s| s.is_dependent())
            .collect()
    }
}

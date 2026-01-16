use crate::circuit::netlist::CircuitReference;
use crate::error::Error;
use crate::math::Symbol;
use crate::math::array::{IndexedArray1, IndexedArray2};
use crate::math::deriv::DifferentiableIndependentScalar;
use crate::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use crate::math::iv::InitialValue;
use crate::math::linear::{SparseLinearSystem, Stamp, SymbolicMatrix};
use crate::math::num::Field;
use crate::solver::Context;
use ndarray::{Array1, ArrayView1};
use std::collections::HashMap;
use std::fmt::Debug;
use tracing::debug;

pub trait NewtonRaphsonStamper<S: Symbol, E: Field> {
    fn static_stamps(
        &mut self,
        state: &IndexedArray2<S, E>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<S, E>>>;

    fn dynamic_stamps(
        &mut self,
        state: &IndexedArray2<S, E>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<S, E>>>;

    fn initial_conditions(
        &mut self,
        context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<S, E>>>;

    fn active_symbols(&self) -> Vec<S>;
    fn independent_symbols(&self) -> Vec<S>;

    fn converged(
        &self,
        state: &IndexedArray2<S, E>,
        solution: &ArrayView1<E>,
        context: &Context,
    ) -> bool;
}

#[derive(Clone)]
pub struct NewtonRaphsonSolver<S: Symbol, E: Field> {
    pub symbolic_matrix: FaerSymbolicMatrix<S>,
    pub state: IndexedArray2<S, E>,
    pub context: Context,
}

impl<S: Symbol + std::fmt::Debug, E: 'static + Field> NewtonRaphsonSolver<S, E> {
    pub fn create(
        stamper: &mut dyn NewtonRaphsonStamper<S, E>,
        context: Context,
    ) -> crate::result::Result<Self> {
        let null_state = IndexedArray2::new(HashMap::new(), 0);
        let stamps: Vec<_> = [
            stamper.static_stamps(&null_state, &context)?,
            stamper.dynamic_stamps(&null_state, &context)?,
        ]
        .into_iter()
        .flatten()
        .collect();

        let active_syms = stamper.active_symbols();
        let symbolic_matrix = FaerSymbolicMatrix::new(active_syms, stamps)?;

        let mut state_map = symbolic_matrix.mapping.clone();
        let mut next_idx = state_map.values().max().cloned().unwrap_or(0) + 1;

        for var in stamper.independent_symbols() {
            if state_map.insert(var, next_idx).is_none() {
                next_idx += 1;
            }
        }

        let mut state = IndexedArray2::new(state_map, 3);

        state.push_align(&IndexedArray1::from_iv(
            stamper.initial_conditions(&context)?,
            symbolic_matrix.mapping.clone(),
        ));

        Ok(Self {
            symbolic_matrix,
            state,
            context,
        })
    }

    pub fn step_steady_state(
        &mut self,
        stamper: &mut dyn NewtonRaphsonStamper<S, E>,
        independent_values: &HashMap<S, E>,
    ) -> crate::result::Result<IndexedArray1<S, E>> {
        let guess = self
            .state
            .latest()
            .expect("Solver uninitialized")
            .to_owned();
        self.state.push(&guess);

        self.update_knowns(independent_values);

        for iter in 0..self.context.max_iter {
            debug!("Newton Iteration {}", iter + 1);

            let stamps = stamper.static_stamps(&self.state, &self.context)?;

            let solution = self.solve_system(stamps)?;

            let converged = stamper.converged(&self.state, &solution.view(), &self.context);

            let mapping = self.symbolic_matrix.mapping();
            self.state
                .latest_mut()
                .unwrap()
                .assign(&solution.view(), mapping);

            self.update_knowns(independent_values);

            if converged {
                debug!("Converged in {} iterations", iter + 1);
                return Ok(self.state.latest().unwrap().to_owned());
            }
        }

        self.state.pop();

        Err(Error::simple(
            "Convergence Failure",
            format!(
                "Failed to converge after {} iterations",
                self.context.max_iter
            ),
        ))
    }

    fn update_knowns(&mut self, values: &HashMap<S, E>) {
        let mut view = self.state.latest_mut().unwrap();
        for (sym, val) in values {
            view.set(sym, val);
        }
    }

    fn solve_system(&self, stamps: Vec<Stamp<S, E>>) -> crate::result::Result<Array1<E>> {
        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(&self.symbolic_matrix, stamps);
        system.solve_with_backend(&self.symbolic_matrix)
    }
}

impl<S: Symbol + Debug, E: Field + DifferentiableIndependentScalar> NewtonRaphsonSolver<S, E> {
    pub fn step_dynamic(
        &mut self,
        stamper: &mut dyn NewtonRaphsonStamper<S, E>,
        independent_values: &HashMap<S, E>,
        integration_symbol: &S,
    ) -> crate::result::Result<IndexedArray1<S, E>> {
        let guess = self
            .state
            .latest()
            .expect("Solver uninitialized")
            .to_owned();
        self.state.push(&guess);

        self.update_knowns(independent_values);

        let (alpha, history) = self
            .state
            .integration_parameters(integration_symbol)
            .unwrap_or((E::zero(), Array1::zeros(self.symbolic_matrix.size())));

        for iter in 0..self.context.max_iter {
            debug!("Newton Iteration {}", iter + 1);

            let mut stamps = stamper.static_stamps(&self.state, &self.context)?;
            let dynamic = stamper.dynamic_stamps(&self.state, &self.context)?;

            self.apply_dynamics(&mut stamps, dynamic, alpha, &history);

            let solution = self.solve_system(stamps)?;

            let converged = stamper.converged(&self.state, &solution.view(), &self.context);

            let mapping = self.symbolic_matrix.mapping();
            self.state
                .latest_mut()
                .unwrap()
                .assign(&solution.view(), mapping);

            self.update_knowns(independent_values);

            if converged {
                debug!("Converged in {} iterations", iter + 1);
                return Ok(self.state.latest().unwrap().to_owned());
            }
        }

        self.state.pop();

        Err(Error::simple(
            "Convergence Failure",
            format!(
                "Failed to converge after {} iterations",
                self.context.max_iter
            ),
        ))
    }

    fn apply_dynamics(
        &self,
        stamps: &mut Vec<Stamp<S, E>>,
        dynamic_stamps: Vec<Stamp<S, E>>,
        alpha: E,
        history: &Array1<E>,
    ) {
        for s in dynamic_stamps {
            if let Stamp::Matrix(row, col, val) = s {
                stamps.push(Stamp::Matrix(row.clone(), col.clone(), val * alpha));

                if let Some(&idx) = self.symbolic_matrix.mapping.get(&col) {
                    let rhs_contribution = val * history[idx];
                    stamps.push(Stamp::Rhs(row, -rhs_contribution));
                }
            }
        }
    }
}

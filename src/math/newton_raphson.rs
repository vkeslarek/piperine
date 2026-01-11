use crate::circuit::netlist::IndependentVariable;
use crate::error::Error;
use crate::math::deriv::BdfCoefficientGenerator;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{InitialValue, LinearSystem, Stamp, Symbol, SymbolicMatrix};
use crate::math::num::{Field, ScalableByReal};
use crate::solver::Context;
use ndarray::{Array1, Array2, ArrayView1, ArrayViewMut1, Zip};
use std::collections::HashMap;

#[derive(Clone)]
pub struct SolverState<S: Symbol, E: Field> {
    pub solver_variables: Array2<E>,
    pub solver_mapping: HashMap<S, usize>,
    pub independent_variables: Array2<f64>,
    pub independent_mapping: HashMap<IndependentVariable, usize>,
    current_index: usize,
    available_datapoints: usize,
    history_depth: usize,
}

impl<S: Symbol, E: Field> SolverState<S, E> {
    fn with_symbol_mapping(
        solver_mapping: HashMap<S, usize>,
        independent_mapping: HashMap<IndependentVariable, usize>,
        history_depth: usize,
    ) -> Self {
        Self {
            solver_variables: Array2::<E>::zeros((history_depth, solver_mapping.len())),
            solver_mapping,
            independent_variables: Array2::<f64>::zeros((history_depth, independent_mapping.len())),
            independent_mapping,
            current_index: 0,
            available_datapoints: 0,
            history_depth,
        }
    }

    fn new() -> Self {
        Self {
            solver_variables: Array2::zeros((0, 0)),
            solver_mapping: HashMap::new(),
            independent_variables: Array2::zeros((0, 0)),
            independent_mapping: HashMap::new(),
            current_index: 0,
            available_datapoints: 0,
            history_depth: 0,
        }
    }

    fn apply_initial_conditions(&mut self, initial_conditions: Vec<InitialValue<S, E>>) {
        for initial_value in initial_conditions {
            let idx = self.solver_mapping.get(&initial_value.reference).unwrap();
            let phys_idx = self.get_physical_index(0);
            self.solver_variables[[phys_idx, *idx]] = initial_value.value;
        }
    }

    fn prepare_iteration(&mut self, independent_variables: &ArrayView1<f64>) {
        let phys_idx = self.get_physical_index(0);
        self.independent_variables
            .row_mut(phys_idx)
            .assign(&independent_variables);

        let prev_phys_idx = self.get_physical_index(1);
        let prev_values = self.solver_variables.row(prev_phys_idx).to_owned();
        self.solver_variables.row_mut(phys_idx).assign(&prev_values);
    }

    fn rollback(&mut self) {
        self.current_index = (self.current_index + self.history_depth - 1) % self.history_depth;
        self.available_datapoints = self.available_datapoints.saturating_sub(1);
    }

    pub fn update_current_guess(&mut self, values: &Array1<E>) {
        let phys_idx = self.get_physical_index(0);
        self.solver_variables.row_mut(phys_idx).assign(values);
    }

    pub fn commit(&mut self, independent_vars: &ArrayView1<f64>) {
        let next_idx = (self.current_index + 1) % self.history_depth;

        // Use the current converged values as the seed for the next step's guess
        let current_vals = self.solver_variables.row(self.current_index).to_owned();
        self.solver_variables
            .row_mut(next_idx)
            .assign(&current_vals);
        self.independent_variables
            .row_mut(next_idx)
            .assign(independent_vars);

        self.current_index = next_idx;
        self.available_datapoints = (self.available_datapoints + 1).min(self.history_depth);
    }

    pub fn get_dependent_value(&self, reference: &S, lookback: usize) -> Option<E> {
        let idx = *self.solver_mapping.get(reference)?;
        let phys_idx = self.get_physical_index(lookback);
        Some(self.solver_variables[[phys_idx, idx]])
    }

    pub fn get_independent_value(
        &self,
        variable: &IndependentVariable,
        lookback: usize,
    ) -> Option<f64> {
        let idx = *self.independent_mapping.get(variable)?;
        let phys_idx = self.get_physical_index(lookback);
        Some(self.independent_variables[[phys_idx, idx]])
    }

    pub fn get_dependent_column(&self, lookback: usize) -> ArrayView1<E> {
        let phys_idx = self.get_physical_index(lookback);
        self.solver_variables.row(phys_idx)
    }

    pub fn get_independent_column(&self, lookback: usize) -> ArrayView1<f64> {
        let phys_idx = self.get_physical_index(lookback);
        self.independent_variables.row(phys_idx)
    }

    pub fn get_current_dependent_column(&mut self) -> ArrayViewMut1<E> {
        let phys_idx = self.get_physical_index(0);
        self.solver_variables.row_mut(phys_idx)
    }

    pub fn get_current_independent_column(&mut self) -> ArrayViewMut1<f64> {
        let phys_idx = self.get_physical_index(0);
        self.independent_variables.row_mut(phys_idx)
    }

    pub fn get_available_datapoints(&self) -> usize {
        self.available_datapoints
    }

    fn get_physical_index(&self, lookback: usize) -> usize {
        (self.current_index + self.history_depth - lookback) % self.history_depth
    }
}

impl<S: Symbol, E: Field + ScalableByReal> SolverState<S, E> {
    pub fn integration_parameters(&self, dx: &IndependentVariable) -> Option<(f64, Array1<E>)> {
        let dx_idx = *self.independent_mapping.get(dx)?;
        let order = self.available_datapoints.saturating_sub(1).min(3);

        let ts_vec: Vec<_> = (0..=order)
            .map(|i| self.independent_variables[[self.get_physical_index(i), dx_idx]])
            .rev()
            .collect(); // Matches BdfCoefficientGenerator expectation

        let coeffs = BdfCoefficientGenerator::generate(order, ts_vec)?;

        // Matrix multiplication-like logic for history sum
        let mut history_sum = Array1::<E>::zeros(self.solver_variables.ncols());
        for (i, &c) in coeffs.history_coeffs.iter().enumerate() {
            let view = self.get_dependent_column(i + 1);
            Zip::from(&mut history_sum)
                .and(&view)
                .for_each(|sum, &val| {
                    *sum += val * c;
                });
        }

        Some((coeffs.alpha, history_sum))
    }
}

pub trait NewtonRaphsonStamper<S: Symbol, E: Field> {
    fn static_stamps(
        &mut self,
        state: &SolverState<S, E>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<S, E>>>;

    fn dynamic_stamps(
        &mut self,
        state: &SolverState<S, E>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<S, E>>>;

    fn initial_conditions(
        &mut self,
        context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<S, E>>>;
    fn active_symbols(&self) -> Vec<S>;
    fn independent_symbols(&self) -> Vec<IndependentVariable>;
    fn converged(
        &self,
        state: &SolverState<S, E>,
        solution: &ArrayView1<E>,
        context: &Context,
    ) -> bool;
}

impl<S: Symbol, E: 'static + Field + ScalableByReal> NewtonRaphsonSolver<S, E> {
    pub fn create(
        stamper: &mut dyn NewtonRaphsonStamper<S, E>,
        context: Context,
    ) -> crate::result::Result<Self> {
        let symbols = stamper.active_symbols();

        // Structural analysis using empty state
        let null_state = SolverState::new();
        let stamps: Vec<_> = [
            stamper.static_stamps(&null_state, &context)?,
            stamper.dynamic_stamps(&null_state, &context)?,
        ]
        .into_iter()
        .flat_map(|f| f.into_iter())
        .collect();

        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, stamps)?;
        let indep_map: HashMap<_, _> = stamper
            .independent_symbols()
            .into_iter()
            .enumerate()
            .map(|(i, s)| (s, i))
            .collect();

        let mut state =
            SolverState::with_symbol_mapping(symbolic_matrix.mapping.clone(), indep_map, 3);
        state.apply_initial_conditions(stamper.initial_conditions(&context)?);

        Ok(Self {
            symbolic_matrix,
            state,
            context,
        })
    }

    pub fn step(
        &mut self,
        stamper: &mut dyn NewtonRaphsonStamper<S, E>,
        independent_vars: &ArrayView1<f64>,
        integration_var: &IndependentVariable,
    ) -> crate::result::Result<Array1<E>> {
        let (alpha, history) = self
            .state
            .integration_parameters(integration_var)
            .unwrap_or((0.0, Array1::zeros(self.symbolic_matrix.size())));

        for iter in 0..self.context.max_iter {
            let mut stamps = stamper.static_stamps(&self.state, &self.context)?;
            let dynamic = stamper.dynamic_stamps(&self.state, &self.context)?;

            // Compact dynamic integration logic
            self.apply_dynamic_stamps(&mut stamps, dynamic, alpha, &history);

            let solution = self.solve_linear_system(stamps)?;
            let converged = stamper.converged(&self.state, &solution.view(), &self.context);

            self.state.update_current_guess(&solution);

            if converged {
                self.state.commit(independent_vars);
                return Ok(solution);
            }
        }

        self.state.rollback();
        Err(Error::simple(
            "Convergence Failure",
            format!("Failed at iteration {}", self.context.max_iter),
        ))
    }

    fn apply_dynamic_stamps(
        &self,
        target: &mut Vec<Stamp<S, E>>,
        dynamic: Vec<Stamp<S, E>>,
        alpha: f64,
        history: &Array1<E>,
    ) {
        for s in dynamic {
            if let Stamp::Matrix(r, c, val) = s {
                target.push(Stamp::Matrix(r.clone(), c.clone(), val * alpha));
                if let Some(&idx) = self.symbolic_matrix.mapping.get(&c) {
                    target.push(Stamp::Rhs(r, -val * history[idx]));
                }
            }
        }
    }

    fn solve_linear_system(&self, stamps: Vec<Stamp<S, E>>) -> crate::result::Result<Array1<E>> {
        let mut system = FaerLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(&self.symbolic_matrix, stamps);
        system.solve_with_backend(&self.symbolic_matrix)
    }
}

#[derive(Clone)]
pub struct NewtonRaphsonSolver<S: Symbol, E: Field> {
    pub symbolic_matrix: FaerSymbolicMatrix<S>,
    pub state: SolverState<S, E>,
    pub context: Context,
}

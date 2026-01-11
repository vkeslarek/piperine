use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix, FaerToNdarray};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::num::{Field, ScalableByReal};
use crate::math::unit::{Conductance, Resistance, UnitExt};
use faer::traits::ComplexField;
use ndarray::{Array1, Array2, ArrayView1, ArrayViewMut1, Zip, s};
use num_traits::real::Real;
use std::collections::HashMap;
use tracing::debug;

pub mod dc;
pub mod transient;

pub struct Context {
    pub gmin: Conductance,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub max_iter: usize,
    pub min_res: Resistance,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            gmin: 1.0.pS(),
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            max_iter: 500,
            min_res: 1.0.uOhms(),
        }
    }
}

impl Context {
    pub fn has_converged(
        &self,
        old_values: ArrayView1<f64>,
        new_values: ArrayView1<f64>,
        mapping: &HashMap<CircuitReference, usize>,
    ) -> bool {
        mapping.iter().all(|(reference, &index)| {
            if index >= old_values.len() || index >= new_values.len() {
                return false;
            }

            let old_v = old_values[index];
            let new_v = new_values[index];

            let abs_limit = if matches!(reference, CircuitReference::Branch(_)) {
                self.abstol // Current (Amps)
            } else {
                self.vntol // Voltage (Volts)
            };

            let magnitude = old_v.abs().max(new_v.abs());
            let allowed_error = self.reltol * magnitude + abs_limit;
            let diff = (new_v - old_v).abs();

            diff <= allowed_error
        })
    }
}

pub trait CircuitState {
    type NumType: Field;
    fn current_guess_mut(&mut self) -> ArrayViewMut1<Self::NumType>;
    fn hist_deriv(&self) -> (Self::NumType, ArrayView1<Self::NumType>);
    fn push(&mut self, new_values: ArrayView1<Self::NumType>);
}

pub trait AnalysisResult {
    type NumType: Field;

    fn new() -> Self;
    fn push_converged(
        &mut self,
        mapping: &HashMap<CircuitReference, usize>,
        values: Array1<<Self as AnalysisResult>::NumType>,
    );
}

pub trait SolverCore {
    type StateType: CircuitState<NumType = Self::NumType>;
    type AnalysisResultType: AnalysisResult<NumType = Self::NumType>;
    type AnalysisOptionsType;
    type NumType: Field + ComplexField;

    fn new_state(
        mapping: HashMap<CircuitReference, usize>,
        size: usize,
        history_depth: usize,
    ) -> Self::StateType;

    fn static_linearize_circuit(
        circuit: &mut Circuit,
        state: &Self::StateType,
        options: &Self::AnalysisOptionsType,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Self::NumType>>>;

    fn dynamic_linearize_circuit(
        circuit: &mut Circuit,
        state: &Self::StateType,
        options: &Self::AnalysisOptionsType,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Self::NumType>>>;

    fn apply_initial_conditions(
        mapping: &HashMap<CircuitReference, usize>,
        context: &Context,
        initial_state: ArrayViewMut1<Self::NumType>,
    ) -> crate::result::Result<()>;

    fn check_convergence(
        state: &Self::StateType,
        new_values: ArrayView1<Self::NumType>,
        reltol: f64,
        vntol: f64,
        abstol: f64,
    ) -> bool;
}

pub struct SolverA<C: SolverCore> {
    circuit: Circuit,
    context: Context,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: C::StateType,
    options: C::AnalysisOptionsType,
    _marker: std::marker::PhantomData<C>,
}
impl<C: SolverCore> SolverA<C> {
    pub fn build(
        mut circuit: Circuit,
        options: C::AnalysisOptionsType,
        context: Context,
    ) -> crate::result::Result<SolverA<C>> {
        let symbols = Self::get_active_symbols(&circuit);

        let zero_state = C::new_state(HashMap::new(), 0, 2);
        let stamps = C::static_linearize_circuit(&mut circuit, &zero_state, &options, &context)?;
        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, stamps)?;

        let mut state = C::new_state(symbolic_matrix.mapping.clone(), symbolic_matrix.size, 2);
        let initial_state = state.current_guess_mut();
        C::apply_initial_conditions(&symbolic_matrix.mapping, &context, initial_state)?;

        Ok(Self {
            circuit,
            context,
            symbolic_matrix,
            state,
            options,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<C::AnalysisResultType>
    where
        <C as SolverCore>::NumType: 'static,
    {
        let mut analysis_result = C::AnalysisResultType::new();
        for iteration in 0..self.context.max_iter {
            let stamps = C::static_linearize_circuit(
                &mut self.circuit,
                &mut self.state,
                &self.options,
                &self.context,
            )?;

            let mut linear_system: FaerLinearSystem<CircuitReference, C::NumType> =
                FaerLinearSystem::new(self.symbolic_matrix.size());
            linear_system.apply_stamps(&self.symbolic_matrix, stamps);

            let (alpha, deriv_hist) = self.state.hist_deriv();

            let stamps_g = C::dynamic_linearize_circuit(
                &mut self.circuit,
                &self.state,
                &self.options,
                &self.context,
            )?
            .into_iter()
            .flat_map(|stamp| match stamp {
                Stamp::Matrix(r, c, val) => {
                    vec![Stamp::Matrix(r, c, val * alpha)]
                }
                Stamp::Rhs(r, val) => {
                    if let Some(&idx) = self.symbolic_matrix.mapping.get(&r) {
                        let history_val = deriv_hist[idx];
                        vec![Stamp::Rhs(r, val * history_val)]
                    } else {
                        vec![]
                    }
                }
            })
            .collect::<Vec<_>>();

            linear_system.apply_stamps(&self.symbolic_matrix, stamps_g);

            let new_values = linear_system.solve_with_backend(&self.symbolic_matrix)?;

            let converged = C::check_convergence(
                &self.state,
                new_values.view(),
                self.context.reltol,
                self.context.vntol,
                self.context.abstol,
            );

            self.state.push(new_values.view());

            if converged {
                analysis_result.push_converged(&self.symbolic_matrix.mapping, new_values);
                debug!("Converged in {} iterations.", iteration);
                return Ok(analysis_result);
            }
        }

        Err(crate::error::Error::simple(
            "Convergence Failure",
            "Newton-Raphson loop exceeded max iterations without converging.",
        ))
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
pub struct SolverState<E> {
    buffer: Array2<E>,
    mapping: HashMap<CircuitReference, usize>,
    is_branch: Array1<bool>,
}

impl<E: Field + ComplexField + Copy> SolverState<E> {
    pub fn new(mapping: HashMap<CircuitReference, usize>, size: usize, depth: usize) -> Self {
        // Pre-calculate branch mask for O(1) lookups during convergence checks
        let mut is_branch = Array1::from_elem(size, false);
        for (reference, &idx) in &mapping {
            if let CircuitReference::Branch(_) = reference {
                if idx < size {
                    is_branch[idx] = true;
                }
            }
        }

        Self {
            buffer: Array2::zeros((depth, size)),
            mapping,
            is_branch,
        }
    }

    pub fn current_guess(&self) -> ArrayView1<E> {
        self.buffer.row(0)
    }

    pub fn current_guess_mut(&mut self) -> ArrayViewMut1<E> {
        self.buffer.row_mut(0)
    }

    pub fn get_history(&self, k: usize) -> ArrayView1<E> {
        self.buffer.row(k)
    }

    pub fn update_guess(&mut self, new_values: ArrayView1<E>) {
        self.buffer.row_mut(0).assign(&new_values);
    }

    pub fn push_converged(&mut self) {
        let rows = self.buffer.nrows();
        if rows > 1 {
            let (mut older, newer) = self
                .buffer
                .multi_slice_mut((s![1..rows, ..], s![0..rows - 1, ..]));
            older.assign(&newer);
        }
    }

    pub fn reset_to_last_converged(&mut self) {
        if self.buffer.nrows() > 1 {
            let last_valid = self.buffer.row(1).to_owned();
            self.buffer.row_mut(0).assign(&last_valid);
        }
    }
}

pub trait SolverBackend: 'static {
    type E: Field + ScalableByReal;

    fn integration_coefficients(
        state: &SolverState<Self::E>,
        context: &Context, // Context might contain 'dt'
    ) -> (Self::E, Array1<Self::E>);

    /// Generates stamps for static components (R, Source, Diode, BJT)
    fn linearize_static(
        circuit: &mut Circuit,
        state: &SolverState<Self::E>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Self::E>>>;

    /// Generates stamps for dynamic components (C, L)
    /// Returns "Mass" stamps (Matrix = C) and "Charge" stamps (Rhs = Q).
    fn linearize_dynamic(
        circuit: &mut Circuit,
        state: &SolverState<Self::E>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Self::E>>>;

    fn check_convergence(
        new_values: ArrayView1<Self::E>,
        solver_state: &SolverState<Self::E>,
        context: &Context,
    ) -> bool {
        let old_values = solver_state.buffer.row(0);

        Zip::from(&old_values)
            .and(&new_values)
            .and(&solver_state.is_branch)
            .all(|&old_v, &new_v, &is_branch| {
                // Get magnitude (handle Complex or Real)
                let old_abs = old_v.abs();
                let new_abs = new_v.abs();
                let diff = (new_v - old_v).abs();

                let abs_limit = if is_branch {
                    context.abstol
                } else {
                    context.vntol
                };

                // SPICE Criteria: RelTol * max(|old|, |new|) + AbsTol
                let limit = context.reltol * old_abs.max(new_abs) + abs_limit;

                diff <= limit
            })
    }
}

pub struct Solver<B: SolverBackend> {
    circuit: Circuit,
    context: Context, // Global/Default context
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: SolverState<B::E>,
    _backend: std::marker::PhantomData<B>,
}

impl<B: SolverBackend> Solver<B> {
    pub fn solve_one_point(&mut self, step_context: &Context) -> crate::result::Result<()> {
        // 1. Get Integration Params (Alpha + History Vector)
        let (alpha, history_vec) = B::integration_coefficients(&self.state, step_context);

        for _iter in 0..step_context.max_iter {
            // 2. Physics: Static Linearization
            let mut stamps = B::linearize_static(&mut self.circuit, &self.state, step_context)?;

            // 3. Physics: Dynamic Linearization
            let dynamic_stamps =
                B::linearize_dynamic(&mut self.circuit, &self.state, step_context)?;

            // 4. Math: Apply Integration Formula (The "Link")
            // Convert C * dv/dt  ->  (G_eq * v) + I_eq
            for s in dynamic_stamps {
                match s {
                    Stamp::Matrix(r, c, val) => {
                        // Matrix term: C * alpha
                        stamps.push(Stamp::Matrix(r, c, val * alpha));
                    }
                    Stamp::Rhs(r, val) => {
                        // RHS term: C * Sum(Coeff_k * v_n-k)
                        if let Some(&idx) = self.state.mapping.get(&r) {
                            let h_val = history_vec[idx];
                            stamps.push(Stamp::Rhs(r, val * h_val));
                        }
                    }
                }
            }

            // 5. Build & Solve Linear System
            let mut system =
                FaerLinearSystem::<CircuitReference, B::E>::new(self.symbolic_matrix.size());
            system.apply_stamps(&self.symbolic_matrix, stamps);

            let result_col = system.solve_with_backend(&self.symbolic_matrix)?;

            // 6. Check Convergence
            let converged = B::check_convergence(result_col.view(), &self.state, step_context);

            // Update the working guess (Row 0)
            self.state.update_guess(result_col.view());

            if converged {
                return Ok(());
            }
        }

        Err(crate::error::Error::simple(
            "Convergence Failure",
            "Newton-Raphson loop exceeded max iterations without converging.",
        ))
    }

    /// Access to state for the Driver (to push/reset history)
    pub fn state_mut(&mut self) -> &mut SolverState<B::E> {
        &mut self.state
    }
}

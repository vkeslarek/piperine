use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix, FaerToNdarray};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::num::Field;
use crate::math::unit::{Conductance, Resistance, UnitExt};
use faer::traits::ComplexField;
use ndarray::{ArrayView1, ArrayViewMut1};
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
        values: ArrayView1<<Self as AnalysisResult>::NumType>,
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

pub struct Solver<C: SolverCore> {
    circuit: Circuit,
    context: Context,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: C::StateType,
    options: C::AnalysisOptionsType,
    _marker: std::marker::PhantomData<C>,
}
impl<C: SolverCore> Solver<C> {
    pub fn build(
        mut circuit: Circuit,
        options: C::AnalysisOptionsType,
        context: Context,
    ) -> crate::result::Result<Solver<C>> {
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
            let arr = new_values.to_ndarray();

            let converged = C::check_convergence(
                &self.state,
                arr.view(),
                self.context.reltol,
                self.context.vntol,
                self.context.abstol,
            );

            self.state.push(arr.view());

            if converged {
                analysis_result.push_converged(&self.symbolic_matrix.mapping, arr.view());
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

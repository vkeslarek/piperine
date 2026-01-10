use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcCircuitState, DcSolver};
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::error::Error;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix, FaerToNdarray};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::solver::{Context, SolverCore};
use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

pub struct DcSolverImpl {
    circuit: Circuit,
    context: Context,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: DcCircuitState,
}

impl DcSolver for DcSolverImpl {
    fn build(mut circuit: Circuit, context: Context) -> crate::result::Result<DcSolverImpl> {
        let symbols = Self::get_active_symbols(&circuit);

        let mut zero_state = DcCircuitState::new(std::collections::HashMap::new(), 0, 2);
        let stamps = Self::linearize_circuit(&mut circuit, &mut zero_state, &context)?;

        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, stamps)?;

        let mut state =
            DcCircuitState::new(symbolic_matrix.mapping.clone(), symbolic_matrix.size, 2);
        let mut initial_state = state.current_guess_mut();
        for (name, component_box) in circuit.components_mut().iter_mut() {
            let component = component_box.as_dc().ok_or_else(|| {
                Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Component does not implement DcAnalysis.",
                )
            })?;

            let initial_values = component.initial_dc_values(&context);
            for init in initial_values {
                if let Some(&idx) = symbolic_matrix.mapping.get(&init.reference) {
                    initial_state[idx] = init.value;
                }
            }
        }

        Ok(Self {
            circuit,
            context,
            symbolic_matrix,
            state,
        })
    }

    fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        for iteration in 0..self.context.max_iter {
            let stamps =
                Self::linearize_circuit(&mut self.circuit, &mut self.state, &self.context)?;

            let mut linear_system: FaerLinearSystem<CircuitReference, f64> =
                FaerLinearSystem::new(self.symbolic_matrix.size());
            linear_system.apply_stamps(&self.symbolic_matrix, stamps);

            let new_values = linear_system.solve_with_backend(&self.symbolic_matrix)?;
            let arr = new_values.to_ndarray();
            let converged = self.state.check_convergence(
                arr.view(),
                self.context.reltol,
                self.context.vntol,
                self.context.abstol,
            );

            println!("ITERATION: {}", iteration);
            println!("NEWV: {:?}", new_values);
            println!("ABS: {:?}", self.state.get_diff(arr.view()));

            if converged {
                println!("Solved in {} iterations", iteration);
                return Ok(DcAnalysisResult {
                    values: new_values,
                    mapping: self.symbolic_matrix.mapping().clone(),
                });
            }

            self.state.push(arr.view());
        }

        Err(Error::simple(
            "Convergence Failure",
            "Newton-Raphson loop exceeded max iterations without converging.",
        ))
    }
}

impl DcSolverImpl {
    fn linearize_circuit(
        circuit: &mut Circuit,
        state: &mut DcCircuitState,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();

        for (name, component_box) in circuit.components_mut().iter_mut() {
            let component = component_box.as_dc().ok_or_else(|| {
                Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Component does not implement DcAnalysis.",
                )
            })?;

            component.update_dc(state, context)?;

            let new_stamps = component.load_dc(state, context);

            stamps.extend(new_stamps.into_iter().filter(|s| !s.has_ground_node()));
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

pub struct DcBackend;

#[derive(Default)]
pub struct DcOptions; // Add fields if you want to support Gmin stepping later

impl SolverCore for DcBackend {
    type StateType = DcCircuitState;
    type AnalysisResultType = DcAnalysisResult;
    type AnalysisOptionsType = ();
    type NumType = f64;

    fn new_state(
        mapping: HashMap<CircuitReference, usize>,
        size: usize,
        history_depth: usize,
    ) -> Self::StateType {
        DcCircuitState::new(mapping, size, history_depth)
    }

    fn static_linearize_circuit(
        circuit: &mut Circuit,
        state: &Self::StateType,
        _options: &Self::AnalysisOptionsType,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Self::NumType>>> {
        let mut stamps = Vec::new();

        for (name, component_box) in circuit.components_mut().iter_mut() {
            let component = component_box.as_dc().ok_or_else(|| {
                crate::error::Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Component does not implement DcAnalysis.",
                )
            })?;

            // Update internal non-linear states (Vbe, etc.)
            component.update_dc(state, context)?;

            // Load stamps and filter ground
            let new_stamps = component.load_dc(state, context);
            stamps.extend(new_stamps.into_iter().filter(|s| !s.has_ground_node()));
        }

        // Apply Gmin for numerical stability on every active node
        for (reference, &idx) in &state.mapping {
            if let CircuitReference::Node(_) = reference {
                stamps.push(Stamp::Matrix(
                    reference.clone(),
                    reference.clone(),
                    context.gmin.value,
                ));
            }
        }

        Ok(stamps)
    }

    fn dynamic_linearize_circuit(
        _circuit: &mut Circuit,
        _state: &Self::StateType,
        _options: &Self::AnalysisOptionsType,
        _context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Self::NumType>>> {
        // DC Analysis ignores Mass/Dynamic stamps (Capacitors = Open, Inductors = Short)
        Ok(Vec::new())
    }

    fn apply_initial_conditions(
        mapping: &HashMap<CircuitReference, usize>,
        context: &Context,
        mut initial_state: ArrayViewMut1<Self::NumType>,
    ) -> crate::result::Result<()> {
        // Since we don't have direct access to the circuit here (following the SolverCore trait),
        // we assume initial_state starts at zero. If you need component-specific
        // initial values, the circuit linearization handles it in the first iteration.
        // Alternatively, you can pass the circuit through a modified trait.
        Ok(())
    }

    fn check_convergence(
        state: &Self::StateType,
        new_values: ArrayView1<Self::NumType>,
        reltol: f64,
        vntol: f64,
        abstol: f64,
    ) -> bool {
        state.check_convergence(new_values, reltol, vntol, abstol)
    }
}

use crate::analysis::dc::DcAnalysis;
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::tf::{
    TransferFunctionAnalysisOptions, TransferFunctionAnalysisResult, TransferType,
};
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::{AnalogReference, AnalogVariable};
use crate::math::faer::FaerSymbolicMatrix;
use crate::math::linear::{SymbolicLinearSystem, SymbolicMatrix};
use crate::solver::dc::DcSolver;
use crate::solver::{Context, init_solver_configuration};
use ndarray::Array1;

/// Transfer Function solver.
///
/// Computes DC small-signal transfer characteristics:
/// - **Gain:** Small-signal transfer ratio dOutput/dInput
/// - **Input Resistance:** Resistance seen by the input source
/// - **Output Resistance:** Thévenin/Norton equivalent at output
///
/// The solver works by:
/// 1. Computing DC operating point
/// 2. Linearizing the circuit around the operating point
/// 3. Solving the linearized system with unit excitations
pub struct TransferFunctionSolver<'a> {
    circuit: &'a mut CircuitInstance,
    context: Context,
    dc_point: DcAnalysisResult,
    options: TransferFunctionAnalysisOptions,
    symbolic_matrix: FaerSymbolicMatrix,

    // Cached references for efficiency
    input_branch_ref: AnalogReference,
    output_ref: AnalogReference,
    output_ref_node: Option<AnalogReference>,
}

impl<'a> TransferFunctionSolver<'a> {
    /// Creates a new Transfer Function solver.
    ///
    /// # Process
    /// 1. Initializes solver configuration
    /// 2. Solves DC operating point (required for linearization)
    /// 3. Builds symbolic matrix structure (sparsity pattern)
    /// 4. Validates and resolves input source reference
    /// 5. Validates and resolves output reference(s)
    ///
    /// # Arguments
    /// * `circuit` - Circuit instance to analyze
    /// * `options` - Transfer function analysis parameters (input source, output variable)
    /// * `context` - Solver context with tolerances and settings
    ///
    /// # Returns
    /// Initialized transfer function solver ready for analysis
    ///
    /// # Errors
    /// - If input source branch is not found in circuit
    /// - If output node/branch is not found in circuit
    /// - If DC operating point fails to converge
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: TransferFunctionAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        init_solver_configuration();

        // Solve DC operating point
        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;
        eprintln!(
            "DEBUG TF::new - DC point solved. Values: {:?}",
            dc_point.values()
        );

        // Resolve input source branch reference
        let input_branch_var = AnalogVariable::Branch(options.input_source.clone());
        let input_branch_ref = circuit
            .netlist()
            .reference_for(&input_branch_var)
            .ok_or_else(|| {
                crate::error::Error::simple(
                    "TF",
                    &format!(
                        "Input source branch '{}' not found",
                        options.input_source.component
                    ),
                )
            })?
            .clone();

        // Resolve output reference
        let output_ref = circuit
            .netlist()
            .reference_for(&options.output)
            .ok_or_else(|| {
                crate::error::Error::simple("TF", "Output variable not found in circuit")
            })?
            .clone();

        eprintln!(
            "DEBUG: Output ref = {:?}, idx = {:?}",
            output_ref.variable(),
            output_ref.idx()
        );

        // Resolve output reference node (for differential voltage)
        let output_ref_node = if let Some(ref_node) = &options.output_ref {
            let ref_var = AnalogVariable::Node(ref_node.clone());
            Some(
                circuit
                    .netlist()
                    .reference_for(&ref_var)
                    .ok_or_else(|| {
                        crate::error::Error::simple(
                            "TF",
                            "Output reference node not found in circuit",
                        )
                    })?
                    .clone(),
            )
        } else {
            None
        };

        // Build symbolic matrix structure
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        let symbolic_stamps = Self::assemble_dc_stamps(circuit, &dc_point, &context)?;
        let symbolic_matrix = FaerSymbolicMatrix::new(size, symbolic_stamps)?;

        Ok(Self {
            circuit,
            context,
            dc_point,
            options,
            symbolic_matrix,
            input_branch_ref,
            output_ref,
            output_ref_node,
        })
    }

    /// Performs Transfer Function analysis.
    ///
    /// Calculates gain, input resistance, and output resistance by solving
    /// the linearized system with appropriate excitations.
    ///
    /// # Returns
    /// Complete transfer function analysis result with gain, R_in, R_out, and transfer type
    pub fn solve(&mut self) -> crate::result::Result<TransferFunctionAnalysisResult> {
        // Determine TF type from input/output variables
        let tf_type = self.determine_tf_type();

        // Calculate gain and get solution vector (for R_in calculation)
        let (gain, solution) = self.calculate_gain()?;

        // Calculate input resistance using same solution
        let input_resistance = self.calculate_input_resistance(&solution)?;

        // Calculate output resistance (requires new solve)
        let output_resistance = self.calculate_output_resistance()?;

        Ok(TransferFunctionAnalysisResult {
            gain,
            input_resistance,
            output_resistance,
            tf_type,
        })
    }

    /// Assembles DC stamps at the operating point for linearized system.
    ///
    /// This creates the Jacobian matrix (linearized system) at the DC operating point.
    /// We use the DC point values to update all devices, then collect their DC stamps.
    fn assemble_dc_stamps(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        context: &Context,
    ) -> crate::result::Result<Vec<crate::math::linear::Stamp<AnalogReference, f64>>> {
        // Create state buffer with DC operating point values
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        // Get DC values and populate state array
        let dc_values_iv = dc_point.as_iv(netlist);
        let mut dc_state_array = ndarray::Array1::zeros(size);
        for iv in dc_values_iv {
            if let Some(idx) = iv.reference.idx() {
                dc_state_array[idx] = iv.value;
            }
        }

        // Create a temporary state buffer for update_all
        // CircularArrayBuffer2::new(capacity, size) where:
        // - capacity = number of state snapshots to keep
        // - size = number of variables in each snapshot
        let mut state = crate::math::circular_array::CircularArrayBuffer2::new(1, size);
        state.push(&dc_state_array.view());

        // Update all devices at DC operating point
        circuit.update_all(&state, context);

        // Collect DC stamps (these are linearized around DC point)
        let mut all_stamps = Vec::new();
        for dc in circuit.all_runtimes() {
            all_stamps.extend(dc.load_dc(&state, context));
        }

        Ok(all_stamps)
    }

    /// Determines the type of transfer function based on input/output variables.
    fn determine_tf_type(&self) -> TransferType {
        let input_is_voltage = self.is_voltage_source();
        let output_is_voltage = self.options.output.is_node();

        match (input_is_voltage, output_is_voltage) {
            (true, true) => TransferType::VoltageGain,
            (true, false) => TransferType::Transconductance,
            (false, true) => TransferType::Transresistance,
            (false, false) => TransferType::CurrentGain,
        }
    }

    /// Checks if input source is a voltage source.
    ///
    /// Currently simplified - assumes branch naming convention.
    /// TODO: Implement proper source type detection.
    fn is_voltage_source(&self) -> bool {
        // Simplified: check if branch name starts with "V"
        self.options.input_source.component.starts_with('V')
    }

    /// Calculates transfer function gain.
    ///
    /// Returns (gain, solution_vector) tuple.
    /// Solution vector is reused for input resistance calculation.
    ///
    /// Algorithm (from ngspice):
    /// 1. Build Jacobian matrix at DC operating point (no RHS terms)
    /// 2. Apply ONLY unit excitation at input (all other RHS = 0)
    /// 3. Solve linearized system
    /// 4. Read output response
    fn calculate_gain(&mut self) -> crate::result::Result<(f64, Array1<f64>)> {
        use crate::math::faer::FaerSparseLinearSystem;
        use crate::math::linear::{LinearSystem, Stamp};

        // Build linear system with ONLY matrix stamps (no RHS)
        // Filter out RHS stamps - we only want the Jacobian matrix
        let all_stamps = Self::assemble_dc_stamps(self.circuit, &self.dc_point, &self.context)?;
        let matrix_only_stamps: Vec<_> = all_stamps
            .into_iter()
            .filter(|stamp| !matches!(stamp, Stamp::Rhs(_, _)))
            .collect();

        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(matrix_only_stamps);

        // Now apply ONLY the unit excitation at input
        let input_is_voltage = self.is_voltage_source();

        if input_is_voltage {
            // Voltage source: apply 1V by setting RHS[branch] = 1.0
            system.apply_stamps(vec![Stamp::Rhs(self.input_branch_ref.clone(), 1.0)]);
        } else {
            // Current source: apply 1A between source nodes
            // TODO: Get current source nodes and apply +1A / -1A
            return Err(crate::error::Error::simple(
                "TransferFunction",
                "Current source input not yet fully implemented",
            ));
        }

        // Solve: Y × V = RHS
        let solution = system.solve_with_backend(&self.symbolic_matrix)?;

        // Extract output from solution
        let output_value = if self.options.output.is_node() {
            // Output is voltage V(node) or V(n1, n2)
            let v_pos = if let Some(idx) = self.output_ref.idx() {
                solution[idx]
            } else {
                0.0
            };

            let v_neg = if let Some(ref_node) = &self.output_ref_node {
                if let Some(idx) = ref_node.idx() {
                    solution[idx]
                } else {
                    0.0
                }
            } else {
                0.0 // GND reference
            };

            v_pos - v_neg
        } else {
            // Output is current I(branch)
            if let Some(idx) = self.output_ref.idx() {
                solution[idx]
            } else {
                0.0
            }
        };

        // Gain = output_value / 1.0 (unit input)
        let gain = output_value;

        Ok((gain, solution))
    }

    /// Calculates input resistance from the gain solution.
    ///
    /// For voltage source: R_in = V_source / I_source = 1V / I_branch
    /// For current source: R_in = V_across / I_source
    fn calculate_input_resistance(&self, solution: &Array1<f64>) -> crate::result::Result<f64> {
        let input_is_voltage = self.is_voltage_source();

        if input_is_voltage {
            // Voltage source: R_in = -1.0 / I_branch
            // The current through voltage source branch tells us input current
            if let Some(idx) = self.input_branch_ref.idx() {
                let i_source = solution[idx];

                if i_source.abs() < 1e-20 {
                    // Open circuit - infinite resistance
                    Ok(1e20)
                } else {
                    // R_in = V / I, where V = 1.0 was applied
                    Ok(-1.0 / i_source)
                }
            } else {
                Ok(1e20) // No valid index
            }
        } else {
            // Current source: measure voltage across source
            // R_in = V_across / 1.0 (1A was applied)
            // TODO: Get current source nodes
            Ok(1e20) // Placeholder for current source
        }
    }

    /// Calculates output resistance with new solve.
    ///
    /// Applies unit perturbation at output and measures response.
    /// For voltage output: Apply 1A test current, measure voltage change
    /// For current output: Apply 1V test voltage, measure current change
    fn calculate_output_resistance(&mut self) -> crate::result::Result<f64> {
        use crate::math::faer::FaerSparseLinearSystem;
        use crate::math::linear::{LinearSystem, Stamp};

        // Build linear system (same as gain calculation)
        let stamps = Self::assemble_dc_stamps(self.circuit, &self.dc_point, &self.context)?;
        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(stamps);

        // Apply unit excitation at OUTPUT
        if self.options.output.is_node() {
            // Voltage output: apply 1A test current between output nodes
            let out_stamps = if let Some(ref_node) = &self.output_ref_node {
                // Differential: I flows from output to ref
                vec![
                    Stamp::Rhs(self.output_ref.clone(), -1.0),
                    Stamp::Rhs(ref_node.clone(), 1.0),
                ]
            } else {
                // Single-ended: I flows from output to GND
                vec![Stamp::Rhs(self.output_ref.clone(), -1.0)]
            };

            system.apply_stamps(out_stamps);

            // Solve
            let solution = system.solve_with_backend(&self.symbolic_matrix)?;

            // Measure voltage response
            let v_pos = if let Some(idx) = self.output_ref.idx() {
                solution[idx]
            } else {
                0.0
            };

            let v_neg = if let Some(ref_node) = &self.output_ref_node {
                if let Some(idx) = ref_node.idx() {
                    solution[idx]
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let v_response = v_pos - v_neg;

            // R_out = V_response / I_test
            // Note: Thévenin resistance has opposite sign from what we get
            Ok(-v_response / 1.0)
        } else {
            // Current output: apply 1V test voltage at branch
            system.apply_stamps(vec![Stamp::Rhs(self.output_ref.clone(), 1.0)]);

            // Solve
            let solution = system.solve_with_backend(&self.symbolic_matrix)?;

            // Measure current response
            let i_response = if let Some(idx) = self.output_ref.idx() {
                solution[idx]
            } else {
                0.0
            };

            if i_response.abs() < 1e-20 {
                Ok(1e20) // Open circuit
            } else {
                // R_out = V_test / I_response
                Ok(1.0 / i_response)
            }
        }
    }
}


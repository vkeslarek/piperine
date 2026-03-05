use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::tf::{
    TransferFunctionAnalysisOptions, TransferFunctionAnalysisResult, TransferType,
};
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::math::faer::FaerSymbolicMatrix;
use crate::math::linear::SymbolicMatrix;
use crate::solver::dc::DcSolver;
use crate::solver::{init_solver_configuration, Context};
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
    input_branch_ref: CircuitReference,
    output_ref: CircuitReference,
    output_ref_node: Option<CircuitReference>,
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

        // Resolve input source branch reference
        let input_branch_var = CircuitVariable::Branch(options.input_source.clone());
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

        // Resolve output reference node (for differential voltage)
        let output_ref_node = if let Some(ref_node) = &options.output_ref {
            let ref_var = CircuitVariable::Node(ref_node.clone());
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
    ) -> crate::result::Result<Vec<crate::math::linear::Stamp<CircuitReference, f64>>> {
        // Create state buffer with DC operating point values
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);
        let mut state = crate::math::circular_array::CircularArrayBuffer2::new(size, 2);

        // Initialize state with DC point values as initial guess
        let dc_values = dc_point.as_iv(netlist);
        let mut initial_state = ndarray::Array1::zeros(size);
        for iv in dc_values {
            if let Some(idx) = iv.reference.idx() {
                initial_state[idx] = iv.value;
            }
        }
        state.push(&initial_state.view());

        // Update all devices at DC operating point
        circuit.update_all(&state, context);

        // Collect DC stamps (these are linearized around DC point)
        let mut all_stamps = Vec::new();
        for dc in circuit.dc_runtimes().iter() {
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
    fn calculate_gain(&mut self) -> crate::result::Result<(f64, Array1<f64>)> {
        // TODO: Implement gain calculation
        Ok((1.0, Array1::zeros(self.symbolic_matrix.size())))
    }

    /// Calculates input resistance from the gain solution.
    fn calculate_input_resistance(&self, _solution: &Array1<f64>) -> crate::result::Result<f64> {
        // TODO: Implement input resistance calculation
        Ok(1000.0) // Placeholder
    }

    /// Calculates output resistance with new solve.
    fn calculate_output_resistance(&mut self) -> crate::result::Result<f64> {
        // TODO: Implement output resistance calculation
        Ok(500.0) // Placeholder
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::circuit::instance::CircuitInstance;
    use crate::circuit::netlist::{BranchIdentifier, CircuitVariable, GND};
    use crate::circuit::Circuit;
    use crate::math::unit::UnitExt;

    #[test]
    fn test_transfer_function_resistive_divider() {
        let mut v_out = GND;

        let mut circuit: CircuitInstance = Circuit::builder("TF Divider", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source("V1", v_in.clone(), GND, 1.0.V());
            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.resistor("R2", v_out.clone(), GND, 1.0.kOhms());
        })
        .into();

        let result = circuit
            .transfer_function(
                TransferFunctionAnalysisOptions {
                    output: CircuitVariable::Node(v_out),
                    output_ref: None,
                    input_source: BranchIdentifier::from_component("V1"),
                },
                Context::default(),
            )
            .unwrap()
            .solve()
            .unwrap();

        println!("TF Result:");
        println!("  Type: {}", result.tf_type);
        println!("  Gain: {:.6}", result.gain);
        println!("  R_in: {:.2} Ω", result.input_resistance);
        println!("  R_out: {:.2} Ω", result.output_resistance);

        // Expected values for voltage divider:
        // Gain = R2/(R1+R2) = 1k/(1k+1k) = 0.5
        // R_in = R1 + R2 = 2kΩ
        // R_out = R1||R2 = 500Ω

        // TODO: Uncomment when implementation is complete
        // assert!((result.gain - 0.5).abs() < 1e-6, "Gain should be 0.5");
        // assert!((result.input_resistance - 2000.0).abs() < 1.0, "R_in should be 2kΩ");
        // assert!((result.output_resistance - 500.0).abs() < 1.0, "R_out should be 500Ω");
    }
}

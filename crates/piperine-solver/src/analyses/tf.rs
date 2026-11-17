//! Transfer function (`.tf`) — the analysis options and the DC small-signal
//! driver: gain, input resistance, and output resistance from unit
//! excitations on the system linearized at the operating point.
//!
//! There is no per-analysis `TfContext`: MD-03 is superseded
//! (p6-cleanup-architecture D2). The input source and output measurement
//! travel on [`TransferFunctionAnalysisOptions`], and the shared
//! [`Context`](crate::analyses::Context) carries what every analysis shares.
use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::faer::FaerSymbolicMatrix;
use piperine_analog_solver::math::linear::{SymbolicLinearSystem, SymbolicMatrix};
use crate::prelude::DcAnalysisResult;
use crate::prelude::{TransferFunctionAnalysisResult, TransferType};

use ndarray::Array1;

// ── request/state ────────────────────────────────────────────────────────

/// Transfer Function analysis options.
///
/// Specifies the input source and output variable for transfer function analysis.
/// Transfer function analysis calculates DC small-signal transfer characteristics:
/// - **Gain:** dOutput/dInput
/// - **Input Resistance:** Resistance seen by the input source
/// - **Output Resistance:** Thévenin/Norton equivalent resistance at output
#[derive(Clone, Debug)]
pub struct TransferFunctionAnalysisOptions {
    /// Output variable to measure.
    ///
    /// Can be:
    /// - `AnalogVariable::Node(n)` for voltage at node n (referenced to GND)
    /// - `AnalogVariable::Branch(b)` for current through branch b
    pub output: AnalogVariable,

    /// Reference node for differential voltage measurement.
    ///
    /// - `None`: Single-ended measurement V(output) with implicit GND reference
    /// - `Some(n)`: Differential measurement V(output, n) = V(output) - V(n)
    ///
    /// Only used when output is a Node. Ignored for Branch outputs.
    pub output_ref: Option<NodeIdentifier>,

    /// Input source branch identifier.
    ///
    /// Identifies the voltage or current source to use as input.
    /// Examples:
    /// - `BranchIdentifier::from_component("V1")` for voltage source V1
    /// - `BranchIdentifier::from_component("I1")` for current source I1
    pub input_source: BranchIdentifier,
}


// ── driver ───────────────────────────────────────────────────────────────

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
    ) -> crate::core::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        // Solve DC operating point
        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;

        // Resolve input source branch reference
        let input_branch_var = AnalogVariable::Branch(options.input_source.clone());
        let input_branch_ref = circuit
            .netlist()
            .reference_for(&input_branch_var)
            .ok_or_else(|| {
                piperine_analog_solver::error::Error::simple(
                    piperine_analog_solver::error::SolverDomain::Tf,
                    format!(
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
                piperine_analog_solver::error::Error::simple(
                    piperine_analog_solver::error::SolverDomain::Tf,
                    "Output variable not found in circuit",
                )
            })?
            .clone();

        // Resolve output reference node (for differential voltage)
        let output_ref_node = if let Some(ref_node) = &options.output_ref {
            let ref_var = AnalogVariable::Node(ref_node.clone());
            Some(
                circuit
                    .netlist()
                    .reference_for(&ref_var)
                    .ok_or_else(|| {
                        piperine_analog_solver::error::Error::simple(
                            piperine_analog_solver::error::SolverDomain::Tf,
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
    pub fn solve(&mut self) -> crate::core::result::Result<TransferFunctionAnalysisResult> {
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
    /// This creates the Jacobian matrix (linearized system) at the DC operating
    /// point from the device assembly.
    fn assemble_dc_stamps(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        context: &Context,
    ) -> crate::core::result::Result<Vec<piperine_analog_solver::math::linear::Stamp<AnalogReference, f64>>> {
        // Create state buffer with DC operating point values
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        // Get DC values and populate state array
        let dc_values_iv = netlist.initial_values(dc_point.values());
        let mut dc_state_array = ndarray::Array1::zeros(size);
        for iv in dc_values_iv {
            if let Some(idx) = iv.reference.idx() {
                dc_state_array[idx] = iv.value;
            }
        }

        let bound = circuit.abi.as_mut().expect("a circuit of devices");
        let amb = piperine_analog_solver::device::Ambient {
            time: 0.0,
            freq: 0.0,
            is_static: true,
            gmin: context.tolerances.gmin,
            source_scale: 1.0,
        };
        bound.analog.assemble_dc(&dc_state_array.to_vec(), amb)
    }

    /// Determines the type of transfer function based on input/output variables.
    fn determine_tf_type(&self) -> TransferType {
        // TF only supports voltage-source inputs today (D5). The branch's
        // component name carries the convention; anything else is rejected
        // with a loud error in `new` / `calculate_gain`. We still call this
        // helper to classify the four transfer kinds for the result.
        let output_is_voltage = self.options.output.is_node();
        let input_is_voltage = self.input_is_voltage_source();
        match (input_is_voltage, output_is_voltage) {
            (true, true) => TransferType::VoltageGain,
            (true, false) => TransferType::Transconductance,
            (false, true) => TransferType::Transresistance,
            (false, false) => TransferType::CurrentGain,
        }
    }

    /// Whether the resolved input is a voltage source: the netlist gives a branch-current
    /// unknown only to a branch that forces a potential, so the resolved reference's kind is the
    /// classification (the component label is a design path such as `Top.v1`, whatever it is
    /// named). A current-source input has no branch unknown and fails to resolve in `new` (D5).
    fn input_is_voltage_source(&self) -> bool {
        self.input_branch_ref.is_branch()
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
    fn calculate_gain(&mut self) -> crate::core::result::Result<(f64, Array1<f64>)> {
        use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
        use piperine_analog_solver::math::linear::{LinearSystem, Stamp};

        // Build linear system with ONLY matrix stamps (no RHS)
        // Filter out RHS stamps - we only want the Jacobian matrix
        let all_stamps = Self::assemble_dc_stamps(self.circuit, &self.dc_point, &self.context)?;
        let matrix_only_stamps: Vec<_> = all_stamps
            .into_iter()
            .filter(|stamp| !matches!(stamp, Stamp::Rhs(_, _)))
            .collect();

        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(matrix_only_stamps);

        // Now apply ONLY the unit excitation at input.
        let input_is_voltage = self.input_is_voltage_source();

        if input_is_voltage {
            // Voltage source: apply 1V by setting RHS[branch] = 1.0
            system.apply_stamps(vec![Stamp::Rhs(self.input_branch_ref.clone(), 1.0)]);
        } else {
            return Err(piperine_analog_solver::error::Error::simple(
                piperine_analog_solver::error::SolverDomain::Tf,
                "TF: current-source input is not supported (D5)",
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

    /// Calculates input resistance from the gain solution: R_in = V_source
    /// / I_source = 1V / I_branch.
    ///
    /// PB-08 (P3b): `calculate_gain` (called before this, `run`'s call
    /// order at the top of this impl) already fails loud whenever
    /// `input_is_voltage_source()` is false — so by construction
    /// this function only ever runs for a voltage-source input. The
    /// previous code carried a dead `if input_is_voltage { .. } else {
    /// current-source placeholder returning 1e20 }` branch that could never
    /// execute (re-verified 2026-07-24, see `.specs/features/
    /// p3b-blocking-fixes/spec.md` Finding #3) — removed, with the
    /// invariant asserted so a future refactor that reintroduces a
    /// current-source path here fails loud instead of silently resurrecting
    /// the dead branch's wrong `1e20`.
    fn calculate_input_resistance(&self, solution: &Array1<f64>) -> crate::core::result::Result<f64> {
        debug_assert_eq!(
            self.input_is_voltage_source(),
            true,
            "calculate_input_resistance reached with a non-voltage input — \
             calculate_gain should have already rejected this (D5)"
        );
        // Voltage source: R_in = -1.0 / I_branch. The current through the
        // voltage-source branch tells us the input current.
        let Some(idx) = self.input_branch_ref.idx() else {
            return Ok(1e20); // No valid index — open circuit.
        };
        let i_source = solution[idx];
        if i_source.abs() < 1e-20 {
            Ok(1e20) // Open circuit - infinite resistance
        } else {
            Ok(-1.0 / i_source) // R_in = V / I, where V = 1.0 was applied
        }
    }

    /// Calculates output resistance with new solve.
    ///
    /// Applies unit perturbation at output and measures response.
    /// For voltage output: Apply 1A test current, measure voltage change
    /// For current output: Apply 1V test voltage, measure current change
    fn calculate_output_resistance(&mut self) -> crate::core::result::Result<f64> {
        use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
        use piperine_analog_solver::math::linear::{LinearSystem, Stamp};

        // Build the linear system the way the gain does: the Jacobian only. The right-hand side
        // of the operating point (a source's own value) is not part of the small-signal problem,
        // and would add the DC solution to the test response.
        let stamps = Self::assemble_dc_stamps(self.circuit, &self.dc_point, &self.context)?;
        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());
        system.apply_stamps(stamps.into_iter().filter(|stamp| !matches!(stamp, Stamp::Rhs(_, _))).collect());

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

// A.13 — `TransferFunctionSolver::new` historically called `eprintln!`
// twice to dump the DC operating point and output-reference resolution to
// stderr. That is debug noise in production. The regression test pins the
// contract: the source must contain zero `eprintln!` calls in non-test code.
// We assert on the literal source string instead of capturing stderr at
// runtime because the solver's `new` requires a fully built `CircuitInstance`,
// which would make a proper runtime test heavyweight.
//
// If you legitimately need to re-add a debug print, gate it behind
// `tracing::debug!` instead so it respects the standard logging machinery.
#[cfg(test)]
mod a13_no_debug_eprintln {
    #[test]
    fn no_eprintln_in_tf_solver_a13() {
        // Track when we enter / leave this test module so its own lines
        // (which mention the macro name in comments and assertions) do
        // not trip the check.
        let src = include_str!("tf.rs");
        let mut in_test_module = false;
        for (idx, line) in src.lines().enumerate() {
            if line.contains("mod a13_no_debug_eprintln") {
                in_test_module = true;
                continue;
            }
            if in_test_module {
                if line.starts_with("}") {
                    in_test_module = false;
                }
                continue;
            }
            let trimmed = line.trim_start();
            assert!(
                !trimmed.starts_with("eprintln!"),
                "A.13: `eprintln!` found at line {}: `{}`",
                idx + 1,
                trimmed
            );
            assert!(
                !trimmed.contains(" eprintln!"),
                "A.13: inline `eprintln!` found at line {}: `{}`",
                idx + 1,
                trimmed
            );
        }
    }
}

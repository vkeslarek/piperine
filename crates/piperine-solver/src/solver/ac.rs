use crate::analysis::ac::{
    AcAnalysisContext, AcAnalysisResult, AcAnalysisStep, AcSweepAnalysisOptions,
};
use crate::analysis::dc::DcAnalysisResult;
use crate::core::circuit::CircuitInstance;
use crate::analog::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::dc::DcSolver;
use crate::solver::Context;
use num_complex::Complex;
use num_traits::Zero;
use std::collections::HashMap;

/// Linear system representation for AC small-signal analysis.
///
/// AC analysis computes the small-signal frequency response of a circuit around
/// its DC operating point. The system is linearized, so Newton-Raphson iteration
/// typically converges in a single step.
pub struct AcSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub dc_point: DcAnalysisResult,
    pub frequency: f64,
}

impl<'a> NonLinearSystem<AnalogReference, Complex<f64>> for AcSystem<'a> {
    /// Assembles the linearized AC system matrix for the current frequency.
    ///
    /// AC analysis is inherently linear (small-signal approximation around DC bias),
    /// so this simply collects the complex-valued stamps from all AC-capable devices.
    /// No iterative updates are needed since the system doesn't change during solving.
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: self.frequency,
        };

        let mut all_stamps = Vec::new();

        // AC analysis is linear - no need to update runtimes
        // We use the DC operating point that was already computed
        for ac in &mut self.circuit.devices {
            all_stamps.extend(ac.load_ac(&self.dc_point, &ac_ctx, &self.context));
        }
        Ok(all_stamps)
    }
}

/// AC analysis solver for computing small-signal frequency response.
///
/// This solver performs AC sweep analysis, computing the circuit's response at
/// multiple frequency points. It first calculates the DC operating point, then
/// linearizes the circuit and solves the complex-valued linear system at each frequency.
pub struct AcSolver<'a> {
    pub system: AcSystem<'a>,
    pub solver:
        NewtonRaphsonSolver<AnalogReference, Complex<f64>, FaerSparseLinearSystem<Complex<f64>>>,
}

impl<'a> AcSolver<'a> {
    /// Creates a new AC solver and computes the DC operating point.
    ///
    /// # Process
    /// 1. Initialize solver configuration
    /// 2. Solve for DC operating point (required for linearization)
    /// 3. Set up complex-valued linear system
    /// 4. Initialize Newton-Raphson solver (converges in 1 iteration for linear systems)
    ///
    /// # Arguments
    /// * `circuit` - Circuit instance to analyze
    /// * `context` - Solver context with tolerances and limits
    ///
    /// # Returns
    /// Initialized AC solver ready for frequency sweep
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> crate::result::Result<Self> {
        Context::init_global();

        let mut dc_solver = DcSolver::new(circuit, context.clone())?;
        let dc_point = dc_solver.solve()?;

        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = AcSystem {
            circuit,
            context,
            dc_point,
            frequency: 0.0,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self { system, solver })
    }

    /// Performs AC frequency sweep analysis.
    ///
    /// Solves the linearized circuit at each frequency point specified in the options.
    /// The frequency points can be linearly or logarithmically spaced.
    ///
    /// # Process
    /// 1. Generate frequency points from options (linear or log spacing)
    /// 2. For each frequency:
    ///    - Update system frequency
    ///    - Solve linear system (single iteration, no Newton needed)
    ///    - Store complex voltages/currents
    /// 3. Return complete frequency response
    ///
    /// # Arguments
    /// * `options` - Sweep parameters (start/stop frequency, steps, spacing type)
    ///
    /// # Returns
    /// AC analysis result containing complex values at each frequency point
    pub fn solve_sweep(
        &mut self,
        options: AcSweepAnalysisOptions,
    ) -> crate::result::Result<AcAnalysisResult> {
        let frequencies = options.generate_frequencies();

        let mut data = Vec::new();

        for &f_hz in frequencies.iter() {
            self.system.frequency = f_hz;

            let max_iter = self.system.context.max_iter;
            let solution = self
                .solver
                .solve(&mut self.system, max_iter)?;

            let mut values = HashMap::new();
            for reference in self.system.circuit.netlist().all_references() {
                if let Some(idx) = reference.idx() {
                    values.insert(reference.variable().clone(), solution[idx]);
                }
            }
            data.push(AcAnalysisStep::new(f_hz, values));
        }

        Ok(AcAnalysisResult::new(
            data,
        ))
    }
}


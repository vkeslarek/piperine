#![allow(dead_code)]
use crate::analog::AnalogReference;
use crate::analyses::Context;
use crate::analyses::dc::{DcAnalysis, DcSolver};
use crate::core::circuit::CircuitInstance;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::DcAnalysisResult;
use crate::prelude::{AcAnalysisResult, AcAnalysisStep};

use num_complex::Complex;
use std::collections::HashMap;

// ── request/state ────────────────────────────────────────────────────────

pub struct AcAnalysisContext {
    pub frequency: f64,
}

pub trait AcAnalysis: DcAnalysis {
    fn load_ac(
        &mut self,
        dc_analysis_result: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex<f64>>>;
}

#[derive(Clone, Debug)]
pub struct AcSweepAnalysisOptions {
    pub start_frequency: f64,
    pub stop_frequency: f64,
    pub steps: usize,
    pub logarithmic: bool,
}

impl AcSweepAnalysisOptions {
    /// Generates frequency points for the sweep.
    ///
    /// # Returns
    ///
    /// A vector of frequencies distributed between `start_frequency` and `stop_frequency`.
    /// If `logarithmic` is true, uses logarithmic spacing; otherwise uses linear spacing.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let options = AcSweepAnalysisOptions {
    ///     start_frequency: 1.0,
    ///     stop_frequency: 1000.0,
    ///     steps: 3,
    ///     logarithmic: true,
    /// };
    /// let freqs = options.generate_frequencies();
    /// // freqs ≈ [1.0, 31.62, 1000.0] (logarithmic spacing)
    /// ```
    pub fn generate_frequencies(&self) -> Vec<f64> {
        if self.steps <= 1 {
            return vec![self.start_frequency];
        }

        (0..self.steps)
            .map(|i| {
                let ratio = i as f64 / (self.steps - 1) as f64;
                if self.logarithmic {
                    // Logarithmic spacing: f = f_start * (f_stop / f_start)^ratio
                    self.start_frequency * (self.stop_frequency / self.start_frequency).powf(ratio)
                } else {
                    // Linear spacing: f = f_start + (f_stop - f_start) * ratio
                    self.start_frequency + (self.stop_frequency - self.start_frequency) * ratio
                }
            })
            .collect()
    }
}



/// Per-analysis config for AC. Thin wrapper over the sweep options.
#[derive(Debug, Clone)]
pub struct AcContext {
    pub sweep: AcSweepAnalysisOptions,
}

// ── driver ───────────────────────────────────────────────────────────────

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

    fn netlist(&self) -> &crate::analog::Netlist {
        self.circuit.netlist()
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
    /// Convergence tunables (MD-04); AC is linear, so only `max_iter` matters.
    pub policy: crate::analyses::Policy,
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
        circuit.setup_all(&context)?;

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

        Ok(Self { system, solver, policy: crate::analyses::Policy::default() })
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

            let max_iter = self.policy.max_iter;
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

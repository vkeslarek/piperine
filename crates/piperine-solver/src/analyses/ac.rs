//! AC small-signal sweep (`.ac`) — the sweep options, plus the driver that linearizes the circuit around
//! the DC operating point and solves the complex system at each frequency.
//!
//! There is no per-analysis `AcContext`: MD-03 is superseded (p6-cleanup-
//! architecture D2). The sweep tunables travel as
//! [`AcSweepAnalysisOptions`] arguments, and the shared
//! [`Context`](crate::analyses::Context) carries what every analysis shares.
use piperine_analog_solver::device::Ambient;
use piperine_analog_solver::netlist::AnalogReference;
use crate::analyses::Context;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
use piperine_analog_solver::math::linear::Stamp;
use piperine_analog_solver::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::DcAnalysisResult;
use crate::prelude::{AcAnalysisResult, AcAnalysisStep};

use num_complex::Complex;
use std::collections::HashMap;

// ── request/state ────────────────────────────────────────────────────────

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
    /// Assembles the linearized AC system matrix for the current frequency from
    /// the device assembly: AC analysis is inherently linear (small-signal
    /// approximation around the DC bias), so no iterative update is needed.
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        self.assemble_device()
    }

    fn netlist(&self) -> &piperine_analog_solver::netlist::Netlist {
        self.circuit.netlist()
    }
}

/// The operating point as a vector by unknown index.
pub(crate) fn operating_vector(circuit: &CircuitInstance, dc_point: &DcAnalysisResult) -> Vec<f64> {
    let mut op = vec![0.0; circuit.netlist().max_index().map_or(0, |i| i + 1)];
    for reference in circuit.netlist().all_references() {
        if let (Some(i), Some(v)) = (reference.idx(), dc_point.values().get(reference.variable())) {
            op[i] = *v;
        }
    }
    op
}

impl AcSystem<'_> {
    /// The small-signal system of a circuit of devices at the operating point
    /// (abi.md §10 "Ac"): the matrix and the stimulus right side of the
    /// assembly. No reaction runs and no pin changes during a sweep.
    fn assemble_device(&mut self) -> crate::core::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        let op = operating_vector(self.circuit, &self.dc_point);
        let amb = Ambient {
            time: 0.0,
            freq: self.frequency,
            is_static: false,
            gmin: self.context.tolerances.gmin,
            source_scale: 1.0,
        };
        let bound = self.circuit.abi.as_mut().expect("a circuit of devices");
        let (mut matrix, rhs) = bound.analog.assemble_ac(&op, amb)?;
        matrix.extend(rhs);
        Ok(matrix)
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
    pub policy: piperine_analog_solver::tolerances::Policy,
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
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> crate::core::result::Result<Self> {
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

        Ok(Self { system, solver, policy: piperine_analog_solver::tolerances::Policy::default() })
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
    ) -> crate::core::result::Result<AcAnalysisResult> {
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

        let result = AcAnalysisResult::new(data);
        Ok(result)
    }
}

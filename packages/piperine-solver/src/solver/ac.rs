use crate::analysis::ac::{
    AcAnalysisContext, AcAnalysisResult, AcAnalysisStep, AcSweepAnalysisOptions,
};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::CircuitReference;
use crate::devices::soa::SoaViolations;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::math::unit::UnitExt;
use crate::solver::dc::DcSolver;
use crate::solver::{init_solver_configuration, Context};
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
    pub soa_violations: SoaViolations,
}

impl<'a> NonLinearSystem<CircuitReference, Complex<f64>> for AcSystem<'a> {
    /// Assembles the linearized AC system matrix for the current frequency.
    ///
    /// AC analysis is inherently linear (small-signal approximation around DC bias),
    /// so this simply collects the complex-valued stamps from all AC-capable devices.
    /// No iterative updates are needed since the system doesn't change during solving.
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
        _alpha: Complex<f64>,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: self.frequency.Hz(),
        };

        let mut all_stamps = Vec::new();

        // AC analysis is linear - no need to update runtimes
        // We use the DC operating point that was already computed
        for ac in self.circuit.ac_runtimes() {
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
        NewtonRaphsonSolver<CircuitReference, Complex<f64>, FaerSparseLinearSystem<Complex<f64>>>,
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
        init_solver_configuration();

        let mut dc_solver = DcSolver::new(circuit, context.clone())?;
        let dc_point = dc_solver.solve()?;

        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);
        let soa_violations = SoaViolations::from_vec(dc_point.soa_violations().clone());

        let mut system = AcSystem {
            circuit,
            context,
            dc_point,
            frequency: 0.0,
            soa_violations,
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
                .solve(&mut self.system, Complex::zero(), max_iter)?;

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
            self.system.soa_violations.clone(),
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::analysis::ac::AcSweepAnalysisOptions;
    use crate::circuit::instance::CircuitInstance;
    use crate::circuit::Circuit;
    use crate::solver::Context;

    #[test]
    fn test_ac_rc_filter() {
        use crate::circuit::netlist::GND;
        use crate::devices::source::Waveform::Sine;
        use crate::math::unit::UnitExt;

        let mut v_out = GND;

        let mut circuit: CircuitInstance = Circuit::builder("AC Low Pass", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source(
                "V1",
                v_in.clone(),
                GND,
                Sine {
                    amplitude: 1.0.V(),
                    frequency: 1.0.Hz(),
                    phase: 0.0.deg(),
                },
            );
            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.capacitor("C1", v_out.clone(), GND, 159.15.nF());
        })
        .into();

        let sweep_options = AcSweepAnalysisOptions {
            start_frequency: 100.0,
            stop_frequency: 10_000.0,
            steps: 21,
            logarithmic: true,
        };

        let result = circuit
            .ac(Context::default())
            .unwrap()
            .solve_sweep(sweep_options.clone())
            .unwrap();

        let frequencies = (0..sweep_options.steps)
            .map(|i| {
                let ratio = i as f64 / (sweep_options.steps - 1) as f64;
                sweep_options.start_frequency
                    * (sweep_options.stop_frequency / sweep_options.start_frequency).powf(ratio)
            })
            .collect::<Vec<f64>>();

        let mut found_cutoff = false;

        for i in 0..result.len() {
            let vector = result.get(i).unwrap();
            let f = frequencies[i];

            if (f - 1000.0).abs() < 1.0 {
                let v_out_value = vector.get_node(&v_out).unwrap();
                let mag = v_out_value.norm();

                println!("At {:.1} Hz: Mag = {:.4} V (Expected ~0.707)", f, mag);

                assert!(
                    (mag - 0.7071).abs() < 0.01,
                    "Filter cutoff magnitude incorrect. Got {:.4}",
                    mag
                );
                found_cutoff = true;
                break;
            }
        }

        assert!(found_cutoff, "Sweep did not cover 1kHz correctly.");
    }
}

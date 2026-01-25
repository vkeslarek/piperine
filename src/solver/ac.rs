use crate::analysis::ac::{AcAnalysisContext, AcAnalysisResult, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp2;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::math::unit::UnitExt;
use crate::solver::dc::DcSolver;
use crate::solver::{Context, init_solver_configuration};
use ndarray::{ArrayView1, ArrayViewMut1};
use num_complex::Complex;
use num_traits::Zero;

pub struct AcSystem<'a> {
    pub circuit: &'a mut Circuit,
    pub dc_point: DcAnalysisResult,
    pub frequency: f64,
}

impl<'a> NonLinearSystem<CircuitReference, Complex<f64>> for AcSystem<'a> {
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
        _alpha: Complex<f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp2<CircuitReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: self.frequency.Hz(),
        };

        let mut all_stamps = Vec::new();

        for (_name, comp) in self.circuit.components_mut() {
            if let Some(ac) = comp.as_ac() {
                ac.update_ac(&self.dc_point, &ac_ctx, context)?;

                all_stamps.extend(ac.load_ac(&self.dc_point, &ac_ctx, context));
            }
        }
        Ok(all_stamps)
    }

    fn converged(
        &self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
        _new_guess: &ArrayView1<Complex<f64>>,
        _context: &Context,
    ) -> bool {
        true
    }

    fn apply_limit(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
        _current_guess: ArrayViewMut1<Complex<f64>>,
        _context: &Context,
    ) {
    }

    fn update_sources(
        &mut self,
        _state: &mut CircularArrayBuffer2<Complex<f64>>,
        _context: &Context,
    ) {
    }
}

pub struct AcSolver<'a> {
    pub system: AcSystem<'a>,
    pub solver:
        NewtonRaphsonSolver<CircuitReference, Complex<f64>, FaerSparseLinearSystem<Complex<f64>>>,
}

impl<'a> AcSolver<'a> {
    pub fn new(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        init_solver_configuration();

        let mut dc_solver = DcSolver::new(circuit, context.clone())?;
        let dc_point = dc_solver.solve()?;

        let netlist = circuit.netlist();

        let mut mapped_vars: Vec<_> = netlist
            .all_references()
            .into_iter()
            .filter(|id| id.idx().is_some())
            .collect();
        mapped_vars.sort_by_key(|id| id.idx().unwrap());

        let size = mapped_vars
            .last()
            .map(|id| id.idx().unwrap() + 1)
            .unwrap_or(0);

        let mut system = AcSystem {
            circuit,
            dc_point,
            frequency: 0.0,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1, context)?;

        Ok(Self { system, solver })
    }

    pub fn solve_sweep(
        &mut self,
        options: AcSweepAnalysisOptions,
    ) -> crate::result::Result<AcAnalysisResult> {
        let frequencies = self.generate_frequencies(&options);

        let mut data = AcAnalysisResult::new(frequencies.len(), self.solver.state.size());

        for &f_hz in frequencies.iter() {
            self.system.frequency = f_hz;

            let solution = self.solver.solve(&mut self.system, Complex::zero())?;

            data.push(&solution.view());
        }

        Ok(data)
    }

    fn generate_frequencies(&self, opt: &AcSweepAnalysisOptions) -> Vec<f64> {
        if opt.steps <= 1 {
            return vec![opt.start_frequency];
        }

        (0..opt.steps)
            .map(|i| {
                let ratio = i as f64 / (opt.steps - 1) as f64;
                if opt.logarithmic {
                    opt.start_frequency * (opt.stop_frequency / opt.start_frequency).powf(ratio)
                } else {
                    opt.start_frequency + (opt.stop_frequency - opt.start_frequency) * ratio
                }
            })
            .collect()
    }
}

mod test {
    use crate::analysis::ac::AcSweepAnalysisOptions;
    use crate::circuit::Circuit;
    use crate::solver::{init_solver_configuration, Context};

    #[test]
    fn test_ac_rc_filter() {
        init_solver_configuration();

        use crate::circuit::netlist::{CircuitVariable, GND};
        use crate::devices::voltage_source::Waveform::Sine;
        use crate::math::unit::UnitExt;

        let mut circuit = Circuit::new("AC Low Pass");

        circuit.voltage_source(
            "V1",
            "in",
            GND,
            Sine {
                amplitude: 1.0.V(),
                frequency: 1.0.Hz(),
                phase: 0.0.deg(),
            },
        );
        circuit.resistor("R1", "in", "out", 1.0.kOhms());
        circuit.capacitor("C1", "out", GND, 159.15.nF());

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

        let out_var = circuit.netlist()
            .reference_for(&CircuitVariable::Node("out".into()))
            .expect("Output node not found")
            .variable();

        let frequencies = (0..sweep_options.steps)
            .map(|i| {
                let ratio = i as f64 / (sweep_options.steps - 1) as f64;
                sweep_options.start_frequency * (sweep_options.stop_frequency / sweep_options.start_frequency).powf(ratio)
            })
            .collect::<Vec<f64>>();

        let mut found_cutoff = false;

        for i in 0..result.len() {
            let lookback = result.len() - 1 - i;
            let vector = result.view(lookback).unwrap();
            let f = frequencies[i];

            if (f - 1000.0).abs() < 1.0 {
                let out_idx = circuit.netlist()
                    .reference_for(&CircuitVariable::Node("out".into()))
                    .unwrap()
                    .idx()
                    .unwrap();

                let v_out = vector[out_idx];
                let mag = v_out.norm();

                println!("At {:.1} Hz: Mag = {:.4} V (Expected ~0.707)", f, mag);

                assert!(
                    (mag - 0.7071).abs() < 0.01,
                    "Filter cutoff magnitude incorrect. Got {:.4}", mag
                );
                found_cutoff = true;
                break;
            }
        }

        assert!(found_cutoff, "Sweep did not cover 1kHz correctly.");
    }
}
use crate::analysis::ac::{AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{NoiseAnalysisOptions, NoiseAnalysisResult};
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::{LinearSystem, Stamp, SymbolicLinearSystem};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::math::unit::UnitExt;
use crate::solver::dc::DcSolver;
use crate::solver::{Context, init_solver_configuration};
use ndarray::{ArrayView1, ArrayViewMut1};
use num_complex::Complex;
use num_traits::Zero;

pub struct NoiseSystem<'a> {
    pub circuit: &'a mut Circuit,
    pub dc_point: DcAnalysisResult,
    pub frequency: f64,
}

impl<'a> NonLinearSystem<CircuitReference, Complex<f64>> for NoiseSystem<'a> {
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
        _alpha: Complex<f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: self.frequency.Hz(),
        };

        let mut all_stamps = Vec::new();

        for (_name, comp) in self.circuit.components_mut() {
            if let Some(ac) = comp.as_ac() {
                all_stamps.extend(ac.load_ac(&self.dc_point, &ac_ctx, context));
            }
        }
        Ok(all_stamps)
    }

    fn converged(
        &self,
        _: &CircularArrayBuffer2<Complex<f64>>,
        _: &ArrayView1<Complex<f64>>,
        _: &Context,
    ) -> bool {
        true
    }

    fn apply_limit(
        &mut self,
        _: &CircularArrayBuffer2<Complex<f64>>,
        _: ArrayViewMut1<Complex<f64>>,
        _: &Context,
    ) {
    }

    fn update_sources(&mut self, _: &mut CircularArrayBuffer2<Complex<f64>>, _: &Context) {}
}

pub struct NoiseSolver<'a> {
    pub system: NoiseSystem<'a>,
    pub solver:
        NewtonRaphsonSolver<CircuitReference, Complex<f64>, FaerSparseLinearSystem<Complex<f64>>>,
    pub options: NoiseAnalysisOptions,
    pub out_ref: CircuitReference,
    pub ref_ref: CircuitReference,
}

impl<'a> NoiseSolver<'a> {
    pub fn new(
        circuit: &'a mut Circuit,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        init_solver_configuration();

        let mut dc_solver = DcSolver::new(circuit, context.clone())?;
        let dc_point = dc_solver.solve()?;

        let mut mapped_vars: Vec<_> = circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|id| id.idx().is_some())
            .collect();
        mapped_vars.sort_by_key(|id| id.idx().unwrap());

        let size = mapped_vars
            .last()
            .map(|id| id.idx().unwrap() + 1)
            .unwrap_or(0);

        let out_ref = circuit
            .netlist()
            .reference_for(&CircuitVariable::Node(options.output_node.clone()))
            .ok_or(crate::error::Error::simple(
                "Output reference not found for identifier",
                "The output reference provided for the noise analysis doesn't exist on the circuit",
            ))?
            .clone();

        let ref_ref = circuit
            .netlist()
            .reference_for(&CircuitVariable::Node(options.reference_node.clone()))
            .ok_or(crate::error::Error::simple(
                "Reference node not found for identifier",
                "The reference node provided for the noise analysis doesn't exist on the circuit",
            ))?
            .clone();

        let mut system = NoiseSystem {
            circuit,
            dc_point,
            frequency: 0.0,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1, context)?;

        Ok(Self {
            system,
            solver,
            options,
            out_ref,
            ref_ref,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<NoiseAnalysisResult> {
        let frequencies = self.generate_frequencies(&self.options.sweep_options);
        let mut out_noise_sq = Vec::with_capacity(frequencies.len());

        for &f in &frequencies {
            self.system.frequency = f;
            let ac_ctx = AcAnalysisContext { frequency: f.Hz() };

            let stamps =
                self.system
                    .assemble(&self.solver.state, Complex::zero(), &self.solver.context)?;

            let mut adjoint_system =
                FaerSparseLinearSystem::<Complex<f64>>::new(self.solver.state.size());
            for stamp in stamps {
                match stamp {
                    Stamp::Matrix(r, c, val) => {
                        adjoint_system.apply_stamps(vec![Stamp::Matrix(c, r, val)]);
                    }
                    _ => {}
                }
            }

            adjoint_system.apply_stamps(vec![Stamp::Rhs(
                self.out_ref.clone(),
                Complex::new(1.0, 0.0),
            )]);
            adjoint_system.apply_stamps(vec![Stamp::Rhs(
                self.ref_ref.clone(),
                Complex::new(-1.0, 0.0),
            )]);

            let adjoint_solution = adjoint_system.solve_with_backend(&self.solver.symbolic)?;

            let mut total_density = 0.0;
            for comp in self.system.circuit.components_mut().values_mut() {
                if let Some(source) = comp.as_noise_source() {
                    let noises = source.noise_current_psd(&self.system.dc_point, &ac_ctx);
                    for n in noises {
                        let z_p = n
                            .terminals
                            .0
                            .idx()
                            .map(|i| adjoint_solution[i])
                            .unwrap_or_default();
                        let z_n = n
                            .terminals
                            .1
                            .idx()
                            .map(|i| adjoint_solution[i])
                            .unwrap_or_default();

                        let gain_sq = (z_p - z_n).norm_sqr();
                        total_density += gain_sq * n.value;
                    }
                }
            }
            out_noise_sq.push(total_density);
        }

        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);

        Ok(NoiseAnalysisResult {
            frequencies,
            out_noise_sq,
            integrated_noise,
        })
    }

    fn generate_frequencies(&self, opt: &AcSweepAnalysisOptions) -> Vec<f64> {
        if opt.steps <= 1 {
            return vec![opt.start_frequency];
        }

        let n_steps = opt.steps;
        let start = opt.start_frequency;
        let stop = opt.stop_frequency;

        (0..n_steps)
            .map(|i| {
                let ratio = i as f64 / (n_steps - 1) as f64;

                if opt.logarithmic {
                    start * (stop / start).powf(ratio)
                } else {
                    start + ratio * (stop - start)
                }
            })
            .collect()
    }

    fn integrate_noise(&self, freqs: &[f64], psd: &[f64]) -> f64 {
        let mut sum_sq = 0.0;
        for i in 0..freqs.len() - 1 {
            let df = freqs[i + 1] - freqs[i];
            let avg_psd = (psd[i] + psd[i + 1]) / 2.0;
            sum_sq += avg_psd * df;
        }
        sum_sq.sqrt()
    }
}

#[cfg(test)]
mod test {
    use crate::analysis::ac::AcSweepAnalysisOptions;
    use crate::analysis::noise::NoiseAnalysisOptions;
    use crate::devices::builder::CircuitBuilderExt;
    use crate::circuit::Circuit;
    use crate::circuit::netlist::GND;
    use crate::math::unit::UnitExt;
    use crate::solver::Context;

    #[test]
    fn test_noise_johnson_nyquist() {
        let mut circuit = Circuit::new("Noise Verification - RC");

        circuit
            .resistor("R1", "out", GND, 100.0.kOhms())
            .with_noise(true);
        circuit.capacitor("C1", "out", GND, 1.0.nF());

        let result = circuit
            .noise(
                NoiseAnalysisOptions {
                    sweep_options: AcSweepAnalysisOptions {
                        start_frequency: 1.0,
                        stop_frequency: 1.0e6,
                        steps: 500,
                        logarithmic: true,
                    },
                    output_node: "out".into(),
                    reference_node: GND.into(),
                    input_source_name: None,
                },
                Context::default(),
            )
            .unwrap()
            .solve()
            .unwrap();

        let k_b = 1.380649e-23;
        let temp = 300.15;
        let cap = 1.0e-9;
        let expected_rms = f64::sqrt(k_b * temp / cap);
        let simulated_rms = result.integrated_noise;

        println!(
            "Theory: {:.4} uV | Sim: {:.4} uV",
            expected_rms * 1e6,
            simulated_rms * 1e6
        );

        let error_pct = (simulated_rms - expected_rms).abs() / expected_rms * 100.0;
        assert!(
            error_pct < 2.0,
            "Noise simulation accuracy error: {:.2}%",
            error_pct
        );
    }
}

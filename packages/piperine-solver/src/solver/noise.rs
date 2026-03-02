use crate::analysis::ac::{AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{NoiseAnalysisOptions, NoiseAnalysisResult};
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicLinearSystem, SymbolicMatrix};
use crate::math::unit::UnitExt;
use crate::solver::dc::DcSolver;
use crate::solver::{init_solver_configuration, Context};
use ndarray::Array1;
use num_complex::Complex;

pub struct NoiseSolver<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub dc_point: DcAnalysisResult,
    pub symbolic_matrix: FaerSymbolicMatrix,
    pub options: NoiseAnalysisOptions,
    pub out_ref: CircuitReference,
    pub ref_ref: CircuitReference,
}

impl<'a> NoiseSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        init_solver_configuration();

        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;

        let (out_ref, ref_ref) = Self::resolve_nodes(circuit, &options)?;
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let symbolic_stamps = Self::assemble_linearized(circuit, &dc_point, 1.0, &context)?;
        let symbolic_matrix = FaerSymbolicMatrix::new(size, symbolic_stamps)?;

        Ok(Self {
            circuit,
            context,
            dc_point,
            symbolic_matrix,
            options,
            out_ref,
            ref_ref,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<NoiseAnalysisResult> {
        let frequencies = self.generate_frequencies(&self.options.sweep_options);
        let mut out_noise_sq = Vec::with_capacity(frequencies.len());

        for &f in &frequencies {
            let ac_ctx = AcAnalysisContext { frequency: f.Hz() };

            let stamps = Self::assemble_linearized(self.circuit, &self.dc_point, f, &self.context)?;
            let adjoint_sol = self.solve_adjoint_system(stamps)?;

            let mut step_density = 0.0;
            for source in self.circuit.noise_runtimes() {
                let noises = source.noise_current_psd(&self.dc_point, &ac_ctx);
                for n in noises {
                    let z_p = self
                        .out_ref
                        .idx()
                        .map(|i| adjoint_sol[i])
                        .unwrap_or_default();
                    let z_n = self
                        .ref_ref
                        .idx()
                        .map(|i| adjoint_sol[i])
                        .unwrap_or_default();
                    let gain_sq = (z_p - z_n).norm_sqr();

                    step_density += gain_sq * n.value;
                }
            }
            out_noise_sq.push(step_density);
        }

        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);
        Ok(NoiseAnalysisResult {
            frequencies,
            integrated_noise,
            out_noise_sq,
        })
    }

    fn solve_adjoint_system(
        &self,
        stamps: Vec<Stamp<CircuitReference, Complex<f64>>>,
    ) -> crate::result::Result<Array1<Complex<f64>>> {
        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());

        for stamp in stamps {
            if let Stamp::Matrix(r, c, val) = stamp {
                system.apply_stamps(vec![Stamp::Matrix(c, r, val)]);
            }
        }

        system.apply_stamps(vec![
            Stamp::Rhs(self.out_ref.clone(), Complex::new(1.0, 0.0)),
            Stamp::Rhs(self.ref_ref.clone(), Complex::new(-1.0, 0.0)),
        ]);

        system.solve_with_backend(&self.symbolic_matrix)
    }

    fn assemble_linearized(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        f_hz: f64,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: f_hz.Hz(),
        };
        Ok(circuit
            .ac_runtimes()
            .iter()
            .flat_map(|ac| ac.load_ac(dc_point, &ac_ctx, context))
            .collect())
    }

    fn resolve_nodes(
        circuit: &CircuitInstance,
        opt: &NoiseAnalysisOptions,
    ) -> crate::result::Result<(CircuitReference, CircuitReference)> {
        let net = circuit.netlist();
        let out = net
            .reference_for(&CircuitVariable::Node(opt.output_node.clone()))
            .ok_or_else(|| crate::error::Error::simple("Noise", "Output node not found"))?;
        let ref_ = net
            .reference_for(&CircuitVariable::Node(opt.reference_node.clone()))
            .ok_or_else(|| crate::error::Error::simple("Noise", "Reference node not found"))?;
        Ok((out.clone(), ref_.clone()))
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
    use crate::circuit::builder;
    use crate::circuit::instance::CircuitInstance;
    use crate::circuit::netlist::GND;
    use crate::math::unit::UnitExt;
    use crate::solver::Context;

    #[test]
    fn test_noise_johnson_nyquist() {
        let mut circuit: CircuitInstance = builder("Noise Verification - RC", |builder| {
            builder
                .resistor("R1", "out", GND, 100.0.kOhms())
                .with_noise(true);
            builder.capacitor("C1", "out", GND, 1.0.nF());
        })
        .into();

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

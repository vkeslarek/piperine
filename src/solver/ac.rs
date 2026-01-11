use crate::analysis::ac::{AcAnalysisContext, AcAnalysisResult, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use ndarray::{Array1, Array2};
use num_complex::Complex;
use std::collections::HashMap;

pub struct AcSolver<'a> {
    circuit: &'a mut Circuit,
    context: Context,
    symbolic: FaerSymbolicMatrix<CircuitReference>,
}

impl<'a> AcSolver<'a> {
    pub fn build(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        let symbols: Vec<_> = circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| s.is_dependent())
            .collect();

        // Dummy context for structural analysis
        let dummy_ctx = AcAnalysisContext {
            frequency: 1.0.Hz(),
        };
        let dummy_dc = DcAnalysisResult {
            values: Array1::zeros(symbols.len()),
            mapping: HashMap::new(),
        };

        let stamps = Self::linearize_ac(circuit, &context, &dummy_dc, &dummy_ctx)?;
        let symbolic = FaerSymbolicMatrix::new(symbols, stamps)?;

        Ok(Self {
            circuit,
            context,
            symbolic,
        })
    }

    pub fn solve_sweep(
        &mut self,
        options: AcSweepAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<AcAnalysisResult> {
        let dc_result = self.circuit.dc(context.clone())?.solve()?;

        let frequencies = self.generate_frequencies(&options);
        let mut data = Array2::zeros((frequencies.len(), self.symbolic.size()));

        for (idx, &f) in frequencies.iter().enumerate() {
            let ac_ctx = AcAnalysisContext { frequency: f.Hz() };
            let stamps = Self::linearize_ac(self.circuit, &self.context, &dc_result, &ac_ctx)?;

            let mut system = FaerLinearSystem::new(self.symbolic.size());
            system.apply_stamps(&self.symbolic, stamps);

            let solution = system.solve_with_backend(&self.symbolic)?;
            data.row_mut(idx).assign(&solution);
        }

        Ok(AcAnalysisResult {
            mapping: self.symbolic.mapping.clone(),
            frequencies,
            data,
        })
    }

    fn linearize_ac(
        circuit: &mut Circuit,
        context: &Context,
        dc_point: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let mut stamps = Vec::new();

        for comp in circuit.components_mut().values_mut() {
            if let Some(ac) = comp.as_ac() {
                stamps.extend(
                    ac.load_ac(dc_point, ac_ctx, &context)
                        .into_iter()
                        .filter(|s| !s.has_ground_node()),
                );
            }
        }
        Ok(stamps)
    }

    fn generate_frequencies(&self, opt: &AcSweepAnalysisOptions) -> Vec<f64> {
        (0..opt.steps)
            .map(|i| {
                let ratio = i as f64 / (opt.steps - 1).max(1) as f64;
                if opt.logarithmic {
                    opt.start_frequency * (opt.stop_frequency / opt.start_frequency).powf(ratio)
                } else {
                    opt.start_frequency + (opt.stop_frequency - opt.start_frequency) * ratio
                }
            })
            .collect()
    }
}

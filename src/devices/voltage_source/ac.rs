use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::math::linear::{Stamp, Stamp2};
use crate::solver::Context;
use num_complex::Complex;
use num_traits::One;

impl AcAnalysis for VoltageSource {
    fn load_ac(
        &self,
        _dc_analysis_result: &DcAnalysisResult,
        _ac_analysis_context: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp2<CircuitReference, Complex<f64>>> {
        let (mag, phase_rad) = match &self.waveform {
            Waveform::Sine {
                amplitude, phase, ..
            } => (*amplitude, *phase),
            Waveform::Step { final_value, .. } => (*final_value, 0.0),
            _ => (0.0, 0.0),
        };

        let phasor = Complex::from_polar(mag, phase_rad);

        vec![
            Stamp2::Matrix(self.branch.clone(), self.node_plus.clone(), Complex::one()),
            Stamp2::Matrix(
                self.branch.clone(),
                self.node_minus.clone(),
                -Complex::one(),
            ),
            Stamp2::Matrix(self.node_plus.clone(), self.branch.clone(), Complex::one()),
            Stamp2::Matrix(
                self.node_minus.clone(),
                self.branch.clone(),
                -Complex::one(),
            ),
            Stamp2::Rhs(self.branch.clone(), phasor),
        ]
    }
}

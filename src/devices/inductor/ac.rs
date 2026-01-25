use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::CircuitReference;
use crate::devices::inductor::Inductor;
use crate::math::linear::Stamp;
use crate::solver::Context;
use num_complex::Complex;

impl AcAnalysis for Inductor {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let omega = 2.0 * std::f64::consts::PI * ac_ctx.frequency;
        let impedance = Complex::new(0.0, omega * self.inductance);

        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.current_ref.clone(),
                Complex::new(1.0, 0.0),
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.current_ref.clone(),
                Complex::new(-1.0, 0.0),
            ),
            Stamp::Matrix(
                self.current_ref.clone(),
                self.node_plus.clone(),
                Complex::new(1.0, 0.0),
            ),
            Stamp::Matrix(
                self.current_ref.clone(),
                self.node_minus.clone(),
                Complex::new(-1.0, 0.0),
            ),
            Stamp::Matrix(
                self.current_ref.clone(),
                self.current_ref.clone(),
                -impedance,
            ),
        ]
    }
}

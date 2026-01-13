use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::CircuitReference;
use crate::devices::resistor::Resistor;
use crate::math::Stamp;
use crate::math::unit::Siemens;
use crate::solver::Context;
use num_complex::Complex;

impl AcAnalysis for Resistor {
    fn update_ac(
        &mut self,
        _: &DcAnalysisResult,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        self.model.clone().update_conductance(self, context);
        Ok(())
    }

    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        _: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let admittance = Complex::new(self.conductance.get::<Siemens>(), 0.0);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), admittance),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -admittance),
        ]
    }
}

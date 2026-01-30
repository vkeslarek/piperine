use crate::analysis::dc::DcAnalysisState;
use crate::circuit::netlist::CircuitReference;
use crate::devices::Runtime;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::solver::Context;

pub trait DcAnalysis2: Runtime {
    fn update_dc(
        &mut self,
        component: &Self::ComponentType,
        _dc_circuit_state: &DcAnalysisState,
        _context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        component: &Self::ComponentType,
        dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn initial_dc_values(
        &self,
        component: &Self::ComponentType,
        _context: &Context,
    ) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
    }
}

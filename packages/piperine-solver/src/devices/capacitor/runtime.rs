use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::{CircuitReference, Netlist};
use crate::devices::capacitor::Capacitor;
use crate::devices::soa::SoaCheck;
use crate::devices::Runtime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::Farad;
use crate::solver::Context;
use num_complex::Complex;
use std::sync::Arc;

pub struct CapacitorRuntime {
    component: Arc<Capacitor>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    capacitance: Farad,
}

impl Runtime for CapacitorRuntime {
    type ComponentType = Capacitor;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let node_plus = netlist.connect_node(component.node_plus.clone());
        let node_minus = netlist.connect_node(component.node_minus.clone());
        let capacitance = component.capacitance;

        Self {
            component,
            node_plus,
            node_minus,
            capacitance,
        }
    }

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, _: &Context) {
        // Do nothing
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        Some(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        Some(self)
    }

    fn as_noise_source(&self) -> Option<&dyn NoiseSource> {
        None
    }

    fn as_soa_check(&self) -> Option<&dyn SoaCheck> {
        None
    }
}

impl DcAnalysis for CapacitorRuntime {
    fn load_dc(
        &self,
        _dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // In DC analysis, capacitor is open circuit
        // Add gmin to prevent floating nodes
        let g = context.gmin;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
        ]
    }
}

impl AcAnalysis for CapacitorRuntime {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;
        let cap_val = self.capacitance;

        let admittance = Complex::new(0.0, omega * cap_val);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), admittance),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -admittance),
        ]
    }
}

impl TransientAnalysis for CapacitorRuntime {
    fn load_transient(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // For capacitor: i = C * dV/dt
        // The dynamic part (C matrix) is returned by load_transient_dynamic()
        // The solver will apply integration coefficients automatically
        // No additional static stamps needed
        vec![]
    }

    fn load_transient_dynamic(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // For capacitor: i = C * dV/dt
        // This returns the C matrix that multiplies the derivative vector
        let c = self.capacitance;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), c),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), c),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -c),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -c),
        ]
    }
}

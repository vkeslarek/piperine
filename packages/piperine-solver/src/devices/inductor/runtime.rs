use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::{BranchIdentifier, CircuitReference, Netlist};
use crate::devices::inductor::Inductor;
use crate::devices::soa::SoaCheck;
use crate::devices::Runtime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::Henry;
use crate::solver::Context;
use num_complex::Complex;
use std::sync::Arc;

pub struct InductorRuntime {
    component: Arc<Inductor>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    current_ref: CircuitReference,
    inductance: Henry,
}

impl Runtime for InductorRuntime {
    type ComponentType = Inductor;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let current_ref = BranchIdentifier::from_component(&component.name);

        let node_plus = netlist.connect_node(component.node_plus.clone());
        let node_minus = netlist.connect_node(component.node_minus.clone());
        let current_ref = netlist.connect_branch(current_ref);
        let inductance = component.inductance;

        Self {
            component,
            node_plus,
            node_minus,
            current_ref,
            inductance,
        }
    }

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, _: &Context)
    where
        Self: Sized,
    {
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

impl DcAnalysis for InductorRuntime {
    fn load_dc(&self, _: &DcAnalysisState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.current_ref.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.current_ref.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.current_ref.clone(), -1.0),
        ]
    }
}

impl AcAnalysis for InductorRuntime {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;
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

impl TransientAnalysis for InductorRuntime {
    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.node_plus.clone(), self.current_ref.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.current_ref.clone(), -1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_minus.clone(), -1.0),
        ]
    }

    fn load_transient_dynamic(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let l = self.inductance;

        vec![Stamp::Matrix(
            self.current_ref.clone(),
            self.current_ref.clone(),
            -l,
        )]
    }
}

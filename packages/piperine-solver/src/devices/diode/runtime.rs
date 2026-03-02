use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::{CircuitReference, Netlist};
use crate::devices::diode::Diode;
use crate::devices::soa::SoaCheck;
use crate::devices::Runtime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::{Ampere, Siemens};
use crate::solver::Context;
use num_complex::Complex;
use std::sync::Arc;

pub struct DiodeRuntime {
    component: Arc<Diode>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    g_eq: Siemens,
    i_eq: Ampere,
    v_d_prev: f64, // Previous diode voltage for damping
}

impl Runtime for DiodeRuntime {
    type ComponentType = Diode;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let node_plus = netlist.connect_node(component.node_plus.clone());
        let node_minus = netlist.connect_node(component.node_minus.clone());

        // Initialize with gmin to avoid open circuit
        // This helps Newton-Raphson convergence
        Self {
            component,
            node_plus,
            node_minus,
            g_eq: 1e-12, // Start with small conductance (gmin-like)
            i_eq: 0.0,
            v_d_prev: 0.0, // Start at zero bias
        }
    }

    fn update(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context)
    where
        Self: Sized,
    {
        let v_anode_new = match self.node_plus.idx() {
            Some(idx) => state
                .latest()
                .and_then(|val| val.get(idx).cloned())
                .unwrap_or(0.0),
            None => 0.0, // GND
        };

        let v_cathode_new = match self.node_minus.idx() {
            Some(idx) => state
                .latest()
                .and_then(|val| val.get(idx).cloned())
                .unwrap_or(0.0),
            None => 0.0, // GND
        };

        let v_anode_old = match self.node_plus.idx() {
            Some(idx) => state
                .view(1)
                .and_then(|val| val.get(idx).cloned())
                .unwrap_or(0.0),
            None => 0.0, // GND
        };

        let v_cathode_old = match self.node_minus.idx() {
            Some(idx) => state
                .view(1)
                .and_then(|val| val.get(idx).cloned())
                .unwrap_or(0.0),
            None => 0.0, // GND
        };

        let v_d_proposed = v_anode_new - v_cathode_new;
        let (g_eq, i_eq, v_d_damped) = self.component.model().get_g_eq_i_eq(
            v_d_proposed,
            self.v_d_prev, // Use stored previous value for proper damping
            context,
        );

        self.g_eq = g_eq;
        self.i_eq = i_eq;
        self.v_d_prev = v_d_damped; // Store the DAMPED value for next iteration!
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

impl DcAnalysis for DiodeRuntime {
    fn load_dc(
        &self,
        dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let g = self.g_eq;
        let i_rhs = self.i_eq;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
            Stamp::Rhs(self.node_plus.clone(), -i_rhs),
            Stamp::Rhs(self.node_minus.clone(), i_rhs),
        ]
    }
}

impl AcAnalysis for DiodeRuntime {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        _: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let g_d = Complex::new(self.g_eq, 0.0);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_d),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_d),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_d),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_d),
        ]
    }
}

impl TransientAnalysis for DiodeRuntime {
    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let g = self.g_eq;
        let i_rhs = self.i_eq;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
            Stamp::Rhs(self.node_plus.clone(), -i_rhs),
            Stamp::Rhs(self.node_minus.clone(), i_rhs),
        ]
    }
}

pub mod cap;
pub mod dio;
pub mod ind;
pub mod res;
pub mod vsrc;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::numerical_method::{GearMethod, NumericalMethod};
use crate::state::CircuitStates;
use std::collections::HashMap;

pub struct Context {
    pub numerical_method: &'static dyn NumericalMethod,
    pub gmin: f64,
    pub reltol: f64,
    pub abstol: f64,
    pub vntol: f64,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            numerical_method: &GearMethod(2),
            gmin: 1e-12,
            reltol: 1e-3,
            abstol: 1e-12,
            vntol: 1e-6,
        }
    }
}

pub trait Component {
    fn name(&self) -> String;

    fn commit(&mut self) -> crate::error::Result<()> {
        Ok(())
    }

    fn rollback(&mut self) -> crate::error::Result<()> {
        Ok(())
    }

    fn update(
        &mut self,
        circuit_states: &CircuitStates,
        context: &Context,
    ) -> crate::error::Result<()> {
        Ok(())
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        None
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        None
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        None
    }

    fn as_security_monitor(&self) -> Option<&dyn SecurityMonitor> {
        None
    }

    fn as_timestep_control(&self) -> Option<&dyn TimestepControl> {
        None
    }
}

pub struct Components {
    pub(crate) components: HashMap<String, Box<dyn Component>>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    pub fn add_component(&mut self, component: Box<dyn Component>) {
        self.components.insert(component.name(), component);
    }

    pub fn get_all(&self) -> Vec<&Box<dyn Component>> {
        self.components.values().collect()
    }
}

pub trait SecurityMonitor: Component {
    /// Safe Operating Area Check (e.g., checking if V > BV_MAX)
    fn check_soa(
        &self,
        circuit_states: &CircuitStates,
        context: &Context,
    ) -> crate::error::Result<()>;
}

pub trait TimestepControl: Component {
    /// Predict the maximum allowable next time-step based on local truncation error (LTE)
    fn truncate(&self, circuit_states: &CircuitStates, context: &Context) -> Option<f64>;
}

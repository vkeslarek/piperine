pub mod cap;
pub mod dio;
pub mod ind;
pub mod res;
pub mod vsrc;

use std::any::Any;
use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::numerical_method::{GearMethod, History, NumericalMethod};
use crate::state::CircuitStates;
use std::collections::HashMap;
use crate::circuit::Netlist;
use crate::experiment::ModelResolver;

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

    // fn ask(&self, measure: &Measure, states: &CircuitStates, _: &Context) -> Option<f64>;

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

pub trait ComponentBlueprint: Any {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
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

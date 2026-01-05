pub mod cap;
pub mod dio;
pub mod ind;
pub mod res;
pub mod vsrc;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::circuit::CircuitSpec;
use crate::component::cap::CapacitorSpec;
use crate::component::dio::DiodeSpec;
use crate::component::res::ResistorSpec;
use crate::component::vsrc::VoltageSourceSpec;
use crate::math::param::{IntoOptionalParameter, IntoParameter};
use crate::math::unit::{Capacitance, Inductance, Resistance, Voltage};
use crate::model::ModelResolver;
use crate::netlist::{IntoNodeIdentifier, Netlist};
use crate::solver::Context;
use std::any::Any;

pub trait Component {
    fn name(&self) -> String;

    fn commit(&mut self) -> crate::error::Result<()> {
        Ok(())
    }

    fn rollback(&mut self) -> crate::error::Result<()> {
        Ok(())
    }

    fn update(&mut self) -> crate::error::Result<()> {
        Ok(())
    }

    fn as_dc_mut(&mut self) -> Option<&mut dyn DcAnalysis> {
        None
    }

    fn as_transient_mut(&mut self) -> Option<&mut dyn TransientAnalysis> {
        None
    }

    fn as_ac_mut(&mut self) -> Option<&mut dyn AcAnalysis> {
        None
    }
}

pub trait ComponentSpec: Any {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait StandardComponentsSpec {
    fn resistor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl IntoOptionalParameter<Resistance>,
    ) -> &mut ResistorSpec;
    fn voltage_source(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        voltage: impl IntoParameter<Voltage>,
    ) -> &mut VoltageSourceSpec;
    fn capacitor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        capacitance: impl IntoParameter<Capacitance>,
    ) -> &mut CapacitorSpec;
    fn inductor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        inductance: impl IntoParameter<Inductance>,
    );
    fn diode(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut DiodeSpec;
}

impl StandardComponentsSpec for CircuitSpec {
    fn resistor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl IntoOptionalParameter<Resistance>,
    ) -> &mut ResistorSpec {
        self.insert_get(name, ResistorSpec::new(name, node_p, node_n, resistance))
            .expect("Failed to insert resistor")
    }

    fn voltage_source(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        voltage: impl IntoParameter<Voltage>,
    ) -> &mut VoltageSourceSpec {
        self.insert_get(name, VoltageSourceSpec::new(name, node_p, node_n, voltage))
            .expect("Failed to insert voltage source")
    }

    fn capacitor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        capacitance: impl IntoParameter<Capacitance>,
    ) -> &mut CapacitorSpec {
        self.insert_get(name, CapacitorSpec::new(name, node_p, node_n, capacitance))
            .expect("Failed to insert capacitor")
    }

    fn inductor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        inductance: impl IntoParameter<Inductance>,
    ) {
        todo!()
    }

    fn diode(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut DiodeSpec {
        self.insert_get(name, DiodeSpec::new(name, node_p, node_n))
            .expect("Failed to insert diode")
    }
}

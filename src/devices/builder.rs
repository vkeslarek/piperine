use crate::circuit::Circuit;
use crate::circuit::netlist::IntoNodeIdentifier;
use crate::devices::Model;
use crate::devices::capacitor::Capacitor;
use crate::devices::diode::Diode;
use crate::devices::inductor::Inductor;
use crate::devices::resistor::Resistor;
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::math::unit::{Farad, Henry, Ohm};
use std::sync::Arc;

pub trait CircuitBuilderExt {
    fn model<M: Model>(&mut self, name: impl Into<String>, model: M) -> Arc<M>;
    fn capacitor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        capacitance: impl Into<Farad>,
    ) -> &mut Capacitor;
    fn diode(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut Diode;
    fn inductor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        inductance: impl Into<Henry>,
    ) -> &mut Inductor;
    fn resistor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl Into<Option<Ohm>>,
    ) -> &mut Resistor;
    fn voltage_source(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        waveform: impl Into<Waveform>,
    ) -> &mut VoltageSource;
}

impl CircuitBuilderExt for Circuit {
    fn model<M: Model>(&mut self, name: impl Into<String>, model: M) -> Arc<M> {
        let instance = Arc::new(model);
        self.add_model(name, instance.clone());
        instance
    }

    fn capacitor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        capacitance: impl Into<Farad>,
    ) -> &mut Capacitor {
        let name = name.into();
        let instance = Capacitor::new(
            name.clone(),
            node_p,
            node_n,
            capacitance.into(),
            &mut self.netlist_mut(),
        );
        self.add_component(name, instance)
    }

    fn diode(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut Diode {
        let name = name.into();
        let instance = Diode::new(name.clone(), node_p, node_n, &mut self.netlist_mut());
        self.add_component(name, instance)
    }

    fn inductor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        inductance: impl Into<Henry>,
    ) -> &mut Inductor {
        let name = name.into();
        let instance = Inductor::new(
            name.clone(),
            node_p,
            node_n,
            inductance.into(),
            &mut self.netlist_mut(),
        );

        self.add_component(name, instance)
    }

    fn resistor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl Into<Option<Ohm>>,
    ) -> &mut Resistor {
        let name = name.into();
        let instance = Resistor::new(
            name.clone(),
            node_p,
            node_n,
            resistance.into(),
            &mut self.netlist_mut(),
        );
        self.add_component(name, instance)
    }

    fn voltage_source(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        waveform: impl Into<Waveform>,
    ) -> &mut VoltageSource {
        let name = name.into();
        let instance = VoltageSource::new(
            name.clone(),
            node_p,
            node_n,
            waveform.into(),
            &mut self.netlist_mut(),
        );
        self.add_component(name, instance)
    }
}

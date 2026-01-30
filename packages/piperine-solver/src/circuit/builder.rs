use crate::circuit::netlist::{IntoNodeIdentifier, Netlist};
use crate::circuit::Circuit;
use crate::devices::capacitor::Capacitor;
use crate::devices::diode::Diode;
use crate::devices::dynamic::Dynamic;
use crate::devices::inductor::Inductor;
use crate::devices::resistor::Resistor;
use crate::devices::source::{VoltageSource, Waveform};
use crate::devices::{AnyModel, Component, Model};
use crate::math::unit::{Farad, Henry, Ohm};
use std::collections::HashMap;
use std::sync::Arc;

pub struct CircuitBuilder {
    title: String,
    netlist: Netlist,
    models: HashMap<String, Arc<dyn AnyModel>>,
    components: HashMap<String, Box<dyn Component>>,
}

impl CircuitBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            netlist: Netlist::new(),
            models: HashMap::new(),
            components: HashMap::new(),
        }
    }

    pub fn add_component<B: Component>(&mut self, name: impl Into<String>, component: B) -> &mut B {
        let name_str = name.into();

        self.components
            .insert(name_str.clone(), Box::new(component));

        let boxed = self.components.get_mut(&name_str).unwrap();

        let any_mut = boxed.as_any_mut();

        match any_mut.downcast_mut::<B>() {
            Some(concrete) => concrete,
            None => {
                panic!(
                    "Downcast failed for component '{}'. Expected type {}, but found something else.",
                    name_str,
                    std::any::type_name::<B>()
                );
            }
        }
    }

    pub fn add_model(&mut self, name: impl Into<String>, model: Arc<dyn AnyModel>) {
        self.models.insert(name.into(), model);
    }

    pub fn model<M: Model>(&mut self, name: impl Into<String>, model: M) -> Arc<M> {
        let instance = Arc::new(model);
        self.add_model(name, instance.clone());
        instance
    }

    pub fn capacitor(
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
            &mut self.netlist,
        );
        self.add_component(name, instance)
    }

    pub fn diode(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut Diode {
        let name = name.into();
        let instance = Diode::new(name.clone(), node_p, node_n, &mut self.netlist);
        self.add_component(name, instance)
    }

    pub fn inductor(
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
            &mut self.netlist,
        );

        self.add_component(name, instance)
    }

    pub fn resistor(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl Into<Dynamic<Ohm>>,
    ) -> &mut Resistor {
        let name = name.into();
        let instance = Resistor::new(
            name.clone(),
            node_p,
            node_n,
            resistance.into(),
            &mut self.netlist,
        );
        self.add_component(name, instance)
    }

    pub fn voltage_source(
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
            &mut self.netlist,
        );
        self.add_component(name, instance)
    }
}

impl Into<Circuit> for CircuitBuilder {
    fn into(self) -> Circuit {
        Circuit::from_raw(self.title, self.netlist, self.models, self.components)
    }
}

pub trait IntoCircuit {
    fn into_circuit(self, title: impl Into<String>) -> Circuit;
}

impl IntoCircuit for Circuit {
    fn into_circuit(self, _: impl Into<String>) -> Circuit {
        self.into()
    }
}

impl<F> IntoCircuit for F
where
    F: FnOnce(&mut CircuitBuilder),
{
    fn into_circuit(self, title: impl Into<String>) -> Circuit {
        let mut builder = CircuitBuilder::new(title);
        self(&mut builder);
        builder.into()
    }
}

pub fn builder<F: FnOnce(&mut CircuitBuilder)>(
    title: impl Into<String>,
    builder_fn: F,
) -> CircuitBuilder {
    let mut builder = CircuitBuilder::new(title);
    builder_fn(&mut builder);
    builder
}

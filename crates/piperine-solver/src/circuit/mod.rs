use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::{IntoNodeIdentifier, NodeIdentifier};
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
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod instance;
pub mod netlist;

pub struct Circuit {
    title: String,
    models: HashMap<String, Arc<dyn AnyModel>>,
    components: HashMap<String, Box<dyn Component>>,
    node_counter: AtomicUsize,
    scope_stack: Vec<String>,
}

impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            models: HashMap::new(),
            components: HashMap::new(),
            node_counter: AtomicUsize::new(0),
            scope_stack: Vec::new(),
        }
    }

    pub fn add_component<B: Component>(&mut self, name: impl Into<String>, component: B) -> &mut B {
        let name_str = name.into();

        // Apply scope prefix if we're inside a scope
        let prefixed_name = if self.scope_stack.is_empty() {
            name_str.clone()
        } else {
            format!("{}.{}", self.scope_stack.join("."), name_str)
        };

        self.components
            .insert(prefixed_name.clone(), Box::new(component));

        let boxed = self.components.get_mut(&prefixed_name).unwrap();

        let any_mut = boxed.as_any_mut();

        match any_mut.downcast_mut::<B>() {
            Some(concrete) => concrete,
            None => {
                panic!(
                    "Downcast failed for component '{}'. Expected type {}, but found something else.",
                    prefixed_name,
                    std::any::type_name::<B>()
                );
            }
        }
    }

    pub fn add_model(&mut self, name: impl Into<String>, model: Arc<dyn AnyModel>) {
        self.models.insert(name.into(), model);
    }

    pub fn port(&self) -> NodeIdentifier {
        NodeIdentifier::Anonymous(self.node_counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn scoped<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.scope_stack.push(name.to_string());
        let result = f(self);
        self.scope_stack.pop();
        result
    }

    pub fn subcircuit(&mut self, name: &str, subcircuit: Circuit) {
        // Enter the scope for this subcircuit
        self.scope_stack.push(name.to_string());

        // Merge all components from the subcircuit
        for (comp_name, component) in subcircuit.components {
            // Component names will be prefixed by add_component
            self.components.insert(
                if self.scope_stack.is_empty() {
                    comp_name
                } else {
                    format!("{}.{}", self.scope_stack.join("."), comp_name)
                },
                component,
            );
        }

        // Merge all models
        self.models.extend(subcircuit.models);

        // Exit the scope
        self.scope_stack.pop();
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
        let instance = Capacitor::new(name.clone(), node_p, node_n, capacitance.into());
        self.add_component(name, instance)
    }

    pub fn diode(
        &mut self,
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut Diode {
        let name = name.into();
        let instance = Diode::new(name.clone(), node_p, node_n);
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
        let instance = Inductor::new(name.clone(), node_p, node_n, inductance.into());

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
        let instance = Resistor::new(name.clone(), node_p, node_n, resistance.into());
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
        let instance = VoltageSource::new(name.clone(), node_p, node_n, waveform.into());
        self.add_component(name, instance)
    }

    pub fn components(&self) -> &HashMap<String, Box<dyn Component>> {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut HashMap<String, Box<dyn Component>> {
        &mut self.components
    }

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn builder<F: FnOnce(&mut Circuit)>(title: impl Into<String>, builder_fn: F) -> Circuit {
        let mut circuit = Circuit::new(title);
        builder_fn(&mut circuit);
        circuit
    }
}

impl Into<CircuitInstance> for Circuit {
    fn into(self) -> CircuitInstance {
        CircuitInstance::instantiate(&self).expect("Failed to instantiate circuit")
    }
}

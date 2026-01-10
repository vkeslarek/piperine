use crate::analysis::dc::DcSolver;
use crate::analysis::transient::{TransientAnalysisOptions, TransientSolver};
use crate::circuit::netlist::{IntoNodeIdentifier, Netlist};
use crate::devices::capacitor::Capacitor;
use crate::devices::diode::Diode;
use crate::devices::resistor::Resistor;
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::devices::{AnyModel, Component};
use crate::math::unit::{Capacitance, Resistance};
use crate::solver::Context;
use crate::solver::dc::DcSolverImpl;
use crate::solver::transient::TransientSolverImpl;
use crate::util::AsAny;
use std::collections::HashMap;
use std::sync::Arc;

pub mod netlist;
pub mod state;

pub struct Circuit {
    title: String,
    netlist: Netlist,
    models: HashMap<String, Arc<dyn AnyModel>>,
    components: HashMap<String, Box<dyn Component>>,
}

impl Circuit {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            netlist: Netlist::new(),
            models: HashMap::new(),
            components: HashMap::new(),
        }
    }

    pub fn insert_get<B: Component>(&mut self, name: &str, component: B) -> &mut B {
        let name_str = name.to_string();

        self.components
            .insert(name_str.clone(), Box::new(component));

        let boxed = self.components.get_mut(&name_str).unwrap();

        // We get the &mut dyn Any from the component
        let any_mut = boxed.as_any_mut();

        // Attempt the downcast
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

    pub fn model(&mut self, name: &str, model: impl AnyModel) {
        self.models.insert(name.to_string(), Arc::new(model));
    }

    pub fn netlist(&self) -> &Netlist {
        &self.netlist
    }

    pub fn components(&self) -> &HashMap<String, Box<dyn Component>> {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut HashMap<String, Box<dyn Component>> {
        &mut self.components
    }

    pub fn dc(self, context: Context) -> crate::result::Result<impl DcSolver> {
        DcSolverImpl::build(self, context)
    }

    pub fn transient(
        self,
        transient_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<impl TransientSolver> {
        TransientSolverImpl::build(self, transient_options, context)
    }

    pub fn resistor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl Into<Option<Resistance>>,
    ) -> &mut Resistor {
        let instance = Resistor::new(name, node_p, node_n, resistance.into(), &mut self.netlist);
        self.insert_get(name, instance)
    }

    pub fn voltage_source(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        waveform: impl Into<Waveform>,
    ) -> &mut VoltageSource {
        let instance = VoltageSource::new(name, node_p, node_n, waveform.into(), &mut self.netlist);
        self.insert_get(name, instance)
    }

    pub fn capacitor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        capacitance: impl Into<Capacitance>,
    ) -> &mut Capacitor {
        let instance = Capacitor::new(name, node_p, node_n, capacitance.into(), &mut self.netlist);
        self.insert_get(name, instance)
    }
    //
    // pub fn inductor(
    //     &mut self,
    //     name: &str,
    //     node_p: impl IntoNodeIdentifier,
    //     node_n: impl IntoNodeIdentifier,
    //     inductance: impl IntoParameter<Inductance>,
    // ) -> &mut InductorSpec {
    //     self.insert_get(name, InductorSpec::new(name, node_p, node_n, inductance))
    //         .expect("Failed to insert Inductor")
    // }
    //
    pub fn diode(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut Diode {
        let instance = Diode::new(name, node_p, node_n, &mut self.netlist);
        self.insert_get(name, instance)
    }
}

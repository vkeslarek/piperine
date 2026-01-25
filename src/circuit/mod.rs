use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::netlist::{IntoNodeIdentifier, Netlist};
use crate::devices::capacitor::Capacitor;
use crate::devices::diode::Diode;
use crate::devices::resistor::Resistor;
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::devices::{AnyModel, Component};
use crate::math::unit::{Farad, Ohm};
use crate::solver::Context;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::transient::TransientSolver;
use std::collections::HashMap;
use std::sync::Arc;
use crate::analysis::noise::NoiseAnalysisOptions;
use crate::solver::noise::NoiseSolver;

pub mod netlist;

pub struct Circuit {
    title: String,
    netlist: Netlist,
    models: HashMap<String, Arc<dyn AnyModel>>,
    components: HashMap<String, Box<dyn Component>>,
}

impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            netlist: Netlist::new(),
            models: HashMap::new(),
            components: HashMap::new(),
        }
    }

    pub fn insert_get<B: Component>(&mut self, name: impl Into<String>, component: B) -> &mut B {
        let name_str = name.into();

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

    pub fn model(&mut self, name: impl Into<String>, model: impl AnyModel) {
        self.models.insert(name.into(), Arc::new(model));
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

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn ac(&mut self, context: Context) -> crate::result::Result<AcSolver<'_>> {
        AcSolver::new(self, context)
    }

    pub fn dc(&mut self, context: Context) -> crate::result::Result<DcSolver<'_>> {
        DcSolver::new(self, context)
    }

    pub fn noise(
        &mut self,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<NoiseSolver<'_>> {
        NoiseSolver::new(self, options, context)
    }

    pub fn transient(
        &mut self,
        transient_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<TransientSolver<'_>> {
        TransientSolver::new(self, transient_options, context)
    }

    pub fn resistor(
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
            &mut self.netlist,
        );
        self.insert_get(name, instance)
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
        self.insert_get(name, instance)
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
        name: impl Into<String>,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> &mut Diode {
        let name = name.into();
        let instance = Diode::new(name.clone(), node_p, node_n, &mut self.netlist);
        self.insert_get(name, instance)
    }
}

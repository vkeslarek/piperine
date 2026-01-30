use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::netlist::Netlist;
use crate::devices::{AnyModel, Component};
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::transient::TransientSolver;
use crate::solver::Context;
use std::collections::HashMap;
use std::sync::Arc;

pub mod builder;
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

    pub fn from_raw(
        title: String,
        netlist: Netlist,
        models: HashMap<String, Arc<dyn AnyModel>>,
        components: HashMap<String, Box<dyn Component>>,
    ) -> Self {
        Self {
            title,
            netlist,
            models,
            components,
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

    pub fn netlist(&self) -> &Netlist {
        &self.netlist
    }

    pub fn netlist_mut(&mut self) -> &mut Netlist {
        &mut self.netlist
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
}

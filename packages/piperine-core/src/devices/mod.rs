pub mod capacitor;
pub mod diode;
pub mod inductor;
pub mod resistor;
pub mod voltage_source;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::error::ErrorDetail;
use crate::math::param::{IntoOptionalParameter, IntoParameter};
use crate::netlist::{IntoNodeIdentifier, Netlist};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;

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

pub trait Model: Debug {
    type ComponentType: Component;

    fn update(&self, component: &mut Self::ComponentType) -> crate::error::Result<()> {
        Ok(())
    }
}

pub trait AnyModel: 'static + Any {
    fn as_any(&self) -> &dyn Any;
    fn name(&self) -> String;
}

impl<M: 'static + Model> AnyModel for M {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> String {
        M::name(self)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ModelProviderCapabilities {
    INSERT,
    FETCH,
}

pub trait ModelProvider {
    fn fetch(&self, name: &str) -> Option<Arc<dyn AnyModel>>;
    fn insert(&mut self, name: &str, model: Arc<dyn AnyModel>);
    fn capabilities(&self) -> HashSet<ModelProviderCapabilities>;
}

pub struct LocalProvider {
    storage: HashMap<String, Arc<dyn AnyModel>>,
}

impl LocalProvider {
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }
}

impl ModelProvider for LocalProvider {
    fn fetch(&self, name: &str) -> Option<Arc<dyn AnyModel>> {
        self.storage.get(name).cloned()
    }

    fn insert(&mut self, name: &str, model: Arc<dyn AnyModel>) {
        self.storage.insert(name.to_string(), model);
    }

    fn capabilities(&self) -> HashSet<ModelProviderCapabilities> {
        HashSet::from_iter(vec![
            ModelProviderCapabilities::INSERT,
            ModelProviderCapabilities::FETCH,
        ])
    }
}

pub struct ModelResolver {
    provider: Box<dyn ModelProvider>,
    model_cache: HashMap<String, Arc<dyn AnyModel>>,
}

impl ModelResolver {
    pub(crate) fn new() -> ModelResolver {
        ModelResolver {
            provider: Box::new(LocalProvider::new()),
            model_cache: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, model: Arc<dyn AnyModel>) -> crate::error::Result<()> {
        if self
            .provider
            .capabilities()
            .contains(&ModelProviderCapabilities::INSERT)
        {
            self.model_cache.insert(name, model.clone());
            Ok(())
        } else {
            Err(ErrorDetail {
                title: "Model provider has no capabilities for this operation".to_string(),
                detail: "The model provider doesn't support inserting new models".to_string(),
                problems: vec![],
            })
        }
    }

    pub fn resolve<C: Component + 'static>(
        &self,
        model: Option<String>,
    ) -> Option<Arc<dyn Model<ComponentType = C>>> {
        // Handles the default case -> TODO

        let model = self
            .model_cache
            .get(&model.clone()?)
            .cloned()
            .or_else(|| self.provider.fetch(&model?));

        model.and_then(|mdl| mdl.as_any().downcast_ref().cloned())
    }
}

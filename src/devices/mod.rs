pub mod capacitor;
pub mod diode;
pub mod resistor;
pub mod voltage_source;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::TransientAnalysis;
use crate::circuit::netlist::Netlist;
use crate::error::Error;
use crate::util::AsAny;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;

pub trait Component: Any + AsAny {
    fn name(&self) -> String;

    fn as_dc(&mut self) -> Option<&mut dyn DcAnalysis>;

    fn as_ac(&mut self) -> Option<&mut dyn AcAnalysis>;

    fn as_transient(&mut self) -> Option<&mut dyn TransientAnalysis>;

    fn as_noise_source(&mut self) -> Option<&mut dyn NoiseSource> {
        None
    }
}

pub trait ComponentSpec: Any {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::result::Result<Box<dyn Component>>;
}

pub trait Model: Debug + AsAny + Any {
    type ComponentType: Component;
}

pub trait AnyModel: 'static + AsAny {}

impl<M: 'static + Model> AnyModel for M {}

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

    pub fn insert(&mut self, name: String, model: Arc<dyn AnyModel>) -> crate::result::Result<()> {
        if self
            .provider
            .capabilities()
            .contains(&ModelProviderCapabilities::INSERT)
        {
            self.model_cache.insert(name, model.clone());
            Ok(())
        } else {
            Err(Error::simple(
                "Model provider has no capabilities for this operation",
                "The model provider doesn't support inserting new models",
            ))
        }
    }
}

//! [`CompiledModule`] — the per-module compilation artifact. One analog
//! and/or digital kernel, compiled once from a `LoweredBody` and shared
//! (`Arc`) across every instance of the module (MD-18: elaboration fixes the
//! devices; a sweep restamps, it never recompiles).

use std::sync::Arc;

use crate::resolve::pom::LoweredBody;
use crate::kernel::analog::AnalogKernel;
use crate::kernel::digital::DigitalKernel;
use crate::error::CodegenError;


/// The compiled artifact for one module: the JIT kernels, shared (`Arc`)
/// across every instance of the module.
#[derive(Clone)]
pub struct CompiledModule {
    name: String,
    analog: Option<Arc<AnalogKernel>>,
    digital: Option<Arc<DigitalKernel>>,
}

impl CompiledModule {
    /// Compile every behavior body of `module`, including `.disto` kernels.
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
        Self::compile_with_options(module, true)
    }

    /// Compile every behavior body of `module`. `compile_disto` gates the
    /// `.disto` 2nd/3rd-derivative kernels (see
    /// [`AnalogKernel::compile_with_options`]) — callers that will never
    /// run `.disto` on this circuit pass `false` to skip that compile cost.
    pub fn compile_with_options(module: &LoweredBody, compile_disto: bool) -> Result<Self, CodegenError> {
        let analog = module
            .analog
            .as_ref()
            .map(|_| AnalogKernel::compile_with_options(module, compile_disto).map(Arc::new))
            .transpose()?;
        let digital = module
            .digital
            .as_ref()
            .map(|_| DigitalKernel::compile(module).map(Arc::new))
            .transpose()?;
        Ok(Self { name: module.name.clone(), analog, digital })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn analog(&self) -> Option<&Arc<AnalogKernel>> {
        self.analog.as_ref()
    }

    pub fn digital(&self) -> Option<&Arc<DigitalKernel>> {
        self.digital.as_ref()
    }
}

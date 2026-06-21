mod cache;
mod compiler;
mod osdi_hardware;

pub use compiler::LibraryCompiler;
pub use osdi_hardware::OsdiHardwareDefinition;

use piperine_interpreter::{AnalogCompilerBackend, Plugin};

/// Plugin that wires the OpenVAF library compiler into the Piperine runtime.
///
/// It provides no simulator backend and no system tasks — only an analog
/// compiler.  Any module with an `analog` block is automatically compiled by
/// OpenVAF; no user-facing task is required.
pub struct OpenVafPlugin {
    /// Directory for compiled `.osdi` artefacts.  Defaults to the system
    /// cache dir (`~/.cache/piperine/osdi`) when `None`.
    pub cache_dir: Option<std::path::PathBuf>,
}

impl OpenVafPlugin {
    pub fn new() -> Self {
        Self { cache_dir: None }
    }

    pub fn with_cache_dir(cache_dir: std::path::PathBuf) -> Self {
        Self { cache_dir: Some(cache_dir) }
    }

    fn resolve_cache_dir(&self) -> std::path::PathBuf {
        if let Some(ref d) = self.cache_dir {
            return d.clone();
        }
        dirs::cache_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("piperine")
            .join("osdi")
    }
}

impl Default for OpenVafPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for OpenVafPlugin {
    fn name(&self) -> &str {
        "openvaf"
    }

    fn analog_compiler(&self) -> Option<Box<dyn AnalogCompilerBackend>> {
        Some(Box::new(LibraryCompiler))
    }
}

/// Compile a Verilog-A source file with caching.
///
/// Returns the path to the `.osdi` artefact, ready to pass to `pre_osdi`.
pub fn compile_va(
    source_path: &std::path::Path,
    cache_dir: &std::path::Path,
) -> Result<std::path::PathBuf, piperine_interpreter::InterpreterError> {
    if let Some(cached) = cache::lookup(source_path, cache_dir) {
        return Ok(cached);
    }
    let output = cache::output_path(source_path, cache_dir).map_err(|e| {
        piperine_interpreter::InterpreterError::SimulatorError(format!(
            "openvaf cache dir: {e}"
        ))
    })?;
    let compiler = LibraryCompiler;
    piperine_interpreter::AnalogCompilerBackend::compile(
        &compiler,
        source_path,
        output.parent().unwrap_or(cache_dir),
    )
}

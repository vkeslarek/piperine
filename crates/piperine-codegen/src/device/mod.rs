//! The solver boundary: compiled kernels wrapped as [`piperine_solver::prelude::Element`]s.
//!
//! - [`CompiledModule`] (`compiled.rs`) — the per-module compilation artifact
//!   (analog and/or digital kernel), shared across instances.
//! - [`PiperineDevice`] (`element.rs`) — one instance: parameter values,
//!   operator state, register banks, netlist references. Implements the solver
//!   `Element` trait for both domains.
//! - [`CircuitCompiler`] (`circuit.rs`) — walks the POM
//!   [`piperine_lang::pom::Design`]'s top [`piperine_lang::pom::Module`] and
//!   its [`piperine_lang::pom::Instance`]s, and builds a ready-to-simulate
//!   `CircuitInstance`.

mod analog;
mod builder;
mod circuit;
mod compiled;
mod digital;
mod element;
mod fusion;
mod plugin;

pub use analog::AnalogInstance;
pub use circuit::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler};
pub use compiled::CompiledModule;
pub use digital::DigitalInstance;
pub use element::PiperineDevice;
pub use plugin::{DeviceProvider, PluginDeviceSpec, PluginPort, PortBinding};

use element::analysis_code;

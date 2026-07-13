//! The plugin-device seam (Plugin plan D4): `CircuitCompiler` delegates
//! `@device`-annotated instances to a [`DeviceProvider`] instead of compiling
//! them from PHDL. The provider (a plugin host) returns a solver `Element`;
//! the solver never learns it came from a plugin (SPEC Part VI §7).
//!
//! Dependency direction: this trait lives in codegen so codegen never
//! depends on the plugin crate — the plugin host implements it.

use piperine_lang::parse::ast::Direction;
use piperine_lang::pom::module::Attribute;
use piperine_lang::Value;
use piperine_solver::analog::AnalogReference;
use piperine_solver::core::element::Element;
use piperine_solver::digital::DigitalNet;

/// How one port of a `@device` module binds into the built circuit.
#[derive(Debug, Clone)]
pub enum PortBinding {
    /// An analog terminal: the netlist reference the device stamps/reads.
    Analog(AnalogReference),
    /// A digital net in the scheduler's namespace.
    Digital(DigitalNet),
}

/// One resolved port of a plugin device.
#[derive(Debug, Clone)]
pub struct PluginPort {
    /// The plugin-facing logical name (`@port(name = "A0")`), or the PHDL
    /// port name when no `@port` attribute names it.
    pub logical: String,
    /// The PHDL port name.
    pub phdl_name: String,
    pub direction: Direction,
    pub binding: PortBinding,
}

/// Everything a device factory needs to construct one plugin device
/// (SPEC Part VI §7.3).
pub struct PluginDeviceSpec {
    /// The plugin named by `@device(plugin = …)`.
    pub plugin: String,
    /// The type id named by `@device(type = …)`.
    pub type_id: String,
    /// The instance label in the parent module.
    pub instance_label: String,
    /// The validated `@device`/`@port` (and any other) attributes on the
    /// module and the instance.
    pub attributes: Vec<Attribute>,
    /// Resolved port bindings, in PHDL port order.
    pub ports: Vec<PluginPort>,
    /// Instance parameter overrides (`{ .name = value }`), unevaluated.
    pub params: Vec<(String, Value)>,
}

/// The bridge between the plugin world and the circuit builder. Errors are
/// plain strings; the compiler wraps them as fail-loud `CodegenError`s.
pub trait DeviceProvider {
    fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Element>, String>;
}

use std::fmt;
use crate::types::{ParameterValue, ParameterMap, ConnectionMap};
use crate::error::ElaborationError;

/// Direction of a port as declared in an extern module or VA module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection { Input, Output, Inout }

/// Declaration of one port on a HardwareDefinition.
#[derive(Debug, Clone)]
pub struct PortDefinition {
    pub name: String,
    pub direction: PortDirection,
}

/// Declaration of one parameter on a HardwareDefinition.
#[derive(Debug, Clone)]
pub struct ParameterDefinition {
    pub name: String,
    pub is_expr: bool,
    /// Default value. `None` means the parameter is mandatory.
    pub default: Option<ParameterValue>,
}

/// Resolves hierarchical Piperine net names to flat SPICE net names.
///
/// Built by the elaborator from the current `NetMap` + hierarchy `path`.
/// Passed to `HardwareDefinition::instantiate()` so plugins can resolve
/// net references found inside `parameter expr` AST nodes.
///
/// Examples (assuming path = "X1"):
///   "X1.mid"  → "X1_mid"   (sub-module internal net)
///   "gnd"     → "0"         (canonical ground)
///   "out"     → "out"       (top-level net, no mangling)
///   "vdd"     → "vdd"       (global power net)
pub trait NetResolver: Send + Sync {
    fn resolve(&self, hierarchical_net: &str) -> String;
}

/// A hardware element type — anything that can be instantiated in a circuit.
///
/// Implement this trait to add new element types:
/// - ngspice built-in elements (resistor, voltage source, …)
/// - Verilog-A modules compiled to OSDI (Phase 2)
/// - B-source behavioral elements (Phase 3)
/// - Subcircuit definitions (future)
///
/// Register implementations via `HardwareRegistry::register()`.
pub trait HardwareDefinition: fmt::Debug + Send + Sync {
    /// Name as declared in source (e.g., `"spice_res"`, `"simple_diode"`).
    fn name(&self) -> &str;

    /// Ordered list of port declarations.
    /// The elaborator uses this to validate named port connections.
    fn ports(&self) -> &[PortDefinition];

    /// Ordered list of parameter declarations with optional defaults.
    /// The elaborator applies defaults before calling `instantiate`.
    fn parameters(&self) -> &[ParameterDefinition];

    /// Create a concrete instance.
    ///
    /// Called by the elaborator after resolving all parameter defaults
    /// and validating connection names. Implementations should assume
    /// `parameters` already has defaults applied — report errors only
    /// for missing mandatory parameters.
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError>;
}

/// A fully resolved hardware instance in the netlist.
///
/// The sole responsibility of a `HardwareInstance` is emitting the SPICE
/// deck lines that represent it. For most elements this is one line.
/// OSDI devices emit `N`-prefix lines. Subcircuit calls emit `X`-prefix lines.
pub trait HardwareInstance: fmt::Debug {
    fn instance_name(&self) -> &str;
    /// SPICE deck lines for this element (no `.model`, `.subckt`, or `.end`).
    fn spice_lines(&self) -> Vec<String>;
}

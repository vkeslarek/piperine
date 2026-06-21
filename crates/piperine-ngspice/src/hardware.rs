use piperine_circuit::{
    HardwareDefinition, HardwareInstance,
    PortDefinition, ParameterDefinition,
    ParameterMap, ConnectionMap, ElaborationError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Return `"{prefix}{name}"` only when `name` doesn't already start with `prefix` (case-insensitive).
/// SPICE instance names like `V1`, `R1` already carry the device letter; don't double it.
fn spice_name(prefix: char, name: &str) -> String {
    if name.chars().next().map(|c| c.to_ascii_uppercase()) == Some(prefix.to_ascii_uppercase()) {
        name.to_string()
    } else {
        format!("{prefix}{name}")
    }
}

fn require_net<'a>(
    connections: &'a ConnectionMap,
    port: &str,
    instance: &str,
) -> Result<&'a str, ElaborationError> {
    connections.get(port).map(|s| s.as_str()).ok_or_else(|| ElaborationError::ConnectionError {
        instance: instance.to_string(),
        detail: format!("port `{port}` not connected"),
    })
}

fn require_parameter(
    parameters: &ParameterMap,
    name: &str,
    instance: &str,
) -> Result<f64, ElaborationError> {
    parameters.get(name)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ElaborationError::MissingParameter {
            parameter: name.to_string(),
            instance: instance.to_string(),
        })
}

// ── SpiceResistor ────────────────────────────────────────────────────────────

/// `extern module spice_res(inout p, inout n; parameter real r = 1e3)`
/// SPICE line: `R{name} {p} {n} {r}`
#[derive(Debug)]
pub struct SpiceResistor;

impl HardwareDefinition for SpiceResistor {
    fn name(&self) -> &str { "spice_res" }
    fn ports(&self) -> &[PortDefinition] { &[] }           // validated by connection resolver
    fn parameters(&self) -> &[ParameterDefinition] { &[] } // default applied by source declaration

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let r = require_parameter(parameters, "r", instance_name)?;
        Ok(Box::new(SpiceResistorInstance { name: instance_name.to_string(), p, n, r }))
    }
}

#[derive(Debug)]
struct SpiceResistorInstance { name: String, p: String, n: String, r: f64 }

impl HardwareInstance for SpiceResistorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} {}", spice_name('R', &self.name), self.p, self.n, self.r)]
    }
}

// ── SpiceVoltageSource ───────────────────────────────────────────────────────

/// `extern module spice_vsource(inout p, inout n; parameter real val = 0.0)`
/// SPICE line: `V{name} {p} {n} DC {val}`
#[derive(Debug)]
pub struct SpiceVoltageSource;

impl HardwareDefinition for SpiceVoltageSource {
    fn name(&self) -> &str { "spice_vsource" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p   = require_net(connections, "p", instance_name)?.to_string();
        let n   = require_net(connections, "n", instance_name)?.to_string();
        let val = require_parameter(parameters, "val", instance_name)?;
        Ok(Box::new(SpiceVoltageSourceInstance { name: instance_name.to_string(), p, n, val }))
    }
}

#[derive(Debug)]
struct SpiceVoltageSourceInstance { name: String, p: String, n: String, val: f64 }

impl HardwareInstance for SpiceVoltageSourceInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} DC {}", spice_name('V', &self.name), self.p, self.n, self.val)]
    }
}

// ── SpiceCurrentSource ───────────────────────────────────────────────────────

/// `extern module spice_isource(inout p, inout n; parameter real val = 0.0)`
/// SPICE line: `I{name} {p} {n} DC {val}`
#[derive(Debug)]
pub struct SpiceCurrentSource;

impl HardwareDefinition for SpiceCurrentSource {
    fn name(&self) -> &str { "spice_isource" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p   = require_net(connections, "p", instance_name)?.to_string();
        let n   = require_net(connections, "n", instance_name)?.to_string();
        let val = require_parameter(parameters, "val", instance_name)?;
        Ok(Box::new(SpiceCurrentSourceInstance { name: instance_name.to_string(), p, n, val }))
    }
}

#[derive(Debug)]
struct SpiceCurrentSourceInstance { name: String, p: String, n: String, val: f64 }

impl HardwareInstance for SpiceCurrentSourceInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} DC {}", spice_name('I', &self.name), self.p, self.n, self.val)]
    }
}

// ── SpiceCapacitor ───────────────────────────────────────────────────────────

/// `extern module spice_cap(inout p, inout n; parameter real c = 1e-12)`
/// SPICE line: `C{name} {p} {n} {c}`
#[derive(Debug)]
pub struct SpiceCapacitor;

impl HardwareDefinition for SpiceCapacitor {
    fn name(&self) -> &str { "spice_cap" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let c = require_parameter(parameters, "c", instance_name)?;
        Ok(Box::new(SpiceCapacitorInstance { name: instance_name.to_string(), p, n, c }))
    }
}

#[derive(Debug)]
struct SpiceCapacitorInstance { name: String, p: String, n: String, c: f64 }

impl HardwareInstance for SpiceCapacitorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} {}", spice_name('C', &self.name), self.p, self.n, self.c)]
    }
}

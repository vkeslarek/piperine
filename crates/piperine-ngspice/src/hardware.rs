use piperine_circuit::{
    HardwareDefinition, HardwareInstance,
    NetResolver, PortDefinition, ParameterDefinition,
    ParameterMap, ConnectionMap, ElaborationError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

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
    connections.get(port).map(|s| s.as_str()).ok_or_else(|| {
        ElaborationError::ConnectionError {
            instance: instance.to_string(),
            detail: format!("missing port {}", port),
        }
    })
}

fn require_parameter(
    parameters: &ParameterMap,
    param: &str,
    instance: &str,
) -> Result<f64, ElaborationError> {
    parameters.get(param).and_then(|v| v.as_f64()).ok_or_else(|| {
        ElaborationError::MissingParameter {
            instance: instance.to_string(),
            parameter: param.to_string(),
        }
    })
}

fn require_string_parameter(
    parameters: &ParameterMap,
    param: &str,
    instance: &str,
) -> Result<String, ElaborationError> {
    parameters.get(param).and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| {
        ElaborationError::MissingParameter {
            instance: instance.to_string(),
            parameter: param.to_string(),
        }
    })
}

fn get_parameter_or(parameters: &ParameterMap, param: &str, default: f64) -> f64 {
    parameters.get(param).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn get_string_parameter_or(parameters: &ParameterMap, param: &str, default: &str) -> String {
    parameters.get(param).and_then(|v| v.as_str()).unwrap_or(default).to_string()
}
// ── SpiceResistor ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceResistor;
impl SpiceResistor { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceResistor {
    fn name(&self) -> &str { "res" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let r = require_parameter(parameters, "r", instance_name)?;
        let model = get_string_parameter_or(parameters, "model", "");
        let ac = get_parameter_or(parameters, "ac", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let l = get_parameter_or(parameters, "l", 0.0);
        let w = get_parameter_or(parameters, "w", 0.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let tc1 = get_parameter_or(parameters, "tc1", 0.0);
        let tc2 = get_parameter_or(parameters, "tc2", 0.0);
        let scale = get_parameter_or(parameters, "scale", 1.0);
        let noisy = get_parameter_or(parameters, "noisy", 1.0) as i64;
        let bv_max = get_parameter_or(parameters, "bv_max", 0.0);
        Ok(Box::new(SpiceResistorInstance { name: instance_name.to_string(), p, n, r, model, ac, temp, dtemp, l, w, m, tc1, tc2, scale, noisy, bv_max }))
    }
}
#[derive(Debug)]
struct SpiceResistorInstance { name: String, p: String, n: String, r: f64, model: String, ac: f64, temp: f64, dtemp: f64, l: f64, w: f64, m: f64, tc1: f64, tc2: f64, scale: f64, noisy: i64, bv_max: f64 }
impl HardwareInstance for SpiceResistorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('R', &self.name), self.p, self.n);
        s.push_str(&format!(" {}", self.r));
        if !self.model.is_empty() { s.push_str(&format!(" {}", self.model)); }
        if self.ac != 0.0 { s.push_str(&format!(" AC={}", self.ac)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.l != 0.0 { s.push_str(&format!(" L={}", self.l)); }
        if self.w != 0.0 { s.push_str(&format!(" W={}", self.w)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.tc1 != 0.0 { s.push_str(&format!(" TC1={}", self.tc1)); }
        if self.tc2 != 0.0 { s.push_str(&format!(" TC2={}", self.tc2)); }
        if self.scale != 1.0 { s.push_str(&format!(" SCALE={}", self.scale)); }
        if self.noisy != 1 { s.push_str(&format!(" NOISY={}", self.noisy)); }
        if self.bv_max != 0.0 { s.push_str(&format!(" BV_MAX={}", self.bv_max)); }
        vec![s]
    }
}

// ── SpiceCapacitor ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceCapacitor;
impl SpiceCapacitor { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCapacitor {
    fn name(&self) -> &str { "cap" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let c = require_parameter(parameters, "c", instance_name)?;
        let model = get_string_parameter_or(parameters, "model", "");
        let ic = get_parameter_or(parameters, "ic", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let w = get_parameter_or(parameters, "w", 0.0);
        let l = get_parameter_or(parameters, "l", 0.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let tc1 = get_parameter_or(parameters, "tc1", 0.0);
        let tc2 = get_parameter_or(parameters, "tc2", 0.0);
        let scale = get_parameter_or(parameters, "scale", 1.0);
        let bv_max = get_parameter_or(parameters, "bv_max", 0.0);
        Ok(Box::new(SpiceCapacitorInstance { name: instance_name.to_string(), p, n, c, model, ic, temp, dtemp, w, l, m, tc1, tc2, scale, bv_max }))
    }
}
#[derive(Debug)]
struct SpiceCapacitorInstance { name: String, p: String, n: String, c: f64, model: String, ic: f64, temp: f64, dtemp: f64, w: f64, l: f64, m: f64, tc1: f64, tc2: f64, scale: f64, bv_max: f64 }
impl HardwareInstance for SpiceCapacitorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('C', &self.name), self.p, self.n);
        s.push_str(&format!(" {}", self.c));
        if !self.model.is_empty() { s.push_str(&format!(" {}", self.model)); }
        if self.ic != 0.0 { s.push_str(&format!(" IC={}", self.ic)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.w != 0.0 { s.push_str(&format!(" W={}", self.w)); }
        if self.l != 0.0 { s.push_str(&format!(" L={}", self.l)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.tc1 != 0.0 { s.push_str(&format!(" TC1={}", self.tc1)); }
        if self.tc2 != 0.0 { s.push_str(&format!(" TC2={}", self.tc2)); }
        if self.scale != 1.0 { s.push_str(&format!(" SCALE={}", self.scale)); }
        if self.bv_max != 0.0 { s.push_str(&format!(" BV_MAX={}", self.bv_max)); }
        vec![s]
    }
}

// ── SpiceInductor ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceInductor;
impl SpiceInductor { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceInductor {
    fn name(&self) -> &str { "ind" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let l = require_parameter(parameters, "l", instance_name)?;
        let model = get_string_parameter_or(parameters, "model", "");
        let ic = get_parameter_or(parameters, "ic", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let tc1 = get_parameter_or(parameters, "tc1", 0.0);
        let tc2 = get_parameter_or(parameters, "tc2", 0.0);
        let scale = get_parameter_or(parameters, "scale", 1.0);
        let nt = get_parameter_or(parameters, "nt", 0.0);
        Ok(Box::new(SpiceInductorInstance { name: instance_name.to_string(), p, n, l, model, ic, temp, dtemp, m, tc1, tc2, scale, nt }))
    }
}
#[derive(Debug)]
struct SpiceInductorInstance { name: String, p: String, n: String, l: f64, model: String, ic: f64, temp: f64, dtemp: f64, m: f64, tc1: f64, tc2: f64, scale: f64, nt: f64 }
impl HardwareInstance for SpiceInductorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('L', &self.name), self.p, self.n);
        s.push_str(&format!(" {}", self.l));
        if !self.model.is_empty() { s.push_str(&format!(" {}", self.model)); }
        if self.ic != 0.0 { s.push_str(&format!(" IC={}", self.ic)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.tc1 != 0.0 { s.push_str(&format!(" TC1={}", self.tc1)); }
        if self.tc2 != 0.0 { s.push_str(&format!(" TC2={}", self.tc2)); }
        if self.scale != 1.0 { s.push_str(&format!(" SCALE={}", self.scale)); }
        if self.nt != 0.0 { s.push_str(&format!(" NT={}", self.nt)); }
        vec![s]
    }
}

// ── SpiceMutual ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceMutual;
impl SpiceMutual { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceMutual {
    fn name(&self) -> &str { "mutual" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        _connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let inductor1 = require_string_parameter(parameters, "inductor1", instance_name)?;
        let inductor2 = require_string_parameter(parameters, "inductor2", instance_name)?;
        let k = get_parameter_or(parameters, "k", 1.0);
        Ok(Box::new(SpiceMutualInstance { name: instance_name.to_string(), inductor1, inductor2, k }))
    }
}
#[derive(Debug)]
struct SpiceMutualInstance { name: String, inductor1: String, inductor2: String, k: f64 }
impl HardwareInstance for SpiceMutualInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{}", spice_name('K', &self.name));
        s.push_str(&format!(" {}", self.inductor1));
        s.push_str(&format!(" {}", self.inductor2));
        if self.k != 1.0 { s.push_str(&format!(" {}", self.k)); }
        vec![s]
    }
}

// ── SpiceVoltageSource ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVoltageSource;
impl SpiceVoltageSource { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVoltageSource {
    fn name(&self) -> &str { "vsource" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let dc = get_parameter_or(parameters, "dc", 0.0);
        let acmag = get_parameter_or(parameters, "acmag", 0.0);
        let acphase = get_parameter_or(parameters, "acphase", 0.0);
        Ok(Box::new(SpiceVoltageSourceInstance { name: instance_name.to_string(), p, n, dc, acmag, acphase }))
    }
}
#[derive(Debug)]
struct SpiceVoltageSourceInstance { name: String, p: String, n: String, dc: f64, acmag: f64, acphase: f64 }
impl HardwareInstance for SpiceVoltageSourceInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('V', &self.name), self.p, self.n);
        if self.dc != 0.0 { s.push_str(&format!(" DC {}", self.dc)); }
        if self.acmag != 0.0 { s.push_str(&format!(" AC {}", self.acmag)); }
        if self.acphase != 0.0 { s.push_str(&format!(" {}", self.acphase)); }
        vec![s]
    }
}

// ── SpiceCurrentSource ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceCurrentSource;
impl SpiceCurrentSource { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCurrentSource {
    fn name(&self) -> &str { "isource" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let dc = get_parameter_or(parameters, "dc", 0.0);
        let acmag = get_parameter_or(parameters, "acmag", 0.0);
        let acphase = get_parameter_or(parameters, "acphase", 0.0);
        Ok(Box::new(SpiceCurrentSourceInstance { name: instance_name.to_string(), p, n, dc, acmag, acphase }))
    }
}
#[derive(Debug)]
struct SpiceCurrentSourceInstance { name: String, p: String, n: String, dc: f64, acmag: f64, acphase: f64 }
impl HardwareInstance for SpiceCurrentSourceInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('I', &self.name), self.p, self.n);
        if self.dc != 0.0 { s.push_str(&format!(" DC {}", self.dc)); }
        if self.acmag != 0.0 { s.push_str(&format!(" AC {}", self.acmag)); }
        if self.acphase != 0.0 { s.push_str(&format!(" {}", self.acphase)); }
        vec![s]
    }
}

// ── SpiceVpulse ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVpulse;
impl SpiceVpulse { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVpulse {
    fn name(&self) -> &str { "vpulse" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let v0 = get_parameter_or(parameters, "v0", 0.0);
        let v1 = get_parameter_or(parameters, "v1", 1.0);
        let td = get_parameter_or(parameters, "td", 0.0);
        let tr = get_parameter_or(parameters, "tr", 1e-9);
        let tf = get_parameter_or(parameters, "tf", 1e-9);
        let pw = get_parameter_or(parameters, "pw", 10e-9);
        let per = get_parameter_or(parameters, "per", 20e-9);
        Ok(Box::new(SpiceVpulseInstance { name: instance_name.to_string(), p, n, v0, v1, td, tr, tf, pw, per }))
    }
}
#[derive(Debug)]
struct SpiceVpulseInstance { name: String, p: String, n: String, v0: f64, v1: f64, td: f64, tr: f64, tf: f64, pw: f64, per: f64 }
impl HardwareInstance for SpiceVpulseInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} PULSE({} {} {} {} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.v0, self.v1, self.td, self.tr, self.tf, self.pw, self.per)]
    }
}

// ── SpiceIpulse ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIpulse;
impl SpiceIpulse { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIpulse {
    fn name(&self) -> &str { "ipulse" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let i0 = get_parameter_or(parameters, "i0", 0.0);
        let i1 = get_parameter_or(parameters, "i1", 1.0);
        let td = get_parameter_or(parameters, "td", 0.0);
        let tr = get_parameter_or(parameters, "tr", 1e-9);
        let tf = get_parameter_or(parameters, "tf", 1e-9);
        let pw = get_parameter_or(parameters, "pw", 10e-9);
        let per = get_parameter_or(parameters, "per", 20e-9);
        Ok(Box::new(SpiceIpulseInstance { name: instance_name.to_string(), p, n, i0, i1, td, tr, tf, pw, per }))
    }
}
#[derive(Debug)]
struct SpiceIpulseInstance { name: String, p: String, n: String, i0: f64, i1: f64, td: f64, tr: f64, tf: f64, pw: f64, per: f64 }
impl HardwareInstance for SpiceIpulseInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} PULSE({} {} {} {} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.i0, self.i1, self.td, self.tr, self.tf, self.pw, self.per)]
    }
}

// ── SpiceVsin ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVsin;
impl SpiceVsin { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVsin {
    fn name(&self) -> &str { "vsin" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let vo = get_parameter_or(parameters, "vo", 0.0);
        let va = get_parameter_or(parameters, "va", 1.0);
        let freq = get_parameter_or(parameters, "freq", 1e6);
        let td = get_parameter_or(parameters, "td", 0.0);
        let theta = get_parameter_or(parameters, "theta", 0.0);
        let phase = get_parameter_or(parameters, "phase", 0.0);
        Ok(Box::new(SpiceVsinInstance { name: instance_name.to_string(), p, n, vo, va, freq, td, theta, phase }))
    }
}
#[derive(Debug)]
struct SpiceVsinInstance { name: String, p: String, n: String, vo: f64, va: f64, freq: f64, td: f64, theta: f64, phase: f64 }
impl HardwareInstance for SpiceVsinInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} SIN({} {} {} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.vo, self.va, self.freq, self.td, self.theta, self.phase)]
    }
}

// ── SpiceIsin ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIsin;
impl SpiceIsin { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIsin {
    fn name(&self) -> &str { "isin" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let io = get_parameter_or(parameters, "io", 0.0);
        let ia = get_parameter_or(parameters, "ia", 1.0);
        let freq = get_parameter_or(parameters, "freq", 1e6);
        let td = get_parameter_or(parameters, "td", 0.0);
        let theta = get_parameter_or(parameters, "theta", 0.0);
        let phase = get_parameter_or(parameters, "phase", 0.0);
        Ok(Box::new(SpiceIsinInstance { name: instance_name.to_string(), p, n, io, ia, freq, td, theta, phase }))
    }
}
#[derive(Debug)]
struct SpiceIsinInstance { name: String, p: String, n: String, io: f64, ia: f64, freq: f64, td: f64, theta: f64, phase: f64 }
impl HardwareInstance for SpiceIsinInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} SIN({} {} {} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.io, self.ia, self.freq, self.td, self.theta, self.phase)]
    }
}

// ── SpiceVexp ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVexp;
impl SpiceVexp { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVexp {
    fn name(&self) -> &str { "vexp" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let v1 = get_parameter_or(parameters, "v1", 0.0);
        let v2 = get_parameter_or(parameters, "v2", 1.0);
        let td1 = get_parameter_or(parameters, "td1", 0.0);
        let tau1 = get_parameter_or(parameters, "tau1", 1e-9);
        let td2 = get_parameter_or(parameters, "td2", 50e-9);
        let tau2 = get_parameter_or(parameters, "tau2", 1e-9);
        Ok(Box::new(SpiceVexpInstance { name: instance_name.to_string(), p, n, v1, v2, td1, tau1, td2, tau2 }))
    }
}
#[derive(Debug)]
struct SpiceVexpInstance { name: String, p: String, n: String, v1: f64, v2: f64, td1: f64, tau1: f64, td2: f64, tau2: f64 }
impl HardwareInstance for SpiceVexpInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} EXP({} {} {} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.v1, self.v2, self.td1, self.tau1, self.td2, self.tau2)]
    }
}

// ── SpiceIexp ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIexp;
impl SpiceIexp { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIexp {
    fn name(&self) -> &str { "iexp" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let i1 = get_parameter_or(parameters, "i1", 0.0);
        let i2 = get_parameter_or(parameters, "i2", 1e-3);
        let td1 = get_parameter_or(parameters, "td1", 0.0);
        let tau1 = get_parameter_or(parameters, "tau1", 1e-9);
        let td2 = get_parameter_or(parameters, "td2", 50e-9);
        let tau2 = get_parameter_or(parameters, "tau2", 1e-9);
        Ok(Box::new(SpiceIexpInstance { name: instance_name.to_string(), p, n, i1, i2, td1, tau1, td2, tau2 }))
    }
}
#[derive(Debug)]
struct SpiceIexpInstance { name: String, p: String, n: String, i1: f64, i2: f64, td1: f64, tau1: f64, td2: f64, tau2: f64 }
impl HardwareInstance for SpiceIexpInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} EXP({} {} {} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.i1, self.i2, self.td1, self.tau1, self.td2, self.tau2)]
    }
}

// ── SpiceVpwl ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVpwl;
impl SpiceVpwl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVpwl {
    fn name(&self) -> &str { "vpwl" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let points = require_string_parameter(parameters, "points", instance_name)?;
        Ok(Box::new(SpiceVpwlInstance { name: instance_name.to_string(), p, n, points }))
    }
}
#[derive(Debug)]
struct SpiceVpwlInstance { name: String, p: String, n: String, points: String }
impl HardwareInstance for SpiceVpwlInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} PWL({})", spice_name('V', &self.name), self.p, self.n, self.points)]
    }
}

// ── SpiceIpwl ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIpwl;
impl SpiceIpwl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIpwl {
    fn name(&self) -> &str { "ipwl" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let points = require_string_parameter(parameters, "points", instance_name)?;
        Ok(Box::new(SpiceIpwlInstance { name: instance_name.to_string(), p, n, points }))
    }
}
#[derive(Debug)]
struct SpiceIpwlInstance { name: String, p: String, n: String, points: String }
impl HardwareInstance for SpiceIpwlInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} PWL({})", spice_name('I', &self.name), self.p, self.n, self.points)]
    }
}

// ── SpiceVsffm ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVsffm;
impl SpiceVsffm { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVsffm {
    fn name(&self) -> &str { "vsffm" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let vo = get_parameter_or(parameters, "vo", 0.0);
        let va = get_parameter_or(parameters, "va", 1.0);
        let fc = get_parameter_or(parameters, "fc", 1e6);
        let mdi = get_parameter_or(parameters, "mdi", 1.0);
        let fs = get_parameter_or(parameters, "fs", 1e4);
        let phasec = get_parameter_or(parameters, "phasec", 0.0);
        let phases = get_parameter_or(parameters, "phases", 0.0);
        Ok(Box::new(SpiceVsffmInstance { name: instance_name.to_string(), p, n, vo, va, fc, mdi, fs, phasec, phases }))
    }
}
#[derive(Debug)]
struct SpiceVsffmInstance { name: String, p: String, n: String, vo: f64, va: f64, fc: f64, mdi: f64, fs: f64, phasec: f64, phases: f64 }
impl HardwareInstance for SpiceVsffmInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} SFFM({} {} {} {} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.vo, self.va, self.fc, self.mdi, self.fs, self.phasec, self.phases)]
    }
}

// ── SpiceIsffm ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIsffm;
impl SpiceIsffm { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIsffm {
    fn name(&self) -> &str { "isffm" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let io = get_parameter_or(parameters, "io", 0.0);
        let ia = get_parameter_or(parameters, "ia", 1.0);
        let fc = get_parameter_or(parameters, "fc", 1e6);
        let mdi = get_parameter_or(parameters, "mdi", 1.0);
        let fs = get_parameter_or(parameters, "fs", 1e4);
        let phasec = get_parameter_or(parameters, "phasec", 0.0);
        let phases = get_parameter_or(parameters, "phases", 0.0);
        Ok(Box::new(SpiceIsffmInstance { name: instance_name.to_string(), p, n, io, ia, fc, mdi, fs, phasec, phases }))
    }
}
#[derive(Debug)]
struct SpiceIsffmInstance { name: String, p: String, n: String, io: f64, ia: f64, fc: f64, mdi: f64, fs: f64, phasec: f64, phases: f64 }
impl HardwareInstance for SpiceIsffmInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} SFFM({} {} {} {} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.io, self.ia, self.fc, self.mdi, self.fs, self.phasec, self.phases)]
    }
}

// ── SpiceVam ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVam;
impl SpiceVam { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVam {
    fn name(&self) -> &str { "vam" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let sa = get_parameter_or(parameters, "sa", 1.0);
        let fc = get_parameter_or(parameters, "fc", 1e6);
        let fm = get_parameter_or(parameters, "fm", 1e4);
        let td = get_parameter_or(parameters, "td", 0.0);
        let phases = get_parameter_or(parameters, "phases", 0.0);
        Ok(Box::new(SpiceVamInstance { name: instance_name.to_string(), p, n, sa, fc, fm, td, phases }))
    }
}
#[derive(Debug)]
struct SpiceVamInstance { name: String, p: String, n: String, sa: f64, fc: f64, fm: f64, td: f64, phases: f64 }
impl HardwareInstance for SpiceVamInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} AM({} {} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.sa, self.fc, self.fm, self.td, self.phases)]
    }
}

// ── SpiceIam ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIam;
impl SpiceIam { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIam {
    fn name(&self) -> &str { "iam" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let sa = get_parameter_or(parameters, "sa", 1.0);
        let fc = get_parameter_or(parameters, "fc", 1e6);
        let fm = get_parameter_or(parameters, "fm", 1e4);
        let td = get_parameter_or(parameters, "td", 0.0);
        let phases = get_parameter_or(parameters, "phases", 0.0);
        Ok(Box::new(SpiceIamInstance { name: instance_name.to_string(), p, n, sa, fc, fm, td, phases }))
    }
}
#[derive(Debug)]
struct SpiceIamInstance { name: String, p: String, n: String, sa: f64, fc: f64, fm: f64, td: f64, phases: f64 }
impl HardwareInstance for SpiceIamInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} AM({} {} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.sa, self.fc, self.fm, self.td, self.phases)]
    }
}

// ── SpiceVnoise ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVnoise;
impl SpiceVnoise { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVnoise {
    fn name(&self) -> &str { "vnoise" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let na = get_parameter_or(parameters, "na", 0.0);
        let nt = get_parameter_or(parameters, "nt", 1e-9);
        let nalpha = get_parameter_or(parameters, "nalpha", 0.0);
        let namp = get_parameter_or(parameters, "namp", 0.0);
        Ok(Box::new(SpiceVnoiseInstance { name: instance_name.to_string(), p, n, na, nt, nalpha, namp }))
    }
}
#[derive(Debug)]
struct SpiceVnoiseInstance { name: String, p: String, n: String, na: f64, nt: f64, nalpha: f64, namp: f64 }
impl HardwareInstance for SpiceVnoiseInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} TRNOISE({} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.na, self.nt, self.nalpha, self.namp)]
    }
}

// ── SpiceInoise ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceInoise;
impl SpiceInoise { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceInoise {
    fn name(&self) -> &str { "inoise" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let na = get_parameter_or(parameters, "na", 0.0);
        let nt = get_parameter_or(parameters, "nt", 1e-9);
        let nalpha = get_parameter_or(parameters, "nalpha", 0.0);
        let namp = get_parameter_or(parameters, "namp", 0.0);
        Ok(Box::new(SpiceInoiseInstance { name: instance_name.to_string(), p, n, na, nt, nalpha, namp }))
    }
}
#[derive(Debug)]
struct SpiceInoiseInstance { name: String, p: String, n: String, na: f64, nt: f64, nalpha: f64, namp: f64 }
impl HardwareInstance for SpiceInoiseInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} TRNOISE({} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.na, self.nt, self.nalpha, self.namp)]
    }
}

// ── SpiceVrandom ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVrandom;
impl SpiceVrandom { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVrandom {
    fn name(&self) -> &str { "vrandom" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let rtype = get_parameter_or(parameters, "rtype", 1.0) as i64;
        let ts = get_parameter_or(parameters, "ts", 1e-9);
        let td = get_parameter_or(parameters, "td", 0.0);
        let param1 = get_parameter_or(parameters, "param1", 0.5);
        let param2 = get_parameter_or(parameters, "param2", 0.0);
        Ok(Box::new(SpiceVrandomInstance { name: instance_name.to_string(), p, n, rtype, ts, td, param1, param2 }))
    }
}
#[derive(Debug)]
struct SpiceVrandomInstance { name: String, p: String, n: String, rtype: i64, ts: f64, td: f64, param1: f64, param2: f64 }
impl HardwareInstance for SpiceVrandomInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} TRRANDOM({} {} {} {} {})", spice_name('V', &self.name), self.p, self.n, self.rtype, self.ts, self.td, self.param1, self.param2)]
    }
}

// ── SpiceIrandom ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIrandom;
impl SpiceIrandom { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIrandom {
    fn name(&self) -> &str { "irandom" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let rtype = get_parameter_or(parameters, "rtype", 1.0) as i64;
        let ts = get_parameter_or(parameters, "ts", 1e-9);
        let td = get_parameter_or(parameters, "td", 0.0);
        let param1 = get_parameter_or(parameters, "param1", 0.5);
        let param2 = get_parameter_or(parameters, "param2", 0.0);
        Ok(Box::new(SpiceIrandomInstance { name: instance_name.to_string(), p, n, rtype, ts, td, param1, param2 }))
    }
}
#[derive(Debug)]
struct SpiceIrandomInstance { name: String, p: String, n: String, rtype: i64, ts: f64, td: f64, param1: f64, param2: f64 }
impl HardwareInstance for SpiceIrandomInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("{} {} {} TRRANDOM({} {} {} {} {})", spice_name('I', &self.name), self.p, self.n, self.rtype, self.ts, self.td, self.param1, self.param2)]
    }
}

// ── SpiceVcvs ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVcvs;
impl SpiceVcvs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVcvs {
    fn name(&self) -> &str { "vcvs" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let cp = require_net(connections, "cp", instance_name)?.to_string();
        let cn = require_net(connections, "cn", instance_name)?.to_string();
        let gain = get_parameter_or(parameters, "gain", 1.0);
        Ok(Box::new(SpiceVcvsInstance { name: instance_name.to_string(), p, n, cp, cn, gain }))
    }
}
#[derive(Debug)]
struct SpiceVcvsInstance { name: String, p: String, n: String, cp: String, cn: String, gain: f64 }
impl HardwareInstance for SpiceVcvsInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('E', &self.name), self.p, self.n, self.cp, self.cn);
        if self.gain != 1.0 { s.push_str(&format!(" {}", self.gain)); }
        vec![s]
    }
}

// ── SpiceVccs ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVccs;
impl SpiceVccs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVccs {
    fn name(&self) -> &str { "vccs" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let cp = require_net(connections, "cp", instance_name)?.to_string();
        let cn = require_net(connections, "cn", instance_name)?.to_string();
        let gm = get_parameter_or(parameters, "gm", 1e-3);
        let m = get_parameter_or(parameters, "m", 1.0);
        Ok(Box::new(SpiceVccsInstance { name: instance_name.to_string(), p, n, cp, cn, gm, m }))
    }
}
#[derive(Debug)]
struct SpiceVccsInstance { name: String, p: String, n: String, cp: String, cn: String, gm: f64, m: f64 }
impl HardwareInstance for SpiceVccsInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('G', &self.name), self.p, self.n, self.cp, self.cn);
        if self.gm != 1e-3 { s.push_str(&format!(" {}", self.gm)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        vec![s]
    }
}

// ── SpiceCcvs ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceCcvs;
impl SpiceCcvs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCcvs {
    fn name(&self) -> &str { "ccvs" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let vsrc = require_string_parameter(parameters, "vsrc", instance_name)?;
        let transres = get_parameter_or(parameters, "transres", 1.0);
        Ok(Box::new(SpiceCcvsInstance { name: instance_name.to_string(), p, n, vsrc, transres }))
    }
}
#[derive(Debug)]
struct SpiceCcvsInstance { name: String, p: String, n: String, vsrc: String, transres: f64 }
impl HardwareInstance for SpiceCcvsInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('H', &self.name), self.p, self.n);
        s.push_str(&format!(" {}", self.vsrc));
        if self.transres != 1.0 { s.push_str(&format!(" {}", self.transres)); }
        vec![s]
    }
}

// ── SpiceCccs ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceCccs;
impl SpiceCccs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCccs {
    fn name(&self) -> &str { "cccs" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let vsrc = require_string_parameter(parameters, "vsrc", instance_name)?;
        let gain = get_parameter_or(parameters, "gain", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        Ok(Box::new(SpiceCccsInstance { name: instance_name.to_string(), p, n, vsrc, gain, m }))
    }
}
#[derive(Debug)]
struct SpiceCccsInstance { name: String, p: String, n: String, vsrc: String, gain: f64, m: f64 }
impl HardwareInstance for SpiceCccsInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('F', &self.name), self.p, self.n);
        s.push_str(&format!(" {}", self.vsrc));
        if self.gain != 1.0 { s.push_str(&format!(" {}", self.gain)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        vec![s]
    }
}

// ── SpiceBSourceV ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceBSourceV;
impl SpiceBSourceV { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceBSourceV {
    fn name(&self) -> &str { "bsource_v" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let V = require_string_parameter(parameters, "V", instance_name)?;
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let tc1 = get_parameter_or(parameters, "tc1", 0.0);
        let tc2 = get_parameter_or(parameters, "tc2", 0.0);
        let reciproctc = get_parameter_or(parameters, "reciproctc", 0.0) as i64;
        Ok(Box::new(SpiceBSourceVInstance { name: instance_name.to_string(), p, n, V, temp, dtemp, tc1, tc2, reciproctc }))
    }
}
#[derive(Debug)]
struct SpiceBSourceVInstance { name: String, p: String, n: String, V: String, temp: f64, dtemp: f64, tc1: f64, tc2: f64, reciproctc: i64 }
impl HardwareInstance for SpiceBSourceVInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('B', &self.name), self.p, self.n);
        s.push_str(&format!(" V={}", self.V));
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.tc1 != 0.0 { s.push_str(&format!(" TC1={}", self.tc1)); }
        if self.tc2 != 0.0 { s.push_str(&format!(" TC2={}", self.tc2)); }
        if self.reciproctc != 0 { s.push_str(&format!(" RECIPROCTC={}", self.reciproctc)); }
        vec![s]
    }
}

// ── SpiceBSourceI ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceBSourceI;
impl SpiceBSourceI { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceBSourceI {
    fn name(&self) -> &str { "bsource_i" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let I = require_string_parameter(parameters, "I", instance_name)?;
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let tc1 = get_parameter_or(parameters, "tc1", 0.0);
        let tc2 = get_parameter_or(parameters, "tc2", 0.0);
        let reciproctc = get_parameter_or(parameters, "reciproctc", 0.0) as i64;
        Ok(Box::new(SpiceBSourceIInstance { name: instance_name.to_string(), p, n, I, temp, dtemp, tc1, tc2, reciproctc }))
    }
}
#[derive(Debug)]
struct SpiceBSourceIInstance { name: String, p: String, n: String, I: String, temp: f64, dtemp: f64, tc1: f64, tc2: f64, reciproctc: i64 }
impl HardwareInstance for SpiceBSourceIInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('B', &self.name), self.p, self.n);
        s.push_str(&format!(" I={}", self.I));
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.tc1 != 0.0 { s.push_str(&format!(" TC1={}", self.tc1)); }
        if self.tc2 != 0.0 { s.push_str(&format!(" TC2={}", self.tc2)); }
        if self.reciproctc != 0 { s.push_str(&format!(" RECIPROCTC={}", self.reciproctc)); }
        vec![s]
    }
}

// ── SpiceVsw ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVsw;
impl SpiceVsw { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVsw {
    fn name(&self) -> &str { "vsw" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let cp = require_net(connections, "cp", instance_name)?.to_string();
        let cn = require_net(connections, "cn", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let on = get_parameter_or(parameters, "on", 0.0) as i64;
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        Ok(Box::new(SpiceVswInstance { name: instance_name.to_string(), p, n, cp, cn, model, on, off }))
    }
}
#[derive(Debug)]
struct SpiceVswInstance { name: String, p: String, n: String, cp: String, cn: String, model: String, on: i64, off: i64 }
impl HardwareInstance for SpiceVswInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('S', &self.name), self.p, self.n, self.cp, self.cn);
        s.push_str(&format!(" {}", self.model));
        if self.on != 0 { s.push_str(&format!(" ON={}", self.on)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        vec![s]
    }
}

// ── SpiceIsw ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceIsw;
impl SpiceIsw { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIsw {
    fn name(&self) -> &str { "isw" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let vsrc = require_string_parameter(parameters, "vsrc", instance_name)?;
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let on = get_parameter_or(parameters, "on", 0.0) as i64;
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        Ok(Box::new(SpiceIswInstance { name: instance_name.to_string(), p, n, vsrc, model, on, off }))
    }
}
#[derive(Debug)]
struct SpiceIswInstance { name: String, p: String, n: String, vsrc: String, model: String, on: i64, off: i64 }
impl HardwareInstance for SpiceIswInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('W', &self.name), self.p, self.n);
        s.push_str(&format!(" {}", self.vsrc));
        s.push_str(&format!(" {}", self.model));
        if self.on != 0 { s.push_str(&format!(" ON={}", self.on)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        vec![s]
    }
}

// ── SpiceDiode ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceDiode;
impl SpiceDiode { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceDiode {
    fn name(&self) -> &str { "d" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let a = require_net(connections, "a", instance_name)?.to_string();
        let c = require_net(connections, "c", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let pj = get_parameter_or(parameters, "pj", 0.0);
        let w = get_parameter_or(parameters, "w", 0.0);
        let l = get_parameter_or(parameters, "l", 0.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let ic = get_parameter_or(parameters, "ic", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpiceDiodeInstance { name: instance_name.to_string(), a, c, model, area, pj, w, l, m, off, ic, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpiceDiodeInstance { name: String, a: String, c: String, model: String, area: f64, pj: f64, w: f64, l: f64, m: f64, off: i64, ic: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpiceDiodeInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('D', &self.name), self.a, self.c);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.pj != 0.0 { s.push_str(&format!(" PJ={}", self.pj)); }
        if self.w != 0.0 { s.push_str(&format!(" W={}", self.w)); }
        if self.l != 0.0 { s.push_str(&format!(" L={}", self.l)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.ic != 0.0 { s.push_str(&format!(" IC={}", self.ic)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpiceNpn ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceNpn;
impl SpiceNpn { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceNpn {
    fn name(&self) -> &str { "npn" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let c = require_net(connections, "c", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let e = require_net(connections, "e", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let areab = get_parameter_or(parameters, "areab", 1.0);
        let areac = get_parameter_or(parameters, "areac", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvbe = get_parameter_or(parameters, "icvbe", 0.0);
        let icvce = get_parameter_or(parameters, "icvce", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpiceNpnInstance { name: instance_name.to_string(), c, b, e, model, area, areab, areac, m, off, icvbe, icvce, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpiceNpnInstance { name: String, c: String, b: String, e: String, model: String, area: f64, areab: f64, areac: f64, m: f64, off: i64, icvbe: f64, icvce: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpiceNpnInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('Q', &self.name), self.c, self.b, self.e);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.areab != 1.0 { s.push_str(&format!(" AREAB={}", self.areab)); }
        if self.areac != 1.0 { s.push_str(&format!(" AREAC={}", self.areac)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvbe != 0.0 { s.push_str(&format!(" ICVBE={}", self.icvbe)); }
        if self.icvce != 0.0 { s.push_str(&format!(" ICVCE={}", self.icvce)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpicePnp ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpicePnp;
impl SpicePnp { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePnp {
    fn name(&self) -> &str { "pnp" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let c = require_net(connections, "c", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let e = require_net(connections, "e", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let areab = get_parameter_or(parameters, "areab", 1.0);
        let areac = get_parameter_or(parameters, "areac", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvbe = get_parameter_or(parameters, "icvbe", 0.0);
        let icvce = get_parameter_or(parameters, "icvce", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpicePnpInstance { name: instance_name.to_string(), c, b, e, model, area, areab, areac, m, off, icvbe, icvce, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpicePnpInstance { name: String, c: String, b: String, e: String, model: String, area: f64, areab: f64, areac: f64, m: f64, off: i64, icvbe: f64, icvce: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpicePnpInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('Q', &self.name), self.c, self.b, self.e);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.areab != 1.0 { s.push_str(&format!(" AREAB={}", self.areab)); }
        if self.areac != 1.0 { s.push_str(&format!(" AREAC={}", self.areac)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvbe != 0.0 { s.push_str(&format!(" ICVBE={}", self.icvbe)); }
        if self.icvce != 0.0 { s.push_str(&format!(" ICVCE={}", self.icvce)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpiceNpn4 ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceNpn4;
impl SpiceNpn4 { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceNpn4 {
    fn name(&self) -> &str { "npn4" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let c = require_net(connections, "c", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let e = require_net(connections, "e", instance_name)?.to_string();
        let sub = require_net(connections, "sub", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let areab = get_parameter_or(parameters, "areab", 1.0);
        let areac = get_parameter_or(parameters, "areac", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvbe = get_parameter_or(parameters, "icvbe", 0.0);
        let icvce = get_parameter_or(parameters, "icvce", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpiceNpn4Instance { name: instance_name.to_string(), c, b, e, sub, model, area, areab, areac, m, off, icvbe, icvce, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpiceNpn4Instance { name: String, c: String, b: String, e: String, sub: String, model: String, area: f64, areab: f64, areac: f64, m: f64, off: i64, icvbe: f64, icvce: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpiceNpn4Instance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('Q', &self.name), self.c, self.b, self.e, self.sub);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.areab != 1.0 { s.push_str(&format!(" AREAB={}", self.areab)); }
        if self.areac != 1.0 { s.push_str(&format!(" AREAC={}", self.areac)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvbe != 0.0 { s.push_str(&format!(" ICVBE={}", self.icvbe)); }
        if self.icvce != 0.0 { s.push_str(&format!(" ICVCE={}", self.icvce)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpicePnp4 ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpicePnp4;
impl SpicePnp4 { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePnp4 {
    fn name(&self) -> &str { "pnp4" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let c = require_net(connections, "c", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let e = require_net(connections, "e", instance_name)?.to_string();
        let sub = require_net(connections, "sub", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let areab = get_parameter_or(parameters, "areab", 1.0);
        let areac = get_parameter_or(parameters, "areac", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvbe = get_parameter_or(parameters, "icvbe", 0.0);
        let icvce = get_parameter_or(parameters, "icvce", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpicePnp4Instance { name: instance_name.to_string(), c, b, e, sub, model, area, areab, areac, m, off, icvbe, icvce, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpicePnp4Instance { name: String, c: String, b: String, e: String, sub: String, model: String, area: f64, areab: f64, areac: f64, m: f64, off: i64, icvbe: f64, icvce: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpicePnp4Instance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('Q', &self.name), self.c, self.b, self.e, self.sub);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.areab != 1.0 { s.push_str(&format!(" AREAB={}", self.areab)); }
        if self.areac != 1.0 { s.push_str(&format!(" AREAC={}", self.areac)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvbe != 0.0 { s.push_str(&format!(" ICVBE={}", self.icvbe)); }
        if self.icvce != 0.0 { s.push_str(&format!(" ICVCE={}", self.icvce)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpiceNmos ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceNmos;
impl SpiceNmos { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceNmos {
    fn name(&self) -> &str { "nmos" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let w = get_parameter_or(parameters, "w", 1e-6);
        let l = get_parameter_or(parameters, "l", 100e-9);
        let ad = get_parameter_or(parameters, "ad", 0.0);
        let as_ = get_parameter_or(parameters, "as", 0.0);
        let pd = get_parameter_or(parameters, "pd", 0.0);
        let ps = get_parameter_or(parameters, "ps", 0.0);
        let nrd = get_parameter_or(parameters, "nrd", 0.0);
        let nrs = get_parameter_or(parameters, "nrs", 0.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvds = get_parameter_or(parameters, "icvds", 0.0);
        let icvgs = get_parameter_or(parameters, "icvgs", 0.0);
        let icvbs = get_parameter_or(parameters, "icvbs", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let nf = get_parameter_or(parameters, "nf", 1.0);
        let sa = get_parameter_or(parameters, "sa", 0.0);
        let sb = get_parameter_or(parameters, "sb", 0.0);
        Ok(Box::new(SpiceNmosInstance { name: instance_name.to_string(), d, g, s, b, model, w, l, ad, as_, pd, ps, nrd, nrs, m, off, icvds, icvgs, icvbs, temp, dtemp, nf, sa, sb }))
    }
}
#[derive(Debug)]
struct SpiceNmosInstance { name: String, d: String, g: String, s: String, b: String, model: String, w: f64, l: f64, ad: f64, as_: f64, pd: f64, ps: f64, nrd: f64, nrs: f64, m: f64, off: i64, icvds: f64, icvgs: f64, icvbs: f64, temp: f64, dtemp: f64, nf: f64, sa: f64, sb: f64 }
impl HardwareInstance for SpiceNmosInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('M', &self.name), self.d, self.g, self.s, self.b);
        s.push_str(&format!(" {}", self.model));
        if self.w != 1e-6 { s.push_str(&format!(" W={}", self.w)); }
        if self.l != 100e-9 { s.push_str(&format!(" L={}", self.l)); }
        if self.ad != 0.0 { s.push_str(&format!(" AD={}", self.ad)); }
        if self.as_ != 0.0 { s.push_str(&format!(" AS={}", self.as_)); }
        if self.pd != 0.0 { s.push_str(&format!(" PD={}", self.pd)); }
        if self.ps != 0.0 { s.push_str(&format!(" PS={}", self.ps)); }
        if self.nrd != 0.0 { s.push_str(&format!(" NRD={}", self.nrd)); }
        if self.nrs != 0.0 { s.push_str(&format!(" NRS={}", self.nrs)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvds != 0.0 { s.push_str(&format!(" ICVDS={}", self.icvds)); }
        if self.icvgs != 0.0 { s.push_str(&format!(" ICVGS={}", self.icvgs)); }
        if self.icvbs != 0.0 { s.push_str(&format!(" ICVBS={}", self.icvbs)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.nf != 1.0 { s.push_str(&format!(" NF={}", self.nf)); }
        if self.sa != 0.0 { s.push_str(&format!(" SA={}", self.sa)); }
        if self.sb != 0.0 { s.push_str(&format!(" SB={}", self.sb)); }
        vec![s]
    }
}

// ── SpicePmos ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpicePmos;
impl SpicePmos { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePmos {
    fn name(&self) -> &str { "pmos" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let w = get_parameter_or(parameters, "w", 1e-6);
        let l = get_parameter_or(parameters, "l", 100e-9);
        let ad = get_parameter_or(parameters, "ad", 0.0);
        let as_ = get_parameter_or(parameters, "as", 0.0);
        let pd = get_parameter_or(parameters, "pd", 0.0);
        let ps = get_parameter_or(parameters, "ps", 0.0);
        let nrd = get_parameter_or(parameters, "nrd", 0.0);
        let nrs = get_parameter_or(parameters, "nrs", 0.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvds = get_parameter_or(parameters, "icvds", 0.0);
        let icvgs = get_parameter_or(parameters, "icvgs", 0.0);
        let icvbs = get_parameter_or(parameters, "icvbs", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        let nf = get_parameter_or(parameters, "nf", 1.0);
        let sa = get_parameter_or(parameters, "sa", 0.0);
        let sb = get_parameter_or(parameters, "sb", 0.0);
        Ok(Box::new(SpicePmosInstance { name: instance_name.to_string(), d, g, s, b, model, w, l, ad, as_, pd, ps, nrd, nrs, m, off, icvds, icvgs, icvbs, temp, dtemp, nf, sa, sb }))
    }
}
#[derive(Debug)]
struct SpicePmosInstance { name: String, d: String, g: String, s: String, b: String, model: String, w: f64, l: f64, ad: f64, as_: f64, pd: f64, ps: f64, nrd: f64, nrs: f64, m: f64, off: i64, icvds: f64, icvgs: f64, icvbs: f64, temp: f64, dtemp: f64, nf: f64, sa: f64, sb: f64 }
impl HardwareInstance for SpicePmosInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('M', &self.name), self.d, self.g, self.s, self.b);
        s.push_str(&format!(" {}", self.model));
        if self.w != 1e-6 { s.push_str(&format!(" W={}", self.w)); }
        if self.l != 100e-9 { s.push_str(&format!(" L={}", self.l)); }
        if self.ad != 0.0 { s.push_str(&format!(" AD={}", self.ad)); }
        if self.as_ != 0.0 { s.push_str(&format!(" AS={}", self.as_)); }
        if self.pd != 0.0 { s.push_str(&format!(" PD={}", self.pd)); }
        if self.ps != 0.0 { s.push_str(&format!(" PS={}", self.ps)); }
        if self.nrd != 0.0 { s.push_str(&format!(" NRD={}", self.nrd)); }
        if self.nrs != 0.0 { s.push_str(&format!(" NRS={}", self.nrs)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvds != 0.0 { s.push_str(&format!(" ICVDS={}", self.icvds)); }
        if self.icvgs != 0.0 { s.push_str(&format!(" ICVGS={}", self.icvgs)); }
        if self.icvbs != 0.0 { s.push_str(&format!(" ICVBS={}", self.icvbs)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        if self.nf != 1.0 { s.push_str(&format!(" NF={}", self.nf)); }
        if self.sa != 0.0 { s.push_str(&format!(" SA={}", self.sa)); }
        if self.sb != 0.0 { s.push_str(&format!(" SB={}", self.sb)); }
        vec![s]
    }
}

// ── SpiceJfetN ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceJfetN;
impl SpiceJfetN { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceJfetN {
    fn name(&self) -> &str { "jfet_n" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let ic = get_parameter_or(parameters, "ic", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpiceJfetNInstance { name: instance_name.to_string(), d, g, s, model, area, m, off, ic, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpiceJfetNInstance { name: String, d: String, g: String, s: String, model: String, area: f64, m: f64, off: i64, ic: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpiceJfetNInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('J', &self.name), self.d, self.g, self.s);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.ic != 0.0 { s.push_str(&format!(" IC={}", self.ic)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpiceJfetP ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceJfetP;
impl SpiceJfetP { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceJfetP {
    fn name(&self) -> &str { "jfet_p" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let ic = get_parameter_or(parameters, "ic", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpiceJfetPInstance { name: instance_name.to_string(), d, g, s, model, area, m, off, ic, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpiceJfetPInstance { name: String, d: String, g: String, s: String, model: String, area: f64, m: f64, off: i64, ic: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpiceJfetPInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('J', &self.name), self.d, self.g, self.s);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.ic != 0.0 { s.push_str(&format!(" IC={}", self.ic)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpiceMesfetN ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceMesfetN;
impl SpiceMesfetN { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceMesfetN {
    fn name(&self) -> &str { "mesfet_n" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvds = get_parameter_or(parameters, "icvds", 0.0);
        let icvgs = get_parameter_or(parameters, "icvgs", 0.0);
        Ok(Box::new(SpiceMesfetNInstance { name: instance_name.to_string(), d, g, s, model, area, m, off, icvds, icvgs }))
    }
}
#[derive(Debug)]
struct SpiceMesfetNInstance { name: String, d: String, g: String, s: String, model: String, area: f64, m: f64, off: i64, icvds: f64, icvgs: f64 }
impl HardwareInstance for SpiceMesfetNInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('Z', &self.name), self.d, self.g, self.s);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvds != 0.0 { s.push_str(&format!(" ICVDS={}", self.icvds)); }
        if self.icvgs != 0.0 { s.push_str(&format!(" ICVGS={}", self.icvgs)); }
        vec![s]
    }
}

// ── SpiceMesfetP ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceMesfetP;
impl SpiceMesfetP { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceMesfetP {
    fn name(&self) -> &str { "mesfet_p" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let area = get_parameter_or(parameters, "area", 1.0);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvds = get_parameter_or(parameters, "icvds", 0.0);
        let icvgs = get_parameter_or(parameters, "icvgs", 0.0);
        Ok(Box::new(SpiceMesfetPInstance { name: instance_name.to_string(), d, g, s, model, area, m, off, icvds, icvgs }))
    }
}
#[derive(Debug)]
struct SpiceMesfetPInstance { name: String, d: String, g: String, s: String, model: String, area: f64, m: f64, off: i64, icvds: f64, icvgs: f64 }
impl HardwareInstance for SpiceMesfetPInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('Z', &self.name), self.d, self.g, self.s);
        s.push_str(&format!(" {}", self.model));
        if self.area != 1.0 { s.push_str(&format!(" AREA={}", self.area)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvds != 0.0 { s.push_str(&format!(" ICVDS={}", self.icvds)); }
        if self.icvgs != 0.0 { s.push_str(&format!(" ICVGS={}", self.icvgs)); }
        vec![s]
    }
}

// ── SpiceVdmos ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceVdmos;
impl SpiceVdmos { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVdmos {
    fn name(&self) -> &str { "vdmos" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let d = require_net(connections, "d", instance_name)?.to_string();
        let g = require_net(connections, "g", instance_name)?.to_string();
        let s = require_net(connections, "s", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let w = get_parameter_or(parameters, "w", 1e-3);
        let l = get_parameter_or(parameters, "l", 1e-6);
        let m = get_parameter_or(parameters, "m", 1.0);
        let off = get_parameter_or(parameters, "off", 0.0) as i64;
        let icvds = get_parameter_or(parameters, "icvds", 0.0);
        let icvgs = get_parameter_or(parameters, "icvgs", 0.0);
        let temp = get_parameter_or(parameters, "temp", 27.0);
        let dtemp = get_parameter_or(parameters, "dtemp", 0.0);
        Ok(Box::new(SpiceVdmosInstance { name: instance_name.to_string(), d, g, s, model, w, l, m, off, icvds, icvgs, temp, dtemp }))
    }
}
#[derive(Debug)]
struct SpiceVdmosInstance { name: String, d: String, g: String, s: String, model: String, w: f64, l: f64, m: f64, off: i64, icvds: f64, icvgs: f64, temp: f64, dtemp: f64 }
impl HardwareInstance for SpiceVdmosInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('M', &self.name), self.d, self.g, self.s);
        s.push_str(&format!(" {}", self.model));
        if self.w != 1e-3 { s.push_str(&format!(" W={}", self.w)); }
        if self.l != 1e-6 { s.push_str(&format!(" L={}", self.l)); }
        if self.m != 1.0 { s.push_str(&format!(" M={}", self.m)); }
        if self.off != 0 { s.push_str(&format!(" OFF={}", self.off)); }
        if self.icvds != 0.0 { s.push_str(&format!(" ICVDS={}", self.icvds)); }
        if self.icvgs != 0.0 { s.push_str(&format!(" ICVGS={}", self.icvgs)); }
        if self.temp != 27.0 { s.push_str(&format!(" TEMP={}", self.temp)); }
        if self.dtemp != 0.0 { s.push_str(&format!(" DTEMP={}", self.dtemp)); }
        vec![s]
    }
}

// ── SpiceTline ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceTline;
impl SpiceTline { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceTline {
    fn name(&self) -> &str { "tline" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let ap = require_net(connections, "ap", instance_name)?.to_string();
        let an = require_net(connections, "an", instance_name)?.to_string();
        let bp = require_net(connections, "bp", instance_name)?.to_string();
        let bn = require_net(connections, "bn", instance_name)?.to_string();
        let z0 = get_parameter_or(parameters, "z0", 50.0);
        let td = get_parameter_or(parameters, "td", 1e-9);
        let f = get_parameter_or(parameters, "f", 0.0);
        let nl = get_parameter_or(parameters, "nl", 0.25);
        let v1 = get_parameter_or(parameters, "v1", 0.0);
        let v2 = get_parameter_or(parameters, "v2", 0.0);
        let i1 = get_parameter_or(parameters, "i1", 0.0);
        let i2 = get_parameter_or(parameters, "i2", 0.0);
        Ok(Box::new(SpiceTlineInstance { name: instance_name.to_string(), ap, an, bp, bn, z0, td, f, nl, v1, v2, i1, i2 }))
    }
}
#[derive(Debug)]
struct SpiceTlineInstance { name: String, ap: String, an: String, bp: String, bn: String, z0: f64, td: f64, f: f64, nl: f64, v1: f64, v2: f64, i1: f64, i2: f64 }
impl HardwareInstance for SpiceTlineInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('T', &self.name), self.ap, self.an, self.bp, self.bn);
        if self.z0 != 50.0 { s.push_str(&format!(" Z0={}", self.z0)); }
        if self.td != 1e-9 { s.push_str(&format!(" TD={}", self.td)); }
        if self.f != 0.0 { s.push_str(&format!(" F={}", self.f)); }
        if self.nl != 0.25 { s.push_str(&format!(" NL={}", self.nl)); }
        if self.v1 != 0.0 { s.push_str(&format!(" V1={}", self.v1)); }
        if self.v2 != 0.0 { s.push_str(&format!(" V2={}", self.v2)); }
        if self.i1 != 0.0 { s.push_str(&format!(" I1={}", self.i1)); }
        if self.i2 != 0.0 { s.push_str(&format!(" I2={}", self.i2)); }
        vec![s]
    }
}

// ── SpiceLtra ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceLtra;
impl SpiceLtra { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceLtra {
    fn name(&self) -> &str { "ltra" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let ap = require_net(connections, "ap", instance_name)?.to_string();
        let an = require_net(connections, "an", instance_name)?.to_string();
        let bp = require_net(connections, "bp", instance_name)?.to_string();
        let bn = require_net(connections, "bn", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let v1 = get_parameter_or(parameters, "v1", 0.0);
        let v2 = get_parameter_or(parameters, "v2", 0.0);
        let i1 = get_parameter_or(parameters, "i1", 0.0);
        let i2 = get_parameter_or(parameters, "i2", 0.0);
        Ok(Box::new(SpiceLtraInstance { name: instance_name.to_string(), ap, an, bp, bn, model, v1, v2, i1, i2 }))
    }
}
#[derive(Debug)]
struct SpiceLtraInstance { name: String, ap: String, an: String, bp: String, bn: String, model: String, v1: f64, v2: f64, i1: f64, i2: f64 }
impl HardwareInstance for SpiceLtraInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {} {}", spice_name('O', &self.name), self.ap, self.an, self.bp, self.bn);
        s.push_str(&format!(" {}", self.model));
        if self.v1 != 0.0 { s.push_str(&format!(" V1={}", self.v1)); }
        if self.v2 != 0.0 { s.push_str(&format!(" V2={}", self.v2)); }
        if self.i1 != 0.0 { s.push_str(&format!(" I1={}", self.i1)); }
        if self.i2 != 0.0 { s.push_str(&format!(" I2={}", self.i2)); }
        vec![s]
    }
}

// ── SpiceUrc ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceUrc;
impl SpiceUrc { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceUrc {
    fn name(&self) -> &str { "urc" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let a = require_net(connections, "a", instance_name)?.to_string();
        let b = require_net(connections, "b", instance_name)?.to_string();
        let ref_ = require_net(connections, "ref", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let length = get_parameter_or(parameters, "length", 1e-3);
        let n = get_parameter_or(parameters, "n", 0.0) as i64;
        Ok(Box::new(SpiceUrcInstance { name: instance_name.to_string(), a, b, ref_, model, length, n }))
    }
}
#[derive(Debug)]
struct SpiceUrcInstance { name: String, a: String, b: String, ref_: String, model: String, length: f64, n: i64 }
impl HardwareInstance for SpiceUrcInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('U', &self.name), self.a, self.b, self.ref_);
        s.push_str(&format!(" {}", self.model));
        if self.length != 1e-3 { s.push_str(&format!(" L={}", self.length)); }
        if self.n != 0 { s.push_str(&format!(" N={}", self.n)); }
        vec![s]
    }
}

// ── SpiceCpl ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceCpl;
impl SpiceCpl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCpl {
    fn name(&self) -> &str { "cpl" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        _connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let ports = require_string_parameter(parameters, "ports", instance_name)?;
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let length = get_parameter_or(parameters, "length", 1.0);
        let dimension = get_parameter_or(parameters, "dimension", 0.0) as i64;
        Ok(Box::new(SpiceCplInstance { name: instance_name.to_string(), ports, model, length, dimension }))
    }
}
#[derive(Debug)]
struct SpiceCplInstance { name: String, ports: String, model: String, length: f64, dimension: i64 }
impl HardwareInstance for SpiceCplInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{}", spice_name('P', &self.name));
        s.push_str(&format!(" {}", self.ports));
        s.push_str(&format!(" {}", self.model));
        if self.length != 1.0 { s.push_str(&format!(" length={}", self.length)); }
        if self.dimension != 0 { s.push_str(&format!(" dimension={}", self.dimension)); }
        vec![s]
    }
}

// ── SpiceTxl ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceTxl;
impl SpiceTxl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceTxl {
    fn name(&self) -> &str { "txl" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let y1p = require_net(connections, "y1p", instance_name)?.to_string();
        let y1n = require_net(connections, "y1n", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let length = get_parameter_or(parameters, "length", 1.0);
        Ok(Box::new(SpiceTxlInstance { name: instance_name.to_string(), y1p, y1n, model, length }))
    }
}
#[derive(Debug)]
struct SpiceTxlInstance { name: String, y1p: String, y1n: String, model: String, length: f64 }
impl HardwareInstance for SpiceTxlInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('Y', &self.name), self.y1p, self.y1n);
        s.push_str(&format!(" {}", self.model));
        if self.length != 1.0 { s.push_str(&format!(" length={}", self.length)); }
        vec![s]
    }
}

// ── SpicePort ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpicePort;
impl SpicePort { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePort {
    fn name(&self) -> &str { "port" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let num = get_parameter_or(parameters, "num", 1.0) as i64;
        let z0 = get_parameter_or(parameters, "z0", 50.0);
        Ok(Box::new(SpicePortInstance { name: instance_name.to_string(), p, n, num, z0 }))
    }
}
#[derive(Debug)]
struct SpicePortInstance { name: String, p: String, n: String, num: i64, z0: f64 }
impl HardwareInstance for SpicePortInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {}", spice_name('P', &self.name), self.p, self.n);
        if self.num != 1 { s.push_str(&format!(" PORT={}", self.num)); }
        if self.z0 != 50.0 { s.push_str(&format!(" Z0={}", self.z0)); }
        vec![s]
    }
}

// ── SpiceSubckt ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct SpiceSubckt;
impl SpiceSubckt { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceSubckt {
    fn name(&self) -> &str { "subckt" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        _connections: &ConnectionMap,
        _resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let ports = require_string_parameter(parameters, "ports", instance_name)?;
        let subckt_name = require_string_parameter(parameters, "subckt_name", instance_name)?;
        let params = get_string_parameter_or(parameters, "params", "");
        Ok(Box::new(SpiceSubcktInstance { name: instance_name.to_string(), ports, subckt_name, params }))
    }
}
#[derive(Debug)]
struct SpiceSubcktInstance { name: String, ports: String, subckt_name: String, params: String }
impl HardwareInstance for SpiceSubcktInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{}", spice_name('X', &self.name));
        s.push_str(&format!(" {}", self.ports));
        s.push_str(&format!(" {}", self.subckt_name));
        if !self.params.is_empty() { s.push_str(&format!(" {}", self.params)); }
        vec![s]
    }
}


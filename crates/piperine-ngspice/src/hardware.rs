//! ngspice built-in device definitions.
//!
//! Every device is a unit struct implementing [`HardwareDefinition`]. Its only
//! real work is in `instantiate`: read the elaborated parameters and append them
//! to an [`Element`] — a small builder that formats one SPICE element line. The
//! finished line is wrapped in a trivial [`Line`] instance.
//!
//! There is no per-device "Instance" struct mirroring every field, and no
//! separate `spice_lines()` formatter: the line is built in one place, so each
//! device reads top-to-bottom like its SPICE datasheet entry.
//!
//! The `Element` vocabulary maps directly to SPICE line syntax:
//!
//! | method            | emits                                   | when            |
//! |-------------------|-----------------------------------------|-----------------|
//! | `start`           | `<Prefix><name> <node> <node> …`        | always          |
//! | `value`           | ` <v>` (required real)                   | always          |
//! | `req_str`         | ` <s>` (required string: model, vsrc)    | always          |
//! | `opt_str`         | ` <s>`                                   | if non-empty    |
//! | `key_str`         | ` KEY=<s>` (required string)             | always          |
//! | `opt` / `opt_int` | ` KEY=<v>`                               | if ≠ default    |
//! | `bare_opt`        | ` <v>`                                   | if ≠ default    |
//! | `spaced`          | ` KEY <v>`                               | if ≠ default    |
//! | `waveform`        | ` FUNC(a b c …)`                         | always          |

use piperine_circuit::{
    HardwareDefinition, HardwareInstance, ParameterDefinition,
    NetResolver, ParameterMap, ConnectionMap, ElaborationError,
};

// ── parameter readers ─────────────────────────────────────────────────────────

fn spice_name(prefix: char, name: &str) -> String {
    if name.chars().next().map(|c| c.to_ascii_uppercase()) == Some(prefix.to_ascii_uppercase()) {
        name.to_string()
    } else {
        format!("{prefix}{name}")
    }
}

fn require_net<'a>(connections: &'a ConnectionMap, port: &str, instance: &str)
    -> Result<&'a str, ElaborationError>
{
    connections.get(port).map(|s| s.as_str()).ok_or_else(|| ElaborationError::ConnectionError {
        instance: instance.to_string(),
        detail: format!("missing port {port}"),
    })
}

fn require_real(parameters: &ParameterMap, param: &str, instance: &str)
    -> Result<f64, ElaborationError>
{
    parameters.get(param).and_then(|v| v.as_f64()).ok_or_else(|| ElaborationError::MissingParameter {
        instance: instance.to_string(),
        parameter: param.to_string(),
    })
}

fn require_string(parameters: &ParameterMap, param: &str, instance: &str)
    -> Result<String, ElaborationError>
{
    parameters.get(param).and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| {
        ElaborationError::MissingParameter {
            instance: instance.to_string(),
            parameter: param.to_string(),
        }
    })
}

fn real_or(parameters: &ParameterMap, param: &str, default: f64) -> f64 {
    parameters.get(param).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn string_or(parameters: &ParameterMap, param: &str, default: &str) -> String {
    parameters.get(param).and_then(|v| v.as_str()).unwrap_or(default).to_string()
}

// ── Element: one SPICE element line, built as the device reads its params ──────

/// A SPICE element line under construction. Borrows the elaborated parameters so
/// each method names a parameter once; defaults double as the "omit if equal"
/// test, exactly matching ngspice's convention of dropping default-valued keys.
struct Element<'a> {
    line: String,
    params: &'a ParameterMap,
    instance: &'a str,
}

impl<'a> Element<'a> {
    /// `<Prefix><name> <node> <node> …`, resolving each named port.
    fn start(prefix: char, instance: &'a str, params: &'a ParameterMap,
             connections: &ConnectionMap, ports: &[&str])
        -> Result<Self, ElaborationError>
    {
        let mut line = spice_name(prefix, instance);
        for port in ports {
            line.push(' ');
            line.push_str(require_net(connections, port, instance)?);
        }
        Ok(Element { line, params, instance })
    }

    /// `<Prefix><name>` with no nodes (mutual, cpl, subckt).
    fn start_nodeless(prefix: char, instance: &'a str, params: &'a ParameterMap) -> Self {
        Element { line: spice_name(prefix, instance), params, instance }
    }

    /// Required real, emitted bare right after the nodes (the element value).
    fn value(&mut self, param: &str) -> Result<(), ElaborationError> {
        let v = require_real(self.params, param, self.instance)?;
        self.line.push_str(&format!(" {v}"));
        Ok(())
    }

    /// Required string, emitted bare (model name, sense-source name).
    fn req_str(&mut self, param: &str) -> Result<(), ElaborationError> {
        let s = require_string(self.params, param, self.instance)?;
        self.line.push_str(&format!(" {s}"));
        Ok(())
    }

    /// Optional string, emitted bare only when set (e.g. an optional model card).
    fn opt_str(&mut self, param: &str) {
        let s = string_or(self.params, param, "");
        if !s.is_empty() { self.line.push_str(&format!(" {s}")); }
    }

    /// Required string emitted as `KEY=<s>` (B-source `V=`/`I=` expressions).
    fn key_str(&mut self, key: &str, param: &str) -> Result<(), ElaborationError> {
        let s = require_string(self.params, param, self.instance)?;
        self.line.push_str(&format!(" {key}={s}"));
        Ok(())
    }

    /// `KEY={<serialized expr>}` — a behavioral expression parameter.
    fn key_expr(&mut self, key: &str, param: &str, resolver: &dyn NetResolver)
        -> Result<(), ElaborationError>
    {
        match self.params.get(param) {
            Some(piperine_circuit::ParameterValue::Ast(expr)) => {
                let s = crate::expr_serializer::serialize_ngspice_expr(expr, resolver)
                    .map_err(|detail| ElaborationError::ConnectionError {
                        instance: self.instance.to_string(), detail })?;
                // B-source value form is `V=<expr>` (bare — no braces).
                self.line.push_str(&format!(" {key}={s}"));
                Ok(())
            }
            // tolerate a raw string for back-compat / pre-serialized exprs
            Some(piperine_circuit::ParameterValue::String(s)) => {
                self.line.push_str(&format!(" {key}={s}"));
                Ok(())
            }
            _ => Err(ElaborationError::MissingParameter {
                instance: self.instance.to_string(), parameter: param.to_string() }),
        }
    }

    /// Like `key_expr`, but returns `true` if emitted, `false` if the parameter was missing.
    fn opt_key_expr(&mut self, key: &str, param: &str, resolver: &dyn NetResolver)
        -> Result<bool, ElaborationError>
    {
        match self.params.get(param) {
            Some(piperine_circuit::ParameterValue::Ast(expr)) => {
                let s = crate::expr_serializer::serialize_ngspice_expr(expr, resolver)
                    .map_err(|detail| ElaborationError::ConnectionError {
                        instance: self.instance.to_string(), detail })?;
                self.line.push_str(&format!(" {key}={{{s}}}"));
                Ok(true)
            }
            Some(piperine_circuit::ParameterValue::String(s)) => {
                self.line.push_str(&format!(" {key}={s}"));
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// `KEY=<v>` only when the real value differs from its default.
    fn opt(&mut self, key: &str, param: &str, default: f64) {
        let v = real_or(self.params, param, default);
        if v != default { self.line.push_str(&format!(" {key}={v}")); }
    }

    /// `KEY=<v>` only when the integer value differs from its default.
    fn opt_int(&mut self, key: &str, param: &str, default: i64) {
        let v = real_or(self.params, param, default as f64) as i64;
        if v != default { self.line.push_str(&format!(" {key}={v}")); }
    }

    /// Bare value only when it differs from its default (gain, k, acphase).
    fn bare_opt(&mut self, param: &str, default: f64) {
        let v = real_or(self.params, param, default);
        if v != default { self.line.push_str(&format!(" {v}")); }
    }

    /// `KEY <v>` (space-separated) only when it differs (V/I source `DC`/`AC`).
    fn spaced(&mut self, key: &str, param: &str, default: f64) {
        let v = real_or(self.params, param, default);
        if v != default { self.line.push_str(&format!(" {key} {v}")); }
    }

    /// `FUNC(a b c …)` — a transient waveform. Every argument is always emitted,
    /// in order, each read with its own default.
    fn waveform(&mut self, func: &str, args: &[(&str, f64)]) {
        let inner = args.iter()
            .map(|(p, d)| real_or(self.params, p, *d).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        self.line.push_str(&format!(" {func}({inner})"));
    }

    /// `FUNC(<s>)` — a waveform whose body is a required string (PWL points).
    fn waveform_str(&mut self, func: &str, param: &str) -> Result<(), ElaborationError> {
        let s = require_string(self.params, param, self.instance)?;
        self.line.push_str(&format!(" {func}({s})"));
        Ok(())
    }

    /// Finish into a single-line instance.
    fn finish(self) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        Ok(Box::new(Line { name: self.instance.to_string(), lines: vec![self.line] }))
    }
}

/// A device instance that has already formatted its SPICE line(s).
#[derive(Debug)]
struct Line { name: String, lines: Vec<String> }

impl HardwareInstance for Line {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> { self.lines.clone() }
}

/// Shorthand for the `instantiate` signature, repeated by every device.
type Built = Result<Box<dyn HardwareInstance>, ElaborationError>;

// ── Passives ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceResistor;
impl SpiceResistor { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceResistor {
    fn name(&self) -> &str { "res" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "r_expr".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('R', name, p, c, &["p", "n"])?;
        if !e.opt_key_expr("R", "r_expr", _r)? {
            e.value("r")?;
        }
        e.opt_str("model");
        e.opt("AC", "ac", 0.0);
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.opt("L", "l", 0.0);
        e.opt("W", "w", 0.0);
        e.opt("M", "m", 1.0);
        e.opt("TC1", "tc1", 0.0);
        e.opt("TC2", "tc2", 0.0);
        e.opt("SCALE", "scale", 1.0);
        e.opt_int("NOISY", "noisy", 1);
        e.opt("BV_MAX", "bv_max", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceCapacitor;
impl SpiceCapacitor { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCapacitor {
    fn name(&self) -> &str { "cap" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "q".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('C', name, p, c, &["p", "n"])?;
        if !e.opt_key_expr("Q", "q", _r)? {
            e.value("c")?;
        }
        e.opt_str("model");
        e.opt("IC", "ic", 0.0);
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.opt("W", "w", 0.0);
        e.opt("L", "l", 0.0);
        e.opt("M", "m", 1.0);
        e.opt("TC1", "tc1", 0.0);
        e.opt("TC2", "tc2", 0.0);
        e.opt("SCALE", "scale", 1.0);
        e.opt("BV_MAX", "bv_max", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceInductor;
impl SpiceInductor { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceInductor {
    fn name(&self) -> &str { "ind" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "flux".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('L', name, p, c, &["p", "n"])?;
        if !e.opt_key_expr("FLUX", "flux", _r)? {
            e.value("l")?;
        }
        e.opt_str("model");
        e.opt("IC", "ic", 0.0);
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.opt("M", "m", 1.0);
        e.opt("TC1", "tc1", 0.0);
        e.opt("TC2", "tc2", 0.0);
        e.opt("SCALE", "scale", 1.0);
        e.opt("NT", "nt", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceMutual;
impl SpiceMutual { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceMutual {
    fn name(&self) -> &str { "mutual" }
    fn instantiate(&self, name: &str, p: &ParameterMap, _c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start_nodeless('K', name, p);
        e.req_str("inductor1")?;
        e.req_str("inductor2")?;
        e.bare_opt("k", 1.0);
        e.finish()
    }
}

// ── Independent sources ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceVoltageSource;
impl SpiceVoltageSource { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVoltageSource {
    fn name(&self) -> &str { "vsource" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.spaced("DC", "dc", 0.0);
        e.spaced("AC", "acmag", 0.0);
        e.bare_opt("acphase", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceCurrentSource;
impl SpiceCurrentSource { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCurrentSource {
    fn name(&self) -> &str { "isource" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.spaced("DC", "dc", 0.0);
        e.spaced("AC", "acmag", 0.0);
        e.bare_opt("acphase", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVpulse;
impl SpiceVpulse { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVpulse {
    fn name(&self) -> &str { "vpulse" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("PULSE", &[("v0", 0.0), ("v1", 1.0), ("td", 0.0),
            ("tr", 1e-9), ("tf", 1e-9), ("pw", 10e-9), ("per", 20e-9)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIpulse;
impl SpiceIpulse { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIpulse {
    fn name(&self) -> &str { "ipulse" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("PULSE", &[("i0", 0.0), ("i1", 1.0), ("td", 0.0),
            ("tr", 1e-9), ("tf", 1e-9), ("pw", 10e-9), ("per", 20e-9)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVsin;
impl SpiceVsin { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVsin {
    fn name(&self) -> &str { "vsin" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("SIN", &[("vo", 0.0), ("va", 1.0), ("freq", 1e6),
            ("td", 0.0), ("theta", 0.0), ("phase", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIsin;
impl SpiceIsin { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIsin {
    fn name(&self) -> &str { "isin" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("SIN", &[("io", 0.0), ("ia", 1.0), ("freq", 1e6),
            ("td", 0.0), ("theta", 0.0), ("phase", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVexp;
impl SpiceVexp { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVexp {
    fn name(&self) -> &str { "vexp" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("EXP", &[("v1", 0.0), ("v2", 1.0), ("td1", 0.0),
            ("tau1", 1e-9), ("td2", 50e-9), ("tau2", 1e-9)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIexp;
impl SpiceIexp { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIexp {
    fn name(&self) -> &str { "iexp" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("EXP", &[("i1", 0.0), ("i2", 1e-3), ("td1", 0.0),
            ("tau1", 1e-9), ("td2", 50e-9), ("tau2", 1e-9)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVpwl;
impl SpiceVpwl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVpwl {
    fn name(&self) -> &str { "vpwl" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform_str("PWL", "points")?;
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIpwl;
impl SpiceIpwl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIpwl {
    fn name(&self) -> &str { "ipwl" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform_str("PWL", "points")?;
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVsffm;
impl SpiceVsffm { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVsffm {
    fn name(&self) -> &str { "vsffm" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("SFFM", &[("vo", 0.0), ("va", 1.0), ("fc", 1e6),
            ("mdi", 1.0), ("fs", 1e4), ("phasec", 0.0), ("phases", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIsffm;
impl SpiceIsffm { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIsffm {
    fn name(&self) -> &str { "isffm" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("SFFM", &[("io", 0.0), ("ia", 1.0), ("fc", 1e6),
            ("mdi", 1.0), ("fs", 1e4), ("phasec", 0.0), ("phases", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVam;
impl SpiceVam { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVam {
    fn name(&self) -> &str { "vam" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("AM", &[("sa", 1.0), ("fc", 1e6), ("fm", 1e4), ("td", 0.0), ("phases", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIam;
impl SpiceIam { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIam {
    fn name(&self) -> &str { "iam" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("AM", &[("sa", 1.0), ("fc", 1e6), ("fm", 1e4), ("td", 0.0), ("phases", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVnoise;
impl SpiceVnoise { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVnoise {
    fn name(&self) -> &str { "vnoise" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("TRNOISE", &[("na", 0.0), ("nt", 1e-9), ("nalpha", 0.0), ("namp", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceInoise;
impl SpiceInoise { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceInoise {
    fn name(&self) -> &str { "inoise" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("TRNOISE", &[("na", 0.0), ("nt", 1e-9), ("nalpha", 0.0), ("namp", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVrandom;
impl SpiceVrandom { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVrandom {
    fn name(&self) -> &str { "vrandom" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('V', name, p, c, &["p", "n"])?;
        e.waveform("TRRANDOM", &[("rtype", 1.0), ("ts", 1e-9), ("td", 0.0),
            ("param1", 0.5), ("param2", 0.0)]);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIrandom;
impl SpiceIrandom { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIrandom {
    fn name(&self) -> &str { "irandom" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('I', name, p, c, &["p", "n"])?;
        e.waveform("TRRANDOM", &[("rtype", 1.0), ("ts", 1e-9), ("td", 0.0),
            ("param1", 0.5), ("param2", 0.0)]);
        e.finish()
    }
}

// ── Behavioral (B-) sources ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceBSourceV;
impl SpiceBSourceV { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceBSourceV {
    fn name(&self) -> &str { "bsource_v" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "V".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('B', name, p, c, &["p", "n"])?;
        e.key_expr("V", "V", _r)?;
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.opt("TC1", "tc1", 0.0);
        e.opt("TC2", "tc2", 0.0);
        e.opt_int("RECIPROCTC", "reciproctc", 0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceBSourceI;
impl SpiceBSourceI { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceBSourceI {
    fn name(&self) -> &str { "bsource_i" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "I".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('B', name, p, c, &["p", "n"])?;
        e.key_expr("I", "I", _r)?;
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.opt("TC1", "tc1", 0.0);
        e.opt("TC2", "tc2", 0.0);
        e.opt_int("RECIPROCTC", "reciproctc", 0);
        e.finish()
    }
}

// ── Controlled sources ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceVcvs;
impl SpiceVcvs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVcvs {
    fn name(&self) -> &str { "vcvs" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "vol".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('E', name, p, c, &["p", "n", "cp", "cn"])?;
        if !e.opt_key_expr("VOL", "vol", _r)? {
            e.bare_opt("gain", 1.0);
        }
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceVccs;
impl SpiceVccs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVccs {
    fn name(&self) -> &str { "vccs" }
    fn parameters(&self) -> &[ParameterDefinition] {
        static PARAMS: std::sync::OnceLock<Vec<ParameterDefinition>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| vec![ParameterDefinition { name: "cur".into(), is_expr: true, is_ref: false, default: None }])
    }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('G', name, p, c, &["p", "n", "cp", "cn"])?;
        if !e.opt_key_expr("CUR", "cur", _r)? {
            e.bare_opt("gm", 1e-3);
        }
        e.opt("M", "m", 1.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceCcvs;
impl SpiceCcvs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCcvs {
    fn name(&self) -> &str { "ccvs" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('H', name, p, c, &["p", "n"])?;
        e.req_str("vsrc")?;
        e.bare_opt("transres", 1.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceCccs;
impl SpiceCccs { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCccs {
    fn name(&self) -> &str { "cccs" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('F', name, p, c, &["p", "n"])?;
        e.req_str("vsrc")?;
        e.bare_opt("gain", 1.0);
        e.opt("M", "m", 1.0);
        e.finish()
    }
}

// ── Switches ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceVsw;
impl SpiceVsw { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVsw {
    fn name(&self) -> &str { "vsw" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('S', name, p, c, &["p", "n", "cp", "cn"])?;
        e.req_str("model")?;
        e.opt_int("ON", "on", 0);
        e.opt_int("OFF", "off", 0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceIsw;
impl SpiceIsw { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceIsw {
    fn name(&self) -> &str { "isw" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('W', name, p, c, &["p", "n"])?;
        e.req_str("vsrc")?;
        e.req_str("model")?;
        e.opt_int("ON", "on", 0);
        e.opt_int("OFF", "off", 0);
        e.finish()
    }
}

// ── Semiconductors ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceDiode;
impl SpiceDiode { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceDiode {
    fn name(&self) -> &str { "d" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('D', name, p, c, &["a", "c"])?;
        e.req_str("model")?;
        e.opt("AREA", "area", 1.0);
        e.opt("PJ", "pj", 0.0);
        e.opt("W", "w", 0.0);
        e.opt("L", "l", 0.0);
        e.opt("M", "m", 1.0);
        e.opt_int("OFF", "off", 0);
        e.opt("IC", "ic", 0.0);
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.finish()
    }
}

/// Bipolar junction transistor. `prefix` is always `'Q'`; `sub` is the optional
/// 4th substrate node present on the `npn4`/`pnp4` variants.
fn bjt(name: &str, p: &ParameterMap, c: &ConnectionMap, ports: &[&str]) -> Built {
    let mut e = Element::start('Q', name, p, c, ports)?;
    e.req_str("model")?;
    e.opt("AREA", "area", 1.0);
    e.opt("AREAB", "areab", 1.0);
    e.opt("AREAC", "areac", 1.0);
    e.opt("M", "m", 1.0);
    e.opt_int("OFF", "off", 0);
    e.opt("ICVBE", "icvbe", 0.0);
    e.opt("ICVCE", "icvce", 0.0);
    e.opt("TEMP", "temp", 27.0);
    e.opt("DTEMP", "dtemp", 0.0);
    e.finish()
}

#[derive(Debug)]
pub struct SpiceNpn;
impl SpiceNpn { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceNpn {
    fn name(&self) -> &str { "npn" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        bjt(name, p, c, &["c", "b", "e"])
    }
}

#[derive(Debug)]
pub struct SpicePnp;
impl SpicePnp { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePnp {
    fn name(&self) -> &str { "pnp" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        bjt(name, p, c, &["c", "b", "e"])
    }
}

#[derive(Debug)]
pub struct SpiceNpn4;
impl SpiceNpn4 { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceNpn4 {
    fn name(&self) -> &str { "npn4" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        bjt(name, p, c, &["c", "b", "e", "sub"])
    }
}

#[derive(Debug)]
pub struct SpicePnp4;
impl SpicePnp4 { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePnp4 {
    fn name(&self) -> &str { "pnp4" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        bjt(name, p, c, &["c", "b", "e", "sub"])
    }
}

/// MOSFET (`M` element). Shared by `nmos`/`pmos`; the n/p type lives on the model.
fn mosfet(name: &str, p: &ParameterMap, c: &ConnectionMap) -> Built {
    let mut e = Element::start('M', name, p, c, &["d", "g", "s", "b"])?;
    e.req_str("model")?;
    e.opt("W", "w", 1e-6);
    e.opt("L", "l", 100e-9);
    e.opt("AD", "ad", 0.0);
    e.opt("AS", "as", 0.0);
    e.opt("PD", "pd", 0.0);
    e.opt("PS", "ps", 0.0);
    e.opt("NRD", "nrd", 0.0);
    e.opt("NRS", "nrs", 0.0);
    e.opt("M", "m", 1.0);
    e.opt_int("OFF", "off", 0);
    e.opt("ICVDS", "icvds", 0.0);
    e.opt("ICVGS", "icvgs", 0.0);
    e.opt("ICVBS", "icvbs", 0.0);
    e.opt("TEMP", "temp", 27.0);
    e.opt("DTEMP", "dtemp", 0.0);
    e.opt("NF", "nf", 1.0);
    e.opt("SA", "sa", 0.0);
    e.opt("SB", "sb", 0.0);
    e.finish()
}

#[derive(Debug)]
pub struct SpiceNmos;
impl SpiceNmos { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceNmos {
    fn name(&self) -> &str { "nmos" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        mosfet(name, p, c)
    }
}

#[derive(Debug)]
pub struct SpicePmos;
impl SpicePmos { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePmos {
    fn name(&self) -> &str { "pmos" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        mosfet(name, p, c)
    }
}

/// JFET (`J` element). Shared by `jfet_n`/`jfet_p`; the n/p type lives on the model.
fn jfet(name: &str, p: &ParameterMap, c: &ConnectionMap) -> Built {
    let mut e = Element::start('J', name, p, c, &["d", "g", "s"])?;
    e.req_str("model")?;
    e.opt("AREA", "area", 1.0);
    e.opt("M", "m", 1.0);
    e.opt_int("OFF", "off", 0);
    e.opt("IC", "ic", 0.0);
    e.opt("TEMP", "temp", 27.0);
    e.opt("DTEMP", "dtemp", 0.0);
    e.finish()
}

#[derive(Debug)]
pub struct SpiceJfetN;
impl SpiceJfetN { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceJfetN {
    fn name(&self) -> &str { "jfet_n" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        jfet(name, p, c)
    }
}

#[derive(Debug)]
pub struct SpiceJfetP;
impl SpiceJfetP { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceJfetP {
    fn name(&self) -> &str { "jfet_p" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        jfet(name, p, c)
    }
}

/// MESFET (`Z` element). Shared by `mesfet_n`/`mesfet_p`.
fn mesfet(name: &str, p: &ParameterMap, c: &ConnectionMap) -> Built {
    let mut e = Element::start('Z', name, p, c, &["d", "g", "s"])?;
    e.req_str("model")?;
    e.opt("AREA", "area", 1.0);
    e.opt("M", "m", 1.0);
    e.opt_int("OFF", "off", 0);
    e.opt("ICVDS", "icvds", 0.0);
    e.opt("ICVGS", "icvgs", 0.0);
    e.finish()
}

#[derive(Debug)]
pub struct SpiceMesfetN;
impl SpiceMesfetN { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceMesfetN {
    fn name(&self) -> &str { "mesfet_n" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        mesfet(name, p, c)
    }
}

#[derive(Debug)]
pub struct SpiceMesfetP;
impl SpiceMesfetP { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceMesfetP {
    fn name(&self) -> &str { "mesfet_p" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        mesfet(name, p, c)
    }
}

#[derive(Debug)]
pub struct SpiceVdmos;
impl SpiceVdmos { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceVdmos {
    fn name(&self) -> &str { "vdmos" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('M', name, p, c, &["d", "g", "s"])?;
        e.req_str("model")?;
        e.opt("W", "w", 1e-3);
        e.opt("L", "l", 1e-6);
        e.opt("M", "m", 1.0);
        e.opt_int("OFF", "off", 0);
        e.opt("ICVDS", "icvds", 0.0);
        e.opt("ICVGS", "icvgs", 0.0);
        e.opt("TEMP", "temp", 27.0);
        e.opt("DTEMP", "dtemp", 0.0);
        e.finish()
    }
}

// ── Transmission lines ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpiceTline;
impl SpiceTline { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceTline {
    fn name(&self) -> &str { "tline" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('T', name, p, c, &["ap", "an", "bp", "bn"])?;
        e.opt("Z0", "z0", 50.0);
        e.opt("TD", "td", 1e-9);
        e.opt("F", "f", 0.0);
        e.opt("NL", "nl", 0.25);
        e.opt("V1", "v1", 0.0);
        e.opt("V2", "v2", 0.0);
        e.opt("I1", "i1", 0.0);
        e.opt("I2", "i2", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceLtra;
impl SpiceLtra { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceLtra {
    fn name(&self) -> &str { "ltra" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('O', name, p, c, &["ap", "an", "bp", "bn"])?;
        e.req_str("model")?;
        e.opt("V1", "v1", 0.0);
        e.opt("V2", "v2", 0.0);
        e.opt("I1", "i1", 0.0);
        e.opt("I2", "i2", 0.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceUrc;
impl SpiceUrc { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceUrc {
    fn name(&self) -> &str { "urc" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('U', name, p, c, &["a", "b", "ref"])?;
        e.req_str("model")?;
        e.opt("L", "length", 1e-3);
        e.opt_int("N", "n", 0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceCpl;
impl SpiceCpl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceCpl {
    fn name(&self) -> &str { "cpl" }
    fn instantiate(&self, name: &str, p: &ParameterMap, _c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start_nodeless('P', name, p);
        e.req_str("ports")?;
        e.req_str("model")?;
        e.opt("length", "length", 1.0);
        e.opt_int("dimension", "dimension", 0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceTxl;
impl SpiceTxl { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceTxl {
    fn name(&self) -> &str { "txl" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('Y', name, p, c, &["y1p", "y1n"])?;
        e.req_str("model")?;
        e.opt("length", "length", 1.0);
        e.finish()
    }
}

// ── RF port / subcircuit ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SpicePort;
impl SpicePort { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpicePort {
    fn name(&self) -> &str { "port" }
    fn instantiate(&self, name: &str, p: &ParameterMap, c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start('P', name, p, c, &["p", "n"])?;
        e.opt_int("PORT", "num", 1);
        e.opt("Z0", "z0", 50.0);
        e.finish()
    }
}

#[derive(Debug)]
pub struct SpiceSubckt;
impl SpiceSubckt { pub fn new() -> Self { Self } }
impl HardwareDefinition for SpiceSubckt {
    fn name(&self) -> &str { "subckt" }
    fn instantiate(&self, name: &str, p: &ParameterMap, _c: &ConnectionMap, _r: &dyn NetResolver) -> Built {
        let mut e = Element::start_nodeless('X', name, p);
        e.req_str("ports")?;
        e.req_str("subckt_name")?;
        e.opt_str("params");
        e.finish()
    }
}

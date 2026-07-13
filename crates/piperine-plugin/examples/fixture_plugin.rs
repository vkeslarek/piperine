//! The test-fixture plugin: exercises both halves of the device ABI
//! (Plugin plan §6 validation matrix) —
//!
//! - `Fixture::Resistor` — an analog two-terminal resistor (param `r`,
//!   default 100 Ω) stamped through `Element::load_dc`/`load_transient`.
//! - `Fixture::Inverter` — a digital inverter through `Element::comb_phase`.
//!
//! Lives as a crate example: `cargo build --example fixture_plugin` builds
//! the cdylib the native smoke test dlopens; the e2e tests compile this
//! source in-process via `#[path]`.

use piperine_plugin::{
    entry, Abi, DeviceFactory, DeviceKind, Manifest, Permissions, Plugin, PluginDeviceSpec,
    PluginPort, PortBinding, Registrar,
};
use piperine_solver::analog::AnalogReference;
use piperine_solver::analysis::dc::DcAnalysisState;
use piperine_solver::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::core::element::{Element, ElementCapabilities};
use piperine_solver::digital::interface::{DigitalPorts, EvalCtx, EventSink};
use piperine_solver::digital::{DigitalNet, LogicValue};
use piperine_solver::math::linear::Stamp;
use piperine_solver::solver::Context;

pub struct FixturePlugin {
    manifest: Manifest,
}

impl FixturePlugin {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                name: "fixture".into(),
                abi: Abi::Native,
                entry: String::new(),
                description: Some("test fixture devices".into()),
                permissions: Permissions::default(),
            },
        }
    }
}

impl Default for FixturePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FixturePlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn register(&self, r: &mut Registrar) {
        r.device("Fixture::Resistor", Box::new(ResistorFactory));
        r.device("Fixture::Inverter", Box::new(InverterFactory));
    }
}

/// Native entry symbols (Plugin plan D7): dlopen loads this cdylib and
/// calls these two.
#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_abi_version() -> u32 {
    piperine_plugin::ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_entry() -> *mut core::ffi::c_void {
    entry(FixturePlugin::new())
}

// ─── Fixture::Resistor ─────────────────────────────────────────────────────────

struct ResistorFactory;

impl DeviceFactory for ResistorFactory {
    fn kind(&self) -> DeviceKind {
        DeviceKind::Analog
    }

    fn instantiate(&self, spec: &PluginDeviceSpec) -> Result<Box<dyn Element>, String> {
        let refs: Vec<AnalogReference> = spec
            .ports
            .iter()
            .map(|p| match &p.binding {
                PortBinding::Analog(r) => Ok(r.clone()),
                PortBinding::Digital(_) => Err(format!("port `{}` must be analog", p.logical)),
            })
            .collect::<Result<_, _>>()?;
        let [a, b] = refs.as_slice() else {
            return Err(format!("Fixture::Resistor needs 2 terminals, got {}", refs.len()));
        };
        let r = spec
            .params
            .iter()
            .find(|(n, _)| n == "r")
            .map(|(_, v)| v.coerce_real().map_err(|e| e.to_string()))
            .transpose()?
            .unwrap_or(100.0);
        if r <= 0.0 {
            return Err(format!("Fixture::Resistor: r must be positive, got {r}"));
        }
        Ok(Box::new(PluginResistor {
            label: spec.instance_label.clone(),
            a: a.clone(),
            b: b.clone(),
            g: 1.0 / r,
        }))
    }
}

/// A linear resistor: constant Jacobian `g`, zero Norton RHS.
struct PluginResistor {
    label: String,
    a: AnalogReference,
    b: AnalogReference,
    g: f64,
}

impl PluginResistor {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        let (a, b, g) = (&self.a, &self.b, self.g);
        vec![
            Stamp::Matrix(a.clone(), a.clone(), g),
            Stamp::Matrix(a.clone(), b.clone(), -g),
            Stamp::Matrix(b.clone(), a.clone(), -g),
            Stamp::Matrix(b.clone(), b.clone(), g),
        ]
    }
}

impl Element for PluginResistor {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
    }

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }

    fn load_transient(
        &mut self,
        _states: &TransientAnalysisState<'_>,
        _tran_ctx: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
}

// ─── Fixture::Inverter ─────────────────────────────────────────────────────────

struct InverterFactory;

impl DeviceFactory for InverterFactory {
    fn kind(&self) -> DeviceKind {
        DeviceKind::Digital
    }

    fn instantiate(&self, spec: &PluginDeviceSpec) -> Result<Box<dyn Element>, String> {
        let digital = |p: &PluginPort| match &p.binding {
            PortBinding::Digital(net) => Ok(*net),
            PortBinding::Analog(_) => Err(format!("port `{}` must be digital", p.logical)),
        };
        let a = spec.ports.iter().find(|p| p.logical == "a").ok_or("missing port `a`")?;
        let y = spec.ports.iter().find(|p| p.logical == "y").ok_or("missing port `y`")?;
        Ok(Box::new(PluginInverter {
            label: spec.instance_label.clone(),
            input: digital(a)?,
            output: digital(y)?,
        }))
    }
}

/// `y <- !a`, one delta cycle, X propagates as X.
struct PluginInverter {
    label: String,
    input: DigitalNet,
    output: DigitalNet,
}

impl Element for PluginInverter {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }

    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts {
            inputs: std::slice::from_ref(&self.input),
            outputs: std::slice::from_ref(&self.output),
        }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out = match ctx.nets.get(self.input.0) {
            Some(LogicValue::Zero) => LogicValue::One,
            Some(LogicValue::One) => LogicValue::Zero,
            _ => LogicValue::X,
        };
        sink.emit(self.output, out, 0.0);
    }
}

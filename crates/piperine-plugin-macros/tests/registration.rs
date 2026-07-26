//! PLG-05: `#[pip::device("Type")]` on an `Element` type registers it —
//! keyed by the type string — into the registry the host reads at load
//! (design §3: the registration is macro-generated, never an imperative
//! `register()` body). These tests run in one binary whose registry
//! contains exactly the declaration below.

use piperine_plugin::{DeviceKind, Element, PluginDevice, PluginDeviceSpec, Registry};
use piperine_solver::abi::{AnalogDevice, DigitalDevice, ElementCapabilities, Introspect};

#[piperine_plugin_macros::device("Probe::Resistor")]
struct ProbeResistor {
    /// Conductance in siemens (pub so the test can read the built value).
    pub g: f64,
}

impl PluginDevice for ProbeResistor {
    const KIND: DeviceKind = DeviceKind::Analog;

    fn from_spec(spec: &PluginDeviceSpec) -> Result<Self, String> {
        let r = spec
            .params
            .iter()
            .find(|(n, _)| n == "r")
            .map(|(_, v)| v.coerce_real().map_err(|e| e.to_string()))
            .transpose()?
            .unwrap_or(100.0);
        if r <= 0.0 {
            return Err(format!("Probe::Resistor: r must be positive, got {r}"));
        }
        Ok(Self { g: 1.0 / r })
    }
}

impl AnalogDevice for ProbeResistor {}
impl DigitalDevice for ProbeResistor {}
impl Introspect for ProbeResistor {}

impl Element for ProbeResistor {
    fn name(&self) -> &str {
        "probe"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
    }
}

fn spec_with_params(params: Vec<(String, piperine_lang::Value)>) -> PluginDeviceSpec {
    PluginDeviceSpec {
        plugin: "probe".into(),
        type_id: "Probe::Resistor".into(),
        instance_label: "r1".into(),
        attributes: Vec::new(),
        ports: Vec::new(),
        params,
    }
}

#[test]
fn device_macro_registers_exactly_the_declared_type_id() {
    let ids: Vec<&str> = Registry::devices().map(|d| d.type_id).collect();
    assert_eq!(ids, ["Probe::Resistor"], "the registry keys the device by its declared type id");
}

#[test]
fn registered_factory_reports_the_declared_kind_and_builds_the_element() {
    let reg = Registry::devices().find(|d| d.type_id == "Probe::Resistor").expect("registered");
    let factory = (reg.make)();
    assert_eq!(factory.kind(), DeviceKind::Analog, "factory kind is the declared PluginDevice::KIND");

    // The default param path: r = 100 Ω → g = 0.01 S.
    let el = factory.instantiate(&spec_with_params(Vec::new())).expect("default build");
    assert_eq!(el.capabilities(), ElementCapabilities::ANALOG);
    let direct = ProbeResistor::from_spec(&spec_with_params(Vec::new())).expect("direct default");
    assert!((direct.g - 0.01).abs() < 1e-12, "default r = 100 Ω → g = 0.01 S, got {}", direct.g);

    // A param override reaches `from_spec`: r = 50 Ω → g = 0.02 S …
    let overridden =
        spec_with_params(vec![("r".into(), piperine_lang::Value::Real(50.0))]);
    factory.instantiate(&overridden).expect("override build");
    let direct = ProbeResistor::from_spec(&overridden).expect("direct override");
    assert!((direct.g - 0.02).abs() < 1e-12, "r = 50 Ω → g = 0.02 S, got {}", direct.g);

    // … and an invalid value fails loud from the device's own validation.
    let bad = spec_with_params(vec![("r".into(), piperine_lang::Value::Real(0.0))]);
    let msg = match factory.instantiate(&bad) {
        Ok(_) => panic!("r = 0 must fail"),
        Err(msg) => msg,
    };
    assert!(msg.contains("positive"), "unexpected message: {msg}");
}

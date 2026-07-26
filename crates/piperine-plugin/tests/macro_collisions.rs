//! Declaration-time collision surface (spec Edge Cases — unchanged
//! SchemaConflict-class behavior, PLG-24): two distinct `#[pip::device]`
//! declarations sharing one `@device` type id collide loudly (P0003) when
//! the host merges them. Own test binary: this one's registry holds the two
//! colliding declarations, and only this suite may see them.

use piperine_plugin::{
    DeviceKind, Element, Manifest, Plugin, PluginDevice, PluginDeviceSpec, PluginError, PluginHost,
};
use piperine_solver::abi::{AnalogDevice, DigitalDevice, ElementCapabilities, Introspect};

#[pip::device("Dup::Device")]
struct DupA;

impl PluginDevice for DupA {
    const KIND: DeviceKind = DeviceKind::Analog;

    fn from_spec(_spec: &PluginDeviceSpec) -> Result<Self, String> {
        Err("stub device: never instantiated".into())
    }
}

impl AnalogDevice for DupA {}
impl DigitalDevice for DupA {}
impl Introspect for DupA {}

impl Element for DupA {
    fn name(&self) -> &str {
        "dup-a"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
    }
}

#[pip::device("Dup::Device")]
struct DupB;

impl PluginDevice for DupB {
    const KIND: DeviceKind = DeviceKind::Analog;

    fn from_spec(_spec: &PluginDeviceSpec) -> Result<Self, String> {
        Err("stub device: never instantiated".into())
    }
}

impl AnalogDevice for DupB {}
impl DigitalDevice for DupB {}
impl Introspect for DupB {}

impl Element for DupB {
    fn name(&self) -> &str {
        "dup-b"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
    }
}

struct OnePlugin {
    manifest: Manifest,
}

impl Plugin for OnePlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

#[test]
fn duplicate_device_type_ids_are_a_loud_conflict() {
    let plugin = OnePlugin {
        manifest: Manifest {
            name: "dup".into(),
            description: None,
            python: None,
            device: None,
            permissions: Default::default(),
        },
    };
    let err = PluginHost::from_plugins(vec![Box::new(plugin)])
        .map(|_| ())
        .expect_err("duplicate device type id must fail");
    match err {
        PluginError::SchemaConflict { schema, .. } => {
            assert!(schema.contains("Dup::Device"), "conflict must name the type id: {schema}");
        }
        other => panic!("expected P0003 SchemaConflict, got {other}"),
    }
}

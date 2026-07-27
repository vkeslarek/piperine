//! Scope: `piperine_api::model`'s descriptor layer (CLA-17) — the reflected
//! children of a module (`Port`, `Net`, `Instance`, `Param`, `Behavior`,
//! `Terminal`) and the four device descriptors the api re-exports. Each
//! accessor is read back against an elaborated fixture design and compared to
//! what the author wrote, so a snapshot that drops or mangles a field fails
//! here rather than at a host call site.

use piperine_api::model::{
    Behavior, Instance, ModelDescriptor, Net, ObservableDescriptor, Param, ParamDescriptor, Port,
    Terminal, TerminalDescriptor,
};
use piperine_lang::parse::ast::{BehaviorKind, Direction};
use piperine_lang::{parse_and_elaborate, Design, SourceMap, Value, ValueType};

/// A two-level fixture: `Top` instantiates `Amp`, which declares one port of
/// every direction, a wire, a defaulted param, an undefaulted param, and an
/// analog block.
const FIXTURE: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Amp(input a: Electrical, output z: Electrical, inout bias: Electrical) {
    param gain: Real = 2.5;
    param label: String;
    wire mid : Electrical;
}
analog Amp { I(a, z) <+ V(a, z) * gain; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire vout : Electrical;
    x1 : Amp(.a = vin, .z = vout, .bias = gnd) { .gain = 4.0 };
}
";

fn fixture() -> Design {
    parse_and_elaborate(FIXTURE, &SourceMap::dummy()).expect("fixture elaborates")
}

#[test]
fn port_reflects_name_direction_and_discipline() {
    let design = fixture();
    let amp = design.module("Amp").expect("Amp present");
    let ports: Vec<Port> = amp.ports().iter().map(Port::of).collect();

    let names: Vec<&str> = ports.iter().map(Port::name).collect();
    assert_eq!(names, vec!["a", "z", "bias"], "ports in declaration order");

    let directions: Vec<&Direction> = ports.iter().map(Port::direction).collect();
    assert_eq!(
        directions,
        vec![&Direction::Input, &Direction::Output, &Direction::Inout],
        "each port's authored direction, typed (not stringified)"
    );

    assert_eq!(ports[0].ty(), "Electrical", "the port's discipline name");
}

#[test]
fn net_reflects_name_and_discipline() {
    let design = fixture();
    let amp = design.module("Amp").expect("Amp present");
    let nets: Vec<Net> = amp.wires().iter().map(Net::of).collect();

    assert_eq!(nets.iter().map(Net::name).collect::<Vec<_>>(), vec!["mid"]);
    assert_eq!(nets[0].ty(), "Electrical");
}

#[test]
fn instance_reflects_label_and_instantiated_module() {
    let design = fixture();
    let top = design.module("Top").expect("Top present");
    let instances: Vec<Instance> = top.instances().iter().map(Instance::of).collect();

    assert_eq!(instances.len(), 1, "Top instantiates exactly one submodule");
    assert_eq!(instances[0].name(), "x1", "the authored instance label");
    assert_eq!(instances[0].module(), "Amp", "the module it instantiates");
}

#[test]
fn param_reflects_name_type_and_default() {
    let design = fixture();
    let amp = design.module("Amp").expect("Amp present");
    let params: Vec<Param> = amp.params().iter().map(Param::of).collect();

    let gain = params.iter().find(|p| p.name() == "gain").expect("`gain` reflected");
    assert_eq!(gain.ty(), &ValueType::Real, "declared value type, typed");
    assert_eq!(gain.default(), Some(&Value::Real(2.5)), "the pre-folded default");

    let label = params.iter().find(|p| p.name() == "label").expect("`label` reflected");
    assert_eq!(label.ty(), &ValueType::Str);
    assert_eq!(label.default(), None, "an undefaulted param reports no default");
}

#[test]
fn behavior_reflects_name_and_kind() {
    let design = fixture();
    let amp = design.module("Amp").expect("Amp present");
    let behaviors: Vec<Behavior> = amp.behaviors().iter().map(Behavior::of).collect();

    assert_eq!(behaviors.len(), 1, "Amp declares one behavior block");
    assert_eq!(behaviors[0].name(), "Amp");
    assert_eq!(behaviors[0].kind(), &BehaviorKind::Analog, "the block's authored kind");
}

#[test]
fn terminal_pairs_a_port_with_its_net() {
    let terminal = Terminal::new("a".to_string(), "vin".to_string());
    assert_eq!(terminal.port(), "a");
    assert_eq!(terminal.net(), "vin");
}

/// The four device descriptors are part of the model surface (CLA-17's eight
/// types), and they are **the api's existing descriptor types** — re-exported,
/// not duplicated. The four annotated bindings below only compile if
/// `model::X` and `piperine_api::X` name the same type, so a second, drifting
/// set of descriptors cannot be introduced without failing this target.
#[test]
fn the_four_device_descriptors_are_the_apis_own_types() {
    let model: piperine_api::ModelDescriptor = ModelDescriptor::default();
    assert_eq!(model.type_id, "", "the no-identity-declared default");
    assert_eq!(model.version, "");

    let terminals: Vec<piperine_api::TerminalDescriptor> = Vec::<TerminalDescriptor>::new();
    let observables: Vec<piperine_api::ObservableDescriptor> = Vec::<ObservableDescriptor>::new();
    let params: Vec<piperine_api::ParamDescriptor> = Vec::<ParamDescriptor>::new();
    assert_eq!((terminals.len(), observables.len(), params.len()), (0, 0, 0));
}

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
use piperine_lang::pom::Direction;
use piperine_lang::{parse_and_elaborate, Design, SourceMap, Value, ValueType};
use piperine_lang::ast::BehaviorKind;

/// A two-level fixture: `Top` instantiates `Amp`, which declares one port of
/// every direction, a wire, two params, and an analog block. (PHDL's
/// undefaulted `param label: String;` has no VAMS spelling — Annex A's
/// `param_assignment` makes the default mandatory — so `label` carries `""`.)
const FIXTURE: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Amp (input electrical a, output electrical z, inout electrical bias);
    parameter real gain = 2.5;
    parameter string label = "";
    electrical mid;
    analog I(a, z) <+ V(a, z) * gain;
endmodule

module Top ();
    electrical gnd, vin, vout;
    Amp #(.gain(4.0)) x1 (.a(vin), .z(vout), .bias(gnd));
endmodule
"#;

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

    assert_eq!(ports[0].ty(), "electrical", "the port's discipline name");
}

#[test]
fn net_reflects_name_and_discipline() {
    let design = fixture();
    let amp = design.module("Amp").expect("Amp present");
    let nets: Vec<Net> = amp.wires().iter().map(Net::of).collect();

    assert_eq!(nets.iter().map(Net::name).collect::<Vec<_>>(), vec!["mid"]);
    assert_eq!(nets[0].ty(), "electrical");
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

/// Every VAMS parameter carries a default (Annex A's `param_assignment`
/// requires one) — the fixture's `label` declares `""` and the readback
/// reports it.
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
    // VAMS Annex A's `param_assignment` makes the default mandatory — the
    // LRM has no undefaulted-parameter form, so `label` carries `""` (the
    // declared default), and the block below asserts exactly that truth
    // rather than the PHDL-era "no default" the old fixture checked.
    assert_eq!(label.default(), Some(&Value::Str(String::new())), "the declared default (the empty string)");
}

/// Gap-fix batch: LRM §2.9.2 standard attributes (`desc`/`units`/`op`/
/// `multiplicity`, T27/LRMW3-10) already parse and land in the POM
/// (`piperine-lang/tests/standard_attributes.rs`) — this proves the other
/// half spec.md's AC4 requires: they surface on the host-facing `Param`
/// descriptor too, through the actual `piperine_api::model` API a host
/// calls (not by reaching into POM internals).
#[test]
fn param_surfaces_standard_attributes_on_the_host_descriptor() {
    let src = r#"
        module m ();
            (* desc="effective resistance", units="Ohms", op="yes", multiplicity="divide" *)
            parameter real reff = 1.0;
            parameter real plain = 2.0;
        endmodule
        "#;
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("fixture elaborates");
    let m = design.module("m").expect("m present");
    let params: Vec<Param> = m.params().iter().map(Param::of).collect();

    let reff = params.iter().find(|p| p.name() == "reff").expect("`reff` reflected");
    assert_eq!(reff.desc(), Some("effective resistance"));
    assert_eq!(reff.units(), Some("Ohms"));
    assert_eq!(reff.op(), Some("yes"));
    assert_eq!(reff.multiplicity(), Some("divide"));

    // A param with no standard attributes carries `None` for all four —
    // not an empty string or a spurious default.
    let plain = params.iter().find(|p| p.name() == "plain").expect("`plain` reflected");
    assert_eq!(plain.desc(), None);
    assert_eq!(plain.units(), None);
    assert_eq!(plain.op(), None);
    assert_eq!(plain.multiplicity(), None);
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

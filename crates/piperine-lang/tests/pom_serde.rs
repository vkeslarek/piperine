//! The POM is the serialization contract (SPEC Part IV §7): the real
//! `Design`/`Value` round-trip through serde as themselves — there is no
//! shadow wire model. These tests pin that contract.

use piperine_lang::parse_and_elaborate;
use piperine_lang::Value;

const CIRCUIT: &str = r#"
discipline Electrical { potential v: Real; flow i: Real; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1.0;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider(inout a: Electrical, inout out: Electrical) {
    wire gnd : Electrical;
    r_top : Resistor(.p = a, .n = out) { .r = 1e3 };
    r_bot : Resistor(.p = out, .n = gnd) { .r = 2e3 };
}
"#;

#[test]
fn design_round_trips_through_json() {
    let design =
        parse_and_elaborate(CIRCUIT, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let json = serde_json::to_string(&design).expect("serialize design");
    let back: piperine_lang::Design = serde_json::from_str(&json).expect("deserialize design");

    assert_eq!(back.module_count(), design.module_count());
    let module = back.module("Divider").expect("Divider survives");
    assert_eq!(module.instances.len(), 2);

    let r_top = module
        .instances
        .iter()
        .find(|i| i.label.as_deref() == Some("r_top"))
        .expect("r_top survives");
    assert_eq!(r_top.module, "Resistor");
    assert_eq!(r_top.ports.len(), 2);
    let (name, value) = &r_top.params[0];
    assert_eq!(name, "r");
    match value {
        Value::Real(r) => assert_eq!(*r, 1e3),
        other => panic!("param survived as {other:?}, expected Real"),
    }

    let resistor = back.module("Resistor").expect("Resistor survives");
    assert_eq!(resistor.ports.len(), 2);
    assert_eq!(resistor.ports[0].name, "p");
}

// ─────────────────────────── POM `doc` field (LSP-07, MD-25) ───────────────

#[test]
fn module_doc_defaults_to_none_and_round_trips_as_none() {
    // T2 only adds the field; elaboration does not fill it yet (T3), so an
    // undocumented module's `doc` is `None` before and after a JSON round
    // trip — no regression, authored structure untouched.
    let design =
        parse_and_elaborate(CIRCUIT, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let resistor = design.module("Resistor").expect("Resistor exists");
    assert_eq!(resistor.doc, None);

    let json = serde_json::to_string(&design).expect("serialize design");
    let back: piperine_lang::Design = serde_json::from_str(&json).expect("deserialize design");
    let resistor_back = back.module("Resistor").expect("Resistor survives");
    assert_eq!(resistor_back.doc, None);
}

#[test]
fn module_and_port_doc_round_trip_through_json_when_present() {
    // Additive field: when `doc` is populated (as elaboration will do from
    // T3 onward), it survives a full serde round trip on both `Module` and
    // `Port`.
    use piperine_lang::parse::ast::Direction;
    use piperine_lang::pom::module::{Module, Port};
    use piperine_lang::pom::net_type::NetType;

    let port = Port {
        span: None,
        attributes: Vec::new(),
        doc: Some("The positive terminal.".to_string()),
        direction: Direction::Inout,
        name: "p".to_string(),
        ty: NetType::Discipline("Electrical".to_string()),
    };
    let mut module = Module::new(
        "Res".to_string(),
        vec![port],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    module.doc = Some("A two-terminal resistor.".to_string());

    let json = serde_json::to_string(&module).expect("serialize module");
    let back: Module = serde_json::from_str(&json).expect("deserialize module");

    assert_eq!(back.doc.as_deref(), Some("A two-terminal resistor."));
    assert_eq!(back.ports[0].doc.as_deref(), Some("The positive terminal."));
}

#[test]
fn value_data_variants_round_trip() {
    let values = vec![
        Value::Unit,
        Value::Int(-7),
        Value::Real(3.25),
        Value::Bool(true),
        Value::Str("hello".into()),
        Value::Complex(1.0, -2.0),
        Value::Tuple(vec![Value::Int(1), Value::Str("x".into())]),
        Value::Option(Some(Box::new(Value::Real(9.5)))),
    ];
    for value in values {
        let json = serde_json::to_string(&value).expect("serialize value");
        let back: Value = serde_json::from_str(&json).expect("deserialize value");
        assert_eq!(format!("{back:?}"), format!("{value:?}"), "round-trip changed {json}");
    }
}

#[test]
fn runtime_handles_fail_loud() {
    // Closures and objects are runtime handles, not data — serializing one
    // must be an error, never a silent placeholder.
    let closure = Value::Closure(std::rc::Rc::new(piperine_lang::value::Closure {
        params: vec!["x".into()],
        body: piperine_lang::parse::ast::Expr::Ident("x".into()),
        captured: Vec::new(),
    }));
    serde_json::to_string(&closure).expect_err("closure serialization must fail loud");
}

#[test]
fn flat_modules_is_skipped_on_serialize() {
    // The flat netlist is a derived artifact (rebuilt every elaboration by
    // the FlattenHierarchy pass) — never part of the wire contract. A
    // round-trip carries the authored hierarchy only.
    let design =
        parse_and_elaborate(CIRCUIT, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let json = serde_json::to_string(&design).expect("serialize design");
    assert!(
        !json.contains("flat_modules"),
        "flat_modules must be #[serde(skip)]; got: {json}"
    );
    let back: piperine_lang::Design = serde_json::from_str(&json).expect("deserialize design");
    // Authored hierarchy survives.
    assert!(back.module("Divider").is_some(), "authored Divider survives");
    assert!(back.module("Resistor").is_some(), "authored Resistor survives");
}

//! The design as one Rust source (feature `ir-rust`, RS-01 AC 2 and 3, RS-02 AC 1, 2 and 5, the
//! edge cases 1, 2 and 4; checks C2, C3, C6, C10, C100, C101, C103): the five M1 bodies, each the
//! design alone, print as the pinned files of `tests/golden/`; a second transpilation gives the
//! same bytes; every module, function, shape entry and function arm sits under a comment with its
//! IR path and `file:line:col`; each IR function is a generic `fn` and each running body a `mod`
//! with a `Device` impl; a keyword name is raw in Rust and unchanged as an ABI name; a design
//! with nothing to run has no module, builds a library of 0 modules and gives a circuit with no
//! device, and a body with no `analog` block no `terms` function.

use std::path::PathBuf;
use std::sync::Arc;

use piperine_ir::{
    Addend, BodyId, BodyNode, BodySpec, Branch, BranchKind, InstanceSpec, Ir, IrBuilder, NodeRole, Operator,
    Origin, Param, QuantityRole, Stmt, Terminal, ValueType,
};
use piperine_transpile::{Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-interp/tests/support/circuits.rs"]
mod circuits;

use circuits::Circuits;
use devices::{Devices, Ex};

/// The five M1 bodies, each alone as its design, at the values the parity twins give them, with
/// the name of its golden file.
fn m1() -> Vec<(&'static str, Ir)> {
    let alone = |build: fn(&mut IrBuilder) -> BodyId, name: &str, params: Vec<f64>| {
        let mut b = IrBuilder::new();
        let body = build(&mut b);
        let top = Devices::alone(&mut b, body, name, 1, params);
        b.finish(top)
    };
    vec![
        ("resistor", alone(Devices::resistor, "R", vec![1000.0])),
        ("capacitor", alone(Devices::capacitor, "C", vec![1e-9])),
        ("vsource", alone(Devices::vsource, "V", vec![0.0, 0.0])),
        ("isource", alone(Devices::isource, "I", vec![0.0, 0.0])),
        ("diode", alone(Devices::diode, "D", vec![1e-14, 1.0, 0.0])),
    ]
}

fn source(ir: Ir) -> String {
    let t = Transpiler::new(Arc::new(ir)).unwrap_or_else(|e| panic!("{e}"));
    t.source().unwrap_or_else(|e| panic!("{e}"))
}

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.rs"))
}

#[test]
fn the_five_m1_bodies_print_as_the_golden_files() {
    for (name, ir) in m1() {
        let path = golden(name);
        let pinned = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(source(ir), pinned, "{name}: the source differs from {}", path.display());
    }
}

#[test]
fn transpiling_twice_gives_the_same_bytes() {
    for (name, ir) in m1() {
        let once = Transpiler::new(Arc::new(ir.clone())).expect("checked");
        let first = once.source().expect("source");
        assert_eq!(once.source().expect("source"), first, "{name}: one transpiler, two calls");
        let again = Transpiler::new(Arc::new(ir)).expect("checked");
        assert_eq!(again.source().expect("source").as_bytes(), first.as_bytes(), "{name}: two transpilers");
    }
}

/// Whether `line` is `// <path> @ <file>:<line>:<col>`, the path not empty.
fn locates(line: &str) -> bool {
    let Some(body) = line.trim_start().strip_prefix("// ") else { return false };
    let Some((path, origin)) = body.rsplit_once(" @ ") else { return false };
    let mut parts = origin.rsplitn(3, ':');
    let (col, row, file) = (parts.next(), parts.next(), parts.next());
    !path.trim().is_empty()
        && file.is_some_and(|f| !f.is_empty())
        && row.is_some_and(|r| r.parse::<u32>().is_ok())
        && col.is_some_and(|c| c.parse::<u32>().is_ok())
}

/// The kind of item a line opens, if it opens one RS-02 AC 1 names.
fn item(line: &str) -> Option<&'static str> {
    let t = line.trim_start();
    if t.starts_with("pub mod ") || t.starts_with("mod ") {
        Some("mod")
    } else if t.starts_with("fn ") || t.starts_with("pub fn ") || t.starts_with("pub extern \"C\" fn ") {
        Some("fn")
    } else if ["s.pin(", "s.term(", "s.input(", "s.watch(", "s.function(", "s.reaction("].iter().any(|c| t.starts_with(c)) {
        Some("shape entry")
    } else if t.split_once(" => Self::").is_some_and(|(k, _)| k.parse::<u32>().is_ok()) {
        Some("function arm")
    } else {
        None
    }
}

#[test]
fn every_item_sits_under_its_path_and_origin() {
    for (name, ir) in m1() {
        let text = source(ir);
        let lines: Vec<&str> = text.lines().collect();
        let mut seen = std::collections::BTreeMap::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(kind) = item(line) else { continue };
            *seen.entry(kind).or_insert(0) += 1;
            // an attribute sits between an item and its comment
            let mut above = i.checked_sub(1).map(|j| lines[j]);
            if above.is_some_and(|l| l.trim_start().starts_with("#[")) {
                above = i.checked_sub(2).map(|j| lines[j]);
            }
            assert!(above.is_some_and(locates), "{name}: the {kind} on line {} has no `// path @ file:line:col` above it: {line}", i + 1);
        }
        // every kind occurs in each design
        for kind in ["mod", "fn", "shape entry", "function arm"] {
            assert!(seen.get(kind).copied().unwrap_or(0) > 0, "{name}: no {kind} found");
        }
    }
}

#[test]
fn each_function_is_a_generic_fn_and_each_body_a_device_mod() {
    let (_, diode) = m1().into_iter().find(|(n, _)| *n == "diode").expect("the diode");
    let text = source(diode.clone());
    // the two IR functions the diode reaches (std/analog.msl's vt and pnjlim), generic over Real
    let fns: Vec<&str> = text.lines().filter(|l| l.starts_with("fn ")).collect();
    assert_eq!(fns.len(), diode.functions().count(), "{fns:?}");
    for f in diode.functions() {
        let prefix = format!("fn {}<R: Real>(", f.name());
        assert!(fns.iter().any(|l| l.starts_with(&prefix)), "no `{prefix}…` in {fns:?}");
    }
    // one module per running body, each with its type and its Device impl
    let mods: Vec<&str> = text.lines().filter(|l| l.starts_with("pub mod ") && l.ends_with('{')).collect();
    assert_eq!(mods, ["pub mod diode {"]);
    assert!(text.contains("    pub struct Diode;\n") && text.contains("    impl Device for Diode {\n"), "{text}");

    // a design of several bodies: one mod each, in body order, each registered by its MSL name
    let text = source(Circuits::diode());
    let mods: Vec<&str> = text.lines().filter(|l| l.starts_with("pub mod ") && l.ends_with('{')).collect();
    assert_eq!(mods, ["pub mod vsource {", "pub mod resistor {", "pub mod diode {"]);
    for (module, ty, abi) in [("vsource", "Vsource", "Vsource"), ("resistor", "Resistor", "Resistor"), ("diode", "Diode", "Diode")] {
        assert!(text.contains(&format!("impl Device for {ty} {{")), "{ty}");
        assert!(text.contains(&format!("module::<{module}::{ty}>({abi:?}),")), "{ty}");
    }
}

/// `Loop(fn, in) { type: f64 }` with `i(fn, in) +> v(fn, in) / type`: a body, two nodes and a
/// param whose MSL names are Rust keywords.
fn keywords() -> Ir {
    let mut b = IrBuilder::new();
    let el = Devices::electrical(&mut b);
    let mut e = Ex::new(&mut b, "loop.msl", 3, 20);
    let v = e.probe(Terminal::Node(0), Terminal::Node(1));
    let r = e.param(0);
    let value = e.binary(piperine_ir::BinaryOp::Div, v, r);
    let stmt = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: QuantityRole::Flow,
        addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
    });
    let at = |line: u32, col: u32| Origin { file: "loop.msl".into(), line, col };
    let port = |name: &str, col| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at(1, col) };
    let body = b.body(BodySpec {
        name: "loop".into(),
        origin: at(1, 1),
        params: vec![Param {
            name: "type".into(),
            ty: ValueType::F64,
            default: Some(piperine_ir::Constant::Real(piperine_ir::Real::new(1.0))),
            check: None,
            structural: false,
            origin: at(2, 5),
        }],
        nodes: vec![port("fn", 12), port("in", 28)],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: Some(vec![stmt]),
        children: vec![],
    });
    let top = Devices::alone(&mut b, body, "L", 1, vec![1.0]);
    b.finish(top)
}

#[test]
fn a_keyword_identifier_is_raw_and_the_abi_name_is_not() {
    let text = source(keywords());
    // in Rust: raw identifiers (RS-02 AC 2)
    assert!(text.contains("pub mod r#loop {"), "{text}");
    assert!(text.contains("fn terms_block<R: Real>(r#fn: R, r#in: R, r#type: R) -> [R; 1] {"), "{text}");
    assert!(text.contains("let b0_flow_ = (r#fn - r#in) / r#type;"), "{text}");
    // in the ABI: the MSL names as written (edge case 4)
    assert!(text.contains("PinSpec::discrete(\"type\", role::PARAM, &F64)"), "{text}");
    assert!(text.contains("PinSpec::continuous(\"fn\", role::PORT)"), "{text}");
    assert!(text.contains("PinSpec::continuous(\"in\", role::PORT)"), "{text}");
    assert!(text.contains("module::<r#loop::Loop>(\"loop\"),"), "{text}");
}

#[test]
fn a_design_with_nothing_to_run_has_no_module() {
    // A top with no `analog` block, no held node and no child (edge case 1): a library of 0
    // modules.
    let mut b = IrBuilder::new();
    let body = b.body(BodySpec {
        name: "Empty".into(),
        origin: Origin { file: "empty.msl".into(), line: 1, col: 1 },
        params: vec![],
        nodes: vec![],
        branches: vec![],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: None,
        children: vec![],
    });
    let top = b.instance(InstanceSpec {
        path: "Empty".into(),
        name: "Empty".into(),
        body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: Origin { file: "empty.msl".into(), line: 1, col: 1 },
    });
    let ir = b.finish(top);
    let text = source(ir.clone());
    let mods: Vec<&str> = text.lines().filter(|l| l.trim_start().starts_with("pub mod ")).collect();
    assert_eq!(mods, ["pub mod rt;"], "{text}");
    assert!(!text.contains("impl Device"), "{text}");
    assert!(text.contains("static MODULES: OnceLock<[Module; 0]> = OnceLock::new();"), "{text}");
    assert!(text.contains("let modules = MODULES.get_or_init(|| []);"), "{text}");

    // verification F8: the library is built and loads with 0 modules, and the circuit made from
    // it binds no device
    let cache = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let compiled = Transpiler::new(Arc::new(ir)).and_then(|t| t.compile(&settings)).unwrap_or_else(|e| panic!("{e}"));
    assert!(compiled.loaded().file().is_file(), "a library is built");
    assert_eq!(compiled.loaded().names(), Vec::<&str>::new(), "0 modules");
    let circuit = compiled.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let bound = circuit.bound().expect("the circuit is bound");
    assert_eq!(bound.names.instances, Vec::<String>::new(), "a circuit with zero devices");
}

#[test]
fn a_body_without_analog_prints_no_terms_function() {
    // `RcHeld` holds `out` at 0 V and has no `analog` block (edge case 2): it runs, with its
    // shape and no function
    let text = source(Circuits::rc_held());
    let start = text.find("pub mod rc_held {").expect("RcHeld has a module");
    let held = &text[start..];
    let end = held.find("\n}\n").map_or(held.len(), |e| e + 3);
    let held = &held[..end];
    assert!(!held.contains("fn terms"), "{held}");
    assert!(!held.contains(" => Self::"), "{held}");
    assert!(held.contains("impl Device for RcHeld {"), "{held}");
    assert!(held.contains("fn eval(&self, k: u32, _cx: &mut Eval<'_>) -> Result<(), Fault> {"), "{held}");
    assert!(held.contains("// pin 1 `out` @ "), "{held}");
    // the bodies that have one keep their terms function
    assert_eq!(text.matches("fn terms(cx_: &mut Eval<'_>)").count(), 3, "Vsource, Resistor, Capacitor");
}

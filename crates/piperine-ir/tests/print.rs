//! Scope: the one-way printer of `piperine-ir` (IR-04). A golden text per device body and one for
//! the divider, compared byte for byte; the text is deterministic; unequal IRs print different
//! texts; no function builds an `Ir` from text. `PIPERINE_UPDATE_GOLDEN=1` rewrites the goldens.

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::{Devices, Ex};
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodySpec, Branch, BranchKind, ExprId, InstanceId, Ir, IrBuilder,
    NodeRole, Operator, Origin, QuantityRole, Stmt, Terminal,
};
use serde_json::{Value, json};

/// One golden case: a name, the IR it prints.
struct Case {
    name: &'static str,
    ir: Ir,
}

struct Cases;

impl Cases {
    /// One device body, placed alone as the design.
    fn device(
        name: &'static str,
        make: fn(&mut IrBuilder) -> BodyId,
        line: u32,
        params: Vec<f64>,
    ) -> Case {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let top: InstanceId = Devices::alone(&mut b, body, name, line, params);
        Case { name, ir: b.finish(top) }
    }

    fn all() -> Vec<Case> {
        let mut b = IrBuilder::new();
        let top = Devices::divider(&mut b);
        vec![
            Cases::device("resistor", Devices::resistor, 5, vec![1000.0]),
            Cases::device("capacitor", Devices::capacitor, 11, vec![1e-12]),
            Cases::device("vsource", Devices::vsource, 17, vec![10.0, 0.0]),
            Cases::device("isource", Devices::isource, 24, vec![1e-3, 0.0]),
            Cases::device("diode", Devices::diode, 31, vec![1e-14, 1.0, 0.0]),
            Case { name: "divider", ir: b.finish(top) },
        ]
    }

    fn resistor() -> Ir {
        let mut b = IrBuilder::new();
        let body = Devices::resistor(&mut b);
        let top = Devices::alone(&mut b, body, "resistor", 5, vec![1000.0]);
        b.finish(top)
    }

    /// The resistor IR after `edit` of its serialized form.
    fn edited(edit: impl FnOnce(&mut Value)) -> Ir {
        let mut v = serde_json::to_value(Cases::resistor()).expect("serialize");
        edit(&mut v);
        serde_json::from_value(v).expect("still an Ir")
    }

    /// The first expression of `variant` in the serialized arena.
    fn first(v: &Value, variant: &str) -> usize {
        v["exprs"]
            .as_array()
            .expect("exprs")
            .iter()
            .position(|e| e["kind"].as_object().is_some_and(|o| o.contains_key(variant)))
            .expect("an expression of the variant")
    }

    fn golden_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.txt"))
    }
}

#[test]
fn printed_text_shows_every_item() {
    for case in Cases::all() {
        let text = case.ir.to_string();
        let path = Cases::golden_path(case.name);
        if std::env::var_os("PIPERINE_UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &text).expect("write a golden");
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(text, golden, "golden {} differs", case.name);
        // Every body, function, instance and node of the IR appears in the text.
        for body in case.ir.bodies() {
            assert!(text.contains(&format!("body {}  @", body.name())), "{}: body {}", case.name, body.name());
        }
        for function in case.ir.functions() {
            assert!(text.contains(&format!("fn {}(", function.name())), "{}: fn {}", case.name, function.name());
        }
        for instance in case.ir.instances() {
            assert!(text.contains(&format!("instance {}: ", instance.path())), "{}: {}", case.name, instance.path());
        }
        for node in case.ir.nodes() {
            assert!(text.contains(&format!("node {}: ", node.path())), "{}: {}", case.name, node.path());
        }
    }
}

#[test]
fn printing_twice_is_byte_identical() {
    for case in Cases::all() {
        assert_eq!(case.ir.to_string(), case.ir.to_string(), "{}", case.name);
    }
    // Two builds of the same sequence print the same text as well.
    let build = || {
        let mut b = IrBuilder::new();
        let top = Devices::divider(&mut b);
        b.finish(top)
    };
    assert_eq!(build().to_string(), build().to_string());
}

#[test]
fn no_parser_of_the_text_exists() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&dir).expect("read src") {
        let path = entry.expect("dir entry").path();
        let text = std::fs::read_to_string(&path).expect("read a source");
        for needle in ["FromStr", "fn parse", "fn from_text", "fn from_str"] {
            assert!(!text.contains(needle), "{} holds `{needle}`", path.display());
        }
    }
}

#[test]
fn unequal_irs_print_different_texts() {
    let with = |r: f64| {
        let mut b = IrBuilder::new();
        let body = Devices::resistor(&mut b);
        let top = Devices::alone(&mut b, body, "resistor", 5, vec![r]);
        b.finish(top)
    };
    let (a, b) = (with(1000.0), with(1001.0));
    assert_ne!(a, b);
    assert_ne!(a.to_string(), b.to_string());
    // Unequal in one field the tree does not show: each edit gives an unequal IR and a new text.
    let base = Cases::resistor();
    let edits: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
        ("an expression origin", Box::new(|v: &mut Value| v["exprs"][0]["origin"]["line"] = json!(999))),
        ("an expression origin file", Box::new(|v: &mut Value| v["exprs"][0]["origin"]["file"] = json!("other.msl"))),
        ("an expression type", Box::new(|v: &mut Value| {
            let probe = Cases::first(v, "Probe");
            v["exprs"][probe]["ty"] = json!("Bool");
        })),
        ("a NaN payload", Box::new(|v: &mut Value| v["instances"][0]["params"][0] = json!({"Real": 0x7ff8_0000_0000_0001_u64}))),
        ("an instance name", Box::new(|v: &mut Value| v["instances"][0]["name"] = json!("other"))),
        ("the top id", Box::new(|v: &mut Value| v["top"] = json!(7))),
        ("an unreachable expression", Box::new(|v: &mut Value| {
            let unit = json!({"kind": "Unit", "ty": "Unit", "origin": {"file": "devices.msl", "line": 1, "col": 1}});
            v["exprs"].as_array_mut().expect("exprs").push(unit);
        })),
        ("a name holding a line break", Box::new(|v: &mut Value| v["bodies"][0]["nodes"][0]["name"] = json!("p\n  node n"))),
    ];
    let mut texts = vec![base.to_string()];
    for (what, edit) in &edits {
        let ir = Cases::edited(|v| edit(v));
        assert_ne!(ir, base, "{what}: the edit changes the IR");
        let text = ir.to_string();
        assert!(!texts.contains(&text), "{what}: prints like an earlier IR");
        texts.push(text);
    }
    // Two NaN payloads: unequal values, different texts.
    let nan = |bits: u64| Cases::edited(|v| v["instances"][0]["params"][0] = json!({"Real": bits}));
    let (x, y) = (nan(0x7ff8_0000_0000_0001), nan(0x7ff8_0000_0000_0002));
    assert_ne!(x, y);
    assert_ne!(x.to_string(), y.to_string());
    // Each pair of distinct golden cases differs too.
    let cases = Cases::all();
    for (i, x) in cases.iter().enumerate() {
        for y in &cases[i + 1..] {
            assert_ne!(x.ir.to_string(), y.ir.to_string(), "{} vs {}", x.name, y.name);
        }
    }
}

/// Expressions whose tree is too deep, cyclic, or shared for a recursive printer.
struct Shapes;

impl Shapes {
    fn ir(value: impl FnOnce(&mut Ex<'_>) -> ExprId) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let value = value(&mut Ex::new(&mut b, "shape.msl", 2, 3));
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        let origin = Origin { file: "shape.msl".into(), line: 1, col: 1 };
        let port = |name: &str| piperine_ir::BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() };
        let body = b.body(BodySpec {
            name: "Shape".into(),
            origin: origin.clone(),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![stmt]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "Shape", 1, vec![]);
        b.finish(top)
    }
}

#[test]
fn a_cyclic_deep_or_shared_ir_prints() {
    // A `Binary` whose left operand is itself, deserialized: the printer marks the cycle.
    let ir = Cases::edited(|v| {
        let binary = Cases::first(v, "Binary");
        v["exprs"][binary]["kind"]["Binary"]["lhs"] = json!(binary);
    });
    assert!(!ir.check().is_empty());
    let text = ir.to_string();
    let binary = (0..).find(|&i| matches!(ir.expr(ExprId(i)), piperine_ir::Expr::Binary { .. })).expect("binary");
    assert!(text.contains(&format!("<cycle e{binary}>")), "{text}");

    // A sum 10 000 terms deep prints whole.
    let ir = Shapes::ir(|e| {
        let mut acc = e.probe(Terminal::Node(0), Terminal::Node(1));
        for _ in 0..10_000 {
            let one = e.real(1.0);
            acc = e.binary(BinaryOp::Add, acc, one);
        }
        acc
    });
    let text = ir.to_string();
    assert_eq!(text.matches(" + 1.0)").count(), 10_000);

    // `acc + acc` doubled 40 times: the second use of an id prints as a reference to it.
    let started = std::time::Instant::now();
    let ir = Shapes::ir(|e| {
        let mut acc = e.probe(Terminal::Node(0), Terminal::Node(1));
        for _ in 0..40 {
            acc = e.binary(BinaryOp::Add, acc, acc);
        }
        acc
    });
    let text = ir.to_string();
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
    assert!(text.contains("(probe(p, n) + e0)"), "{text}");
}

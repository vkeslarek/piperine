//! Scope: the one-way printer of `piperine-ir` (IR-04) and its structural form (IR-11). A golden
//! text per device body and one for the divider in each form, compared byte for byte: the full
//! text under `tests/golden/full/`, the structural form (the contract with `msl-elab`) under
//! `tests/golden/`; the text is deterministic; unequal IRs print different texts; the structural
//! form ignores origins and arena order and shows forms and types; no function builds an `Ir`
//! from text. `PIPERINE_UPDATE_GOLDEN=1` rewrites the goldens.

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;
#[path = "support/sources.rs"]
mod sources;

use devices::{Devices, Ex};
use sources::SourceTree;
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

    /// The golden of `name`: the structural form under `tests/golden/`, the full text under
    /// `tests/golden/full/`.
    fn golden_path(name: &str, structural: bool) -> std::path::PathBuf {
        let dir = if structural { "tests/golden" } else { "tests/golden/full" };
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir).join(format!("{name}.txt"))
    }

    /// `text` against its golden, rewritten first under `PIPERINE_UPDATE_GOLDEN`.
    fn golden(name: &str, structural: bool, text: &str) {
        let path = Cases::golden_path(name, structural);
        if std::env::var_os("PIPERINE_UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, text).expect("write a golden");
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(text, golden, "golden {} differs", path.display());
    }
}

/// Rewrites of a serialized IR that keep its structure: every origin, or the order of every
/// arena but the instance table (whose pre-order IR-08 fixes).
struct Rewrite;

impl Rewrite {
    /// Every `origin` in `v` set to one place.
    fn origins(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (key, field) in map.iter_mut() {
                    if key == "origin" {
                        *field = json!({"file": "elsewhere.msl", "line": 77, "col": 3});
                    } else {
                        Rewrite::origins(field);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(Rewrite::origins),
            _ => {}
        }
    }

    /// The arena `name` reversed; returns its length.
    fn reverse(v: &mut Value, name: &str) -> usize {
        let arena = v[name].as_array_mut().expect("an arena");
        arena.reverse();
        arena.len()
    }

    /// An id of a reversed arena of `len` items, in place.
    fn id(v: &mut Value, len: usize) {
        if let Some(i) = v.as_u64() {
            *v = json!(len as u64 - 1 - i);
        }
    }

    fn ids(v: &mut Value, len: usize) {
        if let Some(items) = v.as_array_mut() {
            items.iter_mut().for_each(|i| Rewrite::id(i, len));
        }
    }

    /// The natures, bodies, functions, expressions, statements and nodes reversed, every
    /// reference to them renumbered: the same IR built in another order.
    fn arenas(v: &mut Value) {
        let natures = Rewrite::reverse(v, "natures");
        let bodies = Rewrite::reverse(v, "bodies");
        let functions = Rewrite::reverse(v, "functions");
        let exprs = Rewrite::reverse(v, "exprs");
        let stmts = Rewrite::reverse(v, "stmts");
        let nodes = Rewrite::reverse(v, "nodes");
        for body in v["bodies"].as_array_mut().expect("bodies") {
            for param in body["params"].as_array_mut().expect("params") {
                Rewrite::id(&mut param["check"], exprs);
            }
            for node in body["nodes"].as_array_mut().expect("nodes") {
                Rewrite::id(&mut node["nature"], natures);
            }
            Rewrite::ids(&mut body["analog"], stmts);
            for child in body["children"].as_array_mut().expect("children") {
                Rewrite::id(&mut child["body"], bodies);
            }
        }
        for function in v["functions"].as_array_mut().expect("functions") {
            Rewrite::id(&mut function["body"], exprs);
        }
        for expr in v["exprs"].as_array_mut().expect("exprs") {
            let Some((variant, fields)) = expr["kind"].as_object_mut().and_then(|o| o.iter_mut().next()) else { continue };
            match variant.as_str() {
                "Unary" => Rewrite::id(&mut fields["expr"], exprs),
                "Binary" => ["lhs", "rhs"].iter().for_each(|k| Rewrite::id(&mut fields[*k], exprs)),
                "If" => ["cond", "then", "otherwise"].iter().for_each(|k| Rewrite::id(&mut fields[*k], exprs)),
                "Block" => {
                    Rewrite::ids(&mut fields["stmts"], stmts);
                    Rewrite::id(&mut fields["value"], exprs);
                }
                "Call" => {
                    Rewrite::id(&mut fields["function"], functions);
                    Rewrite::ids(&mut fields["args"], exprs);
                }
                "Math" => Rewrite::ids(&mut fields["args"], exprs),
                "Closure" => Rewrite::id(&mut fields["body"], exprs),
                "Limit" => ["x", "f", "seed"].iter().for_each(|k| Rewrite::id(&mut fields[*k], exprs)),
                "Stim" => ["mag", "phase"].iter().for_each(|k| Rewrite::id(&mut fields[*k], exprs)),
                _ => {}
            }
        }
        for stmt in v["stmts"].as_array_mut().expect("stmts") {
            let Some((variant, fields)) = stmt.as_object_mut().and_then(|o| o.iter_mut().next()) else { continue };
            match variant.as_str() {
                "Let" => Rewrite::id(&mut fields["init"], exprs),
                "Expr" => Rewrite::id(fields, exprs),
                _ => {
                    for addend in fields["addends"].as_array_mut().expect("addends") {
                        Rewrite::id(&mut addend["value"], exprs);
                    }
                }
            }
        }
        for instance in v["instances"].as_array_mut().expect("instances") {
            Rewrite::id(&mut instance["body"], bodies);
            for node in instance["nodes"].as_array_mut().expect("nodes") {
                if node.get("Node").is_some() {
                    Rewrite::id(&mut node["Node"], nodes);
                }
            }
        }
        for node in v["nodes"].as_array_mut().expect("nodes") {
            Rewrite::id(&mut node["nature"], natures);
        }
    }

    /// `ir` after `edit` of its serialized form.
    fn of(ir: &Ir, edit: impl FnOnce(&mut Value)) -> Ir {
        let mut v = serde_json::to_value(ir).expect("serialize");
        edit(&mut v);
        serde_json::from_value(v).expect("still an Ir")
    }
}

#[test]
fn printed_text_shows_every_item() {
    for case in Cases::all() {
        let text = case.ir.to_string();
        Cases::golden(case.name, false, &text);
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
    for (path, text) in SourceTree::files() {
        for needle in ["FromStr", "fn parse", "fn from_text", "fn from_str"] {
            assert!(!text.contains(needle), "{path} holds `{needle}`");
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

#[test]
fn structural_text_is_the_tree_with_types() {
    for case in Cases::all() {
        let text = case.ir.print_structural();
        Cases::golden(case.name, true, &text);
        // No origin, no arena: the tree alone.
        assert!(!text.contains('@'), "{}: an origin in {text}", case.name);
        assert!(!text.contains("arenas"), "{}: the arena section in {text}", case.name);
        assert!(!text.lines().any(|l| l.trim_start().starts_with('e') && l.contains(" = ")), "{}: an arena line", case.name);
        // Every body, function, instance and node, by name.
        for body in case.ir.bodies() {
            assert!(text.lines().any(|l| l == format!("body {}", body.name())), "{}: body {}", case.name, body.name());
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
        assert_eq!(text, case.ir.print_structural(), "{}: printed twice", case.name);
    }
    // The diode's statements, as `devices.msl` reads them, with no type written: every one is
    // what its form implies.
    let diode = Cases::all().into_iter().find(|c| c.name == "diode").expect("diode").ir.print_structural();
    for line in [
        "    let nvt = (n * vt())",
        "    let vcrit = (nvt * ln((nvt / (sqrt(2.0) * is))))",
        "    let vd = limit L0 (probe(a, k), |new, old| pnjlim(new, old, nvt, vcrit), seed vcrit)",
        "    let id = (is * (exp((vd / nvt)) - 1.0))",
        "    b0.flow +> direct (id + (gmin() * vd)), ddt (cj0 * vd)",
    ] {
        assert!(diode.lines().any(|l| l == line), "missing {line:?} in {diode}");
    }
    assert!(!diode.contains(" : "), "{diode}");
}

#[test]
fn structural_text_ignores_origins_and_arena_order() {
    for case in Cases::all() {
        let structural = case.ir.print_structural();
        // Every origin moved: the IR is unequal, its full text differs, its structure does not.
        let moved = Rewrite::of(&case.ir, Rewrite::origins);
        assert_ne!(moved, case.ir, "{}", case.name);
        assert_ne!(moved.to_string(), case.ir.to_string(), "{}", case.name);
        assert_eq!(moved.print_structural(), structural, "{}: origins", case.name);
        // Every arena but the instance table reversed: the same design built in another order.
        let reordered = Rewrite::of(&case.ir, Rewrite::arenas);
        assert_eq!(reordered.check(), Vec::new(), "{}: the reordered IR is well formed", case.name);
        assert_eq!(reordered.print_structural(), structural, "{}: arena order", case.name);
        // Both at once.
        let both = Rewrite::of(&moved, Rewrite::arenas);
        assert_eq!(both.print_structural(), structural, "{}: origins and arena order", case.name);
    }
    // The reversal does reorder: the divider's bodies and expressions sit elsewhere.
    let divider = Cases::all().pop().expect("divider").ir;
    let reordered = Rewrite::of(&divider, Rewrite::arenas);
    assert_ne!(reordered, divider);
    assert_ne!(reordered.to_string(), divider.to_string());
    assert_ne!(reordered.bodies().next().map(|b| b.name().to_string()), divider.bodies().next().map(|b| b.name().to_string()));

    // A shared expression is numbered by the tree, not the arena: `acc + acc` twice.
    let shared = Shapes::ir(|e| {
        let mut acc = e.probe(Terminal::Node(0), Terminal::Node(1));
        for _ in 0..2 {
            acc = e.binary(BinaryOp::Add, acc, acc);
        }
        acc
    });
    let text = shared.print_structural();
    assert!(text.lines().any(|l| l == "    b0.flow +> direct ((probe(p, n) + ^2) + ^1)"), "{text}");
    assert_eq!(Rewrite::of(&shared, Rewrite::arenas).print_structural(), text);
}

#[test]
fn structural_text_shows_forms_and_types() {
    let base = Cases::resistor();
    let text = base.print_structural();
    let edits: Vec<(&str, Box<dyn Fn(&mut Value)>, &str)> = vec![
        ("an expression type", Box::new(|v: &mut Value| {
            let probe = Cases::first(v, "Probe");
            v["exprs"][probe]["ty"] = json!("Bool");
        }), "(probe(p, n) : bool)"),
        ("an operator", Box::new(|v: &mut Value| {
            let binary = Cases::first(v, "Binary");
            v["exprs"][binary]["kind"]["Binary"]["op"] = json!("Lt");
        }), "(r < 0.0)"),
        ("a constant", Box::new(|v: &mut Value| {
            let real = Cases::first(v, "Real");
            v["exprs"][real]["kind"]["Real"] = json!(1.5_f64.to_bits());
        }), "(r > 1.5)"),
        ("a terminal", Box::new(|v: &mut Value| {
            let probe = Cases::first(v, "Probe");
            v["exprs"][probe]["kind"]["Probe"]["minus"] = json!("Ground");
        }), "probe(p, gnd)"),
        ("a bound value", Box::new(|v: &mut Value| v["instances"][0]["params"][0] = json!({"Real": 2000.0_f64.to_bits()})), "(r = 2000.0)"),
        ("a local's type", Box::new(|v: &mut Value| {
            v["bodies"][0]["locals"] = json!([{"name": "x", "ty": "Bool"}]);
        }), "local x: bool"),
    ];
    let mut texts = vec![text];
    for (what, edit, shows) in &edits {
        let ir = Cases::edited(|v| edit(v));
        let edited = ir.print_structural();
        assert!(edited.contains(shows), "{what}: {shows:?} not in {edited}");
        assert!(!texts.contains(&edited), "{what}: prints like an earlier IR");
        texts.push(edited);
    }
}

#[test]
fn display_stays_the_full_form() {
    for case in Cases::all() {
        let full = case.ir.to_string();
        assert_eq!(full, format!("{}", case.ir), "{}", case.name);
        assert!(full.contains("\narenas\n"), "{}: the arena section", case.name);
        assert!(full.contains("  @"), "{}: origins", case.name);
        assert_ne!(full, case.ir.print_structural(), "{}", case.name);
    }
}

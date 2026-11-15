//! Scope: `Ir::check` (IR-06). Each test corrupts the JSON of a serialized fixture and reads the
//! entry that names the broken item and its origin: ids out of range, wrong instance arity,
//! misplaced or duplicated `limit`/`ac_stim` sites and contributions, wrong types in analog
//! positions, duplicated paths. A built IR checks clean; a lone top instance prints and checks.

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::{Devices, Ex};
use piperine_ir::{
    Addend, BinaryOp, Broken, BodySpec, Branch, BranchKind, ExprId, FunctionId, FunctionSpec, Ir,
    IrBuilder, Local, NodeRole, Operator, Origin, QuantityRole, Stmt, StmtId, Terminal, ValueType,
};
use serde_json::{Value, json};

/// The IR every test starts from: the five devices, `Everything`, and the divider.
struct Corpus;

impl Corpus {
    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        Devices::everything(&mut b);
        Devices::capacitor(&mut b);
        Devices::isource(&mut b);
        Devices::diode(&mut b);
        let top = Devices::divider(&mut b);
        b.finish(top)
    }

    fn value() -> Value {
        serde_json::to_value(Corpus::ir()).expect("serialize")
    }
}

/// A two-port body `Shape(p, n)` alone as the design, contributing one direct addend to the
/// flow of `(p, n)`; `value` returns the statements before the contribution and the addend.
struct Shape;

impl Shape {
    const FILE: &'static str = "shape.msl";

    fn origin() -> Origin {
        Origin { file: Self::FILE.into(), line: 1, col: 1 }
    }

    fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 2, 3)
    }

    fn ir(locals: usize, value: impl FnOnce(&mut IrBuilder) -> (Vec<StmtId>, ExprId)) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let (mut stmts, value) = value(&mut b);
        stmts.push(b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        }));
        let port = |name: &str| piperine_ir::BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Self::origin() };
        let body = b.body(BodySpec {
            name: "Shape".into(),
            origin: Self::origin(),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: (0..locals).map(|i| Local { name: format!("x{i}"), ty: ValueType::F64 }).collect(),
            analog: Some(stmts),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "Shape", 1, vec![]);
        b.finish(top)
    }

    /// `v(p, n) + 1.0 + ... + 1.0`, `terms` constants, left-associated: nested `terms` deep.
    fn deep_sum(terms: usize) -> Ir {
        Shape::ir(0, |b| {
            let mut e = Shape::ex(b);
            let mut acc = e.probe(Terminal::Node(0), Terminal::Node(1));
            for _ in 0..terms {
                let one = e.real(1.0);
                acc = e.binary(BinaryOp::Add, acc, one);
            }
            (vec![], acc)
        })
    }

    /// `acc = acc + acc`, `doublings` times on one id: 2^doublings paths through 41 expressions.
    fn shared_sum(doublings: usize) -> Ir {
        Shape::ir(0, |b| {
            let mut e = Shape::ex(b);
            let mut acc = e.probe(Terminal::Node(0), Terminal::Node(1));
            for _ in 0..doublings {
                acc = e.binary(BinaryOp::Add, acc, acc);
            }
            (vec![], acc)
        })
    }

    /// A one-parameter function `name` whose body calls `callee` with its parameter.
    fn caller(b: &mut IrBuilder, name: &str, callee: FunctionId) -> FunctionId {
        let mut e = Ex::new(b, Self::FILE, 9, 1);
        let x = e.local(0);
        let body = e.call(callee, vec![x]);
        let x = Local { name: "x".into(), ty: ValueType::F64 };
        b.function(FunctionSpec {
            name: name.into(),
            origin: Origin { file: Self::FILE.into(), line: 9, col: 1 },
            params: vec![x.clone()],
            ret: ValueType::F64,
            locals: vec![x],
            body,
        })
    }
}

/// Edits of the serialized form.
struct Json;

impl Json {
    /// The variant name of an externally tagged enum value.
    fn variant(kind: &Value) -> &str {
        match kind {
            Value::String(s) => s,
            Value::Object(o) => o.keys().next().map_or("", String::as_str),
            _ => "",
        }
    }

    /// Indexes of the expressions of this variant, in arena order.
    fn exprs(ir: &Value, variant: &str) -> Vec<usize> {
        ir["exprs"]
            .as_array()
            .expect("exprs")
            .iter()
            .enumerate()
            .filter(|(_, e)| Json::variant(&e["kind"]) == variant)
            .map(|(i, _)| i)
            .collect()
    }

    fn set_kind(ir: &mut Value, expr: usize, kind: Value) {
        ir["exprs"][expr]["kind"] = kind;
    }

    fn ty(ir: &mut Value, expr: usize, ty: &str) {
        ir["exprs"][expr]["ty"] = json!(ty);
    }

    fn broken(ir: Value) -> Vec<Broken> {
        let ir: Ir = serde_json::from_value(ir).expect("still an Ir");
        ir.check()
    }

    fn body_index(ir: &Value, name: &str) -> usize {
        ir["bodies"].as_array().expect("bodies").iter().position(|b| b["name"] == name).expect("body")
    }

    fn instance_index(ir: &Value, path: &str) -> usize {
        ir["instances"].as_array().expect("instances").iter().position(|b| b["path"] == path).expect("instance")
    }

    /// The id a unary-field variant holds: `{"Variant": {"field": id}}`.
    fn field(ir: &Value, expr: usize, variant: &str, field: &str) -> usize {
        ir["exprs"][expr]["kind"][variant][field].as_u64().expect("an id") as usize
    }
}

/// The entries of `broken` that mention `text` in their item or reason.
struct Entries;

impl Entries {
    fn with<'a>(broken: &'a [Broken], text: &str) -> Vec<&'a Broken> {
        broken.iter().filter(|b| b.item.contains(text) || b.what.contains(text)).collect()
    }
}

#[test]
fn a_built_ir_checks_clean() {
    assert_eq!(Corpus::ir().check(), Vec::<Broken>::new());
    for (make, name, line, params) in [
        (Devices::resistor as fn(&mut IrBuilder) -> _, "resistor", 5, vec![1000.0]),
        (Devices::capacitor, "capacitor", 11, vec![1e-12]),
        (Devices::vsource, "vsource", 17, vec![10.0, 0.0]),
        (Devices::isource, "isource", 24, vec![1e-3, 0.0]),
        (Devices::diode, "diode", 31, vec![1e-14, 1.0, 0.0]),
    ] {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let top = Devices::alone(&mut b, body, name, line, params);
        assert_eq!(b.finish(top).check(), Vec::<Broken>::new(), "{name}");
    }
    // An expression nested deeper than any fixed bound is not a cycle.
    for terms in [300, 10_000] {
        assert_eq!(Shape::deep_sum(terms).check(), Vec::<Broken>::new(), "a sum of {terms} terms");
    }
    // An expression id shared by both operands, 40 times over, is checked once per id.
    let started = std::time::Instant::now();
    assert_eq!(Shape::shared_sum(40).check(), Vec::<Broken>::new());
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
}

#[test]
fn an_out_of_range_id_is_named() {
    // A call to function 99.
    let mut v = Corpus::value();
    let call = Json::exprs(&v, "Call")[0];
    v["exprs"][call]["kind"]["Call"]["function"] = json!(99);
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "function id 99");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, format!("expr {call}"));
    assert_ne!(hits[0].origin.line, 0, "the entry carries the expression's origin");

    // An instance whose body is 77.
    let mut v = Corpus::value();
    let r1 = Json::instance_index(&v, "Divider.r1");
    v["instances"][r1]["body"] = json!(77);
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "body id 77");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "instance Divider.r1");

    // A statement id of a body's analog block.
    let mut v = Corpus::value();
    let resistor = Json::body_index(&v, "Resistor");
    v["bodies"][resistor]["analog"][0] = json!(500);
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "stmt id 500");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "body Resistor");

    // A nature id of a node of the design, and a parent id of an instance.
    let mut v = Corpus::value();
    v["nodes"][0]["nature"] = json!(9);
    v["instances"][r1]["parent"] = json!(40);
    let broken = Json::broken(v);
    assert_eq!(Entries::with(&broken, "nature id 9").len(), 1, "{broken:?}");
    assert!(!Entries::with(&broken, "instance id 40").is_empty(), "{broken:?}");
}

#[test]
fn an_instance_with_wrong_arity_is_named() {
    let mut v = Corpus::value();
    let r1 = Json::instance_index(&v, "Divider.r1");
    v["instances"][r1]["nodes"].as_array_mut().expect("nodes").pop();
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "binds 1 nodes");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "instance Divider.r1");

    let mut v = Corpus::value();
    let r2 = Json::instance_index(&v, "Divider.r2");
    v["instances"][r2]["params"].as_array_mut().expect("params").push(json!({"Bool": true}));
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "binds 2 params");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "instance Divider.r2");
}

#[test]
fn a_misplaced_or_duplicated_site_is_named() {
    // A site used twice: the first real of `Everything` becomes a second `ac_stim` of site 0.
    let mut v = Corpus::value();
    let reals = Json::exprs(&v, "Real");
    Json::set_kind(&mut v, reals[0], json!({"Stim": {"site": 0, "mag": reals[1], "phase": null}}));
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "ac_stim site 0 is used by 2");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "body Everything");

    // A site under an `if`.
    let mut v = Corpus::value();
    let reals = Json::exprs(&v, "Real");
    let iff = Json::exprs(&v, "If")[0];
    let then = Json::field(&v, iff, "If", "then");
    Json::set_kind(&mut v, then, json!({"Stim": {"site": 0, "mag": reals[1], "phase": null}}));
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "under an `if`");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].origin, Origin { file: "all.msl".into(), line: 6, col: 1 });

    // A site in a closure.
    let mut v = Corpus::value();
    let reals = Json::exprs(&v, "Real");
    let closure = Json::exprs(&v, "Closure")[0];
    let body = Json::field(&v, closure, "Closure", "body");
    Json::set_kind(&mut v, body, json!({"Stim": {"site": 0, "mag": reals[1], "phase": null}}));
    let broken = Json::broken(v);
    assert_eq!(Entries::with(&broken, "inside a closure").len(), 1, "{broken:?}");

    // A site in a function.
    let mut v = Corpus::value();
    let reals = Json::exprs(&v, "Real");
    let function = v["functions"].as_array().expect("functions").iter().position(|f| f["name"] == "identity").expect("identity");
    let body = v["functions"][function]["body"].as_u64().expect("body") as usize;
    Json::set_kind(&mut v, body, json!({"Stim": {"site": 0, "mag": reals[1], "phase": null}}));
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "outside the analog block");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "function identity");

    // A contribution under an `if`: the `then` arm becomes a block holding the contribution.
    let mut v = Corpus::value();
    let iff = Json::exprs(&v, "If")[0];
    let then = Json::field(&v, iff, "If", "then");
    // The capacitor's contribution does not contain the `if`, so no cycle is made.
    let capacitor = Json::body_index(&v, "Capacitor");
    let contribution = v["bodies"][capacitor]["analog"][0].as_u64().expect("stmt");
    Json::set_kind(&mut v, then, json!({"Block": {"stmts": [contribution], "value": null}}));
    let broken = Json::broken(v);
    assert_eq!(Entries::with(&broken, "contribution outside the top level").len(), 1, "{broken:?}");
}

#[test]
fn a_wrong_type_in_an_analog_position_is_named() {
    // A probe typed `bool`: both the probe and the operator that reads it are named.
    let mut v = Corpus::value();
    let probe = Json::exprs(&v, "Probe").into_iter().find(|&i| v["exprs"][i]["origin"]["file"] == "devices.msl").expect("a device probe");
    let line = v["exprs"][probe]["origin"]["line"].as_u64().expect("line");
    Json::ty(&mut v, probe, "Bool");
    let broken = Json::broken(v);
    let hits: Vec<&Broken> = broken.iter().filter(|b| b.origin.line as u64 == line && b.origin.file == "devices.msl").collect();
    assert!(!hits.is_empty(), "{broken:?}");
    assert!(hits.iter().any(|b| b.what.contains("must be f64, found bool")), "{hits:?}");

    // An addend that is not `f64`: the potential contribution of `Vsource`.
    let mut v = Corpus::value();
    let vsource = Json::body_index(&v, "Vsource");
    let stmt = v["bodies"][vsource]["analog"][0].as_u64().expect("stmt") as usize;
    let value = v["stmts"][stmt]["Contribute"]["addends"][0]["value"].as_u64().expect("value") as usize;
    Json::ty(&mut v, value, "Bool");
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "must be f64, found bool");
    assert!(hits.iter().any(|b| b.item == "body Vsource" && b.origin.file == "devices.msl" && b.origin.line == 21), "{broken:?}");

    // The input of a `limit` that is not `f64`.
    let mut v = Corpus::value();
    let limit = Json::exprs(&v, "Limit").into_iter().find(|&i| v["exprs"][i]["origin"]["file"] == "devices.msl").expect("the diode limit");
    let x = Json::field(&v, limit, "Limit", "x");
    Json::ty(&mut v, x, "Bool");
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "input of a `limit`");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].origin, Origin { file: "devices.msl".into(), line: 39, col: 28 });
}

#[test]
fn a_top_alone_prints_and_checks() {
    let mut b = IrBuilder::new();
    let body = b.body(BodySpec {
        name: "Top".into(),
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
        params: vec![],
        nodes: vec![],
        branches: vec![],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: None,
        children: vec![],
    });
    let top = b.instance(piperine_ir::InstanceSpec {
        path: "Top".into(),
        name: "Top".into(),
        body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
    });
    let ir = b.finish(top);
    assert!(ir.to_string().contains("instance Top: Top"));
    assert_eq!(ir.check(), Vec::<Broken>::new());
}

#[test]
fn a_duplicated_path_is_named() {
    let mut v = Corpus::value();
    let r2 = Json::instance_index(&v, "Divider.r2");
    v["instances"][r2]["path"] = json!("Divider.r1");
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "2 instances share the path Divider.r1");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "instance Divider.r1");

    let mut v = Corpus::value();
    v["nodes"][1]["path"] = json!("Divider.vin");
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "2 nodes share the path Divider.vin");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "node Divider.vin");
}

#[test]
fn a_cyclic_expression_is_named_without_hanging() {
    // An operator that reads itself on both sides: 2^256 paths unless the walk stops at the first.
    let mut v = Corpus::value();
    let binary = Json::exprs(&v, "Binary")[0];
    v["exprs"][binary]["kind"]["Binary"]["lhs"] = json!(binary);
    v["exprs"][binary]["kind"]["Binary"]["rhs"] = json!(binary);
    let broken = Json::broken(v);
    let hits = Entries::with(&broken, "a cycle");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert!(hits[0].what.contains(&format!("expr {binary}")), "{hits:?}");
}

#[test]
fn a_recursive_call_or_an_unbound_local_is_named() {
    // `fn f(x) = f(x)`, called from the addend: `f` is function 0 and calls itself.
    let ir = Shape::ir(0, |b| {
        let f = Shape::caller(b, "f", FunctionId(0));
        let mut e = Shape::ex(b);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        (vec![], e.call(f, vec![v]))
    });
    let broken = ir.check();
    let hits = Entries::with(&broken, "calls itself");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "function f");
    assert_eq!(hits[0].origin, Origin { file: "shape.msl".into(), line: 9, col: 1 });

    // `g` calls `h`, `h` calls `g`: both are named.
    let ir = Shape::ir(0, |b| {
        let g = Shape::caller(b, "g", FunctionId(1));
        Shape::caller(b, "h", FunctionId(0));
        let mut e = Shape::ex(b);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        (vec![], e.call(g, vec![v]))
    });
    let broken = ir.check();
    let items: Vec<&str> = Entries::with(&broken, "calls itself").iter().map(|b| b.item.as_str()).collect();
    assert_eq!(items, ["function g", "function h"], "{broken:?}");

    // A statement reads local 0, whose `let` comes after it.
    let ir = Shape::ir(1, |b| {
        let mut e = Shape::ex(b);
        let x = e.local(0);
        e.at(5, 7);
        let one = e.real(1.0);
        let addend = e.real(0.0);
        let read = b.stmt(Stmt::Expr(x));
        let late = b.stmt(Stmt::Let { local: 0, init: one });
        (vec![read, late], addend)
    });
    let broken = ir.check();
    let hits = Entries::with(&broken, "read before its `let`");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "body Shape");
    assert_eq!(hits[0].origin, Origin { file: "shape.msl".into(), line: 2, col: 3 });

    // A `let` that follows its read in the same block is still late; one before it is not.
    let ir = Shape::ir(1, |b| {
        let mut e = Shape::ex(b);
        let one = e.real(1.0);
        let x = e.local(0);
        let early = b.stmt(Stmt::Let { local: 0, init: one });
        (vec![early], x)
    });
    assert_eq!(ir.check(), Vec::<Broken>::new());
}

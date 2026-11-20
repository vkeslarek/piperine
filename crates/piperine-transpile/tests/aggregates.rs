//! Scope: the printer's aggregate values (composite-values CV-12, CV-13, CV-17, CV-20): the type
//! arena as Rust items (`TypeItems`), aggregate expressions and places, and the `Carried` of a
//! derived type. The text is compiled by `rustc` with the runtime and run; each result is
//! compared with the same Rust written in the test.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/alone.rs"]
mod alone;
#[allow(dead_code)]
#[path = "support/probe.rs"]
mod probe;

use alone::Alone;
use digital::{Dx, Ty};
use piperine_ir::{BinaryOp, Derives, Expr, ExprId, FieldsForm, FunctionSpec, Ir, IrBuilder, LevelPart, Local, Origin, Place, Projection, Stmt, StmtId, TypeData, TypeId, TypeKind, ValueType, VariantData};
use piperine_transpile::print::{TypeItems, ValuePrinter};
use probe::Probe;

const FILE: &str = "aggregates.msl";

/// The types and the functions of this file, built by hand from the MSL each one transcribes.
struct Fixtures;

impl Fixtures {
    fn origin(line: u32) -> Origin {
        Origin { file: FILE.into(), line, col: 1 }
    }

    /// A struct or enum entry of the arena at `line`.
    fn entry(b: &mut IrBuilder, name: &str, line: u32, kind: TypeKind, derives: Derives) -> TypeId {
        b.type_data(TypeData { name: name.into(), path: format!("m::{name}"), origin: Self::origin(line), kind, derives })
    }

    fn variant(name: &str, discriminant: i128, form: FieldsForm, fields: &[(&str, ValueType)]) -> VariantData {
        VariantData { name: name.into(), discriminant, form, fields: fields.iter().map(|(n, ty)| (n.to_string(), ty.clone())).collect() }
    }

    /// The five types of the type-printing test:
    /// `#[derive(Clone, Copy, PartialEq, PartialOrd, Default)] struct Pair { a: f64, b: bool }`,
    /// `struct Wrap(u8, Pair)`, `struct Marker`, `enum Tx { Idle, Start, Data { n: u3 }, Stop }` and
    /// `#[derive(Clone, Copy, PartialEq, Eq)] enum Level { Low = 0, High = 1 }`; and
    /// `fn pass(p: Pair) -> Pair { p }`.
    fn design() -> Ir {
        let mut b = IrBuilder::new();
        let every = Derives { clone: true, copy: true, partial_eq: true, partial_ord: true, default: true, ..Derives::default() };
        let pair_fields = vec![("a".to_string(), ValueType::F64), ("b".to_string(), ValueType::Bool)];
        let pair = Self::entry(&mut b, "Pair", 1, TypeKind::Struct { form: FieldsForm::Named, fields: pair_fields }, every);
        let wrap_fields = vec![("0".to_string(), ValueType::UInt(8)), ("1".to_string(), ValueType::Struct(pair))];
        Self::entry(&mut b, "Wrap", 5, TypeKind::Struct { form: FieldsForm::Tuple, fields: wrap_fields }, Derives::default());
        Self::entry(&mut b, "Marker", 7, TypeKind::Struct { form: FieldsForm::Unit, fields: vec![] }, Derives::default());
        let tx = vec![
            Self::variant("Idle", 0, FieldsForm::Unit, &[]),
            Self::variant("Start", 1, FieldsForm::Unit, &[]),
            Self::variant("Data", 2, FieldsForm::Named, &[("n", ValueType::UInt(3))]),
            Self::variant("Stop", 3, FieldsForm::Unit, &[]),
        ];
        Self::entry(&mut b, "Tx", 9, TypeKind::Enum { tag_levels: 2, variants: tx }, Derives { clone: true, copy: true, ..Derives::default() });
        let level = vec![Self::variant("Low", 0, FieldsForm::Unit, &[]), Self::variant("High", 1, FieldsForm::Unit, &[])];
        Self::entry(&mut b, "Level", 14, TypeKind::Enum { tag_levels: 1, variants: level }, Derives { clone: true, copy: true, partial_eq: true, eq: true, ..Derives::default() });
        let param = Local { name: "p".into(), ty: ValueType::Struct(pair), mutable: false };
        let mut x = Dx::new(&mut b, FILE, 20, 5);
        let body: ExprId = x.local(0, ValueType::Struct(pair));
        b.function(FunctionSpec { name: "pass".into(), origin: Self::origin(20), params: vec![param.clone()], ret: ValueType::Struct(pair), locals: vec![param], body });
        Alone::finish(b)
    }

    /// The type items of `ir` and the text of each of its functions: what a program is built from.
    fn texts(ir: &Ir) -> Vec<String> {
        let mut texts = TypeItems::new(ir).items().unwrap_or_else(|e| panic!("{e}"));
        texts.extend(ir.functions().map(|f| ValuePrinter::function(ir, f.id()).unwrap_or_else(|e| panic!("{e}"))));
        texts
    }
}

#[test]
fn each_source_type_prints_as_one_rust_item_naming_its_path() {
    // CV-20 AC 4: a struct, a tuple struct, a unit struct and two enums each print as one Rust item
    // under a doc comment naming the MSL path and the origin; the type that holds a real is generic
    // over `R`, the ones that do not are plain; the derives are the source's.
    let ir = Fixtures::design();
    let texts = Fixtures::texts(&ir);
    let text = texts.join("\n");
    for line in [
        "/// MSL `m::Pair`, a struct, written at aggregates.msl:1:1.",
        "#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]",
        "pub struct Pair<R> {",
        "    pub a: R,",
        "    pub b: bool,",
        "/// MSL `m::Wrap`, a struct, written at aggregates.msl:5:1.",
        "pub struct Wrap<R>(pub rt::UInt<8>, pub Pair<R>);",
        "/// MSL `m::Marker`, a struct, written at aggregates.msl:7:1.",
        "pub struct Marker;",
        "/// MSL `m::Tx`, an enum, written at aggregates.msl:9:1.",
        "pub enum Tx {",
        "    Data { n: rt::UInt<3> },",
        "#[derive(Clone, Copy, PartialEq, Eq)]",
        "pub enum Level {",
        "    High = 1,",
        "fn pass<R: Real>(p: Pair<R>) -> Pair<R> {",
    ] {
        assert!(text.contains(line), "the text holds `{line}`:\n{text}");
    }
    assert_eq!(texts.iter().filter(|t| t.contains("pub struct") || t.contains("pub enum")).count(), 5, "one item per arena entry");

    // the items compile with the runtime and behave as the Rust they are: a derive, a field, a
    // tuple field, a variant with a field, a discriminant
    let main = "\
fn main() {
    let p = pass(Pair::<f64> { a: 1.5, b: true });
    println!(\"pass {} {}\", p.a, p.b);
    println!(\"default {} {}\", Pair::<f64>::default().a, Pair::<f64>::default().b);
    let w = Wrap::<f64>(rt::UInt::<8>::new(7), p);
    println!(\"wrap {} {}\", w.0.get(), w.1.a);
    let t = Tx::Data { n: rt::UInt::<3>::new(5) };
    match t {
        Tx::Data { n } => println!(\"data {}\", n.get()),
        _ => println!(\"other\"),
    }
    let _ = (Tx::Idle, Tx::Start, Tx::Stop, Marker);
    println!(\"level {} {}\", Level::High as i32, Level::Low == Level::High);
    println!(\"order {}\", Pair::<f64> { a: 1.0, b: true } < Pair::<f64> { a: 2.0, b: false });
}
";
    let out = Probe::run(&texts, main);
    let expected = ["pass 1.5 true", "default 0 false", "wrap 7 1.5", "data 5", "level 1 false", "order true"];
    assert_eq!(out.lines().collect::<Vec<_>>(), expected);
}

// ---- T35: aggregate expressions and indexed places (CV-13, CV-20) -------------------------------------------------

/// The functions of the expression test, built by hand from the MSL each one transcribes.
struct Library;

impl Library {
    fn local(name: &str, ty: ValueType, mutable: bool) -> Local {
        Local { name: name.into(), ty, mutable }
    }

    /// An aggregate form at `line`, of type `ty`.
    fn e(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        b.expr(kind, ty, Fixtures::origin(line))
    }

    /// `fn name(params) -> ret { body }`; `locals` start with the params.
    fn define(b: &mut IrBuilder, name: &str, line: u32, locals: Vec<Local>, params: usize, ret: ValueType, body: ExprId) {
        b.function(FunctionSpec { name: name.into(), origin: Fixtures::origin(line), params: locals[..params].to_vec(), ret, locals, body });
    }

    /// `place = value;`
    fn store(b: &mut IrBuilder, local: u32, path: Vec<Projection>, value: ExprId) -> StmtId {
        b.stmt(Stmt::Assign { place: Place { local, path }, op: None, value })
    }

    /// The design: types and functions, each function with the MSL it transcribes above it.
    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        let (u8t, boolean) = (Ty::u(8), ValueType::Bool);
        let flagged_u8 = Ty::flagged(u8t.clone());
        let pair_fields = vec![("a".to_string(), ValueType::F64), ("b".to_string(), ValueType::Bool)];
        let pair = Fixtures::entry(&mut b, "Pair", 1, TypeKind::Struct { form: FieldsForm::Named, fields: pair_fields }, Derives { clone: true, copy: true, default: true, ..Derives::default() });
        let level = vec![Fixtures::variant("Low", 0, FieldsForm::Unit, &[]), Fixtures::variant("High", 1, FieldsForm::Unit, &[])];
        let level = Fixtures::entry(&mut b, "Level", 3, TypeKind::Enum { tag_levels: 1, variants: level }, Derives { clone: true, copy: true, partial_eq: true, eq: true, partial_ord: true, ord: true, ..Derives::default() });
        let tx = vec![Fixtures::variant("Idle", 0, FieldsForm::Unit, &[]), Fixtures::variant("Data", 1, FieldsForm::Named, &[("n", Ty::u(3))])];
        let tx = Fixtures::entry(&mut b, "Tx", 5, TypeKind::Enum { tag_levels: 1, variants: tx }, Derives { clone: true, copy: true, ..Derives::default() });
        let (pair_t, level_t, tx_t) = (ValueType::Struct(pair), ValueType::Enum(level), ValueType::Enum(tx));
        let array3 = ValueType::Array(Box::new(u8t.clone()), 3);
        let array4 = ValueType::Array(Box::new(u8t.clone()), 4);
        let flagged3 = ValueType::Array(Box::new(flagged_u8.clone()), 3);
        let tuple = ValueType::Tuple(vec![u8t.clone(), boolean.clone()]);
        let option_real = ValueType::Option(Box::new(ValueType::F64));

        // aggregates.msl:10  fn pick(a: [u8; 3], i: u8) -> u8 { a[i] }
        let (a, i) = (Library::e(&mut b, Expr::Local(0), array3.clone(), 10), Library::e(&mut b, Expr::Local(1), u8t.clone(), 10));
        let body = Library::e(&mut b, Expr::Index { expr: a, index: i }, u8t.clone(), 10);
        Library::define(&mut b, "pick", 10, vec![Library::local("a", array3.clone(), false), Library::local("i", u8t.clone(), false)], 2, u8t.clone(), body);

        // aggregates.msl:12  fn fill() -> [u8; 3] { let mut a = [0; 3]; a[1] = 5; a[2] = 7; a }
        let zero = Dx::new(&mut b, FILE, 13, 5).int(0, u8t.clone());
        let repeated = Library::e(&mut b, Expr::ArrayRepeat { element: zero, count: 3 }, array3.clone(), 13);
        let declare = Dx::new(&mut b, FILE, 13, 5).let_(0, repeated);
        let (one_i, two_i) = (Dx::new(&mut b, FILE, 14, 5).int(1, u8t.clone()), Dx::new(&mut b, FILE, 15, 5).int(2, u8t.clone()));
        let (five, seven) = (Dx::new(&mut b, FILE, 14, 12).int(5, u8t.clone()), Dx::new(&mut b, FILE, 15, 12).int(7, u8t.clone()));
        let (first, second) = (Library::store(&mut b, 0, vec![Projection::Index(one_i)], five), Library::store(&mut b, 0, vec![Projection::Index(two_i)], seven));
        let result = Dx::new(&mut b, FILE, 16, 5).local(0, array3.clone());
        let body = Dx::new(&mut b, FILE, 16, 5).block(vec![declare, first, second], Some(result), array3.clone());
        Library::define(&mut b, "fill", 12, vec![Library::local("a", array3.clone(), true)], 0, array3.clone(), body);

        // aggregates.msl:20  fn bump(p: Pair) -> Pair { let mut q = p; q.a = q.a + 1.0; q.b = !q.b; q }
        let p = Dx::new(&mut b, FILE, 21, 5).local(0, pair_t.clone());
        let declare = Dx::new(&mut b, FILE, 21, 5).let_(1, p);
        let q = Library::e(&mut b, Expr::Local(1), pair_t.clone(), 22);
        let q_a = Library::e(&mut b, Expr::Field { expr: q, index: 0 }, ValueType::F64, 22);
        let one = Dx::new(&mut b, FILE, 22, 15).real(1.0);
        let sum = Dx::new(&mut b, FILE, 22, 10).bin(BinaryOp::Add, q_a, one, ValueType::F64);
        let set_a = Library::store(&mut b, 1, vec![Projection::Field(0)], sum);
        let q = Library::e(&mut b, Expr::Local(1), pair_t.clone(), 23);
        let q_b = Library::e(&mut b, Expr::Field { expr: q, index: 1 }, boolean.clone(), 23);
        let flipped = Dx::new(&mut b, FILE, 23, 10).un(piperine_ir::UnaryOp::Not, q_b, boolean.clone());
        let set_b = Library::store(&mut b, 1, vec![Projection::Field(1)], flipped);
        let result = Dx::new(&mut b, FILE, 24, 5).local(1, pair_t.clone());
        let body = Dx::new(&mut b, FILE, 24, 5).block(vec![declare, set_a, set_b], Some(result), pair_t.clone());
        Library::define(&mut b, "bump", 20, vec![Library::local("p", pair_t.clone(), false), Library::local("q", pair_t.clone(), true)], 1, pair_t.clone(), body);

        // aggregates.msl:30  fn span(a: [u8; 4], lo: u8) -> u64 { a[lo..].len() }
        let (a, lo) = (Library::e(&mut b, Expr::Local(0), array4.clone(), 30), Library::e(&mut b, Expr::Local(1), u8t.clone(), 30));
        let view = Library::e(&mut b, Expr::SliceOf { expr: a, low: Some(lo), high: None, inclusive: false }, ValueType::Slice(Box::new(u8t.clone())), 30);
        let body = Library::e(&mut b, Expr::Length(view), Ty::u(64), 30);
        Library::define(&mut b, "span", 30, vec![Library::local("a", array4.clone(), false), Library::local("lo", u8t.clone(), false)], 2, Ty::u(64), body);

        // aggregates.msl:32  fn payload(t: Tx) -> u8 { if matches!(t, Tx::Data) { t.n as u8 } else { 255 } }
        let t = Library::e(&mut b, Expr::Local(0), tx_t.clone(), 32);
        let test = Library::e(&mut b, Expr::IsVariant { expr: t, variant: 1 }, boolean.clone(), 32);
        let n = Library::e(&mut b, Expr::VariantField { expr: t, variant: 1, field: 0 }, Ty::u(3), 32);
        let widened = Dx::new(&mut b, FILE, 32, 30).cast(n, u8t.clone(), u8t.clone());
        let none = Dx::new(&mut b, FILE, 32, 40).int(255, u8t.clone());
        let body = Dx::new(&mut b, FILE, 32, 5).if_(test, widened, none, u8t.clone());
        Library::define(&mut b, "payload", 32, vec![Library::local("t", tx_t.clone(), false)], 1, u8t.clone(), body);

        // aggregates.msl:40  fn maybe(x: f64, ok: bool) -> Option<f64> { option(ok, x) }
        let (x, ok) = (Dx::new(&mut b, FILE, 40, 5).local(0, ValueType::F64), Dx::new(&mut b, FILE, 40, 5).local(1, boolean.clone()));
        let body = Library::e(&mut b, Expr::OptionPack { is: ok, value: x }, option_real.clone(), 40);
        Library::define(&mut b, "maybe", 40, vec![Library::local("x", ValueType::F64, false), Library::local("ok", boolean.clone(), false)], 2, option_real.clone(), body);
        // aggregates.msl:41  fn has(o: Option<f64>) -> bool { o.is } and fn got(o: Option<f64>) -> f64 { o.value }
        let o = Library::e(&mut b, Expr::Local(0), option_real.clone(), 41);
        let body = Library::e(&mut b, Expr::OptionPart { option: o, part: LevelPart::Is }, boolean.clone(), 41);
        Library::define(&mut b, "has", 41, vec![Library::local("o", option_real.clone(), false)], 1, boolean.clone(), body);
        let o = Library::e(&mut b, Expr::Local(0), option_real.clone(), 42);
        let body = Library::e(&mut b, Expr::OptionPart { option: o, part: LevelPart::Value }, ValueType::F64, 42);
        Library::define(&mut b, "got", 42, vec![Library::local("o", option_real.clone(), false)], 1, ValueType::F64, body);

        // aggregates.msl:50  fn below(a: Level, b: Level) -> bool { a < b } and fn same(a: (u8, bool), b: (u8, bool)) -> bool { a == b }
        let (l, r) = (Library::e(&mut b, Expr::Local(0), level_t.clone(), 50), Library::e(&mut b, Expr::Local(1), level_t.clone(), 50));
        let body = Dx::new(&mut b, FILE, 50, 5).bin(BinaryOp::Lt, l, r, boolean.clone());
        Library::define(&mut b, "below", 50, vec![Library::local("a", level_t.clone(), false), Library::local("b", level_t.clone(), false)], 2, boolean.clone(), body);
        let (l, r) = (Library::e(&mut b, Expr::Local(0), tuple.clone(), 51), Library::e(&mut b, Expr::Local(1), tuple.clone(), 51));
        let body = Dx::new(&mut b, FILE, 51, 5).bin(BinaryOp::Eq, l, r, boolean.clone());
        Library::define(&mut b, "same", 51, vec![Library::local("a", tuple.clone(), false), Library::local("b", tuple.clone(), false)], 2, boolean.clone(), body);

        // aggregates.msl:60  fn zero() -> Pair { Pair::default() }  fn make(a: f64) -> Pair { Pair { a, b: true } }
        let body = Library::e(&mut b, Expr::Default(pair_t.clone()), pair_t.clone(), 60);
        Library::define(&mut b, "zero", 60, vec![], 0, pair_t.clone(), body);
        let (a, yes) = (Dx::new(&mut b, FILE, 61, 5).local(0, ValueType::F64), Dx::new(&mut b, FILE, 61, 5).boolean(true));
        let body = Library::e(&mut b, Expr::StructOf { ty: pair, fields: vec![a, yes] }, pair_t.clone(), 61);
        Library::define(&mut b, "make", 61, vec![Library::local("a", ValueType::F64, false)], 1, pair_t.clone(), body);

        // aggregates.msl:70  fn pairup(x: u8) -> (u8, bool) { (x, true) }  fn first(t: (u8, bool)) -> u8 { t.0 }
        let (x, yes) = (Dx::new(&mut b, FILE, 70, 5).local(0, u8t.clone()), Dx::new(&mut b, FILE, 70, 5).boolean(true));
        let body = Library::e(&mut b, Expr::Tuple(vec![x, yes]), tuple.clone(), 70);
        Library::define(&mut b, "pairup", 70, vec![Library::local("x", u8t.clone(), false)], 1, tuple.clone(), body);
        let t = Library::e(&mut b, Expr::Local(0), tuple.clone(), 71);
        let body = Library::e(&mut b, Expr::Field { expr: t, index: 0 }, u8t.clone(), 71);
        Library::define(&mut b, "first", 71, vec![Library::local("t", tuple.clone(), false)], 1, u8t.clone(), body);

        // aggregates.msl:80  fn pick_any(a: [Bit<u8>; 3], i: Bit<u8>) -> Bit<u8> { a[i] }
        let (a, i) = (Library::e(&mut b, Expr::Local(0), flagged3.clone(), 80), Library::e(&mut b, Expr::Local(1), flagged_u8.clone(), 80));
        let body = Library::e(&mut b, Expr::Index { expr: a, index: i }, flagged_u8.clone(), 80);
        Library::define(&mut b, "pick_any", 80, vec![Library::local("a", flagged3.clone(), false), Library::local("i", flagged_u8.clone(), false)], 2, flagged_u8.clone(), body);
        // aggregates.msl:82  fn store_any(a: [Bit<u8>; 3], i: Bit<u8>, v: Bit<u8>) -> [Bit<u8>; 3] { let mut c = a; c[i] = v; c }
        let a = Dx::new(&mut b, FILE, 83, 5).local(0, flagged3.clone());
        let declare = Dx::new(&mut b, FILE, 83, 5).let_(3, a);
        let (i, v) = (Library::e(&mut b, Expr::Local(1), flagged_u8.clone(), 84), Library::e(&mut b, Expr::Local(2), flagged_u8.clone(), 84));
        let set = Library::store(&mut b, 3, vec![Projection::Index(i)], v);
        let result = Dx::new(&mut b, FILE, 85, 5).local(3, flagged3.clone());
        let body = Dx::new(&mut b, FILE, 85, 5).block(vec![declare, set], Some(result), flagged3.clone());
        let locals = vec![Library::local("a", flagged3.clone(), false), Library::local("i", flagged_u8.clone(), false), Library::local("v", flagged_u8.clone(), false), Library::local("c", flagged3.clone(), true)];
        Library::define(&mut b, "store_any", 82, locals, 3, flagged3.clone(), body);
        Alone::finish(b)
    }
}

/// `pick` on Rust's array: the same index, checked.
fn pick(a: [u8; 3], i: usize) -> u8 {
    a[i]
}

#[test]
fn aggregate_expressions_compile_and_run() {
    // CV-13, CV-20: construction, field and element reads and writes, a slice's length, a variant
    // test and field, an option from its pair and back, `default`, comparison of aggregates the
    // derives allow, and an index that may be unknown; each is compared with the same Rust.
    let ir = Library::ir();
    let mut texts = TypeItems::new(&ir).items().unwrap_or_else(|e| panic!("{e}"));
    texts.extend(ir.functions().map(|f| ValuePrinter::function(&ir, f.id()).unwrap_or_else(|e| panic!("{e}"))));
    let text = texts.join("\n");
    for line in [
        "rt::Index::at(&a, i.get() as i128, \"aggregates.msl:10:1\")",
        "q.a = q.a + R::constant(1.0);",
        "a[rt::Index::of(rt::UInt::<8>::new(1).get() as i128, a.len(), \"aggregates.msl:14:12\")] = rt::UInt::<8>::new(5);",
        "rt::Index::slice(&a, Some(lo.get() as i128), None, false, \"aggregates.msl:30:1\")",
        "matches!(t, Tx::Data { .. })",
        "rt::Index::read(&a, ",
        "rt::Index::write(&mut c, ",
    ] {
        assert!(text.contains(line), "the text holds `{line}`:\n{text}");
    }

    let u = |v: u64| format!("rt::UInt::<8>::new({v})");
    let main = format!(
        "\
fn flag(v: Option<u64>) -> String {{
    match v {{ Some(v) => v.to_string(), None => \"?\".to_string() }}
}}
fn show(v: rt::Flagged<rt::UInt<8>>) -> String {{
    flag(if v.is_known() {{ Some(v.known_or(Default::default()).get()) }} else {{ None }})
}}
fn main() {{
    let a = [{u10}, {u20}, {u30}];
    println!(\"pick {{}} {{}} {{}}\", pick(a, {u0}).get(), pick(a, {u1}).get(), pick(a, {u2}).get());
    let f = fill();
    println!(\"fill {{}} {{}} {{}}\", f[0].get(), f[1].get(), f[2].get());
    let q = bump(Pair::<f64> {{ a: 1.5, b: true }});
    println!(\"bump {{}} {{}}\", q.a, q.b);
    let w = [{u1}, {u2}, {u3}, {u4}];
    println!(\"span {{}} {{}}\", span(w, {u1}).get(), span(w, {u4}).get());
    println!(\"payload {{}} {{}}\", payload(Tx::Data {{ n: rt::UInt::<3>::new(6) }}).get(), payload(Tx::Idle).get());
    println!(\"option {{}} {{}} {{}} {{}}\", has(maybe(2.5, true)), has(maybe(2.5, false)), got(maybe(2.5, true)), got(maybe(2.5, false)));
    println!(\"compare {{}} {{}} {{}} {{}}\", below(Level::Low, Level::High), below(Level::High, Level::High), same(({u1}, true), ({u1}, true)), same(({u1}, true), ({u2}, true)));
    let z = zero::<f64>();
    let m = make(4.0);
    println!(\"build {{}} {{}} {{}} {{}} {{}} {{}}\", z.a, z.b, m.a, m.b, first(pairup({u9})).get(), pairup({u9}).1);
    let k = |v: u64| rt::Flagged::known(rt::UInt::<8>::new(v));
    let unknown = rt::Flagged::<rt::UInt<8>>::unknown();
    let e = [k(1), k(1), k(2)];
    println!(\"any {{}} {{}} {{}}\", show(pick_any(e, k(2))), show(pick_any(e, unknown)), show(pick_any([k(3), k(3), k(3)], unknown)));
    let s = store_any(e, unknown, k(1));
    println!(\"store {{}} {{}} {{}}\", show(s[0]), show(s[1]), show(s[2]));
    let s = store_any(e, k(2), k(9));
    println!(\"store {{}} {{}} {{}}\", show(s[0]), show(s[1]), show(s[2]));
    let caught = std::panic::catch_unwind(|| pick(a, {u3}));
    println!(\"{{}}\", caught.err().expect(\"the index panics\").downcast_ref::<String>().expect(\"text\"));
}}
",
        u0 = u(0),
        u1 = u(1),
        u2 = u(2),
        u3 = u(3),
        u4 = u(4),
        u9 = u(9),
        u10 = u(10),
        u20 = u(20),
        u30 = u(30),
    );
    let out = Probe::run(&texts, &main);
    let rust_pick = [pick([10, 20, 30], 0), pick([10, 20, 30], 1), pick([10, 20, 30], 2)];
    let expected = vec![
        format!("pick {} {} {}", rust_pick[0], rust_pick[1], rust_pick[2]),
        "fill 0 5 7".to_string(),
        format!("bump {} {}", 1.5f64 + 1.0, !true),
        format!("span {} {}", [1u8, 2, 3, 4][1..].len(), [1u8, 2, 3, 4][4..].len()),
        "payload 6 255".to_string(),
        format!("option {} {} {} {}", true, false, 2.5f64, 0.0f64),
        format!("compare {} {} {} {}", 0 < 1, 1 < 1, (1u8, true) == (1u8, true), (1u8, true) == (2u8, true)),
        format!("build {} {} {} {} {} {}", 0.0f64, false, 4.0f64, true, 9, true),
        "any 2 ? 3".to_string(),
        "store 1 1 ?".to_string(),
        "store 1 1 9".to_string(),
        "aggregates.msl:10:1: the index 3 is out of range for a length of 3".to_string(),
    ];
    assert_eq!(out.lines().collect::<Vec<_>>(), expected);
}

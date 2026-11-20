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
use digital::Dx;
use piperine_ir::{Derives, ExprId, FieldsForm, FunctionSpec, Ir, IrBuilder, Local, Origin, TypeData, TypeId, TypeKind, ValueType, VariantData};
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

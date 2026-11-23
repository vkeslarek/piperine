//! Scope: the printer's names of generic instances (GN-02, D8) and of a hand-written `Repr`
//! (GN-05): IR built by hand with instance names (`swap::<u8>`, `Pair<u8>`) is compiled with the
//! runtime by `rustc` and run; nested argument lists spell apart.
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
use piperine_ir::{Derives, FieldsForm, FunctionSpec, GenericInstance, Ir, IrBuilder, Local, Origin, TypeData, TypeId, TypeKind, ValueType};
use piperine_transpile::print::{TypeItems, ValuePrinter};
use probe::Probe;

const FILE: &str = "generics.msl";

/// The instances of this file, built by hand from the MSL each one transcribes.
struct Instances;

impl Instances {
    fn origin(line: u32) -> Origin {
        Origin { file: FILE.into(), line, col: 1 }
    }

    /// `struct <name> { <fields> }`, the instance `item<arguments>` of a generic struct.
    fn pair(b: &mut IrBuilder, item: &str, arguments: &[&str], fields: Vec<(String, ValueType)>) -> TypeId {
        let instance = GenericInstance { item: item.into(), arguments: arguments.iter().map(|a| a.to_string()).collect() };
        let derives = Derives { clone: true, copy: true, partial_eq: true, ..Derives::default() };
        let kind = TypeKind::Struct { form: FieldsForm::Tuple, fields };
        b.type_data(TypeData { name: instance.type_name(), path: format!("m::{}", instance.type_name()), origin: Self::origin(1), kind, derives, instance: Some(instance), repr: Default::default() })
    }

    /// `fn <name>(x: <ty>) -> <ty> { x }`, the instance `swap::<argument>`.
    fn identity(b: &mut IrBuilder, argument: &str, ty: ValueType, line: u32) {
        let instance = GenericInstance { item: "swap".into(), arguments: vec![argument.into()] };
        let param = Local { name: "x".into(), ty: ty.clone(), mutable: false };
        let body = Dx::new(b, FILE, line, 5).local(0, ty.clone());
        let function = b.function(FunctionSpec { name: instance.name(), origin: Self::origin(line), params: vec![param.clone()], ret: ty, locals: vec![param], body });
        b.mark_instance(function, instance);
    }

    /// The design: `swap::<u8>`, `swap::<Pair<u8>>`, and the three structs `Pair<u8>`,
    /// `Pair<Pair<u8>, u8>` and `Pair<Pair<u8, u8>>` (the last two differ only in their nesting).
    fn design() -> Ir {
        let mut b = IrBuilder::new();
        let inner = Self::pair(&mut b, "Pair", &["u8"], vec![("0".into(), Ty::u(8))]);
        Self::pair(&mut b, "Pair", &["Pair<u8>", "u8"], vec![("0".into(), ValueType::Struct(inner)), ("1".into(), Ty::u(8))]);
        Self::pair(&mut b, "Pair", &["Pair<u8, u8>"], vec![("0".into(), Ty::u(8)), ("1".into(), Ty::u(8))]);
        Self::identity(&mut b, "u8", Ty::u(8), 10);
        Self::identity(&mut b, "Pair<u8>", ValueType::Struct(inner), 12);
        Alone::finish(b)
    }
}

#[test]
fn instance_names_compile_and_nested_arguments_spell_apart() {
    // GN-02 AC 1, D8: an instance prints under the marks `_g`, `_c`, `_e`; two instances whose
    // arguments nest differently are two Rust items, and the functions run
    let ir = Instances::design();
    let mut texts = TypeItems::new(&ir).items().unwrap_or_else(|e| panic!("{e}"));
    texts.extend(ir.functions().map(|f| ValuePrinter::function(&ir, f.id()).unwrap_or_else(|e| panic!("{e}"))));
    let text = texts.join("\n");
    for name in ["Pair_gu8_e", "Pair_gPair_gu8_e_cu8_e", "Pair_gPair_gu8_cu8_e_e", "swap_gu8_e", "swap_gPair_gu8_e_e"] {
        assert!(text.contains(name), "the text names `{name}`:\n{text}");
    }
    let main = "\
fn main() {
    let inner = Pair_gu8_e(rt::UInt::<8>::new(3));
    let outer = Pair_gPair_gu8_e_cu8_e(inner, rt::UInt::<8>::new(4));
    let flat = Pair_gPair_gu8_cu8_e_e(rt::UInt::<8>::new(5), rt::UInt::<8>::new(6));
    println!(\"{} {} {}\", swap_gu8_e(rt::UInt::<8>::new(9)).get(), swap_gPair_gu8_e_e(inner).0.get(), outer.1.get() + flat.1.get());
}
";
    let out = Probe::run(&texts, main);
    assert_eq!(out.trim(), "9 3 10");
}

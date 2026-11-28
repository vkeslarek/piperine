//! Scope: the printer's names of generic instances (GN-02, D8) and of a hand-written `Repr`
//! (GN-05): IR built by hand with instance names (`swap::<u8>`, `Pair<u8>`) is compiled with the
//! runtime by `rustc` and run; nested argument lists spell apart.
#![cfg(unix)]

use piperine_ir_fixtures::digital;
use piperine_ir_fixtures::devices;
use piperine_ir_fixtures::alone;
use crate::probe;
use piperine_parity::mixed;

use alone::Alone;
use devices::{Devices, Ex};
use digital::{Dx, ValueAliases};
use piperine_ir::{Addend, Branch, BranchKind, BodyId, BodyNode, BodySpec, InstanceSpec, NodeRef, NodeRole, Operator, QuantityRole, Stmt, Terminal};
use piperine_transpile::{Settings, Transpiler};
use std::sync::Arc;
use piperine_ir::{Derives, Expr, FieldsForm, FunctionSpec, GenericInstance, Ir, IrBuilder, Local, Origin, ReprForm, TypeData, TypeId, TypeKind, ValueType, VariantData};
use piperine_transpile::print::{TypeItems, ExpressionPrinter};
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
        let inner = Self::pair(&mut b, "Pair", &["u8"], vec![("0".into(), ValueAliases::u(8))]);
        Self::pair(&mut b, "Pair", &["Pair<u8>", "u8"], vec![("0".into(), ValueType::Struct(inner)), ("1".into(), ValueAliases::u(8))]);
        Self::pair(&mut b, "Pair", &["Pair<u8, u8>"], vec![("0".into(), ValueAliases::u(8)), ("1".into(), ValueAliases::u(8))]);
        Self::identity(&mut b, "u8", ValueAliases::u(8), 10);
        Self::identity(&mut b, "Pair<u8>", ValueType::Struct(inner), 12);
        Alone::finish(b)
    }
}

#[test]
fn instance_names_compile_and_nested_arguments_spell_apart() {
    // GN-02 AC 1, D8: a function instance prints under the marks `_g`, `_c`, `_e` and a type instance in upper camel case (`PairGu8E`); two instances whose
    // arguments nest differently are two Rust items, and the functions run
    let ir = Instances::design();
    let mut texts = TypeItems::new(&ir).items().unwrap_or_else(|e| panic!("{e}"));
    texts.extend(ir.functions().map(|f| ExpressionPrinter::function(&ir, f.id()).unwrap_or_else(|e| panic!("{e}"))));
    let text = texts.join("\n");
    for name in ["PairGu8E", "PairGPairGu8ECu8E", "PairGPairGu8Cu8EE", "swap_gu8_e", "swap_gpair_gu8_e_e"] {
        assert!(text.contains(name), "the text names `{name}`:\n{text}");
    }
    let main = "\
fn main() {
    let inner = PairGu8E(rt::UInt::<8>::new(3));
    let outer = PairGPairGu8ECu8E(inner, rt::UInt::<8>::new(4));
    let flat = PairGPairGu8Cu8EE(rt::UInt::<8>::new(5), rt::UInt::<8>::new(6));
    println!(\"{} {} {}\", swap_gu8_e(rt::UInt::<8>::new(9)).get(), swap_gpair_gu8_e_e(inner).0.get(), outer.1.get() + flat.1.get());
}
";
    let out = Probe::run(&texts, main);
    assert_eq!(out.trim(), "9 3 10");
}

/// A one-hot enum whose `Repr` is written by hand (§2, GN-05), built from the MSL it transcribes.
struct OneHot;

impl OneHot {
    /// The design for
    ///
    /// ```text
    /// #[derive(Clone, Copy, PartialEq)] enum OneHot { S0, S1, S2, S3 }
    /// impl Repr for OneHot {
    ///     const WIDTH: u32 = 4;
    ///     fn encode(self) -> [bool; 4] { [self is S0, self is S1, self is S2, self is S3] }
    ///     fn decode(l: [bool; 4]) -> Option<OneHot> { if l[0] { Some(S0) } else if l[1] { Some(S1) } else if l[2] { Some(S2) } else if l[3] { Some(S3) } else { None } }
    /// }
    /// ```
    fn design() -> Ir {
        let mut b = IrBuilder::new();
        let variants = (0..4).map(|n| VariantData { name: format!("S{n}"), discriminant: n, form: FieldsForm::Unit, fields: vec![] }).collect();
        let derives = Derives { clone: true, copy: true, partial_eq: true, ..Derives::default() };
        let one_hot = b.type_data(TypeData {
            name: "OneHot".into(),
            path: "m::OneHot".into(),
            origin: Instances::origin(1),
            kind: TypeKind::Enum { tag_levels: 2, variants },
            derives,
            instance: None,
            repr: ReprForm::Derived,
        });
        let (value, levels) = (ValueType::Enum(one_hot), ValueType::Array(Box::new(ValueType::Bool), 4));
        let option = ValueType::Option(Box::new(value.clone()));
        let tests: Vec<_> = (0..4)
            .map(|k| {
                let x = Dx::new(&mut b, FILE, 3, 5).local(0, value.clone());
                b.expr(Expr::IsVariant { expr: x, variant: k }, ValueType::Bool, Instances::origin(3))
            })
            .collect();
        let written = b.expr(Expr::Array(tests), levels.clone(), Instances::origin(3));
        let x = Local { name: "x".into(), ty: value.clone(), mutable: false };
        let encode = b.function(FunctionSpec { name: "onehot_encode".into(), origin: Instances::origin(3), params: vec![x.clone()], ret: levels.clone(), locals: vec![x], body: written });
        let none = {
            let (no, payload) = (Dx::new(&mut b, FILE, 7, 40).boolean(false), b.expr(Expr::VariantOf { ty: one_hot, variant: 0, fields: vec![] }, value.clone(), Instances::origin(7)));
            b.expr(Expr::OptionPack { is: no, value: payload }, option.clone(), Instances::origin(7))
        };
        let mut chain = none;
        for k in (0..4).rev() {
            let l = Dx::new(&mut b, FILE, 6, 5).local(0, levels.clone());
            let index = Dx::new(&mut b, FILE, 6, 8).int(i128::from(k), ValueAliases::u(8));
            let level = b.expr(Expr::Index { expr: l, index }, ValueType::Bool, Instances::origin(6));
            let (yes, payload) = (Dx::new(&mut b, FILE, 6, 12).boolean(true), b.expr(Expr::VariantOf { ty: one_hot, variant: k, fields: vec![] }, value.clone(), Instances::origin(6)));
            let some = b.expr(Expr::OptionPack { is: yes, value: payload }, option.clone(), Instances::origin(6));
            chain = Dx::new(&mut b, FILE, 6, 20).if_(level, some, chain, option.clone());
        }
        let l = Local { name: "l".into(), ty: levels.clone(), mutable: false };
        let decode = b.function(FunctionSpec { name: "onehot_decode".into(), origin: Instances::origin(6), params: vec![l.clone()], ret: option, locals: vec![l], body: chain });
        b.set_repr(one_hot, ReprForm::HandWritten { width: 4, encode, decode });
        Alone::finish(b)
    }
}

#[test]
fn a_hand_written_repr_round_trips_and_none_is_returned_to_the_caller() {
    // GN-05: a one-hot type carries 4 levels, level k being bit k of the pin; a `decode` that gives
    // `None` is returned by `try_from_levels` (§2 rule 9 as MD-78 amends it); `from_levels` and the
    // pin's `decode`, which cannot fail, give the type's default, as an enum does for a bad tag
    let ir = OneHot::design();
    let mut texts = TypeItems::new(&ir).items().unwrap_or_else(|e| panic!("{e}"));
    texts.extend(ir.functions().map(|f| ExpressionPrinter::function(&ir, f.id()).unwrap_or_else(|e| panic!("{e}"))));
    let main = "\
fn main() {
    use rt::pin::{Carried, LevelWords};
    for (name, value) in [(\"S0\", OneHot::S0), (\"S1\", OneHot::S1), (\"S2\", OneHot::S2), (\"S3\", OneHot::S3)] {
        let pin = value.pin();
        let back = OneHot::from_pin(&pin);
        println!(\"{name} width {} levels {} pin {} word {} back {}\", OneHot::WIDTH, OneHot::LEVELS, pin[0], value.to_levels().low(), back == value);
    }
    let none = OneHot::try_from_levels(&LevelWords::of(0, 4));
    println!(\"none {}\", none.is_none());
    println!(\"default {}\", OneHot::from_levels(&LevelWords::of(0, 4)) == OneHot::S0);
    println!(\"lowest {}\", OneHot::try_from_levels(&LevelWords::of(0b1100, 4)) == Some(OneHot::S2));
}
";
    let out = Probe::run(&texts, main);
    let expected = [
        "S0 width 1 levels 4 pin 1 word 1 back true",
        "S1 width 1 levels 4 pin 2 word 2 back true",
        "S2 width 1 levels 4 pin 4 word 4 back true",
        "S3 width 1 levels 4 pin 8 word 8 back true",
        "none true",
        "default true",
        "lowest true",
    ];
    assert_eq!(out.lines().collect::<Vec<_>>(), expected);
}

/// Bodies that lower instances of generic modules, built by hand from the MSL each one transcribes.
struct ModuleInstances;

impl ModuleInstances {
    /// `module Load(p) { }` with `i(p) +> v(p)`, as the body `name`, an instance of `item` at `arguments`.
    fn body(b: &mut IrBuilder, name: &str, item: &str, arguments: &[&str]) -> BodyId {
        let nature = Devices::electrical(b);
        let value = Ex::new(b, FILE, 2, 5).probe(Terminal::Node(0), Terminal::Ground);
        let stmt = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
        let body = b.body(BodySpec {
            name: name.into(),
            origin: Instances::origin(1),
            params: vec![],
            nodes: vec![BodyNode { name: "p".into(), nature, role: NodeRole::Port, init: None, origin: Instances::origin(1) }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![stmt]),
            children: vec![],
        });
        b.mark_body_instance(body, GenericInstance { item: item.into(), arguments: arguments.iter().map(|a| a.to_string()).collect() }).expect("a body this builder handed out");
        body
    }

    /// The design of `Ladder::<3>` and the two key variants of `idt::<State>`, the first placed as the top.
    fn design() -> Ir {
        let mut b = IrBuilder::new();
        let ladder = Self::body(&mut b, "Ladder::<3>", "Ladder", &["3"]);
        Self::body(&mut b, "idt::<State>~r=1k", "idt", &["State"]);
        Self::body(&mut b, "idt::<State>~r=2k", "idt", &["State"]);
        let top = b.instance(InstanceSpec { path: "L".into(), name: "L".into(), body: ladder, parent: None, nodes: vec![NodeRef::Ground], params: vec![], origin: Instances::origin(1) });
        b.finish(top)
    }
}

#[test]
fn module_instance_names_compile_and_list_in_the_library() {
    // GN-15 AC 2: a body named for an instance of a generic module (`Ladder::<3>`, `idt::<State>~r=1k`) prints as Rust that
    // compiles with no warning, two instances never share a spelling, and the library lists each by its MSL name
    let ir = ModuleInstances::design();
    assert_eq!(ir.check(), vec![], "{ir}");
    let cache = tempfile::tempdir().expect("a cache directory");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let compiled = Transpiler::new(Arc::new(ir)).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(compiled.report().warnings, "", "rustc has nothing to say of the generated source");
    assert_eq!(compiled.loaded().names(), vec!["Ladder::<3>", "idt::<State>~r=1k", "idt::<State>~r=2k"]);
}

/// A compiled body that reads the run's temperature as the `Option` pair `temperature()` is (GN-24, GN-25).
struct RunTemperature;

impl RunTemperature {
    /// The run's temperature in kelvin, set by the run in the second drive.
    const KELVIN: f64 = 350.0;

    /// `module Reads(out given: Bit<f64>, out kelvin: Bit<f64>) {}` with
    /// `given = if temperature_given { 1.0 } else { 0.0 }; kelvin = temperature;` as drives.
    fn body(b: &mut IrBuilder) -> BodyId {
        let file = "temperature.msl";
        let body = b.body(digital::Buck::spec("Reads", file, 1));
        for (i, name) in ["given", "kelvin"].into_iter().enumerate() {
            b.carrier(body, digital::Buck::carrier(name, ValueType::F64, piperine_ir::CarrierRole::Out, None, file, 1, 20 + 10 * i as u32)).expect("a body this builder handed out");
        }
        let at = digital::Buck::origin(file, 2, 1);
        let flag = b.expr(Expr::Env(piperine_ir::Env::TemperatureGiven), ValueType::Bool, at.clone());
        let (one, zero) = (b.expr(Expr::Real(1.0.into()), ValueType::F64, at.clone()), b.expr(Expr::Real(0.0.into()), ValueType::F64, at.clone()));
        let given = b.expr(Expr::If { cond: flag, then: one, otherwise: zero }, ValueType::F64, at.clone());
        let kelvin = b.expr(Expr::Env(piperine_ir::Env::Temperature), ValueType::F64, at.clone());
        for (carrier, value) in [(0, given), (1, kelvin)] {
            b.drive(body, piperine_ir::Drive { carrier, value, after: None, origin: at.clone(), element: None, path: vec![] }).expect("a body this builder handed out");
        }
        body
    }

    /// The last value of nets `given` and `kelvin` after a run in the session `context`.
    fn run(context: piperine_solver::prelude::Context) -> [f64; 2] {
        let mut w = piperine_ir_fixtures::mixed::TopDesign::new();
        let body = Self::body(&mut w.b);
        w.place("reads", body, vec![], &[("given", ValueType::F64), ("kelvin", ValueType::F64)]);
        let (seen, _) = mixed::BuiltDesign::new(w.finish()).run_in(context, 2e-9, 1e-9);
        let last = |name: &str| {
            let bytes = seen.series(name).last().unwrap_or_else(|| panic!("net {name} has a value")).1.clone();
            f64::from_le_bytes(bytes[..8].try_into().expect("eight bytes"))
        };
        [last("given"), last("kelvin")]
    }
}

#[test]
fn a_device_reads_whether_the_run_set_its_temperature() {
    // GN-24 AC 1, GN-25 AC 1: the device's `TEMPERATURE_GIVEN` pin is 0 and `TEMPERATURE` holds the session's
    // 300.15 K when the run sets none, and 1 and the run's kelvin when it sets one
    let unset = RunTemperature::run(piperine_solver::prelude::Context::default());
    assert_eq!(unset, [0.0, 300.15], "no `temp`: not given, the solver's default");
    let mut context = piperine_solver::prelude::Context::default();
    context.tolerances.set_temperature(RunTemperature::KELVIN);
    assert_eq!(RunTemperature::run(context), [1.0, RunTemperature::KELVIN], "`temp` set: given, the run's kelvin");
}

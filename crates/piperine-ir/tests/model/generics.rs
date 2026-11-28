//! Scope: the IR forms of `generics` (GN-02, GN-05, GN-06), built by hand through `IrBuilder`,
//! checked (valid and refused) and printed: a function that is the instance of a generic item.

use piperine_ir::{BodySpec, Derives, Env, Expr, FieldsForm, FunctionId, FunctionSpec, GenericInstance, InstanceSpec, Ir, IrBuilder, Local, Origin, ReprForm, TypeData, TypeKind, ValueType, VariantData};

/// The small designs every test of this target builds.
struct Designs;

impl Designs {
    /// A position in the file the designs are written in.
    fn origin() -> Origin {
        Origin { file: "generics.msl".into(), line: 1, col: 1 }
    }

    /// `fn <name>(x: f64) -> f64 { x }` in a design of one empty module, marked as `instance` when given.
    fn with_function(name: &str, instance: Option<GenericInstance>) -> Ir {
        let mut b = IrBuilder::new();
        let x = Local { name: "x".into(), ty: ValueType::F64, mutable: false };
        let body = b.expr(Expr::Local(0), ValueType::F64, Self::origin());
        let function: FunctionId = b.function(FunctionSpec { name: name.into(), origin: Self::origin(), params: vec![x.clone()], ret: ValueType::F64, locals: vec![x], body });
        if let Some(instance) = instance {
            b.mark_instance(function, instance);
        }
        let module = b.body(BodySpec { name: "Top".into(), origin: Self::origin(), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: module, parent: None, nodes: vec![], params: vec![], origin: Self::origin() });
        b.finish(top)
    }

    /// The instance `swap::<f64>`.
    fn swap() -> GenericInstance {
        GenericInstance { item: "swap".into(), arguments: vec!["f64".into()] }
    }
}

#[test]
fn a_function_instance_is_built_checked_and_printed() {
    // GN-02 AC 1: the instance is named from the item and its arguments; a half of an `Option`
    // result keeps its suffix; the printer says which item the function instantiates
    let ir = Designs::with_function("swap::<f64>", Some(Designs::swap()));
    assert_eq!(ir.check(), vec![], "{ir}");
    let function = ir.functions().next().expect("the design has its function");
    assert_eq!(function.instance(), Some(&Designs::swap()));
    assert!(ir.to_string().contains("fn \"swap::<f64>\"(x: f64) -> f64"), "{ir}");
    assert!(ir.to_string().contains("// instance of swap"), "{ir}");
    let pair = GenericInstance { item: "widen".into(), arguments: vec!["3".into(), "u8".into()] };
    assert_eq!(pair.name(), "widen::<3, u8>");
    assert_eq!(Designs::with_function("widen::<3, u8>.is", Some(pair)).check(), vec![]);
    let written = Designs::with_function("plain", None);
    assert!(written.functions().next().is_some_and(|f| f.instance().is_none()));
    assert!(!written.to_string().contains("instance of"), "{written}");
    let round_trip: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serializes")).expect("deserializes");
    assert_eq!(round_trip, ir, "the instance survives serde");
}

#[test]
fn an_instance_whose_name_disagrees_is_refused() {
    // GN-02 AC 1: a function marked as `swap::<f64>` named anything else is a malformed IR
    let ir = Designs::with_function("swap::<u8>", Some(Designs::swap()));
    let found = ir.check();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].what.contains("`swap::<f64>`") && found[0].what.contains("`swap::<u8>`"), "{:?}", found[0]);
}

impl Designs {
    /// A design of one empty module named `name`, marked as `instance` when given.
    fn with_body(name: &str, instance: Option<GenericInstance>) -> Ir {
        let mut b = IrBuilder::new();
        let module = b.body(BodySpec { name: name.into(), origin: Self::origin(), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        if let Some(instance) = instance {
            b.mark_body_instance(module, instance).expect("a body this builder handed out");
        }
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: module, parent: None, nodes: vec![], params: vec![], origin: Self::origin() });
        b.finish(top)
    }

    /// The instance `Ladder::<3>`.
    fn ladder() -> GenericInstance {
        GenericInstance { item: "Ladder".into(), arguments: vec!["3".into()] }
    }
}

#[test]
fn a_body_instance_is_built_checked_and_printed() {
    // GN-15 AC 2: a body that lowers an instance of a generic module is named from the module and its arguments, then the
    // suffix of its key params; the printer says which module it instantiates and serde keeps it
    let ir = Designs::with_body("Ladder::<3>", Some(Designs::ladder()));
    assert_eq!(ir.check(), vec![], "{ir}");
    assert_eq!(ir.bodies().next().and_then(|body| body.instance()), Some(&Designs::ladder()));
    assert!(ir.to_string().contains("body \"Ladder::<3>\"") && ir.to_string().contains("instance of Ladder"), "{ir}");
    assert_eq!(Designs::with_body("Ladder::<3>~r=1k", Some(Designs::ladder())).check(), vec![], "the key params' suffix is allowed");
    let written = Designs::with_body("Top", None);
    assert!(written.bodies().next().is_some_and(|body| body.instance().is_none()));
    assert!(!written.to_string().contains("instance of"), "{written}");
    let round_trip: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serializes")).expect("deserializes");
    assert_eq!(round_trip, ir, "the body instance survives serde");
}

#[test]
fn a_body_whose_name_disagrees_with_its_instance_is_refused() {
    // GN-15 AC 2: a body marked as `Ladder::<3>` named anything else, or one that only shares its prefix, is malformed
    for name in ["Ladder::<4>", "Ladder::<3>x"] {
        let found = Designs::with_body(name, Some(Designs::ladder())).check();
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert!(found[0].what.contains("`Ladder::<3>`") && found[0].what.contains(name), "{:?}", found[0]);
    }
}

/// A design with a one-hot enum of four variants whose `Repr` is written by hand.
struct OneHot;

impl OneHot {
    /// The design: the enum `OneHot` (named `name`, marked an instance of `Pair` when `instance`),
    /// `encode` returning `[bool; encoded]` levels and `decode` taking `[bool; decoded]` levels and
    /// returning an `Option` of the enum; `width` is the width the type declares.
    fn ir(name: &str, instance: Option<GenericInstance>, width: u32, encoded: u64, decoded: u64) -> Ir {
        let mut b = IrBuilder::new();
        let variants = (0..4).map(|n| VariantData { name: format!("S{n}"), discriminant: n, form: FieldsForm::Unit, fields: vec![] }).collect();
        let one_hot = b.type_data(TypeData {
            name: name.into(),
            path: "OneHot".into(),
            origin: Designs::origin(),
            kind: TypeKind::Enum { tag_levels: 2, variants },
            derives: Derives { repr: true, ..Derives::default() },
            instance,
            repr: ReprForm::Derived,
        });
        let enum_type = ValueType::Enum(one_hot);
        let array = |n: u64| ValueType::Array(Box::new(ValueType::Bool), n);
        let (x, l) = (Local { name: "x".into(), ty: enum_type.clone(), mutable: false }, Local { name: "l".into(), ty: array(decoded), mutable: false });
        let falses: Vec<_> = (0..encoded).map(|_| b.expr(Expr::Bool(false), ValueType::Bool, Designs::origin())).collect();
        let written = b.expr(Expr::Array(falses), array(encoded), Designs::origin());
        let variant = b.expr(Expr::VariantOf { ty: one_hot, variant: 0, fields: vec![] }, enum_type.clone(), Designs::origin());
        let present = b.expr(Expr::Bool(true), ValueType::Bool, Designs::origin());
        let option = ValueType::Option(Box::new(enum_type));
        let read = b.expr(Expr::OptionPack { is: present, value: variant }, option.clone(), Designs::origin());
        let encode = b.function(FunctionSpec { name: "OneHot::encode".into(), origin: Designs::origin(), params: vec![x.clone()], ret: array(encoded), locals: vec![x], body: written });
        let decode = b.function(FunctionSpec { name: "OneHot::decode".into(), origin: Designs::origin(), params: vec![l.clone()], ret: option, locals: vec![l], body: read });
        b.set_repr(one_hot, ReprForm::HandWritten { width, encode, decode });
        let module = b.body(BodySpec { name: "Top".into(), origin: Designs::origin(), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: module, parent: None, nodes: vec![], params: vec![], origin: Designs::origin() });
        b.finish(top)
    }
}

#[test]
fn a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed() {
    // GN-05 AC 1, GN-06 AC 1: a type is an instance named `item<arguments>`; its hand-written `Repr`
    // is `width` levels through `encode` and `decode`
    let pair = GenericInstance { item: "Pair".into(), arguments: vec!["f64".into()] };
    let ir = OneHot::ir("Pair<f64>", Some(pair.clone()), 4, 4, 4);
    assert_eq!(ir.check(), vec![], "{ir}");
    let data = &ir.types()[0];
    assert_eq!(data.instance, Some(pair));
    assert!(matches!(data.repr, ReprForm::HandWritten { width: 4, .. }), "{:?}", data.repr);
    let text = ir.to_string();
    assert!(text.contains("instance of Pair") && text.contains("repr hand-written 4 levels, encode \"OneHot::encode\", decode \"OneHot::decode\""), "{text}");
    let round_trip: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serializes")).expect("deserializes");
    assert_eq!(round_trip, ir, "the instance and the repr survive serde");
    let derived = OneHot::ir("OneHot", None, 4, 4, 4);
    assert_eq!(derived.check(), vec![]);
}

#[test]
fn a_hand_written_repr_with_wrong_signatures_is_refused() {
    // GN-05: `encode` must return `[bool; width]` and `decode` take it; the width is one level at least
    let encode = OneHot::ir("OneHot", None, 4, 3, 4).check();
    assert!(encode.iter().any(|b| b.what.contains("the encode of a hand-written Repr")), "{encode:?}");
    let decode = OneHot::ir("OneHot", None, 4, 4, 5).check();
    assert!(decode.iter().any(|b| b.what.contains("the decode of a hand-written Repr")), "{decode:?}");
    let empty = OneHot::ir("OneHot", None, 0, 4, 4).check();
    assert!(empty.iter().any(|b| b.what.contains("at least one level")), "{empty:?}");
    let named = OneHot::ir("Pair<u8>", Some(GenericInstance { item: "Pair".into(), arguments: vec!["f64".into()] }), 4, 4, 4).check();
    assert!(named.iter().any(|b| b.what.contains("a generic instance of `Pair`")), "{named:?}");
}

/// The IR reads of the run's settings: whether the run set its temperature (GN-24, GN-25, GN-29).
struct RunSettings;

impl RunSettings {
    /// `fn read(x: f64) -> Option<f64> { Some(temperature) if temperature_given }` in a design of one empty module: the
    /// `Option` pair of `temperature()`, the given flag typed as `given`.
    fn with_read(given: ValueType) -> Ir {
        let mut b = IrBuilder::new();
        let x = Local { name: "x".into(), ty: ValueType::F64, mutable: false };
        let flag = b.expr(Expr::Env(Env::TemperatureGiven), given, Designs::origin());
        let value = b.expr(Expr::Env(Env::Temperature), ValueType::F64, Designs::origin());
        let pair = b.expr(Expr::OptionPack { is: flag, value }, ValueType::Option(Box::new(ValueType::F64)), Designs::origin());
        b.function(FunctionSpec { name: "read".into(), origin: Designs::origin(), params: vec![x.clone()], ret: ValueType::Option(Box::new(ValueType::F64)), locals: vec![x], body: pair });
        let module = b.body(BodySpec { name: "Top".into(), origin: Designs::origin(), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: module, parent: None, nodes: vec![], params: vec![], origin: Designs::origin() });
        b.finish(top)
    }
}

#[test]
fn run_setting_reads_are_built_checked_and_printed() {
    // GN-24 AC 1, GN-25 AC 1, GN-29 AC 1: `temperature()` is the `Option` pair (given, temperature); the given flag is a
    // `bool`, a read typed otherwise is a malformed IR, and the printer names it
    let ir = RunSettings::with_read(ValueType::Bool);
    assert_eq!(ir.check(), vec![], "{ir}");
    assert!(ir.to_string().contains("temperature_given()"), "{ir}");
    let round_trip: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serializes")).expect("deserializes");
    assert_eq!(round_trip, ir, "the read survives serde");
    let wrong = RunSettings::with_read(ValueType::F64);
    assert!(!wrong.check().is_empty(), "a `temperature_given` typed `f64` is refused");
}

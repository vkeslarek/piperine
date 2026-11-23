//! Scope: the IR forms of `generics` (GN-02, GN-05, GN-06), built by hand through `IrBuilder`,
//! checked (valid and refused) and printed: a function that is the instance of a generic item.

use piperine_ir::{BodySpec, Expr, FunctionId, FunctionSpec, GenericInstance, InstanceSpec, Ir, IrBuilder, Local, Origin, ValueType};

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

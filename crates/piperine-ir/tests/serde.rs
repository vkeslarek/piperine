//! Scope: serialization of the whole IR (IR-03): every storage type implements serde, a JSON round
//! trip gives an equal `Ir` (reals bit for bit, `inf` and `-0.0` included), and text that is no
//! `Ir` fails to deserialize.

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::{Devices, Ex};
use piperine_ir::{
    Addend, BinaryOp, BodyData, BodyId, BodyNode, Branch, BranchKind, QuantityRole, Child, Constant, Env, Expr, ExprData,
    ExprId, FunctionData, FunctionId, InstanceData, InstanceId, Ir, IrBuilder, Local, MathFn,
    NatureData, NatureId, NodeData, NodeId, NodeRef, NodeRole, Operator, Origin, Param, Quantity,
    Real, Site, Stmt, StmtId, Terminal, UnaryOp, ValueType,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A static fact: the type implements both directions of serde.
struct SerdeBound<T: Serialize + DeserializeOwned>(std::marker::PhantomData<T>);

/// Every storage type, one bound each.
struct Bounds;

impl Bounds {
    fn hold() {
        let _ = (
            SerdeBound::<Ir>(std::marker::PhantomData),
            SerdeBound::<NatureData>(std::marker::PhantomData),
            SerdeBound::<Quantity>(std::marker::PhantomData),
            SerdeBound::<BodyData>(std::marker::PhantomData),
            SerdeBound::<Param>(std::marker::PhantomData),
            SerdeBound::<BodyNode>(std::marker::PhantomData),
            SerdeBound::<NodeRole>(std::marker::PhantomData),
            SerdeBound::<Terminal>(std::marker::PhantomData),
            SerdeBound::<Branch>(std::marker::PhantomData),
            SerdeBound::<BranchKind>(std::marker::PhantomData),
            SerdeBound::<QuantityRole>(std::marker::PhantomData),
            SerdeBound::<Site>(std::marker::PhantomData),
            SerdeBound::<Local>(std::marker::PhantomData),
            SerdeBound::<Child>(std::marker::PhantomData),
            SerdeBound::<FunctionData>(std::marker::PhantomData),
            SerdeBound::<ExprData>(std::marker::PhantomData),
            SerdeBound::<Expr>(std::marker::PhantomData),
            SerdeBound::<Stmt>(std::marker::PhantomData),
            SerdeBound::<Addend>(std::marker::PhantomData),
            SerdeBound::<Operator>(std::marker::PhantomData),
            SerdeBound::<UnaryOp>(std::marker::PhantomData),
            SerdeBound::<BinaryOp>(std::marker::PhantomData),
            SerdeBound::<MathFn>(std::marker::PhantomData),
            SerdeBound::<Env>(std::marker::PhantomData),
            SerdeBound::<ValueType>(std::marker::PhantomData),
            SerdeBound::<Constant>(std::marker::PhantomData),
            SerdeBound::<InstanceData>(std::marker::PhantomData),
            SerdeBound::<NodeRef>(std::marker::PhantomData),
            SerdeBound::<NodeData>(std::marker::PhantomData),
            SerdeBound::<Origin>(std::marker::PhantomData),
            SerdeBound::<Real>(std::marker::PhantomData),
        );
        let _ = (
            SerdeBound::<NatureId>(std::marker::PhantomData),
            SerdeBound::<BodyId>(std::marker::PhantomData),
            SerdeBound::<FunctionId>(std::marker::PhantomData),
            SerdeBound::<ExprId>(std::marker::PhantomData),
            SerdeBound::<StmtId>(std::marker::PhantomData),
            SerdeBound::<InstanceId>(std::marker::PhantomData),
            SerdeBound::<NodeId>(std::marker::PhantomData),
        );
    }
}

#[test]
fn storage_types_implement_serde() {
    Bounds::hold();
}

/// An IR holding the devices, the divider, and a body whose reals are the awkward ones.
struct Corpus;

impl Corpus {
    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        Devices::diode(&mut b);
        Devices::isource(&mut b);
        let origin = Origin { file: "reals.msl".into(), line: 1, col: 1 };
        let mut e = Ex::new(&mut b, "reals.msl", 1, 1);
        let reals = [f64::INFINITY, f64::NEG_INFINITY, -0.0, 0.0, 1e-14, f64::NAN, f64::MIN_POSITIVE];
        let ids: Vec<ExprId> = reals.iter().map(|&v| e.real(v)).collect();
        let sum = ids.iter().copied().reduce(|a, c| e.binary(BinaryOp::Add, a, c)).expect("reals");
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: sum }],
        });
        b.body(piperine_ir::BodySpec {
            name: "Reals".into(),
            origin: origin.clone(),
            params: vec![Param {
                name: "x".into(),
                ty: ValueType::F64,
                default: Some(Constant::Real(Real::new(f64::NEG_INFINITY))),
                check: None,
                structural: false,
                origin: origin.clone(),
            }],
            nodes: vec![BodyNode {
                name: "a".into(),
                nature: el,
                role: NodeRole::Signal,
                init: Some(Real::new(-0.0)),
                origin: origin.clone(),
            }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![stmt]),
            children: vec![],
        });
        let top = Devices::divider(&mut b);
        b.finish(top)
    }
}

#[test]
fn json_round_trip_gives_an_equal_ir() {
    let ir = Corpus::ir();
    let text = serde_json::to_string(&ir).expect("serialize");
    let back: Ir = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(back, ir);
    // Serializing the copy gives the same text.
    assert_eq!(serde_json::to_string(&back).expect("serialize"), text);
}

#[test]
fn reals_survive_bit_for_bit() {
    let ir = Corpus::ir();
    let text = serde_json::to_string(&ir).expect("serialize");
    let back: Ir = serde_json::from_str(&text).expect("deserialize");
    let reals = back.bodies().find(|b| b.name() == "Reals").expect("Reals");
    let stmt = reals.analog().expect("analog")[0];
    let Stmt::Contribute { addends, .. } = back.stmt(stmt) else { panic!("a contribution") };
    let mut seen = Vec::new();
    Leaves::collect(&back, addends[0].value, &mut seen);
    let want: Vec<u64> = [f64::INFINITY, f64::NEG_INFINITY, -0.0, 0.0, 1e-14, f64::NAN, f64::MIN_POSITIVE]
        .iter()
        .map(|v| v.to_bits())
        .collect();
    assert_eq!(seen, want);
    assert_eq!(reals.nodes()[0].init.map(Real::bits), Some((-0.0f64).to_bits()));
    assert_ne!(Real::new(-0.0), Real::new(0.0), "equality is by bit pattern");
    assert_eq!(Real::new(f64::NAN), Real::new(f64::NAN), "NaN equals itself bitwise");
}

#[test]
fn text_that_is_no_ir_fails_to_deserialize() {
    let valid = serde_json::to_string(&Corpus::ir()).expect("serialize");
    let truncated = &valid[..valid.len() / 2];
    for text in ["", "not json", "{}", "[1, 2, 3]", "null", "{\"natures\": []}", truncated] {
        assert!(serde_json::from_str::<Ir>(text).is_err(), "accepted: {text:.40}");
    }
}

/// The bit patterns of the real leaves of a sum, left to right.
struct Leaves;

impl Leaves {
    fn collect(ir: &Ir, id: ExprId, out: &mut Vec<u64>) {
        match ir.expr(id) {
            Expr::Real(r) => out.push(r.bits()),
            Expr::Binary { lhs, rhs, .. } => {
                Leaves::collect(ir, *lhs, out);
                Leaves::collect(ir, *rhs, out);
            }
            other => panic!("unexpected node {other:?}"),
        }
    }
}

//! Scope: the derive functions of a design's generated source (feature `host-ii`, X9, HII-18):
//! one function per derived param, the export beside `piperine_abi_1`, and a design that derives
//! nothing printing none of it. The design is `Top { t } { m: Mid { rr: t * 3.0 } }` with
//! `Mid { rr } { l: Leaf { r: rr * 2.0 } }`: a chain, so the grandchild derives from a param that
//! is itself derived.
#![cfg(unix)]

use std::sync::Arc;

use piperine_ir::{BinaryOp, BodyId, BodySpec, Child, ChildValue, Constant, Expr, InstanceSpec, Ir, IrBuilder, Origin, Param, Real, ValueType};
use piperine_transpile::{Settings, Transpiler};

fn origin() -> Origin {
    Origin { file: "derive.msl".into(), line: 1, col: 1 }
}

fn param(name: &str) -> Param {
    Param { name: name.into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1.0))), check: None, structural: false, origin: origin() }
}

fn body(b: &mut IrBuilder, name: &str, params: Vec<Param>, children: Vec<Child>) -> BodyId {
    b.body(BodySpec { name: name.into(), origin: origin(), params, nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children })
}

fn placed(name: &str, body: BodyId) -> Child {
    Child { name: name.into(), body, nodes: vec![], params: vec![], origin: origin() }
}

fn instance(b: &mut IrBuilder, path: &str, name: &str, body: BodyId, parent: Option<piperine_ir::InstanceId>, params: Vec<f64>) -> piperine_ir::InstanceId {
    let params = params.into_iter().map(|v| Constant::Real(Real::new(v))).collect();
    b.instance(InstanceSpec { path: path.into(), name: name.into(), body, parent, nodes: vec![], params, origin: origin() })
}

/// `scale * param(0)` as a child value of `owner`'s placement `child`, for its param `index`.
fn scaled(b: &mut IrBuilder, owner: BodyId, child: u32, index: u32, scale: f64) {
    let k = b.expr(Expr::Real(Real::new(scale)), ValueType::F64, origin());
    let p = b.expr(Expr::Param(0), ValueType::F64, origin());
    let product = b.expr(Expr::Binary { op: BinaryOp::Mul, lhs: p, rhs: k }, ValueType::F64, origin());
    b.child_param(owner, child, index, ChildValue::Expr(product));
}

/// The chain design, or the same bodies with no derived placement.
pub fn chain(derived: bool) -> Ir {
    let mut b = IrBuilder::new();
    let leaf = body(&mut b, "Leaf", vec![param("r")], vec![]);
    let mid = body(&mut b, "Mid", vec![param("rr")], vec![placed("l", leaf)]);
    let top = body(&mut b, "Top", vec![param("t")], vec![placed("m", mid)]);
    if derived {
        scaled(&mut b, mid, 0, 0, 2.0);
        scaled(&mut b, top, 0, 0, 3.0);
    }
    let t = instance(&mut b, "Top", "Top", top, None, vec![1.0]);
    let m = instance(&mut b, "Top.m", "m", mid, Some(t), vec![3.0]);
    instance(&mut b, "Top.m.l", "l", leaf, Some(m), vec![6.0]);
    let ir = b.finish(t);
    assert_eq!(ir.check(), vec![], "the fixture passes check()");
    ir
}

fn source(ir: Ir) -> String {
    Transpiler::new(Arc::new(ir)).unwrap_or_else(|e| panic!("{e}")).source().unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_source_holds_one_derive_function_per_binding() {
    // HII-18 AC 1: `Top.m.rr = t * 3.0` over the params of `Top` and `Top.m.l.r = rr * 2.0` over
    // those of `Top.m`; each is a function of its parent's params in declaration order, and the
    // export routes `k` to them
    let text = source(chain(true));
    assert_eq!(text.matches("fn derive_").count(), 2, "{text}");
    assert!(text.contains("fn derive_0_(params_: &[u8]) -> Vec<u8> {\n    let t = <f64 as rt::Carried>::from_pin(&params_[0..8]);\n    <f64 as rt::Carried>::pin(t * 3.0)\n}"), "{text}");
    assert!(text.contains("fn derive_1_(params_: &[u8]) -> Vec<u8> {\n    let rr = <f64 as rt::Carried>::from_pin(&params_[0..8]);\n    <f64 as rt::Carried>::pin(rr * 2.0)\n}"), "{text}");
    assert!(text.contains("pub extern \"C\" fn piperine_derive_3(k: u32, params: List<'_, u8>, mut out: BytesMut<'_>) -> u32"), "{text}");
    assert!(text.contains("0 if params.len() == 8 => Some(derive_0_(&params)),") && text.contains("1 if params.len() == 8 => Some(derive_1_(&params)),"), "{text}");
    // a design that derives nothing prints none of it (the sources of every other design are unchanged)
    let plain = source(chain(false));
    assert!(!plain.contains("fn derive_") && !plain.contains("piperine_derive_3"), "{plain}");
}

#[test]
fn compiled_derive_evaluates_the_placement_expression() {
    // HII-18 AC 1: the loaded function `k` gives the placement's expression over the parent's
    // params: `t * 3.0` for the middle instance, `rr * 2.0` for the leaf; a `k` it has no binding
    // for, or a parent's params of another count, is NaN; a design that derives nothing exports
    // no function, so its answer is NaN too
    let cache = tempfile::tempdir().expect("tmp");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let compiled = Transpiler::new(Arc::new(chain(true))).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    let bytes = |value: f64| value.to_le_bytes().to_vec();
    assert_eq!(compiled.derive(0, &bytes(2.0), 8), Some(bytes(6.0)));
    assert_eq!(compiled.derive(1, &bytes(6.0), 8), Some(bytes(12.0)));
    assert_eq!(compiled.derive(0, &[bytes(1.0), bytes(2.0)].concat(), 8), None, "a parent of another width of params");
    assert_eq!(compiled.derive(2, &bytes(1.0), 8), None, "no binding 2");
    let plain = Transpiler::new(Arc::new(chain(false))).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(plain.derive(0, &bytes(2.0), 8), None);
}

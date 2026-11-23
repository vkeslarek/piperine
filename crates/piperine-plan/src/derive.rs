//! `DerivePlan`: the param values a design derives from other params (feature `host-ii`, X9).
//! A placement `Child(..) { r: r_top }` gives its child a param that is an expression over the
//! parent's params (`ChildValue::Expr`, kept beside the child by the IR); the instance table holds
//! only the constant that expression folds to. A host `set` of a parent's param must recompute
//! what derives from it, so the plan lists every such binding, a parent before its children (a
//! chain derives top-down), with the parent's params each one reads.

use piperine_ir::{ChildValue, Expr, ExprId, Ir, ValueType};

/// One derived param: `child`'s `param` is `expr` over the params of `parent`.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// The index of the generated function that computes it: the binding's place in the plan.
    pub k: u32,
    /// The path of the instance whose params the expression reads.
    pub parent: String,
    /// The path of the instance whose param it gives.
    pub child: String,
    /// The child's param name.
    pub param: String,
    /// The index of that param in the child's body.
    pub param_index: u32,
    /// The type of that param.
    pub ty: ValueType,
    /// The names of the parent's params the expression reads, in the parent's param order.
    pub reads: Vec<String>,
    /// The expression, over the parent body's params.
    pub expr: ExprId,
}

/// Every derived param of a design, in instance order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DerivePlan {
    bindings: Vec<Binding>,
}

impl DerivePlan {
    /// The derived bindings of `ir`: one per `ChildValue::Expr` of each instance's placement, the
    /// instances in the IR's order (a parent before its children), a placement's params in the
    /// order they were set.
    pub fn of(ir: &Ir) -> DerivePlan {
        let mut bindings = Vec::new();
        for child in ir.instances() {
            let Some(parent) = child.parent() else { continue };
            let parent_body = parent.body();
            let Some(placed) = parent_body.children().iter().position(|c| c.name == child.name()) else { continue };
            for (index, value) in parent_body.child_params(placed) {
                let ChildValue::Expr(expr) = value else { continue };
                let Some(param) = child.body().params().get(*index as usize) else { continue };
                let read = ParamReads::of(ir, *expr);
                let reads = parent_body.params().iter().enumerate().filter(|(i, _)| read.contains(&(*i as u32))).map(|(_, p)| p.name.clone()).collect();
                bindings.push(Binding {
                    k: bindings.len() as u32,
                    parent: parent.path().to_string(),
                    child: child.path().to_string(),
                    param: param.name.clone(),
                    param_index: *index,
                    ty: param.ty.clone(),
                    reads,
                    expr: *expr,
                });
            }
        }
        DerivePlan { bindings }
    }

    /// The bindings, `k` the index of each.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Whether the design derives nothing.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// The bindings (by `k`) that read `param` of `instance`, in plan order.
    pub fn readers(&self, instance: &str, param: &str) -> Vec<u32> {
        self.bindings.iter().filter(|b| b.parent == instance && b.reads.iter().any(|r| r == param)).map(|b| b.k).collect()
    }

    /// The binding that gives `param` of `instance`, if the design derives it.
    pub fn giving(&self, instance: &str, param: &str) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.child == instance && b.param == param)
    }
}

/// The params an expression reads: one walk over every form of `Expr`, shared by the derive plan
/// (which bindings a `set` must recompute) and by the host's refusals (which params others derive
/// from). A form that is added to `Expr` fails to compile here until the walk says what it reads.
pub struct ParamReads;

impl ParamReads {
    /// The indices of the params of the body that `expr` reads, in the order the walk meets them
    /// (a param read twice appears twice).
    pub fn of(ir: &Ir, expr: ExprId) -> Vec<u32> {
        let mut found = Vec::new();
        ParamReads::walk(ir, expr, &mut found);
        found
    }

    /// Whether `expr` reads the param at `index`.
    pub fn reads(ir: &Ir, expr: ExprId, index: u32) -> bool {
        ParamReads::of(ir, expr).contains(&index)
    }

    /// Adds to `out` every param index `expr` reads.
    fn walk(ir: &Ir, expr: ExprId, out: &mut Vec<u32>) {
        match ir.expr(expr) {
            Expr::Param(index) => out.push(*index),
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::ParamPrevious(_) => {}
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::LevelCast { expr, .. } | Expr::Table { x: expr, .. } | Expr::TableSlope { x: expr, .. } => ParamReads::walk(ir, *expr, out),
            Expr::Closure { body, .. } => ParamReads::walk(ir, *body, out),
            Expr::CarrierElement { index, .. } | Expr::ElementPrevious { index, .. } | Expr::ElementChanged { index, .. } => ParamReads::walk(ir, *index, out),
            Expr::Binary { lhs: first, rhs: second, .. } | Expr::Unwrap { is: first, value: second, .. } | Expr::Same { a: first, b: second } => ParamReads::walk_all(ir, &[*first, *second], out),
            Expr::If { cond, then, otherwise } => ParamReads::walk_all(ir, &[*cond, *then, *otherwise], out),
            Expr::Call { args, .. } | Expr::Math { args, .. } => ParamReads::walk_all(ir, args, out),
            Expr::Method { receiver, args, .. } => {
                ParamReads::walk(ir, *receiver, out);
                ParamReads::walk_all(ir, args, out);
            }
            Expr::Block { stmts, value } => {
                for stmt in stmts {
                    ParamReads::walk_all(ir, &ir.stmt(*stmt).operands(), out);
                }
                if let Some(value) = value {
                    ParamReads::walk(ir, *value, out);
                }
            }
            Expr::Limit { x, f, seed, .. } => ParamReads::walk_all(ir, &[*x, *f].into_iter().chain(*seed).collect::<Vec<_>>(), out),
            Expr::Stim { mag, phase, .. } => ParamReads::walk_all(ir, &[*mag].into_iter().chain(*phase).collect::<Vec<_>>(), out),
            control @ (Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_)) => {
                ParamReads::walk_all(ir, &control.operands(), out);
                for stmt in control.statements() {
                    ParamReads::walk_all(ir, &ir.stmt(*stmt).operands(), out);
                }
            }
        }
    }

    /// Adds to `out` every param index each of `exprs` reads.
    fn walk_all(ir: &Ir, exprs: &[ExprId], out: &mut Vec<u32>) {
        for expr in exprs {
            ParamReads::walk(ir, *expr, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{BodySpec, Child, Constant, InstanceSpec, IrBuilder, Origin, Param, Real};

    use super::*;

    fn origin() -> Origin {
        Origin { file: "derive.rs".into(), line: 1, col: 1 }
    }

    fn param(name: &str) -> Param {
        Param { name: name.into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1.0))), check: None, structural: false, origin: origin() }
    }

    fn spec(name: &str, params: Vec<Param>, children: Vec<Child>) -> BodySpec {
        BodySpec { name: name.into(), origin: origin(), params, nodes: Vec::new(), branches: Vec::new(), limits: Vec::new(), stims: Vec::new(), locals: Vec::new(), analog: None, children }
    }

    fn placed(name: &str, body: piperine_ir::BodyId) -> Child {
        Child { name: name.into(), body, nodes: Vec::new(), params: Vec::new(), origin: origin() }
    }

    fn instance(path: &str, name: &str, body: piperine_ir::BodyId, parent: Option<piperine_ir::InstanceId>, params: Vec<Constant>) -> InstanceSpec {
        InstanceSpec { path: path.into(), name: name.into(), body, parent, nodes: Vec::new(), params, origin: origin() }
    }

    fn real(v: f64) -> Constant {
        Constant::Real(Real::new(v))
    }

    /// `Leaf { r }`, `Mid { rr } { m: Leaf { r: rr * 2.0 } }`, `Top { rr } { m: Mid { rr: top * 3.0, other: 7.0 } }`:
    /// the leaf derives from Mid and Mid derives from Top.
    fn chain() -> Ir {
        let mut b = IrBuilder::new();
        let leaf = b.body(spec("Leaf", vec![param("r")], Vec::new()));
        let mid = b.body(spec("Mid", vec![param("rr"), param("other")], vec![placed("l", leaf)]));
        let top = b.body(spec("Top", vec![param("t")], vec![placed("m", mid)]));
        let two = b.expr(Expr::Real(Real::new(2.0)), ValueType::F64, origin());
        let rr = b.expr(Expr::Param(0), ValueType::F64, origin());
        let doubled = b.expr(Expr::Binary { op: piperine_ir::BinaryOp::Mul, lhs: rr, rhs: two }, ValueType::F64, origin());
        b.child_param(mid, 0, 0, ChildValue::Expr(doubled));
        let three = b.expr(Expr::Real(Real::new(3.0)), ValueType::F64, origin());
        let t = b.expr(Expr::Param(0), ValueType::F64, origin());
        let tripled = b.expr(Expr::Binary { op: piperine_ir::BinaryOp::Mul, lhs: t, rhs: three }, ValueType::F64, origin());
        b.child_param(top, 0, 0, ChildValue::Expr(tripled));
        b.child_param(top, 0, 1, ChildValue::Const(Constant::Real(Real::new(7.0))));
        let t_i = b.instance(instance("Top", "Top", top, None, vec![real(1.0)]));
        let m_i = b.instance(instance("Top.m", "m", mid, Some(t_i), vec![real(3.0), real(7.0)]));
        b.instance(instance("Top.m.l", "l", leaf, Some(m_i), vec![real(6.0)]));
        b.finish(t_i)
    }

    #[test]
    fn a_derived_child_param_is_a_binding_of_its_parent() {
        // HII-18 AC 1: `rr` of `Top.m` is `t * 3.0` over the params of `Top`, and reads `t`
        let plan = DerivePlan::of(&chain());
        let first = &plan.bindings()[0];
        assert_eq!((first.parent.as_str(), first.child.as_str(), first.param.as_str()), ("Top", "Top.m", "rr"));
        assert_eq!(first.reads, ["t"]);
        assert_eq!(plan.readers("Top", "t"), [0]);
        assert!(plan.readers("Top", "other").is_empty());
        assert_eq!(plan.giving("Top.m", "rr").map(|b| b.k), Some(0));
        assert!(plan.giving("Top", "t").is_none(), "a param no placement gives is not derived");
    }

    #[test]
    fn a_chain_derives_top_down() {
        // HII-18 AC 1: a parent's binding comes before its children's, so applying them in plan
        // order gives the grandchild its value from the already recomputed middle param
        let plan = DerivePlan::of(&chain());
        let order: Vec<(&str, &str)> = plan.bindings().iter().map(|b| (b.child.as_str(), b.param.as_str())).collect();
        assert_eq!(order, [("Top.m", "rr"), ("Top.m.l", "r")]);
        assert_eq!(plan.bindings().iter().map(|b| b.k).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(plan.readers("Top.m", "rr"), [1]);
    }

    #[test]
    fn a_constant_placement_is_no_binding() {
        // `other: 7.0` is a `ChildValue::Const`: the instance table holds it, nothing derives it
        let plan = DerivePlan::of(&chain());
        assert!(plan.giving("Top.m", "other").is_none());
        assert_eq!(plan.bindings().len(), 2);
    }

    /// A body with one param `a` (index 0): the expressions the walk must see through.
    fn read_by(build: impl FnOnce(&mut IrBuilder, ExprId) -> Expr) -> Vec<u32> {
        let mut b = IrBuilder::new();
        let body = b.body(spec("Body", vec![param("a")], Vec::new()));
        let a = b.expr(Expr::Param(0), ValueType::F64, origin());
        let form = build(&mut b, a);
        let root = b.expr(form, ValueType::F64, origin());
        let instance = b.instance(instance("Body", "Body", body, None, vec![real(1.0)]));
        let ir = b.finish(instance);
        ParamReads::of(&ir, root)
    }

    #[test]
    fn every_form_that_holds_an_operand_reports_the_params_it_reads() {
        // F1: the forms the walk once skipped (a method call) and the ones it kept, each over `a`
        let constant = |b: &mut IrBuilder| b.expr(Expr::Real(Real::new(0.0)), ValueType::F64, origin());
        assert_eq!(read_by(|_, a| Expr::Method { method: piperine_ir::Method::Max, receiver: a, args: Vec::new() }), [0]);
        assert_eq!(read_by(|b, a| Expr::Method { method: piperine_ir::Method::Max, receiver: constant(b), args: vec![a] }), [0]);
        assert_eq!(read_by(|_, a| Expr::Cast { expr: a, to: ValueType::F64 }), [0]);
        assert_eq!(read_by(|_, a| Expr::Block { stmts: Vec::new(), value: Some(a) }), [0]);
        assert_eq!(read_by(|_, _| Expr::Real(Real::new(1.0))), Vec::<u32>::new());
    }
}

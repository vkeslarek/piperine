//! Scope: the static dependency sets and the affine test (IR-09). `Body::dependencies` on the
//! five hand-built device bodies (direct addends, `ddt` addends, `limit` and `ac_stim` sites,
//! `where` checks), reached through locals and function calls; the fixed order of each set;
//! `Body::is_affine` with one row per operator of the affine rule.

use piperine_ir_fixtures::devices;

use devices::{Devices, Ex};
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Deps, Env, ExprId,
    FunctionSpec, Ir, IrBuilder, Local, MathFn, NodeRole, Operator, Origin, Param, QuantityRole,
    Site, Stmt, Target, Terminal, UnaryOp, ValueType,
};

/// One device body alone in an IR, with a top instance on ground.
struct One;

impl One {
    fn ir(build: fn(&mut IrBuilder) -> BodyId, params: Vec<f64>) -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let body = build(&mut b);
        let top = Devices::alone(&mut b, body, "Top", 1, params);
        (b.finish(top), body)
    }
}

/// An expected set, written field by field.
struct Want;

impl Want {
    fn deps(nodes: &[u32], params: &[u32], env: &[Env], limits: &[u32], stims: &[u32]) -> Deps {
        Deps {
            nodes: nodes.to_vec(),
            params: params.to_vec(),
            env: env.to_vec(),
            limits: limits.to_vec(),
            stims: stims.to_vec(),
            delays: Vec::new(),
            noise_sources: Vec::new(),
            session_parameters: Vec::new(),
        }
    }
}

#[test]
fn dependencies_follow_locals_and_calls() {
    // Resistor: i(p, n) +> v(p, n) / r.
    let (ir, id) = One::ir(Devices::resistor, vec![1000.0]);
    let r = ir.body(id);
    assert_eq!(r.dependencies(Target::Flow(0)), Want::deps(&[0, 1], &[0], &[], &[], &[]));
    assert_eq!(r.dependencies(Target::Charge(0)), Deps::default());
    assert!(r.is_affine(0));
    // the `where r > 0.0` check reads r alone
    assert_eq!(r.dependencies(Target::Check(0)), Want::deps(&[], &[0], &[], &[], &[]));

    // Capacitor: i(p, n) +> ddt(c * v(p, n)).
    let (ir, id) = One::ir(Devices::capacitor, vec![1e-12]);
    let c = ir.body(id);
    assert_eq!(c.dependencies(Target::Charge(0)), Want::deps(&[0, 1], &[0], &[], &[], &[]));
    assert_eq!(c.dependencies(Target::Flow(0)), Deps::default());

    // Vsource: v(p, n) +> dc * source_scale() + ac_stim(ac); the potential branch's direct part.
    let (ir, id) = One::ir(Devices::vsource, vec![1.0, 0.0]);
    let v = ir.body(id);
    assert_eq!(v.dependencies(Target::Flow(0)), Want::deps(&[], &[0, 1], &[Env::SourceScale], &[], &[0]));
    assert_eq!(v.dependencies(Target::Charge(0)), Deps::default());
    assert_eq!(v.dependencies(Target::Stim(0)), Want::deps(&[], &[1], &[], &[], &[]));
    assert!(v.is_affine(0));

    // Isource: the same reads on a flow branch.
    let (ir, id) = One::ir(Devices::isource, vec![1.0, 0.0]);
    let i = ir.body(id);
    assert_eq!(i.dependencies(Target::Flow(0)), Want::deps(&[], &[0, 1], &[Env::SourceScale], &[], &[0]));

    // Diode: the direct part reads the limit site, not the nodes behind it; `Temperature`
    // reaches it only through the callee `vt`, `n` only through the local `nvt`.
    let (ir, id) = One::ir(Devices::diode, vec![1e-14, 1.0, 0.0]);
    let d = ir.body(id);
    let (is, n, cj0) = (0, 1, 2);
    assert_eq!(
        d.dependencies(Target::Flow(0)),
        Want::deps(&[], &[is, n], &[Env::Temperature, Env::Gmin, Env::TemperatureGiven], &[0], &[])
    );
    assert!(!d.is_affine(0));
    assert_eq!(d.dependencies(Target::Charge(0)), Want::deps(&[], &[cj0], &[], &[0], &[]));
    // The limit site: its input v(a, k), its function (pnjlim over nvt and vcrit) and its seed.
    assert_eq!(
        d.dependencies(Target::Limit(0)),
        Want::deps(&[0, 1], &[is, n], &[Env::Temperature, Env::TemperatureGiven], &[], &[])
    );
    // A param with no check depends on nothing.
    let (ir, id) = One::ir(Devices::vsource, vec![1.0, 0.0]);
    assert_eq!(ir.body(id).dependencies(Target::Check(0)), Deps::default());
}

/// A two-port body whose single direct addend is built by `value`; params `k0`, `k1`, `k2`;
/// one `limit` site and two `ac_stim` sites; locals 0..4 free for the row.
struct Row;

impl Row {
    const FILE: &'static str = "row.msl";

    fn origin() -> Origin {
        Origin { file: Self::FILE.into(), line: 1, col: 1 }
    }

    /// `value` returns the statements before the contribution and the direct addend.
    fn ir(value: impl FnOnce(&mut IrBuilder) -> (Vec<piperine_ir::StmtId>, ExprId)) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let (mut stmts, value) = value(&mut b);
        stmts.push(b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        }));
        let param = |name: &str| Param {
            name: name.into(),
            ty: ValueType::F64,
            default: None,
            check: None,
            structural: false,
            origin: Self::origin(),
        };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Self::origin() };
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
        let site = |name: &str| Site { name: name.into(), origin: Self::origin(), enabled: None };
        let body = b.body(BodySpec {
            name: "Row".into(),
            origin: Self::origin(),
            params: vec![param("k0"), param("k1"), param("k2")],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![site("l0"), site("l1")],
            stims: vec![site("s0"), site("s1")],
            locals: vec![local("x"), local("y"), local("new"), local("old")],
            analog: Some(stmts),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "Top", 1, vec![1.0, 2.0, 3.0]);
        b.finish(top)
    }

    fn affine(value: impl FnOnce(&mut IrBuilder) -> (Vec<piperine_ir::StmtId>, ExprId)) -> bool {
        let ir = Self::ir(value);
        ir.body(BodyId(0)).is_affine(0)
    }

    fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 1, 1)
    }

    fn v(e: &mut Ex<'_>) -> ExprId {
        e.probe(Terminal::Node(0), Terminal::Node(1))
    }

    /// `identity(x) = x`.
    fn identity(b: &mut IrBuilder) -> piperine_ir::FunctionId {
        let x = Self::ex(b).local(0);
        b.function(FunctionSpec {
            name: "identity".into(),
            origin: Self::origin(),
            params: vec![Local { name: "x".into(), ty: ValueType::F64, mutable: false }],
            ret: ValueType::F64,
            locals: vec![Local { name: "x".into(), ty: ValueType::F64, mutable: false }],
            body: x,
        })
    }
}

#[test]
fn dependency_sets_have_a_fixed_order() {
    // Reads made in reverse order: v(n, p), then k2, k0; source_scale(), gmin(), temperature();
    // ac_stim site 1 before site 0.
    let ir = Row::ir(|b| {
        let mut e = Row::ex(b);
        let v = e.probe(Terminal::Node(1), Terminal::Node(0));
        let k2 = e.param(2);
        let k0 = e.param(0);
        let a = e.binary(BinaryOp::Mul, v, k2);
        let a = e.binary(BinaryOp::Mul, a, k0);
        let s = e.env(Env::SourceScale);
        let g = e.env(Env::Gmin);
        let t = e.env(Env::Temperature);
        let a = e.binary(BinaryOp::Add, a, s);
        let a = e.binary(BinaryOp::Add, a, g);
        let a = e.binary(BinaryOp::Add, a, t);
        let one = e.real(1.0);
        let s1 = e.stim(1, one, None);
        let one = e.real(1.0);
        let s0 = e.stim(0, one, None);
        let a = e.binary(BinaryOp::Add, a, s1);
        (vec![], e.binary(BinaryOp::Add, a, s0))
    });
    let deps = ir.body(BodyId(0)).dependencies(Target::Flow(0));
    assert_eq!(deps.nodes, vec![0, 1]);
    assert_eq!(deps.params, vec![0, 2]);
    assert_eq!(deps.env, vec![Env::Temperature, Env::Gmin, Env::SourceScale]);
    assert_eq!(deps.limits, Vec::<u32>::new());
    assert_eq!(deps.stims, vec![0, 1]);
    // The same query twice gives the same value (the walk is memoized, not repeated differently).
    assert_eq!(ir.body(BodyId(0)).dependencies(Target::Flow(0)), deps);
}

#[test]
fn affine_means_probes_meet_only_linear_operators() {
    // `+`: v + v, and v + k0
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (Row::v(&mut e), Row::v(&mut e));
        (vec![], e.binary(BinaryOp::Add, l, r))
    }));
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (Row::v(&mut e), e.param(0));
        (vec![], e.binary(BinaryOp::Add, l, r))
    }));
    // `-`: k0 - v
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (e.param(0), Row::v(&mut e));
        (vec![], e.binary(BinaryOp::Sub, l, r))
    }));
    // unary `-`: -v
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let v = Row::v(&mut e);
        (vec![], e.unary(UnaryOp::Neg, v))
    }));
    // `*` by a constant, on either side: k0 * v, v * 2.0
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (e.param(0), Row::v(&mut e));
        (vec![], e.binary(BinaryOp::Mul, l, r))
    }));
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (Row::v(&mut e), e.real(2.0));
        (vec![], e.binary(BinaryOp::Mul, l, r))
    }));
    // `/` by a constant: v / (k0 * gmin()) — params and environment reads are constants
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let v = Row::v(&mut e);
        let (k, g) = (e.param(0), e.env(Env::Gmin));
        let d = e.binary(BinaryOp::Mul, k, g);
        (vec![], e.binary(BinaryOp::Div, v, d))
    }));
    // ... and through locals: let x = v; x * k0
    assert!(Row::affine(|b| {
        let v = Row::v(&mut Row::ex(b));
        let let_x = b.stmt(Stmt::Let { local: 0, init: v });
        let mut e = Row::ex(b);
        let (x, k) = (e.local(0), e.param(0));
        (vec![let_x], e.binary(BinaryOp::Mul, x, k))
    }));
    // `*` by a probe: v * v; also through a local
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (Row::v(&mut e), Row::v(&mut e));
        (vec![], e.binary(BinaryOp::Mul, l, r))
    }));
    assert!(!Row::affine(|b| {
        let v = Row::v(&mut Row::ex(b));
        let let_x = b.stmt(Stmt::Let { local: 0, init: v });
        let mut e = Row::ex(b);
        let (x, v) = (e.local(0), Row::v(&mut e));
        (vec![let_x], e.binary(BinaryOp::Mul, x, v))
    }));
    // `/` by a probe: k0 / v
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let (l, r) = (e.param(0), Row::v(&mut e));
        (vec![], e.binary(BinaryOp::Div, l, r))
    }));
    // `*` by an expression that reads a limit site: v * limit(v, |new, old| new, 0.0)
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let v = Row::v(&mut e);
        let x = Row::v(&mut e);
        let new = e.local(2);
        let f = e.closure(vec![2, 3], new);
        let seed = e.real(0.0);
        let lim = e.limit(0, x, f, Some(seed));
        (vec![], e.binary(BinaryOp::Mul, v, lim))
    }));
    // a call of a probe: identity(v)
    assert!(!Row::affine(|b| {
        let f = Row::identity(b);
        let mut e = Row::ex(b);
        let v = Row::v(&mut e);
        (vec![], e.call(f, vec![v]))
    }));
    // a call of constants is a constant: identity(k0) * v
    assert!(Row::affine(|b| {
        let f = Row::identity(b);
        let mut e = Row::ex(b);
        let k = e.param(0);
        let c = e.call(f, vec![k]);
        let v = Row::v(&mut e);
        (vec![], e.binary(BinaryOp::Mul, c, v))
    }));
    // a catalogue method of a probe: exp(v)
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let v = Row::v(&mut e);
        (vec![], e.math(MathFn::Exp, v))
    }));
    // an `if` on a constant: if k0 > 0.0 { v } else { -v }
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (k, z) = (e.param(0), e.real(0.0));
        let cond = e.binary(BinaryOp::Gt, k, z);
        let v = Row::v(&mut e);
        let v2 = Row::v(&mut e);
        let neg = e.unary(UnaryOp::Neg, v2);
        (vec![], e.if_f64(cond, v, neg))
    }));
    // an `if` on a probe: if v > 0.0 { v } else { 0.0 }
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let (v, z) = (Row::v(&mut e), e.real(0.0));
        let cond = e.binary(BinaryOp::Gt, v, z);
        let v = Row::v(&mut e);
        let z = e.real(0.0);
        (vec![], e.if_f64(cond, v, z))
    }));
    // an addend that reads no probe at all is affine (a constant): k0 * gmin()
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (k, g) = (e.param(0), e.env(Env::Gmin));
        (vec![], e.binary(BinaryOp::Mul, k, g))
    }));
}

/// `limit(v(p, n), |new, old| new, 0.0)` at `site`: the limited iterate of the branch potential.
struct Limited;

impl Limited {
    fn read(e: &mut Ex<'_>, site: u32) -> ExprId {
        let x = Row::v(e);
        let new = e.local(2);
        let f = e.closure(vec![2, 3], new);
        let seed = e.real(0.0);
        e.limit(site, x, f, Some(seed))
    }
}

#[test]
fn a_limit_read_and_a_constant_if_keep_the_class() {
    // IR-10 AC 1: a `limit` read stands for the limited iterate of its input, an ABI read
    // (abi.md A2): it is classed like a probe. `k0 * limit(v)` is affine...
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let k = e.param(0);
        let lim = Limited::read(&mut e, 0);
        (vec![], e.binary(BinaryOp::Mul, k, lim))
    }));
    // ... `limit(v) + v` is affine ...
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let lim = Limited::read(&mut e, 0);
        let v = Row::v(&mut e);
        (vec![], e.binary(BinaryOp::Add, lim, v))
    }));
    // ... and `limit(v) * limit(v)`, a product of two reads, is not.
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let l0 = Limited::read(&mut e, 0);
        let l1 = Limited::read(&mut e, 1);
        (vec![], e.binary(BinaryOp::Mul, l0, l1))
    }));
    // `gmin() * limit(v)` reads the environment and the site, not the probed nodes: the input is
    // the site's own dependency.
    let ir = Row::ir(|b| {
        let mut e = Row::ex(b);
        let g = e.env(Env::Gmin);
        let lim = Limited::read(&mut e, 0);
        (vec![], e.binary(BinaryOp::Mul, g, lim))
    });
    let body = ir.body(BodyId(0));
    assert!(body.is_affine(0));
    assert_eq!(body.dependencies(Target::Flow(0)), Want::deps(&[], &[], &[Env::Gmin], &[0], &[]));
    assert_eq!(body.dependencies(Target::Limit(0)), Want::deps(&[0, 1], &[], &[], &[], &[]));

    // IR-10 AC 2: an `if` whose condition reads no probe and no `limit` site joins its arms.
    // Both arms affine: affine.
    assert!(Row::affine(|b| {
        let mut e = Row::ex(b);
        let (k, z) = (e.param(0), e.real(0.0));
        let cond = e.binary(BinaryOp::Gt, k, z);
        let (v, k1) = (Row::v(&mut e), e.param(1));
        let then = e.binary(BinaryOp::Mul, k1, v);
        let other = Row::v(&mut e);
        (vec![], e.if_f64(cond, then, other))
    }));
    // One arm nonlinear: not affine.
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let (k, z) = (e.param(0), e.real(0.0));
        let cond = e.binary(BinaryOp::Gt, k, z);
        let v = Row::v(&mut e);
        let then = e.math(MathFn::Exp, v);
        let other = Row::v(&mut e);
        (vec![], e.if_f64(cond, then, other))
    }));
    // A condition on a `limit` read: not affine, though both arms are.
    assert!(!Row::affine(|b| {
        let mut e = Row::ex(b);
        let lim = Limited::read(&mut e, 0);
        let z = e.real(0.0);
        let cond = e.binary(BinaryOp::Gt, lim, z);
        let (v, w) = (Row::v(&mut e), Row::v(&mut e));
        (vec![], e.if_f64(cond, v, w))
    }));
}

#[test]
fn a_shared_expression_is_walked_once() {
    // `acc = acc + acc` 40 times on one id: 2^40 paths through 41 expressions.
    let ir = Row::ir(|b| {
        let mut e = Row::ex(b);
        let mut acc = Row::v(&mut e);
        for _ in 0..40 {
            acc = e.binary(BinaryOp::Add, acc, acc);
        }
        (vec![], acc)
    });
    let started = std::time::Instant::now();
    let body = ir.body(BodyId(0));
    assert_eq!(body.dependencies(Target::Flow(0)), Want::deps(&[0, 1], &[], &[], &[], &[]));
    assert!(body.is_affine(0));
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
}

#[test]
fn a_deep_expression_is_walked_without_native_stack() {
    // `v(p, n) + 1.0 + ... + 1.0`, 10 000 terms left-associated, asked on a 2 MiB thread (the
    // test harness default) in the debug profile: the walk must not grow the native stack.
    let deps = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let ir = Row::ir(|b| {
                let mut e = Row::ex(b);
                let mut acc = Row::v(&mut e);
                for _ in 0..10_000 {
                    let one = e.real(1.0);
                    acc = e.binary(BinaryOp::Add, acc, one);
                }
                (vec![], acc)
            });
            let body = ir.body(BodyId(0));
            (body.dependencies(Target::Flow(0)), body.is_affine(0))
        })
        .expect("spawn")
        .join()
        .expect("the walk returns");
    assert_eq!(deps, (Want::deps(&[0, 1], &[], &[], &[], &[]), true));
}

#[test]
fn a_doubling_call_chain_is_walked_once_per_signature() {
    // 25 functions: `f24(x) = x`, `f_i(x) = f_{i+1}(x) + f_{i+1}(x)`, `f0` called once from the
    // addend: 2^25 call paths, one per distinct (function, argument) pair to walk.
    let ir = Row::ir(|b| {
        let x = Local { name: "x".into(), ty: ValueType::F64, mutable: false };
        let function = |b: &mut IrBuilder, name: String, body: ExprId| {
            b.function(FunctionSpec {
                name,
                origin: Row::origin(),
                params: vec![x.clone()],
                ret: ValueType::F64,
                locals: vec![x.clone()],
                body,
            })
        };
        let mut e = Row::ex(b);
        let body = e.local(0);
        let mut callee = function(b, "f24".into(), body);
        for i in (0..24).rev() {
            let mut e = Row::ex(b);
            let (a, c) = (e.local(0), e.local(0));
            let (l, r) = (e.call(callee, vec![a]), e.call(callee, vec![c]));
            let sum = e.binary(BinaryOp::Add, l, r);
            callee = function(b, format!("f{i}"), sum);
        }
        let mut e = Row::ex(b);
        let v = Row::v(&mut e);
        (vec![], e.call(callee, vec![v]))
    });
    let started = std::time::Instant::now();
    let body = ir.body(BodyId(0));
    assert_eq!(body.dependencies(Target::Flow(0)), Want::deps(&[0, 1], &[], &[], &[], &[]));
    assert!(!body.is_affine(0));
    assert!(started.elapsed() < std::time::Duration::from_secs(1), "took {:?}", started.elapsed());
}

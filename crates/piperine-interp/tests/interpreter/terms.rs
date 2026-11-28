//! The `terms` function of a body (feature `ir-interp`, INT-03 AC 1 to 3, INT-02 AC 5, edge case
//! 4): one walk of the `analog` block writes each term as the signed sum of its addends, probes
//! reading pin differences and ground 0; every seeded partial is the dual derivative through
//! operators, methods, calls and locals, an unseeded one stays zero; `temperature()` comes from
//! the `TEMPERATURE` pin and `gmin()`, `source_scale()` from the ambients; a non-finite value is
//! written as it is; two equal calls write equal bytes. An affine `FLOW` term (INT-12) is written
//! as its constant plus the products of its pin partials folded from `0.0` in declared order, the
//! sum the assembly subtracts, so the resistor matches `g*vp - g*vn` bit for bit; a constant
//! `c ≠ 0` leaves a right side within one ulp of `c` (INT-12 AC 3). A `limit` site
//! reads its input's `VALUE` with derivative 1, its `x`, `f` and seed unevaluated (INT-10 AC 4).
//! A cell the function does not declare never steers its walk: dead code branching on an unread
//! `bool` or comparing an unread real runs no arm and the declared writes are exact (C45).

use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, Constant, Expr, InstanceSpec, IrBuilder, Local, NodeRef, NodeRole,
    Operator, Origin, Param, QuantityRole, Real, Stmt, Terminal, ValueType,
};

use piperine_ir_fixtures::devices;
use crate::models;

use devices::{Devices, Ex};
use models::{In, Models};
use piperine_device_fixtures::shapes::View;

/// The physical constants `vt()` folds (`std/analog.msl`).
const K_B: f64 = 1.380649e-23;
const Q: f64 = 1.602176634e-19;

/// Closed forms and hand-built bodies.
struct Lab;

impl Lab {
    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * a.abs().max(b.abs())
    }

    /// The index of the `terms` function of `body`.
    fn terms(m: &Models, body: &str) -> u32 {
        let names = View { shape: m.module(body).prototype() }.function_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        names.iter().position(|n| n == "terms").expect("a terms function") as u32
    }

    /// `Signs(p, n)`: `i(p, n) +> 3·v(p, n) − 1.0` then `i(p, n) +> −ddt(5·v(p, n))`, written as
    /// the addends `[3·v, −(1.0)]` and `[−ddt(5·v)]`.
    fn signs() -> Models {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "signs.msl", 3, 5);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let three = e.real(3.0);
        let scaled = e.binary(BinaryOp::Mul, three, v);
        let one = e.real(1.0);
        let v2 = e.probe(Terminal::Node(0), Terminal::Node(1));
        let five = e.real(5.0);
        let charge = e.binary(BinaryOp::Mul, five, v2);
        let first = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![
                Addend { operator: Operator::Direct, negated: false, value: scaled },
                Addend { operator: Operator::Direct, negated: true, value: one },
            ],
        });
        let second = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Ddt, negated: true, value: charge }],
        });
        let at = |line: u32| Origin { file: "signs.msl".into(), line, col: 1 };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at(1) };
        let body = b.body(BodySpec {
            name: "Signs".into(),
            origin: at(1),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![first, second]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "S", 1, vec![]);
        Models::of(b.finish(top))
    }

    /// `Mixed(p, n)`: branch 0 `i(p, n) +> v(p, n)·3 + 0.3` (affine), branch 1
    /// `i(p, ground) +> exp(v(p))` (not affine).
    fn mixed() -> Models {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "mixed.msl", 2, 5);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let three = e.real(3.0);
        let scaled = e.binary(BinaryOp::Mul, v, three);
        let c = e.real(0.3);
        let linear = e.binary(BinaryOp::Add, scaled, c);
        let vp = e.probe(Terminal::Node(0), Terminal::Ground);
        let curved = e.math(piperine_ir::MathFn::Exp, vp);
        let direct = |value| Addend { operator: Operator::Direct, negated: false, value };
        let first = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![direct(linear)] });
        let second = b.stmt(Stmt::Contribute { branch: 1, quantity: QuantityRole::Flow, addends: vec![direct(curved)] });
        let at = Origin { file: "mixed.msl".into(), line: 1, col: 1 };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at.clone() };
        let flow = |plus, minus| Branch { plus, minus, kind: BranchKind::Flow };
        let body = b.body(BodySpec {
            name: "Mixed".into(),
            origin: at.clone(),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![flow(Terminal::Node(0), Terminal::Node(1)), flow(Terminal::Node(0), Terminal::Ground)],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![first, second]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "M", 1, vec![]);
        Models::of(b.finish(top))
    }

    /// `Flag(p, n; flag: bool = true, k: f64 = 2.0)`: `let g = if flag { 1.0 } else { 2.0 };` and
    /// `let h = if k > 1.0 { 1.0 } else { 2.0 };`, both dead, then `i(p, n) +> v(p, n)`. Neither
    /// param reaches the write, so the `terms` function reads neither.
    fn flag() -> Models {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let flag = b.expr(Expr::Param(0), ValueType::Bool, Origin { file: "flag.msl".into(), line: 2, col: 13 });
        let mut e = Ex::new(&mut b, "flag.msl", 2, 5);
        let (one, two) = (e.real(1.0), e.real(2.0));
        let pick = e.if_f64(flag, one, two);
        let k = e.param(1);
        let threshold = e.real(1.0);
        let above = e.binary(BinaryOp::Gt, k, threshold);
        let (one, two) = (e.real(1.0), e.real(2.0));
        let pick_k = e.if_f64(above, one, two);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let let_g = b.stmt(Stmt::Let { local: 0, init: pick });
        let let_h = b.stmt(Stmt::Let { local: 1, init: pick_k });
        let direct = Addend { operator: Operator::Direct, negated: false, value: v };
        let contribute = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![direct] });
        let at = Origin { file: "flag.msl".into(), line: 1, col: 1 };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at.clone() };
        let param = |name: &str, ty, default| Param { name: name.into(), ty, default: Some(default), check: None, structural: false, origin: at.clone() };
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
        let body = b.body(BodySpec {
            name: "Flag".into(),
            origin: at.clone(),
            params: vec![param("flag", ValueType::Bool, Constant::Bool(true)), param("k", ValueType::F64, Constant::Real(Real::new(2.0)))],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![local("g"), local("h")],
            analog: Some(vec![let_g, let_h, contribute]),
            children: vec![],
        });
        let top = b.instance(InstanceSpec {
            path: "F".into(),
            name: "F".into(),
            body,
            parent: None,
            nodes: vec![NodeRef::Ground, NodeRef::Ground],
            params: vec![Constant::Bool(true), Constant::Real(Real::new(2.0))],
            origin: at,
        });
        let ir = b.finish(top);
        assert!(ir.check().is_empty(), "{:?}", ir.check());
        Models::of(ir)
    }

    /// The diode's `terms` reads: `[vd, is, n, cj0, temperature, temperature given, GMIN]`; the run set its temperature.
    fn diode_reads(vd: f64, is: f64, n: f64, cj0: f64, t: f64, gmin: f64) -> Vec<In> {
        [In::F64(vd), In::F64(is), In::F64(n), In::F64(cj0), In::F64(t), In::Byte(1), In::F64(gmin)].into()
    }
}

#[test]
fn evaluating_twice_writes_the_same_bytes() {
    let m = Models::all();
    let h = m.instance("Diode");
    let k = Lab::terms(&m, "Diode");
    let reads = Lab::diode_reads(0.62, 1e-14, 1.3, 2e-12, 310.0, 1e-12);
    let first = Models::eval(&h, k, &reads, None).expect("terms");
    let second = Models::eval(&h, k, &reads, None).expect("terms");
    assert_eq!(Models::bits(&first), Models::bits(&second));
    // a second instance of the body writes the same bytes too
    let other = m.instance("Diode");
    assert_eq!(Models::bits(&Models::eval(&other, k, &reads, None).expect("terms")), Models::bits(&first));
    // an unseeded partial is left zero
    let only_charge = Models::eval(&h, k, &reads, Some(&[0, 1])).expect("terms");
    assert_eq!(only_charge.jac[0], 0.0);
    assert_eq!(only_charge.jac[1], 2e-12);
    assert_eq!(only_charge.values, first.values, "the values do not depend on the seed");
}

#[test]
fn the_terms_function_sums_addends_with_signs() {
    // Signs at v(p, n) = 0.75: flow 3·0.75 − 1, charge −5·0.75.
    let s = Lab::signs();
    let h = s.instance("Signs");
    let out = Models::eval(&h, 0, &[In::F64(1.25), In::F64(0.5)], None).expect("terms");
    assert!(Lab::close(out.values[0], 3.0 * 0.75 - 1.0, 1e-12), "{}", out.values[0]);
    assert!(Lab::close(out.values[1], -5.0 * 0.75, 1e-12), "{}", out.values[1]);
    assert_eq!(out.jac, [3.0, -3.0, -5.0, 5.0]);

    // A probe to ground reads the node alone: Signs bound with n read as 0.
    let out = Models::eval(&h, 0, &[In::F64(2.0), In::F64(0.0)], None).expect("terms");
    assert!(Lab::close(out.values[0], 5.0, 1e-12));

    // The capacitor at v = 2, c = 1e-9: charge 2e-9, partials c and −c.
    let m = Models::all();
    let c = m.instance("Capacitor");
    let out = Models::eval(&c, Lab::terms(&m, "Capacitor"), &[In::F64(3.0), In::F64(1.0), In::F64(1e-9)], None).expect("terms");
    assert_eq!(out.values, [2e-9]);
    assert_eq!(out.jac, [1e-9, -1e-9]);

    // The diode's one contribution has a direct and a ddt addend: flow and charge.
    let d = m.instance("Diode");
    let out = Models::eval(&d, Lab::terms(&m, "Diode"), &Lab::diode_reads(0.6, 1e-14, 1.0, 1e-12, 300.15, 1e-12), None)
        .expect("terms");
    let nvt = 1.0 * (K_B * 300.15 / Q);
    let flow = 1e-14 * ((0.6 / nvt).exp() - 1.0) + 1e-12 * 0.6;
    assert!(Lab::close(out.values[0], flow, 1e-12), "{} vs {flow}", out.values[0]);
    assert!(Lab::close(out.values[1], 1e-12 * 0.6, 1e-12));
}

#[test]
fn seeded_partials_are_the_dual_derivatives() {
    // INT-03's independent test: the diode at vd = 0.6 V, 300.15 K. The partial against the
    // limit's value runs through `vt()` (a call), the locals `nvt` and `id`, `exp` and `/`.
    let m = Models::all();
    let d = m.instance("Diode");
    let k = Lab::terms(&m, "Diode");
    for (vd, n, t) in [(0.6, 1.0, 300.15), (0.45, 1.8, 250.0), (-0.3, 1.2, 400.0)] {
        let (is, cj0, gmin) = (1e-14, 3e-12, 1e-12);
        let out = Models::eval(&d, k, &Lab::diode_reads(vd, is, n, cj0, t, gmin), None).expect("terms");
        let vt = n * (K_B * t / Q);
        let flow = is * ((vd / vt).exp() - 1.0) + gmin * vd;
        let g = is * (vd / vt).exp() / vt + gmin;
        assert!(Lab::close(out.values[0], flow, 1e-12), "flow at {vd}: {} vs {flow}", out.values[0]);
        assert!(Lab::close(out.jac[0], g, 1e-12), "∂flow/∂vd at {vd}: {} vs {g}", out.jac[0]);
        assert!(Lab::close(out.jac[1], cj0, 1e-15), "∂charge/∂vd at {vd}");
    }

    // The resistor: ∂/∂p = 1/r, ∂/∂n = −1/r.
    let r = m.instance("Resistor");
    let out = Models::eval(&r, Lab::terms(&m, "Resistor"), &[In::F64(2.0), In::F64(0.5), In::F64(250.0)], None).expect("terms");
    assert!(Lab::close(out.values[0], 1.5 / 250.0, 1e-15));
    assert_eq!(out.jac, [1.0 / 250.0, -(1.0 / 250.0)]);
}

#[test]
fn environment_reads_come_from_pin_and_ambients() {
    let m = Models::all();
    // Vsource terms reads [p, n, b0.flow, ac.VALUE, dc, ac, SOURCE_SCALE].
    let v = m.instance("Vsource");
    let k = Lab::terms(&m, "Vsource");
    let reads = |scale: f64| [3.0, 0.5, 0.125, 0.0, 10.0, 0.0, scale].map(In::F64);
    for scale in [1.0, 0.5, 0.0] {
        let out = Models::eval(&v, k, &reads(scale), None).expect("terms");
        assert_eq!(out.values[0], 0.125, "b0.flow is the private pin");
        assert_eq!(out.values[1], 3.0 - 0.5 - 10.0 * scale, "b0.eq at source_scale {scale}");
        assert_eq!(out.jac, [1.0, 1.0, -1.0, -1.0], "(flow←f) (eq←p) (eq←n) (eq←stim)");
    }

    // The diode's temperature comes from its pin, gmin from the ambient: change each alone.
    let d = m.instance("Diode");
    let kd = Lab::terms(&m, "Diode");
    let flow = |t: f64, gmin: f64| Models::eval(&d, kd, &Lab::diode_reads(0.5, 1e-14, 1.0, 0.0, t, gmin), None).expect("terms").values[0];
    let at = |t: f64, gmin: f64| 1e-14 * ((0.5 / (K_B * t / Q)).exp() - 1.0) + gmin * 0.5;
    for (t, gmin) in [(300.0, 0.0), (350.0, 0.0), (300.0, 1e-3)] {
        assert!(Lab::close(flow(t, gmin), at(t, gmin), 1e-12), "T {t}, gmin {gmin}");
    }
    assert!(flow(350.0, 0.0) < flow(300.0, 0.0), "a warmer junction has a larger vt");
    assert!(Lab::close(flow(300.0, 1e-3) - flow(300.0, 0.0), 1e-3 * 0.5, 1e-9), "gmin·vd is added");
}

#[test]
fn a_non_finite_value_is_written_as_is() {
    // exp(100 V / vt) overflows: the flow is +inf, written without a fault (abi.md A4: the
    // solver fails the solve).
    let m = Models::all();
    let d = m.instance("Diode");
    let out = Models::eval(&d, Lab::terms(&m, "Diode"), &Lab::diode_reads(100.0, 1e-14, 1.0, 1e-12, 300.0, 1e-12), None)
        .expect("a non-finite value is not a fault");
    assert_eq!(out.values[0], f64::INFINITY);
    assert_eq!(out.values[1], 1e-12 * 100.0);
    // a NaN param is carried through too
    let out = Models::eval(&d, Lab::terms(&m, "Diode"), &Lab::diode_reads(0.5, f64::NAN, 1.0, 0.0, 300.0, 0.0), None)
        .expect("NaN is not a fault");
    assert!(out.values[0].is_nan());
}

/// A fixed-seed generator (xorshift64*), so the sweep is the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let bits = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (bits >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `[lo, hi)`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

#[test]
fn an_affine_term_is_constant_plus_folded_products() {
    // Mixed: the affine term is c + (0.0 + g_p·vp + g_n·vn) with c = 0.3 at pins 0, g = 3, −3
    // (not `(vp − vn)·3 + 0.3`, which rounds differently);
    // the exp term is evaluated at the iterate.
    let m = Lab::mixed();
    let h = m.instance("Mixed");
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut differs = 0;
    for _ in 0..200 {
        let (vp, vn) = (rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
        let out = Models::eval(&h, 0, &[In::F64(vp), In::F64(vn)], None).expect("terms");
        let c = (0.0 - 0.0) * 3.0 + 0.3;
        let fold = 0.0 + 3.0 * vp + -3.0 * vn;
        assert_eq!(out.values[0].to_bits(), (c + fold).to_bits(), "affine at ({vp}, {vn})");
        assert_eq!(out.values[1].to_bits(), vp.exp().to_bits(), "exp at the iterate");
        // partials: (b0←p) (b0←n) (b1←p)
        assert_eq!(out.jac, [3.0, -3.0, vp.exp()]);
        differs += usize::from((vp - vn) * 3.0 + 0.3 != c + fold);
    }
    assert!(differs > 0, "the sweep holds points where the two roundings differ");

    // Vsource: b0.flow = 0 + (0.0 + 1·f); b0.eq = c + (0.0 + 1·vp + (−1)·vn) with
    // c = 0 − 0 − (dc·scale + stim); its partial against the stimulus comes from the iterate.
    let all = Models::all();
    let v = all.instance("Vsource");
    let k = Lab::terms(&all, "Vsource");
    for _ in 0..200 {
        let (vp, vn, f, dc, scale) =
            (rng.range(-20.0, 20.0), rng.range(-20.0, 20.0), rng.range(-1.0, 1.0), rng.range(-10.0, 10.0), rng.next());
        let out = Models::eval(&v, k, &[vp, vn, f, 0.0, dc, 1.0, scale].map(In::F64), None).expect("terms");
        assert_eq!(out.values[0].to_bits(), (0.0 + (0.0 + 1.0 * f)).to_bits());
        let c = 0.0 - 0.0 - (dc * scale + 0.0);
        assert_eq!(out.values[1].to_bits(), (c + (0.0 + 1.0 * vp + -1.0 * vn)).to_bits(), "eq at ({vp}, {vn}, {dc}, {scale})");
        assert_eq!(out.jac, [1.0, 1.0, -1.0, -1.0]);
    }

    // The diode's flow is not affine: its value is the iterate's, as in the terms walk.
    let d = all.instance("Diode");
    let out = Models::eval(&d, Lab::terms(&all, "Diode"), &Lab::diode_reads(0.6, 1e-14, 1.0, 0.0, 300.15, 1e-12), None)
        .expect("terms");
    let nvt = 1.0 * (K_B * 300.15 / Q);
    let id = 1e-14 * ((0.6 / nvt).exp() - 1.0);
    assert_eq!(out.values[0].to_bits(), (id + 1e-12 * 0.6).to_bits());
}

#[test]
fn the_resistor_matches_the_hand_written_products_bit_for_bit() {
    let m = Models::all();
    let h = m.instance("Resistor");
    let k = Lab::terms(&m, "Resistor");
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for _ in 0..1000 {
        let (vp, vn) = (rng.range(-100.0, 100.0), rng.range(-100.0, 100.0));
        let r = 10f64.powf(rng.range(-3.0, 9.0));
        let out = Models::eval(&h, k, &[In::F64(vp), In::F64(vn), In::F64(r)], None).expect("terms");
        let g = 1.0 / r;
        let value = out.values[0];
        // the assembly's right side, −(v − Σ g_j·x_j) folded from 0.0, is exactly zero
        assert_eq!(value - (0.0 + g * vp + (-g) * vn), 0.0, "residue at ({vp}, {vn}, {r})");
        assert_eq!(value.to_bits(), (g * vp - g * vn).to_bits(), "({vp}, {vn}, {r})");
        assert_eq!(out.jac, [g, -g]);
    }
}

/// INT-12's bound on the right side: the assembly stamps `−(v − dot)` with `v = c ⊕ dot`. With
/// `c = 0` (the resistor above, a Vsource at `dc = 0`) the residue is exactly 0; with `c ≠ 0` it
/// is `(c ⊕ dot) ⊖ dot`, within one ulp of the row's magnitude `max(|c|, |dot|)` of `c` - no
/// float `v` gives `c` back exactly at every point.
#[test]
fn an_affine_constant_leaves_at_most_one_ulp_on_the_right_side() {
    let all = Models::all();
    let v = all.instance("Vsource");
    let k = Lab::terms(&all, "Vsource");
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut inexact = 0;
    for i in 0..2000 {
        let wide = i % 2 == 1;
        let (vp, vn) = if wide { (rng.range(-1e6, 1e6), rng.range(-20.0, 20.0)) } else { (rng.range(-20.0, 20.0), rng.range(-20.0, 20.0)) };
        let (dc, scale) = if wide { (rng.range(-1e-3, 1e3), rng.next()) } else { (rng.range(-10.0, 10.0), rng.next()) };
        let out = Models::eval(&v, k, &[vp, vn, 0.0, 0.0, dc, 1.0, scale].map(In::F64), None).expect("terms");
        // the assembly's fold over b0.eq's written partials (p, n, the stim at 0), from 0.0
        let dot = out.jac[1..].iter().zip([vp, vn, 0.0]).fold(0.0, |s, (g, x)| s + g * x);
        let c = -(dc * scale);
        let residue = out.values[1] - dot;
        let ulp = f64::from_bits(c.abs().max(dot.abs()).to_bits() + 1) - c.abs().max(dot.abs());
        assert!((residue - c).abs() <= ulp, "residue {residue:e} vs c = {c:e} at ({vp}, {vn}, {dc}, {scale}): ulp {ulp:e}");
        inexact += usize::from(residue != c);

        let out = Models::eval(&v, k, &[vp, vn, 0.0, 0.0, 0.0, 1.0, scale].map(In::F64), None).expect("terms");
        let dot = out.jac[1..].iter().zip([vp, vn, 0.0]).fold(0.0, |s, (g, x)| s + g * x);
        assert_eq!(out.values[1] - dot, 0.0, "c = 0 leaves no residue at ({vp}, {vn})");
    }
    assert!(inexact > 0, "the sweep holds points where c ⊕ dot ⊖ dot is not c");
}

#[test]
fn the_terms_function_reads_the_limit_value_untouched() {
    // Hold's `x` faults when evaluated (two closures compared); its terms function runs and
    // writes the held VALUE with partial 1, so `x`, `f` and the seed were not evaluated.
    let m = Models::hold(Some(0.25), true);
    let h = m.instance("Hold");
    let view = View { shape: m.module("Hold").prototype() };
    assert_eq!(view.function_names(), ["limit:y", "terms"]);
    let (reads, _, _) = view.function("terms");
    assert_eq!(reads.len(), 1, "terms reads the limit's VALUE alone: {reads:?}");
    for value in [0.42, -3.0, 0.0] {
        let out = Models::eval(&h, 1, &[In::F64(value)], None).expect("terms does not evaluate x");
        assert_eq!(out.values, [value]);
        assert_eq!(out.jac, [1.0]);
    }
    // the site's own function does evaluate x, and faults naming the form and its origin
    let err = Models::eval(&h, 0, &[In::F64(0.5), In::F64(0.0), In::F64(0.0), In::Byte(0), In::Byte(1)], None)
        .expect_err("x faults when evaluated");
    assert!(err.message.contains("hold.msl:3:14"), "{}", err.message);

    // The diode's terms read the VALUE, never the pins: the flow follows VALUE alone.
    let all = Models::all();
    let d = all.instance("Diode");
    let k = Lab::terms(&all, "Diode");
    let (reads, _, partials) = View { shape: all.module("Diode").prototype() }.function("terms");
    assert_eq!(reads[0].kind, piperine_abi::raw::cell::INPUT);
    assert!(partials.iter().all(|p| p.read == 0), "every partial is against the VALUE");
    let flow = |vd: f64| Models::eval(&d, k, &Lab::diode_reads(vd, 1e-14, 1.0, 0.0, 300.0, 0.0), None).expect("terms").values[0];
    assert!(Lab::close(flow(0.55), 1e-14 * ((0.55 / (K_B * 300.0 / Q)).exp() - 1.0), 1e-12));
}

#[test]
fn the_terms_function_reads_the_stim_value() {
    // Vsource: `v(p, n) +> dc * source_scale() + ac_stim(ac)`. The branch equation is
    // `v(p, n) - Σ`, so its partial against the stim's VALUE is -1; the stim's arguments are not evaluated.
    let m = Models::all();
    let h = m.instance("Vsource");
    let view = View { shape: m.module("Vsource").prototype() };
    assert_eq!(view.function("terms").2.len(), 4, "b0.flow against its pin; b0.eq against p, n and the stim");
    // reads: p, n, b0.flow, stim VALUE, dc, ac, source_scale
    let reads = [0.3, 0.1, 0.0, 0.4, 1.5, 9.0, 0.5].map(In::F64);
    let out = Models::eval(&h, 1, &reads, None).expect("terms");
    assert_eq!(out.jac[3], -1.0, "b0.eq against the stim VALUE");
    assert_eq!(out.jac[1..3], [1.0, -1.0], "b0.eq against p and n");
    assert!(Lab::close(out.values[1], 0.2 - (1.5 * 0.5 + 0.4), 1e-12), "{}", out.values[1]);
}

#[test]
fn an_undeclared_read_never_steers_the_walk() {
    // `flag` (bool) and `k` (real) reach no write: the terms function reads neither, the dead
    // `if`s over them run no arm, and the flow is v(p, n) with partials 1, -1.
    let m = Lab::flag();
    let k = Lab::terms(&m, "Flag");
    let (reads, _, _) = View { shape: m.module("Flag").prototype() }.function("terms");
    assert_eq!(reads.len(), 2, "the terms function reads the two pins only: {reads:?}");
    let h = m.instance("Flag");
    for (vp, vn) in [(0.75, -0.25), (-3.0, 2.0), (0.0, 0.0)] {
        let out = Models::eval(&h, k, &[In::F64(vp), In::F64(vn)], None).expect("an unread param in dead code is no fault");
        assert_eq!(out.values[0].to_bits(), (vp - vn).to_bits());
        assert_eq!(out.jac, [1.0, -1.0]);
    }
}

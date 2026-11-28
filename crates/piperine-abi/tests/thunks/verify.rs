//! The finite-difference verifier of declared partials (feature
//! `solver-abi-core`, SAC-10, abi.md §8): exact partials verify; a partial
//! off by 1 % is reported with its index, declared value and finite
//! difference; a point where a written limit is active is skipped.

use std::sync::{Arc, OnceLock};

use piperine_abi::caller::Handle;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, input, op, part, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_abi::verify::Verdict;

/// abi.md §11.1, the resistor: `i = v / r`.
struct Resistor;
/// abi.md §11.3, the capacitor: `q = c · v`.
struct Capacitor;
/// The capacitor with `∂q/∂v(n)` declared 1 % too large.
struct SkewedCapacitor;

static FLOW_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DDT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();

impl Device for Resistor {
    fn prototype() -> &'static Shape<'static> {
        CheckedDevices::shared(&FLOW_SHAPE, op::FLOW).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resistor, Arc::clone(CheckedDevices::shared(&FLOW_SHAPE, op::FLOW))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, v / r);
        cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

impl Device for Capacitor {
    fn prototype() -> &'static Shape<'static> {
        CheckedDevices::shared(&DDT_SHAPE, op::DDT).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capacitor, Arc::clone(CheckedDevices::shared(&DDT_SHAPE, op::DDT))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        CheckedDevices::capacitor_eval(cx, 1.0);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

impl Device for SkewedCapacitor {
    fn prototype() -> &'static Shape<'static> {
        CheckedDevices::shared(&DDT_SHAPE, op::DDT).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SkewedCapacitor, Arc::clone(CheckedDevices::shared(&DDT_SHAPE, op::DDT))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        CheckedDevices::capacitor_eval(cx, 1.01);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A limiter: pins a=0, k=1 (PORT); input I0 LIMIT "vd";
/// fn 0: reads [a, k, I0.LAST] writes [I0.PROPOSED, I0.LIMITED]
/// partials (0←0)=1 (0←1)=-1; LIMITED clamps PROPOSED to LAST ± 0.1.
struct Limiter;

impl Device for Limiter {
    fn prototype() -> &'static Shape<'static> {
        CheckedDevices::limiter_shape().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Limiter, Arc::clone(CheckedDevices::limiter_shape())))
    }
    fn eval(&self, _k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let (v, last) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, v);
        cx.set(1, v.clamp(last - 0.1, last + 0.1));
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn exact_partials_verify() {
    let r = Handle::create(CheckedDevices::leaked::<Resistor>("r"), &[]).unwrap();
    let c = Handle::create(CheckedDevices::leaked::<Capacitor>("c"), &[]).unwrap();
    for at in [[1.2, -0.3, 1e3], [0.0, 0.0, 47.0], [-5.0, 2.0, 0.5]] {
        assert_eq!(Verdict::of_partials(&r, 0, &CheckedDevices::point(&at), 1e-6), Ok(Verdict::Exact), "resistor at {at:?}");
    }
    for at in [[1.2, -0.3, 1e-12], [0.7, 0.1, 2e-9], [-3.0, 4.0, 1.0]] {
        assert_eq!(Verdict::of_partials(&c, 0, &CheckedDevices::point(&at), 1e-6), Ok(Verdict::Exact), "capacitor at {at:?}");
    }
}

#[test]
fn a_wrong_partial_is_reported_with_its_index() {
    let c = Handle::create(CheckedDevices::leaked::<SkewedCapacitor>("c_skew"), &[]).unwrap();
    let cap = 2e-9;
    let verdict = Verdict::of_partials(&c, 0, &CheckedDevices::point(&[0.7, 0.1, cap]), 1e-3).unwrap();
    let Verdict::Mismatch(found) = verdict else { panic!("a 1 % error is not found: {verdict:?}") };
    assert_eq!(found.len(), 1, "only partial 1 is wrong: {found:?}");
    let m = found[0];
    assert_eq!(m.partial, 1);
    assert_eq!(m.declared, -cap * 1.01);
    assert!((m.finite - -cap).abs() <= 1e-9 * cap, "the finite difference is the true -c: {}", m.finite);
}

#[test]
fn a_point_where_the_limit_is_active_is_skipped() {
    let h = Handle::create(CheckedDevices::leaked::<Limiter>("lim"), &[]).unwrap();
    // v = 0.9 against LAST = 0.5: clamped to 0.6, so LIMITED != PROPOSED.
    assert_eq!(Verdict::of_partials(&h, 0, &CheckedDevices::point(&[1.0, 0.1, 0.5]), 1e-6), Ok(Verdict::Skipped { limit: 0 }));
    // v = 0.55 against LAST = 0.5: inside the band, the partials are compared.
    assert_eq!(Verdict::of_partials(&h, 0, &CheckedDevices::point(&[0.6, 0.05, 0.5]), 1e-6), Ok(Verdict::Exact));
}

/// One port, f(x) with a declared df/dx: `NanJac` declares NaN; `WrongLog`
/// is ln(x) with a partial 5× too large, whose finite difference is NaN
/// below x = h.
struct NanJac;
struct WrongLog;

impl Device for NanJac {
    fn prototype() -> &'static Shape<'static> {
        CheckedDevices::one_port().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NanJac, Arc::clone(CheckedDevices::one_port())))
    }
    fn eval(&self, _k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let x = cx.f64(0);
        cx.set(0, x * x);
        cx.jac()[0] = f64::NAN;
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

impl Device for WrongLog {
    fn prototype() -> &'static Shape<'static> {
        CheckedDevices::one_port().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((WrongLog, Arc::clone(CheckedDevices::one_port())))
    }
    fn eval(&self, _k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let x = cx.f64(0);
        cx.set(0, x.ln());
        cx.jac()[0] = 5.0 / x;
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn a_non_finite_partial_is_a_mismatch() {
    let h = Handle::create(CheckedDevices::leaked::<NanJac>("nan"), &[]).unwrap();
    let verdict = Verdict::of_partials(&h, 0, &CheckedDevices::point(&[2.0]), 1e-6).unwrap();
    let Verdict::Mismatch(found) = verdict else { panic!("a NaN partial verified: {verdict:?}") };
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].partial, 0);
    assert!(found[0].declared.is_nan());
    assert!((found[0].finite - 4.0).abs() < 1e-6, "the finite difference is 2x = 4: {}", found[0].finite);

    // x − h < 0: the finite difference is NaN, which is not a pass either.
    let h = Handle::create(CheckedDevices::leaked::<WrongLog>("log"), &[]).unwrap();
    let verdict = Verdict::of_partials(&h, 0, &CheckedDevices::point(&[1e-7]), 1e-6).unwrap();
    let Verdict::Mismatch(found) = verdict else { panic!("a NaN finite difference verified: {verdict:?}") };
    assert_eq!(found[0].partial, 0);
    assert!(found[0].finite.is_nan());
    assert!(matches!(Verdict::of_partials(&h, 0, &CheckedDevices::point(&[2.0]), 1e-6), Ok(Verdict::Mismatch(_))));
}

/// The devices and points the verification tests check.
struct CheckedDevices;

impl CheckedDevices {
    /// The pin helper `pin`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// Two-terminal shape, `PARAM` first: k=0 (PARAM F64), p=1, n=2; term T0
    /// `op` {p, n}; fn 0: reads [p, n, k] writes [T0] partials (0←0) (0←1) (0←2).
    fn two_terminal(op: u32) -> ShapeRef {
        let mut s = ShapeBuf::new();
        let k = s.pin(PinSpec::discrete("k", role::PARAM, &F64));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t = s.term(TermSpec { name: "t", origin: "", op, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "f",
            reads: vec![CheckedDevices::pin(p), CheckedDevices::pin(n), CheckedDevices::pin(k)],
            writes: vec![Cell { kind: cell::TERM, index: t, part: part::VALUE }],
            partials: (0..3).map(|read| Partial { write: 0, to: 0, read, at: 0 }).collect(),
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shared helper `shared`.
    fn shared(cell: &'static OnceLock<ShapeRef>, op: u32) -> &'static ShapeRef {
        cell.get_or_init(|| CheckedDevices::two_terminal(op))
    }

    /// The capacitor eval helper `capacitor_eval`.
    fn capacitor_eval(cx: &mut Eval<'_>, skew: f64) {
        let (v, c) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, c * v);
        cx.jac().copy_from_slice(&[c, -c * skew, v]);
    }

    /// The limiter shape helper `limiter_shape`.
    fn limiter_shape() -> &'static ShapeRef {
        static SHAPE: OnceLock<ShapeRef> = OnceLock::new();
        SHAPE.get_or_init(|| {
            let mut s = ShapeBuf::new();
            let a = s.pin(PinSpec::continuous("a", role::PORT));
            let k = s.pin(PinSpec::continuous("k", role::PORT));
            let i0 = s.input(InputSpec { name: "vd", origin: "", kind: input::LIMIT });
            s.function(FunctionSpec {
                name: "limit",
                reads: vec![CheckedDevices::pin(a), CheckedDevices::pin(k), Cell { kind: cell::INPUT, index: i0, part: part::LAST }],
                writes: vec![
                    Cell { kind: cell::INPUT, index: i0, part: part::PROPOSED },
                    Cell { kind: cell::INPUT, index: i0, part: part::LIMITED },
                ],
                partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
                ..FunctionSpec::default()
            });
            s.freeze()
        })
    }

    /// The leaked helper `leaked`.
    fn leaked<D: Device>(name: &'static str) -> &'static Module {
        Box::leak(Box::new(module::<D>(name)))
    }

    /// The point helper `point`.
    fn point(values: &[f64]) -> Vec<Vec<u8>> {
        values.iter().map(|v| v.to_le_bytes().to_vec()).collect()
    }

    /// The one port helper `one_port`.
    fn one_port() -> &'static ShapeRef {
        static SHAPE: OnceLock<ShapeRef> = OnceLock::new();
        SHAPE.get_or_init(|| {
            let mut s = ShapeBuf::new();
            let p = s.pin(PinSpec::continuous("p", role::PORT));
            let t = s.term(TermSpec { name: "t", origin: "", op: op::FLOW, plus: p, minus: piperine_abi::raw::GROUND, abstol: DEFAULT, max: DEFAULT });
            s.function(FunctionSpec {
                name: "f",
                reads: vec![CheckedDevices::pin(p)],
                writes: vec![Cell { kind: cell::TERM, index: t, part: part::VALUE }],
                partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
                ..FunctionSpec::default()
            });
            s.freeze()
        })
    }
}

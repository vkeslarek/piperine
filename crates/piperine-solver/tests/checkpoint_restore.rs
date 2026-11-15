//! LRM-COVERS: 8.4.7
//! Rollback lifecycle contract tests (ABI-01..08) on the `Device` trait: the
//! solver owns every checkpoint (limit iterates and term histories), so a
//! rejected attempt leaves a device exactly where its last acceptance did.
//!
//! - T2: a transient that is forced through LTE rejections follows the
//!   analytic response: a rejected attempt left no trace in a history.
//! - T3: a DC homotopy fallthrough restores the limit state the failed
//!   plain-Newton attempt aged (observed through the limit's `AGE`) and
//!   reaches the operating point of the solve that never fell through.
//!
//! The trait defaults of the removed `checkpoint_state`/`restore_state` hooks,
//! the `SUPPORTS_ROLLBACK` flag and `limiting_report` are gone with them (see
//! `removed.txt`).
//!
//! lrm-conformance-wave3 (Phase 6, T39): §8.4.7 point 2c ("Having
//! calculated the solution for a given time, the analog engine can
//! either accept or reject that solution; it cannot calculate a solution
//! for a future time until it has accepted the solution for the current
//! time") is exactly this file's own subject —
//! `transient_reject_drives_restore_accept_discards` (T2) proves the
//! reject-then-restore half directly; the solver's own transient loop
//! never advances past a rejected step's time, satisfying the second half
//! structurally.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use std::cell::RefCell;
use std::result::Result;
use std::sync::{Arc, OnceLock};

use abi_devices::{add_capacitor, add_resistor, add_vsrc, node_id, vsine};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Policy, Solver, TransientAnalysisOptions};

/// Spec ABI-02/03/08: the sine-driven RC low-pass naturally overshoots its
/// LTE budget on roughly two dozen of its early steps, exercising the reject
/// path. Every accepted point lies on the analytic response of
/// `τ·dv/dt + v = sin(ωt)`, so no rejected attempt corrupted a history.
#[test]
fn transient_reject_drives_restore_accept_discards() {
    let (r, c, freq) = (1000.0_f64, 1e-7_f64, 1000.0_f64);
    let mut b = CircuitBuilder::new("rc-reject");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    b.device(
        vsine(),
        "vsin",
        &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(freq))],
        &[Port::Node(top.clone()), Port::Node(gnd.clone())],
    );
    add_resistor(&mut b, "r1", r, &top, &mid);
    add_capacitor(&mut b, "c1", c, &mid, &gnd);
    let circuit = b.build().expect("build");

    // The sine-driven RC low-pass naturally overshoots its LTE budget on
    // ~26 of its first steps under default tolerances, driving the reject
    // path without an artificial tolerance clamp (which would loop forever).
    let opts = TransientAnalysisOptions::new(1e-4, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    // ABI-02: at least one attempt was rejected and rolled back.
    assert!(res.stats.steps_rejected > 0, "expected rejections to drive the rollback");
    assert!(res.stats.steps_accepted > 0);

    // ABI-01/08: what was accepted is the response of a rolled-back state.
    let (tau, w) = (r * c, 2.0 * std::f64::consts::PI * freq);
    let wt = w * tau;
    let analytic = |t: f64| (((w * t).sin() - wt * (w * t).cos()) + wt * (-t / tau).exp()) / (1.0 + wt * wt);
    let mut worst = 0.0_f64;
    for step in res.iter() {
        let v = step.get_node(&node_id(&mid)).unwrap();
        worst = worst.max((v - analytic(step.time())).abs());
    }
    assert!(worst < 5e-3, "worst deviation from the analytic response {worst}");
}

// ── T3: DC homotopy retry ─────────────────────────────────────────────────
//
// Spec ABI-07: when a DC homotopy strategy falls through (failed attempt →
// next strategy), the retry starts from the pre-attempt state. A stiff diode
// whose plain-Newton attempt is starved of iterations (max_iter=4) falls
// through to gmin stepping, which converges.

const ORIGIN: &str = "checkpoint_restore.rs";
const THERMAL_VOLTAGE: f64 = 0.02585;

thread_local! {
    /// The `AGE` of the junction limit at every evaluation of its limit function.
    static AGES: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn cell_of(kind: u32, index: u32, part: u32) -> Cell {
    Cell { kind, index, part }
}

fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}

/// ngspice's junction limiter.
fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
    if vnew > vcrit && (vnew - vold).abs() > vt + vt {
        if vold > 0.0 {
            let arg = 1.0 + (vnew - vold) / vt;
            if arg > 0.0 { vold + vt * arg.ln() } else { vcrit }
        } else {
            vt * (vnew / vt).ln()
        }
    } else {
        vnew
    }
}

/// A diode with `pnjlim` on its junction voltage that writes down the limit's
/// `AGE` at each evaluation (`AGES`): the limit state the solver checkpoints.
/// pins: is_sat=0 (PARAM F64); p=1, n=2 (PORT)
/// inputs: I0 LIMIT "vd"; terms: T0 FLOW{p, n} "i" = is·(exp(vd/vt) − 1)
/// fn 0 "limit": reads [p, n, is, I0.LAST, I0.AGE, STATIC] writes [I0.PROPOSED, I0.LIMITED]
///               partials (0<-0)=1 (0<-1)=-1
/// fn 1 "i": reads [I0, is] writes [T0] partials (0<-0)=gd (0<-1)
struct AgedDiode;
static AGED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static AGED: OnceLock<Module> = OnceLock::new();
const IS_DEFAULT: [u8; 8] = 1.0e-14_f64.to_le_bytes();

fn aged_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let is = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &IS_DEFAULT, ..PinSpec::discrete("is_sat", role::PARAM, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let i0 = s.input(InputSpec { name: "vd", origin: ORIGIN, kind: input::LIMIT });
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "limit",
        origin: ORIGIN,
        reads: vec![
            cell_of(cell::PIN, p, part::VALUE),
            cell_of(cell::PIN, n, part::VALUE),
            cell_of(cell::PIN, is, part::VALUE),
            cell_of(cell::INPUT, i0, part::LAST),
            cell_of(cell::INPUT, i0, part::AGE),
            cell_of(cell::AMBIENT, ambient::STATIC, part::VALUE),
        ],
        writes: vec![cell_of(cell::INPUT, i0, part::PROPOSED), cell_of(cell::INPUT, i0, part::LIMITED)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![cell_of(cell::INPUT, i0, part::VALUE), cell_of(cell::PIN, is, part::VALUE)],
        writes: vec![cell_of(cell::TERM, t0, part::VALUE)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for AgedDiode {
    fn prototype() -> &'static Shape<'static> {
        AGED_SHAPE.get_or_init(aged_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AgedDiode, Arc::clone(AGED_SHAPE.get_or_init(aged_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let vt = THERMAL_VOLTAGE;
            let vcrit = vt * (vt / (std::f64::consts::SQRT_2 * cx.f64(2))).ln();
            let v = cx.f64(0) - cx.f64(1);
            let (age, is_static) = (cx.u8(4), cx.bool(5));
            AGES.with(|a| a.borrow_mut().push(age));
            let old = if age == 0 || (is_static && age == 1) { vcrit } else { cx.f64(3) };
            cx.set(0, v);
            cx.set(1, pnjlim(v, old, vt, vcrit));
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let (vd, is) = (cx.f64(0), cx.f64(1));
            let e = (vd / THERMAL_VOLTAGE).exp();
            cx.set(0, is * (e - 1.0));
            cx.jac().copy_from_slice(&[is * e / THERMAL_VOLTAGE, e - 1.0]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn diode_circuit() -> (piperine_solver::prelude::CircuitInstance, piperine_solver::abi::AnalogReference) {
    let mut b = CircuitBuilder::new("diode-homotopy");
    let gnd = b.ground();
    let src = b.node("src");
    let anode = b.node("anode");
    add_vsrc(&mut b, "v1", 1.0, &src, &gnd);
    add_resistor(&mut b, "r1", 1000.0, &src, &anode);
    b.device(AGED.get_or_init(|| module::<AgedDiode>("AgedDiode")), "d1", &[], &[
        Port::Node(anode.clone()),
        Port::Node(gnd),
    ]);
    (b.build().expect("build"), anode)
}

/// Spec ABI-07: plain Newton fails on the starved stiff diode and falls
/// through to gmin stepping; the fallthrough restores the junction limit's
/// state (its `AGE` reads 0 again), and gmin stepping converges to the
/// operating point the unstarved solve reaches.
#[test]
fn dc_homotopy_fallthrough_drives_restore_between_strategies() {
    let (circuit, anode) = diode_circuit();
    // max_iter=4 starves plain Newton on the stiff diode so it falls through
    // to gmin stepping (which converges from the easy high-gmin start).
    let policy = Policy { max_iter: 4, ..Policy::default() };
    let mut solver = Solver::new(circuit).with_policy(policy).build();
    AGES.with(|a| a.borrow_mut().clear());
    let res = solver.dc().unwrap().solve().unwrap();
    let ages = AGES.with(|a| a.borrow().clone());

    // Plain Newton fell through to gmin stepping (the homotopy took over).
    assert_eq!(
        res.stats.homotopy_strategy.as_deref(),
        Some("gmin-stepping"),
        "plain Newton must fall through so the homotopy restore fires"
    );
    // ABI-07: the fallthrough restored the limit state the plain attempt left:
    // its four iterations aged the junction limit, and the first evaluation of
    // gmin stepping reads it unevaluated again (AGE 0), as before the attempt.
    // The converging attempt is not restored: no later evaluation reads 0.
    assert!(ages.len() > 4, "{ages:?}");
    assert_eq!(ages[0], 0, "the plain attempt starts on an unevaluated limit: {ages:?}");
    assert!(ages[1..4].iter().all(|a| *a > 0), "the plain attempt ages the limit: {ages:?}");
    assert_eq!(ages[4], 0, "the fallthrough restored the limit state before gmin stepping: {ages:?}");
    assert!(ages[5..].iter().all(|a| *a > 0), "the converging attempt is not restored: {ages:?}");
    let starved = res.get_node(&node_id(&anode)).unwrap();

    let (circuit, anode) = diode_circuit();
    let mut solver = Solver::new(circuit).build();
    let plain = solver.dc().unwrap().solve().unwrap().get_node(&node_id(&anode)).unwrap();
    assert!((starved - plain).abs() < 1e-4, "after the fallthrough {starved} V vs the plain solve {plain} V");
}

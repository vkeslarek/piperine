//! Stamp-perturbation fallback for `∂F/∂p` (dv-gradients T6, DVG-06/07):
//! devices whose function declares no partial against a param get their
//! parameter derivative by differencing two evaluations at the **same**
//! operating point (no re-solve), coexisting with devices that declare it in
//! one circuit, and the collector names every instance that needed the
//! fallback.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::partials::PartialsCheck;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port, Stamp};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context};
use piperine_solver::prelude::{ParamRequest, ParamStampCollector, SensitivityEngine};

const ORIGIN: &str = "sensitivity_fallback.rs";
const G_DEFAULT: [u8; 8] = 1.0e-3_f64.to_le_bytes();

struct Gan;
static GAN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GAN_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Gan {
    fn prototype() -> &'static Shape<'static> {
        GAN_SHAPE.get_or_init(|| SensitivityFallbackCases::conductor_shape(true)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gan, Arc::clone(GAN_SHAPE.get_or_init(|| SensitivityFallbackCases::conductor_shape(true)))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        SensitivityFallbackCases::conductor_eval(cx, k);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

struct Gfb;
static GFB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GFB_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Gfb {
    fn prototype() -> &'static Shape<'static> {
        GFB_SHAPE.get_or_init(|| SensitivityFallbackCases::conductor_shape(false)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gfb, Arc::clone(GFB_SHAPE.get_or_init(|| SensitivityFallbackCases::conductor_shape(false)))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        SensitivityFallbackCases::conductor_eval(cx, k);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ── analytic and fallback coexist; both contribute; fallback named ───────────

#[test]
fn analytic_and_fallback_paths_coexist_and_the_fallback_is_named() {
    let (mut circuit, n1, _n2) = SensitivityFallbackCases::circuit();
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let op = SensitivityFallbackCases::op_vector(&circuit, &dc);

    let mut collector = ParamStampCollector::new(&mut circuit, Context::default());

    let an = collector.collect(&ParamRequest { label: "gan".into(), param: "g".into() }, &op).unwrap();
    let fb = collector.collect(&ParamRequest { label: "gfb".into(), param: "g".into() }, &op).unwrap();

    // Engine attribution (DVG-07).
    assert_eq!(an.engine, SensitivityEngine::Analytic, "gan declares its param partial");
    assert_eq!(fb.engine, SensitivityEngine::StampPerturbation, "gfb declares none → fallback");

    // Both paths deliver ∂F/∂g = V(n1,n2) = 1.0 at node n1 (the gnd terminal is
    // ground for the fallback, so it lands only on n1 — the value is the check).
    let an_v = SensitivityFallbackCases::rhs_at(&an.stamps, &n1);
    let fb_v = SensitivityFallbackCases::rhs_at(&fb.stamps, &n1);
    assert!((an_v - 1.0).abs() < 1e-6, "analytic ∂F(n1)/∂g = V = 1.0, got {an_v}");
    assert!((fb_v - 1.0).abs() < 1e-3, "fallback ∂F(n1)/∂g = V = 1.0 (FD), got {fb_v}");

    // The collector names the fallback instance (DVG-07) — never a silent swap.
    assert!(collector.fallback_users.contains("gfb"), "fallback user reported by label");
    assert!(!collector.fallback_users.contains("gan"), "the analytic instance is not a fallback user");
}

// ── no re-solve: the operating point is untouched after collecting ───────────

#[test]
fn the_fallback_does_not_re_solve_the_operating_point() {
    let (mut circuit, n1, _n2) = SensitivityFallbackCases::circuit();
    let dc_before = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let op = SensitivityFallbackCases::op_vector(&circuit, &dc_before);
    let v_before = dc_before.get_node(&CircuitBuilding::node_id(&n1)).unwrap();

    let mut collector = ParamStampCollector::new(&mut circuit, Context::default());
    let _ = collector.collect(&ParamRequest { label: "gfb".into(), param: "g".into() }, &op).unwrap();

    // Re-solving yields the same operating point: nothing the collector did
    // perturbed the converged solution (a re-solve would have been visible as
    // a changed voltage if state had been corrupted).
    let dc_after = circuit.dc(Context::default()).unwrap().solve().expect("dc re-solve");
    let v_after = dc_after.get_node(&CircuitBuilding::node_id(&n1)).unwrap();

    // The parameter was restored (no residue) — the adjoint economy survives
    // because the fallback re-stamps only, never re-solves. The `g` the
    // re-solve's accepted solution read is the device's `VAR` watch.
    let bytes = circuit.bound().analog.var("gfb", "g").expect("VAR g").to_vec();
    let g_after = f64::from_le_bytes(bytes[..8].try_into().expect("an F64"));
    assert!((g_after - 1e-3).abs() < 1e-15, "param restored after perturbation: {g_after}");

    assert!((v_after - v_before).abs() < 1e-9, "operating point unchanged: {v_before} vs {v_after}");
}

// ── a host-written param is differenced around, and restored to, its written value ──

#[test]
fn the_fallback_restores_a_host_written_param() {
    let (mut circuit, n1, _n2) = SensitivityFallbackCases::circuit();
    circuit.set("gfb", "g", ParamValue::Real(2e-3)).expect("host write");
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let op = SensitivityFallbackCases::op_vector(&circuit, &dc);

    let mut collector = ParamStampCollector::new(&mut circuit, Context::default());
    let fb = collector.collect(&ParamRequest { label: "gfb".into(), param: "g".into() }, &op).unwrap();
    let fb_v = SensitivityFallbackCases::rhs_at(&fb.stamps, &n1);
    assert!((fb_v - 1.0).abs() < 1e-3, "fallback ∂F(n1)/∂g = V = 1.0 (FD), got {fb_v}");

    circuit.dc(Context::default()).unwrap().solve().expect("dc re-solve");
    let bytes = circuit.bound().analog.var("gfb", "g").expect("VAR g").to_vec();
    let g_after = f64::from_le_bytes(bytes[..8].try_into().expect("an F64"));
    assert!((g_after - 2e-3).abs() < 1e-15, "the written value is restored, not the default: {g_after}");
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("Gan", GAN_MODULE.get_or_init(|| module::<Gan>("Gan")));
    PartialsCheck::verify_every_function("Gfb", GFB_MODULE.get_or_init(|| module::<Gfb>("Gfb")));
}

/// The helpers of the sensitivity_fallback tests.
pub struct SensitivityFallbackCases;

impl SensitivityFallbackCases {
    /// The cell of helper `cell_of`.
    fn cell_of(kind: u32, index: u32) -> Cell {
        Cell { kind, index, part: part::VALUE }
    }

    /// A parametric conductor `I = g·V(p, n)` that shows the `g` it reads as a
    /// `VAR` watch. `Gan` declares the partial against its `g` param (the declared
    /// route); `Gfb` declares only the partials against its ports, so its `∂F/∂g`
    /// takes the stamp-perturbation fallback.
    ///
    /// pins: g=0 (PARAM F64, 1e-3); p=1, n=2 (PORT)
    /// terms: T0 FLOW{p, n}; watches: W0 VAR "g"
    /// fn 0 "i": reads [p, n, g] writes [T0] partials (0<-p) (0<-n) [(0<-g) when declared]
    /// fn 1 "show": reads [g] writes [W0]
    fn conductor_shape(declared: bool) -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &G_DEFAULT, ..PinSpec::discrete("g", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let w0 = s.watch(WatchSpec { name: "g", origin: ORIGIN, kind: watch::VAR, layout: &F64, event: DEFAULT, ttol: DEFAULT, etol: DEFAULT });
        let mut partials = vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }];
        if declared {
            partials.push(Partial { write: 0, to: 0, read: 2, at: 0 });
        }
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![SensitivityFallbackCases::cell_of(cell::PIN, p), SensitivityFallbackCases::cell_of(cell::PIN, n), SensitivityFallbackCases::cell_of(cell::PIN, 0)],
            writes: vec![SensitivityFallbackCases::cell_of(cell::TERM, t0)],
            partials,
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "show",
            origin: ORIGIN,
            reads: vec![SensitivityFallbackCases::cell_of(cell::PIN, 0)],
            writes: vec![SensitivityFallbackCases::cell_of(cell::WATCH, w0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The conductor eval helper `conductor_eval`.
    fn conductor_eval(cx: &mut Eval, k: u32) {
        if k == 1 {
            let g = cx.f64(0);
            cx.set(0, g);
            return;
        }
        let (vp, vn, g) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, g * vp - g * vn);
        let jac = cx.jac();
        jac[0] = g;
        jac[1] = -g;
        if jac.len() > 2 {
            // d(g·V)/dg = V(p, n): the declared derivative.
            jac[2] = vp - vn;
        }
    }

    /// 1 V across two parallel conductors: one declaring its param partial, one
    /// not. Each sees V(n1, gnd) = 1.0 → ∂(g·V)/∂g = 1.0.
    fn circuit() -> (CircuitInstance, AnalogReference, AnalogReference) {
        let mut b = CircuitBuilder::new("t6");
        let n2 = b.ground();
        let n1 = b.node("n1");
        CircuitBuilding::add_voltage_source(&mut b, "v", 1.0, &n1, &n2);
        let ports = [Port::Node(n1.clone()), Port::Node(n2.clone())];
        b.device(GAN_MODULE.get_or_init(|| module::<Gan>("Gan")), "gan", &[("g", ParamValue::Real(1e-3))], &ports);
        b.device(GFB_MODULE.get_or_init(|| module::<Gfb>("Gfb")), "gfb", &[("g", ParamValue::Real(1e-3))], &ports);
        (b.build().expect("build"), n1, n2)
    }

    /// The op vector helper `op_vector`.
    fn op_vector(circuit: &CircuitInstance, dc: &piperine_solver::prelude::DcAnalysisResult) -> Vec<f64> {
        let pairs: Vec<(usize, f64)> = circuit
            .netlist()
            .all_references()
            .iter()
            .filter_map(|r| Some((r.idx()?, dc.get(r.variable().clone())?)))
            .collect();
        let n = pairs.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
        let mut op = vec![0.0; n];
        for (i, v) in pairs {
            op[i] = v;
        }
        op
    }

    /// Sum the `Stamp::Rhs` values whose reference matches `node` (the ∂F_node/∂g).
    fn rhs_at(stamps: &[Stamp<AnalogReference, f64>], node: &AnalogReference) -> f64 {
        stamps
            .iter()
            .filter_map(|s| match s { Stamp::Rhs(r, v) if r.idx() == node.idx() => Some(*v), _ => None })
            .sum()
    }
}

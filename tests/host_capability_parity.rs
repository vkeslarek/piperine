//! T30 (DVT-37) — capability parity, proven: the feature's hard success
//! criterion. Four device families — a `transition`-driven source, a
//! pnjlim junction, a `table` nonlinearity, and a `cross`-triggered
//! element — each written **both** in PHDL (reusing
//! `piperine-ir/tests/cross_check_*.rs`'s own proven fixture shapes,
//! T6's obligation to this feature) **and** as a Rust host device (T14-T18's
//! library, calling `piperine-ir`'s extracted operators/limiter/table/
//! events directly — the same functions T6 already proved identical to
//! codegen's JIT copy). Each pair is solved through a real circuit and
//! its **values** are asserted equal, not "both ran".
//!
//! **`codegen-simplification` extends this set rather than replacing it**
//! (that follow-on feature deletes the `piperine-ir/tests/cross_check_*`
//! fixtures once codegen delegates to the crate instead of carrying its own
//! copy — this file's PHDL⟷host-device comparisons are untouched by that;
//! they compare two different *bindings* of the same crate, not two
//! implementations).
//!
//! Any PHDL capability not reachable from the host is named where it
//! matters: `.disto`, noise, and full digital multi-phase participation are
//! not mirrored by `HostElement` (see `piperine-api/src/device/
//! host_element.rs`'s header) — a host device that needs them implements
//! `Element` directly (level 0's escape hatch), a scoped promise, not an
//! eroding one.

use piperine_api::device::{Ctx, HostAdapter, HostElement, StampSink, StateView};
use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_ir::events::{CrossDir, EventDetector, Trigger};
use piperine_ir::limit::pnjlim;
use piperine_ir::ops::Operator;
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, Context, DcAnalysisState, DigitalDevice, Element, ElementCapabilities,
    Introspect, Stamp, TransientAnalysisContext, TransientAnalysisState, UnknownAllocator,
};
use piperine_solver::prelude::{CircuitBuilder, TransientAnalysisOptions};

fn run_phdl_transient(src: &str, top: &str, probe_net: &str, stop: f64, dt: f64) -> Vec<(f64, f64)> {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped(top).expect("circuit builds");
    let probe = info.nets.get(probe_net).expect("probe net").clone();
    let opts = TransientAnalysisOptions::new(stop, dt);
    let result = circuit.transient(opts, Context::default()).unwrap().solve().expect("transient completes");
    (0..result.len())
        .filter_map(|i| {
            let step = result.get(i)?;
            let v = step.get_node(&probe)?;
            Some((step.time(), v))
        })
        .collect()
}

fn run_host_transient(builder: CircuitBuilder, probe: AnalogReference, stop: f64, dt: f64) -> Vec<(f64, f64)> {
    let mut circuit = builder.build().expect("host circuit builds");
    let opts = TransientAnalysisOptions::new(stop, dt);
    let result = circuit.transient(opts, Context::default()).unwrap().solve().expect("transient completes");
    (0..result.len())
        .filter_map(|i| {
            let step = result.get(i)?;
            let v = step.get(probe.clone())?;
            Some((step.time(), v))
        })
        .collect()
}

/// An ideal two-terminal voltage force `V(p,n) = value`, via a branch
/// unknown — the same stamping shape codegen emits for a PHDL `<-`
/// assignment (matches the `Vsrc` pattern `piperine-python/src/device.rs`'s
/// own tests use), so a host device reproduces the ideal-force circuit
/// topology exactly rather than approximating it with a large conductance.
struct IdealForce<F: FnMut(f64) -> f64 + Send + Sync> {
    p: AnalogReference,
    n: AnalogReference,
    branch: Option<AnalogReference>,
    f: F,
}
impl<F: FnMut(f64) -> f64 + Send + Sync> AnalogDevice for IdealForce<F> {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch = Some(alloc.branch("force", "i"));
    }
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps(0.0)
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps(t.time)
    }
}
impl<F: FnMut(f64) -> f64 + Send + Sync> IdealForce<F> {
    fn stamps(&mut self, t: f64) -> Vec<Stamp<AnalogReference, f64>> {
        let b = self.branch.clone().unwrap();
        let v = (self.f)(t);
        vec![
            Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
            Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
            Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
            Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
            Stamp::Rhs(b, v),
        ]
    }
}
impl<F: FnMut(f64) -> f64 + Send + Sync> DigitalDevice for IdealForce<F> {}
impl<F: FnMut(f64) -> f64 + Send + Sync> Introspect for IdealForce<F> {}
impl<F: FnMut(f64) -> f64 + Send + Sync> Element for IdealForce<F> {
    fn name(&self) -> &str {
        "ideal_force"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

// ── 1. A `transition`-driven source ────────────────────────────────────────

/// PHDL: identical to `piperine-ir/tests/cross_check_operators.rs`'s
/// own proven fixture. Host: `Operator::transition` driven directly by an
/// `IdealForce`, `accept`-ed at every accepted step — the same lag rule
/// T6 already documented (the residual reads the *committed* state).
#[test]
fn transition_driven_source_matches_between_phdl_and_a_host_device() {
    let (td, tr, tf) = (0.0, 1e-3, 1.2e-3);
    let (stop, dt) = (3e-3, 1e-4);

    let phdl = format!(
        "module Xition(inout electrical p, inout electrical n);
    parameter real td = {td};
    parameter real tr = {tr};
    parameter real tf = {tf};
    analog V(p, n) <+ transition($abstime, td, tr, tf);
endmodule
module Top();
    electrical gnd, out;
    Xition x1 (.p(out), .n(gnd));
endmodule
"
    );
    let phdl_run = run_phdl_transient(&phdl, "Top", "out", stop, dt);
    assert!(phdl_run.len() > 10, "non-trivial accepted-point sequence");

    // `transition`'s state must be committed **once per accepted step,
    // never inside Newton** (`ideal.md` §3.9 A's invariant, T2's own
    // extraction rule) — `IdealForce`'s plain closure evaluates on every
    // `load_transient` call, which happens once per Newton iteration, so
    // this device implements `HostElement` directly instead: `load_dc`/
    // `load_transient` read the already-committed value only,
    // `accept_timestep` is the one place `Operator::accept` is called.
    struct XitionHost {
        p: AnalogReference,
        n: AnalogReference,
        branch: Option<AnalogReference>,
        op: Operator,
        committed: f64,
    }
    impl HostElement for XitionHost {
        fn name(&self) -> &str {
            "xition_host"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG
                | ElementCapabilities::LOADS_DC
                | ElementCapabilities::LOADS_TRAN
                | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
        }
        fn allocate_unknowns(&mut self, alloc: &mut piperine_solver::abi::UnknownAllocator<'_>) {
            self.branch = Some(alloc.branch("xition", "i"));
        }
        fn load_dc(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            self.stamp(sink);
        }
        fn load_transient(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            self.stamp(sink);
        }
        fn accept_timestep(&mut self, _state: &StateView<'_>, t: f64) {
            self.committed = self.op.accept(t, t);
        }
    }
    impl XitionHost {
        fn stamp(&mut self, sink: &mut StampSink) {
            let b = self.branch.clone().unwrap();
            sink.g(&self.p, &b, 1.0);
            sink.g(&self.n, &b, -1.0);
            sink.g(&b, &self.p, 1.0);
            sink.g(&b, &self.n, -1.0);
            sink.i(&b, self.committed);
        }
    }

    let mut builder = CircuitBuilder::new("host_xition");
    let gnd = builder.ground();
    let out = builder.node("out");
    builder.element(Box::new(HostAdapter(XitionHost {
        p: out.clone(),
        n: gnd.clone(),
        branch: None,
        op: Operator::transition(td, tr, tf, 0.0),
        committed: 0.0,
    })));
    let host_run = run_host_transient(builder, out, stop, dt);

    assert_eq!(host_run.len(), phdl_run.len(), "same accepted-point count");
    for ((t_p, v_p), (t_h, v_h)) in phdl_run.iter().zip(host_run.iter()) {
        assert!((t_p - t_h).abs() < 1e-12, "accepted times must match: phdl={t_p} host={t_h}");
        assert!((v_p - v_h).abs() < 1e-9, "t={t_p}: phdl={v_p} host={v_h}");
    }
}

// ── 2. A `table` nonlinearity ────────────────────────────────────────────

const XS: [f64; 4] = [0.0, 1e-3, 2e-3, 3e-3];
const YS: [f64; 4] = [0.0, 5.0, 1.0, 5.0];

/// PHDL: identical to `cross_check_table.rs`'s fixture. Host:
/// `piperine_ir::behavior::table::Table::value` driving an
/// `IdealForce` directly — `table` is a pure function of `t` here (no
/// accept-lag, unlike `transition`/`cross`).
#[test]
fn table_nonlinearity_matches_between_phdl_and_a_host_device() {
    let (stop, dt) = (4e-3, 2e-4);
    let phdl = format!(
        "module TableLookup(inout electrical p, inout electrical n);
    analog V(p, n) <+ table($abstime, {{{}}}, {{{}}});
endmodule
module Top();
    electrical gnd, out;
    TableLookup t1 (.p(out), .n(gnd));
endmodule
",
        XS.map(|x| x.to_string()).join(", "),
        YS.map(|y| y.to_string()).join(", "),
    );
    let phdl_run = run_phdl_transient(&phdl, "Top", "out", stop, dt);
    assert!(phdl_run.len() > 10, "non-trivial accepted-point sequence");

    let table = piperine_ir::behavior::table::Table::new(XS.to_vec(), YS.to_vec()).unwrap();
    let mut builder = CircuitBuilder::new("host_table");
    let gnd = builder.ground();
    let out = builder.node("out");
    let force = IdealForce { p: out.clone(), n: gnd.clone(), branch: None, f: move |t: f64| table.value(t) };
    builder.element(Box::new(force));
    let host_run = run_host_transient(builder, out, stop, dt);

    assert_eq!(host_run.len(), phdl_run.len(), "same accepted-point count");
    let mut saw_high_clamp = false;
    for ((t_p, v_p), (t_h, v_h)) in phdl_run.iter().zip(host_run.iter()) {
        assert!((t_p - t_h).abs() < 1e-12, "accepted times must match");
        assert!((v_p - v_h).abs() < 1e-9, "t={t_p}: phdl={v_p} host={v_h}");
        if *t_p > XS[3] {
            saw_high_clamp = true;
        }
    }
    assert!(saw_high_clamp, "the fixture must run past the last breakpoint to exercise the end clamp");
}

// ── 3. A `cross`-triggered element ──────────────────────────────────────

/// PHDL: identical to `cross_check_events.rs`'s Sine+`@cross`-counter
/// fixture. Host: `piperine_ir::events::EventDetector` driven from
/// `accept_timestep`, reading the same sine input from a *second* host
/// device's own `IdealForce` value at `ctx.t()` (mirroring the PHDL's own
/// `V(sig,gnd) <- sin(...)` source) — a level-0 device using its own
/// `Ctx::v` to read another element's node, then an `IdealForce` twin to
/// publish the running count.
#[test]
fn cross_triggered_counter_matches_between_phdl_and_a_host_device() {
    let freq = 1e3;
    let (stop, dt) = (3e-3, 1e-5);
    let phdl = format!(
        "module Sine(inout electrical p, inout electrical n);
    parameter real freq = {freq};
    analog V(p, n) <+ sin(6.283185307179586 * freq * $abstime);
endmodule
module Detector(input electrical vin, inout electrical cnt, inout electrical gnd);
    real n;
    analog begin
        V(cnt, gnd) <+ n;
        @(cross(V(vin, gnd), 0)) n = n + 1.0;
    end
endmodule
module Top();
    electrical gnd, sig, cnt;
    Sine src (.p(sig), .n(gnd));
    Detector det (.vin(sig), .cnt(cnt), .gnd(gnd));
endmodule
"
    );
    let phdl_run = run_phdl_transient(&phdl, "Top", "cnt", stop, dt);
    assert!(phdl_run.len() > 10, "non-trivial accepted-point sequence");

    // Host twin: the sine source is an `IdealForce` (pure function of t, no
    // state); the counter is a `HostElement` reading `sig`'s node voltage
    // through `Ctx::v` and detecting crossings in `accept_timestep`.
    struct CrossCounter {
        sig: AnalogReference,
        cnt: AnalogReference,
        gnd: AnalogReference,
        branch: Option<AnalogReference>,
        detector: EventDetector,
        committed: f64,
    }
    impl HostElement for CrossCounter {
        fn name(&self) -> &str {
            "cross_counter"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG
                | ElementCapabilities::LOADS_DC
                | ElementCapabilities::LOADS_TRAN
                | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
        }
        fn allocate_unknowns(&mut self, alloc: &mut piperine_solver::abi::UnknownAllocator<'_>) {
            self.branch = Some(alloc.branch("cross_counter", "i"));
        }
        fn load_dc(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            self.stamp(sink);
        }
        fn load_transient(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            self.stamp(sink);
        }
        fn accept_timestep(&mut self, state: &StateView<'_>, t: f64) {
            let sensed = state.v(&self.sig);
            if self.detector.fired(Trigger::Cross(CrossDir::Either), sensed, t, 0.0) {
                self.committed += 1.0;
            }
        }
    }
    impl CrossCounter {
        /// The same ideal-force branch-row shape as `IdealForce` — `V(cnt,
        /// gnd) = committed`, exactly (a branch unknown, no impedance
        /// approximation), since `StampSink`'s `g`/`i` cover a branch row
        /// identically to a node row (T15's design note: "a branch row is
        /// just another `AnalogReference`").
        fn stamp(&mut self, sink: &mut StampSink) {
            let b = self.branch.clone().unwrap();
            sink.g(&self.cnt, &b, 1.0);
            sink.g(&self.gnd, &b, -1.0);
            sink.g(&b, &self.cnt, 1.0);
            sink.g(&b, &self.gnd, -1.0);
            sink.i(&b, self.committed);
        }
    }

    let mut builder = CircuitBuilder::new("host_cross_counter");
    let gnd = builder.ground();
    let sig = builder.node("sig");
    let cnt = builder.node("cnt");
    let sine = IdealForce {
        p: sig.clone(),
        n: gnd.clone(),
        branch: None,
        f: move |t: f64| (std::f64::consts::TAU * freq * t).sin(),
    };
    builder.element(Box::new(sine));
    builder.element(Box::new(HostAdapter(CrossCounter {
        sig: sig.clone(),
        cnt: cnt.clone(),
        gnd: gnd.clone(),
        branch: None,
        detector: EventDetector::new(0.0),
        committed: 0.0,
    })));
    let host_run = run_host_transient(builder, cnt, stop, dt);

    assert_eq!(host_run.len(), phdl_run.len(), "same accepted-point count");
    let mut saw_multiple_crossings = false;
    for ((t_p, v_p), (t_h, v_h)) in phdl_run.iter().zip(host_run.iter()) {
        assert!((t_p - t_h).abs() < 1e-12, "accepted times must match");
        assert!((v_p - v_h).abs() < 1e-9, "t={t_p}: phdl count={v_p} host count={v_h}");
        if *v_p >= 5.0 {
            saw_multiple_crossings = true;
        }
    }
    assert!(saw_multiple_crossings, "3 periods of a sine should cross zero at least 5 times");
}

// ── 4. A pnjlim junction ─────────────────────────────────────────────────

/// PHDL: a minimal exponential junction using `$limit("pnjlim", ...)`
/// directly (mirrors `piperine-ir/tests/cross_check_limiter.rs`'s own
/// minimal-device idiom, not the full spice-faithful diode header — a
/// smaller, exactly-matched pair rather than replicating ~15 physical
/// effects). Host: the identical equation, evaluated with
/// `piperine_ir::limit::pnjlim` directly and Newton-linearized the
/// same way codegen's own kernel does (companion conductance + RHS around
/// the limited voltage) — solved to the same DC operating point through a
/// resistor divider, values compared, not the bare formula (T6 already
/// proved the formula identical; this proves a *solved circuit* agrees).
#[test]
fn pnjlim_junction_matches_between_phdl_and_a_host_device() {
    let is = 1e-14_f64;
    let n = 1.0_f64;
    let vte = 0.02585_f64;
    let vcrit = n * vte * (n * vte / (is * 2f64.sqrt())).ln();
    let r = 1e3_f64;
    let vsupply = 0.6_f64;

    let phdl = format!(
        "module J(inout electrical d, inout electrical s);
    parameter real is = {is};
    parameter real n = {n};
    parameter real vte = {vte};
    analog I(d, s) <+ is * (exp($limit(\"pnjlim\", V(d, s), 0.0, n * vte, {vcrit}) / (n * vte)) - 1.0);
endmodule
module Vsrc(inout electrical p, inout electrical n);
    parameter real dc = 0.0;
    analog V(p, n) <+ dc;
endmodule
module R(inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule
module Top();
    electrical gnd, vin, d;
    Vsrc #(.dc({vsupply})) src (.p(vin), .n(gnd));
    R #(.r({r})) r1 (.p(vin), .n(d));
    J j1 (.d(d), .s(gnd));
endmodule
"
    );
    let design = parse_and_elaborate(&phdl, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    let d_net = info.nets.get("d").expect("net `d`").clone();
    let phdl_v = circuit
        .dc(Context::default())
        .expect("dc setup")
        .solve()
        .expect("phdl diode dc solves")
        .get_node(&d_net)
        .expect("v(d)");

    // Host twin: `HostElement` reading/writing through `Ctx`/`StampSink`,
    // Newton-linearized around the pnjlim-limited voltage exactly as
    // codegen's own kernel does for `$limit`.
    struct PnLimJunction {
        d: AnalogReference,
        s: AnalogReference,
        is: f64,
        n: f64,
        vte: f64,
        vcrit: f64,
        vold: f64,
    }
    impl HostElement for PnLimJunction {
        fn name(&self) -> &str {
            "pnjlim_junction"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
        }
        fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
            self.stamp(ctx, sink);
        }
        fn load_transient(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
            self.stamp(ctx, sink);
        }
    }
    impl PnLimJunction {
        fn stamp(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
            let vraw = ctx.v(&self.d) - ctx.v(&self.s);
            let vlim = pnjlim(vraw, self.vold, self.n * self.vte, self.vcrit);
            self.vold = vlim;
            let nvte = self.n * self.vte;
            let id = self.is * ((vlim / nvte).exp() - 1.0);
            let gd = (self.is / nvte) * (vlim / nvte).exp();
            let rhs = id - gd * vlim;
            sink.g(&self.d, &self.d, gd);
            sink.g(&self.d, &self.s, -gd);
            sink.g(&self.s, &self.d, -gd);
            sink.g(&self.s, &self.s, gd);
            sink.i(&self.d, -rhs);
            sink.i(&self.s, rhs);
        }
    }

    struct TVsrc {
        p: AnalogReference,
        n: AnalogReference,
        branch: Option<AnalogReference>,
        v: f64,
    }
    impl AnalogDevice for TVsrc {
        fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
            self.branch = Some(alloc.branch("vsrc", "i"));
        }
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let b = self.branch.clone().unwrap();
            vec![
                Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
                Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
                Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
                Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
                Stamp::Rhs(b, self.v),
            ]
        }
    }
    impl DigitalDevice for TVsrc {}
    impl Introspect for TVsrc {}
    impl Element for TVsrc {
        fn name(&self) -> &str {
            "vsrc"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
        }
    }
    struct TResistor {
        r: f64,
        p: AnalogReference,
        n: AnalogReference,
    }
    impl AnalogDevice for TResistor {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let g = 1.0 / self.r;
            vec![
                Stamp::Matrix(self.p.clone(), self.p.clone(), g),
                Stamp::Matrix(self.p.clone(), self.n.clone(), -g),
                Stamp::Matrix(self.n.clone(), self.p.clone(), -g),
                Stamp::Matrix(self.n.clone(), self.n.clone(), g),
            ]
        }
    }
    impl DigitalDevice for TResistor {}
    impl Introspect for TResistor {}
    impl Element for TResistor {
        fn name(&self) -> &str {
            "r"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
    }

    let mut builder = CircuitBuilder::new("host_pnjlim");
    let gnd = builder.ground();
    let vin = builder.node("vin");
    let d = builder.node("d");
    builder.element(Box::new(TVsrc { p: vin.clone(), n: gnd.clone(), branch: None, v: vsupply }));
    builder.element(Box::new(TResistor { r, p: vin.clone(), n: d.clone() }));
    builder.element(Box::new(HostAdapter(PnLimJunction {
        d: d.clone(),
        s: gnd.clone(),
        is,
        n,
        vte,
        vcrit,
        vold: vcrit,
    })));
    let mut circuit = builder.build().expect("host circuit builds");
    let host_v = circuit
        .dc(Context::default())
        .expect("dc setup")
        .solve()
        .expect("host diode dc solves")
        .get(d.clone())
        .expect("v(d)");

    assert!(
        (phdl_v - host_v).abs() < 1e-6,
        "the pnjlim junction's DC operating point must match between hosts: phdl={phdl_v} host={host_v}"
    );
    // Sanity: the junction must actually be forward-biased and clamped well
    // below the supply, not just "both solved to something".
    assert!(phdl_v > 0.3 && phdl_v < vsupply, "phdl_v={phdl_v} should be a forward-biased junction drop");
}

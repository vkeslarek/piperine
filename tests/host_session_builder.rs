//! CLA-14 — `SessionBuilder`: the build-time options a `Session` compilation
//! takes. Staging precedes the build (so it lives on the builder, not on the
//! compiled session), lifecycle hooks fire around the build in a fixed order
//! and abort it loudly on failure, and the `.disto` kernel set is opt-out.
//!
//! The fourth builder option, `provider`, needs a `@device` module — and the
//! `@device` attribute schema is only registered once a plugin host is loaded
//! (`crates/piperine-lang/headers/device_port.phdl` is deliberately not in
//! the prelude), so it is proven where a host exists:
//! `crates/piperine-plugin/tests/e2e.rs`, `inject.rs` and `hooks.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use piperine::{NetRef, Session, SimHooks, SolverConfig};
use piperine_lang::{Design, SourceMap, Value};

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param voltage: Real = 0.0; }
analog V { V(p, n) <- voltage; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire mid : Electrical;
    src   : V(.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : R(.p = vin, .n = mid) { .r = 3e3 };
    r_bot : R(.p = mid, .n = gnd) { .r = 2e3 };
}
";

/// A cubic VCCS — the fixture `.disto` needs a nonlinear branch for.
const POLY_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; param acmag: Real = 0.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 50.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod PolyVccs(inout inp: Electrical, inout inn: Electrical,
             inout outp: Electrical, inout outn: Electrical) {
    param g1: Real = 0.1;
    param g2: Real = 0.02;
    param g3: Real = 0.003;
}
analog PolyVccs {
    I(outp, outn) <+ g1 * V(inp, inn)
                   + g2 * V(inp, inn) * V(inp, inn)
                   + g3 * V(inp, inn) * V(inp, inn) * V(inp, inn);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire vout : Electrical;
    v1 : V(.p = vin, .n = gnd) { .dc = 0.0, .acmag = 1.0 };
    n1 : PolyVccs(.inp = vin, .inn = gnd, .outp = vout, .outn = gnd) {};
    r1 : R(.p = vout, .n = gnd) { .r = 50.0 };
}
";

fn elaborate(src: &str) -> Design {
    piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates")
}

fn mid() -> NetRef {
    NetRef { name: "mid".into() }
}

/// A staged override reaches the compilation: `r_top.r = 2 kΩ` turns the
/// 3k/2k divider (`v(mid) = 2.0 V`) into a 2k/2k one (`2.5 V`) — the value the
/// staged operating point produced before the collapse. And the write is the builder's
/// own: the caller's design still compiles to the authored `2.0 V`.
#[test]
fn builder_stage_reaches_the_compiled_circuit_and_leaves_the_design_alone() {
    let design = elaborate(DIVIDER_PHDL);

    let mut staged = Session::builder(&design, "Divider")
        .stage("r_top", "r", Value::Real(2e3))
        .compile()
        .expect("staged session compiles");
    let v = staged.op(&SolverConfig::default(), None).expect("op solves").v(mid()).expect("v(mid)");
    assert!((v - 2.5).abs() < 1e-9, "staged r_top = 2 kΩ gives v(mid) = 2.5 V, got {v}");

    let mut plain = Session::compile(&design, "Divider").expect("plain session compiles");
    let v0 = plain.op(&SolverConfig::default(), None).expect("op solves").v(mid()).expect("v(mid)");
    assert!(
        (v0 - 2.0).abs() < 1e-9,
        "the builder's stage must not write through to the caller's design: expected the \
         authored 2.0 V, got {v0}"
    );
}

/// The hook order is part of the build contract: `transform_design` (the
/// host's chance to stage) fires first, then the overrides are consumed, then
/// `before_lower` sees the applied design — so a param written by
/// `transform_design` is in the built circuit, and a hook reading
/// `before_lower` sees it already applied.
#[test]
fn builder_hooks_fire_around_the_build_in_order() {
    struct Recorder {
        log: RefCell<Vec<&'static str>>,
        pending_at_before_lower: RefCell<Option<bool>>,
    }
    impl SimHooks for Recorder {
        fn transform_design(&self, design: &Design) -> Result<(), String> {
            self.log.borrow_mut().push("transform_design");
            design.set_param("r_top", "r", Value::Real(2e3));
            Ok(())
        }
        fn before_lower(&self, design: &Design) -> Result<(), String> {
            self.log.borrow_mut().push("before_lower");
            *self.pending_at_before_lower.borrow_mut() = Some(design.has_overrides());
            Ok(())
        }
        fn after_solve(&self, _analysis: &str, _node_voltages: &[(String, f64)]) -> Result<(), String> {
            self.log.borrow_mut().push("after_solve");
            Ok(())
        }
    }

    let design = elaborate(DIVIDER_PHDL);
    let hooks =
        Rc::new(Recorder { log: RefCell::new(Vec::new()), pending_at_before_lower: RefCell::new(None) });
    let mut session = Session::builder(&design, "Divider")
        .hooks(hooks.clone())
        .compile()
        .expect("hooked session compiles");

    assert_eq!(
        *hooks.log.borrow(),
        vec!["transform_design", "before_lower"],
        "the build fires transform_design (stage), then before_lower (applied, read-only)"
    );
    assert_eq!(
        *hooks.pending_at_before_lower.borrow(),
        Some(false),
        "before_lower sees the design with transform_design's staged writes already consumed, \
         not still pending"
    );

    let v = session.op(&SolverConfig::default(), None).expect("op solves").v(mid()).expect("v(mid)");
    assert!(
        (v - 2.5).abs() < 1e-9,
        "the hook's staged r_top = 2 kΩ is in the built circuit: expected 2.5 V, got {v}"
    );
}

/// `after_solve` fires once per analysis on the compiled session, named for
/// the analysis that ran, and carries the solved node voltages for an
/// operating point (empty for the others) — the payload rule `SimHooks`
/// documents.
#[test]
fn after_solve_fires_once_per_analysis_with_the_analysis_name() {
    struct Solves {
        seen: RefCell<Vec<(String, usize)>>,
    }
    impl SimHooks for Solves {
        fn transform_design(&self, _design: &Design) -> Result<(), String> {
            Ok(())
        }
        fn before_lower(&self, _design: &Design) -> Result<(), String> {
            Ok(())
        }
        fn after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), String> {
            self.seen.borrow_mut().push((analysis.to_string(), node_voltages.len()));
            Ok(())
        }
    }

    let design = elaborate(DIVIDER_PHDL);
    let hooks = Rc::new(Solves { seen: RefCell::new(Vec::new()) });
    let mut session =
        Session::builder(&design, "Divider").hooks(hooks.clone()).compile().expect("compiles");
    let config = SolverConfig::default();

    let op = session.op(&config, None).expect("op solves");
    assert_eq!(
        hooks.seen.borrow().len(),
        1,
        "one `after_solve` per analysis, got {:?}",
        hooks.seen.borrow()
    );
    let (name, n_voltages) = hooks.seen.borrow()[0].clone();
    assert_eq!(name, "op", "the hook is told which analysis solved");
    assert!(
        n_voltages >= 3,
        "an operating point carries its solved node voltages (gnd/vin/mid at least), got {n_voltages}"
    );
    assert!((op.v(mid()).expect("v(mid)") - 2.0).abs() < 1e-9, "and the result is still correct");

    session.tran(1e-3, Some(1e-4), 0.0, &config, None, false, &[]).expect("tran solves");
    session.ac(1.0, 1e6, 5, true, &config).expect("ac solves");
    let seen: Vec<(String, usize)> = hooks.seen.borrow().clone();
    assert_eq!(
        seen.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        vec!["op", "tran", "ac"],
        "each analysis fires the hook under its own name, in call order"
    );
    assert_eq!(
        (seen[1].1, seen[2].1),
        (0, 0),
        "only operating points carry a node-voltage payload"
    );
}

/// A hook failure aborts the build with the hook's own message, as
/// `Error::Plugin` — never a partially-built session.
#[test]
fn a_failing_hook_aborts_the_build_loudly() {
    struct Failing;
    impl SimHooks for Failing {
        fn transform_design(&self, _design: &Design) -> Result<(), String> {
            Err("staging refused by the test hook".to_string())
        }
        fn before_lower(&self, _design: &Design) -> Result<(), String> {
            Ok(())
        }
        fn after_solve(&self, _a: &str, _v: &[(String, f64)]) -> Result<(), String> {
            Ok(())
        }
    }

    let design = elaborate(DIVIDER_PHDL);
    let err = match Session::builder(&design, "Divider").hooks(Rc::new(Failing)).compile() {
        Err(e) => e,
        Ok(_) => panic!("a failing hook must abort the build"),
    };
    assert!(
        matches!(err, piperine::Error::Plugin(_)),
        "a hook failure surfaces as Error::Plugin, got {err:?}"
    );
    assert!(
        err.to_string().contains("staging refused by the test hook"),
        "the hook's own message reaches the caller: {err}"
    );
}

/// The `.disto` 2nd/3rd-derivative kernels are opt-in: `disto(true)` makes
/// `Session::disto` solve, and the default build makes it fail loud naming the
/// opt-in — never solve against kernels that were never emitted. Every other
/// analysis runs either way.
#[test]
fn disto_kernels_are_opt_in_and_asking_without_them_is_loud() {
    let design = elaborate(POLY_PHDL);
    let config = SolverConfig::default();

    let mut with =
        Session::builder(&design, "Top").disto(true).compile().expect("opted-in session compiles");
    let disto = with.disto(1e6, None, 0.1, "vout", None, &config).expect("disto solves when asked for");
    assert!(disto.hd2.is_some(), "the opted-in build carries the 2nd-derivative kernels");

    let mut without = Session::builder(&design, "Top").compile().expect("default session compiles");
    let err = without
        .disto(1e6, None, 0.1, "vout", None, &config)
        .expect_err("`.disto` without the disto kernels must fail loud");
    assert!(
        err.to_string().contains("disto(true)"),
        "the error names the opt-in the caller needs: {err}"
    );

    // The default is scoped to `.disto`: every other analysis still runs.
    let op = without.op(&config, None).expect("op still solves without the disto kernels");
    assert!(op.v(NetRef { name: "vout".into() }).is_ok(), "v(vout) readable");
}

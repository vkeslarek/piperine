//! `cover` bin mapping and its own posture (dv-tester T33, DVT-40/41):
//! mirrors `margin_evaluation.rs`'s own precedent exactly — a bin hit
//! reuses the pointwise evaluation path (`AnalogKernel::eval_cover`, the
//! same `compile_rows`-compiled shape as `eval_margin`) with a
//! bin-mapper: a counter increment, no signed distance, no argmin.
//! `cover=on|off` is `Context.cover`, its own posture (D9) — separate
//! from `checks=`, proven by an eval-call count, not by output (a value
//! landing outside every bin also leaves every bin at 0, so counting bins
//! alone would not distinguish "the kernel ran and missed" from "the
//! kernel never ran").

//! **Present-and-red — blocked on the deferred `cover` statement**
//! (constraints-on-vams Out-of-Scope: `cover`/`cover cross` carry their
//! own bin-mapping and posture machinery and are deferred with dv-tester's
//! surface). The fixtures below still use the retired PHDL
//! `constraint Mod { … }` grammar and fail at elaboration until that
//! follow-up lands; they are the executable specification of the coverage
//! machinery — restore them to green then; do not delete them.

use std::sync::Arc;

use piperine_codegen::device::AnalogInstance;
use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{CircularArrayBuffer2, CoverPosture, Netlist, NodeIdentifier};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

fn dut_kernel(constraint: &str) -> Arc<AnalogKernel> {
    let src = format!(
        "{DISCIPLINE}
mod Dut(inout p: Electrical, inout n: Electrical) {{ param hi: Real = 3.0; }}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
{constraint}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    Arc::new(AnalogKernel::compile(&bodies["Dut"]).expect("compile"))
}

fn instance(label: &str, kernel: Arc<AnalogKernel>, params: Vec<f64>) -> AnalogInstance {
    let mut netlist = Netlist::new();
    let terminals = [NodeIdentifier::Anonymous(0), NodeIdentifier::Anonymous(1)];
    AnalogInstance::new(label, kernel, &terminals, params, 0b1, u64::MAX, &mut netlist).expect("instance")
}

fn accept(inst: &mut AnalogInstance, v: f64, t: f64) {
    let mut state = CircularArrayBuffer2::new(1, 2);
    let row = ndarray::Array1::from_vec(vec![v, 0.0]);
    state.push(&row.view());
    inst.accept_timestep(&state, t);
    inst.evaluate_constraints(&state, t, piperine_solver::abi::AnalysisKind::Dc);
}

/// DVT-40: a `cover` hit reuses the pointwise path and is a plain counter
/// increment — `V(p,n) bins [0.0:1.0:3.0]` is 4 bins (0,1,2,3); feeding
/// 0.4 and 2.6 hits bins 0 and 3 (nearest-edge mapping) exactly once and
/// twice respectively, everything else at 0.
#[test]
fn a_cover_hit_is_a_counter_increment_via_the_pointwise_path() {
    let kernel = dut_kernel("constraint Dut { cover c1 : V(p, n) bins [0.0:1.0:3.0]; }");
    let mut inst = instance("d1", kernel, vec![3.0]);
    assert_eq!(inst.cover_counts(), &[vec![0, 0, 0, 0]], "no accepted point yet");

    accept(&mut inst, 0.4, 1e-9);
    accept(&mut inst, 2.6, 2e-9);
    accept(&mut inst, 0.4, 3e-9);

    // Nearest-edge mapping: 0.4 rounds to bin 0 (edge 0.0), 2.6 rounds to
    // bin 3 (edge 3.0).
    assert_eq!(inst.cover_counts(), &[vec![2, 0, 0, 1]], "bin 0 hit twice, bin 3 hit once");
}

/// DVT-40: a value outside every declared bin does not count and does not
/// error.
#[test]
fn a_value_outside_every_bin_is_not_counted() {
    let kernel = dut_kernel("constraint Dut { cover c1 : V(p, n) bins [0.0:1.0:3.0]; }");
    let mut inst = instance("d1", kernel, vec![3.0]);
    accept(&mut inst, 99.0, 1e-9);
    assert_eq!(inst.cover_counts(), &[vec![0, 0, 0, 0]], "out-of-range values never hit a bin");
    assert_eq!(inst.cover_eval_calls(), 1, "the kernel still ran — it just found no bin");
}

/// DVT-41 (D9): `cover=off` is its **own** posture, separate from
/// `checks=` — the coverage kernel is never called (asserted by
/// `eval_calls`, not by the resulting counts, since a miss also leaves
/// every bin at 0).
#[test]
fn cover_off_never_calls_the_coverage_kernel() {
    let kernel = dut_kernel("constraint Dut { cover c1 : V(p, n) bins [0.0:1.0:3.0]; }");
    let mut inst = instance("d1", kernel, vec![3.0]);

    inst.set_cover_posture(CoverPosture::Off);
    accept(&mut inst, 0.4, 1e-9); // would hit bin 0 if evaluated
    assert_eq!(inst.cover_eval_calls(), 0, "cover=off: the coverage kernel was never called");
    assert_eq!(inst.cover_counts(), &[vec![0, 0, 0, 0]]);

    inst.set_cover_posture(CoverPosture::On);
    accept(&mut inst, 0.4, 2e-9);
    assert_eq!(inst.cover_eval_calls(), 1, "posture change takes effect at once (MD-18)");
    assert_eq!(inst.cover_counts(), &[vec![1, 0, 0, 0]]);
}

/// DVT-42/DVT-40: `cover cross` combines the *same instant's* per-point
/// bin hits into a joint counter — 2 points x 2 bins each = 4 joint bins,
/// row-major (`c1_bin * 2 + c2_bin`).
#[test]
fn cover_cross_combines_the_same_instants_per_point_bins() {
    let kernel = dut_kernel(
        "constraint Dut {\n\
             cover c1 : V(p, n) bins [0.0:2.0:2.0];\n\
             cover c2 : V(p, n) bins [0.0:2.0:2.0];\n\
             cover cross joint : c1, c2;\n\
         }",
    );
    let mut inst = instance("d1", kernel, vec![3.0]);
    assert_eq!(inst.cover_cross_counts(), &[vec![0, 0, 0, 0]]);

    // V=0.5 -> both c1 and c2 read the same probe, so both land in bin 0
    // (joint index 0*2+0 = 0).
    accept(&mut inst, 0.5, 1e-9);
    assert_eq!(inst.cover_cross_counts(), &[vec![1, 0, 0, 0]], "both points hit bin 0 at the same instant");
}

/// An "optimizer-shaped loop" (many accepted points, `cover=off`
/// throughout) leaves the coverage database completely untouched —
/// asserted by `eval_calls` staying at `0` across every iterate, not by
/// inspecting the (necessarily all-zero either way) counts.
#[test]
fn an_optimizer_loop_with_cover_off_leaves_the_database_untouched() {
    let kernel = dut_kernel("constraint Dut { cover c1 : V(p, n) bins [0.0:1.0:3.0]; }");
    let mut inst = instance("d1", kernel, vec![3.0]);
    inst.set_cover_posture(CoverPosture::Off);

    for i in 0..1000 {
        accept(&mut inst, (i % 4) as f64, i as f64 * 1e-9);
    }
    assert_eq!(inst.cover_eval_calls(), 0, "1000 inner-loop iterates, zero coverage-kernel calls");
    assert!(inst.cover_counts().iter().all(|bins| bins.iter().all(|&c| c == 0)));
}

/// A module with no `cover` allocates nothing.
#[test]
fn no_cover_allocates_nothing() {
    let kernel = dut_kernel("");
    let mut inst = instance("d1", kernel, vec![3.0]);
    assert!(inst.cover_counts().is_empty());
    assert!(inst.cover_cross_counts().is_empty());
    accept(&mut inst, 1.0, 1e-9);
    assert_eq!(inst.cover_eval_calls(), 0);
}

/// End to end (DVT-41): a transient run with `cover=off` in `Context`
/// completes cleanly — the posture reaches the device through the same
/// per-assemble `Context` handoff `checks=` already uses
/// (`Element::update`, `element.rs`'s `set_cover_posture` call alongside
/// `set_check_posture`). `AnalogInstance`-level access to the resulting
/// counters is `T34`'s host-channel job (MD-01: no downcast through
/// `Element`, so this level can only prove the run itself is unaffected
/// by the presence of a `cover` block under `cover=off`).
#[test]
fn cover_off_end_to_end_through_a_real_transient() {
    let src = format!(
        "{DISCIPLINE}
mod Vsrc(inout p: Electrical, inout n: Electrical) {{ param dc: Real = 1.5; }}
analog Vsrc {{ V(p, n) <- dc; }}
mod Dut(inout p: Electrical, inout n: Electrical) {{}}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
constraint Dut {{ cover c1 : V(p, n) bins [0.0:1.0:3.0]; }}
mod Top() {{
    wire gnd : Electrical;
    wire in  : Electrical;
    v1 : Vsrc(.p = in, .n = gnd);
    d1 : Dut(.p = in, .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");

    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let off = Context { cover: CoverPosture::Off, ..Default::default() };
    let result = circuit.transient(opts, off).unwrap().solve();
    assert!(result.is_ok(), "a cover-bearing circuit solves cleanly under cover=off");
}

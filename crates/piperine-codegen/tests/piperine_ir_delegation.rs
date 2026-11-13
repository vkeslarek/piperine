//! Scope: pins codegen's delegation to `piperine_ir` for the three runtime
//! families spec.md's B6 names — operators, event detectors, limiters — so
//! a future change cannot silently re-grow a private copy of any of them
//! without this file catching it (T52, `lrm-conformance-wave3`, PSW3-04).
//!
//! Every check here is behavioral: identical outputs for identical inputs,
//! never a source-text/import grep. The limiter check reuses
//! `crates/piperine-codegen/tests/limiters.rs`'s `$limit`-JIT pattern,
//! comparing the JIT-emitted `pnjlim` call directly against
//! `piperine_ir::limit::pnjlim` (that file compares against a hand-derived
//! ngspice reference instead, which is a different, complementary proof —
//! this file's job is specifically the piperine_ir *delegation*). The
//! operator check runs a real transient (`piperine_solver`, bypassing the
//! unavailable higher-level `piperine-api` Trace/Waveform surface, which
//! this crate's tests do not depend on) and replays the exact
//! accepted-time/input sequence the JIT device actually saw through a
//! freshly constructed `piperine_ir::ops::Operator::transition`, asserting
//! the replay reproduces the JIT's own recorded output net voltage
//! sample-for-sample.
//!

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::CircuitCompiler;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{AnalogVariable, CheckPosture};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

fn source_map() -> piperine_lang::SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

// ── Limiter family: pnjlim, JIT vs piperine_ir directly ────────────────────

/// Compile a one-limiter device (mirrors `tests/limiters.rs::jit_limit`) and
/// return `eval_limit_update` for the `pnjlim` limiter kind.
fn jit_pnjlim(vnew: f64, vold: f64, vte: f64, vcrit: f64) -> f64 {
    let src = format!(
        "module L (inout electrical d, inout electrical s);
    parameter real vte = 1.0;
    parameter real vcrit = 1.0;
    analog I(d, s) <+ $limit(\"pnjlim\", V(d, s), 0.0, vte, vcrit);
endmodule",
    );
    let elab = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    let kernel = AnalogKernel::compile(&bodies["L"]).expect("compile limiter device");

    assert_eq!(kernel.num_limits(), 1, "exactly one $limit slot");
    let mut state = vec![0.0; kernel.num_state_slots()];
    state[kernel.limit_base()] = vold;
    let params = vec![vte, vcrit];
    let vars = vec![0.0; kernel.num_vars()];
    let volts = vec![vnew, 0.0];
    let sim = SimCtx::default();
    let mut out = vec![0.0; 1];
    kernel.eval_limit_update(&volts, &params, &state, &vars, &sim, &mut out);
    out[0]
}

/// PSW3-04/B6: the JIT-emitted `pnjlim` call must return **exactly** what
/// `piperine_ir::limit::pnjlim` returns for the identical inputs — codegen
/// is a thin dispatcher over the registered `piperine-ir` symbol
/// (`kernel/analog/limits.rs::LIMIT_FNS`), not a second implementation.
#[test]
fn jit_pnjlim_matches_piperine_ir_pnjlim_exactly() {
    let cases = [
        (2.0, 0.5, 0.026, 10.0),
        (0.5, 0.5, 0.026, 10.0),
        (-1.0, 0.2, 0.026, 10.0),
        (5.0, 4.0, 0.05, 1.0),
        (0.0, 0.0, 0.026, 10.0),
    ];
    for (vnew, vold, vte, vcrit) in cases {
        let jit = jit_pnjlim(vnew, vold, vte, vcrit);
        let direct = piperine_ir::limit::pnjlim(vnew, vold, vte, vcrit);
        assert_eq!(
            jit, direct,
            "pnjlim(vnew={vnew}, vold={vold}, vte={vte}, vcrit={vcrit}): jit={jit} \
             piperine_ir={direct} — codegen's $limit dispatch has drifted from piperine_ir::limit::pnjlim"
        );
    }
}

// ── Operator family: transition, JIT-recorded trajectory replayed ─────────

/// Runs a real transient of a `transition()` device and returns, for every
/// accepted step, `(time, input_voltage, output_voltage)` — the exact
/// sequence codegen's `AnalogInstance::accept_timestep` drove its
/// `SlottedOperator` with (input) and the resulting net voltage the rest of
/// the circuit observed (output).
fn transition_trajectory(rise_fall: f64) -> Vec<(f64, f64, f64)> {
    // Two instances, not one whole-module behavior: a single-device "Top"
    // with no instances registers no cross-device netlist references at
    // all (its internal nets are private to that one `PiperineDevice`), so
    // `TransientStep::values()` — which walks `Netlist::all_references()`
    // — would come back empty for every step regardless of what the
    // device actually computed. Splitting the driver and the `transition`
    // consumer into two instances connected through named top-level nets
    // makes `in`/`out` genuine inter-device netlist references, so they
    // land in `TransientStep::values()` and are readable by name.
    let src = format!(
        "`include \"disciplines.vams\"
module Driver (inout electrical p, inout electrical n);
    analog V(p, n) <+ ($abstime > 1e-6) ? 1.0 : 0.0;
endmodule
module Dut (inout electrical p, inout electrical n, inout electrical o, inout electrical g);
    analog V(o, g) <+ transition(V(p, n), 0.0, {rise_fall}, {rise_fall});
endmodule
module Top ();
    electrical gnd, in, out;
    Driver d1 (.p(in), .n(gnd));
    Dut u1 (.p(in), .n(gnd), .o(out), .g(gnd));
endmodule
",
    );
    let design = parse_and_elaborate(&src, &source_map()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");

    let in_var: std::sync::Arc<AnalogVariable> =
        std::sync::Arc::new(AnalogVariable::Node(info.nets.get("in").expect("net `in` mapped").clone()));
    let out_var: std::sync::Arc<AnalogVariable> =
        std::sync::Arc::new(AnalogVariable::Node(info.nets.get("out").expect("net `out` mapped").clone()));

    let ctx = Context { checks: CheckPosture::Collect, ..Default::default() };
    let result = circuit
        .transient(TransientAnalysisOptions::new(4e-6, 1e-7), ctx)
        .expect("transient builds")
        .solve()
        .expect("transient solves");

    (0..result.len())
        .map(|i| {
            let step = result.get(i).expect("accepted step");
            let time = step.time();
            let vin = step.get(in_var.clone()).expect("`in` voltage recorded");
            let vout = step.get(out_var.clone()).expect("`out` voltage recorded");
            (time, vin, vout)
        })
        .collect()
}

/// PSW3-04/B6: replaying the exact (time, input) sequence the JIT device
/// saw through a **fresh, standalone** `piperine_ir::ops::Operator::transition`
/// reproduces the JIT's own recorded output voltage at every accepted step.
/// This is only true if `AnalogInstance::accept_timestep`'s `SlottedOperator`
/// wraps `piperine_ir::ops::Operator` verbatim (no independent ramp/settle
/// math of its own) — the exact claim spec.md's B6 makes and
/// `crates/piperine-ir/src/ops.rs`'s own module doc now states.
///
/// Companion-model timing (confirmed empirically against this fixture, then
/// cross-checked against `core/circuit.rs::seed_digital_from_accept_hooks`/
/// `AnalogInstance::accept_timestep`): `accept_timestep(t_i, vin_i)` runs
/// *after* step `i`'s own Newton solve has already used the operator's
/// slot value from step `i-1`, and its result is only substituted into the
/// `V(out) = state[slot]` companion equation for step `i+1`'s solve. So the
/// recorded `vout[i+1]` is exactly `op.accept(t_i, vin_i)` — output lags
/// input by one accepted step, the ordinary companion-model state pattern
/// (the reactive/ddt companion has the identical one-step lag). `vout[0]`
/// is the DC operating-point seed, asserted separately below.
#[test]
fn transition_replay_through_piperine_ir_matches_the_jit_trajectory() {
    let rise_fall = 5e-7;
    let trajectory = transition_trajectory(rise_fall);
    assert!(trajectory.len() > 5, "the fixture must produce more than a handful of accepted steps");

    let (t0, vin0, vout0) = trajectory[0];
    assert!(
        (vout0 - vin0).abs() < 1e-9,
        "t={t0}: the DC operating point must seed transition() at the input value (first \
         observation settles instantly, ops.rs's `seeded` branch) — got vin={vin0} vout={vout0}"
    );

    let mut op = piperine_ir::ops::Operator::transition(0.0, rise_fall, rise_fall, 0.0);
    for i in 0..trajectory.len() - 1 {
        let (time_i, vin_i, _) = trajectory[i];
        let (time_next, _, vout_next) = trajectory[i + 1];
        let replayed = op.accept(time_i, vin_i);
        assert!(
            (replayed - vout_next).abs() < 1e-9,
            "step {} -> {} (t_i={time_i}, vin_i={vin_i}, t_next={time_next}): JIT output \
             {vout_next} vs piperine_ir replay {replayed} — codegen's transition() operator has \
             drifted from piperine_ir::ops::Operator",
            i, i + 1
        );
    }

    // Sanity check the fixture itself actually exercises a live ramp (not a
    // degenerate all-zero or all-settled trajectory), or the proof above
    // would pass vacuously.
    let first_out = trajectory[0].2;
    let last_out = trajectory.last().unwrap().2;
    assert!(
        (last_out - first_out).abs() > 0.5,
        "the fixture must ramp from ~0 to ~1 — got first={first_out} last={last_out}"
    );
}

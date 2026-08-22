//! LRM-COVERS: 4.5.7
//! LRMW2-31 (`lrm-conformance-wave2`, T36): `absdelay(input, td, maxdelay)`'s
//! third argument was silently discarded — the delay buffer always used
//! `td` verbatim, never clamped to `maxdelay`. §4.5.7: "If the optional
//! maxdelay is specified, then td can vary. If td becomes greater than
//! maxdelay, maxdelay will be used as a substitute for td."
//!
//! SPEC_DEVIATION (disclosed): the task's own Done-when checklist says
//! "a `td > maxdelay` errors citing §4.5.4" — that contradicts the LRM's
//! own text quoted above (a *substitution*, never an error) and cites the
//! wrong section number besides (`absdelay` is §4.5.7 in this LRM copy's
//! own heading numbers, not §4.5.4 — verified against `lrm_sections()`).
//! Per C7 (grammar/semantics from `VAMS-LRM-2-4.md` only), this
//! implements the LRM's literal words: `maxdelay` clamps `td`, no error.
//!
//! Gate note: run through the host `Session` (root `tests/`, not
//! `crates/piperine-codegen/tests/`) because reading a transient
//! waveform by name needs `piperine-api`'s `Trace`, which
//! `piperine-codegen` does not depend on — a "full" gate rather than the
//! tasks.md table's "quick", since the proof genuinely crosses into the
//! host surface.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn v_out_at(td: f64, maxdelay: f64, t: f64) -> f64 {
    let src = format!(
        "
module Dut (inout electrical inp, inout electrical outp);
    parameter real td = 1.0;
    parameter real maxdelay = 1.0;
    analog begin
        V(inp, gnd) <+ $abstime;
        V(outp, gnd) <+ absdelay(V(inp, gnd), td, maxdelay);
    end
endmodule

module Top ();
    electrical inp, outp;
    Dut #(.td({td}), .maxdelay({maxdelay})) d1 (inp, outp);
endmodule
"
    );
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(3.0, Some(0.01), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.v("outp").expect("v(outp)").at(t)
}

/// Buffer sizing observable: a `td` within `maxdelay` reproduces the
/// ramp input delayed by exactly `td` (no clamp needed) — `V(outp)@t =
/// t - td` once the buffer has enough history.
#[test]
fn a_delay_within_maxdelay_reproduces_the_input_exactly() {
    let v = v_out_at(0.5, 2.0, 2.5);
    assert!((v - 2.0).abs() < 0.05, "expected ~2.0 (2.5 - 0.5), got {v}");
}

/// AC14 discrimination: with `td=1.0` fixed, `maxdelay=0.2` (< td) clamps
/// the effective delay to 0.2, while `maxdelay=2.0` (> td) leaves it at
/// td=1.0 — two different `maxdelay` values must give two different
/// effective delays, hence two different `V(outp)` readings at the same
/// sample time. An ignored `maxdelay` argument cannot reproduce this
/// (both would read the same `td`-delayed value).
#[test]
fn two_different_maxdelay_values_give_two_different_effective_delays() {
    let clamped = v_out_at(1.0, 0.2, 3.0);
    let unclamped = v_out_at(1.0, 2.0, 3.0);
    assert!((clamped - 2.8).abs() < 0.05, "maxdelay=0.2 clamps td to 0.2: expected ~2.8, got {clamped}");
    assert!((unclamped - 2.0).abs() < 0.05, "maxdelay=2.0 leaves td=1.0: expected ~2.0, got {unclamped}");
    assert!(
        (unclamped - clamped).abs() > 0.5,
        "maxdelay must make a material difference: clamped={clamped} unclamped={unclamped}"
    );
}

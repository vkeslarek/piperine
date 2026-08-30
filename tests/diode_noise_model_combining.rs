//! LRM-COVERS: 4.6.4.5
//!
//! lrm-conformance-wave3 (Phase 6, T36): §4.6.4.5 "Noise model for diode" —
//! the LRM's own worked example combines the exponential diode I-V law
//! with `white_noise` (shot noise, PSD proportional to the diode's own
//! bias current) and `flicker_noise` in **one** contribution statement:
//! `I(a,c) <+ is*(exp(V(a,c)/(n*$vt)) - 1) + white_noise(2*'P_Q*I(<a>)) +
//! flicker_noise(kf*pow(abs(I(<a>)), af), ef);`.
//!
//! This fixture reproduces that combination with one disclosed
//! substitution: the diode current feeding both noise PSD arguments is
//! computed into a `real id;` variable first (`id =
//! is*(exp(V(a,c)/(n*vt)) - 1);`), then referenced from `white_noise`/
//! `flicker_noise`, instead of the LRM's `I(<a>)` port-current access —
//! the same idiom the real production models already use
//! (`crates/piperine-lang/headers/spice/bjt.va`'s `cc`/`cb` real
//! variables feeding `white_noise(2*NG_Q*m*abs(cc))`). This keeps the
//! fixture's expected PSD independently, exactly derivable (closed-form
//! small-signal algebra below) without depending on how a *self-
//! referential* `I(<port>)` read inside a noise argument resolves — a
//! narrower question this target does not claim to answer.
//!
//! Circuit: an ideal `Vsrc` biases node `a` (a zero-AC-impedance
//! reference), the diode's own branch (a,c) carries `id` plus shot and
//! flicker noise, and `Rload` returns the diode current to ground at
//! node `c`, where noise is observed. Small-signal: the diode's dynamic
//! conductance `J = (id+is)/vt` (in parallel, from `c` to the AC-grounded
//! `a`) and `Rload`'s `1/r` (from `c` to true ground) combine into
//! `Gtotal = J + 1/r`; each noise current source's PSD divides by
//! `Gtotal²` to give its own contribution to the output voltage PSD —
//! independently reconciled below against `NoiseTrace::by_source()`.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const SRC: &str = "
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 0.0;
    analog V(p, n) <+ dc;
endmodule

module Diode (inout electrical a, inout electrical c);
    parameter real is = 1e-14;
    parameter real n = 1.0;
    parameter real vt = 0.025;
    parameter real kf = 1e-14;
    parameter real af = 1.0;
    parameter real ef = 1.0;
    real id;
    analog begin
        id = is*(exp(V(a,c)/(n*vt)) - 1);
        I(a,c) <+ id + white_noise(2*1.602176634e-19*id) + flicker_noise(kf*pow(abs(id), af), ef);
    end
endmodule

module Rload (inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Top ();
    electrical gnd, a, c;
    Vsrc vsrc (.p(a), .n(gnd));
    Diode d1 (.a(a), .c(c));
    Rload rl (.p(c), .n(gnd));
endmodule
";

const IS: f64 = 1e-14;
const VT: f64 = 0.025;
const KF: f64 = 1e-14;
const R_LOAD: f64 = 100.0;
const Q: f64 = 1.602176634e-19;
const FREQ: f64 = 1e3;

#[test]
fn diode_law_plus_shot_and_flicker_noise_combine_correctly() {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::builder(&design, "Top")
        .stage("vsrc", "dc", piperine_lang::Value::Real(0.6))
        .stage("rl", "r", piperine_lang::Value::Real(R_LOAD))
        .compile()
        .expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");

    // The diode's DC bias current, read back independently from the
    // load's own Ohm's-law relation (`op.v("c")` and `Rload`'s known `r`)
    // rather than trusting any internal solver state.
    let v_c = op.v("c").expect("v(c)");
    let id = v_c / R_LOAD;
    assert!(id > 1e-6, "the bias point must actually forward-bias the diode, got id={id}");

    let g_total = (id + IS) / VT + 1.0 / R_LOAD;
    let expected_shot = 2.0 * Q * id / (g_total * g_total);
    let expected_flicker = KF * id.abs() / FREQ / (g_total * g_total);

    let trace =
        session.noise("c", "gnd", (FREQ, FREQ), 1, false, &SolverConfig::default()).expect("noise solves");
    let sources = trace.by_source();
    assert_eq!(sources.len(), 2, "exactly two noise sources (white_noise + flicker_noise) on the diode branch, got {}", sources.len());

    let mut shot_ok = false;
    let mut flicker_ok = false;
    for (key, w) in &sources {
        let (_, psd) = w.points()[0];
        if (psd - expected_shot).abs() / expected_shot < 1e-6 {
            shot_ok = true;
        } else if (psd - expected_flicker).abs() / expected_flicker < 1e-6 {
            flicker_ok = true;
        } else {
            panic!(
                "source {key}: PSD {psd:e} matches neither the expected shot ({expected_shot:e}) \
                 nor flicker ({expected_flicker:e}) contribution"
            );
        }
    }
    assert!(shot_ok, "no source matched the expected shot-noise PSD {expected_shot:e}");
    assert!(flicker_ok, "no source matched the expected flicker-noise PSD {expected_flicker:e}");

    // The total output PSD is the superposition of the two independent
    // (uncorrelated) sources — §4.6.4.6's own baseline rule, "each noise
    // function generates noise which is uncorrelated with... other
    // functions" — reconciled against the summed per-source figures.
    let total = trace.psd().points()[0].1;
    let expected_total = expected_shot + expected_flicker;
    assert!(
        (total - expected_total).abs() / expected_total < 1e-6,
        "total PSD {total:e} must equal the sum of the shot and flicker contributions {expected_total:e}"
    );
}

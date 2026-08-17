//! AC margin evaluation per frequency point (dv-core T20, DVC-11):
//! pointwise margins are evaluated at each frequency of the sweep with
//! the **frequency** as the argmin coordinate, and the DC operating-point
//! solve underneath the sweep is not double-counted (design §C3 — the DC
//! row already covers it). Pointwise kernels are real-valued, so AC
//! evaluation reads the small-signal amplitudes (phasor magnitudes).
//! `strict` aborts the sweep at the first violated frequency; `collect`
//! records each violating point's finding with its frequency.

//! **Require-core migrated** (constraints-on-vams): these tests run
//! against the live producer. Deferred statements (`var`/`target`/
//! `cover`) are exercised in their own files, still red.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::CheckPosture;
use piperine_solver::prelude::{AcSweepAnalysisOptions, CircuitInstance, Context};

const DISCIPLINES: &str = "`include \"disciplines.vams\"";

/// The `SourceMap` reaching `piperine-lang`'s bundled headers from this
/// crate (`SourceMap::dummy`'s relative paths only resolve from that
/// crate's dir).
fn source_map() -> piperine_lang::SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// The filter fixture: an AC-magnitude source (1 V, no DC) through 1 kΩ
/// into 1 µF — a single-pole lowpass with fc ≈ 159 Hz. The capacitor
/// carries two requires over its phasor amplitude: `pos` (V ≥ 0, margin =
/// |V|, worst at the highest frequency) and `amp` (V ≤ 0.5, violated at
/// low frequencies where |V| ≈ 1, satisfied past the rolloff).
fn filter_circuit() -> (CircuitInstance, piperine_codegen::device::CircuitBuildInfo) {
    let src = format!(
        "{DISCIPLINES}
module Vac (inout electrical p, inout electrical n);
    parameter real acmag = 1.0;
    analog begin
        V(p, n) <+ ac_stim(acmag, 0.0);
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module C (inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
    end
    constraint begin
        require pos : V(p, n) >= 0.0;
        require amp : V(p, n) <= 0.5;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vac v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    C c1 (.p(out), .n(gnd));
endmodule
        "
    );
    let design = parse_and_elaborate(&src, &source_map()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit_mapped("Top").expect("circuit builds")
}

fn options() -> AcSweepAnalysisOptions {
    AcSweepAnalysisOptions {
        start_frequency: 1.0,
        stop_frequency: 1e6,
        steps: 6,
        logarithmic: true,
    }
}

fn collect_ac(circuit: &mut CircuitInstance) -> piperine_solver::prelude::AcAnalysisResult {
    let ctx = Context { checks: CheckPosture::Collect, ..Default::default() };
    circuit.ac(ctx).unwrap().solve_sweep(options()).expect("collect completes")
}

/// DVC-11: AC evaluates pointwise margins at each frequency point, with
/// the frequency as the argmin coordinate. `pos`'s worst is the smallest
/// amplitude — the 1 MHz point's |V(out)| — and `amp`'s worst is the
/// largest — the 1 Hz point's ≈ 1 V.
#[test]
fn ac_margins_are_evaluated_per_frequency_with_frequency_argmin() {
    let (mut circuit, info) = filter_circuit();
    let result = collect_ac(&mut circuit);

    let out = info.nets.get("out").expect("net `out`");
    let amplitude_at = |f: f64| {
        let step = result
            .iter()
            .find(|s| s.frequency == f)
            .unwrap_or_else(|| panic!("a sweep point at {f} Hz"));
        step.get_node(out).expect("v(out)").norm()
    };

    assert_eq!(result.stats.margins.len(), 2, "both declared margins reported");
    let pos = &result.stats.margins[0];
    assert_eq!(pos.label, "pos");
    assert_eq!(pos.instance_path, "c1");
    let f_stop_amp = amplitude_at(1e6);
    assert!(
        (pos.worst - f_stop_amp).abs() < 1e-9,
        "pos worst = |V(out)| at 1 MHz = {f_stop_amp}, got {}",
        pos.worst
    );
    assert_eq!(pos.argmin, Some(1e6), "the argmin is a frequency coordinate");

    let amp = &result.stats.margins[1];
    assert_eq!(amp.label, "amp");
    assert!(
        (amp.worst - (0.5 - amplitude_at(1.0))).abs() < 1e-9,
        "amp worst = 0.5 − |V(out)| at 1 Hz, got {}",
        amp.worst
    );
    assert_eq!(amp.argmin, Some(1.0), "the largest amplitude is at the lowest frequency");
}

/// DVC-11: the DC operating-point solve underneath the sweep is not
/// double-counted — the OP has V(out) = 0 (an AC-only source), which
/// would be `pos`'s worst (margin exactly 0.0) if it leaked into the AC
/// channel. The recorded worst is the 1 MHz point's positive amplitude.
#[test]
fn the_dc_solve_underneath_ac_is_not_double_counted() {
    let (mut circuit, _) = filter_circuit();
    let result = collect_ac(&mut circuit);
    let pos = &result.stats.margins[0];
    assert!(
        pos.worst > 0.0,
        "the DC point (margin exactly 0.0) is not in the AC channel, got worst {}",
        pos.worst
    );
    assert_eq!(pos.argmin, Some(1e6), "the argmin is a sweep frequency, never the OP");
}

/// DVC-15 on the AC row: `collect` records one finding per violating
/// frequency point — each carrying its frequency as the coordinate —
/// while `strict` aborts the sweep at the first violated frequency,
/// naming the constraint, instance, coordinate, and margin value.
#[test]
fn ac_findings_carry_the_frequency_and_strict_aborts_mid_sweep() {
    let (mut circuit, _) = filter_circuit();
    let result = collect_ac(&mut circuit);
    let frequencies: Vec<f64> = result.iter().map(|s| s.frequency).collect();
    assert!(!result.stats.validation.is_empty(), "amp is violated at low frequencies");
    for finding in &result.stats.validation {
        assert_eq!(finding.label, "amp");
        assert!(
            frequencies.contains(&finding.time),
            "the finding's coordinate is a sweep frequency, got {}",
            finding.time
        );
        let amplitude = 0.5 - finding.value;
        assert!(amplitude > 0.5, "the finding carries the violating margin, got {amplitude}");
    }

    let (mut circuit, _) = filter_circuit();
    let err = match circuit.ac(Context::default()).unwrap().solve_sweep(options()) {
        Ok(_) => panic!("strict + a violated require aborts the sweep"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(msg.contains("amp"), "names the constraint: {msg}");
    assert!(msg.contains("c1"), "names the instance path: {msg}");
    assert!(msg.contains("at t = 1"), "names the frequency coordinate: {msg}");
    assert!(
        msg.contains("margin = -0.499") || msg.contains("margin = -0.5"),
        "names the margin value (0.5 − |V(out, 1 Hz)| ≈ −0.5): {msg}"
    );
}

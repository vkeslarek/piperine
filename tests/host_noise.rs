//! HOST-11 (host-library T14): per-source noise contributions on the noise
//! `Trace` — `nz.by_source()` (HashMap of `"element/source"` → PSD Waveform)
//! and `nz.contributions()` (the `NoiseContribution` catalog). The
//! conservation check proves the sum of per-source `integrated_sq` reconciles
//! with `total()²` (the output-referred integrated noise squared).

use std::path::PathBuf;

use piperine::{NoiseTrace, Session, SolverConfig};
use piperine_lang::SourceMap;

/// A resistor + output capacitor driven by a DC source — a minimal circuit
/// whose resistor emits thermal noise via `white_noise` (the stdlib
/// convention). The capacitor shapes the output noise but contributes none.
const NOISE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VSource(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog VSource { V(p, n) <- dc; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r : Real = 1e3;
    param t : Real = 300.15;
}
analog Resistor {
    I(p, n) <+ V(p, n) / r;
    I(p, n) <+ white_noise(4.0 * 1.380649e-23 * t / r);
}

mod Capacitor(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-9; }
analog Capacitor { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : VSource   (vin, gnd) {};
    r1 : Resistor  (vin, out) { .r = 1e3 };
    c1 : Capacitor (out, gnd) { .c = 1e-9 };
}
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn noise_trace() -> NoiseTrace {
    let design = piperine_lang::parse_and_elaborate(NOISE_PHDL, &headers_source_map())
        .expect("noise fixture elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    session
        .noise("out", "gnd", (1.0, 1e6), 10, true, &SolverConfig::default())
        .expect("noise solves")
}

/// HOST-11 AC1: `nz.by_source()` returns a non-empty HashMap keyed
/// `"element/source"` — a `white_noise` source on `r1` surfaces with key
/// starting `"r1/"`.
#[test]
fn by_source_returns_per_source_psd_waveforms() {
    let nz = noise_trace();
    let sources = nz.by_source();
    assert!(!sources.is_empty(), "resistor emits at least one noise source");
    let r1_key = sources
        .keys()
        .find(|k| k.starts_with("r1/"))
        .expect("at least one source on r1");
    let wf = &sources[r1_key];
    assert!(
        wf.points().len() == 10,
        "one PSD sample per frequency point (10), got {}",
        wf.points().len()
    );
}

/// HOST-11 AC2: `nz.contributions()` returns the per-source catalog with
/// `element`/`source`/`kind`/`integrated_sq` — beyond the scalar `total()`.
#[test]
fn contributions_lists_per_source_catalog() {
    let nz = noise_trace();
    let contribs = nz.contributions();
    assert!(!contribs.is_empty(), "at least one noise source");
    let r1 = contribs
        .iter()
        .find(|c| c.element == "r1")
        .expect("r1 contributes noise");
    assert!(r1.integrated_sq > 0.0, "noise integrated_sq > 0");
}

/// HOST-11 conservation: the sum of per-source `integrated_sq` reconciles
/// with `total()²` — the output-referred integrated noise squared. This is
/// the conservation property the adjoint-method noise summation guarantees.
#[test]
fn sum_of_integrated_sq_matches_total_squared() {
    let nz = noise_trace();
    let total_sq = nz.total().powi(2);
    let sum: f64 = nz.contributions().iter().map(|c| c.integrated_sq).sum();
    assert!(
        (sum - total_sq).abs() < total_sq * 1e-6 || (sum - total_sq).abs() < 1e-30,
        "sum of integrated_sq ({sum:e}) must match total² ({total_sq:e}) within 1ppm"
    );
}

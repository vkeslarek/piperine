//! Combined two-winding transformer (`xfmr`) over the mutual-flux engine —
//! SC-17. Open-secondary AC voltage ratio is k·√(L2/L1); bad l1/l2/k are loud.

use std::path::PathBuf;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Primary driven by a 1 V AC source, secondary nearly open (1 GΩ). The
/// open-secondary voltage ratio V(sec)/V(pri) = M/L1 = k·√(L2/L1); with
/// L2/L1 = 4 and k = 0.999 that is ≈ 1.998.
#[test]
fn xfmr_open_secondary_voltage_ratio() {
    // Drive the primary through a small series resistor (an ideal source
    // directly across the winding is two parallel voltage branches → singular).
    let phdl = "use piperine::disciplines;
use spice::sources;
use spice::passives;
mod Top() {
    wire gnd: Electrical; wire src: Electrical; wire pri: Electrical; wire sec: Electrical;
    vin: vsrc (.p=src,.n=gnd) { .ac_mag = 1.0 };
    rs: res (.p=src,.n=pri) { .r = 0.01 };
    t1: xfmr (.p1=pri,.n1=gnd,.p2=sec,.n2=gnd) { .l1 = 1.0e-6, .l2 = 4.0e-6, .k = 0.999 };
    rl: res (.p=sec,.n=gnd) { .r = 1.0e9 };
}";
    let design = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect("xfmr design elaborates");
    let trace = SimSession::new(design, "Top".to_string())
        .run_ac(1.0e3, 1.0e6, 20, true, &SolverConfig::default())
        .unwrap_or_else(|e| panic!("xfmr ac failed: {e}"));

    let vsec = trace.v(&NetRef { name: "sec".into() }, None).expect("v(sec)").mag();
    let vpri = trace.v(&NetRef { name: "pri".into() }, None).expect("v(pri)").mag();
    // The ratio is frequency-independent for an open secondary; check a decade.
    for f in [1.0e3, 1.0e4, 1.0e5] {
        let ratio = vsec.at(f) / vpri.at(f);
        assert!(
            (ratio - 1.998).abs() < 0.01,
            "open-secondary ratio ≈ k·√(L2/L1) = 1.998 at {f} Hz, got {ratio}"
        );
    }
}

/// A coupling coefficient with |k| > 1 is unphysical and fails loud.
#[test]
fn xfmr_bad_k_is_loud() {
    let phdl = "use piperine::disciplines;
use spice::passives;
mod Top() {
    wire gnd: Electrical; wire a: Electrical; wire b: Electrical;
    t1: xfmr (.p1=a,.n1=gnd,.p2=b,.n2=gnd) { .l1 = 1.0e-6, .l2 = 1.0e-6, .k = 1.5 };
}";
    let err = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect_err("|k| > 1 must fail loud");
    assert!(format!("{err}").contains("k"), "loud error names k: {err:?}");
}

/// A non-positive winding inductance fails loud.
#[test]
fn xfmr_bad_inductance_is_loud() {    let phdl = "use piperine::disciplines;
use spice::passives;
mod Top() {
    wire gnd: Electrical; wire a: Electrical; wire b: Electrical;
    t1: xfmr (.p1=a,.n1=gnd,.p2=b,.n2=gnd) { .l1 = 0.0, .l2 = 1.0e-6, .k = 0.9 };
}";
    let err = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect_err("l1 = 0 must fail loud");
    assert!(format!("{err}").contains("l1"), "loud error names l1: {err:?}");
}

/// SC-17/SC-21 — coupled-LC energy transfer through the mutual-flux engine
/// under TR-BDF2: two identical lossless tanks (L = 10 µH, C = 10 nF,
/// k = 0.5), the primary pre-charged to 1 V. The tanks' mode split
/// (ω0/√(1±k)) sloshes the energy fully between them with the first
/// secondary peak at π/(ω_a − ω_s) ≈ 1.66 µs. The TR-stage flux companion's
/// previous-voltage dual form is what keeps the tank frequencies (and thus
/// the transfer timing/envelope) correct.
#[test]
fn xfmr_coupled_lc_energy_transfer() {
    let phdl = "use piperine::disciplines;
use spice::passives;
mod Top() {
    wire gnd: Electrical; wire v1: Electrical; wire v2: Electrical;
    c1: cap   (.p=v1,.n=gnd) { .c = 1.0e-8 };
    c2: cap   (.p=v2,.n=gnd) { .c = 1.0e-8 };
    t1: xfmr  (.p1=v1,.n1=gnd,.p2=v2,.n2=gnd) { .l1 = 1.0e-5, .l2 = 1.0e-5, .k = 0.5 };
}";
    let design = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect("coupled tanks elaborate");
    let ic = std::collections::HashMap::from([("v1".to_string(), 1.0)]);
    let trace = SimSession::new(design, "Top".to_string())
        .run_tran(12.0e-6, Some(1.0e-8), 0.0, &SolverConfig::default(), Some(&ic), false)
        .unwrap_or_else(|e| panic!("coupled tanks tran failed: {e}"));

    let v1 = trace.v(&NetRef { name: "v1".into() }, None).expect("v(v1)");
    let v2 = trace.v(&NetRef { name: "v2".into() }, None).expect("v(v2)");
    let pts1 = v1.points();
    let pts2 = v2.points();

    // Bounded: a lossless tank never exceeds its initial stored voltage.
    for ((t, a), (_, b)) in pts1.iter().zip(pts2.iter()) {
        assert!(a.abs() <= 1.1 && b.abs() <= 1.1, "t = {t:e}: energy created: v1 = {a}, v2 = {b}");
    }
    // Transfer: the secondary rings up to ≥ 0.7 of the initial charge while
    // the primary collapses — full slosh is 1.0 for identical tanks. The
    // first beat (t ≤ 3 µs) is the one the analytic timing pins.
    let first_beat: Vec<(f64, f64)> =
        pts2.iter().filter(|(t, _)| *t <= 3.0e-6).cloned().collect();
    let (peak_t, v2_peak) = first_beat
        .iter()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|&(t, v)| (t, v.abs()))
        .expect("non-empty first beat");
    assert!(v2_peak >= 0.7, "secondary peak = {v2_peak}, want ≥ 0.7 (full transfer is 1.0)");
    let v1_at_peak = pts1
        .iter()
        .min_by(|(ta, _), (tb, _)| (ta - peak_t).abs().total_cmp(&(tb - peak_t).abs()))
        .map(|&(_, v)| v)
        .expect("non-empty trace");
    assert!(
        v1_at_peak.abs() < 0.35,
        "at the secondary peak the primary has collapsed: v1 = {v1_at_peak}"
    );
    // Timing discriminates the TR-stage dual form: with the previous-voltage
    // term the first transfer peaks at 1.36 µs; dropping it (the pure
    // backward-difference companion) doubles the derivative estimate and the
    // peak slips to 1.69 µs. The window sits between the two.
    assert!(
        (1.1e-6..=1.55e-6).contains(&peak_t),
        "first transfer peak at t = {peak_t:e}, reference 1.36 µs (mutant: 1.69 µs)"
    );
}

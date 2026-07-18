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
fn xfmr_bad_inductance_is_loud() {
    let phdl = "use piperine::disciplines;
use spice::passives;
mod Top() {
    wire gnd: Electrical; wire a: Electrical; wire b: Electrical;
    t1: xfmr (.p1=a,.n1=gnd,.p2=b,.n2=gnd) { .l1 = 0.0, .l2 = 1.0e-6, .k = 0.9 };
}";
    let err = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect_err("l1 = 0 must fail loud");
    assert!(format!("{err}").contains("l1"), "loud error names l1: {err:?}");
}

//! SC-20 — host-level `.temp` sweep: the shared `Context` temperature is set
//! per analysis and the stdlib models' tnom rescaling flows from it. Proof: a
//! 1 mA-biased silicon diode's forward drop shifts ≈ −2 mV/°C over a
//! 27 → 77 °C sweep, monotonically.

use std::path::PathBuf;

use piperine_api::{NetRef, SimSession, SolverConfig};
use piperine_lang::SourceMap;

/// 5 V through 1 kΩ into a default-model silicon diode (IS = 1e-14): the
/// bias current (≈ 4.3 mA) is set by the resistor, which barely moves as the
/// junction voltage shifts — an approximately fixed operating current that
/// isolates the diode law's temperature dependence (and converges from a
/// cold start, unlike a bare current source into a zero-conductance
/// junction).
const DIODE_BIAS: &str = r#"
    use piperine::disciplines;
    use spice::sources;
    use spice::passives;
    use spice::diode;

    mod TempBias() {
        wire gnd: Electrical; wire vin: Electrical; wire vd: Electrical;
        vs: vsrc (.p=vin,.n=gnd) { .dc = 5.0 };
        rb: res  (.p=vin,.n=vd)  { .r = 1.0e3 };
        d1: dio  (.p=vd,.n=gnd)  {};
    }
"#;

/// A source map rooted at the real stdlib headers (same shape as the root
/// host suites).
fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Forward drop of the 1 mA-biased diode at `t_kelvin` (temperature set on
/// the per-analysis config — the host `.temp` sweep mechanism).
fn vf_at(t_kelvin: f64) -> f64 {
    let design = piperine_lang::parse_and_elaborate(DIODE_BIAS, &headers_source_map())
        .expect("fixture elaborates");
    let session = SimSession::new(design, "TempBias".to_string());
    let config = SolverConfig { temperature: t_kelvin, ..Default::default() };
    session
        .run_op(&config, None)
        .expect("op solves")
        .v(&NetRef { name: "vd".into() }, None)
        .expect("v(vd)")
}

#[test]
fn temp_sweep_diode_forward_drop_shifts_minus_2mv_per_c() {
    let t0 = 300.15; // 27 °C
    let t1 = 350.15; // 77 °C
    let steps = 10;

    let first = vf_at(t0);
    assert!(
        (0.5..0.8).contains(&first),
        "forward drop at 27 °C sits on the forward branch: {first}"
    );

    let mut prev = first;
    for i in 1..=steps {
        let t = t0 + (t1 - t0) * (i as f64) / (steps as f64);
        let v = vf_at(t);
        assert!(v < prev, "forward drop falls with temperature: {v} !< {prev} at {t} K");
        prev = v;
    }

    let coef_mv_per_k = (prev - first) / (t1 - t0) * 1.0e3;
    assert!(
        (-2.5..=-1.5).contains(&coef_mv_per_k),
        "dVf/dT = {coef_mv_per_k} mV/K over 27→77 °C, want ≈ −2 mV/°C"
    );
}

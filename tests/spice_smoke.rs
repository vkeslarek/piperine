//! spice-stdlib SPICE-03: the smoke circuits (junction + validate, ported
//! from the retired external package) converge and measure correctly
//! in-process through the builtin `use spice::…` namespace — driven through
//! the root host API ([`SimSession`]), with the same assertions the
//! bench-block fixtures carried.

use std::path::PathBuf;

use piperine::{NetRef, OpResult, SimSession, SolverConfig};
use piperine_lang::{Design, SourceMap};

/// A source map rooted at the real stdlib headers, mirroring what
/// `piperine-project` builds for a project.
fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Elaborate a fixture from `tests/spice/`.
fn elaborate(name: &str) -> Design {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/spice")).join(name);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    piperine_lang::parse_and_elaborate(&src, &headers_source_map())
        .unwrap_or_else(|e| panic!("{name}: elaboration failed: {e:?}"))
}

/// DC operating point of one module in the design.
fn op(design: &Design, module: &str) -> OpResult {
    SimSession::new(design.fork(), module.to_string())
        .run_op(&SolverConfig::default(), None)
        .unwrap_or_else(|e| panic!("{module}: op failed: {e}"))
}

/// Ground-referenced node voltage by net name.
fn v(op: &OpResult, net: &str) -> f64 {
    op.v(&NetRef { name: net.to_string() }, None)
        .unwrap_or_else(|e| panic!("v({net}): {e}"))
}

/// 5 V through 1 kΩ into a 1e-14 A diode: vd ≈ 0.69 V (Shockley forward
/// drop, pnjlim-converged).
#[test]
fn spice_diode_forward_drop() {
    let design = elaborate("junction.phdl");
    let vd = v(&op(&design, "DioBias"), "out");
    assert!((0.55..0.80).contains(&vd), "Shockley forward drop, got {vd}");
}

/// Diode-connected NPN (base = collector): a single effective B-E junction
/// drop.
#[test]
fn spice_bjt_diode_drop() {
    let design = elaborate("junction.phdl");
    let vbe = v(&op(&design, "BjtDiode"), "bc");
    assert!((0.60..0.95).contains(&vbe), "diode-connected BJT junction drop, got {vbe}");
}

/// NMOS diode-connected (gate=drain): channel + bulk junctions, above
/// threshold.
#[test]
fn spice_mos1_diode_bias() {
    let design = elaborate("junction.phdl");
    let vds = v(&op(&design, "MosBias"), "d");
    assert!((1.0..5.0).contains(&vds), "NMOS diode-connected above threshold, got {vds}");
}

/// JFET: gate junction reverse-biased, channel conducts.
#[test]
fn spice_jfet_bias_conducts() {
    let design = elaborate("junction.phdl");
    let vds = v(&op(&design, "JfetBias"), "d");
    assert!((0.0..5.0).contains(&vds), "JFET conducts, got {vds}");
}

/// Resistive divider: `10 V · 3k/(1k+3k) = 7.5 V`.
#[test]
fn spice_divider_ratio() {
    let design = elaborate("validate.phdl");
    let mid = v(&op(&design, "Divider"), "mid");
    assert!((mid - 7.5).abs() < 1.0e-6, "10 V · 3k/(1k+3k) = 7.5 V, got {mid}");
}

/// RC low-pass at its corner: passband gain ≈ 1 at 1 kHz, −3 dB at 100 kHz.
#[test]
fn spice_rc_lowpass_corner() {
    let design = elaborate("validate.phdl");
    let trace = SimSession::new(design.fork(), "RcLowpass".to_string())
        .run_ac(1.0e3, 1.0e6, 40, true, &SolverConfig::default())
        .unwrap_or_else(|e| panic!("RcLowpass: ac failed: {e}"));
    let mag = trace
        .v(&NetRef { name: "out".to_string() }, None)
        .unwrap_or_else(|e| panic!("RcLowpass: v(out): {e}"))
        .mag();
    let at_1k = mag.at(1.0e3);
    let at_100k = mag.at(1.0e5);
    assert!(at_1k > 0.99, "passband gain ≈ 1, got {at_1k}");
    assert!((0.69..0.72).contains(&at_100k), "−3 dB at the corner, got {at_100k}");
}

/// VCVS gain stage: `4 · 0.5 V = 2 V`.
#[test]
fn spice_vcvs_gain() {
    let design = elaborate("validate.phdl");
    let out = v(&op(&design, "Amp"), "out");
    assert!((out - 2.0).abs() < 1.0e-6, "VCVS: 4 · 0.5 V = 2 V, got {out}");
}

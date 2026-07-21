//! `.disto` host surface (DISTO-06): `SimSession::run_disto` — the cubic
//! VCCS stage's HD2/HD3 against the closed-form Volterra predictions,
//! driven entirely through `piperine::` (real PHDL → JIT disto2/disto3
//! kernels → solver → host).

use piperine::{SimSession, SolverConfig};
use piperine_lang::SourceMap;

const POLY_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; param acmag: Real = 0.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 50.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod PolyVccs(inout inp: Electrical, inout inn: Electrical,
             inout outp: Electrical, inout outn: Electrical) {
    param g1: Real = 0.1;
    param g2: Real = 0.02;
    param g3: Real = 0.003;
}
analog PolyVccs {
    I(outp, outn) <+ g1 * V(inp, inn)
                   + g2 * V(inp, inn) * V(inp, inn)
                   + g3 * V(inp, inn) * V(inp, inn) * V(inp, inn);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire vout : Electrical;
    v1 : V(.p = vin, .n = gnd) { .dc = 0.0, .acmag = 1.0 };
    n1 : PolyVccs(.inp = vin, .inn = gnd, .outp = vout, .outn = gnd) {};
    r1 : R(.p = vout, .n = gnd) { .r = 50.0 };
}
";

#[test]
fn cubic_stage_hd2_hd3_match_closed_form_through_the_host() {
    let design = piperine_lang::parse_and_elaborate(POLY_PHDL, &SourceMap::dummy()).expect("poly stage elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let amplitude = 0.1;
    let result = session
        .run_disto(1e6, None, amplitude, "vout", None, &SolverConfig::default())
        .expect(".disto solves on the cubic stage");

    // Zero bias: HD2 = ½·(g2/g1)·A, HD3 = ¼·(g3/g1)·A² (DISTO-05).
    let (g1, g2, g3) = (0.1_f64, 0.02_f64, 0.003_f64);
    let expected_hd2 = 0.5 * (g2 / g1) * amplitude;
    let expected_hd3 = 0.25 * (g3 / g1) * amplitude * amplitude;

    let hd2 = result.hd2.expect("single-tone reports HD2");
    let hd3 = result.hd3.expect("single-tone reports HD3");
    assert!(
        (hd2 - expected_hd2).abs() / expected_hd2 < 1e-3,
        "HD2 = {hd2}, closed form {expected_hd2}"
    );
    assert!(
        (hd3 - expected_hd3).abs() / expected_hd3 < 1e-3,
        "HD3 = {hd3}, closed form {expected_hd3}"
    );
    assert!(result.im2.is_none() && result.im3.is_none(), "single-tone reports HD only");
}

#[test]
fn cubic_stage_two_tone_reports_im2_im3() {
    let design = piperine_lang::parse_and_elaborate(POLY_PHDL, &SourceMap::dummy()).expect("poly stage elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result = session
        .run_disto(1e6, Some(1.1e6), 0.1, "vout", None, &SolverConfig::default())
        .expect("two-tone .disto solves");

    // The controlling node is pinned by the ideal source (no second-order
    // response there), so IM2/IM3 come from the derivative terms alone:
    // IM2 = ½·f''·A² / (f'·A) = (g2/g1)·A (two-tone ½ mix, twice HD2),
    // IM3 = (f'''·A³/24) / (f'·A) = ¼·(g3/g1)·A².
    let (g1, g2, g3) = (0.1_f64, 0.02_f64, 0.003_f64);
    let amplitude = 0.1_f64;
    let expected_im2 = (g2 / g1) * amplitude;
    let expected_im3 = 0.25 * (g3 / g1) * amplitude * amplitude;

    let im2 = result.im2.expect("two-tone reports IM2");
    let im3 = result.im3.expect("two-tone reports IM3");
    assert!(
        (im2 - expected_im2).abs() / expected_im2 < 1e-3,
        "IM2 = {im2}, closed form {expected_im2}"
    );
    assert!(
        (im3 - expected_im3).abs() / expected_im3 < 1e-3,
        "IM3 = {im3}, closed form {expected_im3}"
    );
    assert!(result.hd2.is_none() && result.hd3.is_none(), "two-tone reports IM only");
}

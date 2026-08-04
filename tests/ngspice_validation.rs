//! ngspice golden cross-validation (spice-stdlib SPICE-05..08).
//!
//! Each circuit pair in `tests/ngspice/` describes the *same* circuit twice:
//! `<name>.cir` for ngspice (the golden reference, run as a subprocess) and
//! `<name>.phdl` for piperine (elaborated and solved in-process — node
//! voltages are read from the result objects, never parsed from stdout).
//! Every node ngspice prints must match piperine within
//! `|Δ| ≤ abstol + reltol·max(|a|,|b|)`.
//!
//! When `ngspice` is not on PATH every golden test prints a SKIP notice and
//! passes — the binary cannot be a hard dependency. All other failure modes
//! are loud: unparseable ngspice output, zero shared nodes (contract
//! violation), piperine non-convergence, per-node mismatch.
//!
//! The comparison machinery itself lives in `tests/support/ngspice.rs` so the
//! per-domain targets share one harness; this target owns the DC operating
//! point, the DC sweeps, the spectral cases and the harness's failure modes.

use std::collections::BTreeMap;
use std::path::PathBuf;

// The harness returns host result objects; naming `OpResult` here is what
// keeps this target a shell proof (MD-28) now that the `piperine::` imports
// the harness needed have moved into `support::ngspice`.
use piperine::OpResult;

mod support;

use support::ngspice::{NgspiceHarness, ToleranceContract};

/// Run one OP circuit against live ngspice, or skip (and pass) without it.
fn ngspice_op_case(circuit: &str) {
    match NgspiceHarness::detect() {
        Some(harness) => harness.op_case(circuit),
        None => eprintln!("SKIP {circuit}: ngspice not on PATH"),
    }
}

// ── Golden OP circuits (SPICE-05) ───────────────────────────────────────────

#[test]
fn ngspice_divider() {
    ngspice_op_case("divider");
}

#[test]
fn ngspice_rdiode() {
    ngspice_op_case("rdiode");
}

#[test]
fn ngspice_diode_series() {
    ngspice_op_case("diode_series");
}

#[test]
fn ngspice_nmos_fixed() {
    ngspice_op_case("nmos_fixed");
}

#[test]
fn ngspice_nmos_load() {
    ngspice_op_case("nmos_load");
}

#[test]
fn ngspice_nmos2_fixed() {
    ngspice_op_case("nmos2_fixed");
}

#[test]
fn ngspice_nmos2_load() {
    ngspice_op_case("nmos2_load");
}

#[test]
fn ngspice_nmos3_fixed() {
    ngspice_op_case("nmos3_fixed");
}

#[test]
fn ngspice_nmos3_load() {
    ngspice_op_case("nmos3_load");
}

#[test]
fn ngspice_jfet_bias() {
    ngspice_op_case("jfet_bias");
}

#[test]
fn ngspice_bjt_ce() {
    ngspice_op_case("bjt_ce");
}

#[test]
fn ngspice_bjt_mirror() {
    ngspice_op_case("bjt_mirror");
}

// ── URC lumped RC line (FLAT-04) ───────────────────────────────────────────
//
// Three DC operating-point cross-checks of the FlattenHierarchy pass at
// work: each `Top` instantiates a mid-level `urcN` module that the flatten
// pass inlines into N series `res` + N shunt `cap` leaves. ngspice sees the
// SAME ladder topology built from discrete R/C elements (`urc_lumpN.cir`),
// so the DC operating point must match exactly. Each lump value yields a
// distinct Vout (Vout = 5·1000/(100·N+1000)) — the test is discriminating:
// dropping or duplicating a segment shifts Vout by ≥0.13 V, well outside
// the harness's 1e-3 reltol. Capacitors are open at DC, so the comparison
// isolates the flatten pass's structural correctness.

#[test]
fn ngspice_urc_lump2() {
    ngspice_op_case("urc_lump2");
}

#[test]
fn ngspice_urc_lump5() {
    ngspice_op_case("urc_lump5");
}

#[test]
fn ngspice_urc_lump10() {
    ngspice_op_case("urc_lump10");
}

// ── DC sweep circuits (SPICE-08) ────────────────────────────────────────────

/// Diode I–V (forward + reverse, 37 points): ngspice `.dc` + `wrdata` export
/// vs piperine staging `v1.dc` per point — the source branch current must
/// match within reltol 1e-3 + abstol 1e-9 A.
#[test]
fn ngspice_diode_iv_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("diode_iv", "v1", "vin", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP diode_iv: ngspice not on PATH"),
    }
}

/// NMOS Id–Vgs (21 points, vds = 2 V, bulk at −1 V): cutoff → saturation →
/// linear with body effect — the harness stages `vg.dc` per point and reads
/// the drain supply's branch current (SPICE-10).
#[test]
fn ngspice_nmos_id_vgs_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos_id_vgs", "vg", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP nmos_id_vgs: ngspice not on PATH"),
    }
}

/// NMOS Id–Vds (26 points, vgs = 3 V, rd/rs = 100 Ω): linear → saturation,
/// exercising the series-resistance force branches (SPICE-10).
#[test]
fn ngspice_nmos_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos_id_vds", "vd", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP nmos_id_vds: ngspice not on PATH"),
    }
}

/// NMOS level 2 Id–Vds (26 points, vgs = 3 V, rd/rs = 100 Ω): linear →
/// saturation with the level-2 channel-length modulation (SC-14).
#[test]
fn ngspice_nmos2_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos2_id_vds", "vd", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP nmos2_id_vds: ngspice not on PATH"),
    }
}

/// NMOS level 2 Id–Vgs (26 points, vds = 3 V, nsub + nfs): cutoff →
/// subthreshold → strong inversion (SC-14).
#[test]
fn ngspice_nmos2_id_vgs_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos2_id_vgs", "vg", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP nmos2_id_vgs: ngspice not on PATH"),
    }
}

/// NMOS level 3 Id–Vds (26 points, vgs = 3 V, rd/rs = 100 Ω): short-channel
/// physics (theta, eta, kappa, vmax, xj) across linear → saturation (SC-14).
#[test]
fn ngspice_nmos3_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos3_id_vds", "vd", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP nmos3_id_vds: ngspice not on PATH"),
    }
}

/// NMOS level 3 Id–Vgs (26 points, vds = 3 V, nsub + nfs): cutoff →
/// subthreshold → strong inversion with short-channel physics (SC-14).
#[test]
fn ngspice_nmos3_id_vgs_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos3_id_vgs", "vg", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP nmos3_id_vgs: ngspice not on PATH"),
    }
}

/// N-JFET Id–Vds (26 points, vgs = −0.5 V, rd/rs = 100 Ω): linear →
/// saturation through the series-resistance force branches (SPICE-11).
#[test]
fn ngspice_jfet_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("jfet_id_vds", "vd", "d", "gnd", &ToleranceContract::DC_CURRENT)
        }
        None => eprintln!("SKIP jfet_id_vds: ngspice not on PATH"),
    }
}

// ── Spectral analyses (.disto/.four/.pz) — spectral-analyses T15 ─────────

/// `.disto`: a hard-biased diode's HD2/HD3 — piperine's Volterra ratios
/// against ngspice's harmonic components normalized by its AC fundamental
/// (both sides run the same nonlinear-currents method on the
/// equation-identical diode, 1 % tolerance).
#[test]
fn ngspice_disto_diode() {
    match NgspiceHarness::detect() {
        Some(harness) => harness.disto_case("disto_diode", 1e6, &ToleranceContract::DISTO_HD),
        None => eprintln!("SKIP disto_diode: ngspice not on PATH"),
    }
}

/// `.four`: the hard-driven diode stage's THD — piperine's transient +
/// `Waveform::fourier` against ngspice's `fourier` over the last period
/// (integrator-pair tolerance 2 %).
#[test]
fn ngspice_four_diode() {
    match NgspiceHarness::detect() {
        Some(harness) => harness.four_case("four_diode", 1e6, 3e-6, &ToleranceContract::FOUR_THD),
        None => eprintln!("SKIP four_diode: ngspice not on PATH"),
    }
}

/// `.pz`: the RC network's single natural pole at −500 rad/s — piperine's
/// QZ poles against ngspice's pole-zero analysis (1e-3 relative).
#[test]
fn ngspice_pz_rc() {
    match NgspiceHarness::detect() {
        Some(harness) => harness.pz_case("pz_rc", &ToleranceContract::PZ_POLE),
        None => eprintln!("SKIP pz_rc: ngspice not on PATH"),
    }
}

// ── Harness failure modes (SPICE-06/SPICE-07) ───────────────────────────────

/// SPICE-06: without a binary on the (injected) PATH, detection yields the
/// skip branch — the case runner prints a notice and passes.
#[test]
fn ngspice_absent_takes_the_skip_branch() {
    let empty = std::env::temp_dir();
    let path = Some(std::env::join_paths([&empty]).unwrap());
    assert!(
        NgspiceHarness::detect_with_path(path).is_none(),
        "no ngspice in an empty PATH must select the skip branch"
    );
    assert!(NgspiceHarness::detect_with_path(None).is_none(), "unset PATH must skip");
}

/// Edge case: unparseable ngspice output fails loud with a raw excerpt,
/// never an empty comparison set.
#[test]
fn ngspice_unparseable_output_fails_loud() {
    let err = NgspiceHarness::parse_op_output("bogus", "Note: nothing to see here\n")
        .expect_err("no v(node) lines must be an error");
    assert!(err.contains("bogus"), "names the circuit: {err}");
    assert!(err.contains("nothing to see here"), "carries a raw excerpt: {err}");
}

/// SPICE-07 edge case: both sides agree on 0 shared nodes → contract
/// violation, not a pass.
#[test]
fn ngspice_zero_shared_nodes_is_a_contract_violation() {
    let golden = BTreeMap::from([("out".to_string(), 1.0)]);
    let err = NgspiceHarness::compare_op("lonely", &golden, |_| None)
        .expect_err("0 shared nodes must fail");
    assert!(err.contains("lonely"), "names the circuit: {err}");
    assert!(err.contains("0 shared nodes"), "names the violation: {err}");
}

/// SPICE-07: a mismatch names the circuit, the node, both values and the
/// delta.
#[test]
fn ngspice_mismatch_failure_is_actionable() {
    let golden = BTreeMap::from([("out".to_string(), 3.0)]);
    let err = NgspiceHarness::compare_op("offby", &golden, |_| Some(1.92))
        .expect_err("out-of-tolerance node must fail");
    assert!(err.contains("offby"), "names the circuit: {err}");
    assert!(err.contains("v(out)"), "names the node: {err}");
    assert!(err.contains("3.0") && err.contains("1.92"), "shows both values: {err}");
    assert!(err.contains("Δ="), "shows the delta: {err}");
}

// The tolerance contract itself — `|Δ| ≤ abstol + reltol·max(|a|,|b|)`, its
// boundary, its near-zero floor and the per-domain rationales — is unit-covered
// beside the contract in `support::ngspice`'s `mod tests` (NGP-07).

// The `wrdata` parser's strictness is unit-covered beside the parser itself,
// in `support::ngspice`'s `mod tests` (SPICE-08 edge cases: malformed columns,
// unparseable numbers, empty export — plus the 3-column AC shape).

/// Regression guard for the DC device-bypass fix (runs without ngspice):
/// two identical series diodes must show equal drops. The old global-scale
/// bypass threshold froze stamps inside a `reltol·max|v|` window (5 mV
/// here), locking in a ~0.65 mV inconsistency between the two junctions.
#[test]
fn ngspice_series_junctions_are_self_consistent() {
    let harness_less = NgspiceHarness { exe: PathBuf::new() };
    let op: OpResult = harness_less.piperine_op("diode_series").unwrap_or_else(|e| panic!("{e}"));
    let va = op.v("a".to_string()).unwrap();
    let vb = op.v("b".to_string()).unwrap();
    let d1 = va - vb;
    let d2 = vb;
    assert!(
        (d1 - d2).abs() < 5e-4,
        "identical series diodes must drop equally: d1={d1:.6e} d2={d2:.6e} Δ={:.3e}",
        (d1 - d2).abs()
    );
}

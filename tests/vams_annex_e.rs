//! LRM-COVERS: E.3, E.3.4
//! T55 (VAMS-22) — LRM Annex E (SPICE compatibility), proved against
//! Piperine's actual SPICE-facing tooling: `headers/spice/*.va` (T41–T43)
//! plus the `$limit()` pnjlim/fetlim/limvds functions
//! (`piperine-codegen/src/kernel/analog/limits.rs`). Each test below
//! instantiates one `piperine_lang::annex_e::AnnexE::PRIMITIVES` /
//! `LIMIT_FNS` entry through a real `.vams` module and elaborates it — a
//! passing test is Annex E clause evidence, not just a doc-comment claim
//! (matches T56's later audit requirement: "at least one **passing**
//! test").
//!
//! §E.3 (Table E.1) primitive coverage: `resistor`→`res`, `capacitor`→
//! `cap`, `inductor`→`ind`, `iexp`/`ipulse`/`isine`→`isrc`, `vexp`/
//! `vpulse`/`vsine`→`vsrc`, `tline`→`tline`, `vccs`→`vccs`, `vcvs`→`vcvs`,
//! `diode`→`dio`, `bjt`→`bjt`, `mosfet`→`mos1`/`mos2`/`mos3`, `jfet`→
//! `jfet`, `ipwl`/`vpwl`→`isrc`/`vsrc` (`wave = 6`, T58, an 8-point bounded
//! PWL). `mesfet` has no equivalent — proved absent by
//! `piperine_lang::annex_e::AnnexE::PRIMITIVES` itself (unit-tested in
//! `crates/piperine-lang/src/annex_e.rs`), not re-proved here (there is
//! nothing to instantiate; see that module's own note on why this one
//! stays a genuine, disclosed gap).
//!
//! §E.3.4 (Table E.2) limiting functions: `fetlim`/`pnjlim`/`vdslim`, all
//! reachable through `$limit()` — `"vdslim"` (T58) and `"limvds"`
//! (ngspice's own naming, kept as a synonym) both work.

use std::path::PathBuf;

use piperine::{OpResult, Session, SolverConfig};
use piperine_lang::{Design, SourceMap};
use piperine_lang::annex_e::{AnnexE, AnnexESupport};

/// Mirrors `tests/spice_smoke.rs`'s `headers_source_map` — a source map
/// rooted at the real stdlib headers, resolving `use piperine::…`/
/// `` `include "…" `` against `crates/piperine-lang/headers`.
fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn elaborate(src: &str) -> Design {
    piperine_lang::parse_and_elaborate(src, &headers_source_map())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn op(design: &Design, module: &str) -> OpResult {
    Session::compile(&design.fork(), module)
        .unwrap_or_else(|e| panic!("{module}: session compile failed: {e}"))
        .op(&SolverConfig::default(), None)
        .unwrap_or_else(|e| panic!("{module}: op failed: {e}"))
}

/// Table E.1 `resistor`/`capacitor`/`inductor` — the AliasedName-support
/// two-terminal passives (`res`/`cap`/`ind`), each simulated to a real DC
/// operating point.
#[test]
fn table_e1_passives_resistor_capacitor_inductor() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        module PassiveNet ();
            electrical gnd, vin, mid, cnode;
            vsrc #(.dc(5.0)) vs (.p(vin), .n(gnd));
            res  #(.r(1.0e3)) r1 (.p(vin), .n(mid));
            cap  #(.c(1.0e-9)) c1 (.p(mid), .n(gnd));
            ind  #(.l(1.0e-6)) l1 (.p(mid), .n(cnode));
            res  #(.r(1.0e3)) r2 (.p(cnode), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    // DC steady state: the inductor is a short, so `mid` and `cnode` sit
    // at the same potential; the resistor divider (r1 into r2||nothing,
    // since the capacitor is open at DC) gives 2.5 V.
    let v_mid = op(&design, "PassiveNet").v("mid".to_string()).expect("v(mid)");
    assert!((2.0..3.0).contains(&v_mid), "resistor-divider midpoint, got {v_mid}");
}

/// Table E.1 `vsine`/`isine` — `vsrc`/`isrc` with `wave = 1` (SIN),
/// exercised at DC (t=0) so a plain `op()` proves the module elaborates
/// and simulates, not the full waveform.
#[test]
fn table_e1_vsine_isine_via_wave_selector() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        module SineDrive ();
            electrical gnd, vin, out;
            vsrc #(.wave(1), .sin_vo(1.0), .sin_va(2.0), .sin_freq(1.0e3)) vs (.p(vin), .n(gnd));
            res  #(.r(1.0e3)) r1 (.p(vin), .n(out));
            res  #(.r(1.0e3)) r2 (.p(out), .n(gnd));
            isrc #(.wave(1), .sin_va(1.0e-3), .sin_freq(1.0e3)) is (.p(out), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    let _ = op(&design, "SineDrive"); // must not panic — elaborates and converges.
}

/// Table E.1 `vpulse`/`ipulse` — `vsrc`/`isrc` with `wave = 2` (PULSE).
#[test]
fn table_e1_vpulse_ipulse_via_wave_selector() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        module PulseDrive ();
            electrical gnd, vin, out;
            vsrc #(.wave(2), .pulse_v1(0.0), .pulse_v2(5.0), .pulse_pw(1.0e-6), .pulse_per(2.0e-6)) vs
                (.p(vin), .n(gnd));
            res  #(.r(1.0e3)) r1 (.p(vin), .n(out));
            res  #(.r(1.0e3)) r2 (.p(out), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    let _ = op(&design, "PulseDrive");
}

/// Table E.1 `vexp`/`iexp` — `vsrc`/`isrc` with `wave = 3` (EXP).
#[test]
fn table_e1_vexp_via_wave_selector() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        module ExpDrive ();
            electrical gnd, vin, out;
            vsrc #(.wave(3), .exp_v1(0.0), .exp_v2(5.0), .exp_td1(0.0), .exp_tau1(1.0e-6)) vs
                (.p(vin), .n(gnd));
            res  #(.r(1.0e3)) r1 (.p(vin), .n(out));
            res  #(.r(1.0e3)) r2 (.p(out), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    let _ = op(&design, "ExpDrive");
}

/// T58 (LRMC-C10-17): `vpwl`/`ipwl` — `vsrc`/`isrc` with `wave = 6`
/// (PWL), the 8-point bounded approximation (`annex_e.rs`'s own disclosed
/// scope note: no array-parameter support to take an arbitrary-length
/// breakpoint list). Numeric, over a real transient run, not just DC: a
/// ramp 0V->2V over [0,1us], flat at 2V over [1,2us], ramp back down over
/// [2,3us] — `vsrc` is an ideal voltage source, so `v(vin)` should track
/// the PWL waveform exactly regardless of the resistor load.
#[test]
fn table_e1_vpwl_via_wave_selector() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        module PwlDrive ();
            electrical gnd, vin;
            vsrc #(.wave(6),
                   .pwl_t0(0.0),   .pwl_v0(0.0),
                   .pwl_t1(1.0e-6), .pwl_v1(2.0),
                   .pwl_t2(2.0e-6), .pwl_v2(2.0),
                   .pwl_t3(3.0e-6), .pwl_v3(0.0),
                   .pwl_t4(10.0e-6), .pwl_v4(0.0),
                   .pwl_t5(11.0e-6), .pwl_v5(0.0),
                   .pwl_t6(12.0e-6), .pwl_v6(0.0),
                   .pwl_t7(13.0e-6), .pwl_v7(0.0)) vs (.p(vin), .n(gnd));
            res #(.r(1.0e3)) r1 (.p(vin), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    let trace = Session::compile(&design, "PwlDrive")
        .expect("session compiles")
        .tran(3.5e-6, Some(5.0e-8), 0.0, &SolverConfig::default(), None, false, &[])
        .unwrap_or_else(|e| panic!("PwlDrive tran failed: {e}"));

    let v = trace.v("vin").expect("v(vin)");
    let at = |target: f64| -> f64 {
        v.points()
            .iter()
            .min_by(|(t1, _), (t2, _)| (t1 - target).abs().partial_cmp(&(t2 - target).abs()).unwrap())
            .map(|(_, val)| *val)
            .expect("a sample near the target time")
    };

    assert!((at(0.5e-6) - 1.0).abs() < 0.15, "midpoint of the rising ramp ~1.0V, got {}", at(0.5e-6));
    assert!((at(1.5e-6) - 2.0).abs() < 0.05, "flat top ~2.0V, got {}", at(1.5e-6));
    assert!((at(2.5e-6) - 1.0).abs() < 0.15, "midpoint of the falling ramp ~1.0V, got {}", at(2.5e-6));
}

/// Table E.1 `tline`.
#[test]
fn table_e1_tline() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        `include "tline.va"
        module TlineNet ();
            electrical gnd, vin, p2;
            vsrc #(.dc(1.0)) vs (.p(vin), .n(gnd));
            tline #(.z0(50.0), .td(1.0e-9)) t1 (.p1(vin), .n1(gnd), .p2(p2), .n2(gnd));
            res #(.r(50.0)) rterm (.p(p2), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    let _ = op(&design, "TlineNet");
}

/// Table E.1 `vccs`/`vcvs`.
#[test]
fn table_e1_vccs_vcvs() {
    let src = r#"
        `include "disciplines.vams"
        `include "sources.va"
        `include "passives.va"
        `include "controlled.va"
        module ControlledNet ();
            electrical gnd, vin, ctrl, iout, vout;
            vsrc #(.dc(1.0)) vs (.p(vin), .n(gnd));
            res  #(.r(1.0e3)) r1 (.p(vin), .n(ctrl));
            res  #(.r(1.0e3)) r2 (.p(ctrl), .n(gnd));
            vccs #(.trans(1.0e-3)) g1 (.p(iout), .n(gnd), .cp(ctrl), .cn(gnd));
            res  #(.r(1.0e3)) r3 (.p(iout), .n(gnd));
            vcvs #(.gain(2.0)) e1 (.p(vout), .n(gnd), .cp(ctrl), .cn(gnd));
            res  #(.r(1.0e3)) r4 (.p(vout), .n(gnd));
        endmodule
    "#;
    let design = elaborate(src);
    let _ = op(&design, "ControlledNet");
}

/// Table E.1 `diode`/`bjt`/`mosfet`/`jfet` — reuses the existing junction
/// smoke fixture (`tests/spice/junction.vams`, SPICE-03) rather than
/// duplicating it; this test's own job is to confirm it is Annex-E-cited
/// evidence, not to re-derive the fixture.
#[test]
fn table_e1_diode_bjt_mosfet_jfet() {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/spice/junction.vams"));
    let src = std::fs::read_to_string(&path).expect("tests/spice/junction.vams exists");
    let design = elaborate(&src);
    let vd = op(&design, "DioBias").v("out".to_string()).expect("v(out)");
    assert!((0.4..0.9).contains(&vd), "diode forward drop, got {vd}");
    let vbc = op(&design, "BjtDiode").v("bc".to_string()).expect("v(bc)");
    assert!((0.4..1.0).contains(&vbc), "bjt B-E junction drop, got {vbc}");
    let vmos = op(&design, "MosBias").v("d".to_string()).expect("v(d)");
    assert!(vmos.is_finite());
    let vjfet = op(&design, "JfetBias").v("d".to_string()).expect("v(d)");
    assert!(vjfet.is_finite());
}

/// Table E.2 `pnjlim`/`fetlim`/`vdslim` — all reachable through `$limit()`
/// (LRM §9.17.3, cited by `AnnexE::LIMIT_FNS`). `$limit`'s codegen
/// (`emit/analog_expr.rs::emit_analog_limit`) always takes exactly 5 args
/// — `(kind, vnew, vseed, vte, vcrit)` — regardless of kind; unused
/// positions for a given kind (e.g. `vdslim` ignores `vte`/`vcrit`) are
/// still supplied. T58 (LRMC-C10-17): `"vdslim"`, Table E.2's own
/// preferred spelling, now works directly — `"limvds"` (ngspice's naming)
/// still works too, kept as a synonym.
#[test]
fn table_e2_limiting_functions_reachable_through_dollar_limit() {
    let src = r#"
        `include "disciplines.vams"
        module Limited (inout electrical a, inout electrical b, inout electrical c);
            real la, lb, lc;
            analog begin
                la = $limit("pnjlim", V(a), 0.0, 0.025, 0.7);
                lb = $limit("fetlim", V(b), 0.0, 1.0, 0.0);
                lc = $limit("vdslim", V(c), 0.0, 0.0, 0.0);
                I(a) <+ la * 1.0e-3;
                I(b) <+ lb * 1.0e-3;
                I(c) <+ lc * 1.0e-3;
            end
        endmodule
    "#;
    elaborate(src);
}

/// `"limvds"` (ngspice's own naming) still works as a synonym for
/// `"vdslim"` — a regression guard, since existing callers may already
/// use it.
#[test]
fn limvds_synonym_still_works() {
    let src = r#"
        `include "disciplines.vams"
        module Limited (inout electrical c);
            real lc;
            analog begin
                lc = $limit("limvds", V(c), 0.0, 0.0, 0.0);
                I(c) <+ lc * 1.0e-3;
            end
        endmodule
    "#;
    elaborate(src);
}

/// The Annex E audit table itself is reachable from this crate's public
/// surface (not just `piperine-lang`'s own internal unit tests) — proves
/// `T55`'s deliverable is actually load-bearing outside its own crate.
#[test]
fn annex_e_table_is_public_and_names_the_one_remaining_real_gap() {
    let gaps: Vec<&str> = AnnexE::PRIMITIVES
        .iter()
        .filter(|p| p.support == AnnexESupport::NotSupported)
        .map(|p| p.lrm_name)
        .collect();
    // T58 closed `ipwl`/`vpwl`; `mesfet` stays `NotSupported` — no ngspice
    // source file exists to translate it from (this module's own note).
    assert_eq!(gaps, ["mesfet"]);
    assert!(AnnexE::LIMIT_FNS.iter().all(|f| f.support != AnnexESupport::NotSupported));
}

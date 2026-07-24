//! HOST-06 — parity test scaffold: the canonical analysis-method list is a
//! single source of truth checked against **both** public surfaces.
//!
//! Rust has no runtime reflection, so "the Rust `Session` has method `X`" is
//! proven the loudest way Rust can prove it: `assert_rust_session_has_every_
//! analysis` below calls every name in [`ANALYSES`] by hand — if a future
//! change removed one of `Session`'s analysis methods, this file would fail
//! to **compile**, not just fail a runtime assertion. The Python side is
//! checked at runtime (`hasattr`) against the same canonical list, embedded
//! through `piperine_python::embed::run_script` (mirrors
//! `piperine-python/tests/facade_hygiene.rs`'s native/facade parity
//! technique, extended cross-crate). `cargo test -p piperine host_parity`
//! (Phase 1 / T8 full gate).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// The uniform analysis-method surface (HOST-02): every name here must
/// exist, spelled identically, as a method on both hosts' compiled session.
/// This is the parity oracle both checks below read from.
const ANALYSES: &[&str] = &["op", "tran", "ac", "noise", "sens", "pss", "pz", "disto", "sp", "tf", "dc"];

const RLC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; param acmag: Real = 1.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod L(inout p: Electrical, inout n: Electrical) { param l: Real = 1e-3; }
analog L { V(p, n) <- l * ddt(I(p, n)); }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    wire b   : Electrical;
    V1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 10.0 };
    l1 : L(.p = a, .n = b) { .l = 1e-3 };
    c1 : C(.p = b, .n = gnd) { .c = 1e-6 };
}
";

/// Every name in [`ANALYSES`], called on the Rust `Session` — a compile-time
/// proof (not merely a runtime one) that the Rust host carries the full set.
fn call_every_rust_analysis(session: &mut Session) {
    let config = SolverConfig::default();
    let _ = session.op(&config, None);
    let _ = session.tran(1e-3, Some(1e-5), 0.0, &config, None, false);
    let _ = session.ac(1.0, 1e6, 5, true, &config);
    let _ = session.noise("a", "gnd", 1.0, 1e6, 5, true, &config);
    let _ = session.sens(&["a"], &[("r1".to_string(), "r".to_string())], 1e-6, &config);
    let _ = session.pss(1e-3, 0.0, &config);
    let _ = session.pz("V1", "b", None, &config);
    let _ = session.disto(1e6, None, 0.1, "a", None, &config);
    let _ = session.sp(1e3, 1e9, 5, true, &config);
    let _ = session.tf("a", None, "V1", &config);
    let _ = session.dc("V1", "dc", &[1.0, 2.0], &config, None);
}

/// Run a Python script (through the embedded facade) that builds a `Session`
/// on the same fixture and checks `hasattr` for every name in [`ANALYSES`],
/// writing the space-separated list of **missing** names to `out_path`
/// (empty file = full parity).
fn python_missing_analyses(out_path: &std::path::Path) {
    let phdl_path = std::env::temp_dir().join("piperine_host_parity_rlc.phdl");
    std::fs::write(&phdl_path, RLC_PHDL).expect("write phdl fixture");
    let names_py = ANALYSES.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
session = design.compile()
names = [{names}]
missing = [n for n in names if not hasattr(session, n)]
with open({out:?}, "w") as f:
    f.write(" ".join(missing))
"#,
        phdl = phdl_path.to_str().unwrap(),
        names = names_py,
        out = out_path.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python parity probe runs");
    let _ = std::fs::remove_file(&phdl_path);
    let _ = std::fs::remove_file(&script_path);
}

/// HOST-06's positive case: every analysis in [`ANALYSES`] exists on the
/// Rust `Session` (compile-time, see [`call_every_rust_analysis`]) AND the
/// Python `Session` (runtime `hasattr`) — both hosts, same names.
#[test]
fn host_parity_analyses_match_on_both_hosts() {
    let design = piperine_lang::parse_and_elaborate(RLC_PHDL, &SourceMap::dummy()).expect("RLC elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    call_every_rust_analysis(&mut session); // fails to COMPILE if Rust drifts

    let out = std::env::temp_dir().join("piperine_host_parity_missing.txt");
    python_missing_analyses(&out);
    let missing = std::fs::read_to_string(&out).expect("read missing-analyses probe output");
    let _ = std::fs::remove_file(&out);
    assert!(
        missing.trim().is_empty(),
        "Python Session is missing analyses the Rust Session has: {missing} (MD-22 breach)"
    );
}

/// HOST-06's negative case (the "fails loud on a synthetic drift" done-when
/// criterion): checking for a bogus, never-implemented analysis name is
/// reported as missing by the same mechanism — proving the parity probe
/// actually discriminates rather than vacuously reporting "all present".
#[test]
fn host_parity_probe_flags_a_synthetic_missing_analysis() {
    let bogus = "definitely_not_a_real_analysis_xyz";
    let out = std::env::temp_dir().join("piperine_host_parity_synthetic.txt");
    let phdl_path = std::env::temp_dir().join("piperine_host_parity_synthetic.phdl");
    std::fs::write(&phdl_path, RLC_PHDL).expect("write phdl fixture");
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
session = design.compile()
missing = [] if hasattr(session, {bogus:?}) else [{bogus:?}]
with open({out:?}, "w") as f:
    f.write(" ".join(missing))
"#,
        phdl = phdl_path.to_str().unwrap(),
        bogus = bogus,
        out = out.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_synthetic_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python synthetic probe runs");
    let missing = std::fs::read_to_string(&out).expect("read synthetic probe output");
    for p in [phdl_path, script_path, out] {
        let _ = std::fs::remove_file(p);
    }
    assert_eq!(missing.trim(), bogus, "the probe must flag a genuinely absent analysis, not pass silently");
}

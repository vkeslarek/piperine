//! API-01/API-02 — the host API works when imported directly from
//! `piperine_api` (not through the root shell), and the crate's dependency
//! set is exactly {lang, codegen, solver}: no python, no cli, no project.

use piperine_api::{NetRef, SimSession, SolverConfig};
use piperine_lang::SourceMap;

/// Self-contained divider (own discipline + devices, no prelude):
/// `mid = 10·1k/(1k+1k) = 5.0 V`.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 10.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 1e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 1e3 };
}
";

/// A DC operating point runs through `piperine_api::…` directly: a 10 V
/// source over a 1k/1k divider reads 5 V at the midpoint.
#[test]
fn op_analysis_through_piperine_api() {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &map).expect("divider elaborates");
    let session = SimSession::new(design, "Divider".to_string());
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let mid = op.v(&NetRef { name: "mid".into() }, None).expect("v(mid)");
    assert!((mid - 5.0).abs() < 1e-9, "divider midpoint: {mid}");
}

/// The api crate's piperine dependency set is exactly lang/codegen/solver —
/// no python, cli, project, or plugin edge (MD-20 topology).
#[test]
fn dependency_set_is_lang_codegen_solver_only() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["tree", "-p", "piperine-api", "--prefix", "none", "-e", "normal"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    let tree = String::from_utf8_lossy(&out.stdout);
    for allowed in ["piperine-lang", "piperine-codegen", "piperine-solver"] {
        assert!(tree.contains(allowed), "expected dependency `{allowed}` missing:\n{tree}");
    }
    for forbidden in ["piperine-python", "piperine-cli", "piperine-project", "piperine-plugin"] {
        assert!(!tree.contains(forbidden), "forbidden dependency `{forbidden}` present:\n{tree}");
    }
}

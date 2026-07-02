//! Phase 3 — CLI integration tests.
//!
//! Exercises the `check` command against small AMS and PHDL fixtures
//! to confirm the front-door wiring (file detection + parse + summary).

use piperine_cli::commands::check::check_file;
use std::io::Write;
use std::path::PathBuf;

fn tmp_ams(name: &str, body: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join(name);
    let mut f = std::fs::File::create(&p).expect("create");
    f.write_all(body.as_bytes()).expect("write");
    // Keep the tempdir alive by leaking it.
    std::mem::forget(dir);
    p
}

#[test]
fn check_ppr_resistor_extracts_module() {
    let p = tmp_ams(
        "resistor.phdl",
        "\
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
        }
        analog R { I(p, n) <+ V(p, n) / r; }
        ",
    );
    let summary = check_file(&p).expect("check_file ok");
    let modules = match summary {
        piperine_cli::commands::check::CheckSummary::Ppr { module_names } => module_names,
        other => panic!("expected PPR summary, got {other:?}"),
    };
    assert_eq!(modules, vec!["R".to_string()]);
}

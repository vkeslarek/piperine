//! `piperine check` on a real project: the command elaborates the target and
//! reports the modules it found.
//!
//! Exercises the `check` command against a small Verilog-AMS fixture
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
fn check_vams_resistor_extracts_module() {
    let p = tmp_ams(
        "resistor.vams",
        "\
        nature Voltage; units = \"V\"; access = V; endnature
        nature Current; units = \"A\"; access = I; endnature
        discipline electrical; potential Voltage; flow Current; enddiscipline
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            analog begin
                I(p, n) <+ V(p, n) / r;
            end
        endmodule
        ",
    );
    let summary = check_file(&p, &piperine_lang::SourceMap::dummy()).expect("check_file ok");
    assert_eq!(summary.module_names, vec!["R".to_string()]);
}

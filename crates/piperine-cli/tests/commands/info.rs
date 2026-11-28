//! `piperine info` (HII-11): the real binary elaborates the top of a run and prints its name and
//! backend, then the tables `params`, `ports` and `streams`, building nothing and simulating
//! nothing; a design that does not elaborate gets the diagnostics `piperine check` prints, exit 1.

use std::path::Path;
use super::process::PiperineProcess;

#[test]
fn info_prints_params_ports_and_streams() {
    // Book claims held here: cli~21c5d5e3
    // HII-11 AC 1: in `examples/rc`, the top and the backend, then the three tables: `r` and `c`
    // of the low-pass are among the params; nothing was compiled
    let cache = tempfile::tempdir().expect("tmp");
    let rc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc");
    let out = PiperineProcess::piperine_cached(&rc, cache.path(), &["info"]);
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", PiperineProcess::text(&out.stderr));
    let stdout = PiperineProcess::text(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "info op: top Step, backend rust", "{stdout}");
    let at = |title: &str| lines.iter().position(|l| *l == title).unwrap_or_else(|| panic!("no `{title}` table in\n{stdout}"));
    let (params, ports, streams) = (at("params"), at("ports"), at("streams"));
    assert!(params < ports && ports < streams, "{stdout}");
    assert_eq!(PiperineProcess::cells(lines[params + 1]), ["path", "type", "value", "where", "structural"]);
    let row = |path: &str| lines[params + 2..ports].iter().find(|l| PiperineProcess::cells(l).first() == Some(&path)).unwrap_or_else(|| panic!("no param {path} in\n{stdout}"));
    let r1 = PiperineProcess::cells(row("Step.r1.r"));
    assert_eq!(&r1[..3], ["Step.r1.r", "f64", "1.000000e3"], "{stdout}");
    assert!(r1.contains(&"where") || r1.iter().any(|c| *c == ">"), "the `where` text is shown: {r1:?}");
    assert_eq!(r1.last(), Some(&"false"), "not structural: {r1:?}");
    assert!(PiperineProcess::cells(row("Step.c1.c")).contains(&"1.000000e-9"), "{stdout}");
    assert_eq!(PiperineProcess::cells(lines[ports + 1]), ["name", "direction", "type"]);
    assert_eq!(PiperineProcess::cells(lines[streams + 1]), ["stream", "fields"]);
    assert!(std::fs::read_dir(cache.path()).expect("cache").next().is_none(), "nothing was compiled");
}

#[test]
fn info_of_a_broken_design_prints_diagnostics_and_exits_1() {
    // Book claims held here: cli~e4cb970e
    // HII-11 AC 2: a design that does not elaborate: the diagnostics of `check`, exit 1, no table
    let cache = tempfile::tempdir().expect("tmp");
    let broken = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects/type_error");
    let out = PiperineProcess::piperine_cached(&broken, cache.path(), &["info"]);
    assert_eq!(out.status.code(), Some(1), "stdout:\n{}", PiperineProcess::text(&out.stdout));
    assert!(PiperineProcess::text(&out.stdout).is_empty(), "no table: {}", PiperineProcess::text(&out.stdout));
    let check = PiperineProcess::piperine_cached(&broken, cache.path(), &["check"]);
    let (shown, checked) = (PiperineProcess::text(&out.stderr), PiperineProcess::text(&check.stderr));
    let code = checked.lines().find(|l| l.trim().starts_with('E') || l.trim().starts_with('P')).unwrap_or_else(|| panic!("no code in\n{checked}")).trim().to_string();
    assert!(shown.contains(&code), "info reports the diagnostic `{code}` that check does:\n{shown}");
}

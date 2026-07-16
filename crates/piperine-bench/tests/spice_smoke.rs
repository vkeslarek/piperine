//! spice-stdlib SPICE-03: the migrated smoke benches (junction + validate,
//! ported from the retired external package) pass in-process through the
//! builtin `use spice::…` namespace.

use std::path::PathBuf;

use piperine_bench::{BenchOutcome, BenchRunner};
use piperine_lang::SourceMap;

/// A source map rooted at the real stdlib headers, mirroring what
/// `piperine-project` builds for a project (bench tests run with the bench
/// crate as cwd, so `SourceMap::dummy`'s relative paths don't apply here).
fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Elaborate a fixture from `tests/spice/` and run every bench entry point,
/// failing with the full list of failures.
fn run_fixture(name: &str) {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/spice")).join(name);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let design = piperine_lang::parse_and_elaborate(&src, &headers_source_map())
        .unwrap_or_else(|e| panic!("{name}: elaboration failed: {e:?}"));

    let report = BenchRunner::new(&design).run_all();
    assert!(!report.results.is_empty(), "{name}: no bench entry points found");
    let failures: Vec<String> = report
        .results
        .iter()
        .filter_map(|r| match &r.outcome {
            BenchOutcome::Passed => None,
            BenchOutcome::Failed(msg) => Some(format!("{}::{} FAILED — {msg}", r.module, r.entry)),
            BenchOutcome::Error(msg) => Some(format!("{}::{} ERROR — {msg}", r.module, r.entry)),
        })
        .collect();
    assert!(failures.is_empty(), "{name}: {} failure(s):\n{}", failures.len(), failures.join("\n"));
}

/// Junction devices (dio/bjt/mos1/jfet) converge to their physical
/// operating points via builtin `use spice::…`.
#[test]
fn spice_junction_devices_converge() {
    run_fixture("junction.phdl");
}

/// Passives, independent sources ($op/$ac), controlled sources and switches.
#[test]
fn spice_validate_smoke_passes() {
    run_fixture("validate.phdl");
}

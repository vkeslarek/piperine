//! Scope: run errors from source (requirements HYG-01 AC 1 and 7, HYG-02 AC 3): small `.msl`
//! designs under `tests/run_errors/` that cannot be solved are loaded through `Bench` on every
//! selected backend, and the operating point fails with the Part V code and the names the author
//! wrote (instance paths and pins), identically on the interpreter and the Rust backend. No
//! ngspice binary is involved.

use std::path::Path;

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_solver::prelude::Context;

/// The error text of the operating point of `tests/run_errors/<name>.msl` on `backend`.
fn operating_point_error(name: &str, backend: Backend) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run_errors").join(format!("{name}.msl"));
    let mut bench = Bench::load_file_on(name, path, backend);
    let error = bench
        .circuit()
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("the operating point of this design must fail");
    error.to_string()
}

/// HYG-01 AC 7: two potential sources in parallel fail `E0503` naming both instance paths.
#[test]
fn a_loop_of_two_vsources_from_source_is_e0503() {
    for backend in Backend::selected() {
        let text = operating_point_error("source_loop", backend);
        assert!(text.contains("E0503"), "{}: {text}", backend.name());
        assert!(text.contains("SourceLoop.v1"), "{}: names v1: {text}", backend.name());
        assert!(text.contains("SourceLoop.v2"), "{}: names v2: {text}", backend.name());
    }
}

/// HYG-01 AC 1: a node reached only through capacitors fails `E0503` naming the node by its pins.
#[test]
fn a_floating_node_from_source_is_e0503() {
    for backend in Backend::selected() {
        let text = operating_point_error("floating", backend);
        assert!(text.contains("E0503"), "{}: {text}", backend.name());
        assert!(
            text.contains("node of Floating.c1.p, Floating.c2.p"),
            "{}: the node is named by the pins bound to it: {text}",
            backend.name()
        );
    }
}

/// HYG-02 AC 3: a NaN contribution fails `E0507` naming the instance.
#[test]
fn a_nan_contribution_from_source_is_e0507() {
    for backend in Backend::selected() {
        let text = operating_point_error("nan", backend);
        assert!(text.contains("E0507"), "{}: {text}", backend.name());
        assert!(text.contains("Nan.x1"), "{}: names the instance: {text}", backend.name());
    }
}

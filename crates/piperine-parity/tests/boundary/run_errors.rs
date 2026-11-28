//! Scope: run errors from source (requirements HYG-01 AC 1 and 7, HYG-02 AC 3): small `.msl`
//! designs under `tests/run_errors/` that cannot be solved are loaded through `Bench` on every
//! selected backend, and the operating point fails with the Part V code and the names the author
//! wrote (instance paths and pins), identically on the interpreter and the Rust backend. No
//! ngspice binary is involved.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use piperine_api::{Failure, Overrides, Project, Record, RecordSink, Session};
use piperine_parity::backend::{Backend, BackendSelection};
use piperine_parity::bench::Bench;
use piperine_parity::host::HostRun;
use piperine_solver::prelude::Context;

/// The message of the refusal a source gives at its operating point.
struct OperatingPointRefusal;

impl OperatingPointRefusal {
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
}

/// HYG-01 AC 7: two potential sources in parallel fail `E0503` naming both instance paths.
#[test]
fn a_loop_of_two_vsources_from_source_is_e0503() {
    for backend in BackendSelection::selected() {
        let text = OperatingPointRefusal::operating_point_error("source_loop", backend);
        assert!(text.contains("E0503"), "{}: {text}", backend.name());
        assert!(text.contains("SourceLoop.v1"), "{}: names v1: {text}", backend.name());
        assert!(text.contains("SourceLoop.v2"), "{}: names v2: {text}", backend.name());
    }
}

/// HYG-01 AC 1: a node reached only through capacitors fails `E0503` naming the node by its pins.
#[test]
fn a_floating_node_from_source_is_e0503() {
    for backend in BackendSelection::selected() {
        let text = OperatingPointRefusal::operating_point_error("floating", backend);
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
    for backend in BackendSelection::selected() {
        let text = OperatingPointRefusal::operating_point_error("nan", backend);
        assert!(text.contains("E0507"), "{}: {text}", backend.name());
        assert!(text.contains("Nan.x1"), "{}: names the instance: {text}", backend.name());
    }
}

/// The designs of `tests/run_errors/`, each failing its operating point.
struct RunErrors;

impl RunErrors {
    /// The failure the host library gives for the operating point of `tests/run_errors/<name>.msl`
    /// on `backend`: the file opened as a project, as `piperine run` does.
    fn failure(name: &str, backend: Backend) -> Failure {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run_errors").join(format!("{name}.msl"));
        let project = Project::file(&path).unwrap_or_else(|f| panic!("{name}: project: {f}"));
        let overrides = Overrides { analysis: Some("op".to_string()), backend: Some(backend.name().to_string()), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{name}: request: {f}"));
        let mut session = Session::open(&project, &spec, &HostRun::settings()).unwrap_or_else(|f| panic!("{name}: open: {f}"));
        session.run(Rc::new(RefCell::new(Ignore))).expect_err("the run of this design must fail")
    }
}

/// A sink that keeps nothing: these runs fail before they record.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// PI-12 AC 2 (Part I §1 rule 11): an integer division by zero under known control fails the run
/// `E0105` naming the operation, the instance and the time.
#[test]
fn a_division_by_zero_under_known_control_fails_the_run_with_e0105() {
    // The interpreter refuses `Int` arithmetic and `unwrap` by name (codegen context R1), so a panic
    // of the model's own values is the compiled backend's.
    for backend in [Backend::Rust] {
        let failure = RunErrors::failure("divide", backend);
        let text = failure.to_string();
        assert!(matches!(failure, Failure::Host { code: "E0105", .. }), "{}: {text}", backend.name());
        assert!(text.contains("integer division by zero"), "{}: the operation: {text}", backend.name());
        assert!(text.contains("Divide.x1"), "{}: the instance: {text}", backend.name());
        assert!(text.contains("t = 0e0 s"), "{}: the time: {text}", backend.name());
    }
}

/// PI-12 AC 2 (Part I §1 rule 11): an `unwrap` of `None` under known control is `E0105` too.
#[test]
fn an_unwrap_of_none_under_known_control_fails_the_run_with_e0105() {
    // The interpreter refuses `Int` arithmetic and `unwrap` by name (codegen context R1), so a panic
    // of the model's own values is the compiled backend's.
    for backend in [Backend::Rust] {
        let failure = RunErrors::failure("unwrap", backend);
        let text = failure.to_string();
        assert!(matches!(failure, Failure::Host { code: "E0105", .. }), "{}: {text}", backend.name());
        assert!(text.contains("Option::unwrap()"), "{}: the operation: {text}", backend.name());
        assert!(text.contains("Unwrap.x1"), "{}: the instance: {text}", backend.name());
        assert!(text.contains("t = 0e0 s"), "{}: the time: {text}", backend.name());
    }
}

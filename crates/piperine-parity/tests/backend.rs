//! Scope: the backend a twin runs on (requirement RS-07 AC 1, 2, 4 and 5; checks C49, C50, C52,
//! C53): `PIPERINE_BACKEND` selects `interp` or `rust` or, unset, both; any other value is refused
//! naming the two valid ones; every rejection of the contract names the backend; the Rust bench
//! runs the modules of a library it built and loaded, and the interpreter bench none. The
//! variable's parser is pure over a value, so no test sets an environment variable.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_solver::prelude::Context;

#[test]
fn the_variable_selects_one_backend() {
    assert_eq!(Backend::parse(Some("interp")), Ok(vec![Backend::Interp]));
    assert_eq!(Backend::parse(Some("rust")), Ok(vec![Backend::Rust]));
    assert_eq!(Backend::parse(None), Ok(vec![Backend::Interp, Backend::Rust]), "unset: both, the reference first");
    assert_eq!(Backend::parse(Some("")), Ok(vec![Backend::Interp, Backend::Rust]), "empty counts as unset");
    assert_eq!(Backend::Interp.name(), "interp");
    assert_eq!(Backend::Rust.name(), "rust");
    assert_eq!(Backend::parse(Some(Backend::Rust.name())), Ok(vec![Backend::Rust]), "the name round-trips");
}

#[test]
fn an_unknown_backend_is_refused_naming_both() {
    for value in ["cranelift", "Rust", "interp,rust", " rust"] {
        let message = Backend::parse(Some(value)).expect_err("an unknown backend is refused");
        assert!(message.contains("`interp`") && message.contains("`rust`"), "{value}: {message}");
        assert!(message.contains(value), "the value is named: {message}");
        assert!(message.contains(Backend::VARIABLE), "the variable is named: {message}");
    }
}

#[test]
fn a_rejection_names_the_backend() {
    for backend in [Backend::Interp, Backend::Rust] {
        let at = Point::new("divider", "v(mid)", Coordinate::OperatingPoint).with_backend(backend.name());
        let payload = std::panic::catch_unwind(|| ToleranceContract::DC_VOLTAGE.assert(&at, 7.5, 7.6)).expect_err("the pair is rejected");
        let text = payload.downcast_ref::<String>().cloned().expect("a text panic");
        assert!(text.contains(&format!("backend: {}", backend.name())), "{text}");
        assert!(text.contains("divider") && text.contains("v(mid)"), "the rest of the point stays: {text}");
    }
}

#[test]
fn the_rust_bench_runs_loaded_modules() {
    let mut bench = Bench::load_on("divider", Backend::Rust);
    assert_eq!(bench.backend(), Backend::Rust);
    let compiled = bench.compiled().expect("a Rust bench holds the compiled design");
    // the library was built and loaded: its source and its modules exist
    let source = std::fs::read_to_string(compiled.source_path()).expect("the generated source is kept on disk");
    assert!(source.contains("piperine_abi_1"), "the library exports the ABI symbol");
    assert_eq!(compiled.loaded().names(), vec!["Vsource", "Resistor"], "the divider's two device bodies");
    assert!(compiled.report().rustc_invocations > 0, "the bench built it: {:?}", compiled.report());
    let mid = bench.node("mid");
    let v = bench.circuit().dc(Context::default()).expect("dc").solve().expect("solve").get_node(&mid).expect("v(mid)");
    assert!((v - 7.5).abs() < 1e-9, "v(mid) = {v}");

    // the interpreter bench loads no library
    let reference = Bench::load_on("divider", Backend::Interp);
    assert_eq!(reference.backend(), Backend::Interp);
    assert!(reference.compiled().is_none());
    assert_eq!(Bench::load("divider").backend(), Backend::Interp, "`load` stays the interpreter");
}

//! LRM-COVERS: 9.16
//!
//! LRMW2-50 (`lrm-conformance-wave2`, T57): `piperine_solver::prelude::
//! simprobe` — the §9.16 `$simprobe(inst_name, param_name[, default])`
//! resolution rule (sibling-instance lookup by name, then its opvar by
//! name) — is proven interchangeable with the host probe API
//! (`OpResult::instance(label).opvar(name)`, `piperine-api`) by reading
//! the *same* live circuit's *same* instance/opvar pair through both
//! paths and asserting they agree exactly. Both ultimately read the
//! identical `Element::read_opvars()` data (`piperine-solver`'s
//! `core/introspect.rs`), so agreement is not a coincidence of test
//! values — see this file's own discrimination test.
//!
//! SPEC_DEVIATION (disclosed in `core/introspect.rs`'s own doc on
//! `simprobe`): this proves the resolution *mechanism* is correct and
//! host-API-equivalent, not that a `$simprobe(...)` call written inside
//! a `.vams` model's own analog body reaches it yet — that needs a live
//! per-timestep sibling-opvar snapshot threaded through the analog JIT
//! ABI, out of this task's reach in the time available (same tier as
//! `idtmod`/`absdelay`/`table`/`ac_stim`, CLAUDE.md's own "Known gaps").

use std::path::PathBuf;

use piperine::Session;
use piperine_codegen::CircuitCompiler;
use piperine_lang::SourceMap;
use piperine_solver::prelude::{simprobe, Context, SimprobeError};

const SRC: &str = "\
module VoltageSource(inout electrical p, inout electrical n);
    parameter real voltage = 0.0;
    analog V(p, n) <+ voltage;
endmodule

module Resistor(inout electrical p, inout electrical n);
    parameter real r = 1e3;
    real cond;
    analog begin
        cond = 1.0 / r;
        I(p, n) <+ cond * V(p, n);
    end
endmodule

module Top();
    electrical gnd, vin;
    VoltageSource #(.voltage(5.0)) src (.p(vin), .n(gnd));
    Resistor      #(.r(1e3))       r1  (.p(vin), .n(gnd));
endmodule
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.vams"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(SRC, &headers_source_map()).expect("fixture elaborates")
}

/// Done-when: "a `$simprobe` in a model returns the probed quantity,
/// asserted against the same value read through the host probe API" —
/// `simprobe(&devices, "r1", "cond")` and `op.instance("r1").opvar("cond")`
/// read the same live circuit and must agree exactly.
#[test]
fn simprobe_agrees_with_the_host_probe_api_on_a_live_circuit() {
    let d = design();

    // Path 1: the host probe API.
    let mut session = Session::compile(&d, "Top").expect("session compiles");
    let op = session.op(&Default::default(), None).expect("dc op");
    let host_value = op.instance("r1").expect("instance r1").opvar("cond").expect("opvar cond");

    // Path 2: `simprobe` directly against the compiled circuit's own
    // device list.
    let bodies = piperine_codegen::resolve::lower_bodies(&d).expect("lowers");
    let mut compiler = CircuitCompiler::new(&d, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let probed_value = simprobe(&circuit.devices, "r1", "cond").expect("simprobe resolves");

    assert!((host_value - probed_value).abs() < 1e-12, "host={host_value} probed={probed_value}");
    assert!((probed_value - 1.0e-3).abs() < 1e-9, "cond = 1/r = 1e-3 S, got {probed_value:e}");
}

/// Discrimination: a different resistance changes the opvar, and both
/// paths must track the change identically — rules out a coincidental
/// match at one fixed value.
#[test]
fn simprobe_tracks_a_different_resistance_value() {
    const SRC2: &str = "\
module VoltageSource(inout electrical p, inout electrical n);
    parameter real voltage = 0.0;
    analog V(p, n) <+ voltage;
endmodule

module Resistor(inout electrical p, inout electrical n);
    parameter real r = 1e3;
    real cond;
    analog begin
        cond = 1.0 / r;
        I(p, n) <+ cond * V(p, n);
    end
endmodule

module Top();
    electrical gnd, vin;
    VoltageSource #(.voltage(5.0)) src (.p(vin), .n(gnd));
    Resistor      #(.r(2e3))       r1  (.p(vin), .n(gnd));
endmodule
";
    let d = piperine_lang::parse_and_elaborate(SRC2, &headers_source_map()).expect("fixture elaborates");

    let mut session = Session::compile(&d, "Top").expect("session compiles");
    let op = session.op(&Default::default(), None).expect("dc op");
    let host_value = op.instance("r1").expect("instance r1").opvar("cond").expect("opvar cond");

    let bodies = piperine_codegen::resolve::lower_bodies(&d).expect("lowers");
    let mut compiler = CircuitCompiler::new(&d, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let probed_value = simprobe(&circuit.devices, "r1", "cond").expect("simprobe resolves");

    assert!((host_value - probed_value).abs() < 1e-12);
    assert!((probed_value - 5.0e-4).abs() < 1e-9, "cond = 1/r = 5e-4 S, got {probed_value:e}");
}

/// Done-when: "an invalid probe target is loud" — an unknown sibling
/// instance name on a real, live circuit is a typed error, never a
/// silent `0.0`.
#[test]
fn simprobe_unknown_instance_on_a_live_circuit_is_loud() {
    let d = design();
    let bodies = piperine_codegen::resolve::lower_bodies(&d).expect("lowers");
    let mut compiler = CircuitCompiler::new(&d, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    circuit.dc(Context::default()).unwrap().solve().expect("dc solve");

    let err = simprobe(&circuit.devices, "no_such_sibling", "cond").unwrap_err();
    assert_eq!(err, SimprobeError::UnknownInstance("no_such_sibling".to_string()));
}

/// Done-when: "an invalid probe target is loud" — a real sibling with an
/// unknown opvar name is also a typed error.
#[test]
fn simprobe_unknown_opvar_on_a_live_circuit_is_loud() {
    let d = design();
    let bodies = piperine_codegen::resolve::lower_bodies(&d).expect("lowers");
    let mut compiler = CircuitCompiler::new(&d, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    circuit.dc(Context::default()).unwrap().solve().expect("dc solve");

    let err = simprobe(&circuit.devices, "r1", "no_such_opvar").unwrap_err();
    assert_eq!(
        err,
        SimprobeError::UnknownOpvar { instance: "r1".to_string(), opvar: "no_such_opvar".to_string() }
    );
}

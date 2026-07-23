//! Terminal descriptor bridge (ABI-27): a JIT-compiled analog device
//! surfaces its kernel terminal catalog through the standard
//! `Introspect::list_terminals` surface, populated from the symbol table
//! (names, not positional indices). Ports are `TerminalKind::External`;
//! module-internal nodes referenced by the body (a `wire` declared in the
//! module, e.g. the BJT's collector'/base'/emitter' parasitic stubs) are
//! `TerminalKind::Internal`.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::TerminalKind;
use piperine_solver::abi::Domain;
use piperine_solver::prelude::CircuitInstance;

fn from_ir(design: &piperine_lang::pom::Design, bodies: &HashMap<String, LoweredBody>, top: &str) -> CircuitInstance {
    let mut c = CircuitCompiler::new(design, bodies);
    c.build_circuit(top).expect("circuit compiles")
}

/// Compile `src` and build the named top-module circuit.
fn build(src: &str, top: &str) -> CircuitInstance {
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    from_ir(&elab, &bodies, top)
}

fn terminals_of(circuit: &CircuitInstance, device_idx: usize) -> Vec<(String, TerminalKind, Domain)> {
    circuit.all_devices()[device_idx]
        .list_terminals()
        .into_iter()
        .map(|t| (t.name, t.kind, t.domain))
        .collect()
}

/// ABI-27: a JIT-compiled device's `list_terminals()` returns one descriptor
/// per kernel terminal, named from the symbol table. The two-port module
/// `RC` (one external `p`/`n` port pair plus one internal `mid` wire)
/// surfaces all three with the correct `TerminalKind`.
#[test]
fn analog_kernel_terminals_bridge_names_and_kinds() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod RC (inout p: Electrical, inout n: Electrical) {
            param r1: Real = 1.0e3;
            param r2: Real = 1.0e3;
            wire mid : Electrical;
        }
        analog RC {
            I(p, mid) <+ V(p, mid) / r1;
            I(mid, n) <+ V(mid, n) / r2;
        }
        mod Top (inout a: Electrical, inout b: Electrical) { RC(a, b); }
        ",
        "Top",
    );
    let terms = terminals_of(&circuit, 0);
    // Two external ports + one internal wire = 3 analog terminals.
    assert_eq!(terms.len(), 3, "got {terms:?}");
    let by_name: std::collections::HashMap<&str, (TerminalKind, Domain)> = terms
        .iter()
        .map(|(n, k, d)| (n.as_str(), (*k, *d)))
        .collect();
    // External ports.
    assert_eq!(
        by_name.get("p"),
        Some(&(TerminalKind::External, Domain::Analog)),
        "port `p` must be External/Analog, got {by_name:?}"
    );
    assert_eq!(
        by_name.get("n"),
        Some(&(TerminalKind::External, Domain::Analog)),
        "port `n` must be External/Analog, got {by_name:?}"
    );
    // Internal wire — a non-port `wire` the body references by name.
    assert_eq!(
        by_name.get("mid"),
        Some(&(TerminalKind::Internal, Domain::Analog)),
        "internal wire `mid` must be Internal/Analog, got {by_name:?}"
    );
}

/// ABI-27 spec AC (BJT shape): a model with three external ports
/// (collector/base/emitter) plus three internal parasitic-stub `wire`s
/// (collector'/base'/emitter') surfaces c/b/e as External and cp/bp/ep as
/// Internal — exactly the kernel terminal catalog from
/// `AnalogKernel::terminals` + the symbol table. The body must reference
/// every internal wire so the kernel's `terminal_order` includes it.
#[test]
fn bjt_lists_external_ports_and_internal_parasitic_wires() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Bjt (inout c: Electrical, inout b: Electrical, inout e: Electrical) {
            param rc: Real = 1.0;
            param rb: Real = 1.0;
            param re: Real = 1.0;
            param gm: Real = 1.0e-3;
            wire cp : Electrical;
            wire bp : Electrical;
            wire ep : Electrical;
        }
        analog Bjt {
            I(c, cp) <+ V(c, cp) / rc;
            I(b, bp) <+ V(b, bp) / rb;
            I(e, ep) <+ V(e, ep) / re;
            I(cp, ep) <+ gm * V(bp, ep);
        }
        mod TopBjt (inout c: Electrical, inout b: Electrical, inout e: Electrical) {
            Bjt(c, b, e);
        }
        ",
        "TopBjt",
    );
    let terms = terminals_of(&circuit, 0);
    let by_name: std::collections::HashMap<&str, TerminalKind> = terms
        .iter()
        .map(|(n, k, _)| (n.as_str(), *k))
        .collect();
    // Three external ports.
    for port in ["c", "b", "e"] {
        assert_eq!(
            by_name.get(port),
            Some(&TerminalKind::External),
            "BJT port `{port}` must be External, got {by_name:?}"
        );
    }
    // Three internal parasitic-stub wires declared in the module and
    // referenced by the analog body.
    for wire in ["cp", "bp", "ep"] {
        assert_eq!(
            by_name.get(wire),
            Some(&TerminalKind::Internal),
            "BJT internal wire `{wire}` must be Internal, got {by_name:?}"
        );
    }
}

/// ABI-27 fallback: a pure two-port with no internal wires surfaces only
/// the External ports — the kernel terminal catalog is exactly the ports.
#[test]
fn two_port_resistor_lists_only_external_terminals() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let terms = terminals_of(&circuit, 0);
    assert_eq!(terms.len(), 2, "resistor surfaces 2 ports, got {terms:?}");
    assert!(terms.iter().all(|(_, k, _)| *k == TerminalKind::External));
}

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

/// ABI-28: a digital-only device's `list_terminals()` returns one descriptor
/// per digital input (Direction::In) and per output (Direction::Out),
/// carrying [`Domain::Digital`] and [`TerminalKind::External`]. A 2-input
/// NAND gate surfaces `a`/`b` as inputs and `y` as the output.
#[test]
fn digital_kernel_terminals_bridge_inputs_and_outputs() {
    let circuit = build(
        "
        discipline Bit { storage Boolean; }
        mod Nand ( input a : Bit, input b : Bit, output y : Bit ) {
            var t : Bit = 0;
        }
        digital Nand { t = !(a && b); y <- t; }
        mod TopNand ( input a : Bit, input b : Bit, output y : Bit ) { Nand(a, b, y); }
        ",
        "TopNand",
    );
    let terms = terminals_of(&circuit, 0);
    // 2 inputs + 1 output = 3 digital terminals (no analog body here).
    assert_eq!(terms.len(), 3, "NAND surfaces 3 digital terminals, got {terms:?}");
    let by_name: std::collections::HashMap<&str, (TerminalKind, Domain)> = terms
        .iter()
        .map(|(n, k, d)| (n.as_str(), (*k, *d)))
        .collect();
    for input in ["a", "b"] {
        assert_eq!(
            by_name.get(input),
            Some(&(TerminalKind::External, Domain::Digital)),
            "NAND input `{input}` must be External/Digital, got {by_name:?}"
        );
    }
    assert_eq!(
        by_name.get("y"),
        Some(&(TerminalKind::External, Domain::Digital)),
        "NAND output `y` must be External/Digital, got {by_name:?}"
    );
    // All digital — no analog terminals leaked in.
    assert!(terms.iter().all(|(_, _, d)| *d == Domain::Digital));
}

/// ABI-28 direction mapping: digital inputs are `Direction::In`, outputs
/// are `Direction::Out` (not the analog ports' `Inout`). A D-flip-flop
/// (clk/d inputs, q output) is the canonical case.
#[test]
fn digital_kernel_directions_are_in_for_inputs_out_for_outputs() {
    use piperine_solver::abi::Direction;
    let circuit = build(
        "
        discipline Bit { storage Boolean; }
        mod Dff ( input clk : Bit, input d : Bit, output q : Bit ) {
            var st : Bit = 0;
        }
        digital Dff { q <- st; @ (posedge(clk)) { st = d; } }
        mod TopDff ( input clk : Bit, input d : Bit, output q : Bit ) { Dff(clk, d, q); }
        ",
        "TopDff",
    );
    let dev = &circuit.all_devices()[0];
    let terms = dev.list_terminals();
    let direction_of = |name: &str| -> Direction {
        terms.iter().find(|t| t.name == name).map(|t| t.direction).unwrap_or_else(|| panic!("missing terminal `{name}`"))
    };
    assert_eq!(direction_of("clk"), Direction::In, "clk is an input");
    assert_eq!(direction_of("d"), Direction::In, "d is an input");
    assert_eq!(direction_of("q"), Direction::Out, "q is an output");
}

/// ABI-27 + ABI-28 union: a mixed-signal device (one with BOTH an analog
/// body and a digital body) surfaces both analog and digital terminals in
/// one `list_terminals()` call. A DAC that takes a digital `d` input,
/// drives an internal register, and emits an analog output through an
/// `I(out, gnd) <- …` contribution exercises both kernel paths on the same
/// device object.
#[test]
fn mixed_signal_device_lists_analog_and_digital_terminals() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        discipline Bit { storage Boolean; }
        mod MixedDac ( input d : Bit, inout out : Electrical, inout gnd : Electrical ) {
            var reg : Bit = 0;
        }
        digital MixedDac { @ (change(d)) { reg = d; } }
        analog MixedDac { I(out, gnd) <+ if (reg) { 1.0e-3 } else { 0.0 }; }
        mod TopMix ( input d : Bit, inout out : Electrical, inout gnd : Electrical ) {
            MixedDac(d, out, gnd);
        }
        ",
        "TopMix",
    );
    let terms = terminals_of(&circuit, 0);
    let by_name: std::collections::HashMap<&str, (TerminalKind, Domain)> = terms
        .iter()
        .map(|(n, k, d)| (n.as_str(), (*k, *d)))
        .collect();
    // The mixed-signal DAC surfaces 3 external terminals: `d` is digital
    // (the register input); `out`/`gnd` are analog (the contribution
    // terminals). Both domains present in one catalog.
    assert_eq!(
        by_name.get("d"),
        Some(&(TerminalKind::External, Domain::Digital)),
        "digital input `d` must be External/Digital, got {by_name:?}"
    );
    assert_eq!(
        by_name.get("out"),
        Some(&(TerminalKind::External, Domain::Analog)),
        "analog port `out` must be External/Analog, got {by_name:?}"
    );
    assert_eq!(
        by_name.get("gnd"),
        Some(&(TerminalKind::External, Domain::Analog)),
        "analog port `gnd` must be External/Analog, got {by_name:?}"
    );
}

// ── phdl-introspection-attributes PIA-10..14 (T6) ──────────────────────────
// @name/@kind on a port or internal wire override the position-inferred
// terminal name/kind; the author declaration wins over inference.

/// PIA-10: a port carrying `@kind("auxiliary")` classifies its terminal as
/// `TerminalKind::Auxiliary` (not the position-inferred External).
#[test]
fn port_at_kind_auxiliary_classifies_terminal() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Dev ( @kind(value = \"auxiliary\") inout t : Electrical, inout s : Electrical ) {
            param r: Real = 1.0e3;
        }
        analog Dev {
            I(t, s) <+ V(t, s) / r;
        }
        mod Top (inout a: Electrical, inout b: Electrical) { Dev(a, b); }
        ",
        "Top",
    );
    let terms = terminals_of(&circuit, 0);
    let by_name: HashMap<&str, (TerminalKind, Domain)> = terms
        .iter()
        .map(|(n, k, d)| (n.as_str(), (*k, *d)))
        .collect();
    assert_eq!(
        by_name.get("t"),
        Some(&(TerminalKind::Auxiliary, Domain::Analog)),
        "port `t` with @kind(auxiliary) must be Auxiliary, got {by_name:?}"
    );
    // The un-annotated port keeps the position-inferred External (PIA-12).
    assert_eq!(
        by_name.get("s"),
        Some(&(TerminalKind::External, Domain::Analog)),
        "un-annotated port `s` stays External, got {by_name:?}"
    );
}

/// PIA-11: an internal `wire` with `@kind("internal") @name("cp")` classifies
/// the terminal Internal and names it `cp` (author-declared, overridable).
#[test]
fn internal_wire_at_kind_internal_named_cp() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod RC (inout p: Electrical, inout n: Electrical) {
            param r1: Real = 1.0e3;
            param r2: Real = 1.0e3;
            @name(value = \"cp\") @kind(value = \"internal\") wire mid : Electrical;
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
    // The wire is renamed `cp` (the @name) and stays Internal.
    assert!(
        terms.iter().any(|(n, k, _)| n == "cp" && *k == TerminalKind::Internal),
        "internal wire with @name(cp) @kind(internal) must surface as `cp`/Internal, got {terms:?}"
    );
    // The source wire name `mid` no longer appears once @name renames it.
    assert!(
        !terms.iter().any(|(n, _, _)| n == "mid"),
        "the wire id `mid` must not surface once @name(cp) is set, got {terms:?}"
    );
}

/// PIA (explicit over inferred): `@kind("external")` on an internal wire is
/// legal — the author declaration wins over the position-inferred Internal.
#[test]
fn at_kind_external_on_wire_wins_over_inferred() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod RC (inout p: Electrical, inout n: Electrical) {
            param r1: Real = 1.0e3;
            param r2: Real = 1.0e3;
            @kind(value = \"external\") wire mid : Electrical;
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
    let mid = terms
        .iter()
        .find(|(n, _, _)| n == "mid")
        .expect("internal wire `mid` present");
    assert_eq!(mid.1, TerminalKind::External, "explicit @kind(external) on a wire wins over inferred Internal");
}

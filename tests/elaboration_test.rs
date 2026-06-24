//! `elaborate_circuit` integration tests.
//!
//! Tests hardware-only `.ppr` content (no `initial` blocks):
//! paramset expansion, gnd→"0" mapping, module hierarchy, SOA checks,
//! `ngspice.ppr` include resolution.

use piperine_circuit::{elaborate_circuit, HardwareRegistry, SoaOp};
use piperine_ngspice::register_hardware;
use piperine_parser::parser::{parse, parse_with_includes};

fn circuit(src: &str) -> piperine_circuit::Circuit {
    let doc = parse(src).expect("parse failed");
    let mut reg = HardwareRegistry::new();
    register_hardware(&mut reg);
    elaborate_circuit(&doc, &reg, None).expect("elaborate_circuit failed")
}

fn circuit_with_includes(src: &str) -> piperine_circuit::Circuit {
    let full = format!("`include \"ngspice.ppr\"\n{src}");
    let dirs = vec![piperine_ngspice::ppr_dir(), piperine_parser::bundled_header_dir()];
    let doc = parse_with_includes(&full, &dirs).expect("parse failed");
    let mut reg = HardwareRegistry::new();
    register_hardware(&mut reg);
    elaborate_circuit(&doc, &reg, None).expect("elaborate_circuit failed")
}

// ── Basic elaboration ─────────────────────────────────────────────────────────

#[test]
fn test_title_line() {
    let c = circuit_with_includes(r#"module lpf; res #(.r(1e3)) R1(.p(a), .n(b)); endmodule"#);
    assert!(c.spice_lines[0].starts_with("* piperine circuit:"),
        "first line should be a comment: {:?}", c.spice_lines[0]);
}

#[test]
fn test_gnd_maps_to_zero() {
    let c = circuit_with_includes(
        r#"module tb; res #(.r(1e3)) R1(.p(a), .n(gnd)); endmodule"#
    );
    assert!(c.spice_lines.iter().any(|l| l == "R1 a 0 1000"),
        "expected 'R1 a 0 1000', got:\n{}", c.spice_lines.join("\n"));
}

#[test]
fn test_paramset_passive_expands() {
    let c = circuit_with_includes(r#"
paramset lpf_r res; .r = 1000.0; endparamset
module lpf;
    wire in, out;
    lpf_r R1(.p(in), .n(out));
endmodule
"#);
    assert!(c.spice_lines.iter().any(|l| l == "R1 in out 1000"),
        "paramset passive should expand to resistor line:\n{}", c.spice_lines.join("\n"));
    // No .model card should be emitted for passives.
    assert!(!c.spice_lines.iter().any(|l| l.starts_with(".model")),
        "passive paramset must not emit .model:\n{}", c.spice_lines.join("\n"));
}

#[test]
fn test_paramset_transistor_references_model() {
    let c = circuit_with_includes(r#"
paramset bc548 npn;
    .model = "BC548";
    .is   = 1e-14;
    .bf   = 200.0;
endparamset
module amp;
    wire b, c, e;
    bc548 Q1(.c(c), .b(b), .e(e));
endmodule
"#);
    // Instance must reference the model name.
    assert!(c.spice_lines.iter().any(|l| l.starts_with("Q") && l.contains("BC548")),
        "transistor instance must reference model name:\n{}", c.spice_lines.join("\n"));
}

#[test]
fn test_module_name_selection() {
    let src = r#"`include "ngspice.ppr"
module a; res #(.r(1e3)) R1(.p(x), .n(y)); endmodule
module b; cap #(.c(1e-9)) C1(.p(x), .n(y)); endmodule
"#;
    let full = format!("`include \"ngspice.ppr\"\n{}", r#"
module a; res #(.r(1e3)) R1(.p(x), .n(y)); endmodule
module b; cap #(.c(1e-9)) C1(.p(x), .n(y)); endmodule
"#);
    let dirs = vec![piperine_ngspice::ppr_dir(), piperine_parser::bundled_header_dir()];
    let doc = parse_with_includes(&full, &dirs).expect("parse failed");
    let mut reg = HardwareRegistry::new();
    register_hardware(&mut reg);

    let ca = elaborate_circuit(&doc, &reg, Some("a")).expect("elaborate a failed");
    assert!(ca.spice_lines.iter().any(|l| l.contains("1000")), "module a should have resistor");
    assert!(!ca.spice_lines.iter().any(|l| l.contains("1e-9")), "module a must not have cap");

    let cb = elaborate_circuit(&doc, &reg, Some("b")).expect("elaborate b failed");
    // Cap value may be formatted as 0.000000001 or 1e-9 depending on serializer.
    assert!(cb.spice_lines.iter().any(|l| l.starts_with("C1")), "module b should have cap");
    assert!(!cb.spice_lines.iter().any(|l| l.contains("1000")), "module b must not have resistor");
}

// ── Module hierarchy ──────────────────────────────────────────────────────────

#[test]
fn test_sub_module_flattened() {
    let c = circuit_with_includes(r#"
module inv(in, out);
    inout in, out;
    res #(.r(500.0)) Rp(.p(out), .n(in));
endmodule

module top;
    wire a, b;
    inv U1(.in(a), .out(b));
endmodule
"#);
    // Flattened resistor exists somewhere in output.
    assert!(c.spice_lines.iter().any(|l| l.contains("500")),
        "flattened sub-module resistor missing:\n{}", c.spice_lines.join("\n"));
}

// ── SOA checks from always @(step) ───────────────────────────────────────────

#[test]
fn test_soa_always_step_compiles() {
    let c = circuit_with_includes(r#"
module bjt_amp;
    wire c, b, e;
    res #(.r(10e3)) Rc(.p(vcc), .n(c));

    always @(step) begin
        if (V(c) > 30.0) $run_error("Vce_max");
    end
endmodule
"#);
    assert!(!c.soa_checks.is_empty(),
        "always @(step) should produce SOA checks");
    let check = &c.soa_checks[0];
    assert_eq!(check.label, "Vce_max");
    assert!((check.threshold - 30.0).abs() < 1e-9);
    assert!(matches!(check.op, SoaOp::Gt));
    // .meas line should be present.
    assert!(c.spice_lines.iter().any(|l| l.starts_with(".meas")),
        "SOA check must emit .meas line:\n{}", c.spice_lines.join("\n"));
}

#[test]
fn test_no_soa_without_always() {
    let c = circuit_with_includes(r#"module tb; res #(.r(1e3)) R1(.p(a), .n(b)); endmodule"#);
    assert!(c.soa_checks.is_empty());
}

// ── ngspice.ppr include ───────────────────────────────────────────────────────

#[test]
fn test_ngspice_ppr_declares_all_passives() {
    let full = "`include \"ngspice.ppr\"\nmodule tb; endmodule\n";
    let dirs = vec![piperine_ngspice::ppr_dir(), piperine_parser::bundled_header_dir()];
    let doc = parse_with_includes(full, &dirs).expect("parse failed");

    for name in ["res", "cap", "ind", "vsource", "isource", "nmos", "pmos", "npn", "pnp"] {
        assert!(doc.extern_modules.iter().any(|m| m.name.0 == name),
            "`{name}` not declared in ngspice.ppr");
    }
}

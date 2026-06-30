//! Tests for the VA → PHDL source emitter.

use piperine_ams::{Document, document_to_phdl};
use std::path::{Path, PathBuf};

fn bundled_headers() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("headers")
}

/// Parse a fixture using bundled headers first so the fixture's own
/// constants.vams (which has a broken `define P_U0(...)` macro) is not used.
fn parse_fixture(name: &str) -> Document {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures_fmt")
        .join(name);
    let input = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {name} failed: {e}"));
    let dirs = vec![
        bundled_headers(),
        path.parent().unwrap().to_path_buf(),
    ];
    Document::parse_with_includes(&input, &dirs)
        .unwrap_or_else(|e| panic!("parse {name} failed: {e}"))
}

// ─────────────────────── conductor ───────────────────────────────────────────

#[test]
fn conductor_has_electrical_discipline() {
    let doc = parse_fixture("conductor.vams");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("discipline Electrical"), "missing discipline: {phdl}");
}

#[test]
fn conductor_module_declaration() {
    let doc = parse_fixture("conductor.vams");
    let phdl = document_to_phdl(&doc);
    // Module with two inout Electrical ports
    assert!(phdl.contains("mod conductor("), "missing mod: {phdl}");
    assert!(phdl.contains("inout p: Electrical"), "missing port p: {phdl}");
    assert!(phdl.contains("inout n: Electrical"), "missing port n: {phdl}");
}

#[test]
fn conductor_param_emitted() {
    let doc = parse_fixture("conductor.vams");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("param g: Real"), "missing param: {phdl}");
}

#[test]
fn conductor_analog_contrib() {
    let doc = parse_fixture("conductor.vams");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("analog conductor"), "missing analog block: {phdl}");
    assert!(phdl.contains("<+"), "missing contribution: {phdl}");
    assert!(phdl.contains("I("), "missing I() call: {phdl}");
    assert!(phdl.contains("V("), "missing V() call: {phdl}");
}

// ─────────────────────── resistor (res1 + res2) ──────────────────────────────

#[test]
fn resistor_both_modules_emitted() {
    let doc = parse_fixture("resistor.va");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("mod res1("), "missing res1: {phdl}");
    assert!(phdl.contains("mod res2("), "missing res2: {phdl}");
}

#[test]
fn resistor_res1_contrib() {
    let doc = parse_fixture("resistor.va");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("analog res1"), "missing analog res1: {phdl}");
    // I(p, n) <+ V(p, n) / r
    assert!(phdl.contains("I(p, n) <+ V(p, n) / r"), "wrong contrib: {phdl}");
}

#[test]
fn resistor_res2_voltage_contrib() {
    let doc = parse_fixture("resistor.va");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("analog res2"), "missing analog res2: {phdl}");
    // V(p, n) <+ r * I(p, n)
    assert!(phdl.contains("V(p, n) <+ r * I(p, n)"), "wrong contrib: {phdl}");
}

#[test]
fn resistor_param_r_with_default() {
    let doc = parse_fixture("resistor.va");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("param r: Real = 1"), "missing param r: {phdl}");
}

// ─────────────────────── ohmmeter ────────────────────────────────────────────

#[test]
fn ohmmeter_ports_all_present() {
    let doc = parse_fixture("ohmmeter.va");
    let phdl = document_to_phdl(&doc);
    for name in &["dutp", "dutm", "iprobe", "r", "g"] {
        assert!(phdl.contains(name), "missing port {name}: {phdl}");
    }
}

#[test]
fn ohmmeter_variables_emitted() {
    let doc = parse_fixture("ohmmeter.va");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("var r_val: Real"), "missing r_val: {phdl}");
    assert!(phdl.contains("var g_val: Real"), "missing g_val: {phdl}");
}

#[test]
fn ohmmeter_if_stmts_emitted() {
    let doc = parse_fixture("ohmmeter.va");
    let phdl = document_to_phdl(&doc);
    // Should contain at least one if block
    assert!(phdl.contains("if ("), "missing if: {phdl}");
    assert!(phdl.contains("max_resistance"), "missing max_resistance: {phdl}");
}

#[test]
fn ohmmeter_analog_block_present() {
    let doc = parse_fixture("ohmmeter.va");
    let phdl = document_to_phdl(&doc);
    assert!(phdl.contains("analog ohmmeter"), "missing analog block: {phdl}");
}

// ─────────────────────── lossy_ind (instances + internal nets) ────────────────

#[test]
fn lossy_ind_internal_wires_emitted() {
    let doc = parse_fixture("lossy_ind.vams");
    let phdl = document_to_phdl(&doc);
    // n1 and n2 are internal electrical nets
    assert!(phdl.contains("wire n1: Electrical"), "missing wire n1: {phdl}");
    assert!(phdl.contains("wire n2: Electrical"), "missing wire n2: {phdl}");
}

#[test]
fn lossy_ind_instances_emitted() {
    let doc = parse_fixture("lossy_ind.vams");
    let phdl = document_to_phdl(&doc);
    // Structural instances: Rp, Hr, Cp, L
    assert!(phdl.contains("Rp:"), "missing instance Rp: {phdl}");
    assert!(phdl.contains("Cp:"), "missing instance Cp: {phdl}");
    assert!(phdl.contains("L:"), "missing instance L: {phdl}");
}

#[test]
fn lossy_ind_si_numbers_converted() {
    let doc = parse_fixture("lossy_ind.vams");
    let phdl = document_to_phdl(&doc);
    // Parameters with SI suffixes should become floats
    // l = 1n → 1e-9, cp = 10f → 1e-14, h = 70K → 70000
    // At minimum, the SI suffix chars should not appear bare in a param default
    assert!(!phdl.contains("= 1n"), "bare SI suffix 1n in output: {phdl}");
    assert!(!phdl.contains("= 10f"), "bare SI suffix 10f in output: {phdl}");
}

// ─────────────────────── snapshot / smoke ────────────────────────────────────

#[test]
fn conductor_output_is_valid_phdl_snapshot() {
    let doc = parse_fixture("conductor.vams");
    let phdl = document_to_phdl(&doc);
    // Structural check: discipline → mod → analog, each exactly once
    assert_eq!(phdl.matches("discipline Electrical").count(), 1);
    assert_eq!(phdl.matches("mod conductor(").count(), 1);
    assert_eq!(phdl.matches("analog conductor").count(), 1);
}

#[test]
fn print_conductor_phdl() {
    let doc = parse_fixture("conductor.vams");
    let phdl = document_to_phdl(&doc);
    // Uncomment to eyeball the output:
    // println!("{}", phdl);
    assert!(!phdl.is_empty());
}

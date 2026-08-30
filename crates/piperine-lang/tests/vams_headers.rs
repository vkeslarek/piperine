//! LRM-COVERS: 9.23.4, D.1, D.2, D.3
//! lrm-conformance-wave3 (Phase 6, T38): §9.23.4 `$driver_type`'s "bit
//! meanings" (the LRM's own words) are the twelve `DRIVER_*` constants
//! `driver_access_vams_bit_values_match_the_lrm_exactly` cross-checks
//! against the LRM text directly, and
//! `driver_access_constants_compose_with_driver_type_return_values`
//! proves they compose as a bitmask the way `$driver_type`'s own return
//! convention requires.
//! T40 (VAMS-09) — the stdlib headers migrated to Verilog-AMS. Proves the
//! two Annex-D-derived headers (`headers/disciplines.vams`,
//! `headers/constants.vams`) each elaborate standalone (via `` `include``,
//! resolved through `SourceMap::dummy()`'s "piperine" -> "headers"
//! namespace, LRM ch. 10 / VAMS-16) and that the migrated `Nature`/
//! `DisciplineDecl` values match the deleted PHDL predecessor's values
//! (`headers/disciplines.phdl`'s `Electrical` discipline: potential unit
//! "V"/abstol 1e-6, flow unit "A"/abstol 1e-12).

use piperine_lang::pom::Design;
use piperine_lang::{SourceMap, parse_and_elaborate};

fn elaborate_including(path: &str) -> Design {
    let src = format!("`include \"{path}\"\n");
    parse_and_elaborate(&src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("`include \"{path}\" failed to elaborate: {e:?}"))
}

#[test]
fn constants_vams_elaborates_standalone_with_no_declarations() {
    // constants.vams is `define-only (Annex D.2) — after preprocessing it
    // contributes zero natures/disciplines/modules, but it must still
    // parse and elaborate cleanly on its own (VAMS-09's "each header
    // elaborates standalone").
    let design = elaborate_including("constants.vams");
    assert_eq!(design.natures().count(), 0, "constants.vams declares no natures");
    assert_eq!(design.disciplines().count(), 0, "constants.vams declares no disciplines");
}

#[test]
fn constants_vams_macros_carry_the_phdl_predecessors_numeric_values() {
    // Cross-check against `constants.phdl`'s values (VAMS-09's "values
    // cross-checked against their PHDL predecessor's constants"): expand
    // `M_PI` and `P_Q` (the NIST2010-selected default, matching
    // `constants.phdl`'s `const P_Q : Real = 1.602176565e-19;`) inside a
    // real module and assert on the folded parameter default.
    let src = r#"
        `include "constants.vams"
        module top ();
            parameter real pi = `M_PI;
            parameter real q = `P_Q;
        endmodule
    "#;
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = design.module("top").expect("module `top`");
    let pi = m.param("pi").expect("param `pi`");
    let q = m.param("q").expect("param `q`");
    let pi_val = pi.default.as_ref().and_then(|v| v.as_real()).expect("pi default is numeric");
    let q_val = q.default.as_ref().and_then(|v| v.as_real()).expect("q default is numeric");
    assert!((pi_val - std::f64::consts::PI).abs() < 1e-12, "M_PI = {pi_val}");
    assert!((q_val - 1.602176565e-19).abs() < 1e-25, "P_Q (NIST2010 default) = {q_val}");
}

#[test]
fn disciplines_vams_elaborates_standalone_and_matches_the_phdl_electrical_discipline() {
    let design = elaborate_including("disciplines.vams");

    // Current nature: PHDL's Electrical discipline used flow unit "A",
    // abstol 1e-12 — same values the LRM's own Current nature carries.
    let current = design.nature("Current").expect("nature `Current`");
    assert_eq!(current.units.as_deref(), Some("A"));
    assert_eq!(current.access.as_deref(), Some("I"));
    assert_eq!(current.idt_nature.as_deref(), Some("Charge"));
    assert_eq!(current.abstol, Some(1e-12));

    // Voltage nature: PHDL's Electrical discipline used potential unit
    // "V", abstol 1e-6 — same values.
    let voltage = design.nature("Voltage").expect("nature `Voltage`");
    assert_eq!(voltage.units.as_deref(), Some("V"));
    assert_eq!(voltage.access.as_deref(), Some("V"));
    assert_eq!(voltage.idt_nature.as_deref(), Some("Flux"));
    assert_eq!(voltage.abstol, Some(1e-6));

    // The `electrical` discipline binds potential=Voltage, flow=Current —
    // the same potential/flow pairing `Electrical { potential v: ...;
    // flow i: ...; }` encoded in PHDL.
    let (_, electrical) = design
        .disciplines()
        .find(|(name, _)| name.as_str() == "electrical")
        .expect("discipline `electrical`");
    assert_eq!(electrical.potential.as_deref(), Some("Voltage"));
    assert_eq!(electrical.flow.as_deref(), Some("Current"));

    // The digital `logic`/`ddiscrete` disciplines (PHDL's `Logic`/
    // `DDiscrete`) carry a discrete domain, no potential/flow.
    let (_, logic) = design
        .disciplines()
        .find(|(name, _)| name.as_str() == "logic")
        .expect("discipline `logic`");
    assert_eq!(logic.domain, Some(piperine_lang::ast::Domain::Discrete));
    assert!(logic.potential.is_none() && logic.flow.is_none());
}

// ── T57: Annex D.3 driver_access.vams (LRMC-C10-16) ──────────────────────

/// Re-derives the twelve `` `define DRIVER_* `` bit-values straight from
/// `VAMS-LRM-2-4.md`'s own Annex D.3 listing text — the LRM's markdown
/// extraction squashes the whitespace around each `32'b<bits>` literal
/// into one run (`DRIVER_UNKNOWN32'b00000000000// No information`), so
/// this scans for `NAME` immediately followed by digits+`'b`+binary
/// digits, independent of `headers/driver_access.vams`'s own text —
/// mirrors `annex_b_keywords.rs`'s "re-derive, don't just copy" gate.
fn driver_bits_from_lrm() -> std::collections::BTreeMap<String, u32> {
    let lrm_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VAMS-LRM-2-4.md");
    let text = std::fs::read_to_string(&lrm_path).unwrap_or_else(|e| panic!("read {}: {e}", lrm_path.display()));

    let start = text.find("D.3 The driver_access.vams file").expect("Annex D.3 heading not found");
    let end = text[start..].find("Annex E").map(|i| start + i).expect("Annex E heading not found after D.3");
    let block = &text[start..end];

    let mut out = std::collections::BTreeMap::new();
    let mut rest = block;
    while let Some(name_start) = rest.find("DRIVER_") {
        let after_name = &rest[name_start..];
        let name_end = after_name.find(|c: char| !(c.is_ascii_uppercase() || c == '_')).unwrap_or(after_name.len());
        let name = &after_name[..name_end];
        let after = &after_name[name_end..];
        // Skip the width digits, then require a `'b` (LRM's typographic
        // apostrophe or ASCII) followed by binary digits.
        let digits_end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
        let after_width = &after[digits_end..];
        let quote_len =
            if after_width.starts_with('\'') { 1 } else { after_width.chars().next().map(char::len_utf8).unwrap_or(0) };
        if after_width.get(quote_len..quote_len + 1) == Some("b") {
            let bits_start = quote_len + 1;
            let bits_end = after_width[bits_start..]
                .find(|c: char| c != '0' && c != '1')
                .map(|i| bits_start + i)
                .unwrap_or(after_width.len());
            let bits = &after_width[bits_start..bits_end];
            if !bits.is_empty() {
                let value = u32::from_str_radix(bits, 2).unwrap_or_else(|e| panic!("bad binary {bits:?}: {e}"));
                out.insert(name.to_string(), value);
            }
        }
        rest = &rest[name_start + name.len()..];
    }
    out
}

#[test]
fn driver_access_vams_bit_values_match_the_lrm_exactly() {
    let expected = driver_bits_from_lrm();
    assert_eq!(expected.len(), 12, "Annex D.3 defines twelve DRIVER_* macros");

    // Expand every `` `define DRIVER_* `` inside a real module and read
    // back the folded parameter default — proves both textual fidelity
    // and that the file actually elaborates as Verilog-AMS.
    let defines: String =
        expected.keys().map(|name| format!("parameter integer {name}_v = `{name};\n")).collect();
    let src = format!("`include \"driver_access.vams\"\nmodule top ();\n{defines}endmodule\n");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("driver_access.vams elaborates");
    let m = design.module("top").expect("module `top`");

    for (name, &bits) in &expected {
        let p = m.param(&format!("{name}_v")).unwrap_or_else(|| panic!("param `{name}_v` not found"));
        // A `32'b...` binary literal folds to `Value::Nat` (unsigned), not
        // `Value::Int` — `as_natural` is the right accessor here.
        let got = p.default.as_ref().and_then(|v| v.as_natural()).unwrap_or_else(|| panic!("`{name}` has no natural default"));
        assert_eq!(got as u32, bits, "`{name}` value mismatch");
    }
}

/// T35's `$driver_type` return value is directly usable against these
/// constants — a plain bitwise-AND against `` `DRIVER_GATE `` etc.
/// elaborates and folds, proving the header and T35's return convention
/// (a bitmask, per §9.22.7's own "returns an integer value with its bits
/// set") compose with no adapter needed.
#[test]
fn driver_access_constants_compose_with_driver_type_return_values() {
    let src = r#"
        `include "driver_access.vams"
        module top ();
            parameter integer is_gate = (`DRIVER_GATE | `DRIVER_UDP) & `DRIVER_GATE;
        endmodule
    "#;
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = design.module("top").unwrap();
    let p = m.param("is_gate").unwrap();
    let default = p.default.as_ref().expect("`is_gate` has a default");
    let got = default.as_natural().or_else(|| default.as_integer().map(|i| i as u64));
    assert_eq!(got, Some(0b10), "default was {default:?}");
}

#[test]
fn disciplines_vams_include_guard_is_idempotent_on_double_include() {
    // The Annex D `` `ifdef DISCIPLINES_VAMS `else ... `endif`` guard
    // (verbatim from the LRM listing) must make a second `` `include`` a
    // no-op rather than a duplicate-declaration failure.
    let src = r#"
        `include "disciplines.vams"
        `include "disciplines.vams"
    "#;
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("double include elaborates");
    assert_eq!(
        design.natures().filter(|(n, _)| n.as_str() == "Current").count(),
        1,
        "the include guard prevents Current from being declared twice"
    );
}

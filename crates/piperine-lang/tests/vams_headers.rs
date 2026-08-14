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

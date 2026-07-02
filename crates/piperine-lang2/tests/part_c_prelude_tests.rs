//! Tests for Part C of `docs/GAPS.md` — standard library & prelude.
//!
//! - C.1: `Ground` discipline predefined.
//! - C.3: `Type`/`Net` capabilities predefined.

use piperine_lang::parse_and_elaborate;

fn elab(src: &str) -> Result<piperine_lang::pom::Design, String> {
    parse_and_elaborate(src)
}

// ── C.1 — `Ground` discipline is predefined ──────────────────────────────────
//
// The spec (§6.2) says `Ground` is predefined and fixed at zero. Before
// the fix, `gnd : Ground` in a `mod` body produced an `UndefinedDiscipline`
// error during elaboration. After the fix, the resolver injects a
// `Ground` discipline into every program, so `Ground` is just a name
// that always elaborates.

#[test]
fn c1_ground_discipline_is_predefined() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Resistor ( inout p : Electrical, inout n : Ground ) {
            param r : Real = 1.0e3;
        }
        analog Resistor { I(p, n) <+ V(p, n) / r; }
    "#;
    let prog = elab(src).expect("`Ground` should be predefined; elaboration must succeed");
    let has_ground = prog
        .disciplines()
        .any(|(k, _)| k == "Ground" || k == "ground");
    assert!(has_ground, "C.1: `Ground` should appear in `prog.disciplines`");
}

#[test]
fn c1_wire_with_ground_type_resolves() {
    // A `wire : Ground` should also elaborate (the resolver injects Ground
    // before module body elaboration, so wire references resolve too).
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Top ( inout p : Electrical ) {
            wire g : Ground;
        }
    "#;
    elab(src).expect("`wire g : Ground` should elaborate (Ground is predefined)");
}

#[test]
fn c1_unknown_discipline_still_errors() {
    // Sanity check: a discipline that isn't predefined (and isn't declared)
    // must still produce an error. `Foo` is not the predefined `Ground`.
    let src = r#"
        mod Bad ( inout p : Foo ) { }
    "#;
    let result = elab(src);
    assert!(
        result.is_err(),
        "C.1: an undeclared discipline `Foo` must still be a hard error"
    );
}
// ── C.3 — `Type` and `Net` capabilities are predefined ───────────────────────
//
// Spec §6.6: "`Type` (any value type) and `Net` (any net type) are the
// root capabilities." After the fix, both names are bound to the
// stdlib's empty marker capabilities so generic bundles like
// `bundle Pair <T: Type>` elaborate without needing a `use`.

#[test]
fn c3_type_and_net_capabilities_are_predefined() {
    let src = r#"
        bundle Pair <T: Type> { fst : T, snd : T }
    "#;
    let prog = elab(src).expect("`Type` should be predefined; <T: Type> must elaborate");
    assert!(
        prog.capability("Type").is_some(),
        "C.3: `Type` capability should appear in `prog.capabilities`"
    );
}

#[test]
fn c3_net_capability_is_predefined() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        bundle Diff <X: Net> { p : X, n : X }
    "#;
    let prog = elab(src).expect("`Net` should be predefined; <X: Net> must elaborate");
    assert!(
        prog.capability("Net").is_some(),
        "C.3: `Net` capability should appear in `prog.capabilities`"
    );
}

#[test]
fn c4_constants_are_resolvable() {
    let src = r#"
        use piperine::constants;
        mod Top () {
            param pi : Real = M_PI;
        }
    "#;
    let prog = elab(src).expect("`M_PI` should be resolvable and elaborate successfully");
    assert!(
        prog.const_("M_PI").is_some(),
        "C.4: `M_PI` should appear in `prog.consts`"
    );
}

#[test]
fn c5_disciplines_are_resolvable() {
    let src = r#"
        use piperine::disciplines;
        mod Top () {
            wire n1 : Electrical;
            wire n2 : Kinematic;
            wire n3 : Thermal;
        }
    "#;
    let prog = elab(src).expect("`Electrical`, `Kinematic`, `Thermal` should be resolvable and elaborate successfully");
    assert!(prog.discipline("Electrical").is_some());
    assert!(prog.discipline("Kinematic").is_some());
    assert!(prog.discipline("Thermal").is_some());
}

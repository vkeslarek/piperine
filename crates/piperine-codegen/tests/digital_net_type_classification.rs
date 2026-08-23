//! LRM-COVERS: 3.7, 7.3.1
//!
//! LRMW2-45 (`lrm-conformance-wave2`, T52): codegen-layer proof that a
//! `Digital1364` net type (§3.7) reaches `resolve::symbols::SymbolTable`
//! correctly classified. `wire`/`tri`/`reg` classify `Domain::Digital`
//! (Table 7-1's "bit" row — read from a continuous context through the
//! digital-to-analog shadow-variable bridge, `Type::Bool`, same as any
//! other digital node). `wreal` is the one exception in the family: §3.7
//! itself describes it as "a real-valued physical connection" with no
//! 4-state semantics at all — it classifies `Domain::Analog`, so its
//! value is carried as a plain `f64` through the *analog* path with no
//! shadow-variable bridge (and no boolean/4-state coercion) at all. Before
//! this task, all four keywords silently became `NetType::Discipline(name)`
//! — an undeclared discipline, which `domain_of` conservatively defaults
//! to `Domain::Analog` — so `wire`/`tri`/`reg` were the ones silently
//! misclassified (this test's own `wire` case would have failed pre-T52,
//! not `wreal`'s). `reg`'s own IR-validation proof (an internal `reg` net
//! as a legitimate digital write target) and the full simulated
//! `onebit_dac` proof are root `tests/onebit_dac_e2e.rs`; `wreal`'s own
//! simulated proof (`assign out = V(in);`) is the pre-existing root
//! `tests/vams_continuous_assign_e2e.rs::wreal_target_is_driven_by_assign_
//! end_to_end`, unaffected by this task.

use piperine_codegen::resolve::Domain;
use piperine_lang::parse_and_elaborate;

fn elab_bodies(src: &str) -> (piperine_lang::Design, std::collections::HashMap<String, piperine_codegen::resolve::LoweredBody>) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    (design, bodies)
}

/// `wreal` classifies `Domain::Analog` — its value never routes through a
/// digital shadow variable at all (`resolve_expr`'s `Ident` arm keeps it
/// as a plain `Ident`, the same path an ordinary `electrical` node takes).
#[test]
fn wreal_node_classifies_as_analog_domain_with_no_shadow_variable() {
    let (_design, bodies) = elab_bodies(
        r#"
        module top ();
            electrical out;
            wreal w;
            assign w = 4.25;
            analog V(out) <+ w;
        endmodule
        "#,
    );
    let top = &bodies["top"];
    let (_, info) = top.symbols.nodes().find(|(_, n)| n.name == "w").expect("node `w`");
    assert_eq!(info.domain, Domain::Analog, "a `wreal` net must classify as Domain::Analog, not Digital");
    assert!(
        top.symbols.vars().all(|(_, v)| v.name != "__shadow_w"),
        "reading `w` from the analog body must never allocate a digital shadow variable"
    );
}

/// Discrimination: `wire` (unlike `wreal`) classifies `Domain::Digital`
/// and *does* route through the shadow-variable bridge when read from an
/// analog body — proving `wreal`'s analog classification above is
/// specific to `wreal`, not a blanket change swallowing every §3.7
/// keyword into the analog domain.
#[test]
fn wire_node_classifies_as_digital_domain_with_a_shadow_variable() {
    let (_design, bodies) = elab_bodies(
        r#"
        module top ();
            electrical out;
            wire w;
            real x;
            analog begin
                if (w == 0) x = 0.0; else x = 1.0;
                V(out) <+ x;
            end
        endmodule
        "#,
    );
    let top = &bodies["top"];
    let (_, info) = top.symbols.nodes().find(|(_, n)| n.name == "w").expect("node `w`");
    assert_eq!(info.domain, Domain::Digital, "a `wire` net must classify as Domain::Digital");
    assert!(
        top.symbols.vars().any(|(_, v)| v.name == "__shadow_w"),
        "reading `w` from the analog body must allocate a digital shadow variable"
    );
}

//! POM → resolved-form lowering: what `resolve::lower_bodies` records for a
//! module — contribution shape, operator state slots, noise sources, symbols —
//! and the two fail-loud unresolved-name cases.
//!
//! Restored from the never-compiled `ppr_ir.rs` (P6/CLN-06): the assertions
//! are the originals, re-expressed against today's API. The old suite matched
//! on `IrStmt`/`IrExpr` variants that no longer exist — `AnalogBody::stmts` is
//! the POM `Stmt` tree now, with names resolved through `symbols` — so the
//! structural checks that survive are the resolved ones (`states`, `noise`,
//! `symbols`, the contribution's statement shape).

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::resolve::{lower_bodies, LoweredBody, NoiseKind, StateKind};
use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "`include \"disciplines.vams\"";

/// The `SourceMap` reaching `piperine-lang`'s bundled headers from this crate
/// (`SourceMap::dummy`'s relative paths only resolve from that crate's dir).
fn source_map() -> piperine_lang::SourceMap {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A two-terminal `TestMod` whose analog body is `body`.
fn test_mod(body: &str) -> String {
    format!(
        "{DISCIPLINE}
module TestMod(p, n);
    inout electrical p, n;
    parameter real R = 1000.0;
    parameter real C = 1e-6;
    analog {body}
endmodule
"
    )
}

/// Lower `src` and return the named module's resolved body.
fn lower(src: &str, module: &str) -> LoweredBody {
    let design = parse_and_elaborate(src, &source_map()).expect("elaborate");
    lower_bodies(&design).expect("lowering")[module].clone()
}

/// The resolved body's single analog statement.
fn only_stmt(body: &LoweredBody) -> Stmt {
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.stmts.len(), 1, "expected exactly one statement");
    analog.stmts[0].clone()
}

// ─── Contributions ────────────────────────────────────────────────────────────

#[test]
fn a_resistive_contribution_lowers_to_a_contrib_bind() {
    let body = lower(&test_mod("I(p, n) <+ V(p, n) / R;"), "TestMod");
    match only_stmt(&body) {
        Stmt::Contribution { target: Expr::AccessFunction { function, .. }, .. } => {
            assert_eq!(function, "I", "a flow contribution");
        }
        other => panic!("expected a `<+` contribution bind, got {other:?}"),
    }
    // The branch nodes are resolved symbols, not names.
    let nodes: Vec<&str> = body.symbols.nodes().map(|(_, info)| info.name.as_str()).collect();
    assert!(nodes.contains(&"p") && nodes.contains(&"n"), "branch nodes resolved: {nodes:?}");
}

/// PHDL spelled a potential *force* `V(p, n) <- 1.0` with its own `BindOp`;
/// Verilog-AMS has one contribution operator and the access function on the
/// left is what makes it a force (LRM §5.4). The lowered shape is therefore a
/// `Contribution` whose target is the `V` access function.
#[test]
fn a_potential_force_lowers_to_a_force_bind() {
    let body = lower(&test_mod("V(p, n) <+ 1.0;"), "TestMod");
    match only_stmt(&body) {
        Stmt::Contribution { target: Expr::AccessFunction { function, .. }, .. } => {
            assert_eq!(function, "V", "a potential force");
        }
        other => panic!("expected a `<+` force bind, got {other:?}"),
    }
}

// ─── Operator state slots ─────────────────────────────────────────────────────

#[test]
fn ddt_registers_one_ddt_state_slot() {
    let body = lower(&test_mod("I(p, n) <+ C * ddt(V(p, n));"), "TestMod");
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.states.len(), 1, "one state slot");
    assert!(
        matches!(body.symbols.state(analog.states[0]).kind, StateKind::Ddt),
        "the slot is a ddt slot"
    );
}

#[test]
fn idtmod_registers_its_state_slot() {
    let body = lower(&test_mod("I(p, n) <+ idtmod(V(p, n), 0.0, 1.0);"), "TestMod");
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.states.len(), 1);
    assert!(
        matches!(body.symbols.state(analog.states[0]).kind, StateKind::IdtMod { .. }),
        "the slot carries the modular-integral kind (ic + modulus)"
    );
}

#[test]
fn transition_registers_its_state_slot() {
    let body = lower(&test_mod("I(p, n) <+ transition(V(p, n), 0.0, 1e-6, 1e-6);"), "TestMod");
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.states.len(), 1);
    assert!(
        matches!(body.symbols.state(analog.states[0]).kind, StateKind::Transition { .. }),
        "the slot carries the transition kind — the JIT's refusal to emit it is a \
         separate contract (see analog_kernel.rs)"
    );
}

// ─── Noise sources ────────────────────────────────────────────────────────────

/// The LRM signature is `white_noise(pwr[, name])` — the trailing name
/// argument is optional and carries no lowered label, so the label assertion
/// the original test made is dropped rather than faked.
#[test]
fn white_noise_registers_a_source_on_its_branch() {
    let body = lower(&test_mod("I(p, n) <+ white_noise(1e-24);"), "TestMod");
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.noise.len(), 1, "one noise source");
    let source = &analog.noise[0];
    assert_eq!(body.symbols.node(source.plus).name, "p");
    assert_eq!(body.symbols.node(source.minus).name, "n");
    assert!(matches!(source.kind, NoiseKind::White { .. }));
}

/// A noise source is attributed to the branch it is contributed to even when
/// the contribution sits inside a conditional — the shape the whole spice
/// stdlib uses (`if (noisy) I(p,n) <+ white_noise(…);`). Recording it on
/// ground↔ground instead makes it stamp nothing, which reads as a model with
/// no noise rather than a lowering that lost the branch: a silent zero, the
/// one thing this project never emits.
#[test]
fn white_noise_inside_a_conditional_keeps_its_branch() {
    let body = lower(
        &test_mod("if (V(p, n) > 0.0) I(p, n) <+ white_noise(1e-24);"),
        "TestMod",
    );
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.noise.len(), 1, "the guarded source is still registered");
    let source = &analog.noise[0];
    assert_eq!(body.symbols.node(source.plus).name, "p", "plus is the contribution's node");
    assert_eq!(body.symbols.node(source.minus).name, "n", "minus is the contribution's node");
}

#[test]
fn flicker_noise_registers_a_flicker_source() {
    let body = lower(&test_mod("I(p, n) <+ flicker_noise(1e-25, 2.0);"), "TestMod");
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.noise.len(), 1);
    assert!(matches!(analog.noise[0].kind, NoiseKind::Flicker { .. }));
}

// ─── Events ───────────────────────────────────────────────────────────────────

/// PHDL's `@ cross(...) when (guard) { … }` had a first-class guard clause;
/// Annex A's `event_expression` has none (a condition is written as an `if`
/// *inside* the event's body), so what survives the frontend swap is the
/// event's body — asserted here.
#[test]
fn a_cross_event_keeps_its_body() {
    let src = format!(
        "{DISCIPLINE}
module GuardMod(p, n);
    inout electrical p, n;
    analog @(cross(V(p, n), 0)) begin
        if (V(p, n) > 0.0) I(p, n) <+ 1.0;
    end
endmodule
"
    );
    let body = lower(&src, "GuardMod");
    match only_stmt(&body) {
        Stmt::EventControl { body, .. } => match *body {
            Stmt::Block { stmts, .. } => {
                assert_eq!(stmts.len(), 1, "the guarded body survives");
            }
            other => panic!("expected a block body, got {other:?}"),
        },
        other => panic!("expected an event statement, got {other:?}"),
    }
}

#[test]
fn an_above_event_lowers_as_an_event() {
    let src = format!(
        "{DISCIPLINE}
module AboveMod(p, n);
    inout electrical p, n;
    analog @(above(V(p, n))) begin
        I(p, n) <+ 0.0;
    end
endmodule
"
    );
    let body = lower(&src, "AboveMod");
    match only_stmt(&body) {
        Stmt::EventControl { .. } => {}
        other => panic!("expected an event statement, got {other:?}"),
    }
}

// ─── Symbols ──────────────────────────────────────────────────────────────────

#[test]
fn a_string_param_keeps_its_default() {
    let src = format!(
        "{DISCIPLINE}
module StrMod(p, n);
    inout electrical p, n;
    parameter string name = \"res1\";
    analog I(p, n) <+ V(p, n) / 1000.0;
endmodule
"
    );
    let body = lower(&src, "StrMod");
    let param = body
        .symbols
        .params()
        .map(|(_, info)| info)
        .find(|info| info.name == "name")
        .expect("`name` param");
    assert!(param.default.is_some(), "a String param keeps its default through lowering");
}

#[test]
fn a_digital_body_is_lowered() {
    let src = "
discipline logic;
    domain discrete;
enddiscipline

module DigMod(clk, q);
    input logic clk;
    output logic q;
    always @(clk) q = 1;
endmodule
";
    let body = lower(src, "DigMod");
    assert!(body.digital.is_some(), "the digital body reaches the resolved form");
}

// ─── Fail-loud unresolved names ───────────────────────────────────────────────

/// An unresolved analog identifier must never reach executable code — it used
/// to lower silently to `ParamId(0)`, stamping whatever param 0 held.
///
/// The refusal now happens at kernel compile (IR validation) rather than in
/// `lower_bodies`, which keeps the name as a POM `Ident` for the flattener to
/// resolve: the guarantee is unchanged (nothing executable is produced), the
/// boundary moved.
#[test]
fn an_unknown_name_in_a_contribution_is_refused_before_any_code_is_emitted() {
    let design =
        parse_and_elaborate(&test_mod("I(p, n) <+ V(p, n) / not_a_param;"), &source_map())
            .expect("elaborate");
    let bodies = lower_bodies(&design).expect("names are resolved later, not here");
    let err = AnalogKernel::compile(&bodies["TestMod"])
        .err()
        .expect("an unresolved identifier must not compile");
    let message = err.to_string();
    assert!(message.contains("not_a_param"), "names the symbol: {message}");
    assert!(message.contains("unresolved"), "says what went wrong: {message}");
}

// PHDL's companion case — `an_unknown_net_in_an_instance_connection_fails_lowering`
// — has no Verilog-AMS meaning and is gone: an undeclared identifier in a port
// connection is a legal *implicit net* declaration (1364 §3.5, inherited by the
// LRM), so the elaborator creates the net rather than refusing it. There is no
// "unknown net" left for `lower_bodies` to fail on.

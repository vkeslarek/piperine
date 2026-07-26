//! POM → resolved-form lowering: what `resolve::lower_bodies` records for a
//! module — contribution shape, operator state slots, noise sources, symbols —
//! and the two fail-loud unresolved-name cases.
//!
//! Restored from the never-compiled `ppr_ir.rs` (P6/CLN-06): the assertions
//! are the originals, re-expressed against today's API. The old suite matched
//! on `IrStmt`/`IrExpr` variants that no longer exist — `AnalogBody::stmts` is
//! the POM `Stmt` tree now, with names resolved through `symbols` — so the
//! structural checks that survive are the resolved ones (`states`, `noise`,
//! `symbols`, the contribution's bind op).

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::resolve::{lower_bodies, LoweredBody, NoiseKind, StateKind};
use piperine_lang::parse::ast::{BindOp, Stmt};
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// A two-terminal `TestMod` whose analog body is `body`.
fn test_mod(body: &str) -> String {
    format!(
        "{DISCIPLINE}
mod TestMod(inout p: Electrical, inout n: Electrical) {{
    param R: Real = 1000.0;
    param C: Real = 1e-6;
}}
analog TestMod {{
    {body}
}}
"
    )
}

/// Lower `src` and return the named module's resolved body.
fn lower(src: &str, module: &str) -> LoweredBody {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
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
        Stmt::Bind { op: BindOp::Contrib, .. } => {}
        other => panic!("expected a `<+` contribution bind, got {other:?}"),
    }
    // The branch nodes are resolved symbols, not names.
    let nodes: Vec<&str> = body.symbols.nodes().map(|(_, info)| info.name.as_str()).collect();
    assert!(nodes.contains(&"p") && nodes.contains(&"n"), "branch nodes resolved: {nodes:?}");
}

#[test]
fn a_potential_force_lowers_to_a_force_bind() {
    let body = lower(&test_mod("V(p, n) <- 1.0;"), "TestMod");
    match only_stmt(&body) {
        Stmt::Bind { op: BindOp::Force, .. } => {}
        other => panic!("expected a `<-` force bind, got {other:?}"),
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

/// The declared signature is `white_noise(pwr)` — the trailing name argument
/// the original test passed (`white_noise(psd, "rn1")`) is no longer part of
/// the operator (`headers/operators.phdl:24`), so the label assertion is
/// dropped rather than faked.
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

#[test]
fn flicker_noise_registers_a_flicker_source() {
    let body = lower(&test_mod("I(p, n) <+ flicker_noise(1e-25, 2.0);"), "TestMod");
    let analog = body.analog.as_ref().expect("analog body");
    assert_eq!(analog.noise.len(), 1);
    assert!(matches!(analog.noise[0].kind, NoiseKind::Flicker { .. }));
}

// ─── Events ───────────────────────────────────────────────────────────────────

#[test]
fn a_cross_event_keeps_its_guard() {
    let src = format!(
        "{DISCIPLINE}
mod GuardMod(inout p: Electrical, inout n: Electrical) {{}}
analog GuardMod {{
    @ cross(V(p, n)) when (V(p, n) > 0.0) {{
        I(p, n) <+ 1.0;
    }}
}}
"
    );
    let body = lower(&src, "GuardMod");
    match only_stmt(&body) {
        Stmt::Event { guard, body, .. } => {
            assert!(guard.is_some(), "the `when` guard survives lowering");
            assert_eq!(body.stmts.len(), 1, "the guarded body survives");
        }
        other => panic!("expected an event statement, got {other:?}"),
    }
}

#[test]
fn an_above_event_lowers_as_an_event() {
    let src = format!(
        "{DISCIPLINE}
mod AboveMod(inout p: Electrical, inout n: Electrical) {{}}
analog AboveMod {{
    @ above(V(p, n)) {{
        I(p, n) <+ 0.0;
    }}
}}
"
    );
    let body = lower(&src, "AboveMod");
    match only_stmt(&body) {
        Stmt::Event { guard: None, .. } => {}
        other => panic!("expected an unguarded event statement, got {other:?}"),
    }
}

// ─── Symbols ──────────────────────────────────────────────────────────────────

#[test]
fn a_global_function_is_registered_in_the_symbol_table() {
    let src = format!(
        "{DISCIPLINE}
fn helper(x: Real) -> Real {{
    return x * 2.0;
}}
mod FnMod(inout p: Electrical, inout n: Electrical) {{}}
analog FnMod {{
    I(p, n) <+ helper(V(p, n));
}}
"
    );
    let body = lower(&src, "FnMod");
    assert!(body.symbols.fn_by_name("helper").is_some(), "`helper` is resolvable");
}

#[test]
fn a_string_param_keeps_its_default() {
    let src = format!(
        "{DISCIPLINE}
mod StrMod(inout p: Electrical, inout n: Electrical) {{
    param name: String = \"res1\";
}}
analog StrMod {{
    I(p, n) <+ V(p, n) / 1000.0;
}}
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
    let src = format!(
        "{DISCIPLINE}
mod DigMod(inout clk: Electrical, inout out: Electrical) {{}}
digital DigMod {{
    @ change(clk) {{
        out <- 1.0;
    }}
}}
"
    );
    let body = lower(&src, "DigMod");
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
    let design = parse_and_elaborate(
        &test_mod("I(p, n) <+ V(p, n) / not_a_param;"),
        &piperine_lang::SourceMap::dummy(),
    )
    .expect("elaborate");
    let bodies = lower_bodies(&design).expect("names are resolved later, not here");
    let err = AnalogKernel::compile(&bodies["TestMod"])
        .err()
        .expect("an unresolved identifier must not compile");
    let message = err.to_string();
    assert!(message.contains("not_a_param"), "names the symbol: {message}");
    assert!(message.contains("unresolved"), "says what went wrong: {message}");
}

#[test]
fn an_unknown_net_in_an_instance_connection_fails_lowering() {
    let design = parse_and_elaborate(
        "discipline Electrical { potential v: Real; flow i: Real; }
        mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
        analog R { I(p, n) <+ V(p, n) / r; }
        mod Top() {
            wire a : Electrical;
            r1 : R(a, no_such_net);
        }",
        &piperine_lang::SourceMap::dummy(),
    )
    .expect("elaborate");
    let err = lower_bodies(&design).expect_err("an unknown net must fail lowering");
    assert!(err.to_string().contains("no_such_net"), "names the net: {err}");
}

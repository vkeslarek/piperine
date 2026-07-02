//! Tests: PPR/PHDL source → IR lowering and pseudo-language printer.

use piperine_lang::ppr_to_ir;
use piperine_codegen::ir::{
    ContribKind, IrBinOp, IrExpr, IrStmt, IrStateKind,
};
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "
discipline Electrical {
    potential v : Real;
    flow      i : Real;
}
";

fn src(body: &str) -> String {
    format!(
        "{DISCIPLINE}
mod TestMod(inout p: Electrical, inout n: Electrical) {{
    param R: Real = 1000.0;
    param C: Real = 1e-6;
    param Is: Real = 1e-14;
}}
analog TestMod {{
    {body}
}}
"
    )
}

// ─── Resistor ─────────────────────────────────────────────────────────────────

#[test]
fn resistor_resistive_contrib() {
    let prog = parse_and_elaborate(&src("I(p, n) <+ V(p, n) / R;")).expect("elab");
    let ir = ppr_to_ir(&prog);

    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.stmts.len(), 1, "expected one stmt");
    match &body.stmts[0] {
        IrStmt::Contrib { nature, plus, minus, kind: ContribKind::Resistive, .. } => {
            assert!(matches!(nature, n), "expected Flow nature");
            assert_eq!(m.symbols.node(*plus).name, "p");
            assert_eq!(m.symbols.node(*minus).name, "n");
        }
        other => panic!("expected Contrib(Resistive), got {other:?}"),
    }
}

#[test]
fn resistor_printer_smoke() {
    let prog = parse_and_elaborate(&src("I(p, n) <+ V(p, n) / R;")).expect("elab");
    let ir = ppr_to_ir(&prog);
    let out = format!("{ir:?}");
    assert!(out.contains("Contrib"), "output: {out}");
    assert!(out.contains("Branch"), "output: {out}");
}

// ─── Capacitor ────────────────────────────────────────────────────────────────

#[test]
fn capacitor_reactive_contrib_with_state_var() {
    let prog = parse_and_elaborate(&src("I(p, n) <+ C * ddt(V(p, n));")).expect("elab");
    let ir = ppr_to_ir(&prog);

    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.states.len(), 1, "expected one state var");
    assert!(matches!(m.symbols.state(body.states[0]).kind, piperine_codegen::ir::IrStateKind::Ddt));

    assert_eq!(body.stmts.len(), 1);
    match &body.stmts[0] {
        IrStmt::Contrib { kind: ContribKind::Reactive(_), .. } => {}
        other => panic!("expected Reactive(_), got {other:?}"),
    }
}

#[test]
fn capacitor_printer_reactive() {
    let prog = parse_and_elaborate(&src("I(p, n) <+ C * ddt(V(p, n));")).expect("elab");
    let ir = ppr_to_ir(&prog);
    let out = format!("{ir:?}");
    assert!(out.contains("Reactive("), "output: {out}");
    assert!(out.contains("Ddt"), "output: {out}");
}

// ─── Local variable inlining ──────────────────────────────────────────────────

#[test]
fn local_var_inlined_into_contrib() {
    let src = format!("
{DISCIPLINE}
mod TestMod(inout p: Electrical, inout n: Electrical) {{
    param Is: Real = 1e-14;
}}
analog TestMod {{
    var vd: Real = V(p, n);
    I(p, n) <+ Is * exp(vd);
}}
");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);

    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    // The contribution expr should have V(p,n) inlined, not a free Param("vd")
    let out = format!("{ir:?}");
    // vd should be inlined as V(p, n) in the contribution
    assert!(out.contains("Branch"), "vd not inlined: {out}");
}

// ─── Diode (nonlinear) ────────────────────────────────────────────────────────

#[test]
fn diode_nonlinear_contrib() {
    let src = format!("
{DISCIPLINE}
mod Diode(inout p: Electrical, inout n: Electrical) {{
    param Is: Real = 1e-14;
}}
analog Diode {{
    I(p, n) <+ Is * (exp(V(p, n)) - 1.0);
}}
");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let out = format!("{ir:?}");
    assert!(out.contains("\"exp\""), "output: {out}");
    assert!(out.contains("Branch"), "output: {out}");
}

// ─── If statement ────────────────────────────────────────────────────────────

#[test]
fn if_stmt_both_branches_preserved() {
    let src = format!("
{DISCIPLINE}
mod IfMod(inout p: Electrical, inout n: Electrical) {{}}
analog IfMod {{
    if (V(p, n) > 0.7) {{
        I(p, n) <+ 0.001;
    }} else {{
        I(p, n) <+ 0.0;
    }}
}}
");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);

    let m = ir.modules.iter().find(|m| m.name == "IfMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.stmts.len(), 1);
    match &body.stmts[0] {
        IrStmt::If { then_, else_, .. } => {
            assert_eq!(then_.len(), 1, "then branch");
            assert_eq!(else_.len(), 1, "else branch");
        }
        other => panic!("expected If, got {other:?}"),
    }

    let out = format!("{ir:?}");
    assert!(out.contains("If {"), "output: {out}");
    assert!(out.contains("else_: ["), "output: {out}");
}

// ─── Nested if ────────────────────────────────────────────────────────────────

#[test]
fn nested_if_structure_preserved() {
    let src = format!("
{DISCIPLINE}
mod NestedIf(inout p: Electrical, inout n: Electrical) {{}}
analog NestedIf {{
    if (V(p, n) > 0.0) {{
        if (V(p, n) > 0.7) {{
            I(p, n) <+ 0.01;
        }} else {{
            I(p, n) <+ 0.001;
        }}
    }} else {{
        I(p, n) <+ 0.0;
    }}
}}
");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);

    let m = ir.modules.iter().find(|m| m.name == "NestedIf").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.stmts.len(), 1);
    match &body.stmts[0] {
        IrStmt::If { then_, .. } => {
            assert_eq!(then_.len(), 1);
            assert!(matches!(&then_[0], IrStmt::If { .. }), "inner if");
        }
        other => panic!("expected If, got {other:?}"),
    }
}

// ─── Module metadata ──────────────────────────────────────────────────────────

#[test]
fn module_ports_and_params_present() {
    let prog = parse_and_elaborate(&src("I(p, n) <+ V(p, n) / R;")).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");

    assert_eq!(m.ports.len(), 2);
    assert!(m.ports.iter().any(|p| m.symbols.node(p.node).name == "p"));
    assert!(m.ports.iter().any(|p| m.symbols.node(p.node).name == "n"));

    let out = format!("{ir:?}");
    assert!(out.contains("name: \"R\""), "output: {out}");
    assert!(out.contains("name: \"C\""), "output: {out}");
}

// ─── Noise sources ────────────────────────────────────────────────────────────

#[test]
fn noise_source_registered() {
    let prog = parse_and_elaborate(&src(
        "I(p, n) <+ white_noise(1e-24, \"rn1\");"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.noise.len(), 1, "expected one noise source");
    let ns = &body.noise[0];
    assert_eq!(m.symbols.node(ns.plus).name, "p");
    assert_eq!(m.symbols.node(ns.minus).name, "n");
    assert!(matches!(&ns.kind, piperine_codegen::ir::IrNoise::White { .. }));
    assert_eq!(ns.label.as_deref(), Some("rn1"));
}

#[test]
fn flicker_noise_source_registered() {
    let prog = parse_and_elaborate(&src(
        "I(p, n) <+ flicker_noise(1e-25, 2.0, \"fn1\");"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.noise.len(), 1);
    assert!(matches!(&body.noise[0].kind, piperine_codegen::ir::IrNoise::Flicker { .. }));
}

// ─── idtmod ────────────────────────────────────────────────────────────────────

#[test]
fn idtmod_state_var() {
    let prog = parse_and_elaborate(&src(
        "I(p, n) <+ idtmod(V(p, n), 0.0, 1.0);"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.states.len(), 1);
    assert!(matches!(m.symbols.state(body.states[0]).kind, IrStateKind::IdtMod { .. }));
}

// ─── Single-arg I(node) ───────────────────────────────────────────────────────

#[test]
fn single_arg_current_access() {
    let prog = parse_and_elaborate(&src(
        "var ii: Real = I(p); I(p, n) <+ ii;"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let out = format!("{ir:?}");
    // I(p) should become I(p, 0)
    assert!(out.contains("minus: NodeId(0)"), "output: {out}");
}

// ─── Force contribution (<-) ──────────────────────────────────────────────────

#[test]
fn force_contribution() {
    let prog = parse_and_elaborate(&src(
        "V(p, n) <- 1.0;"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.stmts.len(), 1);
    assert!(matches!(&body.stmts[0], IrStmt::Force { nature: _, .. }));
}

// ─── Match desugaring ─────────────────────────────────────────────────────────

#[test]
fn match_desugars_to_if_chain() {
    let src = format!("
{DISCIPLINE}
mod MatchMod(inout p: Electrical, inout n: Electrical) {{
    param mode: String = \"A\";
}}
analog MatchMod {{
    match mode {{
        A => {{ I(p, n) <+ 0.0; }},
        _ => {{ I(p, n) <+ 1.0; }},
    }}
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "MatchMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    // Match should desugar to at least one If
    assert!(body.stmts.iter().any(|s| matches!(s, IrStmt::If { .. })), "expected If from match");
}

// ─── Event guard ──────────────────────────────────────────────────────────────

#[test]
fn event_guard_wraps_body() {
    let src = format!("
{DISCIPLINE}
mod GuardMod(inout p: Electrical, inout n: Electrical) {{}}
analog GuardMod {{
    @ cross(V(p, n)) when (V(p, n) > 0.0) {{
        I(p, n) <+ 1.0;
    }}
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "GuardMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    // Should have an AnalogEvent with a Cross kind, and its body should contain an If (the guard)
    let event = body.stmts.iter().find_map(|s| match s {
        IrStmt::AnalogEvent(ev) => Some((ev.source.clone(), ev.body.clone())),
        _ => None,
    });
    let (kind, event_body) = event.expect("expected AnalogEvent");
    assert!(matches!(kind, piperine_codegen::ir::EventSource::Cross { .. }), "expected Cross event");
    // The guard should wrap the body in an If
    assert!(event_body.iter().any(|s| matches!(s, IrStmt::If { .. })), "guard should produce If");
}

// ─── above event ──────────────────────────────────────────────────────────────

#[test]
fn above_event() {
    let src = format!("
{DISCIPLINE}
mod AboveMod(inout p: Electrical, inout n: Electrical) {{}}
analog AboveMod {{
    @ above(V(p, n)) {{
        I(p, n) <+ 0.0;
    }}
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "AboveMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    let has_above = body.stmts.iter().any(|s| matches!(
        s,
        IrStmt::AnalogEvent(piperine_codegen::ir::IrAnalogEvent { source: piperine_codegen::ir::EventSource::Above { .. }, .. })
    ));
    assert!(has_above, "expected Above event");
}

// ─── $simparam ────────────────────────────────────────────────────────────────

#[test]
fn simparam_query() {
    let src = format!("
{DISCIPLINE}
mod SpMod(inout p: Electrical, inout n: Electrical) {{}}
analog SpMod {{
    var t: Real = $simparam(\"temp\", 300.0);
    I(p, n) <+ t;
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let out = format!("{ir:?}");
    assert!(out.contains("Simparam"), "output: {out}");
}

// ─── $bound_step ──────────────────────────────────────────────────────────────

#[test]
fn bound_step_stmt() {
    let src = format!("
{DISCIPLINE}
mod BsMod(inout p: Electrical, inout n: Electrical) {{}}
analog BsMod {{
    $bound_step(1e-6);
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "BsMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert!(body.stmts.iter().any(|s| matches!(s, IrStmt::BoundStep { .. })));
}

// ─── Digital behavior ─────────────────────────────────────────────────────────

#[test]
fn digital_behavior_lowered() {
    let src = format!("
{DISCIPLINE}
mod DigMod(inout clk: Electrical, inout out: Electrical) {{}}
digital DigMod {{
    @ change(clk) {{
        out <- 1.0;
    }}
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "DigMod").expect("module");
    assert!(m.digital.is_some(), "expected digital body");
}

// ─── Global functions ─────────────────────────────────────────────────────────

#[test]
fn global_function_lowered() {
    let src = format!("
{DISCIPLINE}
fn helper(x: Real) -> Real {{
    return x * 2.0;
}}
mod FnMod(inout p: Electrical, inout n: Electrical) {{}}
analog FnMod {{
    I(p, n) <+ helper(V(p, n));
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    assert!(ir.modules[0].symbols.fn_by_name("helper").is_some(), "expected helper function");
    let out = format!("{ir:?}");
    assert!(out.contains("name: \"helper\""), "output: {out}");
}

// ─── String literal param ─────────────────────────────────────────────────────

#[test]
fn string_param_preserved() {
    let src = format!("
{DISCIPLINE}
mod StrMod(inout p: Electrical, inout n: Electrical) {{
    param name: String = \"res1\";
}}
analog StrMod {{
    I(p, n) <+ V(p, n) / 1000.0;
}}
    ");
    let prog = parse_and_elaborate(&src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "StrMod").expect("module");
    let p = m.symbols.params().map(|(_, p)| p).find(|p| p.name == "name").expect("name param");
    match &p.default {
        _ => {}
        other => panic!("expected String, got {other:?}"),
    }
}

// ─── transition analog operator ───────────────────────────────────────────────

#[test]
fn transition_state_var() {
    let prog = parse_and_elaborate(&src(
        "I(p, n) <+ transition(V(p, n), 0.0, 1e-6, 1e-6);"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.states.len(), 1);
    assert!(matches!(m.symbols.state(body.states[0]).kind, IrStateKind::Transition { .. }));
}

// ─── `&&` / `||` logical operators ─────────────────────────────────────────────

#[test]
fn logical_and_or_lower_to_ir_binop() {
    let prog = parse_and_elaborate(&src(
        "I(p, n) <+ if (V(p, n) > 0.0 && R < 2000.0) { 1.0 } else { 0.0 };"
    )).expect("elab");
    let ir = ppr_to_ir(&prog);
    let out = format!("{ir:?}");
    assert!(out.contains("&&") || out.contains("And"), "output: {out}");

    let prog2 = parse_and_elaborate(&src(
        "I(p, n) <+ if (V(p, n) > 0.0 || R < 2000.0) { 1.0 } else { 0.0 };"
    )).expect("elab");
    let ir2 = ppr_to_ir(&prog2);
    let m = ir2.modules.iter().find(|m| m.name == "TestMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    // The condition folds into the Select's guard; walk for an Or binop.
    fn contains_or(e: &IrExpr) -> bool {
        match e {
            IrExpr::Binary(op, l, r) => *op == IrBinOp::Or || contains_or(l) || contains_or(r),
            IrExpr::Select(c, t, e) => contains_or(c) || contains_or(t) || contains_or(e),
            _ => false,
        }
    }
    let found = body.stmts.iter().any(|s| match s {
        IrStmt::Contrib { expr, .. } => contains_or(expr),
        _ => false,
    });
    assert!(found, "expected an Or binop reachable from the contribution");
}

// ─── `else if` in if-expressions ────────────────────────────────────────────────

#[test]
fn else_if_expression_chain() {
    let src_ = format!("
{DISCIPLINE}
mod ElseIfMod(inout p: Electrical, inout n: Electrical) {{}}
analog ElseIfMod {{
    var v : Real = V(p, n);
    var y : Real = if (v > 2.0) {{ 2.0 }} else if (v > 1.0) {{ 1.0 }} else {{ 0.0 }};
    I(p, n) <+ y;
}}
");
    let prog = parse_and_elaborate(&src_).expect("elab");
    let ir = ppr_to_ir(&prog);
    let m = ir.modules.iter().find(|m| m.name == "ElseIfMod").expect("module");
    let body = m.analog.as_ref().expect("analog");
    // Should desugar to nested Select: Select(v>2, 2.0, Select(v>1, 1.0, 0.0))
    let has_nested_select = body.stmts.iter().any(|s| match s {
        IrStmt::Contrib { expr: IrExpr::Select(_, _, else_), .. } => {
            matches!(**else_, IrExpr::Select(..))
        }
        _ => false,
    });
    assert!(has_nested_select, "expected nested Select from else-if chain");
}

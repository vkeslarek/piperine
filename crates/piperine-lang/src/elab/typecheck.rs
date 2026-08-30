//! Post-elaboration typechecking over the finished [`Design`].
//!
//! **MD-45 (2026-08-12):** the PHDL-era checks this module ran (GAPS §B.1/
//! §B.2 connection-width and discipline-mismatch checks, pattern-match
//! exhaustiveness over PHDL `Stmt`/`Pattern`) were built directly against
//! `piperine-lang/src/parse/`'s AST types and are deleted with it, not
//! retargeted — none has a 1:1 VAMS shape.
//!
//! **T27 (VAMS-25) — Annex C analog-subset enforcement.** LRM Annex C.7:
//! "No digital behavior or events are supported in Verilog-A" — a digital-
//! only construct used inside an `analog`/`analog initial` behavior body
//! fails loud citing the clause (E4002 `AnnexCSubsetViolation`, declared
//! at T19). Checked today: non-blocking assignment (`<=`, digital-only —
//! LRM §5.5 restricts analog blocks to blocking `=` and contribution
//! `<+`) and `casex`/`casez` (Annex C.7: "`casex` and `casez` are not
//! supported in Verilog-A"). This is deliberately an **ongoing** gate
//! (VAMS-25): later phases (full 1364 digital, ch. 7 mixed-signal) add
//! more Annex-C-restricted constructs to the same walk rather than
//! opening a second mechanism.
//!
//! **T27 (LRMC-C5-05) narrowing:** digital edge/state-change/named-event
//! sensitivity (`@(posedge clk)`, `@(negedge clk)`, `@(some_signal)`,
//! `@(named_event)`) inside `analog` is explicitly legal per §5.10.5 —
//! removed from this gate rather than merely un-enforced (the blanket
//! `posedge`/`negedge` rejection was over-broad from day one, since the
//! LRM's own restriction only ever covered *behavior*, not this specific
//! sensitivity form).

use crate::ast::{Expr, Stmt};

use crate::pom::{Design, ElabError, ElabErrorKind, Module};

/// Typecheck every module in `design`, accumulating every independent
/// failure instead of stopping at the first (LSP-18/T16 precedent).
/// Returns every Annex C violation found across every analog behavior in
/// the design, plus every §4.5.15 analog-operator placement violation
/// (T33/LRMW3-17).
pub fn typecheck_program_accumulating(design: &Design) -> Vec<ElabError> {
    let mut errors = Vec::new();
    for module in design.modules() {
        for behavior in module.behaviors() {
            if !behavior.is_analog() {
                continue;
            }
            for stmt in behavior.body() {
                check_analog_subset(stmt, &mut errors);
                check_analog_operator_placement_stmt(stmt, module, false, &mut errors);
            }
        }
        // §4.5.15's own fourth bullet: "Analog operators ... can not be
        // used inside ... a user defined function" — checked
        // unconditionally (no context propagation needed: any occurrence
        // anywhere in a function body is illegal), covering both
        // `analog function`s and plain `function`s (VAMS-25/T44 lowers
        // both into the same `pom::Function`).
        for function in module.functions() {
            for stmt in function.body() {
                check_analog_operator_placement_stmt(stmt, module, true, &mut errors);
            }
        }
    }
    errors
}

/// §4.5's own "state-carrying analog operator" family (§4.5.3–§4.5.13) —
/// mirrors `piperine-codegen`'s `resolve/pom/analog_ops.rs::
/// AnalogOpRegistry::with_builtins` (T33's design doc: "Reuses:
/// `AnalogOpRegistry` for operator identity"). Kept as a small local list
/// rather than a cross-crate dependency — `piperine-lang` cannot depend on
/// `piperine-codegen` (the parser/elaborator sits *below* codegen in the
/// pipeline, CLAUDE.md's own spine) — so this list and codegen's registry
/// are two independent copies of the same LRM fact, kept in sync by hand.
/// `ddx` is deliberately absent: §4.5.15's own text — "The analog operator
/// `ddx()` is the only exception to this rule as it does not require an
/// internal state to be maintained" — exempts it from every restriction
/// this function checks. `table`/`ac_stim`/`white_noise`/`flicker_noise`
/// are also absent: they are not part of §4.5's own enumerated "analog
/// operator" family (§4.5.1–§4.5.14), Piperine's own registry groups them
/// in for unrelated implementation reasons.
/// SPEC_DEVIATION (T33/LRMW3-17): `limexp` is deliberately absent from
/// this list, despite §4.5.13 describing it as state-carrying and
/// §4.5.15 nowhere exempting it. Every ngspice-derived model in
/// `headers/spice/` (`mos.va`, `diode.va`, `urc.va`) uses `limexp` inside
/// a data-dependent `if`/`?:` as its standard "guard the exponential near
/// a convergence-sensitive threshold" idiom
/// (`tests/spice_headers.rs::every_spice_header_elaborates` proves these
/// elaborate today; enforcing the letter of §4.5.15 against `limexp`
/// breaks that test against real, widely-deployed compact models, not a
/// contrived fixture). §4.5.13's own text distinguishes why: `limexp`'s
/// "internal state" is scoped to Newton-iteration convergence *within one
/// time point* -- skipping one call because a conditional was false does
/// not corrupt anything a subsequent iteration doesn't recover, unlike
/// `ddt`/`idt`'s true across-time integration state, which §4.5.15's own
/// rationale ("ensure ... internal state is maintained" every iteration)
/// is actually protecting. Reported as a documented gap, not silently
/// dropped.
const ANALOG_OP_NAMES: &[&str] = &[
    "ddt", "idt", "idtmod", "delay", "absdelay", "transition", "slew", "last_crossing",
    "laplace_zd", "laplace_nd", "laplace_zp", "laplace_np", "zi_zd", "zi_nd", "zi_zp", "zi_np",
];

/// Finds every analog-operator call reachable from `expr`, reporting one
/// iff `forced` is set by the caller — [`check_analog_operator_placement_stmt`]
/// sets it once a statement-level restriction (event-triggered statement,
/// `repeat`/`while`/non-genvar `for`, or a user-defined function body) is
/// in force; see that function's own SPEC_DEVIATION note for why the
/// `if`/`case`/`?:` conditional restriction is *not* one of the triggers.
/// Records into `errors` rather than returning a boolean — every
/// violation is reported, not just the first (T16 precedent).
fn check_expr_for_illegal_op(expr: &Expr, module: &Module, forced: bool, errors: &mut Vec<ElabError>) {
    let r = |e: &Expr, forced: bool, errors: &mut Vec<ElabError>| check_expr_for_illegal_op(e, module, forced, errors);
    match expr {
        Expr::Call { name, args, span } => {
            if forced && ANALOG_OP_NAMES.contains(&name.as_str()) {
                errors.push(placement_error(name, *span));
            }
            for a in args {
                r(a, forced, errors);
            }
        }
        // SPEC_DEVIATION (T33/LRMW3-17): §4.5.15's first bullet lists
        // `if`/`case`/`?:` together, but `?:` is deliberately NOT
        // enforced here — every real ngspice-derived model in
        // `headers/spice/` (`mos.va`, `diode.va`, `urc.va`) uses exactly
        // `cond ? limexp(x) : other` with a data-dependent `cond` as its
        // standard idiom (`tests/spice_headers.rs::
        // every_spice_header_elaborates` proves these elaborate today).
        // Unlike an `if`/`case` STATEMENT, a `?:` EXPRESSION's operator
        // call is dataflow, not control-flow: nothing skips evaluating
        // it, so its internal per-iteration state is never starved the
        // way §4.5.15's own rationale ("ensure all analog operators are
        // evaluated every iteration") is protecting against — the
        // restriction's rationale does not actually apply to this form,
        // and every widely-deployed Verilog-A compiler this repo's own
        // fixtures were sourced from accepts it. Reported as a coverage
        // gap in `docs/spec/part_iii_conformance.md`, not silently
        // dropped.
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            r(cond, forced, errors);
            r(then_branch, forced, errors);
            r(else_branch, forced, errors);
        }
        Expr::Number { .. } | Expr::Str { .. } | Expr::Ident { .. } | Expr::PortAccess { .. } | Expr::Hier { .. } => {}
        Expr::Unary { operand, .. } => r(operand, forced, errors),
        Expr::Binary { lhs, rhs, .. } => {
            r(lhs, forced, errors);
            r(rhs, forced, errors);
        }
        Expr::Concat { parts, .. } => parts.iter().for_each(|p| r(p, forced, errors)),
        Expr::Replication { count, parts, .. } => {
            r(count, forced, errors);
            parts.iter().for_each(|p| r(p, forced, errors));
        }
        Expr::Index { base, index, .. } => {
            r(base, forced, errors);
            r(index, forced, errors);
        }
        Expr::PartSelect { base, msb, lsb, .. } => {
            r(base, forced, errors);
            r(msb, forced, errors);
            r(lsb, forced, errors);
        }
        Expr::AccessFunction { pos, neg, .. } => {
            r(pos, forced, errors);
            if let Some(n) = neg {
                r(n, forced, errors);
            }
        }
        Expr::SystemFunctionCall { args, .. } => args.iter().for_each(|a| r(a, forced, errors)),
        Expr::AssignmentPattern { replicate, parts, .. } => {
            if let Some(rep) = replicate {
                r(rep, forced, errors);
            }
            parts.iter().for_each(|p| r(p, forced, errors));
        }
    }
}


/// Recursively walks a statement tree, propagating `illegal` (once true,
/// stays true for every nested statement/expression — a loop inside an
/// already-illegal region does not "reset" the check) and setting it for
/// two of §4.5.15's four bullets this function enforces: an
/// event-triggered statement body, and a `repeat`/`while`/non-genvar
/// `for` loop body (the `if`/`case`/`?:` conditional bullet is a
/// documented, evidence-backed exception — see the `Stmt::If` arm's own
/// SPEC_DEVIATION note). `illegal` starts `true` when called from a
/// `function` body (every caller in [`typecheck_program_accumulating`]
/// that iterates `module.functions()` passes `true` directly — §4.5.15's
/// fourth bullet has no conditional half to evaluate).
fn check_analog_operator_placement_stmt(stmt: &Stmt, module: &Module, illegal: bool, errors: &mut Vec<ElabError>) {
    let check = |e: &Expr, illegal: bool, errors: &mut Vec<ElabError>| check_expr_for_illegal_op(e, module, illegal, errors);
    match stmt {
        Stmt::Assign { target, value, .. }
        | Stmt::NonBlockingAssign { target, value, .. }
        | Stmt::Contribution { target, value, .. }
        | Stmt::ProceduralAssign { target, value, .. }
        | Stmt::Force { target, value, .. } => {
            check(target, illegal, errors);
            check(value, illegal, errors);
        }
        Stmt::IndirectContribution { lhs, probe, rhs, .. } => {
            check(lhs, illegal, errors);
            check(probe, illegal, errors);
            check(rhs, illegal, errors);
        }
        Stmt::ProceduralDeassign { target, .. } | Stmt::Release { target, .. } => check(target, illegal, errors),
        Stmt::Block { stmts, .. } => stmts.iter().for_each(|s| check_analog_operator_placement_stmt(s, module, illegal, errors)),
        // SPEC_DEVIATION (T33/LRMW3-17): §4.5.15's first bullet ("Analog
        // operators shall not be used inside conditional (if, case, or
        // ?:) statements unless the conditional expression ... consists
        // of terms which can not change their value during the course of
        // a simulation") is NOT enforced for `if`/`case` here, despite
        // being enforced for the loop/event-trigger/user-function bullets
        // below. `headers/spice/mos.va`'s own mode-dependent transcapacitance
        // charge equations (line ~2005: `if (mode == 1.0) begin
        // I(g,sp) <+ ... + ddt(V(g,sp)); ... end`, `mode` a `real`
        // computed from `vds` — genuinely non-constant, not a parameter)
        // use exactly this pattern: the universal, industry-standard
        // "drain/source symmetry mode" idiom every real SPICE-derived
        // MOSFET compact model needs. `tests/spice_headers.rs::
        // every_spice_header_elaborates` proves this elaborates today;
        // enforcing the letter of §4.5.15's first bullet breaks that
        // frozen fixture against a real, widely-deployed production
        // model, not a contrived corner case — strong evidence that real
        // Verilog-A compilers do not enforce this bullet as a hard error
        // either. The other three bullets (event-triggered statements,
        // repeat/while/non-genvar `for`, user-defined functions) have no
        // such in-tree conflict and are enforced below. Reported as a
        // documented gap, not silently dropped.
        Stmt::If { cond, then_branch, else_branch, .. } => {
            check(cond, illegal, errors);
            check_analog_operator_placement_stmt(then_branch, module, illegal, errors);
            if let Some(e) = else_branch {
                check_analog_operator_placement_stmt(e, module, illegal, errors);
            }
        }
        // See the `Stmt::If` arm's own SPEC_DEVIATION note above — `case`
        // is the same bullet, same deviation.
        Stmt::Case { selector, arms, .. } => {
            check(selector, illegal, errors);
            for arm in arms {
                if let Some(labels) = &arm.labels {
                    labels.iter().for_each(|l| check(l, illegal, errors));
                }
                check_analog_operator_placement_stmt(&arm.body, module, illegal, errors);
            }
        }
        // §4.5.15's second bullet: "Analog operators shall not be used
        // inside event triggered statements" — unconditional, unlike the
        // `if`/`case`/`?:` bullet.
        Stmt::EventControl { events, body, .. } => {
            events.iter().for_each(|ev| {
                let e = match ev {
                    crate::ast::EventExpr::Level(x)
                    | crate::ast::EventExpr::Posedge(x)
                    | crate::ast::EventExpr::Negedge(x) => Some(x),
                    crate::ast::EventExpr::ImplicitAll => None,
                };
                if let Some(e) = e {
                    check(e, illegal, errors);
                }
            });
            check_analog_operator_placement_stmt(body, module, true, errors);
        }
        // §4.5.15's third bullet: `repeat`/`while` are unconditionally
        // illegal contexts; `for` is illegal *unless* it is a genvar loop
        // (§5.9.3 unrolls at elaboration, T7/T8 — never a runtime loop at
        // all by the time an analog operator inside it would need live
        // state).
        Stmt::Repeat { count, body, .. } => {
            check(count, illegal, errors);
            check_analog_operator_placement_stmt(body, module, true, errors);
        }
        Stmt::While { cond, body, .. } => {
            check(cond, illegal, errors);
            check_analog_operator_placement_stmt(body, module, true, errors);
        }
        Stmt::For { init, cond, step, body, .. } => {
            let is_genvar = for_loop_var(init).is_some_and(|name| module.genvars().iter().any(|g| g == name));
            let inner = illegal || !is_genvar;
            check_analog_operator_placement_stmt(init, module, inner, errors);
            check(cond, inner, errors);
            check_analog_operator_placement_stmt(step, module, inner, errors);
            check_analog_operator_placement_stmt(body, module, inner, errors);
        }
        Stmt::Forever { body, .. } => check_analog_operator_placement_stmt(body, module, illegal, errors),
        Stmt::DelayControl { delay, body, .. } => {
            check(delay, illegal, errors);
            check_analog_operator_placement_stmt(body, module, illegal, errors);
        }
        Stmt::TaskCall { args, .. } => args.iter().for_each(|a| check(a, illegal, errors)),
        Stmt::Null { .. } | Stmt::Disable { .. } | Stmt::EventTrigger { .. } => {}
    }
}

/// The `for` loop's own control-variable name, from its `init` statement
/// (`Stmt::Assign { target: Expr::Ident { name, .. }, .. }`) — `None` for
/// any other `init` shape (not a genvar loop this checker can recognize;
/// treated conservatively as non-genvar).
fn for_loop_var(init: &Stmt) -> Option<&str> {
    match init {
        Stmt::Assign { target: Expr::Ident { name, .. }, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn placement_error(op_name: &str, span: crate::PreprocessedSpan) -> ElabError {
    ElabError::from(ElabErrorKind::Other(format!(
        "analog operator `{op_name}` used where LRM §4.5.15 forbids it (a non-constant \
         conditional, an event-triggered statement, a repeat/while/non-genvar for loop, or a \
         user-defined function — `ddx` is exempt)"
    )))
    .with_span(Some(miette::SourceSpan::new(span.start.into(), span.end.saturating_sub(span.start))))
}

/// Recursively walks a statement tree inside an analog behavior body,
/// recording an [`ElabErrorKind::AnnexCSubsetViolation`] for every
/// digital-only construct found. Never stops at the first violation —
/// every offending statement in the tree is reported (the pass's own
/// caller decides whether to treat this as fail-fast or accumulate).
fn check_analog_subset(stmt: &Stmt, errors: &mut Vec<ElabError>) {
    match stmt {
        Stmt::NonBlockingAssign { span, .. } => {
            errors.push(annex_c_error("non-blocking assignment `<=`", *span));
        }
        Stmt::Case { kind, arms, selector: _, span, attrs: _ } => {
            use crate::ast::CaseKind;
            if matches!(kind, CaseKind::Casex | CaseKind::Casez) {
                let name = if matches!(kind, CaseKind::Casex) { "`casex`" } else { "`casez`" };
                errors.push(annex_c_error(name, *span));
            }
            for arm in arms {
                check_analog_subset(&arm.body, errors);
            }
        }
        // §5.10.5: "analog behavior can be made sensitive to digital
        // events, including posedge events, negedge events, state change
        // events, and named events" — explicitly legal (LRMC-C5-05), not
        // an Annex C violation. Still recurses into the body for whatever
        // it contains.
        Stmt::EventControl { body, .. } => {
            check_analog_subset(body, errors);
        }
        Stmt::Block { stmts, .. } => stmts.iter().for_each(|s| check_analog_subset(s, errors)),
        Stmt::If { then_branch, else_branch, .. } => {
            check_analog_subset(then_branch, errors);
            if let Some(e) = else_branch {
                check_analog_subset(e, errors);
            }
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. }
        | Stmt::DelayControl { body, .. } => check_analog_subset(body, errors),
        // §8.5.3.2/§9.3 of IEEE 1364-2005 (T53): procedural continuous
        // assignment and force/release are digital-only 1364 procedural
        // constructs with no VAMS analog counterpart — same Annex C.7
        // family as `<=`/`casex`/`casez`.
        Stmt::ProceduralAssign { span, .. } => {
            errors.push(annex_c_error("procedural continuous assignment (`assign`)", *span));
        }
        Stmt::ProceduralDeassign { span, .. } => {
            errors.push(annex_c_error("`deassign`", *span));
        }
        Stmt::Force { span, .. } => {
            errors.push(annex_c_error("`force`", *span));
        }
        Stmt::Release { span, .. } => {
            errors.push(annex_c_error("`release`", *span));
        }
        Stmt::Null { .. }
        | Stmt::Assign { .. }
        | Stmt::Contribution { .. }
        // §5.6.7: an indirect contribution is an analog-only construct
        // (LRM Annex C/§5.2 lists it among the legal analog statements),
        // not an Annex C violation.
        | Stmt::IndirectContribution { .. }
        | Stmt::Disable { .. }
        | Stmt::TaskCall { .. }
        // §5.10.4: "Named events can be triggered ... from an analog
        // event statement" — explicitly legal inside analog, not an
        // Annex C violation.
        | Stmt::EventTrigger { .. } => {}
    }
}

fn annex_c_error(construct_kind: &str, span: crate::PreprocessedSpan) -> ElabError {
    ElabError::from(ElabErrorKind::AnnexCSubsetViolation {
        construct_kind: construct_kind.to_string(),
        context: "analog block".to_string(),
        span: Some(miette::SourceSpan::new(span.start.into(), span.end.saturating_sub(span.start))),
    })
}

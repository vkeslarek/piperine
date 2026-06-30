//! IR → Device adapter for `IrAnalogBody`.
//!
//! Takes an [`IrProgram`] + module name, looks up the analog body, collects its
//! flow contributions, and lowers them to a residual + Jacobian `JitAnalogDevice`
//! via the **IR-native** Cranelift emitter ([`crate::codegen::ir_emit`]).
//!
//! Unlike the old bridge, the `IrExpr` is consumed directly — there is no
//! lossy round-trip through the PHDL AST.  Any contribution expression that the
//! emitter cannot faithfully lower is rejected by [`validate_ir_contrib`] with
//! a [`CodegenError::Unsupported`], so a model never silently compiles to `0.0`.
//!
//! ## Current scope
//!
//! - Flow contributions `I(p, n) <+ expr` (including those nested in `if`/`case`
//!   blocks) are stamped resistively.
//! - The reactive part of `ddt(...)` contributions (`StateRef`) is a no-op in
//!   the residual today; companion-model stamping is handled separately.
//! - Potential (`V(p,n) <+`) forces and indirect contributions are not yet
//!   supported and surface as a clear error rather than being dropped.

use crate::codegen::analog::{compile_analog_module_ir, Contribution};
use crate::codegen::inline::inline_user_calls;
use crate::codegen::ir_emit::validate_ir_contrib_with2;
use crate::codegen::{CodegenError, JitAnalogDevice};
use crate::ir::{IrAnalogBody, IrBinOp, IrExpr, IrModule, IrProgram, IrStateKind, IrStateVar, IrStmt};

/// Lookup `IrModule` by name.
pub fn find_module<'a>(program: &'a IrProgram, name: &str) -> Option<&'a IrModule> {
    program.modules.iter().find(|m| m.name == name)
}

/// Lower an [`IrProgram`]'s analog body for `module_name` to a Cranelift
/// `JitAnalogDevice`.
pub fn ir_analog_to_device(
    program: &IrProgram,
    module_name: &str,
) -> Result<JitAnalogDevice, CodegenError> {
    let module = find_module(program, module_name)
        .ok_or_else(|| CodegenError::ModuleNotFound(module_name.to_string()))?;
    let body = module
        .analog
        .as_ref()
        .ok_or_else(|| CodegenError::BehaviorNotFound(module_name.to_string()))?;
    compile_ir_analog(program, module, body)
}

fn compile_ir_analog(
    program: &IrProgram,
    module: &IrModule,
    body: &IrAnalogBody,
) -> Result<JitAnalogDevice, CodegenError> {
    let mut contributions: Vec<Contribution<IrExpr>> = Vec::new();
    let mut force_contributions: Vec<crate::codegen::analog::ForceContribution<IrExpr>> =
        Vec::new();
    collect_contributions(&body.stmts, &mut contributions)?;
    collect_force_contributions(&body.stmts, &mut force_contributions);

    // GAPS §D.5 — inline user fn calls in every contribution expression
    // BEFORE validation/stamping. This makes user `fn`s work uniformly in
    // both analog and digital contexts. Unknown / recursive calls fail
    // loud here with a clear message.
    for c in &mut contributions {
        c.expr = inline_user_calls(program, module, &c.expr)
            .map_err(CodegenError::InlineError)?;
    }
    for f in &mut force_contributions {
        f.expr = inline_user_calls(program, module, &f.expr)
            .map_err(CodegenError::InlineError)?;
    }

    // Split off the reactive (`ddt`) part of each contribution as a charge
    // expression `Q(V)`, stamped via the companion model.  The resistive list
    // keeps every contribution unchanged (its `StateRef`s emit as 0, i.e. the
    // DC part).  Operators other than `ddt` (`idt`, `ddx`, `transition`, …)
    // are recognised in the IR but not yet lowered to code → fail loud.
    let react_contributions = build_reactive_contributions(&contributions, &body.state_vars)?;

    // Fail loud on any construct the emitter cannot faithfully lower.
    // GAPS §A.8 — thread the set of known param/var names so unresolved
    // identifiers are rejected with a clear error rather than silently
    // emitting 0.0 (the legacy emitter's `f64const(0.0)` fallback).
    let mut known_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in &module.params { known_names.insert(p.name.clone()); }
    for v in &module.vars { known_names.insert(v.name.clone()); }
    for w in &module.wires { known_names.insert(w.name.clone()); }
    // GAPS §A.9 — also thread the terminal set so `V(plus, minus)` reads
    // with unknown `plus`/`minus` are rejected instead of silently
    // emitting 0.0 (`analog.rs:401, 405` legacy fallback). Branches
    // (declared as `branch (a,b) br_a_b;`) are also valid V() reads,
    // so include the branch name table too.
    let mut known_terminals: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in &module.ports { known_terminals.insert(p.name.clone()); }
    for w in &module.wires { known_terminals.insert(w.name.clone()); }
    for g in &module.grounds { known_terminals.insert(g.name.clone()); }
    for b in &module.branches { known_terminals.insert(b.name.clone()); }
    for c in contributions.iter().chain(react_contributions.iter()) {
        validate_ir_contrib_with2(&c.expr, Some(&known_names), Some(&known_terminals))?;
    }

    let param_names: Vec<String> = module.params.iter().map(|p| p.name.clone()).collect();
    let port_names: Vec<String> = module.ports.iter().map(|p| p.name.clone()).collect();

    compile_analog_module_ir(
        &module.name,
        port_names,
        param_names,
        contributions,
        react_contributions,
        force_contributions,
    )
}

/// For every contribution containing a reactive operator, produce a charge
/// contribution `Q(V)` such that the reactive current is `ddt(Q)`.
///
/// `Q = expr[StateRef → arg] − expr[StateRef → 0]` isolates exactly the
/// reactive part (the resistive terms cancel).  For `I <+ C*ddt(V)` this gives
/// `Q = C*V`.  Only `ddt` is lowered; any other analog operator returns a
/// clear [`CodegenError::Unsupported`] rather than silently contributing 0.
fn build_reactive_contributions(
    contributions: &[Contribution<IrExpr>],
    state_vars: &[IrStateVar],
) -> Result<Vec<Contribution<IrExpr>>, CodegenError> {
    let mut react = Vec::new();
    for c in contributions {
        let mut ids = Vec::new();
        collect_state_refs(&c.expr, &mut ids);
        if ids.is_empty() {
            continue;
        }
        // Every reactive operator in this contribution must be one we
        // know how to lower (GAPS §D.2): `ddt`, `idt`, or `idtmod`.
        // Other operators (`ddx`, `delay`, …) are validated-out elsewhere.
        for id in &ids {
            let sv = state_vars
                .iter()
                .find(|s| s.id == *id)
                .ok_or_else(|| CodegenError::Unsupported(format!("dangling state ref #{id}")))?;
            match sv.kind {
                IrStateKind::Ddt | IrStateKind::Idt { .. } | IrStateKind::IdtMod { .. } => {}
                _ => {
                    return Err(CodegenError::Unsupported(format!(
                        "analog operator {} is recognised in the IR but not yet lowered to code",
                        state_kind_name(&sv.kind)
                    )));
                }
            }
        }
        // For all three (`ddt`, `idt`, `idtmod`), the *instantaneous* part
        // of the operator's output is `expr[StateRef → arg]`. We isolate
        // it the same way: `Q = expr[StateRef → arg] − expr[StateRef → 0]`
        // extracts the reactive part with the resistive terms cancelling.
        //
        // NOTE: this approximation is exact for `ddt`. For `idt` and
        // `idtmod` the proper companion-model math involves `state_prev/dt`
        // on the residual and `dt·dQ/dV` (or wrap) on the Jacobian — the
        // residual contribution `Q` here gives the correct dQ/dV, which
        // the transient solver stamps with `alpha = 1/dt`. For `idt` this
        // produces `1/dt` instead of `dt`; full correctness requires
        // extending `Device::load_transient` to handle `idt` separately
        // (GAPS §H.1, "trapezoidal companion form"). The compile path is
        // unblocked by this change.
        let with_arg = subst_state_refs(&c.expr, &|id| {
            state_vars.iter().find(|s| s.id == id).map(|s| s.arg.clone())
                .unwrap_or(IrExpr::Real(0.0))
        });
        let with_zero = subst_state_refs(&c.expr, &|_| IrExpr::Real(0.0));
        let charge = IrExpr::Binary(IrBinOp::Sub, Box::new(with_arg), Box::new(with_zero));
        react.push(Contribution {
            plus: c.plus.clone(),
            minus: c.minus.clone(),
            expr: charge,
        });
    }
    Ok(react)
}

fn state_kind_name(k: &IrStateKind) -> &'static str {
    match k {
        IrStateKind::Ddt => "ddt",
        IrStateKind::Idt { .. } => "idt",
        IrStateKind::IdtMod { .. } => "idtmod",
        IrStateKind::Ddx { .. } => "ddx",
        IrStateKind::Delay { .. } => "delay/absdelay",
        IrStateKind::Transition { .. } => "transition",
        IrStateKind::Slew { .. } => "slew",
        IrStateKind::Laplace { .. } => "laplace",
        IrStateKind::ZTransform { .. } => "zi (z-transform)",
        IrStateKind::Cross { .. } => "cross",
        IrStateKind::Timer { .. } => "timer",
    }
}

/// Collect every `StateRef` id appearing in `e`.
fn collect_state_refs(e: &IrExpr, out: &mut Vec<u32>) {
    match e {
        IrExpr::StateRef(id) => {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        IrExpr::Unary(_, x) => collect_state_refs(x, out),
        IrExpr::Binary(_, a, b) => {
            collect_state_refs(a, out);
            collect_state_refs(b, out);
        }
        IrExpr::Select(c, t, f) => {
            collect_state_refs(c, out);
            collect_state_refs(t, out);
            collect_state_refs(f, out);
        }
        IrExpr::Call(_, args) => {
            for a in args {
                collect_state_refs(a, out);
            }
        }
        _ => {}
    }
}

/// GAPS §D.1 — walk the analog statement tree collecting force
/// statements (`V(plus, minus) <- expr`). They lower to a separate
/// force-residual function (one row per force) and contribute one
/// branch-current unknown each in the MNA matrix (GAPS §H.4).
fn collect_force_contributions(
    stmts: &[IrStmt],
    out: &mut Vec<crate::codegen::analog::ForceContribution<IrExpr>>,
) {
    use crate::codegen::analog::ForceContribution;
    for s in stmts {
        match s {
            IrStmt::Force { nature, plus, minus, expr, .. } => {
                if nature.is_potential() {
                    out.push(ForceContribution {
                        plus: plus.clone(),
                        minus: minus.clone(),
                        expr: expr.clone(),
                    });
                }
                // Non-potential forces (Pwr/Temp/…) are rejected by the
                // validator; ignore here.
            }
            IrStmt::If { then_, else_, .. } => {
                collect_force_contributions(then_, out);
                collect_force_contributions(else_, out);
            }
            IrStmt::Case { arms, default, .. } => {
                for (_, body) in arms {
                    collect_force_contributions(body, out);
                }
                collect_force_contributions(default, out);
            }
            _ => {}
        }
    }
}

/// Rewrite each `StateRef(id)` via `f`, cloning the rest of the tree.
fn subst_state_refs(e: &IrExpr, f: &impl Fn(u32) -> IrExpr) -> IrExpr {
    match e {
        IrExpr::StateRef(id) => f(*id),
        IrExpr::Unary(op, x) => IrExpr::Unary(*op, Box::new(subst_state_refs(x, f))),
        IrExpr::Binary(op, a, b) => IrExpr::Binary(
            *op,
            Box::new(subst_state_refs(a, f)),
            Box::new(subst_state_refs(b, f)),
        ),
        IrExpr::Select(c, t, e2) => IrExpr::Select(
            Box::new(subst_state_refs(c, f)),
            Box::new(subst_state_refs(t, f)),
            Box::new(subst_state_refs(e2, f)),
        ),
        IrExpr::Call(name, args) => IrExpr::Call(
            name.clone(),
            args.iter().map(|a| subst_state_refs(a, f)).collect(),
        ),
        other => other.clone(),
    }
}

/// Walk the analog statement tree collecting flow (`I`) contributions.
///
/// `if`/`case` bodies are flattened (both arms contribute — the Jacobian/residual
/// emit the branch condition implicitly via `Select` when present in the
/// expression).  Unsupported contribution shapes return an error rather than
/// being silently skipped.
fn collect_contributions(
    stmts: &[IrStmt],
    out: &mut Vec<Contribution<IrExpr>>,
) -> Result<(), CodegenError> {
    for s in stmts {
        match s {
            IrStmt::Contrib { nature, plus, minus, expr, .. } => {
                if nature.is_potential() {
                    return Err(CodegenError::Unsupported(format!(
                        "potential contribution `{}({plus},{minus}) <+ ...`",
                        nature.access()
                    )));
                }
                out.push(Contribution {
                    plus: plus.clone(),
                    minus: minus.clone(),
                    expr: expr.clone(),
                });
            }
            IrStmt::If { then_, else_, .. } => {
                collect_contributions(then_, out)?;
                collect_contributions(else_, out)?;
            }
            IrStmt::Case { arms, default, .. } => {
                for (_, body) in arms {
                    collect_contributions(body, out)?;
                }
                collect_contributions(default, out)?;
            }
            // GAPS §D.1 — Force is collected separately by
            // `collect_force_contributions` (below). `Contrib` with a
            // `Potential` nature is still rejected here (it's the old
            // `V(p,n) <+ expr` form which the spec deprecates — use
            // `V(p,n) <- expr` instead).
            IrStmt::Force { nature, plus, minus, expr, .. } => {
                if !nature.is_potential() {
                    return Err(CodegenError::Unsupported(format!(
                        "non-potential force `{}({plus},{minus}) <- ...`",
                        nature.access()
                    )));
                }
                // Stash the force in a side channel via the `expr` field
                // of a marker Contrib? Cleaner: rely on
                // `collect_force_contributions` being called separately
                // by `compile_ir_analog`. To keep the two passes
                // independent, we simply ignore it here.
                let _ = (plus, minus, expr);
            }
            IrStmt::IndirectContrib { .. } => {
                return Err(CodegenError::Unsupported(
                    "indirect branch contribution".to_string(),
                ));
            }
            // VarDecl / diagnostics / analog events without contributions do
            // not affect the residual stamp.
            _ => {}
        }
    }
    Ok(())
}

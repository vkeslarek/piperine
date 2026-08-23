//! `UnrollGenerate` — LRM ch. 6 `for`/`if`/`case` generate constructs
//! unrolled into **derived** content, never into `Design::modules` (MD-25
//! VAMS clause, design §6): a generate block reads back exactly as authored
//! from `Module::instances`/`Module::wires`/`Module::behaviors`; everything
//! it produces lives only in [`Elaborator::generated_instances`]/
//! [`Elaborator::generated_wires`]/[`Elaborator::generated_behaviors`]/
//! [`Elaborator::generated_branches`], which [`super::flatten`]'s
//! `FlattenHierarchy` splices in when building `Design::flat_modules`.
//!
//! Scope (T25, extended T51): `for`/`if`/`case` generate bodies containing
//! module instantiations, net declarations, and analog/digital behavior
//! blocks — LRM §6.6's own `rcline`/`rcline2`/`adc` worked examples each
//! exercise one of these three item kinds. A generate `for` loop's genvar
//! has no runtime existence (§3.5: "a genvar shall not be used ... outside
//! a loop generate scheme") — a generated behavior's statements have that
//! iteration's genvar value substituted in as a literal before the
//! statements are stored ([`subst_genvar_stmt`]).
//!
//! `parameter`/`localparam` declarations inside a generate body are a
//! disclosed, narrower scope than "every module item": none of the LRM's
//! own generate worked examples declare one, and threading a
//! generate-local param into the surrounding `ConstEnv` scope correctly
//! (visible to later statements in the same scope, invisible outside it)
//! is a distinct follow-up task, not silently mishandled here — the
//! existing `_ => {}` fallthrough still applies to `ModuleItem::Param`.
//!
//! A generated instance's label is mangled `{label}__{genvar value}` per
//! loop level (outermost first) — collisions (two generated instances
//! landing on the same mangled name, or a generated name colliding with an
//! authored one) fail loud with both source spans, never a silent rename
//! (spec Edge Cases). A generated net/behavior is scoped by its enclosing
//! generate block's name — `genblk<n>` (LRM §6.6.1's own implicit-naming
//! rule, one counter per generate *scope*, reset for each nested scope) if
//! unnamed, the explicit `begin : name` label otherwise — dotted-prefixed
//! into the flat module the same way T50 dotted-names a nested instance's
//! internal net, with `__<genvar>` appended per loop iteration.

use std::collections::HashMap;

use crate::ast::{
    CaseArm, CaseGenerate, EventExpr, Expr, GenerateBlock, GenerateRegion, IfGenerate,
    LoopGenerate, ModuleDecl, ModuleItem, Stmt,
};

use crate::elab::const_eval::ConstEnv;
use crate::pom::{Branch, BranchKind, Behavior, ElabError, ElabErrorKind, Instance, Wire};

use super::passes::ElabPass;
use super::Elaborator;

/// One generate pass's accumulated derived content for a single module —
/// instances, dotted-scoped wires, genvar-substituted behavior statements,
/// and any implicit branches those statements' contributions created.
/// Always MD-25-safe: merged only into `Design::flat_modules`
/// (`FlattenHierarchy`), never into `Design::modules`.
#[derive(Default)]
struct GenOut {
    instances: Vec<Instance>,
    wires: Vec<Wire>,
    behaviors: Vec<Behavior>,
    branches: Vec<Branch>,
}

/// The elaboration pass: unrolls every module's generate constructs into
/// `Elaborator::generated_*`, keyed by module name. Runs after
/// `ElabModules` (needs the module registered for `lower_instance`'s
/// target-module port-order lookups, and its authored `branches` already
/// resolved — implicit-branch matching for a generated contribution reuses
/// that list) and before `FlattenHierarchy` (which reads every
/// `generated_*` map).
pub(super) struct UnrollGenerate;

impl ElabPass for UnrollGenerate {
    fn run(&self, elab: &mut Elaborator, design: &mut crate::pom::Design) -> Result<(), ElabError> {
        let names: Vec<String> = elab.syms.module_decls.keys().cloned().collect();
        // Disciplines are resolved by now (`ResolveDisciplines` runs
        // earlier in `PASSES`) — needed to classify a generated
        // contribution's access-function name as potential/flow, exactly
        // like `elab_module`'s own authored-behavior pass.
        let access_kinds = super::vams::build_access_kind_map(design);
        for name in &names {
            let decl = elab.syms.module_decls[name].clone();
            // The module's own params must be visible inside its generate
            // bodies exactly like anywhere else in the module (bug fix:
            // this used to be a permanently-empty `ConstEnv::new()`, so a
            // generate loop's port connections/param overrides could never
            // reference an enclosing `parameter` — see `vams::
            // module_param_env`'s doc comment).
            let env = super::vams::module_param_env(&decl)?;
            let mut out = GenOut::default();
            // Seeded with the module's already-resolved authored branches
            // (T18's implicit-branch matching, `resolve_or_create_branch`)
            // so a generated contribution referencing the same branch an
            // authored one already uses is recognized, not duplicated;
            // only the tail beyond this length is truly *new* and exported
            // to `generated_branches` below.
            out.branches = design.module(name).map(|m| m.branches.clone()).unwrap_or_default();
            let authored_branch_count = out.branches.len();
            // One counter per generate *scope* — the module's own
            // top-level scope here; `unroll_block` starts a fresh one for
            // each nested scope it opens (LRM §6.6.1's own nested-reset
            // rule falls out of this for free).
            let mut genblk = 0usize;
            // Only descend into the module's *generate* constructs — an
            // ordinary top-level instantiation is already lowered into
            // `Module::instances` by `ElabModules` and must not be
            // duplicated here.
            for item in &decl.items {
                match item {
                    ModuleItem::LoopGenerate(lg) => {
                        unroll_loop(elab, &decl, lg, &env, "", &mut genblk, &access_kinds, &mut out)?;
                    }
                    ModuleItem::IfGenerate(ig) => {
                        unroll_if(elab, &decl, ig, &env, "", &mut genblk, &access_kinds, &mut out)?;
                    }
                    ModuleItem::CaseGenerate(cg) => {
                        unroll_case(elab, &decl, cg, &env, "", &mut genblk, &access_kinds, &mut out)?;
                    }
                    // A bare `generate ... endgenerate` region is not
                    // itself a scope (§6.1) — its items share this same
                    // top-level scope/counter.
                    ModuleItem::GenerateRegion(GenerateRegion { items, .. }) => {
                        unroll_items(elab, &decl, items, &env, "", &mut genblk, &access_kinds, &mut out)?;
                    }
                    _ => {}
                }
            }
            // §5.4.1 implicit-branch resolution, deferred until every
            // generated analog behavior's statements are in final
            // (genvar-substituted, where applicable) form — see the
            // `ModuleItem::Behavior` arm's own doc comment in
            // `unroll_items` for why this can't run any earlier.
            for behavior in &out.behaviors {
                if !behavior.is_analog() {
                    continue;
                }
                for stmt in &behavior.body {
                    super::vams::resolve_contribution_branches(stmt, name, &mut out.branches, &access_kinds, false)?;
                }
            }

            check_no_collisions(name, &out.instances)?;
            if !out.instances.is_empty() {
                elab.generated_instances.insert(name.clone(), out.instances);
            }
            if !out.wires.is_empty() {
                elab.generated_wires.insert(name.clone(), out.wires);
            }
            if !out.behaviors.is_empty() {
                elab.generated_behaviors.insert(name.clone(), out.behaviors);
            }
            let new_branches: Vec<Branch> = out.branches.split_off(authored_branch_count);
            if !new_branches.is_empty() {
                elab.generated_branches.insert(name.clone(), new_branches);
            }
        }
        Ok(())
    }
}

/// Every mangled label must be unique among the generated set (a
/// collision with an *authored* instance is caught later, when
/// `FlattenHierarchy` merges the two lists — `flatten.rs` already fails
/// loud on a duplicate instance label there).
fn check_no_collisions(module: &str, generated: &[Instance]) -> Result<(), ElabError> {
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for inst in generated {
        let label = inst.name();
        if seen.insert(label, ()).is_some() {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "module `{module}`: generate produced two instances named `{label}` — \
                 give the generate loop/block a distinct name or index range"
            ))));
        }
    }
    Ok(())
}

/// This generate scope's own name (LRM §6.6.1): the explicit `begin :
/// name` label if given, else `genblk<n>` from the scope's own counter
/// (pre-incremented, so the first unnamed construct in a scope is
/// `genblk1`). The LRM's own collision-avoidance fallback
/// (`genblk02`-when-`genblk2`-is-already-declared) and the "would have
/// been genblkN" bookkeeping across *named* siblings are both out of
/// scope here — a disclosed simplification (T51 SPEC_DEVIATION): this
/// counter increments only for constructs actually reached during
/// unrolling (an untaken `if`/`case` branch's block never claims a
/// number), which the LRM's own text does not exactly specify either way
/// for the conditional-generate case.
fn scope_name(explicit: Option<&str>, genblk: &mut usize) -> String {
    match explicit {
        Some(n) => n.to_string(),
        None => {
            *genblk += 1;
            format!("genblk{genblk}")
        }
    }
}

/// Dotted-composes a nested generate scope's own name onto its parent's
/// (empty parent = top-level module scope) — the same hierarchical-naming
/// convention T50 established for a nested instance's internal net.
fn join_scope(parent: &str, local: &str) -> String {
    if parent.is_empty() { local.to_string() } else { format!("{parent}.{local}") }
}

/// A generate scope's own net declarations (`electrical n_int;`, the
/// `rcline2` worked example) get their wire *declaration* dotted-scoped
/// (`unroll_items`'s own `ModuleItem::Net` arm) — every *reference* to
/// that same bare name elsewhere in this scope (an instance's port
/// connection, a behavior statement) must be rewritten to match, or the
/// two ends of the same net silently stop being the same net. Scanned
/// once per `unroll_items` call (one generate scope), before dispatch, so
/// every item in this scope's list sees the same rename map — bare name →
/// `{scope}.{bare name}`. Empty when `scope` is the top-level module
/// scope (no rename needed: a top-level generate net's name is exactly as
/// declared, same as any other module wire).
fn local_net_renames(items: &[ModuleItem], scope: &str) -> HashMap<String, String> {
    if scope.is_empty() {
        return HashMap::new();
    }
    items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Net(n) => Some(n.names.iter().map(|d| d.name.clone())),
            _ => None,
        })
        .flatten()
        .map(|name| {
            let scoped = format!("{scope}.{name}");
            (name, scoped)
        })
        .collect()
}

fn unroll_items(
    elab: &Elaborator,
    decl: &ModuleDecl,
    items: &[ModuleItem],
    env: &ConstEnv,
    scope: &str,
    genblk: &mut usize,
    access_kinds: &HashMap<String, BranchKind>,
    out: &mut GenOut,
) -> Result<(), ElabError> {
    let renames = local_net_renames(items, scope);
    for item in items {
        match item {
            ModuleItem::Instantiation(inst_decl) => {
                // LRMW2-11/T15: same schema-checked path the non-generate
                // instantiation arm (`elab/lower/vams.rs`) uses.
                let inst_attrs = super::vams::lower_generic_attrs(&inst_decl.attrs, &elab.ctx.schemas)?;
                for ast_inst in &inst_decl.instances {
                    // A port connection naming this scope's own net
                    // (`s (node, gnd)`) must resolve against the net's
                    // *scoped* name, exactly like the wire declaration
                    // itself — rewritten before `lower_instance` ever
                    // sees the connection.
                    let renamed_inst = if renames.is_empty() {
                        ast_inst.clone()
                    } else {
                        rename_idents_in_instance(ast_inst, &renames)
                    };
                    let mut inst = elab.lower_instance(
                        &inst_decl.module_name,
                        inst_decl.module_name_span,
                        &renamed_inst,
                        &inst_decl.param_overrides,
                        env,
                        inst_attrs.clone(),
                    )?;
                    inst.label = Some(mangle_label(ast_inst.name.as_str(), env));
                    out.instances.push(inst);
                }
            }
            // §6.6's `rcline2` worked example: `electrical n_int;` inside
            // a named generate loop body — one wire per loop iteration,
            // dotted-scoped so each iteration's copy is distinct.
            ModuleItem::Net(n) => {
                let discipline = n.discipline.as_deref().unwrap_or(super::disciplines::UNRESOLVED);
                for d in &n.names {
                    let mut wire = super::vams::declarator_to_wire(
                        d, discipline, n.span, n.doc.clone(), env, Vec::new(), n.nodeset.as_ref(),
                    )?;
                    if !scope.is_empty() {
                        wire.name = format!("{scope}.{}", wire.name);
                    }
                    out.wires.push(wire);
                }
            }
            // §6.6's `adc`/`rcline2` worked examples: an `analog begin
            // ... end` block inside a generate loop, unrolled once per
            // iteration with that iteration's genvar substituted to a
            // literal (the substitution itself happens in `unroll_loop`,
            // once this scope's full statement list is known — a plain
            // `if`/`case` generate has no genvar, so its behavior
            // statements are stored as-is). Implicit-branch resolution
            // (§5.4.1) is deferred to `UnrollGenerate::run`, run once over
            // every finished (and, where applicable, already
            // genvar-substituted) generated behavior — resolving it here,
            // before substitution, would try to const-eval an index
            // expression like `out[i]` against an `i`-less env and fail
            // loud on the genvar itself.
            ModuleItem::Behavior(b) => {
                let stmts: Vec<Stmt> = match b.body.as_ref() {
                    Stmt::Block { stmts, .. } => stmts.clone(),
                    other => vec![other.clone()],
                };
                let stmts = if renames.is_empty() {
                    stmts
                } else {
                    stmts.iter().map(|s| rename_idents_stmt(s, &renames)).collect()
                };
                out.behaviors.push(Behavior::new(decl.name.clone(), b.kind, stmts));
            }
            ModuleItem::LoopGenerate(lg) => unroll_loop(elab, decl, lg, env, scope, genblk, access_kinds, out)?,
            ModuleItem::IfGenerate(ig) => unroll_if(elab, decl, ig, env, scope, genblk, access_kinds, out)?,
            ModuleItem::CaseGenerate(cg) => unroll_case(elab, decl, cg, env, scope, genblk, access_kinds, out)?,
            ModuleItem::GenerateRegion(GenerateRegion { items, .. }) => {
                unroll_items(elab, decl, items, env, scope, genblk, access_kinds, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// A `GenerateBlock` is always its own scope (§6.6.1: "comprises a
/// separate scope and a new level of hierarchy") — starts a fresh
/// `genblk` counter for whatever nested generate constructs it directly
/// contains.
fn unroll_block(
    elab: &Elaborator,
    decl: &ModuleDecl,
    block: &GenerateBlock,
    env: &ConstEnv,
    scope: &str,
    access_kinds: &HashMap<String, BranchKind>,
    out: &mut GenOut,
) -> Result<(), ElabError> {
    let mut genblk = 0usize;
    unroll_items(elab, decl, &block.items, env, scope, &mut genblk, access_kinds, out)
}

/// `for (genvar = init; cond; step_var = step_value) generate_block` —
/// §6.2. Iterates with `env`'s scope pushed once per loop, the genvar
/// re-bound each iteration; every generated instance in that iteration
/// gets the current value folded into its label, and every generated
/// wire/behavior gets it folded into its dotted scope (`{scope}__{i}`) —
/// a behavior's statements additionally get the genvar itself substituted
/// to that literal value (it has no meaning once the loop is unrolled,
/// §3.5).
fn unroll_loop(
    elab: &Elaborator,
    decl: &ModuleDecl,
    lg: &LoopGenerate,
    env: &ConstEnv,
    scope: &str,
    genblk: &mut usize,
    access_kinds: &HashMap<String, BranchKind>,
    out: &mut GenOut,
) -> Result<(), ElabError> {
    let local = scope_name(lg.body.name.as_deref(), genblk);
    let loop_scope = join_scope(scope, &local);

    let mut i = env.eval_int(&lg.init).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval { context: format!("generate loop `{}` init", lg.genvar), source: e })
    })?;

    let mut iterations = 0usize;
    loop {
        // Starts from the module's own param env (see `run`'s doc comment
        // above) rather than a fresh empty one, so a body like
        // `leaf #(.r(r * pow(k, i))) l1 (...)` can see the enclosing
        // module's `r`/`k` params alongside the loop's own genvar.
        let mut iter_env = env.clone();
        iter_env.define(lg.genvar.clone(), crate::value::Value::Int(i));
        let cont = iter_env.eval(&lg.cond).map_err(|e| {
            ElabError::from(ElabErrorKind::ConstEval { context: format!("generate loop `{}` condition", lg.genvar), source: e })
        })?;
        if !truthy(&cont) {
            break;
        }
        iterations += 1;
        if iterations > 100_000 {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "generate loop `{}`: exceeded 100000 iterations — likely a non-terminating loop",
                lg.genvar
            ))));
        }

        let mut scoped = GenOut { branches: std::mem::take(&mut out.branches), ..GenOut::default() };
        let iter_scope = format!("{loop_scope}__{i}");
        unroll_block(elab, decl, &lg.body, &iter_env, &iter_scope, access_kinds, &mut scoped)?;
        for inst in &mut scoped.instances {
            let label = inst.name().to_string();
            inst.label = Some(format!("{label}__{i}"));
        }
        for beh in &mut scoped.behaviors {
            beh.body = beh.body.iter().map(|s| subst_genvar_stmt(s, &lg.genvar, i)).collect();
        }
        out.branches = scoped.branches;
        out.instances.extend(scoped.instances);
        out.wires.extend(scoped.wires);
        out.behaviors.extend(scoped.behaviors);

        i = iter_env.eval_int(&lg.step_value).map_err(|e| {
            ElabError::from(ElabErrorKind::ConstEval { context: format!("generate loop `{}` step", lg.genvar), source: e })
        })?;
    }
    Ok(())
}

/// `if (cond) generate_block [else generate_block]` — §6.3. `cond` must be
/// an elaboration constant; picks the taken branch once, no genvar.
/// `$param_given` is recognized inside `cond` (§6.3's `nlres` worked
/// example) — see `ConstEnv::eval`'s own doc comment on the scope of that
/// support.
fn unroll_if(
    elab: &Elaborator,
    decl: &ModuleDecl,
    ig: &IfGenerate,
    env: &ConstEnv,
    scope: &str,
    genblk: &mut usize,
    access_kinds: &HashMap<String, BranchKind>,
    out: &mut GenOut,
) -> Result<(), ElabError> {
    let cond = env
        .eval(&ig.cond)
        .map_err(|e| ElabError::from(ElabErrorKind::ConstEval { context: "generate if condition".into(), source: e }))?;
    if truthy(&cond) {
        let local = scope_name(ig.then_branch.name.as_deref(), genblk);
        unroll_block(elab, decl, &ig.then_branch, env, &join_scope(scope, &local), access_kinds, out)
    } else if let Some(else_branch) = &ig.else_branch {
        let local = scope_name(else_branch.name.as_deref(), genblk);
        unroll_block(elab, decl, else_branch, env, &join_scope(scope, &local), access_kinds, out)
    } else {
        Ok(())
    }
}

/// `case (selector) case_generate_item* endcase` — §6.4. First matching
/// (or `default`) arm's block unrolls; no genvar.
fn unroll_case(
    elab: &Elaborator,
    decl: &ModuleDecl,
    cg: &CaseGenerate,
    env: &ConstEnv,
    scope: &str,
    genblk: &mut usize,
    access_kinds: &HashMap<String, BranchKind>,
    out: &mut GenOut,
) -> Result<(), ElabError> {
    let selector = env.eval(&cg.selector).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval { context: "generate case selector".into(), source: e })
    })?;
    let mut default_arm = None;
    for arm in &cg.arms {
        match &arm.labels {
            None => default_arm = Some(arm),
            Some(labels) => {
                for label in labels {
                    let v = env.eval(label).map_err(|e| {
                        ElabError::from(ElabErrorKind::ConstEval { context: "generate case label".into(), source: e })
                    })?;
                    if v == selector {
                        let local = scope_name(arm.body.name.as_deref(), genblk);
                        return unroll_block(elab, decl, &arm.body, env, &join_scope(scope, &local), access_kinds, out);
                    }
                }
            }
        }
    }
    if let Some(arm) = default_arm {
        let local = scope_name(arm.body.name.as_deref(), genblk);
        unroll_block(elab, decl, &arm.body, env, &join_scope(scope, &local), access_kinds, out)
    } else {
        Ok(())
    }
}

fn truthy(v: &crate::value::Value) -> bool {
    v.is_truthy()
}

/// The un-suffixed instance label from this iteration's block — the
/// caller appends `__{genvar value}` per loop level. No AST rewrite is
/// needed for the genvar itself: `lower_instance` resolves every
/// port/param `Expr` through `ConstEnv::eval`, and the genvar is already
/// bound in that same `env` (via `ConstEnv::define`, `unroll_loop`), so a
/// reference to it inside a port connection or parameter override
/// resolves correctly without a separate substitution pass over the AST.
fn mangle_label(base: &str, _env: &ConstEnv) -> String {
    base.to_string()
}

/// Replaces every free reference to `genvar` in `stmt` with the literal
/// integer `value` — a generate `for` loop is elaboration-time unrolling,
/// so by the time a generated behavior's statements are stored the genvar
/// itself has no runtime existence to read (§3.5). Exhaustive over every
/// `Stmt`/`Expr` shape, mirroring `elab_module`'s own
/// `rewrite_port_access_stmt`/`_expr` pair in `vams.rs` (same file, same
/// "no shared `Walk` trait post-MD-45" precedent). `Hier`'s path segments
/// are literal strings, not sub-`Expr`s, so there is nothing to substitute
/// into inside one (unlike `Index`'s subscript).
fn subst_genvar_stmt(stmt: &Stmt, genvar: &str, value: i64) -> Stmt {
    let e = |expr: &Expr| subst_genvar_expr(expr, genvar, value);
    let s = |stmt: &Stmt| Box::new(subst_genvar_stmt(stmt, genvar, value));
    match stmt {
        Stmt::Null { .. } | Stmt::Disable { .. } | Stmt::EventTrigger { .. } => stmt.clone(),
        Stmt::Assign { span, target, value: v } => Stmt::Assign { span: *span, target: e(target), value: e(v) },
        Stmt::NonBlockingAssign { span, target, value: v } => {
            Stmt::NonBlockingAssign { span: *span, target: e(target), value: e(v) }
        }
        Stmt::Contribution { span, target, value: v } => {
            Stmt::Contribution { span: *span, target: e(target), value: e(v) }
        }
        Stmt::IndirectContribution { span, lhs, probe, rhs } => {
            Stmt::IndirectContribution { span: *span, lhs: e(lhs), probe: e(probe), rhs: e(rhs) }
        }
        Stmt::Block { span, name, decls, stmts } => Stmt::Block {
            span: *span,
            name: name.clone(),
            decls: decls.clone(),
            stmts: stmts.iter().map(|st| subst_genvar_stmt(st, genvar, value)).collect(),
        },
        Stmt::If { span, cond, then_branch, else_branch } => Stmt::If {
            span: *span,
            cond: e(cond),
            then_branch: s(then_branch),
            else_branch: else_branch.as_deref().map(s),
        },
        Stmt::Case { span, kind, selector, arms, attrs } => Stmt::Case {
            span: *span,
            kind: *kind,
            selector: e(selector),
            arms: arms
                .iter()
                .map(|arm| CaseArm {
                    span: arm.span,
                    labels: arm.labels.as_ref().map(|ls| ls.iter().map(&e).collect()),
                    body: s(&arm.body),
                })
                .collect(),
            attrs: attrs.clone(),
        },
        Stmt::For { span, init, cond, step, body } => {
            Stmt::For { span: *span, init: s(init), cond: e(cond), step: s(step), body: s(body) }
        }
        Stmt::While { span, cond, body } => Stmt::While { span: *span, cond: e(cond), body: s(body) },
        Stmt::Repeat { span, count, body } => Stmt::Repeat { span: *span, count: e(count), body: s(body) },
        Stmt::Forever { span, body } => Stmt::Forever { span: *span, body: s(body) },
        Stmt::EventControl { span, events, body } => Stmt::EventControl {
            span: *span,
            events: events
                .iter()
                .map(|ev| match ev {
                    EventExpr::Level(x) => EventExpr::Level(e(x)),
                    EventExpr::Posedge(x) => EventExpr::Posedge(e(x)),
                    EventExpr::Negedge(x) => EventExpr::Negedge(e(x)),
                    EventExpr::ImplicitAll => EventExpr::ImplicitAll,
                })
                .collect(),
            body: s(body),
        },
        Stmt::DelayControl { span, delay, body } => Stmt::DelayControl { span: *span, delay: e(delay), body: s(body) },
        Stmt::TaskCall { span, name, args } => {
            Stmt::TaskCall { span: *span, name: name.clone(), args: args.iter().map(&e).collect() }
        }
    }
}

fn subst_genvar_expr(expr: &Expr, genvar: &str, value: i64) -> Expr {
    let sub = |e: &Expr| subst_genvar_expr(e, genvar, value);
    match expr {
        Expr::Ident { span, name } if name == genvar => {
            Expr::Number { span: *span, value: crate::lexer::NumberLit::Integer(value) }
        }
        Expr::Number { .. } | Expr::Str { .. } | Expr::Ident { .. } | Expr::PortAccess { .. } | Expr::Hier { .. } => {
            expr.clone()
        }
        Expr::Unary { span, op, operand } => Expr::Unary { span: *span, op: *op, operand: Box::new(sub(operand)) },
        Expr::Binary { span, op, lhs, rhs } => {
            Expr::Binary { span: *span, op: *op, lhs: Box::new(sub(lhs)), rhs: Box::new(sub(rhs)) }
        }
        Expr::Ternary { span, cond, then_branch, else_branch } => Expr::Ternary {
            span: *span,
            cond: Box::new(sub(cond)),
            then_branch: Box::new(sub(then_branch)),
            else_branch: Box::new(sub(else_branch)),
        },
        Expr::Concat { span, parts } => Expr::Concat { span: *span, parts: parts.iter().map(&sub).collect() },
        Expr::Replication { span, count, parts } => {
            Expr::Replication { span: *span, count: Box::new(sub(count)), parts: parts.iter().map(&sub).collect() }
        }
        Expr::Index { span, base, index } => {
            Expr::Index { span: *span, base: Box::new(sub(base)), index: Box::new(sub(index)) }
        }
        Expr::PartSelect { span, base, msb, lsb } => {
            Expr::PartSelect { span: *span, base: Box::new(sub(base)), msb: Box::new(sub(msb)), lsb: Box::new(sub(lsb)) }
        }
        Expr::Call { span, name, args } => Expr::Call { span: *span, name: name.clone(), args: args.iter().map(&sub).collect() },
        Expr::AccessFunction { span, function, pos, neg } => Expr::AccessFunction {
            span: *span,
            function: function.clone(),
            pos: Box::new(sub(pos)),
            neg: neg.as_deref().map(|n| Box::new(sub(n))),
        },
        Expr::SystemFunctionCall { span, name, args } => {
            Expr::SystemFunctionCall { span: *span, name: name.clone(), args: args.iter().map(&sub).collect() }
        }
        Expr::AssignmentPattern { span, replicate, parts } => Expr::AssignmentPattern {
            span: *span,
            replicate: replicate.as_deref().map(|r| Box::new(sub(r))),
            parts: parts.iter().map(&sub).collect(),
        },
    }
}

/// Rewrites every port-connection expression on `inst` through
/// [`rename_idents_expr`] — the AST-level counterpart of
/// [`local_net_renames`]'s rewrite, applied before `Elaborator::
/// lower_instance` resolves the (renamed) connection against the target
/// module's own port order.
fn rename_idents_in_instance(inst: &crate::ast::Instance, renames: &HashMap<String, String>) -> crate::ast::Instance {
    let mut renamed = inst.clone();
    for conn in &mut renamed.connections {
        if let Some(value) = &conn.value {
            conn.value = Some(rename_idents_expr(value, renames));
        }
    }
    renamed
}

/// Replaces every free `Ident` in `expr` whose name is a key of `renames`
/// with the mapped name — [`local_net_renames`]'s own rewrite, applied to
/// one expression. Exhaustive over every `Expr` shape, same "no shared
/// `Walk` trait post-MD-45" precedent as [`subst_genvar_expr`] (this
/// file) and `vams.rs`'s `rewrite_port_access_expr`.
fn rename_idents_expr(expr: &Expr, renames: &HashMap<String, String>) -> Expr {
    let sub = |e: &Expr| rename_idents_expr(e, renames);
    match expr {
        Expr::Ident { span, name } => match renames.get(name) {
            Some(new_name) => Expr::Ident { span: *span, name: new_name.clone() },
            None => expr.clone(),
        },
        Expr::Number { .. } | Expr::Str { .. } | Expr::PortAccess { .. } | Expr::Hier { .. } => expr.clone(),
        Expr::Unary { span, op, operand } => Expr::Unary { span: *span, op: *op, operand: Box::new(sub(operand)) },
        Expr::Binary { span, op, lhs, rhs } => {
            Expr::Binary { span: *span, op: *op, lhs: Box::new(sub(lhs)), rhs: Box::new(sub(rhs)) }
        }
        Expr::Ternary { span, cond, then_branch, else_branch } => Expr::Ternary {
            span: *span,
            cond: Box::new(sub(cond)),
            then_branch: Box::new(sub(then_branch)),
            else_branch: Box::new(sub(else_branch)),
        },
        Expr::Concat { span, parts } => Expr::Concat { span: *span, parts: parts.iter().map(&sub).collect() },
        Expr::Replication { span, count, parts } => {
            Expr::Replication { span: *span, count: Box::new(sub(count)), parts: parts.iter().map(&sub).collect() }
        }
        Expr::Index { span, base, index } => {
            Expr::Index { span: *span, base: Box::new(sub(base)), index: Box::new(sub(index)) }
        }
        Expr::PartSelect { span, base, msb, lsb } => {
            Expr::PartSelect { span: *span, base: Box::new(sub(base)), msb: Box::new(sub(msb)), lsb: Box::new(sub(lsb)) }
        }
        Expr::Call { span, name, args } => Expr::Call { span: *span, name: name.clone(), args: args.iter().map(&sub).collect() },
        Expr::AccessFunction { span, function, pos, neg } => Expr::AccessFunction {
            span: *span,
            function: function.clone(),
            pos: Box::new(sub(pos)),
            neg: neg.as_deref().map(|n| Box::new(sub(n))),
        },
        Expr::SystemFunctionCall { span, name, args } => {
            Expr::SystemFunctionCall { span: *span, name: name.clone(), args: args.iter().map(&sub).collect() }
        }
        Expr::AssignmentPattern { span, replicate, parts } => Expr::AssignmentPattern {
            span: *span,
            replicate: replicate.as_deref().map(|r| Box::new(sub(r))),
            parts: parts.iter().map(&sub).collect(),
        },
    }
}

/// Statement-tree counterpart of [`rename_idents_expr`].
fn rename_idents_stmt(stmt: &Stmt, renames: &HashMap<String, String>) -> Stmt {
    let e = |expr: &Expr| rename_idents_expr(expr, renames);
    let s = |stmt: &Stmt| Box::new(rename_idents_stmt(stmt, renames));
    match stmt {
        Stmt::Null { .. } | Stmt::Disable { .. } | Stmt::EventTrigger { .. } => stmt.clone(),
        Stmt::Assign { span, target, value } => Stmt::Assign { span: *span, target: e(target), value: e(value) },
        Stmt::NonBlockingAssign { span, target, value } => {
            Stmt::NonBlockingAssign { span: *span, target: e(target), value: e(value) }
        }
        Stmt::Contribution { span, target, value } => {
            Stmt::Contribution { span: *span, target: e(target), value: e(value) }
        }
        Stmt::IndirectContribution { span, lhs, probe, rhs } => {
            Stmt::IndirectContribution { span: *span, lhs: e(lhs), probe: e(probe), rhs: e(rhs) }
        }
        Stmt::Block { span, name, decls, stmts } => Stmt::Block {
            span: *span,
            name: name.clone(),
            decls: decls.clone(),
            stmts: stmts.iter().map(|st| rename_idents_stmt(st, renames)).collect(),
        },
        Stmt::If { span, cond, then_branch, else_branch } => Stmt::If {
            span: *span,
            cond: e(cond),
            then_branch: s(then_branch),
            else_branch: else_branch.as_deref().map(s),
        },
        Stmt::Case { span, kind, selector, arms, attrs } => Stmt::Case {
            span: *span,
            kind: *kind,
            selector: e(selector),
            arms: arms
                .iter()
                .map(|arm| CaseArm {
                    span: arm.span,
                    labels: arm.labels.as_ref().map(|ls| ls.iter().map(&e).collect()),
                    body: s(&arm.body),
                })
                .collect(),
            attrs: attrs.clone(),
        },
        Stmt::For { span, init, cond, step, body } => {
            Stmt::For { span: *span, init: s(init), cond: e(cond), step: s(step), body: s(body) }
        }
        Stmt::While { span, cond, body } => Stmt::While { span: *span, cond: e(cond), body: s(body) },
        Stmt::Repeat { span, count, body } => Stmt::Repeat { span: *span, count: e(count), body: s(body) },
        Stmt::Forever { span, body } => Stmt::Forever { span: *span, body: s(body) },
        Stmt::EventControl { span, events, body } => Stmt::EventControl {
            span: *span,
            events: events
                .iter()
                .map(|ev| match ev {
                    EventExpr::Level(x) => EventExpr::Level(e(x)),
                    EventExpr::Posedge(x) => EventExpr::Posedge(e(x)),
                    EventExpr::Negedge(x) => EventExpr::Negedge(e(x)),
                    EventExpr::ImplicitAll => EventExpr::ImplicitAll,
                })
                .collect(),
            body: s(body),
        },
        Stmt::DelayControl { span, delay, body } => Stmt::DelayControl { span: *span, delay: e(delay), body: s(body) },
        Stmt::TaskCall { span, name, args } => {
            Stmt::TaskCall { span: *span, name: name.clone(), args: args.iter().map(&e).collect() }
        }
    }
}

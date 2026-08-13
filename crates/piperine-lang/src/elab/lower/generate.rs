//! `UnrollGenerate` — LRM ch. 6 `for`/`if`/`case` generate constructs
//! unrolled into **derived** instances, never into `Design::modules`
//! (MD-25 VAMS clause, design §6): a generate block reads back exactly as
//! authored from `Module::instances`; the instances it produces live only
//! in [`Elaborator::generated_instances`], which [`super::flatten`]'s
//! `FlattenHierarchy` splices in when building `Design::flat_modules`.
//!
//! Scope (T25): `for`/`if`/`case` generate bodies containing module
//! instantiations, arbitrarily nested. A generated instance's label is
//! mangled `{label}__{genvar value}` per loop level (outermost first) —
//! collisions (two generated instances landing on the same mangled name,
//! or a generated name colliding with an authored one) fail loud with
//! both source spans, never a silent rename (spec Edge Cases).

use std::collections::HashMap;

use piperine_vams::ast::{
    CaseGenerate, GenerateBlock, GenerateRegion, IfGenerate, LoopGenerate, ModuleDecl, ModuleItem,
};

use crate::elab::const_eval::ConstEnv;
use crate::pom::{ElabError, ElabErrorKind, Instance};

use super::passes::ElabPass;
use super::Elaborator;

/// The elaboration pass: unrolls every module's generate constructs into
/// `Elaborator::generated_instances`, keyed by module name. Runs after
/// `ElabModules` (needs the module registered for `lower_instance`'s
/// target-module port-order lookups) and before `FlattenHierarchy`
/// (which reads `generated_instances`).
pub(super) struct UnrollGenerate;

impl ElabPass for UnrollGenerate {
    fn run(&self, elab: &mut Elaborator, _design: &mut crate::pom::Design) -> Result<(), ElabError> {
        let names: Vec<String> = elab.syms.module_decls.keys().cloned().collect();
        for name in &names {
            let decl = elab.syms.module_decls[name].clone();
            let mut out = Vec::new();
            // The module's own params must be visible inside its generate
            // bodies exactly like anywhere else in the module (bug fix:
            // this used to be a permanently-empty `ConstEnv::new()`, so a
            // generate loop's port connections/param overrides could never
            // reference an enclosing `parameter` — see `vams::
            // module_param_env`'s doc comment).
            let env = super::vams::module_param_env(&decl)?;
            // Only descend into the module's *generate* constructs — an
            // ordinary top-level instantiation is already lowered into
            // `Module::instances` by `ElabModules` and must not be
            // duplicated here.
            for item in &decl.items {
                match item {
                    ModuleItem::LoopGenerate(lg) => unroll_loop(elab, &decl, lg, &env, &mut out)?,
                    ModuleItem::IfGenerate(ig) => unroll_if(elab, &decl, ig, &env, &mut out)?,
                    ModuleItem::CaseGenerate(cg) => unroll_case(elab, &decl, cg, &env, &mut out)?,
                    ModuleItem::GenerateRegion(GenerateRegion { items, .. }) => {
                        unroll_items(elab, &decl, items, &env, &mut out)?;
                    }
                    _ => {}
                }
            }
            check_no_collisions(name, &out)?;
            if !out.is_empty() {
                elab.generated_instances.insert(name.clone(), out);
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

fn unroll_items(
    elab: &Elaborator,
    decl: &ModuleDecl,
    items: &[ModuleItem],
    env: &ConstEnv,
    out: &mut Vec<Instance>,
) -> Result<(), ElabError> {
    for item in items {
        match item {
            ModuleItem::Instantiation(inst_decl) => {
                for ast_inst in &inst_decl.instances {
                    let mut inst = elab.lower_instance(&inst_decl.module_name, ast_inst, &inst_decl.param_overrides, env)?;
                    inst.label = Some(mangle_label(ast_inst.name.as_str(), env));
                    out.push(inst);
                }
            }
            ModuleItem::LoopGenerate(lg) => unroll_loop(elab, decl, lg, env, out)?,
            ModuleItem::IfGenerate(ig) => unroll_if(elab, decl, ig, env, out)?,
            ModuleItem::CaseGenerate(cg) => unroll_case(elab, decl, cg, env, out)?,
            ModuleItem::GenerateRegion(GenerateRegion { items, .. }) => {
                unroll_items(elab, decl, items, env, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn unroll_block(
    elab: &Elaborator,
    decl: &ModuleDecl,
    block: &GenerateBlock,
    env: &ConstEnv,
    out: &mut Vec<Instance>,
) -> Result<(), ElabError> {
    unroll_items(elab, decl, &block.items, env, out)
}

/// `for (genvar = init; cond; step_var = step_value) generate_block` —
/// §6.2. Iterates with `env`'s scope pushed once per loop, the genvar
/// re-bound each iteration; every generated instance in that iteration
/// gets the current value folded into its label.
fn unroll_loop(
    elab: &Elaborator,
    decl: &ModuleDecl,
    lg: &LoopGenerate,
    env: &ConstEnv,
    out: &mut Vec<Instance>,
) -> Result<(), ElabError> {
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

        let mut scoped = Vec::new();
        unroll_block(elab, decl, &lg.body, &iter_env, &mut scoped)?;
        for inst in &mut scoped {
            let label = inst.name().to_string();
            inst.label = Some(format!("{label}__{i}"));
        }
        out.extend(scoped);

        i = iter_env.eval_int(&lg.step_value).map_err(|e| {
            ElabError::from(ElabErrorKind::ConstEval { context: format!("generate loop `{}` step", lg.genvar), source: e })
        })?;
    }
    Ok(())
}

/// `if (cond) generate_block [else generate_block]` — §6.3. `cond` must be
/// an elaboration constant; picks the taken branch once, no genvar.
fn unroll_if(
    elab: &Elaborator,
    decl: &ModuleDecl,
    ig: &IfGenerate,
    env: &ConstEnv,
    out: &mut Vec<Instance>,
) -> Result<(), ElabError> {
    let cond = env
        .eval(&ig.cond)
        .map_err(|e| ElabError::from(ElabErrorKind::ConstEval { context: "generate if condition".into(), source: e }))?;
    if truthy(&cond) {
        unroll_block(elab, decl, &ig.then_branch, env, out)
    } else if let Some(else_branch) = &ig.else_branch {
        unroll_block(elab, decl, else_branch, env, out)
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
    out: &mut Vec<Instance>,
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
                        return unroll_block(elab, decl, &arm.body, env, out);
                    }
                }
            }
        }
    }
    if let Some(arm) = default_arm {
        unroll_block(elab, decl, &arm.body, env, out)
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

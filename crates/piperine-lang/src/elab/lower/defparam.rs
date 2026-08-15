//! `defparam` application — §6.3.1, LRMC-C5-06.
//!
//! A `defparam hier.path.param = value;` statement changes an *already
//! declared* parameter's effective value, addressed by hierarchical name,
//! from anywhere in the design — not scoped to the declaring module's own
//! instantiation subtree (§6.3.1's own worked example: `annotate` has no
//! structural relationship to `tgate` at all). This pass reuses the exact
//! mechanism a `#(.param(val))` instantiation override already writes
//! (`Instance::params: Vec<(String, Value)>`) — a `defparam` is just
//! another way to populate that same list, applied globally after every
//! module (and therefore every instance) is registered.
//!
//! **MD-25 (POM navigability):** writes only `Instance::params` entries —
//! never touches `Design::modules`' authored instance/submodule structure.

use crate::ast::{Defparam, ModuleItem};
use crate::elab::const_eval::ConstEnv;
use crate::pom::{Design, ElabError, ElabErrorKind};

use super::Elaborator;

/// Applies every `defparam` statement found across every registered module
/// declaration, in declaration order.
pub(super) struct ApplyDefparams;

impl super::passes::ElabPass for ApplyDefparams {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        let mut items: Vec<(String, Defparam)> = Vec::new();
        for (mod_name, decl) in &elab.syms.module_decls {
            for item in &decl.items {
                if let ModuleItem::Defparam(dp) = item {
                    items.push((mod_name.clone(), dp.clone()));
                }
            }
        }
        // Declaration order isn't guaranteed by the `HashMap` walk above;
        // sort by (declaring module, target) so this pass is deterministic
        // run to run (no LRM ordering requirement across *different*
        // defparam statements to preserve, only reproducibility).
        items.sort_by(|a, b| (&a.0, &a.1.target).cmp(&(&b.0, &b.1.target)));
        for (declaring_module, dp) in &items {
            apply_one(design, declaring_module, dp)?;
        }
        Ok(())
    }
}

fn apply_one(design: &mut Design, declaring_module: &str, dp: &Defparam) -> Result<(), ElabError> {
    let mut segments: Vec<&str> = dp.target.split('.').collect();
    let Some(param_name) = segments.pop() else {
        return Err(defparam_error(&dp.target, "empty target"));
    };
    if segments.is_empty() {
        return Err(defparam_error(
            &dp.target,
            "a defparam target must be a hierarchical instance path, e.g. `inst.param` (§6.3.1)",
        ));
    }

    // §6.3.1: "The referenced parameters (on the right-hand side of a
    // defparam) shall be declared in the same module as the defparam
    // statement" -- evaluate `dp.value` against the declaring module's own
    // already-elaborated parameter values.
    let decl_module = design.module(declaring_module).ok_or_else(|| {
        defparam_error(&dp.target, &format!("declaring module `{declaring_module}` not found"))
    })?;
    let mut env = ConstEnv::new();
    for p in decl_module.params() {
        if let Some(v) = &p.default {
            env.define(p.name.clone(), v.clone());
        }
    }
    let value = env.eval(&dp.value).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval {
            context: format!("defparam `{}` value", dp.target),
            source: e,
        })
    })?;

    // The path's first segment names the root module context (§6.3.1's own
    // example: `tgate.m1.gate_width`, where `tgate` is a module name, not
    // an instance label anywhere in scope -- a top-level module used
    // directly as the implicit root). Every remaining segment before the
    // last is an instance label to descend into.
    let root_module_name = segments[0];
    if design.module(root_module_name).is_none() {
        return Err(defparam_error(&dp.target, &format!("module `{root_module_name}` does not exist")));
    }
    let mut current_module_name = root_module_name.to_string();
    for &label in &segments[1..segments.len() - 1] {
        let m = design.module(&current_module_name).ok_or_else(|| {
            defparam_error(&dp.target, &format!("module `{current_module_name}` does not exist"))
        })?;
        let inst = m.instances.iter().find(|i| i.label.as_deref() == Some(label)).ok_or_else(|| {
            defparam_error(
                &dp.target,
                &format!("no instance labeled `{label}` in module `{current_module_name}`"),
            )
        })?;
        current_module_name = inst.module.clone();
    }

    let final_label = segments[segments.len() - 1];
    let target_module = design.modules_map_mut().get_mut(&current_module_name).ok_or_else(|| {
        defparam_error(&dp.target, &format!("module `{current_module_name}` does not exist"))
    })?;
    let inst = target_module.instances.iter_mut().find(|i| i.label.as_deref() == Some(final_label)).ok_or_else(
        || {
            defparam_error(
                &dp.target,
                &format!("no instance labeled `{final_label}` in module `{current_module_name}`"),
            )
        },
    )?;
    match inst.params.iter_mut().find(|(n, _)| n == param_name) {
        Some(entry) => entry.1 = value,
        None => inst.params.push((param_name.to_string(), value)),
    }
    Ok(())
}

fn defparam_error(target: &str, reason: &str) -> ElabError {
    ElabError::from(ElabErrorKind::Other(format!("defparam `{target}` (§6.3.1): {reason}")))
}

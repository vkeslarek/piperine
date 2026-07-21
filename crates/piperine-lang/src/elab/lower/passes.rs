//! The elaboration pass list (SIMPLIFICATION.md P6).
//!
//! [`Elaborator::elaborate`] runs these in order over shared state — the
//! `Elaborator` itself (symbol tables collected by `Register`, the global
//! const environment) and the `Design` under construction. Each pass is
//! one struct with one job; the pipeline *order* is the [`PASSES`] array,
//! not something buried in a 150-line driver.
//!
//! [`Elaborator::elaborate`]: super::Elaborator::elaborate

use std::collections::HashMap;

use crate::elab::const_eval::ConstEnv;
use crate::pom::{Design, ElabError, ElabErrorKind};

use super::Elaborator;

/// One elaboration stage. `run` mutates the shared `Elaborator` state and
/// the `Design` under construction; the first error aborts the pipeline.
pub(crate) trait ElabPass {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError>;
}

/// The elaboration pipeline, in execution order.
pub(crate) const PASSES: &[&dyn ElabPass] = &[
    &Register,
    &ValidateEvents,
    &FoldGlobals,
    &ElabFns,
    &ElabModules,
    &AttachBehaviors,
    &ResolveCalls,
    &Typecheck,
];

/// Index every top-level item into the elaborator's symbol tables.
struct Register;
impl ElabPass for Register {
    fn run(&self, elab: &mut Elaborator, _design: &mut Design) -> Result<(), ElabError> {
        let items = std::mem::take(&mut elab.pending_items);
        elab.register_items(items.iter())
    }
}

/// Event-kind validation for `mod` bodies and behaviors (borrows the event
/// registry immutably; must precede any monomorphization).
struct ValidateEvents;
impl ElabPass for ValidateEvents {
    fn run(&self, elab: &mut Elaborator, _design: &mut Design) -> Result<(), ElabError> {
        let mod_decls: Vec<_> = elab.syms.module_decls.values().cloned().collect();
        for decl in &mod_decls {
            if decl.const_params.is_empty() && decl.type_params.is_empty() {
                elab.ctx.events.validate_mod_body(&decl.body)?;
            }
        }
        let beh_decls: Vec<_> = elab.syms.behavior_decls.clone();
        for beh in &beh_decls {
            elab.ctx.events.validate_behavior(beh.kind.clone(), &beh.body)?;
        }
        Ok(())
    }
}

/// Copy declaration maps into the design and fold every global constant.
/// Enum variants seed the const environment first (SPEC §6.4 / B.1): a
/// variant is usable bare (`Idle`) or qualified (`SarState::Idle`)
/// wherever a constant is. Consts may reference each other in any order —
/// iterate to a fixed point.
struct FoldGlobals;
impl ElabPass for FoldGlobals {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        *design.disciplines_map_mut() = elab.syms.disciplines.clone();
        *design.bundles_map_mut() = elab.syms.bundles.clone();
        *design.enums_map_mut() = elab.syms.enums.clone();
        *design.capabilities_map_mut() = elab.syms.capability_decls.clone();

        let mut globals = elab.enum_variant_globals()?;
        let mut pending: HashMap<String, crate::parse::ast::ConstDecl> = elab.syms.const_decls.clone();
        let mut last_len = pending.len() + 1;
        while pending.len() < last_len {
            last_len = pending.len();
            let mut resolved = Vec::new();
            for (name, decl) in &pending {
                let env = ConstEnv::with_globals(globals.clone());
                if let Ok(val) = env.eval(&decl.value) {
                    globals.insert(name.clone(), val.clone());
                    design.consts_map_mut().insert(name.clone(), val);
                    resolved.push(name.clone());
                }
            }
            for name in resolved {
                pending.remove(&name);
            }
        }
        if !pending.is_empty() {
            return Err(ElabError::from(ElabErrorKind::Other(
                "could not resolve one or more global constants".into(),
            )));
        }
        elab.syms.globals = globals;
        Ok(())
    }
}

/// Elaborate `impl` blocks and global `fn`s into the design.
struct ElabFns;
impl ElabPass for ElabFns {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        for impl_decl in &elab.syms.impl_decls.clone() {
            let block = elab.elab_impl(impl_decl)?;
            design.impls_vec_mut().push(block);
        }
        for fn_decl in elab.syms.fn_decls.values().cloned().collect::<Vec<_>>() {
            let f = elab.elab_fn(&fn_decl)?;
            design.functions_map_mut().insert(f.name.clone(), f);
        }
        Ok(())
    }
}

/// Elaborate every non-generic module. Generic modules monomorphize on
/// demand when an instance with const args is encountered
/// (`lower_mod_stmt`); the cache drains in [`AttachBehaviors`].
struct ElabModules;
impl ElabPass for ElabModules {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        let mod_names: Vec<String> = elab.syms.module_decls.keys().cloned().collect();
        for name in &mod_names {
            let decl = elab.syms.module_decls[name].clone();
            if decl.const_params.is_empty() && decl.type_params.is_empty() {
                let mut env = ConstEnv::with_globals(elab.syms.globals.clone());
                let elab_mod = elab.elab_mod_inner(&decl, &mut env, &HashMap::new())?;
                design.modules_map_mut().insert(name.clone(), elab_mod);
            }
        }
        Ok(())
    }
}

/// Attach `analog`/`digital` blocks to their modules by name — including
/// monomorphized instances: `analog Capacitor { … }` also attaches to
/// `Capacitor__8` (the `Base__args` mangling). Drains the mono cache
/// between the two attach rounds so on-demand modules participate.
struct AttachBehaviors;
impl ElabPass for AttachBehaviors {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        for beh in &elab.syms.behavior_decls.clone() {
            let behavior = elab.elab_behavior(beh)?;
            if let Some(module) = design.modules_map_mut().get_mut(&behavior.name) {
                module.behaviors.push(behavior);
            }
        }

        // Merge all on-demand monomorphized modules into the program.
        for elab_mod in elab.ctx.components.drain_mono_cache() {
            let name = elab_mod.name.clone();
            design.modules_map_mut().entry(name).or_insert(elab_mod);
        }

        for beh in &elab.syms.behavior_decls.clone() {
            let behavior = elab.elab_behavior(beh)?;
            let base = &behavior.name;
            for (name, module) in design.modules_map_mut().iter_mut() {
                if name == base {
                    continue; // already attached above
                }
                // Monomorphized name: "BaseName__arg1_arg2_..."
                if let Some(rest) = name.strip_prefix(&format!("{base}__"))
                    && !rest.is_empty()
                    && rest.chars().all(|c| c.is_ascii_digit() || c == '_')
                    && !module.behaviors.iter().any(|b| b.name == behavior.name && b.kind == behavior.kind)
                {
                    module.behaviors.push(behavior.clone());
                }
            }
        }
        Ok(())
    }
}

/// GAPS §J.4 — resolve built-in casts and validate diagnostic calls.
struct ResolveCalls;
impl ElabPass for ResolveCalls {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        elab.ctx.resolve_calls(design)
    }
}

/// GAPS §B.1 + §B.2 — connection width/discipline checks over the finished
/// design, before codegen ever sees it.
struct Typecheck;
impl ElabPass for Typecheck {
    fn run(&self, _elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        crate::elab::typecheck::typecheck_program(design)
    }
}

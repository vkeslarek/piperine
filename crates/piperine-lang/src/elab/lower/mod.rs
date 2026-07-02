//! The elaborator: registers top-level symbols, resolves types, and lowers
//! a parsed [`SourceFile`] into a [`Design`].
//!
//! One `Elaborator` struct, its methods spread across sibling files by
//! concern (this is one god struct, not several — see the refactor plan's
//! out-of-scope list for decomposing it into cooperating passes):
//!
//! | File | Concern |
//! |------|---------|
//! | `mod.rs` | struct + fields, `new()`, the `elaborate()` driver |
//! | `register.rs` | top-level symbol registration |
//! | `resolve.rs` | type/net-type resolution, net references, port expansion |
//! | `module.rs` | `mod` body → `Module` (ports/params/wires/instances) |
//! | `behavior.rs` | `analog`/`digital` body → `Behavior` |
//! | `mono.rs` | `fn`/`impl` elaboration, generic monomorphization |

use std::collections::HashMap;

use crate::parse::ast::{
    BehaviorDecl, DisciplineDecl, EnumDecl, FnDecl, ImplDecl, ModuleDeclaration, SourceFile,
};
use crate::elab::const_eval::{ConstEnv, ConstVal};
use crate::elab::event::EventRegistry;
use crate::pom::{ElabError, ElabErrorKind, Design, Module};

mod behavior;
mod module;
mod mono;
mod register;
mod resolve;

pub struct Elaborator {
    disciplines: HashMap<String, DisciplineDecl>,
    bundles: HashMap<String, crate::parse::ast::BundleDecl>,
    enums: HashMap<String, EnumDecl>,
    module_decls: HashMap<String, ModuleDeclaration>,
    behavior_decls: Vec<BehaviorDecl>,
    fn_decls: HashMap<String, FnDecl>,
    capability_decls: HashMap<String, crate::parse::ast::CapabilityDecl>,
    impl_decls: Vec<ImplDecl>,
    const_decls: HashMap<String, crate::parse::ast::ConstDecl>,
    ctx: crate::elab::registry::ElabContext,
}

impl Elaborator {
    /// Creates a new `Elaborator` with empty symbol tables and a
    /// default `EventRegistry` pre-populated with built-in events.
    pub fn new() -> Self {
        Self {
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            enums: HashMap::new(),
            module_decls: HashMap::new(),
            behavior_decls: Vec::new(),
            fn_decls: HashMap::new(),
            capability_decls: HashMap::new(),
            impl_decls: Vec::new(),
            const_decls: HashMap::new(),
            ctx: crate::elab::registry::ElabContext::new(),
        }
    }

    /// Every enum variant's discriminant as a global const: bare (`Idle`)
    /// and qualified (`SarState::Idle`). Values default sequential from
    /// zero, continuing after an explicit discriminant (SPEC §6.4).
    fn enum_variant_globals(&self) -> Result<HashMap<String, ConstVal>, ElabError> {
        let mut globals = HashMap::new();
        for (enum_name, decl) in &self.enums {
            let mut next: i64 = 0;
            for variant in &decl.variants {
                let value = match &variant.value {
                    Some(expr) => {
                        let val = ConstEnv::new().eval(expr).map_err(|e| {
                            ElabErrorKind::ConstEval {
                                context: format!("enum `{enum_name}` variant `{}`", variant.name),
                                source: e,
                            }
                        })?;
                        match val {
                            ConstVal::Int(v) => v,
                            ConstVal::Nat(v) => v as i64,
                            other => {
                                return Err(ElabError::from(ElabErrorKind::Other(format!(
                                    "enum `{enum_name}` variant `{}` has non-integer discriminant {other:?}",
                                    variant.name
                                ))));
                            }
                        }
                    }
                    None => next,
                };
                globals.insert(variant.name.clone(), ConstVal::Int(value));
                globals.insert(format!("{enum_name}::{}", variant.name), ConstVal::Int(value));
                next = value + 1;
            }
        }
        Ok(globals)
    }

    /// Registers all top-level symbols from `source`, validates events,
    /// then elaborates functions, impl blocks, non-generic modules, and
    /// behaviors into a complete `Design`. Generic modules are monomorphized
    /// on demand when encountered via instance lowering.
    pub fn elaborate(&mut self, source: SourceFile) -> Result<Design, ElabError> {
        self.register_items(source.items.iter())?;

        // Validation pass — borrows self.events immutably. Must complete before
        // any &mut self calls (elab_mod_inner, monomorphize).
        {
            let mod_decls: Vec<_> = self.module_decls.values().cloned().collect();
            for decl in &mod_decls {
                if decl.const_params.is_empty() && decl.type_params.is_empty() {
                    self.ctx.events.validate_mod_body(&decl.body)?;
                }
            }
            let beh_decls: Vec<_> = self.behavior_decls.clone();
            for beh in &beh_decls {
                self.ctx.events.validate_behavior(beh.kind.clone(), &beh.body)?;
            }
        }

        let mut prog = Design::new();

        *prog.disciplines_map_mut() = self.disciplines.clone();
        *prog.bundles_map_mut() = self.bundles.clone();
        *prog.enums_map_mut() = self.enums.clone();
        *prog.capabilities_map_mut() = self.capability_decls.clone();

        // Evaluate all global consts. Enum variants seed the global const
        // environment first (SPEC §6.4 / B.1): a variant is usable bare
        // (`Idle`) or qualified (`SarState::Idle`) wherever a constant is.
        let mut evaluated_globals = self.enum_variant_globals()?;
        let mut pending_consts: HashMap<String, crate::parse::ast::ConstDecl> = self.const_decls.clone();
        let mut last_len = pending_consts.len() + 1;
        while pending_consts.len() < last_len {
            last_len = pending_consts.len();
            let mut resolved = Vec::new();
            for (name, decl) in &pending_consts {
                let env = ConstEnv::with_globals(evaluated_globals.clone());
                if let Ok(val) = env.eval(&decl.value) {
                    evaluated_globals.insert(name.clone(), val.clone());
                    prog.consts_map_mut().insert(name.clone(), (&val).into());
                    resolved.push(name.clone());
                }
            }
            for name in resolved {
                pending_consts.remove(&name);
            }
        }
        if !pending_consts.is_empty() {
            return Err(ElabError::from(ElabErrorKind::Other(
                "could not resolve one or more global constants".into(),
            )));
        }

        for impl_decl in &self.impl_decls.clone() {
            let block = self.elab_impl(impl_decl)?;
            prog.impls_vec_mut().push(block);
        }

        for fn_decl in self.fn_decls.values().cloned().collect::<Vec<_>>() {
            let f = self.elab_fn(&fn_decl)?;
            prog.functions_map_mut().insert(f.name.clone(), f);
        }

        // Elaborate all non-generic modules. Monomorphization of generic
        // modules is triggered on demand inside lower_mod_stmt when an
        // instance with const args is encountered.
        let mod_names: Vec<String> = self.module_decls.keys().cloned().collect();
        for name in &mod_names {
            let decl = self.module_decls[name].clone();
            if decl.const_params.is_empty() && decl.type_params.is_empty() {
                let mut env = ConstEnv::with_globals(evaluated_globals.clone());
                let elab_mod = self.elab_mod_inner(&decl, &mut env, &HashMap::new())?;
                prog.modules_map_mut().insert(name.clone(), elab_mod);
            }
        }

        for beh in &self.behavior_decls.clone() {
            let behavior = self.elab_behavior(beh)?;
            if let Some(module) = prog.modules_map_mut().get_mut(&behavior.name) {
                module.behaviors.push(behavior);
            }
        }

        // Merge all on-demand monomorphized modules into the program.
        for elab_mod in self.ctx.components.drain_mono_cache() {
            let name = elab_mod.name.clone();
            prog.modules_map_mut().entry(name).or_insert(elab_mod);
        }

        // Attach behaviors to monomorphized modules. A behavior declared as
        // `analog Capacitor { ... }` (name "Capacitor") must also attach to
        // `Capacitor__8` (monomorphized from `Capacitor[8]`). The base name
        // is the part before the `__` suffix.
        for beh in &self.behavior_decls.clone() {
            let behavior = self.elab_behavior(beh)?;
            let base = &behavior.name;
            for (name, module) in prog.modules_map_mut().iter_mut() {
                if name == base {
                    continue; // already attached above
                }
                // Monomorphized name: "BaseName__arg1_arg2_..."
                if let Some(rest) = name.strip_prefix(&format!("{base}__")) {
                    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '_') {
                        // Avoid duplicate attachment if already present.
                        if !module.behaviors.iter().any(|b| b.name == behavior.name && b.kind == behavior.kind) {
                            module.behaviors.push(behavior.clone());
                        }
                    }
                }
            }
        }

        // GAPS §J.4 — resolve calls to built-in casts and validate diagnostics
        self.ctx.callables.resolve_calls(&mut prog)?;

        // GAPS §B.1 + §B.2 — the typecheck pass walks every module's
        // connections and rejects width mismatches and discipline
        // crossings. Runs after elaboration (so all port/wire/instance
        // bindings are typed) and before codegen.
        crate::elab::typecheck::typecheck_program(&prog)?;

        Ok(prog)
    }

}

impl crate::elab::registry::components::Instantiator for Elaborator {
    fn ctx(&self) -> &crate::elab::registry::ElabContext {
        &self.ctx
    }
    
    fn elaborate_mod_decl(
        &mut self,
        decl: &ModuleDeclaration,
        env: &mut ConstEnv,
        type_subst: &std::collections::HashMap<String, String>,
    ) -> Result<Module, ElabError> {
        self.elab_mod_inner(decl, env, type_subst)
    }
}

/// Constructs an `Elaborator` via [`Elaborator::new`].
impl Default for Elaborator {
    fn default() -> Self {
        Self::new()
    }
}

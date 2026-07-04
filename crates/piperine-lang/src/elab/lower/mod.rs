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
    BehaviorDecl, BenchDecl, DisciplineDecl, EnumDecl, FnDecl, ImplDecl, ModuleDeclaration, SourceFile,
};
use crate::elab::const_eval::ConstEnv;
use crate::value::Value;
use crate::pom::{ElabError, ElabErrorKind, Design, Module};

mod behavior;
mod module;
mod mono;
mod passes;
mod register;
mod resolve;

pub struct Elaborator {
    disciplines: HashMap<String, DisciplineDecl>,
    bundles: HashMap<String, crate::parse::ast::BundleDecl>,
    enums: HashMap<String, EnumDecl>,
    module_decls: HashMap<String, ModuleDeclaration>,
    behavior_decls: Vec<BehaviorDecl>,
    bench_decls: Vec<BenchDecl>,
    fn_decls: HashMap<String, FnDecl>,
    capability_decls: HashMap<String, crate::parse::ast::CapabilityDecl>,
    impl_decls: Vec<ImplDecl>,
    const_decls: HashMap<String, crate::parse::ast::ConstDecl>,
    /// Items handed to `elaborate`, consumed by the `Register` pass.
    pending_items: Vec<crate::parse::ast::Item>,
    /// Folded global constants (enum variants + `const` decls), produced
    /// by the `FoldGlobals` pass and read by `ElabModules`.
    globals: HashMap<String, crate::value::Value>,
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
            bench_decls: Vec::new(),
            fn_decls: HashMap::new(),
            capability_decls: HashMap::new(),
            impl_decls: Vec::new(),
            const_decls: HashMap::new(),
            pending_items: Vec::new(),
            globals: HashMap::new(),
            ctx: crate::elab::registry::ElabContext::new(),
        }
    }

    /// Every enum variant's discriminant as a global const: bare (`Idle`)
    /// and qualified (`SarState::Idle`). Values default sequential from
    /// zero, continuing after an explicit discriminant (SPEC §6.4).
    fn enum_variant_globals(&self) -> Result<HashMap<String, Value>, ElabError> {
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
                            Value::Int(v) => v,
                            Value::Nat(v) => v as i64,
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
                globals.insert(variant.name.clone(), Value::Int(value));
                globals.insert(format!("{enum_name}::{}", variant.name), Value::Int(value));
                next = value + 1;
            }
        }
        Ok(globals)
    }

    /// Run the elaboration pipeline over `source`: every stage is an
    /// explicit entry in [`passes::PASSES`] (SIMPLIFICATION.md P6) — read
    /// that array to read the phase order; this driver only loops.
    pub fn elaborate(&mut self, source: SourceFile) -> Result<Design, ElabError> {
        self.pending_items = source.items;
        let mut design = Design::new();
        for pass in passes::PASSES {
            pass.run(self, &mut design)?;
        }
        Ok(design)
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

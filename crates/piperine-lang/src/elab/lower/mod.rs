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
use crate::elab::const_eval::ConstEnv;
use crate::value::Value;
use crate::pom::{ElabError, ElabErrorKind, Design, Module};

mod behavior;
mod flatten;
mod module;
mod mono;
mod passes;
mod register;
mod resolve;
pub(crate) mod attrs;

/// The symbol table: all registered top-level declarations. Populated by
/// the `Register` pass, read by every subsequent pass. Separated from the
/// pipeline state (`items`) and the registries (`ctx`) for clarity.
pub struct SymbolTable {
    pub disciplines: HashMap<String, DisciplineDecl>,
    pub bundles: HashMap<String, crate::parse::ast::BundleDecl>,
    pub enums: HashMap<String, EnumDecl>,
    pub module_decls: HashMap<String, ModuleDeclaration>,
    pub behavior_decls: Vec<BehaviorDecl>,
    pub fn_decls: HashMap<String, FnDecl>,
    pub capability_decls: HashMap<String, crate::parse::ast::CapabilityDecl>,
    pub impl_decls: Vec<ImplDecl>,
    pub const_decls: HashMap<String, crate::parse::ast::ConstDecl>,
    /// Folded global constants (enum variants + `const` decls).
    pub globals: HashMap<String, crate::value::Value>,
}

impl SymbolTable {
    fn new() -> Self {
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
            globals: HashMap::new(),
        }
    }
}

pub struct Elaborator {
    pub syms: SymbolTable,
    /// Items handed to `elaborate`, consumed by the `Register` pass.
    pending_items: Vec<crate::parse::ast::Item>,
    pub(crate) ctx: crate::elab::registry::ElabContext,
    /// Errors accumulated by passes that don't abort on the first failure
    /// (`ElabModules`, `Typecheck` — LSP-18/T16, see `passes.rs`'s module
    /// docs for exactly which passes accumulate vs. fail fast). Empty for
    /// the ordinary [`elaborate`](Self::elaborate) entry point's callers —
    /// only [`elaborate_accumulating`](Self::elaborate_accumulating)
    /// reads it.
    pub(crate) accumulated_errors: Vec<ElabError>,
}

impl Elaborator {
    /// Creates a new `Elaborator` with empty symbol tables and a
    /// default `EventRegistry` pre-populated with built-in events.
    pub fn new() -> Self {
        Self {
            syms: SymbolTable::new(),
            pending_items: Vec::new(),
            ctx: crate::elab::registry::ElabContext::new(),
            accumulated_errors: Vec::new(),
        }
    }

    /// Every enum variant's discriminant as a global const: bare (`Idle`)
    /// and qualified (`SarState::Idle`). Values default sequential from
    /// zero, continuing after an explicit discriminant (SPEC §6.4).
    fn enum_variant_globals(&self) -> Result<HashMap<String, Value>, ElabError> {
        let mut globals = HashMap::new();
        for (enum_name, decl) in &self.syms.enums {
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

    /// Like [`elaborate`](Self::elaborate), but never aborts the pipeline
    /// early on account of a pass that accumulates its own errors
    /// internally (`ElabModules`, `Typecheck` today — LSP-18/T16): those
    /// passes attempt every independent item they own (every module,
    /// every module's typecheck) rather than stopping at the first
    /// failure, recording each error in `self.accumulated_errors` while
    /// still returning their *first* error to satisfy the ordinary
    /// [`ElabPass::run`] contract other callers (`elaborate`) rely on.
    /// This driver tells the two cases apart by whether
    /// `accumulated_errors` actually grew during the failing pass: if it
    /// did, the pass already recorded everything it could and the
    /// pipeline continues with whatever partial `design` it left behind
    /// (so a *later*, independent pass's error isn't silently swallowed
    /// by the first); if it didn't, the pass is a genuine precondition
    /// (`Register`, `ValidateEvents`, `FoldGlobals`, `ElabFns`,
    /// `AttachBehaviors`, `ResolveCalls`, `FlattenHierarchy` — order-
    /// dependent, each pass's output is the next's input) and the
    /// pipeline stops there, same as `elaborate` would.
    pub fn elaborate_accumulating(&mut self, source: SourceFile) -> (Design, Vec<ElabError>) {
        self.pending_items = source.items;
        let mut design = Design::new();
        for pass in passes::PASSES {
            let before = self.accumulated_errors.len();
            if let Err(e) = pass.run(self, &mut design)
                && self.accumulated_errors.len() == before
            {
                self.accumulated_errors.push(e);
                break;
            }
        }
        (design, std::mem::take(&mut self.accumulated_errors))
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

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Evaluate a `Range` to a concrete `Range<u64>` at elaboration time.
pub(crate) fn eval_range(
    range: &crate::parse::ast::Range,
    env: &ConstEnv,
    context: &str,
) -> Result<std::ops::Range<u64>, ElabError> {
    let start = env.eval_nat(&range.start).map_err(|e| ElabErrorKind::ConstEval {
        context: format!("{} start", context),
        source: e,
    })?;
    let end_val = env.eval_nat(&range.end).map_err(|e| ElabErrorKind::ConstEval {
        context: format!("{} end", context),
        source: e,
    })?;
    let end = if range.inclusive { end_val + 1 } else { end_val };
    Ok(start..end)
}

/// Mangle a module name with const args: `Dac` + `[8, 4]` → `Dac__8_4`.
pub(crate) fn mono_name(base: &str, args: &[u64]) -> String {
    if args.is_empty() {
        base.to_string()
    } else {
        let suffix: Vec<String> = args.iter().map(|n| n.to_string()).collect();
        format!("{}__{}", base, suffix.join("_"))
    }
}

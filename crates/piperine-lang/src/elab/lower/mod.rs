//! The elaborator: registers top-level VAMS items and lowers a parsed
//! [`SourceFile`] into a [`Design`].
//!
//! One `Elaborator` struct, its methods spread across sibling files by
//! concern:
//!
//! | File | Concern |
//! |------|---------|
//! | `mod.rs` | struct + fields, `new()`, the `elaborate()` driver |
//! | `register.rs` | top-level symbol registration |
//! | `module.rs` | `module`/`macromodule`/`connectmodule` → `Module` |
//! | `behavior.rs` | `analog`/`always`/`initial` body → `Behavior` |
//!
//! **MD-45 (2026-08-12):** the PHDL-era passes this pipeline ran
//! (`resolve.rs`'s type/net resolution, `mono.rs`'s generic
//! monomorphization, `constraint.rs`'s dv-core constraint-block
//! elaboration) are deleted with `piperine-lang/src/parse/` — none has a
//! VAMS Annex A production. `.specs/features/verilog-ams-support/tasks.md`
//! T22-T27 rebuild this pipeline's VAMS-shaped passes task by task.

// hygiene-exempt: `SymbolTable` + the `Elaborator` god struct — the shared
// state every sibling file in this module mutates through `&mut Elaborator`;
// splitting the struct definition out from its own `new()`/`elaborate()`
// driver would separate declaration from the one place that constructs it.
// No p6 task splits it.

use std::collections::HashMap;

use crate::ast::{ConnectrulesDecl, DisciplineDecl, Item, ModuleDecl, NatureDecl, SourceFile};
use crate::pom::{ElabError, Design};

mod connect;
mod defparam;
mod disciplines;
mod flatten;
mod generate;
mod passes;
mod register;
mod vams;

/// The symbol table: all registered top-level declarations. Populated by
/// the `Register` pass, read by every subsequent pass.
pub struct SymbolTable {
    pub disciplines: HashMap<String, DisciplineDecl>,
    pub natures: HashMap<String, NatureDecl>,
    pub module_decls: HashMap<String, ModuleDecl>,
    /// `connectrules name { connect A, B; ... } endconnectrules` blocks
    /// (LRM ch. 7), keyed by name — read by `InsertConnectModules` (T52).
    /// Registered (not silently dropped) starting T52; before this task
    /// `register.rs` matched `Item::Connectrules(_) => {}` and discarded
    /// it (see that file's own historical comment).
    pub connectrules: HashMap<String, ConnectrulesDecl>,
    /// `paramset name (module); ... endparamset` blocks (LRM §6.4),
    /// keyed by name — read by `lower_instance` (LRMC-C5-07, T29) so an
    /// instance naming a paramset resolves to `(paramset.module_name,
    /// bound params)` instead of failing `UndefinedModule`.
    pub paramsets: HashMap<String, crate::ast::ParamsetDecl>,
    /// `primitive name (...); ... endprimitive` UDP declarations (Annex
    /// A.5), keyed by name — read by `lower_instance` (LRMC-C7-13, T39)
    /// so an instance naming a UDP (syntactically identical to a module
    /// instantiation) resolves against the UDP's own port list instead
    /// of failing `UndefinedModule`. Registered starting T39; before this
    /// task `register.rs` matched `Item::Primitive(_) => {}`.
    pub primitives: HashMap<String, crate::ast::PrimitiveDecl>,
}

impl SymbolTable {
    fn new() -> Self {
        Self {
            disciplines: HashMap::new(),
            natures: HashMap::new(),
            module_decls: HashMap::new(),
            connectrules: HashMap::new(),
            paramsets: HashMap::new(),
            primitives: HashMap::new(),
        }
    }
}

pub struct Elaborator {
    pub syms: SymbolTable,
    /// Items handed to `elaborate`, consumed by the `Register` pass.
    pending_items: Vec<Item>,
    /// Elaboration-registry seam (T35, VAMS-12/13) — a caller seeds this
    /// *before* calling [`elaborate`](Self::elaborate) (e.g.
    /// `seed(&mut elaborator.ctx)`, mirroring `parse_and_elaborate_
    /// seeded`'s own hook), and may read it back afterward (e.g.
    /// `piperine-lang-server`'s `symbol_index::resolve_at`, T37, needs
    /// the populated `schemas` set for hover/goto-def). `pub`, not
    /// `pub(crate)`, so an external elaboration driver can do both.
    pub ctx: crate::elab::registry::ElabContext,
    /// Generate-produced instances (LRM ch. 6), keyed by the owning
    /// module's name — written by `UnrollGenerate` (T25), read by
    /// `FlattenHierarchy` when building `Design::flat_modules`. **Never**
    /// merged into `Design::modules` (MD-25 VAMS clause): a generate
    /// block reads back as a generate block on the authored side; the
    /// instances it produces exist only on the derived path.
    pub(crate) generated_instances: HashMap<String, Vec<crate::pom::Instance>>,
}

impl Elaborator {
    /// Creates a new `Elaborator` with empty symbol tables.
    pub fn new() -> Self {
        Self {
            syms: SymbolTable::new(),
            pending_items: Vec::new(),
            ctx: crate::elab::registry::ElabContext::new(),
            generated_instances: HashMap::new(),
        }
    }

    /// Run the elaboration pipeline over `source`: every stage is an
    /// explicit entry in `passes::PASSES` — read that array to read the
    /// phase order; this driver only loops.
    pub fn elaborate(&mut self, source: SourceFile) -> Result<Design, ElabError> {
        self.pending_items = source.items;
        let mut design = Design::new();
        design.directives = source.directives;
        for pass in passes::PASSES {
            pass.run(self, &mut design)?;
        }
        Ok(design)
    }
}

/// Constructs an `Elaborator` via [`Elaborator::new`].
impl Default for Elaborator {
    fn default() -> Self {
        Self::new()
    }
}

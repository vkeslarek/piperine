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

use std::collections::HashMap;

use piperine_vams::ast::{DisciplineDecl, Item, ModuleDecl, NatureDecl, SourceFile};
use crate::pom::{ElabError, Design};

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
}

impl SymbolTable {
    fn new() -> Self {
        Self {
            disciplines: HashMap::new(),
            natures: HashMap::new(),
            module_decls: HashMap::new(),
        }
    }
}

pub struct Elaborator {
    pub syms: SymbolTable,
    /// Items handed to `elaborate`, consumed by the `Register` pass.
    pending_items: Vec<Item>,
    pub(crate) ctx: crate::elab::registry::ElabContext,
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

//! # Elaboration phase
//!
//! Transforms a parsed [`SourceFile`][crate::parse::SourceFile] into a fully
//! resolved [`Design`][crate::Design].
//!
//! ```text
//! SourceFile (parse AST)  ──Elaborator──▶  Design (elaborated IR + POM)
//! ```
//!
//! ## What elaboration does
//!
//! 1. **`use` expansion** — `use` declarations are resolved by the [`Resolver`]
//!    (file-based or built-in) and expanded into flat item lists before the
//!    elaborator ever sees them.
//! 2. **Prelude injection** — standard capabilities (`Add`, `Sub`, `map`, …) are
//!    prepended so they are always in scope without requiring an explicit `use`.
//! 3. **Symbol registration** — disciplines, bundles, enums, modules, behaviors,
//!    functions, capabilities, impls are indexed into symbol tables.
//! 4. **Semantic validation** — [`validate`] rejects `<+` in digital blocks,
//!    `<-`/`<+` in mod bodies, and domain-mismatched event kinds.
//! 5. **Type resolution** — every `Type { name: String }` in the AST is
//!    resolved to a `NetType` or `ValueType`; array dimensions are
//!    evaluated to concrete `u64` values.
//! 6. **Structural elaboration** — `StructuralFor` and `StructuralIf` in
//!    `mod` bodies are unrolled / evaluated away.
//! 7. **Bundle expansion** — net-capable bundle ports are expanded to flat
//!    `Port`s named `{port}_{field}`.
//! 8. **Generic monomorphization** — `mod Foo[N]` instantiated as `Foo[8]`
//!    produces `Module { name: "Foo__8", … }` with `N=8` substituted.
//! 9. **Behavioral for unrolling** — `for` loops in `analog`/`digital` blocks
//!    must have elaboration-constant bounds and are fully unrolled.
//! 10. **Event validation** — event names looked up in the [`EventRegistry`].

pub mod const_eval;
pub mod event;
pub mod lower;

use crate::pom::{Design, ElabError};
pub use lower::Elaborator;
use crate::parse::ast::SourceFile;
use crate::resolve::Resolver;

impl SourceFile {
    /// Elaborate this source file using a default [`Resolver`].
    ///
    /// Injects the standard-library prelude, expands all `use` declarations, then
    /// runs the full elaboration pipeline.  For project-level resolution (loading
    /// user files via `use foo::bar;`) use [`elaborate_with`] and supply a
    /// [`Resolver::with_root`].
    pub fn elaborate(self) -> Result<Design, ElabError> {
        self.elaborate_with(&mut Resolver::new())
    }

    /// Elaborate this source file using the supplied [`Resolver`].
    ///
    /// The resolver controls where `use` declarations are looked up:
    /// - Built-in `piperine::*` paths always resolve from embedded sources.
    /// - Other paths resolve relative to the root supplied to [`Resolver::with_root`].
    ///
    /// The standard-library prelude is always injected regardless of resolver
    /// configuration.
    pub fn elaborate_with(self, resolver: &mut Resolver) -> Result<Design, ElabError> {
        let mut items = resolver.prelude_items();
        let expanded = resolver
            .expand(self)
            .map_err(|e| ElabError::Other(e.to_string()))?;
        items.extend(expanded);
        let augmented = SourceFile { items };
        Elaborator::new().elaborate(augmented)
    }
}

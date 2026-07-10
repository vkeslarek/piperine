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
pub mod typecheck;
pub mod resolve;
pub mod registry;

use crate::pom::{Design, ElabError, ElabErrorKind};
pub use lower::Elaborator;
use crate::parse::ast::SourceFile;
use crate::resolve::Resolver;

use crate::source_map::SourceMap;

impl SourceFile {
    /// Elaborate this source file using the supplied [`SourceMap`].
    ///
    /// The source map controls where `use` declarations are looked up.
    pub fn elaborate(self, source_map: &SourceMap) -> Result<Design, ElabError> {
        let mut resolver = Resolver::new(source_map);
        self.elaborate_with(&mut resolver)
    }

    /// Elaborate this source file using the supplied [`Resolver`].
    pub fn elaborate_with(self, resolver: &mut Resolver) -> Result<Design, ElabError> {
        self.elaborate_seeded(resolver, |_| {})
    }

    /// Like [`elaborate_with`](Self::elaborate_with), but lets the caller
    /// seed the [`ElabContext`] registries before elaboration runs — the
    /// entry point plugin hosts use to contribute attribute schemas
    /// (SPEC Part VI §10) without `piperine-lang` knowing about plugins.
    pub fn elaborate_seeded(
        self,
        resolver: &mut Resolver,
        seed: impl FnOnce(&mut crate::elab::registry::ElabContext),
    ) -> Result<Design, ElabError> {
        let mut items = resolver.prelude_items();
        let expanded = resolver
            .expand(self)
            .map_err(|e| ElabError::from(ElabErrorKind::Other(e.to_string())))?;
        items.extend(expanded);
        let augmented = SourceFile { items };
        let mut elaborator = Elaborator::new();
        seed(&mut elaborator.ctx);
        let mut design = elaborator.elaborate(augmented)?;
        design.set_origins(resolver.take_origins());
        Ok(design)
    }
}

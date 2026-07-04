//! [`BenchBlock`] — a `bench ModName { fn ... }` attached to an elaborated
//! module (SPEC_BENCH.md §2), the effectful counterpart to [`Behavior`].
//!
//! Unlike a `Behavior`, a bench body is **not** lowered to IR: it is
//! interpreted directly (`piperine-bench`, via [`crate::eval`]) against the
//! elaborated [`super::Module`] it names, so the raw parsed [`FnDecl`]s are
//! kept as-is.
//!
//! [`Behavior`]: super::Behavior

use crate::parse::ast::FnDecl;

/// A `bench` block, attached to a module by name.
#[derive(Debug, Clone)]
pub struct BenchBlock {
    pub span: Option<miette::SourceSpan>,
    /// The module this bench is rooted at (SPEC_BENCH.md §3).
    pub module: String,
    /// Entry points and helpers, in source order.
    pub fns: Vec<FnDecl>,
}

impl BenchBlock {
    /// The module this bench is rooted at.
    pub fn module(&self) -> &str { &self.module }
    /// Every `fn` in the bench (entry points and helpers alike — a
    /// zero-argument `fn` is an entry point, SPEC_BENCH.md §2).
    pub fn fns(&self) -> &[FnDecl] { &self.fns }
    /// Entry points: zero-argument `fn`s the toolchain runs directly.
    pub fn entry_points(&self) -> impl Iterator<Item = &FnDecl> {
        self.fns.iter().filter(|f| f.sig.params.is_empty())
    }
    /// Look up a `fn` (entry point or helper) by name.
    pub fn fn_by_name(&self, name: &str) -> Option<&FnDecl> {
        self.fns.iter().find(|f| f.sig.name == name)
    }
}

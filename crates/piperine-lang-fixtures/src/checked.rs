//! What the type-checker targets share (`types_aggregates`, `types_control`,
//! `types_value_semantics`; `#[path]`-included, never a target itself): a source text built into a
//! design of `Top`, checked by `Types::check`, and what the check says read back.

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, Reflect};
use piperine_lang::types::{Ty, Types};

use crate::pom::PomSource;

/// A source text whose module `Top` the type checker checks.
pub struct CheckedSource;

impl CheckedSource {
    /// A design of `Top` with `items` before it and `body` as its analog block.
    pub fn source(items: &str, body: &str) -> String {
        format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
    }

    /// The design of `src` over an empty standard library, which must parse and resolve.
    pub fn design_of(src: &str) -> Design {
        let design = PomSource::build(src);
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", PomSource::codes(&design));
        design
    }

    /// The design of `src` over the embedded standard library, which must parse and resolve: a source
    /// that names `Bit`.
    pub fn standard_design_of(src: &str) -> Design {
        let design = Design::builder().source("top.msl", src).build();
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        design
    }

    /// The diagnostics of checking `Top`, as `(code, message)`; empty when the check passes.
    pub fn outcome(src: &str) -> Vec<(String, String)> {
        match Types::check(&Self::design_of(src), "Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }

    /// The diagnostics of checking `Top`, whole.
    pub fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
        Types::check(&Self::design_of(src), "Top").err().unwrap_or_default()
    }

    /// The source checks clean.
    pub fn assert_clean(src: &str) {
        let found = Self::outcome(src);
        assert!(found.is_empty(), "expected a clean check, got {found:?}\n{src}");
    }

    /// The source, a module over the embedded standard library, checks clean; `what` names the case.
    pub fn assert_clean_module(src: &str, what: &str) {
        if let Err(found) = Types::check(&Self::standard_design_of(src), "Top") {
            panic!("{what}: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        }
    }

    /// The source is refused with exactly one diagnostic of `code`, whose message holds `words`.
    pub fn assert_refused(src: &str, code: &str, words: &str) {
        let found = Self::outcome(src);
        assert_eq!(found.len(), 1, "expected one diagnostic, got {found:?}\n{src}");
        assert_eq!(found[0].0, code, "{found:?}\n{src}");
        assert!(found[0].1.contains(words), "`{}` does not name `{words}`\n{src}", found[0].1);
    }

    /// The checked `Top` of `src`, which must check clean.
    fn checked<'d>(design: &'d Design, src: &str) -> Types<'d> {
        Types::check(design, "Top").unwrap_or_else(|d| panic!("{:?}\n{src}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()))
    }

    /// The declaration of kind `kind` named `name`.
    pub fn declaration(design: &Design, name: &str, kind: fn(&DeclKind) -> bool) -> DeclId {
        design.decls().into_iter().find(|d| d.name() == Some(name) && kind(&d.kind())).unwrap_or_else(|| panic!("no declaration `{name}`")).id()
    }

    /// The type of the binding `name` in the checked source.
    pub fn local_type(src: &str, name: &str) -> Ty {
        Self::type_of_declaration(src, name, |k| matches!(k, DeclKind::Binding(_)))
    }

    /// The type of the binding or function parameter `name` in the checked source.
    pub fn binding_or_parameter_type(src: &str, name: &str) -> Ty {
        Self::type_of_declaration(src, name, |k| matches!(k, DeclKind::Binding(_) | DeclKind::FnParam(_)))
    }

    /// The type of the first declaration of kind `kind` named `name` in the checked source.
    fn type_of_declaration(src: &str, name: &str, kind: fn(&DeclKind) -> bool) -> Ty {
        let design = Self::design_of(src);
        let types = Self::checked(&design, src);
        let decl = Self::declaration(&design, name, kind);
        types.local(decl).cloned().unwrap_or_else(|| panic!("`{name}` has no type\n{src}"))
    }

    /// The struct or enum declaration named `name`.
    pub fn type_declaration(src: &str, name: &str) -> DeclId {
        Self::declaration(&Self::design_of(src), name, |k| matches!(k, DeclKind::Struct(_) | DeclKind::Enum(_)))
    }

    /// The level counts the `T::WIDTH` paths of the checked source read, in source order.
    pub fn widths(src: &str) -> Vec<u32> {
        let design = Self::design_of(src);
        let types = Self::checked(&design, src);
        (0..design.expr_count()).filter_map(|i| types.width(ExprId(i as u32))).collect()
    }

    /// The type the checker gave every expression of the design of kind `Assign`.
    pub fn assignment_types(src: &str) -> Vec<Option<Ty>> {
        let design = Self::design_of(src);
        let types = Self::checked(&design, src);
        (0..design.expr_count()).map(|e| ExprId(e as u32)).filter(|e| matches!(design.expr_node(*e).kind, ExprKind::Assign { .. })).map(|e| types.ty(e).cloned()).collect()
    }
}

//! What the type-checker targets share (the `types_*` targets of `piperine-lang`): a source text built into a
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

    /// The diagnostics of checking `Top` of `src` over the embedded standard library, as `(code, message)`; empty when the check passes.
    pub fn standard_outcome(src: &str) -> Vec<(String, String)> {
        match Types::check(&Self::standard_design_of(src), "Top") {
            Ok(_) => Vec::new(),
            Err(found) => Self::messages(&found),
        }
    }

    /// The function `name` of the standard library, by declaration.
    pub fn standard_function(design: &Design, name: &str) -> DeclId {
        design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Fn(_)) && d.krate().is_std()).unwrap_or_else(|| panic!("no std fn `{name}`")).id()
    }

    /// The diagnostics of checking `Top`, whole.
    pub fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
        Types::check(&Self::design_of(src), "Top").err().unwrap_or_default()
    }

    /// The diagnostics of checking `Top` of `src` over the empty standard library, which must not check clean.
    pub fn expected_diagnostics(src: &str) -> Vec<DesignDiagnostic> {
        Types::check(&Self::design_of(src), "Top").expect_err("expected diagnostics")
    }

    /// The source of `items` and `body` (see [`CheckedSource::source`]) and the diagnostics of checking it, which must not check clean.
    pub fn body_diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
        let src = Self::source(items, body);
        let found = Self::expected_diagnostics(&src);
        (src, found)
    }

    /// The diagnostics of checking `Top` of `src` over the embedded standard library, which must not check clean.
    pub fn standard_expected_diagnostics(src: &str) -> Vec<DesignDiagnostic> {
        Types::check(&Self::standard_design_of(src), "Top").expect_err("expected diagnostics")
    }

    /// The `(code, message)` pairs of the design of `src` over the embedded standard library and of the check of its `Top`.
    pub fn standard_findings(src: &str) -> Vec<(String, String)> {
        let design = Design::builder().source("top.msl", src).build();
        let mut found: Vec<(String, String)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
        if let Err(refused) = Types::check(&design, "Top") {
            found.extend(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())));
        }
        found
    }

    /// The codes of `diagnostics`, in order.
    pub fn codes(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
        diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The `(code, message)` of each of `diagnostics`.
    pub fn messages(diagnostics: &[DesignDiagnostic]) -> Vec<(String, String)> {
        diagnostics.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }

    /// The text the primary span of `diagnostic` covers in `src`.
    pub fn snippet<'a>(src: &'a str, diagnostic: &DesignDiagnostic) -> &'a str {
        &src[diagnostic.diagnostic.span().range()]
    }

    /// Checks `top` of the design of `src` over the embedded standard library, which must check clean.
    pub fn assert_standard_top_clean(src: &str, top: &str) {
        let design = Self::standard_design_of(src);
        if let Err(d) = Types::check(&design, top) {
            panic!("{top}: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        }
    }

    /// The binding named `name`.
    pub fn binding(design: &Design, name: &str) -> DeclId {
        Self::declaration(design, name, |k| matches!(k, DeclKind::Binding(_)))
    }

    /// The binding named `name` of the root crate, whatever the standard library binds under the same name.
    pub fn root_binding(design: &Design, name: &str) -> DeclId {
        design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_)) && d.krate().is_root()).unwrap_or_else(|| panic!("no binding `{name}`")).id()
    }

    /// The type of each of the locals `names` of the clean check of the source of `items` and `body` (see [`CheckedSource::source`]).
    pub fn local_types(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
        Self::local_types_of(&Self::source(items, body), names)
    }

    /// The type of each of the locals `names` of the clean check of `src`.
    pub fn local_types_of(src: &str, names: &[&str]) -> Vec<Ty> {
        Self::types_of_locals(&Self::design_of(src), src, names)
    }

    /// The same over the embedded standard library.
    pub fn standard_local_types(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
        let src = Self::source(items, body);
        Self::types_of_locals(&Self::standard_design_of(&src), &src, names)
    }

    /// The source of `items` and `body` and the diagnostics of checking it over the embedded standard library, which must not check clean.
    pub fn standard_body_diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
        let src = Self::source(items, body);
        let found = Self::standard_expected_diagnostics(&src);
        (src, found)
    }

    /// The type of each of the locals `names` in the clean check of `design`, built from `src`.
    fn types_of_locals(design: &Design, src: &str, names: &[&str]) -> Vec<Ty> {
        let types = Types::check(design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", Self::messages(&d)));
        names.iter().map(|n| types.local(Self::binding(design, n)).unwrap_or_else(|| panic!("no type for `{n}`")).clone()).collect()
    }

    /// The expression whose source text is exactly `text` (the first one).
    pub fn expression_of(design: &Design, src: &str, text: &str) -> ExprId {
        (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| &src[design.expr_node(*e).span.range()] == text).unwrap_or_else(|| panic!("no expression `{text}`"))
    }

    /// Whether `expr` is written in `top.msl`, the source under test, and not in the standard library.
    pub fn in_top(design: &Design, expr: ExprId) -> bool {
        design.files()[design.decl_data(design.expr_node(expr).owner).file.0 as usize].name() == "top.msl"
    }

    /// The reached expressions of `top.msl` that read as `text`, in source order.
    pub fn expressions_reading(design: &Design, types: &Types, src: &str, text: &str) -> Vec<ExprId> {
        (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && Self::in_top(design, *e) && src[design.expr_node(*e).span.range()] == *text).collect()
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

    /// The checked `Top` of the design of `src`, which must check clean.
    pub fn checked<'d>(design: &'d Design, src: &str) -> Types<'d> {
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

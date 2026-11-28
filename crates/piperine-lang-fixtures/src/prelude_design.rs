//! A design over the embedded standard library and the reads of its diagnostics and member expressions the `pom` targets share.

use piperine_lang::pom::{Design, DesignDiagnostic, ExprId, ExprKind, Reflect, Resolution};

/// A design of one source over the embedded standard library, and what the pom tests read back from it.
pub struct PreludeDesign;

impl PreludeDesign {
    /// A design of `src` over the embedded standard library.
    pub fn with_prelude(src: &str) -> Design {
        Design::builder().source("top.msl", src).build()
    }

    /// The diagnostics of one code.
    pub fn with_code<'a>(design: &'a Design, code: &str) -> Vec<&'a DesignDiagnostic> {
        design.diagnostics().iter().filter(|d| d.diagnostic.code == code).collect()
    }

    /// The authored text of the primary span of a diagnostic.
    pub fn primary_text<'a>(design: &'a Design, d: &DesignDiagnostic) -> &'a str {
        let file = &design.files()[d.file.index()];
        &file.text()[d.diagnostic.span().range()]
    }

    /// What the member expression with this text resolves to (a field access or a method call).
    pub fn member_resolution(design: &Design, text: &str) -> Resolution {
        let e = (0..design.expr_count())
            .map(|i| design.expr(ExprId(i as u32)))
            .find(|e| e.text() == text && !e.enclosing().file().krate().is_std() && matches!(e.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }))
            .unwrap_or_else(|| panic!("member expression `{text}`"));
        match e.kind() {
            ExprKind::Field { res, .. } | ExprKind::MethodCall { res, .. } => *res,
            _ => unreachable!(),
        }
    }
}

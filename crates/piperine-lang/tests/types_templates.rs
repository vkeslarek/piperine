//! Scope: templates as values (generics phase D, GN-21 and GN-22; `docs/spec` §8.4): a template
//! `Name { params }` is a value of its module's type wherever a value goes (`E0319` for a missing
//! required param, `E0362` outside a const context, `E0112` as a carried type), a placement
//! through a template or a function result types as its module (`E0361` for a param block), and
//! `C: Fn(P)` holds for a module whose ports match.

use piperine_lang::pom::{Design, DesignDiagnostic, ExprId};
use piperine_lang::types::{Ty, Types};

/// The natures and the two modules every source of this file shares.
const PRELUDE: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
module Cell(a: E) { cs: f64 = 1.0; }\n\
module Sizing(a: E) { size: f64; }\n";

/// A source checked from its top module.
struct TemplateSource {
    /// The whole text.
    text: String,
    /// The elaborated design.
    design: Design,
}

impl TemplateSource {
    /// `PRELUDE` followed by `body`, parsed and resolved without a diagnostic.
    fn new(body: &str) -> Self {
        let text = format!("{PRELUDE}{body}");
        let design = Design::builder().source("top.msl", text.clone()).build();
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{text}", design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>());
        Self { text, design }
    }

    /// The checker's diagnostics for `Top`, as codes.
    fn codes(&self) -> Vec<String> {
        match Types::check(&self.design, "Top") {
            Ok(_) => Vec::new(),
            Err(found) => Self::codes_of(&found),
        }
    }

    /// The codes of `found`, in order.
    fn codes_of(found: &[DesignDiagnostic]) -> Vec<String> {
        found.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The first diagnostic message of `Top`'s check.
    fn message(&self) -> String {
        Types::check(&self.design, "Top").err().and_then(|found| found.first().map(|d| d.diagnostic.message.clone())).unwrap_or_default()
    }

    /// The type the checker gives the expression whose text is `snippet`.
    fn type_of(&self, snippet: &str) -> Option<Ty> {
        let types = Types::check(&self.design, "Top").unwrap_or_else(|found| panic!("{:?}\n{}", Self::codes_of(&found), self.text));
        (0..self.design.expr_count()).map(|index| ExprId(index as u32)).find(|id| &self.text[self.design.expr_node(*id).span.range()] == snippet).and_then(|id| types.ty(id).cloned())
    }
}

#[test]
fn a_template_is_a_value_of_its_modules_type() {
    // §8.4.1 (GN-21 AC 1): a template goes in a `const`, a function's result and a param value
    let source = TemplateSource::new(
        "const C1: Cell = Cell { cs: 30.0 };\nfn pick(big: bool) -> Cell { if big { Cell { cs: 2.0 } } else { Cell { cs: 1.0 } } }\n\
         module Top(x: E) { cell: Cell = C1; other: Cell = pick(true); }",
    );
    assert_eq!(source.codes(), Vec::<String>::new(), "{}", source.message());
    assert!(matches!(source.type_of("Cell { cs: 30.0 }"), Some(Ty::Module(_))));
}

#[test]
fn a_template_leaving_a_required_param_is_e0319() {
    // §8.4.2 (GN-21 AC 2): every required param is bound in a template
    let source = TemplateSource::new("module Top(x: E) { cell: Sizing = Sizing { }; }");
    assert_eq!(source.codes(), ["E0319"], "{}", source.message());
    assert!(source.message().contains("size"), "{}", source.message());
}

#[test]
fn a_template_that_is_not_const_is_e0362() {
    // §8.4.6 (GN-21 AC 3): a template-valued expression is a const expression
    let source = TemplateSource::new("module Top(x: E) { let t = Cell { cs: v(x) }; }");
    assert_eq!(source.codes(), ["E0362"], "{}", source.message());
}

#[test]
fn a_template_is_never_carried_e0112() {
    // §8.4.1 (GN-21 AC 4): a template has no `Repr`, so it is never carried
    let source = TemplateSource::new("module Top(x: E) { signal s: Bit<Cell>; }");
    assert_eq!(source.codes(), ["E0112"], "{}", source.message());
}

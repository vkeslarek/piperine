//! POM errors — [`ElabError`] (elaboration failures) and [`ReflectError`]
//! (reflection-layer failures: navigation, staging).

use thiserror::Error;

use crate::elab::const_eval::ConstEvalError;

/// An error raised while elaborating a [`SourceFile`][crate::parse::SourceFile]
/// into a [`Design`][super::Design].
#[derive(Debug, Error, miette::Diagnostic)]
#[error("{kind}")]
pub struct ElabError {
    #[source]
    #[diagnostic_source]
    pub kind: ElabErrorKind,
    #[label("here")]
    pub span: Option<miette::SourceSpan>,
}

impl ElabError {
    pub fn new(kind: ElabErrorKind) -> Self {
        Self { kind, span: None }
    }
    pub fn with_span(mut self, span: Option<miette::SourceSpan>) -> Self {
        if self.span.is_none() {
            self.span = span;
        }
        self
    }
}

impl From<ElabErrorKind> for ElabError {
    fn from(kind: ElabErrorKind) -> Self {
        Self::new(kind)
    }
}

/// An error raised while elaborating a [`SourceFile`][crate::parse::SourceFile]
/// into a [`Design`][super::Design].
#[derive(Debug, Error, miette::Diagnostic)]
pub enum ElabErrorKind {
    /// Constant evaluation failed in a given context.
    #[error("const eval error in `{context}`: {source}")]
    #[diagnostic(code(E2001))]
    ConstEval { context: String, #[source] source: ConstEvalError },
    /// A referenced type name was not found.
    #[error("undefined type: `{0}`")]
    #[diagnostic(code(E2002))]
    UndefinedType(String),
    /// A referenced module name was not found.
    #[error("undefined module: `{0}`")]
    #[diagnostic(code(E2003))]
    UndefinedModule(String),
    /// A bundle contains non-net fields and cannot be used as a net.
    #[error("bundle `{0}` is not net-capable (contains non-net fields)")]
    #[diagnostic(code(E2004))]
    NotNetCapable(String),
    /// A contribution (`<+`) was used inside a digital block.
    #[error("contribution `<+` is not allowed in a digital block")]
    #[diagnostic(code(E2005))]
    ContribInDigital,
    /// A contribution (`<+`) was used inside a `mod` body.
    #[error("contribution `<+` is not allowed in a mod body")]
    #[diagnostic(code(E2006))]
    ContribInModBody,
    /// A force (`<-`) was used inside a `mod` body.
    #[error("force `<-` is not allowed in a mod body")]
    #[diagnostic(code(E2007))]
    ForceInModBody,
    /// An event kind name was not recognized.
    #[error("unknown event kind: `{0}`")]
    #[diagnostic(code(E2008))]
    UnknownEvent(String),
    /// An analog-only event was used in a digital block.
    #[error("analog-only event `{0}` used inside a digital block")]
    #[diagnostic(code(E2009))]
    AnalogEventInDigital(String),
    /// A digital-only event was used in an analog block.
    #[error("digital-only event `{0}` used inside an analog block")]
    #[diagnostic(code(E2010))]
    DigitalEventInAnalog(String),
    /// A required const param was not provided for a module instance.
    #[error("const param `{param}` not provided for module `{module}`")]
    #[diagnostic(code(E2011))]
    MissingConstParam { param: String, module: String },
    /// An expression could not be reduced to a net reference.
    #[error("expression cannot be reduced to a net reference: {0}")]
    #[diagnostic(code(E2012))]
    NotANetRef(String),
    /// GAPS §B.1 — two nets connected in a `Module`'s `connections` list
    /// have mismatched widths (e.g. `Bit[8]` connected to `Bit[4]`).
    #[error("width mismatch in `{module}`: {lhs} ({lhs_w}) ↔ {rhs} ({rhs_w})")]
    #[diagnostic(code(E2013))]
    WidthMismatch {
        module: String,
        lhs: String,
        rhs: String,
        lhs_w: u64,
        rhs_w: u64,
    },
    /// GAPS §B.2 — two nets connected in a `Module`'s `connections` list
    /// have mismatched disciplines (e.g. `Electrical` connected to
    /// `Thermal`). The §10 no-magic rule requires an explicit converter.
    #[error("discipline crossing `{lhs}` ↔ `{rhs}` in module `{module}` requires an explicit converter (§10)")]
    #[diagnostic(code(E2014))]
    DisciplineCrossing {
        module: String,
        lhs: String,
        rhs: String,
    },
    /// GAPS §I.14 — a `param` declared with a bundle type names a bundle
    /// that was never declared.
    #[error("unknown bundle `{0}`")]
    #[diagnostic(code(E2015))]
    UnknownBundle(String),
    /// GAPS §I.14 — a bundle literal used as a `param` default (or an
    /// instance override) names a field the bundle doesn't have.
    #[error("bundle `{bundle}` has no field `{field}`")]
    #[diagnostic(code(E2016))]
    BundleFieldUnknown { bundle: String, field: String },
    /// GAPS §I.14 — a bundle-typed `param`'s default must be a bundle
    /// literal of the same bundle type (`Foo {}` or `Foo { .f = e, .. }`).
    #[error("bundle param `{param}` default must be a `{expected}` literal, found {found}")]
    #[diagnostic(code(E2017))]
    BundleParamDefault { param: String, expected: String, found: String },
    /// GAPS §I.14 — a bundle field has no default and no override was
    /// given, so the flattened scalar param has no value to fall back to.
    #[error("bundle field `{bundle}.{field}` has no default and was not overridden in param `{param}`")]
    #[diagnostic(code(E2018))]
    BundleFieldNoDefault { param: String, bundle: String, field: String },
    /// GAPS §I.14 — a module both declares a bundle-typed `param` and an
    /// explicit scalar `param` whose name collides with the flattened
    /// `{param}_{field}` naming convention.
    #[error("param `{0}` collides with a flattened bundle field name")]
    #[diagnostic(code(E2019))]
    BundleParamNameCollision(String),
    /// GAPS §B.4 — two or more drivers on a net without a resolve clause.
    #[error("multiple drivers on net `{net}` in module `{module}` (discipline `{discipline}` does not resolve)")]
    #[diagnostic(code(E2020))]
    MultipleDrivers {
        module: String,
        net: String,
        discipline: String,
    },
    /// A catch-all for other elaboration errors.
    #[error("{0}")]
    #[diagnostic(code(E2999))]
    Other(String),
}

/// An error raised by the reflection layer — navigating a [`Selection`][super::Selection]
/// or writing a [`Param`][super::Param] via the staging layer.
///
/// Mirrors `docs/reflection_api.md` §6: `NotFound | NotSettable | TypeMismatch | OutOfRange`.
#[derive(Debug, Clone, PartialEq, Error, miette::Diagnostic)]
pub enum ReflectError {
    /// The requested node or attribute was not found.
    #[error("not found: {0}")]
    #[diagnostic(code(E3001))]
    NotFound(String),
    /// The attribute exists but is not writable via the staging layer.
    #[error("attribute is not settable: {0}")]
    #[diagnostic(code(E3002))]
    NotSettable(String),
    /// The value type does not match the attribute's expected type.
    #[error("type mismatch: {0}")]
    #[diagnostic(code(E3003))]
    TypeMismatch(String),
    /// An index or value was outside the allowable range.
    #[error("out of range: {0}")]
    #[diagnostic(code(E3004))]
    OutOfRange(String),
    /// GAPS §B.4 — two or more drivers on a net without a resolve clause.
    #[error("multiple drivers on net `{net}` in module `{module}` (discipline `{discipline}` does not resolve)")]
    #[diagnostic(code(E3005))]
    MultipleDrivers {
        module: String,
        net: String,
        discipline: String,
    },
    /// A catch-all for other reflection errors.
    #[error("{0}")]
    #[diagnostic(code(E3999))]
    Other(String),
}

/// An error parsing or evaluating a selector.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SelectorError {
    #[error("Empty selector")]
    EmptySelector,
    #[error("Expected `::` after axis")]
    ExpectedDoubleColon,
    #[error("Expected NodeTest")]
    ExpectedNodeTest,
    #[error("Unknown axis: {0}")]
    UnknownAxis(String),
    #[error("Axis {0:?} not yet implemented")]
    AxisNotImplemented(crate::pom::selector::ast::Axis),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect_error_display() {
        let e = ReflectError::NotFound("module `foo`".into());
        assert!(e.to_string().contains("foo"));
        let e = ReflectError::NotSettable("name".into());
        assert!(e.to_string().contains("settable"));
    }
}

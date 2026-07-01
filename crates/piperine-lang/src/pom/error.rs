//! POM errors — [`ElabError`] (elaboration failures) and [`ReflectError`]
//! (reflection-layer failures: navigation, staging).

use thiserror::Error;

use crate::elab::const_eval::ConstEvalError;

/// An error raised while elaborating a [`SourceFile`][crate::parse::SourceFile]
/// into a [`Design`][super::Design].
#[derive(Debug, Error)]
pub enum ElabError {
    /// Constant evaluation failed in a given context.
    #[error("const eval error in `{context}`: {source}")]
    ConstEval { context: String, #[source] source: ConstEvalError },
    /// A referenced type name was not found.
    #[error("undefined type: `{0}`")]
    UndefinedType(String),
    /// A referenced module name was not found.
    #[error("undefined module: `{0}`")]
    UndefinedModule(String),
    /// A bundle contains non-net fields and cannot be used as a net.
    #[error("bundle `{0}` is not net-capable (contains non-net fields)")]
    NotNetCapable(String),
    /// A contribution (`<+`) was used inside a digital block.
    #[error("contribution `<+` is not allowed in a digital block")]
    ContribInDigital,
    /// A contribution (`<+`) was used inside a `mod` body.
    #[error("contribution `<+` is not allowed in a mod body")]
    ContribInModBody,
    /// A force (`<-`) was used inside a `mod` body.
    #[error("force `<-` is not allowed in a mod body")]
    ForceInModBody,
    /// An event kind name was not recognized.
    #[error("unknown event kind: `{0}`")]
    UnknownEvent(String),
    /// An analog-only event was used in a digital block.
    #[error("analog-only event `{0}` used inside a digital block")]
    AnalogEventInDigital(String),
    /// A digital-only event was used in an analog block.
    #[error("digital-only event `{0}` used inside an analog block")]
    DigitalEventInAnalog(String),
    /// A required const param was not provided for a module instance.
    #[error("const param `{param}` not provided for module `{module}`")]
    MissingConstParam { param: String, module: String },
    /// An expression could not be reduced to a net reference.
    #[error("expression cannot be reduced to a net reference: {0}")]
    NotANetRef(String),
    /// A catch-all for other elaboration errors.
    #[error("{0}")]
    Other(String),
}

/// An error raised by the reflection layer — navigating a [`Selection`][super::Selection]
/// or writing a [`Param`][super::Param] via the staging layer.
///
/// Mirrors `docs/reflection_api.md` §6: `NotFound | NotSettable | TypeMismatch | OutOfRange`.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReflectError {
    /// The requested node or attribute was not found.
    #[error("not found: {0}")]
    NotFound(String),
    /// The attribute exists but is not writable via the staging layer.
    #[error("attribute is not settable: {0}")]
    NotSettable(String),
    /// The value type does not match the attribute's expected type.
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    /// An index or value was outside the allowable range.
    #[error("out of range: {0}")]
    OutOfRange(String),
    /// A catch-all for other reflection errors.
    #[error("{0}")]
    Other(String),
}

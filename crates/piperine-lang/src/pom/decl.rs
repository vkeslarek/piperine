//! Declarations: one node per declared thing or structural container, in one
//! arena. The payload of each kind is its own struct; fields are added by the
//! task that lowers that kind.

use serde::{Deserialize, Serialize};

use super::ids::{DeclId, ExprId, FileId, PatId, TypeId};
use super::body::Block;
use crate::msl::Span;

/// An identifier as written, with its range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

/// One declaration. `children` is in source order and complete: every
/// declaration lexically inside this one, at the first level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclData {
    pub kind: DeclKind,
    pub name: Option<Name>,
    pub parent: Option<DeclId>,
    pub children: Vec<DeclId>,
    pub file: FileId,
    pub span: Span,
    pub public: bool,
}

/// What a declaration is, with its payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeclKind {
    Namespace(NamespaceData),
    Use(UseData),
    Const(ConstData),
    Fn(FnData),
    FnParam(FnParamData),
    Struct(StructData),
    Field(FieldData),
    Enum(EnumData),
    Variant(VariantData),
    Trait(TraitData),
    Impl(ImplData),
    GenericParam(GenericParamData),
    Nature(NatureData),
    Quantity(QuantityData),
    Module(ModuleData),
    Port(PortData),
    Param(ParamData),
    Signal(SignalData),
    Let(LetData),
    Binding(BindingData),
    Definition(DefinitionData),
    Generation(GenerationData),
    Behavior(BehaviorData),
    Reaction(ReactionData),
    /// The span of a malformed item.
    Error,
}

impl DeclKind {
    /// The variant's name, as `dump()` prints it.
    pub fn label(&self) -> &'static str {
        match self {
            DeclKind::Namespace(_) => "Namespace",
            DeclKind::Use(_) => "Use",
            DeclKind::Const(_) => "Const",
            DeclKind::Fn(_) => "Fn",
            DeclKind::FnParam(_) => "FnParam",
            DeclKind::Struct(_) => "Struct",
            DeclKind::Field(_) => "Field",
            DeclKind::Enum(_) => "Enum",
            DeclKind::Variant(_) => "Variant",
            DeclKind::Trait(_) => "Trait",
            DeclKind::Impl(_) => "Impl",
            DeclKind::GenericParam(_) => "GenericParam",
            DeclKind::Nature(_) => "Nature",
            DeclKind::Quantity(_) => "Quantity",
            DeclKind::Module(_) => "Module",
            DeclKind::Port(_) => "Port",
            DeclKind::Param(_) => "Param",
            DeclKind::Signal(_) => "Signal",
            DeclKind::Let(_) => "Let",
            DeclKind::Binding(_) => "Binding",
            DeclKind::Definition(_) => "Definition",
            DeclKind::Generation(_) => "Generation",
            DeclKind::Behavior(_) => "Behavior",
            DeclKind::Reaction(_) => "Reaction",
            DeclKind::Error => "Error",
        }
    }
}

/// Where a namespace comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamespaceOrigin {
    /// The top of a file.
    Root,
    /// `mod a { ... }`
    Inline,
    /// `mod a;`: `file` is the loaded file, none when no file was loaded.
    External { file: Option<FileId> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamespaceData {
    pub origin: NamespaceOrigin,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct UseData {}

/// `const NAME: ty = value;`; the value is absent in a trait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstData {
    pub ty: TypeId,
    pub value: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FnData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FnParamData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StructData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FieldData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EnumData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VariantData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TraitData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImplData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GenericParamData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NatureData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct QuantityData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ModuleData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PortData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ParamData {}

/// `signal name: ty = init;`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalData {
    pub ty: TypeId,
    pub init: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LetData {}

/// One identifier a pattern introduces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingData {
    /// The pattern that introduces it (the whole struct pattern for the
    /// shorthand `S { x }`).
    pub pat: PatId,
    pub origin: BindingOrigin,
}

/// The syntactic place a binding comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingOrigin {
    /// A module-body `let`.
    BodyLet,
    /// A `let` statement in a block.
    Local,
    ClosureParam,
    MatchArm,
    For,
    /// `if let` / `while let`.
    IfLet,
    /// The pattern of a generation `for`.
    GenFor,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DefinitionData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GenerationData {}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BehaviorData {}

/// `on event { ... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReactionData {
    pub event: ExprId,
    pub body: Block,
}

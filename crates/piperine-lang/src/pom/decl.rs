//! Declarations: one node per declared thing or structural container, in one
//! arena. The payload of each kind is its own struct; fields are added by the
//! task that lowers that kind.

use serde::{Deserialize, Serialize};

use super::body::{Block, BehaviorKind, FieldInit, Lit, QuantityRole};
use super::resolution::{NatureKind, PortKind};
use super::meta::Meta;
use super::ids::{DeclId, ExprId, FileId, PatId, PathId, StmtId, TypeId};
use crate::msl::Span;
pub use crate::msl::ast::Dir;

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
    /// The range of the `<..>` list when the declaration wrote one (even an
    /// empty one); its parameters are `GenericParam` children.
    pub generics_span: Option<Span>,
    pub public: bool,
    pub meta: Meta,
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

/// `use a::{b, c as d, e::*};`: the tree as written, and the same imports
/// flattened to one entry per leaf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UseData {
    pub tree: UseTreeNode,
    pub imports: Vec<Import>,
}

/// One node of a `use` tree: the path written at this level and what follows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UseTreeNode {
    pub path: Vec<Name>,
    pub kind: UseTreeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UseTreeKind {
    Simple { alias: Option<Name> },
    Glob,
    Group(Vec<UseTreeNode>),
}

/// One leaf of a `use` tree with its full path. The path node spans from the
/// first written segment to the last, so for a group leaf its text includes
/// the braces in between.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    pub path: PathId,
    pub alias: Option<Name>,
    pub glob: bool,
}

/// `const NAME: ty = value;`; the value is absent in a trait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstData {
    pub ty: TypeId,
    pub value: Option<ExprId>,
}

/// `fn name<..>(params) -> ret where cond { body }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FnData {
    pub is_const: bool,
    /// `extern "C" fn`: the ABI string.
    pub abi: Option<Lit>,
    pub ret: Option<TypeId>,
    pub where_: Option<ExprId>,
    /// None for a declaration ending in `;`.
    pub body: Option<Block>,
}

/// One parameter of a function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FnParamData {
    /// `self`
    SelfValue,
    Named { ty: TypeId },
    /// A malformed parameter already reported by a diagnostic.
    Error,
}

/// How a struct or a variant lists its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldsForm {
    /// `{ a: T }`
    Named,
    /// `(T, U)`
    Tuple,
    /// no fields
    Unit,
}

/// `struct S<..> { fields }`; the fields are children after the generics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructData {
    pub form: FieldsForm,
    /// Classification slot: none until the classification pass sets it.
    pub bundle: Option<bool>,
}

/// `dir name: ty`; the name is absent in a tuple field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldData {
    pub dir: Option<Dir>,
    pub ty: TypeId,
}

/// `enum E<..> { variants }`; the variants are children after the generics.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EnumData {}

/// `Name`, `Name(T)`, `Name { f: T }`, with an optional `= discriminant`;
/// the fields are children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantData {
    pub form: FieldsForm,
    pub discriminant: Option<ExprId>,
}

/// `trait Name<..>: Super { items }`; generics and items are children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitData {
    pub supertraits: Vec<PathId>,
}

/// `impl<..> Trait for Type { items }`; generics and items are children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplData {
    pub trait_: Option<PathId>,
    pub self_ty: TypeId,
}

/// `<T: Bound = Default>` or `<const N: u32 = 4>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenericParamData {
    Type { bounds: Vec<PathId>, default: Option<TypeId> },
    Const { ty: TypeId, default: Option<ExprId> },
}

/// `nature Name<..> { potential: Voltage { .. }, flow: Current { .. }, value: T, resolve: f }`;
/// generics and quantities are children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NatureData {
    /// The fields in source order, repeated and mixed ones included.
    pub fields: Vec<NatureFieldNode>,
    /// Computed from the declared fields.
    pub kind: NatureKind,
}

/// One field of a nature, as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NatureFieldNode {
    /// `potential: Voltage { .. }` or `flow: ..`: the `Quantity` declaration.
    Quantity(DeclId),
    /// `value: T`; `span` covers the whole field.
    Value { ty: TypeId, span: Span },
    /// `resolve: expr`; `span` covers the whole field.
    Resolve { expr: ExprId, span: Span },
}

/// `potential: Voltage { access: v, units: "V" }`: the attribute list as
/// written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityData {
    pub role: QuantityRole,
    pub attrs: Vec<FieldInit>,
}

/// `[extern "abi"] module Name<..>(ports) -> ret where cond { items result }`;
/// generics, ports and body items are children, in that order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleData {
    pub abi: Option<Lit>,
    pub ret: Option<TypeId>,
    pub where_: Option<ExprId>,
    /// The trailing expression: the module's result.
    pub result: Option<ExprId>,
    /// The behavior blocks bound to this module, in source order; filled by
    /// name resolution.
    pub behaviors: Vec<DeclId>,
    /// Every call of this module's body and blocks classified as an
    /// instance, in source order; filled by classification. An instance is a
    /// view over the call, not a node.
    pub instances: Vec<ExprId>,
}

/// `dir name: ty = default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortData {
    pub dir: Option<Dir>,
    pub ty: TypeId,
    pub default: Option<ExprId>,
    /// Classification slot: unknown until the classification pass sets it.
    pub kind: PortKind,
}

/// `name: ty = default where cond;`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamData {
    pub ty: TypeId,
    pub default: Option<ExprId>,
    pub where_: Option<ExprId>,
}

/// `signal name: ty = init;`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalData {
    pub ty: TypeId,
    pub init: Option<ExprId>,
}

/// `let pat = value;` in a module body; the bindings of the pattern are
/// children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LetData {
    pub pat: PatId,
    pub value: ExprId,
}

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

/// `place = value after delay;`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefinitionData {
    pub place: ExprId,
    pub value: ExprId,
    pub after: Option<ExprId>,
}

/// `for pat in iter { items }` or `if cond { items } else ..` over module or
/// digital items, held as written: nothing is unrolled. The items are
/// children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenerationData {
    /// The bindings of `pat` are children before the items.
    For { pat: PatId, iter: ExprId, body: Vec<DeclId> },
    If { cond: ExprId, then: Vec<DeclId>, else_: Option<GenElseNode> },
}

/// What follows `else` in a generation `if`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenElseNode {
    Block(Vec<DeclId>),
    /// `else if ..`: the nested `Generation`, a child of this one.
    If(DeclId),
}

/// `analog<N> for Target { .. }` or `digital for Target { .. }`; an analog
/// block's bindings and items, a digital block's reactions and generations,
/// are children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehaviorData {
    pub kind: BehaviorKind,
    pub names: Vec<Name>,
    pub target: PathId,
    /// The statements of an `analog` block; empty for `digital`.
    pub body: Vec<StmtId>,
    /// The module of the same file the target names; filled by name
    /// resolution, none when the target names no usable module.
    pub module: Option<DeclId>,
}

/// `on event { ... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReactionData {
    pub event: ExprId,
    pub body: Block,
}

//! Declarations: disciplines, natures, modules and their items.
//!
//! Mirrors `veriloga.ungram` from OpenVAF-Reloaded exactly, plus structural
//! `InstanceDecl` (needed for Piperine testbench netlists — not in OpenVAF's
//! pure-analog scope but unambiguous to parse).

use super::*;

#[derive(Debug, Clone)]
pub struct DisciplineDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub items: Vec<DisciplineAttr>,
}

#[derive(Debug, Clone)]
pub struct DisciplineAttr {
    pub name: Path,
    pub val: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct NatureDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub parent: Option<Path>,
    pub items: Vec<NatureAttr>,
}

#[derive(Debug, Clone)]
pub struct NatureAttr {
    pub name: Name,
    pub val: Expr,
}

#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub ports: Option<Vec<ModulePort>>,
    pub items: Vec<ModuleItem>,
}

/// ungram: `ModuleItem = BodyPortDecl | NetDecl | AnalogBehaviour | Function
///                       | BranchDecl | VarDecl | ParamDecl | AliasParam`
/// Plus `Instance` (Piperine structural extension).
#[derive(Debug, Clone)]
pub enum ModuleItem {
    BodyPortDecl(BodyPortDecl),
    NetDecl(NetDecl),
    AnalogBehaviour(AnalogBehaviour),
    Function(Function),
    BranchDecl(BranchDecl),
    VarDecl(VarDecl),
    ParamDecl(ParamDecl),
    AliasParam(AliasParam),
    Instance(InstanceDecl),
    InitialBlock(InitialBlock),
}

/// Structural instantiation: `ModuleType [#(params)] inst_name (conns);`
#[derive(Debug, Clone)]
pub struct InstanceDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub module: Name,
    pub name: Name,
    pub range: Option<BitRange>,
    pub params: Vec<Connection>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone)]
pub enum Connection {
    Positional(Expr),
    Named { port: Name, expr: Option<Expr> },
}

/// ungram: `Direction = 'inout' | 'input' | 'output'`
#[derive(Debug, Clone)]
pub enum Direction {
    Inout,
    Input,
    Output,
}

/// ungram: `PortDecl = AttrList* Direction discipline:NameRef? 'net_type'? (Name (',' Name)*)`
#[derive(Debug, Clone)]
pub struct PortDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub dir: Direction,
    pub discipline: Option<NameRef>,
    pub range: Option<BitRange>,
    pub names: Vec<Declarator>,
}

/// ungram: `ModulePort = PortDecl | Name`
#[derive(Debug, Clone)]
pub enum ModulePort {
    PortDecl(PortDecl),
    Name(Name),
}

/// ungram: `AnalogBehaviour = AttrList* 'analog' 'initial'? Stmt`
#[derive(Debug, Clone)]
pub struct AnalogBehaviour {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub initial: bool,
    pub stmt: Box<Stmt>,
}

/// ungram: `VarDecl = AttrList* Type (Var (',' Var)*) ';'`
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub ty: Type,
    pub vars: Vec<Var>,
}

#[derive(Debug, Clone)]
pub struct Var {
    pub name: Name,
    pub range: Option<BitRange>,
    pub default: Option<Expr>,
}

/// ungram: `ParamDecl = AttrList* ('parameter'|'localparam') Type? (Param (',' Param)*) ';'`
#[derive(Debug, Clone)]
pub struct ParamDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub kind: ParamKind,
    pub ty: Option<Type>,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub enum ParamKind {
    Parameter,
    LocalParam,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Name,
    pub default: Expr,
    pub constraints: Vec<Constraint>,
}

/// ungram: `AliasParam = AttrList* 'aliasparam' name:Name '=' src:ParamRef ';'`
#[derive(Debug, Clone)]
pub struct AliasParam {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub src: ParamRef,
}

#[derive(Debug, Clone)]
pub enum ParamRef {
    Path(Path),
    SysFun(String),
}

/// ungram: `Constraint = ('from' | 'exclude') (Expr | Range)`
#[derive(Debug, Clone)]
pub enum Constraint {
    From(ConstraintValue),
    Exclude(ConstraintValue),
}

#[derive(Debug, Clone)]
pub enum ConstraintValue {
    Expr(Expr),
    Range(Range),
    Array(Vec<Expr>),
}

/// ungram: `Range = ('(' | '[') start:Expr ':' end:Expr (')' | ']')`
#[derive(Debug, Clone)]
pub struct Range {
    pub inclusive_left: bool,
    pub start: Expr,
    pub end: Expr,
    pub inclusive_right: bool,
}

/// ungram: `NetDecl = AttrList* discipline:NameRef? 'net_type'? (Name (',' Name)*) ';'`
#[derive(Debug, Clone)]
pub struct NetDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub discipline: Option<NameRef>,
    pub range: Option<BitRange>,
    pub names: Vec<Declarator>,
}

/// ungram: `BodyPortDecl = PortDecl ';'`
#[derive(Debug, Clone)]
pub struct BodyPortDecl {
    pub span: Span,
    pub port: PortDecl,
}

/// ungram: `Function = AttrList* 'analog' 'function' Type? Name ';' FunctionItem* 'endfunction'`
#[derive(Debug, Clone)]
pub struct Function {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub ty: Option<Type>,
    pub name: Name,
    pub items: Vec<FunctionItem>,
}

/// ungram: `FunctionItem = ParamDecl | VarDecl | FunctionArg | Stmt`
#[derive(Debug, Clone)]
pub enum FunctionItem {
    ParamDecl(ParamDecl),
    VarDecl(VarDecl),
    FunctionArg(FunctionArg),
    Stmt(Stmt),
}

/// ungram: `FunctionArg = AttrList* Direction (Name (',' Name)*) ';'`
#[derive(Debug, Clone)]
pub struct FunctionArg {
    pub attrs: Vec<Attr>,
    pub dir: Direction,
    pub names: Vec<Name>,
}

/// ungram: `BranchDecl = AttrList* 'branch' ArgList (Name (',' Name)*) ';'`
#[derive(Debug, Clone)]
pub struct BranchDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub ports: Vec<Expr>,
    pub names: Vec<Name>,
}

/// Top-level `extern module name(ports; parameters);` declaration.
/// Ports and parameters separated by `;` (or mixed with `,` — parser accepts both).
#[derive(Debug, Clone)]
pub struct ExternModuleDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub ports: Vec<PortDecl>,
    pub parameters: Vec<ExternParameter>,
}

/// One parameter in an `extern module` declaration.
#[derive(Debug, Clone)]
pub struct ExternParameter {
    pub name: Name,
    pub ty: Type,
    pub default: Option<Expr>,
}

/// `initial begin BlockItem* end` — testbench procedural block.
#[derive(Debug, Clone)]
pub struct InitialBlock {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub stmt: Box<Stmt>,
}

//! Ergonomic, flattened model of a Verilog-A source file.
//!
//! Every syntactic feature the parser recognises is surfaced here so downstream
//! consumers never need to walk the raw AST.

use crate::ast;

pub type Span = ast::Span;
pub type BitRange = ast::BitRange;

#[derive(Debug, Clone, Default)]
pub struct Document {
    pub modules:        Vec<Module>,
    pub disciplines:    Vec<Discipline>,
    pub natures:        Vec<Nature>,
    pub paramsets:      Vec<Paramset>,
    pub connectrules:   Vec<ast::ConnectrulesDecl>,
    pub configs:        Vec<ast::ConfigDecl>,
    pub primitives:     Vec<ast::PrimitiveDecl>,
}

/// A `(* name = value *)` attribute.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub value: Option<ast::Expr>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub ports: Vec<Port>,
    pub parameters: Vec<Parameter>,
    pub aliasparams: Vec<AliasParam>,
    pub nets: Vec<Net>,
    pub variables: Vec<Variable>,
    pub branches: Vec<Branch>,
    pub functions: Vec<Function>,
    pub tasks: Vec<Task>,
    pub analog_blocks: Vec<AnalogBlock>,
    pub instances: Vec<Instance>,
    pub ground_decls: Vec<ast::GroundDecl>,
    pub events: Vec<ast::EventDecl>,
    pub defparams: Vec<ast::DefparamDecl>,
    pub continuous_assigns: Vec<ast::ContinuousAssign>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Port {
    pub name: String,
    pub direction: ast::Direction,
    pub discipline: Option<String>,
    pub range: Option<BitRange>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub is_local: bool,
    pub ty: Option<ast::Type>,
    pub default_value: ast::Expr,
    pub constraints: Vec<ast::Constraint>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AliasParam {
    pub name: String,
    pub source: ParamSource,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ParamSource {
    Path(String),
    SysFun(String),
}

#[derive(Debug, Clone)]
pub struct Net {
    pub members: Vec<NetMember>,
    pub discipline: Option<String>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

/// A single net in a declaration with its effective range.
#[derive(Debug, Clone)]
pub struct NetMember {
    pub name: String,
    pub range: Option<BitRange>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub ty: ast::Type,
    pub range: Option<BitRange>,
    pub default_value: Option<ast::Expr>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AnalogBlock {
    pub is_initial: bool,
    pub stmt: ast::Stmt,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}


#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub return_type: Option<ast::Type>,
    pub args: Vec<FunctionArg>,
    pub variables: Vec<Variable>,
    pub parameters: Vec<Parameter>,
    pub body: Vec<ast::Stmt>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionArg {
    pub name: String,
    pub direction: ast::Direction,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub names: Vec<String>,
    pub ports: Vec<ast::Expr>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}


#[derive(Debug, Clone)]
pub struct Discipline {
    pub name: String,
    pub attributes: Vec<DisciplineAttr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DisciplineAttr {
    pub name: String,
    pub value: Option<ast::Expr>,
}

#[derive(Debug, Clone)]
pub struct Nature {
    pub name: String,
    pub parent: Option<String>,
    pub attributes: Vec<NatureAttr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct NatureAttr {
    pub name: String,
    pub value: ast::Expr,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub automatic: bool,
    pub ports: Vec<ast::TaskPort>,
    pub variables: Vec<Variable>,
    pub body: ast::Stmt,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub module_name: String,
    pub instance_name: String,
    pub range: Option<BitRange>,
    pub param_assignments: Vec<ast::ParamAssignment>,
    pub connections: Vec<ast::PortConnection>,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone)]
pub struct Paramset {
    pub name: String,
    pub base: String,
    pub parameters: Vec<Parameter>,
    pub aliasparams: Vec<AliasParam>,
    pub variables: Vec<Variable>,
    pub statements: Vec<ast::ParamsetStmt>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}


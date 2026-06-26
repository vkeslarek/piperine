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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Module,
    Macromodule,
    Connectmodule,
}

#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub kind: ModuleKind,
    pub name: Name,
    pub param_ports: Vec<ParamDecl>,
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
    TaskDecl(TaskDecl),
    BranchDecl(BranchDecl),
    VarDecl(VarDecl),
    ParamDecl(ParamDecl),
    AliasParam(AliasParam),
    GroundDecl(GroundDecl),
    EventDecl(EventDecl),
    ModuleInstantiation(ModuleInstantiation),
    Defparam(DefparamDecl),
    ContinuousAssign(ContinuousAssign),
    InitialConstruct { span: Span, attrs: Vec<Attr>, stmt: Box<Stmt> },
    AlwaysConstruct { span: Span, attrs: Vec<Attr>, stmt: Box<Stmt> },
    Generate(GenerateRegion),
    LoopGenerate(LoopGenerate),
    IfGenerate(IfGenerate),
    CaseGenerate(CaseGenerate),
    Specify(SpecifyBlock),
    Specparam(SpecparamDecl),
    GateInstantiation(GateInstantiation),
}


/// ungram: `Direction = 'inout' | 'input' | 'output'`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub net_type: Option<NetType>,
    pub var_type: Option<Type>,
    pub signed: bool,
    pub discipline: Option<NameRef>,
    pub range: Option<BitRange>,
    pub names: Vec<Declarator>,
}

/// ungram: `ModulePort = PortDecl | Name`
#[derive(Debug, Clone)]
pub enum ModulePort {
    PortDecl(PortDecl),
    Name(Name),
    NamedExternal { port: Name, expr: Option<PortExpr> },
}

#[derive(Debug, Clone)]
pub enum PortExpr {
    Ref { name: Name, range: Option<BitRange> },
    Concat(Vec<(Name, Option<BitRange>)>),
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
    pub signed: bool,
    pub packed_range: Option<BitRange>,
    pub discipline: Option<Name>,
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
    pub signed: bool,
    pub range: Option<BitRange>,
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
    pub net_type: Option<NetType>,
    pub drive_strength: Option<DriveStrength>,
    pub charge_strength: Option<ChargeStrength>,
    pub delay: Option<Delay>,
    pub ty: Option<Type>,
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
    pub automatic: bool,
    pub ty: Option<Type>,
    pub signed: bool,
    pub range: Option<BitRange>,
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

// ==========================================
// Phase 2 / Phase 6 / Phase 7 Extensions
// ==========================================

#[derive(Debug, Clone)]
pub struct GroundDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub discipline: Option<Name>,
    pub range: Option<BitRange>,
    pub names: Vec<Declarator>,
}

#[derive(Debug, Clone)]
pub struct EventDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub names: Vec<Declarator>,
}

#[derive(Debug, Clone)]
pub enum NetType {
    Wire, Wand, Wor, Tri, Triand, Trior,
    Supply0, Supply1, Tri0, Tri1, Uwire, Trireg,
    Wreal,
}

#[derive(Debug, Clone)]
pub enum Strength {
    Supply0, Strong0, Pull0, Weak0,
    Supply1, Strong1, Pull1, Weak1,
    Highz0, Highz1,
}

#[derive(Debug, Clone)]
pub struct DriveStrength {
    pub strength0: Strength,
    pub strength1: Strength,
}

#[derive(Debug, Clone)]
pub enum ChargeStrength {
    Small, Medium, Large,
}

#[derive(Debug, Clone)]
pub enum Delay {
    Single(Expr),                           // #5 or #ident
    Paren1(Expr),                           // #(expr)      — mintypmax
    Paren2(Expr, Expr),                     // #(rise, fall)
    Paren3(Expr, Expr, Expr),               // #(rise, fall, turnoff)
}

// ==========================================
// Phase 3 & 4 Extensions
// ==========================================

#[derive(Debug, Clone)]
pub struct ModuleInstantiation {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub module_name: Name,                      // the module/paramset being instantiated
    pub param_assignments: Vec<ParamAssignment>, // #(...) parameter overrides
    pub instances: Vec<ModuleInstance>,           // one or more instances
}

#[derive(Debug, Clone)]
pub struct ModuleInstance {
    pub name: Name,
    pub range: Option<BitRange>,      // optional array of instances [3:0]
    pub connections: Vec<PortConnection>,
}

#[derive(Debug, Clone)]
pub enum PortConnection {
    Ordered(Option<Expr>),                     // positional: expr or empty
    Named { port: Name, expr: Option<Expr> },  // .port_name(expr) or .port_name()
    Wildcard,                                  // .*  — auto-connect by name
}

#[derive(Debug, Clone)]
pub enum ParamAssignment {
    Ordered(Expr),
    Named { param: Name, expr: Expr },
    SystemNamed { param: String, expr: Expr },  // .$param_name(expr) — system params
}

#[derive(Debug, Clone)]
pub struct DefparamDecl {
    pub span: Span,
    pub assignments: Vec<(Path, Expr)>,  // hierarchical_param_id = const_expr
}

#[derive(Debug, Clone)]
pub struct ContinuousAssign {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub drive_strength: Option<DriveStrength>,  // optional (strong0, pull1) etc.
    pub delay: Option<Delay>,                   // optional #delay
    pub assignments: Vec<(Expr, Expr)>,          // (lvalue, rvalue) pairs
}

#[derive(Debug, Clone)]
pub struct ParamsetDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub base: Name,
    pub item_decls: Vec<ParamsetItemDecl>,
    pub statements: Vec<ParamsetStmt>,
}

#[derive(Debug, Clone)]
pub enum ParamsetItemDecl {
    Parameter(ParamDecl),
    LocalParameter(ParamDecl),
    AliasParam(AliasParam),
    IntegerDecl(VarDecl),
    RealDecl(VarDecl),
}

#[derive(Debug, Clone)]
pub enum ParamsetStmt {
    DotAssign { name: Name, value: Expr },
    SysDotAssign { name: String, value: Expr },
    AnalogStmt(Box<Stmt>),
}

#[derive(Debug, Clone)]
pub struct ConnectrulesDecl {
    pub span: Span,
    pub name: Name,
    pub items: Vec<ConnectrulesItem>,
}

#[derive(Debug, Clone)]
pub enum ConnectrulesItem {
    Insertion {
        module: Name,
        mode: Option<ConnectMode>,
        params: Vec<ParamAssignment>,
        port_overrides: Option<ConnectPortOverrides>,
    },
    Resolution {
        disciplines: Vec<Name>,
        target: ResolveTarget,
    },
}

#[derive(Debug, Clone)]
pub enum ConnectMode { Merged, Split }

#[derive(Debug, Clone)]
pub enum ResolveTarget { Discipline(Name), Exclude }

#[derive(Debug, Clone)]
pub struct ConnectPortOverrides {
    pub input_disc: Option<Name>,
    pub output_disc: Option<Name>,
}

#[derive(Debug, Clone)]
pub struct ConfigDecl {
    pub span: Span,
    pub name: Name,
    pub design: Vec<ConfigCellRef>,
    pub rules: Vec<ConfigRule>,
}

#[derive(Debug, Clone)]
pub struct ConfigCellRef {
    pub library: Option<Name>,
    pub cell: Name,
}

#[derive(Debug, Clone)]
pub enum ConfigRule {
    Default(LiblistOrUse),
    Inst { path: Vec<Name>, clause: LiblistOrUse },
    Cell { cell_ref: ConfigCellRef, clause: LiblistOrUse },
}

#[derive(Debug, Clone)]
pub enum LiblistOrUse {
    Liblist(Vec<Name>),
    Use { cell_ref: ConfigCellRef, config: bool },
}

// ==========================================
// Phase 5 Extensions
// ==========================================

#[derive(Debug, Clone)]
pub struct GenerateRegion {
    pub span: Span,
    pub items: Vec<ModuleItem>,
}

#[derive(Debug, Clone)]
pub struct LoopGenerate {
    pub span: Span,
    pub init: (Name, Expr),
    pub condition: Expr,
    pub iteration: (Name, Expr),
    pub body: GenerateBlock,
}

#[derive(Debug, Clone)]
pub enum GenerateBlock {
    Single(Box<ModuleItem>),
    Block { label: Option<Name>, items: Vec<ModuleItem> },
}

#[derive(Debug, Clone)]
pub struct IfGenerate {
    pub span: Span,
    pub condition: Expr,
    pub then_block: GenerateBlock,
    pub else_block: Option<GenerateBlock>,
}

#[derive(Debug, Clone)]
pub struct CaseGenerate {
    pub span: Span,
    pub condition: Expr,
    pub items: Vec<CaseGenerateItem>,
}

#[derive(Debug, Clone)]
pub struct CaseGenerateItem {
    pub exprs: Vec<Expr>, // Empty means default
    pub block: GenerateBlock,
}

// ==========================================
// Phase 8 & 9 Extensions
// ==========================================

#[derive(Debug, Clone)]
pub struct SpecifyBlock {
    pub span: Span,
    pub item_count: usize,
}

#[derive(Debug, Clone)]
pub struct SpecparamDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub range: Option<BitRange>,
    pub assignments: Vec<(Name, Expr)>,
}

#[derive(Debug, Clone)]
pub struct GateInstantiation {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub gate_type: Name,
    pub instances: Vec<GateInstance>,
}

#[derive(Debug, Clone)]
pub struct GateInstance {
    pub name: Option<(Name, Option<BitRange>)>,
    pub terminals: Vec<Option<Expr>>,
}

#[derive(Debug, Clone)]
pub struct PrimitiveDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub ports: Vec<Name>,
    pub port_decls: Vec<PortDecl>,
    pub body: UdpBody,
}

#[derive(Debug, Clone)]
pub enum UdpBody {
    Combinational(Vec<UdpEntry>),
    Sequential { initial: Option<(Name, String)>, entries: Vec<UdpEntry> },
}

#[derive(Debug, Clone)]
pub struct UdpEntry {
    pub inputs: Vec<String>,
    pub current_state: Option<String>,
    pub next_state: String,
}

#[derive(Debug, Clone)]
pub struct TaskDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub automatic: bool,
    pub name: Name,
    pub ports: Vec<TaskPort>,  // from parenthesized form; empty for old-style
    pub items: Vec<TaskItem>,
    pub body: Box<Stmt>,
}

#[derive(Debug, Clone)]
pub enum TaskItem {
    BlockItem(BlockItem),   // reg/integer/real/event decl or statement
    Port(TaskPort),         // input/output/inout declaration (old-style body)
}

#[derive(Debug, Clone)]
pub struct TaskPort {
    pub attrs: Vec<Attr>,
    pub dir: Direction,
    pub port_type: Option<Type>,   // integer|real|realtime|time
    pub discipline: Option<NameRef>,
    pub reg: bool,
    pub signed: bool,
    pub range: Option<BitRange>,
    pub names: Vec<Name>,
}


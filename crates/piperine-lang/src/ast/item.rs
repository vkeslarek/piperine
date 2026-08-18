//! Declaration AST — natures, disciplines, modules and everything a module
//! body can contain, plus the top-level paramset/connectrules/config/
//! primitive (UDP) forms. LRM Annex A.1-A.5, A.7, A.9.

use crate::ast::{Attr, Expr, Stmt};
use crate::error::PreprocessedSpan;

// ─────────────────────────── nature / discipline ──────────────────────────

/// `nature name [: parent] { nature_item* } endnature` — §3.6.1, Annex A.1.6.
/// Not attribute-bearing: `nature_declaration` is absent from LRM Syntax
/// 2-5..2-10's attribute-instance sites.
#[derive(Debug, Clone, PartialEq)]
pub struct NatureDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub name: String,
    /// `nature Voltage : Potential;` — a derived nature, §3.6.1.1.
    pub parent: Option<String>,
    /// The five standard attributes, §3.6.1.2 — named, not map entries,
    /// mirroring the POM's own `Nature` type (design.md §5.1) so the
    /// elaborator's translation is a straight field copy.
    pub abstol: Option<Expr>,
    pub access: Option<String>,
    pub units: Option<Expr>,
    pub idt_nature: Option<String>,
    pub ddt_nature: Option<String>,
    /// User-defined attributes (§3.6.1.3) — anything not one of the five
    /// standard ones above.
    pub user_attrs: Vec<(String, Expr)>,
}

/// `domain continuous` / `domain discrete` — §3.6.2, part of a discipline
/// body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Continuous,
    Discrete,
}

/// `discipline name { discipline_item* } enddiscipline` — §3.6.2,
/// Annex A.1.5. Not attribute-bearing (same reasoning as `NatureDecl`).
#[derive(Debug, Clone, PartialEq)]
pub struct DisciplineDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub name: String,
    pub domain: Option<Domain>,
    pub potential: Option<String>,
    pub flow: Option<String>,
}

// ───────────────────────────────── module ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Module,
    Macromodule,
    Connectmodule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
    Inout,
}

/// A `[msb:lsb]` bit/array range.
#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub span: PreprocessedSpan,
    pub msb: Expr,
    pub lsb: Expr,
}

/// A declared identifier with an optional per-name range — `bus[3:0]` in
/// `electrical bus[3:0], scalar;`.
#[derive(Debug, Clone, PartialEq)]
pub struct Declarator {
    pub name: String,
    pub range: Option<Range>,
    /// A variable initializer — Annex A.2.2.1's `variable_type ::=
    /// variable_identifier = constant_expression`. Always `None` for nets
    /// and ports (their productions carry no initializer; the grammar
    /// leaves this empty outside `parse_var_decl`).
    pub init: Option<crate::ast::Expr>,
}

/// A port declaration, either in the module's own port-declaration list
/// style or re-declared in the body — §3.7/Annex A.1.3/A.1.4.
#[derive(Debug, Clone, PartialEq)]
pub struct PortDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub attrs: Vec<Attr>,
    pub dir: Direction,
    pub discipline: Option<String>,
    pub names: Vec<Declarator>,
}

/// A net declaration inside a module body — §3.6.3/3.7.
#[derive(Debug, Clone, PartialEq)]
pub struct NetDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub attrs: Vec<Attr>,
    pub discipline: Option<String>,
    /// `ground` nets are represented via [`GroundDecl`], not here.
    pub names: Vec<Declarator>,
    /// A discipline initial ("nodeset") value, §3.6.3.2.
    pub nodeset: Option<Expr>,
}

/// `ground name;` — §3.6.4.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundDecl {
    pub span: PreprocessedSpan,
    pub name: String,
}

/// `branch (pos[,neg]) name1 [, name2 ...];` — §5.2, Annex A.1.4.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub pos: String,
    pub neg: Option<String>,
    pub names: Vec<String>,
}

/// A scalar variable declaration (`integer`/`real`/`time`/`realtime`) at
/// module scope — §3.2, Annex A.2.1.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    Integer,
    Real,
    Time,
    Realtime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub attrs: Vec<Attr>,
    pub kind: VarKind,
    pub names: Vec<Declarator>,
}

/// `event name;` — §Annex A.2.1.3.
#[derive(Debug, Clone, PartialEq)]
pub struct EventDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub names: Vec<String>,
}

/// `genvar name;` — §6, Annex A.2.1.3.
#[derive(Debug, Clone, PartialEq)]
pub struct GenvarDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub names: Vec<String>,
}

// ────────────────────────────── parameters ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    Integer,
    Real,
    Realtime,
    Time,
    String,
}

/// A parameter value-range constraint, `from`/`exclude` — §3.4.2.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamRange {
    pub span: PreprocessedSpan,
    pub excluded: bool,
    pub lo: Option<Expr>,
    pub lo_exclusive: bool,
    pub hi: Option<Expr>,
    pub hi_exclusive: bool,
}

/// One `name [dims] = expr` parameter declarator/initializer pair — Annex
/// A.2.4 `param_assignment`'s array alternative
/// (`parameter_identifier range constant_arrayinit`) generalized to zero
/// or more `[msb:lsb]` dimensions, alongside the plain scalar alternative
/// (`dims` empty). §3.4.4 (one dimension) and §3.4.8 (multidimensional,
/// e.g. `parameter real c[0:2][0:2] = ...;`) — outermost dimension first.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamAssignment {
    pub name: String,
    pub dims: Vec<Range>,
    pub value: Expr,
}

/// `parameter`/`localparam` declaration — §3.4, Annex A.2.1.1.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub attrs: Vec<Attr>,
    pub is_local: bool,
    pub ty: Option<ParamType>,
    pub names: Vec<ParamAssignment>,
    pub ranges: Vec<ParamRange>,
}

/// `aliasparam name = target;` — §3.4.7.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasParam {
    pub span: PreprocessedSpan,
    pub name: String,
    pub target: String,
}

/// `specparam name = expr;` — 1364 timing-check parameters, carried
/// verbatim by Verilog-AMS. Attribute-bearing, §2.9 Syntax 2-7.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecparamDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub names: Vec<(String, Expr)>,
}

/// `defparam target = expr;` — §Annex A.2.1.2.
#[derive(Debug, Clone, PartialEq)]
pub struct Defparam {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub target: String,
    pub value: Expr,
}

// ─────────────────────────── functions / tasks ─────────────────────────────

/// A `function`/`analog function`/`task` formal port — §5.9/Annex A.2.6/A.2.7.
#[derive(Debug, Clone, PartialEq)]
pub struct PortItemDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub dir: Direction,
    pub ty: Option<ParamType>,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    /// `function` — a general (digital-context) function.
    Digital,
    /// `analog function` — §5.9, callable from analog blocks.
    Analog,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub kind: FunctionKind,
    pub name: String,
    pub ret: Option<ParamType>,
    pub ports: Vec<PortItemDecl>,
    /// Local variable declarations between the port list and the single
    /// body statement — `function_item_declaration`'s non-port forms,
    /// Annex A.2.6.
    pub decls: Vec<crate::ast::BlockItemDecl>,
    pub body: Box<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub name: String,
    pub ports: Vec<PortItemDecl>,
    /// Local variable declarations between the port list and the single
    /// body statement — `task_item_declaration`'s non-port forms,
    /// Annex A.2.7.
    pub decls: Vec<crate::ast::BlockItemDecl>,
    pub body: Box<Stmt>,
}

// ───────────────────────── behavior / structural ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorKind {
    /// `analog stmt` / `analog begin ... end`.
    Analog,
    /// `analog initial stmt` — runs once, §5.7.
    AnalogInitial,
    Initial,
    Always,
}

/// `analog ...` / `analog initial ...` / `initial ...` / `always ...` —
/// Annex A.1.4/A.6.2. All four are attribute-bearing, §2.9 Syntax 2-7.
#[derive(Debug, Clone, PartialEq)]
pub struct BehaviorConstruct {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub kind: BehaviorKind,
    pub body: Box<Stmt>,
}

/// `constraint begin … end` — a **sanctioned Piperine extension** (Annex A
/// has no constraint production; MD-49 records the deviation and its
/// precedents). A module body item mirroring `analog begin … end`: one
/// more module body, so ports, parameters, and branches are simply in
/// scope for every [`RequireDecl`] predicate inside it. `constraint`,
/// `require`, and `tol` are contextual keywords — matched by identifier
/// text at their production sites only, never added to Annex B's reserved
/// set (§2.8.2's escaped-identifier escape hatch keeps ordinary uses of
/// the words working).
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintBlockDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above the block, if any.
    pub doc: Option<String>,
    pub attrs: Vec<Attr>,
    pub requires: Vec<RequireDecl>,
}

/// `require <name> : <predicate> [tol <scale>] ;` — one named per-point
/// constraint inside a [`ConstraintBlockDecl`]. The margin convention
/// (`m ≥ 0` ⟺ satisfied) is imposed by the codegen lowering, not the
/// grammar; `tol` names the normalization scale (default 1, no implicit
/// normalization).
#[derive(Debug, Clone, PartialEq)]
pub struct RequireDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this statement,
    /// if any.
    pub doc: Option<String>,
    pub name: String,
    pub pred: Expr,
    pub tol: Option<Expr>,
}

/// `assign lvalue = expr;` — a continuous assignment, §Annex A.6.1.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuousAssign {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub target: Expr,
    pub value: Expr,
}

/// One port connection at an instantiation site — ordered (positional) or
/// named. Attribute-bearing, §2.9 Syntax 2-9.
#[derive(Debug, Clone, PartialEq)]
pub struct PortConnection {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub name: Option<String>,
    pub value: Option<Expr>,
}

/// One instance in a `module_identifier inst1(...), inst2(...);` list —
/// §6.3, Annex A.4.1.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub span: PreprocessedSpan,
    pub name: String,
    /// `name_of_module_instance ::= module_instance_identifier [ range ]`
    /// — Annex A.4.1/§6.2.2: an array-of-instances range (`buf8 [7:0]
    /// (...)`, LRMC-C5-08). `None` for an ordinary single instance.
    pub range: Option<Range>,
    pub connections: Vec<PortConnection>,
}

/// `module_name #(param_overrides) inst(...), ...;` — module/UDP/gate
/// instantiation, Annex A.4.1.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleInstantiation {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub module_name: String,
    pub param_overrides: Vec<PortConnection>,
    pub instances: Vec<Instance>,
}

// ───────────────────────────────── generate ────────────────────────────────

/// A generate-block body — zero or more module/generate items, optionally
/// named (`begin : label`) — §6.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateBlock {
    pub span: PreprocessedSpan,
    pub name: Option<String>,
    pub items: Vec<ModuleItem>,
}

/// `for (genvar = init; cond; step) generate_block` — §6.2.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopGenerate {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub genvar: String,
    pub init: Expr,
    pub cond: Expr,
    pub step_var: String,
    pub step_value: Expr,
    pub body: GenerateBlock,
}

/// `if (cond) generate_block [else generate_block]` — §6.3.
#[derive(Debug, Clone, PartialEq)]
pub struct IfGenerate {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub cond: Expr,
    pub then_branch: GenerateBlock,
    pub else_branch: Option<GenerateBlock>,
}

/// One `case_generate` arm.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateCaseArm {
    pub span: PreprocessedSpan,
    pub labels: Option<Vec<Expr>>,
    pub body: GenerateBlock,
}

/// `case (selector) case_generate_item* endcase` — §6.4.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseGenerate {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub selector: Expr,
    pub arms: Vec<GenerateCaseArm>,
}

/// `generate module_or_generate_item* endgenerate` — a bare generate
/// region (no loop/conditional), §6.1.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRegion {
    pub span: PreprocessedSpan,
    pub items: Vec<ModuleItem>,
}

// ─────────────────────────────── specify / gates ───────────────────────────

/// `specify specify_item* endspecify` — 1364 timing specification,
/// preserved as an opaque item list (its internals are a P3/VAMS-22
/// concern, not this task's scope).
#[derive(Debug, Clone, PartialEq)]
pub struct SpecifyBlock {
    pub span: PreprocessedSpan,
    pub items: Vec<Stmt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GateKind {
    And, Nand, Or, Nor, Xor, Xnor,
    Buf, Not,
    Bufif0, Bufif1, Notif0, Notif1,
    Nmos, Pmos, Cmos, Rnmos, Rpmos, Rcmos,
    Pullup, Pulldown,
    Tran, Rtran, Tranif0, Tranif1, Rtranif0, Rtranif1,
}

/// One gate instance, `[name] (terminals)`.
#[derive(Debug, Clone, PartialEq)]
pub struct GateInstance {
    pub span: PreprocessedSpan,
    pub name: Option<String>,
    pub terminals: Vec<Expr>,
}

/// A gate-primitive instantiation — §Annex A.4.2.
#[derive(Debug, Clone, PartialEq)]
pub struct GateInstantiation {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub kind: GateKind,
    pub strength0: Option<String>,
    pub strength1: Option<String>,
    pub delay: Option<Expr>,
    pub instances: Vec<GateInstance>,
}

// ─────────────────────────────────── UDP ───────────────────────────────────

/// `output [reg] name [= init];` / `input names;` / `reg name;` — a UDP
/// port declaration, §Annex A.5.2. Attribute-bearing, §2.9 Syntax 2-10.
#[derive(Debug, Clone, PartialEq)]
pub enum UdpPortDecl {
    Output { span: PreprocessedSpan, attrs: Vec<Attr>, name: String, is_reg: bool, init: Option<Expr> },
    Input { span: PreprocessedSpan, attrs: Vec<Attr>, names: Vec<String> },
    Reg { span: PreprocessedSpan, attrs: Vec<Attr>, name: String },
}

/// One row of a UDP state table, §Annex A.5.3 — kept as raw symbol text
/// (`0`/`1`/`x`/`?`/`-`/`*`/`b`/`r`/`f`/`p`/`n`), interpretation is an
/// elaboration/codegen concern (VAMS-19).
#[derive(Debug, Clone, PartialEq)]
pub struct UdpTableRow {
    pub span: PreprocessedSpan,
    pub inputs: Vec<String>,
    pub current_state: Option<String>,
    pub next_state: String,
}

/// `primitive name (ports); port_decl* [reg name;] table row* endtable
/// endprimitive` — §Annex A.5.1.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveDecl {
    pub span: PreprocessedSpan,
    pub attrs: Vec<Attr>,
    pub name: String,
    pub ports: Vec<UdpPortDecl>,
    pub table: Vec<UdpTableRow>,
    pub is_sequential: bool,
}

// ───────────────────────── paramset / connectrules / config ────────────────

/// `paramset name (module_name) { paramset_item* } endparamset` — a named,
/// reusable parameter set (not covered by §2.9's attribute sites).
#[derive(Debug, Clone, PartialEq)]
pub struct ParamsetDecl {
    pub span: PreprocessedSpan,
    pub name: String,
    pub module_name: String,
    pub params: Vec<ParamDecl>,
    /// `.module_parameter_identifier = paramset_constant_expression;`
    /// statements (§6.4, Syntax 6-4's `paramset_statement`'s first
    /// alternative) — binds a value to a parameter of the associated
    /// module (`module_name`). The other three `paramset_statement`
    /// alternatives (module output variable, system parameter,
    /// `analog_function_statement`) are out of LRMC-C5-07's scope.
    pub statements: Vec<ParamsetStatement>,
}

/// One `.field = expr;` binding inside a `paramset` body — see
/// [`ParamsetDecl::statements`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParamsetStatement {
    pub span: PreprocessedSpan,
    pub target: String,
    pub value: Expr,
}

/// `connect_mode`, §7.7.1 Syntax 7-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectMode {
    Merged,
    Split,
}

/// `connect_port_overrides`, §7.7.1 Syntax 7-6 — the four alternatives,
/// each a discipline pair with an optional per-side direction qualifier.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectPortOverrides {
    /// `discipline_identifier , discipline_identifier`
    Both(String, String),
    /// `input discipline_identifier , output discipline_identifier`
    InputOutput(String, String),
    /// `output discipline_identifier , input discipline_identifier`
    OutputInput(String, String),
    /// `inout discipline_identifier , inout discipline_identifier`
    InOut(String, String),
}

/// One rule inside a `connectrules` block, LRM chapter 7 / Annex F.
///
/// Two forms coexist here: the legacy bare `connect A_discipline,
/// B_discipline;` shell (T11) — `connectmodule: None`, `disciplines` holds
/// the pair, `mode`/`params`/`overrides` empty — used by
/// [`super::super::elab::lower::connect::eligible_bridge`]'s discipline-pair
/// eligibility check; and the real `connect_insertion` production
/// (§7.7.1 Syntax 7-6, T31/LRMC-C7-02): `connect connectmodule_identifier
/// [connect_mode] [parameter_value_assignment] [connect_port_overrides] ;`
/// — `connectmodule: Some(name)`, with `disciplines` still populated from
/// `overrides` (either side, flattened) so `eligible_bridge`'s pair lookup
/// works unchanged for both forms.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectRule {
    pub span: PreprocessedSpan,
    pub connectmodule: Option<String>,
    pub disciplines: Vec<String>,
    pub mode: Option<ConnectMode>,
    pub params: Vec<PortConnection>,
    pub overrides: Option<ConnectPortOverrides>,
    pub body: Vec<Stmt>,
}

/// `connect_resolution ::= connect discipline_identifier {,
/// discipline_identifier} resolveto discipline_identifier_or_exclude ;`
/// — §7.7.2 Syntax 7-7 (LRMC-C7-04/C10-20, T33).
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectResolutionTarget {
    Discipline(String),
    Exclude,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectResolution {
    pub span: PreprocessedSpan,
    pub disciplines: Vec<String>,
    pub target: ConnectResolutionTarget,
}

/// `connectrules name { (connect_rule | connect_resolution)* }
/// endconnectrules` — chapter 7. The two statement kinds share the
/// `connect` keyword but are otherwise disjoint productions (a
/// `connect_resolution` always ends in `resolveto`, a `connect_rule`
/// never does), so they're kept in separate lists rather than one
/// mixed-kind enum vec.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectrulesDecl {
    pub span: PreprocessedSpan,
    pub name: String,
    pub rules: Vec<ConnectRule>,
    pub resolutions: Vec<ConnectResolution>,
}

/// `config name; design ...; endconfig` — 1364 configuration declarations,
/// carried by Verilog-AMS Annex A.1.1. Kept as raw body text; configuration
/// semantics are out of this feature's scope (not cited by any spec
/// requirement) beyond parsing so a source using one is not rejected.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDecl {
    pub span: PreprocessedSpan,
    pub name: String,
    pub body: String,
}

// ───────────────────────────────── module item ─────────────────────────────

/// Any item that can appear inside a module body — §Annex A.1.4.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleItem {
    Port(PortDecl),
    Net(NetDecl),
    Ground(GroundDecl),
    Branch(BranchDecl),
    Var(VarDecl),
    Event(EventDecl),
    Genvar(GenvarDecl),
    Param(ParamDecl),
    AliasParam(AliasParam),
    Specparam(SpecparamDecl),
    Defparam(Defparam),
    Function(FunctionDecl),
    Task(TaskDecl),
    Behavior(BehaviorConstruct),
    Constraint(ConstraintBlockDecl),
    ContinuousAssign(ContinuousAssign),
    Instantiation(ModuleInstantiation),
    LoopGenerate(LoopGenerate),
    IfGenerate(IfGenerate),
    CaseGenerate(CaseGenerate),
    GenerateRegion(GenerateRegion),
    Specify(SpecifyBlock),
    Gate(GateInstantiation),
}

/// `module`/`macromodule`/`connectmodule` — §Annex A.1.2. Attribute-bearing,
/// §2.9 Syntax 2-5.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub span: PreprocessedSpan,
    /// Attached `///` doc comment run immediately above this
    /// declaration, if any (LSP hover renders it as Markdown).
    pub doc: Option<String>,
    pub attrs: Vec<Attr>,
    pub kind: ModuleKind,
    pub name: String,
    pub param_ports: Vec<ParamDecl>,
    /// `Some` when the port list is the declaration-list style
    /// (`module m(input electrical a, ...)`); `None` when it is the
    /// name-list style (`module m(a, b); input electrical a; ...`).
    pub ports: Option<Vec<PortDecl>>,
    pub items: Vec<ModuleItem>,
}

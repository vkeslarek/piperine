//! Shared IR for analog hardware — both PPR and AMS frontends lower into this.
//!
//! The IR is designed to be a superset of both Verilog-AMS (piperine-ams) and
//! PHDL (piperine-lang). It is intentionally simple: an intermediate
//! representation that the codegen lowers to the solver's `Device` trait.

// ─── Types ────────────────────────────────────────────────────────────────────

/// A minimal type system. The IR is mostly untyped (everything evaluates to
/// `f64` in analog context), but some type information is retained for the
/// codegen to distinguish e.g. string parameters from real ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    Real,
    Integer,
    String,
    Bool,
    /// 4-state logic: 0, 1, X, Z.
    Quad,
    Complex,
    /// No value (tasks / void functions).
    Void,
}

// ─── Expressions ──────────────────────────────────────────────────────────────

/// An expression in the IR.
#[derive(Debug, Clone, PartialEq)]
pub enum IrExpr {
    Real(f64),
    Int(i64),
    /// String literal.
    String(String),
    /// Boolean literal.
    Bool(bool),
    /// 4-state logic literal: 0=0, 1=1, 2=X, 3=Z.
    Quad(u8),
    /// A compile-time parameter (resolved from the parameter map at instantiation).
    Param(String),
    /// A runtime variable (local var, module-level var, or function arg).
    /// Distinct from Param: the codegen must allocate storage / read from env.
    Var(String),
    /// Branch potential/flow access: `V(plus, minus)`, `I(plus, minus)`,
    /// `Pwr(plus, minus)`, `Temp(plus, minus)`, etc.
    /// `access` is the nature's access function name ("V", "I", "Pwr", ...).
    /// Single-arg form `V(a)` becomes `BranchAccess { access: "V", plus: "a", minus: "0" }`.
    BranchAccess { access: String, plus: String, minus: String },
    /// Reference to a state variable (ddt/idt/transition/...) by slot id.
    StateRef(u32),
    /// Simulator query: $temperature, $vt, etc.
    Sim(SimQuery),
    /// A function call: exp, ln, sqrt, pow, user functions, ...
    Call(String, Vec<IrExpr>),
    Binary(IrBinOp, Box<IrExpr>, Box<IrExpr>),
    Unary(IrUnOp, Box<IrExpr>),
    /// Ternary: cond ? then : else
    Select(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),
    /// Concatenation {a, b, c}
    Concat(Vec<IrExpr>),
    /// Replication {n{a, b}} — count followed by the replicated elements.
    Replicate(Box<IrExpr>, Vec<IrExpr>),
    /// Array literal [a, b, c] or '{a, b, c}'
    Array(Vec<IrExpr>),
    /// Array repeat [value; N]
    ArrayRepeat(Box<IrExpr>, Box<IrExpr>),
    /// Array index a[i]
    Index(Box<IrExpr>, Box<IrExpr>),
    /// Slice a[lo..hi] or a[lo..=hi]
    Slice(Box<IrExpr>, Box<IrRange>),
    /// Part-select a[msb:lsb]
    PartSelect(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),
    /// Indexed part-select a[idx +: width] (up=true) or a[idx -: width] (up=false)
    PartSelectIndexed { base: Box<IrExpr>, idx: Box<IrExpr>, width: Box<IrExpr>, up: bool },
    /// Mintypmax (min:typ:max) — typically `typ` is used; all three retained for specparam.
    Mintypmax(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),
    /// Port flow access <port>
    PortFlow(String),
    /// AC stimulus: ac_stim(mag, phase) — only meaningful in AC analysis.
    AcStim { mag: Box<IrExpr>, phase: Box<IrExpr> },
    /// Bundle literal: `Type { .field = expr, ... }`
    BundleLit { ty: String, fields: Vec<(String, IrExpr)> },
    /// Lambda: `|a, b| body`
    Lambda { params: Vec<String>, body: Box<IrExpr> },
}

/// A range with inclusivity flag.
#[derive(Debug, Clone, PartialEq)]
pub struct IrRange {
    pub start: IrExpr,
    pub end: IrExpr,
    pub inclusive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimQuery {
    Temperature,
    Vt(Option<Box<IrExpr>>),
    Abstime,
    Mfactor,
    XPosition,
    YPosition,
    Angle,
    Simparam { key: String, default: Box<IrExpr> },
    /// analysis("dc"), analysis("tran"), analysis("ac"), etc.
    Analysis(String),
    /// $param_given("param_name")
    ParamGiven(String),
    /// $port_connected("port_name")
    PortConnected(String),
    /// $limit(x, "pnjlim", vt, vcrit) — convergence limiting.
    Limit { kind: String, args: Vec<IrExpr> },
    /// $random / $dist_normal / $dist_uniform / ...
    Random { kind: String, args: Vec<IrExpr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOp {
    Add, Sub, Mul, Div, Rem, Pow,
    Eq, Ne, Lt, Le, Gt, Ge,
    /// Logical && ||
    And, Or,
    /// Bitwise & | ^
    BitAnd, BitOr, BitXor,
    /// Shifts << >> (logical) and <<< >>> (arithmetic)
    Shl, Shr, AShl, AShr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrUnOp {
    Neg,
    Not,
    BitNot,
    /// Reduction operators (&, ~&, |, ~|, ^, ~^)
    RedAnd, RedNand, RedOr, RedNor, RedXor, RedXnor,
}

// ─── Statements ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum IrStmt {
    // ── Analog contributions ──
    Contrib {
        nature: IrNature,
        plus: String,
        minus: String,
        expr: IrExpr,
        kind: ContribKind,
    },
    /// Force contribution: I(p,n) <- expr or V(p,n) <- expr (ideal source).
    Force {
        nature: IrNature,
        plus: String,
        minus: String,
        expr: IrExpr,
    },
    /// Indirect branch contribution: `contrib_nature(cp,cm) : probe_nature(pp,pm) = expr`
    IndirectContrib {
        contrib_nature: IrNature,
        contrib_plus: String,
        contrib_minus: String,
        probe_nature: IrNature,
        probe_plus: String,
        probe_minus: String,
        expr: IrExpr,
    },

    // ── Control flow ──
    If {
        cond: IrExpr,
        then_: Vec<IrStmt>,
        else_: Vec<IrStmt>,
        label: Option<String>,
    },
    Case {
        discriminant: IrExpr,
        arms: Vec<(IrExpr, Vec<IrStmt>)>,
        default: Vec<IrStmt>,
        kind: CaseKind,
        label: Option<String>,
    },
    /// Runtime for loop (digital or analog without contributions).
    For {
        var: String,
        start: IrExpr,
        end: IrExpr,
        step: IrExpr,
        body: Vec<IrStmt>,
    },
    /// Runtime while loop (digital).
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    /// Runtime repeat loop (digital).
    Repeat {
        count: IrExpr,
        body: Vec<IrStmt>,
    },
    /// Infinite loop (digital).
    Forever {
        body: Vec<IrStmt>,
    },
    /// Function return.
    Return(Option<IrExpr>),

    // ── Declarations ──
    /// Local variable declaration: `var x: Real = expr;` / `real x = expr;`
    VarDecl {
        name: String,
        ty: IrType,
        init: Option<IrExpr>,
    },

    // ── Digital assignments ──
    /// Non-blocking assignment: lval <= expr [#delay] [@event]
    NonBlocking {
        lval: String,
        expr: IrExpr,
        delay: Option<IrExpr>,
        event: Option<IrEventSpec>,
    },
    /// Blocking assignment to a digital variable: lval = expr [#delay] [@event]
    Assign {
        lval: String,
        expr: IrExpr,
        delay: Option<IrExpr>,
        event: Option<IrEventSpec>,
    },
    /// Continuous assignment: assign lval = expr [#delay]
    ContinuousAssign {
        lval: String,
        expr: IrExpr,
        delay: Option<IrExpr>,
    },
    /// Procedural assign: assign lval = expr / force lval = expr
    ProcAssign {
        lval: String,
        expr: IrExpr,
        is_force: bool,
    },
    /// Procedural deassign: deassign lval / release lval
    ProcDeassign {
        lval: String,
        is_release: bool,
    },

    // ── Timing & events (digital) ──
    /// #delay stmt
    Delay {
        delay: IrExpr,
        body: Box<IrStmt>,
    },
    /// @(event_spec) stmt
    EventControl {
        spec: IrEventSpec,
        body: Box<IrStmt>,
    },
    /// wait(cond) stmt
    Wait {
        cond: IrExpr,
        body: Box<IrStmt>,
    },
    /// fork ... join / join_any / join_none
    Fork {
        label: Option<String>,
        branches: Vec<Vec<IrStmt>>,
        join: JoinKind,
    },
    /// disable label / disable name
    Disable(String),
    /// ->event_name (event trigger)
    Trigger(String),

    // ── Analog events ──
    AnalogEvent {
        kind: IrEventKind,
        body: Vec<IrStmt>,
    },

    // ── Simulator control ──
    BoundStep(IrExpr),
    Finish,
    /// $discontinuity(n)
    Discontinuity(i32),
    Diagnostic {
        severity: Severity,
        format: String,
        args: Vec<IrExpr>,
    },
}

/// Case kind: standard, casex (x don't-care), casez (z don't-care).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Case,
    CaseX,
    CaseZ,
}

/// Fork join kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// join — wait for all
    All,
    /// join_any — wait for any
    Any,
    /// join_none — wait for none
    None,
}

/// Event specification for digital event control.
#[derive(Debug, Clone)]
pub enum IrEventSpec {
    Posedge(IrExpr),
    Negedge(IrExpr),
    Change(IrExpr),
    Cross(IrExpr, i8),
    Above(IrExpr),
    Initial,
    Final,
    Timer(IrExpr),
    Named(String),
    Or(Vec<IrEventSpec>),
}

// ─── Natures & contributions ──────────────────────────────────────────────────

/// The nature of a branch access or contribution: potential or flow.
/// Carries the access function name ("V", "I", "Pwr", "Temp", ...).
/// The codegen uses the kind (Potential vs Flow) to determine the stamping
/// pattern, and the access name to resolve the branch reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrNature {
    /// Potential (across) contribution: `V(p,n) <+ expr`, `Pwr(p,n) <+ expr`, ...
    Potential(String),
    /// Flow (through) contribution: `I(p,n) <+ expr`, ...
    Flow(String),
}

impl IrNature {
    /// Returns the access function name ("V", "I", "Pwr", ...).
    pub fn access(&self) -> &str {
        match self {
            IrNature::Potential(a) | IrNature::Flow(a) => a,
        }
    }
    /// True if this is a potential (across) nature.
    pub fn is_potential(&self) -> bool {
        matches!(self, IrNature::Potential(_))
    }
}

/// Whether a contribution is resistive (DC) or reactive (contains ddt/idt).
#[derive(Debug, Clone, Copy)]
pub enum ContribKind {
    Resistive,
    Reactive(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Fatal,
}

// ─── Analog events ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum IrEventKind {
    InitialStep,
    FinalStep,
    /// cross(expr, dir) / above(expr). dir: 0=either, 1=rising, -1=falling.
    Cross { dir: i8, expr: Option<IrExpr> },
    /// above(expr)
    Above { expr: Option<IrExpr> },
    /// timer(period)
    Timer { period: Option<IrExpr> },
    /// PHDL/Verilog `posedge(signal)` inside a digital block.
    Posedge(IrExpr),
    /// `negedge(signal)`.
    Negedge(IrExpr),
    /// `change(signal)`.
    Change(IrExpr),
}

// ─── State variables (analog operators) ───────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrStateVar {
    pub id: u32,
    pub kind: IrStateKind,
    /// The input expression to the operator.
    pub arg: IrExpr,
}

#[derive(Debug, Clone)]
pub enum IrStateKind {
    /// ddt(x) — time derivative
    Ddt,
    /// idt(x, ic) — time integral
    Idt { ic: IrExpr },
    /// idtmod(x, ic, modulus) — modular integral
    IdtMod { ic: IrExpr, modulus: IrExpr },
    /// ddx(x, node) — derivative w.r.t. node voltage
    Ddx { node: String },
    /// delay(x, t) / absdelay(x, t) — delayed signal
    Delay { delay: IrExpr },
    /// transition(x, td, tr, tf, ttol) — waveform shaping
    Transition { delay: IrExpr, rise: IrExpr, fall: IrExpr, tol: IrExpr },
    /// slew(x, rise, fall) — rate limiting
    Slew { rise: IrExpr, fall: IrExpr },
    /// laplace_np/zp/pm/nm/npm(x, num, den) — Laplace filter
    Laplace { variant: String, num: IrExpr, den: IrExpr },
    /// zi_zd/zp/nd/np(x, num, den, dt) — Z-transform filter
    ZTransform { variant: String, num: IrExpr, den: IrExpr, sample_dt: IrExpr },
    /// cross() / above() as event-detector state
    Cross { dir: i8 },
    /// timer() as event state
    Timer { period: IrExpr },
}

// ─── Noise ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrNoiseSource {
    pub plus: String,
    pub minus: String,
    pub kind: IrNoise,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IrNoise {
    White { psd: IrExpr },
    Flicker { psd: IrExpr, exponent: IrExpr },
}

// ─── Module structure ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrPort {
    pub name: String,
    pub direction: IrDirection,
    pub discipline: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrDirection {
    In,
    Out,
    Inout,
}

#[derive(Debug, Clone)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
    pub default: Option<IrExpr>,
}

#[derive(Debug, Clone)]
pub struct IrWire {
    pub name: String,
    pub discipline: Option<String>,
}

/// A named branch: `branch (p, n) br1;`
#[derive(Debug, Clone)]
pub struct IrBranch {
    pub name: String,
    pub plus: String,
    pub minus: String,
}

/// An event declaration: `event e;`
#[derive(Debug, Clone)]
pub struct IrEventDecl {
    pub name: String,
}

/// A port connection: named (.port(net)) or positional.
#[derive(Debug, Clone)]
pub struct IrConnection {
    /// Port name for named connections; None for positional.
    pub port: Option<String>,
    pub net: String,
}

#[derive(Debug, Clone)]
pub struct IrInstance {
    pub label: String,
    pub module: String,
    pub connections: Vec<IrConnection>,
    pub params: Vec<(String, IrExpr)>,
}

/// A module-level variable declaration: `real x;` / `integer i;`
#[derive(Debug, Clone)]
pub struct IrVarDecl {
    pub name: String,
    pub ty: IrType,
    pub init: Option<IrExpr>,
}

/// A ground declaration: `ground gnd;`
#[derive(Debug, Clone)]
pub struct IrGroundDecl {
    pub name: String,
    pub discipline: Option<String>,
}

/// A net connection (aliasing): `lhs = rhs;`
#[derive(Debug, Clone)]
pub struct IrConnectionDecl {
    pub lhs: String,
    pub rhs: String,
}

// ─── Digital body ─────────────────────────────────────────────────────────────

/// A digital behavior body.
#[derive(Debug, Clone)]
pub struct IrDigitalBody {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    /// State variables (regs/latches) declared in the digital block.
    pub state_vars: Vec<IrVarDecl>,
    pub stmts: Vec<IrStmt>,
}

// ─── Analog body ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrAnalogBody {
    pub state_vars: Vec<IrStateVar>,
    pub noise_sources: Vec<IrNoiseSource>,
    /// Local variables declared in the analog block (module-level + block-local).
    pub vars: Vec<IrVarDecl>,
    pub stmts: Vec<IrStmt>,
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// A user-defined function or task. Functions return a value; tasks do not
/// (Return(None) or no Return). The body uses the same IrStmt/IrExpr as
/// everywhere else. Calls remain opaque `IrExpr::Call(name, args)` — the
/// codegen resolves user functions vs built-ins at compile time.
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    /// Positional parameter names.
    pub params: Vec<String>,
    pub body: Vec<IrStmt>,
}

// ─── Module ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub ports: Vec<IrPort>,
    pub params: Vec<IrParam>,
    pub wires: Vec<IrWire>,
    pub branches: Vec<IrBranch>,
    pub events: Vec<IrEventDecl>,
    /// Module-level variable declarations.
    pub vars: Vec<IrVarDecl>,
    /// Ground declarations.
    pub grounds: Vec<IrGroundDecl>,
    pub instances: Vec<IrInstance>,
    /// Net connections (aliasing): `lhs = rhs;`
    pub connections: Vec<IrConnectionDecl>,
    /// Continuous assigns: `assign lval = expr;`
    pub continuous_assigns: Vec<IrStmt>,
    pub analog: Option<IrAnalogBody>,
    pub digital: Option<IrDigitalBody>,
    /// Functions/tasks defined in this module.
    pub functions: Vec<IrFunction>,
}

// ─── Program ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrProgram {
    /// "ppr" or "ams"
    pub source: String,
    pub modules: Vec<IrModule>,
    /// Global (file-level) functions/tasks.
    pub functions: Vec<IrFunction>,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Walk an IrExpr looking for the first StateRef id.
pub fn first_state_ref(expr: &IrExpr) -> Option<u32> {
    match expr {
        IrExpr::StateRef(id) => Some(*id),
        IrExpr::Binary(_, l, r) => first_state_ref(l).or_else(|| first_state_ref(r)),
        IrExpr::Unary(_, e) => first_state_ref(e),
        IrExpr::Call(_, args) => args.iter().find_map(first_state_ref),
        IrExpr::Select(c, t, e) => {
            first_state_ref(c).or_else(|| first_state_ref(t)).or_else(|| first_state_ref(e))
        }
        IrExpr::Concat(exprs) | IrExpr::Array(exprs) => exprs.iter().find_map(first_state_ref),
        IrExpr::Replicate(count, exprs) => {
            first_state_ref(count).or_else(|| exprs.iter().find_map(first_state_ref))
        }
        IrExpr::ArrayRepeat(v, n) => first_state_ref(v).or_else(|| first_state_ref(n)),
        IrExpr::Index(b, i) => first_state_ref(b).or_else(|| first_state_ref(i)),
        IrExpr::Slice(b, r) => {
            first_state_ref(b)
                .or_else(|| first_state_ref(&r.start))
                .or_else(|| first_state_ref(&r.end))
        }
        IrExpr::PartSelect(b, msb, lsb) => {
            first_state_ref(b).or_else(|| first_state_ref(msb)).or_else(|| first_state_ref(lsb))
        }
        IrExpr::PartSelectIndexed { base, idx, width, .. } => {
            first_state_ref(base)
                .or_else(|| first_state_ref(idx))
                .or_else(|| first_state_ref(width))
        }
        IrExpr::Mintypmax(a, b, c) => {
            first_state_ref(a).or_else(|| first_state_ref(b)).or_else(|| first_state_ref(c))
        }
        IrExpr::Sim(SimQuery::Vt(Some(e))) => first_state_ref(e),
        IrExpr::Sim(SimQuery::Simparam { default, .. }) => first_state_ref(default),
        IrExpr::Sim(SimQuery::Limit { args, .. }) | IrExpr::Sim(SimQuery::Random { args, .. }) => {
            args.iter().find_map(first_state_ref)
        }
        IrExpr::AcStim { mag, phase } => first_state_ref(mag).or_else(|| first_state_ref(phase)),
        IrExpr::BundleLit { fields, .. } => fields.iter().find_map(|(_, e)| first_state_ref(e)),
        IrExpr::Lambda { body, .. } => first_state_ref(body),
        // Leaf nodes with no sub-expressions: Real, Int, String, Bool, Quad,
        // Param, Var, BranchAccess, PortFlow, and no-arg SimQuery variants.
        _ => None,
    }
}

//! The expression and statement node set, its operators and the places a statement writes.

use serde::{Deserialize, Serialize};

use crate::ids::{WideId, ExprId, FunctionId, StmtId, TypeId};
use crate::real::Real;
use crate::model::{CommitTarget, EmitField, Origin, Terminal, ValueType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprData {
    pub(crate) kind: Expr,
    pub(crate) ty: ValueType,
    pub(crate) origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Real(Real),
    Bool(bool),
    Unit,
    Local(u32),
    Param(u32),
    Unary { op: UnaryOp, expr: ExprId },
    Binary { op: BinaryOp, lhs: ExprId, rhs: ExprId },
    /// `if cond { then } else { otherwise }`: runs the arm `cond` selects.
    If { cond: ExprId, then: ExprId, otherwise: ExprId },
    Block { stmts: Vec<StmtId>, value: Option<ExprId> },
    Call { function: FunctionId, args: Vec<ExprId> },
    Math { f: MathFn, args: Vec<ExprId> },
    Closure { params: Vec<u32>, body: ExprId },
    Probe { plus: Terminal, minus: Terminal },
    Env(Env),
    Limit { site: u32, x: ExprId, f: ExprId, seed: Option<ExprId> },
    Stim { site: u32, mag: ExprId, phase: Option<ExprId> },
    /// An integer literal; its `UInt<N>` or `Int<N>` type is the expression's type.
    Int(i128),
    /// A float literal of a width other than `f64`: its IEEE 754 bit pattern; the expression's type
    /// (`f32`, `f16` or `f128`) is its format.
    FloatBits(u128),
    /// A literal of an integer type wider than 64 bits: an entry of the arena of wide constants.
    Wide(WideId),
    /// `expr as to`: among integer types and `f64`, or from `bool` to an integer type.
    Cast { expr: ExprId, to: ValueType },
    /// `receiver.method(args)`: one of the closed set of value methods.
    Method { method: Method, receiver: ExprId, args: Vec<ExprId> },
    /// A read of a carrier of the body, by index; its type is the carrier's.
    Carrier(u32),
    /// `i(a, b)` of a branch the body drives with a potential contribution: the branch's flow,
    /// an `f64`, by branch index. The branch carries a potential contribution in the body.
    Flow { branch: u32 },
    /// `c.prev()`: carrier `c`'s value before its most recent change (§3.3), of the carrier's type;
    /// before the first change it is the initial value.
    Previous(u32),
    /// `p.prev()` of a param `p` of the body (§5.3.1): the value `p` held before its most recent
    /// change, of the param's own type.
    ParamPrevious(u32),
    /// The rate of the derivative site `k` of the body (`Body::derivatives`): an `f64`, read in
    /// the `analog` block, a body `let`, an event argument or a reaction.
    Derivative(u32),
    /// The output of the delay site `k` of the body (`Body::delays`): an `f64`, read in the
    /// `analog` block, a body `let`, an event argument or a reaction.
    Delay(u32),
    /// The value of the noise source `k` of the body (`Body::noise_sources`): an `f64` that is 0
    /// in every solve and the source's unit-variance input in a noise sweep. Read in the `analog`
    /// block, a body `let` or an event argument; a reaction reads 0.
    Noise(u32),
    /// `simparam(name)` (§14.4): the value of the body's session parameter `parameter`
    /// (`Body::session_parameters`), an `Option<f64>`: `Some` of what the run gave it, else `None`.
    /// Constant during an analysis; its parts are read through `OptionPart` like any packed option.
    SessionParameter { parameter: u32 },
    /// `fired(e)` in a reaction: whether the body's event `e` fired in this microstep (the
    /// `CHANGED` byte of its pin), a known `bool`.
    Fired(u32),
    /// `unwrap`/`expect` of an `Option` carried as the pair `(is, value)`: `value` when `is`
    /// holds, else a fault of the run naming `message` (Rust's `unwrap` text when absent). Its
    /// type is the value's.
    Unwrap { is: ExprId, value: ExprId, message: Option<String> },
    /// `same(a, b)` (§22.1.4): every level has the same value. `a` and `b` have one type; the
    /// result is a `bool`.
    Same { a: ExprId, b: ExprId },
    /// `cast::<A, B>(expr)` (§22.1.3): the levels of `expr` under type `to`, which has the same
    /// number of levels; the result is an `Option<to>`, `None` for a tag that names no variant.
    /// `tags` lists the tags `to` decodes when it is an enum.
    LevelCast { expr: ExprId, to: ValueType, tags: Option<Vec<i128>> },
    /// `loop { body }` (§1.8): runs `body` until a `Break`. Its type is its breaks' value type,
    /// `()` for a `break` with no value.
    Loop { body: Vec<StmtId> },
    /// `while cond { body }`: runs `body` while `cond`, a `bool`, holds. Its type is `()`.
    While { cond: ExprId, body: Vec<StmtId> },
    /// `for local in low..high { body }`, `..=` when `inclusive`, walked from the top down when
    /// `reverse` (`.rev()`): `local` takes each integer of the range, `low` and `high` have one
    /// integer type. Its type is `()`.
    ForRange { local: u32, low: ExprId, high: ExprId, inclusive: bool, reverse: bool, body: Vec<StmtId> },
    /// `break` or `break value`: leaves the innermost loop; a value only out of a `Loop`. It
    /// fits any expected type, as it never produces one.
    Break(Option<ExprId>),
    /// `continue`: starts the innermost loop's next iteration. It fits any expected type.
    Continue,
    /// `return` or `return value` in a function body: the value has the function's result type.
    /// It fits any expected type.
    Return(Option<ExprId>),
    /// A function used as a value (§1.5.10): the name of a function, of type `Fn`.
    FunctionRef(FunctionId),
    /// `callee(args)` where `callee` is a value of type `Fn`, not a named function.
    CallValue { callee: ExprId, args: Vec<ExprId> },
    /// `(a, b, ..)`: a tuple of the operands, lowest field first. Its type is `Tuple`.
    Tuple(Vec<ExprId>),
    /// `[a, b, ..]`: an array of the operands, all of one type. Its type is `Array`.
    Array(Vec<ExprId>),
    /// `[element; count]`: `count` copies of one value (§1.5.25).
    ArrayRepeat { element: ExprId, count: u64 },
    /// `S { x: a, y: b }` or `S(a, b)`: a struct of the arena, its fields in declaration order.
    StructOf { ty: TypeId, fields: Vec<ExprId> },
    /// A variant of an enum of the arena with its fields in declaration order.
    VariantOf { ty: TypeId, variant: u32, fields: Vec<ExprId> },
    /// `expr.field`, `expr.0`: field `index` of a struct or a tuple.
    Field { expr: ExprId, index: u32 },
    /// `expr[index]`: an element of an array; `index` has an integer type.
    Index { expr: ExprId, index: ExprId },
    /// `expr.len()`: the length of an array, a `UInt<64>`.
    Length(ExprId),
    /// `matches!(expr, Enum::Variant ..)`: whether the enum value is variant `variant`. A `bool`.
    IsVariant { expr: ExprId, variant: u32 },
    /// Field `field` of variant `variant` of an enum value: defined where `IsVariant` holds.
    VariantField { expr: ExprId, variant: u32, field: u32 },
    /// The `Option<T>` made from the pair `(is, value)` that value code keeps (MD-67): `is` a
    /// `bool`, `value` a `T`.
    OptionPack { is: ExprId, value: ExprId },
    /// One half of an `Option<T>` as the pair: `Is` gives a `bool`, `Value` the `T`.
    OptionPart { option: ExprId, part: LevelPart },
    /// `T::default()` (H.8 rule 4): the type is the one written.
    Default(ValueType),
    /// `table_model(x, table, control)` (§22.2.15, PI-10): the interpolant of the `table` of `(abscissa,
    /// value)` samples, strictly increasing in the abscissa, read at `x`, an `f64`. The table and the
    /// control are elaboration constants, so the form holds them and the run only evaluates it.
    Table { x: ExprId, table: Vec<(Real, Real)>, control: TableControl },
    /// The derivative of the `Table` with the same `table` and `control` with respect to `x`: the slope of
    /// its interpolant at `x` (§22.2.15 rule 5). An `f64`.
    TableSlope { x: ExprId, table: Vec<(Real, Real)>, control: TableControl },
    /// `x[index]`: the value of one element of the array carrier `carrier`, `index` an integer expression (any
    /// integer; a run-time index past the end is a fault of the run, CV-25 AC 5). Typed as the carrier's `ty`.
    CarrierElement { carrier: u32, index: ExprId },
    /// `x[index].prev()`: the value an element had before its last change; typed as the carrier's `ty`.
    ElementPrevious { carrier: u32, index: ExprId },
    /// `x[index].changed()` read as a value in a reaction (§13 rule 9): a `bool`.
    ElementChanged { carrier: u32, index: ExprId },
    /// `s.others()` (§5.2.11): what the drives of the net but this body's own resolve to, of the type `V` of the
    /// `Resolved` carrier `carrier`.
    Others(u32),
    /// `resolve` of the present drives among the `drives` carriers of the resolver body (part II §4 rule 8), but
    /// `except` when it is `Some`. `by_count[k]` is the instance `resolve::<k>` of the net's `impl Resolve<X, V>`: it
    /// takes `[X; k]` and returns `V`, which is the type of the form. A drive is present when its `driving` byte is 1.
    Resolve { by_count: Vec<FunctionId>, drives: Vec<u32>, except: Option<u32> },
}

/// The control string of a `table_model` as the elaborator parsed it (§22.2.15 rules 2 to 4): the
/// interpolation, and the extrapolation at each end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableControl {
    /// The interpolation character.
    pub degree: TableDegree,
    /// What happens below the first abscissa.
    pub low: Extrapolation,
    /// What happens above the last abscissa.
    pub high: Extrapolation,
}

/// The interpolation of a `table_model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableDegree {
    /// `D`: the closest sample, no interpolation.
    Closest,
    /// `1`: linear.
    Linear,
    /// `2`: a quadratic spline.
    Quadratic,
    /// `3`: a cubic spline.
    Cubic,
}

/// What a `table_model` does outside its table, at one end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Extrapolation {
    /// `C`: the end value.
    Constant,
    /// `L`: the line of the end slope.
    Linear,
    /// `E`: an error, `E0705`, when `x` is outside the table.
    Error,
}

impl TableControl {
    /// The control string that parses to this control: the interpolation character then the two
    /// extrapolation characters.
    pub fn text(&self) -> String {
        let degree = match self.degree {
            TableDegree::Closest => 'D',
            TableDegree::Linear => '1',
            TableDegree::Quadratic => '2',
            TableDegree::Cubic => '3',
        };
        let end = |extrapolation: Extrapolation| match extrapolation {
            Extrapolation::Constant => 'C',
            Extrapolation::Linear => 'L',
            Extrapolation::Error => 'E',
        };
        format!("{degree}{}{}", end(self.low), end(self.high))
    }
}

impl Expr {
    /// The expressions the form evaluates directly, in evaluation order: not the statements of a
    /// block or a loop body (`Expr::statements`).
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) => Vec::new(),
            Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::ParamPrevious(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::Fired(_) | Expr::Continue | Expr::FunctionRef(_) => Vec::new(),
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::LevelCast { expr, .. } => vec![*expr],
            Expr::Binary { lhs: a, rhs: b, .. } | Expr::Unwrap { is: a, value: b, .. } | Expr::Same { a, b } => vec![*a, *b],
            Expr::If { cond, then, otherwise } => vec![*cond, *then, *otherwise],
            Expr::Block { value, .. } => value.iter().copied().collect(),
            Expr::Call { args, .. } | Expr::Math { args, .. } => args.clone(),
            Expr::Closure { body, .. } => vec![*body],
            Expr::Method { receiver, args, .. } => [*receiver].into_iter().chain(args.iter().copied()).collect(),
            Expr::Limit { x, f, seed, .. } => [*x, *f].into_iter().chain(*seed).collect(),
            Expr::Stim { mag, phase, .. } => [*mag].into_iter().chain(*phase).collect(),
            Expr::SessionParameter { .. } => Vec::new(),
            Expr::Loop { .. } => Vec::new(),
            Expr::While { cond, .. } => vec![*cond],
            Expr::ForRange { low, high, .. } => vec![*low, *high],
            Expr::Break(value) | Expr::Return(value) => value.iter().copied().collect(),
            Expr::CallValue { callee, args } => [*callee].into_iter().chain(args.iter().copied()).collect(),
            Expr::Tuple(parts) | Expr::Array(parts) | Expr::StructOf { fields: parts, .. } | Expr::VariantOf { fields: parts, .. } => parts.clone(),
            Expr::ArrayRepeat { element, .. } => vec![*element],
            Expr::Field { expr, .. } | Expr::Length(expr) | Expr::IsVariant { expr, .. } | Expr::VariantField { expr, .. } => vec![*expr],
            Expr::OptionPack { is, value } => vec![*is, *value],
            Expr::OptionPart { option, .. } => vec![*option],
            Expr::Default(_) => Vec::new(),
            Expr::Table { x, .. } | Expr::TableSlope { x, .. } => vec![*x],
            Expr::Index { expr, index } => vec![*expr, *index],
            Expr::CarrierElement { index, .. } | Expr::ElementPrevious { index, .. } | Expr::ElementChanged { index, .. } => vec![*index],
            Expr::Others(_) | Expr::Resolve { .. } => Vec::new(),
        }
    }

    /// The statements the form runs: a block's, a loop's body; none for any other form.
    pub fn statements(&self) -> &[StmtId] {
        match self {
            Expr::Block { stmts: body, .. } | Expr::Loop { body } | Expr::While { body, .. } | Expr::ForRange { body, .. } => body,
            _ => &[],
        }
    }
}

/// Which half of the `Option<bool>` of `x.level(k)` an `Expr::Level` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelPart {
    /// Whether the level is known: `true` for `Some`, `false` for `None`.
    Is,
    /// The level's value when it is known (`false` when it is not, the form `None` takes).
    Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Let { local: u32, init: ExprId },
    Expr(ExprId),
    Contribute { branch: u32, quantity: QuantityRole, addends: Vec<Addend> },
    /// `carrier <- value;` or `carrier <- value after delay;`, in a reaction.
    Commit { carrier: u32, value: ExprId, after: Option<ExprId>, #[serde(default, skip_serializing_if = "Option::is_none")] target: Option<CommitTarget> },
    /// `emit!("stream", f = value, …)`, in a reaction: one record of stream `stream` (its index
    /// in the body's streams). Counted with the commits in the reaction's statement numbering.
    Emit { stream: u32, fields: Vec<EmitField> },
    /// `emit!("stream", f = value, …)` in the `analog` block: one record of the stream of trace
    /// site `site` (`Body::traces`), written when the statement executes.
    Trace { site: u32, fields: Vec<EmitField> },
    /// `bound_step(value);` at the top level of the `analog` block: the solver's next step is no
    /// longer than `value`, an `f64`. `site` numbers the statements of the body. `when` is the
    /// conjunction of the conditions the source wrote it under, a `bool` lowered standalone: a
    /// step whose solution did not execute the call (`when` false) is capped by nothing
    /// (§14.10.2). `None` for an unconditional call.
    BoundStep { site: u32, value: ExprId, when: Option<ExprId> },
    /// `place = value;`, or `place op= value;` when `op` is set (§1.5.12): the target is a place
    /// of a `mutable` local. Not an ABI statement: it writes a local, never the outside.
    Assign { place: Place, op: Option<BinaryOp>, value: ExprId },
}

/// The target of an assignment: a local and the projections that reach inside it (`s.f`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// The index of the local the assignment writes.
    pub local: u32,
    /// The steps from the local to the written part, outermost first; empty for the whole local.
    pub path: Vec<Projection>,
}

/// One step into a value from a place: a field of a struct or a tuple, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Projection {
    /// `.n` or `.name`: the field at this index of a struct or a tuple.
    Field(u32),
    /// `[index]`: an element of an array, the index an expression of an integer type.
    Index(ExprId),
}

impl Place {
    /// The index expressions of the place's projections, outermost first.
    pub fn index_operands(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.path.iter().filter_map(|step| match step {
            Projection::Index(index) => Some(*index),
            Projection::Field(_) => None,
        })
    }
}

impl Stmt {
    /// The expressions the statement evaluates directly, in evaluation order: a commit's value
    /// before its delay, an emit's values in field order.
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Stmt::Let { init, .. } => vec![*init],
            Stmt::Expr(e) => vec![*e],
            Stmt::Assign { place, value, .. } => [*value].into_iter().chain(place.index_operands()).collect(),
            Stmt::Contribute { addends, .. } => addends.iter().map(|a| a.value).collect(),
            Stmt::Commit { value, after, target, .. } => {
                [*value].into_iter().chain(*after).chain(target.iter().flat_map(CommitTarget::index_operands)).collect()
            }
            Stmt::BoundStep { value, when, .. } => [*value].into_iter().chain(*when).collect(),
            Stmt::Emit { fields, .. } | Stmt::Trace { fields, .. } => fields
                .iter()
                .filter_map(|f| match f {
                    EmitField::Value { value, .. } => Some(*value),
                    EmitField::Text { .. } => None,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantityRole {
    Flow,
    Potential,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Addend {
    pub operator: Operator,
    pub negated: bool,
    pub value: ExprId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    Direct,
    Ddt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// `And` and `Or` short-circuit, as in the source. `BitAnd`, `BitOr` and `BitXor` are the
/// non-short-circuit `&`, `|` and `^`, on `bool` or on integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// The value methods an expression may call: the numeric rows of Appendix H.1 (integers) and
/// H.2 (`f64`). `Min`, `Max`, `Clamp`, `Signum` and `RemEuclid` take either; `Floor` through
/// `IsInfinite` are `f64` rows; `IntAbs` through `MulOverflows` are integer rows (`f64`'s `abs`
/// is `MathFn::Abs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Min,
    Max,
    Clamp,
    Floor,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sinh,
    Cosh,
    Tanh,
    Powf,
    Powi,
    Log10,
    Log2,
    Ceil,
    Round,
    Trunc,
    Fract,
    Signum,
    RemEuclid,
    Hypot,
    IsNan,
    IsFinite,
    IsInfinite,
    IntAbs,
    Pow,
    SaturatingAdd,
    SaturatingSub,
    SaturatingMul,
    AddOverflows,
    SubOverflows,
    MulOverflows,
}

impl Method {
    /// The MSL name of the method: `sin` for `x.sin()`, `abs` for an integer's `x.abs()`.
    pub fn name(self) -> &'static str {
        match self {
            Method::Min => "min",
            Method::Max => "max",
            Method::Clamp => "clamp",
            Method::Floor => "floor",
            Method::Sin => "sin",
            Method::Cos => "cos",
            Method::Tan => "tan",
            Method::Asin => "asin",
            Method::Acos => "acos",
            Method::Atan => "atan",
            Method::Atan2 => "atan2",
            Method::Sinh => "sinh",
            Method::Cosh => "cosh",
            Method::Tanh => "tanh",
            Method::Powf => "powf",
            Method::Powi => "powi",
            Method::Log10 => "log10",
            Method::Log2 => "log2",
            Method::Ceil => "ceil",
            Method::Round => "round",
            Method::Trunc => "trunc",
            Method::Fract => "fract",
            Method::Signum => "signum",
            Method::RemEuclid => "rem_euclid",
            Method::Hypot => "hypot",
            Method::IsNan => "is_nan",
            Method::IsFinite => "is_finite",
            Method::IsInfinite => "is_infinite",
            Method::IntAbs => "abs",
            Method::Pow => "pow",
            Method::SaturatingAdd => "saturating_add",
            Method::SaturatingSub => "saturating_sub",
            Method::SaturatingMul => "saturating_mul",
            Method::AddOverflows => "add_overflows",
            Method::SubOverflows => "sub_overflows",
            Method::MulOverflows => "mul_overflows",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathFn {
    Exp,
    Ln,
    Sqrt,
    Abs,
}

/// Ordered as declared: the order of `Deps::env`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Env {
    Temperature,
    Gmin,
    SourceScale,
    /// `now()`: the simulation time, an `f64`. Read by `analog` and by reactions, never by a
    /// drive, an event guard or an `at` argument (abi.md §4.2).
    Time,
    /// `analysis(Static)`: whether the analysis has no time axis, a `bool`. Read where `Time` is.
    Static,
    /// `freq()`: the analysis frequency in hertz, an `f64`; 0 outside an AC analysis (§14.4.4).
    /// An analog read: the ABI gives `FREQ` to analog functions only.
    Freq,
    /// `analysis(kind)`: whether the running analysis is of this kind, a `bool` constant during
    /// the analysis (§14.4.1).
    Analysis(AnalysisKind),
    /// Whether the run set its temperature, a `bool` constant during the analysis: with
    /// `Temperature` it is the pair of `temperature() -> Option<f64>` (§8.5.2). `Temperature`
    /// keeps its meaning: the run's temperature, the solver's 300.15 K when the run sets none.
    TemperatureGiven,
}

/// The analysis kinds `analysis(…)` names, ordered as declared (the order of `Deps::env`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnalysisKind {
    Dc,
    Transient,
    Ac,
    Noise,
}

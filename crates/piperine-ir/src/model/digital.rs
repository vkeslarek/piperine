//! The digital side of a body: carriers, drives, events, reactions and streams.

use serde::{Deserialize, Serialize};

use crate::ids::{ExprId, StmtId};
use crate::model::{Constant, Local, Origin, Projection, ValueType};

/// A discrete carrier of a body: `in clk: Bit`, `out hs: Bit = false`, `signal count: Bit<u9> = 0`.
/// Its index in the body's list is its identity; a read of it (`Expr::Carrier`) has the type
/// `ty`: a carrier holds a value from `(0, 0)` on (§3.3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Carrier {
    pub name: String,
    /// The carried value type: `bool`, `f64` or an integer type.
    pub ty: ValueType,
    pub role: CarrierRole,
    pub init: Option<Constant>,
    pub origin: Origin,
    /// `Some(k)`: an array of `k` carriers of type `ty`, one array net stored sparsely (CV-23, §5.1.8); `None`: one
    /// carrier. An element form (`Expr::CarrierElement` and the rest) names an array carrier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
}

/// An array of nodes (`signal bl: [Electrical; 4]`): `count` consecutive `BodyNode`s from `first` named `bl[0]`,
/// `bl[1]`, ..., kept as one name for the host and the printer (CV-23, CV-26).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeArray {
    pub name: String,
    /// The body index of element 0.
    pub first: u32,
    pub count: u32,
    pub origin: Origin,
}

/// An array declared with no element (`signal none: [Bit<u8>; 0]`, §1.2.3): it holds no carrier and no node, so this
/// record is all the body keeps of it, for a host that names it (`--print` shows `[]`, CV-28).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmptyArray {
    /// The array's name as the host addresses it: the frame's prefix, then the declared name (`p.none` for one of a hoisted
    /// placement `p`).
    pub name: String,
    /// Where the array is declared in the source.
    pub origin: Origin,
}

/// The part of an array of carriers a child's port is given (CV-23): a run of `length` elements from `first`, or one
/// element as a scalar when `length` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierWindow {
    pub first: u64,
    pub length: Option<u64>,
}

/// What a commit or a definition names inside its carrier (CV-25): an element of an array of carriers, then
/// projections into the element's value (a field, an element of a value array). Empty is the whole carrier.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CommitTarget {
    /// The element of an array carrier, an integer expression.
    pub element: Option<ExprId>,
    /// The steps into the value, outermost first.
    pub path: Vec<Projection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarrierRole {
    In,
    Out,
    Signal,
    /// A carrier of a child the body reads (`inst.name`): read only, like an `in` port, bound to
    /// the child instance's net (`IrBuilder::bind_reached_net`).
    Reached,
    /// A carrier of a resolved nature (`signal sda: Sda`, part II §5.2): it reads the net's resolved value, of the
    /// carrier's `ty` (the nature's `V`), and a definition or a commit of it is the body's own drive, of type `driven`
    /// (the `X` of the net's `impl Resolve<X, V>`). `Expr::Others` reads what the other drives resolve to. A `Reach` of the
    /// body may name it: the body then holds a drive of a child's `pub` resolved signal, bound to the child's net. `port` says
    /// the carrier is a port of the module, which a parent binds to its own net (`false` for a signal and for a reached carrier).
    Resolved { driven: ValueType, port: bool },
    /// A signal the elaborator makes for its own use, which the source never names (the mirror of an element of an
    /// array of carriers the `analog` block reads): a signal to the solver, never listed to the user (no default probe,
    /// no metadata, no host write).
    Internal,
}

/// A definition of a carrier: `sample = count == 0;` or, delayed, `g = x after tpd;`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drive {
    pub carrier: u32,
    pub value: ExprId,
    pub after: Option<ExprId>,
    pub origin: Origin,
    /// The element of an array carrier the definition names (`bits[0] = a;`, CV-25); `None` for the whole carrier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<u64>,
    /// The steps into the value (a field, an element of a value array) the definition names inside the carrier or
    /// its element; empty for all of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Projection>,
}

/// An event of a body (§13): what fires it, and a `bool` read at the microstep that must hold
/// for it to fire. `clk.rise()` is `Changed(clk)` guarded by a read of `clk`; `above(e)` is
/// `Cross(e)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub source: EventSource,
    pub guard: Option<ExprId>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventSource {
    /// `c.changed()`: carrier `c` of the body took a new value.
    Changed(u32),
    /// `cross(e, ttol, etol)`: the real `e` went from `<= 0` to `> 0`, located by the solver
    /// within the time tolerance `ttol` and the expression tolerance `etol`, each an `f64` that
    /// reads no probe, carrier or clock, or absent.
    Cross { e: ExprId, ttol: Option<ExprId>, etol: Option<ExprId> },
    /// `at(t)`: simulation time `t`, an `f64` that reads no probe. The event
    /// fires once, when the clock reaches `t`; a later change of what `t` reads re-arms it.
    At(ExprId),
    /// `p.changed()` of a param `p` of the body (§5.3.1): fires when the param's pin changes, a
    /// host `set` during the run (a binding before the run is not a change).
    ParamChanged(u32),
    /// `settle` (§13 rule 17): fires once per time point, in the settle phase that closes the
    /// instant (§15 step 4), when its guard, if any, holds.
    Settle,
    /// `x[k].changed()`, `.rise()`, `.fall()` of element `element` of the array carrier `carrier` (CV-24): the
    /// element took a new value; a constant element, so the plan gives it its own private event pin.
    ElementChanged { carrier: u32, element: u64 },
}

/// `on <events> { … }`: statements run when any of the events fires. Its `let`s bind `locals`,
/// its commits (`Stmt::Commit`) write carriers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    /// Indices into the body's events.
    pub on: Vec<u32>,
    pub stmts: Vec<StmtId>,
    pub locals: Vec<Local>,
    pub origin: Origin,
}

/// A record stream of a body (`emit!("sim", …)`): its name and the value fields its sites write.
/// A site that leaves a field out carries the field's `Default` (§19 rule 11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub name: String,
    pub fields: Vec<StreamField>,
}

/// A value field of a stream, in the order a site first wrote it. Text never crosses the ABI and
/// is not a field: it stays on its site (`EmitField::Text`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamField {
    pub name: String,
    /// `bool`, `f64` or an integer type.
    pub ty: ValueType,
}

/// One `name = value` of an `emit!` site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EmitField {
    /// A value, written to field `field` of the stream (its index in `Stream::fields`).
    Value { field: u32, value: ExprId },
    /// A string literal: kept on the site, never sent.
    Text { name: String, text: String },
}

impl CommitTarget {
    /// The index expressions the target holds: the element, then the projections' indices, outermost first.
    pub fn index_operands(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.element.into_iter().chain(self.path.iter().filter_map(|step| match step {
            Projection::Index(index) => Some(*index),
            Projection::Field(_) => None,
        }))
    }
}

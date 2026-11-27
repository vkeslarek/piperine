//! The state of the type check's code walk: the owner whose code is walked, the walk's memo and
//! scope, where in the code an expression sits, and the steps of the iterative walk.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::contributions::Contributions;
use crate::ids::{ExprId, FunctionId, StmtId};
use crate::model::{BodyData, BranchKind, CarrierRole, FunctionData, Ir, Origin, QuantityRole, ValueType};

/// What names and bounds the code being walked.
pub(super) struct Owner {
    pub(super) item: String,
    pub(super) origin: Origin,
    pub(super) locals: Vec<ValueType>,
    /// Which locals the source declared `let mut`, parallel to `locals`: only these are assigned.
    pub(super) mutable: Vec<bool>,
    /// The result type of the function being walked: what a `return` carries. `()` in a body.
    pub(super) result: ValueType,
    pub(super) params: Vec<ValueType>,
    pub(super) nodes: usize,
    pub(super) branches: Vec<BranchKind>,
    /// The branches the body's `analog` block contributes a potential to.
    pub(super) driven: BTreeSet<u32>,
    pub(super) limits: usize,
    /// For each `limit` site: whether it has an `enabled` (it is read under a condition).
    pub(super) limit_guards: Vec<bool>,
    /// The number of the body's `ac_stim` sites.
    pub(super) stimulus_sites: usize,
    /// The carried types of the body's carriers.
    pub(super) carriers: Vec<ValueType>,
    /// The element count of each carrier that is an array of carriers, parallel to `carriers`.
    pub(super) counts: Vec<Option<u64>>,
    /// The driven type of each `Resolved` carrier, parallel to `carriers` (`None` for every other role): a drive or a
    /// commit of a resolved carrier writes this type, and only these carriers have `others()`.
    pub(super) drive_types: Vec<Option<ValueType>>,
    /// The field types of each of the body's streams.
    pub(super) streams: Vec<Vec<ValueType>>,
    /// The number of the body's events.
    pub(super) events: usize,
    /// The number of the body's derivative sites.
    pub(super) derivatives: usize,
    /// The number of the body's delay sites.
    pub(super) delays: usize,
    /// The number of the body's noise sources.
    pub(super) noise_sources: usize,
    /// The number of the body's session parameters.
    pub(super) session_parameters: usize,
    /// The record stream of each of the body's trace sites.
    pub(super) traces: Vec<u32>,
    /// The statements of a reaction: `fired` may appear.
    pub(super) reaction: bool,
    /// The event whose guard is walked: `fired` of another event may appear there (§13 rule 9),
    /// never of itself.
    pub(super) event: Option<u32>,
    /// A body (probes and params are readable) rather than a function.
    pub(super) in_body: bool,
    /// The `analog` block of a body: sites and contributions may appear.
    pub(super) analog: bool,
    /// A value a primitive evaluates on its own (a derivative state, a delay input, a noise density): it may read an
    /// `ac_stim` site, which a statement of the `analog` block writes.
    pub(super) value_input: bool,
}

impl Owner {
    /// The owner of code of `body` named `item`: its `analog` block when `analog`, any other
    /// position of the body otherwise.
    pub(super) fn of_body(ir: &Ir, body: &BodyData, analog: bool, item: String) -> Owner {
        Owner {
            item,
            origin: body.origin.clone(),
            locals: body.locals.iter().map(|l| l.ty.clone()).collect(),
            mutable: body.locals.iter().map(|l| l.mutable).collect(),
            result: ValueType::Unit,
            params: body.params.iter().map(|p| p.ty.clone()).collect(),
            nodes: body.nodes.len(),
            branches: body.branches.iter().map(|b| b.kind).collect(),
            driven: Owner::potentials(ir, body),
            limits: body.limits.len(),
            limit_guards: body.limits.iter().map(|s| s.enabled.is_some()).collect(),
            stimulus_sites: body.stims.len(),
            carriers: body.carriers.iter().map(|c| c.ty.clone()).collect(),
            counts: body.carriers.iter().map(|c| c.count).collect(),
            drive_types: body.carriers.iter().map(|c| match &c.role {
                CarrierRole::Resolved { driven, .. } => Some(driven.clone()),
                _ => None,
            }).collect(),
            streams: body.streams.iter().map(|s| s.fields.iter().map(|f| f.ty.clone()).collect()).collect(),
            events: body.events.len(),
            derivatives: body.derivatives.len(),
            delays: body.delays.len(),
            noise_sources: body.noise_sources.len(),
            session_parameters: body.session_parameters.len(),
            traces: body.traces.iter().map(|t| t.stream).collect(),
            reaction: false,
            event: None,
            in_body: true,
            analog,
            value_input: false,
        }
    }

    /// The owner of the code of `function`: no body's sites, nodes or carriers.
    pub(super) fn of_function(function: &FunctionData) -> Owner {
        Owner {
            item: format!("function {}", function.name),
            origin: function.origin.clone(),
            locals: function.locals.iter().map(|l| l.ty.clone()).collect(),
            mutable: function.locals.iter().map(|l| l.mutable).collect(),
            result: function.ret.clone(),
            params: Vec::new(),
            nodes: 0,
            branches: Vec::new(),
            driven: BTreeSet::new(),
            limits: 0,
            limit_guards: Vec::new(),
            stimulus_sites: 0,
            carriers: Vec::new(),
            counts: Vec::new(),
            drive_types: Vec::new(),
            streams: Vec::new(),
            events: 0,
            derivatives: 0,
            delays: 0,
            noise_sources: 0,
            session_parameters: 0,
            traces: Vec::new(),
            reaction: false,
            event: None,
            in_body: false,
            analog: false,
            value_input: false,
        }
    }

    /// The branches with a private flow unknown a flow probe may read: those `body`'s `analog`
    /// block contributes a potential to, wherever the contribution stands, and every switch
    /// branch (it has the unknown whichever kind a solve enables).
    fn potentials(ir: &Ir, body: &BodyData) -> BTreeSet<u32> {
        let potential = Contributions::new(ir).of(body.analog.as_deref().unwrap_or_default()).into_iter().filter(|c| c.quantity == QuantityRole::Potential).map(|c| c.branch);
        let switches = body.branches.iter().enumerate().filter(|(_, b)| b.kind == BranchKind::Switch).map(|(i, _)| i as u32);
        potential.chain(switches).collect()
    }
}

/// The state of one walk: one owner's code, every root of it sharing the memo.
pub(super) struct CodeWalk {
    pub(super) owner: Owner,
    /// The distinct expressions that use each `limit` / `ac_stim` site.
    pub(super) limit_users: Vec<BTreeSet<ExprId>>,
    pub(super) stimulus_users: Vec<BTreeSet<ExprId>>,
    /// Locals bound in the scope being walked (a `let` after its init until its block ends,
    /// closure parameters in the closure's body, function parameters throughout).
    pub(super) bound: Vec<bool>,
    /// The id of the current `bound` set, interned: part of a visit's key.
    pub(super) scope: u32,
    scopes: HashMap<Vec<bool>, u32>,
    /// Local reads already reported as unbound.
    pub(super) unbound: HashSet<ExprId>,
    /// Expressions whose own checks ran.
    pub(super) seen: HashSet<ExprId>,
    /// Expression and statement visits already made, per placement and scope.
    pub(super) done: HashSet<(ExprId, WalkPosition, u32)>,
    pub(super) statements_done: HashSet<(StmtId, WalkPosition, bool, u32)>,
    /// Expressions being walked: meeting one again is a cycle.
    pub(super) on_stack: HashSet<ExprId>,
    /// The functions this owner's code calls.
    pub(super) calls: BTreeSet<FunctionId>,
    /// The follow-up steps of the step being taken, in evaluation order.
    pub(super) pending: Vec<WalkStep>,
}

impl CodeWalk {
    /// A walk of `owner`'s code with its first `bound` locals (a function's parameters) bound.
    pub(super) fn new(owner: Owner, bound: usize) -> Self {
        let mut slots = vec![false; owner.locals.len()];
        for slot in slots.iter_mut().take(bound) {
            *slot = true;
        }
        let mut walk = CodeWalk {
            owner,
            limit_users: Vec::new(),
            stimulus_users: Vec::new(),
            bound: slots,
            scope: 0,
            scopes: HashMap::new(),
            unbound: HashSet::new(),
            seen: HashSet::new(),
            done: HashSet::new(),
            statements_done: HashSet::new(),
            on_stack: HashSet::new(),
            calls: BTreeSet::new(),
            pending: Vec::new(),
        };
        walk.intern_scope();
        walk
    }

    /// Interns the current `bound` set as the scope id of the visits that follow.
    pub(super) fn intern_scope(&mut self) {
        let next = self.scopes.len() as u32;
        self.scope = *self.scopes.entry(self.bound.clone()).or_insert(next);
    }
}

/// Where in the code an expression sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct WalkPosition {
    pub(super) conditional: bool,
    pub(super) closure: bool,
    /// The innermost loop expression the code sits in, `None` outside every loop and inside a
    /// closure (`break` and `continue` do not cross a closure).
    pub(super) enclosing_loop: Option<ExprId>,
}

impl WalkPosition {
    /// The position of a root of the walk: unconditional, outside every closure and loop.
    pub(super) fn top() -> Self {
        WalkPosition { conditional: false, closure: false, enclosing_loop: None }
    }
}

/// One step of the iterative walk.
pub(super) enum WalkStep {
    Expression(ExprId, WalkPosition),
    /// The walk of an expression's operands is over: it leaves the stack.
    Exit(ExprId),
    /// A statement, and whether it sits at the top of an `analog` block.
    Statement(StmtId, WalkPosition, bool),
    /// A `let` has evaluated its init: its local is bound.
    Bind(u32),
    /// A block or a closure ends: the locals it bound go out of scope.
    Restore(Vec<bool>),
}

/// One visit of an expression by the walk: which it is, where it was written, the type it records,
/// where it sits, and whether its own checks run (they run on its first visit only).
pub(super) struct ExpressionVisit<'a> {
    pub(super) id: ExprId,
    pub(super) origin: Origin,
    pub(super) recorded_type: &'a ValueType,
    pub(super) position: WalkPosition,
    pub(super) first: bool,
}

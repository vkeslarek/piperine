//! The state of value expressions: what they are typed under and what typing them has learnt.

use std::collections::{BTreeMap, BTreeSet};

use crate::pom::{DeclId, ExprId};

use crate::types::check::Context;
use crate::types::Ty;
use super::assign::{Places, Write};
use super::flow::{Assigned, ExitFrame, Flow};
use super::class::FunctionClass;
use super::closures::ClosureInfo;

/// What value expressions (§1.5, §3) are typed under, and what typing them has learnt that the result does not keep.
pub(in crate::types) struct ValueTables {
    /// The declaration being checked: a module, a function or a constant.
    pub(super) owner: Option<DeclId>,
    /// Where the expression being checked is written.
    pub(super) context: Context,
    /// Set while the `where` of a `fn` is checked: its parameters are not in scope (§3 rule 8).
    pub(super) fn_where: bool,
    /// The one expression that may be an `Event`: an event position hands it down just before it
    /// types the expression, which judges an `Event` anywhere else.
    pub(super) event_slot: Option<ExprId>,
    /// Set while the guard of an event, the right operand of `ev && guard`, is typed.
    pub(super) in_guard: bool,
    /// Bindings whose value could not be typed: a read of one is silent, so one mistake is
    /// one diagnostic.
    pub(super) poisoned: BTreeSet<DeclId>,
    /// The initializer of each `let` with a name, which says whether the name's value depends on the run.
    pub(super) let_values: BTreeMap<DeclId, ExprId>,
    /// Which names are places and which locals the body assigns (§1.5.12).
    pub(super) places: Places,
    /// The locals bound to a bare integer literal whose type no use has fixed yet (§1.4.4), each
    /// with the literal expression: `let mut total = 0;` waits for `total += k`.
    pub(super) open_literals: BTreeMap<DeclId, ExprId>,
    /// The locals declared without a value (`let x;`, §1.7.5).
    pub(super) deferred: BTreeSet<DeclId>,
    /// The control state of the body being checked.
    pub(super) flow: Flow,
    /// The captures and the function class of each closure typed so far.
    pub(super) closures: BTreeMap<ExprId, ClosureInfo>,
    /// The closure each local is bound to by `let` (the local's value is that closure).
    pub(super) closure_locals: BTreeMap<DeclId, ExprId>,
    /// The locals bound to a closure whose parameters wait for the first call to be typed.
    pub(super) pending_closures: BTreeMap<DeclId, ExprId>,
    /// The function class of each function whose class is known.
    pub(super) classes: BTreeMap<DeclId, FunctionClass>,
    /// The type the context expects of one call, handed down by `check_as` just before the call
    /// is typed: the result type of a call that is generic over its result (`logic`, `cast`).
    pub(super) call_expected: Option<(ExprId, Ty)>,
    /// The enums whose variants are typed (`enums.rs`).
    pub(super) declared: BTreeSet<DeclId>,
    /// The structs and enums whose fields are being typed: meeting one again is a type that
    /// contains itself (§1.3.8).
    pub(super) declaring: BTreeSet<DeclId>,
    /// The structs and enums that could not be typed: a use is silent, the cause is reported.
    pub(super) broken: BTreeSet<DeclId>,
}

impl Default for ValueTables {
    fn default() -> Self {
        ValueTables {
            owner: None,
            context: Context::Module,
            fn_where: false,
            event_slot: None,
            in_guard: false,
            poisoned: BTreeSet::new(),
            let_values: BTreeMap::new(),
            places: Places::default(),
            open_literals: BTreeMap::new(),
            deferred: BTreeSet::new(),
            flow: Flow::default(),
            closures: BTreeMap::new(),
            closure_locals: BTreeMap::new(),
            pending_closures: BTreeMap::new(),
            classes: BTreeMap::new(),
            call_expected: None,
            declared: BTreeSet::new(),
            declaring: BTreeSet::new(),
            broken: BTreeSet::new(),
        }
    }
}

/// The value state of one declaration, set aside while another is checked ahead of the queue and put back after.
pub(in crate::types) struct SavedValues {
    /// The declaration that was being checked.
    owner: Option<DeclId>,
    /// Where the expression that was being checked is written.
    context: Context,
    /// The event slot that was open.
    event_slot: Option<ExprId>,
    /// Whether the guard of an event was being typed.
    in_guard: bool,
    /// Whether the `where` of a `fn` was being checked.
    fn_where: bool,
    /// The control state of the body that was being checked.
    flow: Flow,
}

impl ValueTables {
    /// Where the expression being checked is written.
    pub(in crate::types) fn context(&self) -> Context {
        self.context
    }

    /// Set where the expression being checked is written.
    pub(in crate::types) fn set_context(&mut self, context: Context) {
        self.context = context;
    }

    /// Set where the expression being checked is written, and give the place it was in.
    pub(in crate::types) fn replace_context(&mut self, context: Context) -> Context {
        std::mem::replace(&mut self.context, context)
    }

    /// The declaration being checked.
    pub(in crate::types) fn owner(&self) -> Option<DeclId> {
        self.owner
    }

    /// The one expression that may be an `Event`, when an event position has handed it down.
    pub(in crate::types) fn event_slot(&self) -> Option<ExprId> {
        self.event_slot
    }

    /// Hand `expr` down as the one expression that may be an `Event`.
    pub(in crate::types) fn set_event_slot(&mut self, expr: ExprId) {
        self.event_slot = Some(expr);
    }

    /// Whether the guard of an event is being typed.
    pub(in crate::types) fn in_guard(&self) -> bool {
        self.in_guard
    }

    /// Set whether the guard of an event is being typed, and give what it was.
    pub(in crate::types) fn replace_in_guard(&mut self, in_guard: bool) -> bool {
        std::mem::replace(&mut self.in_guard, in_guard)
    }

    /// Set whether the `where` of a `fn` is being checked.
    pub(in crate::types) fn set_fn_where(&mut self, fn_where: bool) {
        self.fn_where = fn_where;
    }

    /// Mark `binding` as one whose value could not be typed: a read of it is silent.
    pub(in crate::types) fn poison(&mut self, binding: DeclId) {
        self.poisoned.insert(binding);
    }

    /// Whether `binding` could not be typed.
    pub(in crate::types) fn is_poisoned(&self, binding: DeclId) -> bool {
        self.poisoned.contains(&binding)
    }

    /// The closure the local `decl` is bound to by `let`.
    pub(in crate::types) fn closure_of(&self, decl: DeclId) -> Option<ExprId> {
        self.closure_locals.get(&decl).copied()
    }

    /// Record that the local `binding` is bound to `closure`.
    pub(in crate::types) fn bind_closure(&mut self, binding: DeclId, closure: ExprId) {
        self.closure_locals.insert(binding, closure);
    }

    /// The initializer of the `let` named `decl`, when it has one.
    pub(in crate::types) fn let_value(&self, decl: DeclId) -> Option<ExprId> {
        self.let_values.get(&decl).copied()
    }

    /// The writes the body makes to the local `decl`.
    pub(in crate::types) fn writes_of(&self, decl: DeclId) -> &[Write] {
        self.places.writes_of(decl)
    }

    /// The type the context expects of the call `expr`, handed down by `check_as`; taken once.
    pub(in crate::types) fn take_expected_call_type(&mut self, expr: ExprId) -> Option<Ty> {
        self.call_expected.take().filter(|(at, _)| *at == expr).map(|(_, ty)| ty)
    }

    /// Start checking `decl`: it is the owner, with no control state yet.
    pub(in crate::types) fn begin_declaration(&mut self, decl: DeclId) {
        self.owner = Some(decl);
        self.flow = Flow::default();
    }

    /// Start checking a function: its own result for `return`.
    pub(in crate::types) fn begin_function(&mut self, exit: ExitFrame) {
        self.context = Context::Function;
        self.flow.exit = Some(exit);
    }

    /// The set of deferred locals assigned so far.
    pub(in crate::types) fn assigned(&self) -> Assigned {
        self.flow.assigned.clone()
    }

    /// Put back the set of deferred locals assigned, as it was before a branch.
    pub(in crate::types) fn restore_assigned(&mut self, assigned: Assigned) {
        self.flow.assigned = assigned;
    }

    /// The locals assigned in the body, once the check is done.
    pub(in crate::types) fn into_assigned(self) -> BTreeSet<DeclId> {
        self.places.into_assigned()
    }

    /// Set the state of the declaration being checked aside for a module checked ahead of the queue: a module context,
    /// no event open, no control state.
    pub(in crate::types) fn save(&mut self) -> SavedValues {
        let saved = SavedValues { owner: self.owner, context: self.context, event_slot: self.event_slot, in_guard: self.in_guard, fn_where: self.fn_where, flow: std::mem::take(&mut self.flow) };
        self.context = Context::Module;
        self.event_slot = None;
        self.in_guard = false;
        self.fn_where = false;
        saved
    }

    /// Put back the state [`ValueTables::save`] set aside.
    pub(in crate::types) fn restore(&mut self, saved: SavedValues) {
        self.owner = saved.owner;
        self.context = saved.context;
        self.event_slot = saved.event_slot;
        self.in_guard = saved.in_guard;
        self.fn_where = saved.fn_where;
        self.flow = saved.flow;
    }
}

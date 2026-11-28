//! The state of module bodies: the `let`s, placements, template reads and `emit!` sites being checked.

use std::collections::{BTreeMap, BTreeSet};

use crate::pom::{DeclId, ExprId};

use super::record::Records;

/// What checking module bodies (§8) is in the middle of: the `let`s being typed, the placements being connected, the
/// reads through templates, the node choices left to judge, and the `emit!` sites.
#[derive(Default)]
pub(in crate::types) struct ModuleTables {
    /// The bindings of the module-body `let`s whose values are being typed, each with its value (a
    /// read of one is a feedback, `ModuleTyping::self_read`).
    pub(super) lets_open: BTreeMap<DeclId, ExprId>,
    /// The `let`s of module bodies already typed: a `let` is typed where it is first read, whatever its place in the body
    /// (§8.2: the items of a body appear in any order), and once.
    pub(super) lets_typed: BTreeSet<DeclId>,
    /// The placements whose arguments are being typed, innermost last: the module, or `None` for
    /// a core module.
    pub(super) placing: Vec<Option<DeclId>>,
    /// The reads of an item of a child only a folded template names (`reach/fields.rs`), typed as a node until a context wants a value
    /// of them (`template_items.rs`).
    pub(super) template_reads: BTreeSet<ExprId>,
    /// The reads through a template made through an instance whose `let` was being typed, to that `let`: a node connection, and
    /// a read of the `let` itself when a context wants a value of it (§8.2.16).
    pub(super) open_template_reads: BTreeMap<ExprId, DeclId>,
    /// The `let`s with no type bound to a read through a template, by binding: the first use that wants a value types them.
    pub(super) template_locals: BTreeMap<DeclId, ExprId>,
    /// The `if`s, `match`es and blocks refused for choosing a node (`E0133`, `handles.rs`): an operand that is one is no second
    /// diagnostic.
    pub(super) chosen_nodes: BTreeSet<ExprId>,
    /// The `if`s and `match`es whose result is a node, judged once the whole design is typed (`judge_node_choices`).
    pub(super) node_choices: Vec<ExprId>,
    /// Whether a bundle `struct` is being named as the type of a handle (`bundle_handles.rs`), where it is no refusal.
    pub(super) naming_bundle: bool,
    /// The `emit!` sites seen so far.
    pub(super) records: Records,
}

/// What checking module bodies keeps of the declaration being checked, set aside while another is checked ahead of the
/// queue and put back after.
pub(in crate::types) struct SavedModules {
    /// The `let`s whose values were being typed.
    lets_open: BTreeMap<DeclId, ExprId>,
    /// The placements whose arguments were being typed.
    placing: Vec<Option<DeclId>>,
}

impl ModuleTables {
    /// Whether the value of the module-body `let` named `binding` is being typed.
    pub(in crate::types) fn is_open_let(&self, binding: DeclId) -> bool {
        self.lets_open.contains_key(&binding)
    }

    /// The module placed by the innermost placement whose arguments are being typed; `None` when there is none or it is a core module.
    pub(in crate::types) fn innermost_placement(&self) -> Option<DeclId> {
        self.placing.last().copied().flatten()
    }

    /// A placement of a core module (`None`) or of `module` has its arguments typed.
    pub(in crate::types) fn begin_placement(&mut self, module: Option<DeclId>) {
        self.placing.push(module);
    }

    /// The placement whose arguments were typed is connected.
    pub(in crate::types) fn end_placement(&mut self) {
        self.placing.pop();
    }

    /// Whether a bundle `struct` is being named as the type of a handle.
    pub(in crate::types) fn naming_bundle(&self) -> bool {
        self.naming_bundle
    }

    /// Set whether a bundle `struct` is being named as the type of a handle, and give what it was.
    pub(in crate::types) fn replace_naming_bundle(&mut self, naming: bool) -> bool {
        std::mem::replace(&mut self.naming_bundle, naming)
    }

    /// The streams of every function that writes one.
    pub(in crate::types) fn emitting(&self) -> BTreeMap<DeclId, BTreeSet<String>> {
        self.records.emitting()
    }

    /// Take the `let`s and placements of the declaration being checked.
    pub(in crate::types) fn save(&mut self) -> SavedModules {
        SavedModules { lets_open: std::mem::take(&mut self.lets_open), placing: std::mem::take(&mut self.placing) }
    }

    /// Put back what [`ModuleTables::save`] took.
    pub(in crate::types) fn restore(&mut self, saved: SavedModules) {
        self.lets_open = saved.lets_open;
        self.placing = saved.placing;
    }
}

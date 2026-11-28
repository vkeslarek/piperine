//! The state of discrete lowering (§12, §13): [`DigitalTables`], the carriers, drives, events, reactions and resolved
//! nets of a run, and [`DigitalScope`], the carrier context of the code being lowered. Both go to `elab/digital/` with
//! the methods that read them.

use std::collections::BTreeMap;

use super::carriers::Structure;
use super::resolvers::ResolverBodies;
use crate::pom::{DeclId, ExprId};

/// What one run builds of the discrete side: the carriers, drives, events and reactions of the body being lowered, and
/// the resolved nets of the placed tree.
#[derive(Default)]
pub(in crate::elab) struct DigitalTables {
    /// The carriers, drives, events and reactions of the body being lowered.
    pub(in crate::elab) structure: Structure,
    /// The resolved nets of the placed tree and their resolver bodies (`resolvers.rs`).
    pub(in crate::elab) resolvers: ResolverBodies,
}

/// The carrier context of the code being lowered: the handles and texts the functions inlined at a call are bound to,
/// the iteration of generation it sits in, and whether it is a drive, which holds no local.
#[derive(Default)]
pub(in crate::elab) struct DigitalScope {
    /// The `&str` params of the functions inlined at a call (`records.rs`), bound to their text.
    pub(in crate::elab) texts: BTreeMap<DeclId, String>,
    /// The handle parameters of the functions inlined at a call (`handles.rs`), each bound to the caller's argument: the
    /// name of a port or a signal of the frame, or `gnd`; and the `let`s bound to a node, each to its initializer.
    pub(in crate::elab) handles: BTreeMap<DeclId, ExprId>,
    /// Code with no local of its own (a drive): a `match` reads its scrutinee again at each test.
    pub(in crate::elab) no_locals: bool,
    /// The generation `for`s the code being lowered sits in and the ordinal of the iteration in each (`generation.rs`).
    pub(in crate::elab) ordinals: Vec<(DeclId, u32)>,
}

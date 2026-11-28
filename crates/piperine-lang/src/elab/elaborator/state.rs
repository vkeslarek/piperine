//! Everything one elaboration run holds: [`ElaborationState`].

use piperine_ir::IrBuilder;

use super::origin::Locator;
use crate::elab::analog::state::AnalogSites;
use crate::elab::consts::ConstEval;
use crate::elab::digital::state::DigitalTables;
use crate::elab::functions::state::FunctionTables;
use crate::elab::structure::state::StructureTables;
use crate::elab::structure::structural::Overrides;
use crate::elab::value::state::ExpressionTables;
use crate::pom::{Design, DesignDiagnostic};
use crate::types::Types;

/// Everything one elaboration run holds: the inputs it only reads, the IR being built and what it reported, and one
/// table per concern. A table is filled and read by the lowering of its concern; the root fields are read by all.
pub(in crate::elab) struct ElaborationState<'d, 't> {
    /// The design being elaborated, only read.
    pub(in crate::elab) design: &'d Design,
    /// The types the check gave the design, only read.
    pub(in crate::elab) types: &'t Types<'d>,
    /// The constant evaluator every concern folds through (§8.1.21).
    pub(in crate::elab) consts: ConstEval<'d, 't>,
    /// The IR being built.
    pub(in crate::elab) builder: IrBuilder,
    /// The source position of a declaration, an expression or a statement: the origin of what lowers from it.
    pub(in crate::elab) locator: Locator<'d>,
    /// The params the host bound before the run.
    pub(in crate::elab) overrides: Overrides,
    /// Every trait and `impl` of the design: what a call through a bound resolves against (`methods.rs`).
    pub(in crate::elab) impls: crate::types::ImplTable<'d>,
    /// What the run has reported so far, in report order.
    pub(in crate::elab) diagnostics: Vec<DesignDiagnostic>,
    /// The state of value code (§1.5, §3): the type, continuity and origin of every IR expression, the divergences, the
    /// type arena.
    pub(in crate::elab) values: ExpressionTables,
    /// The state of functions (§3.4): the IR function of each variant a call reached.
    pub(in crate::elab) functions: FunctionTables,
    /// The state of analog code (§14): natures, ammeters, loops and call cycles.
    pub(in crate::elab) analog: AnalogSites,
    /// The state of discrete code (§12, §13): the carriers, drives, events and reactions of the body, the resolved nets.
    pub(in crate::elab) digital: DigitalTables,
    /// The state of module bodies and the placed tree (§8): bodies, plans, key params, frames, generated items, reads
    /// through children.
    pub(in crate::elab) structure: StructureTables,
}

impl<'d, 't> ElaborationState<'d, 't> {
    /// The state of a run over `design`, typed as `types`, with the params the host bound (`overrides`) and the reads
    /// through a child of a child the runs before it found (`demands`).
    pub(in crate::elab) fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<crate::elab::structure::reach::demands::NestedDemand>) -> Self {
        ElaborationState {
            design,
            types,
            consts: ConstEval::new(design, types),
            builder: IrBuilder::new(),
            locator: Locator::new(design),
            overrides,
            impls: crate::types::ImplTable::new(design),
            diagnostics: Vec::new(),
            values: ExpressionTables::default(),
            functions: FunctionTables::default(),
            analog: AnalogSites::default(),
            digital: DigitalTables::default(),
            structure: StructureTables::new(demands),
        }
    }
}

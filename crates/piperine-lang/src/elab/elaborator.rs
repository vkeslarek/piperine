//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`.

use std::collections::BTreeMap;

use piperine_ir::{BodyId, Ir, IrBuilder, NatureId, ValueType};

use super::body::{ModulePlan, ParamPlan};
use super::carriers::{Frame, Structure};
use super::consts::{Bindings, ConstEval};
use super::exits::Divergences;
use super::origin::Locator;
use super::signal_flow::SignalFlowCheck;
use super::structural::{BodyKey, BodyVariants, KeyParams, Overrides};
use super::unroll::CallCycles;
use super::variants::Variants;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, Design, DesignDiagnostic, FileId};
use crate::types::{Types, UnfinishedForm};

/// The entry point: `Elaborator::new(&design).run("Top")`.
#[derive(Debug, Clone)]
pub struct Elaborator<'d> {
    design: &'d Design,
    /// The unfinished forms the type check admits: none for production, the ones a test names otherwise.
    forms: Vec<UnfinishedForm>,
}

impl<'d> Elaborator<'d> {
    pub fn new(design: &'d Design) -> Self {
        Elaborator { design, forms: Vec::new() }
    }

    /// This elaborator with the unfinished `forms` admitted by its type check (the gate of generics phase C; tests only).
    pub fn admitting(mut self, forms: &[UnfinishedForm]) -> Self {
        self.forms = forms.to_vec();
        self
    }

    /// Elaborate `top`, a module of the root crate.
    ///
    /// The front-end and type-check diagnostics come back unchanged; an unknown top is one
    /// diagnostic naming it, and so is a top with a value input (`E0811`). A discrete port of the
    /// top is a net the host drives (§5.2.3), and a node port is driven by the host source the IR
    /// gains for it (§8.1.14, `host_sources.rs`).
    pub fn run(self, top: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        self.run_with(top, &Overrides::none())
    }

    /// [`run`](Self::run) with the params a host bound before the run: a binding of a key param
    /// selects the variant of its value at that instance (`Overrides`); any other is ignored.
    pub fn run_with(self, top: &str, overrides: &Overrides) -> Result<Ir, Vec<DesignDiagnostic>> {
        let types = Types::check_admitting(self.design, top, &self.forms)?;
        let top_decl = types.reached()[0];
        Elaboration::new(self.design, &types, overrides.clone()).run(top_decl)
    }
}

/// The state of one run. Its passes are `impl` blocks in the files named after them.
pub(super) struct Elaboration<'d, 't> {
    pub(super) design: &'d Design,
    pub(super) types: &'t Types<'d>,
    pub(super) consts: ConstEval<'d, 't>,
    pub(super) ir: IrBuilder,
    pub(super) locator: Locator<'d>,
    pub(super) natures: BTreeMap<DeclId, NatureId>,
    /// The body each reached module will get: bodies are built in first-reach order, so a
    /// placement can name its child's body before that body exists.
    pub(super) body_ids: BTreeMap<DeclId, BodyId>,
    /// The IR function of each `fn` and flag pattern a call reached.
    pub(super) variants: Variants,
    /// The type of every IR expression added so far, by id.
    pub(super) expr_types: Vec<ValueType>,
    /// Whether each IR expression added so far is continuous (part III §8.2), by id.
    pub(super) expr_continuous: Vec<bool>,
    /// Whether each IR statement added so far reads a continuous value, by id.
    pub(super) stmt_continuous: Vec<bool>,
    /// The origin of every IR expression added so far, by id.
    pub(super) expr_origins: Vec<piperine_ir::Origin>,
    /// The plan of every body lowered, by module and values of its key params.
    pub(super) plans: BTreeMap<BodyKey, ModulePlan>,
    /// The params of each reached module with a body, planned before any body is lowered.
    pub(super) param_plans: BTreeMap<DeclId, Vec<ParamPlan>>,
    /// The key params of each reached module with a body (`structural.rs`).
    pub(super) keys: KeyParams,
    /// The variants of keyed bodies requested so far.
    pub(super) body_variants: BodyVariants,
    /// The params the host bound before the run.
    pub(super) overrides: Overrides,
    /// The carriers, drives, events and reactions of the body being lowered.
    pub(super) structure: Structure,
    /// The frames of the body being lowered: frame 0 is its own module.
    pub(super) frames: Vec<Frame>,
    /// The index each placement of the body being lowered takes among its children (`reach.rs`).
    pub(super) child_indices: BTreeMap<crate::pom::ExprId, u32>,
    /// The reach frame of each child a body reads: by the frame that reads it and the placement.
    /// The values of the key params of the body being lowered: what its initial values fold over.
    pub(super) body_keys: Bindings,
    /// The ammeter branch of each port whose `inflow()` the body being lowered reads (`inflow.rs`).
    pub(super) ammeters: BTreeMap<crate::pom::DeclId, u32>,
    pub(super) reach_frames: BTreeMap<(usize, crate::pom::ExprId), usize>,
    /// The diverging expressions lowered so far (`exits.rs`).
    pub(super) divergences: Divergences,
    /// How many placements of a module an unrolled loop has hoisted so far, by the frame that
    /// places them (`unroll.rs`).
    pub(super) unrolled_names: BTreeMap<(usize, DeclId), u32>,
    /// The placements inside a loop, by module (`unroll.rs`).
    pub(super) looped: BTreeMap<DeclId, std::collections::BTreeSet<crate::pom::ExprId>>,
    /// The functions on a call cycle (`unroll.rs`).
    pub(super) cycles: CallCycles,
    /// The templates the params of module type hold (`templates.rs`).
    pub(super) templates: super::templates::Templates,
    /// The IR type each reached struct and enum has (`types.rs`).
    pub(super) type_arena: super::types::TypeArena,
    /// Every trait and `impl` of the design: what a call through a bound resolves against (`methods.rs`).
    pub(super) impls: crate::types::ImplTable<'d>,
    pub(super) diagnostics: Vec<DesignDiagnostic>,
}

impl<'d, 't> Elaboration<'d, 't> {
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides) -> Self {
        Elaboration {
            design,
            types,
            consts: ConstEval::new(design, types),
            ir: IrBuilder::new(),
            locator: Locator::new(design),
            natures: BTreeMap::new(),
            body_ids: BTreeMap::new(),
            variants: Variants::default(),
            expr_types: Vec::new(),
            expr_continuous: Vec::new(),
            stmt_continuous: Vec::new(),
            expr_origins: Vec::new(),
            plans: BTreeMap::new(),
            param_plans: BTreeMap::new(),
            keys: KeyParams::default(),
            overrides,
            body_variants: BodyVariants::default(),
            structure: Structure::default(),
            frames: Vec::new(),
            child_indices: BTreeMap::new(),
            body_keys: Bindings::new(),
            ammeters: BTreeMap::new(),
            reach_frames: BTreeMap::new(),
            divergences: Divergences::default(),
            unrolled_names: BTreeMap::new(),
            looped: BTreeMap::new(),
            cycles: CallCycles::default(),
            templates: super::templates::Templates::default(),
            type_arena: super::types::TypeArena::default(),
            impls: crate::types::ImplTable::new(design),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.refuse_value_inputs(top) {
            return Err(self.diagnostics);
        }
        // A module with a result or a value input gets no body: it is hoisted into each body that
        // places it. Its natures are still the IR's before any body.
        let reached: Vec<DeclId> = self.types.reached().iter().copied().filter(|d| matches!(&self.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        let modules: Vec<DeclId> = reached.iter().copied().filter(|d| !self.is_hoisted(*d)).collect();
        self.read_included_files();
        self.build_type_arena();
        self.plan_params(&modules);
        self.classify_key_params(&modules);
        // An unkeyed module keeps the id of its place in first-reach order; a keyed module's
        // variants take the ids after them, on request.
        let unkeyed: Vec<DeclId> = modules.iter().copied().filter(|m| self.keys.of(*m).is_empty()).collect();
        self.body_ids = unkeyed.iter().enumerate().map(|(k, m)| (*m, BodyId(k as u32))).collect();
        self.body_variants = BodyVariants::starting_at(unkeyed.len() as u32);
        self.lower_natures(&reached);
        for module in &unkeyed {
            if let Some(id) = self.body_ids.get(module).copied() {
                self.lower_body(*module, &Bindings::new(), BodyKey::unkeyed(*module), id);
            }
        }
        self.lower_pending_bodies();
        self.pad_child_nodes();
        let top_instance = self.place_top(top);
        let reprs = self.lower_hand_written_reprs();
        self.add_functions();
        self.set_hand_written_reprs(reprs);
        // the last net: an elaboration that placed no top and said nothing is refused by name, never an empty `Err`
        if top_instance.is_none() && self.diagnostics.is_empty() {
            let (file, span) = (self.design.decl_data(top).file, self.design.decl_data(top).span);
            self.refuse_silent_failure(file, span, "the top module (its elaboration failed without a diagnostic)");
        }
        match top_instance {
            Some(instance) if self.diagnostics.is_empty() => {
                let ir = self.ir.finish(instance);
                self.diagnostics.extend(Self::signal_flow_diagnostics(self.design, top, &ir));
                if !self.diagnostics.is_empty() {
                    return Err(self.diagnostics);
                }
                Self::well_formed(self.design, top, ir)
            }
            _ => Err(self.diagnostics),
        }
    }

    /// The `Ir` itself when `check()` finds nothing; else one `P0004` internal-error diagnostic
    /// per broken invariant, at the top module, and no `Ir`. Never checked in debug builds only:
    /// a lowering gap must not reach a backend as an `Ok`.
    pub(super) fn well_formed(design: &Design, top: DeclId, ir: Ir) -> Result<Ir, Vec<DesignDiagnostic>> {
        let broken = ir.check();
        if broken.is_empty() {
            return Ok(ir);
        }
        let data = design.decl_data(top);
        let span = data.name.as_ref().map_or(data.span, |n| n.span);
        Err(broken
            .iter()
            .map(|b| DesignDiagnostic {
                file: data.file,
                diagnostic: Diagnostic::new("P0004", format!("internal error: elaboration produced an ill-formed IR: {} ({} at {})", b.what, b.item, b.origin), span, "this is a bug in the elaborator, not in the model"),
            })
            .collect())
    }

    /// `E0242` for each signal-flow node of `ir` that does not have exactly one potential driver
    /// (§6 rule 2), reported at the top module and naming the node and each driver.
    fn signal_flow_diagnostics(design: &Design, top: DeclId, ir: &Ir) -> Vec<DesignDiagnostic> {
        let data = design.decl_data(top);
        let (file, span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
        SignalFlowCheck::run(ir)
            .into_iter()
            .map(|fault| {
                let drivers = if fault.drivers.is_empty() { "none".to_string() } else { fault.drivers.join(", ") };
                let message = format!("the signal-flow node `{}` has {} potential driver(s), it takes exactly one (§6 rule 2); drivers: {drivers}", fault.node, fault.drivers.len());
                DesignDiagnostic { file, diagnostic: Diagnostic::new("E0242", message, span, "a signal-flow node with the wrong number of potential drivers") }
            })
            .collect()
    }

    /// Records `diagnostic`, once: a construct lowered in two halves (the `is` and the `val` of an `Option`)
    /// reports the same refusal at the same place twice, which is one finding.
    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        let found = DesignDiagnostic { file, diagnostic };
        if !self.diagnostics.contains(&found) {
            self.diagnostics.push(found);
        }
    }

    /// `P0001`: a construct inside the type-checked subset that M1 elaboration does not lower.
    pub(super) fn unsupported(&mut self, file: FileId, span: Span, what: &str) {
        self.report(file, Diagnostic::new("P0001", format!("{what} is not supported yet"), span, format!("{what} is outside the M1 subset")));
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{InstanceId, IrBuilder};

    use super::*;

    #[test]
    fn an_ill_formed_ir_becomes_an_internal_error_and_no_ir() {
        let design = Design::builder().source("top.msl", "module Top() { }").build();
        let types = Types::check(&design, "Top").expect("a clean model");
        // No instance at all: the top id dangles, so `check()` has something to say.
        let ir = IrBuilder::new().finish(InstanceId(0));
        assert!(!ir.check().is_empty(), "the crafted IR must be broken");
        let err = Elaboration::well_formed(&design, types.reached()[0], ir).expect_err("no Ir");
        assert!(err.iter().all(|d| d.diagnostic.code == "P0004" && d.diagnostic.message.contains("internal error")), "{err:?}");
    }
}

//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`.

use std::collections::BTreeMap;

use piperine_ir::{BodyId, Ir, IrBuilder, NatureId, ValueType};

use super::body::{ModulePlan, ParamPlan};
use super::carriers::{Frame, Structure};
use super::consts::{Bindings, ConstEval};
use super::exits::Divergences;
use super::loops::DriveLoops;
use super::origin::Locator;
use super::signal_flow::SignalFlowCheck;
use super::structural::{BodyKey, BodyVariants, KeyParams, Overrides};
use super::unroll::CallCycles;
use super::variants::Variants;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, Design, DesignDiagnostic, FileId};
use crate::types::Types;

/// The entry point: `Elaborator::new(&design).run("Top")`.
#[derive(Debug, Clone)]
pub struct Elaborator<'d> {
    design: &'d Design,
}

impl<'d> Elaborator<'d> {
    pub fn new(design: &'d Design) -> Self {
        Elaborator { design }
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
        let types = Types::check(self.design, top)?;
        let top_decl = types.reached()[0];
        // the reads the text names through children of children are the demands of the first run (`reach_seed.rs`); a run that meets
        // one its tables lack (a hop through a type-parameter template) says so, and is made again with it
        // (`reach_through_templates.rs`)
        let mut demands = Elaboration::new(self.design, &types, overrides.clone(), Vec::new()).static_demands();
        loop {
            let (result, wanted) = Elaboration::new(self.design, &types, overrides.clone(), demands.clone()).run(top_decl);
            if wanted.is_empty() {
                return result;
            }
            demands.extend(wanted);
        }
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
    /// The params of each instance of a generic module requested so far, planned under its arguments (`generic_bodies.rs`).
    pub(super) instance_param_plans: BTreeMap<super::instances::Instance, Vec<ParamPlan>>,
    /// How deep each body requested so far sits in the chain of module instances that asked for it (`generic_bodies.rs`).
    pub(super) body_depths: BTreeMap<BodyKey, usize>,
    /// The body being lowered.
    pub(super) lowering: Option<BodyKey>,
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
    /// The items of the body being lowered that are written inside generation, and how each unrolls (`generation.rs`).
    pub(super) generated: super::generation::GeneratedItems,
    /// The values of the key params of the body being lowered: what its initial values fold over.
    pub(super) body_keys: Bindings,
    /// The ammeter branch of each port whose `inflow()` the body being lowered reads (`inflow.rs`).
    pub(super) ammeters: BTreeMap<crate::pom::DeclId, u32>,
    /// The reach frame of each child a frame reads, by the frame that reads it and the placement (`reach.rs`).
    pub(super) reach_frames: BTreeMap<(usize, super::hoist::PlacementKey), usize>,
    /// What the design reads through a child of a child, by module (`reach_through_templates.rs`).
    pub(super) nested_demands: super::reach_through_templates::NestedDemands,
    /// The diverging expressions lowered so far (`exits.rs`).
    pub(super) divergences: Divergences,
    /// How many placements of a module an unrolled loop has hoisted so far, by the frame that
    /// places them (`unroll.rs`).
    pub(super) unrolled_names: BTreeMap<(usize, DeclId), u32>,
    /// The placements inside a loop, by module (`unroll.rs`).
    pub(super) looped: BTreeMap<DeclId, std::collections::BTreeSet<crate::pom::ExprId>>,
    /// The functions on a call cycle (`unroll.rs`).
    pub(super) cycles: CallCycles,
    /// The IR type each reached struct and enum has (`types.rs`).
    pub(super) type_arena: super::types::TypeArena,
    /// Every trait and `impl` of the design: what a call through a bound resolves against (`methods.rs`).
    pub(super) impls: crate::types::ImplTable<'d>,
    /// The resolved nets of the placed tree and their resolver bodies (`resolvers.rs`).
    pub(super) resolvers: super::resolvers::ResolverBodies,
    pub(super) diagnostics: Vec<DesignDiagnostic>,
}

impl<'d, 't> Elaboration<'d, 't> {
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<super::reach_through_templates::NestedDemand>) -> Self {
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
            instance_param_plans: BTreeMap::new(),
            body_depths: BTreeMap::new(),
            lowering: None,
            keys: KeyParams::default(),
            overrides,
            body_variants: BodyVariants::default(),
            structure: Structure::default(),
            frames: Vec::new(),
            child_indices: BTreeMap::new(),
            generated: super::generation::GeneratedItems::default(),
            body_keys: Bindings::new(),
            ammeters: BTreeMap::new(),
            reach_frames: BTreeMap::new(),
            nested_demands: super::reach_through_templates::NestedDemands::known(demands),
            divergences: Divergences::default(),
            unrolled_names: BTreeMap::new(),
            looped: BTreeMap::new(),
            cycles: CallCycles::default(),
            type_arena: super::types::TypeArena::default(),
            impls: crate::types::ImplTable::new(design),
            resolvers: super::resolvers::ResolverBodies::default(),
            diagnostics: Vec::new(),
        }
    }

    /// Copies the wide integer constants the evaluator made into the IR, in handle order, so each
    /// `Constant::Wide` and `Expr::Wide` the elaboration emitted names the entry it was made as
    /// (`consts/numbers.rs`).
    fn sync_wides(&mut self) {
        for data in self.consts.take_new_wides() {
            self.ir.wide(data);
        }
    }

    /// The run, and the demands it found that its tables did not hold (`reach_through_templates.rs`): when there are any, the
    /// result is not the run's and the caller runs again with them.
    fn run(mut self, top: DeclId) -> (Result<Ir, Vec<DesignDiagnostic>>, Vec<super::reach_through_templates::NestedDemand>) {
        let result = self.elaborate(top);
        (result, self.nested_demands.into_wanted())
    }

    /// The passes of one run.
    fn elaborate(&mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.refuse_value_inputs(top) {
            return Err(std::mem::take(&mut self.diagnostics));
        }
        // A module with a result or a value input gets no body: it is hoisted into each body that
        // places it. Its natures are still the IR's before any body.
        let reached: Vec<DeclId> = self.types.reached().iter().copied().filter(|d| matches!(&self.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        let modules: Vec<DeclId> = reached.iter().copied().filter(|d| !self.is_hoisted(*d)).collect();
        // a generic module has no body of its own: each instance a placement makes is one (`generic_bodies.rs`)
        let with_generic = modules.clone();
        let modules: Vec<DeclId> = modules.into_iter().filter(|m| !self.is_generic_module(*m)).collect();
        self.read_included_files();
        self.build_type_arena();
        self.plan_params(&modules);
        self.classify_module_param_keys(&with_generic);
        self.classify_folded_param_keys(&with_generic);
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
        // a body variant the placement asked for after the first padding (the top's own, keyed by a host override) is padded too
        self.pad_child_nodes();
        self.build_resolvers();
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
                self.sync_wides();
                let ir = std::mem::replace(&mut self.ir, IrBuilder::new()).finish(instance);
                self.diagnostics.extend(Self::signal_flow_diagnostics(self.design, top, &ir));
                self.diagnostics.extend(Self::loop_diagnostics(self.design, top, &ir));
                if !self.diagnostics.is_empty() {
                    return Err(std::mem::take(&mut self.diagnostics));
                }
                Self::well_formed(self.design, top, ir)
            }
            _ => Err(std::mem::take(&mut self.diagnostics)),
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

    /// `E0339` for each loop of undelayed drives of `ir` that the placed tree closes through a one-driver net
    /// (§8.2.3), reported at the top module and naming the drives.
    fn loop_diagnostics(design: &Design, top: DeclId, ir: &Ir) -> Vec<DesignDiagnostic> {
        let data = design.decl_data(top);
        let (file, span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
        DriveLoops::run(ir)
            .into_iter()
            .map(|fault| {
                let message = format!("undelayed definitions of {} depend on each other across instances: a loop through one-driver carriers (§8.2.3)", fault.drives.iter().map(|drive| format!("`{drive}`")).collect::<Vec<_>>().join(", "));
                DesignDiagnostic { file, diagnostic: Diagnostic::new("E0339", message, span, "a loop of definitions") }
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

    /// `P0004`: an invariant of the elaborator broken (a bug in it, not in the model), reported at the module of the body being
    /// lowered; the run then gives no `Ir`, as for an ill-formed one (`well_formed`).
    pub(super) fn internal_error(&mut self, what: &str) {
        // every lowering runs with frame 0 set; before one, the body asked for (`lowering`) names the module
        let module = self.frames.first().map(|frame| frame.module).or_else(|| self.lowering.as_ref().map(|key| key.module));
        let (file, span) = match module {
            Some(module) => {
                let data = self.design.decl_data(module);
                (data.file, data.name.as_ref().map_or(data.span, |n| n.span))
            }
            None => (FileId(0), Span::default()),
        };
        self.report(file, Diagnostic::new("P0004", format!("internal error: {what}"), span, "this is a bug in the elaborator, not in the model"));
    }

    /// `P0001`: a construct inside the type-checked subset that M1 elaboration does not lower.
    pub(super) fn unsupported(&mut self, file: FileId, span: Span, what: &str) {
        self.report(file, Diagnostic::new("P0001", format!("{what} is not supported yet"), span, format!("{what} is outside the M1 subset")));
    }

    /// [`unsupported`](Self::unsupported) unless the run has refused something already: a read that fails because of a refusal
    /// reported before it (a child whose own lowering was refused) adds no second diagnostic for the same cause.
    pub(super) fn unsupported_after_no_refusal(&mut self, file: FileId, span: Span, what: &str) {
        if self.diagnostics.is_empty() {
            self.unsupported(file, span, what);
        }
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

    #[test]
    fn a_composite_constant_with_no_form_at_its_type_is_an_internal_error_not_a_panic() {
        // the checker's types never ask `constant_expr` for an array at `bool`; asked anyway, the elaborator says `P0004` at the
        // module being lowered and goes on, so the run ends with that diagnostic and no `Ir`
        let design = Design::builder().source("top.msl", "module Top() { }").build();
        let types = Types::check(&design, "Top").expect("a clean model");
        let top = types.reached()[0];
        let mut elaboration = Elaboration::new(&design, &types, Overrides::default(), Vec::new());
        elaboration.frames = vec![Frame::root(top)];
        let pair = elaboration.ir.aggregate(piperine_ir::AggregateData { ty: ValueType::Array(Box::new(ValueType::F64), 2), variant: None, parts: vec![piperine_ir::Constant::Real(piperine_ir::Real::new(1.0)); 2] });
        let origin = piperine_ir::Origin { file: "top.msl".to_string(), line: 1, col: 1 };
        elaboration.constant_expr(piperine_ir::Constant::Aggregate(pair), ValueType::Bool, origin);
        let codes: Vec<&str> = elaboration.diagnostics.iter().map(|d| d.diagnostic.code.as_ref()).collect();
        assert_eq!(codes, ["P0004"], "{:?}", elaboration.diagnostics);
        let found = &elaboration.diagnostics[0];
        assert!(found.diagnostic.message.starts_with("internal error: the composite constant") && found.file == design.decl_data(top).file, "{found:?}");
    }
}

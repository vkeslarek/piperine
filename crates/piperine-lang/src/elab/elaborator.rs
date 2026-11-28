//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`. What every concern reads of the run (its
//! diagnostics, the types it was given, the nodes it emits, the queries of the design) is `impl Elaboration` in the
//! files below `elaborator/`.

mod diagnose;
mod emitted;
mod imports;
mod naming;
mod origin;
mod queries;
mod scope;
mod state;
mod typing;

pub(super) use scope::Scope;
pub(super) use state::ElaborationState;

use crate::elab::analog::AnalogLowering;
use crate::elab::digital::DigitalLowering;
use crate::elab::functions::FunctionLowering;
use crate::elab::structure::StructureLowering;
use crate::elab::value::ValueLowering;

use piperine_ir::{BodyId, Ir, IrBuilder};

use super::consts::{Bindings};
use super::digital::loops::DriveLoops;
use super::analog::signal_flow::SignalFlowCheck;
use super::structure::structural::{BodyKey, BodyVariants, Overrides};
use crate::refusal::forms::ValueForm;
use crate::msl::{Diagnostic};
use crate::pom::{DeclId, DeclKind, Design, DesignDiagnostic};
use crate::types::{Types};

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
        self.run_with_statistics(top, overrides).0
    }

    /// [`run_with`](Self::run_with), and what the run cost ([`RunStatistics`]).
    pub fn run_with_statistics(self, top: &str, overrides: &Overrides) -> (Result<Ir, Vec<DesignDiagnostic>>, RunStatistics) {
        let mut statistics = RunStatistics::default();
        let types = match Types::check(self.design, top) {
            Ok(types) => types,
            Err(diagnostics) => return (Err(diagnostics), statistics),
        };
        let top_decl = types.reached()[0];
        // the reads the text names through children of children are the demands of the first run (`reach/seed.rs`); a run that meets
        // one its tables lack (a hop through a type-parameter template) says so, and is made again with it
        // (`reach/demands.rs`)
        let mut demands = Elaboration::new(self.design, &types, overrides.clone(), Vec::new()).structure().static_demands();
        loop {
            let (result, wanted) = Elaboration::new(self.design, &types, overrides.clone(), demands.clone()).run(top_decl);
            statistics.passes += 1;
            if wanted.is_empty() {
                return (result, statistics);
            }
            demands.extend(wanted);
        }
    }
}

/// What one run of the [`Elaborator`] cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunStatistics {
    /// How many times the design was elaborated: one, plus one for each run that met a read through a child of a child its tables
    /// lacked (a hop through a type-parameter template, `reach/demands.rs`); zero when the type check refused the design.
    pub passes: usize,
}

/// One run: its state and the order of its passes. A concern's lowering is a view of it (`values`, `functions`, `analog`,
/// `digital`, `structure`); what every concern reads of the run is `impl Elaboration` in the files below `elaborator/`.
pub(super) struct Elaboration<'d, 't> {
    /// The inputs of the run, the IR being built, and one table per concern.
    pub(super) state: ElaborationState<'d, 't>,
}

impl<'d, 't> Elaboration<'d, 't> {
    /// The view that lowers functions.
    pub(super) fn functions(&mut self) -> FunctionLowering<'_, 'd, 't> {
        FunctionLowering::new(self)
    }

    /// The view that lowers discrete code.
    pub(super) fn digital(&mut self) -> DigitalLowering<'_, 'd, 't> {
        DigitalLowering::new(self)
    }

    /// The view that lowers module structure.
    pub(super) fn structure(&mut self) -> StructureLowering<'_, 'd, 't> {
        StructureLowering::new(self)
    }

    /// The view that lowers analog code.
    pub(super) fn analog(&mut self) -> AnalogLowering<'_, 'd, 't> {
        AnalogLowering::new(self)
    }

    /// The view that lowers value code.
    pub(super) fn values(&mut self) -> ValueLowering<'_, 'd, 't> {
        ValueLowering::new(self)
    }

    /// A run over `design`, typed as `types`, with the params the host bound (`overrides`) and the reads through a child
    /// of a child the runs before it found (`demands`).
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<super::structure::reach::demands::NestedDemand>) -> Self {
        Elaboration { state: ElaborationState::new(design, types, overrides, demands) }
    }

    /// Copies the wide integer constants the evaluator made into the IR, in handle order, so each
    /// `Constant::Wide` and `Expr::Wide` the elaboration emitted names the entry it was made as
    /// (`consts/numbers.rs`).
    fn sync_wides(&mut self) {
        for data in self.state.consts.take_new_wides() {
            self.state.builder.wide(data);
        }
    }

    /// The run, and the demands it found that its tables did not hold (`reach/demands.rs`): when there are any, the
    /// result is not the run's and the caller runs again with them.
    fn run(mut self, top: DeclId) -> (Result<Ir, Vec<DesignDiagnostic>>, Vec<super::structure::reach::demands::NestedDemand>) {
        let result = self.elaborate(top);
        (result, self.state.structure.nested_demands.into_wanted())
    }

    /// The passes of one run.
    fn elaborate(&mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.analog().refuse_value_inputs(top) {
            return Err(std::mem::take(&mut self.state.diagnostics));
        }
        // A module with a result or a value input gets no body: it is hoisted into each body that
        // places it. Its natures are still the IR's before any body.
        let reached: Vec<DeclId> = self.state.types.reached().iter().copied().filter(|d| matches!(&self.state.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        let modules: Vec<DeclId> = reached.iter().copied().filter(|d| !self.is_hoisted(*d)).collect();
        // a generic module has no body of its own: each instance a placement makes is one (`generic_bodies.rs`)
        let with_generic = modules.clone();
        let modules: Vec<DeclId> = modules.into_iter().filter(|m| !self.structure().is_generic_module(*m)).collect();
        self.values().read_included_files();
        self.values().build_type_arena();
        self.structure().plan_params(&modules);
        self.structure().classify_module_param_keys(&with_generic);
        self.structure().classify_folded_param_keys(&with_generic);
        self.structure().classify_key_params(&modules);
        // An unkeyed module keeps the id of its place in first-reach order; a keyed module's
        // variants take the ids after them, on request.
        let unkeyed: Vec<DeclId> = modules.iter().copied().filter(|m| self.state.structure.keys.of(*m).is_empty()).collect();
        self.state.structure.body_ids = unkeyed.iter().enumerate().map(|(k, m)| (*m, BodyId(k as u32))).collect();
        self.state.structure.body_variants = BodyVariants::starting_at(unkeyed.len() as u32);
        self.analog().lower_natures(&reached);
        for module in &unkeyed {
            if let Some(id) = self.state.structure.body_ids.get(module).copied() {
                self.structure().lower_body(*module, &Bindings::new(), BodyKey::unkeyed(*module), id);
            }
        }
        self.structure().lower_pending_bodies();
        self.structure().pad_child_nodes();
        let top_instance = self.structure().place_top(top);
        // a body variant the placement asked for after the first padding (the top's own, keyed by a host override) is padded too
        self.structure().pad_child_nodes();
        self.digital().build_resolvers();
        let reprs = self.values().lower_hand_written_reprs();
        self.functions().add_functions();
        self.values().set_hand_written_reprs(reprs);
        // the last net: an elaboration that placed no top and said nothing is refused by name, never an empty `Err`
        if top_instance.is_none() && self.state.diagnostics.is_empty() {
            let (file, span) = (self.state.design.decl_data(top).file, self.state.design.decl_data(top).span);
            self.refuse_silent_failure(file, span, ValueForm::TopModuleItsElaborationFailedWithoutDiagnostic);
        }
        match top_instance {
            Some(instance) if self.state.diagnostics.is_empty() => {
                self.sync_wides();
                let ir = std::mem::replace(&mut self.state.builder, IrBuilder::new()).finish(instance);
                self.state.diagnostics.extend(Self::signal_flow_diagnostics(self.state.design, top, &ir));
                self.state.diagnostics.extend(Self::loop_diagnostics(self.state.design, top, &ir));
                if !self.state.diagnostics.is_empty() {
                    return Err(std::mem::take(&mut self.state.diagnostics));
                }
                Self::well_formed(self.state.design, top, ir)
            }
            _ => Err(std::mem::take(&mut self.state.diagnostics)),
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

}

#[cfg(test)]
mod tests {
    use piperine_ir::{InstanceId, IrBuilder, ValueType};

    use super::*;
    use crate::elab::digital::carriers::Frame;

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
        elaboration.state.structure.frames = vec![Frame::root(top)];
        let pair = elaboration.state.builder.aggregate(piperine_ir::AggregateData { ty: ValueType::Array(Box::new(ValueType::F64), 2), variant: None, parts: vec![piperine_ir::Constant::Real(piperine_ir::Real::new(1.0)); 2] });
        let origin = piperine_ir::Origin { file: "top.msl".to_string(), line: 1, col: 1 };
        elaboration.values().constant_expr(piperine_ir::Constant::Aggregate(pair), ValueType::Bool, origin);
        let codes: Vec<&str> = elaboration.state.diagnostics.iter().map(|d| d.diagnostic.code.as_ref()).collect();
        assert_eq!(codes, ["P0004"], "{:?}", elaboration.state.diagnostics);
        let found = &elaboration.state.diagnostics[0];
        assert!(found.diagnostic.message.starts_with("internal error: the composite constant") && found.file == design.decl_data(top).file, "{found:?}");
    }
}

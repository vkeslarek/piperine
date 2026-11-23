//! The reference interpreter (feature `ir-interp`): an elaborated [`Ir`] run through the solver
//! by walking each body's `analog` block over dual numbers. It spawns no process and reads no
//! file; the solver meets it through shapes and `eval` only (abi.md).

use std::sync::Arc;

use piperine_abi::raw::Module;
use piperine_abi::thunk::module_with;

mod composite;
mod device;
mod dual;
mod generics;
mod operators;
mod param_types;
mod widths;
mod walk;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;

pub use device::{BodyFactory, Interpreted};
pub use dual::Dual;
pub use piperine_plan::BodyPlan;
pub use walk::{Value, Walk};
use std::collections::HashSet;

use piperine_ir::{AnalysisKind, Body, BodyId, BranchKind, Broken, Env, EventSource, Ir, Stmt, StmtId, Target};
use composite::CompositeForms;
use generics::GenericForms;
use operators::OperatorForms;
use param_types::ParamTypes;
use widths::NumberWidths;
use piperine_plan::{Modules, Netlist};
use piperine_solver::prelude::CircuitInstance;

/// An `Ir` that failed `check()`: the entries, each naming an item and its origin.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub entries: Vec<Broken>,
}

/// A checked `Ir`, each body that runs planned and turned into a module.
#[derive(Debug, Clone)]
pub struct Interpreter {
    ir: Arc<Ir>,
    /// One module per body, by body index; `None` for a body with nothing to run.
    modules: Vec<Option<&'static Module>>,
}

impl Interpreter {
    /// Accepts an `Ir` whose `check()` is empty; otherwise returns the entries.
    ///
    /// Each body with an `analog` block or a node with an initial potential is planned and
    /// becomes one module. The solver binds a `&'static Module`, so each such body leaks one
    /// `Module`, one `BodyFactory` and one name per `Interpreter`: a count bounded by the bodies
    /// of the design.
    pub fn new(ir: Arc<Ir>) -> Result<Interpreter, Rejected> {
        let entries = ir.check();
        if !entries.is_empty() {
            return Err(Rejected { entries });
        }
        // R1: the interpreter is frozen at the M1 analog subset; a digital body is named, not run,
        // and so is a body that holds a form the msl-digital amendments added.
        let mut refused: Vec<Broken> = Vec::new();
        for b in ir.bodies() {
            if !(b.carriers().is_empty() && b.drives().is_empty() && b.events().is_empty() && b.reactions().is_empty()) {
                refused.push(Broken {
                    item: format!("body {}", b.name()),
                    origin: b.origin().clone(),
                    what: format!("body {} has carriers, drives or reactions: the interpreter runs the M1 analog subset only (use the compiled backend)", b.name()),
                });
            }
            for what in NumberWidths::refused(b) {
                refused.push(Broken { item: format!("body {}", b.name()), origin: b.origin().clone(), what });
            }
            for form in Self::amended_forms(b).into_iter().chain(Self::structure_forms(b)).chain(CompositeForms::of(b)).chain(OperatorForms::of(b)).map(String::from).chain(GenericForms::of(b)) {
                refused.push(Broken {
                    item: format!("body {}", b.name()),
                    origin: b.origin().clone(),
                    what: format!("body {} holds {form}: the interpreter runs the M1 analog subset only", b.name()),
                });
            }
            for what in ParamTypes::refused(b) {
                refused.push(Broken { item: format!("body {}", b.name()), origin: b.origin().clone(), what });
            }
        }
        refused.extend(GenericForms::hand_written_types(&ir));
        if !refused.is_empty() {
            return Err(Rejected { entries: refused });
        }
        let modules = ir
            .bodies()
            .map(|body| {
                let runs = body.analog().is_some() || body.nodes().iter().any(|n| n.init.is_some());
                runs.then(|| {
                    let plan = Arc::new(BodyPlan::new(body));
                    let factory = BodyFactory { ir: Arc::clone(&ir), body: body.id(), plan };
                    let factory: &'static BodyFactory = Box::leak(Box::new(factory));
                    let name: &'static str = Box::leak(body.name().to_string().into_boxed_str());
                    let module: &'static Module = Box::leak(Box::new(module_with(name, factory)));
                    module
                })
            })
            .collect();
        Ok(Interpreter { ir, modules })
    }

    /// Every dependency query of `body`: each branch's two parts, each site, each param, and each
    /// drive, event, reaction, `bound_step` site and `cross` tolerance.
    fn targets(body: Body<'_>) -> Vec<Target> {
        let mut targets: Vec<Target> = Vec::new();
        for i in 0..body.branches().len() as u32 {
            targets.extend([Target::Flow(i), Target::Charge(i)]);
        }
        targets.extend((0..body.limits().len() as u32).map(Target::Limit));
        targets.extend((0..body.stims().len() as u32).map(Target::Stim));
        targets.extend((0..body.params().len() as u32).map(Target::Check));
        for i in 0..body.drives().len() as u32 {
            targets.extend([Target::Drive(i), Target::DriveAfter(i)]);
        }
        for i in 0..body.events().len() as u32 {
            targets.extend([Target::Guard(i), Target::Cross(i), Target::At(i)]);
        }
        targets.extend((0..body.reactions().len() as u32).map(Target::Reaction));
        let bound_steps = body.analog().unwrap_or(&[]).iter().filter_map(|s| match body.ir().stmt(*s) {
            Stmt::BoundStep { site, .. } => Some(*site),
            _ => None,
        });
        targets.extend(bound_steps.map(Target::BoundStep));
        targets.extend((0..body.events().len() as u32).flat_map(|k| [Target::Ttol(k), Target::Etol(k)]));
        targets
    }

    /// The amended forms `body` holds, each once, in a fixed order: what its dependency sets read
    /// (the clock, the analysis, `freq()`, a branch flow, a previous value, a fired byte, through
    /// the functions it calls), the `at` events, emit sites, `bound_step` sites and `cross`
    /// tolerances it declares.
    fn amended_forms(body: Body<'_>) -> Vec<&'static str> {
        let targets = Self::targets(body);
        let (mut time, mut stat, mut flow, mut previous, mut fired, mut freq) = (false, false, false, false, false, false);
        let mut analysis = [false; 4];
        for t in targets {
            let deps = body.dependencies(t);
            time |= deps.env.contains(&Env::Time);
            stat |= deps.env.contains(&Env::Static);
            freq |= deps.env.contains(&Env::Freq);
            for (held, kind) in analysis.iter_mut().zip([AnalysisKind::Dc, AnalysisKind::Transient, AnalysisKind::Ac, AnalysisKind::Noise]) {
                *held |= deps.env.contains(&Env::Analysis(kind));
            }
            flow |= !body.flow_reads(t).is_empty();
            previous |= !body.previous_reads(t).is_empty();
            fired |= !body.fired_reads(t).is_empty();
        }
        let bound_step = body.analog().unwrap_or(&[]).iter().any(|s| matches!(body.ir().stmt(*s), Stmt::BoundStep { .. }));
        let ttol = body.events().iter().any(|e| matches!(e.source, EventSource::Cross { ttol: Some(_), .. }));
        let etol = body.events().iter().any(|e| matches!(e.source, EventSource::Cross { etol: Some(_), .. }));
        let at = body.events().iter().any(|e| matches!(e.source, EventSource::At(_)));
        let settle = body.events().iter().any(|e| matches!(e.source, EventSource::Settle));
        let emit = (0..body.reactions().len() as u32).any(|k| body.reaction_statements(k).iter().any(|s| matches!(body.ir().stmt(*s), Stmt::Emit { .. })));
        [
            (time, "now()"),
            (stat, "analysis(Static)"),
            (at, "at(t)"),
            // §13 rule 17: the settle phase is the digital engine's; the interpreter has none
            (settle, "settle"),
            (emit, "emit!"),
            (flow, "the flow of a potential branch"),
            (previous, "the previous value of a carrier"),
            (freq, "freq()"),
            (analysis[0], "analysis(Dc)"),
            (analysis[1], "analysis(Transient)"),
            (analysis[2], "analysis(Ac)"),
            (analysis[3], "analysis(Noise)"),
            (bound_step, "bound_step"),
            (fired, "fired"),
            (ttol, "the ttol of a cross"),
            (etol, "the etol of a cross"),
        ]
            .into_iter()
            .filter_map(|(held, form)| held.then_some(form))
            .collect()
    }

    /// The forms of `analog-structure` `body` holds, each once, in a fixed order: a contribution
    /// that is not a top-level statement of the `analog` block (under an `if`, a `match` arm or in
    /// a nested block: a selector), a switch branch, a module placed in an expression, a derivative
    /// site, a node or carrier reached in a child, an `inflow()` ammeter, `changed()` or `prev()`
    /// of a param, a `limit` read under a condition and a `bound_step` with a `when` (R1, AS-17).
    fn structure_forms(body: Body<'_>) -> Vec<&'static str> {
        let top: HashSet<StmtId> = body.analog().unwrap_or(&[]).iter().copied().collect();
        let selector = body.contributions().iter().any(|c| !top.contains(&c.stmt));
        let switch = body.branches().iter().any(|b| b.kind == BranchKind::Switch);
        let param_event = body.events().iter().any(|e| matches!(e.source, EventSource::ParamChanged(_)));
        let param_previous = Self::targets(body).into_iter().any(|t| !body.param_previous_reads(t).is_empty());
        let guarded_limit = body.limits().iter().any(|site| site.enabled.is_some());
        let guarded_step = body.analog().unwrap_or(&[]).iter().any(|s| matches!(body.ir().stmt(*s), Stmt::BoundStep { when: Some(_), .. }));
        [
            (selector, "a contribution under a selector"),
            (switch, "a switch branch"),
            (!body.hoisted().is_empty(), "a module placed in an expression with a node or an analog block"),
            (!body.derivatives().is_empty(), "a ddt off a whole addend or with abstol"),
            (!body.reaches().is_empty(), "a reached node or carrier (inst.name)"),
            (!body.inflows().is_empty(), "inflow()"),
            (param_event || param_previous, "changed() or prev() on a param"),
            (guarded_limit, "a guarded limit"),
            (guarded_step, "a bound_step under a selector"),
        ]
        .into_iter()
        .filter_map(|(held, form)| held.then_some(form))
        .collect()
    }

    /// The module of `body`; `None` for a body with no `analog` block and no held node, or an id
    /// out of range.
    pub fn module(&self, body: BodyId) -> Option<&'static Module> {
        self.modules.get(body.index()).copied().flatten()
    }

    /// The design this interpreter runs.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }

    /// The design as a solver circuit, each instance bound to its body's module
    /// (`Netlist::circuit`).
    pub fn circuit(&self) -> piperine_solver::prelude::Result<CircuitInstance> {
        Netlist::new(&self.ir).circuit(self)
    }
}

impl Modules for Interpreter {
    fn module(&self, body: BodyId) -> Option<&'static Module> {
        Interpreter::module(self, body)
    }
}

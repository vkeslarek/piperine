//! The reference interpreter (feature `ir-interp`): an elaborated [`Ir`] run through the solver
//! by walking each body's `analog` block over dual numbers. It spawns no process and reads no
//! file; the solver meets it through shapes and `eval` only (abi.md).

use std::sync::Arc;

use piperine_abi::raw::Module;
use piperine_abi::thunk::module_with;

mod device;
mod dual;
mod walk;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;

pub use device::{BodyFactory, Interpreted};
pub use dual::Dual;
pub use piperine_plan::BodyPlan;
pub use walk::{Value, Walk};
use piperine_ir::{BodyId, Broken, Ir};
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
        // R1: the interpreter is frozen at the M1 analog subset; a digital body is named, not run.
        let digital: Vec<Broken> = ir
            .bodies()
            .filter(|b| !(b.carriers().is_empty() && b.drives().is_empty() && b.events().is_empty() && b.reactions().is_empty()))
            .map(|b| Broken {
                item: format!("body {}", b.name()),
                origin: b.origin().clone(),
                what: format!("body {} has carriers, drives or reactions: the interpreter runs the M1 analog subset only (use the compiled backend)", b.name()),
            })
            .collect();
        if !digital.is_empty() {
            return Err(Rejected { entries: digital });
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

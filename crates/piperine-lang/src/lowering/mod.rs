//! Lower `Design` (PPR/PHDL) → `IrProgram`.

use std::collections::HashMap;

use crate::pom::Design;

use piperine_codegen::ir::*;

mod event;
mod expr;
mod stmt;
mod structure;

use structure::{convert_fn, convert_mod};
use stmt::lower_stmts;

// ─── Context ──────────────────────────────────────────────────────────────────

/// Lowering context carrying the current scope, state-variable counter, and
/// discovered noise-source list.
#[derive(Clone)]
pub(crate) struct LowerCtx {
    /// Name → IR expression bindings for the current scope.
    env: HashMap<String, IrExpr>,
    /// State variables (ddt, idt, etc.) collected during lowering.
    state_vars: Vec<IrStateVar>,
    /// Noise sources discovered from contribution right-hand sides.
    noise_sources: Vec<IrNoiseSource>,
    /// Monotonic counter for allocating state-variable ids.
    counter: u32,
    /// Set to `true` while lowering a `digital` body.  Lets the Bind-Force
    /// arm pick the digital-drive form (`IrStmt::Assign`) instead of the
    /// analog-force form (`IrStmt::Force`).
    is_digital: bool,
}

impl LowerCtx {
    /// Create a fresh lowering context with an empty environment.
    fn new() -> Self {
        Self {
            env: HashMap::new(),
            state_vars: vec![],
            noise_sources: vec![],
            counter: 0,
            is_digital: false,
        }
    }

    /// Allocate a new state variable of `kind`, returning a unique identifier.
    fn alloc_state(&mut self, kind: IrStateKind, arg: IrExpr) -> u32 {
        let id = self.counter;
        self.counter += 1;
        self.state_vars.push(IrStateVar { id, kind, arg });
        id
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Lower a PHDL design into an [`IrProgram`] by converting every module and
/// attaching its analog/digital behavior blocks.
pub fn ppr_to_ir(prog: &Design) -> IrProgram {
    let mut mod_names = Vec::new();
    let mut modules: Vec<IrModule> = Vec::new();
    for m in prog.modules() {
        mod_names.push(m.name().to_string());
        modules.push(convert_mod(m));
    }

    // Attach behaviors to their modules (behaviors are now stored inside
    // each Module after the POM refactoring).
    for m in prog.modules() {
        let Some(module) = modules.iter_mut().find(|mi| mi.name == m.name()) else { continue };
        for behavior in m.behaviors() {
            let mut ctx = LowerCtx::new();
            ctx.is_digital = behavior.is_digital();
            let stmts = lower_stmts(behavior.body(), &mut ctx);
            if behavior.is_analog() {
                module.analog = Some(IrAnalogBody {
                    state_vars: ctx.state_vars,
                    noise_sources: ctx.noise_sources,
                    vars: vec![],
                    stmts,
                });
            } else {
                module.digital = Some(IrDigitalBody {
                    inputs: vec![],
                    outputs: vec![],
                    state_vars: vec![],
                    stmts,
                });
            }
        }
    }

    // Convert global functions
    let functions = prog.functions().map(convert_fn).collect();

    IrProgram {
        source: "ppr".into(),
        modules,
        functions,
    }
}

/// Compile an analog module straight from a `Design`, skipping the
/// intermediate `IrProgram` at the call site (still built internally).
pub fn compile_analog_module(
    prog: &crate::Design,
    module_name: &str,
) -> Result<piperine_codegen::JitAnalogDevice, piperine_codegen::CodegenError> {
    let ir = ppr_to_ir(prog);
    piperine_codegen::ir_analog_to_device(&ir, module_name)
}

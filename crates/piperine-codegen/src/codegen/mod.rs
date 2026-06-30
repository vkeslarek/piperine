//! Codegen for PHDL behavior blocks.
//!
//! - Analog (`analog Foo { I(p,n) <+ ... }`): Cranelift JIT → native code.
//! - Digital (`digital Foo { @ posedge clk { ... } }`): tree-walking interpreter.

pub mod analog;
pub mod autodiff;
pub mod digital;
pub mod expr;

pub use analog::compile_analog_module;
pub use digital::{compile_digital_module, DigitalInterpreter, DigitalVal};

use cranelift_jit::JITModule;
use thiserror::Error;

/// Errors from JIT compilation.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("module `{0}` not found in ElabProgram")]
    ModuleNotFound(String),
    #[error("no analog behavior named `{0}` found in ElabProgram")]
    BehaviorNotFound(String),
    #[error("Cranelift module error: {0}")]
    Module(String),
}

/// JIT-compiled analog device.
///
/// Holds the compiled function pointers and the [`JITModule`] that keeps
/// them alive.  The module is frozen after `finalize_definitions()`, so
/// sharing across threads via `Arc` is safe.
pub struct JitAnalogDevice {
    pub name: String,
    pub param_names: Vec<String>,
    pub num_terminals: usize,
    pub num_params: usize,
    pub(crate) residual: unsafe extern "C" fn(*const f64, *const f64, *mut f64),
    pub(crate) jacobian: unsafe extern "C" fn(*const f64, *const f64, *mut f64),
    pub(crate) _module: JITModule,
}

// JITModule's internal RefCell is not modified after finalize_definitions().
unsafe impl Send for JitAnalogDevice {}
unsafe impl Sync for JitAnalogDevice {}

impl JitAnalogDevice {
    /// Accumulate current contributions into `rhs[0..num_terminals]`.
    ///
    /// `node_voltages[i]` is the voltage at terminal `i`.
    /// `rhs` must be pre-zeroed by the caller.
    pub fn eval_residual(&self, node_voltages: &[f64], params: &[f64], rhs: &mut [f64]) {
        unsafe {
            (self.residual)(node_voltages.as_ptr(), params.as_ptr(), rhs.as_mut_ptr());
        }
    }

    /// Accumulate conductance stamps into `jac` (row-major, `num_terminals²`).
    ///
    /// `jac` must be pre-zeroed by the caller.
    pub fn eval_jacobian(&self, node_voltages: &[f64], params: &[f64], jac: &mut [f64]) {
        unsafe {
            (self.jacobian)(node_voltages.as_ptr(), params.as_ptr(), jac.as_mut_ptr());
        }
    }
}

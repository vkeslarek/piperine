//! Codegen for PHDL behavior blocks.
//!
//! - Analog (`analog Foo { I(p,n) <+ ... }`): Cranelift JIT → native code.
//! - Digital (`digital Foo { @ posedge clk { ... } }`): tree-walking interpreter.

pub mod analog;
pub mod autodiff;
pub mod digital;
pub mod expr;
pub mod inline;
pub mod ir_emit;

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
    #[error("unsupported analog construct: {0}")]
    Unsupported(String),
    /// The module has no analog behavior. Distinct from a compilation error
    /// (which means the body failed to compile); this is the legitimate
    /// "no body of that kind exists" case. See GAPS §A.6.
    #[error("no analog behavior found for module `{0}`")]
    NoAnalogBody(String),
    /// Mirror of `NoAnalogBody` for digital blocks. See GAPS §A.6.
    #[error("no digital behavior found for module `{0}`")]
    NoDigitalBody(String),
    /// GAPS §D.5 — user-fn inlining error (unknown call, recursive call,
    /// arity mismatch, missing Return, depth cap).
    #[error("function inlining: {0}")]
    InlineError(String),
}

/// Live simulator state threaded through every JIT-compiled analog function.
///
/// `#[repr(C)]` keeps the layout stable across the JIT ABI. All fields are
/// plain `f64` (or `u8`); no padding requirements beyond what the compiler
/// inserts naturally (a single 32-byte struct on most targets).
///
/// Used by `$temperature`, `$abstime`, `$mfactor`, `$vt`, `$simparam`.
/// Solvers update this at each `load_dc` / `load_transient` call; the
/// default-constructed `SimCtx` represents T = 300 K (room temperature),
/// t = 0, mfactor = 1, gmin = 1e-12 S. See GAPS §A.2, §A.3.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SimCtx {
    /// Simulation temperature in Kelvin. `T = 300` is room temperature
    /// (the default), the value at which `kT/q ≈ 0.025852 V`.
    pub temperature: f64,
    /// Simulation absolute time in seconds. Read by `$abstime`.
    pub abstime: f64,
    /// Multiplier for device instance counts (Verilog-A `$mfactor`).
    pub mfactor: f64,
    /// Small conductance to ground for matrix regularisation (the solver's
    /// `gmin`). Currently informational; the in-tree solver does not stamp
    /// gmin itself (see GAPS §H.3).
    pub gmin: f64,
}

impl Default for SimCtx {
    /// Defaults: T = 300 K, t = 0, mfactor = 1, gmin = 1e-12.
    fn default() -> Self {
        Self {
            temperature: 300.0,
            abstime: 0.0,
            mfactor: 1.0,
            gmin: 1.0e-12,
        }
    }
}

impl SimCtx {
    /// Construct a SimCtx at an arbitrary temperature, with all other
    /// fields at their defaults.
    pub fn new(temperature: f64) -> Self {
        Self { temperature, ..Self::default() }
    }

    /// Convenience for the common case: T = 300 K (room temperature).
    pub fn at_300k() -> Self {
        Self::default()
    }

    /// Boltzmann constant in eV/K — i.e. `k/q` so that `kT/q = K_B_EV_K * T`
    /// gives the thermal voltage in volts at temperature `T` (Kelvin).
    /// CODATA 2018 value, rounded to 9 significant figures.
    pub const K_B_OVER_Q_EV_PER_K: f64 = 8.617_333_262e-5;
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
    pub(crate) residual: unsafe extern "C" fn(*const f64, *const f64, *const SimCtx, *mut f64),
    pub(crate) jacobian: unsafe extern "C" fn(*const f64, *const f64, *const SimCtx, *mut f64),
    /// Charge `Q(V)` accumulator for reactive (`ddt`) contributions; `None`
    /// when the module has no reactive part.  Stamped via the companion model.
    pub(crate) charge: Option<unsafe extern "C" fn(*const f64, *const f64, *const SimCtx, *mut f64)>,
    /// Charge Jacobian `dQ/dV` accumulator (row-major, `num_terminals²`).
    pub(crate) charge_jacobian: Option<unsafe extern "C" fn(*const f64, *const f64, *const SimCtx, *mut f64)>,
    /// Force residual `V+ − V− − expr` for ideal voltage sources (`V(a,b) <- expr`).
    /// `None` when the module has no force statements. The output length is
    /// `num_force_rows` (one row per force statement). GAPS §D.1.
    pub(crate) force: Option<(usize, unsafe extern "C" fn(*const f64, *const f64, *const SimCtx, *mut f64))>,
    pub(crate) _module: JITModule,
}

// JITModule's internal RefCell is not modified after finalize_definitions().
unsafe impl Send for JitAnalogDevice {}
unsafe impl Sync for JitAnalogDevice {}

impl JitAnalogDevice {
    /// Accumulate current contributions into `rhs[0..num_terminals]`.
    ///
    /// `node_voltages[i]` is the voltage at terminal `i`. `sim` carries the
    /// live simulator state read by `$temperature`, `$abstime`, `$vt`, etc.
    /// `rhs` must be pre-zeroed by the caller.
    pub fn eval_residual(
        &self,
        node_voltages: &[f64],
        params: &[f64],
        sim: &SimCtx,
        rhs: &mut [f64],
    ) {
        unsafe {
            (self.residual)(
                node_voltages.as_ptr(),
                params.as_ptr(),
                sim as *const SimCtx,
                rhs.as_mut_ptr(),
            );
        }
    }

    /// Accumulate conductance stamps into `jac` (row-major, `num_terminals²`).
    ///
    /// `jac` must be pre-zeroed by the caller.
    pub fn eval_jacobian(
        &self,
        node_voltages: &[f64],
        params: &[f64],
        sim: &SimCtx,
        jac: &mut [f64],
    ) {
        unsafe {
            (self.jacobian)(
                node_voltages.as_ptr(),
                params.as_ptr(),
                sim as *const SimCtx,
                jac.as_mut_ptr(),
            );
        }
    }

    /// True if this device has reactive (`ddt`) contributions.
    pub fn has_reactive(&self) -> bool {
        self.charge.is_some()
    }

    /// Accumulate the reactive charge `Q(V)` per terminal into `q`.
    ///
    /// No-op when the device has no reactive part.  `q` must be pre-zeroed.
    pub fn eval_charge(
        &self,
        node_voltages: &[f64],
        params: &[f64],
        sim: &SimCtx,
        q: &mut [f64],
    ) {
        if let Some(f) = self.charge {
            unsafe {
                f(
                    node_voltages.as_ptr(),
                    params.as_ptr(),
                    sim as *const SimCtx,
                    q.as_mut_ptr(),
                );
            }
        }
    }

    /// Accumulate the charge Jacobian `dQ/dV` (row-major, `num_terminals²`).
    ///
    /// No-op when the device has no reactive part.  `qjac` must be pre-zeroed.
    pub fn eval_charge_jacobian(
        &self,
        node_voltages: &[f64],
        params: &[f64],
        sim: &SimCtx,
        qjac: &mut [f64],
    ) {
        if let Some(f) = self.charge_jacobian {
            unsafe {
                f(
                    node_voltages.as_ptr(),
                    params.as_ptr(),
                    sim as *const SimCtx,
                    qjac.as_mut_ptr(),
                );
            }
        }
    }

    /// GAPS §D.1 — `true` if the module has any `V(a,b) <- expr` statements.
    /// When `true`, the device has a force-residual function (see
    /// [`Self::eval_force`]) and a per-instance branch-current unknown
    /// in the MNA matrix.
    pub fn has_force(&self) -> bool {
        self.force.is_some()
    }

    /// GAPS §D.1 — evaluate the force-residual function. `out` must be
    /// pre-zeroed and have length `num_force_rows`. Each row writes
    /// `V(plus) − V(minus) − expr` for the corresponding force statement.
    pub fn eval_force(
        &self,
        node_voltages: &[f64],
        params: &[f64],
        sim: &SimCtx,
        out: &mut [f64],
    ) {
        if let Some((n, f)) = self.force {
            assert_eq!(out.len(), n, "force output length mismatch");
            unsafe {
                f(
                    node_voltages.as_ptr(),
                    params.as_ptr(),
                    sim as *const SimCtx,
                    out.as_mut_ptr(),
                );
            }
        }
    }
}

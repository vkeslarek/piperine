//! Digital kernel compilation: an [`crate::ir::DigitalBody`] to native
//! code. There is no digital interpreter — combinational logic, register
//! updates, and event watching all compile through Cranelift.
//!
//! One [`DigitalKernel`] per module, shared across instances. Per-instance
//! signal values and register banks live in the device (`crate::device`).
//!
//! ## Value encoding
//!
//! Digital signals are 4-state (`Quad`), encoded in `i64` as 0, 1, 2 (X),
//! 3 (Z). Integers/booleans are plain `i64`; reals are `f64`. Variables live
//! in two per-instance banks (int and real) addressed by compile-time slots.
//!
//! ## Compiled functions
//!
//! - `comb(*abi)` — evaluates the combinational statements in source order:
//!   reads inputs and the live variable banks, writes outputs and the banks.
//!   Unassigned-before-read variables hold their previous value (a latch).
//! - `seq(*abi)` — for each clocked block whose `fired` flag is set, runs the
//!   register updates: reads see the *pre-edge* bank copies, writes go to the
//!   live banks (SPEC §9).
//! - `watch(*abi, *out)` — evaluates each atomic event term (the signal under
//!   a `posedge`/`negedge`/`change`); the device compares against the
//!   previous values to derive the per-block `fired` flags.

use crate::jit::SimCtx;
use crate::jit::digital::compile::DigitalCompiler;


use cranelift_jit::JITModule;

use crate::ir::{
    EdgeKind, LoweredBody, ParamId,
    NodeId, VarId,
};

use super::super::CodegenError;

use super::layout::*;

/// The digital JIT ABI: one pointer-table argument. Field order is the JIT
/// contract — the emitter reads fields by index (8 bytes each).
#[repr(C)]
pub struct DigitalAbi {
    /// Quad-coded values of `inputs`, in kernel input order.
    pub inputs: *const i64,
    /// Quad-coded values of `outputs`, in kernel output order.
    pub outputs: *mut i64,
    /// Pre-edge copies of the variable banks (read by `seq` and `comb`).
    pub vars_int_old: *const i64,
    pub vars_real_old: *const f64,
    /// Live variable banks.
    pub vars_int: *mut i64,
    pub vars_real: *mut f64,
    /// Parameter values, indexed by `ParamId`.
    pub params: *const f64,
    /// Per-clocked-block fired flags (0/1), set by the device from edges.
    pub fired: *const i64,
    /// Live simulator state (`$abstime`, `$temperature`).
    pub sim: *const SimCtx,
    /// Per-analog-terminal voltages (the A2D bridge: digital bodies read
    /// analog potentials through this array). Indexed by the analog
    /// terminal order established in `DigitalLayout`.
    pub analog_voltages: *const f64,
}

/// Byte offset of a [`DigitalAbi`] field.
#[derive(Clone, Copy)]
pub(crate) enum AbiField {
    Inputs = 0,
    Outputs = 8,
    VarsIntOld = 16,
    VarsRealOld = 24,
    VarsInt = 32,
    VarsReal = 40,
    Params = 48,
    Fired = 56,
    Sim = 64,
    AnalogVoltages = 72,
}

pub(crate) type DigitalFn = unsafe extern "C" fn(*const DigitalAbi);
pub(crate) type WatchFn = unsafe extern "C" fn(*const DigitalAbi, *mut i64);

/// One clocked block's edge sensitivity: indices into the watch-term array
/// plus the polarity that fires the block. `is_initial` marks a block that
/// fires once during `init` (from `@ initial` in a digital body) rather than
/// on a signal edge.
#[derive(Debug, Clone)]
pub struct ClockedSpec {
    pub terms: Vec<(usize, EdgeKind)>,
    pub is_initial: bool,
}

/// A register power-on value: variable plus its init expression (evaluated
/// with instance parameters). Kept as the POM `Expr` — evaluated at runtime
/// via `codegen::eval_const`.
#[derive(Debug, Clone)]
pub struct RegInit {
    pub var: VarId,
    pub init: piperine_lang::parse::ast::Expr,
}

/// A compiled digital kernel.
pub struct DigitalKernel {
    pub(crate) name: String,
    pub(crate) inputs: Vec<NodeId>,
    pub(crate) outputs: Vec<NodeId>,
    pub(crate) layout: DigitalLayout,
    pub(crate) clocked_blocks: Vec<ClockedSpec>,
    pub(crate) num_watch_terms: usize,
    pub(crate) reg_inits: Vec<RegInit>,
    /// Param name → ParamId, used to evaluate `RegInit.init` POM expressions
    /// against the instance's parameter bank at power-on.
    pub(crate) param_index: std::collections::HashMap<String, ParamId>,
    pub(crate) comb: DigitalFn,
    pub(crate) seq: Option<DigitalFn>,
    pub(crate) watch: Option<WatchFn>,
    pub(crate) _jit: JITModule,
}

unsafe impl Send for DigitalKernel {}
unsafe impl Sync for DigitalKernel {}

impl DigitalKernel {
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
        DigitalCompiler::new(module)?.compile()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inputs(&self) -> &[NodeId] {
        &self.inputs
    }

    pub fn outputs(&self) -> &[NodeId] {
        &self.outputs
    }

    pub fn layout(&self) -> &DigitalLayout {
        &self.layout
    }

    pub fn clocked_blocks(&self) -> &[ClockedSpec] {
        &self.clocked_blocks
    }

    pub fn num_watch_terms(&self) -> usize {
        self.num_watch_terms
    }

    pub fn reg_inits(&self) -> &[RegInit] {
        &self.reg_inits
    }

    /// Run the combinational function.
    pub fn eval_comb(&self, abi: &DigitalAbi) {
        unsafe { (self.comb)(abi as *const DigitalAbi) }
    }

    /// Run the register updates for the fired blocks (`abi.fired`).
    pub fn eval_seq(&self, abi: &DigitalAbi) {
        if let Some(f) = self.seq {
            unsafe { f(abi as *const DigitalAbi) }
        }
    }

    /// Evaluate the event watch terms into `out` (quad-coded).
    pub fn eval_watch(&self, abi: &DigitalAbi, out: &mut [i64]) {
        debug_assert_eq!(out.len(), self.num_watch_terms);
        if let Some(f) = self.watch {
            unsafe { f(abi as *const DigitalAbi, out.as_mut_ptr()) }
        }
    }
}

// ─── Compiler ─────────────────────────────────────────────────────────────────


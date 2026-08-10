//! Analog backend — Jacobian via the IR diff pass (T19, LOV-15/LOV-16).
//!
//! The Jacobian entry is produced by `piperine_ir::diff::Differentiator`
//! (T12's IR-level port of the OLD `resolve/diff.rs`) — this module never
//! re-derives a derivative rule. It compiles the resulting `ValueId` through
//! the SAME `Engine::compile_scalar` ABI the residual path uses (T18): one
//! differentiation implementation, one emission path.

use piperine_ir::core::{BlockId, Module, NetId, ValueId};
use piperine_ir::diff::{Differentiator, Wrt};

use super::AnalogBackend;
use crate::backend::{BackendError, BlockEnv, CellLayout, CompiledFn};

/// One compiled scalar output — a Jacobian entry, or any other single
/// `real` value pulled out of a block's op graph.
pub struct CompiledScalar {
    f: CompiledFn,
    slot: usize,
    total_contrib_slots: usize,
}

impl CompiledScalar {
    /// Evaluate the scalar against the given inputs. `cell_real`/
    /// `cell_bits` are taken `&mut` even though a pure-real Jacobian
    /// graph never writes a cell, to keep the same call shape as
    /// [`super::CompiledAnalogBlock::run`].
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        net_across: &[f64],
        net_through: &[f64],
        net_signal: &[u64],
        seed_real: &[f64],
        seed_bits: &[u64],
        cell_real: &mut [f64],
        cell_bits: &mut [u64],
    ) -> f64 {
        let mut contrib = vec![0.0f64; self.total_contrib_slots];
        let env = BlockEnv {
            net_across: net_across.as_ptr(),
            net_through: net_through.as_ptr(),
            net_signal: net_signal.as_ptr(),
            seed_real: seed_real.as_ptr(),
            seed_bits: seed_bits.as_ptr(),
            cell_real: cell_real.as_mut_ptr(),
            cell_bits: cell_bits.as_mut_ptr(),
            contrib_out: contrib.as_mut_ptr(),
            force_out: std::ptr::null_mut(),
            drive_out: std::ptr::null_mut(),
        };
        // SAFETY: `contrib` is sized from this scalar's own compile-time
        // `PortMap`; force/drive are never touched by a pure-value graph
        // (no `ForceAcross`/`Drive` op reaches a Jacobian's derivative
        // graph — `Differentiator::d_op` never emits either).
        unsafe { f_call(&self.f, &env) };
        contrib[self.slot]
    }
}

unsafe fn f_call(f: &CompiledFn, env: &BlockEnv) {
    unsafe { f.call(env as *const BlockEnv) };
}

/// Differentiate `value` w.r.t. `V(wrt.0, wrt.1)` inside `block` (appending
/// the derivative graph into the block, per `Differentiator::d`'s
/// contract), then compile the result as a [`CompiledScalar`].
pub fn compile_jacobian_entry(
    backend: &mut AnalogBackend,
    module: &mut Module,
    block: BlockId,
    value: ValueId,
    wrt: (NetId, NetId),
    layout: &CellLayout,
) -> Result<CompiledScalar, BackendError> {
    let (d_value, constants) = {
        let mut diff = Differentiator::new(module, block);
        let d_value = diff.d(value, Wrt::Across { a: wrt.0, b: wrt.1 });
        (d_value, diff.constants().clone())
    };
    let ops = module.block_at(block).ops().to_vec();
    let (f, ports, slot) = backend.engine_mut().compile_scalar(module, &ops, d_value, layout, &constants)?;
    Ok(CompiledScalar { f, slot, total_contrib_slots: ports.contrib.len() })
}

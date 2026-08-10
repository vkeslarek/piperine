//! `AnalogBackend`, `CompiledAnalogBlock`, `AnalogOutputs` — the T18 residual
//! compile/run surface. Moved out of `analog/mod.rs` (MD-34).

use piperine_ir::core::{BlockId, Module, NetId};

use crate::backend::{BackendError, BlockEnv, CellLayout, CompiledFn, Engine, PortMap};

/// The compiled analog backend: one JIT [`Engine`] plus every block
/// compiled through it so far (kept alive for the lifetime of the
/// backend — a block's function pointer is only valid while its owning
/// `Engine`'s `JITModule` is alive).
pub struct AnalogBackend {
    engine: Engine,
}

impl AnalogBackend {
    pub fn new() -> Result<Self, BackendError> {
        Ok(Self { engine: Engine::new()? })
    }

    /// Compile `block`'s op sequence into a residual function.
    pub fn compile(
        &mut self,
        module: &Module,
        block: BlockId,
        layout: &CellLayout,
    ) -> Result<CompiledAnalogBlock, BackendError> {
        let ops = module.block_at(block).ops().to_vec();
        let (f, ports) = self.engine.compile_block(module, &ops, layout)?;
        Ok(CompiledAnalogBlock { f, ports })
    }

    /// The shared engine — exposed so [`jacobian`] can compile a derivative
    /// value through the exact same `compile_scalar` ABI as the residual
    /// (T19: "no second differentiation implementation exists" extends to
    /// "no second emission ABI").
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
}

/// One compiled analog block, callable with externally-supplied net/seed/
/// cell values — mirrors `piperine_ir::interp::Interpreter`'s given-values
/// scope exactly (see `backend/mod.rs`'s module doc).
pub struct CompiledAnalogBlock {
    f: CompiledFn,
    ports: PortMap,
}

/// One evaluation's outputs: every `contribute_through`/`force_across`/
/// `drive` this block produced, keyed by net pair (or net).
pub struct AnalogOutputs {
    contrib: Vec<((NetId, NetId), f64)>,
    force: Vec<((NetId, NetId), f64)>,
    drive: Vec<(NetId, u64)>,
}

impl AnalogOutputs {
    pub fn contribution(&self, a: NetId, b: NetId) -> Option<f64> {
        self.contrib.iter().find(|&&(p, _)| p == (a, b)).map(|&(_, v)| v)
    }
    pub fn force(&self, a: NetId, b: NetId) -> Option<f64> {
        self.force.iter().find(|&&(p, _)| p == (a, b)).map(|&(_, v)| v)
    }
    pub fn drive(&self, net: NetId) -> Option<u64> {
        self.drive.iter().find(|&(n, _)| *n == net).map(|&(_, v)| v)
    }
}

impl CompiledAnalogBlock {
    pub fn ports(&self) -> &PortMap {
        &self.ports
    }

    /// Run this block once against the given inputs (mirrors
    /// `Interpreter::set_node_values`/`set_signal`/`seed_value` +
    /// `Interpreter::step`'s single-block evaluation).
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
    ) -> AnalogOutputs {
        let mut contrib = vec![0.0f64; self.ports.contrib.len()];
        let mut force = vec![0.0f64; self.ports.force.len()];
        let mut drive = vec![0u64; self.ports.drive.len()];
        let env = BlockEnv {
            net_across: net_across.as_ptr(),
            net_through: net_through.as_ptr(),
            net_signal: net_signal.as_ptr(),
            seed_real: seed_real.as_ptr(),
            seed_bits: seed_bits.as_ptr(),
            cell_real: cell_real.as_mut_ptr(),
            cell_bits: cell_bits.as_mut_ptr(),
            contrib_out: contrib.as_mut_ptr(),
            force_out: force.as_mut_ptr(),
            drive_out: drive.as_mut_ptr(),
        };
        // SAFETY: every array above is sized from this block's own
        // `PortMap`/caller-provided layout, matching the ABI `CompiledFn`
        // requires.
        unsafe { self.f.call(&env as *const BlockEnv) };
        AnalogOutputs {
            contrib: self.ports.contrib.iter().copied().zip(contrib).collect(),
            force: self.ports.force.iter().copied().zip(force).collect(),
            drive: self.ports.drive.iter().copied().zip(drive).collect(),
        }
    }
}

//! Fused digital-network JIT — the Verilator-style whole-cone evaluator.
//!
//! A connected *combinational* cone of digital instances is compiled into a
//! single Cranelift function ([`super::compile::NetworkComb`]) that evaluates
//! every member's `comb` body in rank order over shared arrays. One native
//! call settles an acyclic cone — no per-device dispatch, no event round-trips
//! between gates that are just wires. The cone presents to the scheduler as one
//! [`Element`]: it consumes the boundary input nets and emits events
//! only for the driven nets that changed.
//!
//! Scope (first cut): pure combinational members — no clocked blocks, no analog
//! sampling. Instances with those stay per-device; the circuit builder only
//! pulls eligible instances into a cone. Register/clocked fusion is the next
//! increment (see `piperine-codegen/docs/DIGITAL_JIT.md` §4).

use std::sync::Arc;

use piperine_solver::abi::{DigitalNet, LogicValue};
use piperine_solver::abi::{Element, ElementCapabilities};
use piperine_solver::abi::{DigitalPorts, EvalCtx, EventSink};

use crate::ir::LoweredBody;
use crate::jit::digital::compile::{NetworkComb, NetworkMemberSpec};
use crate::jit::{CodegenError, SimCtx};

/// Quad encoding shared with the JIT (0, 1, 2 = X, 3 = Z).
fn to_quad(v: LogicValue) -> i64 {
    v as i64
}
fn from_quad(q: i64) -> LogicValue {
    match q {
        0 => LogicValue::Zero,
        1 => LogicValue::One,
        3 => LogicValue::Z,
        _ => LogicValue::X,
    }
}

/// A member's wiring inside a fused cone: its module, the global net ids its
/// comb reads/writes, its parameter values, and its bank/param bases.
pub struct NetworkMember {
    pub module: Arc<LoweredBody>,
    pub in_nets: Vec<DigitalNet>,
    pub out_nets: Vec<DigitalNet>,
    pub params: Vec<f64>,
    pub int_base: usize,
    pub real_base: usize,
    pub param_base: usize,
    /// Power-on register values `(VarId, value)` — the same values the
    /// per-device path writes in `DigitalInstance::init`, applied here to the
    /// network-wide banks at this member's bases.
    pub reg_inits: Vec<(crate::ir::VarId, f64)>,
}

/// The cone boundary: nets fed from outside (its sensitivity list) and every
/// net it drives (published back as events for propagation and readback).
#[derive(Debug, Clone, Default)]
pub struct NetworkPorts {
    pub inputs: Vec<DigitalNet>,
    pub outputs: Vec<DigitalNet>,
}

/// A compiled, runnable fused combinational network. One `Element`
/// over the whole cone.
pub struct DigitalNetwork {
    comb: NetworkComb,
    /// Local quad-coded net values, indexed by global [`DigitalNet`] id.
    nets: Vec<i64>,
    vars_int: Vec<i64>,
    vars_real: Vec<f64>,
    params: Vec<f64>,
    ports: NetworkPorts,
    /// Every net the cone reads or drives — synced from the scheduler each eval.
    cone_nets: Vec<DigitalNet>,
    sim: SimCtx,
    source: usize,
    /// Settle cap: an acyclic rank-ordered cone stabilizes in one pass; the
    /// cap guards genuine combinational loops.
    settle_cap: usize,
}

impl DigitalNetwork {
    /// Plan, lay out banks, and compile a fused combinational cone.
    ///
    /// `members` must be in rank (topological) order — use
    /// `solver::topology::DigitalTopology` to sort. `net_count` is the size of
    /// the scheduler's net array (so local net storage lines up by id).
    /// `source` is the cone's event-provenance id.
    ///
    /// Fails loud if any member is not pure combinational (the builder must not
    /// have put it in the cone).
    pub fn build(
        members: Vec<NetworkMember>,
        net_count: usize,
        source: usize,
    ) -> Result<Self, CodegenError> {
        let specs: Vec<NetworkMemberSpec> = members
            .iter()
            .map(|m| NetworkMemberSpec {
                module: &m.module,
                in_net_slots: m.in_nets.iter().map(|n| n.0).collect(),
                out_net_slots: m.out_nets.iter().map(|n| n.0).collect(),
                int_base: m.int_base,
                real_base: m.real_base,
                param_base: m.param_base,
            })
            .collect();
        let comb = NetworkComb::compile(&specs)?;

        // Network-wide bank sizes = last member base + its module's slot count.
        let mut num_int = 0usize;
        let mut num_real = 0usize;
        let mut num_params = 0usize;
        let mut inputs: Vec<DigitalNet> = Vec::new();
        let mut outputs: Vec<DigitalNet> = Vec::new();
        let mut cone: Vec<DigitalNet> = Vec::new();
        for m in &members {
            let body = m.module.digital.as_ref().unwrap();
            let layout = crate::jit::digital::DigitalLayout::build(&m.module, body);
            num_int = num_int.max(m.int_base + layout.num_int_slots());
            num_real = num_real.max(m.real_base + layout.num_real_slots());
            num_params = num_params.max(m.param_base + m.params.len());
            outputs.extend_from_slice(&m.out_nets);
            inputs.extend_from_slice(&m.in_nets);
            cone.extend_from_slice(&m.in_nets);
            cone.extend_from_slice(&m.out_nets);
        }
        // A cone input is a net read but not driven by any member.
        let driven: std::collections::HashSet<usize> =
            outputs.iter().map(|n| n.0).collect();
        inputs.retain(|n| !driven.contains(&n.0));
        dedup(&mut inputs);
        dedup(&mut outputs);
        dedup(&mut cone);

        // Flatten member params into the network param bank at their bases.
        let mut params = vec![0.0f64; num_params];
        for m in &members {
            for (i, &v) in m.params.iter().enumerate() {
                params[m.param_base + i] = v;
            }
        }

        // Power-on register values at each member's bank bases — the same
        // seed `DigitalInstance::init` writes on the per-device path.
        let mut vars_int = vec![to_quad(LogicValue::X); num_int];
        let mut vars_real = vec![0.0; num_real];
        for m in &members {
            let body = m.module.digital.as_ref().unwrap();
            let layout = crate::jit::digital::DigitalLayout::build(&m.module, body);
            for &(var, value) in &m.reg_inits {
                if let Some(slot) = layout.real_slot(var) {
                    vars_real[m.real_base + slot] = value;
                } else if let Some(slot) = layout.int_slot(var) {
                    vars_int[m.int_base + slot] = value as i64;
                }
            }
        }

        let settle_cap = members.len() + 2;
        Ok(Self {
            comb,
            nets: vec![to_quad(LogicValue::X); net_count],
            vars_int,
            vars_real,
            params,
            ports: NetworkPorts { inputs, outputs },
            cone_nets: cone,
            sim: SimCtx::default(),
            source,
            settle_cap,
        })
    }

    pub fn ports(&self) -> &NetworkPorts {
        &self.ports
    }

    /// The cone's event-provenance id (its `source` in emitted events).
    pub fn source(&self) -> usize {
        self.source
    }

    /// One rank-ordered fused pass over the shared arrays.
    fn run_once(&mut self) {
        let dummy_analog = [0.0f64];
        // SAFETY: the banks are sized to the members' bases + slot counts, and
        // `nets` covers every global net id; the fused fn only touches those.
        unsafe {
            self.comb.run(
                self.nets.as_mut_ptr(),
                self.vars_int.as_mut_ptr(),
                self.vars_real.as_mut_ptr(),
                self.params.as_ptr(),
                &self.sim as *const SimCtx,
                dummy_analog.as_ptr(),
            );
        }
    }
}

impl Element for DigitalNetwork {
    fn name(&self) -> &str {
        "digital_network"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }

    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.ports.inputs, outputs: &self.ports.outputs }
    }

    fn init(&mut self, sink: &mut dyn EventSink) {
        // Settle from the power-on (all-X) state and publish initial outputs.
        self.settle();
        for &net in &self.ports.outputs {
            sink.emit(net, from_quad(self.nets[net.0]), 0.0);
        }
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        self.sim.abstime = ctx.time;
        // Sync every cone net from the scheduler (inputs come from outside;
        // driven nets reflect our last emission, kept consistent for readback).
        for &net in &self.cone_nets {
            self.nets[net.0] = to_quad(ctx.nets[net.0]);
        }
        self.settle();
        // Publish driven nets that differ from the scheduler's view.
        for &net in &self.ports.outputs {
            let new = from_quad(self.nets[net.0]);
            if new != ctx.nets[net.0] {
                sink.emit(net, new, 0.0);
            }
        }
    }
}

impl DigitalNetwork {
    /// Run the fused pass until the driven nets stop changing (bounded by
    /// `settle_cap`). Acyclic rank-ordered cones stabilize on the first pass.
    fn settle(&mut self) {
        for _ in 0..self.settle_cap {
            let before: Vec<i64> = self.ports.outputs.iter().map(|n| self.nets[n.0]).collect();
            self.run_once();
            let stable = self
                .ports
                .outputs
                .iter()
                .zip(&before)
                .all(|(n, &b)| self.nets[n.0] == b);
            if stable {
                return;
            }
        }
    }
}

fn dedup(v: &mut Vec<DigitalNet>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|n| seen.insert(n.0));
}

//! The digital side of a device instance: event-driven evaluation around a
//! [`DigitalKernel`]. The circuit-level boundary stays the solver's event
//! queue; per-device evaluation is JIT-compiled native code.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};

use crate::ir::{EdgeKind, IrType};
use crate::jit::digital::{DigitalAbi, DigitalKernel};
use crate::jit::{CodegenError, SimCtx};

/// Quad encoding shared with the JIT (0, 1, 2 = X, 3 = Z).
struct Quad;

impl Quad {
    const X: i64 = 2;

    fn from_logic(value: LogicValue) -> i64 {
        value as i64
    }

    fn to_logic(value: i64) -> LogicValue {
        match value {
            0 => LogicValue::Zero,
            1 => LogicValue::One,
            3 => LogicValue::Z,
            _ => LogicValue::X,
        }
    }
}

/// The digital half of a device instance.
pub struct DigitalInstance {
    kernel: Arc<DigitalKernel>,
    /// This device's index in the circuit (event `source` id).
    device_id: usize,
    /// Global nets wired to the kernel's inputs/outputs, in kernel order.
    in_nets: Vec<DigitalNet>,
    out_nets: Vec<DigitalNet>,
    params: Vec<f64>,
    sim: SimCtx,
    vars_int: Vec<i64>,
    vars_real: Vec<f64>,
    /// Watch-term values from the previous evaluation (for edge detection).
    prev_watch: Vec<i64>,
    /// Monotonic tiebreaker for emitted events.
    seq: u64,
}

impl DigitalInstance {
    pub fn new(
        kernel: Arc<DigitalKernel>,
        device_id: usize,
        in_nets: Vec<DigitalNet>,
        out_nets: Vec<DigitalNet>,
        params: Vec<f64>,
    ) -> Result<Self, CodegenError> {
        if in_nets.len() != kernel.inputs().len() || out_nets.len() != kernel.outputs().len() {
            return Err(CodegenError::Invalid(format!(
                "`{}` digital net wiring does not match the kernel port count",
                kernel.name()
            )));
        }
        let layout = kernel.layout();
        Ok(Self {
            vars_int: vec![Quad::X; layout.num_int_slots()],
            vars_real: vec![0.0; layout.num_real_slots()],
            prev_watch: vec![Quad::X; kernel.num_watch_terms()],
            kernel,
            device_id,
            in_nets,
            out_nets,
            params,
            sim: SimCtx::default(),
            seq: 0,
        })
    }

    pub fn input_nets(&self) -> &[DigitalNet] {
        &self.in_nets
    }

    pub fn output_nets(&self) -> &[DigitalNet] {
        &self.out_nets
    }

    /// Apply register power-on values, evaluate once with unknown inputs,
    /// and schedule the resulting output values at t = 0.
    pub fn init(&mut self, event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        // Integer-bank slots default to X only where the variable is
        // 4-state; two-state integers start at 0.
        let inits: Vec<(crate::ir::VarId, crate::ir::IrExpr)> = self
            .kernel
            .reg_inits()
            .iter()
            .map(|r| (r.var, r.init.clone()))
            .collect();
        for (var, init) in inits {
            let value = init
                .eval_const(&|id| self.params.get(id.0 as usize).copied())
                .unwrap_or(0.0);
            self.write_var(var, value);
        }

        let inputs = vec![Quad::X; self.in_nets.len()];
        let mut outputs = vec![Quad::X; self.out_nets.len()];
        self.run(&inputs, &mut outputs, &vec![0; self.kernel.clocked_blocks().len()]);

        // Seed edge detection from the initial input state.
        self.prev_watch = self.watch_values(&inputs, &outputs);

        let initial: Vec<(DigitalNet, i64)> =
            self.out_nets.iter().copied().zip(outputs.iter().copied()).collect();
        for (net, value) in initial {
            self.push_event(event_queue, 0.0, net, value);
        }
    }

    /// Event-driven evaluation at time `t` with the current net values.
    pub fn eval(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.sim.abstime = t;
        let inputs: Vec<i64> = self
            .in_nets
            .iter()
            .map(|n| Quad::from_logic(nets[n.0]))
            .collect();
        let previous_outputs: Vec<i64> = self
            .out_nets
            .iter()
            .map(|n| Quad::from_logic(nets[n.0]))
            .collect();
        let mut outputs = previous_outputs.clone();

        // Edge detection against the previous watch values.
        let watch = self.watch_values(&inputs, &outputs);
        let fired: Vec<i64> = self
            .kernel
            .clocked_blocks()
            .iter()
            .map(|block| {
                let fired = block.terms.iter().any(|&(term, edge)| {
                    let (prev, cur) = (self.prev_watch[term], watch[term]);
                    match edge {
                        EdgeKind::Rising => prev != 1 && cur == 1,
                        EdgeKind::Falling => prev != 0 && cur == 0,
                        EdgeKind::Any => prev != cur,
                    }
                });
                i64::from(fired)
            })
            .collect();
        self.prev_watch = watch;

        self.run(&inputs, &mut outputs, &fired);

        let changed: Vec<(DigitalNet, i64)> = self
            .out_nets
            .iter()
            .copied()
            .zip(outputs.iter().copied())
            .zip(previous_outputs.iter().copied())
            .filter(|&((_, new), old)| new != old)
            .map(|((net, new), _)| (net, new))
            .collect();
        for (net, new) in changed {
            self.push_event(event_queue, t, net, new);
        }
    }

    /// Run `seq` (with pre-edge bank copies) then `comb`.
    fn run(&mut self, inputs: &[i64], outputs: &mut [i64], fired: &[i64]) {
        let vars_int_old = self.vars_int.clone();
        let vars_real_old = self.vars_real.clone();
        let abi = DigitalAbi {
            inputs: inputs.as_ptr(),
            outputs: outputs.as_mut_ptr(),
            vars_int_old: vars_int_old.as_ptr(),
            vars_real_old: vars_real_old.as_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
        };
        if fired.iter().any(|&f| f != 0) {
            self.kernel.eval_seq(&abi);
        }
        self.kernel.eval_comb(&abi);
    }

    /// Evaluate the watch terms with the given signal state.
    fn watch_values(&mut self, inputs: &[i64], outputs: &[i64]) -> Vec<i64> {
        let mut out = vec![Quad::X; self.kernel.num_watch_terms()];
        if out.is_empty() {
            return out;
        }
        let fired = vec![0i64; self.kernel.clocked_blocks().len()];
        let mut outputs_copy = outputs.to_vec();
        let abi = DigitalAbi {
            inputs: inputs.as_ptr(),
            outputs: outputs_copy.as_mut_ptr(),
            vars_int_old: self.vars_int.as_ptr(),
            vars_real_old: self.vars_real.as_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
        };
        self.kernel.eval_watch(&abi, &mut out);
        out
    }

    fn write_var(&mut self, var: crate::ir::VarId, value: f64) {
        // The kernel layout knows the bank; the symbol type decides the
        // conversion.
        let layout = self.kernel.layout();
        if let Some(slot) = layout.real_slot(var) {
            self.vars_real[slot] = value;
        } else if let Some(slot) = layout.int_slot(var) {
            self.vars_int[slot] = value as i64;
        }
        let _ = IrType::Real; // conversions are slot-driven
    }

    fn push_event(
        &mut self,
        queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
        time: f64,
        net: DigitalNet,
        value: i64,
    ) {
        queue.push(Reverse(DigitalEvent {
            time,
            net,
            value: Quad::to_logic(value),
            source: self.device_id,
            seq: self.seq,
        }));
        self.seq += 1;
    }
}

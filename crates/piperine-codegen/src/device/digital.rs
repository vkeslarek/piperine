//! The digital side of a device instance: event-driven evaluation around a
//! [`DigitalKernel`]. The circuit-level boundary stays the solver's event
//! queue; per-device evaluation is JIT-compiled native code.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue};

use crate::resolve::{EdgeKind, Type};
use crate::kernel::digital::{DigitalAbi, DigitalKernel};
use crate::error::CodegenError;
use crate::emit::abi::SimCtx;

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

/// Evaluate a POM `Expr` as a compile-time/param constant. `param_lookup`
/// resolves parameter names to their instance values. Returns `None` for
/// anything that isn't a literal, a param reference, or simple arithmetic
/// over those — matching the register-init use case (SPEC §9 power-on).
fn eval_const_expr(
    expr: &piperine_lang::parse::ast::Expr,
    param_lookup: &impl Fn(&str) -> Option<f64>,
) -> Option<f64> {
    use piperine_lang::parse::ast::{BinaryOp, Expr, Literal, UnaryOp};
    match expr {
        Expr::Literal(Literal::Real(v)) => Some(*v),
        Expr::Literal(Literal::Int(v)) => Some(*v as f64),
        Expr::Literal(Literal::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        Expr::Literal(Literal::Quad(s)) => match s.as_str() {
            "0" => Some(0.0),
            "1" => Some(1.0),
            _ => Some(2.0),
        },
        Expr::Ident(name) => param_lookup(name),
        Expr::Binary(lhs, op, rhs) => {
            let l = eval_const_expr(lhs, param_lookup)?;
            let r = eval_const_expr(rhs, param_lookup)?;
            Some(match op {
                BinaryOp::Add => l + r,
                BinaryOp::Sub => l - r,
                BinaryOp::Mul => l * r,
                BinaryOp::Div => l / r,
                _ => return None,
            })
        }
        Expr::Unary(UnaryOp::Neg, x) => Some(-eval_const_expr(x, param_lookup)?),
        _ => None,
    }
}

/// Reusable per-evaluation buffers. The event loop calls
/// [`DigitalInstance::eval`] once per device per delta cycle — allocating
/// these fresh each call dominated the digital simulation path.
#[derive(Default)]
struct Scratch {
    inputs: Vec<i64>,
    outputs: Vec<i64>,
    prev_outputs: Vec<i64>,
    watch: Vec<i64>,
    fired: Vec<i64>,
    /// Pre-edge bank copies, filled only when a clocked block fired
    /// (only `seq` reads them).
    vars_int_old: Vec<i64>,
    vars_real_old: Vec<f64>,
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
    scratch: Scratch,
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
            scratch: Scratch::default(),
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

    /// The compiled digital kernel (shared across instances).
    pub fn kernel(&self) -> &Arc<DigitalKernel> {
        &self.kernel
    }

    /// Hidden state carrier for full-state re-entry (PSS shots): module
    /// vars + edge-detection memory. Empty when the kernel is stateless.
    pub fn hidden_snapshot(&self) -> Option<(Vec<i64>, Vec<f64>)> {
        if self.vars_int.is_empty() && self.vars_real.is_empty() && self.prev_watch.is_empty() {
            return None;
        }
        let mut ints = self.vars_int.clone();
        ints.extend_from_slice(&self.prev_watch);
        Some((ints, self.vars_real.clone()))
    }

    /// Restore a state produced by [`Self::hidden_snapshot`]. Splits the int
    /// carrier back into module vars and watch memory by the current layout
    /// (the layout is kernel-fixed, so a same-kernel snapshot always fits).
    pub fn hidden_restore(&mut self, state: &(Vec<i64>, Vec<f64>)) {
        let (ints, reals) = state;
        let n_int = self.vars_int.len();
        let n_watch = self.prev_watch.len();
        if ints.len() == n_int + n_watch && reals.len() == self.vars_real.len() {
            self.vars_int.clone_from_slice(&ints[..n_int]);
            self.prev_watch.clone_from_slice(&ints[n_int..]);
            self.vars_real.clone_from_slice(reals);
        }
    }

    /// Export all register/variable values as `f64`, indexed by `VarId`.
    /// Used by the D2A bridge to sync digital state into the analog vars
    /// bank after each evaluation.
    pub fn export_vars(&self) -> Vec<f64> {
        self.kernel.layout().export_vars(&self.vars_int, &self.vars_real)
    }

    /// Power-on register values `(VarId, value)`, evaluated from the
    /// kernel's `RegInit` expressions against this instance's parameters —
    /// the same values [`DigitalInstance::init`] writes. The fused
    /// combinational network consumes them for its own power-on bank state.
    pub(crate) fn reg_init_values(&self) -> Vec<(crate::resolve::VarId, f64)> {
        let param_index = &self.kernel.param_index;
        let params = &self.params;
        self.kernel
            .reg_inits()
            .iter()
            .map(|r| {
                let value = eval_const_expr(&r.init, &|name| {
                    param_index
                        .get(name)
                        .and_then(|&id| params.get(id.0 as usize).copied())
                })
                .unwrap_or(0.0);
                (r.var, value)
            })
            .collect()
    }

    /// Apply register power-on values, evaluate once with unknown inputs,
    /// and schedule the resulting output values at t = 0.
    pub fn init(&mut self, event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        // Integer-bank slots default to X only where the variable is
        // 4-state; two-state integers start at 0.
        for (var, value) in self.reg_init_values() {
            self.write_var(var, value);
        }

        let mut s = std::mem::take(&mut self.scratch);
        s.inputs.clear();
        s.inputs.resize(self.in_nets.len(), Quad::X);
        s.outputs.clear();
        s.outputs.resize(self.out_nets.len(), Quad::X);
        // Fire `@ initial` clocked blocks during init (SPEC §10.4): their
        // register updates run once at simulation start, reading the
        // pre-edge (power-on) bank values.
        s.fired.clear();
        s.fired
            .extend(self.kernel.clocked_blocks().iter().map(|b| i64::from(b.is_initial)));
        let analog_voltages: Vec<f64> = vec![0.0; self.kernel.layout().num_analog()];
        self.run(&mut s, &analog_voltages);

        // Seed edge detection from the initial input state.
        self.watch_values(&mut s, &analog_voltages);
        self.prev_watch.copy_from_slice(&s.watch);

        for i in 0..self.out_nets.len() {
            let (net, value) = (self.out_nets[i], s.outputs[i]);
            self.push_event(event_queue, 0.0, net, value);
        }
        self.scratch = s;
    }

    /// Event-driven evaluation at time `t` with the current net values.
    pub fn eval(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: &[f64],
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.sim.abstime = t;
        // Detach the scratch buffers so the kernel calls can borrow `self`.
        let mut s = std::mem::take(&mut self.scratch);

        s.inputs.clear();
        s.inputs
            .extend(self.in_nets.iter().map(|n| Quad::from_logic(nets[n.0])));
        s.prev_outputs.clear();
        s.prev_outputs
            .extend(self.out_nets.iter().map(|n| Quad::from_logic(nets[n.0])));
        s.outputs.clear();
        s.outputs.extend_from_slice(&s.prev_outputs);

        // Edge detection against the previous watch values.
        self.watch_values(&mut s, analog_voltages);
        s.fired.clear();
        s.fired.extend(self.kernel.clocked_blocks().iter().map(|block| {
            let fired = block.terms.iter().any(|&(term, edge)| {
                let (prev, cur) = (self.prev_watch[term], s.watch[term]);
                match edge {
                    EdgeKind::Rising => prev != 1 && cur == 1,
                    EdgeKind::Falling => prev != 0 && cur == 0,
                    EdgeKind::Any => prev != cur,
                }
            });
            i64::from(fired)
        }));
        self.prev_watch.copy_from_slice(&s.watch);

        self.run(&mut s, analog_voltages);

        for i in 0..self.out_nets.len() {
            let (net, new) = (self.out_nets[i], s.outputs[i]);
            if new != s.prev_outputs[i] {
                self.push_event(event_queue, t, net, new);
            }
        }
        self.scratch = s;
    }

    /// Phase 1 of a two-phase delta cycle: detect clock edges and, if any
    /// clocked block fires, commit register writes using the pre-settle
    /// `nets` snapshot. Never writes output nets — see
    /// `Element::digital_seq_phase`.
    pub fn eval_seq_phase(&mut self, t: f64, nets: &[LogicValue], analog_voltages: &[f64]) -> bool {
        self.sim.abstime = t;
        let mut s = std::mem::take(&mut self.scratch);

        s.inputs.clear();
        s.inputs
            .extend(self.in_nets.iter().map(|n| Quad::from_logic(nets[n.0])));
        s.prev_outputs.clear();
        s.prev_outputs
            .extend(self.out_nets.iter().map(|n| Quad::from_logic(nets[n.0])));
        s.outputs.clear();
        s.outputs.extend_from_slice(&s.prev_outputs);

        self.watch_values(&mut s, analog_voltages);
        s.fired.clear();
        s.fired.extend(self.kernel.clocked_blocks().iter().map(|block| {
            let fired = block.terms.iter().any(|&(term, edge)| {
                let (prev, cur) = (self.prev_watch[term], s.watch[term]);
                match edge {
                    EdgeKind::Rising => prev != 1 && cur == 1,
                    EdgeKind::Falling => prev != 0 && cur == 0,
                    EdgeKind::Any => prev != cur,
                }
            });
            i64::from(fired)
        }));
        self.prev_watch.copy_from_slice(&s.watch);

        let any_fired = s.fired.iter().any(|&f| f != 0);
        if any_fired {
            s.vars_int_old.clear();
            s.vars_int_old.extend_from_slice(&self.vars_int);
            s.vars_real_old.clear();
            s.vars_real_old.extend_from_slice(&self.vars_real);
            let abi = DigitalAbi {
                inputs: s.inputs.as_ptr(),
                outputs: s.outputs.as_mut_ptr(),
                vars_int_old: s.vars_int_old.as_ptr(),
                vars_real_old: s.vars_real_old.as_ptr(),
                vars_int: self.vars_int.as_mut_ptr(),
                vars_real: self.vars_real.as_mut_ptr(),
                params: self.params.as_ptr(),
                fired: s.fired.as_ptr(),
                sim: &self.sim as *const SimCtx,
                analog_voltages: analog_voltages.as_ptr(),
            };
            self.kernel.eval_seq(&abi);
        }
        self.scratch = s;
        any_fired
    }

    /// Phase 2: recompute combinational outputs from live `nets` and the
    /// (possibly just-committed) register banks, emitting change events.
    /// Does not redo edge detection or register writes — see
    /// `Element::digital_comb_phase`.
    pub fn eval_comb_phase(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: &[f64],
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.sim.abstime = t;
        let mut s = std::mem::take(&mut self.scratch);

        s.inputs.clear();
        s.inputs
            .extend(self.in_nets.iter().map(|n| Quad::from_logic(nets[n.0])));
        s.prev_outputs.clear();
        s.prev_outputs
            .extend(self.out_nets.iter().map(|n| Quad::from_logic(nets[n.0])));
        s.outputs.clear();
        s.outputs.extend_from_slice(&s.prev_outputs);

        let abi = DigitalAbi {
            inputs: s.inputs.as_ptr(),
            outputs: s.outputs.as_mut_ptr(),
            vars_int_old: self.vars_int.as_ptr(),
            vars_real_old: self.vars_real.as_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: s.fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
            analog_voltages: analog_voltages.as_ptr(),
        };
        self.kernel.eval_comb(&abi);

        for i in 0..self.out_nets.len() {
            let (net, new) = (self.out_nets[i], s.outputs[i]);
            if new != s.prev_outputs[i] {
                self.push_event(event_queue, t, net, new);
            }
        }
        self.scratch = s;
    }

    /// Run `seq` (with pre-edge bank copies) then `comb`. The pre-edge
    /// copies are made only when a clocked block fired — only `seq` reads
    /// them.
    fn run(&mut self, s: &mut Scratch, analog_voltages: &[f64]) {
        let any_fired = s.fired.iter().any(|&f| f != 0);
        if any_fired {
            s.vars_int_old.clear();
            s.vars_int_old.extend_from_slice(&self.vars_int);
            s.vars_real_old.clear();
            s.vars_real_old.extend_from_slice(&self.vars_real);
        }
        let abi = DigitalAbi {
            inputs: s.inputs.as_ptr(),
            outputs: s.outputs.as_mut_ptr(),
            vars_int_old: if any_fired { s.vars_int_old.as_ptr() } else { self.vars_int.as_ptr() },
            vars_real_old: if any_fired { s.vars_real_old.as_ptr() } else { self.vars_real.as_ptr() },
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: s.fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
            analog_voltages: analog_voltages.as_ptr(),
        };
        if any_fired {
            self.kernel.eval_seq(&abi);
        }
        self.kernel.eval_comb(&abi);
    }

    /// Evaluate the watch terms into `s.watch` with the current signal
    /// state. `watch` only reads (no output/var writes), so it runs against
    /// the live buffers with an all-zero `fired` mask.
    fn watch_values(&mut self, s: &mut Scratch, analog_voltages: &[f64]) {
        let n = self.kernel.num_watch_terms();
        s.watch.clear();
        s.watch.resize(n, Quad::X);
        if n == 0 {
            return;
        }
        s.fired.clear();
        s.fired.resize(self.kernel.clocked_blocks().len(), 0);
        let abi = DigitalAbi {
            inputs: s.inputs.as_ptr(),
            outputs: s.outputs.as_mut_ptr(),
            vars_int_old: self.vars_int.as_ptr(),
            vars_real_old: self.vars_real.as_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: s.fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
            analog_voltages: analog_voltages.as_ptr(),
        };
        self.kernel.eval_watch(&abi, &mut s.watch);
    }

    fn write_var(&mut self, var: crate::resolve::VarId, value: f64) {
        // The kernel layout knows the bank; the symbol type decides the
        // conversion.
        let layout = self.kernel.layout();
        if let Some(slot) = layout.real_slot(var) {
            self.vars_real[slot] = value;
        } else if let Some(slot) = layout.int_slot(var) {
            self.vars_int[slot] = value as i64;
        }
        let _ = Type::Real; // conversions are slot-driven
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

// `DigitalInstance` is driven through its inherent `init`/`eval_seq_phase`/
// `eval_comb_phase` methods by the composite [`PiperineDevice`], which is the
// `Element` the solver sees. There is no separate digital-device trait: the
// unified `Element` contract carries digital evaluation directly.

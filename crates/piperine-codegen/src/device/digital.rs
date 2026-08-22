//! The digital side of a device instance: event-driven evaluation around a
//! [`DigitalKernel`]. The circuit-level boundary stays the solver's event
//! queue; per-device evaluation is JIT-compiled native code.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};

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

    /// The same fold as [`Quad::to_logic`], into `signal`'s four-state
    /// type — the input to [`EdgeDirection::fires`].
    fn to_signal(value: i64) -> piperine_ir::logic::Logic {
        match value {
            0 => piperine_ir::logic::Logic::Zero,
            1 => piperine_ir::logic::Logic::One,
            3 => piperine_ir::logic::Logic::Z,
            _ => piperine_ir::logic::Logic::X,
        }
    }
}

impl EdgeKind {
    /// Map codegen's clocked-block edge kind onto the shared detector's
    /// direction — `Any` and `Either` name the same thing.
    fn to_signal(self) -> piperine_ir::logic::EdgeDirection {
        match self {
            EdgeKind::Rising => piperine_ir::logic::EdgeDirection::Rising,
            EdgeKind::Falling => piperine_ir::logic::EdgeDirection::Falling,
            EdgeKind::Any => piperine_ir::logic::EdgeDirection::Either,
        }
    }
}

/// Evaluate a POM `Expr` as a compile-time/param constant. `param_lookup`
/// resolves parameter names to their instance values. Returns `None` for
/// anything that isn't const-evaluable (delegates to the shared
/// `resolve::pom_eval_const` — the same constant-folding this crate already
/// uses at codegen time, not a second implementation).
fn eval_const_expr(
    expr: &piperine_lang::ast::Expr,
    param_lookup: &impl Fn(&str) -> Option<f64>,
) -> Option<f64> {
    crate::resolve::pom_eval_const(expr, param_lookup).ok()
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
    /// T23/LRMW2-14: non-blocking (`<=`) write staging banks (LRM
    /// §8.5.3.4's Nba region) — seeded from the live `vars_int`/`vars_real`/
    /// `outputs` before each `seq`/`comb` call, written by `<=` targets,
    /// committed back into the live banks after the call returns (see
    /// `DigitalInstance::seed_nba`/`commit_nba`). Named `_old` for
    /// historical reasons (see `DigitalAbi::vars_int_old`'s doc comment).
    vars_int_old: Vec<i64>,
    vars_real_old: Vec<f64>,
    outputs_nba: Vec<i64>,
    /// Baselines the above three staging banks were seeded from — the
    /// commit step only copies a staging slot into the live bank when it
    /// differs from its baseline (i.e. a `<=` target actually wrote it).
    /// Without this, unconditionally copying the whole staging bank back
    /// would clobber a `=` (blocking) write the *same* call made straight
    /// to the live bank, since the staging snapshot predates it.
    vars_int_base: Vec<i64>,
    vars_real_base: Vec<f64>,
    outputs_base: Vec<i64>,
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

    /// Refreshes `self.sim`'s run-scoped fields from the host `Context`
    /// (LRMC-C7-07, T43) — mirrors `AnalogInstance::sync_sim`'s `io`
    /// wiring, which digital never had: `sim.io` was set once at
    /// construction to `SimCtx::default()`'s `StdoutSink` singleton and
    /// never updated, so a host-configured sink (`BufferSink`, a test's
    /// own capture) never reached the digital `$display`-family dispatch
    /// this task adds. Called from `PiperineDevice::update`, the same
    /// per-step hook every `Element` already receives `Context` through.
    pub fn sync_sim(&mut self, context: &piperine_solver::abi::Context) {
        self.sim.io = std::rc::Rc::as_ptr(&context.io);
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

    /// Whether this instance owns mutable register state worth checkpointing
    /// (ABI-05): module vars or edge-detection watch memory.
    pub fn has_registers(&self) -> bool {
        !(self.vars_int.is_empty() && self.vars_real.is_empty() && self.prev_watch.is_empty())
    }

    /// Checkpoint the digital register state for per-step rollback (ABI-05):
    /// `vars_int`, `vars_real`, and `prev_watch`. Same register set and layout
    /// as [`hidden_snapshot`](Self::hidden_snapshot) (PSS re-entry) — both
    /// rewind the non-accept-gated registers a rejected settle dirties.
    pub fn checkpoint_registers(&self) -> Option<(Vec<i64>, Vec<f64>)> {
        self.hidden_snapshot()
    }

    /// Restore digital registers from a checkpoint produced by
    /// [`checkpoint_registers`](Self::checkpoint_registers) (ABI-05): rewinds
    /// `vars_int`, `vars_real`, and `prev_watch`.
    pub fn restore_registers(&mut self, ints: &[i64], reals: &[f64]) {
        self.hidden_restore(&(ints.to_vec(), reals.to_vec()));
    }

    /// Export all register/variable values as `f64`, indexed by `VarId`.
    /// Used by the D2A bridge to sync digital state into the analog vars
    /// bank after each evaluation. T25/LRMW2-23: fails loud (naming the
    /// net) if a 4-state var carries X/Z — see
    /// `DigitalLayout::export_vars`'s own doc comment.
    pub fn export_vars(&self) -> Result<Vec<(crate::resolve::VarId, f64)>, CodegenError> {
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
                edge.to_signal().fires(Quad::to_signal(prev), Quad::to_signal(cur))
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
                edge.to_signal().fires(Quad::to_signal(prev), Quad::to_signal(cur))
            });
            i64::from(fired)
        }));
        self.prev_watch.copy_from_slice(&s.watch);

        let any_fired = s.fired.iter().any(|&f| f != 0);
        if any_fired {
            self.seed_nba(&mut s);
            let abi = DigitalAbi {
                inputs: s.inputs.as_ptr(),
                outputs: s.outputs.as_mut_ptr(),
                vars_int_old: s.vars_int_old.as_mut_ptr(),
                vars_real_old: s.vars_real_old.as_mut_ptr(),
                vars_int: self.vars_int.as_mut_ptr(),
                vars_real: self.vars_real.as_mut_ptr(),
                params: self.params.as_ptr(),
                fired: s.fired.as_ptr(),
                sim: &self.sim as *const SimCtx,
                analog_voltages: analog_voltages.as_ptr(),
                outputs_nba: s.outputs_nba.as_mut_ptr(),
            };
            self.kernel.eval_seq(&abi);
            self.commit_nba(&mut s);
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
        // Preserve any writes `eval_seq_phase` made to `s.outputs` this
        // step (a clocked block like `always @(posedge clk) q <= d;`
        // updates the output register in seq, not in comb). For devices
        // whose `comb` writes every output every pass (the combinational
        // case), those writes simply overwrite — same as the previous
        // unconditional reset-to-prev_outputs did. For purely clocked
        // devices whose `comb` is empty, the previous reset silently
        // discarded seq's writes, so a clocked output never propagated
        // to its net. Initialise only when seq didn't run this step
        // (s.outputs was cleared by `std::mem::take` above).
        if s.outputs.len() != s.prev_outputs.len() {
            s.outputs.clear();
            s.outputs.extend_from_slice(&s.prev_outputs);
        }

        self.seed_nba(&mut s);
        let abi = DigitalAbi {
            inputs: s.inputs.as_ptr(),
            outputs: s.outputs.as_mut_ptr(),
            vars_int_old: s.vars_int_old.as_mut_ptr(),
            vars_real_old: s.vars_real_old.as_mut_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: s.fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
            analog_voltages: analog_voltages.as_ptr(),
            outputs_nba: s.outputs_nba.as_mut_ptr(),
        };
        self.kernel.eval_comb(&abi);
        self.commit_nba(&mut s);

        for i in 0..self.out_nets.len() {
            let (net, new) = (self.out_nets[i], s.outputs[i]);
            if new != s.prev_outputs[i] {
                self.push_event(event_queue, t, net, new);
            }
        }
        self.scratch = s;
    }

    /// T23/LRMW2-14: seeds the non-blocking write staging banks
    /// (`vars_int_old`/`vars_real_old`/`outputs_nba` — see `DigitalAbi`'s
    /// doc comment) from the current live values, ready for a `seq`/`comb`
    /// call whose `<=` targets write into them instead of live. Resizing to
    /// the same length every call never reallocates after the first, so a
    /// pointer captured from these buffers stays valid across a
    /// seed → run → commit → seed → run → commit sequence.
    fn seed_nba(&mut self, s: &mut Scratch) {
        s.vars_int_base.resize(self.vars_int.len(), 0);
        s.vars_int_base.copy_from_slice(&self.vars_int);
        s.vars_int_old.resize(self.vars_int.len(), 0);
        s.vars_int_old.copy_from_slice(&self.vars_int);

        s.vars_real_base.resize(self.vars_real.len(), 0.0);
        s.vars_real_base.copy_from_slice(&self.vars_real);
        s.vars_real_old.resize(self.vars_real.len(), 0.0);
        s.vars_real_old.copy_from_slice(&self.vars_real);

        s.outputs_base.resize(s.outputs.len(), 0);
        s.outputs_base.copy_from_slice(&s.outputs);
        s.outputs_nba.resize(s.outputs.len(), 0);
        s.outputs_nba.copy_from_slice(&s.outputs);
    }

    /// T23/LRMW2-14: commits the staged non-blocking writes (LRM §8.5.3.4's
    /// Nba region) into the live banks — called after a `seq`/`comb` call
    /// returns. Only a slot that actually differs from its seeded baseline
    /// (i.e. a `<=` target this call) is copied into the live bank; every
    /// other slot is left as the call itself left it, so a `=` (blocking)
    /// write made straight to the live bank during the same call is never
    /// clobbered by the pre-call staging snapshot.
    fn commit_nba(&mut self, s: &mut Scratch) {
        for i in 0..self.vars_int.len() {
            if s.vars_int_old[i] != s.vars_int_base[i] {
                self.vars_int[i] = s.vars_int_old[i];
            }
        }
        for i in 0..self.vars_real.len() {
            if s.vars_real_old[i] != s.vars_real_base[i] {
                self.vars_real[i] = s.vars_real_old[i];
            }
        }
        for i in 0..s.outputs.len() {
            if s.outputs_nba[i] != s.outputs_base[i] {
                s.outputs[i] = s.outputs_nba[i];
            }
        }
    }

    /// Run `seq` (for fired blocks) then `comb`, committing non-blocking
    /// (`<=`) writes after each (T23/LRMW2-14) so `comb` sees `seq`'s
    /// committed register state and a later statement never observes a
    /// same-block `<=` write before the block finishes.
    fn run(&mut self, s: &mut Scratch, analog_voltages: &[f64]) {
        let any_fired = s.fired.iter().any(|&f| f != 0);
        self.seed_nba(s);
        let abi = DigitalAbi {
            inputs: s.inputs.as_ptr(),
            outputs: s.outputs.as_mut_ptr(),
            vars_int_old: s.vars_int_old.as_mut_ptr(),
            vars_real_old: s.vars_real_old.as_mut_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: s.fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
            analog_voltages: analog_voltages.as_ptr(),
            outputs_nba: s.outputs_nba.as_mut_ptr(),
        };
        if any_fired {
            self.kernel.eval_seq(&abi);
            self.commit_nba(s);
            self.seed_nba(s);
        }
        self.kernel.eval_comb(&abi);
        self.commit_nba(s);
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
        // `watch` only ever reads — `vars_int_old`/`vars_real_old`/
        // `outputs_nba` alias the live pointers rather than a real staging
        // buffer, since nothing here writes through them.
        let abi = DigitalAbi {
            inputs: s.inputs.as_ptr(),
            outputs: s.outputs.as_mut_ptr(),
            vars_int_old: self.vars_int.as_mut_ptr(),
            vars_real_old: self.vars_real.as_mut_ptr(),
            vars_int: self.vars_int.as_mut_ptr(),
            vars_real: self.vars_real.as_mut_ptr(),
            params: self.params.as_ptr(),
            fired: s.fired.as_ptr(),
            sim: &self.sim as *const SimCtx,
            analog_voltages: analog_voltages.as_ptr(),
            outputs_nba: s.outputs.as_mut_ptr(),
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
        // T26/LRMW2-26 (§8.4.3.3): quantize to the module's `` `timescale ``
        // precision grid — a no-op without a `` `timescale `` directive.
        let time = self.kernel.quantize_time(time);
        queue.push(Reverse(DigitalEvent {
            time,
            net,
            value: Quad::to_logic(value),
            source: self.device_id,
            seq: self.seq,
            region: Region::Active,
        }));
        self.seq += 1;
    }
}

// `DigitalInstance` is driven through its inherent `init`/`eval_seq_phase`/
// `eval_comb_phase` methods by the composite [`PiperineDevice`], which is the
// `Element` the solver sees. There is no separate digital-device trait: the
// unified `Element` contract carries digital evaluation directly.

#[cfg(test)]
mod tests {
    use super::*;

    // `s.watch` is a `Vec<i64>` filled by JIT-emitted `eval_watch` code and
    // can in principle hold values outside `0..=3`; `Quad::to_logic`'s
    // `_ => X` arm already folds those to `X` today, and the same fold must
    // hold through `Quad::to_signal` so delegation changes nothing here.
    #[test]
    fn an_out_of_range_watch_value_folds_to_x_and_still_fires_rising_into_one() {
        let out_of_range = 99_i64;
        assert_eq!(Quad::to_signal(out_of_range), piperine_ir::logic::Logic::X);
        assert_eq!(Quad::to_logic(out_of_range), LogicValue::X);

        // Identical to today's rule: not-One before, One after, for prev = X.
        let fired = EdgeKind::Rising
            .to_signal()
            .fires(Quad::to_signal(out_of_range), Quad::to_signal(1));
        assert!(fired, "X (folded from an out-of-range watch value) -> 1 is a rising edge");
    }
}

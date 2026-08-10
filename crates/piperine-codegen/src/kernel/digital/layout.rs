//! Digital kernel compilation: an [`crate::resolve::DigitalBody`] to native
//! code. There is no digital interpreter — combinational logic, register
//! updates, and event watching all compile through Cranelift.
//!
//! One [`DigitalKernel`](crate::kernel::digital::DigitalKernel) per module, shared across instances. Per-instance
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

use std::collections::HashMap;


use crate::resolve::{
    Domain, DigitalBody, LoweredBody, Type,
    NodeId, VarId,
};



#[derive(Debug, Default, Clone)]
pub struct DigitalLayout {
    pub(crate) input_index: HashMap<NodeId, usize>,
    pub(crate) output_index: HashMap<NodeId, usize>,
    pub(crate) int_slot: HashMap<VarId, usize>,
    pub(crate) real_slot: HashMap<VarId, usize>,
    pub(crate) num_int: usize,
    pub(crate) num_real: usize,
    /// Index of each analog terminal in the `analog_voltages` ABI array.
    /// Populated from the module's analog-domain nodes (ports + internal
    /// wires). Used by the A2D bridge to read `V(node)` in digital bodies.
    pub(crate) analog_index: HashMap<NodeId, usize>,
    pub(crate) num_analog: usize,
    /// The vars this digital body actually **writes** at run time: every
    /// assignment target in its statement tree, plus comb-locals it declares.
    /// Not its registers' power-on inits, and not vars it merely has a slot for.
    ///
    /// Distinct from `int_slot`/`real_slot`, which cover every var in the
    /// module: the layout allocates a bank slot for all of them (cheap, and
    /// it keeps `VarId` -> slot total), but owning a *slot* is not owning a
    /// *value*. Only this set may travel the D2A bridge. A module carrying
    /// both an `analog` and a `digital` body shares one var namespace, so
    /// exporting slot-owned-but-unwritten vars pushed the digital bank's
    /// zeros over analog-owned state on every `comb_phase`.
    pub(crate) written: std::collections::HashSet<VarId>,
}

impl DigitalLayout {
    pub(crate) fn build(module: &LoweredBody, body: &DigitalBody) -> Self {
        let mut layout = Self::default();
        for (i, &node) in body.inputs.iter().enumerate() {
            layout.input_index.insert(node, i);
        }
        for (i, &node) in body.outputs.iter().enumerate() {
            layout.output_index.insert(node, i);
        }
        for (id, info) in module.symbols.vars() {
            match info.ty {
                Type::Real => {
                    layout.real_slot.insert(id, layout.num_real);
                    layout.num_real += 1;
                }
                Type::Integer | Type::Bool | Type::Quad => {
                    layout.int_slot.insert(id, layout.num_int);
                    layout.num_int += 1;
                }
            }
        }
        // The written set: every assignment target reachable in the statement
        // tree (nested `if`/`match`/`@` bodies included).
        //
        // Deliberately NOT seeded from `body.regs`. Latch inference marks a
        // var read-before-assignment as a register, so `regs` includes vars
        // this body never writes — including the analog side's own. A real
        // register is assigned inside its clocked block, so the walk below
        // already covers it; seeding from `regs` only re-admits the vars this
        // set exists to exclude.
        let by_name: HashMap<&str, VarId> =
            module.symbols.vars().map(|(id, info)| (info.name.as_str(), id)).collect();
        collect_written(&body.stmts, &by_name, &body.regs, &mut layout.written);

        // Map analog-domain nodes to indices in the analog_voltages array.
        // The order follows the symbol table's node iteration (ground is
        // NodeId(0), always analog, always 0 V — skipped).
        for (id, info) in module.symbols.nodes() {
            if info.domain == Domain::Analog && !id.is_ground() {
                layout.analog_index.insert(id, layout.num_analog);
                layout.num_analog += 1;
            }
        }
        layout
    }

    pub fn num_int_slots(&self) -> usize {
        self.num_int
    }

    pub fn num_real_slots(&self) -> usize {
        self.num_real
    }

    pub fn int_slot(&self, var: VarId) -> Option<usize> {
        self.int_slot.get(&var).copied()
    }

    pub fn real_slot(&self, var: VarId) -> Option<usize> {
        self.real_slot.get(&var).copied()
    }

    /// Number of analog terminals (for the `analog_voltages` array size).
    pub fn num_analog(&self) -> usize {
        self.num_analog
    }

    /// Index of an analog node in the `analog_voltages` array, or `None`
    /// for ground / digital-only nodes.
    pub fn analog_index(&self, node: NodeId) -> Option<usize> {
        if node.is_ground() {
            None
        } else {
            self.analog_index.get(&node).copied()
        }
    }

    /// Export the values of the variables this digital kernel **owns**, each
    /// paired with its `VarId`. Integer-bank vars are converted to `f64`.
    /// Used by the D2A bridge: the analog side reads digital register values
    /// through this export.
    ///
    /// The pairing is load bearing. This used to return a bare `Vec<f64>`
    /// sized by the *digital* var count (`int_slot.len() + real_slot.len()`)
    /// but written at the *global* `VarId` index — two different index spaces.
    /// Every position the digital side did not own kept its `0.0` filler, and
    /// `AnalogInstance::sync_vars` then blitted the whole vector positionally
    /// over the analog var bank after each `comb_phase`. The visible effect
    /// was that a module carrying both an `analog` and a `digital` body had
    /// its analog-owned module vars silently zeroed on every evaluation — so
    /// an `@timer` that maintained state (`@timer(tsw) { t0 = $abstime(); }`)
    /// fired correctly and had its write erased microseconds later, and the
    /// module's analog timebase quietly stopped existing.
    pub fn export_vars(&self, vars_int: &[i64], vars_real: &[f64]) -> Vec<(VarId, f64)> {
        let mut out = Vec::with_capacity(self.written.len());
        for (&var_id, &slot) in &self.int_slot {
            if self.written.contains(&var_id) && slot < vars_int.len() {
                out.push((var_id, vars_int[slot] as f64));
            }
        }
        for (&var_id, &slot) in &self.real_slot {
            if self.written.contains(&var_id) && slot < vars_real.len() {
                out.push((var_id, vars_real[slot]));
            }
        }
        out
    }
}

/// Collect every var a digital statement tree assigns to, resolving names
/// through the module's symbol table. Walks nested `if`/`match`/`@`-event
/// bodies — a var written only inside a clocked block is still written.
fn collect_written(
    stmts: &[piperine_lang::parse::ast::Stmt],
    by_name: &HashMap<&str, VarId>,
    regs: &[VarId],
    out: &mut std::collections::HashSet<VarId>,
) {
    use piperine_lang::parse::ast::{Expr, Stmt};
    for stmt in stmts {
        match stmt {
            Stmt::Bind { dest: Expr::Ident(name), .. } => {
                if let Some(&id) = by_name.get(name.as_str()) {
                    out.insert(id);
                }
            }
            Stmt::VarDecl { name, .. } => {
                // A declaration is a *power-on init*, not a per-evaluation
                // write — `compile.rs` routes it to `reg_inits` whenever the
                // var is a register. Counting it as a write is what let a
                // module-level `var` the analog side owns travel the D2A
                // bridge carrying the digital bank's never-updated value.
                // A genuine comb-local (declared, recomputed each pass, not a
                // register) IS digital-owned, so it still counts.
                if let Some(&id) = by_name.get(name.as_str())
                    && !regs.contains(&id)
                {
                    out.insert(id);
                }
            }
            Stmt::If { then_body, else_body, .. } => {
                collect_written(&then_body.stmts, by_name, regs, out);
                if let Some(eb) = else_body {
                    collect_written(&eb.stmts, by_name, regs, out);
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    collect_written(&arm.body.stmts, by_name, regs, out);
                }
            }
            Stmt::Event { body, .. } => collect_written(&body.stmts, by_name, regs, out),
            Stmt::For { body, .. } => collect_written(&body.stmts, by_name, regs, out),
            _ => {}
        }
    }
}

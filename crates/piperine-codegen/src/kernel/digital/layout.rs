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
//!   register updates: reads see the live banks; `=` writes land live
//!   immediately, `<=` writes stage and are committed by the device after
//!   the call returns (LRM §8.5.3.3/4, T23).
//! - `watch(*abi, *out)` — evaluates each atomic event term (the signal under
//!   a `posedge`/`negedge`/`change`); the device compares against the
//!   previous values to derive the per-block `fired` flags.

use std::collections::HashMap;


use crate::resolve::{
    Domain, DigitalBody, LoweredBody, Type,
    NodeId, VarId,
};
use crate::error::CodegenError;



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
    /// Names of the int-bank vars that are genuinely 4-state (`Type::Quad`,
    /// LRM's `reg`) — as opposed to a 2-state `Type::Integer`/`Bool` var
    /// that merely shares the same bank. T25/LRMW2-23 (§7.3.2): only a
    /// `Quad` var's raw encoding can legitimately mean X(2)/Z(3); an
    /// `Integer` var holding the literal value `2` or `3` is an ordinary
    /// number, never a 4-state bit. [`Self::export_vars`] consults this to
    /// tell "digital X/Z crossing into a continuous (analog) context" apart
    /// from "an integer that happens to be 2 or 3".
    pub(crate) quad_var_names: HashMap<VarId, String>,
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
                    if info.ty == Type::Quad {
                        layout.quad_var_names.insert(id, info.name.clone());
                    }
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
    /// T25/LRMW2-23 (LRM §7.3.2): a `Quad` (4-state) var's raw encoding is
    /// only ever a real *value* when it is defined (`0`/`1`) — `2`/`3`
    /// (X/Z) crossing into the analog side's continuous var bank would
    /// silently read back as the real numbers `2.0`/`3.0`, values §7.3.2
    /// says an implicit conversion must never invent. Fails loud instead,
    /// naming the net, unless the caller uses one of the LRM's own
    /// escape hatches (`===`/`!==`, `case`/`casex`/`casez`) to handle X/Z
    /// explicitly before the value ever reaches a continuous context — a
    /// 2-state `Integer`/`Bool` var sharing the same bank is unaffected
    /// (`2`/`3` there is an ordinary number, not an unknown bit).
    pub fn export_vars(&self, vars_int: &[i64], vars_real: &[f64]) -> Result<Vec<(VarId, f64)>, CodegenError> {
        let mut out = Vec::with_capacity(self.written.len());
        for (&var_id, &slot) in &self.int_slot {
            if self.written.contains(&var_id) && slot < vars_int.len() {
                let raw = vars_int[slot];
                if matches!(raw, 2 | 3) {
                    if let Some(name) = self.quad_var_names.get(&var_id) {
                        let state = if raw == 2 { "X (unknown)" } else { "Z (high impedance)" };
                        return Err(CodegenError::unsupported(format!(
                            "digital net `{name}` is {state} while read in a continuous (analog) context — LRM §7.3.2 requires an explicit `===`/`!==` or `case`/`casex`/`casez` before converting a 4-state value to real"
                        )));
                    }
                }
                out.push((var_id, raw as f64));
            }
        }
        for (&var_id, &slot) in &self.real_slot {
            if self.written.contains(&var_id) && slot < vars_real.len() {
                out.push((var_id, vars_real[slot]));
            }
        }
        Ok(out)
    }
}

/// Collect every var a digital statement tree assigns to, resolving names
/// through the module's symbol table. Walks nested `if`/`match`/`@`-event
/// bodies — a var written only inside a clocked block is still written.
fn collect_written(
    stmts: &[piperine_lang::ast::Stmt],
    by_name: &HashMap<&str, VarId>,
    _regs: &[VarId],
    out: &mut std::collections::HashSet<VarId>,
) {
    for stmt in stmts {
        collect_written_one(stmt, by_name, _regs, out);
    }
}

/// Single-statement counterpart of [`collect_written`] — VAMS's `If`/
/// `EventControl`/etc. bodies are one `Stmt` (often a `Block`), not a
/// statement list, unlike PHDL's `Block { stmts, .. }` everywhere.
fn collect_written_one(
    stmt: &piperine_lang::ast::Stmt,
    by_name: &HashMap<&str, VarId>,
    regs: &[VarId],
    out: &mut std::collections::HashSet<VarId>,
) {
    use piperine_lang::ast::{Expr, Stmt};
    match stmt {
        // VAMS has no `VarDecl`-with-default statement (LRM local
        // declarations are name-only, `BlockItemDecl`) — a register's
        // power-on init is now a first-class `DigitalBody::reg_inits`
        // entry (`resolve/pom/body.rs`), never a statement in this tree,
        // so every `Assign`/`NonBlockingAssign` to a var is a genuine
        // per-evaluation write, register or not.
        Stmt::Assign { target: Expr::Ident { name, .. }, .. }
        | Stmt::NonBlockingAssign { target: Expr::Ident { name, .. }, .. } => {
            if let Some(&id) = by_name.get(name.as_str()) {
                out.insert(id);
            }
        }
        Stmt::Block { stmts, .. } => collect_written(stmts, by_name, regs, out),
        Stmt::If { then_branch, else_branch, .. } => {
            collect_written_one(then_branch, by_name, regs, out);
            if let Some(eb) = else_branch {
                collect_written_one(eb, by_name, regs, out);
            }
        }
        Stmt::Case { arms, .. } => {
            for arm in arms {
                collect_written_one(&arm.body, by_name, regs, out);
            }
        }
        Stmt::EventControl { body, .. }
        | Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. }
        | Stmt::DelayControl { body, .. } => collect_written_one(body, by_name, regs, out),
        // `-> e;` (LRMC-C5-04, T26) desugars to a write of `e` at emission
        // time (`emit/stmt.rs`), not a structural `Assign` node in this
        // tree — this pre-pass must know about that write independently,
        // or `e` never gets exported to the D2A bridge (`export_vars`
        // only exports vars in `self.written`) and a digital `-> e;`
        // silently never reaches an analog `@(e)` sensitivity.
        Stmt::EventTrigger { name, .. } => {
            if let Some(&id) = by_name.get(name.as_str()) {
                out.insert(id);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_layout(var_name: &str) -> (DigitalLayout, VarId) {
        let mut m = LoweredBody::new("t25_unit");
        let dreg = m.symbols.add_var(var_name, Type::Quad);
        let body = DigitalBody { inputs: vec![], outputs: vec![], regs: vec![dreg], reg_inits: vec![], stmts: vec![] };
        let mut layout = DigitalLayout::build(&m, &body);
        // Statements are empty here (this test drives the raw bank
        // directly, bypassing the JIT and the based-literal decoder — see
        // the integration test's own note on why `Z`(3) can't currently
        // be reached through a literal) — `written` must be seeded
        // manually to make the var eligible for export.
        layout.written.insert(dreg);
        (layout, dreg)
    }

    /// T25/LRMW2-23 (LRM §7.3.2): a `Quad` var whose raw encoding is `3`
    /// (Z, high impedance) fails loud and names the net — exercised
    /// directly against the raw bank (not reachable through a based
    /// literal today, per the integration test's own note) to prove the
    /// Z-worded branch of the error message.
    #[test]
    fn export_vars_fails_loud_on_a_quad_var_holding_raw_z() {
        let (layout, _dreg) = quad_layout("dreg");
        let err = layout.export_vars(&[3], &[]).expect_err("raw Z(3) must error");
        match err {
            CodegenError::Unsupported(msg) => {
                assert!(msg.contains("dreg"), "error must name the net: {msg}");
                assert!(msg.contains("Z (high impedance)"), "error must say Z: {msg}");
                assert!(msg.contains("7.3.2"), "error must cite LRM §7.3.2: {msg}");
            }
            other => panic!("expected CodegenError::Unsupported, got {other:?}"),
        }
    }

    /// T25/LRMW2-23: a `Quad` var whose raw encoding is `2` (X) fails loud
    /// too, worded distinctly from the `Z` case.
    #[test]
    fn export_vars_fails_loud_on_a_quad_var_holding_raw_x() {
        let (layout, _dreg) = quad_layout("dreg");
        let err = layout.export_vars(&[2], &[]).expect_err("raw X(2) must error");
        match err {
            CodegenError::Unsupported(msg) => {
                assert!(msg.contains("dreg"), "error must name the net: {msg}");
                assert!(msg.contains("X (unknown)"), "error must say X: {msg}");
            }
            other => panic!("expected CodegenError::Unsupported, got {other:?}"),
        }
    }

    /// Regression: a defined (`0`/`1`) raw value still exports normally.
    #[test]
    fn export_vars_still_works_for_defined_zero_and_one() {
        let (layout, dreg) = quad_layout("dreg");
        assert_eq!(layout.export_vars(&[0], &[]).expect("0 must not error"), vec![(dreg, 0.0)]);
        let (layout, dreg) = quad_layout("dreg");
        assert_eq!(layout.export_vars(&[1], &[]).expect("1 must not error"), vec![(dreg, 1.0)]);
    }
}

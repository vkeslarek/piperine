//! Digital kernel compilation: an [`crate::ir::IrDigitalBody`] to native
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

use std::collections::HashMap;


use crate::ir::{
    Domain, IrDigitalBody, LoweredBody, IrType,
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
}

impl DigitalLayout {
    pub(crate) fn build(module: &LoweredBody, body: &IrDigitalBody) -> Self {
        let mut layout = Self::default();
        for (i, &node) in body.inputs.iter().enumerate() {
            layout.input_index.insert(node, i);
        }
        for (i, &node) in body.outputs.iter().enumerate() {
            layout.output_index.insert(node, i);
        }
        for (id, info) in module.symbols.vars() {
            match info.ty {
                IrType::Real => {
                    layout.real_slot.insert(id, layout.num_real);
                    layout.num_real += 1;
                }
                IrType::Integer | IrType::Bool | IrType::Quad => {
                    layout.int_slot.insert(id, layout.num_int);
                    layout.num_int += 1;
                }
            }
        }
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

    /// Export all variable values as `f64`, indexed by `VarId`. Integer-bank
    /// vars are converted to `f64`. Used by the D2A bridge: the analog side
    /// reads digital register values through this export.
    pub fn export_vars(&self, vars_int: &[i64], vars_real: &[f64]) -> Vec<f64> {
        let num_vars = self.int_slot.len() + self.real_slot.len();
        let mut result = vec![0.0; num_vars];
        for (&var_id, &slot) in &self.int_slot {
            if let Some(i) = var_id.0.checked_sub(0)
                && (i as usize) < num_vars && slot < vars_int.len() {
                    result[i as usize] = vars_int[slot] as f64;
                }
        }
        for (&var_id, &slot) in &self.real_slot {
            if let Some(i) = var_id.0.checked_sub(0)
                && (i as usize) < num_vars && slot < vars_real.len() {
                    result[i as usize] = vars_real[slot];
                }
        }
        result
    }
}

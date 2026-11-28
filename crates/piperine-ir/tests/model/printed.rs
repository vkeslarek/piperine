//! The printed text of an IR.

use piperine_ir::Ir;

/// The structural print of an IR, held to a block of consecutive lines.
pub(crate) struct Printed;

impl Printed {
    /// The structural print of `ir` holds `lines`, consecutive and in order.
    pub(crate) fn holds(ir: &Ir, lines: &[&str]) {
        let text = ir.print_structural();
        let block = lines.join("\n");
        assert!(text.contains(&format!("{block}\n")), "missing:\n{block}\n--- in ---\n{text}");
    }
}

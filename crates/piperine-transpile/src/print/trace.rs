//! `TracePrinter`: the trace sites of a body (analog-operators, design mechanism 6).
//!
//! An `emit!` in `analog` is a `Stmt::Trace` that stands where the block writes it. `terms_block`
//! takes a buffer `traces_` with one slot per site; the statement fills its slot with the record
//! (`ValuePrinter` prints it: a `BOOL` presence byte set, each value field where the watch's
//! layout puts it) wherever it executes, and `terms` copies each filled slot into the site's
//! `TRACE` watch. A slot left empty is the empty record (abi.md A5), which the solver does not
//! deliver. A trace changes no row, so the terms are what they would be without it.

use std::collections::{BTreeSet, HashSet};

use piperine_ir::{Body, Expr, ExprId, Ir, Stmt, StmtId};

use crate::print::ValuePrinter;

/// Finds and names the trace sites of one body.
#[derive(Debug)]
pub struct TracePrinter;

impl TracePrinter {
    /// The top-level statements of `body`'s `analog` block that are, or hold, a trace: the
    /// slice of `terms` keeps them whatever they read.
    pub fn holders(body: Body<'_>) -> BTreeSet<StmtId> {
        let ir = body.ir();
        body.analog().unwrap_or(&[]).iter().copied().filter(|id| Self::holds(ir, *id)).collect()
    }

    /// Whether statement `root` is a trace or holds one in an expression.
    fn holds(ir: &Ir, root: StmtId) -> bool {
        let mut stmts = vec![root];
        let mut seen: HashSet<ExprId> = HashSet::new();
        while let Some(s) = stmts.pop() {
            if matches!(ir.stmt(s), Stmt::Trace { .. }) {
                return true;
            }
            let mut exprs = ir.stmt(s).operands();
            while let Some(e) = exprs.pop() {
                if !seen.insert(e) {
                    continue;
                }
                match ir.expr(e) {
                    Expr::Block { stmts: inner, value } => {
                        stmts.extend(inner);
                        exprs.extend(value);
                    }
                    other => ValuePrinter::children(ir, other, &mut exprs),
                }
            }
        }
        false
    }

    /// The parameter of `terms_block` that holds the trace buffer, typed for `count` sites.
    pub fn parameter(count: usize) -> String {
        format!("{}: &mut [Option<Vec<u8>>; {count}]", ValuePrinter::TRACES)
    }
}

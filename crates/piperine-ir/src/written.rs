//! The locals a piece of code writes (`Ir::written_locals`): the effect of a statement or a closure
//! body is each local it assigns, which the analog printer reads to keep a statement a later read needs.

use std::collections::BTreeSet;

use crate::ids::ExprId;
use crate::model::{Ir, Stmt};

impl Ir {
    /// The locals declared outside `e` that `e` assigns, in index order: every `Stmt::Assign`
    /// target in `e` (a closure body, a loop body and a nested block included) except a local a
    /// `Stmt::Let` inside `e` declares, which no code after `e` can see.
    pub fn written_locals(&self, e: ExprId) -> Vec<u32> {
        let (mut written, mut declared) = (BTreeSet::new(), BTreeSet::new());
        let mut stack = vec![e];
        while let Some(id) = stack.pop() {
            let kind = self.expr(id);
            stack.extend(kind.operands());
            for stmt in kind.statements() {
                match self.stmt(*stmt) {
                    Stmt::Assign { place, .. } => {
                        written.insert(place.local);
                    }
                    Stmt::Let { local, .. } => {
                        declared.insert(*local);
                    }
                    _ => {}
                }
                stack.extend(self.stmt(*stmt).operands());
            }
        }
        written.difference(&declared).copied().collect()
    }
}

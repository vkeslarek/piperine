//! `ContributionSums`: how a structured `terms_block` accumulates the contributions to each
//! branch (analog-structure, design mechanisms 1 and 2).
//!
//! A body whose `analog` block holds a contribution under an `if`, in a `match` arm or in a
//! nested block, or that has a switch branch, prints its block as written, with one mutable
//! accumulator per quantity a branch receives, each contribution adding to its branch's sum where
//! it stands. An arm not taken adds nothing, so a branch whose contributions are all skipped has
//! zero flow (§11 rule 4). A switch branch also keeps one flag per quantity, set where a
//! contribution of that quantity runs: the flags decide which equation the branch has in this
//! solve and detect both kinds enabled (§11 rule 2, `E0418`). A body with neither a nested
//! contribution nor a switch branch does not use this form: its text is what it was.

use std::collections::{BTreeSet, HashSet};

use piperine_ir::{Body, BranchKind, Expr, ExprId, Ir, Operator, Origin, QuantityRole, StmtId, Terminal};

use crate::print::{Text, ValuePrinter};

/// What a branch receives in the `analog` block.
#[derive(Clone, Copy, Debug, Default)]
struct Received {
    /// A contribution to the branch's potential is written.
    potential: bool,
    /// A contribution to the branch's flow is written.
    flow: bool,
    /// A potential contribution has a direct addend: the branch has a potential sum.
    potential_sum: bool,
    /// A flow contribution has a direct addend: the branch has a flow sum.
    flow_sum: bool,
    /// An addend of any contribution is a `ddt`: the branch has a charge sum.
    charge: bool,
}

/// The accumulators of the branches of one body.
#[derive(Clone, Debug)]
pub struct ContributionSums {
    /// Per body branch: its kind and what it receives.
    branches: Vec<(BranchKind, Received)>,
    /// Per body branch: how its `v(a, b)` and its `i(a, b)` read in a comment.
    access: Vec<(String, String)>,
}

impl ContributionSums {
    /// Whether `body` prints in the structured form: a contribution that is not a top-level
    /// statement of the `analog` block, or a switch branch.
    pub fn needed(body: Body<'_>) -> bool {
        let top: HashSet<StmtId> = body.analog().unwrap_or(&[]).iter().copied().collect();
        body.branches().iter().any(|b| b.kind == BranchKind::Switch) || body.contributions().iter().any(|c| !top.contains(&c.stmt))
    }

    /// The accumulators of `body`'s branches, from the contributions its block holds.
    pub fn of(body: Body<'_>) -> ContributionSums {
        let mut branches: Vec<(BranchKind, Received)> = body.branches().iter().map(|b| (b.kind, Received::default())).collect();
        for contribution in body.contributions() {
            let Some((_, received)) = branches.get_mut(contribution.branch as usize) else { continue };
            match contribution.quantity {
                QuantityRole::Potential => received.potential = true,
                QuantityRole::Flow => received.flow = true,
            }
            for addend in contribution.addends {
                match (addend.operator, contribution.quantity) {
                    (Operator::Ddt, _) => received.charge = true,
                    (Operator::Direct, QuantityRole::Potential) => received.potential_sum = true,
                    (Operator::Direct, QuantityRole::Flow) => received.flow_sum = true,
                }
            }
        }
        let access = (0..body.branches().len() as u32)
            .map(|b| (Self::branch_text(body, b, QuantityRole::Potential), Self::branch_text(body, b, QuantityRole::Flow)))
            .collect();
        ContributionSums { branches, access }
    }

    /// `i(a, k)` or `v(p, n)`: what a contribution to `branch` writes, with its nature's access.
    pub(crate) fn branch_text(body: Body<'_>, branch: u32, quantity: QuantityRole) -> String {
        let Some(b) = body.branches().get(branch as usize) else { return format!("branch {branch}") };
        let node = |t: Terminal| match t {
            Terminal::Node(i) => body.nodes().get(i as usize),
            Terminal::Ground => None,
            Terminal::Own(_) => panic!("Ir::check: an own terminal belongs in a child's node slot only"),
        };
        let nature = node(b.plus).or_else(|| node(b.minus)).map(|n| body.ir().nature(n.nature));
        let access = nature.map_or_else(
            || if quantity == QuantityRole::Flow { "i".to_string() } else { "v".to_string() },
            |n| match quantity {
                QuantityRole::Flow => n.flow().map_or_else(|| "flow".to_string(), |q| q.access.clone()),
                QuantityRole::Potential => n.potential().access.clone(),
            },
        );
        let name = |t: Terminal| node(t).map_or_else(|| "gnd".to_string(), |n| n.name.clone());
        match b.minus {
            Terminal::Ground => format!("{access}({})", name(b.plus)),
            _ => format!("{access}({}, {})", name(b.plus), name(b.minus)),
        }
    }

    /// The top-level statements of `body`'s `analog` block that are, or hold, a contribution.
    pub fn holders(body: Body<'_>) -> BTreeSet<StmtId> {
        let ir = body.ir();
        let contributed: HashSet<StmtId> = body.contributions().iter().map(|c| c.stmt).collect();
        body.analog().unwrap_or(&[]).iter().copied().filter(|id| Self::holds(ir, *id, &contributed)).collect()
    }

    /// Whether statement `root` is a contribution of `contributed` or holds one in an expression.
    fn holds(ir: &Ir, root: StmtId, contributed: &HashSet<StmtId>) -> bool {
        let mut stmts = vec![root];
        let mut seen: HashSet<ExprId> = HashSet::new();
        while let Some(s) = stmts.pop() {
            if contributed.contains(&s) {
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

    /// Whether a branch can fault: a switch branch that receives both quantities (`E0418`).
    pub fn faults(&self) -> bool {
        self.branches.iter().any(|(kind, r)| *kind == BranchKind::Switch && r.potential && r.flow)
    }

    /// The variable that sums what `branch` receives with `operator` and `quantity`: `b0_flow_` on
    /// a flow branch, `b0_potential_` on a potential branch (whose `b0_flow_` is its flow pin),
    /// and on a switch branch `b0_potential_` and `b0_flow_sum_`; `b0_charge_` for a `ddt`.
    pub fn sum(&self, branch: u32, quantity: QuantityRole, operator: Operator) -> String {
        if operator == Operator::Ddt {
            return Text::generated(&format!("b{branch}_charge"));
        }
        match self.branches.get(branch as usize).map(|(kind, _)| *kind) {
            Some(BranchKind::Potential) => Text::generated(&format!("b{branch}_potential")),
            Some(BranchKind::Switch) if quantity == QuantityRole::Potential => Text::generated(&format!("b{branch}_potential")),
            Some(BranchKind::Switch) => Text::generated(&format!("b{branch}_flow_sum")),
            _ => Text::generated(&format!("b{branch}_flow")),
        }
    }

    /// The flag set where a contribution of `quantity` to `branch` runs; only a switch branch has
    /// one.
    pub fn flag(&self, branch: u32, quantity: QuantityRole) -> Option<String> {
        let switch = self.branches.get(branch as usize).is_some_and(|(kind, _)| *kind == BranchKind::Switch);
        let name = if quantity == QuantityRole::Potential { "potential" } else { "flow" };
        switch.then(|| Text::generated(&format!("b{branch}_{name}_on")))
    }

    /// `// contribution to v(a), branch b0 @ file:line:col`, over a contribution at `at`.
    pub fn comment(&self, branch: u32, quantity: QuantityRole, at: &Origin) -> String {
        let (potential, flow) = self.access.get(branch as usize).cloned().unwrap_or_default();
        let access = if quantity == QuantityRole::Potential { potential } else { flow };
        format!("// contribution to {access}, branch b{branch} @ {}:{}:{}", at.file, at.line, at.col)
    }

    /// The `let mut` lines that start every accumulator at zero and every flag of a switch branch
    /// at `false`, in branch order.
    pub fn declarations(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (b, (_, received)) in self.branches.iter().enumerate() {
            let b = b as u32;
            let sums = [
                (received.potential_sum, QuantityRole::Potential, Operator::Direct),
                (received.flow_sum, QuantityRole::Flow, Operator::Direct),
                (received.charge, QuantityRole::Flow, Operator::Ddt),
            ];
            for (held, quantity, operator) in sums {
                if held {
                    lines.push(format!("let mut {} = R::constant(0.0);", self.sum(b, quantity, operator)));
                }
            }
            for (held, quantity) in [(received.potential, QuantityRole::Potential), (received.flow, QuantityRole::Flow)] {
                if let Some(flag) = self.flag(b, quantity).filter(|_| held) {
                    // A flag every arm of an `if`/`else` sets (`if c { v +> 1.0 } else { v +> 2.0 }`) is never
                    // read at `false`: the start value is still the one an untaken path reads.
                    lines.push(format!("#[allow(unused_assignments)] let mut {flag} = false;"));
                }
            }
        }
        lines
    }

    /// The `if` lines that return `E0418` for each switch branch that receives both quantities,
    /// naming the branch as the potential access writes it, when `checked` (a Rust expression)
    /// holds.
    pub fn exclusivity_checks(&self, checked: &str) -> Vec<String> {
        let mut lines = Vec::new();
        for (b, (kind, received)) in self.branches.iter().enumerate() {
            if *kind != BranchKind::Switch || !(received.potential && received.flow) {
                continue;
            }
            let b = b as u32;
            let (potential, flow) = (self.flag(b, QuantityRole::Potential).unwrap_or_default(), self.flag(b, QuantityRole::Flow).unwrap_or_default());
            let name = self.access.get(b as usize).map(|(v, _)| v.clone()).unwrap_or_default();
            lines.push(format!("if {checked} && {potential} && {flow} {{"));
            lines.push(format!(
                "    return Err(Fault::new(418, \"E0418: branch {name} receives a potential and a flow contribution in one solve\"));"
            ));
            lines.push("}".to_string());
        }
        lines
    }

    /// The value of branch `branch`'s equation term: `probe` is `x(plus) - x(minus)` and `flow`
    /// the variable of its flow pin. A potential branch is `probe - potential`; a switch branch
    /// selects, per solve, the potential equation, the flow equation `flow - Σ flow`, or zero
    /// flow (design mechanism 2).
    pub fn equation(&self, branch: u32, probe: &str, flow: &str) -> String {
        let Some((kind, received)) = self.branches.get(branch as usize) else { return probe.to_string() };
        let potential = self.sum(branch, QuantityRole::Potential, Operator::Direct);
        let potential_equation = if received.potential_sum { format!("{probe} - {potential}") } else { probe.to_string() };
        if *kind != BranchKind::Switch {
            return potential_equation;
        }
        let mut text = String::new();
        if received.potential {
            let on = self.flag(branch, QuantityRole::Potential).unwrap_or_default();
            text.push_str(&format!("if {on} {{ {potential_equation} }} else "));
        }
        if received.flow {
            let on = self.flag(branch, QuantityRole::Flow).unwrap_or_default();
            let flow_equation = if received.flow_sum {
                format!("{flow} - {}", self.sum(branch, QuantityRole::Flow, Operator::Direct))
            } else {
                flow.to_string()
            };
            text.push_str(&format!("if {on} {{ {flow_equation} }} else "));
        }
        text.push_str(&format!("{{ {flow} }}"));
        text
    }
}

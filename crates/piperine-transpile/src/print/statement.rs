//! `StatementPrinter`: the statements of the IR as Rust lines (one statement each): a `let`, an
//! expression statement, a contribution in the structured form of an `analog` block, and an
//! assignment (composite-values CV-01 to CV-03: `x = e;`, `x = x + e;`, through field and index
//! projections). The values a statement holds print through the owner's `ExpressionPrinter`; a
//! trace, an `emit!` and a commit through its `RecordPrinter`.

use piperine_ir::{Addend, BinaryOp, Expr, ExprId, Local, Place, Projection, QuantityRole, Stmt, StmtId, TypeKind, ValueType};

use crate::Error;
use crate::print::expression::{Piece, Prec};
use crate::print::{ContributionSums, ExpressionPrinter};

/// Prints the statements of one owner: what it reads of each local (its type, whether it is
/// `mut`, whether the elaborator made it) and the accumulators a contribution adds to.
#[derive(Debug)]
pub struct StatementPrinter {
    /// The type of each local of the owner, by index.
    local_types: Vec<ValueType>,
    /// Whether each local of the owner was declared `let mut`, by index: only that local prints
    /// `let mut` and is the target of an assignment (§1.5.12).
    mutable: Vec<bool>,
    /// Whether each local of the owner is a temporary the elaborator made (its IR name holds a `$`, which no
    /// written identifier can), by index: such a local may go unread (a `match` temporary whose arm binds nothing).
    synthesized: Vec<bool>,
    /// The accumulators a contribution adds to, in the structured form of an `analog` block
    /// (`ContributionSums`); `None` where a contribution has no lowering.
    sums: Option<ContributionSums>,
}

impl StatementPrinter {
    /// The printer of the statements of an owner whose locals are `locals`, where a contribution has no lowering.
    pub(crate) fn of_locals(locals: &[Local]) -> StatementPrinter {
        StatementPrinter {
            local_types: locals.iter().map(|l| l.ty.clone()).collect(),
            mutable: locals.iter().map(|l| l.mutable).collect(),
            synthesized: locals.iter().map(Self::is_synthesized).collect(),
            sums: None,
        }
    }

    /// This printer with a contribution printing as an addition to the accumulators `sums` names.
    pub(crate) fn with_contribution_sums(self, sums: ContributionSums) -> StatementPrinter {
        StatementPrinter { sums: Some(sums), ..self }
    }

    /// Whether `local` is a temporary of the elaborator rather than a name the author wrote.
    fn is_synthesized(local: &Local) -> bool {
        local.name.contains('$')
    }

    /// Statement `s` of the owner `expressions` prints: `let name = init;`, `expr;`, a
    /// contribution, an assignment, or a record write.
    pub fn statement(&self, expressions: &ExpressionPrinter<'_>, s: StmtId) -> Result<String, Error> {
        let ir = expressions.ir;
        match ir.stmt(s) {
            Stmt::Let { local, init } => self.let_statement(expressions, *local, *init),
            Stmt::Expr(e) => Self::expression_statement(expressions, *e),
            Stmt::Contribute { branch, quantity, addends } => match &self.sums {
                Some(sums) => Self::contribution(expressions, sums, *branch, *quantity, addends),
                None => Err(Error::Unsupported {
                    form: "contribution".into(),
                    path: format!("{} s{}", expressions.path, s.index()),
                    origin: addends.first().map_or_else(|| expressions.origin.clone(), |a| ir.expr_origin(a.value).clone()),
                }),
            },
            Stmt::Assign { place, op, value } => self.assign(expressions, place, *op, *value),
            Stmt::BoundStep { value, .. } => Err(expressions.unsupported(*value, "`bound_step` outside the top level of the `analog` block")),
            Stmt::Trace { site, fields } => expressions.records().trace(expressions, s, *site, fields),
            Stmt::Emit { stream, fields } => expressions.records().emit(expressions, s, *stream, fields),
            Stmt::Commit { carrier, value, after, target } => expressions.records().commit(expressions, s, *carrier, *value, *after, target.as_ref()),
        }
    }

    /// `let name = init;`: `let mut` for a local declared so, an annotation for a function value.
    fn let_statement(&self, expressions: &ExpressionPrinter<'_>, local: u32, init: ExprId) -> Result<String, Error> {
        let Some(name) = expressions.local(local) else {
            return Err(expressions.unsupported(init, "a let of a local out of range"));
        };
        let init = expressions.expr(init)?;
        // a deferred `let x;` starts at a placeholder its first assignment overwrites
        // a temporary the elaborator makes for a `match` (`match$0.val`) is unread when its arm binds nothing (`Some(_)`)
        let keyword = if self.mutable.get(local as usize).copied().unwrap_or(false) {
            "#[allow(unused_assignments, unused_variables)] let mut"
        } else if self.synthesized.get(local as usize).copied().unwrap_or(false) {
            "#[allow(unused_variables)] let"
        } else {
            "let"
        };
        // a function value has no type of its own to infer: the local is the `Rc<dyn Fn>` it holds
        let annotation = match self.local_types.get(local as usize) {
            Some(ty @ ValueType::Fn(..)) => format!(": {}", expressions.rust_type(ty, &expressions.path)?),
            _ => String::new(),
        };
        Ok(format!("{keyword} {name}{annotation} = {init};"))
    }

    /// `e` as a statement: an `if` closes with its brace; anything else takes a `;`.
    fn expression_statement(expressions: &ExpressionPrinter<'_>, e: ExprId) -> Result<String, Error> {
        let piece: Piece = expressions.piece(e)?;
        let ir = expressions.ir;
        let block = matches!(ir.expr(e), Expr::If { .. }) && *ir.expr_type(e) == ValueType::Unit && piece.prec == Prec::Low;
        Ok(if block { piece.text } else { format!("{};", piece.text) })
    }

    /// A contribution of the structured form (`ContributionSums`): each addend added to its
    /// branch's sum, then the flag of its quantity set when the branch has one. An arm not taken
    /// never runs this, so it adds nothing (§11 rule 4).
    fn contribution(expressions: &ExpressionPrinter<'_>, sums: &ContributionSums, branch: u32, quantity: QuantityRole, addends: &[Addend]) -> Result<String, Error> {
        let mut lines = Vec::with_capacity(addends.len() + 2);
        if let Some(first) = addends.first() {
            lines.push(sums.comment(branch, quantity, expressions.ir.expr_origin(first.value)));
        }
        for addend in addends {
            let sum = sums.sum(branch, quantity, addend.operator);
            let value = if addend.negated { expressions.negated(addend.value)? } else { expressions.right_of_add(addend.value)? };
            lines.push(format!("{sum} = {sum} + {value};"));
        }
        lines.extend(sums.flag(branch, quantity).map(|flag| format!("{flag} = true;")));
        Ok(lines.join("\n"))
    }

    /// `place = value;`, or `place = place op value;` for `place op= value;`: the `rt` integers,
    /// `Real` has the operators but no `op=` forms (§1.5.12).
    fn assign(&self, expressions: &ExpressionPrinter<'_>, place: &Place, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        if !place.path.is_empty() {
            return self.assign_through(expressions, place, op, value);
        }
        let (Some(name), Some(ty)) = (expressions.local(place.local), self.local_types.get(place.local as usize)) else {
            return Err(expressions.unsupported(value, "an assignment to a local out of range"));
        };
        Self::assign_to(expressions, name, ty, op, value)
    }

    /// `target = value;` or `target = target op value;`, `target` a Rust place of type `ty`.
    fn assign_to(expressions: &ExpressionPrinter<'_>, name: &str, ty: &ValueType, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        let Some(op) = op else {
            // a temporary an inlined `Option` call assigns may never be read (`f().is_some()`): an
            // attribute on an assignment is unstable, on a block statement it is not
            return Ok(format!("#[allow(unused_assignments)] {{ {name} = {}; }}", expressions.expr(value)?));
        };
        let real = *ty == ValueType::F64 && !expressions.plain;
        let (symbol, prec) = expressions.operator(value, op, real)?;
        if matches!(prec, Prec::Compare | Prec::And | Prec::Or) {
            return Err(expressions.unsupported(value, &format!("`{op:?}=`: only an arithmetic or bit operator has a compound assignment")));
        }
        let right = expressions.piece(value)?;
        let right = match prec {
            Prec::BitAnd | Prec::BitOr | Prec::BitXor | Prec::Shift => right.at(Prec::Unary),
            _ => right.at(ExpressionPrinter::stronger(prec)),
        };
        // the last assignment of a loop may never be read: an attribute on an assignment is
        // unstable, on a block statement it is not
        Ok(format!("#[allow(unused_assignments)] {{ {name} = {name} {symbol} {right}; }}"))
    }

    /// `place = value;` or `place op= value;` where the place has projections: Rust's place
    /// expression, each index checked.
    fn assign_through(&self, expressions: &ExpressionPrinter<'_>, place: &Place, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        let (Some(name), Some(local_ty)) = (expressions.local(place.local), self.local_types.get(place.local as usize)) else {
            return Err(expressions.unsupported(value, "an assignment to a local out of range"));
        };
        let (mut target, mut ty) = (name.to_string(), local_ty.clone());
        for step in &place.path {
            match step {
                Projection::Field(k) => {
                    let label = expressions.field_name(&ty, *k).ok_or_else(|| expressions.unsupported(value, "an assignment to a field that does not exist"))?;
                    let next = Self::field_type(expressions, &ty, *k).ok_or_else(|| expressions.unsupported(value, "an assignment to a field that does not exist"))?;
                    target = format!("{target}.{label}");
                    ty = next;
                }
                Projection::Index(index) => {
                    let ValueType::Array(element, _) = &ty else { return Err(expressions.unsupported(value, "an assignment to an element of a value that is not an array")) };
                    let site = expressions.site(value);
                    target = format!("{target}[rt::Index::of({}, {target}.len(), {site:?})]", expressions.index_value(*index)?);
                    ty = (**element).clone();
                }
            }
        }
        Self::assign_to(expressions, &target, &ty, op, value)
    }

    /// The type of field `index` of a struct or a tuple type.
    fn field_type(expressions: &ExpressionPrinter<'_>, ty: &ValueType, index: u32) -> Option<ValueType> {
        match ty {
            ValueType::Tuple(parts) => parts.get(index as usize).cloned(),
            ValueType::Struct(id) => match &expressions.ir.type_data(*id).kind {
                TypeKind::Struct { fields, .. } => fields.get(index as usize).map(|(_, ty)| ty.clone()),
                TypeKind::Enum { .. } => None,
            },
            _ => None,
        }
    }
}

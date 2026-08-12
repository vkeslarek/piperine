//! Expression grammar — LRM chapter 4, Annex A.8.3. Pratt (precedence
//! climbing) parser; the precedence table is Table 4-1, encoded as a data
//! table (`binop_bp`), not a chain of `if`/`match` cascades (MD-13 rule 5).

use crate::ast::{BinOp, Expr, UnOp};
use crate::error::ParseError;
use crate::grammar::cursor::Grammar;
use crate::lexer::Tok;

/// (left binding power, right binding power) per Table 4-1, tightest
/// first. Ternary (`?:`) and the logical/bitwise chain are handled by
/// `parse_expr_bp`'s loop; this table covers every binary infix operator.
fn binop_bp(op: BinOp) -> (u8, u8) {
    use BinOp::*;
    match op {
        Pow => (24, 23), // right-associative
        Mul | Div | Mod => (21, 22),
        Add | Sub => (19, 20),
        Shl | Shr => (17, 18),
        Lt | Le | Gt | Ge => (15, 16),
        Eq | Ne | CaseEq | CaseNe => (13, 14),
        BitAnd => (11, 12),
        BitXor | BitXnor => (9, 10),
        BitOr => (7, 8),
        LogAnd => (5, 6),
        LogOr => (3, 4),
    }
}

fn punct_to_binop(p: &str) -> Option<BinOp> {
    use BinOp::*;
    Some(match p {
        "**" => Pow,
        "*" => Mul,
        "/" => Div,
        "%" => Mod,
        "+" => Add,
        "-" => Sub,
        "<<" | "<<<" => Shl,
        ">>" | ">>>" => Shr,
        "<" => Lt,
        "<=" => Le,
        ">" => Gt,
        ">=" => Ge,
        "==" => Eq,
        "!=" => Ne,
        "===" => CaseEq,
        "!==" => CaseNe,
        "&" => BitAnd,
        "^" => BitXor,
        "~^" | "^~" => BitXnor,
        "|" => BitOr,
        "&&" => LogAnd,
        "||" => LogOr,
        _ => return None,
    })
}

fn punct_to_unop(p: &str) -> Option<UnOp> {
    use UnOp::*;
    Some(match p {
        "+" => Plus,
        "-" => Neg,
        "!" => LogNot,
        "~" => BitNot,
        "&" => AndReduce,
        "~&" => NandReduce,
        "|" => OrReduce,
        "~|" => NorReduce,
        "^" => XorReduce,
        "~^" | "^~" => XnorReduce,
        _ => return None,
    })
}

impl Grammar {
    /// `expression`, LRM Annex A.8.3 — the ternary/conditional level, the
    /// grammar's top entry point for any expression.
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let cond = self.parse_expr_bp(0)?;
        if self.eat_punct("?") {
            let then_branch = self.parse_expr()?;
            self.expect_punct(":")?;
            let else_branch = self.parse_expr()?;
            let span = crate::error::Span::new(cond.span().start, else_branch.span().end);
            return Ok(Expr::Ternary {
                span,
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            });
        }
        Ok(cond)
    }

    /// Precedence-climbing loop over Table 4-1's binary operators.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let Tok::Punct(p) = self.peek() else { break };
            let Some(op) = punct_to_binop(p) else { break };
            let (lbp, rbp) = binop_bp(op);
            if lbp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.parse_expr_bp(rbp)?;
            let span = crate::error::Span::new(lhs.span().start, rhs.span().end);
            lhs = Expr::Binary { span, op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    /// `unary_operator primary`, §4.1.3 — unary ops bind tighter than any
    /// binary operator (Table 4-1).
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if let Tok::Punct(p) = self.peek().clone() {
            if let Some(op) = punct_to_unop(&p) {
                let start = self.bump().span;
                let operand = self.parse_unary()?;
                let span = crate::error::Span::new(start.start, operand.span().end);
                return Ok(Expr::Unary { span, op, operand: Box::new(operand) });
            }
        }
        self.parse_postfix()
    }

    /// `primary { '[' expr [':' expr] ']' }` — index / part-select
    /// postfix, §4.1.
    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut base = self.parse_primary()?;
        loop {
            if self.eat_punct("[") {
                let first = self.parse_expr()?;
                if self.eat_punct(":") {
                    let lsb = self.parse_expr()?;
                    let end = self.expect_punct("]")?;
                    let span = crate::error::Span::new(base.span().start, end.end);
                    base = Expr::PartSelect {
                        span,
                        base: Box::new(base),
                        msb: Box::new(first),
                        lsb: Box::new(lsb),
                    };
                } else {
                    let end = self.expect_punct("]")?;
                    let span = crate::error::Span::new(base.span().start, end.end);
                    base = Expr::Index { span, base: Box::new(base), index: Box::new(first) };
                }
            } else {
                break;
            }
        }
        Ok(base)
    }

    /// `primary`, Annex A.8.3 — literals, identifiers, calls, access
    /// functions, concatenation/replication, parenthesized expressions.
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let span = self.peek_span();
        match self.peek().clone() {
            Tok::Number(n) => {
                self.bump();
                Ok(Expr::Number { span, value: n })
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Str { span, value: s })
            }
            Tok::Punct("(") => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            Tok::Punct("{") => self.parse_concat_or_replication(),
            Tok::SystemIdent(name) => {
                self.bump();
                let args = self.parse_optional_arg_list()?;
                Ok(Expr::SystemFunctionCall { span, name, args })
            }
            Tok::Ident(name) if name == "V" || name == "I" => {
                self.bump();
                self.parse_access_function(span, name)
            }
            Tok::Ident(name) => {
                self.bump();
                if self.at_punct("(") {
                    // Any `name(args)` is a call — user function (§5.9), a
                    // §4.3 math function, or a §4.5 time-domain/analog
                    // operator (`ddt`/`idt`/`transition`/`laplace_*`/...).
                    // Which one `name` resolves to is an elaboration
                    // concern (the declared-language-surface lookup), not
                    // a parse-time distinction.
                    let args = self.parse_optional_arg_list()?;
                    return Ok(Expr::Call { span, name, args });
                }
                Ok(Expr::Ident { span, name })
            }
            Tok::EscapedIdent(name) => {
                self.bump();
                Ok(Expr::Ident { span, name })
            }
            Tok::Eof => Err(self.eof_error("an expression".to_string())),
            _ => Err(self.unexpected("an expression".to_string())),
        }
    }

    /// `V(...)`/`I(...)` branch access function — §5.3/§5.4, one or two
    /// terminal/branch arguments.
    fn parse_access_function(
        &mut self,
        start: crate::error::Span,
        function: String,
    ) -> Result<Expr, ParseError> {
        self.expect_punct("(")?;
        let pos = self.parse_expr()?;
        let neg = if self.eat_punct(",") { Some(Box::new(self.parse_expr()?)) } else { None };
        let end = self.expect_punct(")")?;
        let span = crate::error::Span::new(start.start, end.end);
        Ok(Expr::AccessFunction { span, function, pos: Box::new(pos), neg })
    }

    /// `'(' [expr {',' expr}] ')'` — a call's argument list; `()` with no
    /// arguments is legal.
    fn parse_optional_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        if !self.eat_punct("(") {
            return Ok(vec![]);
        }
        let mut args = vec![];
        if !self.at_punct(")") {
            args.push(self.parse_expr()?);
            while self.eat_punct(",") {
                args.push(self.parse_expr()?);
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    /// `'{' expr {',' expr} '}'` (concatenation) or
    /// `'{' expr '{' expr {',' expr} '}' '}'` (replication) — §4.6.1.
    fn parse_concat_or_replication(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect_punct("{")?;
        let first = self.parse_expr()?;
        if self.at_punct("{") {
            // Replication: `first` was the repeat count.
            self.bump();
            let mut parts = vec![self.parse_expr()?];
            while self.eat_punct(",") {
                parts.push(self.parse_expr()?);
            }
            self.expect_punct("}")?;
            let end = self.expect_punct("}")?;
            let span = crate::error::Span::new(start.start, end.end);
            return Ok(Expr::Replication { span, count: Box::new(first), parts });
        }
        let mut parts = vec![first];
        while self.eat_punct(",") {
            parts.push(self.parse_expr()?);
        }
        let end = self.expect_punct("}")?;
        let span = crate::error::Span::new(start.start, end.end);
        Ok(Expr::Concat { span, parts })
    }
}

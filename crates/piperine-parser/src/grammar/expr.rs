//! Pratt expression parser. Binding powers follow the Verilog-A precedence
//! table; `**` is right-associative and `?:` (ternary) binds loosest.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn expr(&mut self) -> PResult<Expr> {
        self.expr_bp(0)
    }

    fn expr_bp(&mut self, min_bp: u8) -> PResult<Expr> {
        let mut lhs = self.prefix()?;
        loop {
            // ternary select: loosest precedence, right-associative
            if min_bp == 0 && self.eat(&Tok::Question) {
                let then_val = self.expr_bp(0)?;
                self.expect(&Tok::Colon)?;
                let else_val = self.expr_bp(0)?;
                lhs = Expr::Select(Box::new(lhs), Box::new(then_val), Box::new(else_val));
                continue;
            }
            // `x inside { ... }` — set membership, binds at equality precedence.
            if self.at_kw("inside") {
                const INSIDE_BP: u8 = 16;
                if INSIDE_BP < min_bp { break; }
                self.bump();
                lhs = self.parse_inside(lhs)?;
                continue;
            }
            let Some((op, lbp, rbp)) = self.peek_binop() else { break };
            if lbp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.expr_bp(rbp)?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
        }
        Ok(lhs)
    }

    /// `(op, left_bp, right_bp)` for the current token if it is a binary
    /// operator. The base power is doubled to make room, and right-associative
    /// operators lower their right power by one.
    fn peek_binop(&self) -> Option<(BinOp, u8, u8)> {
        let (op, bp, right_assoc) = match self.peek()? {
            Tok::PipePipe => (BinOp::OrOr, 2, false),
            Tok::AmpAmp => (BinOp::AndAnd, 3, false),
            Tok::Pipe => (BinOp::BitOr, 4, false),
            Tok::Caret => (BinOp::Xor, 5, false),
            Tok::XnorC => (BinOp::XNor1, 6, false),
            Tok::XnorT => (BinOp::XNor2, 6, false),
            Tok::Amp => (BinOp::BitAnd, 7, false),
            Tok::EqEq => (BinOp::Eq, 8, false),
            Tok::NotEq => (BinOp::Neq, 8, false),
            Tok::Ge => (BinOp::Ge, 9, false),
            Tok::Gt => (BinOp::Gt, 9, false),
            Tok::Le => (BinOp::Le, 9, false),
            Tok::Lt => (BinOp::Lt, 9, false),
            Tok::Shl => (BinOp::Shl, 10, false),
            Tok::Shr => (BinOp::Shr, 10, false),
            Tok::Plus => (BinOp::Add, 11, false),
            Tok::Minus => (BinOp::Sub, 11, false),
            Tok::Star => (BinOp::Mul, 12, false),
            Tok::Slash => (BinOp::Div, 12, false),
            Tok::Percent => (BinOp::Mod, 13, false),
            Tok::Pow => (BinOp::Pow, 14, true),
            _ => return None,
        };
        let lbp = bp * 2;
        let rbp = if right_assoc { lbp - 1 } else { lbp + 1 };
        Some((op, lbp, rbp))
    }

    /// Desugar `lhs inside { items }` into a boolean OR-chain. Each item is a
    /// scalar (`lhs == item`) or a range `[lo:hi]` (`lhs >= lo && lhs <= hi`),
    /// where `$` means the bound is open. An empty set is `0`.
    fn parse_inside(&mut self, lhs: Expr) -> PResult<Expr> {
        // The set opens with `{` (or `'{`).
        if !self.eat(&Tok::LBrace) { self.expect(&Tok::ArrStart)?; }
        let mut terms: Vec<Expr> = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at_end() {
            if self.eat(&Tok::LBrack) {
                let lo = if self.eat_dollar() { None } else { Some(self.expr()?) };
                self.expect(&Tok::Colon)?;
                let hi = if self.eat_dollar() { None } else { Some(self.expr()?) };
                self.expect(&Tok::RBrack)?;
                terms.push(range_term(&lhs, lo, hi));
            } else {
                let v = self.expr()?;
                terms.push(Expr::Binary(Box::new(lhs.clone()), BinOp::Eq, Box::new(v)));
            }
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RBrace)?;
        Ok(or_chain(terms))
    }

    /// Consume a `$` token (lexed as an empty system-call name) if present.
    fn eat_dollar(&mut self) -> bool {
        if matches!(self.peek(), Some(Tok::SysCall(s)) if s.is_empty()) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn prefix(&mut self) -> PResult<Expr> {
        let op = match self.peek() {
            Some(Tok::Minus) => PrefixOp::Neg,
            Some(Tok::Not) => PrefixOp::Not,
            Some(Tok::Tilde) => PrefixOp::BitNot,
            Some(Tok::Plus) => PrefixOp::Pos,
            _ => return self.postfix(),
        };
        self.bump();
        Ok(Expr::Prefix(op, Box::new(self.prefix()?)))
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.atom()?;
        loop {
            if self.eat(&Tok::LBrack) {
                let idx = self.expr()?;
                e = if self.eat(&Tok::Colon) {
                    let lsb = self.expr()?;
                    Expr::PartSelect(Box::new(e), Box::new(idx), Box::new(lsb))
                } else {
                    Expr::Index(Box::new(e), Box::new(idx))
                };
                self.expect(&Tok::RBrack)?;
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn atom(&mut self) -> PResult<Expr> {
        match self.peek() {
            Some(Tok::Int(s)) => {
                let lit = Literal::IntNumber(s.clone());
                self.bump();
                Ok(Expr::Literal(lit))
            }
            Some(Tok::Real(s)) => {
                let lit = real_literal(s);
                self.bump();
                Ok(Expr::Literal(lit))
            }
            Some(Tok::Str(s)) => {
                let lit = Literal::StrLit(s.clone());
                self.bump();
                Ok(Expr::Literal(lit))
            }
            Some(Tok::LParen) => {
                self.bump();
                let inner = self.expr()?;
                self.expect(&Tok::RParen)?;
                Ok(Expr::Paren(Box::new(inner)))
            }
            Some(Tok::ArrStart | Tok::LBrace) => {
                self.bump();
                let mut items = Vec::new();
                while !self.at(&Tok::RBrace) {
                    items.push(self.expr()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(&Tok::RBrace)?;
                Ok(Expr::Array(items))
            }
            Some(Tok::Lt) => {
                // port-flow access: `< path >`
                self.bump();
                let port = self.path()?;
                self.expect(&Tok::Gt)?;
                Ok(Expr::PortFlow(port))
            }
            Some(Tok::SysCall(s)) => {
                let func = FunctionRef::SysFun(format!("${s}"));
                self.bump();
                let args = if self.at(&Tok::LParen) { self.arg_list()? } else { Vec::new() };
                Ok(Expr::Call(func, args))
            }
            Some(Tok::Ident(s)) if s == "inf" => {
                self.bump();
                Ok(Expr::Literal(Literal::Inf))
            }
            Some(Tok::Ident(_)) => {
                let path = self.path()?;
                if self.at(&Tok::LParen) {
                    Ok(Expr::Call(FunctionRef::Path(path), self.arg_list()?))
                } else {
                    Ok(Expr::Path(path))
                }
            }
            other => Err(format!("expected expression, found {other:?}")),
        }
    }

    fn arg_list(&mut self) -> PResult<Vec<CallArg>> {
        self.expect(&Tok::LParen)?;
        let mut args = Vec::new();
        while !self.at(&Tok::RParen) {
            // Named arg: `ident = expr` (not `==`)
            let arg = if matches!(self.peek(), Some(Tok::Ident(_)))
                && matches!(self.peek_at(1), Some(Tok::Assign))
            {
                let name = self.ident()?;
                self.expect(&Tok::Assign)?;
                CallArg::Named(name, self.expr()?)
            } else {
                CallArg::Positional(self.expr()?)
            };
            args.push(arg);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RParen)?;
        Ok(args)
    }
}

/// A trailing letter marks an SI scale factor or a time unit; otherwise the
/// real is a plain decimal/exponent value.
fn real_literal(s: &str) -> Literal {
    match s.bytes().last() {
        Some(c) if c.is_ascii_alphabetic() => Literal::SiRealNumber(s.to_string()),
        _ => Literal::StdRealNumber(s.to_string()),
    }
}

// ── `inside` desugaring helpers ───────────────────────────────────────────────

fn int_lit(n: i64) -> Expr {
    Expr::Literal(Literal::IntNumber(n.to_string()))
}

/// Build the boolean test for one `[lo:hi]` range term of an `inside` set.
/// An open bound (`$`) drops that side of the comparison.
fn range_term(lhs: &Expr, lo: Option<Expr>, hi: Option<Expr>) -> Expr {
    let ge = lo.map(|lo| Expr::Binary(Box::new(lhs.clone()), BinOp::Ge, Box::new(lo)));
    let le = hi.map(|hi| Expr::Binary(Box::new(lhs.clone()), BinOp::Le, Box::new(hi)));
    match (ge, le) {
        (Some(a), Some(b)) => Expr::Binary(Box::new(a), BinOp::AndAnd, Box::new(b)),
        (Some(a), None)    => a,
        (None, Some(b))    => b,
        (None, None)       => int_lit(1), // `[$:$]` matches anything
    }
}

/// OR all membership terms together; an empty set is `0` (matches nothing).
fn or_chain(mut terms: Vec<Expr>) -> Expr {
    if terms.is_empty() { return int_lit(0); }
    let mut acc = terms.remove(0);
    for t in terms {
        acc = Expr::Binary(Box::new(acc), BinOp::OrOr, Box::new(t));
    }
    acc
}

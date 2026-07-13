//! Expression grammar: the Pratt binary-operator parser, primary
//! expressions, and array literals/comprehensions.

use crate::parse::ast::*;
use crate::parse::lexer::Tok;

use super::{Parse, Parser};

impl<'a> Parser<'a> {
    // ─────────────────────────── §8.2  Expressions ───────────────────────────
    //
    // Operator precedence (lowest to highest):
    //   Or(||)=1  And(&&)=2  BitOr(|)=3  BitAnd(&)=4  Eq/Neq=5  Rel<<=>>==6
    //   BitXor(^)=7  Add/Sub=8   Mul/Div/%=9

    /// Parses a full expression including all binary operators (bit-OR allowed).
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, crate::parse::error::ParseError> {
        self.parse_binary_expr(0, false)
    }

    /// Parses an expression but stops before a `|` operator (used inside array comprehensions).
    pub(crate) fn parse_expr_no_bitor(&mut self) -> Result<Expr, crate::parse::error::ParseError> {
        self.parse_binary_expr(0, true)
    }

    /// Pratt-style binary expression parser. `precedence` is the minimum binding power
    /// to continue; `stop_at_bitor` prevents absorbing `|` when scanning comprehension guards.
    pub(crate) fn parse_binary_expr(&mut self, precedence: u8, stop_at_bitor: bool) -> Result<Expr, crate::parse::error::ParseError> {
        let mut lhs = self.parse_primary()?;
        while let Some((op, prec)) = self.peek_binary_op() {
            if prec < precedence {
                break;
            }
            if stop_at_bitor && op == BinaryOp::BitOr {
                break;
            }
            self.pos += 1;
            let rhs = self.parse_binary_expr(prec + 1, stop_at_bitor)?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
        }
        Ok(lhs)
    }

    /// Peeks at the next token; if it is a binary operator, returns `(op, precedence)`.
    /// Precedence levels: Or=1, And=2, BitOr=3, BitAnd=4, Eq/Neq=5, Relational=6,
    /// BitXor=7, Add/Sub=8, Mul/Div/Rem=9.
    pub(crate) fn peek_binary_op(&mut self) -> Option<(BinaryOp, u8)> {
        match self.peek() {
            Some(Tok::Or)     => Some((BinaryOp::Or, 1)),
            Some(Tok::And)    => Some((BinaryOp::And, 2)),
            Some(Tok::BitOr)  => Some((BinaryOp::BitOr, 3)),
            Some(Tok::BitAnd) => Some((BinaryOp::BitAnd, 4)),
            Some(Tok::EqEq)   => Some((BinaryOp::Eq, 5)),
            Some(Tok::NotEq)  => Some((BinaryOp::Neq, 5)),
            Some(Tok::Lt)     => Some((BinaryOp::Lt, 6)),
            Some(Tok::Le)     => Some((BinaryOp::Le, 6)),
            Some(Tok::Gt)     => Some((BinaryOp::Gt, 6)),
            Some(Tok::Ge)     => Some((BinaryOp::Ge, 6)),
            Some(Tok::BitXor) => Some((BinaryOp::BitXor, 7)),
            Some(Tok::Plus)   => Some((BinaryOp::Add, 8)),
            Some(Tok::Minus)  => Some((BinaryOp::Sub, 8)),
            Some(Tok::Star)   => Some((BinaryOp::Mul, 9)),
            Some(Tok::Slash)  => Some((BinaryOp::Div, 9)),
            Some(Tok::Percent)=> Some((BinaryOp::Rem, 9)),
            _ => None,
        }
    }

    /// Parses an `if (cond) { .. } else ..` expression. The `else` arm may be
    /// another `if` (an `else if` chain) instead of a block; that nested
    /// `if`-expression is wrapped in a single-expression [`Block`] so
    /// `Expr::If` only ever needs one shape for `else_body`. Elaboration and
    /// lowering already recurse through nested `Expr::If`, so no changes are
    /// needed downstream — this only teaches the parser the `else if` shape.
    fn parse_if_expr(&mut self) -> Result<Expr, crate::parse::error::ParseError> {
        self.expect_ident_str("if")?;
        self.expect(&Tok::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Tok::RParen)?;
        let then_body = self.parse_block()?;
        self.expect_ident_str("else")?;
        let else_body = if matches!(self.peek(), Some(Tok::Ident(s)) if s == "if") {
            let nested = self.parse_if_expr()?;
            Block { stmts: vec![], expr: Some(Box::new(nested)) }
        } else {
            self.parse_block()?
        };
        Ok(Expr::If { cond: Box::new(cond), then_body, else_body })
    }

    // ─────────────────────────── §8.3  Primaries ─────────────────────────────

    /// Parses a primary expression: literals, identifiers, paths, blocks, `if`-expressions,
    /// lambdas, unary operators, array literals, and parenthesized expressions.
    /// Also handles postfix call/index/field/`::` operators and `BundleLit` sugar.
    pub(crate) fn parse_primary(&mut self) -> Result<Expr, crate::parse::error::ParseError> {
        let mut expr = match self.peek() {
            Some(Tok::Int(i)) => {
                let e = Expr::Literal(Literal::Int(*i));
                self.pos += 1;
                e
            }
            Some(Tok::Real(r)) => {
                let e = Expr::Literal(Literal::Real(*r));
                self.pos += 1;
                e
            }
            Some(Tok::Str(s)) => {
                let e = Expr::Literal(Literal::String(s.clone()));
                self.pos += 1;
                e
            }
            Some(Tok::Quad(q)) => {
                let e = Expr::Literal(Literal::Quad(q.clone()));
                self.pos += 1;
                e
            }
            Some(Tok::Ident(s)) => {
                if s == "if" {
                    self.parse_if_expr()?
                } else if s == "none" {
                    self.pos += 1;
                    Expr::Literal(Literal::None)
                } else {
                    let id = s.clone();
                    self.pos += 1;
                    if self.eat(&Tok::DoubleColon) {
                        let mut segments = vec![id];
                        segments.push(self.parse_ident()?);
                        while self.eat(&Tok::DoubleColon) {
                            segments.push(self.parse_ident()?);
                        }
                        Expr::Path(Path { segments })
                    } else {
                        Expr::Ident(id)
                    }
                }
            }
            Some(Tok::SysCall(s)) => {
                let id = s.clone();
                self.pos += 1;
                Expr::SysCall(id, vec![])
            }
            Some(Tok::LBrack) => {
                self.pos += 1;
                self.parse_array_expr()?
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                if self.eat(&Tok::RParen) {
                    Expr::Tuple(vec![])
                } else {
                    let first = self.parse_expr()?;
                    if self.eat(&Tok::Comma) {
                        let mut items = vec![first];
                        while !self.eat(&Tok::RParen) {
                            items.push(self.parse_expr()?);
                            if !self.eat(&Tok::Comma) {
                                self.expect(&Tok::RParen)?;
                                break;
                            }
                        }
                        Expr::Tuple(items)
                    } else {
                        self.expect(&Tok::RParen)?;
                        first
                    }
                }
            }
            Some(Tok::LBrace) => Expr::Block(self.parse_block()?),
            Some(Tok::Not) => {
                self.pos += 1;
                Expr::Unary(UnaryOp::Not, Box::new(self.parse_primary()?))
            }
            Some(Tok::Minus) => {
                self.pos += 1;
                Expr::Unary(UnaryOp::Neg, Box::new(self.parse_primary()?))
            }
            Some(Tok::BitOr) => {
                self.pos += 1;
                let mut params = Vec::new();
                if !self.eat(&Tok::BitOr) {
                    params.push(self.parse_ident()?);
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::BitOr) {
                            break;
                        }
                        params.push(self.parse_ident()?);
                    }
                    self.expect(&Tok::BitOr)?;
                }
                let body = self.parse_expr()?;
                Expr::Lambda { params, body: Box::new(body) }
            }
            _ => return Err(format!("Expected expression, found {:?}", self.peek()).into()),
        };

        // Postfix: Call, Index/Slice, Field, PathSeg
        loop {
            if self.eat(&Tok::LParen) {
                let mut args = Vec::new();
                if !self.eat(&Tok::RParen) {
                    args.push(self.parse_expr()?);
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::RParen) {
                            break;
                        }
                        args.push(self.parse_expr()?);
                    }
                    self.expect(&Tok::RParen)?;
                }
                expr = if let Expr::SysCall(name, _) = expr {
                    Expr::SysCall(name, args)
                } else {
                    Expr::Call(Box::new(expr), args)
                };
            } else if self.eat(&Tok::LBrack) {
                let idx = self.parse_expr()?;
                if self.eat(&Tok::DotDotEq) {
                    let end = self.parse_expr()?;
                    self.expect(&Tok::RBrack)?;
                    expr = Expr::Slice(
                        Box::new(expr),
                        Range { start: Box::new(idx), end: Box::new(end), inclusive: true },
                    );
                } else if self.eat(&Tok::DotDot) {
                    let end = self.parse_expr()?;
                    self.expect(&Tok::RBrack)?;
                    expr = Expr::Slice(
                        Box::new(expr),
                        Range { start: Box::new(idx), end: Box::new(end), inclusive: false },
                    );
                } else {
                    self.expect(&Tok::RBrack)?;
                    expr = Expr::Index(Box::new(expr), Box::new(idx));
                }
            } else if self.eat(&Tok::Dot) {
                // Tuple index `t.0` (SPEC §6.1) or a named field access.
                if let Some(Tok::Int(n)) = self.peek() {
                    let index = n.to_string();
                    self.pos += 1;
                    expr = Expr::Field(Box::new(expr), index);
                } else {
                    let field = self.parse_ident()?;
                    expr = Expr::Field(Box::new(expr), field);
                }
            } else if self.eat(&Tok::DoubleColon) {
                let seg = self.parse_ident()?;
                expr = match expr {
                    Expr::Ident(id) => Expr::Path(Path { segments: vec![id, seg] }),
                    Expr::Path(mut path) => {
                        path.segments.push(seg);
                        Expr::Path(path)
                    }
                    _ => return Err("Unexpected `::` after non-path expression".into()),
                };
            } else {
                break;
            }
        }

        // Map literal: `Map { k: v, ... }` or `Map {}` (piperine-bench/docs/SPEC.md §5.1).
        // Disambiguated from a bundle literal by the `Map` type name and the
        // `k: v` (colon) entry syntax.
        if let Expr::Ident(name) = &expr
            && name == "Map" && self.peek() == Some(&Tok::LBrace) {
                self.eat(&Tok::LBrace);
                let mut entries = Vec::new();
                if !self.eat(&Tok::RBrace) {
                    let k = self.parse_expr()?;
                    self.expect(&Tok::Colon)?;
                    let v = self.parse_expr()?;
                    entries.push((k, v));
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::RBrace) {
                            break;
                        }
                        let k = self.parse_expr()?;
                        self.expect(&Tok::Colon)?;
                        let v = self.parse_expr()?;
                        entries.push((k, v));
                    }
                    self.expect(&Tok::RBrace)?;
                }
                expr = Expr::MapLit(entries);
            }

        // Set literal: `Set { a, b, c }` or `Set {}` (empty set).
        // Disambiguated from a bundle literal by the `Set` type name and the
        // absence of `.field = ...` syntax (bundle lits require dotted field
        // inits; Set items are bare expressions).
        if let Expr::Ident(name) = &expr
            && name == "Set" && self.peek() == Some(&Tok::LBrace) {
                self.eat(&Tok::LBrace);
                let mut items = Vec::new();
                if !self.eat(&Tok::RBrace) {
                    items.push(self.parse_expr()?);
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::RBrace) {
                            break;
                        }
                        items.push(self.parse_expr()?);
                    }
                    self.expect(&Tok::RBrace)?;
                }
                expr = Expr::SetLit(items);
            }

        // BundleLit: `TypeRef { .field = expr, ... }` — look-ahead on `{ .` or `{ }`.
        if self.peek() == Some(&Tok::LBrace)
            && (self.peek_at(1) == Some(&Tok::Dot) || self.peek_at(1) == Some(&Tok::RBrace)) {
                self.eat(&Tok::LBrace);
                let mut fields = Vec::new();
                if !self.eat(&Tok::RBrace) {
                    self.expect(&Tok::Dot)?;
                    let fname = self.parse_ident()?;
                    self.expect(&Tok::Assign)?;
                    let fexpr = self.parse_expr()?;
                    fields.push((fname, fexpr));
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::RBrace) {
                            break;
                        }
                        self.expect(&Tok::Dot)?;
                        let fname = self.parse_ident()?;
                        self.expect(&Tok::Assign)?;
                        let fexpr = self.parse_expr()?;
                        fields.push((fname, fexpr));
                    }
                    self.expect(&Tok::RBrace)?;
                }
                let mut dims = Vec::new();
                let mut current = &expr;
                while let Expr::Index(inner, dim) = current {
                    dims.push((dim.as_ref()).clone());
                    current = inner;
                }
                dims.reverse();
                let type_name = match current {
                    Expr::Ident(id) => id.clone(),
                    Expr::Path(p) => p.segments.last().unwrap().clone(),
                    _ => return Err("Invalid type in bundle literal".into()),
                };
                expr = Expr::BundleLit { ty: Type { name: type_name, args: vec![], dimensions: dims, optional: false }, fields };
            }

        Ok(expr)
    }

    /// Parses the `[...]` body of an array expression after the leading `[` has been consumed.
    /// Detects whether it is a repeat (`[v; N]`), comprehension (`[expr | i in range]`), or element list.
    pub(crate) fn parse_array_expr(&mut self) -> Result<Expr, crate::parse::error::ParseError> {
        // `[]` — the empty list (bench spec §12.4 `var curve : Vec<…> = [];`).
        if self.eat(&Tok::RBrack) {
            return Ok(Expr::Array(ArrayBody::List(vec![])));
        }
        // Lookahead: detect comprehension `[ expr | var in range ]`.
        let mut is_comp = false;
        let mut brace_depth: i32 = 0;
        let mut paren_depth: i32 = 0;
        let mut brack_depth: i32 = 0;
        for i in self.pos..self.toks.len() {
            match &self.toks[i].tok {
                Tok::LBrace => brace_depth += 1,
                Tok::RBrace => brace_depth -= 1,
                Tok::LParen => paren_depth += 1,
                Tok::RParen => paren_depth -= 1,
                Tok::LBrack => brack_depth += 1,
                Tok::RBrack => {
                    if brack_depth > 0 { brack_depth -= 1; } else { break; }
                }
                Tok::BitOr
                    if brace_depth == 0 && paren_depth == 0 && brack_depth == 0 =>
                {
                    if i + 2 < self.toks.len()
                        && let Tok::Ident(kw) = &self.toks[i + 2].tok
                            && kw == "in" {
                                is_comp = true;
                            }
                    break;
                }
                _ => {}
            }
        }

        let first =
            if is_comp { self.parse_expr_no_bitor()? } else { self.parse_expr()? };

        if self.eat(&Tok::Semi) {
            let n = self.parse_expr()?;
            self.expect(&Tok::RBrack)?;
            Ok(Expr::Array(ArrayBody::Repeat(Box::new(first), Box::new(n))))
        } else if self.eat(&Tok::BitOr) || self.eat_ident("or") {
            let var = self.parse_ident()?;
            self.expect_ident_str("in")?;
            let range = self.parse_range()?;
            self.expect(&Tok::RBrack)?;
            Ok(Expr::Array(ArrayBody::Comprehension(Box::new(first), var, range)))
        } else {
            let mut list = vec![first];
            while self.eat(&Tok::Comma) {
                if self.peek() == Some(&Tok::RBrack) {
                    break;
                }
                list.push(self.parse_expr()?);
            }
            self.expect(&Tok::RBrack)?;
            Ok(Expr::Array(ArrayBody::List(list)))
        }
    }
}

impl Parse for crate::parse::ast::Expr {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        parser.parse_expr()
    }
}

impl Parse for crate::parse::ast::Type {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        parser.parse_type()
    }
}

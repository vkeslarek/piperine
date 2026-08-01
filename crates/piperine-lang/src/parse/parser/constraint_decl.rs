//! `constraint Name { ... }` (dv-core DVC-04) — a third top-level body kind
//! beside `analog`/`digital`, holding `require`/`var`/`target`/`cover`
//! statements.
//!
//! ```text
//! ConstraintDecl ::= "constraint" Ident "{" { ConstraintStmt } "}"
//! ConstraintStmt ::= "require" Ident ":" Expr ";"
//!                  | "var" Ident ":" Type "=" Expr ";"
//!                  | "target" Expr ("<=" | ">=") Expr [ "tol" Expr ] ";"
//!                  | "@" EventSpec "{" { ConstraintStmt } "}"        // DVC-19
//!                  | "cover" Ident ":" Expr "bins" "[" Expr ":" Expr ":" Expr "]" ";"   // T32
//!                  | "cover" Ident ":" Expr "in" "{" Expr { "," Expr } "}" ";"          // T32
//!                  | "cover" "cross" Ident ":" Ident { "," Ident } ";"                  // T32
//! ```
//!
//! The `var` type annotation is mandatory — the parser rejects the bare
//! `var x = expr;` form, exactly like a `mod`-body `var`. Only the
//! non-strict comparisons parse: `<`/`>` have no signed-margin semantics
//! (DVC-08) and fail loud at the offending token. The `@` form reuses the
//! behavior-body `EventSpec` production (dv-core T28, DVC-19); its terms
//! are validated against the `EventRegistry` at elaboration. `cover`
//! mirrors `require`'s own `<name> : <expr>` shape with a bin-partition
//! suffix (dv-tester T32, DVT-42) — `cover cross` instead names two or
//! more already-declared `cover` points by their labels; the elaborator
//! checks the joint (product) bin count against a raisable cap (D10),
//! since bin edges are literal and the product is therefore known
//! statically, before anything allocates.

use crate::parse::ast::*;
use crate::parse::lexer::Tok;
use super::attributes::ParseAttributesExt;
use super::{Parse, Parser};

impl<'a> Parser<'a> {
    /// Parses one statement inside a `constraint` block.
    fn parse_constraint_stmt(&mut self) -> Result<ConstraintStmt, crate::parse::error::ParseError> {
        let start = self.current_span_start();
        if self.eat_ident("require") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let pred = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ConstraintStmt::Require { span: Some((start, end - start).into()), name, pred });
        }
        if self.eat_ident("var") {
            let name = self.parse_ident()?;
            // The type annotation is mandatory — `expect` fails loud at the
            // offending token when it is missing.
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            self.expect(&Tok::Assign)?;
            let value = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ConstraintStmt::Var { span: Some((start, end - start).into()), name, ty, value });
        }
        if self.eat_ident("target") {
            // The operands parse at minimum precedence 7 (BitXor and
            // above) so a following `<=`/`>=` is the statement's
            // comparison, not a binary operator inside the operand. A
            // target's operands are `Real` quantities (DVC-04) —
            // comparisons and logic bind too loosely to belong inside one.
            let expr = self.parse_binary_expr(7, false)?;
            let cmp = if self.eat(&Tok::Le) {
                TargetCmp::Le
            } else if self.eat(&Tok::Ge) {
                TargetCmp::Ge
            } else {
                let peeked = self.peek().cloned();
                return Err(self.make_error(format!(
                    "expected `<=` or `>=` in `target` comparison, found {peeked:?} \
                     — the strict forms have no signed-margin semantics"
                )));
            };
            let level = self.parse_binary_expr(7, false)?;
            let tol = if self.eat_ident("tol") { Some(self.parse_binary_expr(7, false)?) } else { None };
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ConstraintStmt::Target { span: Some((start, end - start).into()), expr, cmp, level, tol });
        }
        // `cover <name> : <expr> bins [lo:step:hi];` / `… in {…};`, or
        // `cover cross <name> : <point1>, <point2>, …;` (dv-tester T32,
        // DVT-42). Mirrors `require`'s own shape (`<name> : <expr>`) with
        // a bin-partition suffix instead of a bare predicate.
        if self.eat_ident("cover") {
            if self.eat_ident("cross") {
                let name = self.parse_ident()?;
                self.expect(&Tok::Colon)?;
                let mut points = vec![self.parse_ident()?];
                while self.eat(&Tok::Comma) {
                    points.push(self.parse_ident()?);
                }
                self.expect(&Tok::Semi)?;
                let end = self.previous_span_end();
                return Ok(ConstraintStmt::CoverCross {
                    span: Some((start, end - start).into()),
                    name,
                    points,
                });
            }
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let expr = self.parse_binary_expr(7, false)?;
            let bins = if self.eat_ident("bins") {
                self.expect(&Tok::LBrack)?;
                let lo = self.parse_binary_expr(7, false)?;
                self.expect(&Tok::Colon)?;
                let step = self.parse_binary_expr(7, false)?;
                self.expect(&Tok::Colon)?;
                let hi = self.parse_binary_expr(7, false)?;
                self.expect(&Tok::RBrack)?;
                CoverBins::Range { lo, step, hi: Box::new(hi) }
            } else if self.eat_ident("in") {
                self.expect(&Tok::LBrace)?;
                let mut values = vec![self.parse_binary_expr(7, false)?];
                while self.eat(&Tok::Comma) {
                    values.push(self.parse_binary_expr(7, false)?);
                }
                self.expect(&Tok::RBrace)?;
                CoverBins::Set(values)
            } else {
                let peeked = self.peek().cloned();
                return Err(self.make_error(format!(
                    "expected `bins [lo:step:hi]` or `in {{…}}` after a `cover` point's expression, found {peeked:?}"
                )));
            };
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ConstraintStmt::Cover { span: Some((start, end - start).into()), name, expr, bins });
        }
        // Analysis-scoped event block (dv-core T28, DVC-19): `@ dc { … }`,
        // `@ (dc | tran) { … }`. Reuses the behavior-body `EventSpec`
        // production — the elaborator resolves the terms against the
        // `EventRegistry` and rejects anything that is not an
        // analysis-scope kind.
        if self.eat(&Tok::At) {
            let spec = self.parse_event_spec()?;
            self.expect(&Tok::LBrace)?;
            let mut body = Vec::new();
            while !self.eat(&Tok::RBrace) {
                body.push(self.parse_constraint_stmt()?);
            }
            let end = self.previous_span_end();
            return Ok(ConstraintStmt::Event { span: Some((start, end - start).into()), spec, body });
        }
        let peeked = self.peek().cloned();
        Err(self.make_error(format!(
            "expected `require`, `var`, `target`, or `@` in a constraint block, found {peeked:?}"
        )))
    }
}

impl Parse for ConstraintDecl {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let start = parser.current_span_start();
        let doc = parser.current_doc();
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("constraint")?;
        let name = parser.parse_ident()?;
        parser.expect(&Tok::LBrace)?;
        let mut body = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            body.push(parser.parse_constraint_stmt()?);
        }
        let end = parser.previous_span_end();
        Ok(ConstraintDecl { span: Some((start, end - start).into()), attrs, doc, is_pub, name, body })
    }
}

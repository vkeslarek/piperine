//! `constraint Name { ... }` (dv-core DVC-04) — a third top-level body kind
//! beside `analog`/`digital`, holding `require`/`var`/`target` statements.
//!
//! ```text
//! ConstraintDecl ::= "constraint" Ident "{" { ConstraintStmt } "}"
//! ConstraintStmt ::= "require" Ident ":" Expr ";"
//!                  | "var" Ident ":" Type "=" Expr ";"
//!                  | "target" Expr ("<=" | ">=") Expr [ "tol" Expr ] ";"
//!                  | "@" EventSpec "{" { ConstraintStmt } "}"        // DVC-19
//! ```
//!
//! The `var` type annotation is mandatory — the parser rejects the bare
//! `var x = expr;` form, exactly like a `mod`-body `var`. Only the
//! non-strict comparisons parse: `<`/`>` have no signed-margin semantics
//! (DVC-08) and fail loud at the offending token. The `@` form reuses the
//! behavior-body `EventSpec` production (dv-core T28, DVC-19); its terms
//! are validated against the `EventRegistry` at elaboration.

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

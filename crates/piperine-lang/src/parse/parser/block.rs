use crate::parse::ast::*;
use crate::parse::lexer::Tok;
use super::Parser;

impl<'a> Parser<'a> {
    // ─────────────────────────── Block ───────────────────────────────────────
    //
    // Block ::= "{" { Stmt } [ Expr ] "}"
    // The trailing Expr (no semicolon) is the block's value.

    /// Parses a block `{ stmts... [trailing_expr] }` with statements and an optional trailing expression.
    pub(crate) fn parse_block(&mut self) -> Result<Block, crate::parse::error::ParseError> {
        self.expect(&Tok::LBrace)?;
        let mut stmts = Vec::new();
        while !self.eat(&Tok::RBrace) {
            if self.eat_ident("return") {
                let expr = self.parse_expr()?;
                self.expect(&Tok::Semi)?;
                stmts.push(Stmt::Return(expr));
            } else if self.eat_ident("if") {
                self.expect(&Tok::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&Tok::RParen)?;
                let then_body = self.parse_block()?;
                let else_body = if self.eat_ident("else") {
                    if self.eat_ident("if") {
                        self.pos -= 1;
                        let if_stmt = self.parse_stmt()?;
                        Some(Block { stmts: vec![if_stmt], expr: None })
                    } else {
                        Some(self.parse_block()?)
                    }
                } else {
                    None
                };
                stmts.push(Stmt::If { cond, then_body, else_body });
            } else if self.eat_ident("match") {
                let expr = self.parse_expr()?;
                self.expect(&Tok::LBrace)?;
                let mut arms = Vec::new();
                while !self.eat(&Tok::RBrace) {
                    let pat = self.parse_pattern()?;
                    self.expect(&Tok::FatArrow)?;
                    let body = self.parse_block()?;
                    self.eat(&Tok::Comma);
                    arms.push(StmtMatchArm { pat, body });
                }
                stmts.push(Stmt::Match { expr, arms });
            } else if self.eat_ident("for") {
                let var = self.parse_ident()?;
                self.expect_ident_str("in")?;
                let range = self.parse_range()?;
                let body = self.parse_block()?;
                stmts.push(Stmt::For { var, range, body });
            } else if self.eat_ident("var") {
                let name = self.parse_ident()?;
                self.expect(&Tok::Colon)?;
                let ty = self.parse_type()?;
                let default =
                    if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
                self.expect(&Tok::Semi)?;
                stmts.push(Stmt::VarDecl { name, ty, default });
            } else {
                let expr = self.parse_expr()?;
                if self.eat(&Tok::Contrib) {
                    let src = self.parse_expr()?;
                    self.expect(&Tok::Semi)?;
                    stmts.push(Stmt::Bind { dest: expr, op: BindOp::Contrib, src });
                } else if self.eat(&Tok::Force) {
                    let src = self.parse_expr()?;
                    self.expect(&Tok::Semi)?;
                    stmts.push(Stmt::Bind { dest: expr, op: BindOp::Force, src });
                } else if self.eat(&Tok::Assign) {
                    let src = self.parse_expr()?;
                    self.expect(&Tok::Semi)?;
                    stmts.push(Stmt::Bind { dest: expr, op: BindOp::Assign, src });
                } else if self.eat(&Tok::Semi) {
                    stmts.push(Stmt::Expr(expr));
                } else {
                    // Trailing expression — block value.
                    self.expect(&Tok::RBrace)?;
                    return Ok(Block { stmts, expr: Some(Box::new(expr)) });
                }
            }
        }
        Ok(Block { stmts, expr: None })
    }
}

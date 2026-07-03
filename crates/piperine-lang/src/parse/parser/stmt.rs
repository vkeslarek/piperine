//! Statement grammar: `mod`-body statements, `analog`/`digital` behavior
//! statements, function-body statements, events, ranges, and patterns.

use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;

use super::{Parse, Parser};

impl<'a> Parser<'a> {

    // ─────────────────────────── §3.2  Instances and connections ─────────────
    //
    // Grammar (left-factored on leading Ident):
    //   InstanceOrConnect ::= Ident { Indexer | Field } InstTail
    //   InstTail ::= ":" ModuleRef PortArgs [ParamArgs] ";"   -- named instance
    //              | ConstArgs PortArgs [ParamArgs] ";"        -- anon + const
    //              | PortArgs [ParamArgs] ";"                  -- anon
    //              | "=" Expr ";"                              -- connection

    /// Parses a single statement inside a `mod` body: `param`, `wire`, `var`, `for`, `if`, instance, or connection.
    pub(crate) fn parse_mod_stmt(&mut self) -> Result<ModuleStatement, crate::parse::error::ParseError> {
        let start = self.current_span_start();
        let attrs = self.parse_attributes()?;
        if matches!(self.peek(), Some(Tok::SysCall(name)) if name == "assert") {
            self.pos += 1;
            self.expect(&Tok::LParen)?;
            let cond = self.parse_expr()?;
            self.expect(&Tok::Comma)?;
            let msg = self.parse_expr()?;
            self.expect(&Tok::RParen)?;
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ModuleStatement::Assert { span: Some((start, end - start).into()), attrs, cond, msg });
        }
        if self.eat_ident("param") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ModuleStatement::ParamDecl { span: Some((start, end - start).into()), attrs, name, ty, default });
        }
        if self.eat_ident("wire") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ModuleStatement::WireDecl { span: Some((start, end - start).into()), attrs, name, ty });
        }
        if self.eat_ident("var") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ModuleStatement::VarDecl { span: Some((start, end - start).into()), attrs, name, ty, default });
        }
        if self.eat_ident("for") {
            let var = self.parse_ident()?;
            self.expect_ident_str("in")?;
            let range = self.parse_range()?;
            self.expect(&Tok::LBrace)?;
            let mut body = Vec::new();
            while !self.eat(&Tok::RBrace) {
                body.push(self.parse_mod_stmt()?);
            }
            let end = self.previous_span_end();
            return Ok(ModuleStatement::StructuralFor { span: Some((start, end - start).into()), attrs, var, range, body });
        }
        if self.eat_ident("if") {
            self.expect(&Tok::LParen)?;
            let cond = self.parse_expr()?;
            self.expect(&Tok::RParen)?;
            self.expect(&Tok::LBrace)?;
            let mut then_body = Vec::new();
            while !self.eat(&Tok::RBrace) {
                then_body.push(self.parse_mod_stmt()?);
            }
            let else_body = if self.eat_ident("else") {
                let mut e_body = Vec::new();
                if self.eat_ident("if") {
                    self.pos -= 1;
                    e_body.push(self.parse_mod_stmt()?);
                } else {
                    self.expect(&Tok::LBrace)?;
                    while !self.eat(&Tok::RBrace) {
                        e_body.push(self.parse_mod_stmt()?);
                    }
                }
                Some(e_body)
            } else {
                None
            };
            let end = self.previous_span_end();
            return Ok(ModuleStatement::StructuralIf { span: Some((start, end - start).into()), attrs, cond, then_body, else_body });
        }

        // InstanceOrConnect — leading Ident, then branch on next token.
        let name = self.parse_ident()?;
        let mut module_name = name.clone();
        let mut is_named_instance = false;

        let mut array_index: Option<Expr> = None;
        if self.eat(&Tok::LBrack) {
            array_index = Some(self.parse_expr()?);
            self.expect(&Tok::RBrack)?;
        }

        if self.eat(&Tok::Assign) {
            let mut lhs = Expr::Ident(name);
            if let Some(idx) = array_index {
                lhs = Expr::Index(Box::new(lhs), Box::new(idx));
            }
            let rhs = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            let end = self.previous_span_end();
            return Ok(ModuleStatement::Connection { span: Some((start, end - start).into()), attrs, lhs, rhs });
        }

        if self.eat(&Tok::Colon) {
            is_named_instance = true;
            module_name = self.parse_ident()?;
        }

        let mut const_args = Vec::new();
        if is_named_instance {
            if self.eat(&Tok::LBrack) {
                const_args.push(self.parse_expr()?);
                while self.eat(&Tok::Comma) {
                    const_args.push(self.parse_expr()?);
                }
                self.expect(&Tok::RBrack)?;
            }
        } else {
            if let Some(idx) = array_index.take() {
                const_args.push(idx);
            }
            if self.eat(&Tok::LBrack) {
                const_args.push(self.parse_expr()?);
                while self.eat(&Tok::Comma) {
                    const_args.push(self.parse_expr()?);
                }
                self.expect(&Tok::RBrack)?;
            }
        }

        let mut type_args = Vec::new();
        if self.eat(&Tok::Lt) {
            type_args.push(self.parse_type()?);
            while self.eat(&Tok::Comma) {
                type_args.push(self.parse_type()?);
            }
            self.expect(&Tok::Gt)?;
        }

        let mut ports = Vec::new();
        if self.eat(&Tok::LParen) {
            if !self.eat(&Tok::RParen) {
                ports.push(self.parse_port_conn()?);
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    ports.push(self.parse_port_conn()?);
                }
                self.expect(&Tok::RParen)?;
            }
        }

        let mut params = Vec::new();
        if self.eat(&Tok::LBrace) {
            if !self.eat(&Tok::RBrace) {
                self.expect(&Tok::Dot)?;
                let pname = self.parse_ident()?;
                self.expect(&Tok::Assign)?;
                let pexpr = self.parse_expr()?;
                params.push(ParamArg { name: pname, expr: pexpr });
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RBrace) {
                        break;
                    }
                    self.expect(&Tok::Dot)?;
                    let pname = self.parse_ident()?;
                    self.expect(&Tok::Assign)?;
                    let pexpr = self.parse_expr()?;
                    params.push(ParamArg { name: pname, expr: pexpr });
                }
                self.expect(&Tok::RBrace)?;
            }
        }

        self.expect(&Tok::Semi)?;
        let end = self.previous_span_end();
        Ok(ModuleStatement::Instance {
            span: Some((start, end - start).into()),
            attrs,
            name: if is_named_instance { Some(name) } else { None },
            array_index,
            module: module_name,
            const_args,
            type_args,
            ports,
            params,
        })
    }

    // ─────────────────────────── §7  Behavior ────────────────────────────────

    /// Parses an `analog Name { ... }` or `digital Name { ... }` behavior block.
    pub(crate) fn parse_behavior(&mut self, attrs: Vec<Attribute>, is_pub: bool, kind: BehaviorKind) -> Result<BehaviorDecl, crate::parse::error::ParseError> {
        let name = self.parse_ident()?;
        self.expect(&Tok::LBrace)?;
        let mut body = Vec::new();
        while !self.eat(&Tok::RBrace) {
            body.push(self.parse_behavior_stmt()?);
        }
        Ok(BehaviorDecl { attrs, is_pub, kind, name, body })
    }

    /// Parses a single statement inside an `analog`/`digital` behavior block.
    pub(crate) fn parse_behavior_stmt(&mut self) -> Result<BehaviorStmt, crate::parse::error::ParseError> {
        if self.eat_ident("var") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            self.expect(&Tok::Semi)?;
            return Ok(BehaviorStmt::VarDecl { name, ty, default });
        }
        if self.eat_ident("if") {
            self.expect(&Tok::LParen)?;
            let cond = self.parse_expr()?;
            self.expect(&Tok::RParen)?;
            self.expect(&Tok::LBrace)?;
            let mut then_body = Vec::new();
            while !self.eat(&Tok::RBrace) {
                then_body.push(self.parse_behavior_stmt()?);
            }
            let else_body = if self.eat_ident("else") {
                let mut e_body = Vec::new();
                if self.eat_ident("if") {
                    self.pos -= 1;
                    e_body.push(self.parse_behavior_stmt()?);
                } else {
                    self.expect(&Tok::LBrace)?;
                    while !self.eat(&Tok::RBrace) {
                        e_body.push(self.parse_behavior_stmt()?);
                    }
                }
                Some(e_body)
            } else {
                None
            };
            return Ok(BehaviorStmt::If { cond, then_body, else_body });
        }
        if self.eat_ident("match") {
            let expr = self.parse_expr()?;
            self.expect(&Tok::LBrace)?;
            let mut arms = Vec::new();
            while !self.eat(&Tok::RBrace) {
                let pat = self.parse_pattern()?;
                self.expect(&Tok::FatArrow)?;
                let mut body = Vec::new();
                if self.eat(&Tok::LBrace) {
                    while !self.eat(&Tok::RBrace) {
                        body.push(self.parse_behavior_stmt()?);
                    }
                } else {
                    body.push(self.parse_behavior_stmt()?);
                }
                self.eat(&Tok::Comma);
                arms.push(MatchArm { pat, body });
            }
            return Ok(BehaviorStmt::Match { expr, arms });
        }
        if self.eat_ident("for") {
            let var = self.parse_ident()?;
            self.expect_ident_str("in")?;
            let range = self.parse_range()?;
            self.expect(&Tok::LBrace)?;
            let mut body = Vec::new();
            while !self.eat(&Tok::RBrace) {
                body.push(self.parse_behavior_stmt()?);
            }
            return Ok(BehaviorStmt::For { var, range, body });
        }
        if self.eat(&Tok::At) {
            let spec = self.parse_event_spec()?;
            let guard = if self.eat_ident("when") {
                self.expect(&Tok::LParen)?;
                let e = self.parse_expr()?;
                self.expect(&Tok::RParen)?;
                Some(e)
            } else {
                None
            };
            let body = self.parse_block()?;
            return Ok(BehaviorStmt::Event { spec, guard, body });
        }
        if let Some(Tok::SysCall(sys)) = self.peek() {
            let sys = sys.clone();
            self.pos += 1;
            self.expect(&Tok::LParen)?;
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
            self.expect(&Tok::Semi)?;
            return Ok(BehaviorStmt::Diagnostic { sys, args });
        }

        let expr = self.parse_expr()?;
        if self.eat(&Tok::Contrib) {
            let src = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            Ok(BehaviorStmt::Bind { dest: expr, op: BindOp::Contrib, src })
        } else if self.eat(&Tok::Force) {
            let src = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            Ok(BehaviorStmt::Bind { dest: expr, op: BindOp::Force, src })
        } else if self.eat(&Tok::Assign) {
            let src = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            Ok(BehaviorStmt::Bind { dest: expr, op: BindOp::Assign, src })
        } else {
            self.expect(&Tok::Semi)?;
            Ok(BehaviorStmt::Expr(expr))
        }
    }

    // ─────────────────────────── §7.2  Events ────────────────────────────────
    //
    // EventSpec is open: any identifier becomes `Named { name, arg }`.
    // The elaborator resolves it against the EventRegistry.

    /// Parses an event specification: a bare event term, or a parenthesized `|`-separated list of event terms.
    pub(crate) fn parse_event_spec(&mut self) -> Result<EventSpec, crate::parse::error::ParseError> {
        if self.eat(&Tok::LParen) {
            let spec = self.parse_event_term()?;
            let mut specs = vec![spec];
            while self.eat(&Tok::BitOr) || self.eat_ident("or") {
                specs.push(self.parse_event_term()?);
            }
            self.expect(&Tok::RParen)?;
            Ok(EventSpec::Or(specs))
        } else {
            self.parse_event_term()
        }
    }

    /// Parses a single event term: `initial`, `final`, or `name(arg)`.
    pub(crate) fn parse_event_term(&mut self) -> Result<EventSpec, crate::parse::error::ParseError> {
        if self.eat_ident("initial") {
            return Ok(EventSpec::Initial);
        }
        if self.eat_ident("final") {
            return Ok(EventSpec::Final);
        }
        let name = self.parse_ident()?;
        self.expect(&Tok::LParen)?;
        let arg = self.parse_expr()?;
        self.expect(&Tok::RParen)?;
        Ok(EventSpec::Named { name, arg })
    }

    // ─────────────────────────── §8.1  Statements ────────────────────────────

    /// Parses a general-purpose statement inside a function body: `var`, `if`, `match`, `for`, `return`, bind, or expression.
    pub(crate) fn parse_stmt(&mut self) -> Result<Stmt, crate::parse::error::ParseError> {
        if self.eat_ident("var") {
            let name = self.parse_ident()?;
            let ty = if self.eat(&Tok::Colon) { Some(self.parse_type()?) } else { None };
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            self.expect(&Tok::Semi)?;
            return Ok(Stmt::VarDecl { name, ty, default });
        }
        if self.eat_ident("if") {
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
            return Ok(Stmt::If { cond, then_body, else_body });
        }
        if self.eat_ident("match") {
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
            return Ok(Stmt::Match { expr, arms });
        }
        if self.eat_ident("for") {
            let var = self.parse_ident()?;
            self.expect_ident_str("in")?;
            let iter = self.parse_for_iter()?;
            let body = self.parse_block()?;
            return Ok(Stmt::For { var, iter, body });
        }
        if self.eat_ident("return") {
            let expr = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            return Ok(Stmt::Return(expr));
        }

        let expr = self.parse_expr()?;
        if self.eat(&Tok::Contrib) {
            let src = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            Ok(Stmt::Bind { dest: expr, op: BindOp::Contrib, src })
        } else if self.eat(&Tok::Force) {
            let src = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            Ok(Stmt::Bind { dest: expr, op: BindOp::Force, src })
        } else if self.eat(&Tok::Assign) {
            let src = self.parse_expr()?;
            self.expect(&Tok::Semi)?;
            Ok(Stmt::Bind { dest: expr, op: BindOp::Assign, src })
        } else {
            self.expect(&Tok::Semi)?;
            Ok(Stmt::Expr(expr))
        }
    }

    /// Parses a range: `start .. end` or `start ..= end`.
    pub(crate) fn parse_range(&mut self) -> Result<Range, crate::parse::error::ParseError> {
        let start = self.parse_expr()?;
        let inclusive = if self.eat(&Tok::DotDotEq) {
            true
        } else if self.eat(&Tok::DotDot) {
            false
        } else {
            return Err("Expected `..` or `..=`".into());
        };
        let end = self.parse_expr()?;
        Ok(Range { start: Box::new(start), end: Box::new(end), inclusive })
    }

    /// Parses a fn-body `for`'s iterable: `start..end`/`start..=end` (a
    /// [`Range`]), or any other expression (a runtime list value —
    /// SPEC Part I §9).
    pub(crate) fn parse_for_iter(&mut self) -> Result<ForIter, crate::parse::error::ParseError> {
        let start = self.parse_expr()?;
        let inclusive = if self.eat(&Tok::DotDotEq) {
            true
        } else if self.eat(&Tok::DotDot) {
            false
        } else {
            return Ok(ForIter::Expr(start));
        };
        let end = self.parse_expr()?;
        Ok(ForIter::Range(Range { start: Box::new(start), end: Box::new(end), inclusive }))
    }

    /// Parses a match pattern: `_` (wildcard) or a path.
    /// One instance port argument: positional `expr` or named `.port = expr`.
    fn parse_port_conn(&mut self) -> Result<PortConnection, crate::parse::error::ParseError> {
        if self.eat(&Tok::Dot) {
            let port = self.parse_ident()?;
            self.expect(&Tok::Assign)?;
            let expr = self.parse_expr()?;
            return Ok(PortConnection::Named { port, expr });
        }
        Ok(self.parse_expr().map(PortConnection::Positional)?)
    }

    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, crate::parse::error::ParseError> {
        if self.eat_ident("_") {
            return Ok(Pattern::Wildcard);
        }
        match self.peek().cloned() {
            Some(Tok::BitPattern(bits)) => {
                self.pos += 1;
                Ok(Pattern::BitPattern(bits))
            }
            Some(Tok::Int(v)) => {
                self.pos += 1;
                Ok(Pattern::Literal(v))
            }
            _ => Ok(Pattern::Path(self.parse_path()?)),
        }
    }

}

impl Parse for ModuleStatement {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        parser.parse_mod_stmt()
    }
}

impl Parse for BehaviorDecl {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let start = parser.current_span_start();
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        let kind = if parser.eat_ident("analog") { BehaviorKind::Analog } else if parser.eat_ident("digital") { BehaviorKind::Digital } else { return Err("Expected analog or digital".into()); };
        parser.parse_behavior(attrs, is_pub, kind)
    }
}

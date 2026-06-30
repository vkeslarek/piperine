//! # Parser
//!
//! A hand-written recursive-descent LL(1) parser that converts a token
//! sequence into a [`SourceFile`] AST.
//!
//! ## Phase contract
//!
//! **Input**: `&[Lexed]` — the output of [`Lexer::tokenize`].
//! **Output**: [`SourceFile`] — the root of the parse AST.
//!
//! ## What the parser does NOT check
//!
//! - **Name resolution**: type names, module names, capability names are
//!   stored as plain `String`s.
//! - **Semantic validity**: `<+` in a `mod` body, `cross` in a `digital`
//!   block, unmatched port counts — all deferred to elaboration.
//! - **Const-evaluability**: array dimensions may be arbitrary expressions.
//!
//! ## Grammar coverage
//!
//! Mirrors the PHDL grammar specification (§2–§8). Left-factoring notes are
//! inline at each non-terminal. The grammar is LL(1) — every choice is
//! resolved by one token of lookahead.

use super::ast::*;
use super::lexer::{Lexed, Tok};

pub struct Parser<'a> {
    toks: &'a [Lexed],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(toks: &'a [Lexed]) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|l| &l.tok)
    }

    fn peek_at(&self, offset: usize) -> Option<&Tok> {
        self.toks.get(self.pos + offset).map(|l| &l.tok)
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_ident(&mut self, expected: &str) -> bool {
        match self.peek() {
            Some(Tok::Ident(s)) if s == expected => {
                self.pos += 1;
                true
            }
            _ => false,
        }
    }

    fn expect(&mut self, tok: &Tok) -> Result<(), String> {
        if self.eat(tok) {
            Ok(())
        } else {
            Err(format!("Expected {:?}, found {:?}", tok, self.peek()))
        }
    }

    fn expect_ident_str(&mut self, expected: &str) -> Result<(), String> {
        if self.eat_ident(expected) {
            Ok(())
        } else {
            Err(format!("Expected `{}`, found {:?}", expected, self.peek()))
        }
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let res = s.clone();
                self.pos += 1;
                Ok(res)
            }
            _ => Err(format!("Expected identifier, found {:?}", self.peek())),
        }
    }

    // ─────────────────────────── §2  Compilation unit ────────────────────────

    pub fn parse_file(&mut self) -> Result<SourceFile, String> {
        let mut items = Vec::new();
        while self.pos < self.toks.len() {
            if self.eat_ident("use") {
                let path = self.parse_path()?;
                self.expect(&Tok::Semi)?;
                items.push(Item::UseDecl(path));
            } else {
                let is_pub = self.eat_ident("pub");
                if self.eat_ident("mod") {
                    items.push(Item::ModDecl(self.parse_mod_decl(is_pub)?));
                } else if self.eat_ident("analog") {
                    items.push(Item::BehaviorDecl(
                        self.parse_behavior(is_pub, BehaviorKind::Analog)?,
                    ));
                } else if self.eat_ident("digital") {
                    items.push(Item::BehaviorDecl(
                        self.parse_behavior(is_pub, BehaviorKind::Digital)?,
                    ));
                } else if self.eat_ident("discipline") {
                    items.push(Item::DisciplineDecl(self.parse_discipline(is_pub)?));
                } else if self.eat_ident("bundle") {
                    items.push(Item::BundleDecl(self.parse_bundle(is_pub)?));
                } else if self.eat_ident("enum") {
                    items.push(Item::EnumDecl(self.parse_enum(is_pub)?));
                } else if self.eat_ident("capability") {
                    items.push(Item::CapabilityDecl(self.parse_capability(is_pub)?));
                } else if self.eat_ident("impl") {
                    items.push(Item::ImplDecl(self.parse_impl(is_pub)?));
                } else if self.eat_ident("fn") {
                    items.push(Item::FnDecl(self.parse_fn_decl(is_pub)?));
                } else {
                    return Err(format!("Unknown top-level item at {:?}", self.peek()));
                }
            }
        }
        Ok(SourceFile { items })
    }

    fn parse_path(&mut self) -> Result<Path, String> {
        let mut segments = vec![self.parse_ident()?];
        while self.eat(&Tok::DoubleColon) {
            segments.push(self.parse_ident()?);
        }
        Ok(Path { segments })
    }

    // ─────────────────────────── §3  Modules ─────────────────────────────────

    fn parse_mod_decl(&mut self, is_pub: bool) -> Result<ModDecl, String> {
        let name = self.parse_ident()?;

        let mut const_params = Vec::new();
        if self.eat(&Tok::LBrack) {
            const_params.push(self.parse_ident()?);
            while self.eat(&Tok::Comma) {
                const_params.push(self.parse_ident()?);
            }
            self.expect(&Tok::RBrack)?;
        }

        let mut type_params = Vec::new();
        if self.eat(&Tok::Lt) {
            type_params.push(self.parse_type_param()?);
            while self.eat(&Tok::Comma) {
                type_params.push(self.parse_type_param()?);
            }
            self.expect(&Tok::Gt)?;
        }

        let mut ports = Vec::new();
        if self.eat(&Tok::LParen) {
            if !self.eat(&Tok::RParen) {
                ports.push(self.parse_port()?);
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    ports.push(self.parse_port()?);
                }
                self.expect(&Tok::RParen)?;
            }
        }

        let mut body = Vec::new();
        if self.eat(&Tok::LBrace) {
            while !self.eat(&Tok::RBrace) {
                body.push(self.parse_mod_stmt()?);
            }
        } else {
            self.expect(&Tok::Semi)?;
        }

        Ok(ModDecl { is_pub, name, const_params, type_params, ports, body })
    }

    fn parse_type_param(&mut self) -> Result<TypeParam, String> {
        let name = self.parse_ident()?;
        let mut bounds = Vec::new();
        if self.eat(&Tok::Colon) {
            bounds.push(self.parse_ident()?);
            while self.eat(&Tok::Plus) {
                bounds.push(self.parse_ident()?);
            }
        }
        Ok(TypeParam { name, bounds })
    }

    fn parse_port(&mut self) -> Result<Port, String> {
        let direction = if self.eat_ident("input") {
            Direction::Input
        } else if self.eat_ident("output") {
            Direction::Output
        } else if self.eat_ident("inout") {
            Direction::Inout
        } else {
            return Err("Expected port direction (input/output/inout)".into());
        };
        let name = self.parse_ident()?;
        self.expect(&Tok::Colon)?;
        let ty = self.parse_type()?;
        Ok(Port { direction, name, ty })
    }

    // ─────────────────────────── §4  Types ───────────────────────────────────

    fn parse_type(&mut self) -> Result<Type, String> {
        let name = self.parse_ident()?;
        let mut args = Vec::new();
        let mut dimensions = Vec::new();

        if name == "fn" && self.peek() == Some(&Tok::LParen) {
            // fn(T, U) -> R
            self.eat(&Tok::LParen);
            if !self.eat(&Tok::RParen) {
                args.push(self.parse_type()?);
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    args.push(self.parse_type()?);
                }
                self.expect(&Tok::RParen)?;
            }
            if self.eat(&Tok::Arrow) {
                args.push(self.parse_type()?);
            }
        } else {
            if self.eat(&Tok::Lt) {
                args.push(self.parse_type()?);
                while self.eat(&Tok::Comma) {
                    args.push(self.parse_type()?);
                }
                self.expect(&Tok::Gt)?;
            }
        }

        while self.eat(&Tok::LBrack) {
            dimensions.push(self.parse_expr()?);
            self.expect(&Tok::RBrack)?;
        }

        Ok(Type { name, args, dimensions })
    }

    // ─────────────────────────── §3.2  Instances and connections ─────────────
    //
    // Grammar (left-factored on leading Ident):
    //   InstanceOrConnect ::= Ident { Indexer | Field } InstTail
    //   InstTail ::= ":" ModuleRef PortArgs [ParamArgs] ";"   -- named instance
    //              | ConstArgs PortArgs [ParamArgs] ";"        -- anon + const
    //              | PortArgs [ParamArgs] ";"                  -- anon
    //              | "=" Expr ";"                              -- connection

    fn parse_mod_stmt(&mut self) -> Result<ModStmt, String> {
        if self.eat_ident("param") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            self.expect(&Tok::Semi)?;
            return Ok(ModStmt::ParamDecl { name, ty, default });
        }
        if self.eat_ident("wire") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            self.expect(&Tok::Semi)?;
            return Ok(ModStmt::WireDecl { name, ty });
        }
        if self.eat_ident("var") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            self.expect(&Tok::Semi)?;
            return Ok(ModStmt::VarDecl { name, ty, default });
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
            return Ok(ModStmt::StructuralFor { var, range, body });
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
            return Ok(ModStmt::StructuralIf { cond, then_body, else_body });
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
            return Ok(ModStmt::Connection { lhs, rhs });
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
                ports.push(self.parse_expr()?);
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    ports.push(self.parse_expr()?);
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
        Ok(ModStmt::Instance {
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

    fn parse_behavior(&mut self, is_pub: bool, kind: BehaviorKind) -> Result<BehaviorDecl, String> {
        let name = self.parse_ident()?;
        self.expect(&Tok::LBrace)?;
        let mut body = Vec::new();
        while !self.eat(&Tok::RBrace) {
            body.push(self.parse_behavior_stmt()?);
        }
        Ok(BehaviorDecl { is_pub, kind, name, body })
    }

    fn parse_behavior_stmt(&mut self) -> Result<BehaviorStmt, String> {
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

    fn parse_event_spec(&mut self) -> Result<EventSpec, String> {
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

    fn parse_event_term(&mut self) -> Result<EventSpec, String> {
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

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if self.eat_ident("var") {
            let name = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
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
            let range = self.parse_range()?;
            let body = self.parse_block()?;
            return Ok(Stmt::For { var, range, body });
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

    fn parse_range(&mut self) -> Result<Range, String> {
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

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        if self.eat_ident("_") {
            Ok(Pattern::Wildcard)
        } else {
            Ok(Pattern::Path(self.parse_path()?))
        }
    }

    // ─────────────────────────── §8.2  Expressions ───────────────────────────
    //
    // Operator precedence (lowest to highest):
    //   BitOr(|)=1  BitAnd(&)=2  Eq/Neq=3  Rel<<=>>==4  BitXor(^)=5
    //   Add/Sub=6   Mul/Div/%=7

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_binary_expr(0, false)
    }

    fn parse_expr_no_bitor(&mut self) -> Result<Expr, String> {
        self.parse_binary_expr(0, true)
    }

    fn parse_binary_expr(&mut self, precedence: u8, stop_at_bitor: bool) -> Result<Expr, String> {
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

    fn peek_binary_op(&self) -> Option<(BinaryOp, u8)> {
        match self.peek() {
            Some(Tok::BitOr)  => Some((BinaryOp::BitOr, 1)),
            Some(Tok::BitAnd) => Some((BinaryOp::BitAnd, 2)),
            Some(Tok::EqEq)   => Some((BinaryOp::Eq, 3)),
            Some(Tok::NotEq)  => Some((BinaryOp::Neq, 3)),
            Some(Tok::Lt)     => Some((BinaryOp::Lt, 4)),
            Some(Tok::Le)     => Some((BinaryOp::Le, 4)),
            Some(Tok::Gt)     => Some((BinaryOp::Gt, 4)),
            Some(Tok::Ge)     => Some((BinaryOp::Ge, 4)),
            Some(Tok::BitXor) => Some((BinaryOp::BitXor, 5)),
            Some(Tok::Plus)   => Some((BinaryOp::Add, 6)),
            Some(Tok::Minus)  => Some((BinaryOp::Sub, 6)),
            Some(Tok::Star)   => Some((BinaryOp::Mul, 7)),
            Some(Tok::Slash)  => Some((BinaryOp::Div, 7)),
            Some(Tok::Percent)=> Some((BinaryOp::Rem, 7)),
            _ => None,
        }
    }

    // ─────────────────────────── §8.3  Primaries ─────────────────────────────

    fn parse_primary(&mut self) -> Result<Expr, String> {
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
                    self.pos += 1;
                    self.expect(&Tok::LParen)?;
                    let cond = self.parse_expr()?;
                    self.expect(&Tok::RParen)?;
                    let then_body = self.parse_block()?;
                    self.expect_ident_str("else")?;
                    let else_body = self.parse_block()?;
                    Expr::If { cond: Box::new(cond), then_body, else_body }
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
                let e = self.parse_expr()?;
                self.expect(&Tok::RParen)?;
                e
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
            _ => return Err(format!("Expected expression, found {:?}", self.peek())),
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
                let field = self.parse_ident()?;
                expr = Expr::Field(Box::new(expr), field);
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

        // BundleLit: `TypeRef { .field = expr, ... }` — look-ahead on `{ .` or `{ }`.
        if self.peek() == Some(&Tok::LBrace) {
            if self.peek_at(1) == Some(&Tok::Dot) || self.peek_at(1) == Some(&Tok::RBrace) {
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
                    dims.push((**dim).clone());
                    current = inner;
                }
                dims.reverse();
                let type_name = match current {
                    Expr::Ident(id) => id.clone(),
                    Expr::Path(p) => p.segments.last().unwrap().clone(),
                    _ => return Err("Invalid type in bundle literal".into()),
                };
                expr = Expr::BundleLit { ty: Type { name: type_name, args: vec![], dimensions: dims }, fields };
            }
        }

        Ok(expr)
    }

    // Parses `[...]` array body after the leading `[` has been consumed.
    fn parse_array_expr(&mut self) -> Result<Expr, String> {
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
                    if i + 2 < self.toks.len() {
                        if let Tok::Ident(kw) = &self.toks[i + 2].tok {
                            if kw == "in" {
                                is_comp = true;
                            }
                        }
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

    // ─────────────────────────── §4.1  Disciplines ───────────────────────────

    fn parse_discipline(&mut self, is_pub: bool) -> Result<DisciplineDecl, String> {
        let name = self.parse_ident()?;
        self.expect(&Tok::LBrace)?;
        let mut items = Vec::new();
        while !self.eat(&Tok::RBrace) {
            if self.eat_ident("potential") {
                let n = self.parse_ident()?;
                self.expect(&Tok::Colon)?;
                let ty = self.parse_type()?;
                let attrs = self.parse_attr_list()?;
                self.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Nature {
                    kind: NatureKind::Potential,
                    name: n,
                    ty,
                    attrs,
                });
            } else if self.eat_ident("flow") {
                let n = self.parse_ident()?;
                self.expect(&Tok::Colon)?;
                let ty = self.parse_type()?;
                let attrs = self.parse_attr_list()?;
                self.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Nature { kind: NatureKind::Flow, name: n, ty, attrs });
            } else if self.eat_ident("storage") {
                let ty = self.parse_type()?;
                self.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Storage(ty));
            } else if self.eat_ident("resolve") {
                let r = if self.eat_ident("tri") {
                    ResolveKind::Tri
                } else if self.eat_ident("or") {
                    ResolveKind::Or
                } else if self.eat_ident("and") {
                    ResolveKind::And
                } else {
                    return Err("Unknown resolve kind (expected tri/or/and)".into());
                };
                self.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Resolve(r));
            } else {
                return Err("Unknown discipline item".into());
            }
        }
        Ok(DisciplineDecl { is_pub, name, items })
    }

    fn parse_attr_list(&mut self) -> Result<Vec<Attr>, String> {
        let mut attrs = Vec::new();
        if self.eat(&Tok::LParen) {
            if !self.eat(&Tok::RParen) {
                let aname = self.parse_ident()?;
                self.expect(&Tok::Assign)?;
                let expr = self.parse_expr()?;
                attrs.push(Attr { name: aname, expr });
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    let aname = self.parse_ident()?;
                    self.expect(&Tok::Assign)?;
                    let expr = self.parse_expr()?;
                    attrs.push(Attr { name: aname, expr });
                }
                self.expect(&Tok::RParen)?;
            }
        }
        Ok(attrs)
    }

    // ─────────────────────────── §4.3  Bundles ───────────────────────────────

    fn parse_bundle(&mut self, is_pub: bool) -> Result<BundleDecl, String> {
        let name = self.parse_ident()?;
        let mut const_params = Vec::new();
        if self.eat(&Tok::LBrack) {
            const_params.push(self.parse_ident()?);
            while self.eat(&Tok::Comma) {
                const_params.push(self.parse_ident()?);
            }
            self.expect(&Tok::RBrack)?;
        }
        let mut type_params = Vec::new();
        if self.eat(&Tok::Lt) {
            type_params.push(self.parse_type_param()?);
            while self.eat(&Tok::Comma) {
                type_params.push(self.parse_type_param()?);
            }
            self.expect(&Tok::Gt)?;
        }
        self.expect(&Tok::LBrace)?;
        let mut fields = Vec::new();
        while !self.eat(&Tok::RBrace) {
            let n = self.parse_ident()?;
            self.expect(&Tok::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            fields.push(FieldDecl { name: n, ty, default });
            if !self.eat(&Tok::Comma) {
                self.expect(&Tok::RBrace)?;
                break;
            }
        }
        Ok(BundleDecl { is_pub, name, const_params, type_params, fields })
    }

    // ─────────────────────────── §4.2  Enums ─────────────────────────────────

    fn parse_enum(&mut self, is_pub: bool) -> Result<EnumDecl, String> {
        let name = self.parse_ident()?;
        let repr = if self.eat(&Tok::Colon) { Some(self.parse_type()?) } else { None };
        self.expect(&Tok::LBrace)?;
        let mut variants = Vec::new();
        while !self.eat(&Tok::RBrace) {
            let n = self.parse_ident()?;
            let value = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
            variants.push(EnumVariant { name: n, value });
            if !self.eat(&Tok::Comma) {
                self.expect(&Tok::RBrace)?;
                break;
            }
        }
        Ok(EnumDecl { is_pub, name, repr, variants })
    }

    // ─────────────────────────── §5  Capabilities ────────────────────────────

    fn parse_capability(&mut self, is_pub: bool) -> Result<CapabilityDecl, String> {
        let name = self.parse_ident()?;
        let mut supers = Vec::new();
        if self.eat(&Tok::Colon) {
            supers.push(self.parse_ident()?);
            while self.eat(&Tok::Comma) {
                supers.push(self.parse_ident()?);
            }
        }
        self.expect(&Tok::LBrace)?;
        let mut items = Vec::new();
        while !self.eat(&Tok::RBrace) {
            if self.eat_ident("fn") {
                let sig = self.parse_fn_sig()?;
                if self.eat(&Tok::Semi) {
                    items.push(CapItem::FnSig(sig));
                } else {
                    let body = self.parse_block()?;
                    items.push(CapItem::FnDecl(FnDecl { is_pub: false, sig, body }));
                }
            } else {
                return Err("Expected `fn` inside capability".into());
            }
        }
        Ok(CapabilityDecl { is_pub, name, supers, items })
    }

    fn parse_impl(&mut self, is_pub: bool) -> Result<ImplDecl, String> {
        let mut ident1 = self.parse_ident()?;
        let mut capability = None;
        if self.eat_ident("for") {
            capability = Some(ident1);
            ident1 = self.parse_ident()?;
        }
        let mut const_args = Vec::new();
        if self.eat(&Tok::LBrack) {
            const_args.push(self.parse_expr()?);
            while self.eat(&Tok::Comma) {
                const_args.push(self.parse_expr()?);
            }
            self.expect(&Tok::RBrack)?;
        }
        let mut type_args = Vec::new();
        if self.eat(&Tok::Lt) {
            type_args.push(self.parse_type()?);
            while self.eat(&Tok::Comma) {
                type_args.push(self.parse_type()?);
            }
            self.expect(&Tok::Gt)?;
        }
        self.expect(&Tok::LBrace)?;
        let mut methods = Vec::new();
        while !self.eat(&Tok::RBrace) {
            if self.eat_ident("fn") {
                methods.push(self.parse_fn_decl(false)?);
            } else {
                return Err("Expected `fn` inside impl".into());
            }
        }
        Ok(ImplDecl { is_pub, capability, ty: ident1, const_args, type_args, methods })
    }

    // ─────────────────────────── §6  Functions ───────────────────────────────

    fn parse_fn_sig(&mut self) -> Result<FnSig, String> {
        let name = self.parse_ident()?;
        let mut type_params = Vec::new();
        if self.eat(&Tok::Lt) {
            type_params.push(self.parse_type_param()?);
            while self.eat(&Tok::Comma) {
                type_params.push(self.parse_type_param()?);
            }
            self.expect(&Tok::Gt)?;
        }
        self.expect(&Tok::LParen)?;
        let mut params = Vec::new();
        if !self.eat(&Tok::RParen) {
            if self.eat_ident("self") {
                params.push(FnParam::SelfParam);
            } else {
                let n = self.parse_ident()?;
                self.expect(&Tok::Colon)?;
                let ty = self.parse_type()?;
                params.push(FnParam::Typed(n, ty));
            }
            while self.eat(&Tok::Comma) {
                if self.peek() == Some(&Tok::RParen) {
                    break;
                }
                let n = self.parse_ident()?;
                self.expect(&Tok::Colon)?;
                let ty = self.parse_type()?;
                params.push(FnParam::Typed(n, ty));
            }
            self.expect(&Tok::RParen)?;
        }
        self.expect(&Tok::Arrow)?;
        let ret = self.parse_type()?;
        Ok(FnSig { name, type_params, params, ret })
    }

    fn parse_fn_decl(&mut self, is_pub: bool) -> Result<FnDecl, String> {
        let sig = self.parse_fn_sig()?;
        let body = self.parse_block()?;
        Ok(FnDecl { is_pub, sig, body })
    }

    // ─────────────────────────── Block ───────────────────────────────────────
    //
    // Block ::= "{" { Stmt } [ Expr ] "}"
    // The trailing Expr (no semicolon) is the block's value.

    fn parse_block(&mut self) -> Result<Block, String> {
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

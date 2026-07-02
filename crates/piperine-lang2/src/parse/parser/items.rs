//! Top-level item grammar: `mod` declarations, ports, types, `discipline`,
//! `bundle`, `enum`, `capability`, `impl`, `fn`, and blocks.

use crate::parse::ast::*;
use crate::parse::lexer::Tok;

use super::Parser;

impl<'a> Parser<'a> {
    /// Parses a `mod Name[CONST]<TYPE>(PORTS) { body }` or `mod Name[CONST]<TYPE>(PORTS);` declaration.
    pub(crate) fn parse_mod_decl(&mut self, is_pub: bool) -> Result<ModDecl, String> {
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

    /// Parses a generic type parameter: `Name` or `Name: Cap1 + Cap2 + ...`.
    pub(crate) fn parse_type_param(&mut self) -> Result<TypeParam, String> {
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

    /// Parses a module port: `direction name : type`.
    pub(crate) fn parse_port(&mut self) -> Result<Port, String> {
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

    /// Parses a type reference: `Name<Args...>[dim1][dim2]...` or `fn(Args...) -> Ret`.
    pub(crate) fn parse_type(&mut self) -> Result<Type, String> {
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

    // ─────────────────────────── §4.1  Disciplines ───────────────────────────

    /// Parses a discipline declaration: `discipline Name { potential/flow/storage/resolve ... }`.
    pub(crate) fn parse_discipline(&mut self, is_pub: bool) -> Result<DisciplineDecl, String> {
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

    /// Parses an optional attribute list `(name=expr, ...)` following a nature declaration.
    pub(crate) fn parse_attr_list(&mut self) -> Result<Vec<Attr>, String> {
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

    /// Parses a bundle declaration: `bundle Name[CONST]<TYPE> { field: Type [= default], ... }`.
    pub(crate) fn parse_bundle(&mut self, is_pub: bool) -> Result<BundleDecl, String> {
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

    /// Parses an enum declaration: `enum Name[: Repr] { Variant [= expr], ... }`.
    pub(crate) fn parse_enum(&mut self, is_pub: bool) -> Result<EnumDecl, String> {
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

    /// Parses a capability declaration: `capability Name[: Super, ...] { fn sig; | fn decl { } }`.
    pub(crate) fn parse_capability(&mut self, is_pub: bool) -> Result<CapabilityDecl, String> {
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

    /// Parses an impl block: `impl [Capability for] Type[CONST]<TYPE> { fn ... }`.
    pub(crate) fn parse_impl(&mut self, is_pub: bool) -> Result<ImplDecl, String> {
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

    /// Parses a function signature (without body): `fn name<TYPE>(params) -> RetType`.
    pub(crate) fn parse_fn_sig(&mut self) -> Result<FnSig, String> {
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

    /// Parses a full function declaration: `fn name<TYPE>(params) -> RetType { body }`.
    pub(crate) fn parse_fn_decl(&mut self, is_pub: bool) -> Result<FnDecl, String> {
        let sig = self.parse_fn_sig()?;
        let body = self.parse_block()?;
        Ok(FnDecl { is_pub, sig, body })
    }

    // ─────────────────────────── Block ───────────────────────────────────────
    //
    // Block ::= "{" { Stmt } [ Expr ] "}"
    // The trailing Expr (no semicolon) is the block's value.

    /// Parses a block `{ stmts... [trailing_expr] }` with statements and an optional trailing expression.
    pub(crate) fn parse_block(&mut self) -> Result<Block, String> {
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

    /// Parses a global const declaration: `const Name : Type = Expr;`.
    pub(crate) fn parse_const_decl(&mut self, is_pub: bool) -> Result<ConstDecl, String> {
        let name = self.parse_ident()?;
        self.expect(&Tok::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&Tok::Assign)?;
        let value = self.parse_expr()?;
        self.expect(&Tok::Semi)?;
        Ok(ConstDecl { is_pub, name, ty, value })
    }
}

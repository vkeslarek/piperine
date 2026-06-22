//! Top-level items and module members — follows `veriloga.ungram`.

use super::*;

impl<'a> Parser<'a> {
    // ── top-level ────────────────────────────────────────────────────────

    pub(super) fn item(&mut self) -> PResult<Item> {
        let start = self.span_start();
        let attrs = self.attrs()?;
        if self.at_kw("discipline") {
            Ok(Item::DisciplineDecl(self.discipline(attrs, start)?))
        } else if self.at_kw("nature") {
            Ok(Item::NatureDecl(self.nature(attrs, start)?))
        } else if self.at_kw("module") || self.at_kw("macromodule") {
            Ok(Item::ModuleDecl(self.module(attrs, start)?))
        } else if self.at_kw("extern") {
            // We need to look ahead to see if it is `extern module` or `extern class`
            // Since we don't have peek_nth, we'll just parse the `extern` token
            self.bump();
            if self.at_kw("class") {
                Ok(Item::ExternClass(self.extern_class()?))
            } else if self.at_kw("module") {
                Ok(Item::ExternModule(self.extern_module(attrs, start)?))
            } else {
                Err(format!("expected 'module' or 'class' after 'extern', found {:?}", self.peek()))
            }
        } else if self.at_kw("typedef") {
            self.bump();
            if self.at_kw("enum") {
                Ok(Item::TypedefEnum(self.typedef_enum()?))
            } else if self.at_kw("struct") {
                Ok(Item::TypedefStruct(self.typedef_struct()?))
            } else {
                Err(format!("expected 'enum' or 'struct' after 'typedef', found {:?}", self.peek()))
            }
        } else if self.at_kw("paramset") {
            Ok(Item::Paramset(self.paramset(start)?))
        } else {
            Err(format!("expected top-level item, found {:?}", self.peek()))
        }
    }

    fn paramset(&mut self, start: usize) -> PResult<ParamsetDecl> {
        self.expect_kw("paramset")?;
        let name = self.name()?;
        let base = self.name()?;
        self.expect(&Tok::Semi)?;

        let mut entries = Vec::new();
        while !self.at_kw("endparamset") && !self.at_end() {
            self.expect(&Tok::Dot)?;
            let entry_name = self.name()?;
            self.expect(&Tok::Assign)?;
            let value = self.expr()?;
            self.expect(&Tok::Semi)?;
            entries.push(ParamsetEntry { name: entry_name, value });
        }
        self.expect_kw("endparamset")?;

        let span = Span { start, end: self.prev_end() };
        Ok(ParamsetDecl { span, name, base, entries })
    }

    fn extern_module(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ExternModuleDecl> {
        self.expect_kw("module")?;
        let name = self.name()?;
        self.expect(&Tok::LParen)?;

        let mut ports = Vec::new();
        let mut parameters = Vec::new();

        while !self.at(&Tok::RParen) && !self.at(&Tok::Semi) {
            if self.at_dir() {
                let port_start = self.span_start();
                let port_attrs = self.attrs()?;
                ports.push(self.port_decl(port_attrs, port_start)?);
            } else if self.at_kw("parameter") {
                parameters.push(self.extern_parameter()?);
            } else {
                return Err(format!("expected port or parameter in extern module, found {:?}", self.peek()));
            }
            if !self.eat(&Tok::Comma) { break; }
        }

        if self.eat(&Tok::Semi) {
            while !self.at(&Tok::RParen) {
                parameters.push(self.extern_parameter()?);
                if !self.eat(&Tok::Comma) { break; }
            }
        }

        self.expect(&Tok::RParen)?;
        self.expect(&Tok::Semi)?;

        let span = Span { start, end: self.prev_end() };
        Ok(ExternModuleDecl { span, attrs, name, ports, parameters })
    }

    fn extern_class(&mut self) -> PResult<ExternClassDecl> {
        self.expect_kw("class")?;
        let name = self.name()?;
        self.expect(&Tok::Semi)?;
        Ok(ExternClassDecl { name })
    }

    fn typedef_enum(&mut self) -> PResult<TypedefEnum> {
        self.expect_kw("enum")?;
        let base_type = if self.is_type_kw() { Some(self.type_()?) } else { None };
        self.expect(&Tok::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at_end() {
            let name = self.name()?;
            let value = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            variants.push(EnumVariant { name, value });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RBrace)?;
        let name = self.name()?;
        self.expect(&Tok::Semi)?;
        Ok(TypedefEnum { name, base_type, variants })
    }

    fn typedef_struct(&mut self) -> PResult<TypedefStruct> {
        self.expect_kw("struct")?;
        self.expect(&Tok::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at_end() {
            let ty = self.type_()?;
            let name = self.name()?;
            self.expect(&Tok::Semi)?;
            fields.push(StructField { ty, name });
        }
        self.expect(&Tok::RBrace)?;
        let name = self.name()?;
        self.expect(&Tok::Semi)?;
        Ok(TypedefStruct { name, fields })
    }
    fn extern_parameter(&mut self) -> PResult<ExternParameter> {
        self.expect_kw("parameter")?;
        let kind = if self.eat_kw("expr") {
            ExternParameterKind::Expr
        } else if self.eat_kw("ref") {
            ExternParameterKind::Ref
        } else {
            ExternParameterKind::Typed(self.type_()?)
        };
        let name = self.name()?;
        let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
        Ok(ExternParameter { name, kind, default })
    }

    // ── discipline ───────────────────────────────────────────────────────

    fn discipline(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<DisciplineDecl> {
        self.expect_kw("discipline")?;
        let name = self.name()?;
        self.eat(&Tok::Semi);
        let mut items = Vec::new();
        while !self.at_kw("enddiscipline") && !self.at_end() {
            let name = self.path()?;
            let val = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            self.eat(&Tok::Semi);
            items.push(DisciplineAttr { name, val });
        }
        self.expect_kw("enddiscipline")?;
        Ok(DisciplineDecl { attrs, name, items, span: Span { start, end: self.prev_end() } })
    }

    // ── nature ───────────────────────────────────────────────────────────

    fn nature(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<NatureDecl> {
        self.expect_kw("nature")?;
        let name = self.name()?;
        let parent = if self.eat(&Tok::Colon) { Some(self.path()?) } else { None };
        self.eat(&Tok::Semi);
        let mut items = Vec::new();
        while !self.at_kw("endnature") && !self.at_end() {
            let name = self.name()?;
            self.expect(&Tok::Assign)?;
            let val = self.expr()?;
            self.eat(&Tok::Semi);
            items.push(NatureAttr { name, val });
        }
        self.expect_kw("endnature")?;
        Ok(NatureDecl { attrs, name, parent, items, span: Span { start, end: self.prev_end() } })
    }

    // ── module ───────────────────────────────────────────────────────────

    fn module(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleDecl> {
        self.bump(); // `module` | `macromodule`
        let name = self.name()?;
        let ports = if self.at(&Tok::LParen) { Some(self.module_ports()?) } else { None };
        self.expect(&Tok::Semi)?;
        let mut items = Vec::new();
        while !self.at_kw("endmodule") && !self.at_end() {
            items.push(self.module_item()?);
        }
        self.expect_kw("endmodule")?;
        Ok(ModuleDecl { attrs, name, ports, items, span: Span { start, end: self.prev_end() } })
    }

    fn module_ports(&mut self) -> PResult<Vec<ModulePort>> {
        self.expect(&Tok::LParen)?;
        let mut ports = Vec::new();
        while !self.at(&Tok::RParen) {
            let start = self.span_start();
            let attrs = self.attrs()?;
            if self.at_dir() {
                ports.push(ModulePort::PortDecl(self.port_decl(attrs, start)?));
            } else {
                ports.push(ModulePort::Name(self.name()?));
            }
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        Ok(ports)
    }

    /// `Direction discipline:NameRef? net_type? [range] names` (no trailing `;`).
    fn port_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<PortDecl> {
        let dir = self.direction()?;
        let discipline = self.opt_discipline();
        self.skip_net_type();
        let range = self.parse_range()?;
        let names = self.declarator_list()?;
        Ok(PortDecl { attrs, dir, discipline, range, names, span: Span { start, end: self.prev_end() } })
    }

    /// Detect an optional leading `discipline NameRef` before the first net name.
    /// Present when current is Ident and, past an optional `[range]`, another Ident follows.
    fn opt_discipline(&mut self) -> Option<NameRef> {
        let next = self.idx_after_range(self.pos + 1);
        match (self.peek(), self.toks.get(next).map(|t| &t.tok)) {
            (Some(Tok::Ident(s)), Some(Tok::Ident(_))) => {
                let name = NameRef(s.clone());
                self.pos += 1;
                Some(name)
            }
            _ => None,
        }
    }

    /// Lookahead: `Type #? Name (` — a module instantiation rather than a net decl.
    fn looks_like_instance(&self) -> bool {
        match self.peek_at(1) {
            Some(Tok::Hash) => true,
            Some(Tok::Ident(_)) => {
                let after = self.idx_after_range(self.pos + 2);
                matches!(self.toks.get(after).map(|t| &t.tok), Some(Tok::LParen))
            }
            _ => false,
        }
    }

    // ── module items ─────────────────────────────────────────────────────

    fn module_item(&mut self) -> PResult<ModuleItem> {
        let start = self.span_start();
        let attrs = self.attrs()?;

        if self.at_kw("initial") {
            return self.initial_block(attrs, start);
        }
        if self.at_kw("always") {
            return self.always_block(attrs, start);
        }

        if self.at_dir() {
            let port = self.port_decl(attrs, start)?;
            self.expect(&Tok::Semi)?;
            return Ok(ModuleItem::BodyPortDecl(BodyPortDecl {
                port,
                span: Span { start, end: self.prev_end() },
            }));
        }
        if self.at_kw("analog") { return self.analog(attrs, start); }
        if self.at_kw("function") { return self.function(attrs, start); }
        if self.at_kw("branch") { return self.branch(attrs, start); }
        if self.at_kw("parameter") || self.at_kw("localparam") {
            return Ok(ModuleItem::ParamDecl(self.param_decl(attrs, start)?));
        }
        if self.at_kw("aliasparam") { return self.alias_param(attrs, start); }
        if self.looks_like_instance() { return self.instance(attrs, start); }
        if self.is_type_kw() || self.at_kw("genvar") {
            return Ok(ModuleItem::VarDecl(self.var_decl(attrs, start)?));
        }
        self.net_decl(attrs, start)
    }

    fn instance(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let module = self.name()?;
        let params = if self.eat(&Tok::Hash) { self.connection_list()? } else { Vec::new() };
        let name = self.name()?;
        let range = self.parse_range()?;
        let connections = self.connection_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::Instance(InstanceDecl {
            attrs, module, name, range, params, connections,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn connection_list(&mut self) -> PResult<Vec<Connection>> {
        self.expect(&Tok::LParen)?;
        let mut conns = Vec::new();
        let named = self.at(&Tok::Dot);
        while !self.at(&Tok::RParen) {
            if named {
                self.expect(&Tok::Dot)?;
                let port = self.name()?;
                self.expect(&Tok::LParen)?;
                let expr = if self.at(&Tok::RParen) { None } else { Some(self.expr()?) };
                self.expect(&Tok::RParen)?;
                conns.push(Connection::Named { port, expr });
            } else {
                conns.push(Connection::Positional(self.expr()?));
            }
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        Ok(conns)
    }

    fn net_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.skip_net_type();
        let discipline = self.opt_discipline();
        self.skip_net_type();
        let range = self.parse_range()?;
        let names = self.declarator_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::NetDecl(NetDecl {
            attrs, discipline, range, names,
            span: Span { start, end: self.prev_end() },
        }))
    }

    pub(super) fn var_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<VarDecl> {
        let ty = if self.eat_kw("genvar") { Type::Integer } else { self.type_()? };
        let mut vars = Vec::new();
        loop {
            let name = self.name()?;
            let range = self.parse_range()?;
            let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            vars.push(Var { name, range, default });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(VarDecl { attrs, ty, vars, span: Span { start, end: self.prev_end() } })
    }

    pub(super) fn param_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ParamDecl> {
        let kind = if self.eat_kw("localparam") { ParamKind::LocalParam }
                   else { self.expect_kw("parameter")?; ParamKind::Parameter };
        let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
        let mut params = Vec::new();
        loop {
            let name = self.name()?;
            self.skip_range();
            self.expect(&Tok::Assign)?;
            let default = self.expr()?;
            let mut constraints = Vec::new();
            while self.at_kw("from") || self.at_kw("exclude") {
                constraints.push(self.constraint()?);
            }
            params.push(Param { name, default, constraints });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ParamDecl { attrs, kind, ty, params, span: Span { start, end: self.prev_end() } })
    }

    fn constraint(&mut self) -> PResult<Constraint> {
        if self.eat_kw("from") {
            Ok(Constraint::From(self.constraint_value()?))
        } else {
            self.expect_kw("exclude")?;
            Ok(Constraint::Exclude(self.constraint_value()?))
        }
    }

    fn constraint_value(&mut self) -> PResult<ConstraintValue> {
        match self.peek() {
            Some(Tok::LParen) => {
                self.bump();
                let start = self.expr()?;
                if self.eat(&Tok::Colon) {
                    let end = self.expr()?;
                    let inclusive_right = self.range_closer()?;
                    Ok(ConstraintValue::Range(Range { inclusive_left: false, start, end, inclusive_right }))
                } else {
                    self.expect(&Tok::RParen)?;
                    Ok(ConstraintValue::Expr(Expr::Paren(Box::new(start))))
                }
            }
            Some(Tok::LBrack) => Ok(ConstraintValue::Range(self.range()?)),
            Some(Tok::LBrace) | Some(Tok::ArrStart) => {
                self.bump();
                let mut items = Vec::new();
                while !self.at(&Tok::RBrace) {
                    items.push(self.expr()?);
                    if !self.eat(&Tok::Comma) { break; }
                }
                self.expect(&Tok::RBrace)?;
                Ok(ConstraintValue::Array(items))
            }
            _ => Ok(ConstraintValue::Expr(self.expr()?)),
        }
    }

    fn range(&mut self) -> PResult<Range> {
        let inclusive_left = match self.bump().tok {
            Tok::LBrack => true,
            Tok::LParen => false,
            ref t => return Err(format!("expected range opener, found {t:?}")),
        };
        let start = self.expr()?;
        self.expect(&Tok::Colon)?;
        let end = self.expr()?;
        let inclusive_right = self.range_closer()?;
        Ok(Range { inclusive_left, start, end, inclusive_right })
    }

    fn range_closer(&mut self) -> PResult<bool> {
        match self.bump().tok {
            Tok::RBrack => Ok(true),
            Tok::RParen => Ok(false),
            ref t => Err(format!("expected range closer, found {t:?}")),
        }
    }

    fn alias_param(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("aliasparam")?;
        let name = self.name()?;
        self.expect(&Tok::Assign)?;
        let src = self.param_ref()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::AliasParam(AliasParam {
            attrs, name, src,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn param_ref(&mut self) -> PResult<ParamRef> {
        if let Some(Tok::SysCall(s)) = self.peek() {
            let s = format!("${s}");
            self.pos += 1;
            Ok(ParamRef::SysFun(s))
        } else {
            Ok(ParamRef::Path(self.path()?))
        }
    }

    fn branch(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("branch")?;
        self.expect(&Tok::LParen)?;
        let mut ports = Vec::new();
        while !self.at(&Tok::RParen) {
            ports.push(self.expr()?);
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        let names = self.name_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::BranchDecl(BranchDecl {
            attrs, ports, names,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn analog(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("analog")?;
        if self.at_kw("function") { return self.function(attrs, start); }
        let initial = self.eat_kw("initial");
        let stmt = Box::new(self.stmt()?);
        Ok(ModuleItem::AnalogBehaviour(AnalogBehaviour {
            attrs, initial, stmt,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn function(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("function")?;
        let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
        let name = self.name()?;
        let mut items = Vec::new();
        // Optional SystemVerilog-style parenthesized argument list:
        // `function real f(input real a, input real b);`
        // (Verilog-A style `input real a;` inside the body is still accepted below.)
        if self.eat(&Tok::LParen) {
            while !self.at(&Tok::RParen) && !self.at_end() {
                let arg_attrs = self.attrs()?;
                let dir = if self.at_dir() { self.direction()? } else { Direction::Input };
                if self.is_type_kw() { let _ = self.type_()?; }
                let arg_name = self.name()?;
                items.push(FunctionItem::FunctionArg(FunctionArg {
                    attrs: arg_attrs, dir, names: vec![arg_name],
                }));
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RParen)?;
        }
        self.expect(&Tok::Semi)?;
        while !self.at_kw("endfunction") && !self.at_end() {
            items.push(self.function_item()?);
        }
        self.expect_kw("endfunction")?;
        Ok(ModuleItem::Function(Function {
            attrs, ty, name, items,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn function_item(&mut self) -> PResult<FunctionItem> {
        let start = self.span_start();
        let attrs = self.attrs()?;
        if self.at_dir() {
            let dir = self.direction()?;
            if self.is_type_kw() { let _ = self.type_()?; } // tolerate `input real x;`
            let names = self.name_list()?;
            self.expect(&Tok::Semi)?;
            return Ok(FunctionItem::FunctionArg(FunctionArg { attrs, dir, names }));
        }
        if self.at_kw("parameter") || self.at_kw("localparam") {
            return Ok(FunctionItem::ParamDecl(self.param_decl(attrs, start)?));
        }
        if !self.at_stmt_kw() && (self.is_type_kw() || self.at_kw("genvar")) {
            return Ok(FunctionItem::VarDecl(self.var_decl(attrs, start)?));
        }
        Ok(FunctionItem::Stmt(self.stmt_with_attrs(attrs)?))
    }

    fn initial_block(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("initial")?;
        let stmt = Box::new(self.stmt()?);
        Ok(ModuleItem::InitialBlock(InitialBlock {
            attrs, stmt, span: Span { start, end: self.prev_end() },
        }))
    }

    fn always_block(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("always")?;
        self.expect(&Tok::At)?;
        self.expect(&Tok::LParen)?;
        let sensitivity = if self.eat_kw("initial_step") {
            AlwaysSensitivity::InitialStep
        } else if self.eat_kw("final_step") {
            AlwaysSensitivity::FinalStep
        } else if self.eat_kw("step") {
            AlwaysSensitivity::Step
        } else if self.eat_kw("above") {
            self.expect(&Tok::LParen)?;
            let expr = self.expr()?;
            self.expect(&Tok::RParen)?;
            AlwaysSensitivity::Above(expr)
        } else if self.eat_kw("cross") {
            self.expect(&Tok::LParen)?;
            let expr = self.expr()?;
            self.expect(&Tok::Comma)?;
            
            let mut sign = 1;
            if self.eat(&Tok::Plus) {
                sign = 1;
            } else if self.eat(&Tok::Minus) {
                sign = -1;
            }
            
            let dir = if let Some(Tok::Int(val)) = self.peek() {
                let val = val.clone();
                self.pos += 1;
                let num: i8 = val.parse().map_err(|_| "invalid crossing direction")?;
                num * sign
            } else {
                return Err("expected crossing direction (+1, -1, 0)".into());
            };
            self.expect(&Tok::RParen)?;
            AlwaysSensitivity::Cross(expr, dir)
        } else {
            return Err("expected sensitivity inside always @(...)".into());
        };
        self.expect(&Tok::RParen)?;
        let stmt = Box::new(self.stmt()?);
        Ok(ModuleItem::AlwaysBlock(AlwaysBlock {
            span: Span { start, end: self.prev_end() },
            sensitivity,
            stmt,
        }))
    }
}

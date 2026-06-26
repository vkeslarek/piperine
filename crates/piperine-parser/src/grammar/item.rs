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
        } else if self.at_kw("module") || self.at_kw("macromodule") || self.at_kw("connectmodule") {
            Ok(Item::ModuleDecl(self.module(attrs, start)?))
        } else if self.at_kw("paramset") {
            Ok(Item::Paramset(self.paramset_decl(attrs, start)?))
        } else if self.at_kw("connectrules") {
            Ok(Item::Connectrules(self.connectrules_decl(attrs, start)?))
        } else if self.at_kw("config") {
            Ok(Item::Config(self.config_decl(attrs, start)?))
        } else {
            Err(format!("expected top-level item, found {:?}", self.peek()))
        }
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
        let kind = if self.eat_kw("macromodule") {
            ModuleKind::Macromodule
        } else if self.eat_kw("connectmodule") {
            ModuleKind::Connectmodule
        } else {
            self.expect_kw("module")?;
            ModuleKind::Module
        };
        let name = self.name()?;
        let ports = if self.at(&Tok::LParen) {
            Some(self.module_ports()?)
        } else {
            None
        };
        self.expect(&Tok::Semi)?;
        let mut items = Vec::new();
        while !self.at_kw("endmodule") && !self.at_kw("endconnectmodule") && !self.at_end() {
            items.push(self.module_item()?);
        }
        self.expect_kw("endmodule")?;
        Ok(ModuleDecl { attrs, kind, name, ports, items, span: Span { start, end: self.prev_end() } })
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
        let net_type = self.opt_net_type();
        let signed1 = self.opt_signed();
        let var_type = if self.is_type_kw() { Some(self.type_()?) } else { None };
        let signed2 = self.opt_signed();
        let signed = signed1 || signed2;
        let discipline = self.opt_discipline();
        let range = self.parse_range()?;
        let names = self.declarator_list()?;
        Ok(PortDecl { attrs, dir, net_type, var_type, signed, discipline, range, names, span: Span { start, end: self.prev_end() } })
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

    fn paramset_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ParamsetDecl> {
        self.expect_kw("paramset")?;
        let name = self.name()?;
        let base = self.name()?;
        self.expect(&Tok::Semi)?;
        let mut item_decls = Vec::new();
        let mut statements = Vec::new();
        while !self.at_kw("endparamset") && !self.at_end() {
            if self.at_kw("parameter") || self.at_kw("localparam") {
                item_decls.push(ParamsetItemDecl::Parameter(self.param_decl(vec![], self.span_start())?));
            } else if self.at_kw("aliasparam") {
                if let ModuleItem::AliasParam(ap) = self.alias_param(vec![], self.span_start())? {
                    item_decls.push(ParamsetItemDecl::AliasParam(ap));
                }
            } else if self.eat(&Tok::Dot) {
                if let Some(Tok::SysCall(s)) = self.peek() {
                    let s = s.clone();
                    self.bump();
                    self.expect(&Tok::Assign)?;
                    let expr = self.expr()?;
                    self.expect(&Tok::Semi)?;
                    statements.push(ParamsetStmt::SysDotAssign { name: s, value: expr });
                } else {
                    let p = self.name()?;
                    self.expect(&Tok::Assign)?;
                    let expr = self.expr()?;
                    self.expect(&Tok::Semi)?;
                    statements.push(ParamsetStmt::DotAssign { name: p, value: expr });
                }
            } else {
                statements.push(ParamsetStmt::AnalogStmt(Box::new(self.stmt()?)));
            }
        }
        self.expect_kw("endparamset")?;
        Ok(ParamsetDecl { span: Span { start, end: self.prev_end() }, attrs, name, base, item_decls, statements })
    }

    fn connectrules_decl(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ConnectrulesDecl> {
        self.expect_kw("connectrules")?;
        let name = self.name()?;
        self.expect(&Tok::Semi)?;
        let mut items = Vec::new();
        while !self.at_kw("endconnectrules") && !self.at_end() {
            self.expect_kw("connect")?;
            if self.eat_kw("module") {
                let module = self.name()?;
                let mut mode = None;
                if self.eat_kw("merged") { mode = Some(ConnectMode::Merged); }
                else if self.eat_kw("split") { mode = Some(ConnectMode::Split); }
                while !self.eat(&Tok::Semi) && !self.at_end() { self.bump(); }
                items.push(ConnectrulesItem::Insertion { module, mode, params: vec![], port_overrides: None });
            } else {
                let mut disciplines = Vec::new();
                loop {
                    disciplines.push(self.name()?);
                    if !self.eat(&Tok::Comma) { break; }
                }
                self.expect_kw("resolveto")?;
                let target = if self.eat_kw("exclude") { ResolveTarget::Exclude } else { ResolveTarget::Discipline(self.name()?) };
                self.expect(&Tok::Semi)?;
                items.push(ConnectrulesItem::Resolution { disciplines, target });
            }
        }
        self.expect_kw("endconnectrules")?;
        Ok(ConnectrulesDecl { span: Span { start, end: self.prev_end() }, name, items })
    }

    fn config_decl(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ConfigDecl> {
        self.expect_kw("config")?;
        while !self.at_kw("endconfig") && !self.at_end() {
            self.bump();
        }
        self.expect_kw("endconfig")?;
        Ok(ConfigDecl { span: Span { start, end: self.prev_end() } })
    }


    // ── module items ─────────────────────────────────────────────────────

    fn module_item(&mut self) -> PResult<ModuleItem> {
        let start = self.span_start();
        let attrs = self.attrs()?;

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
        if self.eat_kw("ground") { return self.ground_decl(attrs, start); }
        if self.eat_kw("event") { return self.event_decl(attrs, start); }
        if self.eat_kw("initial") { return self.initial_construct(attrs, start); }
        if self.eat_kw("always") { return self.always_construct(attrs, start); }
        if self.eat_kw("defparam") { return self.defparam_decl(attrs, start); }
        if self.eat_kw("assign") { return self.continuous_assign(attrs, start); }

        if self.eat_kw("generate") { return self.generate_region(attrs, start); }
        if self.at_kw("for") { return self.loop_generate(attrs, start); }
        if self.at_kw("if") { return self.if_generate(attrs, start); }
        if self.at_kw("case") { return self.case_generate(attrs, start); }

        if self.is_module_instantiation() {
            return self.module_instantiation(attrs, start);
        }

        let custom_var = self.is_type_kw() && self.assign_before_semi();
        if self.at_primitive_type_kw() || custom_var || self.at_kw("genvar") {
            return Ok(ModuleItem::VarDecl(self.var_decl(attrs, start)?));
        }
        self.net_decl(attrs, start)
    }



    fn net_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let net_type = self.opt_net_type();
        let discipline = self.opt_discipline();
        let range = self.parse_range()?;
        let names = self.declarator_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::NetDecl(NetDecl {
            attrs, net_type, drive_strength: None, charge_strength: None, delay: None, discipline, range, names,
            span: Span { start, end: self.prev_end() },
        }))
    }

    pub(super) fn var_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<VarDecl> {
        let ty = if self.eat_kw("genvar") { Type::Integer } else { self.type_()? };
        let signed = self.opt_signed();
        let mut vars = Vec::new();
        loop {
            let name = self.name()?;
            let range = self.parse_range()?;
            let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            vars.push(Var { name, range, default });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(VarDecl { attrs, ty, signed, discipline: None, vars, span: Span { start, end: self.prev_end() } })
    }

    fn ground_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let discipline = if let Some(Tok::Ident(s)) = self.peek() {
            if matches!(self.peek_at(1), Some(Tok::Ident(_))) {
                let name = Name(s.clone());
                self.pos += 1;
                Some(name)
            } else { None }
        } else { None };
        let range = self.parse_range()?;
        let names = self.declarator_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::GroundDecl(GroundDecl { attrs, discipline, range, names, span: Span { start, end: self.prev_end() } }))
    }

    fn event_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let names = self.declarator_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::EventDecl(EventDecl { attrs, names, span: Span { start, end: self.prev_end() } }))
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
                constraints.push(self.param_constraint()?);
            }
            params.push(Param { name, default, constraints });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ParamDecl { attrs, kind, ty, params, span: Span { start, end: self.prev_end() } })
    }

    fn param_constraint(&mut self) -> PResult<Constraint> {
        if self.eat_kw("from") {
            Ok(Constraint::From(self.constraint_value()?))
        } else {
            self.expect_kw("exclude")?;
            Ok(Constraint::Exclude(self.constraint_value()?))
        }
    }

    fn constraint_value(&mut self) -> PResult<ConstraintValue> {
        if self.at(&Tok::LBrack) || self.at(&Tok::LParen) {
            let left = if self.eat(&Tok::LBrack) { true } else { self.eat(&Tok::LParen); false };
            let start = self.expr()?;
            self.expect(&Tok::Colon)?;
            let end = self.expr()?;
            let right = if self.eat(&Tok::RBrack) { true } else { self.expect(&Tok::RParen)?; false };
            Ok(ConstraintValue::Range(Range { inclusive_left: left, start, end, inclusive_right: right }))
        } else if self.eat(&Tok::ArrStart) {
            let mut arr = Vec::new();
            while !self.at(&Tok::RBrace) && !self.at_end() {
                arr.push(self.expr()?);
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RBrace)?;
            Ok(ConstraintValue::Array(arr))
        } else {
            Ok(ConstraintValue::Expr(self.expr()?))
        }
    }

    // ==========================================
    // Phase 3 & 4 Parser Extensions
    // ==========================================

    fn initial_construct(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let stmt = Box::new(self.stmt()?);
        Ok(ModuleItem::InitialConstruct { span: Span { start, end: self.prev_end() }, attrs, stmt })
    }

    fn always_construct(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let stmt = Box::new(self.stmt()?);
        Ok(ModuleItem::AlwaysConstruct { span: Span { start, end: self.prev_end() }, attrs, stmt })
    }

    fn defparam_decl(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let mut assignments = Vec::new();
        loop {
            let path = self.path()?;
            self.expect(&Tok::Assign)?;
            let expr = self.expr()?;
            assignments.push((path, expr));
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::Defparam(DefparamDecl { span: Span { start, end: self.prev_end() }, assignments }))
    }

    fn continuous_assign(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let mut assignments = Vec::new();
        loop {
            let lval = self.expr()?;
            self.expect(&Tok::Assign)?;
            let rval = self.expr()?;
            assignments.push((lval, rval));
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::ContinuousAssign(ContinuousAssign { 
            span: Span { start, end: self.prev_end() }, attrs, drive_strength: None, delay: None, assignments 
        }))
    }

    fn is_module_instantiation(&self) -> bool {
        if let Some(Tok::Ident(_)) = self.peek() {
            if matches!(self.peek_at(1), Some(Tok::Hash)) {
                return true;
            }
            if let Some(Tok::Ident(_)) = self.peek_at(1) {
                let mut idx = 2;
                if matches!(self.toks.get(self.pos + idx).map(|t| &t.tok), Some(Tok::LBrack)) {
                    idx = self.idx_after_range(self.pos + idx) - self.pos;
                }
                if matches!(self.toks.get(self.pos + idx).map(|t| &t.tok), Some(Tok::LParen)) {
                    return true;
                }
            }
        }
        false
    }

    fn module_instantiation(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let module_name = self.name()?;
        let mut param_assignments = Vec::new();
        if self.eat(&Tok::Hash) {
            if self.eat(&Tok::LParen) {
                while !self.at(&Tok::RParen) && !self.at_end() {
                    if self.eat(&Tok::Dot) {
                        let param = self.name()?;
                        self.expect(&Tok::LParen)?;
                        let expr = self.expr()?;
                        self.expect(&Tok::RParen)?;
                        param_assignments.push(ParamAssignment::Named { param, expr });
                    } else {
                        param_assignments.push(ParamAssignment::Ordered(self.expr()?));
                    }
                    if !self.eat(&Tok::Comma) { break; }
                }
                self.expect(&Tok::RParen)?;
            } else {
                param_assignments.push(ParamAssignment::Ordered(self.expr()?));
            }
        }
        let mut instances = Vec::new();
        loop {
            let name = self.name()?;
            let range = self.parse_range()?;
            self.expect(&Tok::LParen)?;
            let mut connections = Vec::new();
            while !self.at(&Tok::RParen) && !self.at_end() {
                if self.eat(&Tok::Dot) {
                    if self.eat(&Tok::Star) {
                        // .* — auto-connect all ports by name
                        connections.push(PortConnection::Wildcard);
                    } else {
                        let port = self.name()?;
                        self.expect(&Tok::LParen)?;
                        let expr = if self.at(&Tok::RParen) { None } else { Some(self.expr()?) };
                        self.expect(&Tok::RParen)?;
                        connections.push(PortConnection::Named { port, expr });
                    }
                } else {
                    let expr = if self.at(&Tok::Comma) || self.at(&Tok::RParen) { None } else { Some(self.expr()?) };
                    connections.push(PortConnection::Ordered(expr));
                }
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RParen)?;
            instances.push(ModuleInstance { name, range, connections });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::ModuleInstantiation(ModuleInstantiation {
            span: Span { start, end: self.prev_end() }, attrs, module_name, param_assignments, instances
        }))
    }

    // ==========================================
    // Phase 5 Parser Extensions
    // ==========================================

    fn generate_region(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let mut items = Vec::new();
        while !self.at_kw("endgenerate") && !self.at_end() {
            items.push(self.module_item()?);
        }
        self.expect_kw("endgenerate")?;
        Ok(ModuleItem::Generate(GenerateRegion { span: Span { start, end: self.prev_end() }, items }))
    }

    fn generate_block(&mut self) -> PResult<GenerateBlock> {
        if self.eat_kw("begin") {
            let label = if self.eat(&Tok::Colon) { Some(self.name()?) } else { None };
            let mut items = Vec::new();
            while !self.at_kw("end") && !self.at_end() {
                items.push(self.module_item()?);
            }
            self.expect_kw("end")?;
            Ok(GenerateBlock::Block { label, items })
        } else if self.eat(&Tok::Semi) {
            Ok(GenerateBlock::Block { label: None, items: vec![] })
        } else {
            Ok(GenerateBlock::Single(Box::new(self.module_item()?)))
        }
    }

    fn loop_generate(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("for")?;
        self.expect(&Tok::LParen)?;
        let init_name = self.name()?;
        self.expect(&Tok::Assign)?;
        let init_expr = self.expr()?;
        self.expect(&Tok::Semi)?;
        let condition = self.expr()?;
        self.expect(&Tok::Semi)?;
        let iter_name = self.name()?;
        self.expect(&Tok::Assign)?;
        let iter_expr = self.expr()?;
        self.expect(&Tok::RParen)?;
        let body = self.generate_block()?;
        Ok(ModuleItem::LoopGenerate(LoopGenerate { 
            span: Span { start, end: self.prev_end() }, 
            init: (init_name, init_expr), condition, iteration: (iter_name, iter_expr), body 
        }))
    }

    fn if_generate(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("if")?;
        self.expect(&Tok::LParen)?;
        let condition = self.expr()?;
        self.expect(&Tok::RParen)?;
        let then_block = self.generate_block()?;
        let else_block = if self.eat_kw("else") { Some(self.generate_block()?) } else { None };
        Ok(ModuleItem::IfGenerate(IfGenerate { span: Span { start, end: self.prev_end() }, condition, then_block, else_block }))
    }

    fn case_generate(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        self.expect_kw("case")?;
        self.expect(&Tok::LParen)?;
        let condition = self.expr()?;
        self.expect(&Tok::RParen)?;
        let mut items = Vec::new();
        while !self.at_kw("endcase") && !self.at_end() {
            let mut exprs = Vec::new();
            if !self.eat_kw("default") {
                loop {
                    exprs.push(self.expr()?);
                    if !self.eat(&Tok::Comma) { break; }
                }
            } else {
                self.eat(&Tok::Colon);
            }
            if !exprs.is_empty() { self.expect(&Tok::Colon)?; }
            let block = self.generate_block()?;
            items.push(CaseGenerateItem { exprs, block });
        }
        self.expect_kw("endcase")?;
        Ok(ModuleItem::CaseGenerate(CaseGenerate { span: Span { start, end: self.prev_end() }, condition, items }))
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

}

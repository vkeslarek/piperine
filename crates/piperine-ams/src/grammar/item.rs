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
        } else if self.eat_kw("primitive") {
            Ok(Item::Primitive(self.primitive_decl(attrs, start)?))
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
        let mut domain = None;
        while !self.at_kw("enddiscipline") && !self.at_end() {
            let attr_name = self.path()?;
            let val = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            self.eat(&Tok::Semi);
            // Extract domain directly while parsing rather than leaving it buried in items.
            if attr_name.qualifier.is_none() {
                if let PathSegment::Ident(ref seg) = attr_name.segment {
                    if seg == "domain" {
                        if let Some(ref e) = val {
                            if let crate::ast::Expr::Path(ref p) = *e {
                                if p.qualifier.is_none() {
                                    if let PathSegment::Ident(ref v) = p.segment {
                                        domain = match v.as_str() {
                                            "continuous" => Some(Domain::Continuous),
                                            "discrete"   => Some(Domain::Discrete),
                                            _ => None,
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
            }
            items.push(DisciplineAttr { name: attr_name, val });
        }
        self.expect_kw("enddiscipline")?;
        Ok(DisciplineDecl { attrs, name, domain, items, span: Span { start, end: self.prev_end() } })
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

        // NEW: optional #(parameter_declaration {, parameter_declaration})
        let mut param_ports = Vec::new();
        if self.eat(&Tok::Hash) {
            self.expect(&Tok::LParen)?;
            while !self.at(&Tok::RParen) && !self.at_end() {
                let pp_start = self.span_start();
                let pp_attrs = self.attrs()?;
                param_ports.push(self.module_param_port(pp_attrs, pp_start)?);
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RParen)?;
        }

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
        Ok(ModuleDecl { attrs, kind, name, param_ports, ports, items, span: Span { start, end: self.prev_end() } })
    }

    /// Parse one parameter declaration in a `#(...)` module header.
    /// Like `param_decl()` but does NOT consume a trailing `;`.
    fn module_param_port(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ParamDecl> {
        let kind = if self.eat_kw("localparam") { ParamKind::LocalParam }
                   else { self.expect_kw("parameter")?; ParamKind::Parameter };
        let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
        let name = self.name()?;
        self.skip_range();
        self.expect(&Tok::Assign)?;
        let default = self.expr()?;
        let mut constraints = Vec::new();
        while self.at_kw("from") || self.at_kw("exclude") {
            constraints.push(self.param_constraint()?);
        }
        Ok(ParamDecl {
            attrs, kind, ty, signed: false, range: None,
            params: vec![Param { name, default, constraints }],
            span: Span { start, end: self.prev_end() },
        })
    }

    fn module_ports(&mut self) -> PResult<Vec<ModulePort>> {
        self.expect(&Tok::LParen)?;
        let mut ports = Vec::new();
        while !self.at(&Tok::RParen) {
            let start = self.span_start();
            let attrs = self.attrs()?;
            if self.at_dir() {
                ports.push(ModulePort::PortDecl(self.port_decl(attrs, start)?));
            } else if self.eat(&Tok::Dot) {
                // .port_id([port_expression])
                let port = self.name()?;
                self.expect(&Tok::LParen)?;
                let expr = if self.at(&Tok::RParen) { None } else { Some(self.port_expr()?) };
                self.expect(&Tok::RParen)?;
                ports.push(ModulePort::NamedExternal { port, expr });
            } else if self.at(&Tok::Comma) || self.at(&Tok::RParen) {
                // Empty port (blank entry)
                ports.push(ModulePort::Name(Name(String::new())));
            } else {
                // port_reference or port_expression
                ports.push(ModulePort::Name(self.name()?));
                // Ignore optional range on bare port names for now — they're informational
                self.skip_range();
            }
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        Ok(ports)
    }

    fn port_expr(&mut self) -> PResult<PortExpr> {
        if self.eat(&Tok::LBrace) {
            // Concatenation: { port_ref, port_ref, ... }
            let mut refs = Vec::new();
            while !self.at(&Tok::RBrace) && !self.at_end() {
                let name = self.name()?;
                let range = self.parse_range()?;
                refs.push((name, range));
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RBrace)?;
            Ok(PortExpr::Concat(refs))
        } else {
            let name = self.name()?;
            let range = self.parse_range()?;
            Ok(PortExpr::Ref { name, range })
        }
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
                
                let params = if self.at(&Tok::Hash) {
                    // parameter_value_assignment #(...)
                    self.eat(&Tok::Hash);
                    self.expect(&Tok::LParen)?;
                    let mut ps = Vec::new();
                    while !self.at(&Tok::RParen) && !self.at_end() {
                        if self.eat(&Tok::Dot) {
                            let param = self.name()?;
                            self.expect(&Tok::LParen)?;
                            let expr = self.expr()?;
                            self.expect(&Tok::RParen)?;
                            ps.push(ParamAssignment::Named { param, expr });
                        } else {
                            ps.push(ParamAssignment::Ordered(self.expr()?));
                        }
                        if !self.eat(&Tok::Comma) { break; }
                    }
                    self.expect(&Tok::RParen)?;
                    ps
                } else {
                    Vec::new()
                };

                // optional connect_port_overrides
                let port_overrides = if !self.at(&Tok::Semi) {
                    // It can be `disc, disc` or `dir disc, dir disc`
                    let (dir1, disc1) = if self.at_dir() {
                        (Some(self.direction()?), self.name()?)
                    } else {
                        (None, self.name()?)
                    };
                    self.expect(&Tok::Comma)?;
                    let (dir2, disc2) = if self.at_dir() {
                        (Some(self.direction()?), self.name()?)
                    } else {
                        (None, self.name()?)
                    };
                    let (input_disc, output_disc) = match (dir1, dir2) {
                        (Some(Direction::Input), Some(Direction::Output)) => (Some(disc1), Some(disc2)),
                        (Some(Direction::Output), Some(Direction::Input)) => (Some(disc2), Some(disc1)),
                        _ => (Some(disc1), Some(disc2)), // For inout/inout or bare disciplines, just map 1 to 1
                    };
                    Some(ConnectPortOverrides { input_disc, output_disc })
                } else {
                    None
                };

                self.expect(&Tok::Semi)?;
                items.push(ConnectrulesItem::Insertion { module, mode, params, port_overrides });
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

    fn primitive_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<PrimitiveDecl> {
        let name = self.name()?;
        self.expect(&Tok::LParen)?;
        let mut ports = Vec::new();
        while !self.at(&Tok::RParen) && !self.at_end() {
            ports.push(self.name()?);
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen)?;
        self.expect(&Tok::Semi)?;

        // Port declarations
        let mut port_decls = Vec::new();
        while self.at_dir() {
            let pd_start = self.span_start();
            let pd_attrs = self.attrs()?;
            let pd = self.port_decl(pd_attrs, pd_start)?;
            self.expect(&Tok::Semi)?;
            port_decls.push(pd);
        }

        // UDP body: optional `initial output_port = init_val ;` then `table ... endtable`
        let mut initial_stmt = None;
        if self.eat_kw("initial") {
            let out_name = self.name()?;
            self.expect(&Tok::Assign)?;
            // init_val: 1'b0 | 1'b1 | 1'bx | ... | 0 | 1
            // crude: accept any token as string for the initialization value
            let mut init_val_str = String::new();
            if let Some(tok) = self.peek() {
                init_val_str = format!("{:?}", tok);
                self.bump();
            }
            self.expect(&Tok::Semi)?;
            initial_stmt = Some((out_name, init_val_str));
        }

        self.expect_kw("table")?;
        let mut entries = Vec::new();
        while !self.at_kw("endtable") && !self.at_end() {
            entries.push(self.udp_entry(initial_stmt.is_some())?);
        }
        self.expect_kw("endtable")?;

        let body = if initial_stmt.is_some() || entries.iter().any(|e| e.current_state.is_some()) {
            UdpBody::Sequential { initial: initial_stmt, entries }
        } else {
            UdpBody::Combinational(entries)
        };

        self.expect_kw("endprimitive")?;
        Ok(PrimitiveDecl {
            span: Span { start, end: self.prev_end() },
            attrs, name, ports, port_decls, body,
        })
    }

    fn udp_entry(&mut self, sequential: bool) -> PResult<UdpEntry> {
        let mut tokens = Vec::new();
        while !self.at(&Tok::Semi) && !self.at_end() {
            match self.peek() {
                Some(Tok::Ident(s)) => { tokens.push(s.clone()); self.bump(); }
                Some(Tok::Int(s)) => { tokens.push(s.clone()); self.bump(); }
                Some(Tok::Star) => { tokens.push("*".to_string()); self.bump(); }
                Some(Tok::Colon) => { tokens.push(":".to_string()); self.bump(); }
                Some(Tok::Minus) => { tokens.push("-".to_string()); self.bump(); }
                Some(Tok::LParen) => { tokens.push("(".to_string()); self.bump(); }
                Some(Tok::RParen) => { tokens.push(")".to_string()); self.bump(); }
                _ => { self.bump(); }
            }
        }
        self.expect(&Tok::Semi)?;
        let colon_positions: Vec<usize> = tokens.iter().enumerate()
            .filter(|(_, t)| t.as_str() == ":")
            .map(|(i, _)| i)
            .collect();
        if sequential && colon_positions.len() >= 2 {
            let c1 = colon_positions[0];
            let c2 = colon_positions[1];
            let inputs = tokens[..c1].to_vec();
            let current_state = Some(tokens[c1+1..c2].join(""));
            let next_state = tokens[c2+1..].join("");
            Ok(UdpEntry { inputs, current_state, next_state })
        } else if let Some(&c) = colon_positions.last() {
            let inputs = tokens[..c].to_vec();
            let next_state = tokens[c+1..].join("");
            Ok(UdpEntry { inputs, current_state: None, next_state })
        } else {
            Err("malformed UDP table entry".to_string())
        }
    }

    fn config_decl(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ConfigDecl> {
        self.expect_kw("config")?;
        let name = self.name()?;
        self.expect(&Tok::Semi)?;

        // design_statement
        self.expect_kw("design")?;
        let mut design = Vec::new();
        while !self.at(&Tok::Semi) && !self.at_end() {
            design.push(self.config_cell_ref()?);
        }
        self.expect(&Tok::Semi)?;

        let mut rules = Vec::new();
        while !self.at_kw("endconfig") && !self.at_end() {
            rules.push(self.config_rule()?);
        }
        self.expect_kw("endconfig")?;
        Ok(ConfigDecl { span: Span { start, end: self.prev_end() }, name, design, rules })
    }

    fn config_cell_ref(&mut self) -> PResult<ConfigCellRef> {
        let first = self.name()?;
        if self.eat(&Tok::Dot) {
            let cell = self.name()?;
            Ok(ConfigCellRef { library: Some(first), cell })
        } else {
            Ok(ConfigCellRef { library: None, cell: first })
        }
    }

    fn config_rule(&mut self) -> PResult<ConfigRule> {
        if self.eat_kw("default") {
            let clause = self.liblist_or_use()?;
            self.expect(&Tok::Semi)?;
            Ok(ConfigRule::Default(clause))
        } else if self.eat_kw("instance") {
            let mut path = vec![self.name()?];
            while self.eat(&Tok::Dot) { path.push(self.name()?); }
            let clause = self.liblist_or_use()?;
            self.expect(&Tok::Semi)?;
            Ok(ConfigRule::Inst { path, clause })
        } else {
            self.expect_kw("cell")?;
            let cell_ref = self.config_cell_ref()?;
            let clause = self.liblist_or_use()?;
            self.expect(&Tok::Semi)?;
            Ok(ConfigRule::Cell { cell_ref, clause })
        }
    }

    fn liblist_or_use(&mut self) -> PResult<LiblistOrUse> {
        if self.eat_kw("liblist") {
            let mut libs = Vec::new();
            while matches!(self.peek(), Some(Tok::Ident(_))) 
                && !self.at_any_kw(&["default","instance","cell","endconfig"]) 
            {
                libs.push(self.name()?);
            }
            Ok(LiblistOrUse::Liblist(libs))
        } else {
            self.expect_kw("use")?;
            let cell_ref = self.config_cell_ref()?;
            let config = self.eat(&Tok::Colon) && self.eat_kw("config");
            Ok(LiblistOrUse::Use { cell_ref, config })
        }
    }


    // ── module items ─────────────────────────────────────────────────────

    fn module_item(&mut self) -> PResult<ModuleItem> {
        let attrs = self.attrs()?;
        let start = self.pos;

        if self.at_dir() {
            let port = self.port_decl(attrs, start)?;
            self.expect(&Tok::Semi)?;
            return Ok(ModuleItem::PortDecl(port));
        }
        if self.at_kw("analog") { return self.analog(attrs, start); }
        if self.at_kw("function") { return self.function(attrs, start); }
        if self.at_kw("branch") { return self.branch(attrs, start); }
        if self.at_kw("parameter") || self.at_kw("localparam") {
            return Ok(ModuleItem::ParamDecl(self.param_decl(attrs, start)?));
        }
        if self.at_kw("aliasparam") { return self.alias_param(attrs, start); }
        if self.eat_kw("specparam") { return self.specparam_decl(attrs, start); }
        if self.eat_kw("ground") { return self.ground_decl(attrs, start); }
        if self.eat_kw("event") { return self.event_decl(attrs, start); }
        if self.eat_kw("initial") { return self.initial_construct(attrs, start); }
        if self.eat_kw("always") { return self.always_construct(attrs, start); }
        if self.eat_kw("task") { return self.task_decl(attrs, start); }
        if self.eat_kw("defparam") { return self.defparam_decl(attrs, start); }
        if self.eat_kw("assign") { return self.continuous_assign(attrs, start); }
        if self.eat_kw("specify") { return self.specify_block(attrs, start); }

        if self.eat_kw("generate") { return self.generate_region(attrs, start); }
        if self.at_kw("for") { return self.loop_generate(attrs, start); }
        if self.at_kw("if") { return self.if_generate(attrs, start); }
        if self.at_kw("case") { return self.case_generate(attrs, start); }

        if self.at_gate_type() {
            return self.gate_instantiation(attrs, start);
        }

        if self.is_module_instantiation() {
            return self.module_instantiation(attrs, start);
        }

        if self.at_net_type() {
            return self.net_decl(attrs, start);
        }

        let custom_var = self.is_type_kw() && self.assign_before_semi();
        if self.at_primitive_type_kw() || custom_var || self.at_kw("genvar") {
            return Ok(ModuleItem::VarDecl(self.var_decl(attrs, start)?));
        }
        self.net_decl(attrs, start)
    }



    fn net_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let net_type = self.opt_net_type();
        // Optional drive_strength or charge_strength (only if LParen follows)
        let (drive_strength, charge_strength) = self.opt_drive_or_charge_strength(net_type.as_ref())?;
        // Optional vectored/scalared (consume and discard — no AST field)
        self.eat_kw("vectored");
        self.eat_kw("scalared");
        // Optional signed keyword
        let _signed = self.eat_kw("signed");
        // Optional delay3 (only if Hash follows)
        let delay = self.opt_delay()?;
        
        let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
        let discipline = self.opt_discipline();
        let range = self.parse_range()?;
        let names = self.declarator_list()?;
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::NetDecl(NetDecl {
            attrs, net_type, drive_strength, charge_strength, delay, ty, discipline, range, names,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn opt_drive_or_charge_strength(&mut self, _nt: Option<&NetType>) -> PResult<(Option<DriveStrength>, Option<ChargeStrength>)> {
        // Only parse if `(` is next AND the token after `(` looks like a strength keyword.
        if !self.at(&Tok::LParen) { return Ok((None, None)); }
        // Peek inside: is next token a strength keyword?
        let is_strength = matches!(self.peek_at(1), Some(Tok::Ident(s)) if 
            matches!(s.as_str(), "supply0"|"strong0"|"pull0"|"weak0"|
                                  "supply1"|"strong1"|"pull1"|"weak1"|
                                  "highz0"|"highz1"|"small"|"medium"|"large")
        );
        if !is_strength { return Ok((None, None)); }

        self.expect(&Tok::LParen)?;
        let s0_str = self.ident()?;

        // charge_strength: (small), (medium), (large)
        if matches!(s0_str.as_str(), "small"|"medium"|"large") {
            self.expect(&Tok::RParen)?;
            let cs = match s0_str.as_str() {
                "small" => ChargeStrength::Small,
                "medium" => ChargeStrength::Medium,
                _ => ChargeStrength::Large,
            };
            return Ok((None, Some(cs)));
        }

        // drive_strength: (s0, s1)
        self.expect(&Tok::Comma)?;
        let s1_str = self.ident()?;
        self.expect(&Tok::RParen)?;
        let strength0 = self.parse_strength(&s0_str)?;
        let strength1 = self.parse_strength(&s1_str)?;
        Ok((Some(DriveStrength { strength0, strength1 }), None))
    }

    /// Parse optional `#delay` or `#(expr)` or `#(e,e,e)`.
    fn opt_delay(&mut self) -> PResult<Option<Delay>> {
        if !self.eat(&Tok::Hash) { return Ok(None); }
        if self.eat(&Tok::LParen) {
            let e1 = self.expr()?;
            let e1 = self.opt_mintypmax(e1)?;
            if self.eat(&Tok::Comma) {
                let e2 = self.expr()?;
                let e2 = self.opt_mintypmax(e2)?;
                if self.eat(&Tok::Comma) {
                    let e3 = self.expr()?;
                    let e3 = self.opt_mintypmax(e3)?;
                    self.expect(&Tok::RParen)?;
                    Ok(Some(Delay::Paren3(e1, e2, e3)))
                } else {
                    self.expect(&Tok::RParen)?;
                    Ok(Some(Delay::Paren2(e1, e2)))
                }
            } else {
                self.expect(&Tok::RParen)?;
                Ok(Some(Delay::Paren1(e1)))
            }
        } else {
            Ok(Some(Delay::Single(self.expr()?)))
        }
    }

    fn parse_strength(&self, s: &str) -> PResult<Strength> {
        match s {
            "supply0" => Ok(Strength::Supply0), "strong0" => Ok(Strength::Strong0),
            "pull0"   => Ok(Strength::Pull0),   "weak0"   => Ok(Strength::Weak0),
            "supply1" => Ok(Strength::Supply1), "strong1" => Ok(Strength::Strong1),
            "pull1"   => Ok(Strength::Pull1),   "weak1"   => Ok(Strength::Weak1),
            "highz0"  => Ok(Strength::Highz0),  "highz1"  => Ok(Strength::Highz1),
            other => Err(format!("unknown strength: {other}")),
        }
    }

    pub(super) fn var_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<VarDecl> {
        let ty = if self.eat_kw("genvar") { Type::Integer } else { self.type_()? };
        let signed = self.opt_signed();
        let packed_range = self.parse_range()?;
        let mut vars = Vec::new();
        loop {
            let name = self.name()?;
            let range = self.parse_range()?;
            let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
            vars.push(Var { name, range, default });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(VarDecl { attrs, ty, signed, packed_range, discipline: None, vars, span: Span { start, end: self.prev_end() } })
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
        let signed = self.opt_signed();
        let range = self.parse_range()?;
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
        Ok(ParamDecl { attrs, kind, ty, signed, range, params, span: Span { start, end: self.prev_end() } })
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
        // Optional drive strength
        let (drive_strength, _) = self.opt_drive_or_charge_strength(None)?;
        // Optional delay3
        let delay = self.opt_delay()?;
        loop {
            let lval = self.expr()?;
            self.expect(&Tok::Assign)?;
            let rval = self.expr()?;
            assignments.push((lval, rval));
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::ContinuousAssign(ContinuousAssign { 
            span: Span { start, end: self.prev_end() }, attrs, drive_strength, delay, assignments 
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
        let automatic = self.eat_kw("automatic");
        let ty = if self.is_type_kw() { Some(self.type_()?) } else { None };
        let signed = self.opt_signed();
        let range = self.parse_range()?;
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
                let _signed = self.opt_signed();
                let _range = self.parse_range()?;
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
            attrs, automatic, ty, signed, range, name, items,
            span: Span { start, end: self.prev_end() },
        }))
    }

    fn function_item(&mut self) -> PResult<FunctionItem> {
        let start = self.span_start();
        let attrs = self.attrs()?;
        if self.at_dir() {
            let dir = self.direction()?;
            if self.is_type_kw() { let _ = self.type_()?; } // tolerate `input real x;`
            let _signed = self.opt_signed();
            let _range = self.parse_range()?;
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

    fn task_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let automatic = self.eat_kw("automatic");
        let name = self.name()?;

        // Detect parenthesized port list: `task foo (input real a, ...);`
        let ports = if self.eat(&Tok::LParen) {
            let mut ports = Vec::new();
            while !self.at(&Tok::RParen) && !self.at_end() {
                let port_attrs = self.attrs()?;
                let dir = self.direction()?;
                ports.push(self.task_port_rest(port_attrs, dir)?);
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RParen)?;
            ports
        } else {
            Vec::new()
        };

        self.expect(&Tok::Semi)?;

        // Body: old-style has declarations + final statement.
        // Parenthesized form has block_item_declarations + statement.
        let mut items = Vec::new();
        while !self.at_kw("endtask") && !self.at_end() {
            // Try to parse as task item (direction decl or block item).
            // A direction keyword starts a port decl; otherwise it's a block item.
            if self.at_dir() && ports.is_empty() {
                // Old-style port: `input real x;`
                let item_attrs = self.attrs()?;
                let dir = self.direction()?;
                let port = self.task_port_rest(item_attrs, dir)?;
                self.expect(&Tok::Semi)?;
                items.push(TaskItem::Port(port));
            } else if !self.at_stmt_kw() && (self.is_type_kw() || self.at_kw("parameter") || self.at_kw("localparam")) {
                let item_attrs = self.attrs()?;
                let item_start = self.span_start();
                if self.at_kw("parameter") || self.at_kw("localparam") {
                    items.push(TaskItem::BlockItem(BlockItem::ParamDecl(self.param_decl(item_attrs, item_start)?)));
                } else {
                    items.push(TaskItem::BlockItem(BlockItem::VarDecl(self.var_decl(item_attrs, item_start)?)));
                }
            } else {
                // Last item is the body statement.
                // Peek: is this the final statement before endtask?
                // We collect all statements as TaskItem::BlockItem(Stmt).
                let s = self.stmt()?;
                items.push(TaskItem::BlockItem(BlockItem::Stmt(s)));
            }
        }
        self.expect_kw("endtask")?;

        // Split items: last BlockItem::Stmt is the body; everything before is task items.
        // If no stmt items found, use Empty stmt as body.
        let body = {
            let last_stmt = items.iter().rposition(|i| matches!(i, TaskItem::BlockItem(BlockItem::Stmt(_))));
            if let Some(idx) = last_stmt {
                if let TaskItem::BlockItem(BlockItem::Stmt(s)) = items.remove(idx) {
                    Box::new(s)
                } else { unreachable!() }
            } else {
                Box::new(Stmt::Empty(EmptyStmt { attrs: vec![] }))
            }
        };

        Ok(ModuleItem::TaskDecl(TaskDecl {
            span: Span { start, end: self.prev_end() },
            attrs, automatic, name, ports, items, body,
        }))
    }

    fn task_port_rest(&mut self, attrs: Vec<Attr>, dir: Direction) -> PResult<TaskPort> {
        // Optional: `task_port_type` (integer|real|realtime|time) OR `reg`
        let mut port_type = None;
        let mut reg = false;
        if self.at_any_kw(&["integer", "real", "realtime", "time"]) {
            port_type = Some(self.type_()?);
        } else {
            reg = self.eat_kw("reg");
        }
        let discipline = self.opt_discipline();
        let signed = self.opt_signed();
        let range = self.parse_range()?;
        let names = self.name_list()?;
        Ok(TaskPort { attrs, dir, port_type, discipline, reg, signed, range, names })
    }

    fn specparam_decl(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let range = self.parse_range()?;
        let mut assignments = Vec::new();
        loop {
            let name = self.name()?;
            self.expect(&Tok::Assign)?;
            let expr = self.expr()?;
            // consume optional `:typ:max` mintypmax suffix
            let expr = self.opt_mintypmax(expr)?;
            assignments.push((name, expr));
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::Specparam(SpecparamDecl {
            span: Span { start, end: self.prev_end() },
            attrs, range, assignments,
        }))
    }

    fn specify_block(&mut self, _attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        // Skip contents: track paren depth to correctly skip nested path
        // expressions like `(posedge clk => (q:data))`.
        let mut item_count = 0;
        let mut depth = 0usize;
        while !self.at_end() {
            if self.at_kw("endspecify") && depth == 0 { break; }
            match self.peek() {
                Some(Tok::LParen) => { depth += 1; self.bump(); }
                Some(Tok::RParen) => { if depth > 0 { depth -= 1; } self.bump(); }
                Some(Tok::Semi) => { item_count += 1; self.bump(); }
                _ => { self.bump(); }
            }
        }
        self.expect_kw("endspecify")?;
        Ok(ModuleItem::Specify(SpecifyBlock { span: Span { start, end: self.prev_end() }, item_count }))
    }

    fn gate_instantiation(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
        let gate_type = self.name()?;  // consumes the gate keyword (it's an Ident)

        if self.eat(&Tok::Hash) {
            if self.eat(&Tok::LParen) {
                let mut depth = 1;
                while depth > 0 && !self.at_end() {
                    match self.peek() {
                        Some(Tok::LParen) => { depth += 1; self.bump(); }
                        Some(Tok::RParen) => { depth -= 1; self.bump(); }
                        _ => { self.bump(); }
                    }
                }
            } else {
                // #scalar_delay
                self.expr()?;
            }
        }

        let mut instances = Vec::new();
        loop {
            // Optional instance name
            let name = if matches!(self.peek(), Some(Tok::Ident(_))) {
                // Has a name: `g1 (...)` or `g1 [range] (...)`
                let n = self.name()?;
                let r = self.parse_range()?;
                Some((n, r))
            } else {
                None
            };
            self.expect(&Tok::LParen)?;
            let mut terminals = Vec::new();
            while !self.at(&Tok::RParen) && !self.at_end() {
                if self.at(&Tok::Comma) {
                    terminals.push(None);  // empty terminal (positional gap)
                } else {
                    terminals.push(Some(self.expr()?));
                }
                if !self.eat(&Tok::Comma) { break; }
            }
            self.expect(&Tok::RParen)?;
            instances.push(GateInstance { name, terminals });
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::Semi)?;
        Ok(ModuleItem::GateInstantiation(GateInstantiation {
            span: Span { start, end: self.prev_end() },
            attrs, gate_type, instances,
        }))
    }
}

//! Declaration grammar — natures, disciplines, modules and their core
//! surface (ports, nets, parameters), LRM Annex A.1.2/A.1.3/A.1.5/A.1.6,
//! A.2.1.1/A.2.1.3.
//!
//! `parse_module_item` is the module-body dispatcher; each task that adds
//! more declaration kinds (T10, T11) extends its `match` with new arms —
//! the method itself, and this file, are *extended* across those tasks by
//! design (declaration-family seams), never duplicated.

use crate::ast::*;
use crate::error::{ParseError, Span};
use crate::grammar::cursor::Grammar;
use crate::lexer::Tok;

/// True when `t` is one of the three port-direction keywords — used to
/// resolve the port-list comma ambiguity (§Syntax 2-6): a comma followed by
/// one of these starts a new `port_declaration` rather than continuing the
/// current one's name list.
fn is_direction_kw(t: &Tok) -> bool {
    matches!(t, Tok::Ident(s) if s == "input" || s == "output" || s == "inout")
}

/// True when `t` is `parameter`/`localparam` — the same comma ambiguity as
/// [`is_direction_kw`], for a `#(...)` parameter port list.
fn is_param_kw(t: &Tok) -> bool {
    matches!(t, Tok::Ident(s) if s == "parameter" || s == "localparam")
}

impl Grammar {
    // ───────────────────────────── nature / discipline ─────────────────

    /// `nature_declaration`, Annex A.1.6 — §3.6.1. Not attribute-bearing
    /// (absent from Syntax 2-5..2-10).
    pub fn parse_nature_decl(&mut self) -> Result<NatureDecl, ParseError> {
        let start = self.expect_kw("nature")?;
        let (name, _) = self.expect_ident()?;
        let parent = if self.eat_punct(":") { Some(self.expect_ident()?.0) } else { None };
        self.eat_punct(";");

        let mut n = NatureDecl {
            span: start,
            name,
            parent,
            abstol: None,
            access: None,
            units: None,
            idt_nature: None,
            ddt_nature: None,
            user_attrs: vec![],
        };
        while !self.at_kw("endnature") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'endnature'".to_string()));
            }
            let (attr_name, _) = self.expect_ident()?;
            self.expect_punct("=")?;
            match attr_name.as_str() {
                "abstol" => n.abstol = Some(self.parse_expr()?),
                "units" => n.units = Some(self.parse_expr()?),
                "access" => n.access = Some(self.expect_ident()?.0),
                "idt_nature" => n.idt_nature = Some(self.expect_ident()?.0),
                "ddt_nature" => n.ddt_nature = Some(self.expect_ident()?.0),
                _ => n.user_attrs.push((attr_name, self.parse_expr()?)),
            }
            self.expect_punct(";")?;
        }
        let end = self.expect_kw("endnature")?;
        n.span = Span::new(start.start, end.end);
        Ok(n)
    }

    /// `discipline_declaration`, Annex A.1.5 — §3.6.2. Not attribute-bearing.
    pub fn parse_discipline_decl(&mut self) -> Result<DisciplineDecl, ParseError> {
        let start = self.expect_kw("discipline")?;
        let (name, _) = self.expect_ident()?;
        self.eat_punct(";");

        let mut d = DisciplineDecl { span: start, name, domain: None, potential: None, flow: None };
        while !self.at_kw("enddiscipline") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'enddiscipline'".to_string()));
            }
            if self.eat_kw("domain") {
                if self.eat_kw("continuous") {
                    d.domain = Some(Domain::Continuous);
                } else {
                    self.expect_kw("discrete")?;
                    d.domain = Some(Domain::Discrete);
                }
                self.expect_punct(";")?;
                continue;
            }
            if self.eat_kw("potential") {
                d.potential = Some(self.expect_ident()?.0);
                self.expect_punct(";")?;
                continue;
            }
            if self.eat_kw("flow") {
                d.flow = Some(self.expect_ident()?.0);
                self.expect_punct(";")?;
                continue;
            }
            return Err(self.unexpected("'domain', 'potential', or 'flow'".to_string()));
        }
        let end = self.expect_kw("enddiscipline")?;
        d.span = Span::new(start.start, end.end);
        Ok(d)
    }

    // ───────────────────────────────── module ───────────────────────────

    /// `module_declaration`, Annex A.1.2 — §Syntax 2-5 (attribute-bearing).
    pub fn parse_module_decl(&mut self) -> Result<ModuleDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let (kind, start) = if self.at_kw("module") {
            (ModuleKind::Module, self.bump().span)
        } else if self.at_kw("macromodule") {
            (ModuleKind::Macromodule, self.bump().span)
        } else {
            (ModuleKind::Connectmodule, self.expect_kw("connectmodule")?)
        };
        let (name, _) = self.expect_ident()?;

        let param_ports = if self.at_punct("#") {
            self.bump();
            self.expect_punct("(")?;
            let mut params = vec![self.parse_param_decl_body(true)?];
            while self.eat_punct(",") {
                params.push(self.parse_param_decl_body(true)?);
            }
            self.expect_punct(")")?;
            params
        } else {
            vec![]
        };

        self.expect_punct("(")?;
        // Declaration-list style (`(input electrical a, ...)`) vs name-list
        // style (`(a, b)`, with `input`/`output`/`inout` re-declared in the
        // body) — distinguished by whether the first port token is a
        // direction keyword.
        let is_decl_style =
            self.at_kw("input") || self.at_kw("output") || self.at_kw("inout") || self.at_punct(")");
        let ports = if is_decl_style {
            let mut ports = vec![];
            if !self.at_punct(")") {
                ports.push(self.parse_port_decl()?);
                while self.eat_punct(",") {
                    ports.push(self.parse_port_decl()?);
                }
            }
            self.expect_punct(")")?;
            Some(ports)
        } else {
            // Name-list style: skip the bare names here — the body's own
            // `input`/`output`/`inout` re-declarations (parsed as
            // `ModuleItem::Port`) are the authoritative port list.
            self.expect_ident()?;
            while self.eat_punct(",") {
                self.expect_ident()?;
            }
            self.expect_punct(")")?;
            None
        };
        self.expect_punct(";")?;

        let mut items = vec![];
        while !self.at_kw("endmodule") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'endmodule'".to_string()));
            }
            items.push(self.parse_module_item()?);
        }
        let end = self.expect_kw("endmodule")?;
        Ok(ModuleDecl {
            span: Span::new(start.start, end.end),
            attrs,
            kind,
            name,
            param_ports,
            ports,
            items,
        })
    }

    /// One item inside a module body — Annex A.1.4. This match is extended
    /// by T10/T11; unrecognized starts fall through to a typed error
    /// naming what was expected.
    pub(crate) fn parse_module_item(&mut self) -> Result<ModuleItem, ParseError> {
        // Dispatch looks past any leading `(* ... *)` attribute instances
        // (§Syntax 2-7) so `(* attr *) branch ...` routes the same as a
        // bare `branch ...` — the chosen production below still consumes
        // the attributes itself via its own `parse_attr_instances()` call.
        let kw = self.kw_after_attrs();
        if kw == Some("input") || kw == Some("output") || kw == Some("inout") {
            return Ok(ModuleItem::Port(self.parse_port_decl()?));
        }
        if kw == Some("ground") {
            return Ok(ModuleItem::Ground(self.parse_ground_decl()?));
        }
        if kw == Some("parameter") || kw == Some("localparam") {
            return Ok(ModuleItem::Param(self.parse_param_decl_body(false)?));
        }
        if kw == Some("aliasparam") {
            return Ok(ModuleItem::AliasParam(self.parse_aliasparam()?));
        }
        if kw == Some("specparam") {
            return Ok(ModuleItem::Specparam(self.parse_specparam()?));
        }
        if kw == Some("branch") {
            return Ok(ModuleItem::Branch(self.parse_branch_decl()?));
        }
        if kw == Some("analog") {
            // `analog function` is a callable declaration; bare `analog`
            // (optionally `analog initial`) is a behavior construct — one
            // more token of lookahead tells them apart.
            if matches!(self.peek_at_next(), Tok::Ident(s) if s == "function") {
                return Ok(ModuleItem::Function(self.parse_function_decl()?));
            }
            return Ok(ModuleItem::Behavior(self.parse_behavior_construct()?));
        }
        if kw == Some("initial") || kw == Some("always") {
            return Ok(ModuleItem::Behavior(self.parse_behavior_construct()?));
        }
        if kw == Some("function") {
            return Ok(ModuleItem::Function(self.parse_function_decl()?));
        }
        if kw == Some("task") {
            return Ok(ModuleItem::Task(self.parse_task_decl()?));
        }
        if kw == Some("genvar") {
            return Ok(ModuleItem::Genvar(self.parse_genvar_decl()?));
        }
        if kw == Some("for") {
            return Ok(ModuleItem::LoopGenerate(self.parse_loop_generate()?));
        }
        if kw == Some("if") {
            return Ok(ModuleItem::IfGenerate(self.parse_if_generate()?));
        }
        if kw == Some("case") {
            return Ok(ModuleItem::CaseGenerate(self.parse_case_generate()?));
        }
        if kw == Some("generate") {
            return Ok(ModuleItem::GenerateRegion(self.parse_generate_region()?));
        }
        if kw == Some("defparam") {
            return Ok(ModuleItem::Defparam(self.parse_defparam()?));
        }
        if kw == Some("assign") {
            return Ok(ModuleItem::ContinuousAssign(self.parse_continuous_assign()?));
        }
        // A bare identifier is either a net declaration (`discipline_name
        // declarator_list ';'`) or a module instantiation (`module_name
        // [#(...)] instance_name(...)`, possibly repeated). Both start
        // with `Ident`, so the dispatch needs lookahead: `#` or a second
        // identifier immediately followed by `(` means instantiation;
        // anything else (`,`/`;`/`[`) means the first identifier was
        // already the sole declarator's discipline.
        if kw.is_some() {
            let looks_like_instantiation = matches!(self.peek_at(1), Tok::Punct("#"))
                || (matches!(self.peek_at(1), Tok::Ident(_)) && matches!(self.peek_at(2), Tok::Punct("(")));
            if looks_like_instantiation {
                return Ok(ModuleItem::Instantiation(self.parse_module_instantiation()?));
            }
            return Ok(ModuleItem::Net(self.parse_net_decl()?));
        }
        Err(self.unexpected(
            "a module item declaration (port, net, ground, parameter, branch, behavior, \
             function, task, generate, instantiation, defparam, or continuous assign)"
                .to_string(),
        ))
    }

    // ───────────────────────────────── ports / nets ─────────────────────

    /// `port_declaration`, Annex A.1.3 — §Syntax 2-6 (attribute-bearing).
    fn parse_port_decl(&mut self) -> Result<PortDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let dir = if self.eat_kw("input") {
            Direction::Input
        } else if self.eat_kw("output") {
            Direction::Output
        } else {
            self.expect_kw("inout")?;
            Direction::Inout
        };
        // An optional discipline name precedes the port name list; if the
        // next identifier is immediately followed by `,`/`;`/`)` it *is*
        // the first port name (no discipline given), not a discipline.
        let discipline = self.parse_optional_discipline_name();
        // A comma inside a port list is ambiguous: it either separates two
        // names under this *same* direction (`inout electrical p, n`) or
        // ends this port_declaration and starts the next one
        // (`inout electrical p, inout electrical n`). Only the former
        // consumes the comma here — a comma immediately followed by a
        // direction keyword belongs to the outer port list.
        let mut names = vec![self.parse_declarator()?];
        while self.at_punct(",") && !is_direction_kw(self.peek_at_next()) {
            self.bump();
            names.push(self.parse_declarator()?);
        }
        let end = self.peek_span();
        self.eat_punct(";");
        Ok(PortDecl { span: Span::new(start.start, end.start), attrs, dir, discipline, names })
    }

    /// `net_declaration`, §3.6.3/3.7 — a bare `discipline_identifier
    /// declarator_list ';'`, optionally followed by a nodeset value
    /// (§3.6.3.2: `net_discipline_identifier name <initial_value>;`).
    fn parse_net_decl(&mut self) -> Result<NetDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let (discipline, _) = self.expect_ident()?;
        let names = self.parse_declarator_list()?;
        let nodeset = if self.eat_punct("=") { Some(self.parse_expr()?) } else { None };
        self.expect_punct(";")?;
        Ok(NetDecl {
            span: Span::new(start.start, start.end),
            attrs,
            discipline: Some(discipline),
            names,
            nodeset,
        })
    }

    /// `ground_declaration`, §3.6.4.
    fn parse_ground_decl(&mut self) -> Result<GroundDecl, ParseError> {
        let start = self.expect_kw("ground")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct(";")?;
        Ok(GroundDecl { span: start, name })
    }

    /// A discipline name where present, distinguished from "no discipline,
    /// this identifier is already the first declarator" by lookahead: a
    /// discipline name is followed by another identifier (the first net
    /// name), never directly by `,`/`;`/`)`/`[`/`='`.
    fn parse_optional_discipline_name(&mut self) -> Option<String> {
        if let Tok::Ident(_) = self.peek() {
            if matches!(self.peek_at_next(), Tok::Ident(_)) {
                return Some(self.expect_ident().ok()?.0);
            }
        }
        None
    }

    /// `declarator { ',' declarator }` — Annex A.2.3 `list_of_port_identifiers`
    /// generalized to also cover the range form used by nets/ports alike.
    fn parse_declarator_list(&mut self) -> Result<Vec<Declarator>, ParseError> {
        let mut names = vec![self.parse_declarator()?];
        while self.eat_punct(",") {
            names.push(self.parse_declarator()?);
        }
        Ok(names)
    }

    fn parse_declarator(&mut self) -> Result<Declarator, ParseError> {
        let (name, _) = self.expect_ident()?;
        let range = if self.eat_punct("[") {
            let msb = self.parse_expr()?;
            self.expect_punct(":")?;
            let lsb = self.parse_expr()?;
            let end = self.expect_punct("]")?;
            Some(Range { span: end, msb, lsb })
        } else {
            None
        };
        Ok(Declarator { name, range })
    }

    // ───────────────────────────────── parameters ───────────────────────

    /// `parameter_declaration`/`local_parameter_declaration`, §3.4,
    /// Annex A.2.1.1 — §Syntax 2-7 (attribute-bearing; `in_port_list`
    /// suppresses the trailing `;` for the `#(...)` port-list form).
    fn parse_param_decl_body(&mut self, in_port_list: bool) -> Result<ParamDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let is_local = if self.eat_kw("localparam") {
            true
        } else {
            self.expect_kw("parameter")?;
            false
        };
        let ty = self.parse_optional_param_type();

        let mut names = vec![self.parse_param_assignment()?];
        let mut ranges = self.parse_param_ranges()?;
        // Same comma ambiguity as the port list: `parameter real a=1, b=2`
        // continues this declaration, but `#(parameter real a=1, parameter
        // real b=2)` starts a new one at the second `parameter`.
        while self.at_punct(",") && !is_param_kw(self.peek_at_next()) {
            self.bump();
            names.push(self.parse_param_assignment()?);
            ranges.extend(self.parse_param_ranges()?);
        }
        if !in_port_list {
            self.expect_punct(";")?;
        }
        Ok(ParamDecl { span: start, attrs, is_local, ty, names, ranges })
    }

    fn parse_optional_param_type(&mut self) -> Option<ParamType> {
        if self.eat_kw("integer") {
            Some(ParamType::Integer)
        } else if self.eat_kw("real") {
            Some(ParamType::Real)
        } else if self.eat_kw("realtime") {
            Some(ParamType::Realtime)
        } else if self.eat_kw("time") {
            Some(ParamType::Time)
        } else if self.eat_kw("string") {
            Some(ParamType::String)
        } else {
            None
        }
    }

    fn parse_param_assignment(&mut self) -> Result<(String, Expr), ParseError> {
        let (name, _) = self.expect_ident()?;
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        Ok((name, value))
    }

    /// `{ value_range }`, §3.4.2 — `from range` / `exclude range`, where
    /// `range` is `[ | (` value `:` value `] | )` (inclusive/exclusive
    /// bounds) or a bare excluded value.
    fn parse_param_ranges(&mut self) -> Result<Vec<ParamRange>, ParseError> {
        let mut ranges = vec![];
        loop {
            let excluded = if self.eat_kw("from") {
                false
            } else if self.eat_kw("exclude") {
                true
            } else {
                break;
            };
            let start = self.peek_span();
            if !self.at_punct("[") && !self.at_punct("(") {
                // `exclude value` — a single excluded value, not a
                // `[lo:hi]` range (only legal on `exclude`, §3.4.2).
                let value = self.parse_expr()?;
                ranges.push(ParamRange {
                    span: start,
                    excluded,
                    lo: Some(value),
                    lo_exclusive: false,
                    hi: None,
                    hi_exclusive: false,
                });
                continue;
            }
            let lo_exclusive = self.eat_punct("(");
            if !lo_exclusive {
                self.expect_punct("[")?;
            }
            let lo = if self.at_punct(":") { None } else { Some(self.parse_expr()?) };
            self.expect_punct(":")?;
            let hi = if self.at_punct("]") || self.at_punct(")") { None } else { Some(self.parse_expr()?) };
            let hi_exclusive = self.eat_punct(")");
            if !hi_exclusive {
                self.expect_punct("]")?;
            }
            ranges.push(ParamRange { span: start, excluded, lo, lo_exclusive, hi, hi_exclusive });
        }
        Ok(ranges)
    }

    /// `aliasparam_declaration`, §3.4.7.
    fn parse_aliasparam(&mut self) -> Result<AliasParam, ParseError> {
        let start = self.expect_kw("aliasparam")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct("=")?;
        let (target, _) = self.expect_ident()?;
        let end = self.expect_punct(";")?;
        Ok(AliasParam { span: Span::new(start.start, end.end), name, target })
    }

    /// `specparam_declaration`, inherited from 1364 — §Syntax 2-7
    /// (attribute-bearing).
    fn parse_specparam(&mut self) -> Result<SpecparamDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("specparam")?;
        let mut names = vec![self.parse_param_assignment()?];
        while self.eat_punct(",") {
            names.push(self.parse_param_assignment()?);
        }
        let end = self.expect_punct(";")?;
        Ok(SpecparamDecl { span: Span::new(start.start, end.end), attrs, names })
    }

    /// `hierarchical_identifier`, Annex A.9.3 — `ident {'.' ident}`, joined
    /// with `.` into one string (`defparam`'s target is the only site this
    /// crate's scope needs it for).
    fn parse_hierarchical_name(&mut self) -> Result<String, ParseError> {
        let mut name = self.expect_ident()?.0;
        while self.eat_punct(".") {
            name.push('.');
            name.push_str(&self.expect_ident()?.0);
        }
        Ok(name)
    }

    // ─────────────────────────────── branch ──────────────────────────────

    /// `branch_declaration`, §5.2 — `branch (pos[,neg]) name {, name};`.
    /// Attribute-bearing.
    fn parse_branch_decl(&mut self) -> Result<BranchDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("branch")?;
        self.expect_punct("(")?;
        let (pos, _) = self.expect_ident()?;
        let neg = if self.eat_punct(",") { Some(self.expect_ident()?.0) } else { None };
        self.expect_punct(")")?;
        let mut names = vec![self.expect_ident()?.0];
        while self.eat_punct(",") {
            names.push(self.expect_ident()?.0);
        }
        let end = self.expect_punct(";")?;
        Ok(BranchDecl { span: Span::new(start.start, end.end), attrs, pos, neg, names })
    }

    // ─────────────────────────── behavior constructs ──────────────────────

    /// `analog_construct` / `initial_construct` / `always_construct` —
    /// §5.7/Annex A.1.4. Attribute-bearing, §Syntax 2-7. `analog`'s body
    /// and digital `initial`/`always`'s body share `parse_stmt` — the
    /// Annex C analog-subset restriction on which statements are legal
    /// inside which is enforced later (VAMS-25), not by this production.
    fn parse_behavior_construct(&mut self) -> Result<BehaviorConstruct, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let kind = if self.eat_kw("analog") {
            if self.eat_kw("initial") {
                BehaviorKind::AnalogInitial
            } else {
                BehaviorKind::Analog
            }
        } else if self.eat_kw("initial") {
            BehaviorKind::Initial
        } else {
            self.expect_kw("always")?;
            BehaviorKind::Always
        };
        let body = Box::new(self.parse_stmt()?);
        Ok(BehaviorConstruct { span: Span::new(start.start, body.span().end), attrs, kind, body })
    }

    // ───────────────────────────── functions / tasks ───────────────────────

    /// `port_declaration`-shaped `tf_port_declaration`, Annex A.2.6 — a
    /// function/task's formal port, declared as a body item (not a
    /// parenthesized list — 1364/Verilog-AMS functions and tasks declare
    /// ports this way). Attribute-bearing, §Syntax 2-8.
    fn parse_port_item_decl(&mut self) -> Result<PortItemDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let dir = if self.eat_kw("input") {
            Direction::Input
        } else if self.eat_kw("output") {
            Direction::Output
        } else {
            self.expect_kw("inout")?;
            Direction::Inout
        };
        let ty = self.parse_optional_param_type();
        let mut names = vec![self.expect_ident()?.0];
        while self.eat_punct(",") {
            names.push(self.expect_ident()?.0);
        }
        let end = self.expect_punct(";")?;
        Ok(PortItemDecl { span: Span::new(start.start, end.end), attrs, dir, ty, names })
    }

    /// `analog_function_declaration` / `function_declaration`, §5.9/
    /// Annex A.2.6. Attribute-bearing, §Syntax 2-7.
    fn parse_function_decl(&mut self) -> Result<FunctionDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let kind = if self.eat_kw("analog") {
            self.expect_kw("function")?;
            FunctionKind::Analog
        } else {
            self.expect_kw("function")?;
            FunctionKind::Digital
        };
        let ret = self.parse_optional_param_type();
        let (name, _) = self.expect_ident()?;
        self.expect_punct(";")?;
        let mut ports = vec![];
        while self.at_kw("input") || self.at_kw("output") || self.at_kw("inout") {
            ports.push(self.parse_port_item_decl()?);
        }
        let mut decls = vec![];
        while let Some(d) = self.try_parse_block_item_decl()? {
            decls.push(d);
        }
        let body = Box::new(self.parse_stmt()?);
        let end = self.expect_kw("endfunction")?;
        Ok(FunctionDecl { span: Span::new(start.start, end.end), attrs, kind, name, ret, ports, decls, body })
    }

    /// `task_declaration`, Annex A.2.7. Attribute-bearing, §Syntax 2-7.
    fn parse_task_decl(&mut self) -> Result<TaskDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("task")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct(";")?;
        let mut ports = vec![];
        while self.at_kw("input") || self.at_kw("output") || self.at_kw("inout") {
            ports.push(self.parse_port_item_decl()?);
        }
        let mut decls = vec![];
        while let Some(d) = self.try_parse_block_item_decl()? {
            decls.push(d);
        }
        let body = Box::new(self.parse_stmt()?);
        let end = self.expect_kw("endtask")?;
        Ok(TaskDecl { span: Span::new(start.start, end.end), attrs, name, ports, decls, body })
    }

    // ────────────────────────────── genvar / generate ──────────────────────

    /// `genvar_declaration`, §6, Annex A.2.1.3. Attribute-bearing.
    fn parse_genvar_decl(&mut self) -> Result<GenvarDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("genvar")?;
        let mut names = vec![self.expect_ident()?.0];
        while self.eat_punct(",") {
            names.push(self.expect_ident()?.0);
        }
        let end = self.expect_punct(";")?;
        Ok(GenvarDecl { span: Span::new(start.start, end.end), attrs, names })
    }

    /// `generate_block`, §6 — `begin [':' name] module_or_generate_item*
    /// end`, or (when unnamed and holding exactly one item) a single bare
    /// item with no `begin`/`end` wrapper.
    fn parse_generate_block(&mut self) -> Result<GenerateBlock, ParseError> {
        if self.at_kw("begin") {
            let start = self.expect_kw("begin")?;
            let name = if self.eat_punct(":") { Some(self.expect_ident()?.0) } else { None };
            let mut items = vec![];
            while !self.at_kw("end") {
                if matches!(self.peek(), Tok::Eof) {
                    return Err(self.eof_error("'end'".to_string()));
                }
                items.push(self.parse_module_item()?);
            }
            let end = self.expect_kw("end")?;
            return Ok(GenerateBlock { span: Span::new(start.start, end.end), name, items });
        }
        let item = self.parse_module_item()?;
        Ok(GenerateBlock { span: self.peek_span(), name: None, items: vec![item] })
    }

    /// `loop_generate_construct`, §6.2. Attribute-bearing, §Syntax 2-7.
    fn parse_loop_generate(&mut self) -> Result<LoopGenerate, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("for")?;
        self.expect_punct("(")?;
        let (genvar, _) = self.expect_ident()?;
        self.expect_punct("=")?;
        let init = self.parse_expr()?;
        self.expect_punct(";")?;
        let cond = self.parse_expr()?;
        self.expect_punct(";")?;
        let (step_var, _) = self.expect_ident()?;
        self.expect_punct("=")?;
        let step_value = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = self.parse_generate_block()?;
        Ok(LoopGenerate {
            span: Span::new(start.start, body.span.end),
            attrs,
            genvar,
            init,
            cond,
            step_var,
            step_value,
            body,
        })
    }

    /// `conditional_generate_construct` (if-form), §6.3. Attribute-bearing.
    fn parse_if_generate(&mut self) -> Result<IfGenerate, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("if")?;
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let then_branch = self.parse_generate_block()?;
        let else_branch = if self.eat_kw("else") { Some(self.parse_generate_block()?) } else { None };
        let end = else_branch.as_ref().map(|b| b.span).unwrap_or(then_branch.span);
        Ok(IfGenerate { span: Span::new(start.start, end.end), attrs, cond, then_branch, else_branch })
    }

    /// `conditional_generate_construct` (case-form), §6.4. Attribute-bearing.
    fn parse_case_generate(&mut self) -> Result<CaseGenerate, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("case")?;
        self.expect_punct("(")?;
        let selector = self.parse_expr()?;
        self.expect_punct(")")?;
        let mut arms = vec![];
        while !self.at_kw("endcase") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'endcase'".to_string()));
            }
            let arm_start = self.peek_span();
            let labels = if self.eat_kw("default") {
                self.eat_punct(":");
                None
            } else {
                let mut labels = vec![self.parse_expr()?];
                while self.eat_punct(",") {
                    labels.push(self.parse_expr()?);
                }
                self.expect_punct(":")?;
                Some(labels)
            };
            let body = self.parse_generate_block()?;
            arms.push(GenerateCaseArm { span: Span::new(arm_start.start, body.span.end), labels, body });
        }
        let end = self.expect_kw("endcase")?;
        Ok(CaseGenerate { span: Span::new(start.start, end.end), attrs, selector, arms })
    }

    /// `generate_region`, §6.1 — a bare `generate ... endgenerate` with no
    /// loop/conditional.
    fn parse_generate_region(&mut self) -> Result<GenerateRegion, ParseError> {
        let start = self.expect_kw("generate")?;
        let mut items = vec![];
        while !self.at_kw("endgenerate") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'endgenerate'".to_string()));
            }
            items.push(self.parse_module_item()?);
        }
        let end = self.expect_kw("endgenerate")?;
        Ok(GenerateRegion { span: Span::new(start.start, end.end), items })
    }

    // ─────────────────────── instantiation / assign / defparam ─────────────

    /// `defparam_statement`, Annex A.2.1.2. Attribute-bearing.
    fn parse_defparam(&mut self) -> Result<Defparam, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("defparam")?;
        let target = self.parse_hierarchical_name()?;
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        let end = self.expect_punct(";")?;
        Ok(Defparam { span: Span::new(start.start, end.end), attrs, target, value })
    }

    /// `continuous_assign`, Annex A.6.1. Attribute-bearing, §Syntax 2-7.
    fn parse_continuous_assign(&mut self) -> Result<ContinuousAssign, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("assign")?;
        let target = self.parse_lvalue()?;
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        let end = self.expect_punct(";")?;
        Ok(ContinuousAssign { span: Span::new(start.start, end.end), attrs, target, value })
    }

    /// One port/parameter connection at an instantiation site — ordered or
    /// named (`.port(expr)`). Attribute-bearing, §Syntax 2-9.
    fn parse_port_connection(&mut self) -> Result<PortConnection, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        if self.eat_punct(".") {
            let (name, _) = self.expect_ident()?;
            self.expect_punct("(")?;
            let value = if self.at_punct(")") { None } else { Some(self.parse_expr()?) };
            let end = self.expect_punct(")")?;
            return Ok(PortConnection { span: Span::new(start.start, end.end), attrs, name: Some(name), value });
        }
        let value = if self.at_punct(",") || self.at_punct(")") { None } else { Some(self.parse_expr()?) };
        let end = value.as_ref().map(|v| v.span()).unwrap_or(start);
        Ok(PortConnection { span: Span::new(start.start, end.end), attrs, name: None, value })
    }

    fn parse_instance(&mut self) -> Result<Instance, ParseError> {
        let start = self.peek_span();
        let (name, _) = self.expect_ident()?;
        self.expect_punct("(")?;
        let mut connections = vec![];
        if !self.at_punct(")") {
            connections.push(self.parse_port_connection()?);
            while self.eat_punct(",") {
                connections.push(self.parse_port_connection()?);
            }
        }
        let end = self.expect_punct(")")?;
        Ok(Instance { span: Span::new(start.start, end.end), name, connections })
    }

    /// `module_instantiation`, Annex A.4.1. Attribute-bearing.
    fn parse_module_instantiation(&mut self) -> Result<ModuleInstantiation, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let (module_name, _) = self.expect_ident()?;
        let param_overrides = if self.eat_punct("#") {
            self.expect_punct("(")?;
            let mut po = vec![];
            if !self.at_punct(")") {
                po.push(self.parse_port_connection()?);
                while self.eat_punct(",") {
                    po.push(self.parse_port_connection()?);
                }
            }
            self.expect_punct(")")?;
            po
        } else {
            vec![]
        };
        let mut instances = vec![self.parse_instance()?];
        while self.eat_punct(",") {
            instances.push(self.parse_instance()?);
        }
        let end = self.expect_punct(";")?;
        Ok(ModuleInstantiation {
            span: Span::new(start.start, end.end),
            attrs,
            module_name,
            param_overrides,
            instances,
        })
    }
}

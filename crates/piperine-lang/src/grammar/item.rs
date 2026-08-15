//! Declaration grammar — natures, disciplines, modules and their core
//! surface (ports, nets, parameters), LRM Annex A.1.2/A.1.3/A.1.5/A.1.6,
//! A.2.1.1/A.2.1.3.
//!
//! `parse_module_item` is the module-body dispatcher; each task that adds
//! more declaration kinds (T10, T11) extends its `match` with new arms —
//! the method itself, and this file, are *extended* across those tasks by
//! design (declaration-family seams), never duplicated.

use crate::ast::*;
use crate::error::{ParseError, ParseErrorKind, Span};
use crate::grammar::cursor::Grammar;
use crate::lexer::{NumberLit, Tok};

/// Parse-time-only result of one `connect ...;` statement inside a
/// `connectrules` block — [`Grammar::parse_connect_item`] returns this;
/// [`Grammar::parse_connectrules_decl`] sorts it into
/// [`ConnectrulesDecl`]'s `rules`/`resolutions` list.
enum ConnectItem {
    Rule(ConnectRule),
    Resolution(ConnectResolution),
}

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

/// Every IEEE-1364 gate-primitive keyword and the `GateKind` it names —
/// Annex A.4.2. A data table (MD-13 rule 5), not a `match` cascade.
const GATE_KEYWORDS: &[(&str, GateKind)] = &[
    ("and", GateKind::And), ("nand", GateKind::Nand), ("or", GateKind::Or), ("nor", GateKind::Nor),
    ("xor", GateKind::Xor), ("xnor", GateKind::Xnor), ("buf", GateKind::Buf), ("not", GateKind::Not),
    ("bufif0", GateKind::Bufif0), ("bufif1", GateKind::Bufif1),
    ("notif0", GateKind::Notif0), ("notif1", GateKind::Notif1),
    ("nmos", GateKind::Nmos), ("pmos", GateKind::Pmos), ("cmos", GateKind::Cmos),
    ("rnmos", GateKind::Rnmos), ("rpmos", GateKind::Rpmos), ("rcmos", GateKind::Rcmos),
    ("pullup", GateKind::Pullup), ("pulldown", GateKind::Pulldown),
    ("tran", GateKind::Tran), ("rtran", GateKind::Rtran),
    ("tranif0", GateKind::Tranif0), ("tranif1", GateKind::Tranif1),
    ("rtranif0", GateKind::Rtranif0), ("rtranif1", GateKind::Rtranif1),
];

fn gate_kind_for(word: &str) -> Option<GateKind> {
    GATE_KEYWORDS.iter().find(|(w, _)| *w == word).map(|(_, k)| *k)
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

    /// `discipline_declaration`, Annex A.1.7 — §3.6.2. Not attribute-bearing.
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
        if kw == Some("event") {
            return Ok(ModuleItem::Event(self.parse_event_decl()?));
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
        if kw == Some("specify") {
            return Ok(ModuleItem::Specify(self.parse_specify_block()?));
        }
        if matches!(kw, Some("integer") | Some("real") | Some("realtime") | Some("time")) {
            return Ok(ModuleItem::Var(self.parse_var_decl()?));
        }
        if kw.and_then(gate_kind_for).is_some() {
            return Ok(ModuleItem::Gate(self.parse_gate_instantiation()?));
        }
        // A bare identifier is either a net declaration (`discipline_name
        // declarator_list ';'`) or a module instantiation (`module_name
        // [#(...)] instance_name(...)`, possibly repeated). Both start
        // with `Ident`, so the dispatch needs lookahead: `#` or a second
        // identifier immediately followed by `(` means instantiation;
        // anything else (`,`/`;`/`[`) means the first identifier was
        // already the sole declarator's discipline.
        if kw.is_some() {
            // `name [msb:lsb] (...)` — an array-of-instances range
            // (LRMC-C5-08) sits between the instance name and its port
            // list, so the plain "second ident directly followed by `(`"
            // check below needs to look *past* a bracketed range first
            // (an ordinary net declarator can equally carry one, e.g.
            // `electrical bus[3:0];` — only what follows the closing `]`
            // disambiguates the two).
            let ranged_instantiation = matches!(self.peek_at(1), Tok::Ident(_))
                && matches!(self.peek_at(2), Tok::Punct("["))
                && {
                    let mut off = 2;
                    let mut depth = 0i32;
                    loop {
                        match self.peek_at(off) {
                            Tok::Punct("[") => {
                                depth += 1;
                                off += 1;
                            }
                            Tok::Punct("]") => {
                                depth -= 1;
                                off += 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            Tok::Eof => break,
                            _ => off += 1,
                        }
                    }
                    matches!(self.peek_at(off), Tok::Punct("("))
                };
            let looks_like_instantiation = matches!(self.peek_at(1), Tok::Punct("#"))
                || (matches!(self.peek_at(1), Tok::Ident(_)) && matches!(self.peek_at(2), Tok::Punct("(")))
                || ranged_instantiation;
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

    /// `event_declaration ::= event list_of_event_identifiers ;` — §5.10.4,
    /// Annex A.2.1.3/Syntax 5-20. `list_of_event_identifiers` allows an
    /// array dimension per name (`event e[0:3];`) — not yet representable
    /// (`ast::EventDecl` is a flat name list, matching `ast::GenvarDecl`'s
    /// own scope note precedent), so a `[` after a name is a genuine parse
    /// error here rather than silently dropped.
    fn parse_event_decl(&mut self) -> Result<EventDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("event")?;
        let (first, _) = self.expect_ident()?;
        let mut names = vec![first];
        while self.eat_punct(",") {
            let (name, _) = self.expect_ident()?;
            names.push(name);
        }
        let end = self.expect_punct(";")?;
        Ok(EventDecl { span: Span::new(start.start, end.end), attrs, names })
    }

    /// `integer_declaration`/`real_declaration`/`time_declaration`/
    /// `realtime_declaration`, Annex A.2.1.3 — one of the
    /// `module_or_generate_item_declaration` alternatives. The `variable_type`
    /// production also allows a `'=' constant_expression` initializer per
    /// name, but `ast::VarDecl` carries no initializer field (only
    /// `Declarator { name, range }`, the same shape ports/nets use) — an
    /// initialized module-level variable is not yet representable, so this
    /// parses the declarator-list form only; a bare `=` here is a genuine
    /// parse error, not a silently dropped initializer.
    fn parse_var_decl(&mut self) -> Result<VarDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let kind = if self.eat_kw("integer") {
            VarKind::Integer
        } else if self.eat_kw("real") {
            VarKind::Real
        } else if self.eat_kw("realtime") {
            VarKind::Realtime
        } else {
            self.expect_kw("time")?;
            VarKind::Time
        };
        let names = self.parse_declarator_list()?;
        let end = self.expect_punct(";")?;
        Ok(VarDecl { span: Span::new(start.start, end.end), attrs, kind, names })
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

    /// `parameter_identifier constant_mintypmax_expression` (scalar) or
    /// `parameter_identifier range { range } constant_arrayinit`
    /// (array/multidimensional-array, §3.4.4/§3.4.8) — Annex A.2.4
    /// `param_assignment`. `range` reuses [`Self::parse_range`], the same
    /// `[msb:lsb]` production nets/ports declare with.
    fn parse_param_assignment(&mut self) -> Result<ParamAssignment, ParseError> {
        let (name, _) = self.expect_ident()?;
        let mut dims = vec![];
        while self.at_punct("[") {
            dims.push(self.parse_range()?);
        }
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        Ok(ParamAssignment { name, dims, value })
    }

    /// `'[' expr ':' expr ']'` — a single `[msb:lsb]` range, shared by
    /// declarators (`parse_declarator`) and parameter array dimensions
    /// (`parse_param_assignment`).
    fn parse_range(&mut self) -> Result<Range, ParseError> {
        self.expect_punct("[")?;
        let msb = self.parse_expr()?;
        self.expect_punct(":")?;
        let lsb = self.parse_expr()?;
        let end = self.expect_punct("]")?;
        Ok(Range { span: end, msb, lsb })
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
        let mut names = vec![self.parse_specparam_assignment()?];
        while self.eat_punct(",") {
            names.push(self.parse_specparam_assignment()?);
        }
        let end = self.expect_punct(";")?;
        Ok(SpecparamDecl { span: Span::new(start.start, end.end), attrs, names })
    }

    /// `specparam`'s own `name = expr` pair — reuses
    /// [`Self::parse_param_assignment`] (the array-dimension form,
    /// §3.4.4/§3.4.8, is a `parameter`-only production; a `[...]` here is
    /// rejected rather than silently dropped).
    fn parse_specparam_assignment(&mut self) -> Result<(String, Expr), ParseError> {
        let p = self.parse_param_assignment()?;
        if let Some(dim) = p.dims.first() {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedToken {
                    expected: "'=' (specparam does not take an array dimension)".into(),
                    found: "'['".into(),
                },
                dim.span,
            ));
        }
        Ok((p.name, p.value))
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
        // `name_of_module_instance ::= module_instance_identifier [ range
        // ]` — an array-of-instances range, LRMC-C5-08.
        let range = if self.at_punct("[") { Some(self.parse_range()?) } else { None };
        self.expect_punct("(")?;
        let mut connections = vec![];
        if !self.at_punct(")") {
            connections.push(self.parse_port_connection()?);
            while self.eat_punct(",") {
                connections.push(self.parse_port_connection()?);
            }
        }
        let end = self.expect_punct(")")?;
        Ok(Instance { span: Span::new(start.start, end.end), name, range, connections })
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

    // ─────────────────────────────── specify / gates ───────────────────────

    /// `specify_block`, Annex A.7.1 — this crate's scope treats the body as
    /// opaque (path declarations and system timing checks are a VAMS-22/P3
    /// concern per the design's Annex E note), so it is skipped token-by-
    /// token rather than deep-parsed; the construct still parses instead of
    /// rejecting a source that uses one.
    fn parse_specify_block(&mut self) -> Result<SpecifyBlock, ParseError> {
        let start = self.expect_kw("specify")?;
        while !self.at_kw("endspecify") {
            if self.at_eof() {
                return Err(self.eof_error("'endspecify'".to_string()));
            }
            self.bump();
        }
        let end = self.expect_kw("endspecify")?;
        Ok(SpecifyBlock { span: Span::new(start.start, end.end), items: vec![] })
    }

    /// `gate_instantiation`, Annex A.4.2. Attribute-bearing. Drive strength
    /// (`(strength0, strength1)`) is not disambiguated from an unnamed
    /// instance's terminal list in this crate's scope — `strength0`/
    /// `strength1` are always `None`; only `#delay` and the instance list
    /// are populated.
    fn parse_gate_instantiation(&mut self) -> Result<GateInstantiation, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let (word, _) = self.expect_ident()?;
        let kind = gate_kind_for(&word)
            .ok_or_else(|| self.unexpected("a gate primitive keyword".to_string()))?;
        let delay = if self.eat_punct("#") { Some(self.parse_expr()?) } else { None };
        let mut instances = vec![self.parse_gate_instance()?];
        while self.eat_punct(",") {
            instances.push(self.parse_gate_instance()?);
        }
        let end = self.expect_punct(";")?;
        Ok(GateInstantiation {
            span: Span::new(start.start, end.end),
            attrs,
            kind,
            strength0: None,
            strength1: None,
            delay,
            instances,
        })
    }

    fn parse_gate_instance(&mut self) -> Result<GateInstance, ParseError> {
        let start = self.peek_span();
        let name = if matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Punct("(")) {
            Some(self.expect_ident()?.0)
        } else {
            None
        };
        self.expect_punct("(")?;
        let mut terminals = vec![self.parse_expr()?];
        while self.eat_punct(",") {
            terminals.push(self.parse_expr()?);
        }
        let end = self.expect_punct(")")?;
        Ok(GateInstance { span: Span::new(start.start, end.end), name, terminals })
    }

    // ─────────────────────────────────── UDP ────────────────────────────────

    /// `udp_declaration`, Annex A.5.1. Attribute-bearing.
    pub fn parse_primitive_decl(&mut self) -> Result<PrimitiveDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("primitive")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct("(")?;
        // The port name list at the header is positional-only (types/
        // directions are restated as `udp_port_declaration`s below,
        // Annex A.5.2) — captured here only to advance past it correctly.
        self.expect_ident()?;
        while self.eat_punct(",") {
            self.expect_ident()?;
        }
        self.expect_punct(")")?;
        self.expect_punct(";")?;

        let mut ports = vec![];
        while !self.at_kw("table") {
            if self.at_eof() {
                return Err(self.eof_error("'table'".to_string()));
            }
            ports.push(self.parse_udp_port_decl()?);
        }
        self.expect_kw("table")?;
        let mut table = vec![];
        while !self.at_kw("endtable") {
            if self.at_eof() {
                return Err(self.eof_error("'endtable'".to_string()));
            }
            table.push(self.parse_udp_table_row()?);
        }
        self.expect_kw("endtable")?;
        let is_sequential = table.iter().any(|r| r.current_state.is_some());
        let end = self.expect_kw("endprimitive")?;
        Ok(PrimitiveDecl { span: Span::new(start.start, end.end), attrs, name, ports, table, is_sequential })
    }

    /// `udp_port_declaration`, Annex A.5.2. Attribute-bearing.
    fn parse_udp_port_decl(&mut self) -> Result<UdpPortDecl, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        if self.eat_kw("output") {
            let is_reg = self.eat_kw("reg");
            let (name, _) = self.expect_ident()?;
            let init = if self.eat_punct("=") { Some(self.parse_expr()?) } else { None };
            let end = self.expect_punct(";")?;
            return Ok(UdpPortDecl::Output {
                span: Span::new(start.start, end.end),
                attrs,
                name,
                is_reg,
                init,
            });
        }
        if self.eat_kw("input") {
            let mut names = vec![self.expect_ident()?.0];
            while self.eat_punct(",") {
                names.push(self.expect_ident()?.0);
            }
            let end = self.expect_punct(";")?;
            return Ok(UdpPortDecl::Input { span: Span::new(start.start, end.end), attrs, names });
        }
        self.expect_kw("reg")?;
        let (name, _) = self.expect_ident()?;
        let end = self.expect_punct(";")?;
        Ok(UdpPortDecl::Reg { span: Span::new(start.start, end.end), attrs, name })
    }

    /// One `table` row, Annex A.5.3 — `level_symbol+ ':' level_symbol
    /// [':' level_symbol] ';'`; a second `:' marks a sequential UDP's
    /// current-state column.
    fn parse_udp_table_row(&mut self) -> Result<UdpTableRow, ParseError> {
        let start = self.peek_span();
        let mut inputs = vec![self.parse_level_symbol()?];
        while !self.at_punct(":") {
            inputs.push(self.parse_level_symbol()?);
        }
        self.expect_punct(":")?;
        let first = self.parse_level_symbol()?;
        let (current_state, next_state) = if self.eat_punct(":") {
            (Some(first), self.parse_level_symbol()?)
        } else {
            (None, first)
        };
        let end = self.expect_punct(";")?;
        Ok(UdpTableRow { span: Span::new(start.start, end.end), inputs, current_state, next_state })
    }

    /// One UDP level symbol (`0`/`1`/`x`/`?`/`-`/`*`/`b`/`r`/`f`/`p`/`n`) —
    /// kept as raw text; interpreting the symbol is an elaboration/codegen
    /// concern (VAMS-19), not this grammar's.
    fn parse_level_symbol(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            Tok::Number(NumberLit::Integer(n)) => {
                self.bump();
                Ok(n.to_string())
            }
            Tok::Ident(s) => {
                self.bump();
                Ok(s)
            }
            Tok::Punct(p) => {
                self.bump();
                Ok(p.to_string())
            }
            _ => Err(self.unexpected("a UDP level symbol".to_string())),
        }
    }

    // ───────────────────────── paramset / connectrules / config ────────────

    /// `paramset_declaration`, Annex A.1.9 (not covered by §2.9's attribute
    /// sites).
    pub fn parse_paramset_decl(&mut self) -> Result<ParamsetDecl, ParseError> {
        let start = self.expect_kw("paramset")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct("(")?;
        let (module_name, _) = self.expect_ident()?;
        self.expect_punct(")")?;
        self.expect_punct(";")?;
        let mut params = vec![];
        let mut statements = vec![];
        while !self.at_kw("endparamset") {
            if self.at_eof() {
                return Err(self.eof_error("'endparamset'".to_string()));
            }
            if self.at_punct(".") {
                statements.push(self.parse_paramset_statement()?);
            } else {
                params.push(self.parse_param_decl_body(false)?);
            }
        }
        let end = self.expect_kw("endparamset")?;
        Ok(ParamsetDecl { span: Span::new(start.start, end.end), name, module_name, params, statements })
    }

    /// `.module_parameter_identifier = paramset_constant_expression;` —
    /// §6.4 Syntax 6-4's `paramset_statement`, first alternative
    /// (LRMC-C5-07's own scope; the other three alternatives — module
    /// output variable, system parameter, `analog_function_statement` —
    /// are not parsed here).
    fn parse_paramset_statement(&mut self) -> Result<ParamsetStatement, ParseError> {
        let start = self.expect_punct(".")?;
        let (target, _) = self.expect_ident()?;
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        let end = self.expect_punct(";")?;
        Ok(ParamsetStatement { span: Span::new(start.start, end.end), target, value })
    }

    /// `connectrules_declaration`, Annex A.1.8 — LRM chapter 7/Annex F.
    pub fn parse_connectrules_decl(&mut self) -> Result<ConnectrulesDecl, ParseError> {
        let start = self.expect_kw("connectrules")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct(";")?;
        let mut rules = vec![];
        let mut resolutions = vec![];
        while !self.at_kw("endconnectrules") {
            if self.at_eof() {
                return Err(self.eof_error("'endconnectrules'".to_string()));
            }
            match self.parse_connect_item()? {
                ConnectItem::Rule(r) => rules.push(r),
                ConnectItem::Resolution(r) => resolutions.push(r),
            }
        }
        let end = self.expect_kw("endconnectrules")?;
        Ok(ConnectrulesDecl { span: Span::new(start.start, end.end), name, rules, resolutions })
    }

    /// One `connect ...;` statement — either a `connect_rule` (legacy
    /// shell or `connect_insertion`) or a `connect_resolution` (§7.7.2
    /// Syntax 7-7, T33), distinguished by lookahead after the first
    /// identifier: a bare `,` next means a comma-separated discipline
    /// list, which is either the legacy shell's pair form (`connect A,
    /// B;`) or — if the list is followed by the `resolveto` keyword — a
    /// `connect_resolution`; anything else after the first identifier
    /// means it's a `connectmodule_identifier` and what follows is
    /// `connect_insertion`, §7.7.1 Syntax 7-6.
    fn parse_connect_item(&mut self) -> Result<ConnectItem, ParseError> {
        let start = self.expect_kw("connect")?;
        let (first, _) = self.expect_ident()?;

        if self.at_punct(",") {
            let mut disciplines = vec![first];
            while self.eat_punct(",") {
                disciplines.push(self.expect_ident()?.0);
            }
            if self.eat_kw("resolveto") {
                let target = if self.eat_kw("exclude") {
                    ConnectResolutionTarget::Exclude
                } else {
                    ConnectResolutionTarget::Discipline(self.expect_ident()?.0)
                };
                let end = self.expect_punct(";")?;
                return Ok(ConnectItem::Resolution(ConnectResolution {
                    span: Span::new(start.start, end.end),
                    disciplines,
                    target,
                }));
            }
            let mut body = vec![];
            let end = if self.eat_punct("{") {
                while !self.at_punct("}") {
                    if self.at_eof() {
                        return Err(self.eof_error("'}'".to_string()));
                    }
                    body.push(self.parse_stmt()?);
                }
                self.expect_punct("}")?
            } else {
                self.expect_punct(";")?
            };
            return Ok(ConnectItem::Rule(ConnectRule {
                span: Span::new(start.start, end.end),
                connectmodule: None,
                disciplines,
                mode: None,
                params: vec![],
                overrides: None,
                body,
            }));
        }

        let connectmodule = first;
        let mode = if self.eat_kw("merged") {
            Some(ConnectMode::Merged)
        } else if self.eat_kw("split") {
            Some(ConnectMode::Split)
        } else {
            None
        };

        let mut params = vec![];
        if self.eat_punct("#") {
            self.expect_punct("(")?;
            if !self.at_punct(")") {
                params.push(self.parse_port_connection()?);
                while self.eat_punct(",") {
                    params.push(self.parse_port_connection()?);
                }
            }
            self.expect_punct(")")?;
        }

        let overrides = if self.eat_kw("input") {
            let (d1, _) = self.expect_ident()?;
            self.expect_punct(",")?;
            self.expect_kw("output")?;
            let (d2, _) = self.expect_ident()?;
            Some(ConnectPortOverrides::InputOutput(d1, d2))
        } else if self.eat_kw("output") {
            let (d1, _) = self.expect_ident()?;
            self.expect_punct(",")?;
            self.expect_kw("input")?;
            let (d2, _) = self.expect_ident()?;
            Some(ConnectPortOverrides::OutputInput(d1, d2))
        } else if self.eat_kw("inout") {
            let (d1, _) = self.expect_ident()?;
            self.expect_punct(",")?;
            self.expect_kw("inout")?;
            let (d2, _) = self.expect_ident()?;
            Some(ConnectPortOverrides::InOut(d1, d2))
        } else if !self.at_punct(";") && !self.at_punct("{") {
            let (d1, _) = self.expect_ident()?;
            self.expect_punct(",")?;
            let (d2, _) = self.expect_ident()?;
            Some(ConnectPortOverrides::Both(d1, d2))
        } else {
            None
        };

        let disciplines = match &overrides {
            Some(ConnectPortOverrides::Both(a, b))
            | Some(ConnectPortOverrides::InputOutput(a, b))
            | Some(ConnectPortOverrides::OutputInput(a, b))
            | Some(ConnectPortOverrides::InOut(a, b)) => vec![a.clone(), b.clone()],
            None => vec![],
        };

        let mut body = vec![];
        let end = if self.eat_punct("{") {
            while !self.at_punct("}") {
                if self.at_eof() {
                    return Err(self.eof_error("'}'".to_string()));
                }
                body.push(self.parse_stmt()?);
            }
            self.expect_punct("}")?
        } else {
            self.expect_punct(";")?
        };
        Ok(ConnectItem::Rule(ConnectRule {
            span: Span::new(start.start, end.end),
            connectmodule: Some(connectmodule),
            disciplines,
            mode,
            params,
            overrides,
            body,
        }))
    }

    /// `config_declaration`, inherited from 1364 (Annex A.1.1). Piperine
    /// tooling has no configuration-selection consumer (spec Out of
    /// Scope), so the body is skipped token-by-token rather than
    /// reconstructed — a source using one parses instead of rejecting.
    pub fn parse_config_decl(&mut self) -> Result<ConfigDecl, ParseError> {
        let start = self.expect_kw("config")?;
        let (name, _) = self.expect_ident()?;
        self.expect_punct(";")?;
        while !self.at_kw("endconfig") {
            if self.at_eof() {
                return Err(self.eof_error("'endconfig'".to_string()));
            }
            self.bump();
        }
        let end = self.expect_kw("endconfig")?;
        Ok(ConfigDecl { span: Span::new(start.start, end.end), name, body: String::new() })
    }

    // ─────────────────────────────── source file ───────────────────────────

    /// `source_text`, Annex A.1.1 — the grammar's top-level entry point:
    /// zero or more `description`s until end of file.
    pub fn parse_source_file(&mut self) -> Result<SourceFile, ParseError> {
        let mut items = vec![];
        while !self.at_eof() {
            items.push(self.parse_item()?);
        }
        Ok(SourceFile { items, directives: Default::default() })
    }

    /// `description`, Annex A.1.1 — one top-level declaration.
    fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.kw_after_attrs() {
            Some("discipline") => Ok(Item::Discipline(self.parse_discipline_decl()?)),
            Some("nature") => Ok(Item::Nature(self.parse_nature_decl()?)),
            Some("module") | Some("macromodule") | Some("connectmodule") => {
                Ok(Item::Module(self.parse_module_decl()?))
            }
            Some("paramset") => Ok(Item::Paramset(self.parse_paramset_decl()?)),
            Some("connectrules") => Ok(Item::Connectrules(self.parse_connectrules_decl()?)),
            Some("config") => Ok(Item::Config(self.parse_config_decl()?)),
            Some("primitive") => Ok(Item::Primitive(self.parse_primitive_decl()?)),
            _ => Err(self.unexpected(
                "a top-level declaration (discipline, nature, module, macromodule, \
                 connectmodule, paramset, connectrules, config, or primitive)"
                    .to_string(),
            )),
        }
    }
}

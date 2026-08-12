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
        if self.at_kw("input") || self.at_kw("output") || self.at_kw("inout") {
            return Ok(ModuleItem::Port(self.parse_port_decl()?));
        }
        if self.at_kw("ground") {
            return Ok(ModuleItem::Ground(self.parse_ground_decl()?));
        }
        if self.at_kw("parameter") {
            return Ok(ModuleItem::Param(self.parse_param_decl_body(false)?));
        }
        if self.at_kw("localparam") {
            return Ok(ModuleItem::Param(self.parse_param_decl_body(false)?));
        }
        if self.at_kw("aliasparam") {
            return Ok(ModuleItem::AliasParam(self.parse_aliasparam()?));
        }
        if self.at_kw("specparam") {
            return Ok(ModuleItem::Specparam(self.parse_specparam()?));
        }
        // A bare identifier starting an item with no direction/parameter
        // keyword is a net declaration (`electrical p, n;`) — the
        // identifier is the discipline name.
        if matches!(self.peek(), Tok::Ident(_)) {
            return Ok(ModuleItem::Net(self.parse_net_decl()?));
        }
        Err(self.unexpected(
            "a port, net, ground, parameter, aliasparam, or specparam declaration".to_string(),
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
}

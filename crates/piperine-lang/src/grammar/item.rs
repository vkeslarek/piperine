//! Declaration grammar — natures, disciplines, modules and their core
//! surface (ports, nets, parameters), LRM Annex A.1.2/A.1.3/A.1.5/A.1.6,
//! A.2.1.1/A.2.1.3.
//!
//! `parse_module_item` is the module-body dispatcher; each task that adds
//! more declaration kinds (T10, T11) extends its `match` with new arms —
//! the method itself, and this file, are *extended* across those tasks by
//! design (declaration-family seams), never duplicated.

use crate::ast::*;
use crate::error::{ParseError, ParseErrorKind, PreprocessedSpan};
use crate::grammar::cursor::Grammar;
use crate::keywords::Keywords;
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

/// Applies a leading `[msb:lsb]` range (Annex A.2.1.2/A.2.2.1's `range`
/// slot, parsed by [`Grammar::parse_optional_leading_range`]) to every
/// declarator in `names` that carries none of its own (§3.6.3: the range
/// applies to the whole declaration list).
///
/// A declarator that already carries its own range is normally a loud
/// [`ParseErrorKind::LeadingRangeConflict`] naming both spans — a
/// declared name may carry only one range. The one exception is a 1364
/// net-type declaration (`allow_memory_reinterpretation`, T5,
/// LRMW3-04): there, the leading range is unambiguously the packed
/// element width, so a lone trailing range on the declarator is
/// reinterpreted as the memory's unpacked depth (`reg [3:0] mem [0:7];`)
/// rather than erroring — unless the declarator *also* already has its
/// own `depth_range` (a second trailing bracket), which would make three
/// ranges total and is still a genuine conflict.
fn apply_leading_range(
    leading: &Option<Range>,
    names: &mut [Declarator],
    allow_memory_reinterpretation: bool,
) -> Result<(), ParseError> {
    let Some(leading) = leading else { return Ok(()) };
    for d in names.iter_mut() {
        match (&d.range, &d.depth_range) {
            (None, _) => d.range = Some(leading.clone()),
            (Some(own), None) if allow_memory_reinterpretation => {
                d.depth_range = Some(own.clone());
                d.range = Some(leading.clone());
            }
            (Some(own), _) => {
                return Err(ParseError::new(
                    ParseErrorKind::LeadingRangeConflict {
                        name: d.name.clone(),
                        leading: leading.span,
                        own: own.span,
                    },
                    own.span,
                ));
            }
        }
    }
    Ok(())
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
        let doc = self.doc_at(self.peek_span());
        let start = self.expect_kw("nature")?;
        let (name, _) = self.expect_ident()?;
        // `: parent` (§3.6.1.1) or `: discipline.slot` (§3.6.2.6) — the
        // hierarchical-separator form names a discipline's `potential`/
        // `flow` binding to derive from instead of another nature.
        let (parent, discipline_parent) = if self.eat_punct(":") {
            let (first, _) = self.expect_ident()?;
            if self.eat_punct(".") {
                let (slot_name, slot_span) = self.expect_ident()?;
                let slot = match slot_name.as_str() {
                    "potential" => NatureSlot::Potential,
                    "flow" => NatureSlot::Flow,
                    other => {
                        return Err(ParseError::new(
                            ParseErrorKind::UnexpectedToken {
                                expected: "'potential' or 'flow'".to_string(),
                                found: format!("`{other}`"),
                            },
                            slot_span,
                        ));
                    }
                };
                (None, Some((first, slot)))
            } else {
                (Some(first), None)
            }
        } else {
            (None, None)
        };
        self.eat_punct(";");

        let mut n = NatureDecl {
            doc,
            span: start,
            name,
            parent,
            discipline_parent,
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
        n.span = PreprocessedSpan::new(start.start, end.end);
        Ok(n)
    }

    /// The body of a `potential`/`flow` discipline item, past the keyword:
    /// either the ordinary binding form (`potential Voltage;`) or a
    /// §3.6.2.5 attribute override (`potential.abstol = 10u;`) —
    /// distinguished by whether a `.` follows the keyword. `access`/
    /// `units` are rejected here (§3.6.1.2 forbids overriding either).
    fn parse_discipline_slot_body(&mut self, d: &mut DisciplineDecl, slot: NatureSlot) -> Result<(), ParseError> {
        if self.eat_punct(".") {
            let (attr, attr_span) = self.expect_ident()?;
            if attr == "access" || attr == "units" {
                return Err(ParseError::new(
                    ParseErrorKind::RestrictedNatureAttributeOverride { attr },
                    attr_span,
                ));
            }
            self.expect_punct("=")?;
            let val = self.parse_expr()?;
            self.expect_punct(";")?;
            d.attr_overrides.push((slot, attr, val));
            return Ok(());
        }
        let (name, _) = self.expect_ident()?;
        match slot {
            NatureSlot::Potential => d.potential = Some(name),
            NatureSlot::Flow => d.flow = Some(name),
        }
        self.expect_punct(";")?;
        Ok(())
    }

    /// `discipline_declaration`, Annex A.1.7 — §3.6.2. Not attribute-bearing.
    pub fn parse_discipline_decl(&mut self) -> Result<DisciplineDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
        let start = self.expect_kw("discipline")?;
        let (name, _) = self.expect_ident()?;
        self.eat_punct(";");

        let mut d = DisciplineDecl {
            span: start,
            doc,
            name,
            domain: None,
            potential: None,
            flow: None,
            attr_overrides: vec![],
            attributes: vec![],
        };
        while !self.at_kw("enddiscipline") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'enddiscipline'".to_string()));
            }
            // §3.6.2.7: `domain`/`potential`/`flow` are reserved binding
            // names (their own keyword forms are handled by the three
            // arms below) — a bare `domain = expr;`/`potential = expr;`/
            // `flow = expr;` is an attempt to redeclare one of them as a
            // user-defined attribute, which collides with the reserved
            // name and must fail loud rather than either silently
            // shadowing the binding or falling into a confusing "expected
            // continuous/discrete" parse error from the arms below.
            if let Tok::Ident(name) = self.peek()
                && matches!(name.as_str(), "domain" | "potential" | "flow")
                && matches!(self.peek_at(1), Tok::Punct("="))
            {
                let attr = name.clone();
                let span = self.peek_span();
                return Err(ParseError::new(ParseErrorKind::ReservedDisciplineAttributeName { attr }, span));
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
                self.parse_discipline_slot_body(&mut d, NatureSlot::Potential)?;
                continue;
            }
            if self.eat_kw("flow") {
                self.parse_discipline_slot_body(&mut d, NatureSlot::Flow)?;
                continue;
            }
            // §3.6.2.7: a bare `attr_name = expr;` user-defined discipline
            // attribute — anything reaching here already isn't
            // `domain`/`potential`/`flow` (those are consumed above), so a
            // reserved binding name written without its keyword form (a
            // token this arm can't produce) simply isn't representable —
            // any other identifier is a legitimate user attribute.
            if matches!(self.peek(), Tok::Ident(_)) {
                let (attr, _) = self.expect_ident()?;
                self.expect_punct("=")?;
                let val = self.parse_expr()?;
                self.expect_punct(";")?;
                d.attributes.push((attr, val));
                continue;
            }
            return Err(self.unexpected("'domain', 'potential', 'flow', or an attribute name".to_string()));
        }
        let end = self.expect_kw("enddiscipline")?;
        d.span = PreprocessedSpan::new(start.start, end.end);
        Ok(d)
    }

    // ───────────────────────────────── module ───────────────────────────

    /// `module_declaration`, Annex A.1.2 — §Syntax 2-5 (attribute-bearing).
    pub fn parse_module_decl(&mut self) -> Result<ModuleDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
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
            doc,
            span: PreprocessedSpan::new(start.start, end.end),
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
        if kw == Some("constraint") {
            return Ok(ModuleItem::Constraint(self.parse_constraint_block()?));
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
        let doc = self.doc_at(self.peek_span());
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
        let leading_range = self.parse_optional_leading_range()?;
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
        apply_leading_range(&leading_range, &mut names, false)?;
        let end = self.peek_span();
        self.eat_punct(";");
        Ok(PortDecl { span: PreprocessedSpan::new(start.start, end.start), doc, attrs, dir, discipline, names })
    }

    /// `net_declaration`, §3.6.3/3.7 — a bare `discipline_identifier
    /// declarator_list ';'`, optionally followed by a nodeset value
    /// (§3.6.3.2: `net_discipline_identifier name <initial_value>;`).
    fn parse_net_decl(&mut self) -> Result<NetDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let (discipline, _) = self.expect_ident()?;
        let leading_range = self.parse_optional_leading_range()?;
        let mut names = self.parse_declarator_list()?;
        // Only a 1364 net-type declaration (`reg`/`wire`/`tri`/`wreal`)
        // can be a memory (Annex A.2.2.1) — a plain discipline net has no
        // depth concept, so a leading range plus its own trailing range
        // stays the T1 conflict error there.
        let is_1364_net_type = crate::pom::DigitalNetKind::from_keyword(&discipline).is_some();
        apply_leading_range(&leading_range, &mut names, is_1364_net_type)?;
        let nodeset = if self.eat_punct("=") { Some(self.parse_expr()?) } else { None };
        self.expect_punct(";")?;
        Ok(NetDecl {
            doc,
            span: PreprocessedSpan::new(start.start, start.end),
            attrs,
            discipline: Some(discipline),
            names,
            nodeset,
        })
    }

    /// `ground_declaration ::= ground [discipline_identifier] [range]
    /// list_of_net_identifiers ;` — §3.6.4, Syntax 3-7. This crate's
    /// `GroundDecl` carries one name (T25's `genvar`/array-of-instances
    /// precedent: a single-name scope, extended if a real fixture ever
    /// needs the list form). `discipline_identifier` is genuinely
    /// ambiguous with the name itself on token shape alone (both are bare
    /// identifiers) — resolved the same way `parse_net_decl`'s own
    /// two-token dispatch does: an identifier is the discipline only when
    /// something else (another identifier, or a `[range]`) still follows
    /// it before `;`.
    fn parse_ground_decl(&mut self) -> Result<GroundDecl, ParseError> {
        let start = self.expect_kw("ground")?;
        let discipline = if matches!(self.peek(), Tok::Ident(_))
            && (matches!(self.peek_at(1), Tok::Ident(_)) || matches!(self.peek_at(1), Tok::Punct("[")))
        {
            let (d, _) = self.expect_ident()?;
            Some(d)
        } else {
            None
        };
        let range = if self.at_punct("[") { Some(self.parse_range()?) } else { None };
        let (name, _) = self.expect_ident()?;
        let end = self.expect_punct(";")?;
        Ok(GroundDecl { span: PreprocessedSpan::new(start.start, end.end), discipline, range, name })
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
        Ok(EventDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, names })
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
        let doc = self.doc_at(self.peek_span());
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
        let leading_range = self.parse_optional_leading_range()?;
        let mut names = self.parse_var_declarator_list()?;
        apply_leading_range(&leading_range, &mut names, false)?;
        let end = self.expect_punct(";")?;
        Ok(VarDecl { span: PreprocessedSpan::new(start.start, end.end), doc, attrs, kind, names })
    }

    /// `variable_type { ',' variable_type }` — Annex A.2.1.3's
    /// `list_of_variable_identifiers`/`list_of_real_identifiers`: a
    /// declarator that additionally accepts the optional `=
    /// constant_expression` initializer of A.2.2.1's `variable_type`/
    /// `real_type` (the port/net `Declarator` productions carry none).
    fn parse_var_declarator_list(&mut self) -> Result<Vec<Declarator>, ParseError> {
        let mut names = vec![self.parse_var_declarator()?];
        while self.eat_punct(",") {
            names.push(self.parse_var_declarator()?);
        }
        Ok(names)
    }

    fn parse_var_declarator(&mut self) -> Result<Declarator, ParseError> {
        let mut d = self.parse_declarator()?;
        if self.eat_punct("=") {
            d.init = Some(self.parse_expr()?);
        }
        Ok(d)
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
            // `discipline_name [msb:lsb] name` (a leading range after a
            // discipline, T2) also starts `Ident '['`, the same shape a
            // bare `name[msb:lsb]` declarator's own range does — resolved
            // by looking past the balanced range for a further identifier
            // (present only in the discipline-then-leading-range case).
            if matches!(self.peek_at_next(), Tok::Punct("[")) && self.range_then_ident_follows() {
                return Some(self.expect_ident().ok()?.0);
            }
        }
        None
    }

    /// True when, starting at the `[` immediately following the current
    /// identifier (`peek_at(1)`), a balanced `[...]` range is itself
    /// followed by another identifier. See
    /// [`Self::parse_optional_discipline_name`]'s leading-range case.
    fn range_then_ident_follows(&self) -> bool {
        let mut off = 1usize; // the '[' at peek_at(1)
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
                Tok::Eof => return false,
                _ => off += 1,
            }
        }
        matches!(self.peek_at(off), Tok::Ident(_))
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

    /// `[msb:lsb]` immediately after the discipline/direction/type token
    /// and before the declarator list — Annex A.2.1.2/A.2.2.1's `range`
    /// slot on `net_declaration`/`port_declaration`/1364 net and variable
    /// declarations (§3.6.3): `electrical [0:3] bus;`, `input [0:7] p;`,
    /// `reg [3:0] q;`. Absent when the next token isn't `[`.
    pub(crate) fn parse_optional_leading_range(&mut self) -> Result<Option<Range>, ParseError> {
        if self.at_punct("[") { Ok(Some(self.parse_range()?)) } else { Ok(None) }
    }

    fn parse_declarator(&mut self) -> Result<Declarator, ParseError> {
        let (name, _) = self.expect_ident()?;
        let range = self.parse_optional_trailing_range()?;
        // A memory's unpacked depth (T5, Annex A.2.2.1): a *second*
        // trailing `[d0:d1]` right after the packed range —
        // `mem[3:0][0:7]`. Absent unless `range` was itself present.
        let depth_range = if range.is_some() { self.parse_optional_trailing_range()? } else { None };
        Ok(Declarator { name, range, depth_range, init: None })
    }

    /// One `[msb:lsb]` bracket immediately at the cursor, or `None` if the
    /// next token isn't `[` — the shared shape [`Self::parse_declarator`]'s
    /// two (optional) trailing brackets both use.
    fn parse_optional_trailing_range(&mut self) -> Result<Option<Range>, ParseError> {
        if !self.eat_punct("[") {
            return Ok(None);
        }
        let msb = self.parse_expr()?;
        self.expect_punct(":")?;
        let lsb = self.parse_expr()?;
        let end = self.expect_punct("]")?;
        Ok(Some(Range { span: end, msb, lsb }))
    }

    // ───────────────────────────────── parameters ───────────────────────

    /// `parameter_declaration`/`local_parameter_declaration`, §3.4,
    /// Annex A.2.1.1 — §Syntax 2-7 (attribute-bearing; `in_port_list`
    /// suppresses the trailing `;` for the `#(...)` port-list form).
    fn parse_param_decl_body(&mut self, in_port_list: bool) -> Result<ParamDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
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
        // The `tol` clause is declaration-level (after every name/range —
        // it applies to all of them); at most one, loud on a duplicate.
        let tol = if self.at_kw("tol") {
            let clause = self.parse_tol_clause()?;
            if self.at_kw("tol") {
                return Err(self.unexpected(
                    "at most one `tol` clause per parameter declaration".to_string(),
                ));
            }
            Some(clause)
        } else {
            None
        };
        if !in_port_list {
            self.expect_punct(";")?;
        }
        Ok(ParamDecl { span: start, doc, attrs, is_local, ty, names, ranges, tol })
    }

    /// `tol <kind>( <keyed-args> ) [global]` — the statistical clause on a
    /// parameter declaration (`tol`/`global` contextual keywords; the kind
    /// is a plain identifier the elaborator validates — `gauss`/`uniform`).
    fn parse_tol_clause(&mut self) -> Result<TolClause, ParseError> {
        let start = self.expect_kw("tol")?;
        let (kind, _) = self.expect_ident()?;
        self.expect_punct("(")?;
        let mut args = Vec::new();
        if !self.at_punct(")") {
            loop {
                let (name, _) = self.expect_ident()?;
                self.expect_punct("=")?;
                let value = self.parse_expr()?;
                args.push((name, value));
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        let close = self.expect_punct(")")?;
        let global = self.eat_kw("global");
        Ok(TolClause {
            span: PreprocessedSpan::new(start.start, close.end),
            kind,
            args,
            global,
        })
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
        let (name, name_span) = self.expect_ident()?;
        let mut dims = vec![];
        while self.at_punct("[") {
            dims.push(self.parse_range()?);
        }
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        Ok(ParamAssignment { name, name_span, dims, value })
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
        Ok(AliasParam { span: PreprocessedSpan::new(start.start, end.end), name, target })
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
        Ok(SpecparamDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, names })
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
        Ok(BranchDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, pos, neg, names })
    }

    // ─────────────────────────── behavior constructs ──────────────────────

    /// `constraint begin { statements } end` — a sanctioned Piperine
    /// extension (MD-49): one more module body, mirroring
    /// `analog begin … end`'s shape. `constraint`/`require`/`tol`/`var`/
    /// `target`/`cover`/`bins` are contextual keywords — identifier text
    /// matched here only, never added to Annex B's reserved set. Anything
    /// inside the block that is not a known statement is a typed error
    /// naming the token — never a silent skip.
    fn parse_constraint_block(&mut self) -> Result<ConstraintBlockDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("constraint")?;
        self.expect_kw("begin")?;
        let mut stmts = Vec::new();
        while !self.at_kw("end") {
            stmts.push(self.parse_constraint_stmt(false)?);
        }
        let end = self.expect_kw("end")?;
        Ok(ConstraintBlockDecl {
            span: PreprocessedSpan::new(start.start, end.end),
            doc,
            attrs,
            stmts,
        })
    }

    /// One statement of a constraint block. `in_scope` is `true` inside
    /// an `@ … { … }` group — nesting another scope there is a loud
    /// error (no semantics for combined scopes yet).
    fn parse_constraint_stmt(&mut self, in_scope: bool) -> Result<ConstraintStmt, ParseError> {
        let stmt_doc = self.doc_at(self.peek_span());
        let stmt_start = self.peek_span();
        if self.at_kw("require") {
            Ok(ConstraintStmt::Require(self.parse_require_stmt()?))
        } else if self.at_kw("var") {
            self.bump();
            let (name, name_span) = self.expect_ident()?;
            Keywords::expect_identifier(&name, name_span)?;
            self.expect_punct("=")?;
            let expr = self.parse_expr()?;
            let end = self.expect_punct(";")?;
            Ok(ConstraintStmt::Var {
                span: PreprocessedSpan::new(stmt_start.start, end.end),
                doc: stmt_doc,
                name,
                expr,
            })
        } else if self.at_kw("target") {
            // `target <comparison> [tol <scale>];` — the comparison is
            // a single `a <= b` / `a >= b` expression (the expression
            // parser owns the operator); the lowering decomposes it.
            self.bump();
            let cmp_expr = self.parse_expr()?;
            let (expr, cmp, level) = match cmp_expr {
                Expr::Binary { op: crate::ast::BinOp::Le, lhs, rhs, .. } => (*lhs, TargetCmp::Le, *rhs),
                Expr::Binary { op: crate::ast::BinOp::Ge, lhs, rhs, .. } => (*lhs, TargetCmp::Ge, *rhs),
                _ => {
                    return Err(self.unexpected(
                        "a `<=`/`>=` comparison (`target a <= level [tol scale];`)".to_string(),
                    ));
                }
            };
            let tol = if self.at_kw("tol") {
                self.bump();
                Some(self.parse_expr()?)
            } else {
                None
            };
            let end = self.expect_punct(";")?;
            Ok(ConstraintStmt::Target {
                span: PreprocessedSpan::new(stmt_start.start, end.end),
                doc: stmt_doc,
                expr,
                cmp,
                level,
                tol,
            })
        } else if self.at_kw("cover") {
            self.bump();
            if self.at_kw("cross") {
                self.bump();
                let (name, name_span) = self.expect_ident()?;
                Keywords::expect_identifier(&name, name_span)?;
                self.expect_punct(":")?;
                let mut points = vec![self.expect_ident()?.0];
                while self.eat_punct(",") {
                    points.push(self.expect_ident()?.0);
                }
                let end = self.expect_punct(";")?;
                Ok(ConstraintStmt::CoverCross {
                    span: PreprocessedSpan::new(stmt_start.start, end.end),
                    doc: stmt_doc,
                    name,
                    points,
                })
            } else {
                let (name, name_span) = self.expect_ident()?;
                Keywords::expect_identifier(&name, name_span)?;
                self.expect_punct(":")?;
                let expr = self.parse_expr()?;
                self.expect_kw("bins")?;
                let bins = if self.eat_punct("[") {
                    let lo = self.parse_expr()?;
                    self.expect_punct(":")?;
                    let step = self.parse_expr()?;
                    self.expect_punct(":")?;
                    let hi = self.parse_expr()?;
                    self.expect_punct("]")?;
                    CoverBins::Range { lo, step, hi }
                } else {
                    self.expect_kw("in")?;
                    self.expect_punct("{")?;
                    let mut elems = vec![self.parse_expr()?];
                    while self.eat_punct(",") {
                        elems.push(self.parse_expr()?);
                    }
                    self.expect_punct("}")?;
                    CoverBins::Set(elems)
                };
                let end = self.expect_punct(";")?;
                Ok(ConstraintStmt::Cover {
                    span: PreprocessedSpan::new(stmt_start.start, end.end),
                    doc: stmt_doc,
                    name,
                    expr,
                    bins,
                })
            }
        } else if self.at_punct("@") {
            if in_scope {
                return Err(self.unexpected(
                    "a non-scope statement (nesting `@ … { … }` groups has no semantics)".to_string(),
                ));
            }
            self.bump(); // `@`
            let (word, _) = self.expect_ident()?;
            let analysis = match word.as_str() {
                "dc" => ScopeAnalysis::Dc,
                "tran" => ScopeAnalysis::Tran,
                "ac" => ScopeAnalysis::Ac,
                other => {
                    return Err(self.unexpected(format!(
                        "an analysis name after `@` (dc, tran, ac) — `{other}` is not one"
                    )));
                }
            };
            let window = if self.eat_punct("(") {
                if analysis != ScopeAnalysis::Tran {
                    return Err(self.unexpected(
                        "a temporal window is a `@ tran` notion — `@ dc`/`@ ac` scope by analysis alone"
                            .to_string(),
                    ));
                }
                let (name, _) = self.expect_ident()?;
                if name != "after" {
                    return Err(self.unexpected(
                        "the window form `after = expr[, dur = expr]` (after comes first)".to_string(),
                    ));
                }
                self.expect_punct("=")?;
                let after = self.parse_expr()?;
                let dur = if self.eat_punct(",") {
                    let (name, _) = self.expect_ident()?;
                    if name != "dur" {
                        return Err(self.unexpected("the window's second field is `dur`".to_string()));
                    }
                    self.expect_punct("=")?;
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect_punct(")")?;
                Some(ScopeWindow { after, dur })
            } else {
                None
            };
            self.expect_punct("{")?;
            let mut stmts = Vec::new();
            while !self.at_punct("}") {
                stmts.push(self.parse_constraint_stmt(true)?);
            }
            let end = self.expect_punct("}")?;
            Ok(ConstraintStmt::Scope {
                span: PreprocessedSpan::new(stmt_start.start, end.end),
                doc: stmt_doc,
                analysis,
                window,
                stmts,
            })
        } else {
            Err(self.unexpected(
                "a constraint statement (`require name : predicate;`, `var name = expr;`, \
                 `target expr <=|>= level [tol scale];`, `cover name : expr bins …;`, \
                 `cover cross name : p1, p2;`, `@ dc|tran|ac [(after…, dur…)] { … }`)".to_string(),
            ))
        }
    }

    fn parse_require_stmt(&mut self) -> Result<RequireDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
        let start = self.expect_kw("require")?;
        let (name, name_span) = self.expect_ident()?;
        Keywords::expect_identifier(&name, name_span)?;
        self.expect_punct(":")?;
        let pred = self.parse_expr()?;
        let tol = if self.at_kw("tol") {
            self.bump();
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = self.expect_punct(";")?;
        Ok(RequireDecl {
            span: PreprocessedSpan::new(start.start, end.end),
            doc,
            name,
            pred,
            tol,
        })
    }

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
        Ok(BehaviorConstruct { span: PreprocessedSpan::new(start.start, body.span().end), attrs, kind, body })
    }

    // ───────────────────────────── functions / tasks ───────────────────────

    /// `const <name> = <expr>;` — a sanctioned Piperine extension
    /// (MD-50): `const` is a contextual keyword here only, never added to
    /// Annex B.
    fn parse_const_decl(&mut self) -> Result<ConstDecl, ParseError> {
        let doc = self.doc_at(self.peek_span());
        let start = self.expect_kw("const")?;
        let (name, name_span) = self.expect_ident()?;
        Keywords::expect_identifier(&name, name_span)?;
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        let end = self.expect_punct(";")?;
        Ok(ConstDecl { span: PreprocessedSpan::new(start.start, end.end), doc, name, value })
    }

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
        Ok(PortItemDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, dir, ty, names })
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
        Ok(FunctionDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, kind, name, ret, ports, decls, body })
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
        Ok(TaskDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, name, ports, decls, body })
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
        Ok(GenvarDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, names })
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
            return Ok(GenerateBlock { span: PreprocessedSpan::new(start.start, end.end), name, items });
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
            span: PreprocessedSpan::new(start.start, body.span.end),
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
        Ok(IfGenerate { span: PreprocessedSpan::new(start.start, end.end), attrs, cond, then_branch, else_branch })
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
            arms.push(GenerateCaseArm { span: PreprocessedSpan::new(arm_start.start, body.span.end), labels, body });
        }
        let end = self.expect_kw("endcase")?;
        Ok(CaseGenerate { span: PreprocessedSpan::new(start.start, end.end), attrs, selector, arms })
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
        Ok(GenerateRegion { span: PreprocessedSpan::new(start.start, end.end), items })
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
        Ok(Defparam { span: PreprocessedSpan::new(start.start, end.end), attrs, target, value })
    }

    /// `continuous_assign`, Annex A.6.1. Attribute-bearing, §Syntax 2-7.
    fn parse_continuous_assign(&mut self) -> Result<ContinuousAssign, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.expect_kw("assign")?;
        let target = self.parse_lvalue()?;
        self.expect_punct("=")?;
        let value = self.parse_expr()?;
        let end = self.expect_punct(";")?;
        Ok(ContinuousAssign { span: PreprocessedSpan::new(start.start, end.end), attrs, target, value })
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
            return Ok(PortConnection { span: PreprocessedSpan::new(start.start, end.end), attrs, name: Some(name), value });
        }
        let value = if self.at_punct(",") || self.at_punct(")") { None } else { Some(self.parse_expr()?) };
        let end = value.as_ref().map(|v| v.span()).unwrap_or(start);
        Ok(PortConnection { span: PreprocessedSpan::new(start.start, end.end), attrs, name: None, value })
    }

    fn parse_instance(&mut self) -> Result<Instance, ParseError> {
        let start = self.peek_span();
        let (name, name_span) = self.expect_ident()?;
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
        Ok(Instance {
            span: PreprocessedSpan::new(start.start, end.end),
            name,
            name_span,
            range,
            connections,
        })
    }

    /// `module_instantiation`, Annex A.4.1. Attribute-bearing.
    fn parse_module_instantiation(&mut self) -> Result<ModuleInstantiation, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let start = self.peek_span();
        let (module_name, module_name_span) = self.expect_ident()?;
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
            span: PreprocessedSpan::new(start.start, end.end),
            attrs,
            module_name,
            module_name_span,
            param_overrides,
            instances,
        })
    }

    // ─────────────────────────────── specify / gates ───────────────────────

    /// `specify_block`, Annex A.7.1 — dispatches each `specify_item` to a
    /// typed production: `specparam_declaration` (reuses
    /// [`Self::parse_specparam`]) and `path_declaration` (simple paths land
    /// on `paths` — T15's scope is the simple form only; system timing
    /// checks and the recognized-but-unsupported forms are later tasks'
    /// extensions to this same dispatcher, not a separate function).
    fn parse_specify_block(&mut self) -> Result<SpecifyBlock, ParseError> {
        let start = self.expect_kw("specify")?;
        let mut specparams = Vec::new();
        let mut paths = Vec::new();
        let mut checks = Vec::new();
        let mut unsupported = Vec::new();
        while !self.at_kw("endspecify") {
            if self.at_eof() {
                return Err(self.eof_error("'endspecify'".to_string()));
            }
            if self.at_kw("specparam") {
                // `pulse_control_specparam`, Annex A.2.4 — `PATHPULSE$`
                // (bare or `PATHPULSE$in$out`) is a naming convention on
                // the specparam identifier, not a distinct keyword, so it
                // is caught here rather than falling into the ordinary
                // `parse_specparam` path (whose value grammar cannot parse
                // `(reject_limit, error_limit)` as an ordinary expression
                // anyway — this gives it a named citation instead of a
                // generic parse failure).
                if matches!(self.peek_at(1), Tok::Ident(s) if s.starts_with("PATHPULSE$")) {
                    let item_start = self.peek_span();
                    self.skip_specify_item();
                    unsupported.push(UnsupportedSpecifyForm {
                        span: item_start,
                        construct: "PATHPULSE$ pulse-control specparam",
                        clause: "A.2.4",
                    });
                } else {
                    specparams.push(self.parse_specparam()?);
                }
            } else if self.at_kw("if") || self.at_kw("ifnone") {
                // `state_dependent_path_declaration`, Annex A.7.2.
                let item_start = self.peek_span();
                self.skip_specify_item();
                unsupported.push(UnsupportedSpecifyForm {
                    span: item_start,
                    construct: "state-dependent path declaration",
                    clause: "A.7.2",
                });
            } else if self.at_kw("pulsestyle_onevent") || self.at_kw("pulsestyle_ondetect") {
                // `pulsestyle_declaration`, Annex A.7.1.
                let item_start = self.peek_span();
                self.skip_specify_item();
                unsupported.push(UnsupportedSpecifyForm {
                    span: item_start,
                    construct: "pulsestyle declaration",
                    clause: "A.7.1",
                });
            } else if self.at_kw("showcancelled") || self.at_kw("noshowcancelled") {
                // `showcancelled_declaration`, Annex A.7.1.
                let item_start = self.peek_span();
                self.skip_specify_item();
                unsupported.push(UnsupportedSpecifyForm {
                    span: item_start,
                    construct: "showcancelled declaration",
                    clause: "A.7.1",
                });
            } else if matches!(self.peek(), Tok::SystemIdent(_)) {
                checks.push(self.parse_system_timing_check()?);
            } else if self.at_punct("(")
                && matches!(self.peek_at(1), Tok::Ident(s) if s == "posedge" || s == "negedge")
            {
                // `edge_sensitive_path_declaration`, Annex A.7.2 — the
                // `[edge_identifier]` immediately after `(` is what
                // distinguishes it from a simple path declaration.
                let item_start = self.peek_span();
                self.skip_specify_item();
                unsupported.push(UnsupportedSpecifyForm {
                    span: item_start,
                    construct: "edge-sensitive path declaration",
                    clause: "A.7.2",
                });
            } else if self.at_punct("(") {
                paths.push(self.parse_path_declaration()?);
            } else {
                return Err(self.unexpected(
                    "a specify_item: specparam, path, or system timing check declaration (A.7.1)"
                        .to_string(),
                ));
            }
        }
        let end = self.expect_kw("endspecify")?;
        Ok(SpecifyBlock {
            span: PreprocessedSpan::new(start.start, end.end),
            specparams,
            paths,
            checks,
            unsupported,
        })
    }

    /// Consumes tokens up to and including the next top-level `;` — keeps
    /// the cursor synchronized past a recognized-but-unsupported
    /// `specify_item` (T17) without deep-parsing its internals; the item
    /// is still typed (construct + clause), never an unread span.
    fn skip_specify_item(&mut self) {
        while !self.at_punct(";") && !self.at_eof() {
            self.bump();
        }
        if self.at_punct(";") {
            self.bump();
        }
    }

    /// `timing_check_event`/`controlled_timing_check_event`, Annex A.7.5.3.
    /// `edge_required` distinguishes the mandatory-edge `controlled_*` form
    /// ($width/$period's sole event) from the optional-edge ordinary form.
    /// The `&&& timing_check_condition` qualifier is out of this task's
    /// scope.
    fn parse_timing_check_event(&mut self, edge_required: bool) -> Result<EventSpec, ParseError> {
        let start = self.peek_span();
        let edge = if self.eat_kw("posedge") {
            Some(EdgeIdent::Posedge)
        } else if self.eat_kw("negedge") {
            Some(EdgeIdent::Negedge)
        } else {
            None
        };
        if edge_required && edge.is_none() {
            return Err(self.unexpected(
                "'posedge' or 'negedge' (A.7.5.3 controlled_timing_check_event)".to_string(),
            ));
        }
        let (terminal, tspan) = self.expect_ident()?;
        Ok(EventSpec { span: PreprocessedSpan::new(start.start, tspan.end), edge, terminal })
    }

    /// `system_timing_check`, Annex A.7.5.1 — the eight kinds LRMW3-07
    /// scopes: `$setup`/`$hold`/`$setuphold`/`$width`/`$period`/
    /// `$recovery`/`$removal`/`$skew`. An unrecognized `$…` (including the
    /// in-LRM-but-out-of-scope `$recrem`/`$timeskew`/`$fullskew`/
    /// `$nochange`) fails loud naming it, never silently accepted.
    fn parse_system_timing_check(&mut self) -> Result<TimingCheckDecl, ParseError> {
        let (name, start) = match self.peek().clone() {
            Tok::SystemIdent(s) => (s, self.bump().span),
            _ => unreachable!("caller checked at_system_ident"),
        };
        let kind = match name.as_str() {
            "setup" => TimingCheckKind::Setup,
            "hold" => TimingCheckKind::Hold,
            "setuphold" => TimingCheckKind::SetupHold,
            "width" => TimingCheckKind::Width,
            "period" => TimingCheckKind::Period,
            "recovery" => TimingCheckKind::Recovery,
            "removal" => TimingCheckKind::Removal,
            "skew" => TimingCheckKind::Skew,
            _ => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedToken {
                        expected: "a system timing check: $setup/$hold/$setuphold/$width/$period/$recovery/$removal/$skew (A.7.5.1)".to_string(),
                        found: format!("'${name}'"),
                    },
                    start,
                ));
            }
        };
        self.expect_punct("(")?;
        let (reference_event, data_event, mut limits): (EventSpec, Option<EventSpec>, Vec<Expr>) = match kind {
            TimingCheckKind::Setup => {
                let d = self.parse_timing_check_event(false)?;
                self.expect_punct(",")?;
                let r = self.parse_timing_check_event(false)?;
                self.expect_punct(",")?;
                let limit = self.parse_expr()?;
                (r, Some(d), vec![limit])
            }
            TimingCheckKind::Hold
            | TimingCheckKind::Recovery
            | TimingCheckKind::Removal
            | TimingCheckKind::Skew => {
                let r = self.parse_timing_check_event(false)?;
                self.expect_punct(",")?;
                let d = self.parse_timing_check_event(false)?;
                self.expect_punct(",")?;
                let limit = self.parse_expr()?;
                (r, Some(d), vec![limit])
            }
            TimingCheckKind::SetupHold => {
                let r = self.parse_timing_check_event(false)?;
                self.expect_punct(",")?;
                let d = self.parse_timing_check_event(false)?;
                self.expect_punct(",")?;
                let setup_limit = self.parse_expr()?;
                self.expect_punct(",")?;
                let hold_limit = self.parse_expr()?;
                (r, Some(d), vec![setup_limit, hold_limit])
            }
            TimingCheckKind::Width | TimingCheckKind::Period => {
                let r = self.parse_timing_check_event(true)?;
                self.expect_punct(",")?;
                let limit = self.parse_expr()?;
                (r, None, vec![limit])
            }
        };
        // `[, [notifier]]` — `$width` additionally allows an optional
        // `threshold` before the notifier (A.7.5.1/A.7.5.2); the threshold
        // is parsed (so the argument list stays well-formed) but not yet
        // enforced (LRMW3-07 scope is the limit, not the threshold).
        let mut notifier = None;
        if matches!(kind, TimingCheckKind::Width) && self.eat_punct(",") {
            if !self.at_punct(")") {
                let _threshold = self.parse_expr()?;
                if self.eat_punct(",") && !self.at_punct(")") {
                    let (n, _) = self.expect_ident()?;
                    notifier = Some(n);
                }
            }
        } else if self.eat_punct(",") && !self.at_punct(")") {
            let (n, _) = self.expect_ident()?;
            notifier = Some(n);
        }
        let end = self.expect_punct(")")?;
        self.expect_punct(";")?;
        limits.shrink_to_fit();
        Ok(TimingCheckDecl {
            span: PreprocessedSpan::new(start.start, end.end),
            kind,
            reference_event,
            data_event,
            limits,
            notifier,
        })
    }

    /// `specify_input_terminal_descriptor`/`specify_output_terminal_descriptor`,
    /// Annex A.7.3 — a bare terminal identifier. The optional
    /// `[constant_range_expression]` bit-select is out of this task's
    /// scope: a source using one fails to parse (never silently drops the
    /// index).
    fn parse_specify_terminal(&mut self) -> Result<String, ParseError> {
        let (name, _) = self.expect_ident()?;
        Ok(name)
    }

    /// `simple_path_declaration`, Annex A.7.2/A.7.3/A.7.4 — parallel
    /// (`specify_input_terminal_descriptor [polarity] => output`) or
    /// full-connection (`list_of_path_inputs [polarity] *> list_of_path_outputs`).
    fn parse_path_declaration(&mut self) -> Result<PathDecl, ParseError> {
        let start = self.expect_punct("(")?;
        let mut inputs = vec![self.parse_specify_terminal()?];
        while self.eat_punct(",") {
            inputs.push(self.parse_specify_terminal()?);
        }
        // `polarity_operator`, Annex A.7.2 — parsed and discarded: it
        // selects which transition of a *state-dependent* path applies,
        // and a simple path has no state dependence for it to modulate.
        let _ = self.eat_punct("+") || self.eat_punct("-");
        let full = if self.eat_punct("=>") {
            false
        } else if self.eat_punct("*>") {
            true
        } else {
            return Err(self.unexpected("'=>' or '*>' (A.7.2 simple_path_declaration)".to_string()));
        };
        if !full && inputs.len() != 1 {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedToken {
                    expected: "a single input terminal before '=>' (A.7.2 parallel_path_description) — use '*>' for a full-connection path".to_string(),
                    found: format!("{} input terminals", inputs.len()),
                },
                start,
            ));
        }
        let mut outputs = vec![self.parse_specify_terminal()?];
        while self.eat_punct(",") {
            outputs.push(self.parse_specify_terminal()?);
        }
        self.expect_punct(")")?;
        let delays = self.parse_path_delay_value()?;
        if !matches!(delays.len(), 1 | 2 | 3 | 6 | 12) {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedToken {
                    expected: "1, 2, 3, 6 or 12 delay expressions (A.7.4 list_of_path_delay_expressions)".to_string(),
                    found: format!("{} delay expressions", delays.len()),
                },
                start,
            ));
        }
        let end = self.expect_punct(";")?;
        Ok(PathDecl { span: PreprocessedSpan::new(start.start, end.end), inputs, outputs, full, delays })
    }

    /// `path_delay_value`, Annex A.7.4 — `= expr` or `= (expr {, expr})`.
    fn parse_path_delay_value(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.expect_punct("=")?;
        if self.eat_punct("(") {
            let mut delays = vec![self.parse_expr()?];
            while self.eat_punct(",") {
                delays.push(self.parse_expr()?);
            }
            self.expect_punct(")")?;
            Ok(delays)
        } else {
            Ok(vec![self.parse_expr()?])
        }
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
            span: PreprocessedSpan::new(start.start, end.end),
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
        Ok(GateInstance { span: PreprocessedSpan::new(start.start, end.end), name, terminals })
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
        Ok(PrimitiveDecl { span: PreprocessedSpan::new(start.start, end.end), attrs, name, ports, table, is_sequential })
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
                span: PreprocessedSpan::new(start.start, end.end),
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
            return Ok(UdpPortDecl::Input { span: PreprocessedSpan::new(start.start, end.end), attrs, names });
        }
        self.expect_kw("reg")?;
        let (name, _) = self.expect_ident()?;
        let end = self.expect_punct(";")?;
        Ok(UdpPortDecl::Reg { span: PreprocessedSpan::new(start.start, end.end), attrs, name })
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
        Ok(UdpTableRow { span: PreprocessedSpan::new(start.start, end.end), inputs, current_state, next_state })
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
    /// sites). §6.4 Syntax 6-4's own production is `` `paramset`
    /// paramset_identifier module_or_paramset_identifier `;` `` — no
    /// parentheses around the second identifier (confirmed by §6.4's own
    /// worked example: `paramset nch nmos3; // default paramset`). This
    /// parser also accepts `paramset name (module);` as a sanctioned
    /// extension (T26) for callers already relying on it — the LRM form
    /// (T61, LRMW2-58) is now accepted too, both producing the identical
    /// `ParamsetDecl`.
    pub fn parse_paramset_decl(&mut self) -> Result<ParamsetDecl, ParseError> {
        let start = self.expect_kw("paramset")?;
        let (name, _) = self.expect_ident()?;
        let parenthesized = self.at_punct("(");
        if parenthesized {
            self.expect_punct("(")?;
        }
        let (module_name, _) = self.expect_ident()?;
        if parenthesized {
            self.expect_punct(")")?;
        }
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
        Ok(ParamsetDecl { span: PreprocessedSpan::new(start.start, end.end), name, module_name, params, statements })
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
        Ok(ParamsetStatement { span: PreprocessedSpan::new(start.start, end.end), target, value })
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
        Ok(ConnectrulesDecl { span: PreprocessedSpan::new(start.start, end.end), name, rules, resolutions })
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
                    span: PreprocessedSpan::new(start.start, end.end),
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
                span: PreprocessedSpan::new(start.start, end.end),
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
            span: PreprocessedSpan::new(start.start, end.end),
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
        Ok(ConfigDecl { span: PreprocessedSpan::new(start.start, end.end), name, body: String::new() })
    }

    // ─────────────────────────────── source file ───────────────────────────

    /// `source_text`, Annex A.1.1 — the grammar's top-level entry point:
    /// zero or more `description`s until end of file.
    pub fn parse_source_file(&mut self) -> Result<SourceFile, ParseError> {
        let mut items = vec![];
        while !self.at_eof() {
            items.push(self.parse_item()?);
        }
        Ok(SourceFile { items, directives: Default::default(), source_provenance: Default::default() })
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
            Some("const") => Ok(Item::Const(self.parse_const_decl()?)),
            _ => Err(self.unexpected(
                "a top-level declaration (discipline, nature, module, macromodule, \
                 connectmodule, paramset, connectrules, config, primitive, or const)"
                    .to_string(),
            )),
        }
    }
}

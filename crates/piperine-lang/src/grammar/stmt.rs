//! Statement grammar — LRM chapter 5 (analog) plus the 1364 procedural
//! statements Verilog-AMS inherits for digital `always`/`initial` bodies,
//! Annex A.6.4/A.6.6/A.6.7/A.6.8.

use crate::ast::{BlockItemDecl, CaseArm, CaseKind, EventExpr, Expr, Stmt};
use crate::error::{ParseError, PreprocessedSpan};
use crate::grammar::cursor::Grammar;
use crate::lexer::Tok;

impl Grammar {
    /// `statement`, Annex A.6.4 — the grammar's statement entry point.
    ///
    /// §2.9's own worked examples (Example 1/2) show `(* full_case,
    /// parallel_case *)` prefixing a `case` statement — a statement-level
    /// `attribute_instance` (LRMW2-39, T43). This is the one concrete
    /// statement position §2.9's examples actually exercise (the other
    /// examples are all declaration/item/expression positions, already
    /// handled at their own sites); a leading `(* ... *)` here still
    /// parses (never a syntax error) ahead of any statement kind, but is
    /// only carried through into the AST for `case`/`casex`/`casez`/
    /// `analog_case` (`Stmt::Case.attrs`) — the position the LRM's prose
    /// actually attributes semantics to. Any other statement kind's
    /// leading attributes are accepted and discarded, the same disclosed
    /// scope limit `$discontinuity`'s degree (LRMW2-38) already has: a
    /// pure simulator hint with no dedicated AST slot yet.
    pub fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek_span();
        let attrs = self.parse_attr_instances()?;
        if !attrs.is_empty() {
            if self.at_kw("case") || self.at_kw("casex") || self.at_kw("casez") || self.at_kw("analog_case") {
                return self.parse_case(span, attrs);
            }
            return self.parse_stmt();
        }
        if self.eat_punct(";") {
            return Ok(Stmt::Null { span });
        }
        if self.at_kw("begin") {
            return self.parse_block();
        }
        if self.at_kw("if") {
            return self.parse_if();
        }
        if self.at_kw("case") || self.at_kw("casex") || self.at_kw("casez") || self.at_kw("analog_case") {
            return self.parse_case(span, Vec::new());
        }
        if self.at_kw("for") {
            return self.parse_for();
        }
        if self.at_kw("while") {
            return self.parse_while();
        }
        if self.at_kw("repeat") {
            return self.parse_repeat();
        }
        if self.at_kw("forever") {
            return self.parse_forever();
        }
        if self.at_kw("disable") {
            return self.parse_disable();
        }
        if self.at_punct("@") {
            return self.parse_event_control();
        }
        if self.at_punct("->") {
            return self.parse_event_trigger();
        }
        if let Tok::SystemIdent(_) = self.peek() {
            return self.parse_task_call();
        }
        self.parse_assign_or_contribution()
    }

    /// `'begin' [ ':' identifier ] block_item_declaration* statement* 'end'`
    /// — Annex A.6.3.
    fn parse_block(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("begin")?;
        let name = if self.eat_punct(":") { Some(self.expect_ident()?.0) } else { None };
        let mut decls = vec![];
        while let Some(d) = self.try_parse_block_item_decl()? {
            decls.push(d);
        }
        let mut stmts = vec![];
        while !self.at_kw("end") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'end'".to_string()));
            }
            stmts.push(self.parse_stmt()?);
        }
        let end = self.expect_kw("end")?;
        Ok(Stmt::Block { span: PreprocessedSpan::new(start.start, end.end), name, decls, stmts })
    }

    /// `block_item_declaration`, Annex A.2.8 — the scalar-variable subset
    /// this crate's scope covers (parameter/localparam block-scoped forms
    /// are item-grammar concerns, T9). Returns `Ok(None)` when the next
    /// token does not start a block item declaration, so callers can loop
    /// until the statement list begins.
    pub(crate) fn try_parse_block_item_decl(&mut self) -> Result<Option<BlockItemDecl>, ParseError> {
        let attrs = self.parse_attr_instances()?;
        let kind = if self.eat_kw("integer") {
            0
        } else if self.eat_kw("real") {
            1
        } else if self.eat_kw("time") {
            2
        } else if self.eat_kw("realtime") {
            3
        } else if self.eat_kw("reg") {
            4
        } else {
            if !attrs.is_empty() {
                return Err(self.unexpected("a block item declaration".to_string()));
            }
            return Ok(None);
        };
        // A block-scoped `reg` never carries a discipline name — disciplines
        // qualify *nets* (§3.6.3), not procedural registers — so this is
        // always `None`; the field exists to match `ast::BlockItemDecl::Reg`
        // and to leave room for the LRM 1364 `reg [discipline] name` form
        // if it turns out to matter for a later requirement.
        let discipline = None;
        let names = self.parse_ident_list()?;
        self.expect_punct(";")?;
        Ok(Some(match kind {
            0 => BlockItemDecl::Integer { attrs, names },
            1 => BlockItemDecl::Real { attrs, names },
            2 => BlockItemDecl::Time { attrs, names },
            3 => BlockItemDecl::Realtime { attrs, names },
            _ => BlockItemDecl::Reg { attrs, discipline, names },
        }))
    }

    /// `identifier { ',' identifier }` — used by block-item declarations.
    fn parse_ident_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut names = vec![self.expect_ident()?.0];
        while self.eat_punct(",") {
            names.push(self.expect_ident()?.0);
        }
        Ok(names)
    }

    /// `'if' '(' expr ')' statement [ 'else' statement ]` — Annex A.6.6.
    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("if")?;
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let then_branch = Box::new(self.parse_stmt()?);
        let else_branch =
            if self.eat_kw("else") { Some(Box::new(self.parse_stmt()?)) } else { None };
        let end = else_branch.as_ref().map(|s| s.span()).unwrap_or_else(|| then_branch.span());
        Ok(Stmt::If { span: PreprocessedSpan::new(start.start, end.end), cond, then_branch, else_branch })
    }

    /// `('case'|'casex'|'casez'|'analog_case') '(' expr ')' case_item*
    /// 'endcase'` — Annex A.6.7.
    /// `attr_span` is the span of this statement's leading token (before
    /// any `(* ... *)` group was consumed) — the case keyword's own span
    /// when there were no attributes — so `Stmt::Case`'s span still starts
    /// at the attribute instance, matching every other attributed
    /// declaration site's convention (`doc_at` is looked up against the
    /// pre-attribute span too, `cursor.rs`'s own doc comment).
    fn parse_case(&mut self, attr_span: PreprocessedSpan, attrs: Vec<crate::ast::Attr>) -> Result<Stmt, ParseError> {
        let (kind, kw_span) = if self.at_kw("case") {
            (CaseKind::Case, self.bump().span)
        } else if self.at_kw("casex") {
            (CaseKind::Casex, self.bump().span)
        } else if self.at_kw("casez") {
            (CaseKind::Casez, self.bump().span)
        } else {
            (CaseKind::AnalogCase, self.bump().span)
        };
        let start = if attrs.is_empty() { kw_span } else { attr_span };
        self.expect_punct("(")?;
        let selector = self.parse_expr()?;
        self.expect_punct(")")?;
        let mut arms = vec![];
        while !self.at_kw("endcase") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(self.eof_error("'endcase'".to_string()));
            }
            arms.push(self.parse_case_arm()?);
        }
        let end = self.expect_kw("endcase")?;
        Ok(Stmt::Case { span: PreprocessedSpan::new(start.start, end.end), kind, selector, arms, attrs })
    }

    /// One `case_item`: `expr {',' expr} ':' stmt` or `'default' [':'] stmt`.
    fn parse_case_arm(&mut self) -> Result<CaseArm, ParseError> {
        let start = self.peek_span();
        if self.eat_kw("default") {
            self.eat_punct(":");
            let body = Box::new(self.parse_stmt()?);
            return Ok(CaseArm { span: PreprocessedSpan::new(start.start, body.span().end), labels: None, body });
        }
        let mut labels = vec![self.parse_expr()?];
        while self.eat_punct(",") {
            labels.push(self.parse_expr()?);
        }
        self.expect_punct(":")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(CaseArm { span: PreprocessedSpan::new(start.start, body.span().end), labels: Some(labels), body })
    }

    /// `'for' '(' assign ';' expr ';' assign ')' statement` — Annex A.6.8.
    fn parse_for(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("for")?;
        self.expect_punct("(")?;
        let init = Box::new(self.parse_assign_or_contribution()?);
        self.eat_punct(";");
        let cond = self.parse_expr()?;
        self.expect_punct(";")?;
        let step = Box::new(self.parse_assign_or_contribution_no_semi()?);
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For { span: PreprocessedSpan::new(start.start, body.span().end), init, cond, step, body })
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("while")?;
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { span: PreprocessedSpan::new(start.start, body.span().end), cond, body })
    }

    fn parse_repeat(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("repeat")?;
        self.expect_punct("(")?;
        let count = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::Repeat { span: PreprocessedSpan::new(start.start, body.span().end), count, body })
    }

    fn parse_forever(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("forever")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::Forever { span: PreprocessedSpan::new(start.start, body.span().end), body })
    }

    fn parse_disable(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("disable")?;
        let (name, name_span) = self.expect_ident()?;
        self.expect_punct(";")?;
        Ok(Stmt::Disable { span: PreprocessedSpan::new(start.start, name_span.end), name })
    }

    /// `event_trigger ::= '->' hierarchical_event_identifier ';'` — §5.10.4,
    /// Annex A.6.5. Legal in both analog and digital statement positions
    /// (this method is the shared statement entry point, so no separate
    /// analog/digital dispatch is needed) — LRMC-C5-04. Only a bare
    /// identifier is supported (no hierarchical `.`-joined path yet,
    /// matching `defparam`'s own target scope note).
    fn parse_event_trigger(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_punct("->")?;
        let (name, _) = self.expect_ident()?;
        let end = self.expect_punct(";")?;
        Ok(Stmt::EventTrigger { span: PreprocessedSpan::new(start.start, end.end), name })
    }

    /// `'@' ( '(' event_expression ')' | '*' ) statement` — §5.6/Annex A.6.5.
    /// `event_expression` covers `posedge`/`negedge`-qualified signals, a
    /// bare expression, and the analog event functions (`cross`/`above`/
    /// `timer`/`initial_step`/`final_step`), which parse as ordinary calls
    /// via `parse_expr` and are wrapped as [`EventExpr::Level`].
    fn parse_event_control(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_punct("@")?;
        let events = if self.eat_punct("*") {
            vec![EventExpr::ImplicitAll]
        } else if self.eat_punct("(*") {
            // `@(*)` lexes as `@`, `(*`, `)` — the lexer's attribute-open
            // token (§2.9) is lexically identical to `(` immediately
            // followed by `*`, which is exactly this construct too.
            self.expect_punct(")")?;
            vec![EventExpr::ImplicitAll]
        } else {
            self.expect_punct("(")?;
            if self.eat_punct("*") {
                self.expect_punct(")")?;
                vec![EventExpr::ImplicitAll]
            } else {
                let mut events = vec![self.parse_event_expr()?];
                while self.eat_kw("or") || self.eat_punct(",") {
                    events.push(self.parse_event_expr()?);
                }
                self.expect_punct(")")?;
                events
            }
        };
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::EventControl { span: PreprocessedSpan::new(start.start, body.span().end), events, body })
    }

    fn parse_event_expr(&mut self) -> Result<EventExpr, ParseError> {
        if self.eat_kw("posedge") {
            return Ok(EventExpr::Posedge(self.parse_expr()?));
        }
        if self.eat_kw("negedge") {
            return Ok(EventExpr::Negedge(self.parse_expr()?));
        }
        Ok(EventExpr::Level(self.parse_expr()?))
    }

    /// A `$`-system-task call used as a statement — chapter 9.
    fn parse_task_call(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek_span();
        let Tok::SystemIdent(name) = self.peek().clone() else {
            return Err(self.unexpected("a system task call".to_string()));
        };
        self.bump();
        let mut args = vec![];
        if self.eat_punct("(") {
            if !self.at_punct(")") {
                args.push(self.parse_expr()?);
                while self.eat_punct(",") {
                    args.push(self.parse_expr()?);
                }
            }
            self.expect_punct(")")?;
        }
        let end = self.expect_punct(";")?;
        Ok(Stmt::TaskCall { span: PreprocessedSpan::new(span.start, end.end), name, args })
    }

    /// `lvalue '=' expr ';'` / `lvalue '<=' expr ';'` / `lvalue '<+' expr
    /// ';'` — blocking assignment (Annex A.6.4), non-blocking assignment
    /// (1364, digital contexts), and the contribution statement (§5.4,
    /// Annex A.6.10). Also handles a bare procedural/task call used as a
    /// statement (`name(args);` or `name;`).
    fn parse_assign_or_contribution(&mut self) -> Result<Stmt, ParseError> {
        let stmt = self.parse_assign_or_contribution_no_semi()?;
        self.expect_punct(";")?;
        Ok(stmt)
    }

    fn parse_assign_or_contribution_no_semi(&mut self) -> Result<Stmt, ParseError> {
        // An lvalue, not a full expression — parsing `q` as `parse_expr()`
        // would let the Pratt loop swallow a following `<=` as the
        // less-or-equal *comparison* operator instead of leaving it for
        // the assignment-operator check below.
        let target = self.parse_lvalue()?;
        let span_start = target.span().start;
        // `branch_lvalue : indirect_expression == analog_expression ;`
        // (§5.6.7, Annex A.6.10, LRMW2-41, T46) — `target` just parsed is
        // exactly `branch_lvalue` (an access-function call, `parse_lvalue`
        // above). The colon here is unambiguous: at this point in the
        // grammar `target` is a complete lvalue with no dangling ternary
        // `? ... :` to close (that closes inside `parse_expr`, never here).
        if self.eat_punct(":") {
            // `parse_lvalue` (⇒ `parse_postfix`, no binary-operator Pratt
            // loop) so a following `==` is left for the `expect_punct`
            // below instead of being swallowed into
            // `Binary{Eq, probe, rhs}` by a general `parse_expr` call.
            let probe = self.parse_lvalue()?;
            if !is_indirect_probe_shape(&probe) {
                return Err(self.unexpected(
                    "an access-function call, or ddt()/idt()/idtmod() applied to one, as the \
                     indirect contribution's probe expression (LRM §5.6.7)"
                        .to_string(),
                ));
            }
            self.expect_punct("==")?;
            let rhs = self.parse_expr()?;
            return Ok(Stmt::IndirectContribution {
                span: PreprocessedSpan::new(span_start, rhs.span().end),
                lhs: target,
                probe,
                rhs,
            });
        }
        if self.eat_punct("<+") {
            let value = self.parse_expr()?;
            return Ok(Stmt::Contribution {
                span: PreprocessedSpan::new(span_start, value.span().end),
                target,
                value,
            });
        }
        if self.eat_punct("<=") {
            let value = self.parse_expr()?;
            return Ok(Stmt::NonBlockingAssign {
                span: PreprocessedSpan::new(span_start, value.span().end),
                target,
                value,
            });
        }
        if self.eat_punct("=") {
            let value = self.parse_expr()?;
            return Ok(Stmt::Assign { span: PreprocessedSpan::new(span_start, value.span().end), target, value });
        }
        // A bare call used as a statement, e.g. `my_task(a, b);`.
        if let crate::ast::Expr::Call { span, name, args } = target {
            return Ok(Stmt::TaskCall { span, name, args });
        }
        Err(self.unexpected("'=', '<=', or '<+'".to_string()))
    }
}

/// Whether `probe` is a syntactically legal §5.6.7 `indirect_expression`:
/// an access-function call (`V(in)`, a custom discipline access name, …),
/// or `ddt`/`idt`/`idtmod` applied to one. The grammar has no symbol
/// table (same limitation `parse_primary`'s own doc comment notes for
/// access-function/custom-access-name disambiguation), so a `Call` whose
/// name is not one of the three time-domain operators is accepted here
/// (it may be a legitimate custom discipline access, or a genuine user
/// function call the elaborator will reject when it *does* have the
/// discipline registry) — this check only rejects shapes that can never
/// be a probe: literals, binary/ternary expressions, bare identifiers,
/// concatenations, and so on.
fn is_indirect_probe_shape(probe: &Expr) -> bool {
    match probe {
        Expr::AccessFunction { .. } | Expr::PortAccess { .. } => true,
        Expr::Call { name, args, .. } => {
            if matches!(name.as_str(), "ddt" | "idt" | "idtmod") {
                matches!(args.first(), Some(Expr::AccessFunction { .. } | Expr::PortAccess { .. }))
            } else {
                true
            }
        }
        _ => false,
    }
}

//! Statement grammar — LRM chapter 5 (analog) plus the 1364 procedural
//! statements Verilog-AMS inherits for digital `always`/`initial` bodies,
//! Annex A.6.4/A.6.6/A.6.7/A.6.8.

use crate::ast::{BlockItemDecl, CaseArm, CaseKind, EventExpr, Stmt};
use crate::error::{ParseError, Span};
use crate::grammar::cursor::Grammar;
use crate::lexer::Tok;

impl Grammar {
    /// `statement`, Annex A.6.4 — the grammar's statement entry point.
    pub fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek_span();
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
            return self.parse_case();
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
        Ok(Stmt::Block { span: Span::new(start.start, end.end), name, decls, stmts })
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
        Ok(Stmt::If { span: Span::new(start.start, end.end), cond, then_branch, else_branch })
    }

    /// `('case'|'casex'|'casez'|'analog_case') '(' expr ')' case_item*
    /// 'endcase'` — Annex A.6.7.
    fn parse_case(&mut self) -> Result<Stmt, ParseError> {
        let (kind, start) = if self.at_kw("case") {
            (CaseKind::Case, self.bump().span)
        } else if self.at_kw("casex") {
            (CaseKind::Casex, self.bump().span)
        } else if self.at_kw("casez") {
            (CaseKind::Casez, self.bump().span)
        } else {
            (CaseKind::AnalogCase, self.bump().span)
        };
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
        Ok(Stmt::Case { span: Span::new(start.start, end.end), kind, selector, arms })
    }

    /// One `case_item`: `expr {',' expr} ':' stmt` or `'default' [':'] stmt`.
    fn parse_case_arm(&mut self) -> Result<CaseArm, ParseError> {
        let start = self.peek_span();
        if self.eat_kw("default") {
            self.eat_punct(":");
            let body = Box::new(self.parse_stmt()?);
            return Ok(CaseArm { span: Span::new(start.start, body.span().end), labels: None, body });
        }
        let mut labels = vec![self.parse_expr()?];
        while self.eat_punct(",") {
            labels.push(self.parse_expr()?);
        }
        self.expect_punct(":")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(CaseArm { span: Span::new(start.start, body.span().end), labels: Some(labels), body })
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
        Ok(Stmt::For { span: Span::new(start.start, body.span().end), init, cond, step, body })
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("while")?;
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { span: Span::new(start.start, body.span().end), cond, body })
    }

    fn parse_repeat(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("repeat")?;
        self.expect_punct("(")?;
        let count = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::Repeat { span: Span::new(start.start, body.span().end), count, body })
    }

    fn parse_forever(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("forever")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::Forever { span: Span::new(start.start, body.span().end), body })
    }

    fn parse_disable(&mut self) -> Result<Stmt, ParseError> {
        let start = self.expect_kw("disable")?;
        let (name, name_span) = self.expect_ident()?;
        self.expect_punct(";")?;
        Ok(Stmt::Disable { span: Span::new(start.start, name_span.end), name })
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
        Ok(Stmt::EventControl { span: Span::new(start.start, body.span().end), events, body })
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
        Ok(Stmt::TaskCall { span: Span::new(span.start, end.end), name, args })
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
        if self.eat_punct("<+") {
            let value = self.parse_expr()?;
            return Ok(Stmt::Contribution {
                span: Span::new(span_start, value.span().end),
                target,
                value,
            });
        }
        if self.eat_punct("<=") {
            let value = self.parse_expr()?;
            return Ok(Stmt::NonBlockingAssign {
                span: Span::new(span_start, value.span().end),
                target,
                value,
            });
        }
        if self.eat_punct("=") {
            let value = self.parse_expr()?;
            return Ok(Stmt::Assign { span: Span::new(span_start, value.span().end), target, value });
        }
        // A bare call used as a statement, e.g. `my_task(a, b);`.
        if let crate::ast::Expr::Call { span, name, args } = target {
            return Ok(Stmt::TaskCall { span, name, args });
        }
        Err(self.unexpected("'=', '<=', or '<+'".to_string()))
    }
}

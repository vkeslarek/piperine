//! Statements and blocks — follows `veriloga.ungram`.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn stmt(&mut self) -> PResult<Stmt> {
        let attrs = self.attrs()?;
        self.stmt_with_attrs(attrs)
    }

    pub(super) fn stmt_with_attrs(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        if self.eat(&Tok::Semi) { return Ok(Stmt::Empty(EmptyStmt { attrs })); }
        if self.at(&Tok::At)               { return self.event_stmt(attrs); }
        if self.at_kw("begin") || self.at(&Tok::LBrace) { return self.block_stmt(attrs); }
        if self.at(&Tok::Hash) || self.at(&Tok::At) {
            let tc = self.opt_timing_control()?.unwrap();
            let stmt = self.stmt_with_attrs(Vec::new())?;
            return Ok(Stmt::TimingControl(TimingControlStmt { attrs, control: tc, stmt: Box::new(stmt) }));
        }
        if self.at_kw("if")                { return self.if_stmt(attrs); }
        if self.at_kw("while")             { return self.while_stmt(attrs); }
        if self.at_kw("for")               { return self.for_stmt(attrs); }
        if self.at_kw("repeat")            { return self.repeat_stmt(attrs); }
        if self.at_kw("forever")           { return self.forever_stmt(attrs); }
        if self.at_kw("case") || self.at_kw("casex") || self.at_kw("casez") {
            return self.case_stmt(attrs);
        }
        if self.at_kw("wait")              { return self.wait_stmt(attrs); }
        if self.at_kw("fork")              { return self.fork_stmt(attrs); }
        if self.at_kw("disable")           { return self.disable_stmt(attrs); }
        if self.at(&Tok::Arrow)            { return self.event_trigger_stmt(attrs); }
        if self.at_kw("assign") || self.at_kw("force") { return self.procedural_assign_stmt(attrs); }
        if self.at_kw("deassign") || self.at_kw("release") { return self.procedural_deassign_stmt(attrs); }
        self.assign_or_expr_stmt(attrs)
    }

    /// Match a `=`, `<+`.
    /// Returns `None` when the next token is not an assignment operator.
    fn assign_op_token(&mut self) -> Option<AssignOp> {
        if self.eat(&Tok::Assign)    { Some(AssignOp::Eq) }
        else if self.eat(&Tok::Contrib)   { Some(AssignOp::Contrib) }
        else { None }
    }

    fn assign_or_expr_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let lval = self.expr()?;
        if self.eat(&Tok::Colon) {
            let indirect_expr = self.expr()?;
            self.expect(&Tok::EqEq)?;
            let rvalue = self.expr()?;
            self.expect(&Tok::Semi)?;
            return Ok(Stmt::IndirectContrib(IndirectContribution { attrs, lvalue: lval, indirect_expr, rvalue }));
        }
        if self.eat(&Tok::Le) {
            let delay_or_event = self.opt_timing_control()?;
            let rvalue = self.expr()?;
            self.eat(&Tok::Semi);
            return Ok(Stmt::NonBlockingAssign(NonBlockingAssignStmt { attrs, lvalue: lval, delay_or_event, rvalue }));
        }
        let stmt = match self.assign_op_token() {
            Some(op) => {
                let delay_or_event = self.opt_timing_control()?;
                let rval = self.expr()?;
                Stmt::Assign(AssignStmt { attrs, delay_or_event, assign: Assign { lval, op, rval } })
            }
            None => Stmt::Expr(ExprStmt { attrs, expr: lval }),
        };
        self.eat(&Tok::Semi);
        Ok(stmt)
    }

    fn for_assign(&mut self) -> PResult<Assign> {
        let lval = self.expr()?;
        let op = self.assign_op_token()
            .ok_or_else(|| "expected assignment operator in for-clause".to_string())?;
        let rval = self.expr()?;
        Ok(Assign { lval, op, rval })
    }

    fn repeat_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("repeat")?;
        self.expect(&Tok::LParen)?;
        let count = self.expr()?;
        self.expect(&Tok::RParen)?;
        let body = Box::new(self.stmt()?);
        Ok(Stmt::Repeat(RepeatStmt { attrs, count, body }))
    }

    fn forever_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("forever")?;
        let body = Box::new(self.stmt()?);
        Ok(Stmt::Forever(ForeverStmt { attrs, body }))
    }

    /// A block delimited by `begin`/`end` or by `{`/`}` — both interchangeable.
    /// A `begin` block may carry a `: label`; the brace form does not.
    fn block_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let brace = if self.eat(&Tok::LBrace) { true }
                    else { self.expect_kw("begin")?; false };
        let label = if !brace && self.eat(&Tok::Colon) { Some(self.name()?) } else { None };
        let mut items = Vec::new();
        loop {
            let at_close = if brace { self.at(&Tok::RBrace) } else { self.at_kw("end") };
            if at_close || self.at_end() { break; }
            items.push(self.block_item()?);
        }
        if brace { self.expect(&Tok::RBrace)?; } else { self.expect_kw("end")?; }
        Ok(Stmt::Block(BlockStmt { attrs, label, items }))
    }

    pub(super) fn block_item(&mut self) -> PResult<BlockItem> {
        let start = self.span_start();
        let attrs = self.attrs()?;
        if self.at_kw("parameter") || self.at_kw("localparam") {
            Ok(BlockItem::ParamDecl(self.param_decl(attrs, start)?))
        } else if !self.at_stmt_kw() && (self.is_type_kw() || self.at_kw("genvar")) {
            // `is_type_kw()` treats `Ident Ident` as a custom-type declaration,
            // which would misread a nested statement like `begin n = ...`.
            // Statement-leading keywords take precedence over the var-decl guess.
            Ok(BlockItem::VarDecl(self.var_decl(attrs, start)?))
        } else {
            Ok(BlockItem::Stmt(self.stmt_with_attrs(attrs)?))
        }
    }

    /// True when the cursor is on a keyword that begins a statement (not a
    /// type name). Used by `block_item` to keep `is_type_kw()`'s `Ident Ident`
    /// heuristic from swallowing nested compound statements.
    pub(super) fn at_stmt_kw(&self) -> bool {
        self.at_any_kw(&[
            "begin", "if", "while", "for", "case", "casex", "casez",
            "repeat", "forever", "wait", "fork", "disable",
            "assign", "force", "deassign", "release",
            "task",
        ]) || self.at(&Tok::Arrow)
    }

    fn if_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("if")?;
        self.expect(&Tok::LParen)?;
        let condition = self.expr()?;
        self.expect(&Tok::RParen)?;
        let then_branch = Box::new(self.stmt()?);
        let else_branch = if self.eat_kw("else") { Some(Box::new(self.stmt()?)) } else { None };
        Ok(Stmt::If(IfStmt { attrs, condition, then_branch, else_branch }))
    }

    fn while_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("while")?;
        self.expect(&Tok::LParen)?;
        let condition = self.expr()?;
        self.expect(&Tok::RParen)?;
        let body = Box::new(self.stmt()?);
        Ok(Stmt::While(WhileStmt { attrs, condition, body }))
    }

    fn for_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("for")?;
        self.expect(&Tok::LParen)?;
        let init = Box::new(Stmt::Assign(AssignStmt {
            attrs: Vec::new(),
            delay_or_event: None,
            assign: self.for_assign()?,
        }));
        self.expect(&Tok::Semi)?;
        let condition = self.expr()?;
        self.expect(&Tok::Semi)?;
        let incr = Box::new(Stmt::Assign(AssignStmt {
            attrs: Vec::new(),
            delay_or_event: None,
            assign: self.for_assign()?,
        }));
        self.expect(&Tok::RParen)?;
        let for_body = Box::new(self.stmt()?);
        Ok(Stmt::For(ForStmt { attrs, init, condition, incr, for_body }))
    }

    fn case_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let kind = if self.eat_kw("casex") {
            CaseKind::Casex
        } else if self.eat_kw("casez") {
            CaseKind::Casez
        } else {
            self.expect_kw("case")?;
            CaseKind::Case
        };
        self.expect(&Tok::LParen)?;
        let discriminant = self.expr()?;
        self.expect(&Tok::RParen)?;
        let mut cases = Vec::new();
        while !self.at_kw("endcase") && !self.at_end() {
            let item = if self.eat_kw("default") {
                self.eat(&Tok::Colon);
                CaseItem::Default
            } else {
                let mut exprs = vec![self.expr()?];
                while self.eat(&Tok::Comma) { exprs.push(self.expr()?); }
                self.expect(&Tok::Colon)?;
                CaseItem::Exprs(exprs)
            };
            cases.push(Case { item, stmt: Box::new(self.stmt()?) });
        }
        self.expect_kw("endcase")?;
        Ok(Stmt::Case(CaseStmt { attrs, kind, discriminant, cases }))
    }

    fn event_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect(&Tok::At)?;
        let event = if self.eat(&Tok::LParen) {
            let e = self.expr()?;
            self.expect(&Tok::RParen)?;
            e
        } else {
            self.expr()?
        };
        let stmt = Box::new(self.stmt()?);
        Ok(Stmt::Event(EventStmt { attrs, event, stmt }))
    }
    // ==========================================
    // Phase 4 Extensions
    // ==========================================

    fn wait_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("wait")?;
        self.expect(&Tok::LParen)?;
        let condition = self.expr()?;
        self.expect(&Tok::RParen)?;
        let stmt = Box::new(self.stmt()?);
        Ok(Stmt::Wait(WaitStmt { attrs, condition, stmt }))
    }

    fn fork_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("fork")?;
        let label = if self.eat(&Tok::Colon) { Some(self.name()?) } else { None };
        let mut items = Vec::new();
        while !self.at_kw("join") && !self.at_end() {
            items.push(self.block_item()?);
        }
        self.expect_kw("join")?;
        Ok(Stmt::Fork(ForkStmt { attrs, label, items }))
    }

    fn disable_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("disable")?;
        let target = self.path()?;
        self.expect(&Tok::Semi)?;
        Ok(Stmt::Disable(DisableStmt { attrs, target }))
    }

    fn event_trigger_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect(&Tok::Arrow)?;
        let event = self.path()?;
        self.expect(&Tok::Semi)?;
        Ok(Stmt::EventTrigger(EventTriggerStmt { attrs, event }))
    }

    fn procedural_assign_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let is_force = self.eat_kw("force");
        if !is_force { self.expect_kw("assign")?; }
        let lvalue = self.expr()?;
        self.expect(&Tok::Assign)?;
        let rvalue = self.expr()?;
        self.expect(&Tok::Semi)?;
        Ok(Stmt::ProceduralAssign(ProceduralAssignStmt { attrs, is_force, lvalue, rvalue }))
    }

    fn procedural_deassign_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let is_release = self.eat_kw("release");
        if !is_release { self.expect_kw("deassign")?; }
        let lvalue = self.expr()?;
        self.expect(&Tok::Semi)?;
        Ok(Stmt::ProceduralDeassign(ProceduralDeassignStmt { attrs, is_release, lvalue }))
    }

    pub(super) fn opt_timing_control(&mut self) -> PResult<Option<TimingControl>> {
        if self.eat(&Tok::Hash) {
            if self.eat(&Tok::LParen) {
                let expr = self.expr()?;
                self.expect(&Tok::RParen)?;
                Ok(Some(TimingControl::DelayParen(expr)))
            } else {
                Ok(Some(TimingControl::Delay(self.expr()?)))
            }
        } else if self.eat(&Tok::At) {
            if self.eat(&Tok::Star) {
                Ok(Some(TimingControl::Event(EventControl::Star)))
            } else if self.eat(&Tok::LParen) {
                if self.eat(&Tok::Star) {
                    self.expect(&Tok::RParen)?;
                    Ok(Some(TimingControl::Event(EventControl::Star)))
                } else {
                    let mut events = Vec::new();
                    while !self.at(&Tok::RParen) && !self.at_end() {
                        events.push(self.event_expr()?);
                        if !self.eat(&Tok::Comma) {
                            if !self.eat_kw("or") {
                                break;
                            }
                        }
                    }
                    self.expect(&Tok::RParen)?;
                    Ok(Some(TimingControl::Event(EventControl::Expr(events))))
                }
            } else {
                Ok(Some(TimingControl::Event(EventControl::Ident(self.path()?))))
            }
        } else {
            Ok(None)
        }
    }

    fn event_expr(&mut self) -> PResult<EventExpr> {
        if self.eat_kw("posedge") {
            Ok(EventExpr::Posedge(self.expr()?))
        } else if self.eat_kw("negedge") {
            Ok(EventExpr::Negedge(self.expr()?))
        } else {
            Ok(EventExpr::Expr(self.expr()?))
        }
    }
}

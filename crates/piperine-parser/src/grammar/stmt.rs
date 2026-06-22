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
        if self.at_kw("if")                { return self.if_stmt(attrs); }
        if self.at_kw("while")             { return self.while_stmt(attrs); }
        if self.at_kw("foreach")           { return self.foreach_stmt(attrs); }
        if self.at_kw("for")               { return self.for_stmt(attrs); }
        if self.at_kw("repeat")            { return self.repeat_stmt(attrs); }
        if self.at_kw("forever")           { return self.forever_stmt(attrs); }
        if self.at_any_kw(&["case", "casex", "casez"]) { return self.case_stmt(attrs); }
        if self.at_kw("assert")            { return self.assert_stmt(attrs, 0); }
        if self.at_kw("assert_run")        { return self.assert_stmt(attrs, 1); }
        if self.at_kw("assert_warn")       { return self.assert_stmt(attrs, 2); }
        if self.eat_kw("break")            { self.eat(&Tok::Semi); return Ok(Stmt::Break(BreakStmt { attrs })); }
        if self.eat_kw("continue")         { self.eat(&Tok::Semi); return Ok(Stmt::Continue(ContinueStmt { attrs })); }
        if self.at_kw("return")            { return self.return_stmt(attrs); }
        if self.at(&Tok::PlusPlus) || self.at(&Tok::MinusMinus) { return self.prefix_incdec_stmt(attrs); }
        self.assign_or_expr_stmt(attrs)
    }

    /// Match a `=`, `<+`, or compound (`+=`, `-=`, …) assignment operator.
    /// Returns `None` when the next token is not an assignment operator.
    fn assign_op_token(&mut self) -> Option<AssignOp> {
        if self.eat(&Tok::Assign)    { Some(AssignOp::Eq) }
        else if self.eat(&Tok::Contrib)   { Some(AssignOp::Contrib) }
        else if self.eat(&Tok::PlusEq)    { Some(AssignOp::AddEq) }
        else if self.eat(&Tok::MinusEq)   { Some(AssignOp::SubEq) }
        else if self.eat(&Tok::StarEq)    { Some(AssignOp::MulEq) }
        else if self.eat(&Tok::SlashEq)   { Some(AssignOp::DivEq) }
        else if self.eat(&Tok::PercentEq) { Some(AssignOp::ModEq) }
        else { None }
    }

    fn assign_or_expr_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let lval = self.expr()?;
        // postfix `i++` / `i--`
        if self.eat(&Tok::PlusPlus) {
            self.eat(&Tok::Semi);
            return Ok(Stmt::Assign(AssignStmt { attrs, assign: incdec_assign(lval, true) }));
        }
        if self.eat(&Tok::MinusMinus) {
            self.eat(&Tok::Semi);
            return Ok(Stmt::Assign(AssignStmt { attrs, assign: incdec_assign(lval, false) }));
        }
        let stmt = match self.assign_op_token() {
            Some(op) => {
                let rval = self.expr()?;
                Stmt::Assign(AssignStmt { attrs, assign: Assign { lval, op, rval } })
            }
            None => Stmt::Expr(ExprStmt { attrs, expr: lval }),
        };
        self.eat(&Tok::Semi);
        Ok(stmt)
    }

    /// `++lval` / `--lval` as a standalone statement.
    fn prefix_incdec_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let inc = self.eat(&Tok::PlusPlus);
        if !inc { self.expect(&Tok::MinusMinus)?; }
        let lval = self.expr()?;
        self.eat(&Tok::Semi);
        Ok(Stmt::Assign(AssignStmt { attrs, assign: incdec_assign(lval, inc) }))
    }

    fn for_assign(&mut self) -> PResult<Assign> {
        // prefix `++i` / `--i`
        if self.eat(&Tok::PlusPlus)  { return Ok(incdec_assign(self.expr()?, true)); }
        if self.eat(&Tok::MinusMinus){ return Ok(incdec_assign(self.expr()?, false)); }
        let lval = self.expr()?;
        // postfix `i++` / `i--`
        if self.eat(&Tok::PlusPlus)  { return Ok(incdec_assign(lval, true)); }
        if self.eat(&Tok::MinusMinus){ return Ok(incdec_assign(lval, false)); }
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

    /// `foreach (array[index]) body`
    fn foreach_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("foreach")?;
        self.expect(&Tok::LParen)?;
        let array = Expr::Path(self.path()?);
        self.expect(&Tok::LBrack)?;
        let index = self.name()?;
        self.expect(&Tok::RBrack)?;
        self.expect(&Tok::RParen)?;
        let body = Box::new(self.stmt()?);
        Ok(Stmt::Foreach(ForeachStmt { attrs, array, index, body }))
    }

    fn return_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("return")?;
        let value = if self.at(&Tok::Semi) || self.at(&Tok::RBrace) || self.at_kw("end") {
            None
        } else {
            Some(self.expr()?)
        };
        self.eat(&Tok::Semi);
        Ok(Stmt::Return(ReturnStmt { attrs, value }))
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
            "begin", "if", "while", "for", "foreach", "case", "casex", "casez",
            "repeat", "forever", "return", "break", "continue",
            "assert", "assert_run", "assert_warn",
        ])
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
            assign: self.for_assign()?,
        }));
        self.expect(&Tok::Semi)?;
        let condition = self.expr()?;
        self.expect(&Tok::Semi)?;
        let incr = Box::new(Stmt::Assign(AssignStmt {
            attrs: Vec::new(),
            assign: self.for_assign()?,
        }));
        self.expect(&Tok::RParen)?;
        let for_body = Box::new(self.stmt()?);
        Ok(Stmt::For(ForStmt { attrs, init, condition, incr, for_body }))
    }

    fn case_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.bump(); // case / casex / casez
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
        Ok(Stmt::Case(CaseStmt { attrs, discriminant, cases }))
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

    fn assert_stmt(&mut self, attrs: Vec<Attr>, kind: u8) -> PResult<Stmt> {
        self.bump();
        self.expect(&Tok::LParen)?;
        let condition = self.expr()?;
        self.expect(&Tok::RParen)?;
        let message = if self.eat_kw("else") {
            Some(self.expr()?)
        } else {
            None
        };
        self.eat(&Tok::Semi);
        let stmt = AssertStmt { attrs, condition, message };
        Ok(match kind {
            0 => Stmt::Assert(stmt),
            1 => Stmt::AssertRun(stmt),
            2 => Stmt::AssertWarn(stmt),
            _ => unreachable!(),
        })
    }
}

/// Desugar `lval++`/`++lval` (or `--`) into `lval += 1` / `lval -= 1`.
fn incdec_assign(lval: Expr, inc: bool) -> Assign {
    Assign {
        lval,
        op: if inc { AssignOp::AddEq } else { AssignOp::SubEq },
        rval: Expr::Literal(Literal::IntNumber("1".to_string())),
    }
}

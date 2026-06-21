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
        if self.at_kw("begin")             { return self.block_stmt(attrs); }
        if self.at_kw("if")                { return self.if_stmt(attrs); }
        if self.at_kw("while")             { return self.while_stmt(attrs); }
        if self.at_kw("for")               { return self.for_stmt(attrs); }
        if self.at_any_kw(&["case", "casex", "casez"]) { return self.case_stmt(attrs); }
        self.assign_or_expr_stmt(attrs)
    }

    fn assign_or_expr_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        let lval = self.expr()?;
        let op = if self.eat(&Tok::Contrib) { Some(AssignOp::Contrib) }
                 else if self.eat(&Tok::Assign) { Some(AssignOp::Eq) }
                 else { None };
        let stmt = match op {
            Some(op) => {
                let rval = self.expr()?;
                Stmt::Assign(AssignStmt { attrs, assign: Assign { lval, op, rval } })
            }
            None => Stmt::Expr(ExprStmt { attrs, expr: lval }),
        };
        self.eat(&Tok::Semi);
        Ok(stmt)
    }

    fn for_assign(&mut self) -> PResult<Assign> {
        let lval = self.expr()?;
        let op = if self.eat(&Tok::Contrib) { AssignOp::Contrib }
                 else { self.expect(&Tok::Assign)?; AssignOp::Eq };
        let rval = self.expr()?;
        Ok(Assign { lval, op, rval })
    }

    fn block_stmt(&mut self, attrs: Vec<Attr>) -> PResult<Stmt> {
        self.expect_kw("begin")?;
        let label = if self.eat(&Tok::Colon) { Some(self.name()?) } else { None };
        let mut items = Vec::new();
        while !self.at_kw("end") && !self.at_end() {
            items.push(self.block_item()?);
        }
        self.expect_kw("end")?;
        Ok(Stmt::Block(BlockStmt { attrs, label, items }))
    }

    pub(super) fn block_item(&mut self) -> PResult<BlockItem> {
        let start = self.span_start();
        let attrs = self.attrs()?;
        if self.at_kw("parameter") || self.at_kw("localparam") {
            Ok(BlockItem::ParamDecl(self.param_decl(attrs, start)?))
        } else if self.is_type_kw() || self.at_kw("genvar") {
            Ok(BlockItem::VarDecl(self.var_decl(attrs, start)?))
        } else {
            Ok(BlockItem::Stmt(self.stmt_with_attrs(attrs)?))
        }
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
}

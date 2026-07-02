//! Emit-and-validation contract (SPEC §11).
//!
//! The emitter must produce only what the codegen implements; validation is
//! the checked half of that contract. It verifies id resolution, per-body
//! statement subsets, and that every `ContribKind` matches the actual
//! presence of a reactive `State` in its expression.

use super::expr::IrExpr;
use super::stmt::{ContribKind, IrStmt, Lval};
use super::symbols::{StateId, SymbolTable};
use super::{IrAnalogBody, IrDigitalBody, IrModule};

/// How bad a validation finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrDiagnosticKind {
    /// The module must not be compiled.
    Error,
    /// Suspicious but compilable (e.g. an inferred digital latch).
    Warning,
}

/// One validation finding.
#[derive(Debug, Clone)]
pub struct IrDiagnostic {
    pub kind: IrDiagnosticKind,
    pub message: String,
}

impl IrDiagnostic {
    fn error(message: impl Into<String>) -> Self {
        Self { kind: IrDiagnosticKind::Error, message: message.into() }
    }
}

/// Which body a statement appears in, restricting the admissible subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyKind {
    Analog,
    Digital,
    Function,
}

impl IrModule {
    /// Validate the module against the SPEC §11 contract. Returns every
    /// finding; the module is compilable iff none is an
    /// [`IrDiagnosticKind::Error`].
    pub fn validate(&self) -> Vec<IrDiagnostic> {
        let mut v = Validator { module: self, findings: Vec::new() };
        v.run();
        v.findings
    }

    /// Validate and fail on the first error, for callers that just want a
    /// yes/no before compiling.
    pub fn validated(&self) -> Result<&Self, IrDiagnostic> {
        match self
            .validate()
            .into_iter()
            .find(|d| d.kind == IrDiagnosticKind::Error)
        {
            Some(err) => Err(err),
            None => Ok(self),
        }
    }
}

struct Validator<'m> {
    module: &'m IrModule,
    findings: Vec<IrDiagnostic>,
}

impl Validator<'_> {
    fn run(&mut self) {
        if let Some(analog) = &self.module.analog {
            self.check_analog(analog);
        }
        if let Some(digital) = &self.module.digital {
            self.check_digital(digital);
        }
        for (id, _) in self.module.symbols.states() {
            let state = self.module.symbols.state(id);
            self.check_expr(&state.arg);
        }
        let fns: Vec<_> = (0..)
            .map_while(|i| self.module.symbols.try_fn(super::symbols::FnId(i)))
            .collect();
        for function in fns {
            self.check_stmts(&function.body, BodyKind::Function);
        }
    }

    fn symbols(&self) -> &SymbolTable {
        &self.module.symbols
    }

    fn error(&mut self, message: impl Into<String>) {
        self.findings.push(IrDiagnostic::error(message));
    }

    fn check_analog(&mut self, body: &IrAnalogBody) {
        for &id in &body.states {
            if self.symbols().try_state(id).is_none() {
                self.error(format!("analog body references dangling state #{}", id.0));
            }
        }
        for source in &body.noise {
            self.check_node(source.plus);
            self.check_node(source.minus);
            match &source.kind {
                super::symbols::IrNoise::White { psd } => self.check_expr(psd),
                super::symbols::IrNoise::Flicker { psd, exponent } => {
                    self.check_expr(psd);
                    self.check_expr(exponent);
                }
            }
        }
        self.check_stmts(&body.stmts, BodyKind::Analog);
    }

    fn check_digital(&mut self, body: &IrDigitalBody) {
        for &node in body.inputs.iter().chain(&body.outputs) {
            self.check_node(node);
        }
        for &var in &body.regs {
            if self.symbols().try_var(var).is_none() {
                self.error(format!("digital body references dangling reg #{}", var.0));
            }
        }
        self.check_stmts(&body.stmts, BodyKind::Digital);
    }

    fn check_stmts(&mut self, stmts: &[IrStmt], ctx: BodyKind) {
        for stmt in stmts {
            self.check_stmt(stmt, ctx);
        }
    }

    fn check_stmt(&mut self, stmt: &IrStmt, ctx: BodyKind) {
        for expr in stmt.exprs() {
            self.check_expr(expr);
        }
        match stmt {
            IrStmt::Contrib { nature, plus, minus, expr, kind } => {
                if ctx != BodyKind::Analog {
                    self.error("`<+` contribution outside an analog body");
                }
                self.check_nature(*nature);
                self.check_node(*plus);
                self.check_node(*minus);
                self.check_contrib_kind(expr, *kind);
            }
            IrStmt::Force { nature, plus, minus, .. } => {
                if ctx != BodyKind::Analog {
                    self.error("`<-` force outside an analog body");
                }
                self.check_nature(*nature);
                self.check_node(*plus);
                self.check_node(*minus);
            }
            IrStmt::AnalogEvent(event) => {
                if ctx != BodyKind::Analog {
                    self.error("analog event outside an analog body");
                }
                self.check_stmts(&event.body, ctx);
            }
            IrStmt::Assign { lval, .. } => {
                self.check_lval(lval, ctx);
            }
            IrStmt::ClockedBlock { body, .. } => {
                if ctx != BodyKind::Digital {
                    self.error("clocked block outside a digital body");
                }
                self.check_stmts(body, ctx);
            }
            IrStmt::Return(_) => {
                if ctx != BodyKind::Function {
                    self.error("`return` outside a function body");
                }
            }
            IrStmt::VarDecl { var, .. } => {
                if self.symbols().try_var(*var).is_none() {
                    self.error(format!("declaration of dangling var #{}", var.0));
                }
            }
            IrStmt::If { .. } | IrStmt::Match { .. } => {
                for body in stmt.bodies() {
                    self.check_stmts(body, ctx);
                }
            }
            IrStmt::BoundStep(_)
            | IrStmt::Finish
            | IrStmt::Discontinuity(_)
            | IrStmt::Diagnostic { .. } => {}
        }
    }

    fn check_lval(&mut self, lval: &Lval, ctx: BodyKind) {
        match lval {
            Lval::Var(id) => {
                if self.symbols().try_var(*id).is_none() {
                    self.error(format!("assignment to dangling var #{}", id.0));
                }
            }
            Lval::Net(id) => {
                if ctx != BodyKind::Digital {
                    self.error("net assignment outside a digital body");
                }
                self.check_node(*id);
            }
            Lval::Index(inner, expr) => {
                self.check_expr(expr);
                self.check_lval(inner, ctx);
            }
            Lval::Slice(inner, lo, hi, _) => {
                self.check_expr(lo);
                self.check_expr(hi);
                self.check_lval(inner, ctx);
            }
        }
    }

    /// The declared `ContribKind` must match the structural presence of a
    /// reactive `State` in the expression.
    fn check_contrib_kind(&mut self, expr: &IrExpr, kind: ContribKind) {
        let actual = self.first_reactive_state(expr);
        match (kind, actual) {
            (ContribKind::Resistive, Some(id)) => self.error(format!(
                "contribution marked resistive but contains reactive `{}` state #{}",
                self.symbols().state(id).kind.name(),
                id.0
            )),
            (ContribKind::Reactive(_), None) => {
                self.error("contribution marked reactive but contains no reactive state");
            }
            _ => {}
        }
    }

    /// SPEC §11: `first_reactive_state` walks resolved `State(id)` nodes.
    fn first_reactive_state(&self, expr: &IrExpr) -> Option<StateId> {
        expr.find_state(&|id| {
            self.symbols()
                .try_state(id)
                .is_some_and(|s| s.kind.is_reactive())
        })
    }

    fn check_node(&mut self, id: super::symbols::NodeId) {
        if self.symbols().try_node(id).is_none() {
            self.findings
                .push(IrDiagnostic::error(format!("dangling node id #{}", id.0)));
        }
    }

    fn check_nature(&mut self, id: super::symbols::NatureId) {
        if self.symbols().try_nature(id).is_none() {
            self.findings
                .push(IrDiagnostic::error(format!("dangling nature id #{}", id.0)));
        }
    }

    fn check_expr(&mut self, expr: &IrExpr) {
        expr.visit(&mut |e| {
            let dangling: Option<String> = match e {
                IrExpr::Param(id) if self.module.symbols.try_param(*id).is_none() => {
                    Some(format!("param #{}", id.0))
                }
                IrExpr::Var(id) if self.module.symbols.try_var(*id).is_none() => {
                    Some(format!("var #{}", id.0))
                }
                IrExpr::State(id) if self.module.symbols.try_state(*id).is_none() => {
                    Some(format!("state #{}", id.0))
                }
                IrExpr::Call(id, _) if self.module.symbols.try_fn(*id).is_none() => {
                    Some(format!("fn #{}", id.0))
                }
                IrExpr::Net(id) if self.module.symbols.try_node(*id).is_none() => {
                    Some(format!("node #{}", id.0))
                }
                IrExpr::Branch { nature, plus, minus } => {
                    if self.module.symbols.try_nature(*nature).is_none() {
                        Some(format!("nature #{}", nature.0))
                    } else if self.module.symbols.try_node(*plus).is_none() {
                        Some(format!("node #{}", plus.0))
                    } else if self.module.symbols.try_node(*minus).is_none() {
                        Some(format!("node #{}", minus.0))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(what) = dangling {
                self.findings.push(IrDiagnostic::error(format!(
                    "expression references dangling {what}"
                )));
            }
        });
    }
}

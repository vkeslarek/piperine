use std::collections::HashMap;
use piperine_parser::ast::*;
use piperine_common::{EventAction, SimEventKind};
use crate::backend::{SimulatorBackend, AnalysisEvent, collect_vectors};
use crate::task::SystemTaskRegistry;
use crate::value::{Value, AnalysisResult, RunError, RunErrorKind};
use crate::error::InterpreterError;
use piperine_circuit::parse_si_real;
use piperine_circuit::elaboration::AlwaysHandlerSet;

/// Variable scope — flat map for Phase 1.
/// Phase 3 adds nested scopes for function calls.
#[derive(Default)]
pub struct Scope {
    variables: HashMap<String, Value>,
}

impl Scope {
    pub fn get(&self, name: &str) -> Option<&Value> { self.variables.get(name) }
    pub fn set(&mut self, name: &str, value: Value)  { self.variables.insert(name.to_string(), value); }
}

/// How control should continue after a statement runs.
/// Propagated up through blocks and loops so `break`/`continue`/`return`
/// are handled where they belong rather than via exceptions.
enum Flow {
    /// Fall through to the next statement.
    Normal,
    /// Exit the innermost loop (`break`).
    Break,
    /// Skip to the next loop iteration (`continue`).
    Continue,
    /// Exit the enclosing function/block carrying a value (`return`).
    Return(Value),
}

pub struct Interpreter<'a> {
    simulator: &'a mut dyn SimulatorBackend,
    tasks:     &'a SystemTaskRegistry,
    pub types: crate::value::TypeRegistry,
    /// `always @(...)` handlers collected from the testbench module.
    /// Set via `set_always_handlers()` before calling `exec()`.
    /// Tasks that run analyses (e.g. `$tran`) use these to wire callbacks.
    always_handlers: AlwaysHandlerSet,
}

impl<'a> Interpreter<'a> {
    pub fn new(simulator: &'a mut dyn SimulatorBackend, tasks: &'a SystemTaskRegistry) -> Self {
        Self {
            simulator,
            tasks,
            types: crate::value::TypeRegistry::default(),
            always_handlers: AlwaysHandlerSet::default(),
        }
    }

    /// Wire always-block handlers collected by the elaborator.
    /// Must be called before `exec()` if the testbench has any `always @(...)` blocks.
    pub fn set_always_handlers(&mut self, handlers: AlwaysHandlerSet) {
        self.always_handlers = handlers;
    }

    /// Run an analysis command with always-handler wiring.
    ///
    /// Drives the `start_analysis` / `poll_analysis` / `respond_to_analysis_event`
    /// protocol internally. Handler dispatch (`always @(step)` etc.) runs here,
    /// cleanly separated from the backend IPC loop — no callbacks, no unsafe.
    pub fn run_analysis(&mut self, cmd: &str) -> Result<AnalysisResult, InterpreterError> {
        // Clone handlers so the borrow checker lets us use both self.simulator
        // (for poll/respond) and self (for eval_statement) in the same loop.
        let handlers = self.always_handlers.clone();
        let fire_step = !handlers.step.is_empty();

        self.simulator.start_analysis(cmd, fire_step)?;

        let mut run_errors: Vec<RunError> = Vec::new();
        let mut had_run_errors = false;
        let plot_name;

        loop {
            // poll_analysis borrows self.simulator; borrow ends when we bind `event`.
            let event = self.simulator.poll_analysis()?;
            match event {
                AnalysisEvent::Event { kind, time, crossing_id } => {
                    // self.simulator is NOT borrowed here — safe to call eval_statement.
                    let action = self.fire_event_internal(kind, time, crossing_id, &handlers);
                    if let EventAction::RunError { ref message } = action {
                        run_errors.push(RunError {
                            message: message.clone(),
                            time: Some(time),
                            kind: RunErrorKind::UserAssert,
                        });
                    }
                    self.simulator.respond_to_analysis_event(action)?;
                }
                AnalysisEvent::Done { plot_name: p, had_run_errors: e } => {
                    plot_name = p;
                    had_run_errors = e;
                    break;
                }
            }
        }

        if had_run_errors && run_errors.is_empty() {
            run_errors.push(RunError {
                message: "run failed (SOA or simulator error)".into(),
                time: None,
                kind: RunErrorKind::SoaViolation,
            });
        }

        let vectors = collect_vectors(self.simulator, &plot_name)?;
        Ok(AnalysisResult {
            kind: crate::value::parse_analysis_kind(cmd),
            plot_name,
            vectors,
            run_errors,
        })
    }

    fn fire_event_internal(
        &mut self,
        kind: SimEventKind,
        time: f64,
        crossing_id: u32,
        handlers: &AlwaysHandlerSet,
    ) -> EventAction {
        let mut scope = Scope::default();
        scope.set("time", Value::Real(time));

        let stmts: &[Stmt] = match kind {
            SimEventKind::InitialStep => &handlers.initial_step,
            SimEventKind::Step        => &handlers.step,
            SimEventKind::FinalStep   => &handlers.final_step,
            SimEventKind::AboveCrossing => {
                if let Some((_, id, stmt)) = handlers.above.iter().find(|(_, id, _)| *id == crossing_id) {
                    let result = self.eval_statement(stmt, &mut scope);
                    return to_event_action(result);
                }
                return EventAction::Continue;
            }
        };

        for stmt in stmts {
            let result = self.eval_statement(stmt, &mut scope);
            let action = to_event_action(result);
            if !matches!(action, EventAction::Continue) {
                return action;
            }
        }
        EventAction::Continue
    }

    pub fn exec(&mut self, statement: &Stmt, scope: &mut Scope) -> Result<(), InterpreterError> {
        self.eval_statement(statement, scope)?;
        Ok(())
    }

    fn eval_statement(&mut self, statement: &Stmt, scope: &mut Scope) -> Result<Flow, InterpreterError> {
        match statement {
            Stmt::Empty(_) => {}

            Stmt::Block(block) => {
                for item in &block.items {
                    match self.eval_block_item(item, scope)? {
                        Flow::Normal => {}
                        other        => return Ok(other),  // break/continue/return exits the block
                    }
                }
            }

            Stmt::Assign(assign) => {
                let a = &assign.assign;
                let rhs = self.eval_expr(&a.rval, scope)?;
                let name = expr_as_variable_name(&a.lval).ok_or_else(|| {
                    InterpreterError::Other("assignment target must be a variable name".into())
                })?;
                // Compound assignments (`+=`, …) combine with the current value.
                let value = match compound_binop(&a.op) {
                    Some(binop) => {
                        let current = self.eval_expr(&a.lval, scope)?;
                        eval_binary_op(current, &binop, rhs)?
                    }
                    None => rhs,
                };
                scope.set(&name, value);
            }

            Stmt::Expr(expr_stmt) => {
                self.eval_expr(&expr_stmt.expr, scope)?;
            }

            Stmt::If(if_stmt) => {
                let condition = self.eval_expr(&if_stmt.condition, scope)?;
                if condition.is_truthy() {
                    return self.eval_statement(&if_stmt.then_branch, scope);
                } else if let Some(else_branch) = &if_stmt.else_branch {
                    return self.eval_statement(else_branch, scope);
                }
            }

            Stmt::While(while_stmt) => {
                loop {
                    let condition = self.eval_expr(&while_stmt.condition, scope)?;
                    if !condition.is_truthy() { break; }
                    match self.eval_statement(&while_stmt.body, scope)? {
                        Flow::Break             => break,
                        Flow::Return(v)         => return Ok(Flow::Return(v)),
                        Flow::Continue | Flow::Normal => {}
                    }
                }
            }

            Stmt::For(for_stmt) => {
                self.eval_statement(&for_stmt.init, scope)?;
                loop {
                    let condition = self.eval_expr(&for_stmt.condition, scope)?;
                    if !condition.is_truthy() { break; }
                    match self.eval_statement(&for_stmt.for_body, scope)? {
                        Flow::Break             => break,
                        Flow::Return(v)         => return Ok(Flow::Return(v)),
                        // `continue` skips to the increment, like C.
                        Flow::Continue | Flow::Normal => {}
                    }
                    self.eval_statement(&for_stmt.incr, scope)?;
                }
            }

            Stmt::Repeat(repeat_stmt) => {
                let count = self.eval_expr(&repeat_stmt.count, scope)?
                    .as_integer().unwrap_or(0);
                for _ in 0..count.max(0) {
                    match self.eval_statement(&repeat_stmt.body, scope)? {
                        Flow::Break             => break,
                        Flow::Return(v)         => return Ok(Flow::Return(v)),
                        Flow::Continue | Flow::Normal => {}
                    }
                }
            }

            Stmt::Forever(forever_stmt) => {
                loop {
                    match self.eval_statement(&forever_stmt.body, scope)? {
                        Flow::Break             => break,
                        Flow::Return(v)         => return Ok(Flow::Return(v)),
                        Flow::Continue | Flow::Normal => {}
                    }
                }
            }

            Stmt::Break(_)    => return Ok(Flow::Break),
            Stmt::Continue(_) => return Ok(Flow::Continue),

            Stmt::Return(ret) => {
                let value = match &ret.value {
                    Some(expr) => self.eval_expr(expr, scope)?,
                    None       => Value::Void,
                };
                return Ok(Flow::Return(value));
            }

            Stmt::Case(case_stmt) => {
                let discriminant = self.eval_expr(&case_stmt.discriminant, scope)?;
                for case in &case_stmt.cases {
                    let hit = match &case.item {
                        CaseItem::Default => true,
                        CaseItem::Exprs(exprs) => exprs.iter().any(|e| {
                            self.eval_expr(e, scope).map(|v| v == discriminant).unwrap_or(false)
                        }),
                    };
                    if hit {
                        return self.eval_statement(&case.stmt, scope);
                    }
                }
            }

            Stmt::Event(_) => {
                // Event statements inside executable contexts (like $tran callbacks)
                // are not directly evaluated. They are collected by the elaborator.
            }

            Stmt::Assert(a) => {
                let cond = self.eval_expr(&a.condition, scope)?;
                if !cond.is_truthy() {
                    let msg = a.message.as_ref()
                        .and_then(|m| self.eval_expr(m, scope).ok())
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "assertion failed".into());
                    return Err(InterpreterError::Fatal { message: msg, exit_code: 1 });
                }
            }

            Stmt::AssertRun(a) => {
                let cond = self.eval_expr(&a.condition, scope)?;
                if !cond.is_truthy() {
                    let msg = a.message.as_ref()
                        .and_then(|m| self.eval_expr(m, scope).ok())
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "run assertion failed".into());
                    return Err(InterpreterError::RunFailed { message: msg });
                }
            }

            Stmt::AssertWarn(a) => {
                let cond = self.eval_expr(&a.condition, scope)?;
                if !cond.is_truthy() {
                    let msg = a.message.as_ref()
                        .and_then(|m| self.eval_expr(m, scope).ok())
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "warning".into());
                    self.simulator.print(&format!("WARNING: {msg}"));
                }
            }
        }
        Ok(Flow::Normal)
    }

    fn eval_block_item(&mut self, item: &BlockItem, scope: &mut Scope) -> Result<Flow, InterpreterError> {
        match item {
            BlockItem::VarDecl(decl) => {
                for var in &decl.vars {
                    let initial_value = match &var.default {
                        Some(expr) => self.eval_expr(expr, scope)?,
                        None       => type_zero_value(&decl.ty),
                    };
                    scope.set(&var.name.0, initial_value);
                }
            }
            BlockItem::ParamDecl(decl) => {
                for param in &decl.params {
                    let value = self.eval_expr(&param.default, scope)?;
                    scope.set(&param.name.0, value);
                }
            }
            BlockItem::Stmt(stmt) => {
                return self.eval_statement(stmt, scope);
            }
        }
        Ok(Flow::Normal)
    }

    pub fn eval_expr(&mut self, expr: &Expr, scope: &mut Scope) -> Result<Value, InterpreterError> {
        match expr {
            Expr::Literal(literal) => Ok(eval_literal(literal)),

            Expr::Path(path) => {
                let name = path_to_string(path);
                if let Some(val) = scope.get(&name).cloned() {
                    Ok(val)
                } else {
                    // Try to resolve as an enum variant
                    for (_, enum_def) in &self.types.enums {
                        for (variant_name, variant_value) in &enum_def.variants {
                            if variant_name == &name {
                                return Ok(Value::Enum { type_id: enum_def.type_id, variant: *variant_value });
                            }
                        }
                    }
                    Err(InterpreterError::UndefinedVariable { name })
                }
            }

            Expr::Paren(inner) => self.eval_expr(inner, scope),

            Expr::Prefix(op, inner) => {
                let value = self.eval_expr(inner, scope)?;
                eval_prefix_op(op, value)
            }

            Expr::Binary(left, op, right) => {
                let left_value  = self.eval_expr(left, scope)?;
                let right_value = self.eval_expr(right, scope)?;
                eval_binary_op(left_value, op, right_value)
            }

            Expr::Select(condition, then_expr, else_expr) => {
                let cond_value = self.eval_expr(condition, scope)?;
                if cond_value.is_truthy() { self.eval_expr(then_expr, scope) }
                else                      { self.eval_expr(else_expr, scope) }
            }

            Expr::Call(function_ref, call_args) => {
                // Split positional vs named args
                let mut positional = Vec::new();
                let mut named: HashMap<String, Value> = HashMap::new();
                for arg in call_args {
                    match arg {
                        CallArg::Positional(e) => positional.push(self.eval_expr(e, scope)?),
                        CallArg::Named(name, e) => { named.insert(name.clone(), self.eval_expr(e, scope)?); }
                    }
                }
                // Combined list for backwards-compat method dispatch
                let all_args: Vec<Value> = positional.iter().cloned()
                    .chain(named.values().cloned())
                    .collect();
                match function_ref {
                    FunctionRef::SysFun(name) => {
                        let task_name = name.trim_start_matches('$');
                        let task = self.tasks.get(task_name).ok_or_else(|| {
                            InterpreterError::UndefinedSystemTask { name: task_name.to_string() }
                        })?;
                        Ok(task.call_named(positional, named, self.simulator)?.unwrap_or(Value::Void))
                    }
                    FunctionRef::Path(path) => {
                        let path_str = path_to_string(path);
                        if let Some((obj_name, method)) = path_str.split_once('.') {
                            if let Some(Value::ExternObject(obj)) = scope.get(obj_name) {
                                return obj.call_method(method, &all_args)
                                    .map_err(|e| InterpreterError::Other(e));
                            }
                        }
                        Err(InterpreterError::Other(format!(
                            "user-defined function `{}` calls not supported in Phase 1",
                            path_str
                        )))
                    }
                }
            }

            Expr::Array(_) | Expr::Index(_, _) | Expr::PartSelect(_, _, _) => {
                Err(InterpreterError::Other("arrays not supported in Phase 1 — arrives in Phase 3".into()))
            }

            Expr::PortFlow(_) => {
                Err(InterpreterError::Other(
                    "port-flow access (`<port>`) not valid inside initial blocks".into()
                ))
            }
        }
    }
}

fn to_event_action(result: Result<Flow, InterpreterError>) -> EventAction {
    match result {
        // break/continue/return inside a handler all just stop the handler body.
        Ok(_) => EventAction::Continue,
        Err(InterpreterError::RunFailed { message }) => EventAction::RunError { message },
        Err(InterpreterError::Fatal { message, .. }) => EventAction::Halt { reason: message },
        Err(e) => EventAction::Halt { reason: e.to_string() },
    }
}

/// Map a compound-assignment operator to its underlying binary operator.
/// Returns `None` for plain `=` / `<+`.
fn compound_binop(op: &AssignOp) -> Option<BinOp> {
    match op {
        AssignOp::AddEq => Some(BinOp::Add),
        AssignOp::SubEq => Some(BinOp::Sub),
        AssignOp::MulEq => Some(BinOp::Mul),
        AssignOp::DivEq => Some(BinOp::Div),
        AssignOp::ModEq => Some(BinOp::Mod),
        AssignOp::Eq | AssignOp::Contrib => None,
    }
}

fn eval_literal(literal: &Literal) -> Value {
    match literal {
        Literal::IntNumber(s)     => s.parse::<i64>().map(Value::Integer)
                                      .unwrap_or_else(|_| Value::Real(s.parse().unwrap_or(0.0))),
        Literal::StdRealNumber(s) => Value::Real(s.parse().unwrap_or(0.0)),
        Literal::SiRealNumber(s)  => Value::Real(parse_si_real(s).unwrap_or(0.0)),
        Literal::StrLit(s)        => {
            // Lexer stores the raw token including surrounding quotes ("foo").
            // Strip them before storing as a Value.
            let inner = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s);
            Value::String(inner.to_string())
        }
        Literal::Inf              => Value::Real(f64::INFINITY),
    }
}

fn eval_prefix_op(op: &PrefixOp, value: Value) -> Result<Value, InterpreterError> {
    match op {
        PrefixOp::Neg => match value {
            Value::Real(v)    => Ok(Value::Real(-v)),
            Value::Integer(i) => Ok(Value::Integer(-i)),
            _ => Err(InterpreterError::TypeError { expected: "numeric".into(), got: value.type_name().into() }),
        },
        PrefixOp::Pos    => Ok(value),
        PrefixOp::Not    => Ok(Value::Integer(if value.is_truthy() { 0 } else { 1 })),
        PrefixOp::BitNot => match value {
            Value::Integer(i) => Ok(Value::Integer(!i)),
            _ => Err(InterpreterError::TypeError { expected: "integer".into(), got: value.type_name().into() }),
        },
    }
}

fn eval_binary_op(left: Value, op: &BinOp, right: Value) -> Result<Value, InterpreterError> {
    match (left, right) {
        (Value::Real(a),    Value::Real(b))    => eval_binary_real(a, op, b),
        (Value::Integer(a), Value::Integer(b)) => eval_binary_integer(a, op, b),
        (Value::Real(a),    Value::Integer(b)) => eval_binary_real(a, op, b as f64),
        (Value::Integer(a), Value::Real(b))    => eval_binary_real(a as f64, op, b),
        (Value::String(a),  Value::String(b))  => match op {
            BinOp::Eq  => Ok(Value::Integer((a == b) as i64)),
            BinOp::Neq => Ok(Value::Integer((a != b) as i64)),
            _ => Err(InterpreterError::TypeError { expected: "numeric operands".into(), got: "string".into() }),
        },
        (left, right) => Err(InterpreterError::TypeError {
            expected: "matching numeric types".into(),
            got: format!("{} and {}", left.type_name(), right.type_name()),
        }),
    }
}

fn eval_binary_real(a: f64, op: &BinOp, b: f64) -> Result<Value, InterpreterError> {
    Ok(match op {
        BinOp::Add    => Value::Real(a + b),
        BinOp::Sub    => Value::Real(a - b),
        BinOp::Mul    => Value::Real(a * b),
        BinOp::Div    => Value::Real(a / b),
        BinOp::Pow    => Value::Real(a.powf(b)),
        BinOp::Mod    => Value::Real(a % b),
        BinOp::Eq     => Value::Integer((a == b) as i64),
        BinOp::Neq    => Value::Integer((a != b) as i64),
        BinOp::Lt     => Value::Integer((a < b) as i64),
        BinOp::Le     => Value::Integer((a <= b) as i64),
        BinOp::Gt     => Value::Integer((a > b) as i64),
        BinOp::Ge     => Value::Integer((a >= b) as i64),
        BinOp::AndAnd => Value::Integer(((a != 0.0) && (b != 0.0)) as i64),
        BinOp::OrOr   => Value::Integer(((a != 0.0) || (b != 0.0)) as i64),
        other => return Err(InterpreterError::TypeError {
            expected: "real-compatible binary operator".into(),
            got: format!("{other:?}"),
        }),
    })
}

fn eval_binary_integer(a: i64, op: &BinOp, b: i64) -> Result<Value, InterpreterError> {
    Ok(match op {
        BinOp::Add    => Value::Integer(a + b),
        BinOp::Sub    => Value::Integer(a - b),
        BinOp::Mul    => Value::Integer(a * b),
        BinOp::Div    => Value::Integer(a / b),
        BinOp::Mod    => Value::Integer(a % b),
        BinOp::Pow    => Value::Integer(a.pow(b.max(0) as u32)),
        BinOp::Eq     => Value::Integer((a == b) as i64),
        BinOp::Neq    => Value::Integer((a != b) as i64),
        BinOp::Lt     => Value::Integer((a < b) as i64),
        BinOp::Le     => Value::Integer((a <= b) as i64),
        BinOp::Gt     => Value::Integer((a > b) as i64),
        BinOp::Ge     => Value::Integer((a >= b) as i64),
        BinOp::AndAnd => Value::Integer(((a != 0) && (b != 0)) as i64),
        BinOp::OrOr   => Value::Integer(((a != 0) || (b != 0)) as i64),
        BinOp::BitAnd => Value::Integer(a & b),
        BinOp::BitOr  => Value::Integer(a | b),
        BinOp::Xor    => Value::Integer(a ^ b),
        BinOp::XNor1 | BinOp::XNor2 => Value::Integer(!(a ^ b)),
        BinOp::Shl    => Value::Integer(a << (b as u32)),
        BinOp::Shr    => Value::Integer(a >> (b as u32)),
    })
}

fn type_zero_value(ty: &Type) -> Value {
    match ty {
        Type::Integer => Value::Integer(0),
        Type::Real    => Value::Real(0.0),
        Type::String  => Value::String(String::new()),
        Type::Custom(_) => Value::Void, // For now, custom types start as void
    }
}

fn expr_as_variable_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path_to_string(path)),
        _ => None,
    }
}

fn path_to_string(path: &Path) -> String {
    let mut parts = Vec::new();
    let mut current = path;
    loop {
        match &current.segment {
            PathSegment::Ident(s) => parts.push(s.clone()),
            PathSegment::Root     => parts.push("root".to_string()),
        }
        match &current.qualifier {
            Some(qualifier) => current = qualifier,
            None            => break,
        }
    }
    parts.reverse();
    parts.join(".")
}

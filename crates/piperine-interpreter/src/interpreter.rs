use std::collections::HashMap;
use cvaf::ast::*;
use crate::backend::SimulatorBackend;
use crate::task::SystemTaskRegistry;
use crate::value::Value;
use crate::error::InterpreterError;
use piperine_circuit::parse_si_real;

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

pub struct Interpreter<'a> {
    simulator: &'a mut dyn SimulatorBackend,
    tasks:     &'a SystemTaskRegistry,
}

impl<'a> Interpreter<'a> {
    pub fn new(simulator: &'a mut dyn SimulatorBackend, tasks: &'a SystemTaskRegistry) -> Self {
        Self { simulator, tasks }
    }

    pub fn exec(&mut self, statement: &Stmt, scope: &mut Scope) -> Result<(), InterpreterError> {
        self.eval_statement(statement, scope)
    }

    fn eval_statement(&mut self, statement: &Stmt, scope: &mut Scope) -> Result<(), InterpreterError> {
        match statement {
            Stmt::Empty(_) => {}

            Stmt::Block(block) => {
                for item in &block.items {
                    self.eval_block_item(item, scope)?;
                }
            }

            Stmt::Assign(assign) => {
                let value = self.eval_expr(&assign.assign.rval, scope)?;
                let name = expr_as_variable_name(&assign.assign.lval).ok_or_else(|| {
                    InterpreterError::Other("assignment target must be a variable name".into())
                })?;
                scope.set(&name, value);
            }

            Stmt::Expr(expr_stmt) => {
                self.eval_expr(&expr_stmt.expr, scope)?;
            }

            Stmt::If(if_stmt) => {
                let condition = self.eval_expr(&if_stmt.condition, scope)?;
                if condition.is_truthy() {
                    self.eval_statement(&if_stmt.then_branch, scope)?;
                } else if let Some(else_branch) = &if_stmt.else_branch {
                    self.eval_statement(else_branch, scope)?;
                }
            }

            Stmt::While(while_stmt) => {
                loop {
                    let condition = self.eval_expr(&while_stmt.condition, scope)?;
                    if !condition.is_truthy() { break; }
                    self.eval_statement(&while_stmt.body, scope)?;
                }
            }

            Stmt::For(for_stmt) => {
                self.eval_statement(&for_stmt.init, scope)?;
                loop {
                    let condition = self.eval_expr(&for_stmt.condition, scope)?;
                    if !condition.is_truthy() { break; }
                    self.eval_statement(&for_stmt.for_body, scope)?;
                    self.eval_statement(&for_stmt.incr, scope)?;
                }
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
                        self.eval_statement(&case.stmt, scope)?;
                        break;
                    }
                }
            }

            Stmt::Event(_) => {
                return Err(InterpreterError::Other(
                    "event statements (`@(...)`) not supported in Phase 1 — arrives in Phase 4".into()
                ));
            }
        }
        Ok(())
    }

    fn eval_block_item(&mut self, item: &BlockItem, scope: &mut Scope) -> Result<(), InterpreterError> {
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
                self.eval_statement(stmt, scope)?;
            }
        }
        Ok(())
    }

    pub fn eval_expr(&mut self, expr: &Expr, scope: &mut Scope) -> Result<Value, InterpreterError> {
        match expr {
            Expr::Literal(literal) => Ok(eval_literal(literal)),

            Expr::Path(path) => {
                let name = path_to_string(path);
                scope.get(&name).cloned().ok_or_else(|| InterpreterError::UndefinedVariable { name })
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

            Expr::Call(function_ref, arguments) => {
                let mut evaluated_args = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    evaluated_args.push(self.eval_expr(arg, scope)?);
                }
                match function_ref {
                    FunctionRef::SysFun(name) => {
                        let task_name = name.trim_start_matches('$');
                        let task = self.tasks.get(task_name).ok_or_else(|| {
                            InterpreterError::UndefinedSystemTask { name: task_name.to_string() }
                        })?;
                        Ok(task.call(evaluated_args, self.simulator)?.unwrap_or(Value::Void))
                    }
                    FunctionRef::Path(path) => Err(InterpreterError::Other(format!(
                        "user-defined function `{}` calls not supported in Phase 1",
                        path_to_string(path)
                    ))),
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
        Type::Real    => Value::Real(0.0),
        Type::Integer => Value::Integer(0),
        Type::String  => Value::String(String::new()),
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

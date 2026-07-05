//! [`Interpreter`] — the tree-walking evaluator shared by const-eval
//! ([`crate::elab::const_eval::ConstEnv`], via [`super::const_host::ConstHost`])
//! and `bench` (via a `SimHost` in `piperine-bench`).
//!
//! One engine, two [`Host`] implementations: the host owns everything that
//! differs between "elaboration-time constant folding" and "post-elaboration
//! effectful scripting" — name resolution against the POM, whether a system
//! task is reachable, and where assignment writes land (nowhere, for
//! `ConstHost`; a staged override, for `SimHost`).

use std::collections::HashMap;
use std::rc::Rc;

use crate::parse::ast::{
    BinaryOp, Block, Expr, Literal, Pattern, Stmt, StmtMatchArm, UnaryOp,
};
use super::error::EvalError;
use super::value::{Closure, Value};

/// What a name resolves to when called: a value-layer closure, or a POM
/// `fn`/bundle-method body to walk directly (already elaborated — one
/// statement type end to end, SIMPLIFICATION.md P3).
pub enum Callable {
    Closure(Rc<Closure>),
    Function {
        params: Vec<String>,
        /// Default expressions, parallel to `params` (the language spec Part I §9.1).
        defaults: Vec<Option<Expr>>,
        body: Vec<Stmt>,
    },
    /// A sibling `bench` fn (bench spec §2 "fn helper(x: T) -> U") — runs
    /// in the effectful bench context, so it may call analyses and stage
    /// overrides, unlike the pure [`Callable::Function`].
    BenchFn {
        params: Vec<String>,
        defaults: Vec<Option<Expr>>,
        body: Vec<Stmt>,
        /// The block's trailing expression, if any (`fn f() -> Real { x * 2 }`).
        tail: Option<Expr>,
    },
}

/// The host-specific half of evaluation: name resolution, system-task
/// dispatch, and assignment targets. See the module doc for the two
/// implementations.
pub trait Host {
    /// Used only in diagnostics (`` `$op` is not available in {context} ``).
    fn context_name(&self) -> &'static str;

    /// Resolve a bare identifier that isn't a local `var`/parameter: POM
    /// nets/instances/params, global consts, enum variants, `gnd`, ...
    fn lookup(&mut self, name: &str) -> Option<Value>;

    /// Resolve a bare identifier used as a call target that isn't a local
    /// closure. Returns `None` to let the interpreter fall back to the
    /// built-in math catalog, then report `Undefined`.
    fn resolve_callable(&mut self, _name: &str) -> Option<Callable> {
        None
    }

    /// Intercept a plain-name call (`name(args)`, not a `$`-syscall) the host
    /// wants to own — e.g. `select("...")` returning a `SelectionRef`.
    /// Consulted between `resolve_callable` and the built-in math fallback;
    /// returns `None` to let the interpreter fall back.
    fn call_host_fn(&mut self, _name: &str, _args: &[Value]) -> Option<Result<Value, EvalError>> {
        None
    }

    /// Resolve `recv.method(...)` on a bundle-literal `Record` of type `ty`
    /// to its `impl` method (SPEC §6.5/§6.6). The interpreter binds `self`
    /// to the receiver and runs the body as a pure fn. `None` = no such
    /// method — the call fails loud as `Undefined`.
    fn resolve_method(&mut self, _ty: &str, _method: &str) -> Option<Callable> {
        None
    }

    /// Dispatch a `$name(args)` system task.
    fn syscall(&mut self, name: &str, args: Vec<Value>) -> Result<Value, EvalError>;

    /// Handle `target = value`. Returns `Ok(true)` if the host consumed the
    /// assignment (e.g. staged a POM override); `Ok(false)` if `target` is
    /// not a host-owned location, so the interpreter should fall back to
    /// assigning a local `var`.
    fn assign(&mut self, target: &Expr, value: &Value) -> Result<bool, EvalError>;

    /// Handle `target.field = value` where `target` is a host object the
    /// host wants to intercept (e.g. `s.ctrl = 1` on a held `SelectionRef`).
    /// The interpreter evaluates the base `Expr` itself (the host only sees
    /// `&Expr` in [`assign`](Self::assign), so it cannot), then calls this
    /// with the resulting value. Returns `Ok(true)` if consumed.
    fn assign_field_on(&mut self, _target: &Value, _field: &str, _value: &Value) -> Result<bool, EvalError> {
        Ok(false)
    }
}

/// The result of executing a block or function body: a plain fallthrough
/// value, or an early `return`.
pub enum Flow {
    Normal(Value),
    Return(Value),
}

impl Flow {
    fn into_value(self) -> Value {
        match self {
            Flow::Normal(v) | Flow::Return(v) => v,
        }
    }
}

pub struct Interpreter<'h, H: Host> {
    host: &'h mut H,
    scopes: Vec<HashMap<String, Value>>,
    /// Nonzero while executing a POM `fn`/bundle-method body — these are
    /// pure value computations (bench spec §1: "elaboration, `analog`,
    /// `digital`, ordinary `fn` ... cannot reach [effectful tasks]").
    /// `$`-syscalls a host marks effectful are rejected while this is set.
    pure_depth: u32,
}

impl<'h, H: Host> Interpreter<'h, H> {
    pub fn new(host: &'h mut H) -> Self {
        Self { host, scopes: vec![HashMap::new()], pure_depth: 0 }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: &str, value: Value) {
        self.scopes.last_mut().expect("interpreter always has a scope").insert(name.to_string(), value);
    }

    fn lookup_local(&self, name: &str) -> Option<Value> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name).cloned())
    }

    // ── Entry points ────────────────────────────────────────────────────

    /// Call a value-layer closure or bench-local `fn` (parsed AST, not yet
    /// elaborated into a POM `Function`).
    pub fn call_fn_decl(&mut self, decl: &crate::parse::ast::FnDecl, mut args: Vec<Value>) -> Result<Value, EvalError> {
        use crate::parse::ast::FnParam;
        let params: Vec<(&str, Option<&Expr>)> = decl
            .sig
            .params
            .iter()
            .filter_map(|p| match p {
                FnParam::Typed { name, default, .. } => Some((name.as_str(), default.as_ref())),
                FnParam::SelfParam => None,
            })
            .collect();
        self.fill_defaults(&params, &mut args)?;
        let names: Vec<&str> = params.iter().map(|(n, _)| *n).collect();
        self.call_with_params(&names, &args, |me| me.exec_block(&decl.body)).map(Flow::into_value)
    }

    /// Call a POM `Function`/bundle-method body (already elaborated —
    /// generics resolved, `for` unrolled). Increments `pure_depth`: these
    /// bodies are the language's pure fn layer, never the bench itself.
    pub fn call_pom_fn(
        &mut self,
        params: &[String],
        defaults: &[Option<Expr>],
        body: &[Stmt],
        mut args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let paired: Vec<(&str, Option<&Expr>)> = params
            .iter()
            .zip(defaults.iter())
            .map(|(n, d)| (n.as_str(), d.as_ref()))
            .collect();
        self.fill_defaults(&paired, &mut args)?;
        let names: Vec<&str> = params.iter().map(String::as_str).collect();
        self.pure_depth += 1;
        let result = self.call_with_params(&names, &args, |me| me.exec_stmts_and_tail(body, None));
        self.pure_depth -= 1;
        result.map(Flow::into_value)
    }

    /// Call a sibling `bench` fn: same binding as [`Self::call_pom_fn`] but
    /// in the *effectful* context (no `pure_depth` guard) — a bench helper
    /// may run analyses and stage overrides (bench spec §2).
    pub fn call_bench_fn(
        &mut self,
        params: &[String],
        defaults: &[Option<Expr>],
        body: &[Stmt],
        tail: Option<&Expr>,
        mut args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let paired: Vec<(&str, Option<&Expr>)> = params
            .iter()
            .zip(defaults.iter())
            .map(|(n, d)| (n.as_str(), d.as_ref()))
            .collect();
        self.fill_defaults(&paired, &mut args)?;
        let names: Vec<&str> = params.iter().map(String::as_str).collect();
        self.call_with_params(&names, &args, |me| me.exec_stmts_and_tail(body, tail))
            .map(Flow::into_value)
    }

    fn call_with_params(
        &mut self,
        params: &[&str],
        args: &[Value],
        body: impl FnOnce(&mut Self) -> Result<Flow, EvalError>,
    ) -> Result<Flow, EvalError> {
        if params.len() != args.len() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} argument(s), got {}",
                params.len(),
                args.len()
            )));
        }
        self.push_scope();
        for (name, value) in params.iter().zip(args.iter()) {
            self.define(name, value.clone());
        }
        let result = body(self);
        self.pop_scope();
        result
    }

    /// Fill a call's missing trailing arguments from their default expressions
    /// (the language spec Part I §9.1). Defaults are elaboration constants, evaluated in a
    /// fresh empty scope so they cannot accidentally read the caller's locals.
    /// A missing argument with no default is a fail-loud arity error.
    fn fill_defaults(
        &mut self,
        params: &[(&str, Option<&Expr>)],
        args: &mut Vec<Value>,
    ) -> Result<(), EvalError> {
        if args.len() > params.len() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} argument(s), got {}",
                params.len(),
                args.len()
            )));
        }
        for (i, (_, default)) in params.iter().enumerate().skip(args.len()) {
            match default {
                Some(expr) => {
                    self.push_scope();
                    let v = self.eval_expr(expr);
                    self.pop_scope();
                    args.push(v?);
                }
                None => {
                    return Err(EvalError::TypeMismatch(format!(
                        "missing argument #{} (no default)",
                        i + 1
                    )));
                }
            }
        }
        Ok(())
    }

    fn call_closure(&mut self, closure: &Closure, args: Vec<Value>) -> Result<Value, EvalError> {
        if closure.params.len() != args.len() {
            return Err(EvalError::TypeMismatch(format!(
                "closure expects {} argument(s), got {}",
                closure.params.len(),
                args.len()
            )));
        }
        let captured_len = closure.captured.len();
        self.scopes.extend(closure.captured.iter().cloned());
        self.push_scope();
        for (name, value) in closure.params.iter().zip(args.into_iter()) {
            self.define(name, value);
        }
        let result = self.eval_expr(&closure.body);
        self.pop_scope();
        self.scopes.truncate(self.scopes.len() - captured_len);
        result
    }

    // ── Statements ──────────────────────────────────────────────────────

    /// Execute a `fn`-body block (SPEC Part I §9 grammar).
    pub fn exec_block(&mut self, block: &Block) -> Result<Flow, EvalError> {
        self.push_scope();
        let result = self.exec_stmts_and_tail(&block.stmts, block.expr.as_deref());
        self.pop_scope();
        result
    }

    fn exec_stmts_and_tail(&mut self, stmts: &[Stmt], tail: Option<&Expr>) -> Result<Flow, EvalError> {
        for stmt in stmts {
            match self.exec_stmt(stmt)? {
                Flow::Normal(_) => {}
                ret @ Flow::Return(_) => return Ok(ret),
            }
        }
        match tail {
            Some(e) => Ok(Flow::Normal(self.eval_expr(e)?)),
            None => Ok(Flow::Normal(Value::Unit)),
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow, EvalError> {
        match stmt {
            Stmt::VarDecl { name, ty, default } => {
                let value = match default {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Unit,
                };
                // Coerce a plain value into `Some` when the binding is optional
                // (`var b : Real? = 2.5`); `none` already evaluates to
                // `Option(None)`, and an existing `Option` passes through.
                let value = match ty {
                    Some(t) if t.optional && !matches!(value, Value::Option(_)) => {
                        Value::Option(Some(Box::new(value)))
                    }
                    _ => value,
                };
                self.define(name, value);
                Ok(Flow::Normal(Value::Unit))
            }
            Stmt::Return(e) => Ok(Flow::Return(self.eval_expr(e)?)),
            Stmt::If { cond, then_body, else_body } => {
                if self.eval_expr(cond)?.is_truthy() {
                    self.exec_block(then_body)
                } else if let Some(b) = else_body {
                    self.exec_block(b)
                } else {
                    Ok(Flow::Normal(Value::Unit))
                }
            }
            Stmt::Match { expr, arms } => self.exec_match(expr, arms),
            Stmt::For { var, iter, body } => self.exec_for(var, iter, body),
            Stmt::Bind { dest, op, src } => self.exec_bind(dest, op, src),
            Stmt::Event { .. } => Err(EvalError::TypeMismatch(
                "event blocks are analog/digital-only, not valid in an interpreted fn body".into(),
            )),
            Stmt::Diagnostic { sys, args } => {
                let arg_values = args.iter().map(|a| self.eval_expr(a)).collect::<Result<Vec<_>, _>>()?;
                self.host.syscall(sys, arg_values)?;
                Ok(Flow::Normal(Value::Unit))
            }
            Stmt::Expr(e) => {
                self.eval_expr(e)?;
                Ok(Flow::Normal(Value::Unit))
            }
        }
    }

    fn exec_bind(&mut self, dest: &Expr, op: &crate::parse::ast::BindOp, src: &Expr) -> Result<Flow, EvalError> {
        use crate::parse::ast::BindOp;
        if !matches!(op, BindOp::Assign) {
            return Err(EvalError::TypeMismatch("`<+`/`<-` are analog/digital-only, not valid in fn bodies".into()));
        }
        let value = self.eval_expr(src)?;
        if self.host.assign(dest, &value)? {
            return Ok(Flow::Normal(Value::Unit));
        }
        // Held-object field assignment (e.g. `s.ctrl = 1` on a `SelectionRef`
        // the bench holds in a local var): `assign` only sees the `Expr`, so
        // evaluate the base here and hand the value to `assign_field_on`.
        if let Expr::Field(base, field) = dest {
            let base_value = self.eval_expr(base)?;
            if self.host.assign_field_on(&base_value, field, &value)? {
                return Ok(Flow::Normal(Value::Unit));
            }
        }
        // Not host-owned: must be a local `var` (`Expr::Ident`).
        match dest {
            Expr::Ident(name) if self.set_local(name, value) => Ok(Flow::Normal(Value::Unit)),
            Expr::Ident(name) => Err(EvalError::Undefined(name.clone())),
            other => Err(EvalError::TypeMismatch(format!("cannot assign to {other:?}"))),
        }
    }

    fn set_local(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = value;
                return true;
            }
        }
        false
    }

    fn exec_for(&mut self, var: &str, iter: &crate::parse::ast::ForIter, body: &Block) -> Result<Flow, EvalError> {
        use crate::parse::ast::ForIter;
        let items: Vec<Value> = match iter {
            ForIter::Range(range) => {
                let start = self.eval_nat(&range.start)?;
                let end = self.eval_nat(&range.end)?;
                let end = if range.inclusive { end.saturating_add(1) } else { end };
                (start..end).map(Value::Nat).collect()
            }
            ForIter::Expr(e) => match self.eval_expr(e)? {
                Value::List(items) => items.borrow().clone(),
                other => {
                    return Err(EvalError::TypeMismatch(format!("cannot iterate over {}", other.type_name())));
                }
            },
        };
        for item in items {
            self.push_scope();
            self.define(var, item);
            let flow = self.exec_stmts_and_tail(&body.stmts, body.expr.as_deref());
            self.pop_scope();
            match flow? {
                Flow::Normal(_) => {}
                ret @ Flow::Return(_) => return Ok(ret),
            }
        }
        Ok(Flow::Normal(Value::Unit))
    }

    fn eval_nat(&mut self, e: &Expr) -> Result<u64, EvalError> {
        match self.eval_expr(e)? {
            Value::Nat(n) => Ok(n),
            Value::Int(n) if n >= 0 => Ok(n as u64),
            other => Err(EvalError::TypeMismatch(format!("expected a Natural bound, got {}", other.type_name()))),
        }
    }

    fn exec_match(&mut self, expr: &Expr, arms: &[StmtMatchArm]) -> Result<Flow, EvalError> {
        let scrutinee = self.eval_expr(expr)?;
        for arm in arms {
            if pattern_matches(&arm.pat, &scrutinee) {
                return self.exec_block(&arm.body);
            }
        }
        Err(EvalError::TypeMismatch(format!("no match arm covers {}", scrutinee.type_name())))
    }

    // ── Expressions ─────────────────────────────────────────────────────

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, EvalError> {
        match expr {
            Expr::Literal(lit) => Ok(eval_literal(lit)),

            Expr::Ident(name) => self
                .lookup_local(name)
                .or_else(|| self.host.lookup(name))
                .ok_or_else(|| EvalError::Undefined(name.clone())),

            Expr::Path(path) if path.segments.len() == 2 => {
                Ok(Value::EnumVariant(path.segments[0].clone(), path.segments[1].clone()))
            }
            Expr::Path(path) => Err(EvalError::TypeMismatch(format!("unsupported path {:?}", path.segments))),

            Expr::Unary(op, inner) => {
                let v = self.eval_expr(inner)?;
                eval_unary(op, v)
            }

            Expr::Binary(lhs, op, rhs) => {
                let l = self.eval_expr(lhs)?;
                let r = self.eval_expr(rhs)?;
                eval_binary(op, l, r)
            }

            Expr::Call(callee, args) => self.eval_call(callee, args),

            Expr::Cast(_, inner) => self.eval_expr(inner),

            Expr::Index(base, idx) => {
                let base_v = self.eval_expr(base)?;
                let idx_v = self.eval_expr(idx)?;
                eval_index(base_v, idx_v)
            }

            Expr::Slice(..) => Err(EvalError::TypeMismatch("slice expressions are not yet supported outside analog/digital bodies".into())),

            Expr::Field(base, field) => self.eval_field(base, field),

            Expr::Block(block) => self.exec_block(block).map(Flow::into_value),

            Expr::If { cond, then_body, else_body } => {
                let block = if self.eval_expr(cond)?.is_truthy() { then_body } else { else_body };
                self.exec_block(block).map(Flow::into_value)
            }

            Expr::Array(body) => self.eval_array(body),

            Expr::Tuple(items) => {
                let values = items.iter().map(|e| self.eval_expr(e)).collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Tuple(values))
            }

            Expr::BundleLit { ty, fields } => self.eval_bundle_lit(ty, fields),

            Expr::MapLit(entries) => {
                let pairs = entries
                    .iter()
                    .map(|(k, v)| Ok((self.eval_expr(k)?, self.eval_expr(v)?)))
                    .collect::<Result<Vec<_>, EvalError>>()?;
                Ok(Value::Map(Rc::new(std::cell::RefCell::new(pairs))))
            }

            Expr::Lambda { params, body } => {
                let captured = self.scopes.iter().flat_map(|s| s.clone()).collect::<HashMap<_, _>>();
                Ok(Value::Closure(Rc::new(Closure {
                    params: params.clone(),
                    body: (**body).clone(),
                    captured: vec![captured],
                })))
            }

            Expr::SysCall(name, args) => {
                let arg_values = args.iter().map(|a| self.eval_expr(a)).collect::<Result<Vec<_>, _>>()?;
                if self.pure_depth > 0 && !super::tasks::is_pure(name) {
                    return Err(EvalError::TaskUnavailable { name: name.clone(), context: "a pure fn/method" });
                }
                self.host.syscall(name, arg_values)
            }
        }
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value, EvalError> {
        let arg_values = args.iter().map(|a| self.eval_expr(a)).collect::<Result<Vec<_>, _>>()?;

        if let Expr::Field(recv, method) = callee {
            let recv_value = self.eval_expr(recv)?;
            // A method on a host `Object` that takes a `Value::Closure`
            // argument needs interpreter support to invoke it — host objects
            // can't call closures themselves. Route through
            // `Object::call_method_with` when a closure is in play; the
            // non-closure fast path is unchanged.
            if let Value::Object(obj) = &recv_value
                && arg_values.iter().any(|a| matches!(a, Value::Closure(_)))
            {
                let obj = obj.clone();
                let mut invoke = |c: &Closure, args: Vec<Value>| self.call_closure(c, args);
                return obj.call_method_with(method, arg_values, &mut invoke);
            }
            // `impl` method dispatch on a bundle value: `card.norm()` binds
            // `self` to the receiver and runs the method body (pure).
            if let Value::Record { ty, .. } = &recv_value {
                let ty = ty.clone();
                if let Some(callable) = self.host.resolve_method(&ty, method) {
                    return match callable {
                        Callable::Closure(c) => self.call_closure(&c, arg_values),
                        Callable::BenchFn { .. } => Err(EvalError::TypeMismatch(
                            "a bench fn is not an impl method".into(),
                        )),
                        Callable::Function { params, defaults, body } => {
                            let mut names = vec!["self".to_string()];
                            names.extend(params);
                            let mut all_defaults = vec![None];
                            all_defaults.extend(defaults);
                            let mut all_args = vec![recv_value.clone()];
                            all_args.extend(arg_values);
                            self.call_pom_fn(&names, &all_defaults, &body, all_args)
                        }
                    };
                }
            }
            return recv_value.call_builtin_method(method, arg_values);
        }

        if let Expr::Ident(name) = callee {
            if let Some(Value::Closure(c)) = self.lookup_local(name) {
                return self.call_closure(&c, arg_values);
            }
            if let Some(callable) = self.host.resolve_callable(name) {
                return match callable {
                    Callable::Closure(c) => self.call_closure(&c, arg_values),
                    Callable::Function { params, defaults, body } => {
                        self.call_pom_fn(&params, &defaults, &body, arg_values)
                    }
                    Callable::BenchFn { params, defaults, body, tail } => {
                        if self.pure_depth > 0 {
                            return Err(EvalError::TaskUnavailable {
                                name: name.clone(),
                                context: "a pure fn/method",
                            });
                        }
                        self.call_bench_fn(&params, &defaults, &body, tail.as_ref(), arg_values)
                    }
                };
            }
            // Host-intercepted plain-name call (e.g. `select("...")`).
            if let Some(result) = self.host.call_host_fn(name, &arg_values) {
                return result;
            }
            let floats: Result<Vec<f64>, EvalError> = arg_values.iter().map(as_real).collect();
            if let Ok(floats) = floats
                && let Some(result) = piperine_math::eval_const_math(name, &floats) {
                return Ok(Value::Real(result));
                }
            return Err(EvalError::Undefined(name.clone()));
        }

        match self.eval_expr(callee)? {
            Value::Closure(c) => self.call_closure(&c, arg_values),
            other => Err(EvalError::TypeMismatch(format!("{} is not callable", other.type_name()))),
        }
    }

    fn eval_field(&mut self, base: &Expr, field: &str) -> Result<Value, EvalError> {
        let base_value = self.eval_expr(base)?;
        match &base_value {
            Value::Tuple(items) => {
                let idx: usize = field
                    .parse()
                    .map_err(|_| EvalError::TypeMismatch(format!("`{field}` is not a tuple index")))?;
                items
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| EvalError::TypeMismatch(format!("tuple index {idx} out of range")))
            }
            Value::Record { fields, .. } => fields
                .borrow()
                .get(field)
                .cloned()
                .ok_or_else(|| EvalError::Undefined(format!("field `{field}`"))),
            Value::Object(obj) => obj.call_method(field, vec![]),
            other => Err(EvalError::TypeMismatch(format!("{} has no field `{field}`", other.type_name()))),
        }
    }

    fn eval_array(&mut self, body: &crate::parse::ast::ArrayBody) -> Result<Value, EvalError> {
        use crate::parse::ast::ArrayBody;
        let items = match body {
            ArrayBody::List(elems) => elems.iter().map(|e| self.eval_expr(e)).collect::<Result<Vec<_>, _>>()?,
            ArrayBody::Repeat(value, count) => {
                let v = self.eval_expr(value)?;
                let n = self.eval_nat(count)?;
                vec![v; n as usize]
            }
            ArrayBody::Comprehension(expr, var, range) => {
                let start = self.eval_nat(&range.start)?;
                let end = self.eval_nat(&range.end)?;
                let end = if range.inclusive { end.saturating_add(1) } else { end };
                let mut items = Vec::with_capacity((end - start) as usize);
                for i in start..end {
                    self.push_scope();
                    self.define(var, Value::Nat(i));
                    let v = self.eval_expr(expr);
                    self.pop_scope();
                    items.push(v?);
                }
                items
            }
        };
        Ok(Value::List(Rc::new(std::cell::RefCell::new(items))))
    }

    fn eval_bundle_lit(&mut self, ty: &crate::parse::ast::Type, fields: &[(String, Expr)]) -> Result<Value, EvalError> {
        let mut values = HashMap::new();
        for (name, expr) in fields {
            values.insert(name.clone(), self.eval_expr(expr)?);
        }
        if let Some(decl) = self.host.lookup(&format!("bundle:{}", ty.name))
            && let Value::Record { fields: defaults, .. } = decl {
                for (name, default) in defaults.borrow().iter() {
                    values.entry(name.clone()).or_insert_with(|| default.clone());
                }
            }
        Ok(Value::Record {
            ty: ty.name.clone(),
            fields: Rc::new(std::cell::RefCell::new(values)),
        })
    }
}

fn as_real(v: &Value) -> Result<f64, EvalError> {
    match v {
        Value::Real(r) => Ok(*r),
        Value::Nat(n) => Ok(*n as f64),
        Value::Int(n) => Ok(*n as f64),
        other => Err(EvalError::TypeMismatch(format!("expected a Real, got {}", other.type_name()))),
    }
}

fn eval_literal(lit: &Literal) -> Value {
    match lit {
        Literal::Int(n) => Value::Nat(*n),
        Literal::Real(r) => Value::Real(*r),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::String(s) => Value::Str(s.clone()),
        Literal::Quad(q) => Value::Str(format!("0q{q}")),
        // `none` — the absent optional value.
        Literal::None => Value::Option(None),
    }
}

fn eval_unary(op: &UnaryOp, v: Value) -> Result<Value, EvalError> {
    match (op, v) {
        (UnaryOp::Neg, Value::Nat(n)) => Ok(Value::Int(-(n as i64))),
        (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
        (UnaryOp::Neg, Value::Real(r)) => Ok(Value::Real(-r)),
        (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
        (UnaryOp::Not, Value::Nat(n)) => Ok(Value::Nat(!n)),
        (op, v) => Err(EvalError::TypeMismatch(format!("cannot apply {op:?} to {}", v.type_name()))),
    }
}

/// Ported verbatim (semantics-preserving) from the legacy
/// `ConstEnv::eval_binary` — `Nat` arithmetic wraps, mixed `Nat`/`Int`
/// widens to `Int`, comparisons are same-type-only.
fn eval_binary(op: &BinaryOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use BinaryOp::*;
    use Value::*;

    match (op, l, r) {
        (Add, Nat(a), Nat(b)) => Ok(Nat(a.wrapping_add(b))),
        (Sub, Nat(a), Nat(b)) => Ok(Nat(a.wrapping_sub(b))),
        (Mul, Nat(a), Nat(b)) => Ok(Nat(a.wrapping_mul(b))),
        (Div, Nat(_), Nat(0)) => Err(EvalError::DivByZero),
        (Div, Nat(a), Nat(b)) => Ok(Nat(a / b)),
        (Rem, Nat(_), Nat(0)) => Err(EvalError::DivByZero),
        (Rem, Nat(a), Nat(b)) => Ok(Nat(a % b)),

        (Add, Int(a), Int(b)) => Ok(Int(a.wrapping_add(b))),
        (Sub, Int(a), Int(b)) => Ok(Int(a.wrapping_sub(b))),
        (Mul, Int(a), Int(b)) => Ok(Int(a.wrapping_mul(b))),
        (Div, Int(_), Int(0)) => Err(EvalError::DivByZero),
        (Div, Int(a), Int(b)) => Ok(Int(a / b)),
        (Rem, Int(_), Int(0)) => Err(EvalError::DivByZero),
        (Rem, Int(a), Int(b)) => Ok(Int(a % b)),

        (Add, Nat(a), Int(b)) => Ok(Int(a as i64 + b)),
        (Add, Int(a), Nat(b)) => Ok(Int(a + b as i64)),
        (Sub, Nat(a), Int(b)) => Ok(Int(a as i64 - b)),
        (Sub, Int(a), Nat(b)) => Ok(Int(a - b as i64)),
        (Mul, Nat(a), Int(b)) => Ok(Int(a as i64 * b)),
        (Mul, Int(a), Nat(b)) => Ok(Int(a * b as i64)),

        (Add, Real(a), Real(b)) => Ok(Real(a + b)),
        (Sub, Real(a), Real(b)) => Ok(Real(a - b)),
        (Mul, Real(a), Real(b)) => Ok(Real(a * b)),
        (Div, Real(a), Real(b)) => Ok(Real(a / b)),

        (Eq, Nat(a), Nat(b)) => Ok(Bool(a == b)),
        (Neq, Nat(a), Nat(b)) => Ok(Bool(a != b)),
        (Lt, Nat(a), Nat(b)) => Ok(Bool(a < b)),
        (Le, Nat(a), Nat(b)) => Ok(Bool(a <= b)),
        (Gt, Nat(a), Nat(b)) => Ok(Bool(a > b)),
        (Ge, Nat(a), Nat(b)) => Ok(Bool(a >= b)),

        (Eq, Int(a), Int(b)) => Ok(Bool(a == b)),
        (Neq, Int(a), Int(b)) => Ok(Bool(a != b)),
        (Lt, Int(a), Int(b)) => Ok(Bool(a < b)),
        (Le, Int(a), Int(b)) => Ok(Bool(a <= b)),
        (Gt, Int(a), Int(b)) => Ok(Bool(a > b)),
        (Ge, Int(a), Int(b)) => Ok(Bool(a >= b)),

        (Eq, Real(a), Real(b)) => Ok(Bool(a == b)),
        (Neq, Real(a), Real(b)) => Ok(Bool(a != b)),
        (Lt, Real(a), Real(b)) => Ok(Bool(a < b)),
        (Le, Real(a), Real(b)) => Ok(Bool(a <= b)),
        (Gt, Real(a), Real(b)) => Ok(Bool(a > b)),
        (Ge, Real(a), Real(b)) => Ok(Bool(a >= b)),

        (Eq, Bool(a), Bool(b)) => Ok(Bool(a == b)),
        (Neq, Bool(a), Bool(b)) => Ok(Bool(a != b)),

        (Eq, Str(a), Str(b)) => Ok(Bool(a == b)),
        (Neq, Str(a), Str(b)) => Ok(Bool(a != b)),

        (Eq, EnumVariant(e1, v1), EnumVariant(e2, v2)) => Ok(Bool(e1 == e2 && v1 == v2)),
        (Neq, EnumVariant(e1, v1), EnumVariant(e2, v2)) => Ok(Bool(e1 != e2 || v1 != v2)),

        (BitAnd, Nat(a), Nat(b)) => Ok(Nat(a & b)),
        (BitOr, Nat(a), Nat(b)) => Ok(Nat(a | b)),
        (BitXor, Nat(a), Nat(b)) => Ok(Nat(a ^ b)),
        (BitAnd, Bool(a), Bool(b)) => Ok(Bool(a & b)),
        (BitOr, Bool(a), Bool(b)) => Ok(Bool(a | b)),
        (BitXor, Bool(a), Bool(b)) => Ok(Bool(a ^ b)),

        (And, Bool(a), Bool(b)) => Ok(Bool(a && b)),
        (Or, Bool(a), Bool(b)) => Ok(Bool(a || b)),

        (op, l, r) => Err(EvalError::TypeMismatch(format!(
            "cannot apply {op:?} to {} and {}",
            l.type_name(),
            r.type_name()
        ))),
    }
}

fn eval_index(base: Value, idx: Value) -> Result<Value, EvalError> {
    match (base, idx) {
        (Value::List(items), Value::Nat(i)) => items
            .borrow()
            .get(i as usize)
            .cloned()
            .ok_or_else(|| EvalError::TypeMismatch(format!("index {i} out of range"))),
        (Value::Tuple(items), Value::Nat(i)) => items
            .get(i as usize)
            .cloned()
            .ok_or_else(|| EvalError::TypeMismatch(format!("index {i} out of range"))),
        (base, idx) => Err(EvalError::TypeMismatch(format!(
            "cannot index {} with {}",
            base.type_name(),
            idx.type_name()
        ))),
    }
}

fn pattern_matches(pat: &Pattern, value: &Value) -> bool {
    match pat {
        Pattern::Wildcard => true,
        Pattern::Literal(n) => matches!(value, Value::Nat(v) if v == n)
            || matches!(value, Value::Int(v) if *v == *n as i64),
        Pattern::Path(path) if path.segments.len() == 2 => {
            matches!(value, Value::EnumVariant(e, v) if e == &path.segments[0] && v == &path.segments[1])
        }
        Pattern::Path(path) => path.segments.last().is_some_and(|last| {
            matches!(value, Value::EnumVariant(_, v) if v == last)
        }),
        Pattern::BitPattern(_) => false,
    }
}

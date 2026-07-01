//! Interpreted digital behavior for PHDL `digital` blocks.
//!
//! Unlike analog codegen (which JIT-compiles to native code), digital
//! evaluation runs as a tree-walking interpreter.  Digital blocks execute
//! at discrete event times — not at every Newton iteration — so the
//! interpreter overhead is negligible.
//!
//! # Usage
//!
//! 1. Call [`compile_digital_module`] to build a [`DigitalInterpreter`] from
//!    an [`Design`].  The interpreter knows which ports are
//!    inputs/outputs but not yet which [`DigitalNet`] indices they map to.
//!
//! 2. Call [`DigitalInterpreter::set_port_nets`] once the circuit builder has
//!    allocated [`DigitalNet`] indices for each wire.
//!
//! 3. Call [`DigitalInterpreter::init`] to run `VarDecl` defaults and any
//!    `@initial` blocks.
//!
//! 4. At simulation time, call [`DigitalInterpreter::eval`].

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;

use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};

use piperine_codegen::CodegenError;
use crate::pom::{Behavior, BehaviorStmt, Design};
use crate::parse::ast::{BindOp, BinaryOp, EventSpec, Expr, Literal, UnaryOp};

// ─────────────────────────────── Value type ──────────────────────────────────

/// Runtime value inside a digital behavior evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum DigitalVal {
    Logic(LogicValue),
    Natural(u64),
    Integer(i64),
    Real(f64),
    Bool(bool),
}

impl DigitalVal {
    /// Test whether the value is truthy (for if-conditionals).
    fn as_bool(&self) -> bool {
        match self {
            DigitalVal::Bool(b)    => *b,
            DigitalVal::Logic(lv)  => *lv == LogicValue::One,
            DigitalVal::Natural(n) => *n != 0,
            DigitalVal::Integer(i) => *i != 0,
            DigitalVal::Real(f)    => *f != 0.0,
        }
    }

    /// Coerce this value into a 4-state logic value.
    fn as_logic(&self) -> LogicValue {
        match self {
            DigitalVal::Logic(lv)  => *lv,
            DigitalVal::Bool(true) => LogicValue::One,
            DigitalVal::Bool(false) => LogicValue::Zero,
            DigitalVal::Natural(0) => LogicValue::Zero,
            DigitalVal::Natural(_) => LogicValue::One,
            DigitalVal::Integer(0) => LogicValue::Zero,
            DigitalVal::Integer(_) => LogicValue::One,
            DigitalVal::Real(f) if *f == 0.0 => LogicValue::Zero,
            DigitalVal::Real(_) => LogicValue::One,
        }
    }
}

// ─────────────────────────────── Interpreter ─────────────────────────────────

/// Interpreter for one PHDL `digital` behavior block.
pub struct DigitalInterpreter {
    /// Body of the `digital Foo { ... }` block (top-level stmts only; nested
    /// statements are walked recursively at eval time).
    body: Vec<BehaviorStmt>,

    /// Port names whose DigitalNet indices are known (set by `set_port_nets`).
    port_net_map: HashMap<String, DigitalNet>,

    /// Nets referenced in event specs → used as digital inputs.
    pub input_port_names: Vec<String>,

    /// Net names assigned to in the body → used as digital outputs.
    pub output_port_names: Vec<String>,

    /// Cached from `port_net_map` after `set_port_nets`; drives
    /// `Device::digital_input_nets`.
    input_nets: Vec<DigitalNet>,

    /// Cached from `port_net_map` after `set_port_nets`; drives
    /// `Device::digital_output_nets`.
    output_nets: Vec<DigitalNet>,

    /// Previous net values keyed by DigitalNet, for edge detection.
    prev_nets: HashMap<DigitalNet, LogicValue>,

    /// Persistent state variables declared with `var` inside the block.
    state: HashMap<String, DigitalVal>,

    /// Monotonically increasing sequence number for scheduled events.
    seq: u64,

    /// Device identity stamp placed on every emitted `DigitalEvent`.
    device_id: usize,

    /// Deferred updates for internal state variables from digital assignments.
    deferred_updates: Vec<(String, DigitalVal)>,
}

impl DigitalInterpreter {
    /// Create a new interpreter from a statement body and port-name lists.
    pub fn new(
        body: Vec<BehaviorStmt>,
        input_port_names: Vec<String>,
        output_port_names: Vec<String>,
        device_id: usize,
    ) -> Self {
        Self {
            body,
            port_net_map: HashMap::new(),
            input_port_names,
            output_port_names,
            input_nets: Vec::new(),
            output_nets: Vec::new(),
            prev_nets: HashMap::new(),
            state: HashMap::new(),
            seq: 0,
            device_id,
            deferred_updates: Vec::new(),
        }
    }

    /// Assign DigitalNet indices to port names.
    ///
    /// Must be called before `init` or `eval`.
    pub fn set_port_nets(&mut self, map: HashMap<String, DigitalNet>) {
        self.input_nets = self.input_port_names.iter()
            .filter_map(|n| map.get(n).copied())
            .collect();
        self.output_nets = self.output_port_names.iter()
            .filter_map(|n| map.get(n).copied())
            .collect();
        self.port_net_map = map;
    }

    /// Snapshot of allocated input nets (valid after `set_port_nets`).
    pub fn input_nets(&self) -> &[DigitalNet] { &self.input_nets }

    /// Snapshot of allocated output nets (valid after `set_port_nets`).
    pub fn output_nets(&self) -> &[DigitalNet] { &self.output_nets }

    /// Initialize state variables from `VarDecl` defaults.
    ///
    /// Also executes any top-level `@ initial` event blocks, scheduling
    /// zero-time output events.
    pub fn init(&mut self, queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        let body = std::mem::take(&mut self.body);
        for stmt in &body {
            match stmt {
                BehaviorStmt::VarDecl { name, default: Some(expr), .. } => {
                    let val = self.eval_expr(expr, &[]);
                    self.state.insert(name.clone(), val);
                }
                BehaviorStmt::VarDecl { name, default: None, ty } => {
                    use crate::pom::ValueType;
                    let val = match ty {
                        ValueType::Real | ValueType::Complex => DigitalVal::Real(0.0),
                        ValueType::Boolean => DigitalVal::Bool(false),
                        ValueType::Integer => DigitalVal::Integer(0),
                        _ => DigitalVal::Natural(0),
                    };
                    self.state.insert(name.clone(), val);
                }
                BehaviorStmt::Event { spec, guard, body: event_body } => {
                    if spec_is_initial(spec) {
                        if let Some(g) = guard {
                            if !self.eval_expr(g, &[]).as_bool() { continue; }
                        }
                        let eb = event_body.clone();
                        self.exec_stmts(&eb, 0.0, &[], queue);
                    }
                }
                _ => {}
            }
        }
        self.body = body;
    }

    /// Evaluate digital behavior given the current net state.
    ///
    /// Fires any event blocks whose specs are satisfied by the transition
    /// `prev_nets → nets`, schedules output `DigitalEvent`s into `queue`.
    pub fn eval(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let body = std::mem::take(&mut self.body);
        for stmt in &body {
            if let BehaviorStmt::Event { spec, guard, body: event_body } = stmt {
                if spec_is_initial(spec) { continue; }
                if self.spec_fires(spec, nets) {
                    if let Some(g) = guard {
                        if !self.eval_expr(g, nets).as_bool() { continue; }
                    }
                    let eb = event_body.clone();
                    self.exec_stmts(&eb, t, nets, queue);
                }
            }
        }
        self.body = body;

        for (name, val) in self.deferred_updates.drain(..) {
            self.state.insert(name, val);
        }

        // Snapshot current values of our input nets for next edge detection.
        for &net in &self.input_nets {
            if let Some(&lv) = nets.get(net.0) {
                self.prev_nets.insert(net, lv);
            }
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Check whether an event spec matches the current net transition
    /// (posedge, negedge, or change).
    fn spec_fires(&self, spec: &EventSpec, nets: &[LogicValue]) -> bool {
        match spec {
            EventSpec::Named { name, arg } => {
                let net_name = expr_ident_name(arg);
                let net = net_name.and_then(|n| self.port_net_map.get(n)).copied();
                let Some(dnet) = net else { return false; };
                let prev = self.prev_nets.get(&dnet).copied().unwrap_or(LogicValue::X);
                let curr = nets.get(dnet.0).copied().unwrap_or(LogicValue::X);
                match name.as_str() {
                    "posedge" => prev != LogicValue::One && curr == LogicValue::One,
                    "negedge" => prev == LogicValue::One && curr != LogicValue::One,
                    "change"  => prev != curr,
                    _ => false,
                }
            }
            EventSpec::Or(specs) => specs.iter().any(|s| self.spec_fires(s, nets)),
            // Initial/Final are handled separately via init(); never fire here.
            EventSpec::Initial | EventSpec::Final => false,
        }
    }

    /// Execute a sequence of behavior statements at time `t`, scheduling
    /// output events into `queue`.
    fn exec_stmts(
        &mut self,
        stmts: &[BehaviorStmt],
        t: f64,
        nets: &[LogicValue],
        queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        for stmt in stmts {
            self.exec_one(stmt, t, nets, queue);
        }
    }

    /// Execute a single behavior statement, dispatching by variant.
    fn exec_one(
        &mut self,
        stmt: &BehaviorStmt,
        t: f64,
        nets: &[LogicValue],
        queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        match stmt {
            BehaviorStmt::VarDecl { name, default, .. } => {
                if let Some(expr) = default {
                    let val = self.eval_expr(expr, nets);
                    self.state.insert(name.clone(), val);
                }
            }

            BehaviorStmt::Bind { dest, op: BindOp::Force | BindOp::Assign, src } => {
                let val = self.eval_expr(src, nets);
                if let Some(name) = expr_ident_name(dest) {
                    if let Some(&dnet) = self.port_net_map.get(name) {
                        // Output to a digital net → schedule event (inherently deferred/non-blocking).
                        let lv = val.as_logic();
                        let seq = self.seq;
                        self.seq += 1;
                        queue.push(Reverse(DigitalEvent {
                            time: t,
                            net: dnet,
                            value: lv,
                            source: self.device_id,
                            seq,
                        }));
                    } else {
                        // Internal state variable (deferred/non-blocking update).
                        self.deferred_updates.push((name.to_string(), val));
                    }
                }
            }

            BehaviorStmt::Bind { .. } => { /* Contrib/unknown — not valid in digital */ }

            BehaviorStmt::If { cond, then_body, else_body } => {
                let tb = then_body.clone();
                let eb = else_body.clone();
                if self.eval_expr(cond, nets).as_bool() {
                    self.exec_stmts(&tb, t, nets, queue);
                } else if let Some(else_b) = eb {
                    self.exec_stmts(&else_b, t, nets, queue);
                }
            }

            BehaviorStmt::Match { expr, arms } => {
                let val = self.eval_expr(expr, nets);
                let arms = arms.clone();
                for arm in &arms {
                    if pattern_matches(arm.pattern(), &val) {
                        let body = arm.body().to_vec();
                        self.exec_stmts(&body, t, nets, queue);
                        break;
                    }
                }
            }

            BehaviorStmt::Event { spec, guard, body: nested } => {
                // Nested event blocks — fire immediately if spec fires now.
                if !spec_is_initial(spec) && self.spec_fires(spec, nets) {
                    if let Some(g) = guard {
                        let gclone = g.clone();
                        if !self.eval_expr(&gclone, nets).as_bool() { return; }
                    }
                    let nb = nested.clone();
                    self.exec_stmts(&nb, t, nets, queue);
                }
            }

            BehaviorStmt::Diagnostic { .. }
            | BehaviorStmt::Expr(_)
            // GAPS §D.5 — fn-body returns are stripped before the
            // interpreter runs (the inliner already inlined them).
            | BehaviorStmt::Return(_) => {}
        }
    }

    /// Evaluate an expression to a [`DigitalVal`], resolving identifiers
    /// against the interpreter state and input nets.
    fn eval_expr(&mut self, expr: &Expr, nets: &[LogicValue]) -> DigitalVal {
        match expr {
            Expr::Literal(Literal::Int(n))  => DigitalVal::Natural(*n),
            Expr::Literal(Literal::Real(f)) => DigitalVal::Real(*f),
            Expr::Literal(Literal::Bool(b)) => DigitalVal::Bool(*b),

            Expr::Ident(name) => {
                if let Some(val) = self.state.get(name) {
                    val.clone()
                } else if let Some(&dnet) = self.port_net_map.get(name) {
                    DigitalVal::Logic(nets.get(dnet.0).copied().unwrap_or(LogicValue::X))
                } else {
                    DigitalVal::Natural(0)
                }
            }

            Expr::Binary(lhs, op, rhs) => {
                let l = self.eval_expr(lhs, nets);
                let r = self.eval_expr(rhs, nets);
                eval_binop(op, l, r)
            }

            Expr::Unary(op, inner) => {
                let v = self.eval_expr(inner, nets);
                eval_unop(op, v)
            }

            Expr::Call(_func, _args) => {
                DigitalVal::Natural(0)
            }

            Expr::SysCall(_, _) => DigitalVal::Natural(0),

            _ => DigitalVal::Natural(0),
        }
    }
}

// ─────────────────────────────── Helpers ─────────────────────────────────────

/// Extract the identifier name from an expression if it is a bare `Expr::Ident`.
fn expr_ident_name(expr: &Expr) -> Option<&str> {
    if let Expr::Ident(name) = expr { Some(name.as_str()) } else { None }
}

/// Check whether an event spec is `@initial` (handled at init time, not
/// during normal evaluation).
fn spec_is_initial(spec: &EventSpec) -> bool {
    match spec {
        EventSpec::Initial => true,
        EventSpec::Named { name, .. } => name == "initial",
        _ => false,
    }
}

/// Check if a match-arm pattern matches a given value (wildcard or
/// path-bindings match everything).
fn pattern_matches(pat: &crate::parse::ast::Pattern, _val: &DigitalVal) -> bool {
    use crate::parse::ast::Pattern;
    match pat {
        Pattern::Wildcard => true,
        Pattern::Path(_)  => true, // enum path or binding — treat as match-all for now
    }
}

/// Coerce Logic to Natural for arithmetic/comparison contexts.
fn coerce(v: DigitalVal) -> DigitalVal {
    match v {
        DigitalVal::Logic(LogicValue::Zero) => DigitalVal::Natural(0),
        DigitalVal::Logic(LogicValue::One)  => DigitalVal::Natural(1),
        other => other,
    }
}

/// Evaluate a binary operator on two digital values, normalizing Logic to
/// Natural for arithmetic/comparison contexts.
fn eval_binop(op: &BinaryOp, l: DigitalVal, r: DigitalVal) -> DigitalVal {
    use DigitalVal::*;
    // Normalize Logic → Natural before any comparison or arithmetic so that
    // `A == 0` works when A is a LogicValue net read.
    let l = coerce(l);
    let r = coerce(r);
    match (op, &l, &r) {
        // Equality / comparison → Bool result
        (BinaryOp::Eq,  _, _) => Bool(l == r),
        (BinaryOp::Neq, _, _) => Bool(l != r),

        // Numeric ops
        (BinaryOp::Add, Natural(a), Natural(b)) => Natural(a.wrapping_add(*b)),
        (BinaryOp::Sub, Natural(a), Natural(b)) => Natural(a.wrapping_sub(*b)),
        (BinaryOp::Mul, Natural(a), Natural(b)) => Natural(a.wrapping_mul(*b)),
        (BinaryOp::Div, Natural(a), Natural(b)) if *b != 0 => Natural(a / b),
        (BinaryOp::Rem, Natural(a), Natural(b)) if *b != 0 => Natural(a % b),

        (BinaryOp::Add, Integer(a), Integer(b)) => Integer(a.wrapping_add(*b)),
        (BinaryOp::Sub, Integer(a), Integer(b)) => Integer(a.wrapping_sub(*b)),
        (BinaryOp::Mul, Integer(a), Integer(b)) => Integer(a.wrapping_mul(*b)),
        (BinaryOp::Div, Integer(a), Integer(b)) if *b != 0 => Integer(a / b),
        (BinaryOp::Rem, Integer(a), Integer(b)) if *b != 0 => Integer(a % b),

        (BinaryOp::Add, Real(a), Real(b)) => Real(a + b),
        (BinaryOp::Sub, Real(a), Real(b)) => Real(a - b),
        (BinaryOp::Mul, Real(a), Real(b)) => Real(a * b),
        (BinaryOp::Div, Real(a), Real(b)) if *b != 0.0 => Real(a / b),

        // Comparison
        (BinaryOp::Lt,  Natural(a), Natural(b)) => Bool(a < b),
        (BinaryOp::Le,  Natural(a), Natural(b)) => Bool(a <= b),
        (BinaryOp::Gt,  Natural(a), Natural(b)) => Bool(a > b),
        (BinaryOp::Ge,  Natural(a), Natural(b)) => Bool(a >= b),

        (BinaryOp::Lt,  Integer(a), Integer(b)) => Bool(a < b),
        (BinaryOp::Le,  Integer(a), Integer(b)) => Bool(a <= b),
        (BinaryOp::Gt,  Integer(a), Integer(b)) => Bool(a > b),
        (BinaryOp::Ge,  Integer(a), Integer(b)) => Bool(a >= b),

        (BinaryOp::Lt,  Real(a), Real(b)) => Bool(a < b),
        (BinaryOp::Le,  Real(a), Real(b)) => Bool(a <= b),
        (BinaryOp::Gt,  Real(a), Real(b)) => Bool(a > b),
        (BinaryOp::Ge,  Real(a), Real(b)) => Bool(a >= b),

        // Bitwise / logic
        (BinaryOp::BitAnd, Natural(a), Natural(b)) => Natural(a & b),
        (BinaryOp::BitOr,  Natural(a), Natural(b)) => Natural(a | b),
        (BinaryOp::BitXor, Natural(a), Natural(b)) => Natural(a ^ b),
        (BinaryOp::And, Bool(a), Bool(b)) => Bool(*a && *b),
        (BinaryOp::Or,  Bool(a), Bool(b)) => Bool(*a || *b),

        _ => Natural(0),
    }
}

/// Evaluate a unary operator (`!` or `-`) on a digital value.
fn eval_unop(op: &UnaryOp, v: DigitalVal) -> DigitalVal {
    use DigitalVal::*;
    match (op, v) {
        (UnaryOp::Not, Bool(b))    => Bool(!b),
        (UnaryOp::Not, Natural(n)) => Natural(!n),
        (UnaryOp::Neg, Integer(i)) => Integer(-i),
        (UnaryOp::Neg, Real(f))    => Real(-f),
        _ => Natural(0),
    }
}

// ─────────────────────────────── Scan helpers ─────────────────────────────────

/// Collect net names referenced as event spec arguments anywhere in `stmts`.
fn scan_event_inputs(stmts: &[BehaviorStmt], out: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            BehaviorStmt::Event { spec, body, .. } => {
                collect_spec_nets(spec, out);
                scan_event_inputs(body, out);
            }
            BehaviorStmt::If { then_body, else_body, .. } => {
                scan_event_inputs(then_body, out);
                if let Some(eb) = else_body { scan_event_inputs(eb, out); }
            }
            BehaviorStmt::Match { arms, .. } => {
                for arm in arms { scan_event_inputs(arm.body(), out); }
            }
            _ => {}
        }
    }
}

/// Collect net names referenced in event-spec arguments from a given spec node.
fn collect_spec_nets(spec: &EventSpec, out: &mut Vec<String>) {
    match spec {
        EventSpec::Named { arg, .. } => {
            if let Some(n) = expr_ident_name(arg) {
                if !out.contains(&n.to_string()) { out.push(n.to_string()); }
            }
        }
        EventSpec::Or(specs) => { for s in specs { collect_spec_nets(s, out); } }
        EventSpec::Initial | EventSpec::Final => {}
    }
}

/// Collect net names that appear as assignment destinations anywhere in `stmts`.
fn scan_output_names(stmts: &[BehaviorStmt], out: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            BehaviorStmt::Bind { dest, op: BindOp::Force | BindOp::Assign, .. } => {
                if let Some(n) = expr_ident_name(dest) {
                    if !out.contains(&n.to_string()) { out.push(n.to_string()); }
                }
            }
            BehaviorStmt::Event { body, .. } => { scan_output_names(body, out); }
            BehaviorStmt::If { then_body, else_body, .. } => {
                scan_output_names(then_body, out);
                if let Some(eb) = else_body { scan_output_names(eb, out); }
            }
            BehaviorStmt::Match { arms, .. } => {
                for arm in arms { scan_output_names(arm.body(), out); }
            }
            _ => {}
        }
    }
}

// ─────────────────────────────── Public entry point ──────────────────────────

/// Build a [`DigitalInterpreter`] for `module_name` from an [`Design`].
///
/// Returns [`CodegenError::BehaviorNotFound`] if the program has no `digital`
/// block named `module_name`.
pub fn compile_digital_module(
    prog: &Design,
    module_name: &str,
    device_id: usize,
) -> Result<DigitalInterpreter, CodegenError> {
    let behavior: &Behavior = prog.module(module_name)
        .and_then(|m| m.behaviors().iter().find(|b| b.is_digital()))
        .ok_or_else(|| CodegenError::BehaviorNotFound(module_name.to_string()))?;

    let mut input_names: Vec<String> = Vec::new();
    let mut output_names: Vec<String> = Vec::new();

    scan_event_inputs(behavior.body(), &mut input_names);
    scan_output_names(behavior.body(), &mut output_names);

    // Outputs that also appear as inputs (e.g. bidirectional `inout`) are fine.

    Ok(DigitalInterpreter::new(
        behavior.body().to_vec(),
        input_names,
        output_names,
        device_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ast::{BindOp, Expr as PExpr};
    use piperine_solver::digital::{LogicValue, DigitalEvent};
    use std::collections::BinaryHeap;

    #[test]
    fn test_digital_assignment_is_non_blocking() {
        let mut queue = BinaryHeap::new();
        let mut map = HashMap::new();
        map.insert("clk".to_string(), DigitalNet(0));

        // Test that ALL assignments in digital blocks are non-blocking!
        // a = 1; b = a; -> b should get the old value of a (0), while a gets 1.
        let mut interp = DigitalInterpreter::new(
            vec![
                BehaviorStmt::Event {
                    spec: EventSpec::Named { name: "change".to_string(), arg: PExpr::Ident("clk".to_string()) },
                    guard: None,
                    body: vec![
                        BehaviorStmt::Bind { dest: PExpr::Ident("a".to_string()), op: BindOp::Assign, src: PExpr::Literal(crate::parse::ast::Literal::Int(1)) },
                        BehaviorStmt::Bind { dest: PExpr::Ident("b".to_string()), op: BindOp::Assign, src: PExpr::Ident("a".to_string()) },
                    ]
                }
            ],
            vec!["clk".to_string()], vec![], 0
        );
        interp.set_port_nets(map);
        interp.state.insert("a".to_string(), DigitalVal::Natural(0));
        interp.state.insert("b".to_string(), DigitalVal::Natural(0));
        
        interp.eval(1.0, &[LogicValue::One], &mut queue);
        assert_eq!(interp.state.get("a").unwrap(), &DigitalVal::Natural(1));
        assert_eq!(interp.state.get("b").unwrap(), &DigitalVal::Natural(0)); // b gets OLD a
    }
}

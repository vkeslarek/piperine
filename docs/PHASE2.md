# Piperine Phase 2 — Architectural Refinements (Implementation Guide)

Foundational structural changes that must land before feature breadth expansion.
This document is written for implementation — every section specifies the exact
file, the exact Rust types to change, a rationale, and unambiguous code examples.

---

## 0. Guiding Principles

1. **Net references are identifiers, not strings.** In an analog expression context
   `V(X1.mid)` is a hierarchical path to a net, not a string argument to a function.
   Strings are for runtime lookups in testbench `initial` blocks (`$V("out")`).
   These are different syntactic contexts with different semantics.

2. **Plugin gets raw AST + resolver, not evaluated values.** For `parameter expr`,
   the elaborator hands the AST node + a `NetResolver` to the plugin. The plugin
   decides what the expression means (e.g., B-source lowers it to ngspice syntax).

3. **Worker ↔ coordinator is a streaming protocol, not request-response.**
   One `Command::RunAnalysis` yields many `Response::Event` + `Response::AnalysisDone`.
   The coordinator processes events synchronously (runs Piperine code) and sends
   `Command::EventResponse` back. Datapoints are pulled after run completion.

4. **Two assertion modes: panic vs run-error.** `assert` halts the process.
   `assert_run` marks the current analysis run as failed and continues the outer loop.
   Optimizer/MC loops need to survive individual run failures.

5. **`extern class` is how plugins expose Rust types to Piperine.** `TranResult`,
   `AcResult`, `Signal` are Rust structs behind the `ExternClass` trait. No new
   parser machinery — method calls dispatch to `ExternClass::call_method()`.

---

## 1. Node References — Identifiers, Not Strings

### 1.1 The Problem

Old (wrong):
```verilog
spice_bsource_v #(.V( V("X1.mid") * V("X1.mid") )) B1 (.p(out), .n(gnd));
```
`"X1.mid"` is a string literal. The parser doesn't know it's a net. The elaborator
can't resolve it at elaboration time. The plugin has to parse it itself.

Correct:
```verilog
spice_bsource_v #(.V( V(X1.mid) * V(X1.mid) )) B1 (.p(out), .n(gnd));
```
`X1.mid` is a **hierarchical path** (`Expr::Path` with a qualifier chain). The
elaborator resolves it through the `NetResolver`. The plugin never sees a raw string.

### 1.2 `V()` and `I()` in Expression Context vs. Testbench Context

| Context | Syntax | Mechanism |
|---------|--------|-----------|
| `parameter expr` (B-source body) | `V(X1.mid)` | Parsed as `Expr::Call("V", [Expr::Path(X1.mid)])` — AST passthrough, resolved by `NetResolver` |
| Testbench `initial` block | `$V("out")` | System task, evaluated at runtime by interpreter, string name → backend lookup |

These are **not** the same construct. `V(...)` (no `$`) in an expression context is an
analog access function. `$V(...)` is a system task. The parser distinguishes them by the
`$` prefix. Never use strings for net references in structural/expr context.

### 1.3 Hierarchical Path in the Existing AST

`crates/piperine-parser/src/ast/mod.rs` — `Path` already supports hierarchy:
```rust
pub struct Path {
    pub qualifier: Option<Box<Path>>,  // Some(Box(Path{Ident("X1")})) for X1.mid
    pub segment: PathSegment,
}
```

`X1.mid` parses to:
```rust
Path {
    qualifier: Some(Box::new(Path { qualifier: None, segment: PathSegment::Ident("X1".into()) })),
    segment: PathSegment::Ident("mid".into()),
}
```

**No AST change needed for hierarchical paths.** Only the `NetResolver` and the
`parameter expr` kind need to be added.

### 1.4 `parameter expr` — New Parameter Kind

**File:** `crates/piperine-parser/src/ast/item.rs`

Add a variant to `ExternParameter`:
```rust
/// One parameter in an `extern module` declaration.
#[derive(Debug, Clone)]
pub struct ExternParameter {
    pub name: Name,
    pub kind: ExternParameterKind,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum ExternParameterKind {
    /// Normal typed parameter: `parameter real r = 1e3`
    Typed(Type),
    /// AST-passthrough parameter: `parameter expr V`
    /// The elaborator passes the raw AST Expr to the plugin — no evaluation.
    Expr,
}
```

**Parser change** (`crates/piperine-parser/src/grammar/item.rs`):
When parsing `extern module` parameter list, if the type keyword is `expr` (a new
contextual keyword, not a reserved word), produce `ExternParameterKind::Expr`.
Otherwise produce `ExternParameterKind::Typed(parse_type())`.

**Elaborator change** (`crates/piperine-circuit/src/elaboration.rs`):
In `resolve_parameters()`, when the definition has `ExternParameterKind::Expr`, do NOT
call `ast_expr_to_parameter_value()`. Instead store the raw `Expr` in a new
`ParameterValue::Ast(Expr)` variant and pass it through to `instantiate()`.

**Types change** (`crates/piperine-circuit/src/types.rs`):
```rust
#[derive(Debug, Clone)]
pub enum ParameterValue {
    Real(f64),
    Integer(i64),
    String(std::string::String),
    Ast(cvaf::ast::Expr),   // NEW — raw AST, only for ExternParameterKind::Expr params
}
```

---

## 2. `NetResolver` — Hierarchical-to-Flat Net Name Resolution

### 2.1 Trait Definition

**File:** `crates/piperine-circuit/src/hardware.rs` (add below imports)

```rust
/// Resolves hierarchical Piperine net names to flat SPICE net names.
///
/// Built by the elaborator from the current `NetMap` + hierarchy `path`.
/// Passed to `HardwareDefinition::instantiate()` so plugins can resolve
/// net references found inside `parameter expr` AST nodes.
///
/// Examples (assuming path = "X1"):
///   "X1.mid"  → "X1_mid"   (sub-module internal net)
///   "gnd"     → "0"         (canonical ground)
///   "out"     → "out"       (top-level net, no mangling)
///   "vdd"     → "vdd"       (global power net)
pub trait NetResolver: Send + Sync {
    fn resolve(&self, hierarchical_net: &str) -> String;
}
```

### 2.2 Concrete Implementation

**File:** `crates/piperine-circuit/src/elaboration.rs`

```rust
struct ConcreteNetResolver<'a> {
    net_map: &'a NetMap,
    path: &'a str,
}

impl<'a> NetResolver for ConcreteNetResolver<'a> {
    fn resolve(&self, raw: &str) -> String {
        resolve_net(raw, self.net_map, self.path)
    }
}
```

The `resolve_net()` function already exists in the elaborator (handles `gnd` → `"0"`,
map lookup, and `mangle_net` fallback). The resolver just wraps it.

### 2.3 `HardwareDefinition::instantiate()` Signature Change

**File:** `crates/piperine-circuit/src/hardware.rs`

```rust
pub trait HardwareDefinition: fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn ports(&self) -> &[PortDefinition];
    fn parameters(&self) -> &[ParameterDefinition];

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
        resolver: &dyn NetResolver,    // NEW — was not here before
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError>;
}
```

**Migration:** All existing `HardwareDefinition` impls (in
`crates/piperine-ngspice/src/hardware.rs` and
`crates/piperine-openvaf/src/osdi_hardware.rs`) must add `resolver: &dyn NetResolver`
to their `instantiate()` signature. They can ignore it if they have no `Expr` params:
```rust
fn instantiate(&self, name: &str, params: &ParameterMap, conns: &ConnectionMap, _resolver: &dyn NetResolver)
    -> Result<Box<dyn HardwareInstance>, ElaborationError> { ... }
```

### 2.4 Elaborator Call Site

**File:** `crates/piperine-circuit/src/elaboration.rs`, `elaborate_instance()`:

```rust
// Build resolver for this call site
let resolver = ConcreteNetResolver { net_map, path };

if let Some(definition) = registry.get(&instance.module) {
    let parameters = resolve_parameters(&instance.params, &instance.name, definition.parameters())?;
    let hw_instance = definition.instantiate(&instance.name, &parameters, &connections, &resolver)?;
    spice_lines.extend(hw_instance.spice_lines());
}
```

---

## 3. B-Source — AST Serializer in ngspice Plugin

### 3.1 Declaration

**File:** `crates/piperine-ngspice/src/hardware.rs`

```rust
// In piperine source (header file concept — users write this):
//
//   extern module spice_bsource_v(inout p, inout n; parameter expr V);
//   extern module spice_bsource_i(inout p, inout n; parameter expr I);
//   extern module spice_bsource_vi(inout p, inout n; parameter expr V; parameter expr I);

pub struct SpiceBSourceV;

impl HardwareDefinition for SpiceBSourceV {
    fn name(&self) -> &str { "spice_bsource_v" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }  // Expr kind, no default

    fn instantiate(
        &self,
        name: &str,
        params: &ParameterMap,
        conns: &ConnectionMap,
        resolver: &dyn NetResolver,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(conns, "p", name)?.to_string();
        let n = require_net(conns, "n", name)?.to_string();
        let v_ast = params.get("V")
            .and_then(|v| if let ParameterValue::Ast(e) = v { Some(e) } else { None })
            .ok_or_else(|| ElaborationError::MissingParameter { parameter: "V".into(), instance: name.into() })?;
        let v_expr = serialize_ngspice_expr(v_ast, resolver)
            .map_err(|e| ElaborationError::TypeError { parameter: "V".into(), detail: e })?;
        Ok(Box::new(SpiceBSourceVInstance {
            name: name.to_string(), p, n, v_expr,
        }))
    }
}

struct SpiceBSourceVInstance { name: String, p: String, n: String, v_expr: String }

impl HardwareInstance for SpiceBSourceVInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("B{} {} {} V={}", self.name, self.p, self.n, self.v_expr)]
    }
}
```

### 3.2 AST → ngspice Expression Serializer

**File:** `crates/piperine-ngspice/src/expr_serializer.rs` (new file)

```rust
use cvaf::ast::{Expr, FunctionRef, Literal, BinOp, PrefixOp, PathSegment};
use piperine_circuit::NetResolver;

/// Convert a Piperine AST expression to an ngspice B-source expression string.
///
/// Design: recursive descent over Expr. Analog access functions V(), I() resolve
/// their net argument through the NetResolver. Unsupported constructs return Err.
pub fn serialize_ngspice_expr(expr: &Expr, r: &dyn NetResolver) -> Result<String, String> {
    match expr {
        Expr::Literal(lit) => serialize_literal(lit),

        Expr::Prefix(PrefixOp::Neg, inner) =>
            Ok(format!("-({})", serialize_ngspice_expr(inner, r)?)),
        Expr::Prefix(PrefixOp::Pos, inner) =>
            serialize_ngspice_expr(inner, r),
        Expr::Prefix(op, _) =>
            Err(format!("unsupported prefix op {:?} in B-source expression", op)),

        Expr::Binary(lhs, op, rhs) => {
            let l = serialize_ngspice_expr(lhs, r)?;
            let ro = serialize_ngspice_expr(rhs, r)?;
            let op_str = serialize_binop(op)?;
            Ok(format!("({l}{op_str}{ro})"))
        }

        Expr::Paren(inner) =>
            Ok(format!("({})", serialize_ngspice_expr(inner, r)?)),

        Expr::Select(cond, then, els) => {
            let c = serialize_ngspice_expr(cond, r)?;
            let t = serialize_ngspice_expr(then, r)?;
            let e = serialize_ngspice_expr(els, r)?;
            Ok(format!("({c})?({t}):({e})"))
        }

        // V(net) or V(net1, net2) — analog voltage access
        Expr::Call(FunctionRef::Path(p), args) if path_is("V", p) => {
            match args.len() {
                1 => {
                    let net = extract_net_path(&args[0])?;
                    Ok(format!("v({})", r.resolve(&net)))
                }
                2 => {
                    let n1 = extract_net_path(&args[0])?;
                    let n2 = extract_net_path(&args[1])?;
                    Ok(format!("v({},{})", r.resolve(&n1), r.resolve(&n2)))
                }
                _ => Err("V() takes 1 or 2 net arguments".into()),
            }
        }

        // I(branch) — branch current access
        Expr::Call(FunctionRef::Path(p), args) if path_is("I", p) => {
            let branch = extract_net_path(&args[0])?;
            Ok(format!("i({})", r.resolve(&branch)))
        }

        // ddt(expr) — time derivative
        Expr::Call(FunctionRef::Path(p), args) if path_is("ddt", p) => {
            let inner = serialize_ngspice_expr(&args[0], r)?;
            Ok(format!("ddt({})", inner))
        }

        // idt(expr, ic) — time integral
        Expr::Call(FunctionRef::Path(p), args) if path_is("idt", p) => {
            let inner = serialize_ngspice_expr(&args[0], r)?;
            let ic = if args.len() > 1 { serialize_ngspice_expr(&args[1], r)? } else { "0".into() };
            Ok(format!("idt({},{})", inner, ic))
        }

        // Math functions: abs sqrt exp ln log sin cos tan ...
        Expr::Call(FunctionRef::Path(p), args) => {
            let fname = path_leaf(p);
            let ng_name = match fname.as_str() {
                "abs"   => "abs",   "sqrt"  => "sqrt",
                "exp"   => "exp",   "ln"    => "ln",
                "log"   => "log",   "log10" => "log",
                "sin"   => "sin",   "cos"   => "cos",
                "tan"   => "tan",   "asin"  => "asin",
                "acos"  => "acos",  "atan"  => "atan",
                "atan2" => "atan2", "pow"   => "pow",
                "floor" => "floor", "ceil"  => "ceil",
                other   => return Err(format!("unknown function `{other}` in B-source expression")),
            };
            let arg_strs: Result<Vec<_>, _> = args.iter().map(|a| serialize_ngspice_expr(a, r)).collect();
            Ok(format!("{}({})", ng_name, arg_strs?.join(",")))
        }

        // $time, $temper — predefined simulator variables
        Expr::Call(FunctionRef::SysFun(name), _) if name == "time" => Ok("time".into()),
        Expr::Call(FunctionRef::SysFun(name), _) if name == "temper" => Ok("temper".into()),

        // Plain identifier — local variable reference, pass through
        Expr::Path(p) if p.qualifier.is_none() => Ok(path_leaf(p)),

        other => Err(format!("unsupported expression {:?} in B-source context", other)),
    }
}

fn serialize_literal(lit: &Literal) -> Result<String, String> {
    match lit {
        Literal::IntNumber(s) | Literal::StdRealNumber(s) | Literal::SiRealNumber(s) => {
            // ngspice understands SI suffixes too, so pass through
            Ok(s.clone())
        }
        Literal::Inf => Ok("1e308".into()),
        Literal::StrLit(_) => Err("string literal not valid in B-source expression".into()),
    }
}

fn serialize_binop(op: &BinOp) -> Result<&'static str, String> {
    Ok(match op {
        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
        BinOp::Div => "/", BinOp::Pow => "**",
        BinOp::Eq  => "==", BinOp::Neq => "!=",
        BinOp::Lt  => "<",  BinOp::Le  => "<=",
        BinOp::Gt  => ">",  BinOp::Ge  => ">=",
        BinOp::AndAnd => "&&", BinOp::OrOr => "||",
        other => return Err(format!("unsupported operator {:?} in B-source expression", other)),
    })
}

/// Extract the flat dot-joined string from a hierarchical Path.
/// Path { qualifier: Some(Path{Ident("X1")}), segment: Ident("mid") } → "X1.mid"
fn extract_net_path(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::Path(p) => Ok(flatten_path(p)),
        other => Err(format!("expected net name (identifier path), got {:?}", other)),
    }
}

fn flatten_path(p: &cvaf::ast::Path) -> String {
    let seg = match &p.segment { PathSegment::Ident(s) => s.as_str(), PathSegment::Root => "root" };
    match &p.qualifier {
        Some(q) => format!("{}.{}", flatten_path(q), seg),
        None    => seg.to_string(),
    }
}

fn path_leaf(p: &cvaf::ast::Path) -> String {
    match &p.segment { PathSegment::Ident(s) => s.clone(), PathSegment::Root => "root".into() }
}

fn path_is(name: &str, p: &cvaf::ast::Path) -> bool {
    p.qualifier.is_none() && matches!(&p.segment, PathSegment::Ident(s) if s == name)
}
```

### 3.3 Usage Example (Piperine source)

```verilog
// Square-law detector
spice_bsource_v #(.V( V(X1.in) * V(X1.in) / 1e3 )) B_sq (.p(sq_out), .n(gnd));

// Full-wave rectifier (absolute value of differential voltage)
spice_bsource_v #(.V( abs(V(vp, vn)) )) B_rect (.p(rect_out), .n(gnd));

// Voltage-controlled current source with nonlinear gm
spice_bsource_i #(.I( 2e-3 * V(gate, source) + 1e-4 * V(gate, source) ** 3 )) B_gm (.p(drain), .n(source));

// Behavioral integrator (RC equivalent)
spice_bsource_v #(.V( idt(V(inp) / (1e3 * 100e-12), 0) )) B_integ (.p(out), .n(gnd));
```

Emitted ngspice lines:
```spice
B_sq  sq_out  0  V=(v(X1_in)*v(X1_in)/1e3)
B_rect rect_out 0  V=abs(v(vp,vn))
B_gm  drain source I=(2e-3*v(gate,source)+1e-4*v(gate,source)**3)
B_integ out 0  V=idt(v(inp)/(1e3*100e-12),0)
```

---

## 4. Two-Way Communication — `SimulatorContext` + `EventSink`

### 4.1 Rationale

Current flow (wrong for Phase 2):
```
Interpreter → $tran() → SimulatorBackend::run_command("op") → Response::Ok
$tran() returns None
```

Needed flow:
```
Interpreter → $tran() → starts analysis
  Worker → Response::Event { Step, t=1e-9 }   (per-step callback fires)
  Interpreter runs always @(step) body
  Interpreter → Command::EventResponse { Continue }
  Worker → Response::Event { Step, t=2e-9 }
  ...
  Worker → Response::AnalysisDone { plot="tran1" }
$tran() pulls all vectors → returns Value::AnalysisHandle(Arc<AnalysisResult>)
```

### 4.2 IPC Protocol Changes

**File:** `crates/piperine-common/src/lib.rs`

```rust
#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    // ── existing ──
    Run { cmd: String },          // kept for non-analysis commands (op, set, etc.)
    LoadCircuit { lines: Vec<String> },
    GetVecData { name: String },
    GetAllVecs { plot: String },
    GetCurPlot,
    GetAllPlots,
    Shutdown,

    // ── NEW ──
    /// Start an analysis that will stream Event responses.
    /// Worker fires Response::Event for each @(step) etc., then Response::AnalysisDone.
    RunAnalysis { cmd: String, fire_step_events: bool },

    /// Coordinator's response to a Response::Event from the worker.
    /// Sent synchronously — worker blocks on this before continuing the callback.
    EventResponse { action: EventAction },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EventAction {
    Continue,
    Halt { reason: String },
    RunError { message: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    // ── existing ──
    Ok,
    Error { code: i32, message: String },
    VecData { values: Vec<f64> },
    VecList { names: Vec<String> },
    CurPlot { name: String },

    // ── NEW ──
    /// Worker fires this for each @(step)/@(initial_step)/@(final_step)/above() crossing.
    /// Coordinator must reply with Command::EventResponse before worker continues.
    Event { kind: SimEventKind, time: f64, crossing_id: u32 },

    /// Analysis complete. All vectors now readable via GetVecData.
    AnalysisDone { plot_name: String, had_run_errors: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SimEventKind {
    InitialStep,
    Step,
    FinalStep,
    AboveCrossing,   // above(expr) threshold crossed positive→negative
}
```

### 4.3 Worker-Side Changes

**File:** `crates/piperine-worker/src/ngspice.rs`

Add to `NgspiceHandler`:
```rust
pub trait NgspiceHandler: Send {
    fn on_step(&self, _time: f64)        {}
    fn on_initial_step(&self, _time: f64) {}
    fn on_final_step(&self, _time: f64)  {}
}
```

**File:** `crates/piperine-worker/src/main.rs`

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Shared flag: coordinator told us to halt the current run.
struct WorkerState {
    halt_requested: AtomicBool,
    run_error_message: std::sync::Mutex<Option<String>>,
}

struct WorkerHandler {
    tx: RespSender,
    rx: std::sync::Mutex<CmdReceiver>,   // mutable recv, lock per callback
    state: Arc<WorkerState>,
    fire_step: bool,
}

impl NgspiceHandler for WorkerHandler {
    fn on_initial_step(&self, time: f64) {
        self.send_event_and_wait(SimEventKind::InitialStep, time, 0);
    }

    fn on_step(&self, time: f64) {
        if self.fire_step {
            self.send_event_and_wait(SimEventKind::Step, time, 0);
        }
    }

    fn on_final_step(&self, time: f64) {
        self.send_event_and_wait(SimEventKind::FinalStep, time, 0);
    }
}

impl WorkerHandler {
    fn send_event_and_wait(&self, kind: SimEventKind, time: f64, crossing_id: u32) {
        if self.state.halt_requested.load(Ordering::Relaxed) { return; }
        let _ = self.tx.send(Response::Event { kind, time, crossing_id });
        // Block until coordinator responds
        if let Ok(cmd) = self.rx.lock().unwrap().recv() {
            match cmd {
                Command::EventResponse { action: EventAction::Halt { reason } } => {
                    self.state.halt_requested.store(true, Ordering::Relaxed);
                    // ngSpice_Command("halt") will stop the current analysis
                    // (called from run_command after the step callback returns)
                }
                Command::EventResponse { action: EventAction::RunError { message } } => {
                    *self.state.run_error_message.lock().unwrap() = Some(message);
                    self.state.halt_requested.store(true, Ordering::Relaxed);
                }
                _ => {}  // Continue
            }
        }
    }
}

// In run_command, handle RunAnalysis:
Command::RunAnalysis { cmd: c, fire_step_events } => {
    // Configure handler with fire_step flag + shared halt state
    // ng.command(&c) → triggers ngspice analysis → callbacks fire WorkerHandler methods
    // After ng.command returns:
    let plot = ng.cur_plot().unwrap_or_default();
    let had_errors = handler.state.run_error_message.lock().unwrap().is_some();
    tx.send(Response::AnalysisDone { plot_name: plot, had_run_errors: had_errors })
}
```

**Note on halting ngspice from callback:** The callback sets a flag. When the callback
returns to ngspice, ngspice continues normally. After the full command returns, the
`had_run_errors` flag is set. The coordinator treats the AnalysisResult as a failed run.
True mid-analysis halt (early termination) requires `ngSpice_Command("halt")` called
from the callback thread — this is supported by libngspice and should be tried first.
If it deadlocks (known issue in some versions), fall back to the flag approach.

### 4.4 Coordinator-Side — Analysis Loop

**File:** `crates/piperine-ngspice/src/backend.rs`

```rust
pub struct NgspiceBackend { /* existing worker IPC fields */ }

impl NgspiceBackend {
    /// Run an analysis command, fire registered always-block handlers at each event,
    /// and return a fully-populated AnalysisResult.
    ///
    /// `handlers`: collected from the testbench module's always blocks.
    /// `interp_ctx`: used to execute always-block statement bodies.
    pub fn run_analysis(
        &mut self,
        cmd: &str,
        handlers: &AlwaysHandlerSet,
        interp_ctx: &mut dyn InterpreterCallbacks,
        fire_step: bool,
    ) -> Result<AnalysisResult, InterpreterError> {
        self.send(Command::RunAnalysis { cmd: cmd.to_string(), fire_step_events: fire_step })?;

        let mut had_run_errors = false;
        let mut plot_name = String::new();

        loop {
            match self.recv()? {
                Response::AnalysisDone { plot_name: p, had_run_errors: e } => {
                    plot_name = p;
                    had_run_errors = e;
                    break;
                }
                Response::Event { kind, time, crossing_id } => {
                    let action = interp_ctx.fire_event(kind, time, crossing_id, handlers);
                    self.send(Command::EventResponse { action })?;
                }
                Response::Error { message, .. } => {
                    return Err(InterpreterError::SimulatorError(message));
                }
                _ => {}
            }
        }

        // Pull all vectors after the run
        let vecs = self.recv_all_vecs(&plot_name)?;
        Ok(AnalysisResult { plot_name, vectors: vecs, had_run_errors })
    }

    fn recv_all_vecs(&mut self, plot: &str) -> Result<HashMap<String, VectorData>, InterpreterError> {
        let names = self.get_all_vecs(plot)?;
        let mut map = HashMap::new();
        for name in names {
            let values = self.get_vec_data(&name)?;
            map.insert(name, VectorData::Real(values));
        }
        Ok(map)
    }
}
```

### 4.5 `InterpreterCallbacks` — Interpreter Re-Entry from Backend

**File:** `crates/piperine-interpreter/src/lib.rs` (new trait)

```rust
/// Allows the simulator backend to call back into the interpreter to run
/// always-block statement bodies during an active analysis.
pub trait InterpreterCallbacks: Send {
    fn fire_event(
        &mut self,
        kind: SimEventKind,
        time: f64,
        crossing_id: u32,
        handlers: &AlwaysHandlerSet,
    ) -> EventAction;
}

/// `AlwaysHandlerSet` — collected from all `always @(...)` blocks in the testbench module.
pub struct AlwaysHandlerSet {
    pub initial_step: Vec<Stmt>,
    pub final_step:   Vec<Stmt>,
    pub step:         Vec<Stmt>,
    /// (expr to evaluate for crossing, body stmt)
    pub above:        Vec<(Expr, u32, Stmt)>,  // (threshold_expr, crossing_id, body)
}
```

The `Interpreter` struct implements `InterpreterCallbacks`. When `fire_event(Step, ...)` fires,
it runs all `step` statements in the current scope, collecting any `RunError` results.

### 4.6 `AnalysisResult` and `Value::AnalysisHandle`

**File:** `crates/piperine-interpreter/src/value.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Real(f64),
    Integer(i64),
    String(std::string::String),
    Void,
    RealVec(Vec<f64>),                     // NEW — $get_vec result
    Complex(f64, f64),                     // NEW — AC result (mag, phase)
    AnalysisHandle(Arc<AnalysisResult>),   // NEW — returned by $tran/$ac/$dc
    ExternObject(Arc<dyn ExternClass>),    // NEW — plugin-provided objects
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub kind: AnalysisKind,
    pub plot_name: String,
    pub vectors: HashMap<String, VectorData>,
    pub run_errors: Vec<RunError>,
}

#[derive(Debug, Clone)]
pub enum AnalysisKind { Op, Tran, Ac, Dc, Noise, Tf, Pz, Sens }

#[derive(Debug, Clone)]
pub enum VectorData {
    Real(Vec<f64>),
    Complex(Vec<(f64, f64)>),   // (real, imag) pairs
}

#[derive(Debug, Clone)]
pub struct RunError {
    pub message: String,
    pub time: Option<f64>,
    pub kind: RunErrorKind,
}

#[derive(Debug, Clone)]
pub enum RunErrorKind { SoaViolation, UserAssert, SimulatorError }
```

---

## 5. `always` Blocks — Parser and Interpreter

### 5.1 AST

**File:** `crates/piperine-parser/src/ast/item.rs`

Add new `ModuleItem` variant and `AlwaysBlock` struct:

```rust
pub enum ModuleItem {
    // ... existing variants ...
    InitialBlock(InitialBlock),
    AlwaysBlock(AlwaysBlock),   // NEW
}

/// `always @(sensitivity) stmt` — testbench event-driven block.
///
/// Sensitivity forms:
///   @(initial_step)          fires once at start of each analysis
///   @(final_step)            fires once at end of each analysis
///   @(step)                  fires at every accepted timepoint (expensive!)
///   @(above(expr))           fires on positive zero-crossing of expr
///   @(cross(expr, +1))       SV-AMS style crossing (direction: +1/-1/0=both)
#[derive(Debug, Clone)]
pub struct AlwaysBlock {
    pub span: Span,
    pub sensitivity: AlwaysSensitivity,
    pub stmt: Box<Stmt>,
}

#[derive(Debug, Clone)]
pub enum AlwaysSensitivity {
    InitialStep,
    FinalStep,
    Step,
    Above(Expr),          // above(threshold_expr)
    Cross(Expr, i8),      // cross(expr, direction): +1, -1, or 0 for both
}
```

### 5.2 Parser

**File:** `crates/piperine-parser/src/grammar/item.rs`

When parsing `ModuleItem`, after matching keyword `always`, parse `@(sensitivity)`:
```
"initial_step" → AlwaysSensitivity::InitialStep
"final_step"   → AlwaysSensitivity::FinalStep
"step"         → AlwaysSensitivity::Step
"above" "(" expr ")"        → AlwaysSensitivity::Above(expr)
"cross" "(" expr "," int ")" → AlwaysSensitivity::Cross(expr, dir)
```
Then parse `Stmt` as body.

**Note:** `initial_step`, `final_step`, `step`, `above`, `cross` are contextual keywords
inside `@(...)` — not global reserved words. Parser peeks for them only in this context.

### 5.3 Elaborator — Collect Handlers

**File:** `crates/piperine-circuit/src/elaboration.rs`, `elaborate()`:

```rust
// After collecting spice_lines, scan for always blocks
let mut always_handlers = AlwaysHandlerSet::default();
let mut crossing_id = 0u32;

for item in &testbench.items {
    if let ast::ModuleItem::AlwaysBlock(ab) = item {
        match &ab.sensitivity {
            AlwaysSensitivity::InitialStep => always_handlers.initial_step.push(*ab.stmt.clone()),
            AlwaysSensitivity::FinalStep   => always_handlers.final_step.push(*ab.stmt.clone()),
            AlwaysSensitivity::Step        => always_handlers.step.push(*ab.stmt.clone()),
            AlwaysSensitivity::Above(expr) => {
                always_handlers.above.push((expr.clone(), crossing_id, *ab.stmt.clone()));
                crossing_id += 1;
            }
            AlwaysSensitivity::Cross(expr, dir) => {
                always_handlers.cross.push((expr.clone(), *dir, crossing_id, *ab.stmt.clone()));
                crossing_id += 1;
            }
        }
    }
}

Ok(ElaborationResult { spice_lines, initial_statement, always_handlers })
```

`ElaborationResult` gains `pub always_handlers: AlwaysHandlerSet`.

### 5.4 Usage Examples

```verilog
module tb;
    spice_mos #(.model("nmos18"), .l(180e-9), .w(1e-6)) M1 (.d(vd), .g(vg), .s(gnd), .b(gnd));
    spice_vsource #(.dc(1.8)) Vdd (.p(vd), .n(gnd));
    spice_vsource #(.dc(0.9)) Vgs (.p(vg), .n(gnd));

    real vds_max;

    // InitialStep: print run info
    always @(initial_step)
        $display("=== starting analysis ===");

    // Step: SOA monitoring — fires every timepoint
    always @(step) begin
        // assert_run: non-fatal — marks run as failed, continues (see §7)
        assert_run (V(vd) <= 1.95)
            else $run_error("Vds=%.3fV exceeds 1.95V at t=%.2gns", V(vd), $time*1e9);
        if (V(vd) > vds_max) vds_max = V(vd);
    end

    // Above: fires ONCE when V(vd) crosses 1.8V going positive
    always @(above(V(vd) - 1.8))
        $warning("Vds exceeded nominal 1.8V");

    // FinalStep: post-analysis summary
    always @(final_step)
        $display("peak Vds = %.3fV", vds_max);

    initial begin
        vds_max = 0.0;
        $tran(1n, 100n);
    end
endmodule
```

---

## 6. `enum`, `typedef struct`, `extern class`

### 6.1 `typedef enum` — Parser

**File:** `crates/piperine-parser/src/ast/item.rs`

New top-level items:
```rust
pub enum Item {
    DisciplineDecl(DisciplineDecl),
    NatureDecl(NatureDecl),
    ModuleDecl(ModuleDecl),
    ExternModule(ExternModuleDecl),
    TypedefEnum(TypedefEnum),    // NEW
    TypedefStruct(TypedefStruct), // NEW
    ExternClass(ExternClassDecl), // NEW
}

/// `typedef enum [base_type] { A [= val], B, ... } TypeName;`
#[derive(Debug, Clone)]
pub struct TypedefEnum {
    pub name: Name,
    pub base_type: Option<Type>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Name,
    pub value: Option<Expr>,   // explicit value e.g. LOW = 0
}

/// `typedef struct { field_decl* } TypeName;`
#[derive(Debug, Clone)]
pub struct TypedefStruct {
    pub name: Name,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub ty: Type,
    pub name: Name,
}

/// `extern class TypeName;` — opaque type backed by a plugin's Rust implementation.
#[derive(Debug, Clone)]
pub struct ExternClassDecl {
    pub name: Name,
}
```

### 6.2 `enum` Interpreter Values

**File:** `crates/piperine-interpreter/src/value.rs`

```rust
pub enum Value {
    // ... existing ...
    Enum { type_id: u32, variant: u32 },          // typedef enum instance
    Struct { type_id: u32, fields: HashMap<String, Value> }, // typedef struct instance
    ExternObject(Arc<dyn ExternClass>),
}
```

Interpreter maintains a `TypeRegistry` (map from type name → type_id):
```rust
pub struct TypeRegistry {
    enums:   HashMap<String, EnumTypeDef>,    // name → (type_id, variants)
    structs: HashMap<String, StructTypeDef>,  // name → (type_id, fields)
}

pub struct EnumTypeDef {
    pub type_id: u32,
    pub variants: Vec<(String, i64)>,   // (variant_name, value)
}
```

Enum assignment: `Corner c = TT;` → interpreter looks up `TT` in current `TypeRegistry`,
finds it's `EnumTypeDef { type_id: 1, variant: 2 }`, stores `Value::Enum { type_id: 1, variant: 2 }`.

`case(c)` discrimination: compare `type_id` + `variant` against each arm.

### 6.3 `extern class` and `ExternClass` Trait

**File:** `crates/piperine-interpreter/src/value.rs`

```rust
/// A Rust-backed type exposed to Piperine code.
///
/// Implement this in a plugin to provide rich result objects like TranResult,
/// AcResult, Signal. Method calls from Piperine dispatch here.
///
/// Example — TranResult in piperine-ngspice:
///
///   struct PiperineTranResult(Arc<AnalysisResult>);
///
///   impl ExternClass for PiperineTranResult {
///       fn type_name(&self) -> &str { "TranResult" }
///       fn call_method(&self, method: &str, args: Vec<Value>)
///           -> Result<Option<Value>, InterpreterError>
///       {
///           match method {
///               "max"   => Ok(Some(Value::Real(self.0.max_of(args[0].as_str()?)))),
///               "min"   => Ok(Some(Value::Real(self.0.min_of(args[0].as_str()?)))),
///               "rms"   => Ok(Some(Value::Real(self.0.rms_of(args[0].as_str()?)))),
///               "signal"=> Ok(Some(Value::ExternObject(Arc::new(PiperineSignal(...))))),
///               "has_errors"  => Ok(Some(Value::Integer(!self.0.run_errors.is_empty() as i64))),
///               "first_error" => Ok(Some(Value::String(...))),
///               _ => Err(InterpreterError::TypeError { ... }),
///           }
///       }
///       fn get_field(&self, _field: &str) -> Option<Value> { None }
///   }
pub trait ExternClass: Send + Sync + std::fmt::Debug {
    fn type_name(&self) -> &str;

    fn call_method(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Option<Value>, InterpreterError>;

    /// Optional field access — `obj.field` (read-only)
    fn get_field(&self, _field: &str) -> Option<Value> { None }
}
```

**Plugin registration:**

**File:** `crates/piperine-interpreter/src/plugin.rs`

```rust
pub trait Plugin: Send + Sync {
    // ... existing ...
    fn register_extern_classes(&self, _registry: &mut ExternClassRegistry) {}
}

/// Maps type names from `extern class Foo;` declarations to their Rust implementations.
/// The interpreter uses this when it sees a dot method call on an ExternObject.
#[derive(Default)]
pub struct ExternClassRegistry {
    // type_name → factory function (creates instances from Value::AnalysisHandle etc.)
    constructors: HashMap<String, Box<dyn Fn(Value) -> Arc<dyn ExternClass>>>,
}
```

**Method call dispatch in interpreter:**

**File:** `crates/piperine-interpreter/src/interpreter.rs`

When evaluating `expr.method(args)`:
```rust
if let Value::ExternObject(obj) = self.eval_expr(receiver, scope)? {
    let arg_vals: Vec<Value> = args.iter().map(|a| self.eval_expr(a, scope)).collect::<Result<_,_>>()?;
    return obj.call_method(method_name, arg_vals);
}
```

### 6.4 Usage Example

```verilog
typedef enum { TT, SS, FF, SF, FS } Corner;

typedef struct {
    real vdd;
    real temp;
    Corner corner;
} SimConfig;

extern class TranResult;   // backed by PiperineTranResult in piperine-ngspice

module tb;
    spice_mos #(.model("nmos18")) M1 (.d(vd), .g(vg), .s(gnd), .b(gnd));
    spice_vsource #(.dc(1.8)) Vdd (.p(vd), .n(gnd));

    initial begin
        SimConfig configs[3];
        configs[0] = '{vdd: 1.8, temp: 27.0, corner: TT};
        configs[1] = '{vdd: 1.6, temp: 85.0, corner: SS};
        configs[2] = '{vdd: 2.0, temp: -40.0, corner: FF};

        for (int i = 0; i < 3; i++) begin
            $alter("Vdd.dc", configs[i].vdd);
            $set_temp(configs[i].temp);

            TranResult t = $tran(1n, 100n);

            if (t.has_errors()) begin
                $warning("corner %0d failed: %s", i, t.first_error());
            end else begin
                $display("corner %0d: peak_vd=%.3f rms=%.4f",
                    i, t.max("v(vd)"), t.rms("v(vd)"));
            end
        end
    end
endmodule
```

---

## 7. Assertions — Panic vs Run Error

### 7.1 Design Rationale

Three distinct failure modes:

| Situation | Desired behavior |
|-----------|-----------------|
| Logic bug in testbench (null pointer equivalent) | Process panic — stop everything, show backtrace |
| SOA violation in one MC run | Mark run as failed, continue outer loop |
| Marginal condition worth noting | Log warning, never halt |

Without this split, a 1000-run Monte Carlo can never complete if any single run
violates an SOA limit. The optimizer gets no data. This is wrong.

### 7.2 New AST Nodes

**File:** `crates/piperine-parser/src/ast/stmt.rs`

```rust
pub enum Stmt {
    // ... existing ...
    Assert(AssertStmt),      // existing — panic on failure
    AssertRun(AssertStmt),   // NEW — run error on failure, continues outer loop
    AssertWarn(AssertStmt),  // NEW — warning on failure, never halts
}

#[derive(Debug, Clone)]
pub struct AssertStmt {
    pub attrs: Vec<Attr>,
    pub condition: Expr,
    pub message: Option<Expr>,   // optional else $fatal/$run_error/"msg" clause
}
```

### 7.3 `InterpreterError` Changes

**File:** `crates/piperine-interpreter/src/error.rs`

```rust
#[derive(Debug, Clone)]
pub enum InterpreterError {
    // ── existing ──
    TypeError { expected: String, got: String },
    UndefinedVariable(String),
    SimulatorError(String),

    // ── NEW ──
    /// Fatal — propagates all the way up, process exits.
    /// Corresponds to `assert` failure, `$fatal`, or unrecoverable simulator error.
    Fatal { message: String, exit_code: u32 },

    /// Non-fatal run error — marks the current analysis run as failed.
    /// Caught at the `$tran()`/`$ac()` call site, stored in AnalysisResult.
    /// The outer `initial` block continues normally.
    RunFailed { message: String },
}
```

### 7.4 Interpreter Handling

**File:** `crates/piperine-interpreter/src/interpreter.rs`

```rust
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
        self.sim.print(&format!("WARNING: {msg}"));  // never halts
    }
}
```

**`$run_error` system task** (usable anywhere, not just assert):
```rust
pub struct RunErrorTask;
impl SystemTask for RunErrorTask {
    fn name(&self) -> &str { "run_error" }
    fn call(&self, args: Vec<Value>, _sim: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = format_display_string(args[0].as_str().unwrap_or("run failed"), &args[1..]);
        Err(InterpreterError::RunFailed { message: msg })
    }
}
```

**`$tran()` catch site** — catches `RunFailed`, stores in result:
```rust
pub struct TransientTask;
impl SystemTask for TransientTask {
    fn name(&self) -> &str { "tran" }
    fn call(&self, args: Vec<Value>, sim: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        // ... parse tstep, tstop from args ...
        let result = match sim.run_analysis(&cmd, &handlers, interp_callbacks, fire_step) {
            Ok(r) => r,
            Err(InterpreterError::RunFailed { message }) => {
                // Non-fatal: return AnalysisResult with the error recorded
                AnalysisResult { run_errors: vec![RunError { message, .. }], .. }
            }
            Err(e) => return Err(e),  // Fatal or simulator error: propagate
        };
        Ok(Some(Value::AnalysisHandle(Arc::new(result))))
    }
}
```

`InterpreterError::Fatal` is **never caught** by system tasks — it propagates to `main.rs`.

**`@(step)` handler wrapper** — inside `fire_event()` in the interpreter:
```rust
fn fire_event(&mut self, kind: SimEventKind, time: f64, ...) -> EventAction {
    for stmt in &handlers.step {
        match self.exec(stmt, &mut scope) {
            Ok(_) => {}
            Err(InterpreterError::RunFailed { message }) =>
                return EventAction::RunError { message },
            Err(InterpreterError::Fatal { message, .. }) =>
                return EventAction::Halt { reason: message },
            Err(e) =>
                return EventAction::Halt { reason: e.to_string() },
        }
    }
    EventAction::Continue
}
```

### 7.5 Usage Example

```verilog
initial begin
    TranResult results[$];

    for (int mc = 0; mc < 200; mc++) begin
        real r_val = $normal(1e3, 50.0);
        $alter("R1.r", r_val);
        TranResult t = $tran(1n, 100n);

        if (t.has_errors())
            $display("MC[%0d] failed: %s — skipping", mc, t.first_error());
        else
            results.push_back(t);
    end

    $display("Completed %0d/%0d runs without SOA violations", results.size(), 200);
end

// In always block — uses assert_run so one violation doesn't kill 200 runs
always @(step) begin
    // Non-fatal — marks current $tran run as failed, outer loop continues
    assert_run (V(vd) <= 2.0)
        else $run_error("Vds %.3fV exceeds 2.0V at t=%.2gns", V(vd), $time*1e9);

    // Fatal panic — should NEVER happen (testbench logic error)
    assert ($time >= 0.0) else $fatal(1, "time is negative — simulator bug");
end
```

---

## 8. Revised `SystemTask` Trait

**File:** `crates/piperine-interpreter/src/task.rs`

Current signature:
```rust
fn call(&self, args: Vec<Value>, sim: &mut dyn SimulatorBackend)
    -> Result<Option<Value>, InterpreterError>
```

Phase 2 — add context parameters:
```rust
fn call(
    &self,
    args: Vec<Value>,
    sim: &mut dyn SimulatorContext,              // two-way, replaces SimulatorBackend
    handlers: &AlwaysHandlerSet,                  // always blocks for wiring events
    interp: &mut dyn InterpreterCallbacks,        // re-entry for event dispatch
    resolver: &dyn NetResolver,                   // hierarchical name resolution
) -> Result<Option<Value>, InterpreterError>;
```

**Migration plan:** `SimulatorContext` is a supertrait of `SimulatorBackend` — all existing
methods stay. Existing tasks add the new params with `_` if unused:
```rust
fn call(&self, args: Vec<Value>, sim: &mut dyn SimulatorContext, _h: &AlwaysHandlerSet, _i: &mut dyn InterpreterCallbacks, _r: &dyn NetResolver)
    -> Result<Option<Value>, InterpreterError>
{ /* existing impl unchanged */ }
```

---

## 9. Implementation Order

Work proceeds in this dependency order — each step unblocks the next:

```
Step 1: NetResolver trait + ConcreteNetResolver
        Files: piperine-circuit/src/hardware.rs, elaboration.rs
        Test: unit test resolve("X1.mid") → "X1_mid", resolve("gnd") → "0"
        Unblocks: Steps 2, 3

Step 2: ExternParameter::Expr kind + ParameterValue::Ast variant
        Files: piperine-parser/src/ast/item.rs, piperine-circuit/src/types.rs,
               piperine-circuit/src/elaboration.rs (resolve_parameters)
        Test: parse `parameter expr V` → ExternParameterKind::Expr;
               elaborator passes Ast(expr) to instantiate()
        Unblocks: Step 3

Step 3: expr_serializer.rs + SpiceBSourceV/I hardware definitions
        Files: piperine-ngspice/src/expr_serializer.rs (new),
               piperine-ngspice/src/hardware.rs
        Test: serialize V(X1.mid) * V(X1.mid) → "v(X1_mid)*v(X1_mid)"
        Unblocks: B-source in any testbench

Step 4: IPC protocol extensions (RunAnalysis, EventResponse, Event, AnalysisDone)
        Files: piperine-common/src/lib.rs
        Note: non-breaking — existing Command/Response variants stay
        Unblocks: Steps 5, 6

Step 5: Worker streaming handler (WorkerState, send_event_and_wait)
        Files: piperine-worker/src/main.rs, piperine-worker/src/ngspice.rs
        Test: run tran with a handler that fires on_step, verify Event responses arrive
        Unblocks: Step 6

Step 6: AnalysisResult + Value::AnalysisHandle + run_analysis loop in backend
        Files: piperine-interpreter/src/value.rs, piperine-ngspice/src/backend.rs
        Test: $tran returns AnalysisHandle with populated vectors
        Unblocks: Steps 7, 8

Step 7: AlwaysBlock AST + parser + elaborator collection
        Files: piperine-parser/src/ast/item.rs, grammar/item.rs,
               piperine-circuit/src/elaboration.rs
        Test: parse `always @(step) stmt` → AlwaysBlock in module items
        Unblocks: Step 8

Step 8: InterpreterCallbacks::fire_event + @(step) dispatch in run_analysis
        Files: piperine-interpreter/src/interpreter.rs, piperine-ngspice/src/backend.rs
        Test: always @(step) body runs with correct $time; $run_error marks result
        Unblocks: Step 9

Step 9: assert/assert_run/assert_warn AST + interpreter handling
        Files: piperine-parser/src/ast/stmt.rs, grammar/stmt.rs,
               piperine-interpreter/src/interpreter.rs, error.rs
        Test: assert_run failure → AnalysisResult.has_errors() true, outer loop continues

Step 10: typedef enum + typedef struct + interpreter TypeRegistry
         Files: piperine-parser/src/ast/item.rs, grammar/item.rs,
                piperine-interpreter/src/interpreter.rs, value.rs
         Test: typedef enum { TT, SS } Corner; Corner c = TT; case(c) works

Step 11: extern class + ExternClass trait + ExternClassRegistry + method dispatch
         Files: piperine-interpreter/src/value.rs, plugin.rs,
                piperine-ngspice/src/tasks.rs (TranResult impl)
         Test: TranResult t = $tran(...); t.max("v(out)") returns correct value
```

---

## 10. What Does NOT Change

The following are **stable** — do not modify them during Phase 2:

- `cvaf` parser core (Verilog-A grammar, analog blocks, expressions) — additive only
- OpenVAF / OSDI compilation pipeline (`piperine-openvaf`) — untouched
- `HardwareRegistry` / `extern module` model — `instantiate()` gains `resolver`, nothing else changes
- Worker process model (one ngspice per process, IPC) — protocol extends, not replaces
- `NetMap` + hierarchical flattening in elaborator — `NetResolver` wraps it, doesn't replace it
- `SpiceResistor`, `SpiceVoltageSource`, `SpiceCurrentSource`, `SpiceCapacitor` — add `_resolver` param, otherwise unchanged

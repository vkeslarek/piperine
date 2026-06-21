# Piperine — Initial Development Roadmap

**Purpose:** step-by-step instructions for implementing the first working Piperine program.
Every file path, trait signature, and design decision is spelled out — no open questions.

---

## 0. Target program and expected output

The goal of this entire document is to make this program run:

```verilog
// examples/voltage_divider.ppr

extern module spice_vsource(inout p, inout n; parameter real val = 0.0);
extern module spice_res    (inout p, inout n; parameter real r   = 1e3);

module tb;
  spice_vsource #(.val(5.0)) V1 (.p(vin), .n(gnd));
  spice_res     #(.r(1e3))   R1 (.p(vin), .n(vmid));
  spice_res     #(.r(1e3))   R2 (.p(vmid), .n(gnd));

  initial begin
    $op();
    $display("Vmid = %g V", $V("vmid"));
  end
endmodule
```

Expected output:
```
Vmid = 2.5 V
```

No OpenVAF / OSDI needed. Full pipeline:
`parse .ppr → elaborate netlist → load into ngspice worker → interpret initial block`.

OpenVAF/OSDI support is described in Section 9 (deferred — implement Sections 1–8 first).

---

## 1. File extension and parser scope

**Extension: `.ppr`** (Piperine PRogram). No other extensions.

A `.ppr` file may contain:
- `extern module` declarations at top level
- `module` declarations (pure-VA or testbench)
- `nature` and `discipline` declarations (already supported by parser)

Module routing (VA → OpenVAF, testbench → interpreter) is decided at elaboration time
based on module content — not file type. One file, no cross-file confusion.

---

## 2. Crate layout — what to create

Add three new crates. Update `[workspace] members` in root `Cargo.toml`.

```
crates/
  piperine-common/      ← IPC protocol types (EXISTING — do not touch)
  piperine-worker/      ← ngspice process wrapper (EXISTING — do not touch)
  piperine-coordinator/ ← worker process pool (EXISTING — add acquire() method)
  piperine-parser/      ← VA + .ppr parser (EXISTING — extend in Section 3)
  piperine-circuit/     ← NEW: HardwareDefinition/HardwareInstance traits, HardwareRegistry, elaboration
  piperine-interpreter/ ← NEW: Value, SystemTask trait, SimulatorBackend trait, Plugin trait, Interpreter
  piperine-ngspice/     ← NEW: NgspicePlugin — registers spice_res/spice_vsource and ngspice backend
```

**Why three crates, why these names:**

- `piperine-circuit` owns everything about what a circuit *is*: hardware element definitions,
  how they connect, how they become a SPICE netlist. Name says it plainly.
- `piperine-interpreter` owns everything about *running* procedural code: values, system tasks,
  the interpreter loop, and the `SimulatorBackend` trait the interpreter calls into.
- `piperine-ngspice` is the ngspice **plugin**: one self-contained unit that registers all
  ngspice-backed hardware definitions and system tasks with the runtime. Swapping ngspice for
  Xyce means writing a `piperine-xyce` crate with the same shape.

**Dependency graph (no cycles):**

```
piperine-common
    ↑
piperine-worker         piperine-parser
    ↑                       ↑
piperine-coordinator    piperine-circuit
                            ↑
                        piperine-interpreter
                            ↑
                        piperine-ngspice
                            ↑
                        piperine (binary)
```

### 2.1 `crates/piperine-circuit/Cargo.toml`

```toml
[package]
name = "piperine-circuit"
version.workspace = true
edition.workspace = true

[dependencies]
piperine-parser = { path = "../piperine-parser" }
thiserror       = { workspace = true }
```

### 2.2 `crates/piperine-interpreter/Cargo.toml`

```toml
[package]
name = "piperine-interpreter"
version.workspace = true
edition.workspace = true

[dependencies]
piperine-parser  = { path = "../piperine-parser" }
piperine-circuit = { path = "../piperine-circuit" }
thiserror        = { workspace = true }
```

### 2.3 `crates/piperine-ngspice/Cargo.toml`

```toml
[package]
name = "piperine-ngspice"
version.workspace = true
edition.workspace = true

[dependencies]
piperine-interpreter = { path = "../piperine-interpreter" }
piperine-circuit     = { path = "../piperine-circuit" }
piperine-coordinator = { path = "../piperine-coordinator" }
piperine-common      = { path = "../piperine-common" }
thiserror            = { workspace = true }
```

### 2.4 Root binary `Cargo.toml` additions

```toml
[dependencies]
# add:
piperine-circuit     = { path = "crates/piperine-circuit" }
piperine-interpreter = { path = "crates/piperine-interpreter" }
piperine-ngspice     = { path = "crates/piperine-ngspice" }
```

---

## 3. Parser extension — new AST nodes

Two additions to `crates/piperine-parser/`:

1. `Item::ExternModule(ExternModuleDecl)` — top-level extern declaration
2. `ModuleItem::InitialBlock(InitialBlock)` — `initial begin ... end` inside a module

Everything else the first program needs already exists in the AST.

### 3.1 `crates/piperine-parser/src/ast/item.rs` — new types

Add at the end of the file:

```rust
/// Top-level `extern module name(ports; parameters);` declaration.
/// Ports and parameters separated by `;` (or mixed with `,` — parser accepts both).
#[derive(Debug, Clone)]
pub struct ExternModuleDecl {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub name: Name,
    pub ports: Vec<PortDecl>,
    pub parameters: Vec<ExternParameter>,
}

/// One parameter in an `extern module` declaration.
#[derive(Debug, Clone)]
pub struct ExternParameter {
    pub name: Name,
    pub ty: Type,
    pub default: Option<Expr>,
}

/// `initial begin BlockItem* end` — testbench procedural block.
#[derive(Debug, Clone)]
pub struct InitialBlock {
    pub span: Span,
    pub attrs: Vec<Attr>,
    pub stmt: Box<Stmt>,
}
```

Add two variants to the existing `Item` enum:

```rust
pub enum Item {
    DisciplineDecl(DisciplineDecl),
    NatureDecl(NatureDecl),
    ModuleDecl(ModuleDecl),
    ExternModule(ExternModuleDecl),   // ← add
}
```

Add one variant to the existing `ModuleItem` enum:

```rust
pub enum ModuleItem {
    BodyPortDecl(BodyPortDecl),
    NetDecl(NetDecl),
    AnalogBehaviour(AnalogBehaviour),
    Function(Function),
    BranchDecl(BranchDecl),
    VarDecl(VarDecl),
    ParamDecl(ParamDecl),
    AliasParam(AliasParam),
    Instance(InstanceDecl),
    InitialBlock(InitialBlock),       // ← add
}
```

### 3.2 `crates/piperine-parser/src/grammar/item.rs` — grammar

**In `item()`** add `extern` branch:

```rust
pub(super) fn item(&mut self) -> PResult<Item> {
    let start = self.span_start();
    let attrs = self.attrs()?;
    if self.at_kw("discipline") {
        Ok(Item::DisciplineDecl(self.discipline(attrs, start)?))
    } else if self.at_kw("nature") {
        Ok(Item::NatureDecl(self.nature(attrs, start)?))
    } else if self.at_kw("module") || self.at_kw("macromodule") {
        Ok(Item::ModuleDecl(self.module(attrs, start)?))
    } else if self.at_kw("extern") {
        Ok(Item::ExternModule(self.extern_module(attrs, start)?))
    } else {
        Err(format!("expected top-level item, found {:?}", self.peek()))
    }
}
```

**New `extern_module()` method:**

```rust
fn extern_module(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ExternModuleDecl> {
    self.expect_kw("extern")?;
    self.expect_kw("module")?;
    let name = self.name()?;
    self.expect(&Tok::LParen)?;

    let mut ports = Vec::new();
    let mut parameters = Vec::new();

    while !self.at(&Tok::RParen) && !self.at(&Tok::Semi) {
        if self.at_dir() {
            let port_start = self.span_start();
            let port_attrs = self.attrs()?;
            ports.push(self.port_decl(port_attrs, port_start)?);
        } else if self.at_kw("parameter") {
            parameters.push(self.extern_parameter()?);
        } else {
            return Err(format!("expected port or parameter in extern module, found {:?}", self.peek()));
        }
        if !self.eat(&Tok::Comma) { break; }
    }

    if self.eat(&Tok::Semi) {
        while !self.at(&Tok::RParen) {
            parameters.push(self.extern_parameter()?);
            if !self.eat(&Tok::Comma) { break; }
        }
    }

    self.expect(&Tok::RParen)?;
    self.expect(&Tok::Semi)?;
    Ok(ExternModuleDecl { attrs, name, ports, parameters, span: Span { start, end: self.prev_end() } })
}

fn extern_parameter(&mut self) -> PResult<ExternParameter> {
    self.expect_kw("parameter")?;
    let ty = self.type_()?;
    let name = self.name()?;
    let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
    Ok(ExternParameter { name, ty, default })
}
```

**In `module_item()`** add `initial` branch at the top:

```rust
fn module_item(&mut self) -> PResult<ModuleItem> {
    let start = self.span_start();
    let attrs = self.attrs()?;
    if self.at_kw("initial") {
        return self.initial_block(attrs, start);
    }
    // ... rest unchanged ...
}
```

**New `initial_block()` method:**

```rust
fn initial_block(&mut self, attrs: Vec<Attr>, start: usize) -> PResult<ModuleItem> {
    self.expect_kw("initial")?;
    let stmt = Box::new(self.stmt()?);
    Ok(ModuleItem::InitialBlock(InitialBlock {
        attrs, stmt, span: Span { start, end: self.prev_end() },
    }))
}
```

### 3.3 `crates/piperine-parser/src/model.rs` — surface new items

Add `extern_modules` to `Document`:

```rust
pub struct Document {
    pub modules: Vec<Module>,
    pub disciplines: Vec<Discipline>,
    pub natures: Vec<Nature>,
    pub extern_modules: Vec<crate::ast::ExternModuleDecl>,  // ← add
}
```

Add `initial_blocks` to `Module`:

```rust
pub struct Module {
    // ... existing fields ...
    pub initial_blocks: Vec<InitialBlock>,  // ← add
}

pub struct InitialBlock {
    pub stmt: crate::ast::Stmt,
    pub span: Span,
}
```

### 3.4 `crates/piperine-parser/src/parser.rs` — conversion

In `parse_with_includes`, add:

```rust
ast::Item::ExternModule(decl) => {
    doc.extern_modules.push(decl);
}
```

In `convert_module`, add:

```rust
ast::ModuleItem::InitialBlock(b) => {
    module.initial_blocks.push(crate::model::InitialBlock {
        stmt: *b.stmt,
        span: b.span,
    });
}
```

**After these changes:** run `cargo test -p piperine-parser`. All existing tests must pass.

---

## 4. `piperine-circuit` — hardware definition traits and elaboration

### 4.1 File structure

```
crates/piperine-circuit/src/
  lib.rs                ← re-exports
  error.rs              ← ElaborationError
  types.rs              ← ParameterValue, ParameterMap, ConnectionMap
  hardware.rs           ← HardwareDefinition + HardwareInstance traits
  registry.rs           ← HardwareRegistry
  builtins.rs           ← helpers shared by backend plugins (not ngspice-specific)
  elaboration.rs        ← elaborate() function → ElaborationResult
```

### 4.2 `src/error.rs`

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ElaborationError {
    #[error("unknown module `{name}` — no plugin registered a HardwareDefinition with this name")]
    UnknownModule { name: String },

    #[error("missing required parameter `{parameter}` on instance `{instance}`")]
    MissingParameter { parameter: String, instance: String },

    #[error("type error in parameter `{parameter}`: {detail}")]
    TypeError { parameter: String, detail: String },

    #[error("connection error on instance `{instance}`: {detail}")]
    ConnectionError { instance: String, detail: String },

    #[error("no testbench found — expected a module with an `initial` block")]
    NoTestbench,
}
```

### 4.3 `src/types.rs`

```rust
use std::collections::HashMap;

/// A resolved parameter value — evaluated at elaboration time from AST literals.
#[derive(Debug, Clone)]
pub enum ParameterValue {
    Real(f64),
    Integer(i64),
    String(std::string::String),
}

impl ParameterValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ParameterValue::Real(v)    => Some(*v),
            ParameterValue::Integer(i) => Some(*i as f64),
            ParameterValue::String(_)  => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { ParameterValue::String(s) => Some(s), _ => None }
    }
}

impl std::fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterValue::Real(v)    => write!(f, "{v}"),
            ParameterValue::Integer(i) => write!(f, "{i}"),
            ParameterValue::String(s)  => write!(f, "{s}"),
        }
    }
}

/// `parameter_name → value` mapping for one instance.
pub type ParameterMap = HashMap<std::string::String, ParameterValue>;

/// `port_name → net_name` mapping for one instance.
pub type ConnectionMap = HashMap<std::string::String, std::string::String>;

/// Parse a Verilog-A SI-suffix real literal into f64.
/// Handles: T G M k m u n p f a ns us ms ps fs
pub fn parse_si_real(s: &str) -> Option<f64> {
    let bytes = s.as_bytes();
    let (number_str, suffix) = match bytes.last() {
        Some(c) if c.is_ascii_alphabetic() => {
            // Check for two-char suffix (ns, us, ms, ps, fs)
            if bytes.len() >= 2 && bytes[bytes.len() - 1] == b's'
                && bytes[bytes.len() - 2].is_ascii_alphabetic()
            {
                (&s[..s.len() - 2], &s[s.len() - 2..])
            } else {
                (&s[..s.len() - 1], &s[s.len() - 1..])
            }
        }
        _ => return s.parse::<f64>().ok(),
    };
    let base: f64 = number_str.parse().ok()?;
    let scale = match suffix {
        "T"  => 1e12,  "G"  => 1e9,   "M"  => 1e6,
        "K" | "k" => 1e3,
        "m"  => 1e-3,  "u"  => 1e-6,  "n"  => 1e-9,
        "p"  => 1e-12, "f"  => 1e-15, "a"  => 1e-18,
        "ns" => 1e-9,  "us" => 1e-6,  "ms" => 1e-3,
        "ps" => 1e-12, "fs" => 1e-15,
        _ => return None,
    };
    Some(base * scale)
}
```

### 4.4 `src/hardware.rs` — the two core traits

**Design rationale:**

`HardwareDefinition` is the *type* of a hardware element — like a class. It knows port
declarations, parameter declarations, and how to construct an instance.

`HardwareInstance` is one *occurrence* in the netlist — like an object. Its only job is
emitting SPICE lines.

Both traits are `dyn`-safe (no `Self` in methods, no generics in methods). This is intentional:
plugins store `Box<dyn HardwareDefinition>` in the registry and the elaborator works with it
without knowing the concrete type.

```rust
use std::fmt;
use crate::types::{ParameterValue, ParameterMap, ConnectionMap};
use crate::error::ElaborationError;

/// Direction of a port as declared in an extern module or VA module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection { Input, Output, Inout }

/// Declaration of one port on a HardwareDefinition.
#[derive(Debug, Clone)]
pub struct PortDefinition {
    pub name: String,
    pub direction: PortDirection,
}

/// Declaration of one parameter on a HardwareDefinition.
#[derive(Debug, Clone)]
pub struct ParameterDefinition {
    pub name: String,
    /// Default value. `None` means the parameter is mandatory.
    pub default: Option<ParameterValue>,
}

/// A hardware element type — anything that can be instantiated in a circuit.
///
/// Implement this trait to add new element types:
/// - ngspice built-in elements (resistor, voltage source, …)
/// - Verilog-A modules compiled to OSDI (Phase 2)
/// - B-source behavioral elements (Phase 3)
/// - Subcircuit definitions (future)
///
/// Register implementations via `HardwareRegistry::register()`.
pub trait HardwareDefinition: fmt::Debug + Send + Sync {
    /// Name as declared in source (e.g., `"spice_res"`, `"simple_diode"`).
    fn name(&self) -> &str;

    /// Ordered list of port declarations.
    /// The elaborator uses this to validate named port connections.
    fn ports(&self) -> &[PortDefinition];

    /// Ordered list of parameter declarations with optional defaults.
    /// The elaborator applies defaults before calling `instantiate`.
    fn parameters(&self) -> &[ParameterDefinition];

    /// Create a concrete instance.
    ///
    /// Called by the elaborator after resolving all parameter defaults
    /// and validating connection names. Implementations should assume
    /// `parameters` already has defaults applied — report errors only
    /// for missing mandatory parameters.
    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError>;
}

/// A fully resolved hardware instance in the netlist.
///
/// The sole responsibility of a `HardwareInstance` is emitting the SPICE
/// deck lines that represent it. For most elements this is one line.
/// OSDI devices emit `N`-prefix lines. Subcircuit calls emit `X`-prefix lines.
pub trait HardwareInstance: fmt::Debug {
    fn instance_name(&self) -> &str;
    /// SPICE deck lines for this element (no `.model`, `.subckt`, or `.end`).
    fn spice_lines(&self) -> Vec<String>;
}
```

### 4.5 `src/registry.rs`

```rust
use std::collections::HashMap;
use crate::hardware::HardwareDefinition;

/// Registry of all known hardware element types.
///
/// Populated at startup by plugins via `Plugin::register_hardware()`.
/// The elaborator looks up instances by module name.
#[derive(Default)]
pub struct HardwareRegistry {
    definitions: HashMap<String, Box<dyn HardwareDefinition>>,
}

impl HardwareRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, definition: Box<dyn HardwareDefinition>) {
        self.definitions.insert(definition.name().to_string(), definition);
    }

    pub fn get(&self, name: &str) -> Option<&dyn HardwareDefinition> {
        self.definitions.get(name).map(|b| b.as_ref())
    }
}
```

### 4.6 `src/elaboration.rs`

```rust
use cvaf::ast::{self, Expr, Literal, PathSegment};
use cvaf::model::{Document, Module};
use crate::error::ElaborationError;
use crate::registry::HardwareRegistry;
use crate::types::{ParameterValue, ParameterMap, ConnectionMap, parse_si_real};

/// Result of elaborating one testbench module.
pub struct ElaborationResult {
    /// SPICE netlist lines without `.end` — caller appends it.
    pub spice_lines: Vec<String>,
    /// The `initial` block body, ready for the interpreter.
    pub initial_statement: ast::Stmt,
}

/// Elaborate the first testbench module found in `document`.
///
/// A testbench module is any module with at least one `initial` block.
/// Modules with only `analog` blocks are not yet supported (Phase 2).
pub fn elaborate(
    document: &Document,
    registry: &HardwareRegistry,
) -> Result<ElaborationResult, ElaborationError> {
    let testbench = find_testbench(document).ok_or(ElaborationError::NoTestbench)?;

    let mut spice_lines = vec![format!("* piperine: {}", testbench.name)];

    for instance in &testbench.instances {
        let definition = registry.get(&instance.module).ok_or_else(|| {
            ElaborationError::UnknownModule { name: instance.module.clone() }
        })?;

        let parameters = resolve_parameters(&instance.params, &instance.name, definition.parameters())?;
        let connections = resolve_connections(&instance.connections, &instance.name)?;

        let hardware_instance = definition.instantiate(&instance.name, &parameters, &connections)?;
        spice_lines.extend(hardware_instance.spice_lines());
    }

    let initial_statement = testbench
        .initial_blocks
        .first()
        .ok_or(ElaborationError::NoTestbench)?
        .stmt
        .clone();

    Ok(ElaborationResult { spice_lines, initial_statement })
}

fn find_testbench(document: &Document) -> Option<&Module> {
    document.modules.iter().find(|m| !m.initial_blocks.is_empty())
}

fn resolve_parameters(
    source_connections: &[cvaf::model::Connection],
    instance_name: &str,
    definitions: &[crate::hardware::ParameterDefinition],
) -> Result<ParameterMap, ElaborationError> {
    // Apply defaults first.
    let mut map: ParameterMap = definitions
        .iter()
        .filter_map(|d| d.default.as_ref().map(|v| (d.name.clone(), v.clone())))
        .collect();

    // Override with values from source.
    for connection in source_connections {
        match connection {
            cvaf::model::Connection::Named { port, expr } => {
                if let Some(expr) = expr {
                    let value = ast_expr_to_parameter_value(expr, port, instance_name)?;
                    map.insert(port.clone(), value);
                }
            }
            cvaf::model::Connection::Positional(_) => {
                return Err(ElaborationError::TypeError {
                    parameter: "<positional>".into(),
                    detail: "positional parameter overrides not supported; use named syntax: #(.r(1k))".into(),
                });
            }
        }
    }

    // Check all mandatory parameters are satisfied.
    for definition in definitions {
        if definition.default.is_none() && !map.contains_key(&definition.name) {
            return Err(ElaborationError::MissingParameter {
                parameter: definition.name.clone(),
                instance: instance_name.to_string(),
            });
        }
    }

    Ok(map)
}

fn resolve_connections(
    source_connections: &[cvaf::model::Connection],
    instance_name: &str,
) -> Result<ConnectionMap, ElaborationError> {
    let mut map = ConnectionMap::new();
    for connection in source_connections {
        match connection {
            cvaf::model::Connection::Named { port, expr } => {
                let net = match expr {
                    Some(Expr::Path(path)) => path_to_net_name(path),
                    None => String::new(), // unconnected port
                    Some(_) => return Err(ElaborationError::ConnectionError {
                        instance: instance_name.to_string(),
                        detail: format!("port `{port}` must connect to a net name, not an expression"),
                    }),
                };
                // Ground substitution: `gnd` → `0` (SPICE convention).
                let net = if net == "gnd" { "0".to_string() } else { net };
                map.insert(port.clone(), net);
            }
            cvaf::model::Connection::Positional(_) => {
                return Err(ElaborationError::ConnectionError {
                    instance: instance_name.to_string(),
                    detail: "positional port connections not supported; use named: .p(net)".into(),
                });
            }
        }
    }
    Ok(map)
}

fn ast_expr_to_parameter_value(
    expr: &Expr,
    parameter: &str,
    instance: &str,
) -> Result<ParameterValue, ElaborationError> {
    match expr {
        Expr::Literal(Literal::IntNumber(s)) => {
            s.parse::<i64>().map(ParameterValue::Integer).map_err(|_| ElaborationError::TypeError {
                parameter: parameter.into(),
                detail: format!("cannot parse integer: {s}"),
            })
        }
        Expr::Literal(Literal::StdRealNumber(s)) => {
            s.parse::<f64>().map(ParameterValue::Real).map_err(|_| ElaborationError::TypeError {
                parameter: parameter.into(),
                detail: format!("cannot parse real: {s}"),
            })
        }
        Expr::Literal(Literal::SiRealNumber(s)) => {
            parse_si_real(s).map(ParameterValue::Real).ok_or_else(|| ElaborationError::TypeError {
                parameter: parameter.into(),
                detail: format!("cannot parse SI real: {s}"),
            })
        }
        Expr::Literal(Literal::StrLit(s)) => Ok(ParameterValue::String(s.clone())),
        Expr::Prefix(ast::PrefixOp::Neg, inner) => {
            match ast_expr_to_parameter_value(inner, parameter, instance)? {
                ParameterValue::Real(v)    => Ok(ParameterValue::Real(-v)),
                ParameterValue::Integer(v) => Ok(ParameterValue::Integer(-v)),
                _ => Err(ElaborationError::TypeError {
                    parameter: parameter.into(),
                    detail: "cannot negate a string".into(),
                }),
            }
        }
        _ => Err(ElaborationError::TypeError {
            parameter: parameter.into(),
            detail: format!("parameter `{parameter}` on instance `{instance}` must be a literal"),
        }),
    }
}

pub fn path_to_net_name(path: &ast::Path) -> String {
    match &path.segment {
        PathSegment::Ident(s) => s.clone(),
        PathSegment::Root     => "root".to_string(),
    }
}
```

### 4.7 `src/lib.rs`

```rust
pub mod error;
pub mod hardware;
pub mod registry;
pub mod types;
pub mod elaboration;

pub use elaboration::{elaborate, ElaborationResult};
pub use error::ElaborationError;
pub use hardware::{HardwareDefinition, HardwareInstance, PortDefinition, PortDirection, ParameterDefinition};
pub use registry::HardwareRegistry;
pub use types::{ParameterValue, ParameterMap, ConnectionMap, parse_si_real};
```

---

## 5. `piperine-interpreter` — value types, system task traits, Plugin trait

### 5.1 File structure

```
crates/piperine-interpreter/src/
  lib.rs            ← re-exports
  value.rs          ← Value enum
  error.rs          ← InterpreterError
  backend.rs        ← SimulatorBackend trait + AnalogCompilerBackend trait
  plugin.rs         ← Plugin trait (the main extension point)
  task.rs           ← SystemTask trait + SystemTaskRegistry
  interpreter.rs    ← Interpreter, Scope, eval_stmt, eval_expr
```

### 5.2 `src/value.rs`

**Why enum, not trait:** `Value` is a closed set for the MVP. An enum forces exhaustive
pattern matching, which catches missing cases at compile time. A trait would require
`Box<dyn Value>` everywhere, adding indirection for no benefit when the set is fixed.
Dynamic arrays and queues (Phase 3) arrive as new variants, not new trait impls.

```rust
use std::fmt;

/// A runtime value in the Piperine interpreter.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Real(f64),
    Integer(i64),
    String(std::string::String),
    Void,
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Real(v)    => Some(*v),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { Value::String(s) => Some(s), _ => None }
    }
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Real(v)    => Some(*v as i64),
            _ => None,
        }
    }
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Real(v)    => *v != 0.0,
            Value::Integer(i) => *i != 0,
            Value::String(s)  => !s.is_empty(),
            Value::Void       => false,
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Real(_)    => "real",
            Value::Integer(_) => "integer",
            Value::String(_)  => "string",
            Value::Void       => "void",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Real(v)    => write!(f, "{v}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::String(s)  => write!(f, "{s}"),
            Value::Void       => write!(f, "<void>"),
        }
    }
}
```

### 5.3 `src/error.rs`

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InterpreterError {
    #[error("undefined variable `{name}`")]
    UndefinedVariable { name: String },

    #[error("type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("undefined system task `${name}`")]
    UndefinedSystemTask { name: String },

    #[error("simulator error: {0}")]
    SimulatorError(String),

    #[error("assertion failed: {message}")]
    AssertionFailed { message: String },

    #[error("{0}")]
    Other(String),
}
```

### 5.4 `src/backend.rs` — simulator and analog compiler backend traits

**Design rationale:** `SimulatorBackend` decouples the interpreter from ngspice. Any simulator
that can load a SPICE netlist, run a command, and return named vectors implements this trait.
The `AnalogCompilerBackend` trait is the slot for OpenVAF — it compiles Verilog-A to a loadable
artifact and tells the simulator to load it. Both traits are registered via the `Plugin` system.

```rust
use std::path::{Path, PathBuf};
use crate::error::InterpreterError;

/// A live simulator session — the interpreter calls into this to run analyses
/// and read results.
///
/// The ngspice implementation (`NgspiceBackend` in `piperine-ngspice`) wraps
/// a process-isolated worker via IPC. Future backends (Xyce, FOSS SPICE) implement
/// the same trait without touching the interpreter.
pub trait SimulatorBackend: Send {
    /// Load a SPICE netlist into the simulator.
    /// Must be called once before any analysis.
    fn load_circuit(&mut self, lines: &[String]) -> Result<(), InterpreterError>;

    /// Run a simulator command (e.g., `"op"`, `"tran 1n 5m"`).
    fn run_command(&mut self, command: &str) -> Result<(), InterpreterError>;

    /// Retrieve all values of a named vector from the current plot.
    /// For OP analysis, returns a single-element Vec.
    /// Vector names follow ngspice convention: `"v(vmid)"`, `"i(v1)"`.
    fn get_vector(&mut self, name: &str) -> Result<Vec<f64>, InterpreterError>;

    /// Print a line to stdout. Default implementation uses `println!`.
    fn print(&self, line: &str) { println!("{line}"); }
}

/// A backend that compiles Verilog-A modules to loadable simulator objects.
///
/// The OpenVAF implementation (`OpenVafCompiler` in `piperine-openvaf`, Phase 2)
/// invokes the `openvaf` binary and caches results by source hash.
pub trait AnalogCompilerBackend: Send + Sync {
    /// Compiler identifier (e.g., `"openvaf"`).
    fn name(&self) -> &str;

    /// Compile a Verilog-A source file to a simulator-loadable artifact (e.g., `.osdi`).
    /// `output_directory`: where to place the compiled artifact.
    /// Returns the path of the compiled file.
    fn compile(
        &self,
        source_path: &Path,
        output_directory: &Path,
    ) -> Result<PathBuf, InterpreterError>;

    /// Load the compiled artifact into a live simulator session.
    /// For OSDI this runs `pre_osdi <path>` via the simulator backend.
    fn pre_load(
        &self,
        artifact_path: &Path,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<(), InterpreterError>;
}
```

### 5.5 `src/plugin.rs` — the Plugin trait

**This is the main extension point.** A plugin is a self-contained unit that brings new
capabilities to the Piperine runtime. It registers hardware definitions, system tasks, and
optionally provides the simulator backend or analog compiler backend.

**How plugins work at startup:**
1. The main binary creates a `Runtime` (or just holds the registries directly for the MVP).
2. It calls `runtime.register_plugin(Box::new(NgspicePlugin::new()))`.
3. The plugin's `register_hardware()` adds `SpiceResistor`, `SpiceVoltageSource`, etc.
4. The plugin's `register_tasks()` adds `$op`, `$tran`, `$V`, `$display`, etc.
5. The plugin's `simulator_backend()` returns the live ngspice session.

Adding a new device library: ship a crate with a struct implementing `Plugin`, call
`runtime.register_plugin(Box::new(MyLibraryPlugin))`. Zero changes to core.

```rust
use crate::task::SystemTaskRegistry;
use crate::backend::{SimulatorBackend, AnalogCompilerBackend};
use piperine_circuit::HardwareRegistry;

/// A Piperine plugin — the primary extension mechanism.
///
/// Plugins bring hardware definitions, system tasks, and simulator/compiler
/// backends into the runtime. The main binary registers plugins at startup;
/// all capabilities flow from the registered set.
pub trait Plugin: Send + Sync {
    /// Unique plugin identifier (e.g., `"ngspice"`, `"openvaf"`, `"xyce"`).
    fn name(&self) -> &str;

    /// Register hardware element definitions this plugin provides.
    ///
    /// Called once before elaboration. Implementations call
    /// `registry.register(Box::new(MyElement))` for each element type.
    fn register_hardware(&self, _registry: &mut HardwareRegistry) {}

    /// Register system tasks (`$xxx`) this plugin provides.
    ///
    /// Called once before interpretation begins. Implementations call
    /// `registry.register(Box::new(MyTask))` for each task.
    fn register_tasks(&self, _registry: &mut SystemTaskRegistry) {}

    /// Provide a live simulator backend session.
    ///
    /// Return `Some(backend)` if this plugin owns the simulator.
    /// Only the first plugin that returns `Some` is used — register simulator
    /// plugins before device-library plugins. Return `None` if not applicable.
    fn simulator_backend(&self) -> Option<Box<dyn SimulatorBackend>> { None }

    /// Provide an analog compiler backend (Phase 2).
    ///
    /// Return `Some(compiler)` if this plugin can compile Verilog-A modules.
    /// Used by the `$pre_osdi` system task and the elaborator for analog modules.
    fn analog_compiler(&self) -> Option<Box<dyn AnalogCompilerBackend>> { None }
}
```

### 5.6 `src/task.rs` — SystemTask trait and registry

```rust
use std::collections::HashMap;
use std::fmt;
use crate::value::Value;
use crate::error::InterpreterError;
use crate::backend::SimulatorBackend;

/// A callable system task or function (`$name`).
///
/// Tasks return `None` (void). Functions return `Some(Value)`.
/// Register implementations via `SystemTaskRegistry::register()`.
///
/// Implement this trait to add new `$xxx` calls — ngspice analyses,
/// measurement functions, display routines, assertion handlers, etc.
pub trait SystemTask: fmt::Debug + Send + Sync {
    /// Name WITHOUT the `$` prefix (e.g., `"op"`, `"display"`, `"V"`).
    fn name(&self) -> &str;

    /// Execute the task.
    ///
    /// `arguments`: evaluated argument values, left to right.
    /// `simulator`: mutable access to the simulator backend.
    fn call(
        &self,
        arguments: Vec<Value>,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError>;
}

/// Registry of all known system tasks and functions.
///
/// Populated at startup by plugins via `Plugin::register_tasks()`.
#[derive(Default)]
pub struct SystemTaskRegistry {
    tasks: HashMap<String, Box<dyn SystemTask>>,
}

impl SystemTaskRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, task: Box<dyn SystemTask>) {
        self.tasks.insert(task.name().to_string(), task);
    }

    pub fn get(&self, name: &str) -> Option<&dyn SystemTask> {
        self.tasks.get(name).map(|b| b.as_ref())
    }
}
```

### 5.7 `src/lib.rs`

```rust
pub mod value;
pub mod error;
pub mod backend;
pub mod plugin;
pub mod task;
pub mod interpreter;

pub use value::Value;
pub use error::InterpreterError;
pub use backend::{SimulatorBackend, AnalogCompilerBackend};
pub use plugin::Plugin;
pub use task::{SystemTask, SystemTaskRegistry};
pub use interpreter::{Interpreter, Scope};
```

---

## 6. Interpreter — `crates/piperine-interpreter/src/interpreter.rs`

### 6.1 Scope

```rust
use std::collections::HashMap;
use crate::value::Value;

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
```

### 6.2 Interpreter struct

```rust
use cvaf::ast::*;
use crate::backend::SimulatorBackend;
use crate::task::SystemTaskRegistry;
use crate::value::Value;
use crate::error::InterpreterError;

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
}
```

### 6.3 Statement execution

```rust
impl<'a> Interpreter<'a> {
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
                let mut matched = false;
                for case in &case_stmt.cases {
                    let hit = match &case.item {
                        CaseItem::Default => !matched,
                        CaseItem::Exprs(exprs) => exprs.iter().any(|e| {
                            self.eval_expr(e, scope).map(|v| v == discriminant).unwrap_or(false)
                        }),
                    };
                    if hit {
                        self.eval_statement(&case.stmt, scope)?;
                        matched = true;
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
}
```

### 6.4 Expression evaluation

```rust
impl<'a> Interpreter<'a> {
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
```

### 6.5 Helpers (bottom of `interpreter.rs`)

```rust
use crate::value::Value;
use crate::error::InterpreterError;
use cvaf::ast::*;
use piperine_circuit::parse_si_real;

fn eval_literal(literal: &Literal) -> Value {
    match literal {
        Literal::IntNumber(s)     => s.parse::<i64>().map(Value::Integer)
                                      .unwrap_or_else(|_| Value::Real(s.parse().unwrap_or(0.0))),
        Literal::StdRealNumber(s) => Value::Real(s.parse().unwrap_or(0.0)),
        Literal::SiRealNumber(s)  => Value::Real(parse_si_real(s).unwrap_or(0.0)),
        Literal::StrLit(s)        => Value::String(s.clone()),
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
        BinOp::Shl    => Value::Integer(a << (b as u32)),
        BinOp::Shr    => Value::Integer(a >> (b as u32)),
        other => return Err(InterpreterError::TypeError {
            expected: "integer-compatible binary operator".into(),
            got: format!("{other:?}"),
        }),
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
```

---

## 7. `piperine-ngspice` — the ngspice plugin

This crate is one self-contained plugin. It contains:
- `NgspiceBackend` — implements `SimulatorBackend` via IPC to the worker process
- `SpiceResistor`, `SpiceVoltageSource`, etc. — implement `HardwareDefinition`
- `OperatingPointTask`, `VoltageTask`, `DisplayTask`, etc. — implement `SystemTask`
- `NgspicePlugin` — the single struct the main binary registers

### 7.1 File structure

```
crates/piperine-ngspice/src/
  lib.rs         ← NgspicePlugin + re-exports
  backend.rs     ← NgspiceBackend implementing SimulatorBackend
  hardware.rs    ← SpiceResistor, SpiceVoltageSource, etc.
  tasks.rs       ← OperatingPointTask, TransientTask, VoltageTask, DisplayTask, etc.
```

### 7.2 `src/backend.rs` — NgspiceBackend

```rust
use piperine_common::{Command, Response, CmdSender, RespReceiver};
use piperine_interpreter::{SimulatorBackend, InterpreterError};

/// A `SimulatorBackend` backed by a process-isolated ngspice worker.
/// Communicates via IPC channels established by `piperine-coordinator`.
pub struct NgspiceBackend {
    command_sender:   CmdSender,
    response_receiver: RespReceiver,
}

impl NgspiceBackend {
    pub fn new(command_sender: CmdSender, response_receiver: RespReceiver) -> Self {
        Self { command_sender, response_receiver }
    }

    fn send(&mut self, command: Command) -> Result<Response, InterpreterError> {
        self.command_sender
            .send(command)
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))?;
        self.response_receiver
            .recv()
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))
    }
}

impl SimulatorBackend for NgspiceBackend {
    fn load_circuit(&mut self, lines: &[String]) -> Result<(), InterpreterError> {
        match self.send(Command::LoadCircuit { lines: lines.to_vec() })? {
            Response::Ok => Ok(()),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }

    fn run_command(&mut self, command: &str) -> Result<(), InterpreterError> {
        match self.send(Command::Run { cmd: command.to_string() })? {
            Response::Ok => Ok(()),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }

    fn get_vector(&mut self, name: &str) -> Result<Vec<f64>, InterpreterError> {
        match self.send(Command::GetVecData { name: name.to_string() })? {
            Response::VecData { values } => Ok(values),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }
}
```

### 7.3 `src/hardware.rs` — ngspice built-in element definitions

**How to add a new element:** copy one of the structs below, change the name, SPICE prefix,
ports, and parameters. Register it in `NgspicePlugin::register_hardware()`. Nothing else changes.

```rust
use std::fmt;
use piperine_circuit::{
    HardwareDefinition, HardwareInstance,
    PortDefinition, PortDirection, ParameterDefinition,
    ParameterMap, ConnectionMap, ElaborationError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn require_net<'a>(
    connections: &'a ConnectionMap,
    port: &str,
    instance: &str,
) -> Result<&'a str, ElaborationError> {
    connections.get(port).map(|s| s.as_str()).ok_or_else(|| ElaborationError::ConnectionError {
        instance: instance.to_string(),
        detail: format!("port `{port}` not connected"),
    })
}

fn require_parameter(
    parameters: &ParameterMap,
    name: &str,
    instance: &str,
) -> Result<f64, ElaborationError> {
    parameters.get(name)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ElaborationError::MissingParameter {
            parameter: name.to_string(),
            instance: instance.to_string(),
        })
}

// ── SpiceResistor ────────────────────────────────────────────────────────────

/// `extern module spice_res(inout p, inout n; parameter real r = 1e3)`
/// SPICE line: `R{name} {p} {n} {r}`
#[derive(Debug)]
pub struct SpiceResistor;

impl HardwareDefinition for SpiceResistor {
    fn name(&self) -> &str { "spice_res" }
    fn ports(&self) -> &[PortDefinition] { &[] }           // validated by connection resolver
    fn parameters(&self) -> &[ParameterDefinition] { &[] } // default applied by source declaration

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let r = require_parameter(parameters, "r", instance_name)?;
        Ok(Box::new(SpiceResistorInstance { name: instance_name.to_string(), p, n, r }))
    }
}

#[derive(Debug)]
struct SpiceResistorInstance { name: String, p: String, n: String, r: f64 }

impl HardwareInstance for SpiceResistorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("R{} {} {} {}", self.name, self.p, self.n, self.r)]
    }
}

// ── SpiceVoltageSource ───────────────────────────────────────────────────────

/// `extern module spice_vsource(inout p, inout n; parameter real val = 0.0)`
/// SPICE line: `V{name} {p} {n} DC {val}`
#[derive(Debug)]
pub struct SpiceVoltageSource;

impl HardwareDefinition for SpiceVoltageSource {
    fn name(&self) -> &str { "spice_vsource" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p   = require_net(connections, "p", instance_name)?.to_string();
        let n   = require_net(connections, "n", instance_name)?.to_string();
        let val = require_parameter(parameters, "val", instance_name)?;
        Ok(Box::new(SpiceVoltageSourceInstance { name: instance_name.to_string(), p, n, val }))
    }
}

#[derive(Debug)]
struct SpiceVoltageSourceInstance { name: String, p: String, n: String, val: f64 }

impl HardwareInstance for SpiceVoltageSourceInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("V{} {} {} DC {}", self.name, self.p, self.n, self.val)]
    }
}

// ── SpiceCurrentSource ───────────────────────────────────────────────────────

/// `extern module spice_isource(inout p, inout n; parameter real val = 0.0)`
/// SPICE line: `I{name} {p} {n} DC {val}`
#[derive(Debug)]
pub struct SpiceCurrentSource;

impl HardwareDefinition for SpiceCurrentSource {
    fn name(&self) -> &str { "spice_isource" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p   = require_net(connections, "p", instance_name)?.to_string();
        let n   = require_net(connections, "n", instance_name)?.to_string();
        let val = require_parameter(parameters, "val", instance_name)?;
        Ok(Box::new(SpiceCurrentSourceInstance { name: instance_name.to_string(), p, n, val }))
    }
}

#[derive(Debug)]
struct SpiceCurrentSourceInstance { name: String, p: String, n: String, val: f64 }

impl HardwareInstance for SpiceCurrentSourceInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("I{} {} {} DC {}", self.name, self.p, self.n, self.val)]
    }
}

// ── SpiceCapacitor ───────────────────────────────────────────────────────────

/// `extern module spice_cap(inout p, inout n; parameter real c = 1e-12)`
/// SPICE line: `C{name} {p} {n} {c}`
#[derive(Debug)]
pub struct SpiceCapacitor;

impl HardwareDefinition for SpiceCapacitor {
    fn name(&self) -> &str { "spice_cap" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let c = require_parameter(parameters, "c", instance_name)?;
        Ok(Box::new(SpiceCapacitorInstance { name: instance_name.to_string(), p, n, c }))
    }
}

#[derive(Debug)]
struct SpiceCapacitorInstance { name: String, p: String, n: String, c: f64 }

impl HardwareInstance for SpiceCapacitorInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        vec![format!("C{} {} {} {}", self.name, self.p, self.n, self.c)]
    }
}
```

### 7.4 `src/tasks.rs` — ngspice system task implementations

```rust
use piperine_interpreter::{SystemTask, SimulatorBackend, Value, InterpreterError};

// ── $op() ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct OperatingPointTask;

impl SystemTask for OperatingPointTask {
    fn name(&self) -> &str { "op" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if !arguments.is_empty() {
            return Err(InterpreterError::TypeError {
                expected: "0 arguments".into(),
                got: format!("{} arguments", arguments.len()),
            });
        }
        simulator.run_command("op")?;
        Ok(None)
    }
}

// ── $tran(step, stop) ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TransientTask;

impl SystemTask for TransientTask {
    fn name(&self) -> &str { "tran" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if arguments.len() < 2 {
            return Err(InterpreterError::TypeError {
                expected: "2 arguments: $tran(step, stop)".into(),
                got: format!("{} arguments", arguments.len()),
            });
        }
        let step = arguments[0].as_f64().ok_or_else(|| InterpreterError::TypeError {
            expected: "real (step)".into(), got: arguments[0].type_name().into(),
        })?;
        let stop = arguments[1].as_f64().ok_or_else(|| InterpreterError::TypeError {
            expected: "real (stop)".into(), got: arguments[1].type_name().into(),
        })?;
        simulator.run_command(&format!("tran {step} {stop}"))?;
        Ok(None)
    }
}

// ── $V("node") ───────────────────────────────────────────────────────────────

/// Returns the node voltage after an analysis. Result is a `real`.
#[derive(Debug)]
pub struct VoltageTask;

impl SystemTask for VoltageTask {
    fn name(&self) -> &str { "V" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let node = arguments.first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| InterpreterError::TypeError {
                expected: "string node name".into(),
                got: arguments.first().map(|v| v.type_name()).unwrap_or("nothing").into(),
            })?
            .to_string();
        let vector = simulator.get_vector(&format!("v({node})"))?;
        let last_value = vector.last().copied().ok_or_else(|| {
            InterpreterError::SimulatorError(format!("vector v({node}) is empty after analysis"))
        })?;
        Ok(Some(Value::Real(last_value)))
    }
}

// ── $I("branch") ─────────────────────────────────────────────────────────────

/// Returns the branch current after an analysis. Result is a `real`.
#[derive(Debug)]
pub struct CurrentTask;

impl SystemTask for CurrentTask {
    fn name(&self) -> &str { "I" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let branch = arguments.first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| InterpreterError::TypeError {
                expected: "string branch name (e.g., \"v1\")".into(),
                got: arguments.first().map(|v| v.type_name()).unwrap_or("nothing").into(),
            })?
            .to_string();
        let vector = simulator.get_vector(&format!("i({branch})"))?;
        let last_value = vector.last().copied().ok_or_else(|| {
            InterpreterError::SimulatorError(format!("vector i({branch}) is empty after analysis"))
        })?;
        Ok(Some(Value::Real(last_value)))
    }
}

// ── $display(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct DisplayTask;

impl SystemTask for DisplayTask {
    fn name(&self) -> &str { "display" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output = if arguments.is_empty() {
            String::new()
        } else {
            let format_string = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
                expected: "string format".into(),
                got: arguments[0].type_name().into(),
            })?.to_string();
            format_display_string(&format_string, &arguments[1..])
        };
        simulator.print(&output);
        Ok(None)
    }
}

/// Minimal `$display` format string processor.
/// Supported: `%g` `%f` `%d` `%s` `%0d` `%%` and literal text.
fn format_display_string(format: &str, arguments: &[Value]) -> String {
    let mut output = String::new();
    let mut chars = format.chars().peekable();
    let mut argument_index = 0;

    while let Some(character) = chars.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        // Consume optional width digits (e.g., `%0d`)
        while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            chars.next();
        }
        match chars.next() {
            Some('%')      => output.push('%'),
            Some('g') | Some('f') => {
                let value = arguments.get(argument_index).and_then(|v| v.as_f64()).unwrap_or(0.0);
                output.push_str(&format!("{value:g}"));
                argument_index += 1;
            }
            Some('d') => {
                let value = arguments.get(argument_index).and_then(|v| v.as_integer()).unwrap_or(0);
                output.push_str(&format!("{value}"));
                argument_index += 1;
            }
            Some('s') => {
                let value = arguments.get(argument_index).map(|v| v.to_string()).unwrap_or_default();
                output.push_str(&value);
                argument_index += 1;
            }
            Some(other) => { output.push('%'); output.push(other); }
            None        => { output.push('%'); }
        }
    }
    output
}
```

### 7.5 `src/lib.rs` — NgspicePlugin

```rust
mod backend;
mod hardware;
mod tasks;

pub use backend::NgspiceBackend;

use std::path::PathBuf;
use piperine_circuit::HardwareRegistry;
use piperine_interpreter::{Plugin, SimulatorBackend, SystemTaskRegistry};
use piperine_coordinator::pool::{ProcessPool, PoolConfig};

/// The ngspice plugin — registers all ngspice-backed hardware definitions,
/// system tasks, and provides the ngspice simulator backend.
///
/// Register this plugin at startup before any other plugin:
/// ```rust
/// runtime.register_plugin(Box::new(NgspicePlugin::default()));
/// ```
#[derive(Default)]
pub struct NgspicePlugin {
    /// Override path to the `piperine-worker` binary. `None` = auto-discover.
    pub worker_binary: Option<PathBuf>,
}

impl Plugin for NgspicePlugin {
    fn name(&self) -> &str { "ngspice" }

    fn register_hardware(&self, registry: &mut HardwareRegistry) {
        use hardware::*;
        registry.register(Box::new(SpiceResistor));
        registry.register(Box::new(SpiceVoltageSource));
        registry.register(Box::new(SpiceCurrentSource));
        registry.register(Box::new(SpiceCapacitor));
    }

    fn register_tasks(&self, registry: &mut SystemTaskRegistry) {
        use tasks::*;
        registry.register(Box::new(OperatingPointTask));
        registry.register(Box::new(TransientTask));
        registry.register(Box::new(VoltageTask));
        registry.register(Box::new(CurrentTask));
        registry.register(Box::new(DisplayTask));
    }

    fn simulator_backend(&self) -> Option<Box<dyn SimulatorBackend>> {
        let config = PoolConfig { size: 1, worker_binary: self.worker_binary.clone() };
        let mut pool = ProcessPool::spawn(config).ok()?;
        let handle = pool.take_first(); // see Section 8.1
        Some(Box::new(NgspiceBackend::new(handle.cmd, handle.resp)))
    }
}
```

---

## 8. Main binary wiring — `src/main.rs`

The main binary owns the registries, loads plugins into them, elaborates, and runs:

```rust
use std::path::PathBuf;
use piperine_circuit::{HardwareRegistry, elaborate};
use piperine_interpreter::{Plugin, SystemTaskRegistry, Interpreter, Scope};
use piperine_ngspice::NgspicePlugin;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: piperine <file.ppr>");
        std::process::exit(1);
    }
    if let Err(error) = run(PathBuf::from(&args[1])) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(path: PathBuf) -> Result<(), String> {
    // 1. Parse.
    let document = cvaf::parse_file(&path).map_err(|e| format!("parse: {e}"))?;

    // 2. Create registries.
    let mut hardware_registry = HardwareRegistry::new();
    let mut task_registry     = SystemTaskRegistry::new();

    // 3. Load plugins.
    //    Add plugins here as new ones are created. Order matters for simulator_backend():
    //    the first plugin returning Some wins. Always put the simulator plugin first.
    let plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(NgspicePlugin::default()),
        // Box::new(MyDeviceLibraryPlugin),  ← future device libraries go here
    ];

    let mut simulator_backend = None;
    for plugin in &plugins {
        plugin.register_hardware(&mut hardware_registry);
        plugin.register_tasks(&mut task_registry);
        if simulator_backend.is_none() {
            simulator_backend = plugin.simulator_backend();
        }
    }

    let mut simulator = simulator_backend
        .ok_or_else(|| "no plugin provided a simulator backend — is piperine-ngspice registered?".to_string())?;

    // 4. Elaborate — find testbench, build SPICE netlist.
    let mut elaboration = elaborate(&document, &hardware_registry)
        .map_err(|e| format!("elaboration: {e}"))?;
    elaboration.spice_lines.push(".end".to_string());

    // 5. Load circuit into simulator.
    use piperine_interpreter::SimulatorBackend;
    simulator.load_circuit(&elaboration.spice_lines)
        .map_err(|e| format!("circuit load: {e}"))?;

    // 6. Run interpreter on initial block.
    let mut interpreter = Interpreter::new(simulator.as_mut(), &task_registry);
    let mut scope = Scope::default();
    interpreter.exec(&elaboration.initial_statement, &mut scope)
        .map_err(|e| format!("runtime: {e}"))?;

    Ok(())
}
```

### 8.1 `piperine-coordinator/src/pool.rs` — add `take_first()`

Add this method to `ProcessPool`:

```rust
impl ProcessPool {
    /// Take ownership of the first worker's handle.
    /// For MVP single-worker use only.
    pub fn take_first(&mut self) -> WorkerHandle {
        self.workers.remove(0).handle
    }
}
```

`IpcReceiver` does not implement `Clone`, so ownership transfer is required. `take_first()`
removes the worker from the pool and returns its handle. For a pool of size 1, this is fine.
Phase 2 multi-worker support will add `acquire(index: usize)` returning a scoped lease.

---

## 9. OpenVAF / OSDI support (Phase 2 — deferred)

Implement this ONLY after Sections 1–8 produce `Vmid = 2.5 V`.

### 9.1 New crate: `piperine-openvaf`

```
crates/piperine-openvaf/src/
  lib.rs       ← OpenVafPlugin
  compiler.rs  ← OpenVafCompiler implementing AnalogCompilerBackend
  osdi.rs      ← OsdiHardwareDefinition implementing HardwareDefinition
```

### 9.2 `OpenVafPlugin` (sketch)

```rust
// piperine-openvaf/src/lib.rs

pub struct OpenVafPlugin {
    /// Path to `openvaf` binary. Checked via $PIPERINE_OPENVAF env var first, then PATH.
    pub binary: PathBuf,
    /// Directory for compiled .osdi cache.
    pub cache_directory: PathBuf,
}

impl Plugin for OpenVafPlugin {
    fn name(&self) -> &str { "openvaf" }

    // register_hardware: called after elaboration routes VA modules through compile().
    // The elaborator (Phase 2) compiles each analog module and calls
    // registry.register(Box::new(OsdiHardwareDefinition { ... })) for each one.
    // The plugin itself does not register hardware at startup — it registers
    // compiled modules dynamically.

    fn analog_compiler(&self) -> Option<Box<dyn AnalogCompilerBackend>> {
        Some(Box::new(compiler::OpenVafCompiler {
            binary: self.binary.clone(),
            cache_directory: self.cache_directory.clone(),
        }))
    }
}
```

### 9.3 `OsdiHardwareDefinition` (sketch)

```rust
// piperine-openvaf/src/osdi.rs

/// A Verilog-A module compiled to OSDI, registered as a HardwareDefinition.
/// SPICE line uses the `N` prefix (ngspice OSDI convention):
///   `N{name} {nodes...} {model_name} {param=value...}`
pub struct OsdiHardwareDefinition {
    pub module_name: String,
    pub osdi_path:   PathBuf,
    pub ports:       Vec<PortDefinition>,
    pub parameters:  Vec<ParameterDefinition>,
}

impl HardwareDefinition for OsdiHardwareDefinition {
    fn name(&self) -> &str { &self.module_name }
    fn ports(&self) -> &[PortDefinition] { &self.ports }
    fn parameters(&self) -> &[ParameterDefinition] { &self.parameters }
    fn instantiate(&self, _instance_name: &str, _parameters: &ParameterMap, _connections: &ConnectionMap)
        -> Result<Box<dyn HardwareInstance>, ElaborationError>
    {
        todo!("Phase 2: emit N-prefix SPICE line for OSDI instance")
    }
}
```

### 9.4 Phase 2 target program

```verilog
// examples/diode_op.ppr
module simple_diode(anode, cathode);
  inout anode, cathode;
  electrical anode, cathode;
  parameter real is = 1e-14;
  analog I(anode, cathode) <+ is * (exp(V(anode, cathode) / 0.02585) - 1);
endmodule

module tb;
  extern module spice_vsource(inout p, inout n; parameter real val = 0.0);
  simple_diode  #(.is(1e-14)) D1 (.anode(vout), .cathode(gnd));
  spice_vsource #(.val(0.7))  V1 (.p(vout), .n(gnd));

  initial begin
    $pre_osdi("simple_diode.osdi");
    $op();
    $display("Id = %g A", $I("d1"));
  end
endmodule
```

`$pre_osdi` is a new `SystemTask` in `piperine-openvaf` (or `piperine-ngspice`) that loads the
`.osdi` file via `simulator.run_command("pre_osdi simple_diode.osdi")`.

---

## 10. Implementation checklist

Complete in order. Each item must compile before the next begins.

**Parser extension:**
- [ ] Add `ExternModuleDecl`, `ExternParameter`, `InitialBlock` to `ast/item.rs`
- [ ] Add `Item::ExternModule` and `ModuleItem::InitialBlock` variants
- [ ] Add `extern_module()` and `initial_block()` grammar methods to `grammar/item.rs`
- [ ] Add `extern_modules` to `model::Document`, `initial_blocks` to `model::Module`
- [ ] Update `parser.rs` conversion for both new item types
- [ ] `cargo test -p piperine-parser` → all existing tests pass

**`piperine-circuit` crate:**
- [ ] Create `Cargo.toml`
- [ ] Write `error.rs` (`ElaborationError`)
- [ ] Write `types.rs` (`ParameterValue`, `ParameterMap`, `ConnectionMap`, `parse_si_real`)
- [ ] Write `hardware.rs` (`HardwareDefinition`, `HardwareInstance`, `PortDefinition`, `ParameterDefinition`)
- [ ] Write `registry.rs` (`HardwareRegistry`)
- [ ] Write `elaboration.rs` (`elaborate`, `ElaborationResult`)
- [ ] Write `lib.rs`
- [ ] `cargo build -p piperine-circuit` → compiles

**`piperine-interpreter` crate:**
- [ ] Create `Cargo.toml`
- [ ] Write `value.rs` (`Value`)
- [ ] Write `error.rs` (`InterpreterError`)
- [ ] Write `backend.rs` (`SimulatorBackend`, `AnalogCompilerBackend`)
- [ ] Write `plugin.rs` (`Plugin` trait)
- [ ] Write `task.rs` (`SystemTask`, `SystemTaskRegistry`)
- [ ] Write `interpreter.rs` (`Interpreter`, `Scope`, all eval methods)
- [ ] Write `lib.rs`
- [ ] `cargo build -p piperine-interpreter` → compiles

**`piperine-ngspice` crate:**
- [ ] Create `Cargo.toml`
- [ ] Write `backend.rs` (`NgspiceBackend`)
- [ ] Write `hardware.rs` (all four `SpiceX` structs)
- [ ] Write `tasks.rs` (all five task structs + `format_display_string`)
- [ ] Write `lib.rs` (`NgspicePlugin` with all registrations)
- [ ] `cargo build -p piperine-ngspice` → compiles

**Coordinator change:**
- [ ] Add `take_first()` to `ProcessPool` in `piperine-coordinator/src/pool.rs`

**Main binary:**
- [ ] Rewrite `src/main.rs` with plugin-based startup
- [ ] `cargo build` → full workspace compiles with zero warnings

**End-to-end test:**
- [ ] `cargo run -- examples/voltage_divider.ppr` → prints `Vmid = 2.5 V`

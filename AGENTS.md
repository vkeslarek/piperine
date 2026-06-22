# Piperine — Agent Instructions

This file briefs AI coding agents on the Piperine codebase. Read it before making changes.

## What Piperine is

A simulator frontend language: `.ppr` files describe analog circuits using `extern module` instances and drive simulation from `initial` blocks (SystemVerilog procedural). The backend is ngspice, connected via IPC. Verilog-A device models can be compiled to OSDI and loaded at runtime.

## Build and verify

Always build and run tests before declaring work done:

```sh
cargo build -p piperine-worker   # rebuild worker first
cargo build                       # build all
cargo test                        # must pass: 11 tests
```

If tests fail with "unexpected event" errors, the worker binary is stale — rebuild it.

## Codebase map

```
src/main.rs                              Entry point: parse → elaborate → run
crates/piperine-parser/src/
  ast/item.rs                            AST types: Module, ExternModule, Paramset, Instance, ExternParameter
  grammar/item.rs                        Parser: module, extern module, paramset, instance
  grammar/expr.rs                        Expression parser
crates/piperine-circuit/src/
  hardware.rs                            HardwareDefinition + HardwareInstance traits; ParameterDefinition
  elaboration.rs                         elaborate() → spice_lines; resolve_ref_params()
  paramset.rs                            Paramset expansion
crates/piperine-ngspice/src/
  hardware.rs                            All ~50 ngspice device implementations
  lib.rs                                 NgspicePlugin: register_hardware(), register_tasks()
  tasks.rs                               $op, $tran, $voltage, $current system tasks
  backend.rs                             NgspiceBackend: IPC to worker
  ppr/ngspice.ppr                        Bundled extern module declarations
crates/piperine-interpreter/src/
  interpreter.rs                         Procedural interpreter
  stdlib.rs                              $display, $fatal, $run_error system tasks
crates/piperine-coordinator/src/
  pool.rs                                ProcessPool: spawn/manage worker subprocesses
crates/piperine-worker/src/main.rs       Worker process: wraps libngspice
crates/piperine-openvaf/src/lib.rs       OpenVAF compiler wrapper
crates/piperine-common/src/lib.rs        IPC message types
```

## Core traits

### `HardwareDefinition` (piperine-circuit)

```rust
trait HardwareDefinition {
    fn name(&self) -> &str;                    // module name used in .ppr
    fn ports(&self) -> &[PortDefinition];      // return &[] for ngspice devices
    fn parameters(&self) -> &[ParameterDefinition]; // return &[] for ngspice devices
    fn instantiate(...) -> Result<Box<dyn HardwareInstance>, ElaborationError>;
    fn spice_model_type(&self) -> Option<&'static str> { None }   // "NMOS", "D", etc.
    fn spice_instance_prefix(&self) -> Option<char> { None }      // 'L' for inductors
}
```

### `HardwareInstance` (piperine-circuit)

```rust
trait HardwareInstance {
    fn instance_name(&self) -> &str;
    fn spice_lines(&self) -> Vec<String>;      // SPICE element line(s)
}
```

## Naming rules (critical — do not deviate)

| Thing | Convention | Example |
|-------|-----------|---------|
| Module name in `.ppr` | bare lowercase | `res`, `cap`, `nmos`, `jfet_n` |
| Rust struct | `Spice` + PascalCase | `SpiceResistor`, `SpiceNmos`, `SpiceJfetN` |
| `.name()` return value | same as module name | `"res"`, `"nmos"` |

No `spice_` prefix in module names. The prefix was removed — do not re-add it.

## Adding a new ngspice device

Follow this exact pattern:

**Step 1 — `ngspice.ppr`**:
```verilog
extern module mydev(
    inout p, inout n;
    parameter string model,
    parameter real value = 1.0
);
```

**Step 2 — `hardware.rs`**:
```rust
#[derive(Debug)]
pub struct SpiceMydev;
impl HardwareDefinition for SpiceMydev {
    fn name(&self) -> &str { "mydev" }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &[] }
    fn instantiate(&self, instance_name: &str, parameters: &ParameterMap,
                   connections: &ConnectionMap, _resolver: &dyn NetResolver)
        -> Result<Box<dyn HardwareInstance>, ElaborationError>
    {
        let p = require_net(connections, "p", instance_name)?.to_string();
        let n = require_net(connections, "n", instance_name)?.to_string();
        let model = require_string_parameter(parameters, "model", instance_name)?;
        let value = get_parameter_or(parameters, "value", 1.0);
        Ok(Box::new(SpiceMydevInstance { name: instance_name.to_string(), p, n, model, value }))
    }
}
#[derive(Debug)]
struct SpiceMydevInstance { name: String, p: String, n: String, model: String, value: f64 }
impl HardwareInstance for SpiceMydevInstance {
    fn instance_name(&self) -> &str { &self.name }
    fn spice_lines(&self) -> Vec<String> {
        let mut s = format!("{} {} {} {}", spice_name('X', &self.name), self.p, self.n, self.model);
        if self.value != 1.0 { s.push_str(&format!(" VALUE={}", self.value)); }
        vec![s]
    }
}
```

**Step 3 — `lib.rs`**:
```rust
registry.register(Box::new(SpiceMydev));
```

## ExternParameterKind variants

```rust
enum ExternParameterKind {
    Typed(Type),          // parameter real x, parameter string s, parameter integer n
    Expr,                 // parameter expr e  — raw AST (B-source behavioral expressions)
    Ref,                  // parameter ref l1  — resolves to sibling instance SPICE name
}
```

`Expr` and `Ref` are advanced features. Most devices use `Typed`.

## `mutual` inductor special case

`mutual` uses `parameter string inductor1, inductor2` — the user passes inductor instance names as strings. It does NOT use `parameter ref`. The SPICE line format is:
```
K<name> <inductor1> <inductor2> [coupling_factor]
```

## Parameter helpers (in `hardware.rs`)

```rust
require_net(connections, "p", instance_name)?         // required port
require_string_parameter(parameters, "model", ...)    // required string param
require_parameter(parameters, "r", ...)               // required real param
get_parameter_or(parameters, "tc1", 0.0)              // optional with default
get_string_parameter_or(parameters, "model", "")      // optional string
```

## Ground node

The elaborator converts net name `gnd` → SPICE node `"0"`. Device code never needs to handle this.

## `paramset` elaboration

```verilog
paramset nmos_svt nmos;
    .model("NMOS_SVT"), .w(1e-6), .l(180e-9);
endparamset
```

Elaborator emits `.model NMOS_SVT NMOS ...` + uses preset params for all instances of `nmos_svt`. Devices that have a `.model` line need `spice_model_type()` to return the SPICE model keyword.

## Documentation locations

- Language syntax: `docs/lang/`
- ngspice component reference: `docs/ngspice/`
- OpenVAF device models: `docs/openvaf/`
- Internal development notes: `docs/development/`
- Implementation recipe for devices: `docs/development/SPICE_COMPONENTS_IMPL.md`

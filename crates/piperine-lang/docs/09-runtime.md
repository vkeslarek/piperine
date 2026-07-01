# Runtime — IrProgram to CircuitInstance

The runtime module in `src/runtime/` builds a solver-ready `CircuitInstance` from an `IrProgram`.

## Entry point

```rust
pub fn from_ir(program: &IrProgram, top: &str) -> Result<CircuitInstance, String>
```

This is the main entry point. It takes the compiled `IrProgram` and the name
of the top-level module, and produces a fully wired `CircuitInstance` that the
solver can simulate.

## Node allocation strategy

Analog and digital nets are assigned unique identifiers via global atomics:

- `net_to_node: HashMap<String, NodeIdentifier>` — maps net names to analog
  nodes.
- `wire_to_dnet: HashMap<String, DigitalNet>` — maps wire names to digital
  nets.
- `NODE_CTR: AtomicUsize` and `DNET_CTR: AtomicUsize` — per-process atomic
  counters that generate unique IDs.

### Ground detection

Net names matching any of `gnd`, `GND`, `vss`, or `VSS` are assigned
`NodeIdentifier::Gnd`.  All other nets receive
`NodeIdentifier::Anonymous(counter)` with an incrementing counter.

### Connections

When a connection statement (`lhs = rhs`) is encountered, both sides resolve to
the same `NodeIdentifier`.  The `net_to_node` map ensures that any subsequent
reference to either name yields the shared node.

## Instance compilation

For each child instance in the top module:

1. **Module lookup.**  The child IR module is looked up by name in the
   program’s module map.

2. **Port mapping.**  Instance connections (`inst.connections`) are matched
   against the child module’s ports.  Both positional and named connections are
   supported:
   - Positional: matched by index into `child.ports`.
   - Named: matched by the `port` name field.
   A `terminal_list: Vec<NodeIdentifier>` is assembled in port order.

3. **Parameter resolution.**  Default parameter values are evaluated in
   declaration order.  Instance-level overrides are applied afterwards.
   `eval_ir_const()` performs best-effort compile-time evaluation.  If an
   expression cannot be reduced to a constant, it falls back to `0.0`.

4. **Analog compilation.**  `ir_analog_to_device()` lowers the analog body to
   an `Arc<JitAnalogDevice>`.  Errors are propagated with the instance label
   prepended.

5. **Digital compilation.**  `ir_digital_to_interp()` lowers the digital body
   to a `DigitalInterpreter`.  Port nets are assigned via `set_port_nets()`.
   Errors are propagated with the instance label.

6. **Device assembly.**  A `PhdlDevice` is created wrapping both the analog and
   digital components.  `dev.allocate_nodes()` is called to connect the
   terminal list to the solver’s netlist.

7. The assembled `Box<dyn Device>` is pushed into the `devices` vector.

After all instances are processed, the result is:

```rust
CircuitInstance::from_devices_and_netlist(top, devices, netlist)
```

## `eval_ir_const()`

Best-effort compile-time evaluation of `IrExpr` to `f64`.  Returns `Some(f64)`
on success, `None` for expressions that cannot be statically reduced.

| Category     | Supported constructs                                                    |
| ------------ | ----------------------------------------------------------------------- |
| Literals     | `Real`, `Int`, `Bool`                                                   |
| Lookups      | `Param` / `Var` resolved from the parameter environment; `inf` special case |
| Unary        | `Neg`, `Not`                                                            |
| Binary       | `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Pow`, `Eq`/`Ne`, `Lt`/`Le`/`Gt`/`Ge`, `And`/`Or` |
| Ternary      | `Select` (conditional evaluation)                                       |
| Calls        | `exp`, `ln`/`log`, `log10`, `sqrt`, `abs`, `sin`, `cos`, `tan`, `floor`, `ceil`, `pow`, `min`, `max` |
| Everything else | Falls back to `None` → caller uses `0.0`                             |

## `PhdlDevice`

`PhdlDevice` (`device.rs`) wraps analog and digital behavior into a single
implementation of the solver’s `Device` trait.

### Fields

| Field        | Type                              | Purpose                              |
| ------------ | --------------------------------- | ------------------------------------ |
| `name`       | `String`                          | Instance name                        |
| `analog`     | `Option<Arc<JitAnalogDevice>>`    | JIT-compiled analog behaviour        |
| `digital`    | `Option<DigitalInterpreter>`      | Interpreted digital behaviour        |
| `node_refs`  | `Vec<Option<AnalogReference>>`    | Terminal → solver node mapping       |
| `params`     | `Vec<f64>`                        | Resolved parameter values            |
| `sim_ctx`    | `SimCtx`                          | Temperature, time, mfactor, gmin     |

### Core methods

- **`allocate_nodes()`** — resolves terminal `NodeIdentifier`s to
  `AnalogReference`s via the solver netlist.
- **`collect_node_voltages()`** — reads current node voltages from the solver
  by index.
- **`eval_rhs_jac()`** — calls the JIT device’s `eval_residual()` and
  `eval_jacobian()`.
- **`norton_rhs()`** — performs the Norton transform:
  `-res[i] + sum_j(jac[i*n+j] * V[j])`.
- **`rhs_stamps()` / `jac_stamps_f64()` / `jac_stamps_complex()`** — convert
  internal vectors to the solver’s `Stamp` matrix format.

### Analysis methods

- **`load_analog_dc()`** — collects voltages, evaluates RHS + Jacobian, stamps
  entries.
- **`load_analog_ac()`** — same as DC but additionally adds the reactive
  contribution `jω·dQ/dV` from `eval_charge_jacobian()` to the Jacobian.
- **`load_analog_transient()`** — uses the Backward-Euler companion model. The
  reactive contribution adds `alpha·dQ/dV` to the Jacobian (where
  `alpha = 1/dt`), then applies the Norton transform.

### Device trait implementation

| Trait method               | Delegation                        |
| -------------------------- | --------------------------------- |
| `load_dc()`                | → `load_analog_dc()`              |
| `load_ac()`                | → `load_analog_ac()`              |
| `load_transient()`         | → `load_analog_transient()`       |
| `noise_current_psd()`      | returns empty `Vec`               |
| `digital_input_nets()`     | → `DigitalInterpreter`            |
| `digital_output_nets()`    | → `DigitalInterpreter`            |
| `digital_init()`           | → `DigitalInterpreter`            |
| `digital_state_size()`     | → `DigitalInterpreter`            |
| `eval_discrete()`          | → `DigitalInterpreter`            |

## `DigitalInterpreter`

`DigitalInterpreter` (`digital.rs`) is a tree-walking interpreter for PHDL
digital blocks.  It evaluates digital behaviour one event at a time, driven by
the solver’s event loop.

### Fields

| Field              | Type                                        | Purpose                                    |
| ------------------ | ------------------------------------------- | ------------------------------------------ |
| `body`             | `Vec<BehaviorStmt>`                         | Behaviour body to interpret                |
| `port_net_map`     | `HashMap<String, DigitalNet>`               | Port name → digital net index              |
| `input_port_names` | `Vec<String>`                               | Names of input ports                       |
| `output_port_names`| `Vec<String>`                               | Names of output ports                      |
| `input_nets`       | `Vec<DigitalNet>` (cached)                  | Digital nets for input ports               |
| `output_nets`      | `Vec<DigitalNet>` (cached)                  | Digital nets for output ports              |
| `prev_nets`        | `Vec<DigitalVal>`                           | Previous net values (for edge detection)   |
| `state`            | `HashMap<String, DigitalVal>`               | Variable state                             |
| `seq`              | `u64`                                       | Monotonic sequence number for event ordering |
| `device_id`        | `usize`                                     | Solver-assigned device index               |

### Key methods

- **`set_port_nets(net_map)`** — assigns `DigitalNet` indices to port names
  and caches the input/output net arrays.
- **`init()`** — executes `VarDecl` defaults via `eval_expr()`, runs `@initial`
  event blocks, and schedules zero-time output events.
- **`eval()`** — processes pending event blocks.  For each block, fires
  matching event specs (posedge/negedge/change) and executes the body via
  `exec_stmts()`.
- **`spec_fires(spec, nets, prev_nets)`** — checks whether an event spec
  triggers given the current and previous net states.  Supports:
  - `posedge` — transition from `0` to `1`
  - `negedge` — transition from `1` to `0`
  - `change` — any difference between `prev_nets` and `nets`
- **`exec_one(stmt)`** — executes a single `BehaviorStmt`:
  - `VarDecl` — evaluates the default expression and stores it in `state`.
  - `Bind(Force | Assign)` — evaluates the RHS.  If the target is a port net,
    schedules a `DigitalEvent` into the solver queue; otherwise stores in
    `state`.
  - `If` — evaluates the condition and executes the then-branch or else-branch.
  - `Match` — evaluates the discriminant, matches the pattern, executes the
    matching arm.
  - `Event` — nested event block; fires if the spec fires now.
- **`eval_expr(expr, state, nets)`** — evaluates a PHDL `Expr` to a
  `DigitalVal`.  Handles:
  - Literals (logic, integer, natural, real, bool)
  - Identifiers (lookup in `state`, then port nets)
  - Binary operations
  - Unary operations

## `DigitalVal` enum

```rust
enum DigitalVal {
    Logic(LogicValue),
    Natural(u64),
    Integer(i64),
    Real(f64),
    Bool(bool),
}
```

Coercion methods:
- `as_bool()` — returns `true` if non-zero / non-false.
- `as_logic()` — casts to `LogicValue` (zero → `0`, non-zero → `1`).

## Helper functions

- **`coerce(a, b)`** — coerces `Logic` to `Natural` for arithmetic.
- **`eval_binop(op, a, b)`** — type-dispatched binary operations with wrapping
  arithmetic for integer types.
- **`eval_unop(op, a)`** — negation and logical-not.

## Scan helpers

- **`scan_event_inputs(body)`** — walks the behaviour body to discover which
  port names appear as event arguments (inputs).
- **`scan_output_names(body)`** — walks the behaviour body to discover which
  port names appear as assignment targets (outputs).

## `compile_digital_module()`

Public entry point for digital compilation:

```rust
pub fn compile_digital_module(design: &Design, module_name: &str)
    -> Result<DigitalInterpreter, String>
```

Finds the digital `Behavior` in the named module, scans input/output port
names, and constructs a `DigitalInterpreter`.

## `ir_digital_to_interp()` — IR→Interpreter bridge

Located in `digital_lower.rs`.  Converts an `IrProgram` digital module into a
`DigitalInterpreter`.

Pipeline:

1. **Find the `IrModule`** by name, extract its `IrDigitalBody`.
2. **Inline user function calls** via `inline_user_calls()` (GAPS D.5).
3. **Validate** the inlined body via `validate_ir_digital_stmt()` (GAPS
   A.4/A.5).  Rejects unsupported operators:
   - `Pow`
   - `Shl`, `Shr`, `AShl`, `AShr`
   - Unary reductions: `BitNot`, `RedAnd`, `RedOr`, `RedXor`, `RedNand`,
     `RedNor`, `RedXnor`
4. **Lower** `IrStmt` nodes back to `BehaviorStmt` via `lower_stmts()` /
   `lower_stmt()`.
5. **Construct** a synthetic `Design` with a single `Module` containing the
   lowered behaviour.
6. **Call** `compile_digital_module()`.

### `lower_stmt()`

Converts `IrStmt` variants to `BehaviorStmt`:

| `IrStmt` variant    | `BehaviorStmt` equivalent       |
| ------------------- | ------------------------------- |
| `Assign`            | `Bind(Force, ...)`              |
| `NonBlocking`       | `Bind(Force, ...)` (same path)  |
| `AnalogEvent(...)`  | `Event` with `Named(name)` spec |
| `EventControl(...)` | `Event` with `Named(name)` spec |

### `ir_expr_to_phdl()`

Converts `IrExpr` back to PHDL `Expr` for the interpreter.  Handles:

- `Real`, `Int`, `Bool` literals
- `Param` / `Var` lookups
- `BranchAccess` (port/potential access)
- `Call` (function calls)
- `Binary` (with operator mapping table)
- `Unary` (negation, logical-not)

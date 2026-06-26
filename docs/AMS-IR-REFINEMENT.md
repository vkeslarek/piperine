# Piperine AMS Intermediate Representation (IR) Refinement

## 1. Introduction and Overview

Piperine is a Verilog-AMS frontend that lowers mixed-signal source code into
an executable form for `piperine-solver`. The compilation pipeline is:

```
.ppr / .va source
       │
       ▼
  piperine-parser   ← lexer + recursive-descent parser → AST + IrDesign
       │
       ▼
  Elaboration       ← symbol resolution, hierarchy flattening, param folding
       │
       ├──► Analog blocks ──► OpenVAF ──► .osdi .so ──► OsdiDevice (AnalogDevice)
       │
       ├──► Digital blocks ──► Rust codegen ──► Box<dyn DigitalDevice>
       │
       └──► Connect modules ──► A2DState / D2ADevice (built-in bridges)
              │
              ▼
         piperine-solver::CircuitInstance
              │ transient / op / AC
              ▼
         simulation results
```

The IR (`IrDesign` in `piperine-parser/src/ir/design.rs`) is the strongly-typed
in-memory intermediate form that decouples elaboration from solver construction.

---

## 2. Core Trait Architecture

The solver exposes two fundamental traits. Every compiled Verilog-AMS construct
maps to one or both of these:

```rust
// crates/piperine-solver/src/analog/device.rs
pub trait AnalogDevice {
    fn setup_model(&self, ...);
    fn setup_instance(&mut self, ...);
    fn bind_nodes(&mut self, ...);
    fn set_params(&mut self, ...);
    fn eval(&mut self, ...);
    fn load_residual_resist(&self, ...);
    fn load_jacobian_resist(&self, ...);
    // + reactive, noise variants
}

// crates/piperine-solver/src/digital/state.rs
pub trait DigitalDevice {
    fn has_input_on(&self, changed_nets: &HashSet<DigitalNet>) -> bool;
    fn eval(&mut self, t: f64, nets: &[LogicValue],
            queue: &mut BinaryHeap<Reverse<DigitalEvent>>);
    fn input_nets(&self)  -> &[DigitalNet] { &[] }
    fn output_nets(&self) -> &[DigitalNet] { &[] }
}
```

### Concrete implementations today

| Solver type | Implements | How created |
|-------------|-----------|-------------|
| `OsdiDevice` | `AnalogDevice` | Loads a compiled `.osdi` shared library (OpenVAF output) |
| `A2DState` | (standalone helper) | Analog → digital threshold detector |
| `D2ADevice` | `DigitalDevice` | Digital → analog ramp driver |
| Test devices (`Inverter`, `DFF`, `NorGate`, ...) | `DigitalDevice` | Pure Rust structs in tests |

### Future: compiled Verilog-AMS digital blocks

When we compile a Verilog-AMS `always`/`initial` block, the codegen will emit
a Rust struct `struct FooDigital { ... }` that implements `DigitalDevice`.  
There is **no FFI layer** — the trait IS the ABI. This is the reason the
`DigitalDevice` trait exists and is kept.

---

## 3. What the IR Must Represent

### 3.1 Module splitting (mixed-signal abstraction)

A single Verilog-AMS `module` may contain:
- `analog` blocks → analog behavior (→ `OsdiDevice`)
- `initial`/`always` blocks → digital behavior (→ `Box<dyn DigitalDevice>`)
- Both → must be split into TWO runtime objects + implicit connect modules

The IR expresses this **before** compilation, in three lists:

```rust
// piperine-parser/src/ir/design.rs
pub struct IrDesign {
    pub nets:             HashMap<NodeId, IrNet>,
    pub analog_instances: Vec<AnalogIrInstance>,   // → OsdiDevice
    pub digital_instances:Vec<DigitalIrInstance>,  // → Box<dyn DigitalDevice>
    pub connect_instances:Vec<ConnectIrInstance>,  // → A2DState or D2ADevice
}
```

A mixed-signal `module foo(inout electrical in, output logic out)` that
reads an analog voltage and drives a digital output splits into:

```
AnalogIrInstance "foo_analog" { terminals: [in_node] }
ConnectIrInstance A2D          { analog_port: in_node, digital_port: virt_a2d_0 }
DigitalIrInstance "foo_digital"{ terminals: [virt_a2d_0, out_node] }
```

### 3.2 Net domain resolution

Each `IrNet` carries a `Domain`:
- `Domain::Analog` — drives/is driven by KCL/KVL equations (OSDI matrix stamps)
- `Domain::Digital` — carries `LogicValue` (Zero/One/X/Z), lives in `DigitalState`
- Mixed ports get two `NodeId`s (one per domain) with an implicit connect module

```rust
pub struct IrNet {
    pub id:            NodeId,
    pub original_path: String,
    pub domain:        Domain,
}
```

### 3.3 Parameter folding

By the time an `AnalogIrInstance` or `DigitalIrInstance` reaches the solver,
all parameters are reduced to `(String, f64)` or `(String, String)` pairs.
No expressions, no `defparam` references, no `paramset` indirections — those
are all resolved during elaboration (the High-IR → Flat-IR pass).

---

## 4. IR Lifecycle

```
1. Parse (.ppr → AST)
   │
2. Symbol scan (High-IR)
   │  Build symbol table: disciplines, natures, module types, params
   │
3. Elaboration (High-IR → Flat-IR = IrDesign)
   │
   │  For each module instantiation (top-down, DFS):
   │    a. Substitute parameters (lexical inheritance + defparam)
   │    b. Constant-fold param exprs → f64 / String
   │    c. Map logical port names → globally unique NodeId
   │    d. Classify module as Analog | Digital | Mixed
   │    e. If Mixed: split → AnalogIrInstance + DigitalIrInstance
   │    f. Insert ConnectIrInstance at every cross-domain port
   │       (using active `connectrules` or defaults: A2D threshold=0.9V,
   │        D2A v_high=1.8V v_low=0V rise_time=100ps)
   │    g. If `paramset`: expand base module with preset params
   │    h. If `generate`: evaluate conditions, expand chosen branch
   │
4. Compilation targets extraction
   │
   │  Analog:  collect unique analog-block source text per AnalogIrInstance.model_name
   │           → call OpenVAF: `openvaf <module>.va -o <module>.osdi`
   │           → AnalogModel::load(&osdi_path) → OsdiDevice::new_with_params(...)
   │
   │  Digital: collect unique digital-block source per DigitalIrInstance.model_name
   │           → emit Rust struct + DigitalDevice impl (codegen — Phase 2 work)
   │
5. Solver instantiation (Circuit builder)
   │
   │  let mut circuit = Circuit::new("top");
   │  for ai in design.analog_instances:
   │      circuit.components_mut().insert(
   │          ai.instance_name.clone(),
   │          OsdiDevice::new_with_params(
   │              ai.instance_name, model.lib.clone(), model.descriptor_idx,
   │              ai.terminals.iter().map(node_to_port).collect(),
   │              ai.parameters.iter().collect(),
   │          ),
   │      );
   │
   │  let mut instance = CircuitInstance::instantiate(&circuit)?;
   │  for di in design.digital_instances:
   │      instance.digital_runtimes.push(compiled_digital(di));
   │  for ci in design.connect_instances:
   │      match ci.connect_type.as_str() {
   │          "A2D" => { /* hook into transient solver callbacks */ }
   │          "D2A" => instance.digital_runtimes.push(Box::new(
   │              D2ADevice::new(DigitalNet(ci.digital_port.0)))),
   │          _ => {}
   │      }
   │
   │  instance.rebuild_digital_topology();
   │  instance.transient(options, context)?.solve()
```

---

## 5. Connect Module Rules

### 5.1 Default rules (no explicit `connectrules` declaration)

| Crossing direction | Inserted module | Parameters |
|-------------------|-----------------|-----------|
| Analog → Digital  | `A2DState` | `threshold=0.9`, `hysteresis=0.0` |
| Digital → Analog  | `D2ADevice` | `v_high=1.8`, `v_low=0.0`, `rise_time=100e-12` |

### 5.2 Explicit `connectrules`

```verilog
connectrules cmos18;
    connect module my_a2d merged;   // custom A2D
    connect logic, electrical resolveto logic;
endconnectrules
```

The `ConnectrulesDecl` AST node is parsed. Elaboration must look up the named
connect module, instantiate it as a `ConnectIrInstance`, and apply discipline
resolution rules when two disciplines meet at a port.

### 5.3 Cross-domain boundary detection

A port boundary is cross-domain when:
- An `electrical` net connects to a `logic`/`wire` port
- A `wreal` net (continuous-time digital value) connects to a discrete port
- The module's analog block uses `V(dig_port)` or `I(dig_port)` directly

---

## 6. Analog Block Compilation (OpenVAF path)

Each unique analog-behavior module becomes a Verilog-A source fragment compiled
by OpenVAF into an OSDI `.so`:

```
module foo_analog(inout electrical a, b);
    parameter real r = 1k;
    analog begin
        I(a, b) <+ V(a, b) / r;
    end
endmodule
```

↓ `openvaf foo_analog.va -o foo_analog.osdi`

↓ `AnalogModel::load("foo_analog.osdi")`

↓ `OsdiDevice::new_with_params("R1", model.lib, idx, [node_a, node_b], [("r", 1000.0)])`

OSDI parameter types supported:
- `PARA_TY_REAL` → written as `f64`
- `PARA_TY_INT`  → written as `i32` (supported; was previously dropped)
- String params  → carried in `str_parameters: HashMap<String, String>`

---

## 7. Digital Block Compilation (Rust codegen path)

Each unique digital-behavior module becomes a Rust struct + `DigitalDevice` impl.
The codegen is NOT implemented yet; this section specifies what it must produce.

### Input: DigitalIrInstance

```rust
DigitalIrInstance {
    instance_name: "U1_digital",
    model_name: "my_inverter",
    terminals: [NodeId(3), NodeId(4)],  // [in, out] DigitalNet indices
    parameters: {},
    str_parameters: {},
}
```

### Output: generated Rust code

```rust
struct MyInverter {
    input: DigitalNet,
    output: DigitalNet,
    delay: f64,
}

impl DigitalDevice for MyInverter {
    fn has_input_on(&self, c: &HashSet<DigitalNet>) -> bool {
        c.contains(&self.input)
    }
    fn eval(&mut self, t: f64, nets: &[LogicValue],
            q: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        let out = match nets[self.input.0] {
            LogicValue::Zero => LogicValue::One,
            LogicValue::One  => LogicValue::Zero,
            _                => LogicValue::X,
        };
        q.push(Reverse(DigitalEvent {
            time: t + self.delay,
            net: self.output,
            value: out,
            source: 0,
            seq: 0,
        }));
    }
    fn input_nets(&self)  -> &[DigitalNet] { std::slice::from_ref(&self.input) }
    fn output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.output) }
}
```

### Verilog-AMS → DigitalDevice mapping

| Verilog-AMS construct | Generated Rust |
|----------------------|---------------|
| `input logic clk`    | `DigitalNet` field, listed in `input_nets()` |
| `output logic q`     | `DigitalNet` field, listed in `output_nets()` |
| `inout logic bus`    | Listed in both `input_nets()` and `output_nets()` |
| `always @(posedge clk)` | Posedge detection in `eval()` |
| `#delay` after `<=`  | `q.push(... t + delay ...)` |
| `@(cross(V(a)-thr))` | Calls `A2DState::check_crossing` in cosim bridge |
| `$display(...)` | Translated to `eprintln!` or tracing macro |
| Variable declarations | Struct fields |

---

## 8. Topology and DAG Scheduling

Once `CircuitInstance` is built:

```rust
instance.rebuild_digital_topology();
```

This calls `DigitalTopology::build(&instance.digital_runtimes)` which:
1. Builds a dependency graph from `input_nets()` / `output_nets()`
2. Performs DFS topological sort
3. Identifies back edges (combinational cycles, ring oscillators, latches)

The transient solver uses `evaluate_dag_ordered` (Verilator-style):
- One forward pass in topo order; zero-delay events propagate inline
- Back edges trigger restart from earliest affected position
- 1000-iter cap for non-converging cycles

This is critical for correct RS-latch, ring oscillator, and pipelined DFF
simulation.

---

## 9. IrDesign → Circuit Builder (Reference Code)

```rust
pub fn build_circuit(design: &IrDesign, models: &HashMap<String, AnalogModel>)
    -> Result<CircuitInstance, BuildError>
{
    let mut circuit = Circuit::new("top");

    // 1. Analog instances
    for ai in &design.analog_instances {
        let model = models.get(&ai.model_name).ok_or(BuildError::ModelNotFound)?;
        let terminals: Vec<_> = ai.terminals.iter()
            .map(|id| node_to_port(design, *id))
            .collect();
        let params: Vec<_> = ai.parameters.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        circuit.components_mut().insert(
            ai.instance_name.clone(),
            OsdiDevice::new_with_params(
                ai.instance_name.clone(),
                model.lib.clone(),
                model.descriptor_idx,
                terminals,
                params,
            ),
        );
    }

    let mut instance = CircuitInstance::instantiate(&circuit)?;

    // 2. Digital instances (generated Rust structs)
    for di in &design.digital_instances {
        let dev = instantiate_digital(di)?;   // codegen produces this fn
        instance.digital_runtimes.push(dev);
    }

    // 3. Connect modules
    for ci in &design.connect_instances {
        match ci.connect_type.as_str() {
            "D2A" => {
                let net = DigitalNet(ci.digital_port.0);
                instance.digital_runtimes.push(Box::new(D2ADevice::new(net)));
            }
            "A2D" => {
                // A2DState is driven from the transient solver's post-step hook
                // (not a DigitalDevice); register it in the cosim bridge table
                instance.register_a2d(ci.analog_port, ci.digital_port);
            }
            _ => return Err(BuildError::UnknownConnectType),
        }
    }

    // 4. DAG topology
    instance.rebuild_digital_topology();
    Ok(instance)
}
```

---

## 10. Verilog-AMS Parser Coverage

### Fully covered ✓

| Feature | AST node |
|---------|---------|
| `discipline` / `nature` | `DisciplineDecl`, `NatureDecl` |
| `module` / `macromodule` / `connectmodule` | `ModuleDecl` |
| `analog` / `analog initial` | `AnalogBehaviour` |
| `initial` / `always` | `ModuleItem::InitialConstruct/AlwaysConstruct` |
| `parameter` / `localparam` / `aliasparam` | `ParamDecl`, `AliasParam` |
| `branch` / `ground` / `event` | `BranchDecl`, `GroundDecl`, `EventDecl` |
| Port directions (`input`/`output`/`inout`) | `PortDecl` |
| Net types (`wire`, `wand`, `wor`, `wreal`, ...) | `NetType` |
| `paramset` / `endparamset` | `ParamsetDecl` |
| `connectrules` / `endconnectrules` | `ConnectrulesDecl` |
| `generate` (region, for, if, case) | `GenerateRegion`, `LoopGenerate`, ... |
| Module instantiation with `#(params)` | `ModuleInstantiation` |
| `defparam` | `DefparamDecl` |
| `assign` (continuous) | `ContinuousAssign` |
| Contribution `<+` | `AssignOp::Contrib` |
| Indirect contribution `V(a,b) : I(a,b) == expr` | `IndirectContribution` |
| Non-blocking assign `<=` | `NonBlockingAssignStmt` |
| Event trigger `->` | `EventTriggerStmt` |
| `fork`/`join` | `ForkStmt` |
| `wait`, `disable` | `WaitStmt`, `DisableStmt` |
| `repeat`, `forever` | `RepeatStmt`, `ForeverStmt` |
| `case`/`casex`/`casez` | `CaseStmt { kind: CaseKind }` |
| Wildcard port connections `.*` | `PortConnection::Wildcard` |
| Gate instantiations (`and`, `nand`, ...) | `GateInstantiation` |
| `specparam` | `SpecparamDecl` |
| System functions (`$sin`, `$ln`, `$display`, ...) | `FunctionRef::SysFun` |
| Analog event functions (`cross`, `above`, `timer`) | `EventExpr::AnalogEventFn` |
| `inside` set membership | `Expr::Binary` (desugared) |
| Ternary `? :`, concat `{}`, replicate `{{}}` | `Expr::Select/Concat/Replicate` |
| Part-select `[msb:lsb]`, `[+:w]`, `[-:w]` | `Expr::PartSelect*` |

### Stubs (parsed but AST carries no structure) ⚠

| Feature | Status |
|---------|--------|
| `specify`/`endspecify` | `SpecifyBlock { span }` — body skipped |
| `primitive` UDP declarations | `PrimitiveDecl { span }` — body skipped |
| `config`/`endconfig` | `ConfigDecl { span }` — body skipped |
| `connectrules` param overrides in `connect module` | Params not parsed |

These constructs are rare in AMS behavioral modeling. Specify blocks define
timing constraints only relevant to gate-level netlists; primitive UDPs and
configs are structural SV features not needed for AMS simulation.

---

## 11. Open Questions and Next Steps

### 11.1 Resolved from previous doc version

- **DOSDI is gone.** Digital devices implement `DigitalDevice` directly in Rust.
  No C ABI, no dynamic loading for digital. FFI infrastructure (`FfiDigitalDevice`)
  remains for possible future compiled-from-hardware digital blocks but is not
  the primary path.

- **Cross-domain virtual nets** use a simple counter to allocate fresh `NodeId`s
  that fall outside the analog matrix (no index in `CircuitInstance::nodes`).
  They only live in `DigitalState::nets`.

- **Hierarchy mapping** for probing/debugging: the `IrNet::original_path` field
  carries the hierarchical path string (e.g., `"top.u1.q"`), which survives
  flattening and can be used for waveform output.

### 11.2 Next implementation priorities

1. **Elaborator** (`piperine-parser/src/ir/elaborator.rs` — to be created):
   - Top-down module instantiation walk
   - Symbol table (`HashMap<String, ModuleKind>`)
   - Parameter folding (constant evaluator over `Expr`)
   - Module domain classification (scan for `analog` vs `always` blocks)
   - Net splitting and `ConnectIrInstance` insertion

2. **Digital codegen** (`piperine-solver` / code emitter):
   - Walk `DigitalIrInstance.model_name` → find `ModuleDecl` in AST
   - Emit `struct` + `DigitalDevice impl` for `initial`/`always` blocks
   - Map `posedge`/`negedge` event controls to clock-edge detection
   - Map `#delay` to `DigitalEvent { time: t + delay }` scheduling

3. **Cosim A2D bridge**:
   - Register `A2DState` callbacks in the transient solver's post-Newton hook
   - After each accepted time step, call `a2d.check_crossing(v_prev, v_now, t_prev, t)`
   - If `Some((t_event, val))`: inject `DigitalEvent` into `DigitalState::event_queue`

4. **Simulation control task lowering** (`$tran`, `$op`, `$voltage`, `$current`):
   - These already work via the interpreter in `piperine-interpreter`
   - For compiled AMS modules, lower to direct solver API calls instead

### 11.3 What should NOT change

- `DigitalDevice` and `AnalogDevice` traits: frozen. All codegen targets these.
- `IrDesign` structure: add fields but don't rename existing ones.
- `OsdiDevice` as the sole `AnalogDevice` implementation: OpenVAF is the one
  true analog compiler.
- The DAG scheduler and back-edge handling: correct, tested, don't touch.

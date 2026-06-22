# Phase 4 — Circuit Introspection & In-Run Control

Implementation-ready refinement of [ROADMAP.md](ROADMAP.md) Phase 4. Each feature
below is self-contained: rationale, the exact Piperine surface, the AST / parser /
interpreter / backend changes, the netlist emission, and a test plan. Written so it
can be implemented step by step without design decisions left open.

The headline: read a circuit's *internal* state (`M1.gm`, `D1.id`) and change it
between runs (`$alter`, options, temperature). This is what turns a testbench from
"stimulate and probe outputs" into "characterize the device."

---

## 0. Syntax decision: `inst.gm`, not `@M1[gm]`

`@<instance>[<param>]` (e.g. `@M1[gm]`) is **ngspice / Nutmeg** expression syntax —
a SPICE simulator convention used in `.control` scripts, `print`, `let`, `.meas`.
It is **not** Verilog-A, **not** Verilog-AMS, **not** SystemVerilog. SystemVerilog
accesses sub-objects with a dotted hierarchical name (`m1.gm`); that is the model
we follow.

**Decision:** the Piperine surface is object access — `M1.gm()` (method) and
`M1.gm` (property sugar). The `@M1[gm]` string is an implementation detail produced
only when querying ngspice; it never appears in `.ppr` source.

```verilog
// Piperine (what the user writes)
$op();
real g  = M1.gm();      // method form
real id = M1.id;        // property sugar (same thing, no parens)

// ngspice (what the backend generates, hidden)
//   .save @M1[gm] @M1[id]
//   ... after op ... read vectors @M1[gm], @M1[id]
```

Rationale: consistent with the rest of Piperine's object surface (`result.signal()`,
`df["x"]`, `q.push_back()`), and it keeps SPICE-isms out of the language.

---

## 1. Device handles + operating-point access  *(headline feature)*

### 1.1 What the user writes

Every instantiated device is in scope as a handle named exactly as instantiated.
After an analysis, its operating-point parameters are readable:

```verilog
module tb;
    nmos #(.model("nm"), .w(1e-6), .l(100e-9)) M1(.d(out), .g(in), .s(gnd), .b(gnd));
    res  #(.r(1e3)) R1(.p(out), .n(gnd));

    initial begin
        $op();
        $display("gm = %e  id = %e  vth = %e", M1.gm(), M1.id, M1.vth);
        $display("R1 power = %e", R1.p);     // @R1[p]
    end
endmodule
```

`M1.<param>()` and `M1.<param>` both return the last value of the ngspice vector
`@M1[<param>]` (a `real`). Any param name ngspice accepts for that device works
(`gm`, `gds`, `id`, `vth`, `vdsat`, `cgs`, … for a MOSFET; `gm`, `ic`, `ib`, `vbe`
for a BJT; `i`, `p` for R; `gd`, `id`, `vd` for a diode). Piperine does not need a
per-device whitelist — it forwards the name and surfaces ngspice's error if invalid.

### 1.2 `DeviceHandle` — new ExternClass

`crates/piperine-interpreter/src/extern_types.rs`:

```rust
/// A handle to an elaborated circuit instance. Method/field access reads the
/// device's operating-point parameter `@<name>[<param>]` from the simulator.
#[derive(Debug)]
pub struct DeviceHandle {
    pub name: String,   // SPICE instance name, e.g. "M1" / "RR1"
}

impl DeviceHandle {
    pub fn new(name: String) -> Value {
        Value::ExternObject(Arc::new(Self { name }))
    }
}
```

It cannot reach the simulator from `call_method` (that signature has no backend).
So operating-point reads go through the interpreter, not `ExternClass`. Two options;
**use option B**:

- **Option A (rejected):** give `ExternClass::call_method` a `&mut dyn
  SimulatorBackend`. Too invasive — touches every ExternClass.
- **Option B (chosen):** the interpreter special-cases method/property access whose
  receiver is a `DeviceHandle`, because the interpreter *does* hold the backend.

So `DeviceHandle::call_method` only needs to error (it should never be reached); the
real work is in the interpreter (§1.4).

### 1.3 Seeding device handles into scope

`ElaborationResult` must carry the instance names so the interpreter can bind them.

`crates/piperine-circuit/src/elaboration.rs`:

```rust
pub struct ElaborationResult {
    pub spice_lines: Vec<String>,
    pub initial_statement: ast::Stmt,
    pub always_handlers: AlwaysHandlerSet,
    pub functions: Vec<piperine_parser::model::Function>,
    pub instances: Vec<String>,   // NEW: SPICE instance names, e.g. ["M1","RR1",...]
}
```

Populate `instances` during `elaborate_instances` — push the **final SPICE name**
(the same string `spice_name(prefix, name)` produces, e.g. `R1` → `RR1`, `M1` →
`M1`). The interpreter binds each as a `DeviceHandle`.

`crates/piperine-interpreter/src/interpreter.rs`:

```rust
// new field + setter, mirroring set_functions
devices: Vec<String>,

pub fn set_devices(&mut self, names: Vec<String>) { self.devices = names; }

// in exec(), before running the body, seed handles into the scope:
pub fn exec(&mut self, statement: &Stmt, scope: &mut Scope) -> Result<(), InterpreterError> {
    for name in &self.devices.clone() {
        scope.set(name, crate::extern_types::DeviceHandle::new(name.clone()));
    }
    self.eval_statement(statement, scope)?;
    Ok(())
}
```

`src/main.rs`: `interpreter.set_devices(elaboration.instances);` next to
`set_functions(...)`. Tests that exercise device access seed similarly.

> Name-collision note: if a device is named the same as a user variable, the
> variable assignment wins (it overwrites the handle in the flat scope). Acceptable;
> document it.

### 1.4 Interpreter: resolve `M1.gm()` / `M1.gm`

Method form — `M1.gm()` is `Expr::Call(FunctionRef::Path(["M1","gm"]), [])`. The
existing call dispatch already splits `"M1.gm"` into receiver `M1` + method `gm` and
looks up an `ExternObject`. Add a branch: when the receiver is a `DeviceHandle`,
query the operating point instead of calling `ExternClass::call_method`.

```rust
// in Expr::Call, FunctionRef::Path arm, before generic ExternObject dispatch:
if let Some((recv, param)) = path_str.split_once('.') {
    if let Some(Value::ExternObject(obj)) = scope.get(recv) {
        if obj.type_name() == "Device" {
            return self.read_op_param(recv, param);   // §1.5
        }
    }
}
```

Property form — `M1.gm` (no parens) is `Expr::Path(["M1","gm"])`. In the
`Expr::Path` arm, before the "undefined variable" error, add:

```rust
if let Some((recv, param)) = name.split_once('.') {
    if let Some(Value::ExternObject(obj)) = scope.get(recv) {
        if obj.type_name() == "Device" {
            return self.read_op_param(recv, param);
        }
    }
}
```

Both call one helper:

```rust
fn read_op_param(&mut self, inst: &str, param: &str) -> Result<Value, InterpreterError> {
    let vec = format!("@{inst}[{param}]");
    let data = self.simulator.get_vector(&vec)?;
    let last = data.last().copied().ok_or_else(|| {
        InterpreterError::SimulatorError(format!("operating-point vector {vec} is empty"))
    })?;
    Ok(Value::Real(last))
}
```

### 1.5 Make `@inst[param]` available in ngspice (auto-save)

ngspice does **not** save internal device params (`gm`, `vth`, …) by default — only
node voltages and source branch currents. To read `@M1[gm]`, the deck must contain
`.save @M1[gm]` (or `.save all` won't include it). Accessing happens at runtime but
saves must be in the deck before the run, so resolve it at **elaboration** with a
static pass:

1. Walk the `initial_statement` and every `always` body AST.
2. Collect every `(inst, param)` where `inst` is a known device name and the node is
   either `Expr::Call(Path([inst,param]), [])` or `Expr::Path([inst,param])`.
3. For each, emit one `.save @<inst>[<param>]` line into `spice_lines` (before
   `.end`). De-duplicate.

`crates/piperine-circuit/src/elaboration.rs` — a new `collect_op_saves(stmt,
&device_names) -> Vec<String>` AST walker; append results to `spice_lines`. This
keeps `M1.gm` "just works" with no user-side `.save`.

> Fallback if a param is accessed dynamically (name built at runtime, not statically
> visible): document that such access requires an explicit `$save_op("M1","gm")`
> task (thin wrapper emitting the save before the next analysis). Optional; ship the
> static pass first.

### 1.6 Tests

`tests/e2e_phase4_test.rs` (MockBackend returns a known vector for `@M1[gm]`):
- `M1.gm()` returns the mocked last value.
- `M1.gm` (property) returns the same.
- `.save @M1[gm]` appears in `elaboration.spice_lines` when `M1.gm` is referenced.
- Unknown device `X9.gm()` → error "unknown function/undefined".

---

## 2. Model parameter access

Read `.model` card values: `nm.vth0`, `nm.tox`. Same mechanism as §1 but the ngspice
vector form is `@<model>[<param>]`. A `ModelHandle` ExternClass + seed model names
(from `paramset` / `.model` emission) into scope, dispatched by `type_name() ==
"Model"`. Identical `read_op_param`-style helper (the `@m[p]` form is the same). Lower
priority than §1; spec mirrors §1.

```verilog
real vt = nm.vth0;     // @nm[vth0]
```

---

## 3. `$alter` / `$altermod` / `$alterparam` — change params between runs

Re-run with a changed instance/model/global parameter **without re-elaborating** the
whole circuit — the core of fast parametric and optimization loops.

```verilog
initial begin
    real best = 1e30;
    for (real rl = 1e3; rl <= 10e3; rl += 1e3) begin
        $alter("RL", "resistance", rl);   // change R "RL" value
        TranResult t = $tran(1e-9, 1e-6);
        real e = t.signal("v(out)").rms();
        if (e < best) best = e;
    end
end
```

System tasks (`crates/piperine-ngspice/src/tasks.rs`):

| Task | ngspice command emitted |
|------|--------------------------|
| `$alter(inst, param, value)` | `alter @<inst>[<param>] = <value>` (or `alter <inst> <param>=<value>`) |
| `$altermod(model, param, value)` | `altermod @<model>[<param>] = <value>` |
| `$alterparam(param, value)` | `alterparam <param>=<value>` then `reset` |

Each is a `SystemTask` that builds the command string and calls
`simulator.run_command(&cmd)`. Args: string inst/model/param + real/string value.
Return `Void`. No parser changes.

> ngspice note: `alterparam` requires a following `reset` to take effect; the task
> issues both. `alter`/`altermod` apply immediately to the next analysis.

Tests: assert the exact command string reaches a recording MockBackend.

---

## 4. Solver options — `$set_option`

Expose `.options` (`reltol`, `abstol`, `method`, `gmin`, `itl1`, `maxord`, …).

```verilog
$set_option("reltol", 1e-4);
$set_option("method", "gear");
$set_option("gmin", 1e-13);
```

Implementation choice — **emit into the deck at elaboration is impossible** (these
are set before load) so use the runtime command: `$set_option(key, val)` issues
`option <key>=<val>` via `run_command`. For string values emit `option method=gear`;
for reals `option reltol=0.0001`. A `SystemTask`; no parser change.

> If a value must be set before the first analysis, `option` issued at the top of the
> `initial` block (before any `$op/$tran`) is sufficient — it applies to subsequent
> runs. Document that ordering.

Reference list of keys + defaults: `NGSPICE_NETLIST.md §.options`.

---

## 5. Temperature — `$set_temp` and sweep

```verilog
$set_temp(85.0);                 // option temp=85
TranResult hot = $tran(1e-9, 1e-6);

for (real tc = -40.0; tc <= 125.0; tc += 25.0) begin
    $set_temp(tc);
    $op();
    $display("T=%g  Vbe=%e", tc, Q1.vbe);
end
```

`$set_temp(t)` → `run_command("option temp=<t>")`. Also expose `$set_tnom(t)` →
`option tnom=<t>`. Trivial `SystemTask`s. (`NGSPICE_NETLIST.md §.temp`.)

---

## 6. Initial conditions / node hints — `$set_ic`, `$nodeset`

```verilog
$set_ic("cap_node", 1.8);        // .ic v(cap_node)=1.8 behavior
$nodeset("internal", 0.9);       // convergence hint
```

These influence the *next* analysis. ngspice has both deck cards (`.ic`, `.nodeset`)
and the runtime is limited, so implement by **emitting deck cards at elaboration**
when called from a position before the first analysis, OR via `run_command`. Chosen:
provide `$set_ic`/`$nodeset` as tasks that issue the runtime equivalent
(`run_command("ic v(node)=val")` style) where supported; otherwise document that for
guaranteed effect, ICs belong on the device (`.ic` instance param, already supported
via device `ic` parameters). Keep this feature minimal; lean on existing device `ic`
params first.

---

## 7. Physical constants

Predefined read-only identifiers usable in any expression. No `$` — they read like
constants.

```verilog
real vt = BOLTZMANN * 300.0 / ECHARGE;   // thermal voltage
real w  = 2.0 * M_PI * freq;
```

Implementation: in the interpreter's `Expr::Path` resolution, before the
undefined-variable error, check a constant table:

```rust
fn builtin_constant(name: &str) -> Option<f64> {
    Some(match name {
        "M_PI"      => std::f64::consts::PI,
        "M_TWO_PI"  => std::f64::consts::TAU,
        "M_E"       => std::f64::consts::E,
        "BOLTZMANN" | "P_K"  => 1.380649e-23,
        "ECHARGE"   | "P_Q"  => 1.602176634e-19,
        "P_CELSIUS0"=> 273.15,
        "P_EPS0"    => 8.8541878128e-12,
        "P_U0"      => 1.25663706212e-6,
        "P_H"       => 6.62607015e-34,
        "P_C"       => 299792458.0,
        _ => return None,
    })
}
```

Resolve to `Value::Real`. These names match Verilog-A's `constants.vams` (`P_Q`,
`P_K`, …) plus the common `M_PI` set, so they read naturally to analog engineers.
No parser change. Tests: `M_PI` evaluates; `BOLTZMANN*300/ECHARGE ≈ 0.02585`.

---

## 8. Full vector retrieval + differential probes

`$V`/`$I` return only the *last* sample. Add whole-vector and differential access so
post-processing (and DataFrames, see [DATAFRAME.md](DATAFRAME.md)) have the data.

```verilog
real[] vout = $get_vec("v(out)");    // entire sweep as an array
real   vd   = $V("a", "b");          // differential v(a)-v(b), last sample
```

- `$get_vec(name)` → `SystemTask` calling `simulator.get_vector(name)` and wrapping
  the whole `Vec<f64>` as a Piperine array (`ArrayObj::new(values.map(Value::Real))`
  or `Value::RealVec`). Returns the full series.
- `$V(a, b)` two-arg form → fetch `v(a)` and `v(b)`, subtract last samples. Extend the
  existing `VoltageTask` to accept an optional second node.

(`NGSPICE_CONTROL.md §let/print`, `NGSPICE_EXPRESSIONS.md`.)

---

## Parser changes summary

Almost none — that's by design (object access reuses existing call/path syntax):

| Feature | Parser change |
|---------|---------------|
| `M1.gm()` (method) | none — existing `Expr::Call(Path)` |
| `M1.gm` (property) | none — existing `Expr::Path`; handled in interpreter |
| `$alter`, `$set_option`, `$set_temp`, `$get_vec`, … | none — system tasks |
| Physical constants | none — interpreter path resolution |

The only non-trivial work is the **elaboration auto-save AST walk** (§1.5) and the
two interpreter dispatch branches (§1.4, §7) — all in Rust, no grammar edits.

---

## Implementation order

1. **§1 device handles + operating-point access** — `DeviceHandle`, scope seeding,
   interpreter dispatch (method + property), auto-save AST pass. Tests. *(headline)*
2. **§7 physical constants** — tiny, unblocks realistic expressions.
3. **§3 `$alter` family** + **§4 `$set_option`** + **§5 `$set_temp`** — parametric /
   sweep workflow; all thin `run_command` tasks.
4. **§8 `$get_vec` + differential `$V`** — feeds DataFrame and post-processing.
5. **§2 model params**, **§6 ic/nodeset** — lower priority, mirror §1 / lean on
   existing device params.

Each step is independently shippable and testable with a recording/mock backend.
Done when a testbench can sweep a parameter, read `M1.gm`/`M1.id` per step, and pick
an optimum — entirely in object-access syntax, with no `@dev[param]` in sight.

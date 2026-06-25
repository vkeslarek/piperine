# Piperine Mixed-Signal Simulation — Development Specification

> **Purpose:** Complete, unambiguous development specification for mixed-signal simulation in
> Piperine. Written so that an implementer with zero prior context can build the system from
> these instructions alone. Every design decision is explained with rationale.

---

## 1. Core Principle: One Simulator, Not Two

There is no "cosimulation." There is **one simulator** that understands two kinds of variables:
**continuous** (analog) and **discrete** (digital). The transient loop is a single unified loop
that contains both a Newton-Raphson matrix solver and a discrete event queue as internal mechanisms.

**Design Decision: No separate coordinator, no separate engines.**

The `TransientSolver` itself is extended to manage both continuous integration and discrete events
as part of its own state. The analog matrix and the digital event queue are peer data structures
inside a single simulation state.

**Rationale:** In Verilog-AMS, a module's ports are just ports. The discipline (`electrical`,
`logic`) determines behavior, but the port identity is unified. A signal named `clk` is `clk`
whether it carries voltage or logic. Our architecture must reflect this: one namespace, one time
authority, one simulation. A `CosimCoordinator` mediating between two foreign engines would be the
wrong abstraction — it implies two separate programs being glued together rather than one program
that naturally handles both domains.

---

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Continuous variable** | A variable solved by Newton-Raphson at every timestep. Voltages, currents, charges. Currently represented by `AnalogVariable` in `netlist.rs`. |
| **Discrete variable** | A variable that changes only at specific event times. 4-value logic (0, 1, X, Z). Stored in a flat state table indexed by `DigitalNet`. |
| **OSDI** | Open Standard Device Interface. The existing C ABI for analog device models (`.so` shared libraries). Devices contribute to the Jacobian matrix. Already fully implemented in `piperine-solver/src/osdi/`. |
| **D-OSDI** | Digital Open Standard Device Interface. A new C ABI proposed in this document for digital device models. Devices respond to input events and schedule output events. Does NOT participate in the Jacobian matrix. |
| **Connect module** | A device that has BOTH analog and digital ports. It translates between continuous and discrete domains. It is a full Verilog-AMS `analog begin` block compiled to a hybrid `.so` that exposes both OSDI and D-OSDI descriptors sharing the same instance data. |
| **Breakpoint** | A time instant forced into the stepper's schedule because a digital event or a connect module transition boundary requires it. |
| **Zero-crossing** | The exact time a continuous signal crosses a threshold. Detected by a connect module's A2D logic after the analog solver converges a timestep. The connect module owns this detection, not the simulator core. |
| **Delta cycle** | One round of combinational propagation at zero elapsed time in the digital domain. Multiple delta cycles can occur at a single time instant before the digital state is considered stable. |
| **Ghost node** | An extra `OsdiNode` terminal emitted by the compiler when a digital signal crosses into an `analog begin` block. Inside the block, the signal is treated as a continuous voltage. The elaborator automatically places a connect module on the boundary. |

---

## 3. The Unified Port Model

### 3.1 Current State of the Codebase and Required Harmonization

The existing netlist in `circuit/netlist.rs` uses these types:

```
NodeIdentifier          — Anonymous(usize) | Gnd
BranchIdentifier        — { component: String, name: Option<String> }
AnalogVariable          — Node(NodeIdentifier) | Branch(BranchIdentifier) | Time | Frequency | Iteration
AnalogReference         — { variable: Arc<AnalogVariable>, idx: Option<usize> }
Netlist                 — BiMap<AnalogReference, Arc<AnalogVariable>>
```

These are purely analog/continuous concepts. There is no representation for digital nets.

**Heritage problem:** These types carry ngspice-era naming conventions that do not align with
Verilog-AMS or OSDI semantics. The key mismatch is `BranchIdentifier`:

- In **ngspice**: a branch is a named component ("R1", "L2") with a string name.
- In **Verilog-AMS / OSDI**: a branch is a **pair of nodes**, written `I(a, b)` or `V(a, b)`.
  There is no component name attached to a branch — it is purely the (node_p, node_n) tuple.
  The branch current is a flow variable associated with two terminals.

The current `BranchIdentifier { component, name }` was designed for ngspice's component-oriented
world, where each SPICE element line ("R1 node1 node2 1k") has an explicit instance name. In our
pure OSDI solver, branches should be represented as node pairs, not named component strings.

**Required cleanup (future refactor, not blocking for mixed-signal):**

| Current type | Problem | Target |
|-------------|---------|--------|
| `BranchIdentifier { component, name }` | ngspice artifact. Branches are not named components. | `Branch(NodeIdentifier, NodeIdentifier)` — a pair of nodes. |
| `NodeIdentifier::Anonymous(usize)` | "Anonymous" implies it could be named; in OSDI all nodes are index-mapped. | `Node(usize)` — a flat index. GND remains special. |
| `AnalogVariable::Time`, `::Frequency`, `::Iteration` | These are simulation state, not circuit variables. They don't belong in the same enum as nodes/branches. | Move to `SimulationContext` or similar. |

This harmonization is noted here for completeness but is **not a prerequisite** for implementing
mixed-signal. The existing types work for the OSDI solver as-is. When we build the compiler,
we will need to align these types with the Verilog-AMS AST naturally.

### 3.2 The `Port` Enum

We introduce a single `Port` enum that represents any connection point in the simulation,
regardless of discipline. A port is resolved during elaboration into a concrete binding.

```rust
/// Represents a resolved port in the simulation.
/// The discipline determines which variant is used.
///
/// This enum is the SINGLE type used across the compiler, elaborator, and solver
/// to refer to a named signal. The compiler produces Port values. The elaborator
/// resolves them to their variant. The solver consumes them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Port {
    /// An analog (continuous) port. Participates in the Jacobian matrix.
    /// The AnalogReference contains the matrix index used by the NR solver.
    ///
    /// Example: `inout electrical vdd;` → Port::Analog(AnalogReference { ... })
    Analog(AnalogReference),

    /// A digital (discrete) port. Participates in the event queue.
    /// The DigitalNet indexes into the DigitalState.nets[] array.
    ///
    /// Example: `input logic clk;` → Port::Digital(DigitalNet(42))
    Digital(DigitalNet),
}
```

**There is no `Bridge` variant.** A connect module is a device with multiple ports — some
`Port::Analog` and some `Port::Digital`. The "bridging" is internal to the connect module's
compiled code. The simulator never needs to know that a port is "bridged"; it just sees analog
ports on the analog side and digital ports on the digital side, connected through the connect
module's shared `inst_data`.

**Design Decision: `Port` is a resolved enum, not a compile-time struct.**

The compiler initially works with port *names* (strings). During elaboration, each port name is
resolved to a `Port::Analog` or `Port::Digital` based on the discipline declared in the source.
The `Port` enum is the output of elaboration, not the input.

**Rationale:** In Verilog-AMS, ports are declared generically (`module foo(a, b, c)`), and
disciplines are declared separately (`electrical a; logic b;`). The port name itself carries no
discipline information. By making `Port` a resolved enum, we mirror this two-phase resolution
exactly. The compiler produces names, the elaborator resolves them to `Port` variants, and the
solver operates on the resolved `Port` values without caring how they were named in source.

### 3.3 `DigitalNet`

```rust
/// A digital net in the simulation. Indexes into DigitalState.nets[].
/// This IS the net — not an "identifier of" a net. It directly represents
/// the net's slot in the simulation state, analogous to how AnalogReference
/// directly represents a continuous variable's slot in the Jacobian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DigitalNet(pub usize);
```

The elaborator assigns `DigitalNet` values sequentially as it encounters digital nets,
exactly as it assigns `AnalogReference` indices for analog nodes today.

### 3.4 Compilation Rule: `logic` Inside `analog begin` Becomes `electrical`

When the compiler encounters a `logic` variable being read inside an `analog begin` block, it
**always** treats that variable as `electrical` within the analog context. The compiler emits a
ghost `OsdiNode` for it and generates C code that reads the node voltage and compares against 0.5.

This is a strict, no-exception rule: **every `logic` signal that crosses into `analog begin`
becomes a continuous voltage node** at the OSDI level. The elaborator then automatically inserts
a connect module to bridge the digital net to the ghost analog node.

The user never sees this. It is entirely transparent. But the implementer must understand that
this transformation happens at compile time, not at runtime.

---

## 4. Connect Modules

### 4.1 What a Connect Module Is

A connect module is a **full Verilog-AMS module** that happens to have ports in different
disciplines. It has an `analog begin` block with complete access to the Verilog-A language,
including:

- **`$analysis("dc")`, `$analysis("tran")`, `$analysis("ac")`:** The connect module can change
  its behavior based on the current analysis type. This is critical because:
  - During **DC analysis**, a D2A connect module should present a steady-state voltage source
    (no ramp, no transition — just the final value corresponding to the digital input).
  - During **transient analysis**, it should produce a smooth ramp with rise/fall times.
  - During **AC analysis**, it should present a proper small-signal impedance. A D2A module
    might present a low output impedance (voltage source) and an A2D module might present a
    high input impedance (voltage monitor). The digital ports are frozen at their DC values.
  - During **noise analysis**, the connect module may contribute thermal noise from its
    internal resistance model.

- **`transition()`:** Used to smooth digital-to-analog transitions. The connect module controls
  the rise time, fall time, and delay.

- **`@(cross(...))`:** Used for analog-to-digital threshold detection. The connect module
  monitors continuous voltages and fires digital events when thresholds are crossed.

- **Parameters:** Users can customize connect module behavior (e.g., different voltage levels
  for different I/O standards like LVCMOS, LVDS, etc.).

### 4.2 How Connect Modules Are Compiled

A connect module is compiled by our compiler into a **single `.so` shared library** that
exposes two descriptors:

1. **An `OsdiDescriptor`** — for the analog half. The analog ports become `OsdiNode` terminals.
   The `eval()` function implements the Verilog-A `analog begin` block (ramps, impedances, etc.).
   The `$analysis()` calls become checks on the OSDI `flags` field (`ANALYSIS_DC`, `ANALYSIS_AC`,
   `ANALYSIS_TRAN`).

2. **A `DosdiDescriptor`** — for the digital half. The digital ports become `DosdiPort` entries.
   The `eval()` function handles digital input changes (for D2A direction) and schedules digital
   output events (for A2D direction).

Both descriptors point to the **same `inst_data` memory** (same `instance_size`). They
communicate through shared offsets within `inst_data`:

```
inst_data layout (example for ppr_d2a):
┌─────────────────────────────────────────┐
│ offset 0:   f64 target_voltage          │  ← Written by D-OSDI eval (digital input)
│ offset 8:   f64 transition_start_time   │  ← Written by D-OSDI eval
│ offset 16:  f64 v_from                  │  ← Written by D-OSDI eval
│ offset 24:  u8  current_digital_value   │  ← Written by D-OSDI eval
│ offset 25:  [rest: OSDI internal state] │  ← Written by OSDI eval
└─────────────────────────────────────────┘
```

The D-OSDI `eval()` writes `target_voltage`, `transition_start_time`, and `v_from` when a
digital input changes. The OSDI `eval()` reads these during each Newton-Raphson iteration to
compute the instantaneous voltage of the analog ramp.

### 4.3 How the Simulator Treats Connect Modules

**The simulator treats connect modules exactly like any other device.** There is no special
`ConnectModule` type. There is no special connect-module loop. The only thing that makes a connect
module special is that it appears in BOTH the analog runtime list and the digital runtime list,
sharing the same `inst_data`. The elaborator links the two halves during circuit instantiation.

Concretely:
- The analog half is an `OsdiRuntime` in `CircuitInstance.runtimes`.
- The digital half is a `DosdiRuntime` in `CircuitInstance.digital_runtimes`.
- Both hold a `Arc<Vec<u8>>` (or similar shared pointer) to the same `inst_data` allocation.

### 4.4 Zero-Crossing Ownership

The connect module itself owns the zero-crossing detection logic. The simulator core does NOT
inspect analog node voltages to look for threshold crossings. The flow is:

1. The analog solver converges a timestep from `t` to `t_next`.
2. The simulator calls `accept_timestep()` on all OSDI runtimes (this already exists today).
3. The connect module's OSDI code, inside `accept_timestep()` (or a post-convergence callback),
   compares the voltage at `t` against the voltage at `t_next` and the configured threshold.
4. If a crossing occurred, the connect module computes `t_cross` using linear interpolation:
   ```
   t_cross = t + (v_threshold - v_at_t) / (v_at_t_next - v_at_t) * (t_next - t)
   ```
5. The connect module writes the crossing event into the D-OSDI event sink, which places it
   into the `DigitalState.event_queue`.

**Rationale:** This keeps crossing logic encapsulated in the device. Different connect modules
can use different thresholds, hysteresis bands, or interpolation methods without the simulator
core knowing about any of it. The simulator core only knows: "after convergence, call
`accept_timestep()` on all devices." That's it.

### 4.5 Test Scaffolding Connect Modules

> **These are throwaway implementations.** They exist solely to validate the mixed-signal
> infrastructure before we have a compiler. Once the compiler can produce real connect modules
> from Verilog-AMS source, these hand-written stubs will be deleted. Do not over-engineer them.

Implement two minimal connect modules as native Rust code (no `.so`, no FFI — just Rust structs
that implement the OSDI and D-OSDI traits directly):

**`TestD2A`:** Digital input → analog voltage ramp. Linear ramp from 0.0 to 1.8V with
hardcoded 100ps rise/fall. Enough to verify that a digital event propagates into the analog
matrix and that the NR solver converges through the transition.

**`TestA2D`:** Analog voltage monitor → digital output event. Hardcoded threshold at 0.9V,
no hysteresis. On post-convergence, does linear interpolation for `t_cross` and schedules
a digital event. Enough to verify that an analog signal crossing a threshold produces a
correctly-timed digital event.

---

## 5. The Unified Simulation Loop

### 5.1 Transient Analysis

The transient loop lives in the existing `TransientSolver` (`solver/transient.rs`). It is
**extended, not replaced**. The existing `solve()` method gains digital awareness.

**Pseudocode (annotated with what already exists vs. what is new):**

```
FUNCTION solve(stop_time):
    t = 0.0

    // [EXISTING] Compute DC operating point.
    dc_solution = solve_dc()

    // [NEW] Initialize digital state from DC solution.
    // For each A2D connect module: read the DC voltage on its analog port
    // and set the initial digital output value (above threshold → 1, else → 0).
    // For each D-OSDI device: set all inputs to their initial values (typically X).
    digital_state.initialize_from_dc(dc_solution, connect_modules)

    WHILE t < stop_time:

        // ── [MODIFIED] Compute next time target ──
        // The stepper proposes dt based on LTE and OSDI bound_step (existing).
        // We additionally clamp to the next digital event time (new).
        dt_proposed = stepper.propose_dt()                          // [EXISTING]
        t_next_event = digital_state.peek_next_event_time()         // [NEW]
        t_breakpoint = breakpoint_queue.peek_next()                 // [NEW]
        t_next = min(t + dt_proposed, t_next_event, t_breakpoint, stop_time)
        dt = t_next - t

        // ── [NEW] Checkpoint digital state (for rollback if NR fails) ──
        digital_state.checkpoint()

        // ── [NEW] Process pending digital events at t_next ──
        // Dequeue all events with time <= t_next.
        // Propagate through D-OSDI devices until stable (delta cycles).
        // This updates connect modules' D2A internal state (target voltage, etc.)
        // so that the OSDI analog eval() will use the correct ramp values.
        digital_state.evaluate_until_stable(t_next, &mut digital_runtimes)

        // ── [EXISTING, MODIFIED] Solve the analog timestep [t, t_next] ──
        // The NR solver calls assemble() which calls eval() on all OsdiRuntimes.
        // Connect modules' OSDI eval() reads D2A state from inst_data and
        // computes the correct analog voltage for time t_next.
        result = self.execute_timestep(t_next, dt)

        IF result IS ERROR:
            // [MODIFIED] NR failed to converge. Halve dt and retry.
            // Roll back digital state to before we processed events.
            digital_state.rollback()
            dt = dt / 2
            CONTINUE

        // ── [NEW] Post-convergence: let connect modules detect crossings ──
        // For each A2D connect module (which is just a regular OsdiRuntime):
        //   1. Its accept_timestep() compares v_prev vs v_now vs threshold.
        //   2. If crossed, it computes t_cross and writes an event into
        //      the digital_state.event_queue via the D-OSDI event sink.
        //   3. If t_cross < t_next, it also inserts a breakpoint so that
        //      the next iteration's t_next will land exactly on t_cross.
        for runtime in self.circuit.all_runtimes_mut():
            runtime.accept_timestep(state, context)

        // ── [NEW] Commit digital state ──
        digital_state.commit()

        // [EXISTING] Record snapshot, advance time.
        steps.push(self.snapshot(t_next))
        t = t_next

    RETURN TransientAnalysisResult::new(steps)
```

### 5.2 Design Decision: Digital Events Processed BEFORE the Analog Solve

Digital events at `t_next` are resolved BEFORE the analog solver runs for `[t, t_next]`.

**Rationale:** The D2A side of connect modules must know the current digital state before the
analog solver calls `eval()`. If we solved the analog step first and then processed digital
events, the connect modules' `eval()` would use stale digital values. We would then need to roll
back the analog solution and re-solve it with the updated D2A state. Processing digital events
first ensures the analog solver always sees the correct ramp targets on its first attempt.

### 5.3 AC / Noise / Transfer Function (Analog-Only Analyses)

These analyses have no digital component. When the simulation is purely analog:

- The `DigitalState.event_queue` is empty.
- `peek_next_event_time()` returns `f64::INFINITY`.
- Connect modules' digital ports are frozen at their DC operating point values (set during
  `initialize_from_dc()`).
- The analyses proceed exactly as they do today, with zero overhead.

**How connect modules behave during these analyses:**

The connect module's OSDI `eval()` receives the analysis type flags (`ANALYSIS_AC`,
`ANALYSIS_NOISE`, etc.) via the existing `OsdiSimInfo.flags` field. The module can do
**whatever it wants** with this information — it is a full Verilog-AMS `analog begin` block
with complete freedom. A module author might present a small-signal impedance in AC, contribute
thermal noise sources, or simply go high-impedance. The simulator imposes no constraints on
what a connect module does with its analog interface during any analysis. It is entirely the
module's responsibility to implement correct behavior for each analysis type via
`$analysis()` checks (which compile to `if (flags & ANALYSIS_*)` in C).

**Design Decision: No special-casing for pure-analog.**

The unified loop handles this naturally. There is no `if has_digital { ... } else { ... }`.
The event queue is simply empty, and `min(t + dt_proposed, INFINITY)` equals `t + dt_proposed`.
The `evaluate_until_stable()` call returns immediately when there are no events. Cost: one
function call per timestep that checks an empty heap. Negligible.

---

## 6. The D-OSDI ABI

### 6.1 Scope

D-OSDI defines a C ABI for discrete event devices. Like OSDI, devices are compiled to `.so`
shared libraries and loaded at runtime via `dlopen`. Unlike OSDI, D-OSDI devices do **not**
stamp into the Jacobian matrix. They consume and produce events.

D-OSDI is used for:
- Pure digital devices (inverters, flip-flops, standard cells).
- The digital half of connect modules.
- Any behavioral model that is purely event-driven.

### 6.2 The Descriptor

```c
/// The D-OSDI descriptor. One per device type in the .so library.
/// Loaded via dlopen + dlsym, exactly like OsdiDescriptor.
typedef struct {
    /// Device name. Must match the module name in source code.
    /// For connect modules, this is the same name as the OsdiDescriptor.name.
    const char* name;

    /// Total number of ports (inputs + outputs + inouts).
    uint32_t num_ports;

    /// Array of port descriptors. Length = num_ports.
    /// Port order must be consistent: inputs first, then outputs, then inouts.
    const DosdiPort* ports;

    /// Number of configurable parameters (e.g., gate delay, setup time).
    uint32_t num_params;

    /// Array of parameter descriptors. Length = num_params.
    const DosdiParam* params;

    /// Size in bytes of per-instance opaque state blob.
    /// For connect modules, this MUST equal the OsdiDescriptor.instance_size
    /// because both halves share the same inst_data allocation.
    uint32_t instance_size;

    /// Size in bytes of per-model opaque state blob.
    uint32_t model_size;

    // ── Lifecycle function pointers ──

    /// Initialize model-level data. Called once per unique model.
    /// The simulator allocates model_size bytes, zeroes them, and passes the pointer.
    void (*setup_model)(void* model_data, const DosdiSimParas* sim);

    /// Initialize instance-level data. Called once per device instantiation.
    /// The simulator allocates instance_size bytes, zeroes them, and passes the pointer.
    /// For connect modules, the simulator passes the SAME inst_data pointer that was
    /// allocated for the OSDI half. setup_instance must not re-zero shared fields.
    void (*setup_instance)(void* inst_data, void* model_data, const DosdiSimParas* sim);

    // ── Evaluation function pointer ──

    /// Called by the simulator when one or more input ports change value.
    ///
    /// Arguments:
    ///   inst_data   — Per-instance opaque blob.
    ///   model_data  — Per-model opaque blob.
    ///   inputs      — Array of LogicValue bytes. Length = number of input bits.
    ///                 For a 1-bit input port, one byte. For an 8-bit bus port,
    ///                 8 bytes (LSB first).
    ///   outputs     — Array the device writes its output values into.
    ///                 Length = number of output bits. The simulator reads these
    ///                 after eval() returns.
    ///   event_sink  — Opaque handle to schedule future output events.
    ///                 The device calls event_sink->schedule() to post events.
    ///   current_time — Absolute simulation time in seconds.
    ///
    /// Returns:
    ///   0 on success.
    ///   Nonzero on fatal error (simulation aborts).
    ///
    /// The device MUST NOT modify inputs[]. It MUST write to outputs[] the
    /// combinational output values that are valid RIGHT NOW (at current_time).
    /// For sequential outputs that change after a delay, the device calls
    /// event_sink->schedule() and leaves outputs[] unchanged until the event fires.
    uint32_t (*eval)(
        void* inst_data,
        void* model_data,
        const uint8_t* inputs,
        uint8_t* outputs,
        DosdiEventSink* event_sink,
        double current_time
    );

    /// Read or write a parameter value. Same semantics as OSDI access().
    void* (*access)(void* inst_data, void* model_data, uint32_t param_id, uint32_t flags);

} DosdiDescriptor;
```

### 6.3 Port Descriptor

```c
typedef struct {
    /// Port name. MUST match the Verilog-AMS port declaration exactly.
    /// The elaborator uses this string to link D-OSDI ports to digital nets
    /// and to match connect module ports across OSDI/D-OSDI halves.
    const char* name;

    /// Direction: DOSDI_DIR_INPUT=0, DOSDI_DIR_OUTPUT=1, DOSDI_DIR_INOUT=2.
    uint32_t direction;

    /// Bit width. 1 for scalar (`logic x`), 8 for vector (`logic [7:0] bus`).
    uint32_t width;
} DosdiPort;
```

### 6.4 Event Scheduling Interface

```c
/// Provided by the simulator to the device during eval().
/// The device uses this to schedule future output changes.
typedef struct {
    /// Opaque handle. The device passes this back to schedule/cancel.
    void* handle;

    /// Schedule a value change on output port `port_idx` after `delay` seconds.
    ///
    /// port_idx: Index into the outputs[] array (0-based, counting output bits).
    /// value:    The LogicValue (0, 1, 2=X, 3=Z) the port will take.
    /// delay:    Seconds from current_time. Must be >= 0.
    ///           delay=0 means "next delta cycle" (zero-time propagation).
    ///
    /// If delay > 0, this is a transport delay event.
    /// The simulator converts to absolute time: event_time = current_time + delay.
    void (*schedule)(void* handle, uint32_t port_idx, uint8_t value, double delay);

    /// Cancel all pending events on output port `port_idx`.
    /// Used to implement inertial delay semantics (Verilog default):
    /// when a new assignment is made before a previous one fires, the old one
    /// is cancelled. The device should call cancel() before schedule() when
    /// implementing inertial delay.
    void (*cancel)(void* handle, uint32_t port_idx);
} DosdiEventSink;
```

### 6.5 LogicValue Encoding

```c
#define DOSDI_LOGIC_0  0   // Strong zero
#define DOSDI_LOGIC_1  1   // Strong one
#define DOSDI_LOGIC_X  2   // Unknown / conflict / uninitialized
#define DOSDI_LOGIC_Z  3   // High-impedance (tri-state)
```

**Design Decision: 4-value logic (IEEE 1364).**

We use 4-value logic rather than 2-value. This is necessary to correctly model tri-state buses,
uninitialized registers, and bus contention. It matches the Verilog standard.

### 6.6 Simulation Parameters

```c
typedef struct {
    double timescale;         // Time unit in seconds (e.g., 1e-9 for `timescale 1ns`)
    double temperature;       // Kelvin (default 300.15)
    double supply_voltage;    // VDD in volts (default 1.8). Used by connect modules
                              // for default v_high and threshold calculations.
} DosdiSimParas;
```

### 6.7 How D-OSDI Differs from Analog OSDI

| Aspect | Analog OSDI | Digital D-OSDI |
|--------|-------------|----------------|
| **When `eval()` is called** | Every Newton-Raphson iteration (many times per timestep) | Only when input port values change |
| **Input/output type** | `f64` node voltages via `prev_solve[]` | `uint8_t` LogicValues via `inputs[]`/`outputs[]` |
| **Time model** | Continuous (`abstime` in `OsdiSimInfo`) | Event-driven with explicit delays |
| **Matrix participation** | Stamps residuals and Jacobian entries | None — pure behavioral |
| **State vectors** | `prev_state[]` / `next_state[]` for `ddt()` integration | Opaque `inst_data` blob (device manages its own state) |
| **Noise** | `load_noise()` returns PSD per source | Not applicable |
| **Timestep control** | `bound_step_offset` limits `dt` | Not applicable (events have explicit times) |
| **Analysis awareness** | `flags & ANALYSIS_*` in `OsdiSimInfo` | Not applicable (digital is only active during transient) |

---

## 7. The Digital State

### 7.1 Data Structures

The digital state is NOT a separate "engine." It is a field of `CircuitInstance`, alongside the
existing `netlist` and `runtimes`.

```rust
/// 4-value logic. Matches DOSDI_LOGIC_* C constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicValue {
    Zero = 0,
    One  = 1,
    X    = 2,  // Unknown / uninitialized / conflict
    Z    = 3,  // High-impedance
}

/// A scheduled change to a digital net.
#[derive(Debug, Clone)]
pub struct DigitalEvent {
    /// Absolute simulation time at which this event fires.
    pub time: f64,
    /// The net whose value changes.
    pub net: DigitalNet,
    /// The new value.
    pub value: LogicValue,
    /// Which device instance generated this event (for cancellation and debugging).
    pub source: usize,  // index into digital_runtimes[]
}

/// Implements Ord so events can be stored in a min-heap ordered by time.
impl Ord for DigitalEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.partial_cmp(&other.time)
            .unwrap_or(Ordering::Equal)
    }
}

/// The complete digital simulation state.
pub struct DigitalState {
    /// Current value of every digital net. Indexed by DigitalNet.0.
    /// Length = total number of digital nets in the circuit.
    pub nets: Vec<LogicValue>,

    /// Min-heap of future events, ordered by time.
    pub event_queue: BinaryHeap<Reverse<DigitalEvent>>,

    /// Saved snapshot for rollback. Created by checkpoint(), consumed by commit().
    checkpoint: Option<(Vec<LogicValue>, BinaryHeap<Reverse<DigitalEvent>>)>,
}
```

### 7.2 Evaluation: `evaluate_until_stable()`

This is the digital equivalent of one "step." It processes all events at a given time and
propagates through D-OSDI devices until no more zero-delay events remain.

```rust
impl DigitalState {
    /// Process all events at time `t` and propagate through D-OSDI devices
    /// until no more zero-delay events remain (delta cycle resolution).
    ///
    /// # Arguments
    /// * `t` — The current simulation time. Only events with event.time == t
    ///         (within floating-point tolerance) are processed.
    /// * `devices` — All D-OSDI device runtimes. Their eval() is called when
    ///               inputs change.
    ///
    /// # Algorithm
    /// 1. Drain all events from event_queue where event.time <= t + epsilon.
    /// 2. If no events, return immediately (nothing to do).
    /// 3. Apply each event: set nets[event.net] = event.value.
    /// 4. Collect the set of nets that changed.
    /// 5. For each D-OSDI device whose input ports include any changed net:
    ///    a. Build the inputs[] array by reading self.nets for each input port.
    ///    b. Call device.eval(inputs, outputs, event_sink, t).
    ///    c. The device may call event_sink.schedule(), which appends new events
    ///       to self.event_queue.
    /// 6. If any newly scheduled events have time == t (zero delay), go to step 1.
    ///    This is a "delta cycle."
    /// 7. If all new events have time > t, stop. The digital state is stable.
    /// 8. Safety: if delta_count exceeds 1000, log a warning and stop. Set any
    ///    oscillating nets to LogicValue::X.
    pub fn evaluate_until_stable(&mut self, t: f64, devices: &mut [DosdiRuntime]) {
        let epsilon = 1e-20; // floating-point comparison tolerance
        let max_delta_cycles = 1000;
        let mut delta_count = 0;

        loop {
            // Step 1: drain events at time t
            let mut events_now = Vec::new();
            while let Some(Reverse(event)) = self.event_queue.peek() {
                if event.time <= t + epsilon {
                    events_now.push(self.event_queue.pop().unwrap().0);
                } else {
                    break;
                }
            }

            // Step 2: nothing to do?
            if events_now.is_empty() {
                break;
            }

            // Step 3 + 4: apply events, collect changed nets
            let mut changed = HashSet::new();
            for event in &events_now {
                if self.nets[event.net.0] != event.value {
                    self.nets[event.net.0] = event.value;
                    changed.insert(event.net);
                }
            }

            // Step 5: evaluate affected devices
            for device in devices.iter_mut() {
                if device.has_input_on(&changed) {
                    device.eval(t, &self.nets, &mut self.event_queue);
                }
            }

            // Step 6 + 8: delta cycle check
            delta_count += 1;
            if delta_count >= max_delta_cycles {
                log::warn!(
                    "Delta cycle limit ({}) exceeded at t={}. Possible combinational loop.",
                    max_delta_cycles, t
                );
                break;
            }

            // Check if any new events are at time t (zero delay → another delta cycle)
            let has_more_at_t = self.event_queue.peek()
                .map(|Reverse(e)| e.time <= t + epsilon)
                .unwrap_or(false);
            if !has_more_at_t {
                break; // Step 7: stable
            }
        }
    }

    /// Return the time of the earliest pending event, or f64::INFINITY if empty.
    pub fn peek_next_event_time(&self) -> f64 {
        self.event_queue.peek()
            .map(|Reverse(e)| e.time)
            .unwrap_or(f64::INFINITY)
    }

    /// Save a snapshot of the current state. Call this BEFORE processing events
    /// at a timestep that the analog solver might reject.
    pub fn checkpoint(&mut self) {
        self.checkpoint = Some((self.nets.clone(), self.event_queue.clone()));
    }

    /// Restore the state to the last checkpoint. Call this when the analog solver
    /// fails to converge and we need to retry with a smaller dt.
    pub fn rollback(&mut self) {
        if let Some((nets, queue)) = self.checkpoint.take() {
            self.nets = nets;
            self.event_queue = queue;
        }
    }

    /// Discard the checkpoint. Call this after the analog solver successfully
    /// converges and the timestep is accepted.
    pub fn commit(&mut self) {
        self.checkpoint = None;
    }
}
```

---

## 8. Hybrid Modules (Compilation Strategy)

When our compiler encounters a module with mixed-discipline ports:

```verilog
module ldo_enable(en, vin, vout, gnd);
    input logic en;
    inout electrical vin, vout, gnd;
    analog begin
        if (en)
            V(vout, gnd) <+ transition(V(vin, gnd), 0, 1u, 1u);
        else
            V(vout, gnd) <+ transition(0.0, 0, 1u, 1u);
    end
endmodule
```

### 8.1 Step 1: Ghost Node Emission (Compiler)

The compiler detects that `en` is `logic` but is used inside `analog begin`. Per the strict rule
(Section 3.4), it becomes `electrical` inside the analog context. The compiler emits:

- An OSDI descriptor with **4 terminals** (not 3): `vin`, `vout`, `gnd`, and `__bridge_en`.
- In the generated C `eval()` function, the read of `en` becomes:
  ```c
  double en_voltage = prev_solve[node_mapping[3]]; // __bridge_en is terminal index 3
  int en_val = (en_voltage > 0.5) ? 1 : 0;
  ```

The `__bridge_en` terminal is flagged in the OSDI descriptor metadata so the elaborator knows
it is a compiler-generated ghost node, not a user-declared port.

### 8.2 Step 2: Connect Module Insertion (Elaborator)

The elaborator processes the user's netlist. It sees that:
- Port `en` on the `ldo_enable` instance is declared `logic` in the source.
- The OSDI descriptor has a terminal `__bridge_en` that expects an analog connection.

The elaborator consults its connect rule table. The default rule for `logic` → `electrical` is
to insert a `ppr_d2a` connect module. It does so:

1. Allocates a `DigitalNet` for the `en` digital net.
2. Instantiates the D2A connect module with:
   - Digital input port → `DigitalNet` of `en`.
   - Analog output port → connected to the same `NodeIdentifier` as `__bridge_en`.
3. The `ppr_d2a` instance appears in both `runtimes` (OSDI half) and `digital_runtimes`
   (D-OSDI half).

### 8.3 Step 3: Runtime Behavior

During transient simulation:
1. A digital event changes `en` from 0 to 1.
2. `evaluate_until_stable()` calls the D-OSDI `eval()` of `ppr_d2a`.
3. `ppr_d2a`'s D-OSDI `eval()` writes `target_voltage = 1.8`, `v_from = current_voltage`,
   and `transition_start = current_time` into `inst_data`.
4. The analog solver runs. It calls the OSDI `eval()` of both `ppr_d2a` and `ldo_enable`.
5. `ppr_d2a`'s OSDI `eval()` reads `inst_data`, computes the ramp voltage at the current NR
   time, and contributes the voltage source equation to the matrix.
6. `ldo_enable`'s OSDI `eval()` reads `__bridge_en` via `prev_solve[]`. It sees a smooth ramp,
   not a step. The `transition()` call in the user's code also smooths the output. The NR
   solver converges without difficulty.

---

## 9. Integration with Existing Code

### 9.1 Extended `CircuitInstance`

```rust
// File: crates/piperine-solver/src/circuit/instance.rs

pub struct CircuitInstance {
    pub title: String,
    pub runtimes: Vec<OsdiRuntime>,           // [EXISTING] Analog OSDI devices
    pub digital_runtimes: Vec<DosdiRuntime>,  // [NEW] Digital D-OSDI devices
    pub digital_state: DigitalState,          // [NEW] Net values + event queue
    pub netlist: Netlist,                     // [EXISTING] Analog netlist
}
```

For connect modules, the same `inst_data` memory is shared between an entry in `runtimes` and
an entry in `digital_runtimes`. The elaborator sets this up during `CircuitInstance::instantiate()`.

### 9.2 Extended `TransientSolver::solve()`

The existing `solve()` method in `solver/transient.rs` gains the following changes, marked
clearly so the implementer knows exactly what to add:

```rust
// PSEUDOCODE — not literal Rust, but close enough to implement from.

pub fn solve(&mut self) -> Result<TransientAnalysisResult> {
    let stop_time: f64 = self.options.stop_time.into();
    let dt: f64 = self.options.dt.into();

    // [EXISTING]
    let initial_snapshot = self.compute_initial_conditions()?;
    let mut steps = vec![initial_snapshot];

    // [NEW] Initialize digital state from DC solution.
    self.system.circuit.digital_state.initialize_from_dc(
        &dc_result,
        &mut self.system.circuit.digital_runtimes,
    );

    let mut current_time = 0.0;

    while current_time < stop_time {
        // [MODIFIED] Compute dt considering digital events.
        let dt_proposed = dt; // or adaptive stepper proposal
        let t_next_event = self.system.circuit.digital_state.peek_next_event_time();
        let t_next = f64::min(current_time + dt_proposed, t_next_event)
            .min(stop_time);
        let actual_dt = t_next - current_time;

        // [NEW] Checkpoint digital state before processing events.
        self.system.circuit.digital_state.checkpoint();

        // [NEW] Process digital events at t_next.
        self.system.circuit.digital_state.evaluate_until_stable(
            t_next,
            &mut self.system.circuit.digital_runtimes,
        );

        // [EXISTING, MODIFIED] Execute the analog timestep.
        match self.execute_timestep(t_next, actual_dt) {
            Ok(Some(snapshot)) => {
                // [NEW] Post-convergence: accept_timestep on all runtimes
                // (connect modules detect crossings here and schedule events).
                for runtime in self.system.circuit.all_runtimes_mut() {
                    runtime.accept_timestep(&self.solver.state, &self.system.context);
                }

                // [NEW] Commit digital state (discard checkpoint).
                self.system.circuit.digital_state.commit();

                steps.push(snapshot);
                current_time = t_next;
            }
            Err(_) => {
                // [NEW] NR failed. Rollback digital state and retry.
                self.system.circuit.digital_state.rollback();
                // Reduce dt and retry (existing adaptive behavior).
            }
        }
    }

    Ok(TransientAnalysisResult::new(steps))
}
```

### 9.3 File Layout

```
crates/piperine-solver/src/
    digital/
        mod.rs              // pub mod dosdi; pub use types.
                            // Contains: LogicValue, DigitalNet, DigitalEvent,
                            //           DigitalState (with evaluate_until_stable,
                            //           checkpoint, rollback, commit).
        dosdi/
            ffi.rs          // #[repr(C)] structs: DosdiDescriptor, DosdiPort,
                            //   DosdiParam, DosdiEventSink, DosdiSimParas.
                            //   Constants: DOSDI_LOGIC_*, DOSDI_DIR_*.
            loader.rs       // DosdiLib: dlopen() a .so, locate the descriptor
                            //   array (same pattern as osdi/loader.rs).
            runtime.rs      // DosdiRuntime: per-instance wrapper.
                            //   Fields: lib, descriptor_idx, device_name, inst_data,
                            //           model_data, input_net_ids, output_net_ids.
                            //   Methods: eval(), has_input_on(), setup().
    circuit/
        netlist.rs          // [MODIFIED] Add: DigitalNet, Port enum.
        instance.rs         // [MODIFIED] Add: digital_runtimes, digital_state fields.
    solver/
        transient.rs        // [MODIFIED] Add digital event processing to solve().
```

---

## 10. Implementation Phases

| Phase | What to Build | What to Test | Depends On |
|-------|---------------|--------------|------------|
| **1** | `LogicValue` enum, `DigitalNet` struct, `DigitalEvent` struct, `DigitalState` struct with `evaluate_until_stable()`, `checkpoint()`, `rollback()`, `commit()`, `peek_next_event_time()`. All in `digital/mod.rs`. | Unit test: chain of 3 mock inverters. Insert event on input. Verify output toggles after 3 delta cycles. Test checkpoint+rollback restores prior state. Test event ordering (FIFO at same time). | Nothing |
| **2** | `Port` enum, `DigitalNet` in `circuit/netlist.rs`. | Unit test: create `Port::Analog(...)` and `Port::Digital(...)`, verify enum matching. | Nothing |
| **3** | D-OSDI FFI structs in `digital/dosdi/ffi.rs`: `DosdiDescriptor`, `DosdiPort`, `DosdiParam`, `DosdiEventSink`, `DosdiSimParas`, `DOSDI_LOGIC_*` constants. | Compile check only (these are `#[repr(C)]` struct definitions). | Nothing |
| **4** | `DosdiRuntime` in `digital/dosdi/runtime.rs`: instance allocation, `eval()` wrapper that builds `inputs[]` from `DigitalState.nets`, calls FFI `eval()`, processes `outputs[]`. | Unit test with a mock inverter `.so`: input 0→1, verify output schedules 1→0 event with correct delay. | Phases 1, 3 |
| **5** | `DosdiLib` loader in `digital/dosdi/loader.rs`: `dlopen`, locate `__dosdi_descriptors` symbol. | Unit test: load a test `.so`, verify descriptor fields. | Phase 3 |
| **6** | Extend `CircuitInstance`: add `digital_runtimes: Vec<DosdiRuntime>`, `digital_state: DigitalState`. Extend `instantiate()` to allocate digital nets and runtimes. | Unit test: instantiate a circuit with one digital device, verify state and runtimes are populated. | Phases 1, 4 |
| **7** | Extend `TransientSolver::solve()`: add digital event processing, checkpoint/rollback, breakpoint clamping (as shown in Section 9.2). | Integration test: pure-digital ring oscillator (5 inverters). Verify oscillation period = 10 × gate_delay. | Phase 6 |
| **8** | Implement `TestD2A` connect module (native Rust, throwaway scaffolding). | Integration test: digital square wave → D2A → OSDI RC low-pass filter. Verify analog output shows exponential charge/discharge. | Phase 7 |
| **9** | Implement `TestA2D` connect module (native Rust, throwaway scaffolding). | Integration test: OSDI voltage ramp → A2D → digital net. Verify event time matches expected `t_cross` within interpolation tolerance. | Phase 7 |
| **10** | Full-loop integration test. | Clock → D2A → analog amplifier → A2D → digital flip-flop. Verify flip-flop captures correct values. | Phases 8, 9 |

Phases 1, 2, and 3 can proceed in parallel. Phase 4 depends on 1+3. Phase 5 depends on 3 only.

---

## 11. Test Strategy

### 11.1 Unit Tests

| Test | What It Verifies |
|------|------------------|
| `LogicValue` resolution table | `Zero` + `One` on same net → `X`. `Z` + `One` → `One`. `Z` + `Z` → `Z`. `X` + anything → `X`. |
| `DigitalState::evaluate_until_stable` with 3 mock inverters | Input goes 0→1. After 1 delta cycle, inverter 1 output goes 1→0. After 2 delta cycles, inverter 2 goes 0→1. After 3, inverter 3 goes 1→0. Verify net values after stabilization. |
| `DigitalState::checkpoint` + `rollback` | Process events, checkpoint, process more events, rollback. Verify state matches checkpoint exactly. |
| Event queue ordering | Schedule 3 events at t=5.0, t=3.0, t=5.0. Verify they fire in order: t=3.0, then the two at t=5.0 in FIFO insertion order. |
| `DosdiRuntime::eval` with mock inverter | Set input net to `One`. Call eval. Verify it schedules an event with value `Zero` and delay = gate_delay parameter. |
| `D2AConnector` ramp voltage | Set target=1.8, t_rise=100ps, start at t=0. Query voltage_at(50ps). Verify it returns 0.9 (midpoint of linear ramp). |
| `A2DConnector` crossing detection | v_old=0.8, v_new=1.1, threshold=1.0. Verify crossing detected. Verify t_cross = t + (1.0-0.8)/(1.1-0.8) * dt. |
| `A2DConnector` hysteresis | v_old=0.95, v_new=1.05, threshold=1.0, hysteresis=0.2. Verify NO crossing (within hysteresis band). |

### 11.2 Integration Tests

| Test | Circuit | Verification |
|------|---------|-------------|
| Pure digital ring oscillator | 5 inverters in a ring, each with delay=1ns | Period = 10ns. Run for 100ns, verify 10 complete cycles in event log. |
| D2A → RC filter | Digital 1MHz square wave → `ppr_d2a` → R=1kΩ + C=1nF to ground | Verify analog output has exponential shape. Steady-state amplitude ≈ V_high × (1 - e^(-T/2RC)). |
| A2D → event timing | OSDI linear ramp source (0V to 2V in 10ns) → `ppr_a2d` (threshold=1.0V) | Verify digital event fires at t ≈ 5ns (within dt tolerance). |
| Full mixed-signal loop | Clock(10MHz) → D2A → RC → A2D → D-flip-flop | Verify DFF captures correct logic value at each rising clock edge. The RC delay should cause the DFF to see the previous clock's analog state. |

### 11.3 Regression Guard

All 27 existing analog-only tests (`cargo test -p piperine-solver`) must continue to pass
without modification. The digital extensions must introduce zero behavioral change to circuits
that contain no digital devices. Verify this by running the full test suite after every phase.

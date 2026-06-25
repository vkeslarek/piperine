# Piperine Mixed-Signal Cosimulation — Development Specification

> **Purpose:** This document is a complete, unambiguous development specification for implementing
> mixed-signal (analog + digital) cosimulation in the Piperine simulator. It is written so that an
> implementer with no prior context can build the system from these instructions alone.

---

## 1. Background and Terminology

| Term | Definition |
|------|-----------|
| **Analog Solver** | The existing Newton-Raphson + OSDI transient engine (`TransientSolver` in `solver/transient.rs`). It advances time by computing solutions to differential-algebraic equations at discrete timesteps. |
| **Digital Solver** | A new event-driven simulation engine to be built. It maintains a priority queue of future events and evaluates combinational/sequential logic only when inputs change. |
| **OSDI** | Open Standard Device Interface. A C ABI for analog device models (`.so` libraries). Already implemented in `piperine-solver/src/osdi/`. |
| **D-OSDI** | Digital Open Standard Device Interface. A new C ABI proposed in this document for digital device models. Analogous to OSDI but event-driven instead of matrix-driven. |
| **Connect Module** | A bridge device that converts signals between the analog and digital domains. Two kinds exist: D2A (digital-to-analog) and A2D (analog-to-digital). |
| **Breakpoint** | A time instant where the analog solver is forced to place a timestep boundary, typically because a digital event occurs there. |
| **Zero-Crossing** | The exact time at which a continuous analog signal crosses a threshold. Detected by interpolation after the analog solver completes a timestep. |
| **Delta Cycle** | A digital simulation concept: a round of combinational propagation that occurs at zero elapsed time. Multiple delta cycles may occur at a single time instant. |

---

## 2. Architecture Overview

The system consists of three major components that communicate through a central coordinator:

```
┌──────────────────────────────────────────────────────────────┐
│                    Cosim Coordinator                         │
│  (owns the global clock, breakpoint queue, connect modules)  │
│                                                              │
│  ┌─────────────────┐     ┌──────────────────┐               │
│  │  Analog Engine   │◄───►│  Digital Engine   │               │
│  │  (OSDI devices)  │     │  (D-OSDI devices) │               │
│  │  NR + Transient  │     │  Event Queue       │               │
│  └────────┬─────────┘     └────────┬──────────┘               │
│           │                        │                          │
│           └────────┐  ┌────────────┘                          │
│                    ▼  ▼                                       │
│            ┌───────────────┐                                  │
│            │Connect Modules│                                  │
│            │  D2A     A2D  │                                  │
│            └───────────────┘                                  │
└──────────────────────────────────────────────────────────────┘
```

### 2.1 Design Decision: Single-Process, Multi-Engine

Both engines run in the same process. No IPC. No threads for the initial implementation.

**Rationale:** IPC introduces latency and complexity (serialization, synchronization). The lockstep
algorithm requires tight back-and-forth between engines at every timestep. In-process function calls
are orders of magnitude faster. Threading can be added later as an optimization once correctness is
proven.

### 2.2 Design Decision: The Coordinator Owns Time

Neither the analog engine nor the digital engine independently decides what time to advance to.
The `CosimCoordinator` is the single authority that computes `t_next` by considering:
1. The analog stepper's proposed `dt` (from LTE / `bound_step`).
2. The digital engine's next pending event time.
3. Any forced breakpoints from connect modules.

**Rationale:** If each engine independently advanced time, rollbacks would be frequent and expensive.
A single time authority prevents causal violations entirely.

---

## 3. The Lockstep Synchronization Algorithm

This is the core loop of the cosimulation. It runs inside `CosimCoordinator::run_transient()`.

### Pseudocode

```
FUNCTION run_transient(stop_time):
    t = 0.0
    dc_solution = analog_engine.solve_dc()
    digital_engine.initialize(dc_solution)
    connect_modules.initialize(dc_solution)

    WHILE t < stop_time:
        // ── PHASE 1: Compute next time target ──
        dt_analog = analog_engine.propose_timestep()
        t_next_analog = t + dt_analog
        t_next_digital = digital_engine.peek_next_event_time()  // may be INFINITY

        t_next = min(t_next_analog, t_next_digital, stop_time)
        dt = t_next - t

        // ── PHASE 2: Process digital events at t_next ──
        IF digital_engine.has_events_at(t_next):
            digital_engine.evaluate_until_stable(t_next)
            // This resolves all delta cycles at t_next.
            // After stabilization, read output port values.

            FOR EACH d2a_connector IN connect_modules.d2a:
                new_digital_value = digital_engine.read_port(d2a_connector.digital_port)
                d2a_connector.start_transition(new_digital_value, t_next)
                // The D2A connector records the target value and the time at
                // which the transition begins. It does NOT step-change the
                // analog source. Instead, it will produce a smooth ramp
                // over t_rise/t_fall during subsequent analog evaluations.

        // ── PHASE 3: Solve the analog timestep [t, t_next] ──
        analog_result = analog_engine.step(t, t_next, dt)
        // Inside this step:
        //   - D2A connectors inject time-varying source values into the matrix.
        //   - OSDI devices are evaluated via eval().
        //   - Newton-Raphson iterates until convergence.

        IF analog_result == FAILED_TO_CONVERGE:
            dt = dt / 2
            CONTINUE  // Retry with smaller timestep. No time advances.

        // ── PHASE 4: Detect analog-to-digital crossings ──
        FOR EACH a2d_connector IN connect_modules.a2d:
            v_old = a2d_connector.voltage_at(t)
            v_new = a2d_connector.voltage_at(t_next)

            IF crossed_threshold(v_old, v_new, a2d_connector.threshold):
                t_cross = interpolate_crossing_time(t, v_old, t_next, v_new, threshold)
                new_logic_value = (v_new > threshold) ? 1 : 0
                digital_engine.schedule_event(a2d_connector.digital_port, new_logic_value, t_cross)

        // ── PHASE 5: Commit and advance ──
        digital_engine.commit_state()
        analog_engine.accept_timestep()
        t = t_next

    RETURN collect_results()
```

### 3.1 Design Decision: Phase 2 Before Phase 3

Digital events are processed **before** the analog step, not after.

**Rationale:** If we solved the analog step first and then discovered a digital event at the same
time, we would need to roll back the analog solution and redo it with the updated D2A sources. By
processing digital events first, D2A connectors are already in the correct state when the analog
solver runs. This eliminates rollbacks in the common case.

### 3.2 Design Decision: Linear Interpolation for Zero-Crossing

For the initial implementation, use linear interpolation to find `t_cross`:

```
t_cross = t + (threshold - v_old) / (v_new - v_old) * dt
```

**Rationale:** Higher-order interpolation (quadratic, cubic) requires storing multiple past solution
points and is more complex to implement. Linear interpolation is sufficient for most practical
cases. The error is bounded by `O(dt²)`, which is acceptable because the analog stepper already
controls `dt` to meet truncation error tolerances.

---

## 4. Connect Modules

Connect modules are not OSDI devices. They are lightweight Rust structs managed directly by the
coordinator. They have no Jacobian, no matrix stamping. They are pure signal translators.

### 4.1 D2A Connect Module (Digital → Analog)

```rust
/// A D2A connector injects a smooth voltage transition into an analog node
/// whenever the digital side changes value.
pub struct D2AConnector {
    /// The analog node this connector drives (an AnalogReference in the netlist).
    pub analog_node: AnalogReference,

    /// The digital port name this connector reads from.
    pub digital_port: PortId,

    /// Voltage levels for logic 0 and logic 1.
    pub v_low: f64,   // e.g., 0.0
    pub v_high: f64,  // e.g., 1.8

    /// Rise and fall times for the analog transition.
    pub t_rise: f64,  // e.g., 100e-12 (100 ps)
    pub t_fall: f64,  // e.g., 100e-12

    // ── Internal state ──
    /// The time at which the current transition started.
    transition_start: f64,
    /// The voltage at transition_start.
    v_from: f64,
    /// The target voltage of the current transition.
    v_to: f64,
}
```

**Key method: `voltage_at(t: f64) -> f64`**

This returns the instantaneous voltage at time `t` during a transition. It computes a linear ramp
between `v_from` and `v_to` over the appropriate rise/fall time. If `t` is past the transition end,
it returns `v_to`. If no transition is active, it returns the steady-state value.

**How it participates in the analog solve:**

The D2A connector does NOT stamp into the Jacobian matrix. Instead, it acts as a **controlled
voltage source**. During `TransientSystem::assemble()`, the coordinator reads
`d2a.voltage_at(current_time)` and injects it as a known-voltage boundary condition on
`d2a.analog_node`. This is equivalent to setting a fixed voltage on that node (grounding the node
through a very small resistance to the target voltage, i.e., a Norton equivalent with `G = 1/R_src`
where `R_src` is very small).

**Design Decision: Linear ramp, not RC or sigmoid.**

A piecewise-linear ramp is trivially differentiable (constant derivative during transition, zero
outside) and produces clean Jacobian entries. RC curves or sigmoids add unnecessary complexity for
the initial implementation.

### 4.2 A2D Connect Module (Analog → Digital)

```rust
/// An A2D connector monitors an analog voltage and generates digital events
/// when thresholds are crossed.
pub struct A2DConnector {
    /// The analog node this connector monitors.
    pub analog_node: AnalogReference,

    /// The digital port this connector drives.
    pub digital_port: PortId,

    /// Threshold voltage for logic high.
    pub v_threshold: f64,  // e.g., 0.9 (VDD/2)

    /// Hysteresis band (optional, prevents oscillation at threshold).
    pub hysteresis: f64,   // e.g., 0.1

    // ── Internal state ──
    /// Last known digital value (to detect edges, not levels).
    last_digital_value: LogicValue,
    /// Voltage at previous accepted timestep (for interpolation).
    v_prev: f64,
}
```

**Key method: `check_crossing(v_old: f64, v_new: f64) -> Option<(CrossingDirection, f64)>`**

Returns `Some((Rising, t_cross))` or `Some((Falling, t_cross))` if a threshold was crossed, or
`None` if no crossing occurred. The crossing time is computed via linear interpolation.

**Hysteresis logic:**

- A rising crossing is detected only when `v_old < (v_threshold - hysteresis/2)` and
  `v_new >= (v_threshold + hysteresis/2)`.
- A falling crossing is detected only when `v_old > (v_threshold + hysteresis/2)` and
  `v_new <= (v_threshold - hysteresis/2)`.

This prevents rapid toggling when the analog signal hovers near the threshold.

---

## 5. The Digital Engine

### 5.1 Overview

The digital engine is a discrete-event simulator. It maintains:
1. A **time-ordered event queue** (priority queue / min-heap keyed by time).
2. A **net state table** mapping each digital net to its current `LogicValue`.
3. A set of **D-OSDI device instances**, each loaded from a compiled `.so` library.

### 5.2 LogicValue

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicValue {
    Zero,
    One,
    X,     // Unknown / uninitialized
    Z,     // High-impedance
}
```

**Design Decision: 4-value logic (IEEE 1364).**

We use 4-value logic (0, 1, X, Z) rather than 2-value (0, 1). This is necessary to correctly model
tri-state buses, uninitialized registers, and contention. It matches the Verilog standard.

### 5.3 Event

```rust
pub struct DigitalEvent {
    /// The time at which this event occurs.
    pub time: f64,
    /// The net whose value changes.
    pub net: DigitalNetId,
    /// The new value of the net.
    pub value: LogicValue,
    /// The source device that generated this event (for debugging / cancellation).
    pub source: DeviceInstanceId,
}
```

### 5.4 Evaluation Loop (`evaluate_until_stable`)

When the coordinator calls `digital_engine.evaluate_until_stable(t)`, the engine:

1. **Dequeues all events at time `t`** from the event queue.
2. **Applies** them to the net state table.
3. **Identifies** which D-OSDI devices have inputs that changed.
4. **Calls `eval_digital()`** on each affected device.
5. **Collects** new events generated by the devices.
6. If any new events have time `t` (zero-delay, i.e., a delta cycle), **go to step 1**.
7. If all new events have time `> t`, **stop**. The digital state is stable at time `t`.

**Delta cycle limit:** To prevent infinite loops from combinational feedback, enforce a maximum
delta cycle count (e.g., 1000). If exceeded, flag the offending nets as `X` and emit a warning.

### 5.5 Commit / Rollback

```rust
impl DigitalEngine {
    /// Snapshot the current state. Called before the analog solver runs.
    fn checkpoint(&mut self) { ... }

    /// Discard events added since the last checkpoint.
    /// Restore net state to the checkpoint.
    fn rollback(&mut self) { ... }

    /// Make the current state permanent. Called after the analog solver converges.
    fn commit(&mut self) { ... }
}
```

**Design Decision: Checkpoint/Rollback instead of shadow copies per device.**

We snapshot the entire engine state (net table + event queue) rather than requiring each D-OSDI
device to maintain its own shadow state. This is simpler and less error-prone because:
- The engine state is small (a HashMap + a BinaryHeap).
- Device implementations don't need to know about rollback semantics.
- We can add per-device rollback later if performance requires it.

---

## 6. The D-OSDI ABI (Digital Device Interface)

### 6.1 Motivation

The analog OSDI ABI centers on continuous evaluation: the simulator calls `eval()` at every
Newton-Raphson iteration, passing node voltages and receiving currents/charges/Jacobians. This
makes no sense for digital logic, which operates on discrete values and only needs evaluation when
inputs change.

D-OSDI defines a minimal C ABI for event-driven digital devices. Like OSDI, devices are compiled
to `.so` shared libraries and loaded at runtime via `dlopen`.

### 6.2 The Descriptor

```c
typedef struct {
    const char* name;              // Device name, e.g., "nand2", "dff"

    uint32_t num_input_ports;      // Number of input ports
    uint32_t num_output_ports;     // Number of output ports
    uint32_t num_inout_ports;      // Number of bidirectional ports (e.g., tri-state bus)
    const DigitalPortDescriptor* ports;  // Array of port descriptors

    uint32_t num_params;           // Number of parameters (e.g., delay values)
    const DigitalParamDescriptor* params;

    uint32_t instance_size;        // Size in bytes of per-instance opaque state
    uint32_t model_size;           // Size in bytes of per-model opaque state

    // ── Function pointers ──

    /// Initialize model-level data (called once per unique model).
    void (*setup_model)(void* model_data, const DigitalSimParas* sim);

    /// Initialize instance-level data (called once per instantiation).
    void (*setup_instance)(void* inst_data, void* model_data, const DigitalSimParas* sim);

    /// Evaluate the device. Called when one or more inputs change.
    /// Reads input values from `inputs[]`, writes output values to `outputs[]`,
    /// and appends scheduled events to `event_queue`.
    /// Returns 0 on success, nonzero on fatal error.
    uint32_t (*eval)(
        void* inst_data,
        void* model_data,
        const LogicValue* inputs,        // [num_input_ports + num_inout_ports]
        LogicValue* outputs,             // [num_output_ports + num_inout_ports]
        DigitalEventQueue* event_queue,  // Opaque queue handle; device appends events
        double current_time
    );

    /// Read a parameter value (for inspection / debugging).
    void* (*access)(void* inst_data, void* model_data, uint32_t param_id, uint32_t flags);

} DigitalDescriptor;
```

### 6.3 Port Descriptor

```c
typedef struct {
    const char* name;       // Port name, e.g., "A", "B", "Q", "Qn"
    uint32_t direction;     // 0 = input, 1 = output, 2 = inout
    uint32_t width;         // Bit width (1 for scalar, >1 for bus)
} DigitalPortDescriptor;
```

### 6.4 Event Queue Interface

The `DigitalEventQueue` is an opaque handle provided by the simulator. The device appends events
to it via a C callback:

```c
typedef struct {
    void* handle;
    void (*schedule)(void* handle, uint32_t port_idx, LogicValue value, double delay);
} DigitalEventQueue;
```

**Design Decision: Delay-relative scheduling.**

The device specifies output changes as `delay` (relative to `current_time`), not as absolute times.
This is simpler for device authors and matches Verilog semantics (`#5 Q = 1` means "5 time units
from now").

### 6.5 Simulation Parameters

```c
typedef struct {
    double timescale;        // Time unit in seconds (e.g., 1e-9 for ns)
    double temperature;      // Temperature in Kelvin
} DigitalSimParas;
```

### 6.6 Differences from Analog OSDI

| Aspect | Analog OSDI | Digital D-OSDI |
|--------|-------------|----------------|
| Evaluation trigger | Every NR iteration | Only when inputs change |
| Input/output type | `f64` voltages/currents | `LogicValue` (0, 1, X, Z) |
| Time model | Continuous (`abstime`) | Discrete events with delays |
| Matrix participation | Stamps into Jacobian | None — pure behavioral |
| State management | `prev_state` / `next_state` f64 arrays | Opaque `inst_data` blob |
| Noise | `load_noise()` PSD | Not applicable |
| Bound step | `bound_step_offset` | Not applicable (events are explicit) |

---

## 7. Hybrid Modules (Mixed-Signal Compilation)

When our compiler encounters a Verilog-AMS module that mixes analog and digital ports:

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

The compiler must split this into components that the cosimulation framework can manage:

### 7.1 OSDI Devices That Receive Digital Signals

The digital input `en` cannot appear in an OSDI `eval()` function because OSDI only understands
analog nodes. The compiler transforms the digital input into an additional analog terminal:

1. **Emit an extra OsdiNode** in the OSDI descriptor for `en`, named `__d2a_en`.
2. **In the generated `eval()` C code**, replace reads of `en` with:
   ```c
   int en_val = (prev_solve[node_mapping[__d2a_en_idx]] > 0.5) ? 1 : 0;
   ```
3. **In the Piperine netlist**, the elaborator automatically instantiates a `D2AConnector`
   driving the `__d2a_en` analog node.

### 7.2 OSDI Devices That Emit Digital Events

When a Verilog-AMS module contains `@(cross(...))` statements that assign to digital outputs:

1. **Emit an extra OsdiNode** in the OSDI descriptor for the crossing monitor, named `__a2d_out`.
2. **In the generated `eval()` C code**, write the monitored analog expression to that node's
   residual (so the solver sees it as a normal voltage).
3. **In the Piperine netlist**, the elaborator automatically instantiates an `A2DConnector`
   monitoring the `__a2d_out` analog node and scheduling events on the corresponding digital net.

### 7.3 Design Decision: Ghost Nodes Over ABI Extensions

We do NOT extend the OSDI ABI to support digital signals natively. Instead, we map digital
signals to analog "ghost nodes" at compile time.

**Rationale:**
- Preserves full compatibility with the standard OSDI ABI.
- Any existing OSDI `.so` (compiled by OpenVAF or any future compiler) works unmodified.
- The D2A/A2D translation overhead is negligible (one extra node per digital pin).
- The complexity lives in the compiler and elaborator, not in the runtime hot path.

---

## 8. Integration Points with Existing Code

### 8.1 Changes to `TransientSolver`

The current `TransientSolver::solve()` loop (in `solver/transient.rs`) uses a fixed timestep and
has no concept of external time constraints. It must be refactored to:

1. Accept a `CosimCoordinator` reference (or trait object) that can inject breakpoints.
2. Call `coordinator.compute_next_dt()` instead of using `self.options.dt` directly.
3. After convergence, call `coordinator.post_convergence_checks()` which runs A2D detection.
4. Support timestep rejection (already partially there via error return from `execute_timestep`).

### 8.2 Changes to `CircuitInstance`

`CircuitInstance` currently holds only `Vec<OsdiRuntime>`. It must be extended to also hold:
- `Vec<D2AConnector>` — driven by the coordinator during Phase 2.
- `Vec<A2DConnector>` — checked by the coordinator during Phase 4.

During `assemble()`, D2A connectors contribute voltage source stamps to the matrix just like any
other device.

### 8.3 New Crate: `piperine-digital`

The digital engine should live in a new crate `piperine-digital` with the following modules:

```
crates/piperine-digital/src/
    lib.rs              // Public API
    engine.rs           // DigitalEngine: event queue, net state, evaluation loop
    event.rs            // DigitalEvent, EventQueue types
    logic.rs            // LogicValue, resolution functions
    dosdi/
        ffi.rs          // D-OSDI C ABI struct definitions
        loader.rs       // dlopen + symbol resolution for D-OSDI .so files
        runtime.rs      // DigitalRuntime: per-instance state management
```

### 8.4 New Module in `piperine-solver`: `cosim`

```
crates/piperine-solver/src/
    cosim/
        mod.rs           // CosimCoordinator
        connector.rs     // D2AConnector, A2DConnector
        breakpoint.rs    // BreakpointQueue (sorted time instants)
```

---

## 9. Implementation Order

| Phase | Deliverable | Depends On |
|-------|-------------|------------|
| **Phase 1** | `D2AConnector` and `A2DConnector` structs with unit tests | Nothing |
| **Phase 2** | `BreakpointQueue` and `CosimCoordinator` skeleton | Phase 1 |
| **Phase 3** | Refactor `TransientSolver::solve()` to accept coordinator | Phase 2 |
| **Phase 4** | `piperine-digital` crate: `LogicValue`, `DigitalEvent`, `EventQueue` | Nothing |
| **Phase 5** | `DigitalEngine` with `evaluate_until_stable()`, checkpoint/rollback | Phase 4 |
| **Phase 6** | D-OSDI loader (`dlopen` for digital `.so` files) | Phase 5 |
| **Phase 7** | End-to-end integration test: inverter driving RC load | All above |
| **Phase 8** | Compiler support for ghost node emission (hybrid modules) | Phase 7 |

Phases 1-3 and 4-6 can proceed in parallel.

---

## 10. Test Strategy

### 10.1 Unit Tests

- `D2AConnector::voltage_at()` returns correct ramp values at boundary times.
- `A2DConnector::check_crossing()` detects crossings with hysteresis correctly.
- `BreakpointQueue` returns times in order and deduplicates.
- `LogicValue` resolution: `One` vs `Zero` = `X`, `Z` vs `One` = `One`, etc.
- `EventQueue` ordering: events at same time processed in insertion order.

### 10.2 Integration Tests

- **Pure digital:** Ring oscillator (chain of inverters). Verify oscillation period matches
  expected gate delays.
- **Pure analog with breakpoints:** RC circuit where an external breakpoint forces a timestep at
  a specific time. Verify solution matches reference.
- **Mixed-signal:** Digital clock driving an analog RC filter via D2A. Verify the analog output
  shows the expected exponential charging/discharging waveform.
- **A2D feedback loop:** Analog ramp crossing a threshold, generating a digital event, which
  toggles a D2A that changes the analog circuit. Verify the feedback settles.

# Piperine Mixed-Signal Simulation — Development Specification

> **Purpose:** Complete, unambiguous development specification for mixed-signal simulation in
> Piperine. Written for an implementer with no prior context.

---

## 1. Core Principle: One Simulator, Not Two

There is no "cosimulation." There is one simulator that understands two kinds of variables:
**continuous** (analog) and **discrete** (digital). The transient loop is a single unified loop
that happens to contain both a matrix solver and an event queue as internal mechanisms.

**Design Decision: No `CosimCoordinator`.**

A separate coordinator implies two independent engines being synchronized externally. That is the
wrong mental model. Instead, the `TransientSolver` itself is extended to manage both continuous
integration and discrete events as part of its own state. The analog matrix and the digital event
queue are peers inside a single `SimulationState`, not foreign systems requiring a mediator.

**Rationale:** In Verilog-AMS, a module's ports are just ports. The discipline (electrical, logic)
determines behavior, but the port identity is unified. A signal named `clk` is `clk` whether it
carries voltage or logic. Our architecture must reflect this: one namespace, one time authority,
one simulation.

---

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Continuous variable** | A variable solved by Newton-Raphson at every timestep. Voltages, currents, charges. Represented by `AnalogVariable` in the existing netlist. |
| **Discrete variable** | A variable that changes only at specific event times. Logic values (0, 1, X, Z). Stored in a flat state table. |
| **OSDI** | The existing C ABI for analog device models. Devices contribute to the Jacobian matrix. Already implemented. |
| **D-OSDI** | A new C ABI for digital device models. Devices respond to input events and schedule output events. Does NOT participate in the matrix. |
| **Connect module** | A device that has BOTH analog ports and digital ports. It translates between continuous and discrete domains. Implemented as a hybrid that participates in both the matrix (analog side) and the event queue (digital side). |
| **Breakpoint** | A time instant forced into the stepper's schedule because a digital event or a connect module transition requires it. |
| **Zero-crossing** | The exact time a continuous signal crosses a threshold. Detected by a connect module's A2D logic after the analog solver converges a timestep. |
| **Delta cycle** | One round of combinational propagation at zero elapsed time in the digital domain. |

---

## 3. The Unified Port Model

### 3.1 Current State

The existing netlist uses `AnalogVariable` (an enum of `Node`, `Branch`, `Time`, `Frequency`,
`Iteration`) and `AnalogReference` (an `AnalogVariable` + matrix index). These are purely
continuous concepts.

### 3.2 Required Extension

We need a unified port identity that the compiler, elaborator, and solver all share. A port is
just a named connection point on a module. Its *discipline* determines how it participates in
simulation.

```rust
/// A port identifier as declared in source code.
/// This is the compiler-facing identity. It has no matrix index, no event queue slot.
/// Those bindings are created during elaboration.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct PortId {
    /// The module instance that owns this port.
    pub instance: String,
    /// The port name as declared in source (e.g., "clk", "vdd", "out").
    pub name: String,
}
```

During elaboration, each `PortId` is resolved to one of:
- An `AnalogReference` (if the port's discipline is continuous, e.g., `electrical`).
- A `DigitalNetId` (if the port's discipline is discrete, e.g., `logic`).
- **Both** (if the port sits on a connect module boundary — it has a ghost analog node AND a
  digital net, linked by the connect module's internal logic).

```rust
/// How a port is bound in the simulation.
#[derive(Debug, Clone)]
pub enum PortBinding {
    /// Pure analog: participates in the matrix.
    Continuous(AnalogReference),
    /// Pure digital: participates in the event queue.
    Discrete(DigitalNetId),
    /// Mixed boundary: the connect module owns both handles.
    /// The AnalogReference is the ghost node; the DigitalNetId is the event net.
    Bridge {
        analog: AnalogReference,
        digital: DigitalNetId,
    },
}
```

**Design Decision: PortId is discipline-agnostic.**

The `PortId` type does not encode whether a port is analog or digital. That information lives in
the discipline annotation from the source code and is resolved during elaboration. This means the
compiler can name ports uniformly, and the elaborator decides how to bind them. No special
"digital port" vs "analog port" type proliferation.

**Rationale:** In Verilog-AMS, `module foo(a, b, c)` declares three ports. The disciplines are
declared separately: `electrical a; logic b; electrical c;`. If we bake the discipline into the
port identity type, we force the compiler to resolve disciplines before it can even name ports.
Keeping `PortId` discipline-agnostic mirrors the language semantics and avoids premature coupling.

---

## 4. Connect Modules Are Devices, Not Special Structs

### 4.1 The Problem with Separate D2A/A2D Structs

The previous version of this spec defined `D2AConnector` and `A2DConnector` as standalone Rust
structs with no ABI. This creates problems:
- They become a special case in every part of the system (elaborator, solver loop, result
  collection).
- Users cannot define custom connect rules (the Verilog-AMS `connectrules` feature).
- The D2A and A2D behaviors are artificially separated when in reality they are two facets of
  the same boundary device.

### 4.2 Connect Modules as Hybrid Devices

A connect module is a device that has **both** analog and digital ports. It is compiled (by our
compiler) into a `.so` that exposes:
- An **OSDI descriptor** for its analog side (the ghost node that participates in the matrix).
- A **D-OSDI descriptor** for its digital side (the event port).
- A shared `inst_data` blob that both sides read/write.

At the OSDI level, the connect module's `eval()` produces the smooth voltage ramp (D2A direction)
or writes the monitored analog voltage to a known offset in `inst_data` (A2D direction).

At the D-OSDI level, the connect module's `eval_digital()`:
- **D2A direction:** Reads the new digital input value and writes a target voltage + transition
  start time into `inst_data`. The OSDI `eval()` reads these to compute the ramp.
- **A2D direction:** Reads the monitored voltage from `inst_data`, applies threshold + hysteresis
  logic, and schedules a digital event if a crossing occurred.

**The simulator treats connect modules exactly like any other device.** The only special thing
about them is that they appear in BOTH the analog device list and the digital device list,
sharing the same `inst_data`. The elaborator is responsible for linking the two halves.

### 4.3 Zero-Crossing Ownership

The connect module itself owns the zero-crossing detection logic. The simulator does NOT inspect
analog node voltages to look for crossings. Instead:

1. After the analog solver converges a timestep, the simulator calls `accept_timestep()` on all
   OSDI devices (including connect modules).
2. The connect module's `accept_timestep()` (or a post-convergence hook) compares the current
   monitored voltage against the previous voltage and the threshold.
3. If a crossing is detected, the connect module computes `t_cross` internally and schedules
   a digital event via the D-OSDI event queue interface.

**Rationale:** This keeps crossing logic encapsulated in the device where it belongs. Different
connect modules can use different thresholds, hysteresis bands, or interpolation methods without
the simulator core knowing about any of it.

### 4.4 Standard Library Connect Modules

We ship two built-in connect modules:

**`ppr_d2a` (Digital-to-Analog):**
- Digital port: 1 input.
- Analog port: 2 terminals (p, n — voltage source).
- Parameters: `v_low` (default 0.0), `v_high` (default 1.8), `t_rise` (default 100ps),
  `t_fall` (default 100ps).
- Behavior: On digital input change, ramps the analog voltage linearly from current value to
  target over `t_rise` or `t_fall`.

**`ppr_a2d` (Analog-to-Digital):**
- Analog port: 2 terminals (p, n — voltage monitor).
- Digital port: 1 output.
- Parameters: `v_threshold` (default 0.9), `hysteresis` (default 0.1).
- Behavior: After each converged analog timestep, checks if V(p,n) crossed the threshold band.
  If so, computes `t_cross` via linear interpolation and schedules a digital event.

**`ppr_bidir` (Bidirectional):**
- Combines D2A and A2D. Digital input controls the analog source; analog voltage monitors back
  to digital output. Used for bidirectional buses where both directions are active.

---

## 5. The Unified Simulation Loop

### 5.1 Transient Analysis

The transient loop lives in the existing `TransientSolver`. It is extended, not replaced.

```
FUNCTION solve(stop_time):
    t = 0.0
    dc_solution = solve_dc()
    digital_state.initialize(dc_solution)   // A2D modules read DC voltages, set initial logic

    WHILE t < stop_time:
        // ── Compute next time ──
        dt_proposed = stepper.propose_dt()              // LTE, bound_step
        t_next_event = digital_state.peek_next_event()  // may be INFINITY
        t_next = min(t + dt_proposed, t_next_event, stop_time)
        dt = t_next - t

        // ── Process pending digital events at t_next ──
        // This includes events from A2D crossings detected in the previous step,
        // as well as events generated by D-OSDI devices themselves.
        digital_state.evaluate_until_stable(t_next)

        // ── Solve the analog timestep [t, t_next] ──
        // During NR iterations:
        //   - OSDI devices (including connect modules' analog halves) are evaluated.
        //   - Connect module D2A sides compute time-varying ramp values.
        //   - The matrix is assembled and solved as usual.
        result = newton_raphson.solve(t_next, dt)

        IF result == FAILED:
            dt = dt / 2
            digital_state.rollback()
            CONTINUE

        // ── Post-convergence: let connect modules detect crossings ──
        // Connect modules compare v_prev vs v_now internally.
        // If a crossing is detected, they schedule events into digital_state.
        for device in connect_modules:
            device.post_convergence(t, t_next, &analog_solution, &mut digital_state)

        // ── Accept and advance ──
        digital_state.commit()
        accept_timestep()
        t = t_next
```

### 5.2 Design Decision: Digital Events Processed Before Analog Solve

Digital events at `t_next` are resolved BEFORE the analog solver runs for `[t, t_next]`.

**Rationale:** The D2A side of connect modules must know the current digital state to compute
correct ramp targets. If we solved the analog step first, connect modules would use stale digital
values, requiring a rollback and re-solve once the digital events are processed. Processing events
first avoids this.

### 5.3 AC / Noise / Transfer Function (Analog-Only Analyses)

These analyses have no digital component. When the simulation is purely analog:
- The digital event queue is empty.
- `peek_next_event()` returns `INFINITY`.
- Connect modules' digital ports are initialized to a fixed value from the DC operating point
  and never change.
- The analysis proceeds exactly as it does today, with zero overhead.

**Design Decision: No special-casing for pure-analog.**

The unified loop handles this naturally. There is no `if has_digital { ... } else { ... }` branch.
The event queue is simply empty, and `min(t + dt_proposed, INFINITY)` equals `t + dt_proposed`.

---

## 6. The D-OSDI ABI

### 6.1 Scope

D-OSDI defines a C ABI for discrete event devices. Like OSDI, devices are compiled to `.so`
shared libraries and loaded at runtime via `dlopen`. Unlike OSDI, D-OSDI devices do not stamp into
a matrix. They consume and produce events.

### 6.2 The Descriptor

```c
typedef struct {
    const char* name;                      // e.g., "nand2", "dff", "ppr_a2d"

    uint32_t num_ports;                    // Total port count
    const DosdiPort* ports;                // Port descriptors (direction, width)

    uint32_t num_params;
    const DosdiParam* params;              // Parameter descriptors

    uint32_t instance_size;                // Per-instance opaque state (bytes)
    uint32_t model_size;                   // Per-model opaque state (bytes)

    // ── Lifecycle ──
    void (*setup_model)(void* model_data, const DosdiSimParas* sim);
    void (*setup_instance)(void* inst_data, void* model_data, const DosdiSimParas* sim);

    // ── Evaluation ──
    // Called when one or more input ports change value.
    // The device reads inputs, updates internal state, and schedules output events.
    // Returns 0 on success, nonzero on fatal error.
    uint32_t (*eval)(
        void* inst_data,
        void* model_data,
        const uint8_t* inputs,         // LogicValue per input bit (packed by port order)
        uint8_t* outputs,              // Device writes output values here
        DosdiEventSink* event_sink,    // Opaque handle to schedule future events
        double current_time
    );

    // ── Parameter access ──
    void* (*access)(void* inst_data, void* model_data, uint32_t param_id, uint32_t flags);

} DosdiDescriptor;
```

### 6.3 Port Descriptor

```c
typedef struct {
    const char* name;         // Port name, matching the Verilog-AMS source
    uint32_t direction;       // DOSDI_DIR_INPUT=0, DOSDI_DIR_OUTPUT=1, DOSDI_DIR_INOUT=2
    uint32_t width;           // Bit width: 1 for scalar, >1 for vector/bus
} DosdiPort;
```

**Design Decision: Port names match source identifiers exactly.**

The `name` field in `DosdiPort` uses the same string as the Verilog-AMS port declaration. This
allows the elaborator to match OSDI analog ports and D-OSDI digital ports by name, without
maintaining a separate mapping table. When a connect module has both an OSDI and D-OSDI half, the
elaborator links them by matching port names.

### 6.4 Event Scheduling

```c
// Provided by the simulator, not the device.
typedef struct {
    void* handle;
    // Schedule a value change on output port `port_idx` after `delay` seconds.
    void (*schedule)(void* handle, uint32_t port_idx, uint8_t value, double delay);
    // Cancel all pending events on output port `port_idx` (inertial delay model).
    void (*cancel)(void* handle, uint32_t port_idx);
} DosdiEventSink;
```

**Design Decision: Relative delays, not absolute times.**

Devices specify delays relative to `current_time`. This matches Verilog semantics (`#5ns Q = 1`).
The simulator converts to absolute time internally.

**Design Decision: `cancel` for inertial delay.**

Verilog uses inertial delay by default: if a new event is scheduled on a port before a previous
pending event fires, the previous event is cancelled. The `cancel` callback supports this. Without
it, we could not correctly model standard cell behavior.

### 6.5 LogicValue Encoding

```c
// Single-byte encoding, matching Verilog 4-value logic.
#define DOSDI_LOGIC_0  0
#define DOSDI_LOGIC_1  1
#define DOSDI_LOGIC_X  2
#define DOSDI_LOGIC_Z  3
```

### 6.6 Simulation Parameters

```c
typedef struct {
    double timescale;         // Time unit in seconds (e.g., 1e-9 for `timescale 1ns)
    double temperature;       // Kelvin
    double supply_voltage;    // VDD, used by connect modules for default v_high
} DosdiSimParas;
```

---

## 7. The Digital State Machine

### 7.1 Data Structures

The digital state is NOT a separate "engine." It is a component of `SimulationState`, alongside
the existing `CircularArrayBuffer2` (analog state history).

```rust
pub struct DigitalState {
    /// Current value of every digital net. Indexed by DigitalNetId.
    pub nets: Vec<LogicValue>,

    /// Time-ordered priority queue of future events.
    pub event_queue: BinaryHeap<Reverse<DigitalEvent>>,

    /// Checkpoint for rollback (cloned on checkpoint()).
    checkpoint: Option<DigitalStateSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DigitalNetId(pub usize);
```

### 7.2 Evaluation

```rust
impl DigitalState {
    /// Process all events at time `t` and propagate through D-OSDI devices
    /// until no more zero-delay events remain (delta cycle resolution).
    pub fn evaluate_until_stable(&mut self, t: f64, devices: &mut [DosdiRuntime]) {
        let mut delta_count = 0;
        loop {
            let events_at_t = self.drain_events_at(t);
            if events_at_t.is_empty() { break; }

            // Apply events to net state
            for event in &events_at_t {
                self.nets[event.net.0] = event.value;
            }

            // Find devices with changed inputs, evaluate them
            let changed_nets: HashSet<DigitalNetId> = events_at_t.iter()
                .map(|e| e.net).collect();

            for device in devices.iter_mut() {
                if device.has_input_on(&changed_nets) {
                    device.eval(t, &self.nets, &mut self.event_queue);
                }
            }

            delta_count += 1;
            if delta_count > 1000 {
                log::warn!("Delta cycle limit exceeded at t={}", t);
                break;
            }
        }
    }

    pub fn peek_next_event_time(&self) -> f64 {
        self.event_queue.peek()
            .map(|Reverse(e)| e.time)
            .unwrap_or(f64::INFINITY)
    }

    pub fn checkpoint(&mut self) { ... }
    pub fn rollback(&mut self) { ... }
    pub fn commit(&mut self) { self.checkpoint = None; }
}
```

### 7.3 Design Decision: `DigitalState` Lives Inside `SimulationState`, Not in a Separate Crate

The digital event queue and net state are fields of the solver's simulation state, not a foreign
engine accessed through a trait boundary.

**Rationale:** Connect modules need to simultaneously read analog solution vectors AND schedule
digital events. If the digital engine were a separate crate behind a trait, connect modules would
need complex borrowing patterns or message passing. Colocating the state makes the borrow checker
happy and the code straightforward.

When the codebase grows large enough to warrant separation, we extract a crate. Not before.

---

## 8. Hybrid Modules (Compilation Strategy)

When our compiler encounters a module with mixed-discipline ports:

```verilog
module ldo_enable(en, vin, vout, gnd);
    input logic en;
    inout electrical vin, vout, gnd;
    ...
endmodule
```

### 8.1 Ghost Nodes

The compiler emits the analog body as a standard OSDI `.so`. The digital input `en` becomes an
extra `OsdiNode` terminal named `__bridge_en`. In the generated `eval()` C code, reads of `en`
become comparisons against the ghost node voltage:

```c
int en_val = (prev_solve[node_mapping[bridge_en_idx]] > 0.5) ? 1 : 0;
```

### 8.2 Automatic Connect Module Insertion

The elaborator detects that port `en` has discipline `logic` but is connected to an OSDI device
expecting an analog terminal. It automatically inserts a `ppr_d2a` connect module instance:
- Digital input: bound to the `en` net in the digital state.
- Analog output: bound to the `__bridge_en` ghost node.

This happens transparently. The user's source code is unchanged. The elaborator's connect rule
table determines which connect module to insert based on the source and destination disciplines.

### 8.3 Design Decision: Ghost Nodes, Not ABI Extensions

We do NOT extend the OSDI ABI to handle digital signals. Instead, we map digital signals to
analog proxy nodes at compile time.

**Rationale:**
- Preserves OSDI ABI compatibility. Third-party OSDI models work unmodified.
- The overhead is one extra matrix node per digital-to-analog boundary. Negligible.
- All complexity is in the compiler and elaborator, not in the runtime hot path.
- If we later build our own compilation backend (replacing OpenVAF), we emit OSDI descriptors
  with ghost nodes as part of our standard compilation pipeline.

---

## 9. Integration with Existing Code

### 9.1 Extended `CircuitInstance`

```rust
pub struct CircuitInstance {
    pub title: String,
    pub runtimes: Vec<OsdiRuntime>,          // Analog devices (unchanged)
    pub digital_runtimes: Vec<DosdiRuntime>,  // Digital devices (new)
    pub digital_state: DigitalState,          // Net values + event queue (new)
    pub netlist: Netlist,                     // Analog netlist (unchanged)
}
```

Connect modules appear in BOTH `runtimes` (for their analog half) and `digital_runtimes` (for
their digital half). They share the same `inst_data` memory, linked during elaboration.

### 9.2 Extended `TransientSolver::solve()`

The existing loop in `solver/transient.rs` gains three additions:
1. Before each NR solve: `self.circuit.digital_state.evaluate_until_stable(t_next, ...)`.
2. After convergence: call post-convergence hooks on connect modules (crossing detection).
3. Timestep proposal: `dt = min(dt_proposed, digital_state.peek_next_event_time() - t)`.

The loop structure remains a single `while t < stop_time` loop. No nested loops, no coordinator.

### 9.3 File Layout

```
crates/piperine-solver/src/
    digital/
        mod.rs          // DigitalState, LogicValue, DigitalEvent, DigitalNetId
        dosdi/
            ffi.rs      // D-OSDI C ABI struct definitions (DosdiDescriptor, etc.)
            loader.rs   // dlopen + symbol resolution for D-OSDI .so files
            runtime.rs  // DosdiRuntime: per-instance state, eval() wrapper
    solver/
        transient.rs    // Extended with digital event processing (modified)
    circuit/
        instance.rs     // Extended with digital_runtimes, digital_state (modified)
```

**Design Decision: `digital/` module inside `piperine-solver`, not a separate crate.**

**Rationale:** The digital state is tightly coupled to the transient solver loop. A separate crate
would require defining trait abstractions for the coupling points, adding complexity without
benefit at this stage. We can extract later if needed.

---

## 10. Implementation Phases

| Phase | Deliverable | Depends On |
|-------|-------------|------------|
| **1** | `LogicValue`, `DigitalEvent`, `DigitalNetId`, `DigitalState` (with event queue, evaluate_until_stable, checkpoint/rollback) + unit tests | Nothing |
| **2** | D-OSDI FFI struct definitions (`DosdiDescriptor`, `DosdiPort`, etc.) | Nothing |
| **3** | `DosdiRuntime` (loader, instance allocation, eval wrapper) | Phase 2 |
| **4** | `PortId`, `PortBinding` types in `circuit/netlist.rs` | Nothing |
| **5** | Extend `CircuitInstance` with `digital_runtimes` and `digital_state` | Phases 1, 3, 4 |
| **6** | Extend `TransientSolver::solve()` with event processing and breakpoint logic | Phase 5 |
| **7** | Built-in connect modules (`ppr_d2a`, `ppr_a2d`) as compiled `.so` or Rust-native | Phase 6 |
| **8** | Integration test: digital clock → D2A → RC filter → A2D → digital counter | Phase 7 |

Phases 1, 2, and 4 can proceed in parallel.

---

## 11. Test Strategy

### 11.1 Unit Tests (Phase 1)

- `LogicValue` resolution: `Zero` drives against `One` → `X`. `Z` against `One` → `One`.
- `DigitalState::evaluate_until_stable()` with a chain of 3 inverters: verifies delta cycles.
- `DigitalState::checkpoint()` + `rollback()` restores prior state exactly.
- Event queue ordering: events at same time processed FIFO within that time.

### 11.2 Integration Tests (Phase 8)

- **Pure digital:** Ring oscillator (5 inverters). Period = 10 × gate_delay.
- **D2A only:** Digital square wave → `ppr_d2a` → RC low-pass filter (OSDI resistor + capacitor).
  Verify exponential charge/discharge in analog output.
- **A2D only:** Analog ramp (OSDI voltage source) → `ppr_a2d` → digital net.
  Verify event time matches expected `t_cross` within linear interpolation error.
- **Full loop:** Clock → D2A → analog amplifier → A2D → digital flip-flop.
  Verify the flip-flop captures the correct logic value at each clock edge.

### 11.3 Regression Guard

All existing analog-only tests (`cargo test -p piperine-solver`, currently 27 tests) must continue
to pass without modification. The digital extensions must not change the behavior of pure-analog
simulations.

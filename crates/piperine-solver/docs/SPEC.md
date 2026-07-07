# Piperine Solver Specification

## 1. Overview
The `piperine-solver` crate is the numerical and topological simulation engine of the Piperine toolchain. It consumes a `CircuitInstance` (a linked netlist of devices, provided by `piperine-codegen`) and executes the requested analyses (DC, AC, Transient, Noise, TF). It is also responsible for driving event-driven digital logic and coordinating mixed-signal boundaries (A2D/D2A).

Unlike `piperine-lang` (which deals with syntax and elaboration) and `piperine-codegen` (which handles POM lowering and JIT compilation), the solver purely executes mathematical models and graph topology. It has **no dependency** on the AST or the codegen stages. 

*Dependency Direction*: The solver is the bottom-most computational layer. `piperine-codegen` depends on `piperine-solver` (to instantiate its structs and traits), not the other way around.

---

## 2. Architecture & Directory Structure

The solver is strictly partitioned into distinct modules to enforce separation of concerns between analog physics, digital events, core orchestration, and numerical algorithms.

```text
crates/piperine-solver/src/
├── core/
│   ├── circuit.rs      # CircuitInstance: The root orchestration structure containing devices and state.
│   └── device.rs       # The Device, AnalogDevice, and DigitalDevice traits.
├── analog/
│   └── netlist.rs      # Analog node mapping, matrix sizing, and topological mappings.
├── digital/
│   ├── events.rs       # DigitalState, DigitalEvent, and LogicValue structures.
│   ├── interface.rs    # Mixed-signal A2D and D2A bridge definitions.
│   └── scheduler.rs    # Event queue, delta-cycle evaluation, and DAG-ordered digital execution.
├── solver/
│   ├── dc.rs           # Non-linear DC operating point solver.
│   ├── ac.rs           # Linearized AC frequency sweep solver.
│   ├── transient.rs    # Time-domain solver with dynamic time-stepping.
│   ├── noise.rs        # Adjoint Matrix-based Noise analysis.
│   └── tf.rs           # Small-signal Transfer Function analysis.
├── math/               # Sparse matrices (faer), integration formulas, Newton-Raphson driver.
├── analysis/           # Configuration structures (e.g., TransientAnalysisOptions).
└── osdi/               # OpenVAF / OSDI shared library loader for Verilog-A models.
```

---

## 3. Core Traits: The Device Interface

To cleanly decouple analog continuous-time evaluation from digital event-driven evaluation, components implement specific capability traits. A component interacts with the solver via the unified `Device` trait, which acts purely as a dynamic downcaster to domain-specific traits:

### 3.1. `Device` (Downcaster)
```rust
pub trait Device: Send + Sync {
    fn device_name(&self) -> &str;
    fn as_analog(&mut self) -> Option<&mut dyn AnalogDevice> { None }
    fn as_analog_ref(&self) -> Option<&dyn AnalogDevice> { None }
    fn as_digital(&mut self) -> Option<&mut dyn DigitalDevice> { None }
    fn as_digital_ref(&self) -> Option<&dyn DigitalDevice> { None }
}
```

### 3.2. `AnalogDevice`
Implemented by devices that contribute to the MNA (Modified Nodal Analysis) matrices (e.g., Resistors, Transistors, OSDI models).
- **MNA Stamping**: `load_dc`, `load_ac`, `load_transient` return arrays of `Stamp` objects that dictate how values are added to the Jacobian matrix $G$ and right-hand side vector $I$.
- **Lifecycle & Convergence**: `update` (commits a successful non-linear iteration), `accept_timestep` (commits a successful transient step), `limiting_active` (indicates non-linear limiting, preventing convergence), and `bound_step_hint` (forces a maximum time-step size to capture fast analog dynamics).
- **Noise Analysis**: `noise_current_psd` provides current noise models evaluated at the DC operating point.

### 3.3. `DigitalDevice`
Implemented by event-driven logic (e.g., logic gates, flip-flops, comparators).
- **Topology**: `digital_input_nets` and `digital_output_nets` allow the solver to construct a dependency DAG.
- **Evaluation Phase 1 (Sequential)**: `digital_seq_phase` simulates clocked updates (e.g., flip-flop clock edges). Reads pre-edge inputs and schedules writes to outputs.
- **Evaluation Phase 2 (Combinational)**: `digital_comb_phase` simulates combinational logic (e.g., AND gates). Reads current inputs and schedules future output changes in the event queue.
- **Mixed-Signal Hooks**: `samples_analog` flags whether the digital kernel needs to read analog voltages during evaluation.

---

## 4. Algorithms: Analog Solvers

The analog portion of Piperine relies on solving the non-linear equation system $I(V, t) + \frac{d}{dt} Q(V, t) = 0$.

### 4.1. Newton-Raphson Driver (`math/newton_raphson.rs`)
Used heavily by `dc.rs` and `transient.rs`. 
1. Iterates $x_{k+1} = x_k - J^{-1} F(x_k)$.
2. Assembles the Jacobian $J$ and vector $F$ by calling `.load_dc()` or `.load_transient()` on all `AnalogDevice`s.
3. Solves the sparse linear system using `faer` LU decomposition.
4. Convergence is reached when the update vector norm $\Delta x$ falls below configurable relative and absolute tolerances (`reltol`, `abstol`). If a device reports `limiting_active`, convergence is forced to fail to allow PN junctions and MOSFETs to limit voltage swings.

### 4.2. Transient Integration (`solver/transient.rs`)
Steps through simulation time $t$:
1. Computes the DC operating point to use as $t=0$ initial conditions.
2. Predicts the next time-step $dt$ based on Local Truncation Error (LTE) of the numerical integration method (Trapezoidal or Gear).
3. Executes a Newton-Raphson solve for $t + dt$.
4. **Step Rejection**: If Newton-Raphson fails to converge, or if the LTE is too high, the step is rejected, $dt$ is shrunk, and the step is retried.

### 4.3. Noise & AC (`solver/noise.rs`, `solver/ac.rs`)
- **AC**: Linearizes the circuit around the DC operating point. Replaces non-linear devices with small-signal conductances. Solves a complex-valued sparse matrix $Y \cdot V = I$ across a frequency range.
- **Noise**: Uses the **Adjoint Matrix** method. Instead of applying individual noise sources and solving the matrix $N$ times, it transposes the Jacobian, applies a single 1A source at the output port, and solves once. The result vector is multiplied by each device's `noise_current_psd` to find the total output noise contribution.

---

## 5. Algorithms: Digital & Mixed-Signal

Piperine adopts a Verilator-style execution model for digital domains to ensure correct zero-delay propagation and eliminate simulation races.

### 5.1. Digital Scheduler (`digital/scheduler.rs`)
The digital engine is strictly event-driven. Events are encapsulated in `DigitalEvent` and sorted in a priority queue by timestamp. 
When the simulator reaches $t_{next\_event}$, it executes a **Delta Cycle**:
1. **Drain Queue**: Pops all events at $t$ and applies them to the global `nets` array, recording which nets changed.
2. **Phase 1 (Sequential)**: Iterates the topological DAG. Any device sensitive to the changed nets fires its `digital_seq_phase`. All flip-flops read the *old* (pre-settle) net state, ensuring deterministic shift-register behavior.
3. **Phase 2 (Combinational)**: Iterates the topological DAG again. Devices evaluate `digital_comb_phase` and push new events. If a device has 0-delay logic, events scheduled at $t$ are instantly applied to the netlist, and the loop repeats (Delta Cycle iteration) until stability.

### 5.2. Mixed-Signal Bridges (A2D and D2A)
The architecture treats mixed-signal boundaries symmetrically. There is no fundamental distinction between an A2D component and a D2A component: any component that bridges the domains simply implements **both** the `AnalogDevice` and `DigitalDevice` traits. They communicate across domains by maintaining internal state.

- **A2D (Analog to Digital)**: Implements `AnalogDevice` to guide the analog solver's time-stepping (e.g., providing a strict `bound_step_hint` to prevent stepping over a voltage threshold) and to model its physical input impedance (via `load_transient`). It simultaneously implements `DigitalDevice` (flagging `samples_analog = true`) to read the converged analog voltages during the `accept_and_run_digital` phase and schedule resulting digital events into the queue.
- **D2A (Digital to Analog)**: Implements `DigitalDevice` to be evaluated during the digital delta cycle, where it samples the state of its input digital nets and updates its internal representation. It simultaneously implements `AnalogDevice` so that during the subsequent analog integration step (`load_transient`), it can stamp the MNA matrix according to its new internal state (e.g., changing conductance or injecting current). If a digital event causes a discontinuous change in the analog state, the transient solver detects this and forces a step restart with a small $dt$ to accurately integrate the transient.

---

## 6. OSDI & External Models (`osdi/`)
The solver natively parses and executes OpenVAF standard `.osdi` compiled shared libraries.
- The loader reads the XML device descriptor, mapping Verilog-A nodes and parameters to Piperine's internal MNA structures.
- OSDI models execute native machine code for calculating partial derivatives, making them as fast as native Rust devices.
- They are seamlessly wrapped into `OsdiDevice : AnalogDevice`, injecting their variables into the `CircularArrayBuffer2` used by the Newton-Raphson solver.

---

## 7. Upcoming Evolutions & Known Gaps

1. **Verilator-style Digital Eliding (Planned)**: 
   Currently, the digital scheduler operates by evaluating events dynamically at runtime through trait methods. The next architectural evolution is **Digital Eliding**: `piperine-codegen` will JIT-compile entire cones of purely digital logic into single native function calls. The solver will no longer execute event loops for pure-digital blocks; instead, it will invoke the JIT-compiled C/Rust function at clock edges, and use the event scheduler *only* at the A2D/D2A boundaries.
2. **Topology Compilation**:
   The `DigitalTopology` (DAG sorting and loop detection) is currently built by the solver during initialization. This is fundamentally a compilation step and belongs in `piperine-codegen`. The solver should merely receive a pre-sorted execution plan.
3. **OSDI Relocation**:
   The `osdi/` folder lives inside the solver, but loading dynamic `.so` libraries and parsing descriptors is project-level infrastructure. Ideally, OSDI models should be loaded by `piperine-codegen` or a standalone crate, converting them into opaque `AnalogDevice`s before they ever reach the solver.

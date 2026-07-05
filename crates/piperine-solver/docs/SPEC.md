# Piperine Solver Specification

## 1. Overview
The `piperine-solver` crate is the numerical and topological simulation engine of the Piperine toolchain. It consumes a `CircuitInstance` (a linked netlist of devices, provided by `piperine-codegen`) and executes the requested analyses (DC, AC, Transient, Noise, TF). It is also responsible for driving event-driven digital logic and coordinating mixed-signal boundaries (A2D/D2A).

Unlike `piperine-lang` (which deals with syntax and elaboration) and `piperine-codegen` (which handles POM lowering and JIT compilation), the solver purely executes mathematical models and graph topology. It has **no dependency** on the AST or the codegen stages.

---

## 2. Core Concepts & Contexts

The solver houses multiple internal contexts that interact tightly. Some of these contexts overlap architecturally, leading to a slight "mixing" of concerns (documented in Section 4).

### 2.1. The Circuit Model (`circuit.rs`, `device.rs`, `analog.rs`, `port.rs`)
- **`CircuitInstance`**: The root simulation state object. It holds a list of `Device` trait objects, an analog `Netlist` (mapping abstract nodes to matrix indices), and a `DigitalState` array.
- **`Device`**: The universal trait for all simulatable components. Devices can be pure-analog (e.g., resistors, transistors), pure-digital (logic gates, flip-flops), or mixed-signal (comparators, ADCs). Devices provide:
  - DC/Transient/AC stamps (analog contributions to the Modified Nodal Analysis (MNA) matrix).
  - Digital evaluation logic.
  - Cross-domain event detection (e.g., triggering a digital transition when an analog voltage crosses a threshold).

### 2.2. Mathematical Foundation (`math/`)
- **Matrix Operations (`math::faer`)**: Safe wrappers around the `faer` crate for sparse matrix creation, symbolic factorization (LU), and numeric solving ($A \cdot x = b$).
- **Newton-Raphson (`math::newton_raphson`)**: The non-linear equation solver engine used by DC and Transient analyses. It iteratively updates device states, assembles the Jacobian matrix, applies damping, and solves until convergence criteria (tolerance limits) are met.
- **Integration (`math::iv`)**: Formulas for numerical integration (Trapezoidal, Gear) used to step capacitors and inductors through time.

### 2.3. Analyses & Solvers (`analysis/` vs `solver/`)
There is a structural split between analysis configuration and algorithmic execution:
- **`analysis/`**: Contains the option structs, configuration data, and result shapes (e.g., `DcAnalysisOptions`, `TransientAnalysisOptions`). These define *what* to run (sweep ranges, tolerances, nodes of interest, frequencies).
- **`solver/`**: Contains the actual algorithmic engines for each analysis type:
  - **DC (`dc.rs`)**: Finds the non-linear operating point using Newton-Raphson. Includes Safe Operating Area (SOA) checks.
  - **AC (`ac.rs`)**: Linearizes the circuit around the DC operating point and solves the complex sparse matrix across a frequency sweep.
  - **Transient (`transient.rs`)**: Simulates time-domain behavior. Manages dynamic time-step control based on local truncation error (LTE) and scheduled breakpoints.
  - **Noise (`noise.rs`)**: Computes noise power spectral density using the Adjoint Matrix method for efficiency (solving the transposed system once per frequency).
  - **TF (Transfer Function, `tf.rs`)**: Computes small-signal gain, input resistance, and output resistance.

### 2.4. Digital & Mixed-Signal Execution (`topology.rs`, `digital.rs`, `digital_interface.rs`)
- **`DigitalTopology`**: A DAG (Directed Acyclic Graph) of digital nets and devices. It ensures combinational logic is evaluated in correct topological order, avoiding glitches and correctly breaking/flagging zero-delay feedback loops.
- **Event-Driven Engine**: During transient simulation, the solver interleaves analog time-steps with digital events. 
- **`digital_interface.rs`**: Manages the Analog-to-Digital (A2D) and Digital-to-Analog (D2A) conversions. It schedules exact breakpoints in the transient solver when a digital transition occurs, ensuring the analog solver captures sharp edges without stepping over them.

### 2.5. OSDI & Verilog-A Loading (`osdi/`)
- The solver includes an embedded loader for OpenVAF/OSDI compiled shared libraries (`.osdi`).
- It dynamically loads compiled Verilog-A device models, parses their descriptors, and wraps them into `Device` instances fully compatible with the native Rust MNA solver.

---

## 3. Data Flow & Execution Pipeline

1. **Initialization**: A `CircuitInstance` is built by `piperine-codegen` and handed to the solver along with an analysis configuration from `analysis/`.
2. **Matrix Assembly (Analog)**: The solver requests stamps (conductance matrix $G$, current vector $I$, capacitance $C$) from each `Device` at the current voltage/time state.
3. **Iteration (DC/Tran)**: The `newton_raphson` engine loops: updates devices $\rightarrow$ builds Jacobian $\rightarrow$ solves sparse system $\rightarrow$ checks convergence (L2 norm of the update vector).
4. **Digital Evaluation**: If a digital event occurs (or is triggered by an A2D crossing), the solver walks the `DigitalTopology` and propagates logic states instantly (in simulation time).
5. **Results**: The solver extracts requested values into a `Result` object (e.g., `TransientAnalysisResult`, `DcAnalysisResult`) containing node voltages, branch currents, and digital states, which is returned to the `piperine-bench` host.

---

## 4. Architectural Boundaries & Context Mixing (Gaps)

As observed in code audits, the solver currently suffers from mildly mixed contexts that could benefit from clearer abstraction boundaries:

1. **`analysis/` vs `solver/` Overlap**: 
   - The separation between *intent* (`analysis/` configs) and *execution* (`solver/` algorithms) is sometimes blurry. Some option generation or initialization logic bleeds into the solver, and some execution context bleeds into the analysis structs. They could be strictly decoupled (e.g., `analysis` simply provides plain-data structs to an opaque `SolverRunner`).
2. **Topology Compilation in the Solver**:
   - `topology.rs` builds and sorts the digital DAG. However, topological sorting and loop detection are technically *compilation/elaboration* steps. Currently, the solver does this at runtime during circuit initialization. Ideally, `piperine-codegen` should construct the static execution plan and hand a pre-sorted execution array to the solver.
3. **OSDI Subsystem Placement**:
   - The `osdi/` module lives deep inside the solver. Loading external dynamic libraries and parsing model descriptors is an infrastructure concern. The solver should ideally only consume a generic `Device` factory. Moving OSDI up to the project boundary (or a standalone crate) and passing instantiated devices down to the solver would enforce a cleaner dependency injection pattern.
4. **Digital State Mutability**:
   - The digital interface directly mutates states during event propagation. A cleaner split would separate the "proposed next state" from the "committed state", aligning better with how the Newton-Raphson solver handles analog state updates.

# Piperine Architecture Guide

## Overview

Piperine is a circuit simulator written in Rust that separates circuit **definitions** from circuit **instances**. This separation is crucial for supporting future features like subcircuits and netlist flattening.

## Project Structure

```
packages/
├── piperine-solver/     # Main solver implementation (active development)
│   ├── src/
│   │   ├── analysis/    # Analysis trait definitions (DC, AC, Transient, Noise)
│   │   ├── circuit/     # Circuit definition and instantiation
│   │   ├── devices/     # Device components and models
│   │   ├── math/        # Mathematical utilities (Newton-Raphson, linear algebra)
│   │   ├── solver/      # Solver implementations per analysis type
│   │   ├── spice/       # SPICE parser (to be abandoned)
│   │   └── test.rs      # Scale tests (Titan benchmark)
└── piperine-api/        # REST API attempt (rejected - too complex)
```

## Core Architecture

### 1. Circuit Definition (`Circuit`)

**Location:** `packages/piperine-solver/src/circuit/mod.rs`

The `Circuit` struct represents the **static definition** of a circuit:

```rust
pub struct Circuit {
    title: String,
    models: HashMap<String, Arc<dyn AnyModel>>,
    components: HashMap<String, Box<dyn Component>>,
}
```

**Key Points:**
- Components store only **topology** (node connections) and **parameters**
- No runtime state (voltages, currents, linearization data)
- No netlist references yet
- Created using the `builder()` function

**Builder Pattern:**
```rust
let circuit = builder("My Circuit", |b| {
    b.voltage_source("V1", "in", GND, 5.0.V());
    b.resistor("R1", "in", "out", 1.0.kOhms());
});
```

### 2. Circuit Instance (`CircuitInstance`)

**Location:** `packages/piperine-solver/src/circuit/instance.rs`

The `CircuitInstance` struct represents an **instantiated** circuit ready for simulation:

```rust
pub struct CircuitInstance {
    title: String,
    runtimes: Vec<Box<dyn AnyRuntime>>,
    netlist: Netlist,
}
```

**Key Points:**
- Created by calling `CircuitInstance::instantiate(&circuit)`
- Each component creates a **Runtime** object
- Runtimes hold:
  - `CircuitReference` (resolved node/branch indices)
  - Runtime state (e.g., diode's `g_eq`, `i_eq`)
- Netlist is fully resolved (nodes have indices)
- Used directly by solvers

**Conversion:**
```rust
let circuit: Circuit = builder("Test", |b| { ... });
let mut instance: CircuitInstance = circuit.into();
```

### 3. Component vs Runtime

#### Component Trait

**Location:** `packages/piperine-solver/src/devices/mod.rs`

```rust
pub trait Component: Any + AsAny + Send + Sync {
    fn name(&self) -> String;
    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime>;
}
```

**Purpose:**
- Represents the **definition** of a device
- Stores only **parameters** (resistance, capacitance, etc.)
- Stores **node identifiers** (e.g., `NodeIdentifier` - just strings)
- No runtime state

**Examples:**
- `Resistor` - stores resistance value and node names
- `Diode` - stores model and node names
- `Capacitor` - stores capacitance and node names

#### Runtime Trait

**Location:** `packages/piperine-solver/src/devices/mod.rs`

```rust
pub trait Runtime {
    type ComponentType: Component;
    
    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self;
    fn update(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context);
    fn as_dc(&self) -> Option<&dyn DcAnalysis>;
    fn as_ac(&self) -> Option<&dyn AcAnalysis>;
    fn as_transient(&self) -> Option<&dyn TransientAnalysis>;
}
```

**Purpose:**
- Represents the **instantiated** device
- Stores `CircuitReference` (resolved netlist indices)
- Stores **runtime state** (linearization data, history)
- Updated each iteration via `update()`

**Example (DiodeRuntime):**
```rust
pub struct DiodeRuntime {
    component: Arc<Diode>,          // Reference to definition
    node_plus: CircuitReference,    // Resolved node index
    node_minus: CircuitReference,   // Resolved node index
    g_eq: Siemens,                  // Runtime: conductance
    i_eq: Ampere,                   // Runtime: current offset
}
```

### 4. Analysis Traits

**Location:** `packages/piperine-solver/src/analysis/`

Each analysis type defines a trait that Runtimes implement:

#### DcAnalysis
```rust
pub trait DcAnalysis {
    fn load_dc(&self, state: &DcAnalysisState, context: &Context) 
        -> Vec<Stamp<CircuitReference, f64>>;
}
```

#### AcAnalysis
```rust
pub trait AcAnalysis: DcAnalysis {
    fn load_ac(&self, dc_result: &DcAnalysisResult, ac_ctx: &AcAnalysisContext, context: &Context)
        -> Vec<Stamp<CircuitReference, Complex<f64>>>;
}
```

#### TransientAnalysis
```rust
pub trait TransientAnalysis {
    fn load_transient(&self, states: &TransientAnalysisState, ctx: &TransientAnalysisContext, context: &Context)
        -> Vec<Stamp<CircuitReference, f64>>;
}
```

**Key Insight:**
- Runtimes implement these traits
- Runtimes are **updated** before loading (via `update()`)
- Loading returns MNA stamps for the system matrix

### 5. Solvers

**Location:** `packages/piperine-solver/src/solver/`

Each analysis has a dedicated solver:

- **DcSolver** - Newton-Raphson DC operating point
- **AcSolver** - Small-signal AC frequency sweep (requires DC bias)
- **TransientSolver** - Time-domain with implicit integration
- **NoiseSolver** - Noise analysis (under development)

**Workflow:**
```rust
let mut instance: CircuitInstance = circuit.into();
let result = instance
    .dc(Context::default())?
    .solve()?;
```

## Current Refactor Status

### What Changed

The recent refactor separated `Circuit` from `CircuitInstance`:

1. **Before:**
   - Components stored `CircuitReference` directly
   - Components had runtime state (`g_eq`, `i_eq`)
   - Circuit was directly passed to solvers

2. **After:**
   - Components store `NodeIdentifier` (strings)
   - Components have **no** runtime state
   - Runtimes created during instantiation
   - Runtimes store `CircuitReference` and state
   - CircuitInstance passed to solvers

### Fixed Issues ✅

1. **DiodeRuntime.update() saves results correctly**
   - Fixed: Stores g_eq, i_eq, and v_d_prev for damping
   - Uses internal state for damping instead of buffer history

2. **CircularArrayBuffer bounds checking**
   - Fixed: Proper checks for lookback >= count
   - AC analysis no longer calls update_all with invalid buffer

3. **GND node handling**
   - Fixed: Runtime correctly handles nodes with idx() == None
   - Returns 0.0 for ground voltage

### Current Status (2025-03-02)

**✅ ALL TESTS PASSING: 11/11 (100%)**

- ✅ DC Analysis (resistor divider, diode bias, floating node handling)
- ✅ AC Analysis (RC filter, LC resonance)  
- ✅ Transient Analysis (RC charging, RC step, RL current rise)
- ✅ Noise Analysis

**Key Achievements:**
- Diode damping working correctly with internal v_d_prev tracking
- Capacitor and inductor transient analysis using BDF integration
- Voltage sources correctly evaluate waveforms with context.time
- Floating nodes stabilized with gmin in DC analysis

## How to Add a New Device

### Step 1: Define the Component

```rust
// In devices/mydevice/mod.rs
pub struct MyDevice {
    name: String,
    node_a: NodeIdentifier,
    node_b: NodeIdentifier,
    parameter: f64,
}

impl MyDevice {
    pub fn new(name: String, node_a: impl IntoNodeIdentifier, 
               node_b: impl IntoNodeIdentifier, parameter: f64) -> Self {
        Self {
            name,
            node_a: node_a.into(),
            node_b: node_b.into(),
            parameter,
        }
    }
}
```

### Step 2: Implement Component Trait

```rust
impl Component for MyDevice {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime> {
        Box::new(MyDeviceRuntime::allocate(Arc::new(self.clone()), netlist))
    }
}
```

### Step 3: Define the Runtime

```rust
// In devices/mydevice/runtime.rs
pub struct MyDeviceRuntime {
    component: Arc<MyDevice>,
    node_a: CircuitReference,
    node_b: CircuitReference,
    // Runtime state here
    conductance: f64,
}

impl Runtime for MyDeviceRuntime {
    type ComponentType = MyDevice;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self {
        let node_a = netlist.connect_node(component.node_a.clone());
        let node_b = netlist.connect_node(component.node_b.clone());

        Self {
            component,
            node_a,
            node_b,
            conductance: 0.0,
        }
    }

    fn update(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) {
        // Update runtime state based on current voltages/currents
        let v_a = state.latest().and_then(|s| s.get(self.node_a.idx().unwrap())).unwrap_or(0.0);
        let v_b = state.latest().and_then(|s| s.get(self.node_b.idx().unwrap())).unwrap_or(0.0);
        
        self.conductance = compute_conductance(v_a - v_b, &self.component);
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }
}
```

### Step 4: Implement Analysis Traits

```rust
impl DcAnalysis for MyDeviceRuntime {
    fn load_dc(&self, _state: &DcAnalysisState, _ctx: &Context) 
        -> Vec<Stamp<CircuitReference, f64>> {
        
        let g = self.conductance;
        
        vec![
            Stamp::Matrix(self.node_a.clone(), self.node_a.clone(), g),
            Stamp::Matrix(self.node_b.clone(), self.node_b.clone(), g),
            Stamp::Matrix(self.node_a.clone(), self.node_b.clone(), -g),
            Stamp::Matrix(self.node_b.clone(), self.node_a.clone(), -g),
        ]
    }
}
```

## Math and Numerical Methods

### Newton-Raphson

**Location:** `packages/piperine-solver/src/math/newton_raphson.rs`

The solver uses Newton-Raphson iteration:

```
J(x) * Δx = -F(x)
x_{n+1} = x_n + Δx
```

**Implementation:**
1. Each runtime provides stamps (via `load_dc()`, etc.)
2. Stamps are assembled into system matrix J and RHS F
3. Faer library solves the linear system
4. Solution updates the state
5. Repeat until convergence

### Linear Algebra

**Location:** `packages/piperine-solver/src/math/faer.rs`

Uses the [Faer](https://github.com/sarah-ek/faer-rs) library for:
- Sparse matrix factorization
- Linear system solving
- High performance for medium-scale problems (~40k nodes)

### MNA Stamping

**Location:** `packages/piperine-solver/src/math/linear.rs`

Modified Nodal Analysis (MNA) stamps:

```rust
pub enum Stamp<R, T> {
    Matrix(R, R, T),     // J[row][col] += value
    Rhs(R, T),           // F[row] += value
}
```

## Future: Subcircuits and Flattening

The current refactor prepares for:

1. **Hierarchical Circuits:**
   - Circuits can contain other circuits as components
   - Each subcircuit is a `Component` with internal topology

2. **Netlist Flattening:**
   - During instantiation, subcircuits are "flattened"
   - All internal nodes get unique global names
   - Hierarchy is preserved for probing/debugging

3. **Reusable Definitions:**
   - Same `Circuit` can be instantiated multiple times
   - Different instances have different runtime state
   - Enables Monte Carlo / parameter sweeps

## Key Files to Understand

1. **`src/circuit/mod.rs`** - Circuit builder and definition
2. **`src/circuit/instance.rs`** - Instantiation and solver interface
3. **`src/devices/mod.rs`** - Component and Runtime traits
4. **`src/devices/diode/runtime.rs`** - Example runtime implementation
5. **`src/solver/dc.rs`** - DC solver showing update/load cycle
6. **`src/math/newton_raphson.rs`** - Convergence loop

## Testing

**Location:** `packages/piperine-solver/src/test.rs`

- **`test()`** - Simple diode bias test
- **`titan_test(n)`** - RC grid benchmark (n×n grid)
- **Device-specific tests** - In each device's `test.rs`

**Run tests:**
```bash
cargo test --package piperine-solver
```

**Run benchmark:**
```bash
cargo test --package piperine-solver full_titan_test --release -- --ignored
```

## Common Patterns

### Creating a circuit
```rust
use piperine::prelude::*;

let circuit = builder("My Circuit", |b| {
    b.voltage_source("V1", "in", GND, 5.0.V());
    b.resistor("R1", "in", "out", 10.0.kOhms());
    b.capacitor("C1", "out", GND, 100.0.nF());
});
```

### Running a simulation
```rust
let mut instance: CircuitInstance = circuit.into();

let result = instance
    .dc(Context::default())?
    .solve()?;

let v_out = result.get_node("out")?;
println!("Output: {:.3} V", v_out);
```

### Accessing device state during simulation
Use the `Ask` trait (under development) to query device internals like power dissipation, junction temperature, etc.

## Debugging Tips

1. **Check netlist indices:** Ensure `CircuitReference` has valid `idx()`
2. **Verify update() is called:** Runtime state must be refreshed before load
3. **Watch for uninitialized state:** Runtimes should initialize sensible defaults
4. **Use tracing:** Enable `RUST_LOG=debug` for detailed solver output
5. **Test standalone devices:** Write unit tests for each device's runtime

## Notes

- **SPICE compatibility:** Limited - we're building a modern API, not parsing SPICE syntax
- **Performance target:** 40k node grids in reasonable time (<1s per step)
- **Stability:** Pre-alpha - API will change frequently
- **License:** MIT - permissive for research and commercial use

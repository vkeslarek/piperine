# Solver Strategy Composition — Design

**Spec:** `.specs/features/solver-strategy-composition/spec.md`
**Decisions:** MD-03, MD-04, MD-05, MD-13

## Architecture

```
ConvergencePlan
├── newton: Box<dyn NewtonStrategy>       ← damping + convergence policy
├── homotopies: Vec<Box<dyn HomotopyStrategy>>  ← (already done)
├── stepper: Option<Box<dyn StepperStrategy>>   ← transient only
└── limits: PlanLimits                          ← (already done)
```

The plan owns every strategy. DC uses `newton` + `homotopies`. Transient uses
`newton` + `stepper`. AC/Noise/TF use `newton` only (linear, one iteration).

## Components

### 1. NewtonStrategy

Replaces the free functions `check_convergence`, `residual_converged`,
`apply_damping` in `solver/mod.rs`. Today these are `pub(crate) fn` — they
violate MD-13 rule 1 (every method has an owner).

```rust
/// Newton iteration policy: damping, convergence test, iteration cap.
/// The `ConvergencePlan` owns one; `NewtonRaphsonSolver` consults it
/// instead of calling `NonLinearSystem::apply_limit`/`converged`/
/// `residual_converged` directly.
pub trait NewtonStrategy: Send + Sync {
    /// Damp the Newton update in-place before the convergence test.
    /// Default: the midpoint damping ngspice uses (halve if norm > threshold).
    /// `policy.dc_damp_tolerance` controls the threshold.
    fn damp_update(
        &self,
        prev: ArrayView1<f64>,
        update: ArrayViewMut1<f64>,
        policy: &Policy,
    );

    /// Whether the Newton loop has converged: update test AND residual test
    /// AND no device reports active limiting. Reads tolerances for reltol/
    /// abstol/vntol, reads netlist for per-row classification.
    fn is_converged(
        &self,
        devices: &[Box<dyn Element>],
        state: &CircularArrayBuffer2<f64>,
        guess: &ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &Netlist,
        tolerances: &Tolerances,
    ) -> bool;

    /// Maximum Newton iterations. Reads `policy.max_iter`.
    fn max_iter(&self, policy: &Policy) -> usize;
}
```

**Default impl:** `DampedNewton` — wraps today's `apply_damping` +
`check_convergence` + `residual_converged` logic, moved from free fns into
the trait impl. This is the zero-behavior-change path.

**`NewtonRaphsonSolver::solve` change:** instead of calling
`system.apply_limit()` / `system.converged()` / `system.residual_converged()`,
it calls `strategy.damp_update(prev, update, policy)` /
`strategy.is_converged(devices, state, guess, residual, scale, netlist, tolerances)`.
The `NonLinearSystem` trait loses `apply_limit`, `converged`,
`residual_converged`, and `alpha` — it keeps only `assemble` and the
lifecycle callbacks. The solver receives both `&Tolerances` (immutable,
from `Context`) and `&Policy` (mutable, from the driver/plan) so strategies
can read damping thresholds and iteration caps without reaching into
`Context`.

### 2. StepperStrategy

Replaces the inline timestep logic in `TransientSolver::solve()`. Today
the stepper grows 2× on success, halves on failure, clamps to
`[dt_min, dt_max]`, and consults LTE via `suggest_transient_step`.

```rust
/// Transient timestep policy: propose, accept, reject.
/// The transient driver owns one; it calls `propose_dt` after each
/// accepted step and `reject_dt` after a failed step.
pub trait StepperStrategy: Send + Sync {
    /// Propose the next timestep after an accepted step.
    /// Consults LTE suggestions, breakpoints, digital events, growth limits.
    fn propose_dt(
        &self,
        current_time: f64,
        last_dt: f64,
        dt_prev: f64,
        circuit: &CircuitInstance,
        solver_state: &CircularArrayBuffer2<f64>,
        tolerances: &Tolerances,
        tran_ctx: &TransientContext,
    ) -> f64;

    /// React to a rejected step: return the reduced dt to retry with.
    fn reject_dt(&self, failed_dt: f64, tran_ctx: &TransientContext) -> f64;
}
```

**Default impl:** `LteStepper` — wraps today's inline logic (LTE min,
2× growth fallback, halve on reject, clamp to `[dt_min, dt_max]`).

### 3. Tolerances / Policy split

Today `Context` has 13 flat fields. Split:

```rust
/// Immutable per-run tolerances. `Copy`. Shared across all analyses.
#[derive(Debug, Clone, Copy)]
pub struct Tolerances {
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub gmin: Siemens,
    pub min_res: Ohm,
    pub trtol: f64,
    pub chgtol: f64,
    pub temperature: f64,
    pub tnom: f64,
    pub integration: IntegrationMethod,
}

/// Mutable state owned by the active ConvergencePlan / strategies.
/// Never on the shared Context.
#[derive(Debug, Clone)]
pub struct Policy {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
    pub time: f64,
}

/// The shared, immutable context every analysis sees.
#[derive(Debug, Clone)]
pub struct Context {
    pub tolerances: Tolerances,
}
```

`Context::default()` constructs default `Tolerances`. `has_converged` moves
to `Tolerances` (it only reads tolerance fields). `init_global` stays on
`Context` (MD-06).

### 4. AnalysisContext enum

```rust
pub enum AnalysisContext {
    Dc(DcContext),
    Ac(AcContext),
    Transient(TransientContext),
    Noise(NoiseContext),
    Tf(TfContext),
}

pub struct DcContext {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
}

pub struct TransientContext {
    pub dt: f64,
    pub dt_min: f64,
    pub dt_max: f64,
    pub adaptive: bool,
    pub record_from: f64,
    pub stop_time: f64,
}
```

Each solver driver receives its specific context. `TransientAnalysisOptions`
becomes a thin constructor for `TransientContext`; the driver reads
`tran_ctx.dt_min` instead of `self.options.dt_min`.

### 5. SignalBridge

Extract from `CircuitInstance::accept_and_run_digital`:

```rust
/// Internal component owned by `CircuitInstance`. Handles the
/// analog→digital bridge: builds the solution buffer, seeds the digital
/// event queue from analog accept hooks, and runs the digital scheduler.
pub struct SignalBridge {
    // stateless today; future home for bridge-specific config
}

impl SignalBridge {
    pub fn accept_and_settle(
        &mut self,
        circuit: &mut CircuitInstance,
        solution: &[f64],
        ctx: &Context,
        t: f64,
    ) -> Result<bool>;  // true if digital changed
}
```

`CircuitInstance` holds `bridge: SignalBridge` and delegates.

## Migration path

1. **Tolerances/Policy split** — pure refactor, no behavior change. `Context`
   fields move into `Tolerances`; callers read `ctx.tolerances.reltol` instead
   of `ctx.reltol`. `max_iter`/`dc_damp_tolerance`/`time` move to `Policy`,
   which the drivers own locally (not on `Context`).

2. **NewtonStrategy** — move free fns into trait impl. `NewtonRaphsonSolver`
   takes `&dyn NewtonStrategy` instead of calling `system.apply_limit` etc.
   `NonLinearSystem` loses `apply_limit`/`converged`/`residual_converged`/
   `alpha`.

3. **StepperStrategy** — move inline transient logic into trait impl.
   `TransientSolver` takes `&dyn StepperStrategy`.

4. **AnalysisContext** — `TransientAnalysisOptions` → `TransientContext`;
   `Context::max_iter` → `DcContext::max_iter`. Drivers receive their context.

5. **SignalBridge** — extract method from `CircuitInstance`.

Each step is independently testable: `cargo test --workspace` green after each.

## File map — where every new type lives

Every crate path is relative to `crates/piperine-solver/src/`. An implementing
agent should not need to guess where a type goes.

### `solver/mod.rs` — `Context`, `Tolerances`, `Policy`

This file today holds `Context` (13 flat fields), `init_global`, three free
fns (`check_convergence`, `residual_converged`, `apply_damping`), and the
`pub mod` declarations.

**After:**

```rust
// solver/mod.rs

pub mod ac;
pub mod convergence;
pub mod dc;
pub mod noise;
pub mod tf;
pub mod transient;

static INIT: Once = Once::new();

// ── Tolerances (immutable, Copy) ───────────────────────────────────────
// NEW: extracted from Context's flat fields. Lives here because every
// analysis reads it through Context.
#[derive(Debug, Clone, Copy)]
pub struct Tolerances {
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub gmin: Siemens,
    pub min_res: Ohm,
    pub trtol: f64,
    pub chgtol: f64,
    pub temperature: f64,
    pub tnom: f64,
    pub integration: crate::math::integration::IntegrationMethod,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            gmin: 1e-12,
            min_res: 1e-12,
            trtol: 7.0,
            chgtol: 1e-14,
            temperature: 300.15,
            tnom: 300.15,
            integration: crate::math::integration::IntegrationMethod::Gear { order: 2 },
        }
    }
}

impl Tolerances {
    /// Today's `Context::has_converged` — moved here because it only reads
    /// tolerance fields. Same logic, same signature except `&self` is
    /// `&Tolerances` instead of `&Context`.
    pub fn has_converged(
        &self,
        old_values_opt: Option<ArrayView1<f64>>,
        new_values: &ArrayView1<f64>,
        netlist: &Netlist,
    ) -> bool {
        // ... identical body to today's Context::has_converged ...
    }
}

// ── Policy (mutable, owned by drivers/plan) ────────────────────────────
// NEW: the mutable fields that used to be on Context. NOT on Context.
// Each driver constructs a local Policy and passes it around.
#[derive(Debug, Clone)]
pub struct Policy {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
    pub time: f64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_iter: 500,
            dc_damp_tolerance: 0.5,
            time: 0.0,
        }
    }
}

// ── Context (shared, immutable) ────────────────────────────────────────
// SIMPLIFIED: just Tolerances + init_global. No flat fields.
#[derive(Debug, Clone)]
pub struct Context {
    pub tolerances: Tolerances,
}

impl Default for Context {
    fn default() -> Self {
        Self { tolerances: Tolerances::default() }
    }
}

impl Context {
    pub fn init_global() {
        INIT.call_once(|| { /* same as today */ });
    }
}

// FREE FNS REMOVED: check_convergence, residual_converged, apply_damping
// move into DampedNewton impl in solver/convergence.rs (see below).
```

### `solver/convergence.rs` — strategies (existing + new)

This file today holds `PlanLimits`, `HomotopyDriver`, `HomotopyStrategy`,
`ConvergencePlan`, `GminStepping`, `SourceStepping`.

**Add to this file (do not create a new file):**

```rust
// solver/convergence.rs — appended after existing types

use crate::core::element::Element;
use crate::analog::Netlist;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::{Context, Tolerances, Policy};
use ndarray::{ArrayView1, ArrayViewMut1};

// ── NewtonStrategy ─────────────────────────────────────────────────────
// NEW trait. Replaces the free fns check_convergence/residual_converged/
// apply_damping that used to live in solver/mod.rs.

/// Newton iteration policy: damping, convergence test, iteration cap.
pub trait NewtonStrategy: Send + Sync {
    /// Damp the Newton update in-place before the convergence test.
    fn damp_update(
        &self,
        prev: ArrayView1<f64>,
        update: ArrayViewMut1<f64>,
        tolerances: &Tolerances,
    );

    /// Converged if: update test passes AND residual test passes AND
    /// no element reports limiting_active.
    fn is_converged(
        &self,
        devices: &[Box<dyn Element>],
        state: &CircularArrayBuffer2<f64>,
        guess: &ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &Netlist,
        tolerances: &Tolerances,
    ) -> bool;

    /// Max Newton iterations.
    fn max_iter(&self, policy: &Policy) -> usize;
}

/// Default: midpoint damping + voltage-step + residual convergence.
/// Body is the exact logic from today's free fns, just moved into a trait impl.
pub struct DampedNewton;

impl NewtonStrategy for DampedNewton {
    fn damp_update(
        &self,
        prev: ArrayView1<f64>,
        mut update: ArrayViewMut1<f64>,
        policy: &Policy,
    ) {
        // Body: today's apply_damping, using policy.dc_damp_tolerance
        // as the threshold. Same logic, just moved from free fn to trait impl.
        // ... today's apply_damping body, reading policy.dc_damp_tolerance ...
    }

    fn is_converged(
        &self,
        devices: &[Box<dyn Element>],
        state: &CircularArrayBuffer2<f64>,
        guess: &ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &Netlist,
        tolerances: &Tolerances,
    ) -> bool {
        // Body: today's check_convergence + residual_converged, combined.
        // 1. for device in devices: if device.limiting_active() { return false }
        // 2. tolerances.has_converged(state.view(0), guess, netlist)
        // 3. residual check: for each netlist reference, |residual[i]| <= tol
        // ... today's bodies, reading tolerances.* instead of ctx.* ...
    }

    fn max_iter(&self, policy: &Policy) -> usize {
        policy.max_iter
    }
}

// ── StepperStrategy ────────────────────────────────────────────────────
// NEW trait. Replaces the inline dt logic in TransientSolver::solve.

/// Transient timestep policy.
pub trait StepperStrategy: Send + Sync {
    /// Propose the next dt after an accepted step.
    /// Consults LTE, breakpoints, digital events, growth limits.
    fn propose_dt(
        &self,
        current_time: f64,
        last_dt: f64,
        dt_prev: f64,
        circuit: &crate::core::circuit::CircuitInstance,
        solver_state: &CircularArrayBuffer2<f64>,
        tolerances: &Tolerances,
        tran_ctx: &crate::analysis::transient::TransientContext,
    ) -> f64;

    /// Reduced dt after a rejected step.
    fn reject_dt(
        &self,
        failed_dt: f64,
        tran_ctx: &crate::analysis::transient::TransientContext,
    ) -> f64;
}

/// Default: LTE-driven stepper. Body is today's inline logic from
/// TransientSolver::solve — the LTE loop, 2× fallback, halve on reject,
/// clamp to [dt_min, dt_max].
pub struct LteStepper;

impl StepperStrategy for LteStepper {
    fn propose_dt(...) -> f64 {
        // Body: today's LTE loop from TransientSolver::solve lines:
        //   let method = context.integration;
        //   let time_history = [dt_actual, dt_prev];
        //   let tran_state = TransientAnalysisState::new(solver_state, &[]);
        //   let mut lte_dt = dt_max;
        //   ... iterate devices, suggest_transient_step, take min ...
        //   if any_lte { lte_dt.clamp(dt_min, dt_max) }
        //   else { (last_dt * 2.0).clamp(dt_min, dt_max) }
    }

    fn reject_dt(...) -> f64 {
        // Body: today's (failed_dt * 0.5).max(dt_min)
    }
}
```

**Update `ConvergencePlan` to own `NewtonStrategy`:**

```rust
// solver/convergence.rs — ConvergencePlan gains a newton field

pub struct ConvergencePlan {
    newton: Box<dyn NewtonStrategy>,           // NEW
    strategies: Vec<Box<dyn HomotopyStrategy>>, // existing
    stepper: Option<Box<dyn StepperStrategy>>,  // NEW (None for DC, Some for transient)
    limits: PlanLimits,                         // existing
}

impl Default for ConvergencePlan {
    fn default() -> Self {
        Self {
            newton: Box::new(DampedNewton),
            strategies: vec![Box::new(GminStepping), Box::new(SourceStepping)],
            stepper: None,
            limits: PlanLimits::default(),
        }
    }
}

impl ConvergencePlan {
    pub fn newton(&self) -> &dyn NewtonStrategy { self.newton.as_ref() }
    pub fn stepper(&self) -> Option<&dyn StepperStrategy> {
        self.stepper.as_deref()
    }
    // with_newton, with_stepper builders...
}
```

### `math/newton_raphson.rs` — `NonLinearSystem` simplified

This file today holds `NonLinearSystem` (with `assemble`, `converged`,
`residual_converged`, `apply_limit`, `alpha`, lifecycle callbacks) and
`NewtonRaphsonSolver`.

**`NonLinearSystem` trait — remove these methods:**
- `converged` — moves to `NewtonStrategy::is_converged`
- `residual_converged` — moves to `NewtonStrategy::is_converged`
- `apply_limit` — moves to `NewtonStrategy::damp_update`
- `alpha` parameter on `assemble` — removed

**`NonLinearSystem` trait — keep only:**
```rust
pub trait NonLinearSystem<A: AsIndex, E: Scalar> {
    fn assemble(&mut self, state: &CircularArrayBuffer2<E>)
        -> crate::result::Result<Vec<Stamp<A, E>>>;

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<E>) {}
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<E>, _iter: usize) {}
    fn convergence_failed_callback(&mut self, ...) {}
    fn convergence_success_callback(&mut self, ...) {}
}
```

**`NewtonRaphsonSolver::solve` signature change:**
```rust
// BEFORE:
pub fn solve(
    &mut self,
    system: &mut dyn NonLinearSystem<A, E>,
    alpha: E,
    max_iter: usize,
) -> Result<Array1<E>>

// AFTER:
pub fn solve(
    &mut self,
    system: &mut dyn NonLinearSystem<A, E>,
    strategy: &dyn NewtonStrategy,   // for f64 only; Complex (AC) uses a trivial pass-through
    tolerances: &Tolerances,         // immutable, from Context
    policy: &Policy,                 // mutable, from driver/plan
) -> Result<Array1<E>>
```

Note: `NewtonStrategy` is f64-only (damping makes no sense for Complex AC).
AC solver passes a `LinearNewton` strategy that does no damping and always
returns `is_converged = true` after one iteration.

### `analysis/transient.rs` — `TransientContext` (new) + options stays as constructor

This file today holds `TransientAnalysisState`, `TransientAnalysisOptions`,
`TransientAnalysisContext`, `TransientStep`.

**Add `TransientContext` (new struct, same file):**
```rust
// analysis/transient.rs

/// Per-analysis config for transient. Lives here because it's the
/// transient-specific sibling of DcContext/AcContext.
/// TransientAnalysisOptions stays as a pub constructor that builds this.
#[derive(Debug, Clone)]
pub struct TransientContext {
    pub dt: f64,
    pub dt_min: f64,
    pub dt_max: f64,
    pub adaptive: bool,
    pub record_from: f64,
    pub stop_time: f64,
}

impl From<TransientAnalysisOptions> for TransientContext {
    fn from(opts: TransientAnalysisOptions) -> Self {
        Self {
            dt: opts.dt,
            dt_min: opts.dt_min,
            dt_max: opts.dt_max,
            adaptive: opts.adaptive,
            record_from: opts.record_from,
            stop_time: opts.stop_time,
        }
    }
}
```

### `analysis/dc.rs` — `DcContext` (new)

This file today holds `DcAnalysisState`, `DcAnalysis` trait, `DcAnalysisResult`.

**Add `DcContext` (new struct, same file):**
```rust
// analysis/dc.rs

/// Per-analysis config for DC. Carries what used to be on Context:
/// max_iter and dc_damp_tolerance.
#[derive(Debug, Clone)]
pub struct DcContext {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
}

impl Default for DcContext {
    fn default() -> Self {
        Self { max_iter: 500, dc_damp_tolerance: 0.5 }
    }
}
```

### `analysis/ac.rs`, `analysis/noise.rs`, `analysis/tf.rs` — per-analysis contexts

Each file gets a minimal context struct:
- `analysis/ac.rs`: `AcContext { sweep: AcSweepAnalysisOptions }`
- `analysis/noise.rs`: `NoiseContext { sweep: AcSweepAnalysisOptions, output_node, reference_node, input_source_name }`
- `analysis/tf.rs`: `TfContext { options: TransferFunctionAnalysisOptions }`

These are thin wrappers — they exist for enum completeness, not because they
add logic today.

### `core/circuit.rs` — `SignalBridge` (new struct, same file)

This file today holds `CircuitInstance` with `accept_and_run_digital` doing
three jobs inline.

**Add `SignalBridge` (new struct, same file):**
```rust
// core/circuit.rs

/// Internal: the analog↔digital bridge. Extracted from
/// CircuitInstance::accept_and_run_digital so CircuitInstance is storage
/// and delegation, not logic.
pub struct SignalBridge {
    // stateless today; future home for bridge config and cached state
}

impl SignalBridge {
    pub fn new() -> Self { Self {} }

    /// Build the solution buffer, seed the digital event queue from
    /// analog accept hooks, run the digital scheduler.
    /// Returns true if any digital net changed.
    /// Body: today's accept_and_run_digital logic, moved verbatim.
    pub fn accept_and_settle(
        &mut self,
        circuit: &mut CircuitInstance,
        solution: &[f64],
        ctx: &Context,
        t: f64,
    ) -> crate::result::Result<bool> {
        // ... today's accept_and_run_digital body ...
    }
}
```

**`CircuitInstance` updated:**
```rust
pub struct CircuitInstance {
    pub title: String,
    pub devices: Vec<Box<dyn Element>>,
    pub digital_topology: Option<DigitalTopology>,
    pub digital_state: DigitalState,
    pub netlist: Netlist,
    bridge: SignalBridge,  // NEW field
}

// accept_and_run_digital becomes a one-liner delegation:
impl CircuitInstance {
    pub fn accept_and_run_digital(
        &mut self, solution: &[f64], ctx: &Context, t: f64,
    ) -> crate::result::Result<bool> {
        self.bridge.accept_and_settle(self, solution, ctx, t)
    }
}
```

Note: `accept_and_settle` takes `&mut CircuitInstance` which creates a borrow
issue (self-referential). To avoid this, either:
- (a) Make `accept_and_settle` a free function that takes `&mut CircuitInstance`
  directly (but then it's not a method on SignalBridge — violates MD-13).
- (b) Make `SignalBridge` hold only the logic, not a reference to circuit, and
  pass circuit as a parameter. The borrow checker allows `&mut self.bridge`
  and `&mut self` in the same call if we split the borrow — extract the fields
  the bridge needs (devices, digital_state, netlist) and pass them.
- (c) Simplest: `accept_and_run_digital` stays on `CircuitInstance` but
  delegates the buffer-building and queue-seeding to `SignalBridge` methods
  that take borrowed slices. The bridge owns the *method*, the circuit owns
  the *data*.

**Decision: (c).** `SignalBridge` has two methods:
```rust
impl SignalBridge {
    /// Build a 1-row CircularArrayBuffer2 from the solution slice.
    pub fn build_accept_state(&self, solution: &[f64]) -> CircularArrayBuffer2<f64> { ... }

    /// Run analog accept hooks + digital scheduler. Takes borrowed slices.
    pub fn settle(
        &mut self,
        devices: &mut [Box<dyn Element>],
        digital_state: &mut DigitalState,
        state: &CircularArrayBuffer2<f64>,
        ctx: &Context,
        t: f64,
    ) -> Result<bool> { ... }
}
```

`CircuitInstance::accept_and_run_digital` splits the borrow:
```rust
pub fn accept_and_run_digital(&mut self, solution: &[f64], ctx: &Context, t: f64) -> Result<bool> {
    let state = self.bridge.build_accept_state(solution);
    let CircuitInstance { devices, digital_state, bridge, .. } = self;
    bridge.settle(devices, digital_state, &state, ctx, t)
}
```

### `prelude.rs` — new exports

Add to the existing re-exports:
```rust
pub use crate::solver::{Tolerances, Policy};
pub use crate::solver::convergence::{NewtonStrategy, StepperStrategy, DampedNewton, LteStepper};
pub use crate::analysis::transient::TransientContext;
pub use crate::analysis::dc::DcContext;
```

### Summary table — every new type and its home

| Type | Kind | File (relative to `src/`) | Module path |
|------|------|---------------------------|-------------|
| `Tolerances` | struct | `solver/mod.rs` | `piperine_solver::solver::Tolerances` |
| `Policy` | struct | `solver/mod.rs` | `piperine_solver::solver::Policy` |
| `NewtonStrategy` | trait | `solver/convergence.rs` | `piperine_solver::solver::convergence::NewtonStrategy` |
| `DampedNewton` | struct (impl) | `solver/convergence.rs` | `piperine_solver::solver::convergence::DampedNewton` |
| `StepperStrategy` | trait | `solver/convergence.rs` | `piperine_solver::solver::convergence::StepperStrategy` |
| `LteStepper` | struct (impl) | `solver/convergence.rs` | `piperine_solver::solver::convergence::LteStepper` |
| `DcContext` | struct | `analysis/dc.rs` | `piperine_solver::analysis::dc::DcContext` |
| `AcContext` | struct | `analysis/ac.rs` | `piperine_solver::analysis::ac::AcContext` |
| `TransientContext` | struct | `analysis/transient.rs` | `piperine_solver::analysis::transient::TransientContext` |
| `NoiseContext` | struct | `analysis/noise.rs` | `piperine_solver::analysis::noise::NoiseContext` |
| `TfContext` | struct | `analysis/tf.rs` | `piperine_solver::analysis::tf::TfContext` |
| `SignalBridge` | struct | `core/circuit.rs` | `piperine_solver::core::circuit::SignalBridge` |

### What gets removed

| Removed | From file | Replaced by |
|---------|-----------|-------------|
| `Context::reltol` (and 9 other flat fields) | `solver/mod.rs` | `Context::tolerances.reltol` |
| `Context::max_iter` | `solver/mod.rs` | `Policy::max_iter` |
| `Context::dc_damp_tolerance` | `solver/mod.rs` | `Policy::dc_damp_tolerance` |
| `Context::time` | `solver/mod.rs` | `Policy::time` |
| `Context::has_converged` | `solver/mod.rs` | `Tolerances::has_converged` |
| `check_convergence()` free fn | `solver/mod.rs` | `DampedNewton::is_converged` |
| `residual_converged()` free fn | `solver/mod.rs` | `DampedNewton::is_converged` |
| `apply_damping()` free fn | `solver/mod.rs` | `DampedNewton::damp_update` |
| `NonLinearSystem::converged` | `math/newton_raphson.rs` | `NewtonStrategy::is_converged` |
| `NonLinearSystem::residual_converged` | `math/newton_raphson.rs` | `NewtonStrategy::is_converged` |
| `NonLinearSystem::apply_limit` | `math/newton_raphson.rs` | `NewtonStrategy::damp_update` |
| `alpha` param on `NonLinearSystem::assemble` | `math/newton_raphson.rs` | (removed) |
| Inline dt logic in `TransientSolver::solve` | `solver/transient.rs` | `LteStepper::propose_dt` / `reject_dt` |

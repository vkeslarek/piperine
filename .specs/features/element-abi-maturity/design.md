# Element ABI Maturity Design

**Spec**: `.specs/features/element-abi-maturity/spec.md`
**Status**: Draft

---

## Architecture Overview

Ten stories, one central contract: the `Element` trait
(`AnalogDevice + DigitalDevice + Introspect`). Every story either adds hooks to
the trait, changes what the solver drivers call, or bridges codegen kernel data
to the ABI surface. No story introduces a new crate — all work is inside
`piperine-solver` and `piperine-codegen`.

```
                     Element trait (solver/abi)
                    ┌──────────┴──────────┐
            AnalogDevice            DigitalDevice         Introspect
           ┌────┴────┐              ┌──┴──┐              ┌──┴──┐
      checkpoint*   limiting*   checkpoint*         terminals   opvars
      restore*     _report*     restore*            (kernel    (kernel
                   (replaces      (extends            bridge)    bridge)
                   limiting_active  digital_hidden)
                   + convergence_hint)
                        │
              ┌─────────┼──────────────────┐
              │         │                  │
     TransientSolver  DcSolver      ConvergencePlan
     (wires ckpt/     (wires ckpt/  (folds StepperStrategy;
      restore into     restore into   currently owns
      reject path;     homotopy       NewtonStrategy +
      unified EventQ   retry; wires   HomotopyStrategy)
      in predict_step) set_temperature)
```

`*` = new hook on the trait.

### Story → component mapping

| Story | Solver changes | Codegen changes | Doc/test |
|-------|---------------|-----------------|----------|
| P1 Rollback | `Element::checkpoint_state`/`restore_state`; transient + DC reject wiring | `PiperineDevice` checkpoints limiter + digital registers | — |
| P2 Limiting | `LimitingReport` replaces bool + hint; Newton gate updated | `Limiter` produces `LimitingReport` | — |
| P3 Lifecycle | Contract test instrumenting hooks | — | Part VII § per-analysis chart + algorithm flow |
| P4 Temperature | `set_temperature` called in `setup`; sweep invalidation | `PiperineDevice` overrides `set_temperature` | — |
| P5 Jacobian | New capability bits; `.disto` fail-loud | Kernel declares analytic disto2/3 | — |
| P6 Terminals/opvars | — | `PiperineDevice` bridges kernel → `list_terminals`/`read_opvars` | — |
| P7 Save/probe | `ProbeSelection` in options; `collect_device_banks` filters | `PiperineDevice` declares `ObservableDescriptor` | — |
| P8 Events | `EventQueue<EventEntry>` in transient; `predict_step` reads from it | — | — |
| P9 Strategy | `ConvergencePlan` owns `StepperStrategy`; transient delegates | — | — |
| P10 Introspect | `ModelDescriptor` on Introspect | Kernel data surfaced | — |

### Dependency ordering

```
P1 (rollback hooks) ──► P3 (lifecycle chart includes rollback)
P2 (limiting API)   ──► P3 (lifecycle chart includes limiting)
P4 (temperature)    ──► P3 (lifecycle chart includes temperature)
P1 ──► P8 (event queue entries carry rollback behavior)
P1 ──► P9 (stepper fold moves reject logic; checkpoint wiring must exist)
P6 (terminals)      ──► P10 (model descriptor + full catalog)
```

P5, P7 are independent. P3 must land LAST (documents the final state).

---

## Code Reuse Analysis

### Existing components to leverage

| Component | Location | How to use |
|-----------|----------|------------|
| `DigitalState::Checkpoint` (one-deep) | `digital/state.rs:9-26` | Pattern for `ElementCheckpoint`: same one-deep semantics (checkpoint before attempt, restore on reject, commit on accept). |
| `digital_hidden_snapshot`/`restore` carrier `(Vec<i64>, Vec<f64>)` | `element.rs:289-295` | **Reuse the carrier type** for `ElementCheckpoint` — devices already know how to pack `(int, real)` state. |
| `Limiter` struct | `codegen/device/analog/limits.rs:13-19` | Extend to produce `LimitingReport` (it already computes `active` + tracks `seeds`/vold — just expose the values). |
| `NewtonRaphsonSolver::state_snapshot`/`restore_state` | `newton_raphson.rs:300-308` | Precedent: the solver already snapshots/restores solution history. The new device checkpoint follows the same lifecycle (snapshot before attempt, restore on reject). |
| `ConvergencePlan` struct | `convergence.rs:275-296` | Add `stepper: Box<dyn StepperStrategy>` field — same pattern as `newton: Box<dyn NewtonStrategy>`. |
| `TerminalDescriptor` / `QueryDescriptor` | `introspect.rs:114-191` | Extend, don't replace — add `TerminalKind` field, populate from kernel data. |
| `AnalogKernel` catalog accessors | `codegen/kernel/analog/mod.rs:234-435` | Bridge to `list_terminals`/`read_opvars` — the kernel already has `terminals()`, `param_names()`, slot counts. |
| `PiController` | `convergence.rs:166-230` | Move into `ConvergencePlan` unchanged — it's already a `StepperStrategy` impl. |
| `TransientSolver` phase methods | `transient.rs:746-1115` | Wire checkpoint/restore into existing `attempt_step`/`reject_step`/`reject_lte_step` — no new methods, extend existing ones. |

### Integration points

| System | Integration method |
|--------|-------------------|
| Transient reject path (`transient.rs:1069,1105`) | Insert `checkpoint_state()` before attempt, `restore_state()` inside both reject methods |
| DC homotopy cascade (`convergence.rs:298-368`) | Insert checkpoint before `plan.solve()`, restore on strategy fallthrough |
| Newton convergence gate (`newton_raphson.rs:375`) | Replace `!system.any_limiting()` with `!system.any_limiting_report()`; replace `apply_convergence_hints` with `apply_limiting_reports` |
| DC bypass gate (`dc.rs:123`) | Replace `!self.any_limiting()` with `!self.any_limiting_report()` |
| `.disto` driver (`disto.rs:375,414,510,560`) | Add capability check before the `let Some(d2) = … else { continue }` |
| `predict_step` (`transient.rs:734-782`) | Read from `EventQueue` instead of four ad-hoc sources |
| `TransientAnalysisOptions` (`transient.rs:84-91`) | Add `probe_selection: ProbeSelection` field |

---

## Components

### C1: Rollback hooks (P1)

- **Purpose**: Checkpoint/restore device-internal mutable state on rejected steps.
- **Location**: `solver/core/element.rs` (trait), `solver/analyses/transient.rs` + `dc.rs` (wiring), `codegen/device/mod.rs` + `analog/mod.rs` (PiperineDevice impl).
- **Interfaces** (on `Element`):
  ```rust
  fn checkpoint_state(&self) -> Option<ElementCheckpoint> { None }
  fn restore_state(&mut self, _ckpt: &ElementCheckpoint) {}
  ```
- **Data model**: see `ElementCheckpoint` below.
- **Dependencies**: `SUPPORTS_ROLLBACK` capability bit (activate from "Reserved").
- **Reuses**: `(Vec<i64>, Vec<f64>)` carrier from `digital_hidden_snapshot`.

### C2: LimitingReport (P2)

- **Purpose**: Structured limiting feedback replacing `limiting_active: bool` + `convergence_hint: Option<ConvergenceHint>`.
- **Location**: `solver/core/element.rs` (trait + struct), `solver/core/circuit.rs` (apply), `solver/math/newton_raphson.rs` (gate), `codegen/device/analog/limits.rs` (produce).
- **Interfaces** (on `AnalogDevice`):
  ```rust
  fn limiting_report(&self) -> Option<LimitingReport> { None }
  ```
  Removes: `limiting_active()`, `convergence_hint()`, `ConvergenceHint` struct.
- **Data model**: see `LimitingReport` below.
- **Reuses**: the existing apply mechanism (`circuit.rs:404` → hard-overwrite guess; same semantics, structured source).

### C3: Lifecycle contract (P3)

- **Purpose**: Ordered hook chart + algorithm flow per analysis, documented + tested.
- **Location**: `docs/spec/part_vii_solver.md` (documentation), `solver/tests/lifecycle_contract.rs` (executable test).
- **Interfaces**: test instruments a recording Element, runs each analysis, asserts hook order.
- **Dependencies**: P1 + P2 + P4 must land first (chart documents the final hook set).

### C4: Temperature protocol (P4)

- **Purpose**: Wire `set_temperature` into the solver lifecycle; add per-instance delta.
- **Location**: `solver/core/element.rs` (existing `set_temperature`), `solver/analyses/dc.rs` + `transient.rs` (call sites), `codegen/device/mod.rs` (override).
- **Interfaces**: `set_temperature(&mut self, t: f64)` — called in `setup` after `allocate_unknowns`.
  Per-instance delta: instance params carry `dtemp`; effective temperature = `t_nominal + dtemp`; `set_temperature` receives the effective value.
- **Reuses**: `Invalidation::Temperature` (already declared, never driven).

### C5: Jacobian capability (P5)

- **Purpose**: Element declares derivative capability; analyses fail loud when absent.
- **Location**: `solver/core/element.rs` (capability bits), `solver/analyses/disto.rs` (fail-loud check).
- **Interfaces**: New `ElementCapabilities` bits: `HAS_DISTO2 = 1 << 12`, `HAS_DISTO3 = 1 << 13`. A `NUMERIC_JACOBIAN = 1 << 14` bit for finite-difference-only devices.
- **Reuses**: `AnalogKernel::has_disto2()`/`has_disto3()` (already compiled; surface as capability).

### C6: Terminal/opvar kernel→ABI bridge (P6)

- **Purpose**: JIT-compiled devices populate `list_terminals`/`read_opvars` from kernel data.
- **Location**: `codegen/device/mod.rs` (`PiperineDevice` Introspect impl), `codegen/kernel/analog/mod.rs` (expose terminal names + opvar path).
- **Interfaces**: `PiperineDevice` overrides `list_terminals()`, `read_opvars()`, `list_queries()`.
- **Reuses**: `AnalogKernel::terminals()`, `DigitalKernel::inputs()`/`outputs()`, symbol table names.

### C7: Save/probe selection (P7)

- **Purpose**: Per-observable recording, not all-or-nothing.
- **Location**: `solver/analyses/transient.rs` (options + collect), `codegen/device/mod.rs` (declare observables).
- **Interfaces**: `ObservableDescriptor { name, kind, cost }` on Introspect; `ProbeSelection` in `TransientAnalysisOptions`.
- **Reuses**: existing `runtime_banks()` + `collect_device_banks()` — extend with filtering.

### C8: Unified event queue (P8)

- **Purpose**: One typed queue for all time-discontinuity sources.
- **Location**: `solver/analyses/transient.rs` (new `EventQueue` field, `predict_step` reads from it), `solver/digital/state.rs` (digital events push into unified queue).
- **Interfaces**: `EventQueue<EventEntry>` with `push`, `peek_next_time`, `drain_due`, `rollback`.
- **Data model**: see `EventEntry` below.
- **Reuses**: `DigitalEvent` (subsumed into `EventEntry`), `SetQueue` (subsumed).

### C9: StepperStrategy fold (P9)

- **Purpose**: `StepperStrategy` owned by `ConvergencePlan`, not `TransientSolver`.
- **Location**: `solver/analyses/convergence.rs` (ConvergencePlan field), `solver/analyses/transient.rs` (delegate to plan).
- **Interfaces**: `ConvergencePlan::stepper()` accessor + `with_stepper()` builder.
- **Reuses**: `PiController` (already implements `StepperStrategy`).

### C10: Model descriptor + introspect leftovers (P10)

- **Purpose**: Model identity + kernel named catalogs surfaced.
- **Location**: `solver/core/introspect.rs` (ModelDescriptor), `codegen/device/mod.rs` (bridge).
- **Interfaces**: `Introspect::model_descriptor() -> ModelDescriptor`.
- **Reuses**: kernel slot counts, terminal names (from P6).

---

## Data Models

### `ElementCheckpoint` (C1)

```rust
/// Opaque device-state checkpoint for rollback on rejected timesteps.
/// Devices pack whatever mutable non-accept-gated state they own.
/// Default `None` = stateless (zero cost).
pub struct ElementCheckpoint {
    pub int_state: Vec<i64>,   // digital registers, edge-detection memory
    pub real_state: Vec<f64>,  // limiter active/seeds/vold, analog vars
}
```

Same shape as `digital_hidden_snapshot`'s `(Vec<i64>, Vec<f64>)` carrier —
deliberately. If PSS recording and per-step rollback are later unified, the
types are compatible.

### `LimitingReport` (C2)

```rust
/// Structured limiting feedback from a device limiter (pnjlim/fetlim lineage).
/// Replaces `limiting_active: bool` + `convergence_hint: Option<ConvergenceHint>`.
///
/// `is_some()` gates Newton convergence (same as the old bool).
/// `limited_value` applied to `net` in the Newton guess (same as the old hint).
/// `limiter_name` + `reason` are diagnostics for hosts.
pub struct LimitingReport {
    /// The unknown the limiter clamped (node voltage or branch current).
    pub net: AnalogReference,
    /// The raw Newton-proposed value before limiting.
    pub proposed: f64,
    /// The clamped value the solver should use.
    pub limited_value: f64,
    /// Which limiter fired (`"pnjlim"`, `"fetlim"`, `"limvds"`, …).
    pub limiter_name: &'static str,
    /// Why the limiter clamped (diagnostic, not behavioral).
    pub reason: LimitReason,
}

pub enum LimitReason {
    /// Junction voltage step too large (pnjlim/fetlim).
    VoltageStep,
    /// Drain-source voltage step too large (limvds).
    VdsStep,
    /// Custom limiter reason (plugin-defined).
    Other(&'static str),
}
```

**Merge semantics**: when multiple devices report the same `net`, the solver
applies each in iteration order (last wins) — same as today's
`apply_convergence_hints`. This is documented, not "correct"; a future
enhancement could detect conflicts.

### `LimitingReport` consumer API (replaces `any_limiting` + `apply_convergence_hints`)

```rust
// On NonLinearSystem (replaces any_limiting + apply_convergence_hints):
fn any_limiting_report(&self) -> bool {
    self.circuit.devices.iter().any(|d| d.limiting_report().is_some())
}

fn apply_limiting_reports(&self, mut guess: ndarray::ArrayViewMut1<f64>) {
    for dev in &self.circuit.devices {
        if let Some(report) = dev.limiting_report()
            && let Some(i) = report.net.as_index()
            && i < guess.len()
        {
            guess[i] = report.limited_value;
        }
    }
}
```

### `EventEntry` (C8)

```rust
/// One entry in the unified event queue.
pub struct EventEntry {
    pub kind: EventKind,
    pub time: f64,
    pub target: EventTarget,
    pub priority: EventPriority,
    pub source: EventSource,
    pub rollback: RollbackBehavior,
}

pub enum EventKind {
    /// Digital scheduler net change.
    Digital,
    /// Analog discontinuity (pulse edge, PWL corner, @timer).
    Breakpoint,
    /// `$bound_step` advisory floor.
    StepHint,
    /// Analog crossing detected (A2D without digital scheduler).
    Crossing,
}

pub enum EventTarget {
    /// Digital net index.
    Net(DigitalNet),
    /// Analog source (for breakpoint re-evaluation).
    Source(usize),
    /// Advisory — no specific target (StepHint).
    Advisory,
}

pub enum EventPriority {
    /// Must land exactly on this time (digital, breakpoint).
    Exact,
    /// Soft floor (StepHint — preferred, not mandatory).
    Advisory,
}

pub enum EventSource {
    /// Element name (for diagnostics).
    Element(String),
    /// Scheduled live-parameter set.
    ScheduledSet,
    /// System (integrator, etc.).
    System,
}

pub enum RollbackBehavior {
    /// Restore this event on step rejection (digital events).
    Restore,
    /// Re-poll next step (breakpoints are stateless — re-declared each step).
    RePoll,
    /// Discard on rejection (crossings — re-detected next attempt).
    Discard,
}
```

The queue is a `BinaryHeap<Reverse<EventEntry>>` ordered by `(time, priority)`,
same as the current `DigitalState::event_queue`.

### `ModelDescriptor` (C10)

```rust
/// Model identity and version for diagnostics + introspection.
pub struct ModelDescriptor {
    /// Source-level type (`"mos"`, `"diode"`, `"bjt"`).
    pub type_id: String,
    /// Model version (`"3"`, `"3.1"`, `""` if unversioned).
    pub version: String,
}
```

### `TerminalKind` (C6, extends `TerminalDescriptor`)

```rust
/// Whether a terminal is user-facing or internal.
pub enum TerminalKind {
    /// A port declared in the module signature (user-facing).
    External,
    /// An internal node (non-port `wire` — series-R, thermal, etc.).
    Internal,
    /// An auxiliary node (hidden, diagnostic-only — e.g., a probe point).
    Auxiliary,
}
```

### `ObservableDescriptor` + `ProbeSelection` (C7)

```rust
/// A device-declared observable that a host can request for recording.
pub struct ObservableDescriptor {
    pub name: String,
    pub kind: ObservableKind,
    /// Relative recording cost (0 = free, 1 = full bank clone).
    pub cost: f32,
}

pub enum ObservableKind {
    BranchCurrent,
    Charge,
    Flux,
    State,
    Var,
}

/// Per-device list of requested observables for recording.
pub struct ProbeSelection {
    /// (device_label, observable_name) pairs.
    pub requests: Vec<(String, String)>,
}
```

---

## Error Handling Strategy

| Error scenario | Handling | User impact |
|----------------|----------|-------------|
| Device declares `SUPPORTS_ROLLBACK` but `checkpoint_state` returns `None` | Treat as stateless — skip restore. Not an error. | None |
| Checkpoint taken but device destroyed before step resolves (live-param rebuild mid-step) | Checkpoint is owned by the step-attempt scope (`StepAttempt`), dropped on rebuild. No use-after-free. | None |
| `.disto` runs and no device declares `HAS_DISTO2` | Emit named warning (`SolverDomain::Element`, "no device provides disto2 capability; results will be zero"). | Warning in output; analysis completes with zeros. |
| `.disto` runs and a device has `NUMERIC_JACOBIAN` | Fail loud: `Err(SolverDomain::Element, "device `{name}` has numeric-only Jacobian; .disto requires analytic derivatives")`. | Named error; analysis aborts. |
| Temperature sweep hits device that doesn't override `set_temperature` | Default no-op — device reads `$temperature` at eval time (backward compatible). | None |
| `ProbeSelection` requests unknown observable | Fail loud: `Err(SolverDomain::Element, "device `{label}` has no observable `{name}`")`. | Named error at setup time. |
| Unified event queue empty (no events) | `predict_step` falls back to PI-proposed dt. | None |
| Multiple `LimitingReport` on same net | Last-iterated device wins (documented). | None (same as today) |

---

## Risks & Concerns

| Concern | Location | Impact | Mitigation |
|---------|----------|--------|------------|
| **Rollback changes parity baselines** | `parity_baseline.rs` | The transient step sequence changes if rejected steps now restore limiter state differently. Baselines might not be bit-identical. | Run baselines first; if they shift, the shift IS the bugfix (dirty limiter was producing a different — wrong — sequence). Re-pin baselines with a documented commit. |
| **Checkpoint cost on every attempt** | `transient.rs:790` | `checkpoint_state` is called on every attempt, including accepted steps (checkpoint then discard). For devices with large state vectors, this is a per-step clone. | Default `None` = zero cost. Only devices that declare `SUPPORTS_ROLLBACK` pay. The codegen PiperineDevice only checkpoints the limiter (~2 fields) + digital registers (small Vec). Benchmark in tasks. |
| **EventQueue unification is a large refactor** | `transient.rs:734-782`, `digital/state.rs`, `digital/events.rs` | Four ad-hoc sources → one typed queue touches the hottest transient path. Risk of regressions. | Land P8 AFTER P1 (rollback behavior per entry type is defined). Keep `DigitalState::event_queue` as the backing store for digital events inside the unified queue (adapter, not rewrite). Run parity baselines. |
| **`convergence_hint` removal is a public API break** | `prelude.rs:17`, `abi.rs:6` | `ConvergenceHint` is re-exported. Removing it breaks downstream code. | No in-repo consumer produces one (dead wire). For external: the removal is documented; `LimitingReport` is the replacement. Flag in release notes. |
| **`set_temperature` in setup changes all circuits** | `dc.rs`, `transient.rs` | Today `set_temperature` is never called; calling it in `setup` means every device's `set_temperature` override (default no-op) runs once. Codegen devices that DON'T override are unaffected. | Default no-op = zero behavioral change for devices that don't override. Codegen's override will be additive (compute temp constants, same as `$temperature` eval). |
| **Opvar compilation path is new codegen work** | `codegen/kernel/analog/` | P6 requires compiling an opvar-evaluation function. This is the most open-ended codegen task — the kernel doesn't have one today. | Scope P6 carefully: terminals bridge is straightforward (data already in kernel); opvars may need a deferred follow-up if the compilation path proves large. Flag in tasks. |
| **StepperStrategy fold touches the transient hot loop** | `transient.rs:963,1069,1105` | Moving reject/propose logic into the plan changes the call path for every step. | The logic itself doesn't change — only its owner. PiController's impl stays bit-identical. Parity baselines must pass. |

---

## Tech Decisions

Resolves all 12 open questions from `spec.md` Assumptions & Open Questions.

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | Checkpoint carrier type | `ElementCheckpoint { int_state: Vec<i64>, real_state: Vec<f64> }` — same shape as `digital_hidden_snapshot` carrier | Devices already know how to pack `(int, real)` state; compatible if PSS + per-step unify later |
| 2 | PSS vs per-step rollback | Separate mechanisms for now. `digital_hidden_snapshot` stays for PSS recording; new `checkpoint_state`/`restore_state` for per-step rollback. | Different lifecycles (PSS records every step into `TransientStep`; rollback fires on reject only). Unifying risks complicating the hot path. Revisit if profiling shows duplication. |
| 3 | DC homotopy retry rollback | `checkpoint_state` before `plan.solve()`; `restore_state` on strategy fallthrough (before next strategy attempt). Same hook as transient. | DC Newton failures dirty the limiter identically. |
| 4 | LimitingReport replaces both | `Option<LimitingReport>` — `is_some()` gates convergence (replaces bool); `limited_value` applied to `net` (replaces hint). Strategy/test separation preserved. | One concept, one method. The strategy stays limiting-agnostic (`convergence.rs:66` comment honored). |
| 5 | `.disto` fail-loud threshold | **Warn** (not fail) when no device contributes disto2/disto3 — the analysis completes with zeros + a named diagnostic. **Fail loud** only when a `NUMERIC_JACOBIAN` device is present and `.disto` requires analytic derivatives. | Warn matches "the circuit is linear" (legitimate zero result). Fail matches "the device can't provide what the analysis needs" (real error). |
| 6 | Jacobian capability shape | New `ElementCapabilities` bits: `HAS_DISTO2`, `HAS_DISTO3`, `NUMERIC_JACOBIAN`. Not an enum. | Consistent with existing bitflags pattern; a device sets the bits it supports; the analysis checks before running. |
| 7 | Unified event queue type | `EventQueue<EventEntry>` — `BinaryHeap<Reverse<EventEntry>>` ordered by `(time, priority)`. Backed by adapters for the four current sources. | Same heap type as `DigitalState::event_queue`; adapters preserve existing semantics while unifying the read path in `predict_step`. |
| 8 | Save/probe observable catalog | `ObservableDescriptor { name, kind, cost }` on Introspect; `ProbeSelection` in `TransientAnalysisOptions`. Global `record_device_state: bool` becomes shorthand for "all observables on all devices". | Backward compatible (empty `ProbeSelection` = today's `record_device_state=false`). |
| 9 | Temperature protocol shape | Keep single-arg `set_temperature(f64)` — called in `setup` with the effective temperature (`t_nominal + dtemp_instance`). Per-instance `dtemp` stays an instance param (existing mechanism). `Invalidation::Temperature` driven by temp sweeps. | Minimal API surface change; `dtemp` already exists in stdlib models; `set_temperature` receives the composed value so devices don't re-derive it. |
| 10 | Terminal internal/auxiliary kind | New `TerminalKind { External, Internal, Auxiliary }` field on `TerminalDescriptor`. | Follows the existing descriptor pattern (add a field, not a new type). |
| 11 | Opvar compilation path | Codegen compiles an opvar-evaluation `AnalogFn` alongside the residual (reads the same state/var banks, evaluates declared opvar expressions). `read_opvars` calls it post-solve. | Follows the existing kernel pattern (one `AnalogFn` per concern). The opvar expressions are already in the PHDL source (e.g., `var gm = …` in `mos.phdl`) — they just need a compiled accessor. **Risk**: if the compilation path proves large, defer to a follow-up; the terminal bridge (straightforward) ships first. |
| 12 | StepperStrategy fold mechanics | Add `stepper: Box<dyn StepperStrategy>` to `ConvergencePlan` (same pattern as `newton`). Transient delegates `propose_dt`/`reject_dt` to `plan.stepper()`. `PiController` impl unchanged. | Completes the composition triad (Newton + Homotopy + Stepper). `ConvergencePlan` becomes the single strategy owner for all analyses. |

> **Project-level decisions:** decisions 1, 4, and 12 set conventions (checkpoint
> carrier shape, limiting API shape, strategy composition completeness) that
> future features must follow. If approved, append to `.specs/STATE.md` as
> MD-26/27/28 (or amend existing MDs).

---

## Phasing recommendation

The 10 stories have natural dependencies (see diagram above). Recommended
implementation order, grouped into batches:

| Batch | Stories | Rationale |
|-------|---------|-----------|
| 1 | **P1 + P2** | Rollback + limiting are coupled (limiter state is checkpointed). Both touch the Newton convergence gate. ~13 requirements. |
| 2 | **P4 + P5** | Temperature + Jacobian capability are independent ABI additions. ~9 requirements. |
| 3 | **P6 + P10** | Terminal/opvar bridge + introspect leftovers are the codegen→ABI surface. ~8 requirements. |
| 4 | **P9** | StepperStrategy fold — pure refactor, parity-gated. ~4 requirements. |
| 5 | **P8** | Unified event model — largest refactor, depends on P1 (rollback behavior per entry). ~6 requirements. |
| 6 | **P7** | Save/probe selection — independent, lowest risk. ~4 requirements. |
| 7 | **P3** | Lifecycle contract — documents the final state, must land last. ~5 requirements. |

Total: 48 requirements across 7 batches. At ~7 tasks/batch, this is ~7 worker
batches if sub-agents are used (see Sub-Agent Delegation in SKILL.md).

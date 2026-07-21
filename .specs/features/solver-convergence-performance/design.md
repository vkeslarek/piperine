# Solver Convergence & Performance Design

**Spec**: `.specs/features/solver-convergence-performance/spec.md`
**Status**: Draft

---

## Architecture Overview

This feature is **surgical** — it wires up existing-but-unused machinery
(`reset()`, `BYPASS_OK`, `suggest_transient_step`), threads misplaced state
(`max_iter`/`dc_damp_tolerance` off `Context` onto `Policy`), and adds one new
data type (`SolverStats`). No new architecture; every change targets a specific
`file:line` identified by the audit.

```
┌─────────────────────────────────────────────────────────────┐
│ DcSolver / TransientSolver                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐ │
│  │ Newton loop  │  │ Step loop    │  │ SolverStats        │ │
│  │  reset() ←───┼──┤ accept/reject├──┤ accumulates here   │ │
│  │  hoisted vec │  │ dt_min flag  │  │ counters + timing  │ │
│  └──────┬───────┘  └──────┬───────┘  └─────────┬──────────┘ │
│         │                 │                     │            │
│  ┌──────▼───────┐  ┌──────▼───────┐            │            │
│  │ Policy       │  │ Predictor    │            │            │
│  │ (from Context│  │ x̂=xₙ+Δ·dtₙ₊₁│            │            │
│  │  NOT default)│  │  /dtₙ        │            │            │
│  └──────────────┘  └──────────────┘            │            │
│                                                │            │
│  ┌─────────────────────────────────────────────▼──────────┐ │
│  │ Element (per-device)                                    │ │
│  │  BYPASS_OK → skip evaluate if terminals unchanged       │ │
│  │  convergence_hint() → Option<(NetRef, f64)>             │ │
│  │  suggest_transient_step() → dt floor                    │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  Result: DcAnalysisResult / TransientAnalysisResult          │
│    .stats: SolverStats  ←─── new field                       │
└─────────────────────────────────────────────────────────────┘
```

### Approach

One approach — no alternatives needed. The audit identified the exact changes;
this feature executes them. The risk is low because:
- `reset()` exists and is tested (just never called)
- `BYPASS_OK` is declared (just never consulted)
- `Policy` already has the fields (just hardcoded to `default()`)
- The Tolerances/Policy split is documented in STATE.md (MD-04) as locked-but-pending

---

## Code Reuse Analysis

### Existing Components to Leverage

| Component | Location | How to Use |
|-----------|----------|------------|
| `FaerSystem::reset()` | `math/faer.rs:70` | Call instead of `L::new()` in Newton loop — eliminates per-iter matrix alloc |
| `ElementCapabilities::BYPASS_OK` | `core/element.rs:71` | Consult in DC/transient `assemble` — skip devices whose terminals haven't moved |
| `Element::suggest_transient_step` | `core/element.rs:309` | Call in transient driver's dt proposal — per-device LTE floor |
| `Element::limiting_active` | `core/element.rs:123` | Evolve into `convergence_hint()` returning structured data |
| `Policy` struct | `solver/mod.rs:142` | Already has `dc_damp_tolerance` — just thread the real one instead of `default()` |
| `DampedNewton` strategy | `solver/convergence.rs:86` | Already implements `damp_update` — just receives the real `Policy` |
| `CircularArrayBuffer2` | `math/circular_array.rs` | Already stores history — predictor reads `view(0)` + `view(1)` |

### Integration Points

| System | Integration Method |
|--------|-------------------|
| `SolverStats` → result types | Add `pub stats: SolverStats` field to `DcAnalysisResult` + `TransientAnalysisResult` in `result.rs` |
| `SolverStats` → Python | Add `_SolverStats` pyclass in `piperine-python/src/results.rs`; expose via `op.stats` getter |
| `SolverStats` → CLI | Print summary after `piperine run` when `--stats` flag is set (future; not in this feature) |
| `Policy` → Newton | `solve_with_strategy` gains a `&Policy` param (replacing the 5 `Policy::default()` sites) |
| Bypass → `assemble` | Track per-element last-terminal-voltages; compare against tolerance before calling `load_dc`/`load_transient` |

---

## Components

### SolverStats

- **Purpose**: Per-analysis convergence + performance diagnostics. Accumulates
  counters during the solve; returned on the result type.
- **Location**: `crates/piperine-solver/src/result.rs` (new struct)
- **Interface**:
  ```rust
  pub struct SolverStats {
      // Newton (DC + each transient step)
      pub newton_iterations: usize,
      pub converged: bool,
      // Transient
      pub steps_accepted: usize,
      pub steps_rejected: usize,
      pub dt_min_floor_hits: usize,
      pub dt_min: f64,
      pub dt_max: f64,
      // Device-level
      pub bypass_hits: usize,
      pub bypass_misses: usize,
      // Homotopy
      pub homotopy_strategy: Option<String>,
      pub homotopy_levels: usize,
      // Timing (nanoseconds)
      pub assembly_time_ns: u64,
      pub solve_time_ns: u64,
  }
  ```
- **Dependencies**: none (plain data struct; `Default::default()` zeroes everything)
- **Reuses**: nothing — new type

### Policy threading

- **Purpose**: Replace the 5 `Policy::default()` sites with the real `Policy`
  derived from `Context` / analysis options.
- **Location**: `solver/dc.rs:101,185,222`; `solver/transient.rs:84,206`;
  `math/newton_raphson.rs` (`solve_with_strategy` gains `&Policy` param)
- **Interface**: `solve_with_strategy(&mut self, system, strategy, policy: &Policy) -> Result<...>`
- **Reuses**: `Policy` struct already exists (`solver/mod.rs:142`); `DampedNewton::damp_update` already takes `&Policy`

### Matrix reuse (`reset()`)

- **Purpose**: Stop rebuilding the linear system every Newton iteration.
- **Location**: `math/newton_raphson.rs:154,266` — replace `self.linear_system = L::new(...)` with `self.linear_system.reset()`
- **Reuses**: `FaerSystem::reset()` at `math/faer.rs:70`

### Hoisted work vectors

- **Purpose**: Stop allocating `residual` + `scale` Vecs every iteration.
- **Location**: `math/newton_raphson.rs` — move to fields on the Newton solver struct; `.fill(0.0)` per iteration
- **Reuses**: existing `Vec` usage pattern, just hoisted

### Device bypass

- **Purpose**: Skip re-evaluation of nonlinear devices whose terminals haven't moved.
- **Location**: `solver/dc.rs:52` (DC assemble loop), `solver/transient.rs:60` (tran assemble loop)
- **Interface**: Before calling `device.load_dc(...)`, check `device.capabilities().contains(BYPASS_OK)` AND terminal-voltage delta < tolerance; if both true, reuse last stamps (skip `load_dc`).
- **Dependencies**: Per-element last-terminal-voltage storage (on `CircuitInstance` or a side table)
- **Reuses**: `ElementCapabilities::BYPASS_OK` (`core/element.rs:71`); tolerance from `Context.tolerances`

### Convergence hint

- **Purpose**: Evolve `limiting_active() -> bool` into structured data the solver acts on.
- **Location**: `core/element.rs:123` — add `fn convergence_hint(&self) -> Option<ConvergenceHint>`; `solver/dc.rs:77` + `solver/transient.rs:68` — apply the hint before convergence test
- **Interface**:
  ```rust
  pub struct ConvergenceHint {
      pub net: NetRef,
      pub limited_value: f64,
  }
  ```
- **Reuses**: replaces the boolean `limiting_active` gate (which only refused convergence; the hint also clamps the value)

### Newton predictor

- **Purpose**: Seed Newton with a first-order extrapolation instead of the last accepted point.
- **Location**: `math/newton_raphson.rs:226-231` — when `state.depth() >= 2`, compute `x̂ = xₙ + (xₙ − xₙ₋₁) · dtₙ₊₁/dtₙ`
- **Reuses**: `CircularArrayBuffer2::view(0)` (xₙ) + `view(1)` (xₙ₋₁)

### Tolerances/Policy split

- **Purpose**: Move `max_iter`, `dc_damp_tolerance`, `time` off `Context` (claimed immutable, actually mutated) onto `Policy` / analysis context.
- **Location**: `solver/mod.rs:161-174` — `Context` keeps only `Tolerances` (immutable, `Copy`); `Policy` gains `max_iter` + `dc_damp_tolerance`
- **Reuses**: MD-04 (Tolerances vs Policy) is already a locked decision; this implements it

### Dead code removal

- **Purpose**: Remove `apply_limit` (dead, `dc.rs:96-102`, `transient.rs:79-85`), `Policy::damp_update` (duplicate, `mod.rs:142-158`), `_alpha` parameter (3 `assemble` impls)
- **Reuses**: nothing — pure deletion

---

## Data Models

### SolverStats

```rust
#[derive(Debug, Clone, Default)]
pub struct SolverStats {
    pub newton_iterations: usize,
    pub converged: bool,
    pub steps_accepted: usize,
    pub steps_rejected: usize,
    pub dt_min_floor_hits: usize,
    pub dt_min: f64,
    pub dt_max: f64,
    pub bypass_hits: usize,
    pub bypass_misses: usize,
    pub homotopy_strategy: Option<String>,
    pub homotopy_levels: usize,
    pub assembly_time_ns: u64,
    pub solve_time_ns: u64,
}
```

**Relationships**: carried by `DcAnalysisResult.stats` and `TransientAnalysisResult.stats`; exposed to Python as `_SolverStats` pyclass.

---

## Error Handling Strategy

| Error Scenario | Handling | User Impact |
|----------------|----------|-------------|
| `max_iter` exceeded | Existing: `Error::NonConvergence` — no change | Solver reports failure; user sees `stats.converged = false` + `stats.newton_iterations = max_iter` |
| LTE rejected at `dt_min` | Accept step + `tracing::warn!` + `stats.dt_min_floor_hits += 1` | Waveform is slightly inaccurate at that point; user can see the count in stats |
| Predictor overshoots | Newton still runs (predictor is just a seed); damping/homotopy catch divergence | No user impact — Newton converges from the predicted seed or doesn't |
| Bypass stale across steps | Force re-eval at least once per accepted step (reset bypass flag on accept) | No stale stamps — correctness preserved |

---

## Risks & Concerns

| Concern | Location | Impact | Mitigation |
|---------|----------|--------|------------|
| **Symbolic LU pattern cloned per iter** | `faer.rs:120` | Per-iteration `.clone()` of the pattern; the "REUSE Symbolic" comment is a lie | P2 task: hold the pattern in an `Rc` or store alongside the solver; not P1-critical (numeric factorization dominates anyway) |
| **Unsafe raw-pointer aliasing** (3 sites) | `dc.rs:187`, `dc.rs:227`, `transient.rs:212` | Borrow-checker workaround; if CircuitInstance layout changes, UB | Not touched by this feature; flagged for future `SignalBridge` extraction |
| **`Context.time` mutated per-iteration** | `transient.rs:56` | Breaks the "immutable Context" contract (MD-03 violation) | Tolerances/Policy split task moves `time` to `TransientContext`; partial fix in this feature |
| **`apply_damping` is global midpoint** | `convergence.rs:89` | Can't target the specific oscillating junction | Convergence hint (CP-12) addresses this: the device says which net to clamp |
| **`init_global` pins faer to 1 thread** | `mod.rs:196` | Linear solve is single-threaded even with rayon available | Out of scope (parallelism follow-up); but SolverStats timing will make the bottleneck measurable |

---

## Tech Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Stats always-on vs opt-in | Always-on (counter increments) | Negligible overhead (usize += 1); opt-in adds complexity for ~zero gain |
| Bypass storage | Per-element last-terminal snapshot on `CircuitInstance` | Side-table would need element indexing; storing on the element is natural (Element already has state) |
| Convergence hint shape | `Option<ConvergenceHint { net, limited_value }>` | Minimal evolution of `limiting_active`; the device says WHAT net and WHAT value, not just "something is wrong" |
| Predictor order | First-order linear extrapolation | Simplest meaningful predictor; ngspice default; higher orders (Adams-Bashforth) are follow-up |

> **Project-level decisions:** none new. This feature implements existing
> locked decisions (MD-04 Tolerances/Policy split, MD-05 strategy composition,
> MD-12 ABI vs policy classification).

# Solver Convergence & Performance Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.**

---

**Design**: `.specs/features/solver-convergence-performance/design.md`
**Status**: Delivered (13/13, 2026-07-16)

---

## Test Coverage Matrix

> Generated from `AGENTS.md` (test placement, zero warnings) + spec. Guidelines: `AGENTS.md` (Hard rules, Test placement, MD-13).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| SolverStats struct + result wiring | unit | Fields exist, Default zeroes, carried by result types | `piperine-solver/src/result.rs` (`#[cfg(test)]`) | `cargo test -p piperine-solver` |
| Newton loop (reset, hoist, Policy, predictor) | integration | DC/tran results identical (±1e-9); max_iter honored; zero per-iter allocs after first | existing bench tests + `piperine-solver/tests/` | `cargo test --workspace` |
| Device bypass / convergence hint / suggest_step / gshunt | integration | Bypass skips unchanged devices; hint clamps; iteration count drops vs no-bypass | `piperine-solver/tests/` | `cargo test -p piperine-solver` |
| Python `_SolverStats` | unit (embedded Python) | `op.stats.newton_iterations` returns meaningful positive int; `trace.stats.steps_rejected` exists | `piperine-python/src/lib.rs` (`#[cfg(test)]`) | `cargo test -p piperine-python` |
| Tolerances/Policy split + MD-13 | build + integration | `Context` has no mutable policy fields; `solver/mod.rs` has no free `pub(crate) fn`; results identical | build gate + `cargo test --workspace` | `cargo build --workspace` + `cargo test --workspace` |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Crate | After solver-only tasks | `cargo test -p piperine-solver` |
| Python | After Python exposure tasks | `cargo test -p piperine-python` |
| Build | Every task | `cargo build --workspace` (zero warnings) |
| Full | Phase end | `cargo test --workspace` + 21/21 `examples/*.py` |

---

## Execution Plan

```
Phase 1 (stats+tol)  →  Phase 2 (zero-alloc+cleanup)  →  Phase 3 (device-level)  →  Phase 4 (arch)
```

### Phase 1: SolverStats + tolerance threading

```
T1 → T2 → T3 → T4
```

### Phase 2: Zero-alloc Newton loop + dead code cleanup

```
T5 → T6
```

### Phase 3: Device bypass + convergence hints + gshunt

```
T7 → T8 → T9 → T10
```

### Phase 4: Architecture (Tolerances/Policy split + predictor + MD-13)

```
T11 → T12 → T13
```

---

## Task Breakdown

### T1: SolverStats struct + result type wiring

**What**: Define `SolverStats` in `piperine-solver/src/result.rs` with all fields from design (newton_iterations, converged, steps_accepted/rejected, dt_min_floor_hits, dt_min, dt_max, bypass_hits/misses, homotopy_strategy, homotopy_levels, assembly_time_ns, solve_time_ns). Add `pub stats: SolverStats` to `DcAnalysisResult` and `TransientAnalysisResult`. `Default::default()` zeroes everything.
**Where**: `crates/piperine-solver/src/result.rs`
**Depends on**: None
**Requirement**: CP-01, CP-02 (struct only)

**Done when**:
- [ ] `SolverStats` struct exists with all fields from design
- [ ] `DcAnalysisResult` and `TransientAnalysisResult` carry `pub stats: SolverStats`
- [ ] `Default::default()` zeroes/sanes every field
- [ ] `cargo build --workspace` zero warnings
**Tests**: unit — `SolverStats::default()` zeroes all fields; result types construct with stats
**Gate**: crate
**Commit**: `feat(solver): SolverStats struct + result type wiring`

---

### T2: Thread real Policy through solve_with_strategy

**What**: Add `policy: &Policy` parameter to `solve_with_strategy` in `newton_raphson.rs`. Replace the 5 `Policy::default()` sites (`dc.rs:101,185,222`, `transient.rs:84,206`) with the real `Policy` derived from `Context.dc_damp_tolerance` / `Context.max_iter`. This makes user-set tolerances actually reach the Newton loop.
**Where**: `crates/piperine-solver/src/math/newton_raphson.rs`, `solver/dc.rs`, `solver/transient.rs`
**Depends on**: T1 (result types have stats field ready)
**Requirement**: CP-04, CP-05

**Done when**:
- [ ] `solve_with_strategy` takes `&Policy` (not hardcoded `default()`)
- [ ] Setting `Context.max_iter=3` causes non-convergence after exactly 3 iterations
- [ ] Setting `Context.dc_damp_tolerance=0.3` changes damping behavior
- [ ] DC/tran results identical (±1e-9) with default settings
**Tests**: integration — `max_iter=3` on divider → non-converge; `max_iter=200` → converge; default → identical results
**Gate**: full
**Commit**: `fix(solver): thread real Policy through Newton (tolerances no longer ignored)`

---

### T3: Wire stats accumulation into DC + transient drivers

**What**: Instrument the DC and transient drivers to populate `SolverStats` during the solve. Count Newton iterations, track converged flag, count step accept/reject, track dt range, count dt_min floor hits, count bypass hits/misses, record homotopy strategy/levels, time assembly vs solve with `std::time::Instant`. Return the populated stats on the result.
**Where**: `crates/piperine-solver/src/solver/dc.rs`, `solver/transient.rs`, `solver/convergence.rs` (homotopy tracking)
**Depends on**: T1 (struct), T2 (Policy threading — stats are on the same code paths)
**Requirement**: CP-01, CP-02, CP-03

**Done when**:
- [ ] `op.stats.newton_iterations` returns a positive integer on the divider
- [ ] `trace.stats.steps_accepted > 0` and `trace.stats.dt_max > 0` on a tran
- [ ] `trace.stats.steps_rejected` is populated (may be 0 on easy circuits)
- [ ] `trace.stats.assembly_time_ns > 0` and `solve_time_ns > 0`
- [ ] DC/tran results identical (±1e-9)
**Tests**: integration — run divider op + RC tran, assert stats fields are populated
**Gate**: full
**Commit**: `feat(solver): wire SolverStats accumulation into DC + transient drivers`

---

### T4: Python _SolverStats exposure

**What**: Add `_SolverStats` pyclass in `piperine-python/src/results.rs` wrapping the solver `SolverStats`. Add `.stats` getter to `_OpResult`, `_Trace`, `_AcTrace`, `_NoiseTrace`. Every field exposed as a typed Python attribute.
**Where**: `crates/piperine-python/src/results.rs`, `crates/piperine-python/src/lib.rs` (register class)
**Depends on**: T3 (stats are populated on the results)
**Requirement**: CP-09

**Done when**:
- [ ] `op.stats.newton_iterations` returns an int from Python
- [ ] `trace.stats.steps_accepted` returns an int
- [ ] `trace.stats.dt_min_floor_hits` returns an int (0 on easy circuits)
- [ ] All 13 fields accessible from Python
**Tests**: unit — embedded Python: load divider, op, assert `op.stats.newton_iterations > 0`; tran, assert `trace.stats.steps_accepted > 0`
**Gate**: python
**Commit**: `feat(python): expose SolverStats via op.stats / trace.stats`

---

### T5: Zero-alloc Newton loop (reset + hoist vectors)

**What**: Replace `self.linear_system = L::new(...)` with `self.linear_system.reset()` at `newton_raphson.rs:154,266`. Hoist `residual` and `scale` Vecs from per-iteration locals to fields on the Newton solver struct; `.fill(0.0)` per iteration instead of `vec![...]`.
**Where**: `crates/piperine-solver/src/math/newton_raphson.rs`, `math/faer.rs` (verify `reset()` is sound)
**Depends on**: T2 (Policy threading changed the same function signature)
**Requirement**: CP-06

**Done when**:
- [ ] `reset()` is called instead of `L::new()` in both `solve` and `solve_with_strategy`
- [ ] `residual` and `scale` are struct fields, not per-iteration allocations
- [ ] DC/tran results identical (±1e-9)
**Tests**: integration — divider op + RC tran, assert identical results; (alloc verification is structural — code review confirms no `vec!` in the loop)
**Gate**: full
**Commit**: `perf(solver): zero-alloc Newton loop (reset + hoisted work vectors)`

---

### T6: Delete dead code + dt_min floor warning

**What**: Delete `DcSystem::apply_limit` (`dc.rs:96-102`), `TransientSystem::apply_limit` (`transient.rs:79-85`), and `Policy::damp_update` (`mod.rs:142-158`) — all dead (bypassed by `solve_with_strategy`). At `transient.rs:324-337`, when an LTE-rejected step is accepted at `dt_min`, increment `stats.dt_min_floor_hits` and emit `tracing::warn!`.
**Where**: `crates/piperine-solver/src/solver/dc.rs`, `solver/transient.rs`, `solver/mod.rs`
**Depends on**: T3 (stats struct is where the counter goes), T5 (same files)
**Requirement**: CP-07, CP-08

**Done when**:
- [ ] `grep -rn "apply_limit" crates/piperine-solver/src/` finds only the trait default (no-op)
- [ ] `grep -rn "Policy::damp_update" crates/piperine-solver/src/` finds nothing
- [ ] `tracing::warn!` fires when LTE-rejected step accepted at dt_min
- [ ] `stats.dt_min_floor_hits` is incremented in that case
- [ ] DC/tran results identical
**Tests**: integration — grep checks + existing tests pass
**Gate**: full
**Commit**: `refactor(solver): delete dead apply_limit + surface dt_min floor hits`

---

### T7: Device bypass (BYPASS_OK)

**What**: In DC and transient `assemble` loops, before calling `device.load_dc(...)` / `device.load_transient(...)`, check `device.capabilities().contains(BYPASS_OK)`. If true AND all terminal voltages changed by less than `reltol·|V| + abstol` since the last evaluation, skip the load and reuse the previous stamps. Track per-element last-terminal-voltages on `CircuitInstance` or a side vector. Count hits/misses in stats.
**Where**: `crates/piperine-solver/src/solver/dc.rs` (assemble), `solver/transient.rs` (assemble), `core/circuit.rs` (terminal tracking), `core/element.rs` (BYPASS_OK already declared)
**Depends on**: T3 (stats counters), T5 (same assemble loops)
**Requirement**: CP-11

**Done when**:
- [ ] A device with `BYPASS_OK` whose terminals don't move is skipped (reuse last stamps)
- [ ] `stats.bypass_hits > 0` on a circuit with bypassable devices
- [ ] DC/tran results identical (±1e-9)
- [ ] Every device is re-evaluated at least once per accepted transient step (no stale stamps)
**Tests**: integration — divider (linear, all bypassable) → bypass_hits > 0; results identical; re-eval per step verified
**Gate**: full
**Commit**: `feat(solver): device bypass (BYPASS_OK capability consulted)`

---

### T8: Convergence hint (evolve limiting_active)

**What**: Add `ConvergenceHint { net: NetRef, limited_value: f64 }` type in `core/element.rs`. Add `fn convergence_hint(&self) -> Option<ConvergenceHint>` to `Element` (default: derive from existing `limiting_active()` for backward compat). In DC/transient convergence check (`dc.rs:77`, `transient.rs:68`), apply the hint (clamp the net value) before testing convergence, instead of the boolean-only gate.
**Where**: `crates/piperine-solver/src/core/element.rs`, `solver/dc.rs`, `solver/transient.rs`
**Depends on**: T7 (same convergence check code)
**Requirement**: CP-12

**Done when**:
- [ ] `ConvergenceHint` type exists
- [ ] `Element::convergence_hint()` returns the structured data
- [ ] The solver applies the limited value before convergence test
- [ ] DC/tran results identical (±1e-9) — existing circuits don't use hints
**Tests**: integration — existing tests pass (no circuit uses hints yet); type exists + is accessible
**Gate**: full
**Commit**: `feat(solver): convergence_hint replaces boolean limiting_active`

---

### T9: suggest_transient_step consulted by stepper

**What**: In `transient.rs:362`, after the PI controller proposes `dt`, call `suggest_transient_step` on every reactive device. If any device returns a floor smaller than the proposed dt, shrink dt to the minimum floor.
**Where**: `crates/piperine-solver/src/solver/transient.rs`, `core/element.rs` (method already exists)
**Depends on**: T7 (same transient driver)
**Requirement**: CP-13

**Done when**:
- [ ] `suggest_transient_step` is called by the transient driver
- [ ] A device returning a tight floor causes dt to shrink
- [ ] Existing tran results identical (±1e-9) — current devices don't override it
**Tests**: integration — existing tran tests pass
**Gate**: full
**Commit**: `feat(solver): consult suggest_transient_step for per-device dt floor`

---

### T10: gshunt diagonal conductance

**What**: Add `gshunt: f64` to `Tolerances` (default 0.0). When `gshunt > 0.0`, stamp a diagonal `gshunt` conductance to ground on every analog node during DC and transient assembly. This is a circuit-wide convergence aid for floating topologies.
**Where**: `crates/piperine-solver/src/solver/mod.rs` (Tolerances), `solver/dc.rs` (stamp), `solver/transient.rs` (stamp)
**Depends on**: T5 (same assembly code)
**Requirement**: CP-14

**Done when**:
- [ ] `gshunt=0.0` (default) → no behavioral change, results identical
- [ ] `gshunt=1e-12` → small shift in operating point (documented, not a bug)
- [ ] `gshunt=1e-3` → noticeable shift
**Tests**: integration — default gshunt → identical results; gshunt>0 → shifted but converges
**Gate**: full
**Commit**: `feat(solver): gshunt circuit-wide diagonal conductance`

---

### T11: Tolerances/Policy split

**What**: Move `max_iter`, `dc_damp_tolerance` off `Context` (claimed immutable, actually mutated) onto `Policy`. `Context` keeps only `Tolerances` (immutable, `Copy`). `Policy` owns the mutable convergence tunables and is passed to the plan/driver. This implements MD-04 (locked decision). Also move `Context.time` to the transient analysis context.
**Where**: `crates/piperine-solver/src/solver/mod.rs`, `solver/dc.rs`, `solver/transient.rs`, `math/newton_raphson.rs`
**Depends on**: T2 (Policy already threaded through), T6 (same files cleaned up)
**Requirement**: CP-17

**Done when**:
- [ ] `Context` has only `Tolerances` (no `max_iter`, `dc_damp_tolerance`, `time`)
- [ ] `Policy` carries `max_iter` + `dc_damp_tolerance`
- [ ] `Context.time` moved to transient analysis context or `Policy`
- [ ] DC/tran results identical
**Tests**: integration — existing tests pass; struct field grep confirms split
**Gate**: full
**Commit**: `refactor(solver): Tolerances/Policy split (MD-04 implemented)`

---

### T12: Newton predictor

**What**: In `newton_raphson.rs:226-231`, when `state.depth() >= 2`, compute the first-order predictor `x̂ = xₙ + (xₙ − xₙ₋₁) · dtₙ₊₁/dtₙ` as the Newton seed instead of just `xₙ`. Fall back to `xₙ` when depth < 2 or after a breakpoint (no valid history).
**Where**: `crates/piperine-solver/src/math/newton_raphson.rs`
**Depends on**: T5 (same function), T3 (stats can measure predictor effectiveness)
**Requirement**: CP-16

**Done when**:
- [ ] Predictor seeds Newton with extrapolated value when history available
- [ ] Falls back to previous point when no history (first step, after breakpoint)
- [ ] Tran results identical (±1e-9)
- [ ] Average Newton iterations per step is ≤ without predictor (measurable via stats)
**Tests**: integration — RC tran, compare iteration counts with/without predictor
**Gate**: full
**Commit**: `feat(solver): first-order Newton predictor in transient`

---

### T13: Dead alpha removal + MD-13 free fn cleanup

**What**: Remove the dead `_alpha` / `_alpha_hint` parameter from all 3 `assemble` impls (`dc.rs:44`, `transient.rs:40`, `ac.rs:38`) and from the call sites. Convert free functions `check_convergence`, `residual_converged`, `apply_damping` (`solver/mod.rs:18-77`) into methods on their owning struct/trait (Tolerances or a convergence-check struct).
**Where**: `crates/piperine-solver/src/solver/mod.rs`, `solver/dc.rs`, `solver/transient.rs`, `solver/ac.rs`, `math/newton_raphson.rs`
**Depends on**: T11 (Tolerances/Policy split — the free fns operate on Tolerances)
**Requirement**: CP-18, CP-19

**Done when**:
- [ ] `grep -rn "_alpha" crates/piperine-solver/src/` finds nothing
- [ ] `grep -rn "pub(crate) fn" crates/piperine-solver/src/solver/mod.rs` finds nothing
- [ ] `check_convergence`/`residual_converged` are methods on Tolerances or a convergence struct
- [ ] DC/tran results identical
**Tests**: integration — grep checks + existing tests pass
**Gate**: full
**Commit**: `refactor(solver): remove dead alpha + MD-13 free fn cleanup`

---

## Phase Execution Map

```
Phase 1:  T1 ──→ T2 ──→ T3 ──→ T4
Phase 2:  T5 ──→ T6
Phase 3:  T7 ──→ T8 ──→ T9 ──→ T10
Phase 4:  T11 ──→ T12 ──→ T13
```

Execution is strictly sequential — one task at a time, in order.

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| T1 SolverStats struct | 1 struct + 2 field additions | ✅ |
| T2 Policy threading | 1 signature + 5 call sites | ✅ |
| T3 Stats accumulation | 2 drivers instrumented | ✅ |
| T4 Python _SolverStats | 1 pyclass + getters | ✅ |
| T5 Zero-alloc loop | 2 methods (reset + hoist) | ✅ |
| T6 Dead code + dt_min warn | 3 deletions + 1 warn | ✅ |
| T7 Device bypass | 1 capability consulted + tracking | ✅ |
| T8 Convergence hint | 1 type + 1 method + wiring | ✅ |
| T9 suggest_transient_step | 1 call site | ✅ |
| T10 gshunt | 1 field + 1 stamp | ✅ |
| T11 Tolerances/Policy split | 3 structs refactored | ✅ |
| T12 Predictor | 1 seed computation | ✅ |
| T13 Alpha + MD-13 | 3 param removals + 3 fn→method | ✅ |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
|------|-------------------|---------------|--------|
| T1 | None | Phase 1 start | ✅ |
| T2 | T1 | T1 → T2 | ✅ |
| T3 | T1, T2 | T2 → T3 | ✅ |
| T4 | T3 | T3 → T4 | ✅ |
| T5 | T2 | Phase 2 ← T4 (via T2) | ✅ |
| T6 | T3, T5 | T5 → T6 | ✅ |
| T7 | T3, T5 | Phase 3 ← T6 (via T5) | ✅ |
| T8 | T7 | T7 → T8 | ✅ |
| T9 | T7 | T8 → T9 | ✅ |
| T10 | T5 | T9 → T10 (T5 satisfied Phase 2) | ✅ |
| T11 | T2, T6 | Phase 4 ← T10 (via T6) | ✅ |
| T12 | T5, T3 | T11 → T12 | ✅ |
| T13 | T11 | T12 → T13 | ✅ |

---

## Test Co-location Validation

| Task | Code Layer | Matrix Requires | Task Says | Status |
|------|-----------|-----------------|-----------|--------|
| T1 | SolverStats struct | unit | unit | ✅ |
| T2 | Newton loop (Policy) | integration | integration | ✅ |
| T3 | Driver instrumentation | integration | integration | ✅ |
| T4 | Python binding | unit (embedded) | unit | ✅ |
| T5 | Newton loop (allocs) | integration | integration | ✅ |
| T6 | Dead code + warn | integration | integration | ✅ |
| T7 | Device bypass | integration | integration | ✅ |
| T8 | Convergence hint | integration | integration | ✅ |
| T9 | suggest_transient_step | integration | integration | ✅ |
| T10 | gshunt | integration | integration | ✅ |
| T11 | Tolerances/Policy split | build + integration | integration | ✅ |
| T12 | Predictor | integration | integration | ✅ |
| T13 | Alpha + MD-13 | build + integration | integration | ✅ |

All co-located; no test deferral. ✅

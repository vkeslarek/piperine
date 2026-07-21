# TR-BDF2 Engine Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/solver-trbdf2-engine/design.md`
**Status**: Draft

---

## Test Coverage Matrix

> Generated from `AGENTS.md` (Test placement table) + spec. Guidelines found: `AGENTS.md` (Hard rules, Test placement), `CLAUDE.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver math (integration formula, LTE) | unit (co-located) | 1:1 to spec ACs (TRB-01/06); formula exactness; phase-coeff edge cases | `crates/piperine-solver/src/math/integration.rs` (`#[cfg(test)]`) | `cargo test -p piperine-solver lib::math` |
| Solver engine (driver, stepper, breakpoints, bypass) | unit + integration | Every AC in groups B/C/D; reject/rollback paths; breakpoint survival | `crates/piperine-solver/tests/*.rs` + co-located | `cargo test -p piperine-solver` |
| Codegen kernel (phase coeffs, breakpoint schedule) | integration | Phase-coeff stamping; source breakpoint emission (TRB-12) | `crates/piperine-codegen/tests/analog_jit.rs` | `cargo test -p piperine-codegen` |
| Discrimination (TRB-20) | integration | Monotonic V(out) vs pw; within 500-step budget; ngspice reltol | `crates/piperine-codegen/tests/analog_jit.rs` | `cargo test -p piperine-codegen` |
| Bench e2e (existing examples) | e2e | Every `examples/*.phdl` stays green; bench transient tests migrated | `crates/piperine-bench/tests/{bench,run_examples}.rs` | `cargo test -p piperine-bench` |

## Gate Check Commands

> Generated from `AGENTS.md` (Build and verify) + repo manifests.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After math/unit-only tasks | `cargo test -p piperine-solver` |
| Codegen | After kernel/codegen tasks | `cargo test -p piperine-codegen` |
| Full | After engine/driver/bench tasks | `cargo test --workspace` |
| Build | After every task (zero warnings is the bar) | `cargo build --workspace` |

---

## Execution Plan

> **Plan revision (2026-07-14, from T5 implementation attempt):** two changes.
> (1) The original T5 (context phase) and T6 (two-phase driver) **merge** — there
> is no coherent single-solve interim (the TR phase advances `γh`, not `h`); the
> kernel switch and the driver go two-phase in one commit. (2) A **new T5a** is
> inserted before the merged driver task: the kernel's reactive companion is
> pure-derivative (BDF style); the TR stage needs a trapezoidal companion that
> tracks the **previous capacitor current** per reactive port. Without T5a the
> TR stage silently degrades to backward-Euler-over-half-step (measured
> `τ_eff ≈ 1.55τ`). See `design.md` § "Design Discovery". T1–T4 are DONE and
> committed (`6fd9ed3` … `7d3cb6c`).

Phases are ordered and run sequentially — each phase completes before the next begins, and tasks within a phase execute in order.

```
Phase 1 (seams)   →  Phase 2 (engine)  →  Phase 3 (breakpoints)
                                   ↓
Phase 4 (reuse+cleanup)  →  Phase 5 (verify)
```

### Phase 1: Math & ABI seams (no behavior change)

```
T1 → T2 → T3 → T4
```

### Phase 2: Two-phase engine + PI controller

```
T5 → T6 → T7
```

### Phase 3: Unified breakpoints

```
T8 → T9 → T10
```

### Phase 4: Factorization reuse + method-selection removal

```
T11 → T12 → T13
```

### Phase 5: Verification & gates

```
T14 → T15 → T16
```

---

## Task Breakdown

### T1: TrBdf2 phase coefficients + Milne LTE

**What**: Add `TrBdf2` (γ = 2−√2), `TrBdf2Phase` enum, `phase_coeffs(phase, h)`, and `milne_lte(x_n, x_nγ, x_n1)` to `math/integration.rs`. Pure math, no driver wiring.
**Where**: `crates/piperine-solver/src/math/integration.rs`
**Depends on**: None
**Reuses**: existing `bdf2_coeffs(dt0, dt1)` for the BDF2 phase (`dt0=(1−γ)h, dt1=γh`)
**Requirement**: TRB-01, TRB-06

**Tools**: NONE
**Done when**:
- [ ] `TrBdf2::GAMMA == 2.0 - sqrt(2)` exact
- [ ] `phase_coeffs(Trap, h) == (2/(γh), −2/(γh), 0)`; `phase_coeffs(Bdf2, h)` matches `bdf2_coeffs((1−γ)h, γh)`
- [ ] `milne_lte` returns 0 for collinear inputs; positive for curvature
- [ ] Co-located unit tests pass; quick gate green
**Tests**: unit (co-located)
**Gate**: quick
**Commit**: `feat(solver): TR-BDF2 phase coefficients + Milne LTE formula`

---

### T2: Element::next_breakpoints ABI; remove BreakpointProvider

**What**: Add `fn next_breakpoints(&self, _from: Second, _horizon: Second) -> &[Second] { &[] }` to `Element` (default empty). Delete the orphan `BreakpointProvider` trait.
**Where**: `crates/piperine-solver/src/core/element.rs`, `crates/piperine-solver/src/math/integration.rs`
**Depends on**: None
**Reuses**: MD-13 rule 2 (the function has an owner)
**Requirement**: TRB-10

**Tools**: NONE
**Done when**:
- [ ] `Element::next_breakpoints` defined with default `&[]`
- [ ] `BreakpointProvider` trait + its doc removed; no remaining references (`grep` clean)
- [ ] Build green (zero warnings)
**Tests**: none (ABI surface; exercised in T8)
**Gate**: build
**Commit**: `feat(solver): Element::next_breakpoints ABI; drop BreakpointProvider trait`

---

### T3: ElementCapabilities::BYPASS_OK flag

**What**: Add the `BYPASS_OK` bit to `ElementCapabilities` with doc. No logic yet (driver consumes it in T12).
**Where**: `crates/piperine-solver/src/core/element.rs`
**Depends on**: None
**Reuses**: existing bitflags pattern
**Requirement**: TRB-17

**Tools**: NONE
**Done when**:
- [ ] `const BYPASS_OK = 1 << 11;` with doc explaining the stamp-reuse contract
- [ ] Build green
**Tests**: none (flag; exercised in T12)
**Gate**: build
**Commit**: `feat(solver): ElementCapabilities::BYPASS_OK flag`

---

### T4: FaerSparseLinearSystem::reset buffer-reuse method

**What**: Add `fn reset(&mut self)` that clears `triplets` and zeroes `b_vec` without reallocating. No call-site change yet (wired in T11).
**Where**: `crates/piperine-solver/src/math/faer.rs`
**Depends on**: None
**Reuses**: existing `FaerSparseLinearSystem` fields
**Requirement**: TRB-16

**Tools**: NONE
**Done when**:
- [ ] `reset()` clears triplets (`Vec::clear`) and zeroes `b_vec` (`fill(E::zero())`)
- [ ] Unit test: `reset` after stamps leaves an empty system; capacity retained
- [ ] Quick gate green
**Tests**: unit (co-located)
**Gate**: quick
**Commit**: `feat(solver): FaerSparseLinearSystem::reset for buffer reuse`

---

### T5a: Kernel trapezoidal companion (previous-current tracking) — NEW PREREQUISITE

**What**: The TR stage of TR-BDF2 needs the trapezoidal companion `i_{C,n+1} = (2C/s)(V_{n+1}−V_n) − i_{C,n}`, which requires the **previous capacitor current** `i_{C,n}` per reactive port. Add a previous-current state bank to the analog device state, populated after each accepted step and applied during the TR phase. The BDF2 phase keeps the existing pure-derivative companion. (Discovered in the T5 attempt — without this, the TR stage silently degrades to BE-over-half-step.)
**Where**: `crates/piperine-codegen/src/device/analog.rs` (state + companion), `crates/piperine-codegen/src/jit/` (state bank), `crates/piperine-solver/src/analysis/transient.rs` (context signals TR phase)
**Depends on**: T1, T2
**Reuses**: the existing charge-history plumbing (`eval_charge`); mirrors how `vold`/limit state banks are threaded
**Requirement**: TRB-01 (the TR phase must actually be 2nd-order trapezoidal)

**Tools**: NONE
**Done when**:
- [ ] A per-reactive-port previous-current value is stored in device state and checkpointed/committed with the step
- [ ] During the TR phase the companion includes `−i_{C,n}`; during BDF2 it does not
- [ ] Integration test: RC discharge (τ=RC), fixed small dt, `V(τ)` within 1% of `e⁻¹` (the case that returned 0.524 before)
- [ ] Full gate green
**Tests**: integration (codegen)
**Gate**: codegen + full
**Commit**: `feat(codegen): trapezoidal companion for the TR-BDF2 TR stage`

---

### T5: TransientAnalysisContext carries phase; kernel calls TrBdf2::phase_coeffs (MERGED into T6)

**What**: Replace `TransientAnalysisContext.{integration, order, dt_prev}` with `{phase: TrBdf2Phase, h: f64}`. The kernel calls `TrBdf2::phase_coeffs(ctx.phase, ctx.h)`. **Merged with T6** — there is no coherent single-solve interim; this lands together with the two-phase driver.
**Where**: `crates/piperine-solver/src/analysis/transient.rs`, `crates/piperine-codegen/src/device/analog.rs`
**Depends on**: T1
**Reuses**: existing `bdf_coeffs` call sites (swap to `TrBdf2::phase_coeffs`)
**Requirement**: TRB-01, TRB-02 (partial)

**Tools**: NONE
**Done when**:
- [ ] `TransientAnalysisContext` has `phase` + `h`, no `integration`/`order`/`dt_prev`
- [ ] Kernel stamps `TrBdf2::phase_coeffs` for both charge and flux companions
- [ ] Driver sets phase=Trap, one solve; existing transient tests still pass (TR single-phase ≈ trapezoidal)
- [ ] Full gate green
**Tests**: integration (codegen)
**Gate**: codegen + full
**Commit**: `refactor(solver,codegen): TransientAnalysisContext carries TR-BDF2 phase`

---

### T6: Two-phase TransientSolver (TR → BDF2 → Milne LTE)

**What**: Rewrite `TransientSolver::solve`'s timestep body to: phase 1 TR solve → `x_{n+γ}`; phase 2 BDF2 solve → `x_{n+1}` (warm-start from `x_{n+γ}`); compute `TrBdf2::milne_lte`. Reject whole step (halve dt, rollback digital, retry from `x_n`) on any phase failure OR LTE > tol.
**Where**: `crates/piperine-solver/src/solver/transient.rs`
**Depends on**: T5
**Reuses**: `NewtonRaphsonSolver::solve_with_strategy` (×2), `DigitalState::{checkpoint,rollback,commit}`
**Requirement**: TRB-03, TRB-04, TRB-05

**Tools**: NONE
**Done when**:
- [ ] Each timestep runs two Newton solves with the intermediate point `x_{n+γ}`
- [ ] Phase-fail OR LTE>tol → reject whole step (TRB-05); `dt ≤ dt_min` still fails → loud error
- [ ] LC-tank L-stability test added: amplitude within 0.5% after one period (TRB-04)
- [ ] Existing transient tests pass (tolerances may need slight relaxation — rewritten, not weakened)
- [ ] Full gate green
**Tests**: integration (solver)
**Gate**: full
**Commit**: `feat(solver): two-phase TR-BDF2 transient step with Milne LTE`

---

### T7: PiController (StepperStrategy); replace LteStepper as primary

**What**: Add `PiController { kp, ki, prev_error }` implementing `StepperStrategy`. Evolve the trait so `propose_dt` receives the global Milne LTE. Wire it as the primary stepper in the driver; per-device `suggest_transient_step` stays as a floor (TRB-08). `reject_dt` halves dt and resets `prev_error`. Remove `LteStepper` as a primary driver (keep the per-device floor helper).
**Where**: `crates/piperine-solver/src/solver/convergence.rs`, `crates/piperine-solver/src/solver/transient.rs`
**Depends on**: T6
**Reuses**: `StepperStrategy` trait seam
**Requirement**: TRB-07, TRB-08, TRB-09

**Tools**: NONE
**Done when**:
- [ ] `PiController::propose_dt` uses `dt·(e/target)^(kp + ki·(e−e_prev)/e)`, clamped `[dt_min, dt_max]`
- [ ] Per-device LTE floor clamps dt down when smaller (TRB-08)
- [ ] `reject_dt` halves + resets history (TRB-09)
- [ ] Unit tests: PI growth monotone on smooth error; floor clamps; reject resets
- [ ] Full gate green
**Tests**: unit (co-located) + integration
**Gate**: full
**Commit**: `feat(solver): PiController timestep policy (replaces reactive LTE stepper)`

---

### T8: BreakpointTable + unified analog/digital landing

**What**: Add `BreakpointTable` (sorted absolute times). Each step, rebuild from `Element::next_breakpoints(t_now, 2·dt_max)` + digital scheduler future event times. `t_next = min(PI dt, next breakpoint, stop_time)`. Dedup with `digital_time_epsilon`. Breakpoints survive rollback (absolute, not checkpointed). This replaces `peek_next_event_time` in the driver.
**Where**: `crates/piperine-solver/src/solver/transient.rs` (or new `solver/breakpoints.rs`)
**Depends on**: T2, T7
**Reuses**: `digital_time_epsilon` (PlanLimits), `DigitalState` event queue
**Requirement**: TRB-11, TRB-14

**Tools**: NONE
**Done when**:
- [ ] `BreakpointTable::next(from)` returns the next landing point
- [ ] `t_next` is the min of PI dt / next breakpoint / stop_time (TRB-11)
- [ ] Breakpoints survive rollback (TRB-14) — test: reject does not lose future breakpoints
- [ ] `peek_next_event_time` no longer called by the transient driver
- [ ] Full gate green
**Tests**: unit + integration
**Gate**: full
**Commit**: `feat(solver): unified BreakpointTable (analog + digital landing points)`

---

### T9: Codegen breakpoint schedule for source models

**What**: JIT-compiled time-varying source models expose their edge/corner times through `Element::next_breakpoints`. Add a kernel `eval_breakpoints(from, horizon, out)` analogous to `eval_charge`, compiled from the source's piecewise structure. Wire `PiperineDevice::next_breakpoints` to call it.
**Where**: `crates/piperine-codegen/src/device/{analog,mod}.rs`, the kernel interface
**Depends on**: T2, T8
**Reuses**: `eval_charge` plumbing as the model
**Requirement**: TRB-12

**Tools**: NONE
**Done when**:
- [ ] A compiled `Pulse`-style source returns its edge times from `next_breakpoints`
- [ ] Integration test: a Pulse-driven RC lands on each declared edge (within epsilon)
- [ ] Codegen gate green
**Tests**: integration (codegen)
**Gate**: codegen + full
**Commit**: `feat(codegen): emit breakpoint schedules for time-varying source models`

---

### T10: Post-breakpoint fixed step (bp_dt) + PI history freeze

**What**: Add `bp_dt` (default `dt_min·100`) to `TransientAnalysisOptions`. When `t_next` is a breakpoint, the step uses fixed `bp_dt` and the PI controller does NOT update its error history (TRB-13). PI resumes from its pre-breakpoint state on the next free step.
**Where**: `crates/piperine-solver/src/analysis/transient.rs`, `crates/piperine-solver/src/solver/transient.rs`
**Depends on**: T8
**Reuses**: `PiController` state
**Requirement**: TRB-13

**Tools**: NONE
**Done when**:
- [ ] `bp_dt` configurable on `TransientAnalysisOptions` (default `dt_min·100`)
- [ ] Breakpoint step uses fixed `bp_dt`; `PiController.prev_error` unchanged across it (TRB-13)
- [ ] Unit/integration test: PI history identical before and after a breakpoint step
- [ ] Full gate green
**Tests**: unit + integration
**Gate**: full
**Commit**: `feat(solver): post-breakpoint fixed step + PI history freeze`

---

### T11: Wire FaerSparseLinearSystem::reset into NewtonRaphsonSolver

**What**: Replace `self.linear_system = L::new(self.symbolic.size())` in `NewtonRaphsonSolver::{solve, solve_with_strategy}` with `self.linear_system.reset()`. Avoids per-iteration reallocation.
**Where**: `crates/piperine-solver/src/math/newton_raphson.rs`
**Depends on**: T4
**Reuses**: `reset()` from T4
**Requirement**: TRB-16

**Tools**: NONE
**Done when**:
- [ ] No `L::new` per Newton iteration; `reset()` called instead
- [ ] All existing tests pass (results bit-identical)
- [ ] Full gate green
**Tests**: integration (solver)
**Gate**: full
**Commit**: `perf(solver): reuse linear-system buffer across Newton iterations`

---

### T12: Device bypass logic (BYPASS_OK) + limiting suppression

**What**: In the driver, track per-element "terminals changed since last eval" (`reltol·|v| + vntol`). When an element has `BYPASS_OK` and terminals unchanged, skip its re-eval/re-stamp (reuse previous). When NO element changed stamps this iteration, reuse the numeric LU (TRB-18). Suppress bypass globally while any element has `limiting_active()` (TRB-19).
**Where**: `crates/piperine-solver/src/solver/transient.rs`, `crates/piperine-solver/src/math/newton_raphson.rs`
**Depends on**: T3, T11
**Reuses**: `ElementCapabilities::BYPASS_OK`, `Element::limiting_active`
**Requirement**: TRB-17, TRB-18, TRB-19

**Tools**: NONE
**Done when**:
- [ ] Bypassed elements skipped on unchanged terminals (TRB-17)
- [ ] Numeric LU reused when no stamps changed (TRB-18)
- [ ] Bypass suppressed while any `limiting_active()` (TRB-19)
- [ ] Test: resistor-heavy circuit, bypass ON ≡ bypass OFF (bit-identical), fewer evals
- [ ] Full gate green
**Tests**: integration (solver)
**Gate**: full
**Commit**: `feat(solver): device bypass + numeric LU reuse`

---

### T13: Remove IntegrationMethod enum + method-selection surface

**What**: Delete `IntegrationMethod`, `Tolerances.integration`, the Gear order-ramp, the Trapezoidal codegen branch, and `bdf_coeffs`. Update `Element::suggest_transient_step` (drop the `method` param). Migrate `math/integration.rs` tests + every caller. TR-BDF2 is now the sole scheme everywhere.
**Where**: `crates/piperine-solver/src/math/integration.rs`, `crates/piperine-solver/src/solver/mod.rs`, `crates/piperine-codegen/src/device/analog.rs`, callers
**Depends on**: T6, T7 (driver/kernel no longer reference the enum)
**Reuses**: compiler enforces completeness (enum gone → every match arm errors)
**Requirement**: TRB-02

**Tools**: NONE
**Done when**:
- [ ] `grep -r "IntegrationMethod\|Trapezoidal\|Gear" crates/` returns only comments/docs (no code)
- [ ] `Tolerances` has no `integration` field
- [ ] All migrated tests pass; no test weakened or deleted
- [ ] Full gate green
**Tests**: unit + integration (migration)
**Gate**: full
**Commit**: `refactor(solver): remove IntegrationMethod — TR-BDF2 is the sole scheme`

---

### T14: Default dt_max = stop/500 (500-step budget)

**What**: Change `TransientAnalysisOptions::{new, new_adaptive}` defaults so `dt_max = stop/500` (the 500-step target budget).
**Where**: `crates/piperine-solver/src/analysis/transient.rs`
**Depends on**: None (config default)
**Reuses**: existing builders
**Requirement**: supports TRB-20/23 (the budget the baseline was measured against)

**Tools**: NONE
**Done when**:
- [ ] `dt_max` defaults to `stop/500.0` in both constructors
- [ ] Existing tests that assumed `stop/100` updated
- [ ] Full gate green
**Tests**: integration
**Gate**: full
**Commit**: `feat(solver): default dt_max = stop/500 (500-step budget)`

---

### T15: Discrimination test — narrow-pulse charge pump (TRB-20)

**What**: The permanent regression gate. A pure-PHDL `Pulse` source drives an RC; sweep pw ∈ {1, 10, 100, 500} ns at per=1 µs, simulate 100 µs under the 500-step budget. Assert V(out) is **monotonic** in pw, every width distinguished, within `reltol` of ngspice. This is the test whose baseline failure is recorded in `design.md`.
**Where**: `crates/piperine-codegen/tests/analog_jit.rs`
**Depends on**: T6, T7, T8, T9, T10, T14 (the full engine)
**Reuses**: the pure-PHDL `Pulse` fixture (100% PHDL, no Rust primitive)
**Requirement**: TRB-20

**Tools**: NONE
**Done when**:
- [ ] Test asserts V(out, pw=1ns) < V(out, pw=10ns) < V(out, pw=100ns) < V(out, pw=500ns) (monotonic)
- [ ] 1ns and 10ns results differ (width distinguished — the baseline had them identical)
- [ ] Step count ≤ ~500·K for the sweep (no budget blowup)
- [ ] Codegen gate green
**Tests**: integration (the discrimination gate itself)
**Gate**: codegen
**Commit**: `test(codegen): TR-BDF2 discrimination — narrow-pulse charge pump (TRB-20)`

---

### T16: ngspice parity sweep + step-count benchmark (TRB-22, TRB-23)

**What**: Run the ngspice cross-validation corpus (`~/Git/plugins/piperine-spice/validation/`) — diode/passives/RC/RL within `reltol`. Compare PI-controller step count vs the recorded baseline on the van der Pol + PWM+RC cases; report `kp`/`ki` ±50% sensitivity.
**Where**: `crates/piperine-solver/tests/` (+ validation harness if present)
**Depends on**: T15
**Reuses**: ngspice validation corpus
**Requirement**: TRB-22, TRB-23

**Tools**: NONE
**Done when**:
- [ ] ngspice corpus circuits match within `reltol` (no regression from the engine swap)
- [ ] PI step count ≤ baseline on van der Pol / PWM+RC (TRB-23)
- [ ] Sensitivity table recorded in `validation.md`
- [ ] Full gate green
**Tests**: integration
**Gate**: full
**Commit**: `test(solver): ngspice parity + PI step-count benchmark`

---

## Phase Execution Map

```
Phase 1:  T1 ──→ T2 ──→ T3 ──→ T4
Phase 2:  T5 ──→ T6 ──→ T7
Phase 3:  T8 ──→ T9 ──→ T10
Phase 4:  T11 ──→ T12 ──→ T13
Phase 5:  T14 ──→ T15 ──→ T16
```

Execution is strictly sequential — no intra-phase parallelism. One agent (or batch worker) per task at a time, in order.

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| T1: TrBdf2 math | 1 module, pure math | ✅ Granular |
| T2: next_breakpoints ABI | 1 trait method + trait deletion | ✅ Granular |
| T3: BYPASS_OK flag | 1 bitflag | ✅ Granular |
| T4: reset() method | 1 method | ✅ Granular |
| T5: context phase + kernel swap | 2 files, cohesive refactor | ✅ Granular |
| T6: two-phase driver | 1 driver rewrite (core change) | ✅ Granular |
| T7: PiController | 1 trait impl + wiring | ✅ Granular |
| T8: BreakpointTable | 1 new struct + driver wiring | ✅ Granular |
| T9: codegen breakpoint schedule | 1 codegen path | ✅ Granular |
| T10: bp_dt + PI freeze | 1 config + driver tweak | ✅ Granular |
| T11: reset() wired | 1 call-site change | ✅ Granular |
| T12: bypass logic | 1 driver feature | ✅ Granular |
| T13: remove IntegrationMethod | 1 invasive deletion (compiler-enforced) | ✅ Granular |
| T14: dt_max default | 1 default change | ✅ Granular |
| T15: discrimination test | 1 test (the gate) | ✅ Granular |
| T16: parity + benchmark | 1 verification task | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
|------|-------------------|---------------|--------|
| T1 | None | Phase 1 start | ✅ |
| T2 | None | Phase 1 | ✅ (no intra-phase dep arrow → may run after T1) |
| T3 | None | Phase 1 | ✅ |
| T4 | None | Phase 1 | ✅ |
| T5 | T1 | Phase 2 start ← Phase 1 | ✅ |
| T6 | T5 | T5 → T6 | ✅ |
| T7 | T6 | T6 → T7 | ✅ |
| T8 | T2, T7 | Phase 3 ← Phase 2 (T7) + Phase 1 (T2) | ✅ |
| T9 | T2, T8 | T8 → T9 | ✅ |
| T10 | T8 | T9 → T10 (T8 via T9) | ✅ |
| T11 | T4 | Phase 4 ← Phase 1 (T4) | ✅ |
| T12 | T3, T11 | T11 → T12 (T3 via T11) | ✅ |
| T13 | T6, T7 | T12 → T13 (T6/T7 satisfied in Phase 2) | ✅ |
| T14 | None | Phase 5 start | ✅ |
| T15 | T6,T7,T8,T9,T10,T14 | T14 → T15 (all earlier deps satisfied) | ✅ |
| T16 | T15 | T15 → T16 | ✅ |

> Note: T2/T3/T4 have no intra-phase dependencies — they run in order after T1
> but each stands alone. The diagram shows Phase 1 as a linear chain for
> simplicity; the bodies' "Depends on: None" reflects that ordering is
> conventional, not a hard gate.

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
|------|----------------------------|-----------------|-----------|--------|
| T1 | solver math | unit (co-located) | unit | ✅ |
| T2 | solver ABI (Element trait) | none (exercised T8) | none | ✅ |
| T3 | solver ABI (bitflag) | none (exercised T12) | none | ✅ |
| T4 | solver math | unit (co-located) | unit | ✅ |
| T5 | solver analysis + codegen kernel | integration | codegen+full | ✅ |
| T6 | solver engine | integration | full | ✅ |
| T7 | solver engine | unit + integration | full | ✅ |
| T8 | solver engine | unit + integration | full | ✅ |
| T9 | codegen kernel | integration | codegen+full | ✅ |
| T10 | solver analysis + engine | unit + integration | full | ✅ |
| T11 | solver math | integration | full | ✅ |
| T12 | solver engine | integration | full | ✅ |
| T13 | solver + codegen (deletion) | unit + integration | full | ✅ |
| T14 | solver analysis | integration | full | ✅ |
| T15 | discrimination gate | integration | codegen | ✅ |
| T16 | verification | integration | full | ✅ |

All co-located; no test deferral. ✅

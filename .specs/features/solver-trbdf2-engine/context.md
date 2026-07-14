# TR-BDF2 Engine Context

**Gathered:** 2026-07-14 (round 2)
**Spec:** `.specs/features/solver-trbdf2-engine/spec.md`
**Status:** Ready for design

---

## Feature Boundary

A TR-BDF2 (γ = 2−√2) two-phase transient integration engine with a stateful
PI timestep controller, a **unified** breakpoint table (analog source
edges + digital switching times are one event kind), and factorization
reuse (buffer reset + device bypass). TR-BDF2 is the **sole** integration
scheme — the `IntegrationMethod` enum and method-selection surface are
removed. No MVP slicing: ships as one coherent body. Supersedes the
standalone `solver-breakpoints` stub (its ACs fold in here).

---

## Implementation Decisions

### Phase-failure policy (TRB-04)

- **Decision:** On any phase failure (TR non-convergence, BDF2
  non-convergence, or post-step Milne LTE exceeding tolerance), reject the
  WHOLE step: discard the intermediate point `x_{n+γ}`, halve `dt`, retry
  both phases from `x_n`.
- **Rationale:** Clean state machine, no partial-failure bookkeeping.
  "Zero backtracking" in the ROADMAP means no restoration of matrix state
  (which we never do — the symbolic LU is immutable and the numeric LU is
  rebuilt each iteration anyway), not a prohibition on discarding a
  candidate timestep. This matches the existing reject path
  (`LteStepper::reject_dt`) and keeps the driver single-pass.

### PI controller inputs (TRB-07, TRB-08)

- **Decision:** The `PiController` is driven by the **global** Milne LTE
  from the TR-BDF2 step, but per-device LTE suggestions
  (`suggest_transient_step`) still act as a **floor** — `dt` is clamped
  down to the smallest per-device suggestion if it exceeds one.
- **Rationale:** PI controls primary growth/smoothing; per-device LTE keeps
  guarding against a single reactive element the global estimate
  under-weights. The `StepperStrategy` trait evolves to receive the
  optional global LTE; method-dependent (Gear/Trapezoidal keep per-device
  min only, TR-BDF2 layers PI on top).

### Factorization reuse scope (TRB-14, TRB-15, TRB-16)

- **Decision:** Buffer reuse (reset `triplets`/`b_vec` instead of
  reallocating) + symbolic LU reuse (already done). Plus a new
  `ElementCapabilities::BYPASS_OK` flag: when an element opts in and its
  terminal voltages are unchanged within `reltol·|v| + vntol` since its
  last evaluation, the solver skips re-evaluating and re-stamping it; if
  every changed-element set is empty for an iteration, the numeric LU is
  reused (no refactor).
- **Excluded:** Selective/incremental sub-block refactor — high risk, low
  TR-BDF2 coupling, deferred.
- **Rationale:** faer has no numeric-LU *update* API; reuse is only valid
  when A is unchanged. Device bypass is the principled way to make A
  unchanged. This is the realistic ceiling of reuse without a different
  linear backend.

### Breakpoint × PI controller interaction (TRB-11)

- **Decision:** Breakpoint steps use a **fixed** small `dt`
  (user-configurable `bp_dt`, default `dt_min · 100`); the PI controller
  does **not** update its error history for breakpoint steps — it resumes
  PI control from its pre-breakpoint state on the next free step.
- **Rationale:** Artificially-short breakpoint steps would pollute the PI's
  error memory and cause post-breakpoint oscillation. Fixed-dt breakpoints
  isolate the discontinuity; PI owns the smooth regions between. Simpler
  than resetting and matches the "PI controls growth, breakpoints own
  edges" split.

### Default integration method (TRB-02) — REVISED

- **Decision:** TR-BDF2 is the **sole** integration scheme. The
  `IntegrationMethod` enum, the `Tolerances.integration` field, the Gear
  order-ramp logic, the Trapezoidal codegen branch, and the
  `LteStepper`-as-primary-driver path are all **removed**. There is no
  method selection and no fallback — TR-BDF2 is the only scheme because it
  is the only one needed.
- **Rationale (user, round 2):** "não vamos ter fallback nem seleção de
  métodos, esse vai ser o único e default." Keeping a dead enum around a
  single-method world violates MD-13 (clean and simple — no vestigial
  surface). The centralised formula stays in `math/integration.rs` (MD-07)
  but as a `TrBdf2` phase-coefficient function, not an enum match.
- **Migration cost accepted:** every test that selects `Trapezoidal` /
  `Gear` is rewritten against TR-BDF2 outcomes; the LC-tank L-stability
  check is added as a positive gate. None deleted, none weakened.

### Element contract owns breakpoints (TRB-10) — NEW

- **Decision:** The `Element` trait gains `fn next_breakpoints(&self, from:
  Second, horizon: Second) -> &[Second]` (default empty). The standalone
  `BreakpointProvider` trait is removed. Every element (analog source,
  digital, future comparator) declares its own landing points.
- **Rationale (user, round 2):** "precisa modificar o contrato do Element
  para prover os próximos breakpoints dentro de X tempo." MD-13 rule 2 —
  the function has an owner. A separate trait for a per-element concern is
  the missing abstraction.

### Breakpoints unify analog + digital (TRB-11) — NEW

- **Decision:** One `BreakpointTable` merges (a) every element's
  `next_breakpoints` and (b) the digital scheduler's future event times.
  `t_next` is the min of (PI target, next table entry, `stop_time`). The
  `DigitalState::peek_next_event_time` "get" path is replaced by an
  "advance until" contract: the solver drives to the next table entry, and
  the digital scheduler runs there. Digital edges ARE breakpoints.
- **Rationale (user, round 2):** " Esses breakpoints também correspondem
  às viradas digitais ... o que pode substituir o get de eventos por apply
  de until." This is a partial realisation of MD-12 (unified event model).
- **Open (design):** whether the digital queue is poured into the table
  each step, or the table queries the scheduler — a structural choice for
  design.md.

### Codegen emits breakpoint schedules (TRB-12) — NEW

- **Decision:** Codegen compiles a breakpoint schedule for time-varying
  analog source models (pulse / PWL / sine corners and edges) exposed
  through the kernel (analogous to `eval_charge`). Digital events already
  carry future times via `sink.emit(net, value, delay)` and need no new
  codegen — only routing into the table.
- **Rationale (user, round 2):** "Vamos precisar modificar o codegen para
  suportar me dar os breakpoints, inclusive o digital."

### No MVP slicing — NEW

- **Decision:** The engine ships as one coherent body. There is no P1/P2
  phasing of the deliverable; the spec's ACs are grouped by subsystem
  (A–E), not by priority. Atomic commits per task still apply during
  execution (one task → one commit → gate green), but no partial engine
  ships.
- **Rationale (user, round 2):** "Não vamos fazer um MVP, vamos fazer ele
  inteiro."

### Discrimination test design (TRB-20) — NEW

- **Decision:** A deliberately very-stiff, hard-to-converge circuit is the
  integration gate. It MUST be demonstrated to FAIL on the current
  architecture FIRST (baseline recorded — timestep-too-small at `dt ≤
  dt_min`, or Newton non-convergence), then converge within `reltol` of
  ngspice on the new architecture. The concrete circuit is picked in
  design.md (candidates: PWM-driven buck/RC with sharp edges; high-μ van
  der Pol; diode-chain reverse recovery). The baseline-failure run is
  recorded before implementation; the passing run is a permanent
  regression gate.
- **Rationale (user, round 2):** "faça um circuito BEEEM stiff, de difícil
  convergência, valide que ele não converge na arquitetura atual e depois
  implemente e valide novamente." This is the spec-anchored outcome +
  discrimination sensor baked into the test plan.

---

## Agent's Discretion

- PI controller gain defaults (`kp`/`ki`) — ngspice-lineage defaults
  chosen (`0.7`/`0.4`); tunable via `Tolerances` without rebuild. Open to
  tuning during the step-count benchmark (TRB-23).
- The exact field name and placement for the global LTE hand-off into
  `StepperStrategy::propose_dt` (trait evolution is design's job).
- Whether `bp_dt` lives on `TransientAnalysisOptions` or
  `TransientContext` (MD-03 placement — design decides).
- The concrete stiff discrimination circuit (design.md picks + proves the
  baseline failure).
- Whether the digital queue is pushed into the BreakpointTable each step
  or the table pulls from the scheduler (design.md).
- Whether `IntegrationMethod` becomes a private `TrBdf2` struct or is
  removed outright (design.md — the enum is gone either way).

## Declined / Undiscussed Gray Areas → Assumptions

- **AC small-signal impact:** assumed unaffected (AC linearizes at the DC
  op with `1/jω`, independent of transient method). Logged as TRB-15 guard
  test in the spec.
- **PI gain tuning surface:** assumed `kp`/`ki` ride on `Tolerances` with
  ngspice-derived defaults; sensitivity folded into TRB-23.
- **Comparator/crossing event sources:** the `next_breakpoints` ABI seam
  lands here; a comparator model that emits crossings through it is a
  follow-up (out of scope).

---

## Specific References

- **Reference simulator:** ngspice-46, cross-validation corpus at
  `~/Git/plugins/piperine-spice/validation/`.
- **Math source:** Hosea & Shampine (1996) for the TR-BDF2 γ = 2−√2
  formulation and Milne-device LTE.
- **ngspice analogs:** `NIconvTest` (residual — already landed),
  `tdmax`/`hmin` PI lineage, breakpoint forcing at source discontinuities.
- **Codebase seams (round 2):**
  - `math/integration.rs` — `IntegrationMethod` enum **removed**; the
    centralised TR-BDF2 phase-coefficient formula stays (MD-07).
  - `solver/mod.rs::Tolerances` — `.integration` field **removed**.
  - `solver/convergence.rs::StepperStrategy` — gains `PiController` impl
    (MD-05); `LteStepper` removed as primary driver (per-device LTE stays
    as floor).
  - `core/element.rs::Element` — gains `next_breakpoints(from, horizon)`;
    `BreakpointProvider` trait removed.
  - `digital/scheduler.rs::DigitalState::peek_next_event_time` — replaced
    by the unified `BreakpointTable` "advance until" contract.
  - `math/faer.rs::FaerSparseLinearSystem` — gains `reset()` for buffer
    reuse; `core/element.rs::ElementCapabilities` gains `BYPASS_OK`.
  - `codegen/device/analog.rs::bdf_coeffs` — replaced by the TR-BDF2
    phase call; codegen gains a breakpoint-schedule emission for sources.

---

## Deferred Ideas

- **Comparator / analog-crossing event sources:** the `next_breakpoints`
  ABI seam lands here; a comparator model that emits crossings is a
  follow-up.
- **Enforced UIC hold (`.ic` + `uic` clamp branch):** separate SOLVER_GAPS
  §3 item; not engine-coupled.
- **Full unified event model (MD-12):** this epic realises the
  breakpoint+digital unification; analog crossing events and timer events
  join later.
- **KLU-style symbolic+numeric refactor with partial update:** would need
  a non-faer sparse backend; future backend swap.

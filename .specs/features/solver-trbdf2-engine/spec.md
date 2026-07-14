# TR-BDF2 Transient Engine — sole integration scheme, PI control, unified breakpoints, factorization reuse

**Implements:** MD-05 (strategy composition), MD-07 (centralised integration
formula), MD-08 (LTE drives timestep), MD-12 (unified event model — partial),
MD-13 (Rust idiom rules).
**ROADMAP reference:** "Epic: TR-BDF2 Transient Integration Engine with PI
Timestep Controller".
**SOLVER_GAPS reference:** §3 (Breakpoints — MISSING), §5 (Device bypass /
Matrix reuse — MISSING).
**Supersedes:** `solver-breakpoints` (4 ACs fold into the breakpoint group
here) and the multi-method `IntegrationMethod` selection surface.

## Problem Statement

The transient engine has three coupled weaknesses. (1) The reactive LTE
stepper shrinks `dt` sharply on error then grows it back 2× — the classic
SPICE "timestep too small" failure on stiff non-linear and switched
circuits. (2) There is no breakpoint forcing: the integrator steps across
source discontinuities and digital edges, diverging or losing accuracy at
every kink. (3) Each timestep rebuilds the linear system from scratch
(reallocation every Newton iteration) and re-evaluates every device even
when its terminals barely moved — a cost that bites harder once the engine
runs a structured two-phase step.

This epic replaces the whole transient integration core with **TR-BDF2**
(Trapezoidal Rule / Backward Differentiation Formula 2, γ = 2−√2) driven
by a **Proportional-Integral timestep controller**, fed by a **unified
breakpoint table** (analog source edges **and** digital switching times are
the same kind of event), and backed by **factorization reuse** (buffer
reset + device bypass). TR-BDF2 becomes the **sole** integration scheme —
the `IntegrationMethod` enum and the method-selection surface are removed.
There is no MVP slicing: the engine ships as one coherent body.

## Goals

- [ ] **TR-BDF2 as the sole integration scheme.** The `IntegrationMethod`
      enum, `Tolerances.integration`, the Gear order-ramp, and the
      Trapezoidal codegen branch are removed. One centralised phase
      formula lives in `math/integration.rs` (MD-07).
- [ ] **Two-phase step** with intermediate point `x_{n+γ}`; L-stable
      (BDF2 stage is a native low-pass filter — no false numerical ringing
      on fast nodes).
- [ ] **Global Milne LTE** feeding a stateful **PI controller**
      (`StepperStrategy` impl) that smooths `dt`; per-device LTE stays as a
      floor.
- [ ] **Unified breakpoint table.** `Element::next_breakpoints(from,
      horizon)` replaces the separate `BreakpointProvider` trait; analog
      source edges and digital switching times feed one table the stepper
      lands on. The "get next digital event" (`peek_next_event_time`) is
      replaced by an "advance until" contract driven by the table.
- [ ] **Codegen exposes breakpoints** for time-varying analog source
      models (pulse / PWL / sine); digital events already carry their
      times and route into the table.
- [ ] **Factorization reuse:** linear-system buffer reset (no
      reallocation) + `ElementCapabilities::BYPASS_OK` (skip re-evaluation
      and re-stamping when an element's terminals are unchanged → numeric
      LU reusable that iteration).
- [ ] **Discrimination test:** a deliberately very-stiff, hard-to-converge
      circuit that **fails on the current architecture** (timestep-too-small
      or non-convergence) and **converges on the new one** — recorded as a
      baseline failure first, then a permanent regression gate.
- [ ] All existing transient tests continue to pass (migrated to TR-BDF2
      outcomes; none weakened or deleted).

## Out of Scope

| Feature | Reason |
|---------|--------|
| Method selection / Trapezoidal / Gear / fallback | Removed by design — TR-BDF2 is the sole scheme. |
| `.pz` / `.disto` / `.sens` / `.sp` analyses | Niche; not transient integration. |
| gmin / source stepping in transient | Homotopy is DC-only (MD-04); transient uses dt rejection. |
| Enforced UIC hold (`.ic` + `uic` clamp branch) | Separate SOLVER_GAPS §3 item. |
| MOS1 / JFET model-equation bugs | Model-side (SOLVER_GAPS §6), not the engine. |
| Selective/incremental sub-block matrix refactor | High risk, low coupling; deferred (would need a non-faer backend). |
| Higher-order / variable-order TR-BDF2 | TR-BDF2 is fixed two-stage by definition. |
| Full analog-crossing event sources (comparators as breakpoint emitters) | The ABI seam (`next_breakpoints`) lands here; a comparator model using it is a follow-up. |

---

## Assumptions & Open Questions

Every ambiguity is resolved or recorded here — nothing is left silently unclear.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| AC analysis impact | Unchanged — AC linearizes at the DC op with `1/jω`, independent of the transient scheme. | `load_ac` does not consume the transient companion. | y (TRB-15 guard) |
| Verification corpus | van der Pol (stiff), ideal LC tank (amplitude/L-stability), PWM + RC / buck (switched), `~/Git/plugins/piperine-spice/validation/` (ngspice parity). | ROADMAP deliverables; ngspice is the reference. | y |
| γ value | γ = 2 − √2 (Hosea & Shampine). | Standard TR-BDF2; equal-weight stages. | y |
| PI gains | `kp = 0.7`, `ki = 0.4` (ngspice lineage), overridable via `Tolerances`. | Tunable without rebuild. | y (TRB-23 sensitivity) |
| Bypass tolerance | `reltol·|v| + vntol` for "terminals unchanged" — same vocabulary as convergence. | No new knob. | y |
| Milne LTE normalization | `LTE / (trtol·chgtol + reltol·|Q|)` (ngspice charge form). | Consistent with existing `suggest_transient_step` scale (MD-08). | y |
| Breakpoint horizon | The stepper queries `next_breakpoints(t_now, dt_max · 2)` each step; the table is rebuilt incrementally, not globally. | Bounds per-step cost; absolute times survive rollback. | y |
| `bp_dt` (post-breakpoint small step) | Default `dt_min · 100`, configurable on `TransientAnalysisOptions`. | Small enough to resolve the edge, not so small it stalls. | y |
| Digital events as breakpoints | The digital scheduler's future event times are poured into the unified table each step; the scheduler still runs at `t_next`, but `t_next` is table-driven. | Unifies analog + digital landing points (MD-12 partial). | y |

**Open questions:** none — all resolved or logged above (required before the spec is confirmed).

---

## Acceptance Criteria

Grouped by subsystem (no priority slicing — the engine ships whole).

### A. TR-BDF2 integration core (sole scheme)

1. **TRB-01.** WHEN the TR phase is assembled over sub-step `γh` THEN the
   centralised formula SHALL return companion coefficients
   `(2/(γh), −2/(γh), 0)`; WHEN the BDF2 phase is assembled over sub-step
   `(1−γ)h` with previous sub-step `γh` THEN it SHALL return the
   non-uniform BDF2 coefficients `(c0,c1,c2)` for `dQ/dt`. γ = 2 − √2.
2. **TRB-02.** WHEN `Tolerances::default()` is constructed THEN there
   SHALL be no `integration` field; the `IntegrationMethod` enum, the Gear
   order-ramp, and the Trapezoidal codegen branch SHALL be removed. The
   kernel calls ONE centralised phase-coefficient function.
3. **TRB-03.** WHEN a transient step runs THEN the driver SHALL execute
   two Newton solves: phase 1 (TR) → intermediate point `x_{n+γ}`; phase 2
   (BDF2) → `x_{n+1}` from `x_{n+γ}` and `x_n`.
4. **TRB-04.** WHEN an ideal lossless LC tank is simulated over one
   period with TR-BDF2 THEN the amplitude after one period SHALL be within
   0.5% of the initial amplitude (L-stability proof; Trapezoidal rings).
5. **TRB-05.** WHEN either phase fails to converge OR the post-step
   Milne LTE exceeds tolerance THEN the driver SHALL reject the WHOLE step
   (discard `x_{n+γ}`), halve `dt`, and retry both phases from `x_n`.

### B. Milne LTE + PI controller

6. **TRB-06.** WHEN a step is accepted THEN a global Local Truncation
   Error SHALL be computed via Milne's device (predictor extrapolation of
   `x_n` and `x_{n+γ}` differenced from `x_{n+1}`), normalized by
   `(trtol·chgtol + reltol·|Q|)`.
7. **TRB-07.** WHEN a step is accepted THEN the next `dt` SHALL be
   proposed by a `PiController` implementing `StepperStrategy`:
   `dt_{n+1} = dt_n · (e_n/target)^(kp + ki·(e_n−e_{n−1})/e_n)`, clamped
   to `[dt_min, dt_max]`.
8. **TRB-08.** WHEN any element's per-device LTE suggestion
   (`suggest_transient_step`) is smaller than the PI-proposed `dt` THEN
   `dt` SHALL be clamped down to that floor (per-device LTE still guards).
9. **TRB-09.** WHEN a step is rejected THEN `reject_dt` SHALL halve `dt`
   AND the PI controller's error history SHALL be reset (no memory of the
   failed error).

### C. Unified breakpoints (analog + digital)

10. **TRB-10.** WHEN the `Element` trait is examined THEN it SHALL carry
    `fn next_breakpoints(&self, from: Second, horizon: Second) ->
    &[Second]` (default empty) owning this element's required landing
    points. The standalone `BreakpointProvider` trait SHALL be removed
    (MD-13 rule 2 — the function has an owner).
11. **TRB-11.** WHEN the stepper builds its next-step target THEN it
    SHALL consult a unified `BreakpointTable` fed by (a) every element's
    `next_breakpoints` and (b) the digital scheduler's future event times;
    `t_next` SHALL be the minimum of (PI-proposed target, next table
    entry, `stop_time`). The `peek_next_event_time` "get" path SHALL be
    replaced by this table-driven "advance until" contract.
12. **TRB-12.** WHEN a time-varying analog source model (pulse / PWL /
    sine) is compiled THEN codegen SHALL emit its edge/corner times
    through `next_breakpoints` (the kernel exposes a breakpoint schedule
    analogous to `eval_charge`).
13. **TRB-13.** WHEN a step lands on a breakpoint THEN that step SHALL
    use the fixed `bp_dt` (default `dt_min · 100`); the PI controller
    SHALL NOT update its error history for that step and SHALL resume from
    its pre-breakpoint state on the next free step.
14. **TRB-14.** WHEN a step is rejected and rolled back THEN every
    breakpoint strictly greater than `current_time` SHALL remain in the
    table (breakpoints are absolute times, not state).
15. **TRB-15.** WHEN an AC analysis runs THEN its results SHALL be
    unchanged by this epic (guard test — AC does not consume the transient
    companion).

### D. Factorization reuse

16. **TRB-16.** WHEN the Newton solver assembles a new iteration THEN
    `FaerSparseLinearSystem` SHALL reuse its `triplets`/`b_vec` allocations
    (reset-and-refill); the symbolic LU (`SymbolicLu`) SHALL continue to be
    reused across the whole run.
17. **TRB-17.** WHEN an element declares `ElementCapabilities::BYPASS_OK`
    and its terminal voltages are unchanged within `reltol·|v| + vntol`
    since its last evaluation THEN the solver SHALL skip re-evaluating and
    re-stamping it for that iteration.
18. **TRB-18.** WHEN the set of elements whose stamps changed this
    iteration is empty THEN the solver SHALL reuse the previous numeric LU
    factorization (no refactor).
19. **TRB-19.** WHEN any element reports `limiting_active()` THEN bypass
    SHALL be suppressed for every element that iteration (limiting devices
    must re-evaluate every iteration until limiting clears).

### E. Discrimination test + migration + parity

20. **TRB-20 (discrimination).** WHEN a deliberately very-stiff switched
    circuit (design picks the concrete instance — e.g. PWM-driven buck/RC
    with sharp edges, or high-μ van der Pol) is simulated on the CURRENT
    architecture THEN it SHALL fail (timestep-too-small at `dt ≤ dt_min`,
    or Newton non-convergence); WHEN the same circuit is simulated on the
    NEW architecture THEN it SHALL converge and the recorded waveform
    SHALL match ngspice within `reltol`. The baseline failure SHALL be
    recorded before implementation begins; the passing case is a permanent
    regression gate.
21. **TRB-21.** WHEN `cargo test --workspace` runs THEN every target SHALL
    pass. Existing transient tests that asserted Gear-2 / Trapezoidal
    outcomes SHALL be migrated to TR-BDF2 outcomes (rewritten, not
    weakened or deleted); the LC-tank L-stability check (TRB-04) is added
    as a positive gate.
22. **TRB-22.** WHEN the ngspice cross-validation corpus
    (`~/Git/plugins/piperine-spice/validation/`) is run THEN diode /
    passives / RC / RL circuits SHALL match ngspice within `reltol` (no
    regression from the engine swap).
23. **TRB-23.** WHEN the PI controller is compared against the old
    `LteStepper` on the van der Pol and PWM+RC cases THEN the PI
    controller SHALL use ≤ the number of accepted steps (smooth `dt`
    growth, not thrashing). PI gain sensitivity (`kp`/`ki` ±50%) SHALL be
    reported.

---

## Edge Cases

- WHEN `dt` is reduced to `dt_min` and a step still fails THEN the driver
  SHALL return the underlying solver error (no silent stall, MD-09).
- WHEN no element provides breakpoints and the digital queue is empty
  THEN the engine SHALL behave exactly as the no-breakpoint path (PI
  controls `dt` freely).
- WHEN a breakpoint coincides with a digital event time THEN both SHALL
  be honoured at the same `t_next` (single landing point).
- WHEN an element opts into `BYPASS_OK` but is actively converging
  nonlinearly THEN bypass SHALL be suppressed while `limiting_active()`
  is true on any element (TRB-19).
- WHEN the first transient step runs (no history) THEN phase 1 (TR) SHALL
  seed from the DC operating point; the BDF2 phase SHALL use `x_n` as the
  second history point (self-starting — no Gear-style order ramp needed).
- WHEN a digital event is scheduled mid-step (delta cycle during the
  analog solve) THEN it SHALL land at the current `t_next`, not force an
  unscheduled breakpoint (existing scheduler semantics preserved).

---

## Requirement Traceability

| Requirement ID | Subsystem | Status |
| -------------- | --------- | ------ |
| TRB-01 | A — TR-BDF2 phase coefficients | Pending |
| TRB-02 | A — sole scheme, enum removed | Pending |
| TRB-03 | A — two-phase step | Pending |
| TRB-04 | A — LC L-stability | Pending |
| TRB-05 | A — phase-fail policy | Pending |
| TRB-06 | B — Milne global LTE | Pending |
| TRB-07 | B — PiController | Pending |
| TRB-08 | B — per-device LTE floor | Pending |
| TRB-09 | B — reject resets PI history | Pending |
| TRB-10 | C — Element::next_breakpoints | Pending |
| TRB-11 | C — unified BreakpointTable | Pending |
| TRB-12 | C — codegen breakpoint schedule | Pending |
| TRB-13 | C — fixed bp_dt, PI resume | Pending |
| TRB-14 | C — breakpoints survive rollback | Pending |
| TRB-15 | C — AC unaffected guard | Pending |
| TRB-16 | D — buffer reuse | Pending |
| TRB-17 | D — BYPASS_OK flag | Pending |
| TRB-18 | D — numeric LU reuse | Pending |
| TRB-19 | D — bypass × limiting | Pending |
| TRB-20 | E — stiff discrimination test | Pending |
| TRB-21 | E — existing tests migrated | Pending |
| TRB-22 | E — ngspice parity | Pending |
| TRB-23 | E — step-count benchmark | Pending |

**Coverage:** 23 total, 0 mapped to tasks ⚠️ (Tasks phase produces the mapping).

**ID format:** `TRB-NN`.

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

---

## Success Criteria

- [ ] **Discrimination (TRB-20):** the stiff circuit fails on current main
      (recorded), converges on the new engine within `reltol` of ngspice.
- [ ] **L-stability (TRB-04):** LC tank amplitude within 0.5% after one
      period.
- [ ] **Edges land exactly (TRB-11/12):** PWM + RC records a timepoint at
      every pulse edge within `digital_time_epsilon`.
- [ ] **No "timestep too small":** van der Pol (μ=1000) completes; PI
      uses ≤ steps vs the old stepper (TRB-23).
- [ ] **Reuse pays off (TRB-17):** a resistor-heavy circuit runs with
      >20% fewer device evaluations at identical results.
- [ ] **Suite green (TRB-21):** `cargo build --workspace` zero warnings;
      `cargo test --workspace` all targets pass.
- [ ] **Parity holds (TRB-22):** ngspice corpus within `reltol`.

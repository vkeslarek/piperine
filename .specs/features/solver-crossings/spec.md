# Solver Crossings Specification

**Approved**: 2026-11-15 - user: "Pode ir rodando os grandes passos" (standing delegation)

## Problem Statement

Two solver defects make the boundary between analog and digital observable at the wrong value or the wrong time. (1) The digital engine applies the commits of one microstep to a net one at a time: `x <- 1; x <- 2;` from a net that held 2 produces two `Change` observations, and `x.changed()` fires although the settled value did not change (found by `ir-rust` T42; docs/spec Part III §8.2.3, Part IV §12.1.1, §12.1.2, §12.1.18). (2) A `CROSS` watch fires at the first accepted solution after its sign change instead of at the located crossing: an RC charge trips a 0.625 V comparator at 0.6352 V, and a 1 kHz sine against 0.5 V fires 0.97 µs late (found by `ir-rust` T45; Part V §17.3, abi.md §4.3 "located in the step"). (3) A reaction may read the environment (Part IV §12.1.12, §14.4: `temperature()`, `gmin()`, `source_scale()`), but the ABI's validator refuses a reaction that declares the ambient inputs `GMIN` or `SOURCE_SCALE`, and the digital engine supplies neither to a reaction or a discrete function (user, via the coordinator: "temperature e gmin precisa estar wired..."). Both defects are the scope of `solver-crossings` in `.specs/features/solver-completeness/spec.md` (SCX-01, SCX-02, SCX-03 AC 3 and 7), plus the digital fix.

## Goals

- [ ] Within one microstep, commits to one net settle to one value: one write, one `Change` observation, `CHANGED` against the value before the microstep.
- [ ] A transient lands on a crossing: the landing tick exceeds the true crossing by at most `ttol` plus one tick, or `0 < e < etol` holds there, on a ramp, a sine and an RC charge.
- [ ] `E0510` and `E0509` fail the analysis on an invalid tolerance or step cap.
- [ ] A reaction and a discrete function read `temperature()` (the `GLOBAL TEMPERATURE` pin), `gmin()` and `source_scale()` (the ambient inputs) with the values the host set and the continuation values in force.
- [ ] `parity_baseline` and every parity test pass unchanged.

## Out of Scope

| Feature | Reason |
| ----------- | -------------- |
| A `Dc` sweep driver in the solver, with discrete state carried across points and inits at the first point only | SCT-10, feature `solver-time-point`; this feature proves the crossing comparison across two operating points of one circuit |
| Locating a crossing in the step that reads a jump | §17.3 rule 8 forbids it: such a crossing fires at the step's end, as today |
| Coalescing two firings of one event net in one microstep | §12.1.2 and §12.1.18 speak of levels of a value; an event net has none |
| A `bound_step` step cap reaching the next step (SCX-03 AC 6) | done by `solver-abi-analog` and `solver-abi-integration` (`a_cap_watch_bounds_the_next_step`) |
| Guarded crossings (§17.3 rule 6) and unknown expressions (rule 5) | the guard and the unknown flag are codegen's: a watch reaches the solver as a plain `F64` |
| Retuning a tolerance or the step grid of a design with no `CROSS` watch | ngspice parity is the project's top priority |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The default `ttol` (Part VIII §23 lists it as a session option whose default lives in Appendix C.2) | `TransientAnalysisOptions::ttol`, 1 ps; Appendix C.2.1 gains the row | the spec's examples use `1p` (Part IV §12, Part I §2); delegated by the standing approval | y - delegated |
| The default `etol` of a watch whose `etol` is `DEFAULT` | the session's potential `abstol` (`Tolerances::vntol`, 1 µV) | the ABI carries no nature for a watch; §17.3 rule 2 names the nature's `abstol`, and Appendix C.2.1's potential `abstol` is the default nature's | y - delegated |
| What "known within `ttol`" measures | the landing minus the solver's estimate of the crossing time: the secant of the bracket (the last accepted solution and the attempt), or the secant of the last two attempts of the same step when both are past the crossing and it falls inside the bracket | Appendix C.2: "secant prediction from the bracketing pair, bisection when it stalls, retake to the predicted time"; the two-point secant is what keeps a curved `e` from stalling one-sided | y - delegated |
| How a retake lands | at the estimate rounded up to a tick (§15 T4), never on or past the attempt's own tick; when the estimate rounds to the attempt's tick the attempt is accepted | §15 T4; the protocol's "landed earlier" path already lands on a tick | y - settled by the spec |
| A retake whose `e` is still `≤ 0` | it is accepted as an ordinary step and the next step brackets the crossing from there | §17.3 rule 3 fires only where `e > 0`; the accepted point narrows the bracket | y - delegated |
| The step grid of a design with a `CROSS` watch | every transient step ends exactly on its tick; a design with no `CROSS` watch keeps `device_attempt`'s SPEC_DEVIATION (a step ends on the controller's own time, within a tick) | AC 2 asks for a tick; the deviation exists only for the parity oracles, none of which has a `CROSS` watch | y - delegated |
| A "sweep point" at the solver level | a later operating point (`dc()` then `solve()`) of the same circuit, after a host write; the `CROSS` comparison carries from the last solution of one to the first of the next | the solver has no sweep driver (SCT-10); `piperine-api`'s `Sweep` runs one operating point per point on the held circuit | y - delegated |
| The value of `GMIN` and `SOURCE_SCALE` a reaction or a discrete function reads | the session `gmin` (`Tolerances::gmin`) and 1.0: an instant always runs after the last solve of its sequence, where the continuation variables are at rest (abi.md §4.2 "at rest in the last solve of every sequence") | the user's request: "the continuation values in force at that instant, i.e. their final values outside a homotopy" | y - user (via the coordinator) |
| How `temperature()` reaches a reaction | it stays the `GLOBAL TEMPERATURE` pin it already is (abi.md §4.2: "a value that must exist at instants ... is a `GLOBAL` pin"); the new rule is proven on it, not re-plumbed as an ambient | the ABI already gives temperature that form; an ambient `TEMPERATURE` would duplicate it | y - delegated |
| The digital fix's writer name for a non-canonical net (P3) | the writer of the last commit to that net in the microstep | the last commit is the one whose bytes the net holds | y - delegated |
| The `CAP` watch error | `E0509` for a value that is not finite or is `≤ 0`, replacing the generic A4 "not finite" message for `CAP` | §17.2 resolution: "A `dt` that is not finite and positive is error `E0509`" | y - settled by the spec |

**Open questions:** none - all resolved or logged above.

---

## User Stories

### P1: One settled value per microstep ⭐ MVP

**User Story**: As a codegen author, I want the commits of one microstep to apply together, so that `changed()` reports the settled value and a later statement simply wins.

**Why P1**: a reaction with two commits to one level is ordinary MSL (§12.1.2); today it fires `changed()` downstream when nothing changed.

**Acceptance Criteria**:

1. WHEN two or more commits to one discrete net apply in the same microstep THEN the engine SHALL write the net once, with each commit's bytes laid over the value before the microstep in C2 order (the last commit to a byte wins).
2. WHEN the settled bytes of such a net equal its bytes before the microstep THEN the engine SHALL report no `Change` observation and `CHANGED` SHALL read false for that microstep.
3. WHEN the settled bytes differ from the bytes before the microstep THEN the engine SHALL report exactly one `Change` observation carrying the settled bytes, and `PREVIOUS` SHALL read the bytes before the microstep.
4. WHEN a reaction commits `x <- HIGH; x <- LOW;` to a net holding `LOW` on the solver THEN a downstream function reading `x.CHANGED` SHALL not fire its event in that microstep.

**Independent Test**: two host commits to one net at one tick, then the pure-ABI repro of T42 on the solver: `x.changed()` fires once, at the initial value.

---

### P2: Located crossings

**User Story**: As a codegen author, I want the solver to land on a crossing, so that `cross`, `above` and every converter built on them fire at the right time.

**Why P2**: `cross` is the only analog-to-digital path of MSL and fires one step late today.

**Acceptance Criteria**:

1. WHEN a `CROSS` watch goes from `≤ 0` at the last accepted solution to `> 0` at a converged attempt, and the step reads no jump, THEN the solver SHALL retake the step until the landing exceeds the true crossing time by at most `ttol` plus one tick, or `0 < e < etol` holds there. (SCX-01)
2. WHEN a crossing is located THEN its landing SHALL be a tick, the accepted solution SHALL be at that tick's time, and its `Crossing` observation and the commit to its event pin SHALL be at that time. (SCX-01)
3. WHEN several `CROSS` watches go positive in one step THEN the solver SHALL land on the earliest, and those whose crossings round to the same tick SHALL fire together at it. (SCX-01)
4. WHERE a watch names no `ttol` or `etol` pin the solver SHALL use `TransientAnalysisOptions::ttol` (1 ps by default) and the session's potential `abstol` (`vntol`, 1 µV by default). (SCX-01)
5. WHEN a step reads a jump (the step after a delivered boundary) THEN the solver SHALL not locate a crossing in it: the watch fires at the step's end. (SCX-01)
6. WHEN the next round of an operating point moves a `CROSS` watch from `≤ 0` to `> 0` THEN its event SHALL fire in that round. (SCX-02)
7. WHEN a later operating point of the same circuit moves a `CROSS` watch from `≤ 0` at the last solution of the earlier one to `> 0` THEN its event SHALL fire at that later point. (SCX-02)
8. IF a `CROSS` watch's `ttol` or `etol` pin holds a value that is not finite or is `≤ 0` THEN the solver SHALL fail the analysis with an error naming `E0510`, the instance and the watch. (SCX-03 AC 3)
9. IF a `CAP` watch evaluates to a value that is not finite or is `≤ 0` THEN the solver SHALL fail the analysis with an error naming `E0509`, the instance and the watch. (SCX-03 AC 7)
10. The solver SHALL leave every result of a design with no `CROSS` watch unchanged: `parity_baseline` and the parity tests pass unedited.

**Independent Test**: a ramp through 0.5 V lands within `ttol` plus one tick for several step sizes; a 1 kHz sine against 0.5 V and an RC charge against 0.625 V fire within `etol` or `ttol` plus one tick of the analytic crossing.

---

### P3: The environment in reactions and discrete functions

**User Story**: As a codegen author, I want a reaction and a discrete function to read `temperature()`, `gmin()` and `source_scale()`, so that §12.1.12 lowers without a workaround.

**Why P3**: the language allows it today (§12.1.12, §14.4) and nothing in the ABI carries it to a reaction.

**Acceptance Criteria**:

1. The ABI's shape validator SHALL accept a reaction or a discrete function that reads the ambient input `GMIN` or `SOURCE_SCALE`, and SHALL keep refusing `FREQ` in a reaction and `TIME` in a discrete function. (ENV-01)
2. WHEN a reaction or a discrete function reads `GMIN` or `SOURCE_SCALE` THEN the digital engine SHALL supply the continuation values its owner set, `0` and `1` until one is set. (ENV-02)
3. WHEN a reaction or a discrete function of a circuit runs on the solver THEN `GMIN` SHALL read the session's `gmin` and `SOURCE_SCALE` 1.0, in the operating point and in the transient. (ENV-03)
4. WHEN a reaction reads the `GLOBAL TEMPERATURE` pin THEN it SHALL read the analysis temperature the host set, in kelvin. (ENV-03)
5. The solver SHALL leave every result of a design whose reactions read no ambient input unchanged. (ENV-03)

**Independent Test**: a pure-ABI device whose reaction commits `temperature()`, `gmin()` and `source_scale()` to three carriers reads back the host's temperature, the session's `gmin` and 1.0.

---

## Edge Cases

- WHEN the estimate rounds up to the attempt's own tick THEN the solver SHALL accept the attempt and fire there (it is already within one tick).
- WHEN the crossing lies exactly on a tick and the landing there reads `e = 0` THEN the step SHALL be accepted without firing, and the next step SHALL fire one tick later at most.
- IF the retakes of one step exceed the protocol's bound on consecutive rejections THEN the transient SHALL fail naming the time, as for any rejection (no silent acceptance).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| DSV-01 | P1: Settled value, AC 1 to 4 | T1, T2 | Implementing |
| SCX-01 | P2: Crossings, AC 1 to 5 (location) | T5, T6 | Implementing |
| SCX-02 | P2: Crossings, AC 6, 7 (across rounds and operating points) | T7 | Implementing |
| SCX-03 | P2: Crossings, AC 8, 9 (`E0510`, `E0509`) | T3, T4, T7 | Implementing |
| SCX-04 | P2: Crossings, AC 10 (parity unchanged) | T6 | Implementing |
| ENV-01 | P3: Environment, AC 1 (the validator) | Tasks | Pending |
| ENV-02 | P3: Environment, AC 2 (the digital engine) | Tasks | Pending |
| ENV-03 | P3: Environment, AC 3 to 5 (the solver) | Tasks | Pending |

**Coverage:** 8 total, 8 mapped to tasks, 0 unmapped.

---

## Sources

- `docs/spec/part_iii_structure.md` §8.2.3 ("Only the settled value is observable: `changed()` compares it with the value before the microstep"); `part_iv_behavior.md` §12.1.1, §12.1.2, §12.1.18, §13 rule 5.
- `docs/spec/part_v_execution.md` §15 T4, §16.1 rule 2, §17.2 (A4, rule 5, `E0509`), §17.3 (rules 1 to 4 and 8, `E0510`); Appendix C.2 and C.2.1; Part VIII §23 session options.
- `.specs/features/solver-completeness/spec.md`: P2 "Located crossings and `bound_step`", the `solver-crossings` row, SCX-01 to SCX-03.
- `.specs/features/device-abi/abi.md` §3 (`Watch { event, ttol, etol }`), §4.3 (watch table), §11.5 (comparator).
- `docs/spec/part_iv_behavior.md` §12.1.12, §14.4 (`temperature()`, `gmin()`, `source_scale()`); abi.md §4.2 (ambient inputs, `GLOBAL` pins).
- Repros: `piperine-ir-rust/.specs/features/ir-rust/parked/{t42_repro_changed,t42_commits,t45_repro_cross,t45_boundary}.rs.txt`.

---

## Success Criteria

- [ ] The sine repro fires with `e < 1 µV` or within 1 ps plus one tick of `asin(0.5)/(2π·1 kHz)`; today it fires 0.97 µs late.
- [ ] The RC repro trips within `etol` of 0.625 V; today at 0.6352 V.
- [ ] `parity_baseline` reports 161 steps and its pinned values, unedited.

# Solver ABI Analog Specification

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

One of four features that make the solver adopt the device ABI (`.specs/features/device-abi/abi.md`, the authority): `solver-abi-core`, then `solver-abi-digital` and `solver-abi-analog` (this one) in parallel, then `solver-abi-integration`. This feature edits `crates/piperine-analog-solver` only, so it runs in its own worktree beside `solver-abi-digital`.

## Problem Statement

Every device the solver runs today returns MNA stamps, builds its own TR-BDF2 companion and keeps its own limiter state. The device ABI moves that into the solver: a device declares terms, inputs and watches and returns values and partials. The code that turns those into rows does not exist. This feature builds it inside `piperine-analog-solver` as a new module, `device`, that produces the same `Stamp`s the Newton engine consumes today. It is standalone and unit-tested; `solver-abi-integration` then points the drivers at it.

The numbers must not move. The Newton iteration, TR-BDF2, the convergence plan and the tolerance mapping are not edited (user: "a paridade com ngspice é um strenght bem alto"); the solver-owned companion reproduces today's hand-written capacitor stamping expression for expression.

## Goals

- [ ] `piperine-analog-solver` builds every row from terms and partials, and owns the `ddt` companion, the delay history, the limit iterate, noise and stimulus injection and the watches.
- [ ] Analytic circuits (a divider, an RC step, a diode) solve to their known values through the unchanged Newton engine with stamps from the new module.
- [ ] No file outside `crates/piperine-analog-solver` changes, the crate's 24 existing unit tests pass unchanged, and the module offers what an `AnalogSide` adapter needs.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Any edit to `piperine-solver` (bind, drivers, the protocol, ports of tests, removal of `Element`) | `solver-abi-integration` |
| Any edit to the numeric core (`math/*`, `convergence.rs`, `tolerances.rs`, `config.rs`) | user, Q7 and Q9: the present algorithm and mapping stay |
| Applying a `CAP` to the step, turning a fired `CROSS` into a commit, delivering traces to a sink | the owner's protocol; here the assembly reports them |
| Locating a crossing inside a step, `E0509`, `E0510` | `solver-crossings` |
| Truncation per `ddt` state (`Term.abstol`), per-row residual tolerances, named singular solves | `solver-numerics`; the truncation test stays Milne over node potentials |
| Tolerance-based stamp bypass | the ABI replaces it with F3; it was opt-in and off by default |
| Discrete functions, reactions, commits | `solver-abi-digital` (parallel) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Numerics | Newton, TR-BDF2, Milne, the PI controller, the convergence plan and the tolerance mapping are not edited; the solver-owned companion reproduces today's hand-written capacitor stamping expression for expression | user: "eu manteria o algoritmo atual do solver que funcionou bem nos testes"; "a paridade com ngspice é um strenght bem alto" | y |
| What "unchanged" means | every numeric assertion and tolerance of a surviving test passes as written, and `solver-abi-integration` holds `Device` and `Element` circuits within 1e-12 relative with equal step counts in a differential test. Here the stamps are asserted against hand-computed values and analytic circuits. Bit identity is not promised: `g·vp − g·vn` and `g·(vp − vn)` differ in the last bit | the caller's brief: "numeric assertions/tolerances unchanged" | y |
| Limiting | the ABI form (abi.md A2): the function reports values at the held iterate `x̂`, the solver adds `g·p` to the Jacobian and `g·(x − x̂)` to the right side, and vetoes convergence while `abs(x̂ − x) > 1e-6 + 1e-4·abs(x)`. Today's "apply the limited value to the guess" (`apply_limiting_reports`) is removed with `LimitingReport` | abi.md §4.3, A2, A3; Appendix C.2 "limiting"; no surviving numeric baseline uses a limiter (the JIT devices are gone) | y - settled by the ABI |
| Noise | one `NOISE` input injected on several terms is one fully correlated source (abi.md A2). This meets umbrella SCN-05 by construction | abi.md §10 "Noise" | y - settled by the ABI |
| Sparsity | the pattern is the declared terms × partials, whatever the values. This meets umbrella SCN-04 by construction | abi.md §9 "build the sparsity pattern from terms × partials" | y - settled by the ABI |
| Who models, who verifies | Sonnet implements and Opus verifies by default; the orchestrator may have Opus implement (a different Opus verifies). This feature is numerically delicate and not large (10 tasks): the case the user named for Opus as implementer | user, 2026-11-15, through the coordinator | y |
| Parallel work | the feature touches `crates/piperine-analog-solver` only; its boundary guard is a test of this crate | user, 2026-11-15: "runnable in parallel in separate worktrees (neither touches the other's crate)" | y |
| What the crate depends on | `piperine-abi` only | MD-61 (proposed by `solver-abi-core`) | n - follows MD-61 |
| Discrete values the analog side reads | the assembly keeps its own byte copy per slot, written by `set_discrete`; it names no net and no digital type | abi.md §10: "its own copy of the discrete pins its functions read, updated only by boundary events" | y - settled by the ABI |

**Open questions:** none blocking.

**Implicit-requirement dimensions.** Input validation & bounds: SAN-02 (bind errors), SAN-05 (`max`). Failure / partial failure: SAN-07. Idempotency / retry: SAN-03 (restore then repeat gives the same stamps). Auth and rate limits: N/A because it is an in-process library. Concurrency / ordering: N/A because one solve runs at a time and functions are pure. Data lifecycle: SAN-03, SAN-05 (histories). Observability: SAN-10 (`TRACE`, `VAR`). External-dependency failure: SAN-07 and a device fault returned as an error naming the instance. State-transition integrity: SAN-04 (`AGE` and `LAST`).

---

## User Stories

### P1: Rows from terms and partials ⭐ MVP

**User Story**: As a codegen author, I want to declare terms, inputs and watches and return values and partials, so that the solver owns stamping, the companion model, limiting and history.

**Why P1**: it is the analog half of the ABI.

**Acceptance Criteria**:

1. The crate `piperine-analog-solver` SHALL depend on `piperine-abi` and on no other workspace crate, and SHALL name no digital type.
2. WHEN a `FLOW` term between `plus` and `minus` has value `v` and partials `g_j` against continuous pins THEN the assembly SHALL add `g_j` to the Jacobian at (`plus`, pin `j`), subtract it at (`minus`, pin `j`), and add `−(v − Σ g_j·x_j)` to the right side of `plus` and its negative to `minus`.
3. WHILE an equilibrium solve runs, a `DDT` term SHALL contribute nothing.
4. WHEN a transient stage runs THEN a `DDT` term of value `q` SHALL contribute the companion current that `TrBdf2::stage_coeffs` gives over the term's own accepted history of `q`, with the previous-derivative correction of the trapezoidal stage, and its partials scaled by the leading coefficient.
5. WHEN a step is rejected THEN the history of every `DDT` and `DELAY` term and the `LAST` of every limit SHALL equal their values before the step.
6. WHEN a function reads the `VALUE` of a `LIMIT` input THEN it SHALL read the iterate held for this Newton iteration, and the assembly SHALL add `g·p` to the Jacobian and `g·(x − x̂)` to the right side, with `g` the partials against the limit and `p` the partials of `PROPOSED`.
7. WHILE any limit has `abs(x̂ − x) > 1e-6 + 1e-4·abs(x)`, the solve SHALL NOT be reported as converged.
8. WHERE a function sets `OWNS_CORRECTION`, the assembly SHALL apply no correction for the limits it writes and SHALL still apply the veto of criterion 7.
9. WHEN a transient stage runs THEN a `DELAY` term SHALL contribute its value at `t − td` interpolated from its accepted history, and its value itself in an equilibrium solve.
10. IF a `DELAY` term's `td` exceeds its declared `max` THEN the solve SHALL fail naming the instance and the term.
11. WHILE no input of an analog function has changed since its outputs were computed, the assembly SHALL NOT call it and SHALL reuse its outputs.
12. IF a value or a seeded partial that the current solve reads is not finite THEN the solve SHALL fail naming the instance, the cell and its `origin`.
13. The matrix pattern SHALL be the set of positions the declared terms and partials can reach, the same in every solve.
14. WHEN a small-signal sweep runs at angular frequency `ω` THEN each term SHALL contribute its partials multiplied by 1 for `FLOW`, `jω` for `DDT` and `exp(−jω·td)` for `DELAY`, and each `STIM` input SHALL contribute `−g·(RE + j·IM)` to the right side.
15. WHEN a noise sweep runs THEN each `NOISE` input SHALL be injected as one source through its partials on every term that reads it, with density `PSD`, and the output density SHALL be the sum over inputs of `|H·b|²·PSD`.
16. WHEN a solution is accepted THEN every `VAR` watch SHALL be readable by instance path and name.
17. WHEN the value of a `CROSS` watch goes from `≤ 0` to `> 0` between two accepted solutions THEN the assembly SHALL report its event pin as fired at the later one.
18. WHEN a solution is accepted THEN the assembly SHALL report the smallest value of the `CAP` watches evaluated there.
19. WHEN a function writes a `TRACE` watch during an evaluation THEN the record SHALL be kept with the Newton iteration number and SHALL survive a restore.
20. WHEN a function reads an ambient input THEN it SHALL read: `TIME` the time of the solve; `FREQ` the sweep frequency or 0; `STATIC` 1 in an equilibrium solve; `GMIN` and `SOURCE_SCALE` the continuation values, at rest (the session `gmin`, 1.0) in the last solve of a sequence.
21. WHEN a discrete slot is set THEN every function that reads it SHALL be evaluated again in the next solve.
22. The module SHALL offer, as plain methods, what an adapter of the owner's `AnalogSide` needs: set a discrete slot, assemble a stage, end an iteration, accept, snapshot, restore, read the samples, drain the traces.

**Independent Test**: a resistor divider, an RC step and a diode, written as `Device`s and assembled by the analog crate alone, give the analytic divider ratio, `1 − exp(−t/RC)` and the diode's own equation.

---

## Edge Cases

- WHEN a term names `GROUND` as `plus` or `minus` THEN the assembly SHALL stamp only the other row.
- WHEN an instance has no analog function THEN the assembly SHALL hold nothing for it.
- WHEN a limit is not evaluated in an iteration THEN it SHALL be answered `LIMITED = PROPOSED = LAST` (abi.md A3).
- IF a device returns `FAULT` from an analog function THEN the solve SHALL fail with the device's code and message and the instance path.
- WHEN the transient step size is not positive THEN a `DDT` term SHALL contribute nothing.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SAN-01 | P1: Rows, AC 1 (crate boundary) | Tasks | In Tasks |
| SAN-02 | P1: Rows, AC 2, 13 (`FLOW` rows, pattern) | Tasks | In Tasks |
| SAN-03 | P1: Rows, AC 3, 4, 5 (`DDT` companion, histories) | Tasks | In Tasks |
| SAN-04 | P1: Rows, AC 6, 7, 8 (limiting) | Tasks | In Tasks |
| SAN-05 | P1: Rows, AC 9, 10 (`DELAY`) | Tasks | In Tasks |
| SAN-06 | P1: Rows, AC 11 (run when an input changed) | Tasks | In Tasks |
| SAN-07 | P1: Rows, AC 12 (non-finite) | Tasks | In Tasks |
| SAN-08 | P1: Rows, AC 14 (small-signal, `STIM`) | Tasks | In Tasks |
| SAN-09 | P1: Rows, AC 15 (noise) | Tasks | In Tasks |
| SAN-10 | P1: Rows, AC 16, 17, 18, 19 (watches) | Tasks | In Tasks |
| SAN-11 | P1: Rows, AC 20 (ambient inputs) | Tasks | In Tasks |
| SAN-12 | P1: Rows, AC 21, 22 and Goals (discrete slots, the facade, nothing outside the crate) | Tasks | In Tasks |

**Coverage:** 12 total, 12 mapped to tasks, 0 unmapped. Umbrella requirements met here by construction: SCN-04 (the pattern is the declared union), SCN-05 (one noise input is one correlated source).

---

## Sources

- `.specs/features/device-abi/abi.md`: §3 (shape), §4 (functions, cells, inputs, A1 to A5), §8 (verifier), §10 (call sequences), §11.1 to §11.3, §11.6 (examples).
- `.specs/features/solver-completeness/context.md`: Q7, Q9, rounds 1 to 3.
- `.specs/features/solver-abi-core/design.md`: `caller::Handle`, `validate`, and the `AnalogSide` trait of component `protocol`.
- `crates/piperine-analog-solver/src/math/integration.rs`; `crates/piperine-solver/tests/helpers/devices.rs` (`C::load_transient`, `D::norton`, `branch_force`): the stamps to reproduce.
- `docs/spec/appendix_c_implementation.md` C.2 (limiting activity).
- The coordinator's message of 2026-11-15 (user): parallel worktrees, model roles.
- `.specs/STATE.md` MD-28, MD-31, MD-60, MD-61.

---

## Success Criteria

- [ ] `cargo test -p piperine-analog-solver` is green with zero warnings: the 24 old unit tests and the new targets.
- [ ] The `DDT` stamps equal, for both TR-BDF2 phases, the values computed from `stage_coeffs` and `phase_coeffs` as the hand-written capacitor computes them.
- [ ] `git diff --stat` of the feature shows `crates/piperine-analog-solver` and `.specs/` only.

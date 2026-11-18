# Solver Hygiene Specification

**Approved**: 2026-11-17 - user: "Topo" (lanes) and "Aceito todas (Recommended)" (Q0-Q7)

The approval and the answers reached the planner through the orchestrator on 2026-11-17; they are recorded in `context.md`.

Lane C of the 2026-11-17 plan (user): the "Solver hygiene" cluster of `GAPS.md` (V1 to V11, V14,
V15 and §4.8). Goal: numerics closer to ngspice, and failures that are loud and named.

Every number quoted below was measured on 2026-11-17 at `4293de6b` with ngspice-47
(`/usr/bin/ngspice`), the interpreter backend, and the probes kept under the session scratchpad
`plan-c/` (not committed).

## Questions answered (user, 2026-11-17)

The user's answer to all eight: "Aceito todas (Recommended)". Each question is decided as option (a), the recommendation below. The golden and pin changes listed under "Results" are authorized only with the before/after deviation table vs ngspice in the commit and the unchanged tolerance contract. No tolerance moves.

| # | Question | Options | Decided: (a) |
| - | -------- | ------- | ------------ |
| Q0 | Scope: a silent defect found while planning. A `ddt` whose argument is nonlinear in a `limit`ed value is wrong from the first transient step: a diode with diffusion charge `ddt(tt * id)` jumps from its operating point 0.629 V to -32.47 V at the first step, on both backends; the same charge written on the unlimited `v(a, k)` stays at 0.629 V, and a linear `ddt(cj0 * vd)` through the same `limit` is right. It is not in `GAPS.md`. | (a) fix it here as HYG-05, P1: the nonlinear-charge twin of V1 cannot pass without it; (b) record it in `GAPS.md` §3 as a silent entry and leave it | (a). Results: none of the 7 twins places a nonlinear charge |
| Q1 | V1, the basis of the truncation-error test | (a) the `ddt` states, as ngspice `CKTterr`: each `DDT` term's Milne difference against `max(h·(abstol + reltol·max(|i_n|, |i_n+1|)), reltol·max(|q_n|, |q_n+1|, chgtol))`; node potentials are no longer measured; `chgtol` is read; the PI target (1) and the gate (`trtol` 7) do not change. (b) states and node potentials both (today's §17.2 Resolution). (c) node potentials only, as today; `chgtol` is deleted and the spec says so | (a). It is ngspice's basis and the one §15.2 rule 1 names as the state. Results: the grid of every circuit with a `ddt`; `tran_rc` runs at the harness's 10 ns cap with no rejection (1 000 accepted steps, probe), so `goldens/tran_rc.txt` is not expected to move but may; `parity_baseline.rs` `parity_rc_transient` (161 steps, `v_last`) is re-pinned with converged-grid evidence |
| Q2 | V2, the Milne skip of a component whose two differences are asymmetric (`math/integration.rs:148`) | (a) remove it: ngspice has none; landings and the step after a jump are already exempt by the protocol. (b) keep it. (c) keep it only for the first step after the operating point | (a). The skip also hides the start of a fast rise from flat. Results: `goldens/tran_rc.txt` may move; `parity_baseline.rs` `parity_rc_transient` re-pinned with Q1 |
| Q3 | V3, a step that still fails the truncation test at `dt_min` | (a) fail with `E0508` naming the time, as ngspice `dctran.c:896-910` ("Timestep too small": the first time the step reaches the floor it retries there, the second time it fails); Appendix C.2.2 item 4 and the `E0508` row are amended. (b) keep accepting, but report it: a warning observation at the sink and the count in the result. (c) an option, default (b) | (a). Results: none of the 7 twins touches the floor (probe: 0 floor hits); a buck target that touches it would fail: T2 measures this before any change, and a hit stops the task |
| Q4 | V4, Newton damping | (a) as ngspice: no damping by default (`CKTnodeDamping` is 0 in `cktinit.c:91`); an opt-in `NodeDamping` strategy with ngspice's rule (operating-point solves only, voltage unknowns only, while a `limit` is active, when the largest step exceeds 10 V: factor `max(10 / maxΔv, 0.1)`, `niiter.c:297-323`); the drivers use the plan's `NewtonStrategy`, which they ignore today. (b) per-unknown damping (no reference implementation to be faithful to). (c) keep the midpoint damping on the L2 norm of the whole update (volts and amperes mixed), documented | (a). Probe with damping off: `diode_iv` and `diode_rev` converge at every point, worst deviation 0.3685 to 0.3686 of the `DC_CURRENT` bound and 0.0000 unchanged, Newton iterations 154 to 134 and 149 to 87; `tran_rc` identical. Results: DC values move inside `reltol`; `goldens/{divider,isrc_r,diode_series,diode_iv,diode_rev,ac_rc}.txt` may change in the last printed digit; `parity_baseline.rs` DC pins (`parity_diode_dc_point`, `parity_divider_op_dc`) re-pinned |
| Q5 | V11, the first step after a digital-to-analog jump (F-B7) | (a) ngspice's breakpoint rule (`dctran.c:487-524`): the step after a `Restart::Integrator` is at most `0.1·min(last accepted step, time to the next scheduled time point)`, first order, still exempt. (b) (a), and the step is tested for truncation (once Q1(a) measures states, which a jump carries unless a constraint forces them, §15.2 rule 2). (c) keep today's controller proposal | (a). Results: no twin has a discrete part; the buck takes a few more steps after each analog-read edge; `boundary.rs` passes at the harness's 10 ns cap instead of 1 ns |
| Q6 | V14, a transient with a non-zero start and no captured step (fails loud today) | (a) it starts from an operating point at instant `(t_start, 0)`: the digital engine starts at `tick(t_start)`, `now()` reads `t_start`. (b) ngspice `tstart`: the run integrates from 0 and keeps from the start, `with_start` alone becomes `record_from`. (c) keep the refusal, give it a spec sentence, close V14 as a limit | (a). It is what `with_start` claims and the shooting of PSS (lane B) can start from it. Results: `transient_reentry.rs` `a_late_start_needs_a_step_a_transient_recorded` changes its expectation |
| Q7 | Two existing assertions name what the user's scope calls defects | (a) rewrite them: `failure_rules.rs` asserts `E0507` instead of `LRM §7.3.2.1` (§4.8 names this citation), and `boundary.rs` runs at the 10 ns cap instead of 1 ns (a stricter test, F-B7). (b) leave them and add new tests beside them | (a) |

## Problem Statement

The solver passes its 7 ngspice twins, but it departs from ngspice in four places a twin does not
see (truncation basis, step floor, damping, restart after a jump), keeps two dead hooks and a stale
document, and fails without a name. A singular solve prints `SpaceMatrix: The library threw an
error while trying to create the RHS of the sparse matrix`; a floating node prints `Newton: Linear
solver returned NaN/Inf`. Part V wants `E0503` naming the branches or the node. Re-entry from a
captured step is off the continuous run by up to 7.3e-4 relative (N1), which a periodic
steady-state shooting method (lane B) cannot use: its map must be a function of the captured state.

## Goals

- [ ] Every run failure of Part V that this cluster covers carries its spec code (`E0503`, `E0504`, `E0505`, `E0507`, `E0508`) and names what caused it.
- [ ] Re-entry from a captured step continues the run bit for bit.
- [ ] Each numerics change is closer to ngspice or to the spec's stated outcome, proven by a twin under the unchanged `ToleranceContract` or by a test of the spec's outcome, with a before/after deviation table in its commit.
- [ ] The 7 twins and every parity target stay green; only the goldens named in Q1, Q2 and Q4 may change, and only to values closer to ngspice.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| V12 multi-rate stepping | User, 2026-11-17: a future improvement, much later |
| V13 averaged or surrogate models | User, 2026-11-17: a future improvement, much later |
| V16 periodic small-signal and periodic noise | A missing analysis on top of PSS, not hygiene; PSS and analyses exposure are lane B (`host-ii`) |
| Exposing PSS, noise, TF, PZ, SP, sensitivity to a host | Lane B (`host-ii`) |
| `P0001` saying "outside the M1 subset" (§4.8, third bullet) | Front end, lane A (`msl-fixes`) |
| The other spec codes no source mentions (`E0102`, `E0105`, `E0803` to `E0806`) | Front end and host (lanes A and B) |
| Naming an unknown by its source signal (`Top.x`) instead of the pins on it | The solver does not hold source names; a host may map them later (lane B) |
| Changing any `ToleranceContract`, `contract.rs`, or a twin's `.cir` | Hard rule of the lane |
| A `dt_min` default of ngspice's `1e-11·dt_max` | The tick is 1 fs (C.2.1): ngspice's floor would lie under one tick for every run with `dt_max` under 100 µs |

---

## Assumptions & Open Questions

Technical decisions the user delegated ("for each item decide (technical) whether to fix, and how
to prove it").

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Exactness achievable for re-entry | Bit-identical: a run re-entered from any step it recorded, with the same options, gives the same accepted times and the same values (`f64` bits) as the continuous run from there on | Every state the continuation reads can be captured: the Newton history, the step-size controller's memory, `dt`, `prev_h`, the pending restart, the digital checkpoint, the assembly. A continuation with another stop time is not the continuous run: §16.4 makes the old stop a time point | y (delegated) |
| What re-entry keeps per step (V9) | A new option `Reentry::{EveryStep, LastStep}`, default `EveryStep` | The default does not move (no result changes); the host already keeps one step (MD-66). `GAPS.md` V9 sizes it "S (an opt-out)" | y (delegated) |
| How a run error is carried | A typed `RunError { code, domain, detail, time, unknowns, names }` in the analog sub-solver's `Error`; the owner fills `names` from the bind before the error leaves `piperine-solver` | A host can read the code without parsing text; the sub-solver knows indices, the owner knows instance and pin names | y (delegated) |
| How an unknown is named | By the pins bound to it: `node of Top.r1.n, Top.c1.p`; a flow unknown by its instance and branch: `flow of Top.v1 (b0)` | The solver holds instance paths and pin names, not source signal names | y (delegated) |
| How a singular solve is diagnosed | On a failed or non-finite linear solve with finite stamps, a dense rank-revealing factorization of the Jacobian at that point gives the null vector; the unknowns whose component exceeds 1e-6 of the largest are named. Above 2 000 unknowns the report says the diagnosis was skipped and why | Only on failure, so the cost is paid once; 2 000 unknowns is under a second dense | y (delegated) |
| The nonlinear-charge twin's golden | `tests/ngspice/tran_dq.cir` runs ngspice with `.options interp reltol=1e-4 trtol=1`, stated in the file | At default options ngspice's own error on this circuit is 1.7× `TRAN_VOLTAGE` against an ngspice run at `reltol=1e-7` (probe); with these options it is 0.57×. Piperine at the harness's cap is 0.12× from the same reference. A golden less accurate than the contract cannot judge parity | y (delegated) |
| The diode's `tt` | `tests/ngspice/devices.msl` `Diode` gains `tt: f64 = 0.0` (ngspice's name and default) and `ddt(tt * id)`; the 6 other twins' goldens must stay byte-identical, else a separate `DiodeTt` module is used | ngspice's diode model has both; one model per device | y (delegated) |
| Spec edits | Part V and Appendix C sentences change with the decided behaviour, each with an assertion in `crates/piperine-lang/tests/spec_text.rs` (MD-57) | The spec leads; the guard is the project's | y (delegated) |
| The digital checkpoint (V7) | A journal of first writes since a mark, in the store and the queue; `checkpoint()` (a full copy) stays for re-entry | The look-ahead marks and restores every instant; a full copy per mark is O(nets) | y (delegated) |

**Open questions:** none - Q0 to Q7 answered by the user on 2026-11-17 ("Aceito todas (Recommended)"), all option (a).

---

## User Stories

### P1: Loud, named run failures ⭐ MVP

**User Story**: As a circuit author, I want a run that cannot solve to say which spec rule failed and which node, branch or instance caused it, so that I can fix the circuit without reading the solver.

**Why P1**: §4.8 of `GAPS.md`: today the message names a matrix library, not the circuit.

**Acceptance Criteria**:

1. HYG-01: WHEN a solve of any kind (operating point, DC point, transient step, AC point) meets a singular Jacobian THEN the solver SHALL fail with `E0503` and name the unknowns of the singular part: a loop of two potential sources `v1` and `v2` names both flows, a node reached only through capacitors names that node by its pins.
2. HYG-01: IF the linear solve returns a non-finite value while every stamp is finite THEN the solver SHALL report `E0503`, never `Linear solver returned NaN/Inf`.
3. HYG-02: IF a device stamps a non-finite value THEN the solver SHALL fail with `E0507` naming the instance and the contribution or unknown, and the message SHALL NOT cite `LRM §7.3.2.1`.
4. HYG-03: IF an equilibrium solve does not converge after Newton and every homotopy THEN the solver SHALL fail with `E0505` naming the unknowns that failed the update or residual test, or the active `limit` instances.
5. HYG-03: IF a transient step does not converge at the smallest step THEN the solver SHALL fail with `E0508` naming the time and the failing unknowns or active `limit` instances.
6. HYG-03: IF that step is the one that reads a jump (the first step after a `Restart::Integrator` delivery) THEN the solver SHALL fail with `E0504` naming the time and the carriers or params whose delivery it reads.
7. HYG-01: WHEN a twin-style `.msl` source with a loop of two potential sources is run from source on either backend THEN the error SHALL contain `E0503` and both instance paths.

**Independent Test**: Run a `.msl` with two `Vsource`s in parallel through the parity bench: the error reads `E0503` and names `Top.v1` and `Top.v2`.

---

### P1: Exact re-entry ⭐ MVP

**User Story**: As the PSS author of lane B, I want a transient re-entered from a captured step to continue exactly as the run it was captured from, so that a shooting map is a function of the state.

**Why P1**: N1 (`GAPS.md` V8); the user names it the lane's priority.

**Acceptance Criteria**:

1. HYG-04: WHEN a run is re-entered from any step it recorded, with the same options and stop time THEN every later accepted time and every value of every unknown SHALL equal the continuous run's, bit for bit.
2. HYG-04: WHEN the captured step is the first step after a jump's delivery THEN the re-entered run SHALL take the restart the continuous run took (first-order stage, exemption, step size).
3. HYG-04: The captured state SHALL hold the Newton history, the step-size controller's memory, `dt`, `prev_h`, the last accepted step size, and the pending restart.
4. HYG-13: WHERE `Reentry::LastStep` is set the solver SHALL attach a re-entry state to the last step only, and every other step SHALL carry none; with the default `Reentry::EveryStep` the result SHALL be what it is today.

**Independent Test**: Re-enter `build_clocked_rc` (`transient_reentry.rs`) at each of its clock-edge steps; every later step equals the continuous run's bits.

---

### P1: A nonlinear charge through a limit is right ⭐ MVP

**User Story**: As a model author, I want a `ddt` of a quantity computed from a `limit`ed value (a diode's diffusion charge) to integrate the circuit's charge, so that a junction device's transient is not silently wrong.

**Why P1**: A silent wrong answer (Q0); every ngspice junction model with `tt` or depletion charge needs it.

**Acceptance Criteria**:

1. HYG-05: WHEN a transient starts from the operating point of a circuit whose only dynamics are a `ddt` of a nonlinear function of a `limit` output THEN the first accepted step SHALL stay within `reltol·|v| + vntol` of the operating point, since nothing drives it away.
2. HYG-05: The charge a `DDT` term stores in its history at each converged stage SHALL be the value of its argument at the solution of that stage.
3. HYG-06: WHEN the twin `tran_dq` (1 V, 1 kΩ, 1.59 nF and a diode with `tt` = 1 µs, held at 0 V at the start) runs to 10 µs under the harness's policy THEN `v(a)` SHALL agree with ngspice under `TRAN_VOLTAGE` at every exported time, on both backends.
4. HYG-06: The parity crate SHALL print, per twin and quantity, the worst ratio of `|ngspice − piperine|` to its contract bound and where it occurs, so that a commit can carry a before/after table.

**Independent Test**: `cargo test -p piperine-parity --test parity_tran tran_dq` with ngspice present.

---

### P2: Truncation error measured on the states

**User Story**: As a circuit author, I want the step-size control to measure the error where ngspice measures it, on the charge of each `ddt`, with `chgtol`, so that a node forced by a source does not hide an inaccurate current.

**Why P2**: V1; waits for Q1 (and Q2 for the skip).

**Acceptance Criteria**:

1. HYG-07: The truncation test SHALL measure each `DDT` term's state with the normalization of Q1(a), and SHALL read `chgtol`.
2. HYG-07: WHEN a sine source of 1 V at 100 kHz drives a nonlinear capacitor `q = c0·v + c1·v³` (c0 = 1e-10, c1 = 1e-9) directly THEN the source's current SHALL differ from the analytic `dq/dt` by at most `trtol·tol`, `tol = max(abstol + reltol·|i|, reltol·max(|q|, chgtol)/h)` (ngspice `CKTterr`'s criterion), at every accepted step of one period, which the node-potential basis does not achieve (measured 2.28 of that bound). (Amended 2026-11-17, orchestrator: AC 2 restated as ngspice CKTterr's criterion per Q1(a); the literal `reltol·|i| + abstol·10³` is unreachable, its charge floor being `reltol·|q|/h`.)
3. HYG-07: WHEN `chgtol` is raised by 10⁶ on a circuit whose charges lie under it THEN the run SHALL take fewer accepted steps than with the default.
4. HYG-08: The truncation estimate SHALL skip no state for the asymmetry of its two differences; WHEN a node is flat over the first sub-step and rises over the second by more than the tolerance THEN the step SHALL be rejected.
5. HYG-07: The 7 twins and `tran_dq` SHALL stay within their unchanged contracts on both backends.
6. HYG-07: The commit that changes the basis SHALL carry the deviation table of HYG-06 before and after; a twin whose worst ratio grows by more than 10 % of its bound SHALL be shown closer to an ngspice run at `reltol=1e-7`, else the change is not committed.

**Independent Test**: `cargo test -p piperine-solver --test truncation`.

---

### P2: A step that cannot meet the error at the floor fails

**User Story**: As a circuit author, I want a transient that cannot meet its truncation tolerance at the smallest step to stop and say so, as ngspice does, instead of silently relaxing §17.2.1.

**Why P2**: V3; waits for Q3.

**Acceptance Criteria**:

1. HYG-09: WHEN a step at `dt_min` fails the truncation test for the second consecutive attempt THEN the transient SHALL fail with `E0508` naming the time, and SHALL NOT accept the step.
2. HYG-09: WHEN a step first reaches `dt_min` after a larger step THEN the solver SHALL retry once at `dt_min` before failing.
3. HYG-09: A run that never reaches the floor SHALL print the same bytes as before.

**Independent Test**: A hand-written source with a corner it does not announce, across a capacitor, with `dt_min` = 1 ns: the run fails with `E0508` at the corner's time.

---

### P2: Newton damping as ngspice

**User Story**: As a circuit author, I want Newton to converge the way ngspice does, so that an operating point Piperine finds is the one ngspice finds.

**Why P2**: V4; waits for Q4.

**Acceptance Criteria**:

1. HYG-10: The DC and transient drivers SHALL iterate with the `NewtonStrategy` of their `ConvergencePlan`, not a hard-coded one.
2. HYG-10: The default strategy SHALL apply no damping, and `NodeDamping` SHALL apply ngspice's rule: only in operating-point solves, only to node potentials, only while a `limit` is active, when the largest node step exceeds 10 V, scaling the step by `max(10 / maxΔv, 0.1)`.
3. HYG-10: The 7 twins SHALL converge at every point with the default strategy and stay within their contracts.

**Independent Test**: `cargo test -p piperine-analog-solver --lib node_damping` and the parity targets.

---

### P2: The step after a jump

**User Story**: As a mixed-signal author, I want the first step after a digital change that `analog` reads to be small enough that its first-order error meets the contract at the harness's step, so that a switch's response is right without a hand-picked step cap.

**Why P2**: V11 (F-B7); waits for Q5.

**Acceptance Criteria**:

1. HYG-11: WHEN a step follows a `Restart::Integrator` delivery THEN its size SHALL be at most `0.1·min(last accepted step, time from the jump to the next scheduled time point)`, and no larger than the controller's proposal.
2. HYG-11: WHEN `boundary.rs` `a_carrier_change_is_a_time_point_and_starts_the_response` runs at the harness's 10 ns cap THEN every accepted time SHALL agree with the analytic response under `TRAN_VOLTAGE`.
3. HYG-11: A host `set` SHALL keep its restart of 10⁻³ of the last step (§15.2 rule 6).

**Independent Test**: `cargo test -p piperine-parity --test boundary`.

---

### P2: A transient that starts later

**User Story**: As a host author, I want a transient with a start time and no captured step to start from an operating point at that time, so that `with_start` means what it says.

**Why P2**: V14; waits for Q6.

**Acceptance Criteria**:

1. HYG-12: WHEN a transient of devices has `start_time` = 1 ms and no captured step THEN it SHALL solve the operating point at instant `(1 ms, 0)`, its first recorded step SHALL be at 1 ms, and `now()` there SHALL read 1 ms.
2. HYG-12: WHEN that circuit holds a timer device that commits its output `delay` after its `START` reaction THEN the `init` commits SHALL run at `(1 ms, 0)` and the timer's commit SHALL land at `1 ms + delay`, and no instant before 1 ms SHALL run.
3. HYG-12: IF `start_time` is not finite or is negative THEN the transient SHALL fail before the operating point, naming the value.

**Independent Test**: `cargo test -p piperine-solver --test transient_reentry a_late_start`.

---

### P3: Dead code, stale text, guards and the digital checkpoint

**User Story**: As a solver maintainer, I want no hook nobody calls, no document of a retired language, a partials guard that cannot be satisfied by the wrong device, and a digital checkpoint that does not copy every net, so that the code reads as what it does.

**Why P3**: V5, V6, V10, V7: no result changes.

**Acceptance Criteria**:

1. HYG-15: The analog sub-solver SHALL define neither `apply_limit` nor `apply_limiting_reports`, and `removed_surface.rs` SHALL fail if either name returns to a solver crate.
2. HYG-15: No source of a solver crate SHALL mention `PHDL` or `@rfport`; the `.sp` documentation SHALL say that ports are given by the host on the analysis (`SpPort`).
3. HYG-16: `every_hand_written_device_is_in_a_partials_check` SHALL accept a device only when a partials check verifies that device's module, and SHALL fail on a planted `verify_every_function("X", module::<Y>())`.
4. HYG-14: WHEN the digital engine marks and later rewinds THEN its store, queue, dirty set and counters SHALL equal a full `checkpoint()` taken at the mark.
5. HYG-14: WHEN an instant changes k of N nets between a mark and a rewind THEN the mark SHALL hold k net entries, not N.

**Independent Test**: `cargo test -p piperine-solver --test removed_surface --test abi_devices` and `cargo test -p piperine-digital-solver --test checkpoint`.

---

### P3: The records say what is true

**User Story**: As a reader of the repository, I want `GAPS.md`, `ROADMAP.md`, `.specs/STATE.md` and the spec to describe the solver as it now is.

**Why P3**: `GAPS.md` says to remove an entry in the commit that closes it.

**Acceptance Criteria**:

1. HYG-17: WHEN the feature closes THEN `GAPS.md` §6 SHALL no longer list V1 to V11, V14 and V15, and §4.8 SHALL no longer list the first bullet, each removed in the commit that closes it or in the closing commit; V12, V13 and V16 SHALL stay.
2. HYG-17: Appendix C.2 and Part V SHALL describe the chosen truncation basis, damping, step floor, restart step and start, each with an assertion in `spec_text.rs`.
3. HYG-17: `.specs/STATE.md` SHALL record the feature's decisions as `MD-67`.

---

## Edge Cases

- IF a singular Jacobian has more than 2 000 unknowns THEN the `E0503` report SHALL say the diagnosis was skipped and give the unknown count.
- IF a singular part involves only ground-referenced unknowns of one instance THEN the report SHALL name that instance once.
- WHEN a re-entry is made with a stop time other than the captured run's THEN the continuation SHALL follow §16.4 (the old stop is a time point) and is not required to equal the continuous run.
- IF a run re-entered under `Reentry::LastStep` is asked to re-enter from a middle step THEN it SHALL fail as today ("carries no state of the device instances").
- WHEN two jumps fall within one restart step THEN the restart rule SHALL apply from the later one.
- WHEN the step floor is reached at the stop time THEN `E0508` SHALL still name that time.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| HYG-01 | P1: Loud, named run failures (E0503) | Tasks | Pending |
| HYG-02 | P1: Loud, named run failures (E0507) | Tasks | Pending |
| HYG-03 | P1: Loud, named run failures (E0504, E0505, E0508) | Tasks | Pending |
| HYG-04 | P1: Exact re-entry | Tasks | Pending |
| HYG-05 | P1: A nonlinear charge through a limit | Tasks | Pending |
| HYG-06 | P1: The nonlinear-charge twin and the deviation table | Tasks | Pending |
| HYG-07 | P2: Truncation error on the states (V1) | Tasks | Pending |
| HYG-08 | P2: No asymmetric skip (V2) | Tasks | Pending |
| HYG-09 | P2: The step floor fails (V3) | Tasks | Pending |
| HYG-10 | P2: Newton damping as ngspice (V4) | Tasks | Pending |
| HYG-11 | P2: The step after a jump (V11) | Tasks | Pending |
| HYG-12 | P2: A transient that starts later (V14) | Tasks | Pending |
| HYG-13 | P1: Re-entry state per step is optional (V9) | Tasks | Pending |
| HYG-14 | P3: The digital checkpoint journal (V7) | Tasks | Pending |
| HYG-15 | P3: Dead hooks and stale text (V5, V6) | Tasks | Pending |
| HYG-16 | P3: The partials guard by module (V10) | Tasks | Pending |
| HYG-17 | P3: The records | Tasks | Pending |

**Coverage:** 17 total, 17 mapped to tasks, 0 unmapped.

---

## Sources

- `GAPS.md` (2026-11-17) §6 V1 to V16, §4.8 - the cluster and its evidence
- `.specs/features/solver-abi-integration/verification.md` N1 to N3 - re-entry, per-step state, the partials guard
- `.specs/features/ir-rust/verification.md` F-B7 - the restart after a jump
- `docs/spec/part_v_execution.md` §15.2, §16, §16.4, §17.1, §17.2 - the codes and rules
- `docs/spec/appendix_c_implementation.md` C.2 - the reference numerics
- ngspice-47 sources, `src/spicelib/analysis/{dctran.c,cktterr.c}`, `src/maths/ni/niiter.c` - the reference behaviour of Q1 to Q5
- `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits), MD-60 to MD-62, MD-66

---

## Success Criteria

- [ ] A singular, non-finite or non-converging run prints a spec code and names a node, branch or instance; `grep -c 'E050[3-8]'` over the solver crates finds each of the five codes.
- [ ] `transient_reentry.rs` compares re-entered runs bit for bit.
- [ ] `tran_dq` is a twin under `TRAN_VOLTAGE`; every commit that moves a twin carries a before/after deviation table.
- [ ] `GAPS.md` §6 holds V12, V13 and V16 only.

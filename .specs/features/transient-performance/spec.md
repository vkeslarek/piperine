# Transient Performance Specification

**Approved**: 2026-11-20 - under the user's standing AFK rule (2026-11-19: "Tu sempre assume o default mais razoável e depois me conta pois vou ficar afk"). The user asked for the feature directly on 2026-11-20: "quero uma feature de melhora de performance". The defaults D1 to D12 below are the planner's and are the user's to correct; none of them is the user's own answer.

**Absorbs** `multi-rate` (spec approved by the user 2026-11-18, never designed). Its requirements MR-01 to MR-08 map to TP-01 to TP-03, TP-06, TP-09 and TP-10 here; its answers Q1 (option B), Q2 (C) and Q4 (A) are kept as the user gave them; its Q3 gate (135 000 steps) is replaced, because the measurement shows it cannot be met (D6).

**Measured basis**: `diagnosis.md` in this directory. Every number in this spec is from it.

## Problem Statement

A mixed-signal transient of the buck converter takes about 6 million accepted steps for 5.2 ms of converter time and about 21 minutes for one 4.7 ms run; the 13 characterization runs cost 2.9 hours of CPU. Measured on 100 µs of the start: 127 608 accepted steps, of which 39 999 end on a landing at a 200 MHz clock edge although only 367 instants change anything the analog side reads and 50 sample it; the step never reaches the 10 ns `max_step`; each step costs 120 to 160 µs. Two kinds of step are exempt from the truncation-error test, which is harmless only while every step is tiny: every variant that removes the landings without closing that moves `buck.vout` by 1 to 5 mV.

## Goals

- [ ] The buck's 600 µs start takes at most 300 000 accepted steps with `max_step = 1e-8` (from about 675 000), with its SPEC limits held and its checks passing.
- [ ] `buck.vout` over the first 100 µs of the start differs from a `max_step = 1e-10` reference by at most 0.104 mV on the 0.5 µs grid (today's difference), so the speed is not bought with accuracy.
- [ ] The solver reports why it landed and how its steps were spent, so that the next slow design is diagnosed by reading a table.
- [ ] One device-function evaluation and one assembly of the transient stamps allocate nothing after the first call, and a Newton iteration reads no clock unless timing was asked for.
- [ ] No ngspice parity value or tolerance moves; the 7 twins print the same bytes.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| TP-D1: a higher-order interpolant for the print grid, so that `max_step` can be dropped | Measured gain 1.5x more (26 906 to 17 915 steps per 100 µs). It changes every printed row between two accepted steps, so it moves the twins' bytes and the Book's listings: a separate decision (D9) |
| TP-D2: sample-only instants read the step's interpolant instead of landing (the user's interpolation idea) | Measured for the buck: 50 landings in about 27 000 steps per 100 µs, under 0.5 %. `diagnosis.md` §7. Specified as a later feature with its trigger (TP-01 AC 4); the design sketches it (D10) |
| Accepting a truncated step from interpolated state instead of re-solving | The protocol runs the digital side before the analog solve, so a write to an analog-read net costs a discrete restore and no Newton solve (measured: 0 analog retakes). There is nothing to save |
| Changing the text of §17.2 rule 2 | `multi-rate` Q1 option B, the user's: skip, as-if, only the landings no model can observe |
| Step-controller gains (growth cap 1.5, rejection ÷ 8), `trtol`, `reltol`, `chgtol`, any tolerance | Never retuned. Measured anyway: growth cap 2 gives 9 %, and moves every pinned time grid |
| Keeping the controller's step across a landing-shortened step | Measured 13 % once landings are dynamic; moves the time grid after every landing in every circuit |
| The cost of the edges themselves: about 45 steps per gate-ramp corner | It is the model's stiffness (200 pF against 20 mΩ) under the truncation test as ngspice applies it. `surrogate-models` (V13) owns averaged power stages |
| Different integration rates per analog partition | Part V has one transient step for the whole analog side (§15, I6) |
| Sparse-LU reuse across Newton iterations, device bypass | 12 % of the profile and correctness-sensitive; not needed for the goals. Reported, not planned |
| `Assembly::push_stage` re-evaluating every function after each converged stage (21 % of the profile) | Removing it changes which iterate the histories are taken at, so it can move every transient result in the last bits, parity included. Reported to the user (D11) |
| A CLI flag or a run key for the statistics or for the reference mode | `multi-rate` Q4 A: a solver and `piperine-api` option only (D8) |
| Rewriting Part V §15.2 "Before the solve of `t`" to the protocol's order | A divergence between Part V and `protocol/transient.rs` that predates this feature (`diagnosis.md` §5). Reported to the user (D12) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D1. How a landing nothing observes is skipped | `multi-rate` Q1 option B: as-if, never in a design with a `settle` reaction, a host waiter, or a host probe at that time | the user, 2026-11-18: "vou de aceito todas" | y - user (via `multi-rate`) |
| D2. A reference mode exists | A solver option that restores today's landings; no run key, no flag (`multi-rate` Q4 A) | the comparison tests need both modes in one test | y - user (via `multi-rate`) |
| D3. The step that ends on a landing is tested for truncation error | Yes. It reads the left value, so it is an ordinary step; Part V exempts the step that reads the change | Measured: with the exemption the fast variants are wrong by 4.9 mV, without it by 0.003 mV. Appendix C.2, D and F rows are amended; §15.2's rule is restated by what a step reads | n - planner |
| D4. The step that reads a jump is error-controlled | It keeps the bound of §15.2 rule 6 (a tenth of the last step and of the gap) and is also tested with a first-order error estimate taken from its own two stages against the tolerance of §17.2 rule 1; a failing step is rejected and retaken smaller. §15.2 rules 6 and 7 are amended | Measured: bounding that step takes the error from 2.7 mV to 0.002 mV. An estimate from the step's own stages uses nothing from before the jump, so the reason for the exemption (Milne measures the jump) does not apply to it | n - planner |
| D5. If D3 or D4 would move a parity value, a twin's printed bytes or a pinned time grid of a parity target | Stop and return to the user with the moved values. No parity file or tolerance is edited | brief: parity is the top priority | n - planner |
| D6. The buck gate | At most 300 000 accepted steps for the 600 µs start with `max_step = 1e-8`. Expected 185 000 to 225 000: 30 852 per 100 µs measured with the same three changes and the jump step bounded at 25 ps; the error-tested jump step of D4 may start smaller and regrow over about 15 more steps per jump. `multi-rate` Q3 gave 135 000, on the assumption that only the 60 000-step floor of `max_step` remains; the measurement shows the 8 gate-ramp corners per period cost about 45 steps each (108 000 per 600 µs), so the floor is about 168 000 and 135 000 cannot be met | The limit is set from the measurement with margin; a miss returns to the user and the limit is never moved to fit | n - planner; replaces the user's Q3 answer |
| D7. The accuracy gate | The largest difference of `buck.vout` on the 0.5 µs grid over 100 µs against a `max_step = 1e-10` run is at most 0.104 mV, the baseline's own difference | The feature may not be less accurate than what it replaces; the bound is measured, not chosen | n - planner |
| D8. Where the statistics are read | `piperine-solver`'s result and a `piperine-api` accessor on the session; `buck_long` prints them. No CLI flag | D2's reasoning; MD-65 can add a flag later | n - planner |
| D9. `max_step` in the buck's runs | Unchanged (`1e-8`) in `examples/buck/Piperine.toml` and `tests/e2e/buck/Piperine.toml`; `max_steps` in the e2e package stays 15 000 000 and its comment states the new count | The printed rows are linear interpolation between steps; without a better interpolant a larger step flattens the ripple the scenarios measure | n - planner |
| D10. The interpolation idea | Not built in this feature. The statistics report sampling landings as a share of accepted steps; the design records the interpolant (the quadratic through the three points of a TR-BDF2 step) and the rule for a later feature | `diagnosis.md` §7: under 0.5 % for the buck | n - planner |
| D11. `push_stage`'s re-evaluation (21 % of the time) | Left as it is | It can move transient results in the last bits everywhere | n - planner |
| D12. Mixed-signal goldens | The buck's listings, the Book's buck rows and any mixed golden may move only by TP-04 and TP-05; each moved value is listed with its old and new value in `moved.md` of this directory. Records and digital columns do not move | brief | n - planner |
| What a reaction that probes reads | The latest accepted solution, as today (§15 step 3.3) | Removing a landing that nothing observes moves "the previous time point" the way any step-control choice does (`multi-rate`, same row) | y - user (via `multi-rate`) |
| Wall time | Printed by `buck_long`, never asserted | `long-run` decision 5 | y - prior decision |
| Baseline for the profile | 220 stack samples with `gdb`; `perf` is refused on the machine (`perf_event_paranoid = 3`) | `diagnosis.md` §3 | y - measured |
| In-flight branches | Implementation starts after `analog-operators` (T9: the step that reads a jump is first order in both stages) and `unknown-semantics-events` (`settle` is an MSL event) are on main; the worktree is rebased then | Both edit `instant.rs`, `protocol/transient.rs` and `analyses/transient.rs`; TP-05's estimate is defined on T9's two backward-Euler stages; D1's condition needs `settle` to exist in MSL | n - planner |

**Open questions:** none - all resolved or logged above. D3 to D12 are the planner's defaults under the AFK rule and are listed for the user in the final report.

**Implicit-requirement dimensions.** Input validation: TP-06 AC 3 (the option has two values; no user text reaches it). Failure states: TP-03 AC 4 (a speculated instant that turns out to touch the analog side is undone), TP-05 AC 3 (a jump step that fails at the floor). Idempotency: N/A because a transient runs once and a restored step replays the same instants (TP-09 AC 2). Auth and rate limits: N/A, local tool. Concurrency and ordering: TP-09 AC 3 (instants in `(t, n)` order in both modes). Data lifecycle: N/A because nothing new is stored. Observability: TP-01. External dependency: TP-09 AC 5 (ngspice absent). State transitions: TP-09 AC 2 (a rejected step undoes every instant run ahead of it).

---

## User Stories

### P1: The solver says how its steps were spent ⭐ MVP

**User Story**: As a designer whose mixed design runs slowly, I want the solver to report its accepted and rejected steps, its landings by cause and its jump steps, so that I can see what sets the step without instrumenting the solver.

**Why P1**: Every gate below is stated in these counts; the baseline is recorded with them before anything changes.

**TP-01** (step statistics)

**Acceptance Criteria**:

1. WHEN a transient completes THEN the solver SHALL report the number of landings for each cause: an analog-read net changed, a firing reaction reads a continuous pin, a firing reaction is flagged by the static rule only, an `IRREVERSIBLE` reaction or a host waiter, a `FINISH`.
2. WHEN a transient completes THEN the solver SHALL report the accepted steps, the attempts rejected by the truncation test, the attempts rejected by Newton, the steps retaken toward a crossing, the steps that read a jump, the steps that ended at `max_step`, and the instants run.
3. The solver SHALL count each accepted step under exactly one end: a landing cause, a located crossing, `max_step`, a `bound_step` cap, the stop time, or the controller's proposal.
4. WHEN a transient completes THEN `piperine-api` SHALL return those counts from the session, and the share of accepted steps that ended on a sampling landing SHALL be computable from them.
5. WHEN the buck's `startup` run completes in the reference mode THEN `buck_long` SHALL print every count of AC 1 and AC 2 on standard error.

**Independent Test**: a solver test on a hand-built device set with one landing of each cause, each count asserted; the buck's counts printed by `buck_long`.

---

### P1: The solver lands only where something observable happens ⭐ MVP

**User Story**: As a designer of a clocked mixed system, I want a clock edge that changes nothing the analog side reads and samples nothing to cost no analog step, so that a 200 MHz controller does not set the analog step.

**Why P1**: 39 815 of 39 999 landings per 100 µs.

**TP-02** (the landings the rule requires, in both modes)

**Acceptance Criteria**:

1. WHEN a scheduled commit to `clk` low is heard only by a reaction on `clk.rise()` THEN the solver SHALL NOT land at that time (§17.2 table, "a falling `clk` heard by `rise()`").
2. WHEN a commit due at `(t, 0)` changes a carrier `analog` reads THEN the solver SHALL land on `t`.
3. WHEN a reaction that reads a continuous pin fires at `t` THEN the solver SHALL land on `t`.
4. WHEN a digital net that no reaction reaching the analog side reads changes at `t` THEN the solver SHALL NOT land on `t`.

**TP-03** (the as-if skip)

**Acceptance Criteria**:

1. WHERE the design has no reaction on `settle`, no host waiter and no host probe at `t`, WHEN the instant at `t` runs to quiescence with no analog-read carrier changed and no firing reaction that reads a continuous pin THEN the solver SHALL NOT land on `t`.
2. WHERE the design has a reaction on `settle` the solver SHALL land on every time point §17.2 rule 2 gives, as in the reference mode.
3. WHEN a host waiter's event fires at `t` THEN the solver SHALL land on `t` as in the reference mode (abi.md R7).
4. IF an instant run ahead of the analog step changes a carrier `analog` reads at a microstep after the first THEN the solver SHALL restore that instant's checkpoint and land on its time, leaving no record of the undone run.
5. WHEN a reaction's only statement on a net that reaches the analog side is a commit made with a delay of at least one tick, whatever expression gives the delay, THEN the solver SHALL NOT land at the instant that makes the commit (the buck's `Osc`, whose delay `0.5 / f` is a param expression: 20 000 landings per 100 µs today).
6. WHEN the buck's first 100 µs run THEN the solver SHALL report at most 1 000 landings (measured: 417 expected; 39 999 today).

**Independent Test**: `cargo test -p piperine-digital-solver --test advance`, one test per criterion on hand-built devices; the buck's count in `buck_long`.

---

### P1: Every step that can be wrong is tested ⭐ MVP

**User Story**: As the owner of the buck's SPEC limits, I want every transient step held to the truncation tolerance, so that the solution does not depend on the steps being accidentally small.

**Why P1**: Without it the faster run is wrong by 1 to 5 mV; with it, it is more accurate than today's.

**TP-04** (the step that ends on a landing)

**Acceptance Criteria**:

1. WHEN a step ends on a landing and reads no jump THEN the solver SHALL apply the truncation test of §17.2 rule 1 to it.
2. IF that step fails the test above the smallest step THEN the solver SHALL reject it, restore the checkpoint and propose a smaller step, replaying the instants run ahead.
3. The documents SHALL say the same: Appendix C.2's "the jump" and "truncation error" rows, Appendix D.4's and Appendix F's "Restart" rows state that the step that arrives at a landing is tested and the step that reads the change is the one treated specially.

**TP-05** (the step that reads a jump)

**Acceptance Criteria**:

1. WHEN a step reads a jump THEN the solver SHALL size it by §15.2 rule 6 and SHALL test it with the first-order error estimate of its two stages against the tolerance of §17.2 rule 1.
2. IF that step fails the estimate above the smallest step THEN the solver SHALL reject it and retake it at a smaller size, still as the step that reads the jump.
3. IF that step fails the estimate at the smallest step THEN the solver SHALL accept it and count it, as §17.2's `E0508` row says of any step at the floor.
4. WHEN an ideal step of 1 V is read by an RC of 1 kΩ and 1 nF THEN the solution one time constant after the jump SHALL be within `reltol` of `1 − e⁻¹` volts.
5. WHEN the buck's first 100 µs run THEN the largest difference of `buck.vout` on the 0.5 µs grid against the same run with `max_step = 1e-10` SHALL be at most 0.104 mV.
6. The documents SHALL say the same: §15.2 rule 6 states the estimate and the retake, and Appendix C.2's "restart" row and its "restart step" constant row state them.

**Independent Test**: `cargo test -p piperine-solver --test jump_step` (the RC, the floor, the retake); `cargo test -p piperine-api --test buck_accuracy`.

---

### P1: The buck is fast and still right ⭐ MVP

**User Story**: As the author of the buck's scenarios, I want the start and the five scenario families to run in a fraction of today's time with every SPEC limit held.

**Why P1**: The user's goal.

**TP-06** (the buck gates and the reference mode)

**Acceptance Criteria**:

1. WHEN the buck's `startup` run (600 µs, `max_step = 1e-8`) completes THEN the solver SHALL have accepted at most 300 000 steps.
2. WHEN that run completes THEN the largest `buck.vout` SHALL be at most 0.815 V, the settling time inside ±10 mV SHALL be between 207.5 and 307.5 µs, the ripple over the last 40 µs SHALL be greater than 0 and at most 10 mV, every record of `checks` SHALL have `pass` true, and the last row SHALL hold `buck.ctl.reference` 128, `mode` 2 and `pgood` true (`long-run` LR-09 to LR-11, unchanged).
3. WHERE the solver option for the reference mode is set the solver SHALL land wherever it lands before this feature; WHEN the option is absent THEN the solver SHALL use the rule of TP-03.
4. WHEN that run completes THEN `buck_long` SHALL print the wall time and the counts of TP-01 on standard error, asserting no time.
5. WHEN the five runs of `tests/e2e/buck` complete THEN every test of `buck_scenarios` SHALL pass with its source unedited.

**Independent Test**: `cargo test -p piperine-api --test buck_long`; `cargo test -p piperine --test buck_scenarios`.

---

### P2: A step costs less

**User Story**: As anyone running a long transient, I want the solver to spend its time in the device equations and the linear solve, not in allocation, validation and clocks.

**Why P2**: 1.5x to 2x on every transient, mixed or not; the P1 stories give the buck 4x without it.

**TP-07** (the Newton iteration)

**Acceptance Criteria**:

1. WHEN `Handle::eval` is called for a function it has been called for before THEN it SHALL allocate nothing, and it SHALL still refuse a buffer shorter than its cell with the error it gives today.
2. WHEN `Assembly::assemble_tran` is called a second time for the same stage shape THEN it SHALL allocate nothing.
3. WHILE no timing was requested the Newton solver SHALL read no clock, and `assembly_time_ns` and `solve_time_ns` SHALL be 0.
4. WHERE timing is requested the solver SHALL report both totals as today.

**TP-08** (the idle instant)

**Acceptance Criteria**:

1. WHEN a step is accepted at a time with no commit due, no crossing fired and no reaction on `settle` THEN the solver SHALL run no instant there.
2. WHEN the buck's first 100 µs run THEN the instants run SHALL be at most the scheduled instants plus the located crossings plus the landings (today: one per accepted step, 127 611).

**Independent Test**: allocation tests with the counting allocator in `piperine-abi` and `piperine-analog-solver`; a solver test counting instants.

---

### P1: Nothing else moves ⭐ MVP

**User Story**: As the owner of the parity claim and of Part V, I want the faster solver to change nothing a model or a host can see except where the amended rules say so.

**Why P1**: The brief: ngspice parity is the top priority.

**TP-09** (exactness)

**Acceptance Criteria**:

1. WHEN the open-loop fixture (a 200 MHz `Osc`, a counter that commits every edge, a `D2A` whose input changes once per 400 edges, into an RC; no reaction probes) runs with the option absent and in the reference mode THEN every record of every stream SHALL be equal in both runs, field for field, instant for instant.
2. WHEN a step is rejected after instants ran ahead of it THEN the solver SHALL leave carriers, pending events, instance state and records as they were before those instants (§15 I8).
3. The solver SHALL run instants in increasing order of `(t, n)` in both modes (§15 I3).
4. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran`, `parity_grid`, `parity_corpus`, `deviation` and `twins` of `piperine-parity` and `twins` of `piperine-cli` with their sources, fixtures and `contract.rs` unedited.
5. WHERE ngspice is absent the parity targets SHALL print the harness's skip line and still run the twins.
6. WHEN a mixed-signal golden moves THEN `moved.md` SHALL list it with its old value, its new value and the requirement (TP-04 or TP-05) that moves it, and no record and no digital column SHALL be in the list.

**Independent Test**: `cargo test -p piperine-solver --test landing_rule`; `cargo test -p piperine-parity`; `cargo test -p piperine-cli --test twins`.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md`, Appendix C and the buck's records, I want them to describe the landings and the step counts as they are.

**Why P3**: No behaviour depends on it.

**TP-10** (documents)

**Acceptance Criteria**:

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list V12, and `ROADMAP.md` SHALL mark `multi-rate` delivered by `transient-performance`.
2. WHEN this feature merges THEN Appendix C.2's "time points" row SHALL describe the as-if rule, its three exceptions and the reference mode.
3. WHEN this feature merges THEN the comment of `tests/e2e/buck/Piperine.toml` SHALL state the measured step count of 5.2 ms, and `diagnosis.md` SHALL hold an "After" table of the same counts as its section 2.

**Independent Test**: `cargo test -p piperine-cli --test docs`; `cargo test -p piperine-lang --test spec_text`.

---

## Edge Cases

- WHEN two commits fall on the same tick and only the second microstep changes an analog-read carrier THEN the solver SHALL land on that tick once.
- WHEN an instant that is not landed on is due exactly at the stop time THEN the solver SHALL land on the stop time (§17.2: the stop is a time point).
- WHEN a `finish` record is written at an instant that was run ahead THEN the solver SHALL land on that instant and end the analysis there (§19 rule 5).
- IF an `IRREVERSIBLE` reaction fires at an instant THEN the solver SHALL land there before running it, in both modes (abi.md R6).
- WHEN a `cross` goes from `≤ 0` to `> 0` in a step THEN the solver SHALL locate and land on it in both modes (§17.3).
- WHEN `bound_step` caps the step THEN the cap SHALL apply in both modes (§17.2 rule 5).
- IF a jump step is retaken more than `max_rejections` times THEN the solver SHALL fail with the error it gives today for consecutive rejections, naming the time.
- WHEN a step reads a jump and also ends on a landing THEN the solver SHALL apply TP-05 to it, not TP-04.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| TP-01 | P1: the solver says how its steps were spent | Tasks | In Tasks |
| TP-02 | P1: the solver lands only where something observable happens | Tasks | In Tasks |
| TP-03 | P1: the solver lands only where something observable happens | Tasks | In Tasks |
| TP-04 | P1: every step that can be wrong is tested | Tasks | In Tasks |
| TP-05 | P1: every step that can be wrong is tested | Tasks | In Tasks |
| TP-06 | P1: the buck is fast and still right | Tasks | In Tasks |
| TP-07 | P2: a step costs less | Tasks | In Tasks |
| TP-08 | P2: a step costs less | Tasks | In Tasks |
| TP-09 | P1: nothing else moves | Tasks | In Tasks |
| TP-10 | P3: the documents say what is true | Tasks | In Tasks |

**Coverage:** 10 total, 10 mapped to tasks, 0 unmapped.

---

## Gaps closed

| `GAPS.md` id | Requirement(s) | How |
| ------------ | -------------- | --- |
| V12 | TP-01 to TP-06, TP-09 | measured, held to rule 2, skipped as-if, gated on the buck, parity unmoved |

---

## Sources

- The user, 2026-11-20: "quero uma feature de melhora de performance"; and the interpolation idea, verbatim: "quando eu faço um passo analógico, posso ter X breakpoints no meio do caminho e isso que deixa o run do buck lento... E se os breakpoints fossem na verdade separados com backtracking? ... Se eu passar o passo analógico normal e em cada passo digital, eu fizer o interpolado? Aí só quebra no momento que o digital levantar um evento que interfira no analógico. Aí precisa fazer backtracking, ou seja, posso até reiniciar o passo anterior com um timestep menor ou, na melhor das hipóteses, consideramos aquele trecho razoavelmente linear (o que faz sentido dado o adaptative timestep) e tomamos a interpolação do estado no breakpoint do event."
- `.specs/features/multi-rate/spec.md` - Q1 to Q4 as the user answered them, MR-01 to MR-08.
- `.specs/features/transient-performance/diagnosis.md` - every count, the profile, the lever table.
- `docs/spec/part_v_execution.md` §15, §15.2 (rules 4, 6, 7), §17.2 (rules 1, 2, the table), §17.3.
- `.specs/features/device-abi/abi.md` §5 (R4, R6, R7, the protocol).
- `docs/spec/appendix_c_implementation.md` C.2 rows "the jump", "time points", "restart", "truncation error", and the constants table.
- `crates/piperine-solver/src/protocol/transient.rs`, `crates/piperine-solver/src/analyses/transient.rs` (`device_propose`, `device_attempt`, `assess_step`), `crates/piperine-digital-solver/src/instant.rs`, `plan.rs` (`reachability`), `crates/piperine-abi/src/caller.rs` (`Handle::eval`), `crates/piperine-analog-solver/src/device/assembly.rs`, `math/newton_raphson.rs`.
- `examples/buck/src/dpwm.msl`, `power.msl`, `sense.msl`; `tests/e2e/buck/Piperine.toml`.
- `.specs/features/long-run/spec.md` LR-09 to LR-12 (the buck's limits; wall time printed).

---

## Success Criteria

- [ ] The buck's 600 µs start: at most 300 000 accepted steps, SPEC limits held, checks passing.
- [ ] The buck's first 100 µs: `vout` within 0.104 mV of the fine reference, at most 1 000 landings.
- [ ] Parity targets and twins pass unedited; `moved.md` lists every moved mixed golden.
- [ ] `cargo build` has no warning and `cargo test` passes at the root.

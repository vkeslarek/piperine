# Multi-Rate Stepping Specification

Priority: **later** (user, 2026-11-17: multi-rate is a "melhoria futura… bem depois"). It is specified now so that the gap has an owner; it is queued after `solver-hygiene` and `host-ii`.

## Open questions for the user

Nothing below is decided. Each question has options and the planner's recommendation.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does an edge that changes nothing analog stop cutting the step? | **A.** Amend Part V §17.2.2 rule 2, second bullet, from "can change" (static) to "changes" (dynamic): a scheduled time is a time point only when the commits and microsteps at it actually change a carrier `analog` reads, or a reaction that probes fires. Fewer time points everywhere; `settle` sees fewer of them (the `device-abi` critic's amendment 1, the same trade). **B.** Keep §17.2.2 as written and skip, as-if, only the landings no model can observe: a time point that rule 2 gives only through its static clause, in a design with no reaction on `settle`, no host waiter and no host probe at that time. A design with any of those keeps every landing. **C.** B now, and A as a spec change later if a design with `settle` needs the speed | **B.** The brief is "every Part V rule preserved". B changes no sentence of Part V. The buck has no `settle` reaction (`settle` is refused today, `GAPS.md` L52), so B gives the buck the whole gain |
| Q2 | Which buck run is the measured one? | **A.** `startup` exactly as `examples/buck/Piperine.toml` writes it (`max_step = 1e-8`). Its floor is 60 000 steps, so at most an 11x cut from 675 000. **B.** `startup` with `max_step` removed for the measurement, the SPEC limits still asserted. **C.** Both: A gated, B printed | **C.** A is what a user runs; B shows what multi-rate buys when nothing else caps the step |
| Q3 | What does the gate assert? | **A.** At most 135 000 accepted steps for the 600 µs start (5x fewer than about 675 000). **B.** At most 67 500 (10x). **C.** Measure and print only | **A.** It sits above the 60 000 floor of Q2-A with room for the switching edges' own steps. The baseline is measured in the first task, before any change. A miss goes back to the user; the limit is never moved to fit |
| Q4 | Is there a way to turn multi-rate off? | **A.** A solver option (`piperine-solver` and `piperine-api` only), off giving today's landings; no run key. **B.** Also a run key `multirate = false` with its flag (MD-65). **C.** No switch | **A.** The comparison tests need the reference mode. A run key would be a new user surface for a debugging aid; MD-65 can add it later |

## Problem Statement

A mixed design lands the analog solver near every digital edge. The buck takes about 45 000 accepted steps per 40 µs, about 675 000 for its 600 µs start (average step 0.9 ns), about 130 s in a dev build (`GAPS.md` V12). Its 200 MHz `Dpwm` reaction commits `hs`, `ls` and `count` at every rising `clk`. `count` feeds `sample`, which an ADC reaction that probes listens to. So `digital-solver`'s static reachability (`plan.rs::reachability`) marks that reaction as touching the analog side, and the look-ahead (`instant.rs`, `probing`) lands at every rising edge, although `hs` and `ls` change four times per 2 µs period and `sample` rises once.

§17.2.2 says "a digital clock that nothing analog depends on costs the solver nothing". The buck's clock reaches the analog side only through rarely changing values, and it costs one landing, one integrator restart and a few short steps per edge.

## Goals

- [ ] A digital instant that changes no carrier `analog` reads and fires no reaction that probes does not cut the analog step, in every design where no model can observe the difference (Q1).
- [ ] The buck's 600 µs start takes at most 135 000 accepted steps (Q3), down from about 675 000. Its SPEC limits still hold, and its wall time is printed.
- [ ] Every landing the solver makes is one the time-point rule requires, and the solver says why it landed.
- [ ] Parity does not move: the 7 twins print the same bytes and every parity target passes unedited.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Changing the text of §17.2.2 rule 2 | Q1 option B needs no spec change. Option A would be the user's call and a separate edit |
| Different integration rates for different analog partitions (true multi-rate integration, latency) | Part V has one transient step for the whole analog side (§15, I6). It would be a new solver architecture |
| Averaged or surrogate power stages | `surrogate-models` (V13) |
| The cost of the digital checkpoint per instant (`GAPS.md` V7) | `solver-hygiene`. The look-ahead takes one per instant, so V7 bounds this feature's wall time (Dependencies) |
| The first-order step after a digital-to-analog jump (V11) | `solver-hygiene` |
| LTE on charge, Milne skip, damping (V1, V2, V4) | `solver-hygiene` |
| `settle` in MSL (L52) | `unknown-semantics`. This feature's `settle` criteria are proven on a solver test device |
| A new run key or flag | Q4 recommends none |
| A digital-only transient (L56) | `host-ii` |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| How landings are skipped | Q1 option B: as-if, never in a design with a `settle` reaction, a host waiter or a host probe | brief: "every Part V rule preserved"; awaiting Q1 | n - planner |
| What a reaction that probes reads before its solve | the latest accepted solution, as today (§15 step 3.3). Which accepted step that is depends on step control, which is the simulator's (§17.2, "How the candidate is chosen") | Removing a landing that nothing observes moves "the previous time point" the way any step-control choice does. The ADC's sample can move by the ripple slope times one step. Closed-loop results are judged by SPEC limits, never by equality | n - planner; tied to Q1 |
| Dynamic test | the look-ahead's own check after each microstep: an analog-read net changed (`analog_now`), or a firing reaction reads a continuous pin. The static `probes` flag no longer forces a landing under option B | `crates/piperine-digital-solver/src/instant.rs` already runs both checks; only the static one is the coarse part | n - planner |
| Baseline | measured in the first task on the current tree: accepted steps, rejected steps and landings by reason for the 600 µs start | V12 says "not measured here" | n - planner |
| The gate | at most 135 000 accepted steps (Q3 A); wall time printed, never asserted | `long-run` decision 5: wall time is printed | n - planner; awaiting Q3 |
| The reference mode | a solver option that restores today's landings (Q4 A) | the comparison criteria need both modes in one test | n - planner; awaiting Q4 |
| Open-loop fixture | a 200 MHz `Osc`, a counter that commits every edge and a `D2A` whose input changes once per 400 edges, into an RC; no reaction probes | Its digital side reads no analog value, so its records are independent of the step grid and can be compared bit for bit | n - planner |
| Tolerance | the existing `ToleranceContract::TRAN_VOLTAGE`; no new constant | MD-38: a new domain gets a constant, this is not one | n - planner |

**Open questions:** Q1 to Q4 at the top of this file are open; the rows above follow the recommendations until the user answers.

**Implicit-requirement dimensions.** Input validation: N/A because no new input exists (Q4 A). Failure states: MR-05 AC 3 (a speculated instant that turns out to touch analog is undone and landed on). Idempotency: N/A because a transient runs once. Auth and rate limits: N/A, local tool. Concurrency and ordering: MR-05 (instants still run in `(t, n)` order, I3). Data lifecycle: N/A because nothing new is stored. Observability: MR-01 (landings counted by reason). External dependency: MR-08 AC 2 (ngspice absent: the parity harness's skip line). State transitions: MR-05 AC 4 (a rejected step undoes every speculated instant, I8).

---

## User Stories

### P1: The solver says why it landed ⭐ MVP

**User Story**: As the solver's maintainer, I want every landing counted by the clause of §17.2.2 that caused it, so that the buck's 675 000 steps can be explained and the cut measured.

**Why P1**: V12 is a suspicion until it is measured; the gate of P1 below compares against this baseline.

**MR-01** (landing reasons)

**Acceptance Criteria**:
1. WHEN a transient completes THEN the solver SHALL report, in its statistics, the number of landings for each reason: a due change read by `analog`, a reaction that probes, the static clause only, a located crossing, a host action, a `bound_step` cap, a breakpoint, the stop time.
2. WHEN a transient completes THEN the solver SHALL report the accepted and rejected step counts as it does today, and the sum of the landing counts SHALL not exceed the accepted step count.
3. WHEN the buck's `startup` run completes in the reference mode THEN the test SHALL print the accepted steps, the rejected steps and each landing count on standard error.

**Independent Test**: a solver test on a fixture with one landing of each reason, each count asserted; the buck figures printed by `buck_long`.

---

### P1: The landings equal what the rule requires ⭐ MVP

**User Story**: As the owner of Part V, I want the solver to land exactly where §17.2.2 rule 2 requires and nowhere else, so that "a clock nothing analog depends on costs nothing" is true.

**Why P1**: V12 suggests the reachability set is coarser than the rule. This story proves or refutes it before any landing is skipped.

**MR-02** (the rule's own instants)

**Acceptance Criteria**:
1. WHEN a scheduled commit to `clk` low is heard only by a reaction on `clk.rise()` that writes a carrier `analog` reads THEN the solver SHALL NOT land at that time (the guarded instant of Appendix D).
2. WHEN a reaction commits with a delay of at least one tick and nothing else at its instant reaches `analog` THEN the solver SHALL NOT land at the instant that made the commit, and SHALL land at the commit's due time when the commit changes a carrier `analog` reads.
3. WHEN a commit due at `(t, 0)` changes a carrier `analog` reads THEN the solver SHALL land on `t`.
4. WHEN a reaction that reads a continuous pin fires at `t` THEN the solver SHALL land on `t`.
5. WHEN a digital net that no reaction reaching the analog side reads changes at `t` THEN the solver SHALL NOT land on `t`.

**Independent Test**: a solver test per criterion on a hand-built device set, counting landings through MR-01.

---

### P1: An edge that changes nothing analog does not cut the step ⭐ MVP

**User Story**: As a designer of a mixed system, I want a clock edge whose reactions leave every analog-read carrier unchanged and probe nothing to cost no analog step, so that a 200 MHz controller does not set the analog step.

**Why P1**: The user's goal: the buck runs much faster.

**MR-03** (the skip)

**Acceptance Criteria**:
1. WHERE the design has no reaction on `settle`, no host waiter and no host probe at `t`, WHEN the instant at `t` runs to quiescence with no analog-read carrier changed and no firing reaction that reads a continuous pin THEN the solver SHALL NOT land on `t`.
2. WHERE the design has a reaction on `settle`, the solver SHALL land on every time point §17.2.2 rule 2 gives, as in the reference mode.
3. WHEN a host waiter or a host probe is scheduled at `t` THEN the solver SHALL land on `t` as in the reference mode (§17.2.2 rule 6, `abi.md` R7).
4. WHEN a commit at an instant that was not landed on changes a carrier `analog` reads at a later microstep of the same instant THEN the solver SHALL land on that instant's time.
5. WHERE the reference mode is selected the solver SHALL land on every time it lands on before this feature.

**MR-04** (the buck)

**Acceptance Criteria**:
1. WHEN the buck's `startup` run (600 µs, `max_step = 1e-8`) completes with multi-rate on THEN the solver SHALL have accepted at most 135 000 steps.
2. WHEN that run completes THEN the test SHALL print the accepted steps, the rejected steps, the landing counts and the wall time on standard error, asserting no time.
3. WHEN that run completes THEN the largest `buck.vout` SHALL be at most 0.815 V, the settling time inside ±10 mV SHALL be between 207.5 and 307.5 µs, and the ripple over the last 40 µs SHALL be greater than 0 and at most 10 mV (`long-run` LR-09 to LR-11, unchanged).
4. WHEN that run completes THEN every record of its `checks` stream SHALL have `pass` true and the last row SHALL hold `buck.ctl.reference` 128, `mode` 2 and `pgood` true.
5. WHEN the same run completes with `max_step` removed THEN the test SHALL print its accepted steps and wall time and SHALL assert the SPEC limits of AC 3.

**Independent Test**: `cargo test -p piperine-api --test buck_long` with both modes, the figures on standard error.

---

### P1: Nothing observable moves ⭐ MVP

**User Story**: As the owner of the parity claim and of Part V, I want the skipped landings to change nothing a model or a host can see except where step control already may, so that the speed is not bought with semantics.

**Why P1**: The brief: parity and every Part V rule preserved.

**MR-05** (exactness)

**Acceptance Criteria**:
1. WHEN the open-loop fixture runs with multi-rate on and in the reference mode THEN every record of every stream SHALL be equal in both runs, field for field, instant for instant.
2. WHEN the open-loop fixture runs in both modes with the same `print_step` THEN each analog cell of the two tables SHALL agree under `ToleranceContract::TRAN_VOLTAGE`.
3. IF an instant run ahead of the analog step turns out to change a carrier `analog` reads THEN the solver SHALL restore that instant's checkpoint and land on its time, leaving no record of the undone run.
4. WHEN a step is rejected after instants ran ahead of it THEN the solver SHALL leave carriers, pending events, instance state and records as they were before those instants (§15 I8, §17.2.3, §19 rule 7).
5. The solver SHALL run instants in increasing order of `(t, n)` in both modes (§15 I3).
6. WHEN a `cross` goes from `≤ 0` to `> 0` in a step THEN the solver SHALL locate and land on it in both modes (§17.3).

**MR-06** (parity)

**Acceptance Criteria**:
1. WHEN each of the 7 twins runs from the command line with the flags of `crates/piperine-cli/tests/twins.rs` THEN the system SHALL print, on each backend, the bytes it printed before this feature.
2. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran`, `parity_grid`, `parity_corpus` and every `buck_*` target of `piperine-parity` with their sources and `contract.rs` unedited.

**Independent Test**: `cargo test -p piperine-solver --test multirate` (the open-loop fixture and the rollback cases), `cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins`.

---

### P2: The reference mode can be selected

**User Story**: As a maintainer bisecting a difference, I want to run a design with today's landings, so that a change caused by multi-rate can be told from any other.

**Why P2**: The P1 tests need it; users can work without it.

**MR-07** (the switch)

**Acceptance Criteria**:
1. WHERE the solver option for the reference mode is set the solver SHALL land as it does before this feature.
2. WHEN the option is absent THEN the solver SHALL use multi-rate.
3. The system SHALL NOT add a run key or a flag for the option (Q4 A).

**Independent Test**: a `piperine-api` test running `tran_rc` and the open-loop fixture with and without the option.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md`, Appendix C and the Book, I want them to describe the landings as they are.

**Why P3**: No behaviour depends on it.

**MR-08** (documents)

**Acceptance Criteria**:
1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list V12 and Appendix C.2's "time points" row SHALL describe the dynamic test and its `settle` exception.
2. WHERE ngspice is absent the parity targets SHALL print the harness's skip line and still run the twins.
3. WHEN this feature merges THEN `ROADMAP.md` SHALL mark multi-rate stepping delivered with its commit.

**Independent Test**: `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test spec_text`.

---

## Edge Cases

- WHEN two edges fall on the same tick and only the second microstep changes an analog-read carrier THEN the solver SHALL land on that tick once.
- WHEN an instant that is not landed on is due exactly at the stop time THEN the solver SHALL land on the stop time, which is a time point (§17.2.2).
- WHEN a `finish` record is written at an instant that was run ahead THEN the solver SHALL land on that instant and end the analysis there (§19 rule 5).
- IF an `IRREVERSIBLE` reaction fires at an instant THEN the solver SHALL land there before running it, as today (`abi.md` R6).
- WHEN a settle commit of the previous time point is due THEN the solver SHALL land as today (§15 S3).
- WHEN `bound_step` caps the step THEN the cap SHALL apply in both modes (§17.2 rule 5).

---

## Gaps closed

| `GAPS.md` id | Requirement(s) | How |
| ------------ | -------------- | --- |
| V12 | MR-01 to MR-06 | MR-01 measures the landings, MR-02 holds them to rule 2, MR-03 skips the ones nothing observes, MR-04 gates the buck at 135 000 steps, MR-05 and MR-06 keep records, Part V and parity unmoved |

---

## Dependencies

- `solver-hygiene` (V7): the look-ahead takes a checkpoint per instant; with every net cloned per mark, the 120 000 edges of the start stay expensive in wall time even when they cost no step. V11 (first-order step after a jump) changes the steps near every remaining landing.
- `host-ii`: `max_steps` as a session option, so the reference-mode baseline of the full start runs at all.
- `long-run` (delivered): the step observer, the streamed grid and `buck_long`, which this feature extends.
- `unknown-semantics` (L52): the MSL form of MR-03 AC 2. Until it lands, AC 2 is proven on a solver test device that declares the `SETTLE` global.

## Size

L: about 14 tasks. Measure (2), the reachability against rule 2 (3), the dynamic skip and its guards (4), rollback and records (2), the buck gate (2), documents (1).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| MR-01 | P1: the solver says why it landed | - | Pending |
| MR-02 | P1: the landings equal what the rule requires | - | Pending |
| MR-03 | P1: an edge that changes nothing analog does not cut the step | - | Pending |
| MR-04 | P1: an edge that changes nothing analog does not cut the step | - | Pending |
| MR-05 | P1: nothing observable moves | - | Pending |
| MR-06 | P1: nothing observable moves | - | Pending |
| MR-07 | P2: the reference mode can be selected | - | Pending |
| MR-08 | P3: the documents say what is true | - | Pending |

**Coverage:** 8 total, 0 mapped to tasks, 8 unmapped (spec stage).

---

## Sources

- The user, 2026-11-17: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps… Não precisa avançar para o design… Preciso que TODOS os gaps estejam endereçados"; earlier, multi-rate is "melhoria futura… bem depois".
- `GAPS.md` §6 V12: 45 000 steps per 40 µs, 675 000 for the start, 130 s.
- `docs/spec/part_v_execution.md` §15 (time points, I3, I6, I8, S3), §17.2 (rule 2, its table, "a digital clock that nothing analog depends on costs the solver nothing"), §17.3.
- `.specs/features/device-abi/abi.md` §5 R4, R6, R7 and amendment 1 (the observable weight of a dynamic rule).
- `crates/piperine-digital-solver/src/plan.rs` (`reachability`), `instant.rs` (the look-ahead test), `advance.rs`.
- `examples/buck/src/dpwm.msl` (`Dpwm` commits every rising `clk`), `src/sense.msl` (the probing ADC on `sample`), `Piperine.toml` (`max_step = 1e-8`).
- `.specs/features/long-run/spec.md` LR-09 to LR-12 (the buck's limits; wall time printed).

---

## Success Criteria

- [ ] The buck's 600 µs start takes at most 135 000 accepted steps with its SPEC limits held and its checks passing.
- [ ] Every landing has a reason that rule 2 requires, and the counts are printed.
- [ ] The open-loop fixture gives the same records in both modes; the 7 twins print the same bytes.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

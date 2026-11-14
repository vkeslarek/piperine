# Solver Completeness Specification

## Problem Statement

The MSL frontend (spec, parser, POM) is done and the solver is the only other crate that builds. The elaborator and codegen that will join them do not exist yet. Before they are written, the solver must offer an engine for every dynamic rule of the MSL specification, and nothing else: today it lacks the MSL instant model (integer ticks, microsteps, typed carriers with `resolve`, the jump solve, settle commits, located crossings), it carries about 3.4k lines that serve only Verilog-AMS, and none of its tests can run because they depend on the dead frontend.

Evidence, row by row: `.specs/features/solver-completeness/analysis.md` (68 rules traced: 15 present, 31 partial, 22 missing; 14 surplus items).

## Goals

- [ ] `cargo test -p piperine-solver` builds and passes with no in-tree dev-dependency.
- [ ] Every row of `analysis.md` section 2 except rows 67 and 68 is `present`, each proven by a solver test that uses hand-written Rust `Element`s only.
- [ ] Every surplus item of `analysis.md` section 3 that the user approves for removal is gone, and the crate has no in-tree dependency.
- [ ] The solver stays an engine: it evaluates no model expression, knows no MSL type, formats no output and reads no file.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The elaborator, codegen, JIT and any change to `piperine-lang` | they are what this feature prepares for; "depois só conectar" |
| Repairing `piperine-codegen`, `piperine-api`, `piperine-python`, `piperine-cli`, `piperine-plugin`, `piperine-lang-server` | they target the deleted frontend; separate work |
| Everything in `analysis.md` section 4 (known-flag arithmetic, symbolic differentiation, `m`/`dtemp` scaling, limiting functions, the stdlib modules, monitors, sweeps, Monte Carlo, Fourier, paths, formatting, Python) | belongs to the language, the elaborator, codegen or the host |
| Sensitivities along a trajectory and across events (§23.3, `E0807` to `E0809`; row 67) | large and only the optimizer needs it; a later feature once a design runs end to end |
| Periodic small-signal and periodic noise (§16.3; row 68) | the spec lists it as unimplemented and untraced (G.37) |
| The C interface of `extern "cosim"` (G.65) | open in the spec; this feature gives the generic wake request only |
| New analyses, new integration methods, performance work | the solver must stay minimal; none is required by a rule |
| Moving `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` to a host crate | no host crate builds, so the move cannot be verified (Q5 in `context.md`) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Where the discrete engine lives (Q1) | the event kernel (ticks, queue, carrier store, commit ordering, microstep loop, checkpoint) lives in the solver; evaluation of definitions, `resolve` and reactions is element code called by the kernel | time, ordering and rollback are engine invariants (§15 I1 to I8) and must be owned and tested in one place | n |
| How the solver stores a carrier value (Q2) | opaque fixed-size bytes per drive and per carrier, laid out by codegen, flags included; a change is a bytewise difference | the solver must not know MSL types; §5.3.4 to §5.3.6 hold bitwise when unknown levels are stored in a canonical form | n |
| Removal of the Verilog-AMS surface (Q3, Q4) | remove S1 to S14 of `analysis.md` section 3, with the tests that cover only them | "o solver tem que ser mínimo"; each item has no MSL construct | n |
| Tests that depend on `piperine-codegen` or `piperine-lang` (Q6) | rewrite the five targets against hand-written `Element`s with the same assertions; drop both dev-dependencies | a core crate that cannot be tested alone is not a core | n |
| Host-built analyses housed in the solver (Q5) | `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` stay; `disto` goes | they need the assembled linearization; §16.3 defines distortion as a transient plus Fourier | n |
| The jump solve (Q7) | Appendix C.2: backward Euler with `h` equal to one tick, time not advanced, repeated once, `1/h` rows scaled by `h` | the spec allows a finite `h` within tolerance and gives this as the reference | n |
| Who decides which scheduled times are time points (Q8) | the elaborator gives one `touches_analog` flag per reaction and per carrier; the kernel applies the dynamic test of §17.2.2 | the static half needs the dependency graph, which the solver does not have | n |
| The residual test's tolerances (Q9) | follow §17.1.2 (node row against the flow `abstol`, potential-branch row against the potential `abstol`) | the spec is the contract; today's code does the reverse | n |
| `piperine-ir` dependency (Q10) | removed; the secant predictor is 20 lines and moves into the solver | the solver becomes a leaf crate | n |
| Size of the feature | six ordered sub-features (`analysis.md` section 6); this document is the umbrella and each sub-feature gets its own design and tasks | 53 open rows, a new kernel and a new solve kind do not fit one task list | n |
| Proof vehicle | every criterion is proven by a solver test built from hand-written Rust `Element`s | no frontend exists to compile a model | y - follows from the brief |
| Numeric defaults | Appendix C.2.1 (tick 1 fs, 1000 microsteps, 20 rounds, `reltol` 1e-3, `trtol` 7, hold 1e12 S) | the spec's reference values; most are already the code's | y - settled by the spec |
| Error identity | a run error carries the spec's code (`E0501` to `E0510`, `E0418`, `E0435`, `E0453`, `E0454`), the instant and the names, as typed fields | §15: "the host receives the code, the instant and the names" | y - settled by the spec |

**Open questions:** Q1 to Q10 are written out with options and a recommendation in `context.md`. They are the user's to answer before this spec is approved.

**Implicit-requirement dimensions.** Input validation & bounds: SCI-02, SCI-08, SCX-03, SCH-04. Failure / partial failure: SCI-07, SCT-06, SCT-07, SCN-02. Idempotency / retry: SCT-09 (a rejected step leaves no trace). Auth and rate limits: N/A because the solver is an in-process library. Concurrency / ordering: SCI-04, SCI-05, SCI-09 (determinism). Data lifecycle: SCT-12 (what a continuation keeps), SCT-11 (what an analysis without time drops). Observability: SCH-05 (records), SCI-07 (errors with names). External-dependency failure: SCH-07 (cosim), otherwise N/A because the solver calls no external service. State-transition integrity: SCT-01 to SCT-05 (the time-point procedure).

---

## User Stories

Roles: the **codegen author** writes the elaborator and device code that will target the solver. The **host author** writes `piperine-api`.

### P1: A solver that can be tested alone and carries nothing MSL lacks ⭐ MVP

**User Story**: As a codegen author, I want the solver's own suite to run with no other crate of the tree, and its surface to hold only what MSL uses, so that I target a small, proven contract.

**Why P1**: Nothing else can be verified until the suite runs. Sub-feature `solver-baseline`.

**Acceptance Criteria**:

1. WHEN `cargo test -p piperine-solver` runs on a tree where `piperine-codegen` does not compile THEN the solver SHALL build every test target and pass.
2. The solver's `Cargo.toml` SHALL list no crate of this workspace under `[dependencies]` or `[dev-dependencies]`.
3. WHEN the five targets that used JIT devices (`adjoint_dc`, `adjoint_vs_sens`, `pss`, `sens`, `transient_reentry`) run THEN each SHALL assert the same quantities as before, on hand-written `Element`s.
4. The public surface (`prelude`, `abi`) SHALL export none of: `Region`, `RegionQueue`, `Strength`, `DriverKind`, `DelayTable`, `PathDelay`, `TimingCheckTable`, `SimOutput`, `FileTable`, `McdTable`, `ValidationReport`, `MarginReport`, `CoverageReport`, `CheckPosture`, `CoverPosture`, `RequireScope`, `DistoSolver`, `Disto2`, `Disto3`, `simprobe`.
5. The `Element` traits SHALL have no method among `discontinuity`, `initial_conditions`, `load_disto2`, `load_disto3`, `validation_reports`, `evaluate_constraints`, `margin_report`, `coverage_report`, `reset_margin_tracking`.
6. WHEN the surface test (`tests/abi_surface.rs`, `tests/prelude_surface.rs`) runs THEN it SHALL fail if a removed name is exported again.

**Independent Test**: on this worktree, with codegen broken, `cargo test -p piperine-solver` is green and the surface tests list the removed names.

---

### P1: Instants, typed carriers and commits ⭐ MVP

**User Story**: As a codegen author, I want an event kernel with integer time, microsteps, typed carriers, drives, `resolve` and commits, so that every `digital` block and every definition of MSL has defined, deterministic execution.

**Why P1**: Every discrete construct of Parts II to IV runs on it. Sub-feature `solver-instants`.

**Acceptance Criteria**:

1. The kernel SHALL keep every scheduled time as a signed 64-bit count of ticks, with the tick length a session option whose default is 1 fs.
2. IF a scheduled time falls outside the 64-bit tick range THEN the kernel SHALL fail the analysis with `E0502` naming what was scheduled.
3. WHEN a commit is made at time `t` with delay `d` THEN the kernel SHALL make it due at the tick nearest `t + d`, a half-tick tie going to the later tick, or at the next microstep when `d <= 0` or the due tick is not later than `t`.
4. WHEN several commits to one level of one drive apply in one microstep THEN the kernel SHALL apply them in the order (instant made, source index, execution index) and keep the last.
5. WHEN a statement makes a commit to a level THEN the kernel SHALL remove the pending commits of the same statement to that level whose due instant is at or after the new one, and no commit of any other statement.
6. WHILE a microstep runs, every reaction SHALL read the carrier values settled at the start of that microstep, and no commit made in it SHALL be visible in it.
7. IF the microstep loop of one time exceeds the session's bound (default 1000) THEN the kernel SHALL fail the analysis with `E0501` naming the carriers that changed in the last microstep.
8. IF a commit is made with a NaN delay THEN the kernel SHALL fail the analysis with `E0435`; a delay of `+inf` SHALL never come due.
9. WHEN the same elements, initial values and host actions are run twice THEN the kernel SHALL produce the same sequence of (instant, carrier, value) changes.
10. The kernel SHALL store each carrier and each drive as a fixed number of bytes given at build time, and SHALL treat a carrier as changed in a microstep exactly when its bytes differ from those of the previous microstep.
11. WHEN any drive of a resolved carrier changes in a microstep THEN the kernel SHALL call that carrier's resolve function with all its drives in the order given at build time and store the result as the carrier's value.
12. WHEN an element asks for the value of a resolved carrier without its own drive THEN the kernel SHALL return the resolve of every other drive.
13. WHEN definitions among resolved drives form a loop THEN the kernel SHALL relax it from the empty state for at most (drive count + 1) passes, and SHALL count only the settled value as a change.
14. WHEN a carrier's value changes THEN the kernel SHALL keep its previous value and make it readable by elements until the next change.
15. WHEN a scheduled event (`at`) is armed with a tick THEN the kernel SHALL fire it once at microstep 0 of that tick, or at the next microstep when the tick equals the current one, and never when the tick is in the past.
16. WHEN a host `set` changes a param during a run THEN the kernel SHALL fire that param's change event at the instant the set applies.
17. IF an element reports a run error while it evaluates (a load, a reaction, a definition) THEN the solver SHALL fail the analysis with that error's code, the instant and the element's instance name.
18. Every run error of the solver SHALL carry its code, its instant `(t, n)` when it has one, and the names involved, as typed fields.

**Independent Test**: a purely discrete design of hand-written elements (the `Front` trace of §15.1, a counter, a pulse written as two commits, a three-drive resolved net with a loop) reproduces the spec's tables instant by instant.

---

### P1: The mixed time point ⭐ MVP

**User Story**: As a codegen author, I want the time-point procedure of §15 and §16 (left value, microstep loop with jump solves, settle, operating-point rounds, held nodes, sweep points, continuation), so that an `analog` block that reads a carrier and a reaction that probes a node meet as the spec says.

**Why P1**: It is the coupling itself; the spec's conformance set (`transition`, `zi_nd`, `inertial`) depends on it. Sub-feature `solver-time-point`.

**Acceptance Criteria**:

1. WHEN the transient reaches a time point `t` THEN the solver SHALL solve the analog system with the discrete state of before `t` first, and only then apply what is due at `(t, 0)`.
2. WHEN a carrier or param that an analog element reads changes at `(t, n)` to a value the last solve had not read THEN the solver SHALL perform a jump solve at unchanged `t` before microstep `n + 1` reacts.
3. WHILE a jump solve runs, every charge-like state SHALL be discretized by backward Euler over one tick from its value at the previous solve of the instant, and the solve SHALL be repeated once from its own result.
4. WHEN a 1 pF capacitor at 1 V is shorted to a 3 pF capacitor at 0 V by a carrier change THEN both SHALL read 0.25 V after the jump within the convergence tolerance.
5. WHEN a jump solve ends THEN the integrator SHALL drop the history and error estimate of exactly the states whose value or derivative moved by more than the convergence tolerance, and SHALL keep those of every other state.
6. IF a jump solve does not converge THEN the solver SHALL fail the analysis with `E0504` naming the instant and the carriers or params that changed.
7. IF the rounds of an operating point exceed the session's bound (default 20) THEN the solver SHALL fail the analysis with `E0506` naming the carriers still changing.
8. WHEN a scheduled time has no due commit that changes a carrier an analog element reads, and no firing reaction flagged as touching the analog side THEN the solver SHALL run its instants without an analog solve and SHALL NOT end a step there.
9. WHEN a transient step is rejected THEN carriers, pending commits, armed events, element state and the analog history SHALL equal their values before the step.
10. WHEN the microstep loop of a time point exits THEN the solver SHALL run every settle reaction once, with the final discrete state and the final solution.
11. WHEN a settle reaction commits with no advancing delay at time point `t` THEN the commit SHALL be due at microstep 0 of the next time point, the solve that arrives there SHALL already read its value, and no jump solve SHALL run for it.
12. IF the step to the next time point is rejected THEN a pending settle commit SHALL move to the new next time point.
13. WHILE the operating point of a transient is solved, every node given a held value at build time SHALL be tied to it by a potential source to ground, removed before the first step.
14. WHEN a `Dc` analysis runs several sweep points THEN the discrete state SHALL carry from one point to the next, initial values SHALL be committed at the first point only, and settle reactions SHALL run once per point.
15. WHEN an analysis without time ends THEN every commit and scheduled event that was waiting for a later time SHALL be dropped.
16. WHEN a transient is continued to a later stop time THEN time, the event queue, carriers, element state and the analog history SHALL be those of its last instant, and no initial value SHALL be committed again.
17. WHEN a `Dc`, `Ac` or `Noise` analysis runs at a transient's last instant THEN elements SHALL read that instant as the current time, Newton SHALL start from the transient's final solution, and the queue SHALL be unchanged afterwards.
18. WHILE any solve runs, an element SHALL be able to read the analysis kind (`Dc`, `Tran`, `Ac`, `Noise`) and the kind of solve (equilibrium, transient step, jump, small-signal).

**Independent Test**: hand-written `Switch`, `SampleHold`, charge-sharing pair and a settle-commit element reproduce the traces of §15.1, §15.2 and Appendix F.5; a divider clock that no analog element reads adds no time point.

---

### P2: Located crossings and `bound_step`

**User Story**: As a codegen author, I want the solver to land on a crossing and to honor a step cap, so that `cross`, `above` and every converter built on them fire at the right time.

**Why P2**: `cross` is the only analog-to-digital path of MSL, and it works coarsely today (it fires one step late). Sub-feature `solver-crossings`.

**Acceptance Criteria**:

1. WHEN a crossing expression goes from `<= 0` to `> 0` inside a step THEN the solver SHALL retake the step until the landing time exceeds the true crossing by at most `ttol` plus one tick, or `0 < e < etol` holds there.
2. WHEN a crossing is located THEN its landing time SHALL be a tick, a time point, and its event SHALL fire at microstep 0 of it.
3. IF a crossing is registered with a `ttol` or `etol` that is not finite and positive THEN the solver SHALL fail the analysis with `E0510`.
4. WHEN several crossings fall in one step THEN the solver SHALL land on the earliest, and those that round to the same tick SHALL fire together.
5. WHEN a jump solve, a new round of an operating point or the next sweep point moves a crossing expression from `<= 0` to `> 0` THEN its event SHALL fire at the next microstep, or at microstep 0 of the later sweep point.
6. WHEN an element reports a step cap at an accepted solution THEN the next step SHALL be at most the smallest cap reported there.
7. IF an element reports a step cap that is not finite and positive THEN the solver SHALL fail the analysis with `E0509`.

**Independent Test**: a ramp through a threshold lands within `ttol` plus one tick for several step sizes; a sine with a 1 ns cap never takes a longer step.

---

### P2: The host seam

**User Story**: As a host author, I want to drive carriers, force and release, act at scheduled times, receive records and stop on `finish`, so that a testbench of §23.1 can be written over the solver.

**Why P2**: Without it a design can run but not be driven or observed as the spec states. Sub-feature `solver-host-seam`.

**Acceptance Criteria**:

1. WHEN the host writes a drive it holds before the run THEN the value SHALL be committed at `(0, 0)` as an initial value.
2. WHEN the host schedules a drive, a `set`, a `force` or a `release` at a time THEN it SHALL enter that instant with the commits collected there.
3. WHILE a carrier is forced, its value SHALL be the forced value whatever its drives, and its drives SHALL go on updating underneath.
4. IF the host schedules an action at a time already past THEN the solver SHALL refuse it with `E0806` and change nothing.
5. WHEN the host schedules a probe at a time THEN that time SHALL be a time point and the host SHALL receive the solution there.
6. WHEN an element writes a record from a reaction in a step that is later rejected THEN the record SHALL not reach the sink; a record written from an analog evaluation SHALL reach it.
7. WHEN a record flagged `finish` is accepted THEN the analysis SHALL end at its instant, which SHALL be its last time point.
8. The record sink SHALL deliver, with each record, the instance name, the analysis kind, the instant, and inside an analog evaluation the Newton iteration.
9. WHEN an element asks for a session parameter by name THEN the solver SHALL return the value the host set, or report that there is none.
10. WHEN a discrete element asks to be run again at a time THEN that time SHALL be a scheduled time, and an element flagged as never rolled back SHALL be run only at instants of accepted steps.

**Independent Test**: a hand-written UART receiver is driven bit by bit from a test acting as the host, with a forced reset, and the host receives the `byte` record and a `finish`.

---

### P3: Numeric conformance

**User Story**: As a codegen author, I want tolerances per nature, named singular solves, a per-state truncation test, switch branches and correlated noise, so that the remaining rows of §11, §14 and §17 hold.

**Why P3**: The engine runs without them; results are within today's global tolerances. Sub-feature `solver-numerics`.

**Acceptance Criteria**:

1. The update test SHALL use, for each unknown, the absolute tolerance given for it at build time (potential for a node, flow for a branch).
2. The residual test SHALL use, for each row, the absolute tolerance given for it at build time (flow for a node row, potential for a potential-branch row).
3. IF a linear solve is singular THEN the solver SHALL fail with `E0503` naming the unknowns of the rows that make it so.
4. WHERE an element declares a state with its own absolute tolerance, the truncation-error test SHALL measure that state against it.
5. WHEN an element stamps a different set of matrix positions in one solve than in another, both sets having been declared at build time, THEN each solve SHALL factor correctly.
6. WHEN one noise source is injected at several rows with complex weights THEN the output density SHALL be that of one fully correlated source.
7. WHEN a noise source is injected on the row of a potential branch THEN the output density SHALL include it.

**Independent Test**: a `Nullor` with a 1 µV servo nature converges to that tolerance; a floating node reports its name; a resistor split in two with one shared noise instance gives zero noise across the pair.

---

## Edge Cases

- WHEN a design has no analog unknown THEN the solver SHALL still run the operating point, every scheduled instant and the stop time, with settle reactions at the operating point and at the stop time.
- WHEN a design has no discrete carrier THEN the transient SHALL take the same steps as before this feature within tolerance.
- IF a commit is due at a time later than the stop time THEN it SHALL stay pending and SHALL be applied only if the transient is continued.
- WHEN two event times round to the same tick THEN they SHALL be one instant.
- WHEN a settle commit is made at the stop time and the transient is not continued THEN it SHALL be dropped.
- IF a hold closes a loop of potential sources THEN the solve SHALL fail with `E0503` naming both.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SCB-01 | P1: Baseline, AC 1 to 3 (suite runs alone) | solver-baseline | Pending |
| SCB-02 | P1: Baseline, AC 4 to 6 (surplus removed, guarded) | solver-baseline | Pending |
| SCI-01 | P1: Instants, AC 1, 3 (ticks, rounding) | solver-instants | Pending |
| SCI-02 | P1: Instants, AC 2, 8 (`E0502`, `E0435`) | solver-instants | Pending |
| SCI-03 | P1: Instants, AC 10, 14 (carrier store, change, previous value) | solver-instants | Pending |
| SCI-04 | P1: Instants, AC 4, 5 (commit order, transport) | solver-instants | Pending |
| SCI-05 | P1: Instants, AC 6 (snapshot) | solver-instants | Pending |
| SCI-06 | P1: Instants, AC 11, 12, 13 (resolve, others, relaxation) | solver-instants | Pending |
| SCI-07 | P1: Instants, AC 7, 17, 18 (`E0501`, element errors, typed errors) | solver-instants | Pending |
| SCI-08 | P1: Instants, AC 15, 16 (scheduled events, param change) | solver-instants | Pending |
| SCI-09 | P1: Instants, AC 9 (determinism) | solver-instants | Pending |
| SCT-01 | P1: Time point, AC 1 (left value) | solver-time-point | Pending |
| SCT-02 | P1: Time point, AC 2, 3, 4 (jump solve) | solver-time-point | Pending |
| SCT-03 | P1: Time point, AC 5 (restart per state) | solver-time-point | Pending |
| SCT-04 | P1: Time point, AC 8 (event times that are not time points) | solver-time-point | Pending |
| SCT-05 | P1: Time point, AC 10, 11, 12 (settle, settle commit) | solver-time-point | Pending |
| SCT-06 | P1: Time point, AC 6 (`E0504`) | solver-time-point | Pending |
| SCT-07 | P1: Time point, AC 7 (`E0506`) | solver-time-point | Pending |
| SCT-08 | P1: Time point, AC 13 (held nodes) | solver-time-point | Pending |
| SCT-09 | P1: Time point, AC 9 (rejection leaves no trace) | solver-time-point | Pending |
| SCT-10 | P1: Time point, AC 14 (sweep points) | solver-time-point | Pending |
| SCT-11 | P1: Time point, AC 15 (what waits for time is dropped) | solver-time-point | Pending |
| SCT-12 | P1: Time point, AC 16, 17 (continuation) | solver-time-point | Pending |
| SCT-13 | P1: Time point, AC 18 (analysis and solve kind) | solver-time-point | Pending |
| SCX-01 | P2: Crossings, AC 1, 2, 4 (location) | solver-crossings | Pending |
| SCX-02 | P2: Crossings, AC 5 (across jumps, rounds, sweep points) | solver-crossings | Pending |
| SCX-03 | P2: Crossings, AC 3, 6, 7 (`E0510`, step cap, `E0509`) | solver-crossings | Pending |
| SCH-01 | P2: Host seam, AC 1, 2 (drives and scheduled actions) | solver-host-seam | Pending |
| SCH-02 | P2: Host seam, AC 3 (force, release) | solver-host-seam | Pending |
| SCH-03 | P2: Host seam, AC 5 (scheduled probe) | solver-host-seam | Pending |
| SCH-04 | P2: Host seam, AC 4 (`E0806`) | solver-host-seam | Pending |
| SCH-05 | P2: Host seam, AC 6, 7, 8 (records, `finish`) | solver-host-seam | Pending |
| SCH-06 | P2: Host seam, AC 9 (session parameters) | solver-host-seam | Pending |
| SCH-07 | P2: Host seam, AC 10 (wake request, no rollback) | solver-host-seam | Pending |
| SCN-01 | P3: Numerics, AC 1, 2 (tolerances per nature) | solver-numerics | Pending |
| SCN-02 | P3: Numerics, AC 3 (`E0503` named) | solver-numerics | Pending |
| SCN-03 | P3: Numerics, AC 4 (truncation per state) | solver-numerics | Pending |
| SCN-04 | P3: Numerics, AC 5 (switch-branch pattern) | solver-numerics | Pending |
| SCN-05 | P3: Numerics, AC 6, 7 (correlated and potential noise) | solver-numerics | Pending |

**Coverage:** 39 total, 0 mapped to tasks, 39 unmapped. Tasks are written per sub-feature after this spec is approved.

---

## Sources

- `docs/spec/part_v_execution.md` §15 to §18: the time-point procedure, the jump, the operating point, convergence, steps, crossings, limiting.
- `docs/spec/part_iv_behavior.md` §11 to §14: contributions, switch branches, reactions, commits, events, core primitives.
- `docs/spec/part_ii_carriers.md` §4, §5 and `part_iii_structure.md` §8.2.3, §8.5: drives, `resolve`, history, relaxation, extern modules.
- `docs/spec/part_vi_io.md` §19, §21 and `part_viii_host.md` §23: records, metadata, host actions.
- `docs/spec/appendix_c_implementation.md` C.2: the reference algorithms and defaults; `appendix_g_scope.md` G.0, G.6: what is excluded and what the implementation lacks.
- `.specs/features/solver-completeness/analysis.md`: the traceability matrix, the surplus list, the split.
- `.specs/STATE.md` MD-01 (one `Element` ABI), MD-17 (`prelude` + `abi`), MD-18 (no re-JIT during simulation), MD-31 (an invariant lives in a gate).

---

## Success Criteria

- [ ] `cargo test -p piperine-solver` is green on a tree where no other crate compiles.
- [ ] The spec's conformance set (C.2.3) can be written as hand-built elements and passes: the trace of §15.1, `transition`'s commit pattern, `zi_nd`'s settle commits, `inertial`'s re-armed `at`, a relaxed loop of `Tran` on `Wire`s.
- [ ] A design with a free-running digital clock that no analog element reads takes the same number of analog steps with and without the clock.
- [ ] The crate has no in-tree dependency and fewer source lines than at e129b869 after the baseline sub-feature.

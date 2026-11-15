# Solver ABI Integration Specification

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

Last of four features that make the solver adopt the device ABI (`.specs/features/device-abi/abi.md`, the authority): `solver-abi-core`, then `solver-abi-digital` and `solver-abi-analog` in parallel, then `solver-abi-integration` (this one). It starts when the two parallel branches are merged.

## Problem Statement

After the three features before it, the pieces exist and nothing uses them: `piperine-abi`, the protocol on stubs, a digital engine and an analog assembly, each proven alone. The solver still runs every device through `Element`, with hand-made companions, device checkpoints, a four-state logic snapshot and a strength table. This feature makes the owner bind device instances, point its drivers at the assembly, run the protocol over the two real sub-solvers, port every surviving test to `Device`, and delete the old contract.

The numbers must not move (user: "a paridade com ngspice é um strenght bem alto"). While both contracts exist a differential test holds them equal; a ported test keeps its name, its quantities and its tolerances.

## Goals

- [ ] `piperine-solver` binds device instances to nets and unknowns and its drivers (DC, transient, AC, noise, sensitivities, PSS, tf, pz, sp) take their stamps from the analog assembly.
- [ ] The protocol of `solver-abi-core` runs over the real sub-solvers through two adapters.
- [ ] Every surviving solver test runs on `Device`s with its numeric assertions and tolerances unchanged; a recorded differential run holds `Device` and `Element` circuits within 1e-12 relative with equal step counts.
- [ ] `Element`, the capability flags, device checkpoints, the strength table, four-state `LogicValue`, the `seq`/`comb` contract and the shared digital snapshot are gone.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| A jump solve, a change of the tolerance mapping, a new integration scheme or step controller | user, Q7 and Q9: the present algorithm and mapping stay |
| New logic in the assembly, the digital engine or the protocol | built and proven by the three features before; a defect found here is fixed where it lives, with a test there |
| Locating a crossing inside a step, `E0509`, `E0510`, crossings across rounds and sweep points | `solver-crossings`; a `CROSS` watch fires at the first accepted solution after the sign change, as the solver does today |
| The settle commit, discrete state carried across `Dc` sweep points, dropping what waits for time, continuing a transient | `solver-time-point` |
| `force` and `release`, host drives of ports, scheduled probes as an API, `E0806` | `solver-host-seam` |
| Per-row residual tolerances, named singular solves, truncation per `ddt` state | `solver-numerics` |
| Tolerance-based stamp bypass (`BYPASS_OK`) | replaced by F3; it was opt-in and off by default |
| `dlopen`, the OSDI adapter, the AVR emulator; rebuilding `piperine-api`, `piperine-cli`, `piperine-lang-server` | not this feature; the kept crates do not compile today and still will not |
| Integration tests and ngspice parity runs | user: unit tests only until the solver is connected to the language |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Numerics | Newton, TR-BDF2, Milne, the PI controller, the convergence plan and the tolerance mapping are not edited; `assess_step`'s arithmetic and `propose_dt`'s controller call keep their bodies | user: "eu manteria o algoritmo atual do solver que funcionou bem nos testes"; "a paridade com ngspice é um strenght bem alto" | y |
| What "unchanged" means | every numeric assertion and tolerance of a surviving test passes as written, and the differential test holds `Device` and `Element` circuits within 1e-12 relative with equal step counts. Bit identity is not promised | the caller's brief: "numeric assertions/tolerances unchanged" | y |
| The two restart rules | after a host write to a `PARAM` or `GLOBAL` net inside a transient the next step is 10⁻³ of the last, floored at the minimum step; after any other boundary change the controller's proposal stands and only the integrator restarts | `transient.rs::propose_dt` (`sets_just_applied`) today; `live_params` asserts it | y - present behavior |
| Limiting | the ABI form only; today's "apply the limited value to the guess" (`apply_limiting_reports`) is removed with `LimitingReport` | abi.md A2, A3; no surviving numeric baseline uses a limiter | y - settled by the ABI |
| Sources that have kinks | a pulse or clock source is a device with a timer reaction and a discrete phase pin its analog function reads; the landing comes from the time-point test. `next_breakpoints` is removed | abi.md §1 "timer, breakpoint: a commit with a delay to a private event pin" | y - settled by the ABI |
| Introspection | params come from the prototype's `PARAM` pins, operating-point variables from `VAR` watches; `Introspect`, `Invalidation` and the capability flags are removed; a `set` is a host commit checked on the cone | abi.md §1 "Not concepts" | y - settled by the ABI |
| Tests whose subject is removed | removed by name, listed in `removed.txt` with the removed item; everything else is ported with its name, quantities and tolerances | user, Q3: "o que precisar apagar, pode apagar"; the caller's brief: "DELETE strength table, LogicValue four-state, seq/comb contract, shared snapshot" | y |
| Strategy | strangler: a circuit is built from `Element`s or from `Device`s, never both; one driver stack branches until every test is ported, then the `Element` branch is deleted | each batch leaves the crates green; the differential test needs both | y - agent's discretion (structure) |
| Who models, who verifies | Sonnet implements and Opus verifies by default; the orchestrator decides otherwise per batch. This feature is large (25 tasks) and numerically and protocol delicate in Phases 1 to 3 | user, 2026-11-15, through the coordinator | y |

**Open questions:** none blocking.

**Implicit-requirement dimensions.** Input validation & bounds: SAI-01 (bind). Failure / partial failure: SAI-04, SAI-08. Idempotency / retry: SAI-04 (a rejected step leaves no trace). Auth and rate limits: N/A because it is an in-process library. Concurrency / ordering: SAI-07 (the sink's order). Data lifecycle: SAI-08 (re-create on `RESHAPE`). Observability: SAI-07. External-dependency failure: device faults, typed by the sub-solvers and passed through (SAI-03). State-transition integrity: SAI-03, SAI-04, SAI-05.

---

## User Stories

### P1: Bind, drivers and the protocol over the real sub-solvers ⭐ MVP

**User Story**: As the owner of events, I want to bind instances to nets, take stamps from the assembly and run the protocol over the two real sub-solvers, so that a design of `Device`s runs every analysis and the two sides meet only through boundary events.

**Why P1**: it is the seam of the user's vision: "praticamente como dois solvers distintos que rodam no mesmo solver".

**Acceptance Criteria**:

1. WHEN an instance is added THEN its `PORT` pins SHALL bind by position, its `PARAM` pins by name to a net seeded with the value given or the prototype's default, its `GLOBAL` pins by constant or by name to one design-wide net, and its `PRIVATE` pins to nets the solver allocates.
2. WHEN continuous `PORT` pins are bound to one net THEN they SHALL share one unknown, and each `PRIVATE` continuous pin SHALL be an unknown of its own.
3. WHEN a continuous pin has `init` THEN it SHALL be Newton's starting value and SHALL be held through the operating point of a transient, as the solver's present hold does.
4. WHEN a continuous pin has a non-zero `abstol` for its unknown THEN the node SHALL use the smallest such value in the update and residual tests, through the solver's present per-node override.
5. WHEN a discrete instant changes nothing that an analog function reads THEN the analog sub-solver SHALL NOT end a step at its time.
6. WHEN a commit due at microstep 0 of a landing changes a discrete pin an analog function reads THEN the step that arrives there SHALL be solved with the new bytes, SHALL be exempt from the truncation test, and the next step SHALL restart the integrator.
7. WHEN a transient step is rejected, or lands earlier than proposed THEN the discrete store, the queue, the sink buffer and every history SHALL equal their values at the last accepted time, and the discrete instants up to the new landing SHALL be replayed.
8. WHEN an `IRREVERSIBLE` reaction or a host waiter is due THEN the owner SHALL have the analog solution accepted up to its instant before running it, and SHALL run it once.
9. WHEN a reaction returns `FINISH` THEN the analysis SHALL end after that instant, which SHALL be an accepted analog time.
10. WHEN a design has no continuous pin THEN the owner SHALL run every scheduled instant and the stop time with no analog solve.
11. WHEN a design has no discrete pin read by an analog function THEN a transient SHALL take the same accepted steps as the same design on `Element`s.
12. WHEN an operating point of a mixed design is solved THEN a comparator, a flip-flop and a DAC wired in a loop SHALL settle to the consistent point, with `SETTLE` fired once.
13. IF the same loop is wired as an oscillator THEN the operating point SHALL fail with `E0506` naming its nets.
14. WHEN a sink is installed on the solver THEN it SHALL receive, in instant order, every change, commit, fired crossing and record of a real mixed run with its instance path, analysis kind and time, and every `TRACE` record with its Newton iteration.
15. WHEN an instant is undone THEN its records SHALL NOT reach the sink.
16. WHEN the host sets a param THEN the owner SHALL commit it only after the shadow run accepts it; a false `check` SHALL refuse the write.
17. IF a shadow run returns `RESHAPE` inside a transient THEN the owner SHALL refuse the write; between analyses or at a sweep point it SHALL destroy, create and bind that instance again and then commit.
18. WHEN the host writes a `PARAM` or `GLOBAL` net at a scheduled time inside a transient THEN that time SHALL be a landing and the next step SHALL be 10⁻³ of the last, floored at the minimum step.
19. WHEN an analysis runs THEN the `GLOBAL` nets `TEMPERATURE`, `RESOLUTION`, `DC`, `TRAN`, `AC` and `NOISE` SHALL hold the session's values, and a `NAMED` global SHALL hold the host's session parameter of that name.
20. WHEN `sens` or `adjoint` asks for the derivative of a row with respect to a param THEN it SHALL use the declared partials against the param's pin, chained through the partials of discrete functions, and SHALL fall back to a finite difference by re-evaluation where no declared path exists.
21. WHEN a circuit is built from devices THEN the DC, transient, AC and noise drivers SHALL take every stamp, the limit veto, the noise columns and the stimulus from the analog assembly, with the homotopy's continuation values supplied as `GMIN` and `SOURCE_SCALE`.
22. WHEN a solution is accepted THEN the next step SHALL be at most the smallest `CAP` the assembly reports, and every fired `CROSS` SHALL be committed to its event pin at microstep 0 of that landing.

**Independent Test**: a free-running timer that nothing analog reads leaves the step count of an RC transient unchanged; the same timer driving a switch makes the transient land on each edge; a comparator, a flip-flop and a DAC close the loop through the sink's records.

---

### P1: Every surviving test on `Device`, and the old contract removed ⭐ MVP

**User Story**: As a maintainer, I want the whole solver suite on the new contract with the same numbers, and the old contract deleted, so that there is one way to write a device.

**Why P1**: two contracts in one solver is the state this feature exists to end.

**Acceptance Criteria**:

1. WHEN a test whose subject survives runs after the port THEN it SHALL pass with its name, its asserted quantities and its tolerances unchanged.
2. WHEN a test is removed THEN its name and the removed item it was about SHALL be listed in `removed.txt`.
3. WHILE both contracts exist, the same circuit built from `Element` doubles and from their `Device` ports SHALL give DC, transient, AC, noise and sensitivity results equal within 1e-12 relative, and equal accepted step counts.
4. WHEN the partials verifier runs on any function of any test device THEN it SHALL report no mismatch.
5. The solver crates SHALL contain none of `Element`, `AnalogDevice`, `DigitalDevice`, `Introspect`, `ElementCapabilities`, `ElementCheckpoint`, `LimitingReport`, `Invalidation`, `Strength`, `DriverTable`, `LogicValue`, `seq_phase`, `comb_phase`, `DigitalState`, `EventQueue`, `next_breakpoints`.
6. The analog sub-solver's sources SHALL name no item of the digital sub-solver, and the reverse.
7. The three solver crates and `piperine-abi` SHALL build with zero warnings.

**Independent Test**: `cargo test -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver -p piperine-abi` is green; `parity_baseline` passes with its literals untouched.

---

## Edge Cases

- IF a circuit is built from both `Element`s and `Device`s THEN `build` SHALL fail.
- IF a commit is due later than the stop time THEN it SHALL stay pending when the transient ends.
- IF a host write targets a `STRUCTURAL` param during a run THEN it SHALL be refused and nothing SHALL change.
- WHEN the transient's last landing is the stop time THEN `SETTLE` SHALL fire there.
- WHEN a design has no discrete pin at all THEN the digital side SHALL be an empty plan and SHALL cost no call per step.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SAI-01 | P1: Protocol, AC 1 to 4 (bind) | Tasks | In Tasks |
| SAI-02 | P1: Protocol, AC 5, 10, 11 (which instants cut a step) | Tasks | In Tasks |
| SAI-03 | P1: Protocol, AC 6 (boundary delivery, restart) | Tasks | In Tasks |
| SAI-04 | P1: Protocol, AC 7 (reject leaves no trace) | Tasks | In Tasks |
| SAI-05 | P1: Protocol, AC 8, 9 (irreversible, host waiter, `FINISH`) | Tasks | In Tasks |
| SAI-06 | P1: Protocol, AC 12, 13 (operating point of a mixed design, `E0506`) | Tasks | In Tasks |
| SAI-07 | P1: Protocol, AC 14, 15 (the sink) | Tasks | In Tasks |
| SAI-08 | P1: Protocol, AC 16, 17, 18 (host set, `RESHAPE`) | Tasks | In Tasks |
| SAI-09 | P1: Protocol, AC 19 (globals, session params) | Tasks | In Tasks |
| SAI-10 | P1: Protocol, AC 20 (sensitivities) | Tasks | In Tasks |
| SAI-11 | P1: Protocol, AC 21, 22 (drivers on the assembly, step caps and crossings) | Tasks | In Tasks |
| SAI-12 | P1: Port, AC 1, 2 (ported and removed tests) | Tasks | In Tasks |
| SAI-13 | P1: Port, AC 3 (differential) | Tasks | In Tasks |
| SAI-14 | P1: Port, AC 5, 6 (removed names, boundary) | Tasks | In Tasks |
| SAI-15 | P1: Port, AC 4 (partials verified) | Tasks | In Tasks |
| SAI-16 | P1: Port, AC 7 (zero warnings) | Tasks | In Tasks |

**Coverage:** 16 total, 16 mapped to tasks, 0 unmapped. With the three features before it, this closes the umbrella's SCE-01 to SCE-04, SCI-01 to SCI-09, SCT-05 AC 1, SCT-07, SCT-08, SCT-09, SCT-13, SCX-03 AC 6, SCH-05, SCH-06, SCH-07, SCN-04, SCN-05.

---

## Sources

- `.specs/features/device-abi/abi.md`: §3.1 (P5, P6), §5, §9, §10, §11.
- `.specs/features/solver-abi-core/` (the protocol and its traits), `solver-abi-digital/` (`Digital`), `solver-abi-analog/` (`Assembly`): what this feature wires.
- `.specs/features/solver-completeness/spec.md` and `context.md`: Q7, Q8, Q9, rounds 1 to 3.
- `crates/piperine-solver/src/analyses/{dc,transient,ac,noise,sens,adjoint,sensitivity,pss,tf,pz,sp}.rs`, `core/{element,circuit,builder,introspect}.rs`, `tests/`: the present behavior, the doubles and the tests to port.
- `docs/spec/appendix_c_implementation.md` C.2, C.2.1: the defaults.
- The coordinator's message of 2026-11-15 (user): the split and the model roles.
- `.specs/STATE.md` MD-17, MD-18, MD-28, MD-31, MD-60, MD-61.

---

## Success Criteria

- [ ] The four crates' suites are green with zero warnings, and `parity_baseline`'s numeric literals and tolerances are byte-identical to their text at 98b30ad6.
- [ ] `grep -rnE "\b(Element|LogicValue|DriverTable|Strength|seq_phase|comb_phase)\b" crates/piperine-solver/src crates/piperine-analog-solver/src crates/piperine-digital-solver/src` matches nothing.
- [ ] A design with a free-running digital clock that no analog function reads takes the same number of analog steps with and without the clock.
- [ ] `removed.txt` names every removed test and the item it was about.

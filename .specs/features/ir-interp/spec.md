# IR Interpreter (reference backend) Specification

**Status**: Draft. Not approved. Open questions INT-Q1 to INT-Q7 block approval.

## Problem Statement

After `msl-elab` an MSL design exists as an IR, and nothing executes it. The retired path compiled straight to machine code through Cranelift, which was hard to debug and to verify (context.md "Why"). The chain needs a backend that runs an IR directly, with no compiler in the environment, whose behavior is easy to read and to trust. That backend is the reference: the debug mode, and the oracle every fast backend is compared against (C6).

## Goals

- [ ] An elaborated design runs on the solver from its IR alone, with no `rustc` and no compilation step (C6 "always available").
- [ ] The interpreter gives every construct the behavior `docs/spec/` defines for it, per kind of solve (§16.2) and per instant (§15).
- [ ] A run-phase error names its code and the instance path (§0.3 items 3 and 4, §1 rule 11).
- [ ] A differential harness runs one IR on the interpreter and on another backend and reports the first difference (C6).
- [ ] This is the first end-to-end MSL to solver path (O6 proposal), so integration tests can exist again.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The IR model and its construction | features `msl-ir` and `msl-elab` |
| The Rust transpiler | feature `ir-rust` |
| The device ABI: device shape, state, events, small-signal, noise, introspection, external devices, versioning | ABI duel (C8, O8); the interpreter implements it and does not design it |
| Time, ticks, the event queue, the carrier store, commit ordering, the microstep loop, checkpoint and rollback | owned by the solver (C9; solver-completeness context Q1) |
| Newton, step control, crossings location, limiting algorithms, the jump solve | solver (Part V, Appendix C.2) |
| The history and state of `ddt`, `delay`, `limit` and `noise` | §14 rule 5: their value inputs "are read by the solver"; the split of state is the ABI's |
| `extern "osdi"` and `extern "cosim"` devices | they are other devices under the ABI, not interpreted IR |
| Interpreter speed work (bytecode, caching of evaluation plans) | the interpreter is the reference, not the fast path (C6); see INT-Q4 |
| Re-enabling the ngspice parity targets | follows the first end-to-end run; planned separately (solver-completeness context, round 2: "Tests") |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision or by `docs/spec/`. Rows marked `n` are unconfirmed proposals with an entry under "Open questions (detail)".

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The interpreter is the reference backend | always available, the debug mode, the differential oracle | C6 | y |
| It executes the IR, not the POM | input is an IR value | C3 | y |
| The solver owns events and time; devices evaluate | the interpreter evaluates definitions, `resolve`, reactions and `analog` when the solver calls it | C9; solver-completeness context Q1, user: "pode ser modelado como solver o dono dos eventos" | y |
| Carriers cross the boundary as opaque bytes | the interpreter encodes levels and known flags into the byte layout the IR fixes, unknown levels in a canonical form | C9; solver-completeness context Q2, user: "Prefiro bytes também" | y |
| Value semantics | Rust edition 2024 value semantics with the deltas of Part I | §0.4 | y |
| How the Jacobian is computed (INT-Q1, O1) | forward-mode dual numbers while evaluating `analog` | proposal of O1; the language requires only that everything is differentiated with respect to every probe (§11 rule 5) | n |
| What "debug mode" contains (INT-Q2) | backend selection plus an evaluation trace keyed by instance path; no stepping debugger in the first version | C6 names the mode and no content | n |
| How two backends are compared (INT-Q3) | discrete carriers, records and event order bit for bit; analog values within the session's convergence tolerances | not decided | n |
| Whether the interpreter is the default backend for small runs (INT-Q4, O5) | undecided here; the choice is a host option | proposal of O5 | n |
| How the interpreter meets the ABI (INT-Q5, O8) | one generic device type that holds an IR handle and implements the ABI for any module | depends on the duel | n |
| Crate (INT-Q6) | a new crate `piperine-interp` depending on the IR crate and on the ABI crate | no decision recorded | n |
| Arbitrary-width integers and `Float<16>`/`Float<128>` (INT-Q7) | exact semantics in the interpreter from the first version | §1 rules 5 and 6; affects the oracle's authority | n |
| Who implements and who verifies (C10) | Opus implements, Fable verifies | whether the interpreter counts as "core" is not stated | n |

**Open questions:** INT-Q1 to INT-Q7 are open and listed below. This spec is not ready for approval.

**Implicit-requirement dimensions.** Input validation and bounds: INT-01 AC 3 (an IR that fails `check()`), INT-05. Failure states: INT-05 (panic, non-finite contribution, run errors). Idempotency: INT-02 AC 4 (re-evaluation with the same inputs gives the same outputs). Auth and rate limits: N/A because the interpreter is an in-process library. Concurrency and ordering: INT-04 AC 3 (no dependence on evaluation order, I1). Data lifecycle: N/A because state lifetime is fixed by the ABI (INT-Q5). Observability: INT-05 AC 3, INT-07 and INT-Q2. External-dependency failure: N/A because the interpreter has no external dependency; that is its purpose. State-transition integrity: INT-04 AC 4 (a rejected step leaves no trace, I8).

**Open questions (detail)**

- **INT-Q1 (O1). Jacobian.** Options: (a) forward-mode duals: `analog` is evaluated over a number type that carries a value and its partials with respect to the probes; (b) symbolic differentiation on the IR, the interpreter then evaluates derivative expressions. Recommendation: (a), the proposal of O1: it removes a stage and is exact to machine precision. Two language constraints hold either way: `ddx` is defined over probe leaves, with other branches held constant and `limit` transparent (§14.2.3); the small-signal sweep needs the resistive and the reactive part of each contribution separately (§16.2, `ddt` is `jω·x`).
- **INT-Q2. Debug mode.** C6 calls the interpreter "the debug mode" without content. Options: (a) selection only: the same run, interpreted; (b) plus a trace of evaluations (which definition, reaction or contribution, its inputs and result, its path); (c) plus breakpoints and stepping driven by the host. Recommendation: (b) now, (c) as its own feature with the host. The solver's record sink is already the first-class debug surface for events (solver-completeness context Q4).
- **INT-Q3. Oracle comparison.** Options: (a) everything bit for bit; (b) discrete side bit for bit, analog within tolerance; (c) final waveforms only, within tolerance. Recommendation: (b). Rust has no fast-math, so (a) may hold in practice if both backends evaluate in the same order, but libm calls and a different Jacobian method (INT-Q1) can move the last bit.
- **INT-Q4 (O5). Default backend.** Whether small runs use the interpreter by default to avoid compile latency. Recommendation: decide in `ir-rust` with measurements; this feature only guarantees the interpreter is selectable.
- **INT-Q5 (O8). ABI fit.** Where instance state lives, how the interpreter reports events and step bounds, and how small-signal and noise are asked for, are all the duel's output. This spec states behavior and leaves the seam open.
- **INT-Q6. Crate.** Options: (a) `piperine-interp`; (b) a module of the IR crate. Recommendation: (a): the IR crate stays free of solver dependencies.
- **INT-Q7. Wide and odd numeric types.** `UInt<N>`/`Int<N>` for any `N` in 1..=128 wrap modulo `2^N`; `Float<N>` has `N` in {16, 32, 64, 128}. Options: (a) exact from the start; (b) fail loud on widths with no native Rust type until needed. Recommendation: (a) for integers (masking is cheap), (b) for `Float<16>` and `Float<128>`, which stable Rust does not have.

---

## Dependencies & Order

| Depends on | For | State |
| ---------- | --- | ----- |
| `msl-ir` | the input | drafted with this spec |
| `msl-elab` | real IR values to run; without it only hand-built IRs are testable | drafted with this spec |
| ABI duel (C8, O8) | the interface the interpreter implements | open |
| `solver-restructure` and the solver features that follow it | the event owner, the digital DAG, the analog sub-solver the interpreter is called by | planned on `feature/solver-completeness` |
| `msl-stdlib` | the §22 sources the conformance set uses | in progress |

Order proposed by O6 (not decided): `msl-elab`, `msl-ir` and `ir-interp` first; `ir-rust` after, tested against this feature's oracle. `ir-interp` is last of its group and is the first point where integration tests and ngspice parity can run.

---

## User Stories

### P1: Run a design from its IR ⭐ MVP

**User Story**: As a model author, I want my elaborated design to run with no compiler installed so that I can simulate anywhere and immediately.

**Why P1**: C6; first end-to-end path (O6).

**Requirement**: INT-01

**Acceptance Criteria**:

1. WHEN a session selects the interpreter backend for an IR THEN the system SHALL run the requested analysis without invoking `rustc` or any other external program.
2. The system SHALL present an interpreted design to the solver through the device ABI (C8), with no interface of its own.
3. IF the IR fails its invariant check THEN the system SHALL refuse to run it and SHALL return the check's entries.

**Independent Test**: run a `Dc` analysis of a resistor divider elaborated from MSL source on a machine with no `rustc` in `PATH`.

---

### P1: Value semantics ⭐ MVP

**User Story**: As a model author, I want value-level code to compute what the language defines so that functions, structs, enums and `match` behave as written.

**Why P1**: §0.4 and Part I; every other story evaluates expressions.

**Requirement**: INT-02

**Acceptance Criteria**:

1. The system SHALL evaluate integer arithmetic of `UInt<N>` and `Int<N>` wrapping modulo `2^N` (§1 rule 6).
2. WHEN a float is divided by zero THEN the system SHALL yield the IEEE 754 result and SHALL not fail (§1 rule 12).
3. The system SHALL propagate the known flag of every level through every operator as §3.3.2 defines.
4. WHEN an expression is evaluated twice with the same inputs THEN the system SHALL return the same levels and the same known flags.

**Independent Test**: unit tests over hand-built IR expressions: `unknown & false` is known `false`; `200u8 + 100u8` is `44`.

---

### P1: Analog evaluation ⭐ MVP

**User Story**: As the analog sub-solver, I want each `analog` block evaluated at the current iterate with its derivatives so that Newton can assemble and solve.

**Why P1**: §11, §16.2; the analog half of the first end-to-end run.

**Requirement**: INT-03

**Acceptance Criteria**:

1. WHEN the solver asks for an evaluation THEN the system SHALL execute the `analog` block with probes reading the current iterate and SHALL return, for every branch, the sum of the contributions enabled in that solve (§11 rules 15 and 16).
2. The system SHALL return the derivative of every contribution with respect to every probe it depends on, through function calls, value inputs and results (§11 rule 5).
3. The system SHALL give each core primitive the value of its row and of the solve's column in the first table of §16.2.
4. WHEN `ddx(e, probe)` is evaluated THEN the system SHALL return the partial derivative of `e` with a probe of the same branch contributing 1, the reversed branch −1 and every other branch held constant (§14.2.3).
5. IF a contribution evaluates to a non-finite value THEN the system SHALL fail the solve (§11 rule 19).
6. WHEN `bound_step(dt)` is executed at the accepted solution of a transient step THEN the system SHALL report `dt` to the solver as a cap on the next step (§14.10.2).

**Independent Test**: evaluate a diode `analog` block at a fixed bias on a hand-built IR and compare current and conductance with the closed form.

---

### P1: Discrete evaluation ⭐ MVP

**User Story**: As the digital sub-solver, I want definitions, `resolve`s and reactions evaluated on demand so that the microstep loop settles and reacts as §15 states.

**Why P1**: §8.2, §12, §13, §15; the digital half of the first end-to-end run.

**Requirement**: INT-04

**Acceptance Criteria**:

1. WHEN the solver asks for a definition or a `resolve` THEN the system SHALL return its value computed from the carrier values the solver supplies.
2. WHEN the solver runs a reaction THEN the system SHALL execute it once to completion and SHALL return its commits, each with its target, value and delay, without writing any carrier itself (§12.1.1, §12.1.6).
3. The system SHALL give the same result for any order in which the solver evaluates the reactions of one microstep (§15 I1, I2).
4. WHEN a step is rejected THEN the system SHALL keep no state from the evaluations made during it (§15 I8, §14 rule 8).
5. WHEN an `emit!` is executed THEN the system SHALL hand the solver one record with its stream, its fields and the instance path (§19 rule 1).
6. WHEN the solver asks whether an event expression fires THEN the system SHALL answer from the state and the solution the solver supplies for that instant (§13, §15 "What each kind of code reads").

**Independent Test**: run the trace of §15.1 (the `Front` model) and compare the order of commits, definitions and reactions with the table of §15.1.

---

### P1: Run errors ⭐ MVP

**User Story**: As a model author, I want a failure during a run to say what failed and where so that I can fix the model.

**Why P1**: §0.3 items 3 and 4; the motivation of the whole redesign is debuggability.

**Requirement**: INT-05

**Acceptance Criteria**:

1. IF value-level code panics during a run (§1 rule 11) THEN the system SHALL fail the analysis and SHALL name the instance path.
2. IF a condition of phase run in an error table of Part IV is met during an evaluation THEN the system SHALL fail the analysis with that condition's code.
3. The system SHALL attach to every run error the hierarchical path of the instance and the source location the IR records for the failing item.

**Independent Test**: index an array out of range in a reaction and read the error's code, path and source location.

---

### P1: Differential oracle ⭐ MVP

**User Story**: As a backend author, I want to run one IR on the interpreter and on my backend and see the first difference so that a fast backend is proven against the reference.

**Why P1**: C6: "the differential oracle every fast backend is tested against".

**Requirement**: INT-06

**Acceptance Criteria**:

1. WHEN the harness is given an IR, an analysis and a second backend THEN the system SHALL run the analysis on both and SHALL return either "equal" or the first differing item.
2. WHEN the two runs differ THEN the system SHALL report the instant, the path of the item and both values.
3. WHEN the second backend is the interpreter itself THEN the system SHALL report "equal".

**Independent Test**: run the harness against a deliberately wrong test backend that negates one contribution and read the reported path.

---

### P2: Debug mode

**User Story**: As a model author, I want to select the interpreter to investigate a run so that I can see what the model computed.

**Why P2**: C6 names the mode; its content is INT-Q2.

**Requirement**: INT-07

**Acceptance Criteria**:

1. WHERE the debug trace is enabled the system SHALL emit one trace entry per evaluated definition, reaction and contribution, with its instance path and its result.
2. WHILE the debug trace is disabled the system SHALL emit no trace entry.

**Independent Test**: run the `Front` model with the trace enabled and find the three reactions by path.

---

## Edge Cases

- WHEN a design has no `analog` block THEN the system SHALL run its discrete evaluation with no analog evaluation requested.
- WHEN `noise(psd)` is evaluated outside a `Noise` sweep THEN the system SHALL return 0 and SHALL not evaluate `psd` (§16.2 note 1).
- IF `analog` code reaches a value with an unknown level THEN the system SHALL fail the analysis, since elaboration guarantees it cannot (§3.3.5).
- WHEN a reaction makes no commit THEN the system SHALL return an empty list of commits.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| INT-01 | P1: Run a design from its IR | - | Pending |
| INT-02 | P1: Value semantics | - | Pending |
| INT-03 | P1: Analog evaluation | - | Pending |
| INT-04 | P1: Discrete evaluation | - | Pending |
| INT-05 | P1: Run errors | - | Pending |
| INT-06 | P1: Differential oracle | - | Pending |
| INT-07 | P2: Debug mode | - | Pending |

**Coverage:** 7 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` - decisions C1 to C10 and open items O1 to O8.
- `docs/spec/` Part I (§1, §3.3), Part IV (§11 to §14), Part V (§15, §16.2), §19, Appendix C.2.3 - the behavior to reproduce.
- `feature/solver-completeness:.specs/features/solver-completeness/context.md` Q1, Q2, Q4 - what the solver owns.

---

## Success Criteria

- [ ] Every item of the conformance set (Appendix C.2.3) runs on the interpreter and matches its hand trace in `docs/spec/`.
- [ ] The trace of §15.1 is reproduced event for event.
- [ ] The whole suite runs on a machine without `rustc`.
- [ ] The differential harness is the test entry point `ir-rust` uses.

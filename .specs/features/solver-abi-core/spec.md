# Solver ABI Core Specification

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

First of four features that make the solver adopt the device ABI (`.specs/features/device-abi/abi.md`, the authority). Order: `solver-abi-core` (this one: the crate `piperine-abi` and the owner's protocol on two traits), then `solver-abi-digital` and `solver-abi-analog` in parallel (each sub-solver on the ABI, in its own crate and worktree), then `solver-abi-integration` (the owner wires both, the tests are ported, `Element` is removed). Together they replace feature B (`solver-event-boundary`) of `solver-completeness`.

## Problem Statement

The device ABI is decided and nothing of it compiles. Every later step (the two sub-solvers, the test devices, the interpreter and the generated Rust of the codegen features) needs one crate that holds the `#[repr(C)]` contract, the Rust `Device` trait over it, and the checks that tell a device author the shape is wrong before a solver is involved. Without it each consumer would restate the contract.

The synchronization protocol of abi.md §5 has the same problem one level up: it is the owner's logic, it needs both sub-solvers, and neither exists on the ABI yet. Written against two traits and proven on scripted stubs, it lets the two sub-solvers be built in parallel against a fixed seam.

## Goals

- [ ] A crate `piperine-abi` with no dependency holds the whole contract of abi.md §3 and the Rust side of §7.
- [ ] A device written as a Rust `impl Device` is callable through the six entry points of abi.md §2 with no macro and no unwinding.
- [ ] A shape that breaks a shape-local rule of abi.md §9 is refused with a typed error before any solver sees it.
- [ ] A finite-difference verifier checks any function's declared partials.
- [ ] The owner's protocol (abi.md §5 and §10) runs over two traits, `DiscreteSide` and `AnalogSide`, and is proven on scripted stubs with no sub-solver behind them.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Any change to the two sub-solver crates, to the drivers (`analyses/*.rs`) or to `Element` | `solver-abi-digital`, `solver-abi-analog`, `solver-abi-integration`. The protocol is a new module of the owner that nothing calls yet |
| Implementing the two traits for the real sub-solvers, bind, re-creating an instance on `RESHAPE` | `solver-abi-integration` |
| Loading a library with `dlopen`, `cbindgen` header generation, `piperine inspect` | the solver core has no `libloading`; a host or plugin feature. An in-process `Library` uses the same `Module` pointers (abi.md §7 "One path") |
| Bind rules that need nets (one writer P1, layout equality across a net P4, cycles, the parameter cone P6) | they need the netlist: `solver-abi-digital`, `solver-abi-analog` |
| The OSDI adapter (abi.md §11.8) and the AVR emulator (§11.7) | examples of the ABI, not part of the solver |
| Record/replay of calls, poisoned output buffers (abi.md §12 "Debugging") | later debug feature; the verifier (§8) is in scope |
| Gradients across events (abi.md §14.4) | deferred by the ABI itself |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The contract | abi.md as merged at 98b30ad6, with its §15 assumptions (amendments 1, 2, 3a, 4, 6, 8, 9 accepted; 3b not; 5, 7 withdrawn) | `codegen/context.md` "Device ABI decisions (user, 2026-11-15)" | y |
| `piperine-abi` sits under all three solver crates | MD-60 said no in-tree crate sits under a sub-solver. This feature records MD-61: `piperine-abi` (no dependency, vocabulary only) is the one exception | the caller's brief: "new crate `piperine-abi`", "digital sub-solver on the ABI", "analog sub-solver on the ABI"; the sub-solvers still never see each other | n - recommended; reported to the user as the one structural question |
| `Device::create` returns the shape's owner | `Result<(Self, ShapeRef), Fault>` with `ShapeRef = Arc<ShapeBuf>`; abi.md §7 writes `Arc<Shape>`, which cannot own the lists a `Shape` points to | abi.md: "Nothing here has been compiled"; the C layout is untouched; abi.md §7 is updated in the same commit | y - Rust-side only, settled by the compiler |
| `module::<D>(name)` | a plain `fn`, not `const fn`: it calls `D::prototype()`; a `Library` is built once behind a `OnceLock` | trait methods are not `const`; "A library is one hand-written function returning a `static Library`" still holds | y - Rust-side only, settled by the compiler |
| No macro | constants are `pub const`, `Pod` impls and thunks are written by hand | user memory "no macros preference"; abi.md §7 "no macro" | y |
| Who models, who verifies | Sonnet implements and Opus verifies by default; the orchestrator may have Opus implement a delicate feature that is not large, with a different Opus verifying. This feature: 16 tasks; the protocol phase is protocol-delicate, nothing is numerically delicate | user, 2026-11-15, through the coordinator | y |
| The seam for parallel work | the two traits use plain data only (ticks as `i64`, nets and slots as `u32`, bytes, `f64`), so neither sub-solver crate depends on the owner and `solver-abi-integration` writes two thin adapters | user, 2026-11-15: digital and analog "runnable in parallel in separate worktrees", "both write against the core's trait/protocol" | y |
| When a discrete change reaches the analog side | what is due at microstep 0 of a landing is previewed on a copy and delivered before the attempt that arrives there; a change made in a later microstep is read by the next step, which restarts the integrator; after a host write to a `PARAM` or `GLOBAL` net the restart is the small one | umbrella AC 6: "land on its time, solve the step with the new value and restart its integrator, as the solver does today for a breakpoint"; abi.md §10 uses the same preview for settle commits; `transient.rs::propose_dt` (`sets_just_applied`) | y - follows Q7; it is what `spec-abi-amendments` writes into §15.2 |
| Operating-point rounds | bounded at 20; beyond it `E0506` naming the nets (today's loop exits silently) | `CLAUDE.md`: fail loud; umbrella SCT-07 | y - agent's discretion, fail-loud rule |
| Proof vehicle | unit tests of the crate; the worked examples of abi.md §11.1 to §11.6 are the fixtures | user: unit tests only for now | y |
| Solver-side call helper | lives in `piperine-abi` (`caller`), so the two sub-solvers share one `unsafe` path | both sub-solvers call devices; a fifth crate for 150 lines is not worth it | y - agent's discretion (structure) |

**Open questions:** none blocking. One for the user, with a recommendation, is the MD-61 row above (recommended: yes).

**Implicit-requirement dimensions.** Input validation & bounds: SAC-06, SAC-09. Failure / partial failure: SAC-05, SAC-14 (a rejected attempt), SAC-16 (`E0506`). Idempotency / retry: N/A because functions are pure by contract (F1) and the crate holds no state. Auth and rate limits: N/A because it is an in-process library. Concurrency / ordering: SAC-07 (shape shared across instances; `Send`), SAC-17 (instant order). Data lifecycle: SAC-04, SAC-07 (instance and shape lifetime). Observability: SAC-05 (the note), SAC-09 (errors carry `origin`), SAC-17 (the sink). External-dependency failure: N/A because the crate has no dependency. State-transition integrity: SAC-14, SAC-15 (the protocol's steps).

---

## User Stories

### P1: The contract as data ⭐ MVP

**User Story**: As a device author, I want the shape, the frames and the constants of the ABI as plain `#[repr(C)]` Rust, so that a device in Rust, C or generated code describes itself the same way.

**Why P1**: every other requirement reads these types.

**Acceptance Criteria**:

1. The crate `piperine-abi` SHALL declare no dependency and no dev-dependency on any crate.
2. The crate SHALL define every struct of abi.md §3 as `#[repr(C)]` with its fields in the listed order, and every enumeration as named `u32` constants.
3. WHEN a consumer reads element `i` of a `List<T>` whose `stride` is larger than `size_of::<T>()` THEN it SHALL read at `ptr + i * stride`.
4. The crate SHALL ship the layouts `BIT`, `REAL` and `BYTE` as constants.
5. WHEN two layouts have equal size, alignment and, per field, equal leaf, offset, count and known offset THEN `layout_eq` SHALL return true whatever the field names.
6. IF a field with a known byte of 0 holds non-zero value bytes THEN `is_canonical` SHALL return false.
7. WHEN a shape is built with `ShapeBuf` and frozen THEN its `Shape` pointer and every list it reaches SHALL stay valid until the last `ShapeRef` is dropped.

**Independent Test**: build the resistor shape of abi.md §11.1 with `ShapeBuf`, read it back through the raw pointers and get the same pins, term, function, reads, writes and partials.

---

### P1: A device in Rust ⭐ MVP

**User Story**: As a device author, I want to implement one trait and get the C entry points, so that I write no `unsafe` and no macro.

**Why P1**: test devices, the interpreter and generated code all use it.

**Acceptance Criteria**:

1. WHEN `module::<D>(name)` is called THEN it SHALL return a `Module` whose `create`, `destroy`, `eval` and `react` pointers call the methods of `D`.
2. WHEN `create` succeeds THEN it SHALL return status `OK`, an instance and the shape, and `destroy` SHALL drop the device and release its hold on the shape.
3. IF a method of `D` panics THEN the thunk SHALL return `FAULT` with the panic message in the note and SHALL NOT unwind.
4. IF a method of `D` returns a `Fault` THEN the thunk SHALL return `FAULT` with the fault's code in `note.code` and its message in `note.text`, cut to 248 bytes at a character boundary.
5. WHEN `react` calls `cx.commit(statement, bytes, after)` THEN the thunk SHALL call `Host.commit` once with that statement, offset 0, the length of `bytes` and `after`.
6. IF a typed lens access names a type larger than the cell's bytes THEN it SHALL return `None`.
7. WHEN `React` returns `Flow::Finish` THEN the thunk SHALL return status `FINISH`.
8. WHEN a device is called through `caller::Handle` THEN `eval` and `react` SHALL build the frames of abi.md §3, pass them to the module's pointers and return the status as a `Result`.
9. The devices of abi.md §11.1 to §11.6 SHALL produce through the thunks the values and partials their text states.

**Independent Test**: the diode of §11.2 evaluated through `Handle` returns `is * (exp(vd / nvt) - 1)` and its declared partials; a device that panics returns `FAULT`.

---

### P1: Checks before a solver exists ⭐ MVP

**User Story**: As a device author, I want a shape check and a Jacobian check I can run in my own tests, so that a wrong declaration or a wrong partial is found at the device.

**Why P1**: the bind of both sub-solvers starts with the shape check; the port of the test devices uses the Jacobian check.

**Acceptance Criteria**:

1. IF a shape breaks one of the shape-local rules listed below THEN `validate` SHALL return an error naming the rule, the entry and its `origin`.
2. WHEN `validate` runs on the shapes of abi.md §11.1 to §11.6 THEN it SHALL return no error.
3. WHEN `verify_partials` runs on a function whose declared partials are exact THEN it SHALL return no mismatch.
4. IF a declared partial differs from the central finite difference by more than the given relative tolerance THEN `verify_partials` SHALL report the partial's index, the declared value and the finite difference.
5. IF the function writes a `LIMIT` input whose `LIMITED` differs from `PROPOSED` at the given point THEN `verify_partials` SHALL report the point as skipped and compare nothing.

Shape-local rules (abi.md §9), one planted defect each: an unknown constant; an index out of range; a kind and role pair outside §3.1; a partial whose `to` or `at` is not an `F64`; a term target that is not continuous; a function writing both sorts (F2); a discrete function reading an input or a continuous pin; `CHANGED` read by a function that writes a discrete pin; a function writing a limit and a term without `OWNS_CORRECTION`, or with it and a partial against that limit; a function reading a `QUIET` pin; a reaction listening to a non-event pin; a `PARAM` pin after a non-`PARAM` pin (P5). Settled mid-build (orchestrator, B2): a statement on a continuous pin; a `CROSS` watch whose event pin is not an event pin; a term whose `abstol` or `max` pin is not a discrete `F64`.

**Independent Test**: plant each defect in the resistor or flip-flop shape and read the rule's name in the error; flip one partial of the capacitor by 1 % and read its index in the report.

---

### P1: The owner's protocol on two traits ⭐ MVP

**User Story**: As the owner of events, I want the synchronization protocol written once against a discrete side and an analog side, so that the two sub-solvers are built in parallel and meet only through it.

**Why P1**: it fixes the seam `solver-abi-digital` and `solver-abi-analog` implement.

**Acceptance Criteria**:

1. The protocol module SHALL reach the sub-solvers through the traits `DiscreteSide` and `AnalogSide` only, and SHALL name no item of `piperine-analog-solver` or `piperine-digital-solver`.
2. WHEN the discrete side reports no landing before the proposed time THEN the protocol SHALL ask the analog side for one attempt to that time.
3. WHEN the discrete side reports a landing before the proposed time THEN the protocol SHALL deliver that instant's boundary preview to the analog side and attempt a step to the landing, exempt from the truncation test when the preview changed a value. *(Superseded by SAC-24, verification round 1: the user's order "ABI order: solve, then react". Kept as approved history.)*
4. WHEN an attempt is rejected, or lands earlier than its target THEN the protocol SHALL restore both sides and the sink mark to the last accepted time and advance the discrete side again to the new target.
5. WHEN an attempt is accepted THEN the protocol SHALL commit the fired crossings at microstep 0 of the landing, run the landing instant with the accepted samples, and ask for a restart when the instant changed a value the analog side reads.
6. WHEN the change that caused a restart was a host write to a `PARAM` or `GLOBAL` net THEN the protocol SHALL ask for the small restart.
7. WHEN the discrete side stops for an irreversible reaction THEN the protocol SHALL run that instant once, after the analog solution is accepted up to it, and SHALL fire `SETTLE` there only if the instant is a time point.
8. WHEN a reaction returns `FINISH` THEN the analysis SHALL end after that instant, and a `FINISH` seen ahead of a step that then lands earlier SHALL be dropped.
9. WHEN the analog side has no unknown THEN the protocol SHALL run every due instant and the stop time with no attempt.
10. WHEN an operating point is solved THEN the protocol SHALL start the discrete side, solve the equilibrium, run the instant, solve again each time the instant changed a value the analog side reads, and fire `SETTLE` once when it did not.
11. IF the rounds of an operating point exceed the bound (default 20) THEN the protocol SHALL fail with `E0506` naming the nets still changing.
12. The protocol SHALL deliver to the host's sink, after each acceptance and in instant order, every observation of the discrete side with its instance path, analysis kind and time, and after every attempt every trace record with its Newton iteration.
13. WHEN an instant is undone THEN its observations SHALL NOT reach the sink.
14. WHEN the host registers a waiter on an event net THEN the protocol SHALL call it once, at an instant whose analog solution is accepted, and SHALL queue what it writes as host commits due at the next microstep.
15. WHEN the host writes a `PARAM` or `GLOBAL` net THEN the protocol SHALL commit it only after the discrete side's shadow accepts it, SHALL refuse it when a `check` is false, and SHALL refuse a `RESHAPE` inside a transient and report the instance outside one.

**Independent Test**: two scripted stubs (a discrete side that lands at given ticks, an analog side that rejects a given attempt) drive the protocol through a landing, a rejection with replay, an irreversible instant and a `FINISH`, and the recorded call sequence equals the sequence of abi.md §5.

---

### P2: The workspace knows the crate

**User Story**: As a maintainer, I want the structural guards and the decision log to state where `piperine-abi` sits, so that the boundary of MD-60 stays enforced.

**Why P2**: a guard already forbids any workspace dependency under a sub-solver; it must name the one exception before the next feature adds it.

**Acceptance Criteria**:

1. The workspace SHALL list `crates/piperine-abi` in `members` and `default-members`.
2. The structural guard SHALL allow a sub-solver to depend on `piperine-abi` and on no other workspace crate.
3. `.specs/STATE.md` SHALL carry MD-61, and `CLAUDE.md` and `AGENTS.md` SHALL list the crate.

**Independent Test**: `cargo test -p piperine-solver --test crate_boundaries --test workspace_shape` passes; adding `piperine-digital-solver` as a dependency of `piperine-analog-solver` makes it fail.

---

### P1: Verification round 1 fixes

**User Story**: As the owner of the ABI, I want the findings of verification round 1 closed with requirements of their own, so that each fix has a criterion and a check.

**Acceptance Criteria** (appended; approved ACs above are history):

- **SAC-19** (1). IF a declared partial or its finite difference is not finite THEN `verify_partials` SHALL report it as a mismatch naming the partial (F7).

- **SAC-20** (2). IF a list's stride is below its element's size or breaks its alignment, or its pointer is misaligned, THEN `List::get` SHALL read nothing and `validate` SHALL refuse the shape (`UnreadableList`) (F8).

- **SAC-21** (3). The pointer fields of `Str`, `Bytes`, `List` and `Module` SHALL NOT be public; a view of foreign memory SHALL be built only through an `unsafe fn from_raw_parts`, so no safe accessor can reach undefined behaviour (F9).

- **SAC-22** (4). No source of `piperine-abi` other than `raw.rs`, `thunk.rs` and `caller.rs` SHALL use `unsafe`, and a test SHALL scan for it (F11).

- **SAC-23** (5). IF an event pin's layout is not empty, a continuous pin's layout is not one `F64`, a `check` is not a discrete pin holding a `BOOL` at byte 0, or a reaction reads `PREVIOUS` or `CHANGED` of a `QUIET` pin, THEN `validate` SHALL refuse the shape naming the rule and the entry; proven by rows of check C8 (F12).

- **SAC-24** (6). WHEN a step lands on a scheduled instant THEN the protocol SHALL solve at that time on the value before the instant (the left value), exempt from the truncation test, accept, run the instant (its reactions read the solution at that time), and only then deliver what the instant changed (microstep 0 included) with the restart the next step starts from; user (solver-completeness context.md, 2026-11-15): "ABI order: solve, then react (Recommended)" (F1).

- **SAC-25** (7). Every step of a transient SHALL end on the tick grid (a proposal is floored to a tick, at least one past the last acceptance; a located crossing lands on its nearest tick inside the step), and the instant run at a step's end SHALL be that tick's own (F2).

- **SAC-26** (8). WHEN a reaction returns `FINISH` at the operating point THEN `operating_point` SHALL return `Ended::Finish` and a later `transient` SHALL run nothing and return the same end (F3).

- **SAC-27** (9). IF consecutive attempts from one accepted time are rejected `max_rejections` times (default 64) THEN the transient SHALL fail with a run error naming that time; the step reduction of a rejection SHALL survive the analog side's `restore` (F4).

- **SAC-28** (10). WHEN the host writes a net through `Protocol::set` between runs with the write due now THEN the protocol SHALL run the instant at the current time after the commit and deliver what it changed with `IntegratorSmall` after a param write (`Integrator` otherwise), and the next transient SHALL step forward with no zero-length step; proven by check C30 (F5).

- **SAC-29** (11). WHEN the protocol runs an instant THEN it SHALL bound the `HORIZON` of a reaction a `CROSS` watch can reach by the current instant while the analog side has unknowns (by the stop time when it has none), and of any other reaction by the stop time (abi.md R5) (F6).

- **SAC-30** (12). The sink SHALL receive trace records and observations as one stream in time order: a trace (even of a rejected attempt) is delivered once an acceptance reaches its time, before that time's instant observations (F13).

- **SAC-31** (13). IF the value list passed to `create` is not readable (stride below a `Value` or misaligned) THEN the thunk SHALL return `FAULT` with a note, never create the device on its defaults; `ShapeBuf` SHALL refuse a layout whose field list is unreadable (N3).

- **SAC-32** (14). WHEN an attempt reports a crossing (`LandedEarlier`) THEN the protocol SHALL retry at the nearest tick at least one tick short of the target, SHALL take the step at its end when the crossing is at or past the target or rounds to the end of a one-tick step, and SHALL count every retry toward `max_rejections`, so no identical step is retried and no loop is unbounded (N1).

- **SAC-33** (15). `Str`, `Bytes` and `List` (and so `Shape` and every struct holding them) SHALL carry the lifetime of the memory they point to, so safe code cannot keep a view past its owner: a list from a dropped slice, or a shape copied out of a dropped `ShapeRef`, SHALL NOT compile; this makes SAC-21's claim hold (N2, F9).

---

## Edge Cases

- WHEN a list is empty THEN its pointer SHALL be non-null and aligned, and reading it SHALL yield no element.
- IF `create` is given a `Value` whose pin is not a `PARAM` pin of the prototype THEN the thunk SHALL return `FAULT` naming the pin index.
- WHEN a statement on an event pin is committed THEN `cx.commit` SHALL pass a length of 0.
- IF `Host.commit` returns a status other than `OK` THEN `cx.commit` SHALL return it to the device as an error.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SAC-01 | P1: Contract, AC 1, 2 (crate, `repr(C)` types, constants) | Tasks | In Tasks |
| SAC-02 | P1: Contract, AC 3 (stride-aware lists) | Tasks | In Tasks |
| SAC-03 | P1: Contract, AC 4, 5, 6 (layouts, equality, canonical form) | Tasks | In Tasks |
| SAC-04 | P1: Device, AC 1, 2 (thunks, create and destroy) | Tasks | In Tasks |
| SAC-05 | P1: Device, AC 3, 4 (no unwinding, the note) | Tasks | In Tasks |
| SAC-06 | P1: Device, AC 5, 6, 7 (commit, typed access, finish) | Tasks | In Tasks |
| SAC-07 | P1: Contract, AC 7 (owned shape) | Tasks | In Tasks |
| SAC-08 | P1: Device, AC 8 (caller) | Tasks | In Tasks |
| SAC-09 | P1: Checks, AC 1, 2 (shape validation) | Tasks | In Tasks |
| SAC-10 | P1: Checks, AC 3, 4, 5 (partials verifier) | Tasks | In Tasks |
| SAC-11 | P1: Device, AC 9 (worked examples) | Tasks | In Tasks |
| SAC-12 | P2: Workspace, AC 1, 2, 3 (members, guard, MD-61) | Tasks | In Tasks |
| SAC-13 | P1: Protocol, AC 1 (two traits, no sub-solver type) | Tasks | In Tasks |
| SAC-14 | P1: Protocol, AC 2 to 6 (landing, preview, restore and replay, accept, restart) | Tasks | In Tasks |
| SAC-15 | P1: Protocol, AC 7, 8, 14 (irreversible, `FINISH`, host waiter) | Tasks | In Tasks |
| SAC-16 | P1: Protocol, AC 9, 10, 11 (no analog, operating point, `E0506`) | Tasks | In Tasks |
| SAC-17 | P1: Protocol, AC 12, 13 (the sink) | Tasks | In Tasks |
| SAC-18 | P1: Protocol, AC 15 (host writes) | Tasks | In Tasks |
| SAC-19 | P1: Verification round 1, AC 1 | Execute | Done |
| SAC-20 | P1: Verification round 1, AC 2 | Execute | Done |
| SAC-21 | P1: Verification round 1, AC 3 | Execute | Done |
| SAC-22 | P1: Verification round 1, AC 4 | Execute | Done |
| SAC-23 | P1: Verification round 1, AC 5 | Execute | Done |
| SAC-24 | P1: Verification round 1, AC 6 | Execute | Done |
| SAC-25 | P1: Verification round 1, AC 7 | Execute | Done |
| SAC-26 | P1: Verification round 1, AC 8 | Execute | Done |
| SAC-27 | P1: Verification round 1, AC 9 | Execute | Done |
| SAC-28 | P1: Verification round 1, AC 10 | Execute | Done |
| SAC-29 | P1: Verification round 1, AC 11 | Execute | Done |
| SAC-30 | P1: Verification round 1, AC 12 | Execute | Done |
| SAC-31 | P1: Verification round 1, AC 13 | Execute | Done |
| SAC-32 | P1: Verification round 1, AC 14 | Execute | Done |
| SAC-33 | P1: Verification round 1, AC 15 | Execute | Done |

**Coverage:** 30 total (SAC-19 to SAC-30 from verification round 1), 30 mapped to tasks or checks, 0 unmapped. Umbrella requirements whose owner half is met here: SCE-01, SCE-03, SCE-04, SCT-05 AC 1, SCT-07, SCT-09, SCH-05, SCH-07.

---

## Sources

- `.specs/features/device-abi/abi.md`: §2 (entry points), §3 (shape and frames), §4.1 (cells), §5 (R4 to R7 and the synchronization protocol), §6 (C5, C6), §7 (Rust side), §8 (verifier), §9 (bind validation), §10 (call sequences), §11 (examples), §12 (safety, versioning).
- The coordinator's message of 2026-11-15 (user): the four-feature split, parallel sub-solvers, model roles.
- `.specs/features/codegen/context.md`: C8, C10 and "Device ABI decisions".
- `.specs/features/solver-completeness/context.md`: rounds 1 and 2 (opaque bytes, unit tests).
- `.specs/STATE.md` MD-17 (two-tier surface), MD-28 (test placement), MD-31, MD-33, MD-60.

---

## Success Criteria

- [ ] `cargo test -p piperine-abi` is green with zero warnings and no dependency in `cargo tree -p piperine-abi`.
- [ ] The six devices of abi.md §11.1 to §11.6 run through `module::<D>` and `Handle`.
- [ ] Every shape-local rule of abi.md §9 has a planted defect that `validate` names.
- [ ] The protocol's recorded call sequence on stubs equals abi.md §5 for a landing, a rejection, an irreversible instant and a `FINISH`.

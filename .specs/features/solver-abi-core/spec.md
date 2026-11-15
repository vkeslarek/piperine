# Solver ABI Core Specification

First of three features that make the solver adopt the device ABI (`.specs/features/device-abi/abi.md`, the authority). Order: `solver-abi-core` (this one: the crate `piperine-abi`), `solver-abi-digital` (the digital sub-solver on the ABI), `solver-abi-analog` (the analog sub-solver on the ABI, the owner protocol, the port of every test, the removal of `Element`). Together they replace feature B (`solver-event-boundary`) of `solver-completeness`.

## Problem Statement

The device ABI is decided and nothing of it compiles. Every later step (the two sub-solvers, the test devices, the interpreter and the generated Rust of the codegen features) needs one crate that holds the `#[repr(C)]` contract, the Rust `Device` trait over it, and the checks that tell a device author the shape is wrong before a solver is involved. Without it each consumer would restate the contract.

## Goals

- [ ] A crate `piperine-abi` with no dependency holds the whole contract of abi.md §3 and the Rust side of §7.
- [ ] A device written as a Rust `impl Device` is callable through the six entry points of abi.md §2 with no macro and no unwinding.
- [ ] A shape that breaks a shape-local rule of abi.md §9 is refused with a typed error before any solver sees it.
- [ ] A finite-difference verifier checks any function's declared partials.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Any change to the three solver crates' code | `solver-abi-digital` and `solver-abi-analog` |
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
| Who models, who verifies | Opus implements, Fable verifies | user, round 2: "mexer no core core (mudança de logica) seja implementada por opus e verificada com fable" | y |
| Proof vehicle | unit tests of the crate; the worked examples of abi.md §11.1 to §11.6 are the fixtures | user: unit tests only for now | y |
| Solver-side call helper | lives in `piperine-abi` (`caller`), so the two sub-solvers share one `unsafe` path | both sub-solvers call devices; a fifth crate for 150 lines is not worth it | y - agent's discretion (structure) |

**Open questions:** none blocking. One for the user, with a recommendation, is the MD-61 row above (recommended: yes).

**Implicit-requirement dimensions.** Input validation & bounds: SAC-06, SAC-09. Failure / partial failure: SAC-05. Idempotency / retry: N/A because functions are pure by contract (F1) and the crate holds no state. Auth and rate limits: N/A because it is an in-process library. Concurrency / ordering: SAC-07 (shape shared across instances; `Send`). Data lifecycle: SAC-04, SAC-07 (instance and shape lifetime). Observability: SAC-05 (the note), SAC-09 (errors carry `origin`). External-dependency failure: N/A because the crate has no dependency. State-transition integrity: N/A because the crate has no state machine.

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

Shape-local rules (abi.md §9), one planted defect each: an unknown constant; an index out of range; a kind and role pair outside §3.1; a partial whose `to` or `at` is not an `F64`; a term target that is not continuous; a function writing both sorts (F2); a discrete function reading an input or a continuous pin; `CHANGED` read by a function that writes a discrete pin; a function writing a limit and a term without `OWNS_CORRECTION`, or with it and a partial against that limit; a function reading a `QUIET` pin; a reaction listening to a non-event pin; a `PARAM` pin after a non-`PARAM` pin (P5).

**Independent Test**: plant each defect in the resistor or flip-flop shape and read the rule's name in the error; flip one partial of the capacitor by 1 % and read its index in the report.

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

**Coverage:** 12 total, 12 mapped to tasks, 0 unmapped.

---

## Sources

- `.specs/features/device-abi/abi.md`: §2 (entry points), §3 (shape and frames), §4.1 (cells), §7 (Rust side), §8 (verifier), §9 (bind validation), §11 (examples), §12 (safety, versioning).
- `.specs/features/codegen/context.md`: C8, C10 and "Device ABI decisions".
- `.specs/features/solver-completeness/context.md`: rounds 1 and 2 (opaque bytes, unit tests, Opus implements and Fable verifies).
- `.specs/STATE.md` MD-17 (two-tier surface), MD-28 (test placement), MD-31, MD-33, MD-60.

---

## Success Criteria

- [ ] `cargo test -p piperine-abi` is green with zero warnings and no dependency in `cargo tree -p piperine-abi`.
- [ ] The six devices of abi.md §11.1 to §11.6 run through `module::<D>` and `Handle`.
- [ ] Every shape-local rule of abi.md §9 has a planted defect that `validate` names.

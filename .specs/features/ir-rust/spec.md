# IR to Rust Transpiler Specification

**Status**: Draft. Not approved. Open questions RS-Q1 to RS-Q9 block approval.

## Problem Statement

The interpreter (`ir-interp`) runs any design but is not the fast path. The previous fast path, Cranelift, is retired because its output could not be read, debugged or verified (C1, context.md "Why"). The user chose Rust as the production target: MSL value-level code is nearly Rust already, so most expressions are reprinted and only the MSL-specific forms are lowered (C4). This feature prints an IR as Rust source, compiles it with the `rustc` of the environment (C5), caches the result (C7) and hands it to the solver through the device ABI (C8).

## Goals

- [ ] An IR is printed as Rust source: value-level code reprinted, MSL-specific forms lowered (C4).
- [ ] The source is compiled with the environment's `rustc`; without `rustc` the user gets a clear error and the interpreter still runs (C5, C6).
- [ ] A design is compiled once and cached by hash; nothing is compiled during a simulation (C7, MD-18).
- [ ] The compiled design gives the results of the interpreter on the same IR (C6).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The IR model, its construction and the interpreter | features `msl-ir`, `msl-elab`, `ir-interp` |
| The device ABI | ABI duel (C8, O8); the generated code implements it and this spec does not design it |
| Shipping `rustc` inside the Piperine binary | C5, O7: "isso a gnt vê depois" |
| Any other target language or a JIT | C1, C4 |
| Optimization passes of our own | not requested; `rustc` and LLVM optimize the output |
| Cross-compilation to a target other than the host | not requested |
| `extern "osdi"` and `extern "cosim"` devices | other devices under the ABI, not transpiled IR |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision or by `docs/spec/`. Rows marked `n` are unconfirmed proposals with an entry under "Open questions (detail)".

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Target language | Rust | C4, user: "Eu aceito rust como linguagem final" | y |
| Value-level code is reprinted, MSL forms are lowered | lowered: unknown values (§3.3), SI literals, carriers, drives and commits, the analog forms (`+>`, `ddt`, `idt`, `delay`, `noise`), events and reactions | C4 | y |
| `rustc` is required in the environment | found on the machine or the build fails with a diagnostic | C5 | y |
| Compiled once per design at elaboration, cached by hash, never during a simulation | C7 as written | C7, MD-18 | y |
| Tested against the interpreter | the differential harness of `ir-interp` | C6 | y |
| Output shape (RS-Q1, O4) | one crate per module, compiled as a `cdylib` | proposal of O4, "pending the duel" | n |
| How generated code reaches the ABI (RS-Q2, O4, O8) | a C ABI function table over byte buffers | proposal of O4, pending the duel | n |
| Cache key and location (RS-Q3, O4) | key: hash of the serialized IR unit, the transpiler version, `rustc -vV` and the compile flags; location: the user cache directory | not decided | n |
| How `rustc` is driven (RS-Q4) | `rustc` invoked directly on generated files, no `cargo`, no dependency but a small runtime source file generated with them | C5 names `rustc` only | n |
| Jacobian in generated code (RS-Q5, O1) | `analog` code generic over a `Real` number type, run with dual numbers | proposal of O1 | n |
| MSL value types with no stable Rust type (RS-Q6) | `UInt<N>`/`Int<N>` as masked native integers; `Float<16>` and `Float<128>` rejected with a diagnostic | §1 rules 5 and 6; stable Rust has no `f16`/`f128` | n |
| Compile latency and default backend (RS-Q7, O5) | cache per module; the interpreter for fast iteration | proposal of O5 | n |
| Generated source is kept and readable (RS-Q8) | the `.rs` files stay in the cache beside the artifact, formatted, with MSL names and a comment naming each item's path | the user's reason for leaving Cranelift; not stated as a requirement | n |
| Trust in a cached native library (RS-Q9) | a cache entry is loaded only when its recorded hash matches its content | loading a library executes code; not discussed | n |
| Who implements and who verifies (C10) | Opus implements, Fable verifies | whether the transpiler counts as "core" is not stated | n |

**Open questions:** RS-Q1 to RS-Q9 are open and listed below. This spec is not ready for approval.

**Implicit-requirement dimensions.** Input validation and bounds: RS-01 AC 5 (an IR form with no Rust lowering). Failure states: RS-02 AC 2 and AC 3, RS-04 AC 3. Idempotency: RS-01 AC 4, RS-03 AC 1. Auth and rate limits: N/A because everything is local; the trust question is RS-Q9. Concurrency and ordering: open, RS-Q3 (two processes filling one cache entry). Data lifecycle: open, RS-Q3 (eviction and size of the cache). Observability: RS-02 AC 3 (compiler output is shown), RS-Q8. External-dependency failure: RS-02 (`rustc` missing, `rustc` failing). State-transition integrity: RS-03 AC 3 and AC 4 (no compilation while a simulation runs).

**Open questions (detail)**

- **RS-Q1 (O4). Output shape.** Options: (a) one crate per monomorphized module, linked or loaded together; (b) one crate per design; (c) one crate per design with one Rust module per MSL module. Recommendation: wait for `msl-ir` IR-Q3 and the duel, then prefer (a) for cache reuse across designs (the stdlib and device models compile once), as O4 proposes. The artifact kind (`cdylib` or other) follows RS-Q2.
- **RS-Q2 (O4, O8). Reaching the ABI.** Options: (a) C ABI function table over byte buffers; (b) Rust traits across a `dylib`, which ties the artifact to one compiler version; (c) static linking into a per-design host binary. Decided by the duel; O4 proposes (a).
- **RS-Q3 (O4). Cache.** Key contents, directory, eviction, and what two concurrent processes do with one entry. Recommendation: the key above; write to a temporary name and rename atomically; no eviction in the first version, with a `clean` command.
- **RS-Q4. Driving the compiler.** Options: (a) `rustc` directly; (b) `cargo` with a generated manifest; (c) a prebuilt runtime `rlib` shipped with Piperine that generated code links. Recommendation: (a). C5 requires `rustc`, not `cargo`; (c) ties the shipped `rlib` to one `rustc` version. Also to settle: optimization level, debug info, edition (2024 matches §0.4).
- **RS-Q5 (O1). Jacobian.** Same question as `ir-interp` INT-Q1; both backends should use one method so the oracle compares like with like.
- **RS-Q6. Types Rust does not have.** C4 says "near 1:1". The exceptions found in `docs/spec/`: `UInt<N>`/`Int<N>` for any `N` in 1..=128 with wrapping modulo `2^N` in every build (§1 rules 5 and 6), `Float<16>` and `Float<128>`, a known flag per level (§3.3), and the closed library catalogue of Appendix H (a Rust method outside it must not resolve, which elaboration enforces). Options for the floats: (a) reject; (b) software implementation in the runtime file. Recommendation: (a) until a model needs them.
- **RS-Q7 (O5). Latency.** How long a large design takes to compile, whether modules compile in parallel, and which backend a session uses by default. Recommendation: measure on the galleries before deciding; keep the choice a host option.
- **RS-Q8. Readable output.** Whether readability of the generated Rust is a requirement with a test (for example: every generated function carries the path of its IR item). Recommendation: yes; it is the stated reason for this feature.
- **RS-Q9. Trust.** A cache directory writable by others would let them run code in the simulator's process. Options: (a) content hash recorded and checked at load; (b) rely on file permissions of the user cache directory. Recommendation: (b) with (a) as a cheap guard against corruption.

---

## Dependencies & Order

| Depends on | For | State |
| ---------- | --- | ----- |
| `msl-ir` | the input | drafted with this spec |
| `msl-elab` | real IR values | drafted with this spec |
| `ir-interp` | the differential oracle (C6) and the fallback when `rustc` is absent | drafted with this spec |
| ABI duel (C8, O8) | the interface generated code implements; fixes RS-Q1 and RS-Q2 | open |
| `solver-restructure` and the solver features that follow it | the solver the compiled devices load into | planned on `feature/solver-completeness` |

Order proposed by O6 (not decided): `msl-elab`, `msl-ir` and `ir-interp` first; `ir-rust` last. This feature should not start before the conformance set runs on the interpreter, because its acceptance is equality with it.

---

## User Stories

### P1: Print an IR as Rust ⭐ MVP

**User Story**: As a maintainer, I want the IR printed as Rust source so that the compiled model is something I can read and debug with ordinary Rust tools.

**Why P1**: C4; the reason the chain changed.

**Requirement**: RS-01

**Acceptance Criteria**:

1. WHEN an IR is transpiled THEN the system SHALL produce Rust source that the `rustc` of the environment compiles without errors.
2. The system SHALL print each value-level function, struct, enum, `match` and `impl` method of the IR as the corresponding Rust item (C4).
3. The system SHALL lower each MSL-specific form of the IR: unknown values, carriers, drives, commits, contributions, the analog operators, events and reactions (C4).
4. WHEN the same IR is transpiled twice THEN the system SHALL produce byte-identical source.
5. IF an IR form has no Rust lowering THEN the system SHALL produce no source and SHALL report a diagnostic naming the form and its path.

**Independent Test**: transpile the IR of the `Resistor` example of §8 and compile the result with `rustc`.

---

### P1: `rustc` is required ⭐ MVP

**User Story**: As a user, I want a clear message when `rustc` is missing or fails so that I know what to install or report.

**Why P1**: C5.

**Requirement**: RS-02

**Acceptance Criteria**:

1. WHEN the compiled backend is selected THEN the system SHALL locate `rustc` in the environment and compile the generated source with it.
2. IF no `rustc` is found THEN the system SHALL fail before any analysis starts with a diagnostic that names `rustc` as missing and states that the interpreter backend needs no compiler.
3. IF `rustc` exits with an error on generated source THEN the system SHALL fail before any analysis starts and SHALL report the compiler's output and the location of the generated source.

**Independent Test**: select the compiled backend with an empty `PATH` and read the diagnostic.

---

### P1: Compile once, cache by hash ⭐ MVP

**User Story**: As a user, I want a design compiled once so that a second run, a sweep or a `set` never waits for a compiler.

**Why P1**: C7, MD-18.

**Requirement**: RS-03

**Acceptance Criteria**:

1. WHEN a design whose IR is unchanged is built a second time THEN the system SHALL invoke `rustc` zero times and SHALL load the cached artifact.
2. WHEN the IR of a design changes THEN the system SHALL not load an artifact compiled from the earlier IR.
3. WHILE an analysis runs the system SHALL invoke `rustc` zero times.
4. WHEN the host sets a non-structural param during or between analyses THEN the system SHALL apply it on the compiled design without transpiling or compiling (§8.1.23).
5. WHEN the host sets a structural param THEN the system SHALL compile only after the re-elaboration the host asks for (§8.1.24), under AC 1 and AC 2.

**Independent Test**: build a design twice with a `rustc` wrapper that counts invocations; the second count is 0.

---

### P1: The compiled design runs on the solver ⭐ MVP

**User Story**: As a host author, I want the compiled design loaded and presented to the solver so that it simulates like any other device.

**Why P1**: C3, C8.

**Requirement**: RS-04

**Acceptance Criteria**:

1. WHEN a compiled artifact is loaded THEN the system SHALL present its devices to the solver through the device ABI (C8), with no interface of its own.
2. The system SHALL expose from a compiled design the same metadata (§21) as the IR it was compiled from.
3. IF an artifact cannot be loaded THEN the system SHALL fail before any analysis starts with a diagnostic naming the artifact.

**Independent Test**: load the compiled resistor divider and run a `Dc` analysis.

---

### P1: Equal to the interpreter ⭐ MVP

**User Story**: As a maintainer, I want the compiled backend proven against the reference so that speed never changes a result.

**Why P1**: C6.

**Requirement**: RS-05

**Acceptance Criteria**:

1. WHEN the differential harness of `ir-interp` runs an item of the conformance set (Appendix C.2.3) on the interpreter and on the compiled backend THEN the system SHALL report "equal" under the comparison rule of `ir-interp` INT-Q3.
2. IF value-level code panics in compiled code during a run (§1 rule 11) THEN the system SHALL fail the analysis with the instance path, as the interpreter does, and SHALL not abort the process.
3. The system SHALL evaluate integer arithmetic in compiled code wrapping modulo `2^N` in every build (§1 rule 6).

**Independent Test**: run the harness on `transition` (§22.2.11) with both backends.

---

## Edge Cases

- WHEN two designs share an identical compilation unit THEN the system SHALL compile it once (pending RS-Q1 for what a unit is).
- IF the `rustc` version changes between two builds THEN the system SHALL not load an artifact compiled by the other version (pending RS-Q3 for the key).
- WHEN a design has no `analog` block THEN the system SHALL generate and compile its discrete code only.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| RS-01 | P1: Print an IR as Rust | - | Pending |
| RS-02 | P1: `rustc` is required | - | Pending |
| RS-03 | P1: Compile once, cache by hash | - | Pending |
| RS-04 | P1: The compiled design runs on the solver | - | Pending |
| RS-05 | P1: Equal to the interpreter | - | Pending |

**Coverage:** 5 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` - decisions C1 to C10 and open items O1 to O8.
- `docs/spec/` §0.4, Part I (§1, §3.3), §8.1, §21, Appendix C.2.3, Appendix H - what "near 1:1 Rust" covers and where it stops.
- `.specs/STATE.md` MD-18 - nothing is recompiled during a simulation.

---

## Success Criteria

- [ ] Every item of the conformance set runs on the compiled backend and equals the interpreter.
- [ ] A second build of an unchanged design invokes `rustc` zero times.
- [ ] Without `rustc` the compiled backend fails with the stated diagnostic and the interpreter still runs the design.

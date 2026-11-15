# IR to Rust Transpiler Specification

**Approved**: 2026-11-15 - user: "Pode ir rodando os grandes passos" (standing delegation, R4)

## Problem Statement

The interpreter (`ir-interp`) proves the chain from MSL to the solver on the M1 analog subset, and it is frozen there (R1). The production path is Rust: the IR is printed as Rust source, compiled by the `rustc` of the environment, cached, loaded and handed to the solver through the device ABI (C4, C5, C7, C8). The previous fast path, Cranelift, was retired because its output could not be read or debugged; the generated Rust is therefore the debug surface, kept on disk and readable (R3). M2 (digital, mixed-signal, the buck) is built on this path (R2), so this feature also gives the IR, the plan and the generated code the digital and mixed forms the interpreter never had.

## Goals

- [ ] The five M1 device bodies are printed as readable Rust, compiled with `rustc` alone, cached by hash and loaded through `piperine_abi_1`.
- [ ] Every function of every M1 body gives the interpreter's bytes on the compiled backend, and the seven ngspice twins pass on the compiled backend under the unchanged `ToleranceContract`.
- [ ] A second build of an unchanged design invokes `rustc` zero times; no analysis and no `set` ever invokes it.
- [ ] The digital and mixed forms the buck needs (integers, unknown values, carriers, drives, events, reactions, commits, the two crossings of the boundary) exist in the IR, in the shape plan and in the generated Rust, and behave on the solver as `docs/spec/` and `examples/buck/SPEC.md` say.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Type checking and elaboration of digital or mixed MSL source (`msl-typeck`, `msl-elab` beyond the M1 subset) | M2. Phase B of this feature is proven on IR built by hand through `IrBuilder`, as `ir-interp` was before `msl-elab` existed |
| The buck from its `.msl` sources, galleries E and F, the conformance set of Appendix C.2.3 | M2 (R2). This feature delivers what M2 stands on |
| Extending the interpreter beyond its M1 subset | R1. It gains one refusal by name and nothing else |
| The merge of effects under unknown control (§3.3.3), `match`, loops, `let mut`, structs, enums, `Option` as a value, arrays, bundles, `resolve` and resolver instances (abi.md D1) | not needed by the buck slice; each is refused by name (RS-01 AC 4), an unknown condition at run time is a fault (RS-11 AC 5) |
| `transition`, `D2A`, `A2D` and every other stdlib module as source; modules with a result; `init()`, `every`, `settle`, records and `emit!` | stdlib and front-end work of M2. The two primitive crossings they are built on are in scope (RS-12 AC 6 to 8) |
| Analog forms beyond M1: selectors, flow probes, `ddx`, `delay`, `noise`, `bound_step`, `now()`, `analysis()`, `idt` | M2 or later; refused by name |
| `UInt<N>`/`Int<N>` wider than 64 bits, `Float<16>`, `Float<128>` | RS-Q6: refused by name |
| Shipping `rustc` inside the Piperine binary; cross-compilation; a JIT; optimization passes of our own | C5, O7, C1; `rustc` optimizes the output |
| A cache eviction policy and a `clean` command | RS-Q3: the CLI crate does not build today; `Cache::clear` exists as an API |
| A default backend chosen by a heuristic | RS-Q7: the host names the backend |
| `extern "osdi"` and `extern "cosim"` devices | other devices under the ABI, not transpiled IR |

---

## Assumptions & Open Questions

Every RS-Q of the draft is settled here under R4 (`.specs/features/codegen/context.md`: the orchestrator settles the open RS-Q questions under the user's standing delegation). `y (R4 delegation)` marks an answer given that way; plain `y` marks one a recorded decision already fixed.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Target language | Rust | C4 | y |
| Compiled once per design, cached by hash, never during a simulation | as written | C7, MD-18 | y |
| Order and oracle | `ir-rust` before M2; the interpreter is the oracle on the M1 analog subset; elsewhere the oracle is the behavior `docs/spec/` and `examples/buck/SPEC.md` define | R1, R2 | y |
| RS-Q1: output shape | one generated crate per design: one source file `design.rs` with one Rust `mod` per IR body, compiled to one `cdylib` that holds one ABI `Library` with one `Module` per body | the ABI's unit is a `Library` with a list of modules (abi.md §3); the IR is one arena per design whose functions are shared by bodies, so a per-body crate would need a slicing step and a link step; one `rustc` call and one `dlopen` per design. Measured on this machine: 0.16 s for an empty `cdylib` against the prebuilt ABI `rlib`. Reuse across designs is by whole-design hash only, accepted until a design is measured slow (RS-08) | y (R4 delegation) |
| RS-Q2: reaching the ABI | the `cdylib` exports the one symbol `piperine_abi_1` (abi.md §2 entry 1); only `#[repr(C)]` data and `extern "C"` pointers cross. Generated code implements `piperine_abi::device::Device` and is wrapped by `thunk::module::<D>`, linking a `piperine_abi` `rlib` that the transpiler compiles once per compiler version from a copy of that crate's sources embedded in the transpiler | abi.md §7 "One path": the solver calls every device through `Module`'s pointers. Using the ABI crate's own lenses and thunks keeps `unsafe` and `catch_unwind` out of generated code and makes it read like the hand-written devices. `piperine-abi` has no dependency, so `rustc` alone builds it (1.2 s, measured) | y (R4 delegation) |
| RS-Q3: cache | key: SHA-256 over the generated source, the runtime source, the embedded ABI sources, the output of `rustc -vV` and the flag list; 32 hex characters. Location: `PIPERINE_CACHE_DIR`, else `$XDG_CACHE_HOME/piperine`, else `$HOME/.cache/piperine`, else `%LOCALAPPDATA%\piperine`. An entry is built in a temporary sibling directory and renamed into place; the loser of a race discards its copy. No eviction | the generated source is deterministic (RS-01 AC 3) and holds everything the IR contributes, so hashing it is exact and needs no transpiler version number. Rename is atomic on one file system. `sha2` is already in `Cargo.lock` | y (R4 delegation) |
| RS-Q4: driving the compiler | `rustc` directly, never `cargo`; found as `PIPERINE_RUSTC`, else `rustc` on `PATH`; edition 2024, `-C opt-level=2 -C debuginfo=1 --cap-lints warn`; minimum version 1.85 (the first with edition 2024) | C5 names `rustc` only. `debuginfo=1` keeps file and line of the generated source in a backtrace. A capped lint never breaks a user's build on a newer compiler; the golden sources are held to zero warnings by a test instead | y (R4 delegation) |
| RS-Q5: Jacobian | forward-mode dual numbers: every analog function body is generic over a `Real` trait, run with `Dual<N>` where `N` is the function's count of differentiable reads, a fixed array | S4. `N` is static per function, so no allocation; the formulas are those of the interpreter's `Dual`, so the two backends compare like with like | y |
| RS-Q6: types Rust lacks | `UInt<N>` and `Int<N>` for `N` in 1..=64 are runtime types over `u64`/`i64`, masked to `N` bits after every operation; `N > 64`, `Float<16>` and `Float<128>` are refused by name; `Float<32>` is `f32` | §1 rule 6 (wrap modulo `2^N` in every build). The ABI's leaves end at 64 bits (abi.md §3), so a wider integer could not be carried anyway. Stable Rust has no `f16`/`f128` | y (R4 delegation) |
| RS-Q7: latency and default backend | no default: a host constructs `Compiled` or `Interpreter` by name, and the parity suite runs both. A build reports its transpile, compile and load times and whether the cache was hit; no test asserts a duration | a time bound in a test is flaky; the numbers land in `proofs.md` and decide later whether per-body units are worth it | y (R4 delegation) |
| RS-Q8: readable output | a requirement with tests (RS-02): the source is kept beside the library, printed with fixed layout rules, named after MSL identifiers, every item under a comment with its IR path and origin; the five M1 bodies are pinned as golden files | R3: "ainda podemos olhar no resultado intermediário" | y |
| RS-Q9: trust in a cached library | the user cache directory's permissions are the trust boundary (created `0700` on Unix); each entry records the SHA-256 of its library, checked before `dlopen`; a mismatch discards and rebuilds the entry | loading a library executes code; the hash guards against corruption and truncation, not against an attacker who can write the cache, whom nothing local stops | y (R4 delegation) |
| Who implements and who verifies | Sonnet implements, Opus verifies, Opus implements the delicate tasks directly | S10, which amends C10 | y |
| Where the shape plan lives | `BodyPlan` and the netlist loop move from `piperine-interp` to a new crate `piperine-plan` both backends depend on; the interpreter re-exports `BodyPlan` and its tests stay untouched | one plan gives both backends the same pins, read order, partial order and affine flags, which is what makes S8's comparison and the linear-term rule hold by construction. Moving code extends nothing, so R1 holds | y (R4 delegation) |
| Crate name | `piperine-transpile` | `piperine-codegen` is a retired name under the `workspace_shape` guard | y (R4 delegation) |
| The linear-term rule (lesson of `solver-abi-integration` T5) | generated code writes a term affine in its continuous reads as `c + Σ partial·read`, with `c` and the partials from an evaluation at pins read as 0 and the sum folded from `0.0` in declared partial order: the interpreter's INT-12, reproduced | bit-equal right-hand sides with the hand-written devices; a constant `c ≠ 0` keeps the interpreter's 1-ulp bound (`ir-interp` verification F3) | y |
| Phase B scope | the forms of the buck's `Osc`, `Dpwm`, `RefRamp`, `Pid`, `SetpointReg`, `GateDriver`, `Adc`, the comparator under `A2D` and an analog device that reads a `Bit`; proven on hand-built IR | the caller's brief allows scoping to the buck; the front end for these forms is M2 | y (R4 delegation) |
| One known flag per scalar | a carried `bool`, integer or `f64` has one known flag, as the ABI layout has one known byte per field element; `&`, `\|`, `!` on `bool` follow §3.3.2, every other operator is all-or-nothing | abi.md §3 `Field.known`; per-bit flags of an integer are not needed by the buck and are refused with the composite types | y (R4 delegation) |
| `as` from a float to `UInt<N>`/`Int<N>` (buck `GAPS.md` G4) | saturates to the range of the `N`-bit type; NaN gives 0 | §1 rule 1 "as in Rust", read at the type's own width. The buck clamps before the cast, so it is the same under either reading. The language text should say it: reported, not blocking | y (R4 delegation) |
| A shift by `N` or more | the mathematical result truncated to `N` bits: `<<` gives 0, `>>` gives 0 for `UInt` and the sign fill for `Int` | no rule in `docs/spec/`; a result that does not depend on the storage width is the only reading consistent with §1 rule 6. Reported, not blocking | y (R4 delegation) |
| `rise()`, `fall()` in the IR | the IR holds two core event sources, `changed` of a carrier and `cross` of a real expression, each with an optional `bool` guard read at the microstep; `rise` is `changed` guarded by "known and true" | §13: `rise` is stdlib over `changed`; §12.1 ("a reaction that must ignore it says so") makes unknown to true a rise | y |
| `LATER` on a statement | set only when the commit's delay is a literal constant greater than 0, with `min_after` that constant; every other statement has no flag | abi.md C1: a generator sets `LATER` only where it can state a bound | y (R4 delegation) |

**Open questions:** none - all resolved or logged above.

**Implicit-requirement dimensions.** Input validation and bounds: RS-01 AC 4 and 5, RS-09 AC 4, RS-10 AC 5. Failure states: RS-03 AC 2 to 4, RS-04 AC 5 and 8, RS-05 AC 3, RS-06 AC 6. Idempotency: RS-01 AC 3, RS-04 AC 1. Auth and rate limits: N/A because everything is local; trust is RS-04 AC 5 and the RS-Q9 row. Concurrency and ordering: RS-04 AC 4. Data lifecycle: no eviction (RS-Q3 row); `Cache::clear` in the design. Observability: RS-02, RS-03 AC 3, RS-08. External-dependency failure: RS-03 (`rustc`); ngspice follows `msl-parity`. State-transition integrity: RS-04 AC 6 and 7.

---

## Phases of the feature

| Phase | Content | Oracle | Requirements |
| ----- | ------- | ------ | ------------ |
| A | the M1 analog subset through the transpiler | the interpreter, function by function and circuit by circuit; ngspice on the seven twins | RS-01 to RS-08 |
| B | digital and mixed forms of the buck slice | `docs/spec/` and `examples/buck/SPEC.md`, written as expected values in tests (R2) | RS-09 to RS-13 |

Phase A is complete and mergeable without phase B.

---

## User Stories

### P1: Print an IR as Rust ⭐ MVP

**User Story**: As a maintainer, I want the IR printed as Rust source so that the compiled model is ordinary Rust.

**Why P1**: C4.

**Requirement**: RS-01

**Acceptance Criteria**:

1. WHEN an `Ir` whose `check()` is empty and that holds only supported forms is transpiled THEN the system SHALL return one Rust source text that `rustc` compiles with zero errors and zero warnings.
2. The system SHALL print each IR function as one Rust `fn` generic over `Real`, and each body that runs as one Rust `mod` holding a type that implements `piperine_abi::device::Device`.
3. WHEN the same `Ir` is transpiled twice THEN the system SHALL return byte-identical source.
4. IF the `Ir` holds a form with no Rust lowering THEN the system SHALL return no source and SHALL return an error naming the form, its IR path and its origin.
5. IF the `Ir` fails `check()` THEN the system SHALL return no source and SHALL return the check's entries.

**Independent Test**: transpile the hand-built `Resistor` body and compile the text with `rustc`.

---

### P1: The generated Rust is the debug surface ⭐ MVP

**User Story**: As a maintainer, I want to open the generated source and recognise my model so that I debug with a text editor and ordinary Rust tools.

**Why P1**: R3; the reason the chain changed.

**Requirement**: RS-02

**Acceptance Criteria**:

1. The system SHALL print, on the line before every generated `mod`, `fn`, shape entry and function arm, a comment holding the item's IR path and its origin as `file:line:col`.
2. The system SHALL name each generated item after its MSL identifier, written as a raw identifier when it is a Rust keyword.
3. The system SHALL print with 4-space indentation, one statement per line, and every real literal in the shortest decimal form that parses back to the same bits.
4. WHEN a design is built THEN the system SHALL keep the generated source in the cache entry beside the library and SHALL report its path.
5. The system SHALL print the five M1 device bodies as the texts of the golden files `crates/piperine-transpile/tests/golden/*.rs`.

**Independent Test**: build the divider and open the reported `design.rs`.

---

### P1: `rustc` is required ⭐ MVP

**User Story**: As a user, I want a clear message when the compiler is missing or fails so that I know what to install or report.

**Why P1**: C5.

**Requirement**: RS-03

**Acceptance Criteria**:

1. WHEN a design is built THEN the system SHALL run the compiler named by `PIPERINE_RUSTC`, or `rustc` found on `PATH` when that variable is unset, and SHALL run no `cargo`.
2. IF no compiler is found THEN the system SHALL fail before any analysis with an error that names `rustc` as missing and states that the interpreter backend needs no compiler.
3. IF the compiler exits with an error THEN the system SHALL fail before any analysis with an error holding the compiler's standard error and the path of the generated source, which stays on disk.
4. IF the compiler's version is below 1.85 THEN the system SHALL fail with an error naming the version found and the version required.

**Independent Test**: build with `PIPERINE_RUSTC` pointing at a file that does not exist.

---

### P1: Compile once, cache by hash ⭐ MVP

**User Story**: As a user, I want a design compiled once so that a second run, a sweep or a `set` never waits for a compiler.

**Why P1**: C7, MD-18.

**Requirement**: RS-04

**Acceptance Criteria**:

1. WHEN a design whose generated source is unchanged is built a second time THEN the system SHALL invoke the compiler zero times and SHALL load the cached library.
2. WHEN a change to the `Ir` changes the generated source THEN the system SHALL build and load a different cache entry.
3. WHEN the output of `rustc -vV` differs between two builds THEN the system SHALL use different cache entries.
4. WHEN two builds of one design run at the same time on an empty cache THEN the system SHALL complete both and SHALL leave one complete entry.
5. IF the bytes of a cached library do not match the hash its entry records THEN the system SHALL not load it and SHALL rebuild the entry.
6. WHILE an analysis runs the system SHALL invoke the compiler zero times.
7. WHEN the host sets a non-structural param between analyses THEN the system SHALL apply it on the loaded design and SHALL invoke the compiler zero times.
8. IF the cache directory cannot be created or written THEN the system SHALL fail before any analysis with an error naming the directory.
9. The system SHALL place the cache under `PIPERINE_CACHE_DIR`, else `$XDG_CACHE_HOME/piperine`, else `$HOME/.cache/piperine`, else `%LOCALAPPDATA%\piperine`.

**Independent Test**: build a design twice with a compiler wrapper that counts invocations; the second count is 0.

---

### P1: The compiled design runs on the solver ⭐ MVP

**User Story**: As a host author, I want the compiled design loaded and presented to the solver so that it simulates like any other device.

**Why P1**: C3, C8.

**Requirement**: RS-05

**Acceptance Criteria**:

1. WHEN a library is loaded THEN the system SHALL read its modules through the symbol `piperine_abi_1` only and SHALL bind each instance of the design with `CircuitBuilder::device`, as the interpreter's circuit does.
2. The system SHALL give every loaded module a shape equal, entry by entry, to the shape `piperine-plan` gives the interpreter for the same body.
3. IF a library cannot be opened, lacks the symbol, or reports a `Library.size` or `minor` the host does not accept THEN the system SHALL fail before any analysis with an error naming the file.
4. The system SHALL keep every `unsafe` block of the feature in `crates/piperine-transpile/src/load.rs`, and none in generated source.

**Independent Test**: load the compiled resistor divider and run a `Dc` analysis.

---

### P1: Equal to the interpreter ⭐ MVP

**User Story**: As a maintainer, I want the compiled backend proven against the reference so that speed never changes a result.

**Why P1**: C6, S8.

**Requirement**: RS-06

**Acceptance Criteria**:

1. WHEN any function of the five M1 bodies is evaluated on both backends with the same input bytes, every `AGE` and `STATIC` combination of a `limit` included, THEN the system SHALL write output cells with equal bits and seeded partials that compare equal as `f64`.
2. The system SHALL pass `piperine_abi::verify::verify_partials` with `rel_tol` 1e-6 for every function of every M1 body in compiled form.
3. The system SHALL write a term affine in its continuous reads as `c + Σ partial·read`, folded from `0.0` in declared partial order, so that the compiled resistor's value has the bits of `g·vp − g·vn` and leaves a right-hand side of exactly 0.
4. WHEN an affine term has a constant `c ≠ 0` THEN the system SHALL leave a right-hand side within 1 ulp of `max(|c|, |Σ|)` of `−c`.
5. WHEN a circuit of the M1 fixtures runs a `Dc`, `Ac` or transient analysis on both backends THEN the system SHALL give results that agree under the `ToleranceContract` of the analysis's domain.
6. IF value-level code panics in compiled code THEN the system SHALL fail the solve with a fault carrying the panic message and SHALL keep the process and the instance usable.
7. WHEN a compiled function computes a non-finite value THEN the system SHALL write it as it is, as the interpreter does.

**Independent Test**: evaluate the diode's `terms` function at 0.6 V on both backends and compare bytes.

---

### P1: ngspice parity on the compiled backend ⭐ MVP

**User Story**: As a maintainer, I want the seven twins to pass through generated Rust so that the production path carries the project's main strength.

**Why P1**: "a paridade com ngspice é um strenght bem alto" (MD-60 item 4); the brief's top priority.

**Requirement**: RS-07

**Acceptance Criteria**:

1. WHEN each of the seven twins of `tests/ngspice/` is loaded through the compiled backend THEN the system SHALL agree with ngspice under the `ToleranceContract` of its domain.
2. The system SHALL run every parity test once per backend and SHALL name the backend in every rejection.
3. The system SHALL leave every bound of `ToleranceContract` at its M1 value.
4. WHERE `PIPERINE_BACKEND` is set to `interp` or `rust` the system SHALL run that backend only.
5. IF `PIPERINE_BACKEND` holds any other value THEN the system SHALL fail the test naming the two valid values.

**Independent Test**: `PIPERINE_BACKEND=rust cargo test -p piperine-parity`.

---

### P2: Build report

**User Story**: As a maintainer, I want each build to say what it cost so that the unit of compilation is revisited on numbers.

**Why P2**: RS-Q7.

**Requirement**: RS-08

**Acceptance Criteria**:

1. WHEN a design is built THEN the system SHALL report the durations of transpiling, compiling and loading, the count of compiler invocations and whether the cache was hit.

**Independent Test**: print the report of a cold and a warm build of `diode_iv`.

---

### P1: The IR holds the digital forms of the buck slice

**User Story**: As the author of M2's elaborator, I want IR nodes for integers, unknown values, carriers, drives, events and reactions so that elaboration has a target and the backends an input.

**Why P1**: R2; the interpreter's IR has no digital node.

**Requirement**: RS-09

**Acceptance Criteria**:

1. The system SHALL represent in the IR the value types `UInt<N>` and `Int<N>` for `N` in 1..=64, integer literals, wrapping arithmetic, comparisons, `&`, `|`, `^`, `!`, shifts, `as` among integers and `f64`, and the methods `min`, `max`, `clamp` and `floor`.
2. The system SHALL represent in the IR a value that may be unknown, the literal `Unknown`, and the reads `known_or`, `is_known` and `high`.
3. The system SHALL represent in the IR, per body, discrete carriers (port or signal, direction, value type, initial value), drives with an optional `after`, events (`changed` of a carrier or `cross` of a real expression, each with an optional guard), and reactions holding `let`, `if` and commits with an optional `after`.
4. IF a body commits to or drives a carrier it has no right to write, gives one carrier both a drive and a commit, or reads a carrier in `analog` outside `known_or`, `is_known` and `high` THEN the system SHALL report one `check()` entry per defect, naming the item and its origin.
5. The system SHALL leave the printed IR of every M1 fixture byte-identical.
6. IF an `Ir` holds a body with a carrier, a drive or a reaction THEN the system SHALL return `Rejected` from `Interpreter::new`, naming the body.

**Independent Test**: build the `Dpwm` body through `IrBuilder` and print it.

---

### P1: Integers wrap

**User Story**: As a model author, I want `u9` arithmetic to behave as nine bits so that a counter written in MSL is the counter that runs.

**Why P1**: §1 rule 6; every digital module of the buck.

**Requirement**: RS-10

**Acceptance Criteria**:

1. The system SHALL evaluate `+`, `-`, `*`, `/`, `%` and unary `-` on `UInt<N>` and `Int<N>` modulo `2^N`, `Int<N>` in two's complement, for every `N` in 1..=64, a zero divisor being a panic (§1 rule 11).
2. The system SHALL evaluate `as` between integer types by truncating to the target width and extending by the source's signedness.
3. The system SHALL evaluate `as` from `f64` to an integer type by saturating to the target's range, with NaN giving 0.
4. The system SHALL evaluate a shift by `N` or more as the mathematical result truncated to `N` bits.
5. IF an `Ir` holds `UInt<N>` or `Int<N>` with `N > 64`, `Float<16>` or `Float<128>` THEN the system SHALL return no source and SHALL return an error naming the type and its path.

**Independent Test**: `UInt<9>` 511 + 1 is 0 in a compiled function.

---

### P1: Unknown values

**User Story**: As a model author, I want an unread carrier to be unknown until written so that a model says what an unknown means.

**Why P1**: §3.3; `known_or` is on every input of the buck's controllers.

**Requirement**: RS-11

**Acceptance Criteria**:

1. WHEN an arithmetic operator, a comparison or an `as` has an unknown operand THEN the system SHALL give an unknown result.
2. The system SHALL evaluate `&`, `|` and `!` on `bool` per §3.3.2: `unknown & false` is `false`, `unknown | true` is `true`, every other case with an unknown operand is unknown.
3. The system SHALL evaluate `known_or(d)` as the value when known and `d` otherwise, `is_known()` as the flag, and `high()` as "known and true".
4. WHEN an unknown value is written to a pin THEN the system SHALL write its known byte as 0 and its value bytes as 0.
5. IF the condition of an `if` is unknown at run time THEN the system SHALL return a fault naming the origin of the `if` and SHALL make no commit of that reaction.
6. WHEN the delay of a commit is unknown THEN the system SHALL commit `Unknown` at the next microstep (§3.3.3).

**Independent Test**: a reaction over an uninitialised `Bit<u8>` input commits `data.known_or(7)` as 7.

---

### P1: Digital and mixed shapes

**User Story**: As a maintainer, I want every digital form mapped to the ABI in one place so that the solver sees pins, functions and reactions and nothing else.

**Why P1**: abi.md §4 to §6.

**Requirement**: RS-12

**Acceptance Criteria**:

1. The system SHALL declare each carrier as a `DISCRETE` pin, `PORT` for a port and `PRIVATE` for a signal, with one field `v` and a known byte: `BIT` for `bool`, `REAL` for `f64`, and for `UInt<N>`/`Int<N>` the narrowest of `U8` to `U64` or `I8` to `I64` that holds `N` bits; an initial value gives `HAS_INIT` with its bytes.
2. The system SHALL declare each drive without `after` as one discrete function that reads the pins of its dependency set and writes the carrier's pin.
3. The system SHALL declare each event as a `PRIVATE` `EVENT` pin written by one discrete function that reads `VALUE` and `CHANGED` of its carrier and the pins of its guard.
4. The system SHALL declare each reaction as one ABI reaction on its events' pins, reading the `VALUE` of every pin its body reads, with one statement per commit in source order, flagged `LATER` with `min_after` only when the delay is a literal constant greater than 0.
5. The system SHALL declare a drive with `after` as a private pin holding the undelayed value, a `changed` event on it and a reaction committing that value after the delay.
6. The system SHALL declare a `cross` event as a `CROSS` watch firing its event pin, written by one analog function with a partial against each continuous pin of its dependency set.
7. WHEN an `analog` block reads a carrier THEN the system SHALL declare the carrier's pin as a read of the `terms` function.
8. WHEN a reaction probes THEN the system SHALL declare the probed continuous pins as reads of the reaction.
9. The system SHALL give every body of the buck slice a shape that passes `piperine_abi::validate`.

**Independent Test**: print the shape of the hand-built D flip-flop and compare with abi.md §11.4.

---

### P1: Digital and mixed behavior on the solver

**User Story**: As the author of M2, I want reactions, commits and the two crossings running through generated Rust so that the buck needs only its front end.

**Why P1**: R2.

**Requirement**: RS-13

**Acceptance Criteria**:

1. WHEN the oscillator of the buck (`f` = 200 MHz, `clk` initially `false`) runs for 100 ns THEN the system SHALL change `clk` at `k × 2.5 ns` for `k` = 1 to 40 and at no other time.
2. WHEN two commits of one reaction exchange two carriers (`a <- b; b <- a;`) THEN the system SHALL exchange their values at the next microstep (§12.1.1).
3. WHEN two statements of one reaction commit to one carrier for the same microstep THEN the system SHALL keep the value of the later statement (§12.1.2).
4. WHEN one reaction commits `p <- true after t` and `p <- false after t + w` through two statements THEN the system SHALL hold `p` true from `t` to `t + w` after the event (§12.1.4).
5. WHEN a commit leaves a carrier's value as it was THEN the system SHALL fire no `changed` of that carrier (§12.1.18).
6. WHEN the `Dpwm` body runs with `run` true and duty word 1325 for 8 periods of 400 counts THEN the system SHALL hold `hs` true for 166 counts in the periods whose bit-reversed frame index is below 5 and 165 counts in the others, hold `ls` true from 4 counts after `hs` falls to count 396, and never hold both (buck SPEC §5).
7. WHEN the `RefRamp` and `Pid` bodies run a scripted sequence of `reference` and `meas` codes THEN the system SHALL commit, sample by sample, the `duty`, `integ`, `deriv` and `e_prev` values of the difference equations of buck SPEC §6.2 to §6.4 and §7.2, bit for bit.
8. WHEN the `Adc` body sees a rising `sample` with its input held at `V` THEN the system SHALL commit `code` = `floor(V / vref × 1024 + 0.5)` clamped to 0..=1023 after `tconv`, with `done` true from `tconv` to `tconv + tstrobe` (buck SPEC §4.1).
9. WHEN the comparator body watches an RC charge and then its discharge THEN the system SHALL set its output true at a time where the analytic input equals `vth + hyst / 2`, and false at a time where it equals `vth − hyst / 2`, each under the transient `ToleranceContract`.
10. WHEN a carrier read by an `analog` block changes at time `t0` THEN the system SHALL make `t0` a time point and SHALL give the analog response from `t0`, agreeing with the analytic RC response under the transient `ToleranceContract`.
11. WHEN one circuit of the buck slice runs twice THEN the system SHALL give the same sequence of pin changes, bit for bit and tick for tick.

**Independent Test**: run the compiled oscillator into the compiled `Dpwm` and count `hs` per period.

---

## Edge Cases

- WHEN a design has no body that runs THEN the system SHALL build a library with zero modules and a circuit with zero devices.
- WHEN a body has no `analog` block THEN the system SHALL generate its discrete code only.
- IF two builds race for one cache entry THEN the system SHALL leave no temporary directory behind after both finish.
- WHEN an MSL identifier is a Rust keyword THEN the system SHALL print it as a raw identifier and SHALL keep the ABI name unchanged.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| RS-01 | P1: Print an IR as Rust | Tasks | In Tasks |
| RS-02 | P1: The generated Rust is the debug surface | Tasks | In Tasks |
| RS-03 | P1: `rustc` is required | Tasks | In Tasks |
| RS-04 | P1: Compile once, cache by hash | Tasks | In Tasks |
| RS-05 | P1: The compiled design runs on the solver | Tasks | In Tasks |
| RS-06 | P1: Equal to the interpreter | Tasks | In Tasks |
| RS-07 | P1: ngspice parity on the compiled backend | Tasks | In Tasks |
| RS-08 | P2: Build report | Tasks | In Tasks |
| RS-09 | P1: The IR holds the digital forms of the buck slice | Tasks | In Tasks |
| RS-10 | P1: Integers wrap | Tasks | In Tasks |
| RS-11 | P1: Unknown values | Tasks | In Tasks |
| RS-12 | P1: Digital and mixed shapes | Tasks | In Tasks |
| RS-13 | P1: Digital and mixed behavior on the solver | Tasks | In Tasks |

**Coverage:** 13 total, 13 mapped to tasks, 0 unmapped.

---

## Sources

- `.specs/features/codegen/context.md` - C1 to C10, S1 to S10, R1 to R4, and the linear-term lesson of `solver-abi-integration` T5.
- `.specs/features/device-abi/abi.md` - the contract generated code implements: §2 entry points, §3 shape, §4 functions and cells, §5 reactions, §6 commits, §7 the Rust side, §11.4 to §11.6 the digital and mixed examples.
- `.specs/features/ir-interp/design.md` and `verification.md` - the mapping from a body to a shape, the affine rule (INT-12) and its 1-ulp bound (F3), the `limit` rules (INT-10).
- `.specs/features/msl-parity/spec.md` and `crates/piperine-parity/src/contract.rs` - the tolerance contract, never retuned.
- `docs/spec/` Part I §1 and §3.3, Part II §5, Part IV §12 and §13, Part V §15 - integers, unknown values, carriers, reactions, events, the instant.
- `examples/buck/SPEC.md` §4 to §7 and `examples/buck/src/*.msl` - the expected behavior of the phase B fixtures; `examples/buck/GAPS.md` G4.
- A spike on 2026-11-15 (`rustc` 1.98.0): `piperine-abi` builds as an `rlib` with `rustc` alone in 1.2 s; a `cdylib` exporting `piperine_abi_1` links against it in 0.16 s.

---

## Success Criteria

- [ ] `cargo test -p piperine-parity` is green with both backends on all seven twins, with `contract.rs` unchanged.
- [ ] Every function of every M1 body is bit-equal on the two backends.
- [ ] A warm build invokes `rustc` zero times; a build without `rustc` fails with the stated error while the interpreter still runs.
- [ ] The generated source of the five M1 bodies is pinned as golden files a reader can follow from MSL line to Rust line.
- [ ] The buck-slice circuits (oscillator, `Dpwm`, `RefRamp` with `Pid`, `Adc`, comparator, switched analog device) behave as their checks state on the compiled backend.

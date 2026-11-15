# MSL Elaboration (POM to IR) Specification

**Status**: Draft. Not approved. Open questions ELAB-Q1 to ELAB-Q7 block approval.

## Problem Statement

`piperine_lang::pom` gives an immutable `Design` that mirrors the source: no types, no evaluation, no unrolling, no instance copies (`docs/pom/index.md`, "What the POM does not do"). The language defines an elaboration phase that evaluates const generics, checks params, runs generation, places every instance and applies every static check (§0.3, §8 rule 8, §16 step 1). Nothing implements it for MSL, so no MSL model can reach the solver. This feature is that phase: `Design` in, IR out (C3).

## Goals

- [ ] One call turns a `Design`, a top module and param values into an IR (feature `msl-ir`), or into diagnostics and no IR.
- [ ] A model that meets an elaboration-phase error condition of `docs/spec/` is rejected with its error code and a source location or path (§0.3 items 3 and 4).
- [ ] The `Design` is never modified (C3).
- [ ] The IR carries the metadata of §21, so a host lists params, paths and streams without running the model.
- [ ] A construct elaboration cannot lower fails with a diagnostic; nothing is dropped silently (project rule "fail loud").

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The IR's storage, handles, serde and printer | feature `msl-ir` |
| Running the IR | features `ir-interp` and `ir-rust` |
| The device ABI and how a design is handed to the solver | ABI duel (C8, O8); a dependency, not designed here |
| Package, parse and name-resolution diagnostics | already produced by `piperine-project` and the POM (`E0161` and the POM's resolution codes) |
| Run-phase and host-phase errors (`E05xx`, `E08xx`, run rows of other parts) | raised by the backends and the host |
| `extern "osdi"` and `extern "cosim"` modules (§8.5) | their runtime form is the ABI's (O8, G.65); see ELAB-Q6 |
| Gradients across events (§23.3) beyond listing gradient params (§21 rule 7) | deferred in the solver context too |
| Incremental re-elaboration | not requested; a structural `set` re-runs elaboration (§8.1.24) |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision, by `docs/spec/` or by POM code. Rows marked `n` are unconfirmed proposals with an entry under "Open questions (detail)".

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Elaboration reads a `Design` and writes an IR; the POM stays as written | shared borrow in, IR out | C3; POM "one rule" | y |
| What elaboration does | evaluates const generics, checks params, runs generation, places every instance; type checking, name resolution and every static check belong to it | §8 rule 8, §0.3 Resolution | y |
| A model with an elaboration-phase error is rejected | diagnostics, no IR | §0.3 item 3 | y |
| A structural `set` re-runs elaboration; a non-structural `set` does not | elaboration takes param values as input; non-structural params stay run-time values in the IR | §8.1.2, §8.1.23, §8.1.24, §21 rule 4, C7 | y |
| The standard library is ordinary source | elaborated like user code | §22, §14.13 | y |
| The elaborator, not the solver, computes which reactions and carriers can touch `analog` | one flag per reaction and per carrier | solver-completeness context Q8 (user round 2), C9 | y |
| Who type-checks (ELAB-Q1) | a separate feature before this one fills the POM's `Deferred` slots and gives every expression a type | `msl-pom` listed type checking as "next feature"; no feature exists; not decided | n |
| What the IR keeps of the hierarchy (ELAB-Q2, O3) | flat per-design IR with a name map back to POM handles | proposal of O3 | n |
| Monomorphization and inlining policy (ELAB-Q3, O3) | one body per distinct set of generic and structural-param values; functions kept as functions, not inlined | C4 says value-level code is reprinted as Rust; Appendix C.1.1 (informative) says "every function is inlined"; the two disagree | n |
| The lowered form of each construct (ELAB-Q4, O2) | follows the node set of `msl-ir` IR-Q1 | open there | n |
| Which subset ships first (ELAB-Q5) | the conformance set of Appendix C.2.3 plus the galleries of Appendices D to F, in that order | C.2.3: "the first compiler pass SHOULD run the following as its conformance set" | n |
| `extern` modules in the first version (ELAB-Q6) | rejected with a diagnostic naming the module | their runtime form waits for O8 | n |
| Crate (ELAB-Q7) | a module of `piperine-lang` (`piperine_lang::elab`) depending on the IR crate | elaboration is a language phase; no decision recorded | n |
| Who implements and who verifies (C10) | Opus implements, Fable verifies | whether elaboration counts as "core" is not stated | n |

**Open questions:** ELAB-Q1 to ELAB-Q7 are open and listed below. This spec is not ready for approval.

**Implicit-requirement dimensions.** Input validation and bounds: ELAB-02, ELAB-03 (params, generation bounds, recursion). Failure states: ELAB-01 AC 3 and AC 4, ELAB-06. Idempotency: ELAB-01 AC 5. Auth and rate limits: N/A because elaboration is an in-process function. Concurrency and ordering: ELAB-03 AC 4 and ELAB-07 AC 3 (the orders §21 fixes). Data lifecycle: N/A because the IR is owned by the caller. Observability: ELAB-05 (diagnostics carry code and location). External-dependency failure: ELAB-03 AC 6 (`include_str!` file unreadable). State-transition integrity: ELAB-04 (structural or not is fixed at elaboration).

**Open questions (detail)**

- **ELAB-Q1. Type checking.** The POM has no expression types and leaves members `Deferred`. §0.3 places type checking in the elaboration phase. Options: (a) a separate feature (`msl-types`) that runs on the `Design` before this one; (b) type checking inside `msl-elab`, interleaved with monomorphization; (c) no checker of our own: lean on `rustc` rejecting the transpiled code. Recommendation: (a). (c) fails because the interpreter must run with no `rustc` (C6) and diagnostics must name MSL source (§0.3.4). (b) makes this feature very large. This question changes the size of the feature and its place in the order.
- **ELAB-Q2 (O3). Hierarchy kept in the IR.** Same question as `msl-ir` IR-Q3. This spec requires only the paths of §21.
- **ELAB-Q3 (O3). Monomorphization and inlining.** Options: (a) one body per distinct variant, functions kept; (b) full inlining and one body per instance, as Appendix C.1.1 describes for AIR v2; (c) one body per module, generics kept and resolved by the backends. Recommendation: (a). It keeps the transpiler's output close to the source (C4) and the compiled size proportional to the number of distinct modules, not instances (O5). Value inputs, results and named expressions still need the hoisting of §8.3.
- **ELAB-Q4 (O2). Lowered forms.** Decided with `msl-ir` IR-Q1; the two features should share one design pass.
- **ELAB-Q5. First subset.** Options: (a) conformance set then galleries; (b) analog only first (resistor, capacitor, diode) for early ngspice parity; (c) the whole language before anything runs. Recommendation: (a), with an analog slice of (b) as its first milestone, since ngspice parity is the stated strength.
- **ELAB-Q6. `extern` modules.** Options: (a) reject until the ABI defines external devices; (b) represent them in the IR now as opaque instances. Recommendation: (a).
- **ELAB-Q7. Crate.** Options: (a) `piperine_lang::elab`; (b) a new crate `piperine-elab`. Recommendation: (a) unless the IR crate must not be a dependency of the language crate.

---

## Dependencies & Order

| Depends on | For | State |
| ---------- | --- | ----- |
| `msl-pom` | the `Design` | done |
| type checking (ELAB-Q1) | expression types, `Deferred` members, trait and generic resolution | no feature exists |
| `msl-stdlib` | the §22 sources (`Bit`, `Electrical`, `above`, `idt`, `transition`, ...) that every model of the conformance set uses | in progress on `feature/msl-stdlib` |
| `msl-ir` | the output type | drafted with this spec |
| `solver-restructure` | removes the old `piperine-codegen` this replaces (C1) | planned |
| ABI duel (C8, O8) | what a device must expose (state, events, small-signal, noise, introspection, external devices) can add items elaboration must compute | open |

Order proposed by O6 (not decided): `msl-elab`, `msl-ir` and `ir-interp` first, `ir-rust` after. Inside the group: `msl-ir` (model), then `msl-elab`, then `ir-interp`; `msl-elab` cannot be tested end to end until `ir-interp` runs, so its own tests are golden tests of the printed IR.

---

## User Stories

### P1: One call from a design to an IR ⭐ MVP

**User Story**: As a host author, I want one call that elaborates a top module with given param values so that I get an IR or the reasons there is none.

**Why P1**: C3; the entry point of the chain.

**Requirement**: ELAB-01

**Acceptance Criteria**:

1. WHEN elaboration is called with a `Design` without diagnostics, the name of a top module and a set of param values THEN the system SHALL return an IR of that module's elaborated design.
2. The system SHALL take the `Design` by shared borrow and leave `Design::dump()` byte-identical after the call.
3. IF the `Design` holds a diagnostic of phase package, parse or elaboration THEN the system SHALL return no IR and SHALL return those diagnostics.
4. IF the named top module does not exist in the `Design` THEN the system SHALL return no IR and one diagnostic naming the module.
5. WHEN elaboration is called twice with the same `Design`, top module and param values THEN the system SHALL return two IR values with byte-identical printed texts.

**Independent Test**: elaborate the `Resistor` example of §8 and compare the printed IR with a golden file.

---

### P1: Params and constants ⭐ MVP

**User Story**: As a model author, I want params bound, defaulted and checked at elaboration so that a bad value is refused before any solve.

**Why P1**: §8 rule 8, §8.1.

**Requirement**: ELAB-02

**Acceptance Criteria**:

1. WHEN a param has no value given by the caller THEN the system SHALL bind it to its default evaluated as a const expression.
2. IF a param with no default is given no value THEN the system SHALL reject the model with `E0319` (§8.1.17).
3. IF a bound param value violates the param's `where` condition THEN the system SHALL reject the model with `E0317` and SHALL name the param's path.
4. IF an environment function of §14.4 appears in a const context THEN the system SHALL reject the model with `E0323`.
5. IF evaluating a const expression panics (§1 rule 11) THEN the system SHALL reject the model with an elaboration error at that expression.
6. The system SHALL evaluate every const generic argument and array length to a known value before placing instances (§3.3.8).

**Independent Test**: elaborate a module whose param has `where r > 0.0` with `r = -1.0` and read the diagnostic.

---

### P1: Generation and instance placement ⭐ MVP

**User Story**: As a model author, I want generation unrolled and every instance placed so that the elaborated netlist is the one my source describes.

**Why P1**: §8 rule 8, §8.2.7, §8.3.

**Requirement**: ELAB-03

**Acceptance Criteria**:

1. WHEN a body or a `digital` block holds a generation `for` THEN the system SHALL produce one copy of its items per iteration, in iteration order, indexed from 0 (§8.2.23).
2. WHEN a generation `if` has a const condition THEN the system SHALL keep the items of the taken arm and no item of the others.
3. IF a generation range or condition is not a const expression THEN the system SHALL reject the model with `E0340`.
4. The system SHALL place one instance for every placement reached after generation, core modules of §14 included, and name it by §21 rules 1 to 3.
5. IF a module places itself through generation without end THEN the system SHALL reject the model with `E0341` and SHALL name the module (§8.2.7).
6. IF the file named by an `include_str!` cannot be read THEN the system SHALL reject the model with a diagnostic naming the path resolved against the source file (§20).

**Independent Test**: elaborate a module with `for k in 0..3 { let r = Resistor { r: 1k }(a, b); }` and list the paths `r[0]`, `r[1]`, `r[2]`.

---

### P1: Structural and run-time params ⭐ MVP

**User Story**: As a host author, I want to know which params fix structure so that a `set` of the others needs no re-elaboration and no recompilation.

**Why P1**: §8.1.2, §8.1.23, §8.1.24, C7 (MD-18).

**Requirement**: ELAB-04

**Acceptance Criteria**:

1. The system SHALL mark a param as structural in the IR when generation, a const generic argument, an array length, a template or the trip count of a loop that places instances reads it, or when it alone selects the kind of a branch (§8.1.2).
2. The system SHALL keep every non-structural param in the IR as a value read at run time, never folded into the expressions that read it.
3. WHEN elaboration is called again with a different value of a structural param THEN the system SHALL return an IR elaborated with that value and new metadata (§21 rule 4).

**Independent Test**: elaborate a ladder whose length is a param with two lengths and compare the instance counts; check the resistance param is printed as a run-time read.

---

### P1: Static checks of the elaboration phase ⭐ MVP

**User Story**: As a model author, I want every static rule of the language checked so that a non-conforming model never runs.

**Why P1**: §0.3 item 3. Staging is ELAB-Q5; type rules depend on ELAB-Q1.

**Requirement**: ELAB-05

**Acceptance Criteria**:

1. IF a model meets an error condition whose phase is elaboration in an error table of Parts I to VI THEN the system SHALL reject the model and report that condition's code.
2. The system SHALL attach to every diagnostic the source location or the hierarchical path the condition names (§0.3 item 4).
3. IF an `analog` block reads a discrete carrier other than through `level` or a flag-total function (§3.3.5 to §3.3.7) THEN the system SHALL reject the model with `E0150` and name the carrier.
4. IF a core primitive is written where the §14 table does not allow it THEN the system SHALL reject the model with `E0450`.
5. WHEN a model raises no elaboration error under `docs/spec/` THEN the system SHALL return an IR and no error diagnostic.

**Independent Test**: one negative fixture per error code, each asserting the code and the span.

---

### P1: Nothing is dropped silently ⭐ MVP

**User Story**: As a maintainer, I want an unsupported construct to stop elaboration so that a partial implementation never gives a wrong simulation.

**Why P1**: project rule "fail loud"; the staged delivery of ELAB-Q5 depends on it.

**Requirement**: ELAB-06

**Acceptance Criteria**:

1. IF a construct of a conforming model has no lowering yet THEN the system SHALL return no IR and one diagnostic naming the construct and its source location.
2. The system SHALL represent in the IR every definition, commit, contribution, reaction, event, `emit!` site and core-primitive placement of the elaborated design.

**Independent Test**: elaborate a model with an `extern "osdi"` module (pending ELAB-Q6) and read the diagnostic.

---

### P2: Metadata

**User Story**: As a host author, I want the metadata of §21 from the IR so that I list and address items without running the model.

**Why P2**: §21; needed by the host, not by the first solve.

**Requirement**: ELAB-07

**Acceptance Criteria**:

1. The system SHALL record in the IR, for every param, its type, default, `where` condition, structural flag and `///` text, and SHALL list the implicit params `m` and `dtemp` with every instance (§21 rule 8).
2. The system SHALL record the path of every node, branch, carrier, drive and instance of the elaborated design.
3. The system SHALL record every `emit!` site with its stream name relative to its instance, its field names and its field types (§19, §21).
4. The system SHALL record every named `let` of an `analog` block as an operating-point variable (§11 rule 6).
5. The system SHALL record the set of gradient params found by dependency analysis (§21 rule 7).

**Independent Test**: elaborate the `Front` model of §0.7 and list its params, paths and streams from the IR alone.

---

### P2: Boundary reachability

**User Story**: As the solver, I want to know which reactions and carriers can touch `analog` so that only their scheduled times become time points.

**Why P2**: §17.2.2; solver-completeness context Q8 assigns the analysis to the elaborator.

**Requirement**: ELAB-08

**Acceptance Criteria**:

1. The system SHALL flag in the IR every carrier and param that an `analog` block reads, directly or through definitions, `let`s and value inputs.
2. The system SHALL flag in the IR every reaction that can change a flagged carrier or that reads a probe (§12.1.5, §14.1.5).

**Independent Test**: in the `Front` model, the reactions on `above(..)` are flagged and `on clk.rise()` is not.

---

### P2: Dependency order of definitions

**User Story**: As a backend author, I want definitions and `resolve`s ordered at elaboration so that a microstep settles them in dependency order.

**Why P2**: §15 step 3.2, §8.2.3.

**Requirement**: ELAB-09

**Acceptance Criteria**:

1. The system SHALL record in the IR an evaluation order of definitions and `resolve`s in which each is evaluated after the definitions it reads.
2. WHEN definitions among resolved drives form a loop THEN the system SHALL record the loop as one group to be relaxed (§8.2.3).
3. IF definitions form a loop through a one-driver carrier THEN the system SHALL reject the model with `E0339` (§8.2.3).

**Independent Test**: elaborate the `Tran` on `Wire`s example (§22.3.7) and read one relaxation group in the printed IR.

---

## Edge Cases

- WHEN a generation `for` has an empty range THEN the system SHALL produce no item and no error.
- IF a decision that fixes structure has an unknown level (§3.3.8) THEN the system SHALL reject the model with `E0151`.
- WHEN an instance has no `let` of its own THEN the system SHALL name it `<module>#<k>` with `k` counted as §21 rule 3 states.
- IF a param value given by the caller names no param of the design THEN the system SHALL return no IR and a diagnostic naming the path.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| ELAB-01 | P1: One call from a design to an IR | - | Pending |
| ELAB-02 | P1: Params and constants | - | Pending |
| ELAB-03 | P1: Generation and instance placement | - | Pending |
| ELAB-04 | P1: Structural and run-time params | - | Pending |
| ELAB-05 | P1: Static checks of the elaboration phase | - | Pending |
| ELAB-06 | P1: Nothing is dropped silently | - | Pending |
| ELAB-07 | P2: Metadata | - | Pending |
| ELAB-08 | P2: Boundary reachability | - | Pending |
| ELAB-09 | P2: Dependency order of definitions | - | Pending |

**Coverage:** 9 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` - decisions C1 to C10 and open items O1 to O8.
- `docs/spec/` §0.3, §3.3, §8, §14, §16 step 1, §17.2.2, §19 to §21, Appendix C.1 and C.2.3 - the elaboration phase.
- `docs/pom/` and `crates/piperine-lang/src/pom/` - what the input holds and does not hold.
- `feature/solver-completeness:.specs/features/solver-completeness/context.md` Q8 - the elaborator computes boundary reachability.

---

## Success Criteria

- [ ] Every model of the conformance set (Appendix C.2.3) elaborates to an IR whose printed text is pinned by a golden test.
- [ ] Every elaboration-phase error code in scope has a negative fixture that asserts the code and the location.
- [ ] No test finds a `Design` changed by elaboration.

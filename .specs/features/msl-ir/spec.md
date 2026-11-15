# MSL IR (object model) Specification

**Status**: Draft. Not approved. Open questions IR-Q1 to IR-Q7 block approval.

## Problem Statement

The POM mirrors the source and stops before elaboration: it holds a generation `for` as one `for`, evaluates nothing and places no instance copy. The solver needs the elaborated design in a form two backends can execute (C3: an interpreter and a Rust transpiler). The previous form, Cranelift over the old `piperine-ir`, is retired (C1) because it was too low level to debug, inspect and verify. This feature defines the IR as an object model in the style of the POM (C2), so the elaborated design can be navigated, serialized, printed and diffed.

## Goals

- [ ] One IR value holds an elaborated design in arenas addressed by typed ids, with navigable `Copy` handles, the way `piperine_lang::pom::Design` holds a source tree (C2).
- [ ] The IR round-trips through `serde` (C2).
- [ ] A one-way printer gives a deterministic text of an IR for debugging and golden tests; no parser of that text exists (C2).
- [ ] The IR is a side artifact: building one never changes the `Design` it came from (C3, the POM's one rule).
- [ ] The interpreter (`ir-interp`) and the transpiler (`ir-rust`) both run from the IR (C3).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Building an IR from a `Design` (const evaluation, generation, instance placement, static checks) | feature `msl-elab` |
| Executing an IR | feature `ir-interp` |
| Printing an IR as Rust and compiling it | feature `ir-rust` |
| The device ABI between a compiled or interpreted design and the solver | decided by the ABI duel (C8, O8); this spec depends on it and does not design it |
| A textual IR format with a parser | C2: the IR is an object model; the printer is one-way |
| Symbolic differentiation as an IR to IR pass | open (O1); owned by IR-Q2, not assumed |
| Optimization passes over the IR (CSE, constant folding beyond elaboration, dead-code removal) | not requested |
| Deleting the old `piperine-ir` and `piperine-codegen` crates | feature `solver-restructure` (C1) |
| Amending `docs/spec/appendix_c_implementation.md`, which names "AIR v2" | a spec edit; raised as IR-Q6 |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision (C-number of `.specs/features/codegen/context.md`), by `docs/spec/` or by POM code. Rows marked `n` are proposals nobody has confirmed; each has an entry under "Open questions (detail)".

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The IR is an object model, not a text format | arenas, typed ids, navigable handles, `serde`; one-way printer, no parser | C2, user: "Eu recomendo que a IR seja uma IR não textual e sim um object model." | y |
| The IR is a side artifact of the POM | built from an immutable `Design`; the `Design` is never written | C3; `crates/piperine-lang/src/pom/mod.rs` "The one rule" | y |
| Both backends consume the IR | interpreter and transpiler read the IR | C3: "POM -> IR -> Interpreter ou IR -> Transpiler" | y |
| Ids are plain `u32` newtypes written without macros | same as `pom/ids.rs` | C2 "like the POM"; `pom/ids.rs` header: "no macro: ids are data" | y |
| Which POM properties "like the POM" carries beyond arenas, ids, handles and serde (IR-Q5) | immutable after build, `Send + Sync`, deterministic ids, a `check()` that lists broken invariants | all four are properties of `Design` today; C2 names only arena, ids, handles, serde | n |
| The node set (IR-Q1, O2) | value-level nodes mirror the POM's `ExprKind`/`StmtKind`/`PatKind`; dedicated nodes for the lowered MSL forms | proposal of O2 | n |
| How derivatives are represented (IR-Q2, O1) | the IR holds no derivative nodes; backends compute Jacobians by forward-mode dual numbers | proposal of O1 | n |
| What the IR keeps of the hierarchy (IR-Q3, O3) | flat per-design IR plus a name map back to POM handles | proposal of O3 | n |
| Whether a backend may read the `Design` besides the IR (IR-Q3) | no: the IR is self-sufficient for execution; POM links are for diagnostics and tools only | a serialized IR must run without its sources; reading of C3, not stated by the user | n |
| Crate and module path (IR-Q4) | a new crate named `piperine-ir`, created after `solver-restructure` deletes the old one | the name is free once C1 lands; no decision recorded | n |
| Reference documentation under `docs/ir/` (IR-Q7) | pages mirroring `docs/pom/` (overview, model, navigation, printer), `rust` examples run as doctests | orchestrator brief: "like the POM in crates/piperine-lang/src/pom and docs/pom"; not in the user's words | n |
| Who implements and who verifies (C10) | Opus implements, Fable verifies | C10 covers "logic changes in the core"; whether a new IR crate counts as core is not stated | n |

**Open questions:** IR-Q1 to IR-Q7 are open and listed below. This spec is not ready for approval.

**Implicit-requirement dimensions.** Input validation and bounds: IR-06 (a deserialized IR is checked). Failure states: IR-06 AC 2, IR-03 AC 3. Idempotency: IR-01 AC 3 (deterministic ids), IR-04 AC 2 (deterministic text). Auth and rate limits: N/A because the IR is an in-process library value. Concurrency and ordering: pending IR-Q5 (`Send + Sync`). Data lifecycle: N/A because an IR lives as long as its owner; persistence is `ir-rust`'s cache. Observability: IR-04 (the printer). External-dependency failure: N/A because the IR depends on no external service. State-transition integrity: pending IR-Q5 (immutability).

**Open questions (detail)**

- **IR-Q1 (O2). The node set.** How analog contributions, the digital definition DAG, reactions, events, drives and known flags are represented, and how close value-level nodes stay to the POM. Options: (a) value-level nodes mirror the POM one to one, with dedicated nodes for the lowered forms; (b) a lower-level SSA-like value graph (the shape of the retired IR); (c) reuse POM expression ids directly and add only elaboration tables. Recommendation: (a). It keeps C4's "reprint most expressions" true for the transpiler and keeps the printer readable. (c) breaks IR-Q3's self-sufficiency after monomorphization. The requirements below fix what must be representable, never the node shapes.
- **IR-Q2 (O1). Derivatives.** Options: (a) no derivative nodes, backends use forward-mode duals; (b) symbolic differentiation as an IR to IR pass that adds derivative expressions. Recommendation: (a), the proposal of O1. Constraint from the language either way: `ddx` is defined symbolically over probe leaves (§14.2.3), so its result must match that definition whichever method computes it.
- **IR-Q3 (O3). Hierarchy and POM links.** Options: (a) flat per-design IR with a name map to POM handles; (b) IR keeps the instance tree, one body per monomorphized module, instances reference bodies; (c) both: bodies per module plus a flat instance table. Recommendation: (c) or (b) deserve a look before (a) is fixed, because O4 proposes one compiled crate per module and a flat-only IR cannot serve it. This spec requires only what §21 and §0.3.4 need: every item carries its hierarchical path and a source location.
- **IR-Q4. Crate and name.** Options: (a) new crate `piperine-ir`; (b) a module of `piperine-lang` beside `pom`; (c) another crate name to avoid confusion with the retired crate. Recommendation: (a), sequenced after `solver-restructure`.
- **IR-Q5. POM properties carried over.** Immutable after build, `Send + Sync`, deterministic ids, `check()`. Recommendation: all four; they are what makes golden tests and a shared cache key stable.
- **IR-Q6. Appendix C names "AIR v2".** `docs/spec/appendix_c_implementation.md` C.1 (informative) describes lowering to "AIR v2", and G.6 lists "AIR and Piperine gaps". Options: (a) amend Appendix C to describe this IR once the node set is fixed; (b) leave it until the chain runs. Recommendation: (a), as a task of `msl-elab`, since C.1.2 is a lowering table.
- **IR-Q7. `docs/ir/`.** Whether this feature ships reference docs with a completeness test, as `msl-pom` did. Recommendation: yes.

---

## Dependencies & Order

| Depends on | For | State |
| ---------- | --- | ----- |
| `msl-pom` | the `Design`, its ids and handles the IR links back to | done |
| `solver-restructure` | deletes the old `piperine-ir` and `piperine-codegen` (C1), freeing the crate name | planned on `feature/solver-completeness` |
| ABI duel (C8, O8) | none directly; the IR must hold whatever the ABI asks a device to expose (state, events, small-signal, noise, introspection) | open; a late duel result can add IR requirements |

Order proposed by O6 (not decided): `msl-ir`, `msl-elab` and `ir-interp` first, as the first end-to-end MSL to solver path; `ir-rust` after. Inside that group `msl-ir` comes first: `msl-elab` produces it and `ir-interp` consumes it. Its node set (IR-Q1) is best designed together with `msl-elab`'s lowering.

---

## User Stories

### P1: Storage in arenas with typed ids ⭐ MVP

**User Story**: As a backend author, I want the elaborated design in arenas addressed by typed ids so that I can hold, compare and serialize references to any item.

**Why P1**: C2. Everything else is built on it.

**Requirement**: IR-01

**Acceptance Criteria**:

1. The system SHALL store every IR node in an arena owned by one IR value and address it by a typed id that is a `u32` newtype, one id type per arena, each declared without a macro.
2. The system SHALL keep every node at its id for the life of the IR value: no node moves or is removed after the build.
3. WHEN the same `Design` is elaborated twice with the same param values THEN the system SHALL produce two IR values whose printed texts are byte-identical.

**Independent Test**: build an IR by hand through its builder in a unit test, read each node back by id.

---

### P1: Navigable handles ⭐ MVP

**User Story**: As a tool or backend author, I want `Copy` handles that borrow the IR so that I can walk it up and down by name, the way I walk a `Design`.

**Why P1**: C2 "navigable handles, like the POM".

**Requirement**: IR-02

**Acceptance Criteria**:

1. The system SHALL expose each kind of IR item through a `Copy` handle that pairs a borrow of the IR with an id.
2. WHEN a handle is asked for the items it contains THEN the system SHALL return handles of those items in a fixed, documented order.
3. WHEN a handle of a contained item is asked for its container THEN the system SHALL return the handle of the item that contains it.
4. WHEN an item is looked up by its hierarchical path of §21 THEN the system SHALL return its handle, or nothing when no item has that path.

**Independent Test**: on the IR of the `Front` model of §0.7, look up `Front.above_th` by path and navigate to the item that contains it.

---

### P1: Serialization ⭐ MVP

**User Story**: As a toolchain author, I want the IR to serialize so that it can be cached, shipped and inspected outside the process that built it.

**Why P1**: C2 names serde; `ir-rust`'s cache (C7) hashes a design.

**Requirement**: IR-03

**Acceptance Criteria**:

1. The system SHALL implement `serde::Serialize` and `serde::Deserialize` for the IR value and every storage type it holds.
2. WHEN an IR is serialized to JSON and deserialized THEN the system SHALL return an IR equal to the original and with the same printed text.
3. IF the input of a deserialization is not a serialized IR THEN the system SHALL return an error and no IR.

**Independent Test**: round-trip the IR of a gallery model through `serde_json` and compare printed texts.

---

### P1: One-way printer ⭐ MVP

**User Story**: As a developer debugging the chain, I want a readable text of any IR so that I can read it, diff it and pin it in golden tests.

**Why P1**: C2. The reason Cranelift was retired is that the lowered form could not be read (context.md "Why").

**Requirement**: IR-04

**Acceptance Criteria**:

1. WHEN an IR is printed THEN the system SHALL return a text that shows every node reachable from the IR's roots.
2. WHEN the same IR is printed twice THEN the system SHALL return byte-identical texts.
3. The system SHALL expose no function that builds an IR from the printed text.
4. WHEN two IR values differ in any node THEN the system SHALL print texts that differ.

**Independent Test**: a golden test pins the printed IR of the `Front` model of §0.7.

---

### P1: The IR is a side artifact ⭐ MVP

**User Story**: As a POM user, I want elaboration to leave my `Design` untouched so that the POM keeps mirroring the source.

**Why P1**: C3 and the project's POM navigability rule.

**Requirement**: IR-05

**Acceptance Criteria**:

1. The system SHALL build an IR through a shared borrow of the `Design` only.
2. WHEN an IR has been built from a `Design` THEN the system SHALL leave `Design::dump()` byte-identical to its value before the build.
3. The system SHALL record on every IR item that a diagnostic or the host can name (§0.3.4, §21) the hierarchical path of that item and a source location in the `Design`.

**Independent Test**: dump a `Design`, build its IR, dump again, compare.

---

### P1: Coverage of the elaborated language ⭐ MVP

**User Story**: As a backend author, I want every construct that survives elaboration to have an IR form so that no backend reads the POM to execute a design.

**Why P1**: C3 (the backends run from the IR) and C4 (the list of forms the transpiler lowers).

**Requirement**: IR-07

**Acceptance Criteria**:

1. The system SHALL represent value-level code: functions, structs, enums, `match`, `impl` methods, closures and every expression and pattern form of the POM's `ExprKind` and `PatKind` that elaboration does not remove.
2. The system SHALL represent, for every discrete value, the levels of its `Repr` and the known flag of each level (§2, §3.3).
3. The system SHALL represent every carrier, every drive of it, the nature that combines the drives, each definition and each commit with its optional delay (§4, §5, §8.2, §12.1).
4. The system SHALL represent every branch, every contribution to its potential or flow, and the selector that enables it (§6, §11).
5. The system SHALL represent every reaction with its event expression and guard, and every event form of §13.
6. The system SHALL represent every placement of a core primitive of the §14 table, with its value inputs.
7. The system SHALL represent every `emit!` site with its stream name, field names and field types (§19).
8. The system SHALL represent every param with its type, default, `where` condition and whether it is structural (§8.1, §21).

**Independent Test**: for each row of the §14 table, a hand-built IR holding that primitive prints a text that names it.

---

### P2: Invariant check

**User Story**: As a backend author, I want an IR to state whether it is well formed so that a corrupted or hand-built IR fails before execution.

**Why P2**: a POM property (`Design::check()`); carried over only if IR-Q5 confirms it.

**Requirement**: IR-06

**Acceptance Criteria**:

1. WHEN `check()` is called on an IR built by elaboration THEN the system SHALL return an empty list.
2. IF an id held by an IR node points outside its arena THEN `check()` SHALL return one entry naming the node and the id.

**Independent Test**: deserialize a JSON IR with one id edited out of range and call `check()`.

---

### P2: Reference documentation

**User Story**: As a contributor, I want the IR documented the way the POM is so that I can write a backend without reading the crate's source.

**Why P2**: orchestrator brief; pending IR-Q7.

**Requirement**: IR-08

**Acceptance Criteria**:

1. The system SHALL document every public type and method of the IR under `docs/ir/`.
2. IF a public type or method of the IR has no entry under `docs/ir/` THEN the documentation completeness test SHALL fail and name it.

**Independent Test**: run the completeness test after adding an undocumented public method.

---

## Edge Cases

- WHEN an IR holds no instance (a design with nothing but a top module with no body) THEN the system SHALL print, serialize and check it like any other IR.
- IF two items would have the same hierarchical path THEN `check()` SHALL report both.
- WHEN an item comes from a generation `for` THEN the system SHALL give it the indexed path of §21 rule 2.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| IR-01 | P1: Storage in arenas with typed ids | - | Pending |
| IR-02 | P1: Navigable handles | - | Pending |
| IR-03 | P1: Serialization | - | Pending |
| IR-04 | P1: One-way printer | - | Pending |
| IR-05 | P1: The IR is a side artifact | - | Pending |
| IR-06 | P2: Invariant check | - | Pending |
| IR-07 | P1: Coverage of the elaborated language | - | Pending |
| IR-08 | P2: Reference documentation | - | Pending |

**Coverage:** 8 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` - decisions C1 to C10 and open items O1 to O8.
- `docs/spec/` Parts I to VI, §14, §15, §21, Appendix C - what the elaborated design contains.
- `crates/piperine-lang/src/pom/` and `docs/pom/` - the object-model style the IR follows.

---

## Success Criteria

- [ ] The IR of every model of the conformance set (Appendix C.2.3) prints, round-trips through serde and passes `check()`.
- [ ] `ir-interp` and `ir-rust` compile against the IR crate without depending on `piperine_lang::pom` for execution.
- [ ] A golden test of a printed IR fails on any change of a node.

# MSL IR (object model, M1 node set) Specification

**Approved**: 2026-11-15 - user: "Concordo e pode disparar"; M1 questions: all recommended options accepted

**Status**: Approved. Rewritten for M1 of the end-to-end slice (`.specs/features/codegen/context.md` S1, S4, S6).

## Problem Statement

The POM mirrors the source and stops before elaboration. The solver needs the elaborated design in a form a backend can execute: the interpreter now, the Rust transpiler in M3 (C3). The previous form, Cranelift over the old IR, is retired because it could not be read or verified (C1). This feature defines the IR as an object model in the style of the POM (C2), with the node set the M1 analog subset needs and nothing more.

## Goals

- [ ] One `Ir` value holds an elaborated design: one body per module variant, the functions they call, and a flat table of instances and nodes (S6).
- [ ] The node set covers the M1 analog subset: every construct of `tests/ngspice/*.msl` has a node.
- [ ] The IR round-trips through `serde`, prints a deterministic text, and states whether it is well formed.
- [ ] The crate depends on no in-tree crate: a backend that reads an IR does not link the front end.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Building an IR from a `Design` | feature `msl-elab` |
| Executing an IR | feature `ir-interp` |
| Derivative nodes or a differentiation pass | S4: Jacobians come from dual numbers in the backends |
| Discrete carriers, drives, natures with `resolve`, definitions, commits, reactions, events, known flags, `emit!` | M2 (S2) |
| Core primitives beyond the subset: `ddx`, `delay`, `noise`, `cross`, `at`, `bound_step`, flow probes, `simparam`, `analysis`, `now`, `freq` | M2; no M1 model uses them |
| Contributions under a selector, switch branches, node merging | M2; no M1 model uses them |
| Structs, enums, `match`, loops, integers, arrays, generics | M2; outside the type-checked subset |
| Operating-point variables, `///` text and the rest of the §21 metadata beyond paths, params and origins | M2, with the host |
| A textual format with a parser | C2: the printer is one-way |
| Reference documentation under `docs/ir/` | deferred until the node set covers the digital side (IR-Q7 closed); rustdoc carries M1 |
| Amending Appendix C, which names "AIR v2" | a spec edit; Q8, decided: after M1 runs, with `spec-abi-amendments` |
| Optimization passes | not requested |

---

## Assumptions & Open Questions

Rows marked `y` are settled by a recorded decision, by `docs/spec/` or by code. No row is left open: the user answered `.specs/features/e2e-m1/questions.md` on 2026-11-15.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The IR is an object model: arenas, typed ids, handles, `serde`, a one-way printer | as stated | C2, user: "Eu recomendo que a IR seja uma IR não textual e sim um object model." | y |
| Hierarchy | one body per monomorphized module and a flat instance table | S6, user: "Concordo e pode disparar" | y |
| Derivatives | none in the IR | S4, same answer | y |
| Functions | kept as functions, called by id | S7, same answer | y |
| The node set (IR-Q1) | value nodes mirror the POM's forms of the subset; dedicated nodes for probe, environment read, `limit` site, `ac_stim` site, and a contribution split into addends | closed in `design.md`: it is the smallest set the five models lower to, and it keeps C4's "reprint most expressions" true for M3 | y |
| POM properties carried over (IR-Q5) | immutable after build, `Send + Sync`, deterministic ids, `check()` | all four hold for `Design` today; golden tests and a cache key need them | y |
| A backend reads the IR only | the IR is self-sufficient for execution; it holds origins as text, not POM ids | a serialized IR must run without its sources; it also keeps the crate free of the front end | y |
| Ids | `u32` newtypes written without a macro | `pom/ids.rs`: "no macro: ids are data"; AGENTS rule 5 | y |
| Crate name (Q6) | `piperine-ir`, a new crate; the guard that lists it as retired is edited in the task that creates it | `crates/piperine-solver/tests/workspace_shape.rs:22` fails on a crate of that name today | y - Q4 to Q8, user 2026-11-15: "Accept all (Recommended)" |
| Static dependency analysis lives in this crate | `Body::dependencies` is a query over the IR, not stored data | both backends need the same sets for reads and partials; a stored copy could disagree with the code it describes | y |

**Open questions:** none - all resolved or logged above. The eight questions of `.specs/features/e2e-m1/questions.md` were answered by the user on 2026-11-15, every one with the recommended option.

**Implicit-requirement dimensions.** Input validation and bounds: IR-06 (a hand-built or deserialized IR is checked). Failure states: IR-03 AC 3, IR-06. Idempotency: IR-01 AC 3, IR-04 AC 2. Auth and rate limits: N/A because the IR is an in-process value. Concurrency and ordering: IR-01 AC 4 (`Send + Sync`), IR-02 AC 2 (fixed orders). Data lifecycle: N/A because an IR lives as long as its owner. Observability: IR-04 (the printer). External-dependency failure: N/A because the crate reads nothing external. State-transition integrity: IR-01 AC 2 (no mutation after `finish`).

---

## User Stories

### P1: Storage in arenas with typed ids ⭐ MVP

**User Story**: As a backend author, I want the elaborated design in arenas addressed by typed ids so that I can hold, compare and serialize a reference to any item.

**Why P1**: C2; everything else is built on it.

**Requirement**: IR-01

**Acceptance Criteria**:

1. The system SHALL store every node in an arena owned by one `Ir` value and address it by a typed id that is a `u32` newtype, one id type per arena, each declared without a macro.
2. WHEN `IrBuilder::finish` has returned an `Ir` THEN the system SHALL offer no operation that adds, moves or removes a node of it.
3. WHEN the same sequence of builder calls is made twice THEN the system SHALL produce two `Ir` values that are equal and print byte-identical texts.
4. The system SHALL make `Ir` `Send + Sync`.

**Independent Test**: build the resistor body by hand through `IrBuilder`, read each node back by id.

---

### P1: Navigable handles ⭐ MVP

**User Story**: As a tool or backend author, I want `Copy` handles that borrow the IR so that I walk it the way I walk a `Design`.

**Why P1**: C2 "navigable handles, like the POM".

**Requirement**: IR-02

**Acceptance Criteria**:

1. The system SHALL expose bodies, functions, instances and nodes through `Copy` handles that pair a borrow of the `Ir` with an id.
2. WHEN a handle is asked for the items it contains THEN the system SHALL return them in declaration order: a body's params, nodes, branches, sites and children; an instance's children.
3. WHEN an instance handle is asked for its parent or its body THEN the system SHALL return that handle.
4. WHEN an instance or a node is looked up by its path THEN the system SHALL return its handle, or nothing when no item has that path.

**Independent Test**: on a hand-built divider, look up `Divider.r1`, navigate to its body `Resistor`, and from the node `Divider.mid` list the two instances bound to it.

---

### P1: The M1 analog node set ⭐ MVP

**User Story**: As the elaborator, I want a node for every construct of the M1 subset so that the five models lower with nothing left in the POM.

**Why P1**: S1; C3 (backends run from the IR).

**Requirement**: IR-07

**Acceptance Criteria**:

1. The system SHALL represent value code of the subset: `f64` and `bool` constants, locals, param reads, unary and binary operators, `if` with `else`, blocks with `let`, calls of functions by id, calls of the catalogue methods `exp`, `ln`, `sqrt` and `abs`, and a closure with its parameters and body.
2. The system SHALL represent a body with its params (name, type, default, `where` expression, structural flag), its nodes (ports first, then signals, each with its nature and optional initial potential), its branches, its `limit` sites, its `ac_stim` sites and its `analog` block.
3. The system SHALL represent a potential probe between two terminals, each a node of the body or ground.
4. The system SHALL represent the environment reads `temperature()`, `gmin()` and `source_scale()`.
5. The system SHALL represent a `limit` site with its input, its function and its optional seed, and an `ac_stim` site with its magnitude and optional phase, each with an index unique in its body.
6. The system SHALL represent a contribution as a branch, the quantity contributed (flow or potential) and a list of addends, each a value expression that is either direct or under `ddt`, with its sign.
7. The system SHALL represent a nature with its name and, for potential and flow, the `abstol` and `units` of the quantity.
8. The system SHALL record the type (`f64`, `bool`, `()`, function) of every expression.

**Independent Test**: build each of the five device bodies of `devices.msl` by hand and print it; every construct of the source appears in the text.

---

### P1: Bodies and the flat instance table ⭐ MVP

**User Story**: As a backend author, I want each module variant once and every placed instance in one table so that I compile per body and bind per instance.

**Why P1**: S6.

**Requirement**: IR-08

**Acceptance Criteria**:

1. The system SHALL store one body per module variant and reference it from every instance that places it.
2. The system SHALL store every instance of the elaborated design in one table in pre-order of the hierarchy, each with its path, its body, its parent, its children, one design node or ground per node of its body, and one bound value per param of its body.
3. The system SHALL store every node of the elaborated design in one table, each with its path, its nature, the instance that declares it and its initial potential when it has one.
4. The system SHALL keep the authored tree walkable: from the top instance through children to every leaf.

**Independent Test**: on the hand-built divider, the body `Resistor` is stored once and referenced by `Divider.r1` and `Divider.r2`.

---

### P1: Serialization ⭐ MVP

**User Story**: As a toolchain author, I want the IR to serialize so that it can be cached and inspected outside the process that built it.

**Why P1**: C2 names serde; M3's cache hashes it (C7).

**Requirement**: IR-03

**Acceptance Criteria**:

1. The system SHALL implement `serde::Serialize` and `serde::Deserialize` for `Ir` and every storage type it holds.
2. WHEN an `Ir` is serialized to JSON and deserialized THEN the system SHALL return an `Ir` equal to the original.
3. IF the input of a deserialization is not a serialized `Ir` THEN the system SHALL return an error and no `Ir`.

**Independent Test**: round-trip the hand-built diode body through `serde_json` and compare.

---

### P1: One-way printer ⭐ MVP

**User Story**: As a developer debugging the chain, I want a readable text of any IR so that I can read it, diff it and pin it in golden tests.

**Why P1**: C2; unreadable lowered code is why Cranelift was retired.

**Requirement**: IR-04

**Acceptance Criteria**:

1. WHEN an `Ir` is printed THEN the system SHALL return a text that shows every body, function, instance and node, and every expression and statement reachable from them.
2. WHEN the same `Ir` is printed twice THEN the system SHALL return byte-identical texts.
3. The system SHALL expose no function that builds an `Ir` from text.
4. WHEN two `Ir` values are not equal THEN the system SHALL print texts that differ.

**Independent Test**: a golden test pins the printed IR of the hand-built diode.

---

### P1: Invariant check ⭐ MVP

**User Story**: As a backend author, I want an IR to state whether it is well formed so that a corrupted or hand-built IR fails before execution.

**Why P1**: `ir-interp` refuses an IR that fails it; in M1 the interpreter's own tests run on hand-built IRs.

**Requirement**: IR-06

**Acceptance Criteria**:

1. WHEN `Ir::check` is called on an IR built by a correct builder sequence THEN the system SHALL return an empty list.
2. IF an id held by a node is out of range of its arena THEN the system SHALL return an entry naming the node and the id.
3. IF an instance binds a number of nodes or params different from its body's THEN the system SHALL return an entry naming the instance's path.
4. IF a `limit` or `ac_stim` site index is used by two expressions, or a site or a contribution appears outside the `analog` block of a body or under an `if`, THEN the system SHALL return an entry naming the body and the origin.
5. IF a contribution's addend, a probe operand of an operator, or a `limit` input does not have the type its position requires THEN the system SHALL return an entry naming the expression's origin.

**Independent Test**: deserialize a JSON IR edited to hold an out-of-range `FunctionId`; `check()` names it.

---

### P1: Static dependencies ⭐ MVP

**User Story**: As a backend author, I want to ask what a contribution, a `limit` input or a stimulus reads so that every backend declares the same reads and the same partials.

**Why P1**: the ABI wants declared reads and an exact partial pattern (abi.md F5, §8: "the pattern is the IR's static dependency set").

**Requirement**: IR-09

**Acceptance Criteria**:

1. WHEN a body is asked for the dependencies of a branch's addends, of a `limit` site or of an `ac_stim` site THEN the system SHALL return the set of body nodes probed, params read, environment functions read, and `limit` and `ac_stim` sites read, through locals and function calls.
2. The system SHALL return each set in a fixed order: nodes by index, then params by index, then environment reads, then sites by index.
3. WHEN a body is asked whether a branch's direct addends are affine in the node potentials THEN the system SHALL answer yes exactly when every probe reaches the sum only through `+`, `-`, unary `-`, and `*` or `/` by an expression that reads no probe and no `limit` site.

**Independent Test**: on the hand-built resistor, the flow of `(p, n)` depends on nodes `p`, `n` and param `r`, and is affine; on the diode, the flow depends on the `limit` site and is not affine.

---

## Edge Cases

- WHEN an `Ir` holds no instance but its top THEN the system SHALL print and check it without error.
- WHEN a body has no `analog` block THEN the system SHALL store it with no branches and no sites.
- IF two instances or two nodes have the same path THEN `Ir::check` SHALL return an entry naming the path.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| IR-01 | P1: Storage in arenas with typed ids | Design | Pending |
| IR-02 | P1: Navigable handles | Design | Pending |
| IR-03 | P1: Serialization | Design | Pending |
| IR-04 | P1: One-way printer | Design | Pending |
| IR-06 | P1: Invariant check | Design | Pending |
| IR-07 | P1: The M1 analog node set | Design | Pending |
| IR-08 | P1: Bodies and the flat instance table | Design | Pending |
| IR-09 | P1: Static dependencies | Design | Pending |

IR-05 of the first draft (the IR is a side artifact of the POM) is now `msl-elab` ELAB-01 AC 2: this crate no longer sees a `Design`.

**Coverage:** 8 total, 0 mapped to tasks.

---

## Sources

- `.specs/features/codegen/context.md` C1 to C7, S1, S4, S6, S7 - the decisions.
- `tests/ngspice/*.msl` - the constructs the node set must hold.
- `.specs/features/device-abi/abi.md` §4, §8 - what a backend must declare, hence IR-09.
- `crates/piperine-lang/src/pom/` (`ids.rs`, `body.rs`, `design.rs`) - the style carried over.

---

## Success Criteria

- [ ] The five device bodies and the five twins of M1 are representable, and `msl-elab` produces them.
- [ ] `cargo tree -p piperine-ir` lists no in-tree crate.
- [ ] A golden test pins one printed IR per device.

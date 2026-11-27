# Device ABI Simplification Specification

**Approved**:

## Open questions (for the user)

Eight decisions are the user's. Each has a recommendation; the documents below are written on the recommended side, and each question names what changes on the other side. A ninth item is listed because it must stay open.

| # | Question | Recommendation | The other side |
| --- | --- | --- | --- |
| Q1 | **Where the ABI specification lives and how it is published.** | Its own document set, `docs/abi/` ("The Piperine Device ABI"), built by `docs/abi/build.py` into one page and published as its own artifact, beside the language specification and the Book. The three link to each other. | A Part IX of `docs/spec/`: one site and one build, but `docs/spec/` is "the only grammar and semantics source" of MSL, has its own version (`spec_version`), and is read by model authors. The ABI is read by device implementers in any language and is versioned by its own symbol. |
| Q2 | **One Book chapter or two.** | Two chapters in Part 2: "The device ABI" (concepts, data types, how the solver uses them; the present chapter 18 rewritten) and "Writing a device" (by hand, with the attributes, worked examples, the compile errors). | One chapter. It would be the longest of the Book, and a reader who wants to write a device would have to read the reference part first. |
| Q3 | **How an attribute function declares what it writes.** Your example returns a value (`-> Abc`). | Writes and gradients are `&mut` parameters; the function returns `Result<(), Fault>`. Every parameter is then one entry of `reads`, `writes` or `partials`, in order, and nothing maps a struct to a list behind the author. | The function returns a struct and a second attribute on that struct maps its fields to `writes`. Closer to your example, one more macro and one more type per function. |
| Q4 | **How far the attributes go.** | They generate methods on the author's own type and nothing else. The author still writes the shape function (three lines) and `impl DeviceImplementation` (four short methods) by hand, so the builder and the trait stay visible. | One top attribute that also generates the trait implementation, the shared shape and the device type. Shorter, and the trait is no longer in the file. |
| Q5 | **Where the attributes may be used inside this repository.** | Only in the attribute crate's own tests and in the Book's listings. Generated code, the solvers and the device fixtures stay written by hand, so rule 7 ("no macros") keeps holding for the rest of the tree. | The device fixtures move to the attributes too. Less test code, and every solver test then depends on a proc-macro. |
| Q6 | **The new name of `piperine_solver::abi`.** The proposal renames it to `piperine_solver::circuit`. `code-clarity` (T137) gives that name to today's `core` module. | `piperine_solver::device_authoring`: the module re-exports what a device author needs. | Another name of your choice. The proposal's name cannot be used. |
| Q7 | **`code-clarity` renames the identifiers of `piperine-abi` (T132) and this feature renames most of them again.** | Leave `code-clarity` as approved. The second rename is this feature's and costs nothing extra: every file it touches is rewritten here anyway. | Amend `code-clarity` so T132 skips the names that this feature deletes (`Cell`, `Statement`, `Input`, `Watch` and their fields). Saves part of one mechanical task there and changes an approved plan. |
| Q8 | **The AVR emulator of the present Book chapter cannot run:** it is a sketch of a device that does not exist, and every listing of the Book must run. | Replace it by a small foreign device that exists and is tested: a programmable counter with its own clock, which shows the same four ideas (state in a private pin, `HORIZON`, "wake me at" with `REPLACES_PENDING`, a second reaction that reads a continuous pin). | Keep the AVR text as prose with no listing. Building a real AVR emulator is not in this feature. |
| S21 | **Not decided here, and it stays open:** `code-clarity` S21, removing the ground aliases `GND`, `vss`, `VSS`. | This feature takes no side. The ABI's `GROUND` is a constant, not a name, and nothing here reads or changes `GroundName`. | — |

## Problem Statement

The device ABI is the binary contract between the solver and the code of a device. The user's complaint about it: "concepts were not separated, names did not match what things do, and it was unclear where each thing goes". A design review produced proposal v6 (`input/proposal-v6.md`), which keeps the idea of the present ABI (a device is immutable data plus pure code, and has no memory) and rearranges it into nine kinds of thing and four closed catalogues. The user accepted it on 2026-11-27 and asked for three more things in the same feature: the ABI published as a specification, a didactic Book chapter, and authoring macros for device implementers.

Today the specification of record is a feature note (`.specs/features/device-abi/abi.md`) that diverges from the code in fourteen places, the Book chapter shows listings that are not compiled, and writing a device by hand means keeping a list of reads, a list of writes and the positions in a frame in step by hand.

## Dependency

**This feature executes after `code-clarity` merges to `main`.** This branch is cut from the `code-clarity` integration branch at `97d327d58` (phases 1 to 4 merged, 151 tasks remaining). The later phases of `code-clarity` rename crates (`piperine-lang` → `piperine-language`, `piperine-interp` → `piperine-interpreter`), split modules (`raw.rs` → `raw/`, `solver/src/core` → `circuit`), move every hand-written test device into `piperine-device-fixtures`, merge test targets and put the guards crate in charge of owners, names and documentation. Every path, crate name and test target in these documents names the tree as `code-clarity`'s design leaves it. The first task (T1) re-takes every count and path on the merged `main` and corrects these documents before any code is written.

## Goals

- [ ] The ABI is the contract of proposal v6, Part II: eleven tables, 32 structs, 66 constants, nine validation rules, five device entries and two solver entries.
- [ ] Nothing a run computes changes: the 98 capabilities (C1 to C98) are carried, and every numeric oracle passes with unchanged expected values.
- [ ] The six refusals of the proposal are in force, each with a test.
- [ ] The ABI specification is published beside the language specification and the Book, and is the specification of record: a difference between it and the code fails a test.
- [ ] The Book explains the ABI to a newcomer: its data types, how the solver uses them, how to write a device, with worked examples that all run.
- [ ] A device implementer can write a function or a reaction with typed parameters and attributes; what the attributes generate is plain Rust over the public builder and traits, and can be written by hand.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| `force` and `release` (S1 of the proposal) | Decision 7, recommended side: struck from the ABI document and tracked as an open item of the host surface (a `GAPS.md` row with an owner). The device ABI is not involved either way. |
| Protocol divergences 1, 3, 6, 8, 11, 12 of proposal §23 | The proposal marks them out of scope: they are behaviour of the owner protocol or stale comments, not the device contract. |
| A loader for arbitrary third-party libraries, a text leaf, gradients across events | Listed by the proposal as unchanged limits (§6). |
| Moving the language specification's builder (`/var/tmp/piperine-artifacts/specsite/build.py`) into the repository | It lives outside the repository in a directory a reboot can clear. Worth doing; it is a chore on the language specification, not part of the ABI. Reported to the user. |
| Attributes for shapes whose number of entries depends on a parameter | Such a shape is written with the builder. The attributes cover a fixed list of entries; `declare` is an ordinary method and mixes with builder calls. |
| A C header or bindings for another language | The specification is language-neutral and gives every offset; no binding is generated here. |
| Removing the ground aliases (S21) | Open with the user in `code-clarity`. |
| A version bump, a second exported symbol, a compatibility layer for the contract before this one | The user, 2026-11-27: "nao avanca a versão do ABI, ainda nao fizemos release". The contract changes in place |
| New device models, new analyses, performance work | The feature restructures a contract. |

---

## Decisions recorded

**The user accepted the proposal**: 2026-11-27 - user: "Fio, aceito a proposta do piperine device abi." Every decision of the proposal's Part IV therefore takes its recommended side.

| # | Decision (proposal, Part IV) | Side taken |
| --- | --- | --- |
| 1 | *continuous/discrete* and *analog/digital* | The language's rule: *continuous* and *discrete* say what is held, *analog* and *digital* say code and the solver that runs it. The crates keep their names. |
| 2 | The names | *pin* kept; *linkage*; *small-signal source*; *where condition* in place of the ABI's `check`. User-visible error text changes for the last. |
| 3 | Where to stop merging | Eleven tables, every row of the decision's table taken. |
| 4 | The kind of a function | Derived: digital when it writes pins, analog when it writes analog tables; one definition, implemented once in `piperine-abi`. |
| 5 | Six refusals | All six accepted. |
| 6 | The alignment promise | Withdrawn. Read and write pointers carry no alignment and a device reads by copy. This is an approved exception to "code follows the specification": the specification now says what the code does. |
| 7 | `force` and `release` | Struck from the ABI document; tracked as an open item of the host surface. The language specification keeps the term until that feature is scheduled; the user accepted this knowingly with the proposal. |
| 8 | Migration | Staged, steps A to E, renames done once in the same steps. `abi.md` is rewritten from Part II as the specification of record. |

**The rest of the request**: 2026-11-27 - user: "Recomendo que o ABI vire uma spec publicada, junto com a spec da linguagem e com o book. [...] o abi precisa ser melhor descrito no book, com seus tipos de dados, funcionamento, etc... for dummies mesmo. Outra coisa que vou pedir para colocar NA MESMA FEATURE é macro facilitadoras de implementação de ABIs como por exemplo `#[function(...)] fn functionA ( #[pin_read(...)] pinA : Pin, ... ) -> Abc { ... }` que melhore a vida do desenvolvedor como um todo mas que não pareça mágica aos olhos de um desenvolvedor (usando o máximo possível de funções nativas, declarações nativas mas mantendo uma boa ergonomia na escrita)."

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Every Part IV decision takes its recommended side | The table above | The user's acceptance of the proposal | y - user: "aceito a proposta do piperine device abi" |
| The feature runs after `code-clarity` merges | T1 re-baselines on the merged `main` | The coordinator's brief; `code-clarity` renames crates and modules this feature edits | n |
| "For dummies" | The chapters are written for a reader who has never seen a simulator's device interface, with no condescending word and no "for dummies" framing | The user asked for the depth ("for dummies mesmo"); the project's standing rule asks for a respectful tone | n - standing rule of the project, to be confirmed with the chapters |
| The Book is written in English, like the rest of it | English | `docs/book/content.html` is in English; the Portuguese ABI manual is a tone reference only | n |
| Section numbers of the published specification | Chapters 1 to 13 (the proposal's sections 8 to 20, renumbered), cited in code as `ABI §n.m`; the rule names V1 to V9 are kept | A published document starts at 1; design.md has the mapping | n |
| The Rust helper names of proposal §20.1 that are free functions (`device_type::<D>`, `device_type_with`, `library`, `plain_data_reference`, …) | Associated functions: `DeviceType::implemented_by::<D>(name)`, `DeviceType::made_by(name, &factory)`, `Library::of(name, build, device_types)`, methods of `PlainData` | Code rule 3 (no free functions) is binding after `code-clarity`; §20.1 is informative | n |
| The macro crate | A new crate `piperine-abi-macros`. `piperine-plugin-macros` does not exist in this tree and its name is on the dead-architecture list of `suite_hygiene` | Checked in the tree | n |
| `piperine-abi` does not depend on the macro crate and does not re-export it | An author depends on both crates | `piperine-abi` is a leaf with no dependency, its sources are embedded in the transpiler and built with `rustc` alone; a proc-macro dependency would break that | n |
| Project code rule 7 ("no macros") | Amended by a decision record: one crate may define macros, for device authoring only (Q5 fixes where they may be used) | The user's explicit request, scoped | y - user: "macro facilitadoras de implementação de ABIs [...] que não pareça mágica" |
| What may change that a user can see | The error text that says "check" for a `where` condition; `E0509`, which names the function in place of the watch; the transpiler's golden files, regenerated once; the Book's chapter 18 and its inventory rows. Nothing numeric | Proposal §6 and §7.3 | n |
| Publishing the artifact and pushing are remote actions | The closing tasks ask for the user's go-ahead before either | The skill's blast-radius rule | n |
| Version | The ABI stays at the symbol `piperine_abi_1`, `MAJOR = 1`, `MINOR = 0`, and changes in place. No task bumps a version, adds a second symbol or keeps a compatibility layer. The feature is named `device-abi-simplification` so that its name suggests no version | Proposal §18.1. The cache key over the ABI's sources already makes a library compiled before the change miss the cache | y - user, 2026-11-27: "nao avanca a versão do ABI, ainda nao fizemos release" |

**Open questions:** Q1 to Q8 at the top of this document, open until the user answers. S21 stays open and is not this feature's.

### Dimensions sweep

| Dimension | Where it lands |
| --- | --- |
| Input validation and bounds | ABI-03 (the nine rules over a shape), ABI-06 (a created shape against the default shape), ABI-25 (what an attribute may hold) |
| Failure and partial failure | ABI-05 (statuses per entry, `RESHAPE` outside its two conditions), ABI-10 (a numeric oracle that moves stops the migration) |
| Idempotency, retry, duplicates | ABI-12 AC 3 and AC 5 (a second writer of a strength, two parameters of one name). A function is pure, so a repeated call is defined by the contract (proposal §15.4); no new requirement |
| Auth boundaries and rate limits | N/A because the ABI is an in-process contract with no caller identity |
| Concurrency and ordering | ABI-24 AC 1 (an expansion declares no `static`, so a library keeps no mutable global state, §12 of the specification). The threading rules themselves are unchanged and carried by C81 (ABI-09) |
| Data lifecycle and expiry | ABI-07 AC 2 (a library compiled before the flip has another cache key and is never opened) |
| Observability | ABI-08 (the words of a refused host write), ABI-05 AC 4 (`E0509`), ABI-26 (the origin of an entry declared by attribute) |
| External-dependency failure | ABI-16 AC 3: publication needs the artifact service; when it is not reachable the specification is still complete in the repository and the task reports. `python3` is already required by the Book's build test |
| State-transition integrity | ABI-11 (no scaffold of the migration survives), the process constraints below (the order A to E) |

---

## User Stories

### P1: The contract is the one the user approved ⭐ MVP

**User Story**: As a reader of a shape, I want every datum in exactly one table whose name says what the solver does with it, so I can tell where each thing goes.

**Why P1**: It is the proposal the user accepted.

**Acceptance Criteria** (ABI-01 to ABI-08):

1. The crate `piperine-abi` SHALL define each `#[repr(C)]` struct of the specification with the size and the field offsets the specification's listing gives for a 64-bit target, checked by a test that reads the listings of `docs/abi/` (ABI-01)
2. The crate `piperine-abi` SHALL define every constant of every group with the value the specification's tables give, checked by a test that reads those tables (ABI-01)
3. WHEN a device adds an entry to a shape THEN the builder SHALL return an index typed for that entry's table, and a call that passes an index of another table SHALL not compile (ABI-02)
4. WHEN a device reads or writes a frame buffer through a pointer that is not aligned for the value's type THEN the lens SHALL return or store the value by copy, and the lens SHALL offer no accessor that borrows a typed reference into a buffer (ABI-02)
5. IF a shape breaks a case of rule V1, V2, V3, V4, V5, V6, V7 or V9 THEN validation SHALL refuse it with the rule, the entry's table and name, and its origin (ABI-03)
6. IF a read, a write or a commit target is outside the access table of specification §7.3, or breaks one of its three conditions, or a function writes both a pin and an analog table, THEN validation SHALL refuse the shape with `AccessNotAllowed`; the test SHALL take the allowed cells from the specification's table (ABI-03)
7. IF a shape breaks two rules THEN validation SHALL report the one that comes first in the order V1 to V9 (ABI-03)
8. The crate `piperine-abi` SHALL decide the kind of a function by one definition: analog when it writes an analog table or `STEP_BOUND`, digital when it writes a pin, neither when it writes nothing (ABI-04)
9. WHEN the only writes of a function are traces THEN no solve SHALL call it (ABI-04)
10. The analog solver and the digital solver SHALL take the kind of a function from `piperine-abi` and SHALL hold no predicate of their own over a function's writes (ABI-04)
11. WHEN a reaction commits to a single value, to one element of an array or to an event pin THEN it SHALL do so through the one `commit` entry, with `element` 0 where there is no array (ABI-05)
12. IF a digital function returns `RESHAPE` outside the parameter cone or in a call that is not on the shadow store THEN the solver SHALL fail with a fault that names the function (ABI-05)
13. IF an entry returns a status that the entry or the context does not allow THEN the solver SHALL fail with a fault that names the instance, the entry and the status (ABI-05)
14. IF a function writes a `STEP_BOUND` that is neither finite and positive nor positive infinity THEN the solver SHALL fail with `E0509`, naming the function and its origin (ABI-05)
15. WHEN `create` returns `OK` THEN the caller SHALL validate the returned shape and check its parameters against the default shape (equal names, layouts and element counts, and no other parameter) before any other call on the instance, and SHALL refuse a shape that differs with the instance path, the parameter and what differs (ABI-06)
16. IF a library's `size` is under the `Library` the loader reads, or its `minor` is newer than the loader's, THEN the loader SHALL refuse the library (ABI-07)
17. WHEN the raw layout changes THEN a library compiled before the change SHALL have another cache key and SHALL not be opened (ABI-07)
18. The crate `piperine-abi` SHALL export the symbol `piperine_abi_1` and no other ABI symbol, with `MAJOR` 1 and `MINOR` 0 (ABI-07)
19. IF a host write leaves a where condition false THEN the refusal SHALL name the instance and the pin of the condition and SHALL call it a "where condition" (ABI-08)

**Independent Test**: build a resistor and a flip-flop with the new builder, validate, bind and run an operating point and a transient; the layout test passes against `docs/abi/`.

---

### P1: Nothing a run computes changes ⭐ MVP

**User Story**: As the user, I want the restructuring to leave every result where it was, so a difference is a defect of the migration and can be bisected.

**Why P1**: The proposal's acceptance contract (§7.3): the capabilities C1 to C98 and the numeric oracles.

**Acceptance Criteria** (ABI-09 to ABI-11):

1. The feature SHALL keep a ledger with one row per capability C1 to C98, each naming at least one test that proves it, and a guard SHALL fail on a row whose test does not exist (ABI-09)
2. The ledger SHALL hold a row for S1 (`force` and `release`) that names the `GAPS.md` row tracking it, and the guard SHALL fail when that row is missing from `GAPS.md` (ABI-09)
3. WHEN the feature closes THEN `crates/piperine-parity/src/contract.rs`, every twin golden and every frozen fixture SHALL be byte-identical to the base tag `device-abi-simplification-base` (ABI-10)
4. WHEN the feature closes THEN every parity target SHALL pass against ngspice under the unchanged `ToleranceContract` (ABI-10)
5. WHEN the transpiler's golden files are regenerated THEN the default shape of every compiled device type SHALL equal, entry by entry, the shape its plan computed (ABI-10)
6. WHEN the feature closes THEN no identifier of the migration's scaffold (the lowering of step A, the pin-number translation of step C) and no identifier of the previous contract (`Cell`, `Statement`, `InputSpec`, `WatchSpec`, `ShapeBuf`, `pin_flag`, `role`, `ambient`) SHALL remain in the tree (ABI-11)

**Independent Test**: the final suite: `cargo test --workspace`, parity with ngspice, the twins, the buck targets, with no expected value edited.

---

### P1: The six refusals ⭐ MVP

**User Story**: As a producer of a shape, I want what the contract cannot honour to be refused by name, so nothing is accepted and then ignored.

**Why P1**: Decision 5 of the proposal; each refusal gets a test (§7.3).

**Acceptance Criteria** (ABI-12):

1. IF an analog function reads a discrete pin that is an array THEN validation SHALL refuse the shape with `AccessNotAllowed`
2. IF a dependency loop among digital functions passes through an event net THEN bind SHALL refuse the circuit, and `EventPin` SHALL have no flag and no initial value
3. IF two functions of one shape write the strength of one small-signal source THEN bind SHALL refuse the instance, naming the source and both functions
4. IF a discrete pin is linked to `START` or `SETTLE`, or an event pin to any session net that holds a value, THEN validation SHALL refuse the shape with `LinkageNotAllowed`
5. IF two parameters of one shape have the same name THEN validation SHALL refuse the shape with `LinkageRepeated`
6. IF a read whose aspect is not of a discrete pin has a nonzero `element` THEN validation SHALL refuse the shape with `NotApplicable`

**Independent Test**: six malformed shapes or circuits, each refused with its rule.

---

### P2: The seam between the owner and the sub-solvers says what it passes

**User Story**: As a reader of the solver, I want the seam between the owner and the two sub-solvers to use the same word pairs as the ABI and typed identifiers, so a net is not mistaken for a slot.

**Why P2**: Step E of the migration. It is independent of the contract and nothing else waits for it.

**Acceptance Criteria** (ABI-13):

1. WHEN the feature closes THEN the seam SHALL use the names of proposal §7.2 step E (`DigitalSide`, `DigitalSolverAdapter`, `DeliveredChange`, `SolveEnvironment`, `InstantEnvironment`, `BoundNet`, the module name Q6 fixes), and a guard SHALL fail on any of the retired names
2. The traits `AnalogSide` and `DigitalSide` SHALL take a net, an analog-copy slot, an event net and a sample as `NetId`, `AnalogCopySlot`, `EventNetId` and `SampleIndex`, and a guard SHALL fail on a method of either trait that takes one as a bare integer

**Independent Test**: the guard over the seam's signatures; the protocol tests green and unedited apart from the names.

---

### P1: The ABI is a published specification ⭐ MVP

**User Story**: As a device implementer in any language, I want one published document that is the contract, so I do not read a feature note or the solver's source.

**Why P1**: The user's request; and the project's rule that code follows the specification needs a specification of record.

**Acceptance Criteria** (ABI-14 to ABI-16):

1. The directory `docs/abi/` SHALL hold the specification: chapters 1 to 13 (normative) and annexes A to F, each listed in `docs/abi/index.md`, with every internal link and every link into `docs/spec/` resolving (ABI-14)
2. The specification SHALL state the one-sentence contract of each of the twelve tables and fields of proposal §0.3, and a guard SHALL fail when a table of `Shape` has no contract sentence (ABI-14)
3. WHEN the feature closes THEN `.specs/features/device-abi/abi.md` SHALL be a pointer to `docs/abi/`, and no source file, doc comment or tracked document outside `.specs/features/` SHALL cite `abi.md` (ABI-15)
4. WHEN a source file cites a section as `ABI §n` or `ABI §n.m` THEN that section SHALL exist in `docs/abi/` (ABI-15)
5. WHEN `python3 docs/abi/build.py <output>` runs THEN it SHALL write one self-contained HTML page that holds every chapter and annex of the index (ABI-16)
6. The file `mkdocs-spec.yml` SHALL list every file of `docs/abi/` under a "Device ABI" section (ABI-16)
7. WHEN the feature closes THEN the page SHALL be published as an artifact, and `docs/abi/index.md`, `docs/spec/index.md` and the Book's ABI chapter SHALL each link to it (ABI-16)

**Independent Test**: build the page; open it; follow the links from the language specification and the Book.

---

### P1: The Book teaches the ABI ⭐ MVP

**User Story**: As someone who has never written a simulator device, I want the Book to take me from "what is a device" to a working device of my own, so I can write one without reading the solver.

**Why P1**: The user's request: "o abi precisa ser melhor descrito no book, com seus tipos de dados, funcionamento, etc".

**Acceptance Criteria** (ABI-17 to ABI-20):

1. The Book SHALL hold, in Part 2, the chapters `abi` and `device-authoring`, each with its sections in the order concepts and motivation, examples, structures, reference (ABI-17)
2. The chapter `abi` SHALL define each of the nine kinds and every entry of the four catalogues (3 pins, 13 linkages, 6 analog tables, 24 aspects), and a guard SHALL fail on a kind or an entry the chapter does not define (ABI-17)
3. The chapter `abi` SHALL follow one device through load, create and bind, a DC solve, a transient step, a small-signal analysis and a host write, one section each (ABI-17)
4. The inventory SHALL give every `rust` and `msl` block of the two chapters the verdict `run` or `held`, and none the verdict `shown`, `pending` or `refused` (ABI-18)
5. WHEN a chapter prints a `rust` listing THEN its text SHALL equal a marked region of a source file that the `book` test target compiles (ABI-18)
6. WHEN a chapter states a value or an outcome of a worked example THEN a test SHALL run the example and assert it (ABI-18)
7. WHEN the chapter `device-authoring` prints a device in the hand-written form and in the attribute form THEN a test SHALL prove that the two produce equal shapes and, for equal reads, equal writes, partials and commits (ABI-19)
8. The two chapters SHALL contain none of the phrases "for dummies", "dummies", "obviously", "of course", "trivial", "as you know", "same as Rust", "as in Rust", "like Rust" (ABI-20)

**Independent Test**: read the two chapters in order and write the resistor from them; `cargo test -p piperine --test book abi:: device_authoring::`.

---

### P1: Attributes that write the bookkeeping, and nothing else ⭐ MVP

**User Story**: As a device implementer, I want to write a function with typed parameters and say beside each one what it reads or writes, so I do not keep three lists and a frame in step by hand, and I can still see everything that happens.

**Why P1**: The user's request, in the same feature.

**Acceptance Criteria** (ABI-21 to ABI-27):

1. The workspace SHALL hold the proc-macro crate `piperine-abi-macros`, whose only dependencies are `syn`, `quote` and `proc-macro2`, and `piperine-abi` SHALL have no dependency (ABI-21)
2. The transpiler's printed code and every crate's sources SHALL use no attribute of `piperine-abi-macros`, outside the places Q5 allows (ABI-21)
3. The crate `piperine-abi-macros` SHALL be the only crate of the workspace that defines a macro (ABI-21)
4. The crate SHALL export exactly four attribute macros (`shape_tables`, `device_code`, `function`, `reaction`) and accept exactly thirteen helper attributes (`continuous_pin`, `discrete_pin`, `event_pin`, `term`, `limit`, `small_signal_source`, `crossing`, `operating_point_variable`, `trace`, `read`, `write`, `partials`, `commit_site`) (ABI-22)
5. WHEN a device is written with the attributes THEN its shape SHALL equal, entry by entry, the shape of the same device written with the builder, for each of the six twins of design.md (ABI-23)
6. WHEN the attribute form and the hand-written form of a twin are called with the same read bytes THEN their writes, partials and commits SHALL be byte-identical (ABI-23)
7. An expansion SHALL hold only the author's items and inherent constants and methods on the author's type, and SHALL hold no `unsafe`, no `static`, no trait implementation, no new type, no macro definition and no path outside `::piperine_abi` and `::core` (ABI-24)
8. The expansion of each twin SHALL equal a golden file kept in the tree as formatted Rust (ABI-24)
9. IF an attribute is misused in one of the ten ways of the error table below THEN the macro SHALL stop the build with a message that names the attribute, what is wrong and what to write, at the span of the offending token (ABI-25)
10. WHEN an entry is declared by attribute THEN its `origin` SHALL be the `file:line:column` of its declaration (ABI-26)
11. IF an attribute names an index of another table than its aspect or field needs THEN the build SHALL fail with a type error at that expression (ABI-27)

**The ten compile errors (ABI-25)**:

| # | Misuse | Message |
| --- | --- | --- |
| E1 | a parameter other than `&self` with no ABI attribute | `` `<name>` is neither read nor written: add #[read(...)], #[write(...)], #[partials(...)] or #[commit_site(...)] `` |
| E2 | two ABI attributes on one parameter | `` `<name>` has two ABI attributes; a parameter is one entry of one list `` |
| E3 | `#[partials(of = w, against(r, ...))]` naming something that is not a `#[write]` or a `#[read]` parameter of this function | `` `<x>` is not a #[write] parameter of `<function>` `` (or `#[read]`) |
| E4 | a gradient whose array length differs from the number of reads it lists | `` the gradient of `<w>` lists <n> reads and its type holds <m> `` |
| E5 | `#[write]` or `#[partials]` on a parameter that is not `&mut T`; `#[read]` on a reference | `` a write is a `&mut T`: the function fills it `` / `` a read is taken by value: the frame gives no alignment (ABI §8.1) `` |
| E6 | `#[commit_site]` in a `#[function]`; `#[write]` or `#[partials]` in a `#[reaction]` | `` a function writes, a reaction commits (ABI §7) `` |
| E7 | `#[reaction]` with no `triggering_events(...)` | `` a reaction names the event pins that trigger it: #[reaction(triggering_events(...))] `` |
| E8 | a return type other than `Result<(), Fault>` (function) or `Result<ReactionOutcome, Fault>` (reaction) | `` a function returns Result<(), Fault> `` / `` a reaction returns Result<ReactionOutcome, Fault> `` |
| E9 | no `&self` receiver, or a generic, `async` or `unsafe` method | `` an ABI function is a plain method taking &self `` |
| E10 | an unknown key or helper attribute | `` unknown key `<k>`; `<attribute>` takes: <the allowed keys> `` |

**Independent Test**: write the resistor twice, compare the shapes, read the golden expansion and find nothing in it that the hand-written form does not have.

---

### P1: The feature follows the project's rules and leaves its records ⭐ MVP

**User Story**: As a maintainer, I want the new code to pass the guards `code-clarity` put in place and the decision to be on record.

**Why P1**: The rules are binding once `code-clarity` merges.

**Acceptance Criteria** (ABI-28, ABI-29):

1. WHEN the feature closes THEN the guards of `piperine-repository-guards` (structure, boundaries, ownership, naming, documentation, suite) SHALL pass with no allowance row added, except the four proc-macro entry functions, which the language requires to be free functions (ABI-28)
2. The crates `piperine-abi` and `piperine-abi-macros` SHALL build under `#![deny(missing_docs)]` with no `#[allow]` attribute (ABI-28)
3. The file `.specs/STATE.md` SHALL hold a decision record with the eight decisions and the user's words, the macro exception and its scope, and the home of the ABI specification; `CLAUDE.md` rule 7 SHALL name the exception; `CLAUDE.md`, `ARCHITECTURE.md` and the Book's organization chapter SHALL list `piperine-abi-macros` (ABI-29)
4. The file `GAPS.md` SHALL hold a row for `force` and `release` with an owner (ABI-29)

**Independent Test**: the guards crate green; read the decision record.

---

## Process constraints

These bind the execution. They are not product behaviour, so they are held by the task gates and the Verifier, not by a check.

- **Order.** Steps A (a new authoring surface over the present layout), B (producers), C (consumers, through a reading view), D (the raw layout flips), E (the seam). The Build line and the touched crates' tests pass after every commit.
- **Each hand-written device is rewritten once**, in the task that owns its file.
- **One commit crosses three crates**: step C4 (the binding vectors become one per pin table). No other task does.
- **A numeric oracle that moves stops the task.** Expected values are never edited; a difference is a defect of the migration.
- **Gate policy**: per task, the Build line and the touched crates' tests; the full suite once, at final verification.
- **Who does what**: Sonnet implements, Opus verifies, and an author never verifies their own work. Opus implements the tasks marked for it (the lowering, the reading view, C4, the flip, the expanders).
- **Up to two lanes**, each with its own worktree and target directory and its own paths.

## Edge Cases

- WHEN a function writes nothing THEN it is neither kind, it is never called, and its reads are checked for known constants and indices in range only.
- WHEN a function writes a stimulus strength THEN it has one write buffer of 16 bytes; while the present layout stands (steps A to C) the lowering splits it in two cells and moves every later write index by one, partials included.
- IF a `minimum_delay` is under one tick of the session, zero included, THEN it promises nothing and is ignored at bind.
- WHEN `OPTIONAL` is set on a linkage that always provides a net THEN it changes nothing.
- IF a shape other than a default shape adds, drops or retypes a parameter THEN the caller refuses it when `create` returns (ABI-06).
- WHEN an attribute function has more parameters than a reviewer accepts (about five; clippy's `too_many_arguments` fires above seven) THEN the author groups the gradient of a write in one array parameter (`#[partials]`), reads a multi-field pin as one `PlainData` struct, or writes that function against the lens by hand; the two forms mix in one device.
- WHEN a shape's number of entries depends on a parameter THEN the author uses the builder for those entries and may still use `#[function]` for the code.
- IF the artifact service is not reachable at closing THEN the specification is complete in the repository, the task reports, and the links are added when it is published.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| ABI-01 | P1: the contract | Design | Pending |
| ABI-02 | P1: the contract | Design | Pending |
| ABI-03 | P1: the contract | Design | Pending |
| ABI-04 | P1: the contract | Design | Pending |
| ABI-05 | P1: the contract | Design | Pending |
| ABI-06 | P1: the contract | Design | Pending |
| ABI-07 | P1: the contract | Design | Pending |
| ABI-08 | P1: the contract | Design | Pending |
| ABI-09 | P1: nothing changes | Design | Pending |
| ABI-10 | P1: nothing changes | Design | Pending |
| ABI-11 | P1: nothing changes | Design | Pending |
| ABI-12 | P1: six refusals | Design | Pending |
| ABI-13 | P2: the seam | Design | Pending |
| ABI-14 | P1: published specification | Design | Pending |
| ABI-15 | P1: published specification | Design | Pending |
| ABI-16 | P1: published specification | Design | Pending |
| ABI-17 | P1: the Book | Design | Pending |
| ABI-18 | P1: the Book | Design | Pending |
| ABI-19 | P1: the Book | Design | Pending |
| ABI-20 | P1: the Book | Design | Pending |
| ABI-21 | P1: attributes | Design | Pending |
| ABI-22 | P1: attributes | Design | Pending |
| ABI-23 | P1: attributes | Design | Pending |
| ABI-24 | P1: attributes | Design | Pending |
| ABI-25 | P1: attributes | Design | Pending |
| ABI-26 | P1: attributes | Design | Pending |
| ABI-27 | P1: attributes | Design | Pending |
| ABI-28 | P1: rules and records | Design | Pending |
| ABI-29 | P1: rules and records | Design | Pending |

**Coverage:** 29 requirements, 63 acceptance criteria, all mapped to tasks and checks in `tasks.md`.

---

## Departures from the proposal

The proposal was written against `origin/main` at its review snapshot. Read against the tree this feature starts from (`code-clarity`), seven things in it do not hold as written. None changes the contract of Part II.

| The proposal says | What holds | Settled by |
| --- | --- | --- |
| 180 implementations of `Device` and `Factory`, in six crates; steps B2 to B5 take them crate by crate (§7.1, §7.2) | 187 at `97d327d58` (two more in `piperine-plan`'s tests, which the proposal does not list). After `code-clarity` T34 every hand-written device lives once in `piperine-device-fixtures`, so step B is grouped by family of device, not by crate, and the count is far lower | T1 re-takes the count; tasks.md phase 6 |
| `piperine_solver::abi` becomes `piperine_solver::circuit` (step E) | `code-clarity` T137 renames `solver/src/core` to `circuit`; the name is taken | Q6 |
| Step E renames `Outcome`, `Advance`, `Attempt`, `Accepted`, `Shadow` and moves `analyses/adapters.rs` | `code-clarity` T134 gives `Outcome`, `Step`, `Source`, `Held` and `Bound` their subject first, and T31 closes the deviation in `adapters.rs` | T1 lists what is left for step E |
| "Names change once" (§6) | The identifiers of `piperine-abi` are renamed by `code-clarity` T132 and most of them again here | Q7 |
| The Rust helpers are functions `device_type::<D>`, `device_type_with`, `library`, `plain_data_reference` (§20.1, informative); "no macro" | Code rule 3 forbids free functions: they are associated functions and methods. The user asked for authoring attributes, as a scoped exception to rule 7 | Assumptions above; ABI-21 to ABI-27 |
| `raw.rs` takes the structs of Part II; the layout test is in `raw.rs` (step D) | `code-clarity` T78 splits `raw.rs` into `raw/{views,shape,calls,constants}.rs`, and the embedded copy of the crate's sources (`AbiSource::FILES`) lists every file by hand: a new file of `piperine-abi` is added to that table in the same commit | design.md |
| `abi.md` is rewritten from Part II and the manual regenerated from it (decision 8) | The specification of record is published in `docs/abi/` and `abi.md` becomes a pointer to it; 601 lines of Rust in 11 crates cite `abi.md` by section and are re-pointed | Q1; ABI-15 |

## Sources

- `input/proposal-v6.md` - the approved design. Part II is the normative text; §7 the migration; §22 the capabilities C1 to C98; §6 the six refusals; Part IV the decisions.
- `input/proposal-v6-review-history.md` - the findings of each review round and the answer to each.
- `.specs/features/device-abi/abi.md` - the present specification, which this feature retires.
- `.specs/features/code-clarity/{spec,design,tasks}.md` - the code rules, the guards crate and the tree this feature starts from.
- `CLAUDE.md` - the code rules in force, the gate policy, the Book's rules.
- The user's messages of 2026-11-27, quoted under "Decisions recorded" and in the Assumptions table (the version).

---

## Success Criteria

- [ ] A reader holds nine kinds and finds anything else in one of four closed catalogues; every datum of a device has one place.
- [ ] The full suite, parity with ngspice, the twins and the buck targets pass with no expected value edited.
- [ ] A difference between `docs/abi/` and `piperine-abi` (a size, an offset, a constant, a rule name, a cell of the access table) fails a test.
- [ ] The specification is published and linked from the language specification and the Book.
- [ ] A reader new to the subject writes a working resistor from the Book alone, by hand and with the attributes; every listing of the two chapters is compiled and run by a test.
- [ ] The golden expansion of an attribute device reads as the hand-written device.

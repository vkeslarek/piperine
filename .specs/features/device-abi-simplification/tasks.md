# Device ABI Simplification Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/device-abi-simplification/design.md`
**Status**: Draft
**Approved**:
**Build**: `cargo build --workspace --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite`
**Run abicontract**: `cargo test -p piperine-abi --test contract`
**Run abidoc**: `cargo test -p piperine-abi --doc`
**Run analog**: `cargo test -p piperine-analog-solver --test boundary`
**Run digital**: `cargo test -p piperine-digital-solver --test boundary`
**Run host**: `cargo test -p piperine-solver --test host`
**Run transpile**: `cargo test -p piperine-transpile`
**Run macros**: `cargo test -p piperine-abi-macros --lib`
**Run twins**: `cargo test -p piperine-abi-macros --test twins`
**Run expansion**: `cargo test -p piperine-abi-macros --test expansion`
**Run macrodoc**: `cargo test -p piperine-abi-macros --doc`
**Run book**: `cargo test -p piperine --test book`
**Run suite**: `cargo test -p piperine-repository-guards --test suite`
**Run boundaries**: `cargo test -p piperine-repository-guards --test boundaries`
**Run worklists**: `cargo test -p piperine-repository-guards --test worklists`
**Run documentation**: `cargo test -p piperine-repository-guards --test documentation`
**Run authoring**: `cargo test -p piperine-repository-guards --test authoring`

**This feature executes after `code-clarity` merges to `main`.** Every path, crate name and test target below names the tree as `code-clarity`'s design leaves it. T1 re-takes them on the merged `main` and corrects this file before any code is written. The `Run` lines and the expected lines of the proofs name tests this feature writes; each expected line is replaced by the line a real run prints, in the commit of the task that writes the test.

**The version does not move** (user, 2026-11-27: "nao avanca a versão do ABI, ainda nao fizemos release"). The symbol stays `piperine_abi_1`, `MAJOR = 1`, `MINOR = 0`; the contract changes in place. No task bumps a version, adds a second symbol or keeps a compatibility layer.

Executors: Sonnet, except the tasks marked **Opus** (the lowering, the thunks, the producers' commit, the reading view, C4, the flip, the expanders). Opus verifies the feature; an author never verifies their own work. A second round of fixes is Opus's, by an agent that is not the Verifier.

**Lanes.** design.md "Lanes" is the map: two lanes, C (the contract) and S (specification, attributes, Book), each a worktree with its own `CARGO_TARGET_DIR` and `TMPDIR`, each owning the paths the table gives it. A phase merges into the integration branch when its last task is committed; a phase starts from the integration branch. Phases 1, 12 and 13 run alone.

Rules every task follows (repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`. Commands run in the foreground; never `git stash`; never `cargo clean`; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-23).** Per task: the `**Build**` line, `cargo test -p <crate>` for each crate the task touched, and what its `Gate` names. The full suite runs once, in T87.
- **Nothing a run computes changes.** Every twin golden, parity target, buck target and Book value stays as it is, and no tolerance moves; `crates/piperine-parity/src/contract.rs` is not edited. Only what design.md "What may move" lists may move, under the task it names. A task that would move anything else stops and reports.
- **A migration task edits no assertion.** Its proof is the touched crates' existing tests. A test is edited only where it names a renamed item or builds a device, and then only there.
- **Each device is rewritten once**, in the task that owns its file. A task that rewrites a file also re-points that file's `abi.md` citations to `ABI §`.
- **`AbiSource::FILES`.** A task that adds, moves or removes a file of `crates/piperine-abi/src` edits the transpiler's table in the same commit.
- **The code follows the specification.** Where `docs/abi/` and the code differ, the code is wrong. If the specification itself is wrong (a sum of offsets, a rule that contradicts another), the task stops and reports: the text is the user's approved proposal.
- **Code rules.** No free function, tests included (the language's exceptions: `#[test]`, `extern "C"`, the four `#[proc_macro_attribute]` entries); one purpose per type; full words; a `///` on every new item; no `#[allow]`, ever, and no file-scope lint attribute; qualified enum variants; no `unwrap()`/`expect()` on a path a device author's input reaches; about 40 lines and 5 parameters is what a reviewer looks for. The only macros of the tree are those `piperine-abi-macros` defines. New code adds no allowance row to the guards crate, beyond the four proc-macro entries and one row per reasoned `#[expect]` below.
- **`#[expect]`, the last resort (rule 5 as amended; user: "#[allow] é permitido em casos específicos onde não tem jeito MESMO").** Only where there is truly no other way, an item-scoped `#[expect(lint, reason = "…")]` is allowed; the reason says why no design avoids it, and the guard lists it. In this feature the one foreseen case is `clippy::too_many_arguments` on a device function that takes one parameter per read. The design fixes come first, in this order: one gradient array per write; a multi-field pin read as one `PlainData` struct; the function written against the lens. Only **T30** (the hand-written diode fixture), **T40** (its attribute twin) and **T63** (the Book's diode listing) may use it, at most once each, on the authored method. No other task may, an expansion never emits one, and it never hides a design problem: a task that wants one elsewhere stops and reports.
- **The Book.** Every listing runs; a Rust listing is a region of a compiled file; the Book is self-contained and never says "as in Rust"; no condescending phrase.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push and no publication without the user's go-ahead.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Build and test", "Tests", "Code rules"), `AGENTS.md`, the guards crate (`suite_hygiene`, the ratchets of `code-clarity`), `.specs/STATE.md` MD-28, `tests/book/inventory.tsv`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| The contract and the authoring surface (`piperine-abi/src`) | unit | every struct, constant, rule case and cell of the access table of the specification; every field and flag a specification can set; every aspect | `crates/piperine-abi/src/**` (inline), `crates/piperine-abi/tests/contract/**` | `cargo test -p piperine-abi` |
| The call path and the thunks | integration | each entry, each status per entry, a panic in each entry, the caller's checks at `create` | `crates/piperine-abi/tests/{contract,thunks}/**` | `cargo test -p piperine-abi` |
| A migration of producers or consumers (steps B, C, E) | integration | the touched crates' existing targets, unedited; the goldens where the Golden gate applies | `crates/<crate>/tests/**` | `cargo test -p <crate>` |
| Bind checks of the sub-solvers | integration | each refusal with the names it reports | `crates/piperine-{analog,digital}-solver/tests/boundary/**` | `cargo test -p <crate> --test boundary` |
| The expanders (`piperine-abi-macros/src`) | unit | every attribute and key; each of the ten errors by message, line and column; a golden file per expansion | `crates/piperine-abi-macros/src/**` (inline) | `cargo test -p piperine-abi-macros --lib` |
| The attributes in use | integration | six twins: shape and answers against the hand-written form; the audit of every expansion | `crates/piperine-abi-macros/tests/{twins,expansion}/**` | `cargo test -p piperine-abi-macros` |
| Repository guards | integration | one test per guard, each with a unit test of a violating and a clean input | `crates/piperine-repository-guards/tests/**` | `cargo test -p piperine-repository-guards` |
| The specification's text (`docs/abi/`) | integration | the structure guards; the layout and constant tests that read it; the page builds | `crates/piperine-repository-guards/tests/documentation.rs` | `cargo test -p piperine-repository-guards --test documentation` |
| The Book (`docs/book/`, `tests/book/`) | integration | every block of the two chapters `run` or `held`; every stated value asserted; the outline and phrase guards | `tests/book/{abi,device_authoring}.rs`, `tests/book/listings/**` | `cargo test -p piperine --test book` |
| Tracking files with no guard (`baseline.md`, `proofs.md`), publication | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | every task that changes Rust | the `**Build**` line; `cargo test -p <crate>` for each crate whose `src/` or `tests/` the task changed; the `Run` lines its `Done when` names |
| Golden | a task on the path from MSL source to generated bytes (plan, transpile, interpreter), and the flip | Quick; `cargo test -p piperine-transpile --test golden`; `cargo test -p piperine-cli --test twins`; `cargo test -p piperine --test integrated` |
| Solvers | a task of step C, D or E in a solver crate | Quick over `piperine-analog-solver`, `piperine-digital-solver`, `piperine-solver` and `piperine-device-fixtures`; `cargo test -p piperine --test integrated` |
| Book | a task that edits `docs/book/` or `tests/book/` | Quick; `cargo test -p piperine --test book` |
| Build | a task that changes no Rust | the `**Build**` line |
| Final | T87 only | `cargo test --workspace`; `cargo test -p piperine-parity -- --nocapture` with ngspice; the buck targets of `examples/buck/README.md`; `check_proofs.py device-abi-simplification`; `check_proofs.py <feature>` for every feature with `## Checks` |

---

## Execution Plan

Phases are ordered. Within a lane they run in sequence; design.md "Lanes" says which phases of the two lanes run side by side. Tasks within a phase execute in order.

### Phase 1: Ground

Single. Everything later tasks stand on. Runs on the merged `main`.

```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: The specification's text

Lane s. `docs/abi/` from proposal Part II. Wave 1, beside phase 3.

```
T6 → T7 → T8 → T9 → T10 → T11 → T12
```

### Phase 3: Step A: a new authoring surface over the present layout

Lane c. Only `piperine-abi` changes (and the transpiler's table of its files). Wave 1.

```
T13 → T14 → T15 → T16 → T17 → T18 → T19 → T20 → T21
```

### Phase 4: Attributes I: the crate and the tables

Lane s. Starts from phase 3 merged. Wave 2.

```
T22 → T23 → T24 → T25 → T26 → T27
```

### Phase 5: Step B: producers move

Lane c. After each task both surfaces produce the same raw shape. Wave 2.

```
T28 → T29 → T30 → T31 → T32 → T33 → T34
```

### Phase 6: Attributes II: code and the twins

Lane s. Starts from phase 5 merged (the hand-written twins). Wave 3.

```
T35 → T36 → T37 → T38 → T39 → T40 → T41 → T42
```

### Phase 7: Step C: consumers move to the reading view

Lane c. Wave 3.

```
T43 → T44 → T45 → T46 → T47 → T48 → T49
```

### Phase 8: Step D: the raw layout flips

Lane c. Reads `docs/abi/` (phase 2). Wave 4.

```
T50 → T51 → T52 → T53 → T54 → T55 → T56 → T57 → T58
```

### Phase 9: Book: Writing a device

Lane s. Wave 4.

```
T59 → T60 → T61 → T62 → T63 → T64 → T65
```

### Phase 10: Step E: the seam

Lane c. Wave 5.

```
T66 → T67 → T68 → T69 → T70 → T71
```

### Phase 11: Book: The device ABI

Lane s. Starts from phase 8 merged (the listings of the flipped structs). Wave 5.

```
T72 → T73 → T74 → T75 → T76 → T77
```

### Phase 12: The specification of record

Single. Wave 6.

```
T78 → T79 → T80 → T81 → T82 → T83
```

### Phase 13: Close

Single. Wave 6.

```
T84 → T85 → T86 → T87
```

---

## Task Breakdown

### Phase 1: Ground

#### T1: The baseline on the merged `main`

**What**: After `code-clarity` merges: rebase this branch on `main`, tag the base `device-abi-simplification-base`, and write the baseline: every implementation of the device traits by crate and file; every path, crate name, test target and guard test these documents name, as the tree has it; the count of `abi.md` citations by crate; step E's list minus what `code-clarity` T31 and T134 already did; whether `AbiSource` writes sub-directories. Correct the `Where`, `Run` and `Proof` lines of this file to match, in this commit.
**Where**: `.specs/features/device-abi-simplification/baseline.md`
**Depends on**: None
**Reuses**: spec.md "Departures from the proposal"; design.md "Risks & Concerns"
**Requirement**: ABI-10

**Done when**:

- [ ] `baseline.md` holds the five inventories, each with the command that produced it
- [ ] the tag `device-abi-simplification-base` exists at the rebased base
- [ ] every `Run` line of this file selects at least one test at the base, or names a target a later task creates
- [ ] the `**Build**` line is green

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): device-abi-simplification baseline on the merged main`

---

#### T2: The decision record

**What**: Append the next free `MD-n` to `.specs/STATE.md`: the eight decisions of the proposal with the user's words, the version constraint with the user's words, `docs/abi/` as the specification of record cited as `ABI §n.m`, rule 7 amended for `piperine-abi-macros`, a Rust Book listing as a region of a compiled file. Amend rule 7 of `CLAUDE.md` and `AGENTS.md` to name the exception. Add the guard `state_records_the_device_abi_decision` to `boundaries`.
**Where**: `.specs/STATE.md`
**Depends on**: T1
**Reuses**: design.md "Project-level decision"; `state_records_the_solver_structure_decision`
**Requirement**: ABI-29

**Done when**:

- [ ] the record quotes "aceito a proposta do piperine device abi" and "nao avanca a versão do ABI, ainda nao fizemos release"
- [ ] `state_records_the_device_abi_decision` passes and fails on a planted `STATE.md` without the record
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test boundaries`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): record the device ABI decisions and the macro exception`

---

#### T3: The capability ledger

**What**: Write `capabilities.tsv`: one row per C1 to C98 and S1 (capability, mechanism of proposal §22.7, the tests that prove it as `<crate>/<target>::<test>`), filled from the tests as they stand. Add `every_capability_of_the_ledger_names_a_test_that_exists` and `the_unimplemented_capability_names_its_gap` to `worklists`, each with a unit test of a violating and a clean ledger.
**Where**: `.specs/features/device-abi-simplification/capabilities.tsv`
**Depends on**: T2
**Reuses**: proposal §22.1 to §22.7; the guards crate's `Worklist`
**Requirement**: ABI-09

**Done when**:

- [ ] 99 rows; no C-row without a test; a capability with no test today is reported on the Handoff, not given a weaker one
- [ ] the first guard passes; the second is added in T5 with the `GAPS.md` row it reads
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test worklists`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): the ledger of the device ABI's capabilities`

---

#### T4: The guards of the macro exception

**What**: Add to `suite`: `only_the_abi_authoring_crate_defines_a_macro` (no `macro_rules!` anywhere; `proc-macro = true` only in `crates/piperine-abi-macros`) and `the_attributes_are_used_only_where_allowed` (no source outside `crates/piperine-abi-macros/tests` and `tests/book` names `piperine_abi_macros`; the transpiler's printed text and goldens included). Both pass today, before the crate exists.
**Where**: `crates/piperine-repository-guards/tests/suite`
**Depends on**: T3
**Reuses**: `SourceTree`, `ItemScan`
**Requirement**: ABI-21

**Done when**:

- [ ] each guard has a unit test of a violating and a clean source
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test suite`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): only the ABI authoring crate may define a macro`

---

#### T5: The tracking rows

**What**: Add the `GAPS.md` row for `force` and `release` (specified by the language, not implemented, owner: the host surface) and the `ROADMAP.md` row of this feature. Add `the_unimplemented_capability_names_its_gap` to `worklists`: the ledger's S1 row names a `GAPS.md` id that exists.
**Where**: `GAPS.md`
**Depends on**: T4
**Reuses**: proposal §22.6, decision 7
**Requirement**: ABI-29, ABI-09

**Done when**:

- [ ] the S1 row of the ledger names the new id
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test worklists`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs: track force and release as an open item of the host surface`

---

### Phase 2: The specification's text

#### T6: The index of the specification and its structure guards

**What**: Create `docs/abi/index.md`: status (major 1, minor 0, unreleased, changes in place), the nine kinds and four catalogues, the map of files, how to cite (`ABI §n.m`). Add to `documentation`: `every_file_of_the_abi_specification_is_in_its_index`, `every_link_of_the_abi_specification_resolves` (internal anchors and links into `docs/spec/`), `every_table_of_a_shape_has_its_contract` (reads the list of tables from the `Shape` listing once it exists; vacuous until T7).
**Where**: `docs/abi/index.md`
**Depends on**: T2
**Reuses**: proposal §0.1; design.md "The specification"
**Requirement**: ABI-14

**Done when**:

- [ ] the three guards pass, each with a unit test of a violating and a clean tree
- [ ] no sentence names a version other than major 1, minor 0
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): the index of the device ABI specification and its guards`

---

#### T7: Part I: conventions, library and device type, the shape

**What**: Write `part_i_packaging.md` from proposal §8, §9, §10, normative, in the rules of design.md "The specification": every struct one listing with offsets and size, every constant group one table with values, no history, no line citation of the solver. Each of the eleven tables of `Shape` carries its one-sentence contract (proposal §0.3). Add the file to the index.
**Where**: `docs/abi/part_i_packaging.md`
**Depends on**: T6
**Reuses**: proposal §8, §9, §10
**Requirement**: ABI-14

**Done when**:

- [ ] every MUST, offset, size and constant of the proposal's sections is in the file; a sum of offsets that does not match a stated size is reported on the Handoff, not adjusted
- [ ] the three structure guards pass with the file listed
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): part I: conventions, library and device type, the shape`

---

#### T8: Part II: pins, linkage, the analog tables

**What**: Write `part_ii_tables.md` from proposal §11, §12, §13, normative, in the rules of design.md "The specification": every struct one listing with offsets and size, every constant group one table with values, no history, no line citation of the solver. The contract sentences of the three pin tables, the linkage and the six analog tables are here. Add the file to the index.
**Where**: `docs/abi/part_ii_tables.md`
**Depends on**: T7
**Reuses**: proposal §11, §12, §13
**Requirement**: ABI-14

**Done when**:

- [ ] every MUST, offset, size and constant of the proposal's sections is in the file; a sum of offsets that does not match a stated size is reported on the Handoff, not adjusted
- [ ] the three structure guards pass with the file listed
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): part II: pins, linkage, the analog tables`

---

#### T9: Part III: code and aspects, entry points, execution rules

**What**: Write `part_iii_code.md` from proposal §14, §15, §16, normative, in the rules of design.md "The specification": every struct one listing with offsets and size, every constant group one table with values, no history, no line citation of the solver. The aspect table (24 rows) and the access table of §7.3 (13 rows, 3 conditions) are tables a test can parse. Add the file to the index.
**Where**: `docs/abi/part_iii_code.md`
**Depends on**: T8
**Reuses**: proposal §14, §15, §16
**Requirement**: ABI-14

**Done when**:

- [ ] every MUST, offset, size and constant of the proposal's sections is in the file; a sum of offsets that does not match a stated size is reported on the Handoff, not adjusted
- [ ] the three structure guards pass with the file listed
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): part III: code and aspects, entry points, execution rules`

---

#### T10: Part IV: validation, versioning, threading, error reporting

**What**: Write `part_iv_conformance.md` from proposal §17, §18, §19, §20, normative, in the rules of design.md "The specification": every struct one listing with offsets and size, every constant group one table with values, no history, no line citation of the solver. §11 states that the contract is unreleased at major 1, minor 0 and changes in place. Add the file to the index.
**Where**: `docs/abi/part_iv_conformance.md`
**Depends on**: T9
**Reuses**: proposal §17, §18, §19, §20
**Requirement**: ABI-14

**Done when**:

- [ ] every MUST, offset, size and constant of the proposal's sections is in the file; a sum of offsets that does not match a stated size is reported on the Handoff, not adjusted
- [ ] the three structure guards pass with the file listed
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): part IV: validation, versioning, threading, error reporting`

---

#### T11: The informative annexes A to D and F

**What**: Write `annex_a_concepts.md` (the twelve contracts, the three merges by judgement, "where does X go?"), `annex_b_glossary.md`, `annex_c_runtime_areas.md`, `annex_d_solver_phases.md`, `annex_f_previous_contract.md` (the mapping of proposal §21, kept until the first release). Add them to the index.
**Where**: `docs/abi`
**Depends on**: T10
**Reuses**: proposal §0.3, §3, §3.1, §3.2, §4, §5, §21
**Requirement**: ABI-14

**Done when**:

- [ ] each annex is marked informative
- [ ] the structure guards pass
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): the informative annexes`

---

#### T12: The builder of the specification's page

**What**: Write `docs/abi/build.py <output>`: one self-contained HTML page with every chapter and annex of the index and an outline. Add the "Device ABI" section to `mkdocs-spec.yml`. Add to `documentation`: `the_abi_specification_builds_to_one_page` (runs the builder, finds every heading of the index in the page) and `the_specification_site_lists_the_abi`.
**Where**: `docs/abi/build.py`
**Depends on**: T11
**Reuses**: `docs/book/build.py`; the `book` test of `piperine-language` that runs it
**Requirement**: ABI-16

**Done when**:

- [ ] the built page is not tracked (ignored, as the Book's `index.html` is)
- [ ] both guards pass
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `feat(docs): build the device ABI specification as one page`

---

### Phase 3: Step A: a new authoring surface over the present layout

#### T13: Typed indices

**What**: Add `shape/index.rs`: one index type per table (`ContinuousPinIndex`, `DiscretePinIndex`, `EventPinIndex`, `TermIndex`, `LimitIndex`, `SmallSignalSourceIndex`, `CrossingIndex`, `OperatingPointVariableIndex`, `TraceIndex`, `FunctionIndex`, `ReactionIndex`, `CommitSiteIndex`), `ReadPosition`, `WritePosition`. A `compile_fail` doc test passes a `DiscretePinIndex` where a `ContinuousPinIndex` is wanted. Every new file of `crates/piperine-abi/src` is added to `AbiSource::FILES` in this commit.
**Where**: `crates/piperine-abi/src/shape/index.rs`
**Depends on**: T3
**Reuses**: design.md "Data Models"
**Requirement**: ABI-02

**Done when**:

- [ ] the doc test is reported as `compile fail ... ok`
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`, `cargo test -p piperine-transpile --test abi_source`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): typed indices, one type per table of a shape`

---

#### T14: The specifications of pins and the linkage

**What**: Add `ContinuousPinSpecification`, `DiscretePinSpecification`, `EventPinSpecification` and the authoring `Linkage` (`private`, `port`, `parameter`, one constructor per session net, `optional`), with the Rust enums of the contract's constants in qualified variants. Every new file of `crates/piperine-abi/src` is added to `AbiSource::FILES` in this commit.
**Where**: `crates/piperine-abi/src/shape/specification.rs`
**Depends on**: T13
**Reuses**: proposal §11, §12
**Requirement**: ABI-02

**Done when**:

- [ ] one test per field and flag of the three pins and per linkage kind (13)
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): specifications of the three pins and their linkage`

---

#### T15: The specifications of the analog tables

**What**: Add `TermSpecification` (three operators, `GROUND`, `SESSION_DEFAULT`), `LimitSpecification`, `SmallSignalSourceSpecification`, `CrossingSpecification`, `OperatingPointVariableSpecification`, `TraceSpecification`.
**Where**: `crates/piperine-abi/src/shape/specification.rs`
**Depends on**: T14
**Reuses**: proposal §13
**Requirement**: ABI-02

**Done when**:

- [ ] one test per table and per field
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): specifications of the six analog tables`

---

#### T16: Reads, writes and the specifications of code

**What**: Add the authoring `Read` and `Write` (one constructor per aspect, taking the typed index of the aspect's table; `element`, `whole_array`), `FunctionSpecification` (`read::<T>`, `write::<T>`, `partial`, `partial_at`, `owns_limit_correction`), `ReactionSpecification` (`triggered_by`, `read::<T>`, `commit_site`, `irreversible`), `CommitSiteSpecification`.
**Where**: `crates/piperine-abi/src/shape/specification.rs`
**Depends on**: T15
**Reuses**: proposal §14
**Requirement**: ABI-02

**Done when**:

- [ ] one test per aspect (24) and per field of a commit site
- [ ] a typed read records the byte length of its type
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): reads, writes and the specifications of functions and reactions`

---

#### T17: `ShapeBuilder` and the lowering to the present layout (Opus)

**What**: Add `ShapeBuilder` (one method per table, `freeze`) and `Lowering`, which writes what was built as today's raw `Shape`, item by item as the table of proposal §7.2: pin order, linkage as role and global, presence bits, `WHERE_CONDITION` as `HAS_CHECK`, limits and sources as inputs, crossings, variables and traces as watches, one `CAP` watch per `STEP_BOUND` write, a stimulus strength as two cells with every later write index and `Partial.write` moved. `freeze` refuses a typed read or write whose length differs from its aspect's. Every new file of `crates/piperine-abi/src` is added to `AbiSource::FILES` in this commit.
**Where**: `crates/piperine-abi/src/shape/builder.rs`
**Depends on**: T16
**Reuses**: the present `ShapeBuf`; proposal §7.2 step A
**Requirement**: ABI-02, ABI-10

**Done when**:

- [ ] one test per row of the lowering table (14)
- [ ] a function with a write after a stimulus strength and a partial that names it lowers to the right indices
- [ ] the present authoring surface is untouched
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): the shape builder, lowered to the present layout`

---

#### T18: The kind of a function, defined once

**What**: Add `FunctionKind::of`: analog when the function writes an analog table or `STEP_BOUND`, digital when it writes a pin, neither when it writes nothing. Every new file of `crates/piperine-abi/src` is added to `AbiSource::FILES` in this commit.
**Where**: `crates/piperine-abi/src/function_kind.rs`
**Depends on**: T17
**Reuses**: proposal §14.2
**Requirement**: ABI-04

**Done when**:

- [ ] `a_function_is_analog_digital_or_neither_by_what_it_writes` covers each writable aspect and the trace-only function
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): one definition of the kind of a function`

---

#### T19: The traits and the lenses

**What**: Add `DeviceImplementation`, `DeviceFactory`, `ReactionOutcome`, `Fault::needs_another_shape`, `ParameterValues`, and `lens.rs`: `FunctionFrameLens` (`read::<T>`, `write`, `partials`, `wanted`, `array`), `ReactionFrameLens` (`read::<T>`, `commit_site`), `CommitSiteHandle` (`commit`, `commit_element`, `fire`). Copying accessors only. Every new file of `crates/piperine-abi/src` is added to `AbiSource::FILES` in this commit.
**Where**: `crates/piperine-abi/src/lens.rs`
**Depends on**: T18
**Reuses**: the present `Eval`, `React`, `Reads`
**Requirement**: ABI-02

**Done when**:

- [ ] `a_value_is_read_and_written_through_an_unaligned_pointer` passes with a buffer at an odd address
- [ ] no method of a lens returns a reference to a typed value
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): the device traits and the copying lenses`

---

#### T20: The thunks over the new traits (Opus)

**What**: Add to `Thunk<D>` the entries for `DeviceImplementation` and `DeviceFactory`, and `DeviceType::implemented_by`, `DeviceType::made_by`, `Library::of`. They produce today's `Module`: the merged commit becomes `commit` or `commit_element` by the target, a stimulus strength is split into `RE` and `IM`, a panic becomes `FAULT`, `needs_another_shape` becomes `RESHAPE` only from `evaluate`.
**Where**: `crates/piperine-abi/src/thunk.rs`
**Depends on**: T19
**Reuses**: the present thunks
**Requirement**: ABI-05

**Done when**:

- [ ] a device on the new traits is created, evaluated and reacted to through the present `Handle`
- [ ] a panic in each entry returns `FAULT` with the text
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): the C entries over the new device traits`

---

#### T21: Both surfaces give one shape

**What**: Write the six worked examples (resistor, diode, capacitor, D flip-flop, comparator, DAC) on the new surface, in the fixtures crate beside the present ones, and test that each lowers to a raw shape equal, entry by entry, to the present one's, and answers the same bytes.
**Where**: `crates/piperine-device-fixtures/src`
**Depends on**: T20
**Reuses**: the fixtures of `code-clarity` T34; the transpiler's shape comparison rule
**Requirement**: ABI-10

**Done when**:

- [ ] six comparisons pass, each naming the entry on a difference
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`, `cargo test -p piperine-device-fixtures`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(abi): the new authoring surface lowers to the same shapes`

---

### Phase 4: Attributes I: the crate and the tables

#### T22: The crate `piperine-abi-macros`

**What**: Create the proc-macro crate: `Cargo.toml` (`syn`, `quote`, `proc-macro2`; dev: `prettyplease`, `piperine-abi`, `piperine-device-fixtures`), `lib.rs` with its purpose, module map and data path and the four `#[proc_macro_attribute]` entries (each returns its input for now), the workspace member, the four allowance rows for those entries. Add the guards target `authoring`: `the_authoring_macros_depend_on_no_crate_of_the_tree`, `the_abi_crate_has_no_dependency`.
**Where**: `crates/piperine-abi-macros`
**Depends on**: T4, T12, T21
**Reuses**: design.md "`piperine-abi-macros`"
**Requirement**: ABI-21, ABI-28

**Done when**:

- [ ] `#![deny(missing_docs)]`; no `#[allow]` and no `#[expect]` in the crate's sources
- [ ] both guards pass; `only_the_abi_authoring_crate_defines_a_macro` still passes
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`, `cargo test -p piperine-repository-guards`)

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi-macros): the crate of the device authoring attributes`

---

#### T23: `AttributeArguments`: closed keys

**What**: Add `AttributeArguments`: parses the arguments of each of the seventeen attributes against its closed set of keys (flags bare, fields `key = value`, an aspect as `aspect(index)`), keeping the author's spans.
**Where**: `crates/piperine-abi-macros/src/arguments.rs`
**Depends on**: T22
**Reuses**: design.md "The catalogue"
**Requirement**: ABI-22

**Done when**:

- [ ] `the_catalogue_is_four_macros_and_thirteen_helpers` lists the names and their keys
- [ ] an unknown key is error E10 with the allowed keys
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros --lib`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): parse attribute arguments against closed key sets`

---

#### T24: `AuthoringError`: the errors and their spans

**What**: Add `AuthoringError` with one variant per error E1 to E10 of spec ABI-25, its message and its span, and its conversion to a compiler error at that span.
**Where**: `crates/piperine-abi-macros/src/authoring_error.rs`
**Depends on**: T23
**Reuses**: spec.md "The ten compile errors"
**Requirement**: ABI-25

**Done when**:

- [ ] each message equals the spec's table
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros --lib`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): the ten authoring errors`

---

#### T25: The golden expansion files

**What**: Add the test support type `ExpansionGolden`: formats an expansion with `prettyplease` and compares it with `tests/expansion/<name>.expanded.rs`, printing the difference; and `ExpansionAudit`, which fails on `unsafe`, `static`, a trait implementation, a new type, a macro definition, a lint attribute (`#[allow]`, `#[expect]`) that the author did not write, or a path outside `::piperine_abi` and `::core`.
**Where**: `crates/piperine-abi-macros/src/expansion_golden.rs`
**Depends on**: T24
**Reuses**: design.md "Three rules"
**Requirement**: ABI-24

**Done when**:

- [ ] each has a unit test of a clean and a violating token stream
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros --lib`)

**Tests**: unit
**Gate**: quick

**Commit**: `test(abi-macros): golden files and an audit of what an expansion may hold`

---

#### T26: `#[shape_tables]`: pins and linkage (Opus)

**What**: Add `ShapeTablesExpansion` for `#[continuous_pin]`, `#[discrete_pin]`, `#[event_pin]`: `declare` adds each field's entry with the builder in field order and returns the struct. A field's name is the entry's name; `linkage = …` maps to the `Linkage` constructor of the same name.
**Where**: `crates/piperine-abi-macros/src/shape_tables.rs`
**Depends on**: T25
**Reuses**: T14's specifications
**Requirement**: ABI-22, ABI-24

**Done when**:

- [ ] the expansion of a struct of three pins equals its golden file and passes the audit
- [ ] a field with no table attribute is error E1's counterpart for fields, at the field
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): declare pins and their linkage from a struct of indices`

---

#### T27: `#[shape_tables]`: the analog tables and the origin

**What**: Extend `ShapeTablesExpansion` to `#[term]`, `#[limit]`, `#[small_signal_source]`, `#[crossing]`, `#[operating_point_variable]`, `#[trace]`; a key that names another field refers to the index declared above it. Add `OriginStamp`: every entry declared by attribute gets `file:line:column` of its declaration.
**Where**: `crates/piperine-abi-macros/src/shape_tables.rs`
**Depends on**: T26
**Reuses**: T15's specifications
**Requirement**: ABI-22, ABI-26

**Done when**:

- [ ] one golden file with every analog table
- [ ] a key that names a field declared below it is an error at the key
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): declare the analog tables and stamp origins`

---

### Phase 5: Step B: producers move

#### T28: B1: the plan, the printer and the interpreter (Opus)

**What**: Move `piperine-plan`, the printer of `piperine-transpile` and `piperine-interpreter` to the new builder, traits and lenses, in one commit, because the loader compares the printed shape with the plan's. Regenerate the transpile goldens here, once. `plan`'s two test devices move with it.
**Where**: `crates/piperine-plan/src`
**Depends on**: T21
**Reuses**: proposal §7.2 B1
**Requirement**: ABI-10

**Done when**:

- [ ] the loader's shape check passes for every compiled body
- [ ] the golden diff shows the shape-building code and the renamed identifiers and no change in the printed model code
- [ ] no numeric expectation of a twin or of the integrated suite moves
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `refactor(plan,transpile,interpreter): build shapes with the new authoring surface`

---

#### T29: The plan refuses an array read from analog code

**What**: Add to the plan the assertion of ABI §7.3 condition 3: an analog function never reads a discrete pin that is an array (the elaborator never emits one; a foreign producer had no guard).
**Where**: `crates/piperine-plan/src`
**Depends on**: T28
**Reuses**: proposal §14.3, D14
**Requirement**: ABI-12

**Done when**:

- [ ] a planted IR with such a read is refused by name
- [ ] Gate check passes: Golden

**Tests**: unit
**Gate**: golden

**Commit**: `fix(plan): assert that analog code reads no array`

---

#### T30: The fixtures: analog devices

**What**: Rewrite the analog devices of `piperine-device-fixtures` on the new surface, each once, raw `Cell` literals becoming typed reads and writes. Re-point the `abi.md` citations of the files touched to `ABI §`.
**Where**: `crates/piperine-device-fixtures/src`
**Depends on**: T29
**Reuses**: T21's six examples
**Requirement**: ABI-10

**Done when**:

- [ ] the fixtures crate's tests and every dependant crate's tests pass unedited
- [ ] no numeric expectation moves
- [ ] Gate check passes: Quick over the fixtures crate and its dependants

**Tests**: integration
**Gate**: quick

**Last resort**: the diode's hand-written method may carry one reasoned `#[expect(clippy::too_many_arguments, …)]` if the gradient array, struct reads and the lens do not apply; the guard lists it and the Handoff names it.

**Commit**: `refactor(fixtures): the analog devices on the new authoring surface`

---

#### T31: The fixtures: digital devices

**What**: Rewrite the digital devices of `piperine-device-fixtures` on the new surface, each once, raw `Cell` literals becoming typed reads and writes. Re-point the `abi.md` citations of the files touched to `ABI §`.
**Where**: `crates/piperine-device-fixtures/src`
**Depends on**: T30
**Reuses**: T21's six examples
**Requirement**: ABI-10

**Done when**:

- [ ] the fixtures crate's tests and every dependant crate's tests pass unedited
- [ ] no numeric expectation moves
- [ ] Gate check passes: Quick over the fixtures crate and its dependants

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(fixtures): the digital devices on the new authoring surface`

---

#### T32: The fixtures: mixed-signal and analysis devices

**What**: Rewrite the mixed-signal and analysis devices of `piperine-device-fixtures` on the new surface, each once, raw `Cell` literals becoming typed reads and writes. Re-point the `abi.md` citations of the files touched to `ABI §`.
**Where**: `crates/piperine-device-fixtures/src`
**Depends on**: T31
**Reuses**: T21's six examples
**Requirement**: ABI-10

**Done when**:

- [ ] the fixtures crate's tests and every dependant crate's tests pass unedited
- [ ] no numeric expectation moves
- [ ] Gate check passes: Quick over the fixtures crate and its dependants

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(fixtures): the mixed-signal and analysis devices on the new authoring surface`

---

#### T33: The devices left in the tests of `piperine-abi` and of the sub-solvers

**What**: Rewrite every device still written inline in the tests of `piperine-abi` (except the validator's malformed shapes, which keep the present surface until step D), `piperine-analog-solver` and `piperine-digital-solver`.
**Where**: `crates/piperine-analog-solver/tests`
**Depends on**: T32
**Reuses**: baseline.md's inventory
**Requirement**: ABI-10

**Done when**:

- [ ] the three crates' tests pass with no assertion edited
- [ ] Gate check passes: Quick over the three crates

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(tests): the inline devices of the ABI and the sub-solvers on the new surface`

---

#### T34: The devices left in the tests of `piperine-solver`

**What**: Rewrite every device still written inline in the tests of `piperine-solver`. After this task nothing outside `piperine-abi` names the present authoring surface.
**Where**: `crates/piperine-solver/tests`
**Depends on**: T33
**Reuses**: baseline.md's inventory
**Requirement**: ABI-10

**Done when**:

- [ ] `cargo test -p piperine-solver` passes with no assertion edited
- [ ] a search for the present builder's names outside `crates/piperine-abi` finds nothing
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver): the inline test devices on the new surface`

---

### Phase 6: Attributes II: code and the twins

#### T35: `#[function]`: reads (Opus)

**What**: Add `FunctionExpansion` for `&self` and `#[read]` parameters: `<method>_specification` declares each read in parameter order with its Rust type, `<method>_from_frame` reads each by copy and calls the method. Errors E1, E2, E5 (a read by reference), E8, E9.
**Where**: `crates/piperine-abi-macros/src/function.rs`
**Depends on**: T27, T32
**Reuses**: T16's specifications; T19's lens
**Requirement**: ABI-22, ABI-24, ABI-25

**Done when**:

- [ ] a golden file of a function with three reads
- [ ] each error asserted by message, line and column
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): a function's reads from its parameters`

---

#### T36: `#[function]`: writes and gradients (Opus)

**What**: Extend `FunctionExpansion` to `#[write]` (`&mut T`) and `#[partials(of = w, against(r, …))]` (`&mut [f64; N]`, `name[byte_offset]`): writes and partials are declared in parameter order, the frame's write buffers and partials are filled after the call. `owns_limit_correction`. Errors E3, E4, E5 (a write that is not `&mut`).
**Where**: `crates/piperine-abi-macros/src/function.rs`
**Depends on**: T35
**Reuses**: proposal §14.4
**Requirement**: ABI-22, ABI-24, ABI-25

**Done when**:

- [ ] the golden file of the resistor's `current` equals the hand-written form of design.md
- [ ] an attribute the author wrote on the method (a doc comment, a reasoned `#[expect]`) stays on it unchanged; the generated methods carry none
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): a function's writes and gradients`

---

#### T37: `#[reaction]`

**What**: Add `ReactionExpansion`: `triggering_events(…)`, `#[read]`, `#[commit_site(aspect(index), offset, length, minimum_delay, replaces_pending)]` as a `CommitSiteHandle` parameter, `irreversible`. Errors E6, E7, E8.
**Where**: `crates/piperine-abi-macros/src/reaction.rs`
**Depends on**: T36
**Reuses**: proposal §14.5
**Requirement**: ABI-22, ABI-24, ABI-25

**Done when**:

- [ ] the golden file of the flip-flop's `capture`
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): a reaction's events, reads and commit sites`

---

#### T38: `#[device_code]`

**What**: Add `DeviceCodeExpansion`: on an inherent `impl`, one index constant per `#[function]` and `#[reaction]` method in source order, `declare_code`, `evaluate_function`, `run_reaction`; `tables = T` becomes the `tables: &T` parameter of each method's specification.
**Where**: `crates/piperine-abi-macros/src/device_code.rs`
**Depends on**: T37
**Reuses**: design.md "Side by side"
**Requirement**: ABI-22, ABI-24

**Done when**:

- [ ] a golden file of an `impl` with two functions and one reaction
- [ ] an index out of range returns a fault, never a panic
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi-macros): index constants, registration and dispatch for a device's code`

---

#### T39: The ten errors, end to end

**What**: Close the error table: `each_of_the_ten_misuses_stops_with_its_message_at_its_token` runs the four expanders over one misuse per error and asserts message, line and column; a `compile_fail` doc test shows an index of another table failing as a type error at the expression.
**Where**: `crates/piperine-abi-macros/src/authoring_error.rs`
**Depends on**: T38
**Reuses**: T24
**Requirement**: ABI-25, ABI-27

**Done when**:

- [ ] ten cases, none merged
- [ ] the doc test is reported as `compile fail ... ok`
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: unit
**Gate**: quick

**Commit**: `test(abi-macros): every authoring error at its token`

---

#### T40: The twins: resistor, capacitor and diode

**What**: Write the attribute form of the resistor, capacitor and diode in `tests/twins/` and compare each with its hand-written fixture: equal shapes entry by entry, and for a set of read bytes equal writes, partials and commits.
**Where**: `crates/piperine-abi-macros/tests/twins`
**Depends on**: T39
**Reuses**: the hand-written fixtures; the shape comparison rule
**Requirement**: ABI-23

**Done when**:

- [ ] `every_twin_has_the_shape_of_its_hand_written_form` and `every_twin_answers_as_its_hand_written_form` cover these twins
- [ ] each twin passes the partials verifier
- [ ] the diode's function is first written with a gradient array and struct reads; a reasoned `#[expect(clippy::too_many_arguments, …)]` on it is the last resort, listed by the guard and named on the Handoff
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros --test twins`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(abi-macros): the attribute twins of the resistor, capacitor and diode`

---

#### T41: The twins: D flip-flop, comparator and DAC

**What**: Write the attribute form of the D flip-flop, comparator and DAC in `tests/twins/` and compare each with its hand-written fixture: equal shapes entry by entry, and for a set of read bytes equal writes, partials and commits.
**Where**: `crates/piperine-abi-macros/tests/twins`
**Depends on**: T40
**Reuses**: the hand-written fixtures; the shape comparison rule
**Requirement**: ABI-23

**Done when**:

- [ ] `every_twin_has_the_shape_of_its_hand_written_form` and `every_twin_answers_as_its_hand_written_form` cover these twins
- [ ] each twin passes the partials verifier
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros --test twins`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(abi-macros): the attribute twins of the D flip-flop, comparator and DAC`

---

#### T42: The golden expansion of each twin

**What**: Pin the expansion of each of the six twins as a golden file and run the audit over all of them; `an_entry_declared_by_attribute_has_the_origin_of_its_declaration` reads the origins of a twin's shape.
**Where**: `crates/piperine-abi-macros/tests/expansion`
**Depends on**: T41
**Reuses**: T25
**Requirement**: ABI-24, ABI-26

**Done when**:

- [ ] `every_twin_expands_to_its_golden_file`, `an_expansion_holds_only_inherent_items_of_the_authors_type`
- [ ] a reviewer reads the resistor's golden file against design.md's hand-written form
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(abi-macros): pin and audit the expansion of every twin`

---

### Phase 7: Step C: consumers move to the reading view

#### T43: `ShapeView` and the two numberings (Opus)

**What**: Add `ShapeView`, a read-only accessor in the new vocabulary over the present raw shape: the eleven tables as iterators, the kind of each function, the aspects and statuses a consumer matches on, the byte length of any read and write, `InstanceHandle::evaluate`; and `PresentNumbering`, the translation between a pin's index in its table and the present pin index. Every new file goes into `AbiSource::FILES`.
**Where**: `crates/piperine-abi/src/shape/view.rs`
**Depends on**: T34
**Reuses**: proposal §7.2 step C
**Requirement**: ABI-10

**Done when**:

- [ ] for each of the six examples the view of the lowered shape lists what the builder was given
- [ ] the translation is a bijection on every fixture's shape
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): a reading view of a shape in the new vocabulary`

---

#### T44: C1: the analog solver reads the view

**What**: `AnalogPlan`, the assembly and the accept step read `ShapeView`; bindings stay one per pin, looked up through the translation. Re-point the file's `abi.md` citations.
**Where**: `crates/piperine-analog-solver/src/device`
**Depends on**: T43
**Reuses**: proposal §7.2 C1
**Requirement**: ABI-10

**Done when**:

- [ ] `cargo test -p piperine-analog-solver` passes unedited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(analog-solver): read shapes through the view`

---

#### T45: C2: the digital solver reads the view

**What**: `DigitalPlan`, `ReactionRun` and the shadow read `ShapeView`; bindings stay one per pin.
**Where**: `crates/piperine-digital-solver/src`
**Depends on**: T44
**Reuses**: proposal §7.2 C2
**Requirement**: ABI-10

**Done when**:

- [ ] `cargo test -p piperine-digital-solver` passes unedited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(digital-solver): read shapes through the view`

---

#### T46: C3: the owner reads the view

**What**: `Bind::run` goes by linkage kind; the sensitivity analyses and the circuit read `ShapeView`; the owner still builds per-pin vectors. A refused host write says "where condition".
**Where**: `crates/piperine-solver/src/circuit`
**Depends on**: T45
**Reuses**: proposal §7.2 C3; decision 2
**Requirement**: ABI-10, ABI-08

**Done when**:

- [ ] `a_refused_host_write_names_the_instance_and_the_where_condition`
- [ ] `cargo test -p piperine-solver` passes, the one message aside
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver): bind by linkage through the view`

---

#### T47: C4: one binding vector per pin table (Opus)

**What**: In one commit across the three solver crates: the binding vectors become three, one per pin table, indexed by the typed indices; no consumer uses the translation any more.
**Where**: `crates/piperine-solver/src/circuit/bind.rs`
**Depends on**: T46
**Reuses**: proposal §7.2 C4
**Requirement**: ABI-10

**Done when**:

- [ ] the three crates build and pass together
- [ ] `PresentNumbering` has no use outside `piperine-abi`
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver,analog-solver,digital-solver): bindings by pin table`

---

#### T48: C5: the transpiler's comparison and loader

**What**: The shape comparison and the loader read `ShapeView`; `every_compiled_default_shape_equals_its_plan` compares every table.
**Where**: `crates/piperine-transpile/src`
**Depends on**: T47
**Reuses**: proposal §7.2 C5, C90
**Requirement**: ABI-10

**Done when**:

- [ ] a planted difference in each of the eleven tables is refused naming the entry
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `refactor(transpile): compare and load shapes through the view`

---

#### T49: One predicate for the kind of a function

**What**: Both solvers take a function's kind from `FunctionKind`; their own predicates over writes are deleted. Add `the_function_kind_is_decided_in_the_abi_crate` to `boundaries` and `a_function_that_writes_only_traces_is_never_called` to the analog solver.
**Where**: `crates/piperine-analog-solver/src/device/plan.rs`
**Depends on**: T48
**Reuses**: T18
**Requirement**: ABI-04

**Done when**:

- [ ] the guard fails on a planted predicate over write aspects in a solver crate
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solvers): take the kind of a function from the ABI crate`

---

### Phase 8: Step D: the raw layout flips

#### T50: The structs and constants of the specification

**What**: Add the raw structs and constants of ABI Parts I to III under `raw/` beside the present ones, in the modules of design.md, not yet used. Add `SpecificationTables` to the tests' support (reads the listings and constant tables of `docs/abi/`) and the tests `every_struct_has_the_size_and_offsets_of_the_specification`, `every_constant_has_the_value_of_the_specification`, `the_symbol_is_piperine_abi_1_at_major_1_minor_0`.
**Where**: `crates/piperine-abi/src/raw`
**Depends on**: T49, T10
**Reuses**: `docs/abi/` (T7 to T10); the present layout pin
**Requirement**: ABI-01, ABI-07

**Done when**:

- [ ] 32 structs and 11 groups compared; a struct of the specification with no Rust type, or the reverse, fails
- [ ] `MAJOR`, `MINOR` and the symbol are unchanged
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): the structs and constants of the specification, pinned to its text`

---

#### T51: The validator: rules V1 to V7 and V9

**What**: Write `Validator::check` over the new raw structs for V1 to V7 and V9, in order, with `Refusal` (rule, table, name, origin, detail). Tests build malformed shapes as raw literals: one per case (22), the order test, and refusals 4, 5 and 6.
**Where**: `crates/piperine-abi/src/validate.rs`
**Depends on**: T50
**Reuses**: ABI §10; the present `Validator`
**Requirement**: ABI-03, ABI-12

**Done when**:

- [ ] `every_case_of_every_rule_is_refused_with_its_rule_entry_and_origin`, `the_first_rule_broken_is_the_one_reported`
- [ ] `a_session_net_linked_from_the_wrong_table_is_refused`, `two_parameters_of_one_name_are_refused`, `an_element_on_a_read_that_takes_none_is_refused`
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): validation rules V1 to V7 and V9`

---

#### T52: The validator: the access table

**What**: Add `AccessTable` and rule V8: the table of ABI §7.3, its three conditions, and the sentence of §7.2 (a function writes pins or analog tables, not both). The test takes every cell from the specification's table and tries the allowed and the refused ones. Refusal 1.
**Where**: `crates/piperine-abi/src/validate.rs`
**Depends on**: T51
**Reuses**: ABI §7.3
**Requirement**: ABI-03, ABI-12

**Done when**:

- [ ] `every_cell_outside_the_access_table_of_the_specification_is_refused`, `an_analog_function_that_reads_an_array_is_refused`
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(abi): validation rule V8, from the specification's access table`

---

#### T53: The flip (Opus)

**What**: In `piperine-abi`, one commit: the builder writes the new raw structs and the view reads them; `Thunk`, `InstanceHandle` and the verifier use the new frames, the one `commit` entry and a 16-byte stimulus strength; `Lowering`, `PresentNumbering`, the present raw structs, the present authoring surface and the present validator with its tests are deleted; `AbiSource::FILES` follows. No other crate changes.
**Where**: `crates/piperine-abi/src`
**Depends on**: T52
**Reuses**: T50 to T52; proposal §7.2 step D
**Requirement**: ABI-01, ABI-05, ABI-10

**Done when**:

- [ ] no crate other than `piperine-abi` and the transpiler's file table is in the diff
- [ ] every crate's tests pass unedited; the goldens move by identifiers only
- [ ] `cargo test --workspace --no-run` builds
- [ ] Gate check passes: Golden and Solvers

**Tests**: integration
**Gate**: golden

**Commit**: `feat(abi)!: the raw layout of the specification`

---

#### T54: The caller's checks at `create`, and statuses per entry

**What**: `InstanceHandle::create` validates the returned shape and checks its parameters against the default shape before any other call; a status the entry or the context does not allow is a fault naming the instance, the entry and the status.
**Where**: `crates/piperine-abi/src/caller.rs`
**Depends on**: T53
**Reuses**: ABI §8.2, §3.3
**Requirement**: ABI-06, ABI-05

**Done when**:

- [ ] `a_created_shape_is_validated_and_checked_against_the_default_shape` (a missing, an extra and a retyped parameter)
- [ ] `a_status_the_entry_does_not_allow_is_a_fault_naming_instance_entry_and_status`
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi`)

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): the caller checks a created shape against the default shape`

---

#### T55: Analog bind: one writer per strength; the step bound names its function

**What**: Bind refuses a second function that writes the strength of a small-signal source, naming the source and both functions (refusal 3). `E0509` names the function and its origin.
**Where**: `crates/piperine-analog-solver/src/device/plan.rs`
**Depends on**: T54
**Reuses**: the existing check of one writer per term
**Requirement**: ABI-12, ABI-05

**Done when**:

- [ ] `a_strength_written_by_two_functions_is_refused_naming_both`, `a_step_bound_that_is_not_finite_and_positive_fails_naming_its_function`
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `feat(analog-solver): one writer per small-signal strength`

---

#### T56: Digital bind: no loop through an event net; one commit entry; `RESHAPE` in its two conditions

**What**: A dependency loop through an event net is refused (refusal 2). Tests of the merged `commit` over a value, an element and an event, and of `RESHAPE` outside the parameter cone or off the shadow store.
**Where**: `crates/piperine-digital-solver/src/plan.rs`
**Depends on**: T55
**Reuses**: ABI §9.1, §9.3
**Requirement**: ABI-12, ABI-05

**Done when**:

- [ ] `a_loop_through_an_event_net_is_refused`, `one_commit_entry_serves_a_value_an_element_and_an_event`, `a_reshape_outside_the_cone_or_off_the_shadow_is_a_fault_naming_the_function`
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `feat(digital-solver): refuse a loop through an event net`

---

#### T57: The cache and the loader after the change

**What**: Add `a_library_compiled_before_the_layout_changed_has_another_key` (the key over the base tag's ABI sources differs from the present one, so the old library is not opened) and `a_library_of_a_smaller_size_or_a_newer_minor_is_refused`.
**Where**: `crates/piperine-transpile/tests`
**Depends on**: T56
**Reuses**: the cache key over `AbiSource`
**Requirement**: ABI-07

**Done when**:

- [ ] both tests pass; neither changes `MAJOR`, `MINOR` or the symbol
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `test(transpile): a library of the previous layout is never opened`

---

#### T58: Nothing of the migration survives

**What**: Add to `suite`: `no_migration_scaffold_survives` (`Lowering`, `PresentNumbering`, `PresentShape`) and `no_identifier_of_the_previous_contract_survives` (`Cell`, `Statement`, `InputSpec`, `WatchSpec`, `ShapeBuf`, `pin_flag`, `role::`, `ambient::`, `statement_flag`), over sources, doc comments and the transpiler's printed text.
**Where**: `crates/piperine-repository-guards/tests/suite`
**Depends on**: T57
**Reuses**: the dead-identifier guard of `suite_hygiene`
**Requirement**: ABI-11

**Done when**:

- [ ] each guard has a unit test of a violating source
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test suite`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): no scaffold and no name of the previous contract survives`

---

### Phase 9: Book: Writing a device

#### T59: Rust listings of the Book are regions of compiled files

**What**: Add the support type `RustListing` (a region between `// listing <id>` and `// end listing` of a file under `tests/book/listings/`), the module `device_authoring` of the `book` target, the inventory's handling of a `rust` block with the verdict `run` or `held`, and the guards `no_listing_of_the_abi_chapters_is_only_shown` and `the_chapters_use_no_condescending_phrase` (both read a list of chapter ids, empty until a chapter joins it). Add `piperine-abi-macros` to the root package's dev-dependencies.
**Where**: `tests/book/support`
**Depends on**: T42
**Reuses**: `Chapter`, the inventory, the claims guard
**Requirement**: ABI-18, ABI-20

**Done when**:

- [ ] each guard has a test of a violating chapter text
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `test(book): hold Rust listings to regions of compiled files`

---

#### T60: "Writing a device": concepts, and the resistor by hand

**What**: Open the chapter `device-authoring` in Part 2 after `abi`: what a device is to its author (a type; a shape built once; a function is a method that turns reads into writes), then the resistor written with the builder and the lens, line by line, placed in a divider and run. Every construct beyond functions and structs is explained where it first appears.
**Where**: `docs/book/content.html`
**Depends on**: T59
**Reuses**: design.md "The Book's two chapters"; the fixture resistor
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] every `rust` and `msl` block has an inventory row `run`
- [ ] the divider's middle node is 2 V in the chapter and in the test
- [ ] the chapter joins the two guards' list
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): writing a device, the resistor by hand`

---

#### T61: "Writing a device": the resistor with the attributes

**What**: The same resistor with the attributes, side by side with the hand-written form; what each attribute expands to, shown from the golden file; what the attributes never do. `the_two_forms_of_a_device_are_one_device` compares the two listings' devices.
**Where**: `docs/book/content.html`
**Depends on**: T60
**Reuses**: T42's golden files; design.md "Side by side"
**Requirement**: ABI-18, ABI-19

**Done when**:

- [ ] the expansion listing is `held` against the golden file
- [ ] the two forms have equal shapes and equal answers
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): the attributes, side by side with the hand-written device`

---

#### T62: "Writing a device": the flip-flop

**What**: A digital function that fires an event pin, a reaction, a commit site, in both forms; the flip-flop clocked in a small circuit and run.
**Where**: `docs/book/content.html`
**Depends on**: T61
**Reuses**: the twin of T41
**Requirement**: ABI-18, ABI-19

**Done when**:

- [ ] the output is low, then high after the edge, in the chapter and in the test
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): a flip-flop as a device, by hand and with attributes`

---

#### T63: "Writing a device": the diode

**What**: A limit and its four aspects, a noise source, a partial against the limit; why the solver holds the iterate. The diode run at an operating point and in a noise analysis.
**Where**: `docs/book/content.html`
**Depends on**: T62
**Reuses**: the twin of T40
**Requirement**: ABI-18

**Done when**:

- [ ] the stated current and noise density are asserted by the test
- [ ] the listing shows the gradient array and the struct read; if the compiled region carries a reasoned `#[expect]`, the chapter prints it and says in one sentence why
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): a diode with limiting and noise`

---

#### T64: "Writing a device": a foreign device with its own clock

**What**: The programmable counter (spec Q8): state in a private discrete pin, `HORIZON`, a wake commit with `REPLACES_PENDING`, a second reaction that reads a continuous pin, and what `IRREVERSIBLE` would cost. Written, compiled and run; it replaces the AVR sketch of the present chapter.
**Where**: `tests/book/listings`
**Depends on**: T63
**Reuses**: proposal §15.5; the present chapter's four points on the emulator
**Requirement**: ABI-18

**Done when**:

- [ ] the counter's value after a stated time is asserted, with and without a rejected step in between
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): a foreign device that runs its own clock`

---

#### T65: "Writing a device": structures and reference

**What**: The builder, the specifications, the lenses and the trait, a sentence per method; the seventeen attributes in one table; the ten compile errors, each with the code that causes it and its message, `held` against `AuthoringError`.
**Where**: `docs/book/content.html`
**Depends on**: T64
**Reuses**: annex E's content; spec.md's error table
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] `the_chapters_follow_the_manual_order` passes for the chapter
- [ ] `every_rust_listing_is_a_region_of_a_compiled_file`, `the_worked_examples_give_the_values_the_chapter_states`
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): the authoring reference and the compile errors`

---

### Phase 10: Step E: the seam

#### T66: `DigitalSide` and its adapter

**What**: `DiscreteSide` becomes `DigitalSide`; the adapter of that name becomes `DigitalSolverAdapter`. Every use in the workspace follows in this commit; no test's assertion changes.
**Where**: `crates/piperine-solver/src/protocol/sides.rs`
**Depends on**: T58
**Reuses**: proposal §7.2 step E; baseline.md's list of what is left
**Requirement**: ABI-13

**Done when**:

- [ ] the three solver crates' tests pass with only names edited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver): the digital side of the seam is named for its solver`

---

#### T67: Typed identifiers on the seam

**What**: The bare `u32` of the seam become `NetId`, `AnalogCopySlot`, `EventNetId` and `SampleIndex`. Every use in the workspace follows in this commit; no test's assertion changes.
**Where**: `crates/piperine-solver/src/protocol/sides.rs`
**Depends on**: T66
**Reuses**: proposal §7.2 step E; baseline.md's list of what is left
**Requirement**: ABI-13

**Done when**:

- [ ] the three solver crates' tests pass with only names edited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver): typed identifiers on the seam`

---

#### T68: `DeliveredChange` and the results' nouns

**What**: `Boundary` becomes `DeliveredChange`; `Advance`, `Attempt`, `Accepted` and `Shadow` gain the noun they are the result of, where `code-clarity` left them. Every use in the workspace follows in this commit; no test's assertion changes.
**Where**: `crates/piperine-solver/src/protocol`
**Depends on**: T67
**Reuses**: proposal §7.2 step E; baseline.md's list of what is left
**Requirement**: ABI-13

**Done when**:

- [ ] the three solver crates' tests pass with only names edited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver): name what the seam delivers and returns`

---

#### T69: The two environments and `BoundNet`

**What**: The two `Ambient` become `SolveEnvironment` and `InstantEnvironment`; the owner's `Site` becomes `BoundNet`. Every use in the workspace follows in this commit; no test's assertion changes.
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs`
**Depends on**: T68
**Reuses**: proposal §7.2 step E; baseline.md's list of what is left
**Requirement**: ABI-13

**Done when**:

- [ ] the three solver crates' tests pass with only names edited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solvers): the environment of a solve and of an instant`

---

#### T70: `piperine_solver::device_authoring`

**What**: `piperine_solver::abi` becomes `piperine_solver::device_authoring` (spec Q6); `analyses/adapters.rs` moves to `protocol/` if `code-clarity` left it. Every use in the workspace follows in this commit; no test's assertion changes.
**Where**: `crates/piperine-solver/src/device_authoring.rs`
**Depends on**: T69
**Reuses**: proposal §7.2 step E; baseline.md's list of what is left
**Requirement**: ABI-13

**Done when**:

- [ ] the three solver crates' tests pass with only names edited
- [ ] Gate check passes: Solvers

**Tests**: integration
**Gate**: solvers

**Commit**: `refactor(solver): the device author's module says what it is`

---

#### T71: The seam's guards

**What**: Add to `suite`: `the_seam_uses_its_final_names` (the retired names of T66 to T70 appear nowhere) and `the_seam_passes_no_bare_integer` (no method of `AnalogSide` or `DigitalSide` takes a net, a slot, an event net or a sample as an integer).
**Where**: `crates/piperine-repository-guards/tests/suite`
**Depends on**: T70
**Reuses**: `ItemScan`
**Requirement**: ABI-13

**Done when**:

- [ ] each guard has a unit test of a violating source
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test suite`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): the seam's names and typed identifiers`

---

### Phase 11: Book: The device ABI

#### T72: "The device ABI": concepts and motivation

**What**: Rewrite the chapter `abi` from its opening: what a device is; why it has no memory and what that buys; the nine kinds, one paragraph and one small example each; "where does X go?". Add the guards `the_chapters_follow_the_manual_order` and `every_kind_and_catalogue_entry_is_defined`, each over an outline file (`tests/book/abi_outline.tsv`) that later tasks extend. The chapter joins the lists of T59's guards.
**Where**: `docs/book/content.html`
**Depends on**: T58, T65
**Reuses**: design.md "The Book's two chapters"; annexes A, C and D of the specification
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] the outline holds the nine kinds; the two guards pass and each has a test of a violating text
- [ ] no sentence carries over the previous vocabulary (cell, input, watch, module, prototype)
- [ ] every block of the section has an inventory row `run` or `held`; every sentence that states a value has a claim row
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): the device ABI, concepts and motivation`

---

#### T73: "The device ABI": a device through the solver, I

**What**: Follow the resistor and the flip-flop through load, create and bind, and a DC solve: which table the solver reads at each step and which entry it calls.
**Where**: `docs/book/content.html`
**Depends on**: T72
**Reuses**: design.md "The Book's two chapters"; annexes A, C and D of the specification
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] the outline gains the three phase sections
- [ ] the values the sections state are asserted by `one_device_is_followed_through_every_phase`
- [ ] every block of the section has an inventory row `run` or `held`; every sentence that states a value has a claim row
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): a device through load, bind and a DC solve`

---

#### T74: "The device ABI": a device through the solver, II

**What**: A transient step (checkpoint, look-ahead, landing, rejection), a small-signal analysis, a host write with a where condition.
**Where**: `docs/book/content.html`
**Depends on**: T73
**Reuses**: design.md "The Book's two chapters"; annexes A, C and D of the specification
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] the outline gains the three phase sections
- [ ] a refused host write is shown with its message, `held`
- [ ] every block of the section has an inventory row `run` or `held`; every sentence that states a value has a claim row
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): a device through a transient step, AC and a host write`

---

#### T75: "The device ABI": structures

**What**: The three pins and the linkage, the six analog tables, a function and a reaction, the two frames: each with its listing (a region of `crates/piperine-abi/src/raw/**`, compiled by definition) and a sentence per field.
**Where**: `docs/book/content.html`
**Depends on**: T74
**Reuses**: design.md "The Book's two chapters"; annexes A, C and D of the specification
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] the outline gains the 3 pins, the 13 linkages and the 6 analog tables
- [ ] every block of the section has an inventory row `run` or `held`; every sentence that states a value has a claim row
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): the structures of the device ABI`

---

#### T76: "The device ABI": reference

**What**: The 24 aspects with who may use each, the statuses, the nine validation rules with an example of each refusal, the links to the sections of the specification.
**Where**: `docs/book/content.html`
**Depends on**: T75
**Reuses**: design.md "The Book's two chapters"; annexes A, C and D of the specification
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] the outline gains the 24 aspects; `every_kind_and_catalogue_entry_is_defined` now covers 9 kinds and 46 entries
- [ ] each reference table names its `ABI §`
- [ ] every block of the section has an inventory row `run` or `held`; every sentence that states a value has a claim row
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): the reference tables of the device ABI`

---

#### T77: "What your MSL becomes" and the cheat sheet

**What**: Rewrite the table "What your MSL becomes" in the new vocabulary, each row run from an MSL listing; update the cheat sheet's ABI rows.
**Where**: `docs/book/content.html`
**Depends on**: T76
**Reuses**: design.md "The Book's two chapters"; annexes A, C and D of the specification
**Requirement**: ABI-17, ABI-18

**Done when**:

- [ ] each row's MSL listing compiles and its shape holds the entry the row names
- [ ] every block of the section has an inventory row `run` or `held`; every sentence that states a value has a claim row
- [ ] Gate check passes: Book

**Tests**: integration
**Gate**: book

**Commit**: `docs(book): what MSL becomes, in the words of the device ABI`

---

### Phase 12: The specification of record

#### T78: Annex E: the Rust authoring surface and the attributes

**What**: Write `annex_e_rust_authoring.md`: the builder, the specifications, the traits, the lenses, the seventeen attributes with their keys and what each generates, the three rules, the ten errors. Normative for the two crates. `the_catalogue_is_four_macros_and_thirteen_helpers` reads its attribute table.
**Where**: `docs/abi/annex_e_rust_authoring.md`
**Depends on**: T71, T77
**Reuses**: design.md "`piperine-abi-macros`"; proposal §20.1
**Requirement**: ABI-14, ABI-22

**Done when**:

- [ ] the structure guards pass with the annex listed
- [ ] an attribute or key in the crate and not in the annex, or the reverse, fails
- [ ] Gate check passes: Quick (`cargo test -p piperine-abi-macros`, `cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): annex E, the Rust authoring surface and the attributes`

---

#### T79: The section map and the citation guards

**What**: Write `abi_section_map.tsv` (each section of the retired document to its `ABI §n.m`, or `gone` with the reason). Add to `documentation`, as ratchets with one allowance row per crate that still cites: `no_file_cites_the_retired_abi_document`, `every_cited_abi_section_exists`.
**Where**: `.specs/features/device-abi-simplification/abi_section_map.tsv`
**Depends on**: T78
**Reuses**: proposal §21; the guards crate's `Allowance`
**Requirement**: ABI-15

**Done when**:

- [ ] every section of the retired document has a row
- [ ] both guards pass with the allowances
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): citations of the device ABI resolve against its specification`

---

#### T80: Citations re-pointed: `piperine-abi`, `piperine-plan`, `piperine-ir`, `piperine-language`, `piperine-api`, `piperine-parity`

**What**: Re-point every `abi.md` citation of `piperine-abi`, `piperine-plan`, `piperine-ir`, `piperine-language`, `piperine-api`, `piperine-parity` to `ABI §n.m` by the section map; a citation of a section that is gone is rewritten to the rule that replaced it or removed with its sentence. Delete the crates' allowance rows.
**Where**: `crates/piperine-plan/src`
**Depends on**: T79
**Reuses**: T79's map
**Requirement**: ABI-15

**Done when**:

- [ ] no row of these crates is left in the allowance
- [ ] the crates build; no test is edited
- [ ] Gate check passes: Quick over the crates touched

**Tests**: integration
**Gate**: quick

**Commit**: `docs: cite the device ABI specification in `piperine-abi`, `piperine-plan`, `piperine-ir`, `piperine-language`, `piperine-api`, `piperine-parity``

---

#### T81: Citations re-pointed: `piperine-transpile` and `piperine-interpreter`

**What**: Re-point every `abi.md` citation of `piperine-transpile` and `piperine-interpreter` to `ABI §n.m` by the section map; a citation of a section that is gone is rewritten to the rule that replaced it or removed with its sentence. Delete the crates' allowance rows.
**Where**: `crates/piperine-transpile/src`
**Depends on**: T80
**Reuses**: T79's map
**Requirement**: ABI-15

**Done when**:

- [ ] no row of these crates is left in the allowance
- [ ] the crates build; no test is edited
- [ ] Gate check passes: Quick over the crates touched

**Tests**: integration
**Gate**: quick

**Commit**: `docs: cite the device ABI specification in `piperine-transpile` and `piperine-interpreter``

---

#### T82: Citations re-pointed: the three solver crates

**What**: Re-point every `abi.md` citation of the three solver crates to `ABI §n.m` by the section map; a citation of a section that is gone is rewritten to the rule that replaced it or removed with its sentence. Delete the crates' allowance rows.
**Where**: `crates/piperine-solver/src`
**Depends on**: T81
**Reuses**: T79's map
**Requirement**: ABI-15

**Done when**:

- [ ] no row of these crates is left in the allowance
- [ ] the crates build; no test is edited
- [ ] Gate check passes: Quick over the crates touched

**Tests**: integration
**Gate**: quick

**Commit**: `docs: cite the device ABI specification in the three solver crates`

---

#### T83: The retired document becomes a pointer

**What**: Reduce `.specs/features/device-abi/abi.md` to a pointer: what it was, the decision that retired it, where the specification is. `docs/spec/index.md` and `docs/spec/appendix_c_implementation.md` link to `docs/abi/`. The two citation guards lose their allowance.
**Where**: `.specs/features/device-abi/abi.md`
**Depends on**: T82
**Reuses**: T2's decision record
**Requirement**: ABI-15, ABI-16

**Done when**:

- [ ] the pointer is under twenty lines
- [ ] the two guards pass with no allowance
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards --test documentation`)

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): the device ABI document of record is docs/abi`

---

### Phase 13: Close

#### T84: The project's documents

**What**: `CLAUDE.md`, `AGENTS.md` (crate table, pipeline, rule 7, the Documentation section), `ARCHITECTURE.md`, `README.md` where it names the ABI, and the Book's "How Piperine is organized": `piperine-abi-macros`, `docs/abi/`, the new names.
**Where**: `CLAUDE.md`
**Depends on**: T83
**Reuses**: `instructions_list_the_kept_crates`, `the_architecture_document_names_every_step_and_owner`
**Requirement**: ABI-29

**Done when**:

- [ ] both guards pass
- [ ] Gate check passes: Book and `cargo test -p piperine-repository-guards`

**Tests**: integration
**Gate**: book

**Commit**: `docs: the device ABI's specification and attributes in the project's documents`

---

#### T85: The ledger and the guards at the close

**What**: Bring `capabilities.tsv` to the final test names; confirm no allowance row was added by this feature beyond the four proc-macro entries; give every guard of the guards crate a last run.
**Where**: `.specs/features/device-abi-simplification/capabilities.tsv`
**Depends on**: T84
**Reuses**: T3
**Requirement**: ABI-09, ABI-28

**Done when**:

- [ ] `every_capability_of_the_ledger_names_a_test_that_exists` passes with 98 capability rows
- [ ] the diff of the guards' lists against the base tag adds the four proc-macro entries and one row per reasoned `#[expect]` (at most three), each named on the Handoff
- [ ] Gate check passes: Quick (`cargo test -p piperine-repository-guards`)

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): the device ABI's capability ledger at the close`

---

#### T86: Publication

**What**: Ask the user's go-ahead. Build the page with `docs/abi/build.py`, publish it as an artifact beside the language specification and the Book, and write its link into `docs/abi/index.md`, `docs/spec/index.md` and the Book's chapter `abi`; rebuild and republish the Book and the language specification, whose pages gained the link.
**Where**: `docs/abi/index.md`
**Depends on**: T85
**Reuses**: the Book's and the specification's publication
**Requirement**: ABI-16

**Done when**:

- [ ] the three pages open and link to each other; logged on the Handoff's `**Walked:**` line
- [ ] if the service is not reachable the task reports and the feature closes without the links
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(abi): link the published specification`

---

#### T87: Final verification

**What**: The Final gate, once: `cargo test --workspace`; parity with ngspice; the buck targets by name; `check_proofs.py device-abi-simplification`; `check_proofs.py` of every feature with a `## Checks`. Then the Verifier (Opus, not an author of any task).
**Where**: `.specs/features/device-abi-simplification/proofs.md`
**Depends on**: T86
**Reuses**: the skill's `check_proofs.py`
**Requirement**: ABI-10

**Done when**:

- [ ] every proof is green at `HEAD`
- [ ] no expected value, tolerance or frozen fixture differs from the base tag
- [ ] Gate check passes: Final

**Tests**: none
**Gate**: final

**Commit**: `docs(specs): device-abi-simplification final verification`

---

## Checks

One check per acceptance criterion of spec.md, in its order.

**C1** - Every `#[repr(C)]` struct has the size and the field offsets that the listings of `docs/abi/` give. (ABI-01, AC 1)
Proof: run abicontract | test layout::every_struct_has_the_size_and_offsets_of_the_specification ... ok

**C2** - Every constant of every group has the value the tables of `docs/abi/` give. (ABI-01, AC 2)
Proof: run abicontract | test layout::every_constant_has_the_value_of_the_specification ... ok

**C3** - The builder returns an index typed for the entry's table; passing an index of another table does not compile. (ABI-02, AC 3)
Proof: run abidoc | shape::index::ContinuousPinIndex compile fail ... ok

**C4** - A lens reads and writes by copy through a pointer that is not aligned, and offers no typed borrow. (ABI-02, AC 4)
Proof: run abicontract | test lens::a_value_is_read_and_written_through_an_unaligned_pointer ... ok

**C5** - Each of the 22 cases of V1 to V7 and V9 is refused with its rule, the entry's table and name, and its origin. (ABI-03, AC 5)
Proof: run abicontract | test validate::every_case_of_every_rule_is_refused_with_its_rule_entry_and_origin ... ok

**C6** - Every cell outside the access table of ABI §7.3, each of its three conditions, and a function that writes a pin and an analog table are refused with `AccessNotAllowed`. (ABI-03, AC 6)
Proof: run abicontract | test validate::every_cell_outside_the_access_table_of_the_specification_is_refused ... ok

**C7** - A shape that breaks two rules is refused with the one that comes first in V1 to V9. (ABI-03, AC 7)
Proof: run abicontract | test validate::the_first_rule_broken_is_the_one_reported ... ok

**C8** - A function is analog when it writes an analog table or `STEP_BOUND`, digital when it writes a pin, neither when it writes nothing. (ABI-04, AC 8)
Proof: run abicontract | test function_kind::a_function_is_analog_digital_or_neither_by_what_it_writes ... ok

**C9** - A function whose only writes are traces is never called. (ABI-04, AC 9)
Proof: run analog | test a_function_that_writes_only_traces_is_never_called ... ok

**C10** - Neither solver holds a predicate of its own over a function's writes. (ABI-04, AC 10)
Proof: run boundaries | test the_function_kind_is_decided_in_the_abi_crate ... ok

**C11** - A commit to a value, to an element of an array and to an event pin goes through the one `commit` entry. (ABI-05, AC 11)
Proof: run digital | test one_commit_entry_serves_a_value_an_element_and_an_event ... ok

**C12** - `RESHAPE` outside the parameter cone, or off the shadow store, is a fault that names the function. (ABI-05, AC 12)
Proof: run digital | test a_reshape_outside_the_cone_or_off_the_shadow_is_a_fault_naming_the_function ... ok

**C13** - A status the entry or the context does not allow is a fault naming the instance, the entry and the status. (ABI-05, AC 13)
Proof: run abicontract | test caller::a_status_the_entry_does_not_allow_is_a_fault_naming_instance_entry_and_status ... ok

**C14** - A step bound that is neither finite and positive nor positive infinity fails with `E0509`, naming the function and its origin. (ABI-05, AC 14)
Proof: run analog | test a_step_bound_that_is_not_finite_and_positive_fails_naming_its_function ... ok

**C15** - The caller validates a created shape and refuses one whose parameters differ from the default shape's, before any other call. (ABI-06, AC 15)
Proof: run abicontract | test caller::a_created_shape_is_validated_and_checked_against_the_default_shape ... ok

**C16** - A library with a `size` under the loader's `Library`, or a newer `minor`, is refused. (ABI-07, AC 16)
Proof: run transpile | test a_library_of_a_smaller_size_or_a_newer_minor_is_refused ... ok

**C17** - A library compiled before the layout changed has another cache key and is not opened. (ABI-07, AC 17)
Proof: run transpile | test a_library_compiled_before_the_layout_changed_has_another_key ... ok

**C18** - The exported symbol is `piperine_abi_1`, `MAJOR` is 1 and `MINOR` is 0. (ABI-07, AC 18)
Proof: run abicontract | test layout::the_symbol_is_piperine_abi_1_at_major_1_minor_0 ... ok

**C19** - A refused host write names the instance and the pin of the condition, and calls it a "where condition". (ABI-08, AC 19)
Proof: run host | test a_refused_host_write_names_the_instance_and_the_where_condition ... ok

**C20** - Each of C1 to C98 has a row in the ledger naming a test that exists. (ABI-09, AC 1)
Proof: run worklists | test every_capability_of_the_ledger_names_a_test_that_exists ... ok

**C21** - The S1 row names a `GAPS.md` row that exists. (ABI-09, AC 2)
Proof: run worklists | test the_unimplemented_capability_names_its_gap ... ok

**C22** - The tolerance contract, the twin goldens and the frozen fixtures are byte-identical to the base tag. (ABI-10, AC 3)
Proof: `test -z "$(git diff --name-only device-abi-simplification-base HEAD -- crates/piperine-parity/src/contract.rs crates/piperine-cli/tests/twins tests/e2e)"`

**C23** - Every parity target passes against ngspice under the unchanged contract. (ABI-10, AC 4)
Proof: `cargo test -p piperine-parity -- --nocapture`

**C24** - The default shape of every compiled device type equals its plan's shape, entry by entry. (ABI-10, AC 5)
Proof: run transpile | test every_compiled_default_shape_equals_its_plan ... ok

**C25** - No scaffold of the migration and no identifier of the previous contract remains. (ABI-11, AC 6)
Proof: run suite | test no_migration_scaffold_survives ... ok
Proof: run suite | test no_identifier_of_the_previous_contract_survives ... ok

**C26** - An analog function that reads an array discrete pin is refused with `AccessNotAllowed`. (ABI-12, AC 1)
Proof: run abicontract | test validate::an_analog_function_that_reads_an_array_is_refused ... ok

**C27** - A dependency loop through an event net is refused at bind; `EventPin` has three fields and 48 bytes. (ABI-12, AC 2)
Proof: run digital | test a_loop_through_an_event_net_is_refused ... ok

**C28** - A strength written by two functions is refused at bind, naming the source and both functions. (ABI-12, AC 3)
Proof: run analog | test a_strength_written_by_two_functions_is_refused_naming_both ... ok

**C29** - A session net linked from the wrong pin table is refused with `LinkageNotAllowed`. (ABI-12, AC 4)
Proof: run abicontract | test validate::a_session_net_linked_from_the_wrong_table_is_refused ... ok

**C30** - Two parameters of one name are refused with `LinkageRepeated`. (ABI-12, AC 5)
Proof: run abicontract | test validate::two_parameters_of_one_name_are_refused ... ok

**C31** - A nonzero element on a read that is not of a discrete pin is refused with `NotApplicable`. (ABI-12, AC 6)
Proof: run abicontract | test validate::an_element_on_a_read_that_takes_none_is_refused ... ok

**C32** - The seam uses its final names and none of the retired ones. (ABI-13, AC 1)
Proof: run suite | test the_seam_uses_its_final_names ... ok

**C33** - No method of `AnalogSide` or `DigitalSide` takes a net, a slot, an event net or a sample as a bare integer. (ABI-13, AC 2)
Proof: run suite | test the_seam_passes_no_bare_integer ... ok

**C34** - `docs/abi/` holds chapters 1 to 13 and annexes A to F, each in the index, every link resolving. (ABI-14, AC 1)
Proof: run documentation | test every_file_of_the_abi_specification_is_in_its_index ... ok
Proof: run documentation | test every_link_of_the_abi_specification_resolves ... ok

**C35** - Each of the eleven tables of `Shape`, and the linkage, has its one-sentence contract. (ABI-14, AC 2)
Proof: run documentation | test every_table_of_a_shape_has_its_contract ... ok

**C36** - `abi.md` is a pointer and nothing outside `.specs/features/` cites it. (ABI-15, AC 3)
Proof: run documentation | test no_file_cites_the_retired_abi_document ... ok

**C37** - Every `ABI §` citation names a section that exists. (ABI-15, AC 4)
Proof: run documentation | test every_cited_abi_section_exists ... ok

**C38** - `docs/abi/build.py` writes one page holding every chapter and annex of the index. (ABI-16, AC 5)
Proof: run documentation | test the_abi_specification_builds_to_one_page ... ok

**C39** - `mkdocs-spec.yml` lists every file of `docs/abi/`. (ABI-16, AC 6)
Proof: run documentation | test the_specification_site_lists_the_abi ... ok

**C40** - The page is published and linked from `docs/abi/index.md`, `docs/spec/index.md` and the Book's chapter. (ABI-16, AC 7)
Proof: walk - the published page opens; the language specification's index and the Book's chapter `abi` each link to it, and it links back

**C41** - The chapters `abi` and `device-authoring` are in Part 2, each in the order concepts, examples, structures, reference. (ABI-17, AC 1)
Proof: run book | test abi::the_chapters_follow_the_manual_order ... ok

**C42** - The chapter `abi` defines the nine kinds and the 46 entries of the four catalogues. (ABI-17, AC 2)
Proof: run book | test abi::every_kind_and_catalogue_entry_is_defined ... ok

**C43** - The chapter `abi` follows one device through load, bind, a DC solve, a transient step, a small-signal analysis and a host write. (ABI-17, AC 3)
Proof: run book | test abi::one_device_is_followed_through_every_phase ... ok

**C44** - No `rust` or `msl` block of the two chapters is `shown`, `pending` or `refused`. (ABI-18, AC 4)
Proof: run book | test abi::no_listing_of_the_abi_chapters_is_only_shown ... ok

**C45** - Every `rust` listing equals a marked region of a file the `book` target compiles. (ABI-18, AC 5)
Proof: run book | test device_authoring::every_rust_listing_is_a_region_of_a_compiled_file ... ok

**C46** - Each worked example gives the values its chapter states. (ABI-18, AC 6)
Proof: run book | test device_authoring::the_worked_examples_give_the_values_the_chapter_states ... ok

**C47** - The hand-written and the attribute form the chapter prints are one device: equal shapes, equal answers. (ABI-19, AC 7)
Proof: run book | test device_authoring::the_two_forms_of_a_device_are_one_device ... ok

**C48** - Neither chapter contains a phrase of the forbidden list. (ABI-20, AC 8)
Proof: run book | test abi::the_chapters_use_no_condescending_phrase ... ok

**C49** - `piperine-abi-macros` depends on `syn`, `quote` and `proc-macro2` only; `piperine-abi` has no dependency. (ABI-21, AC 1)
Proof: run authoring | test the_authoring_macros_depend_on_no_crate_of_the_tree ... ok
Proof: run authoring | test the_abi_crate_has_no_dependency ... ok

**C50** - No source and no printed code uses the attributes outside the allowed places. (ABI-21, AC 2)
Proof: run suite | test the_attributes_are_used_only_where_allowed ... ok

**C51** - `piperine-abi-macros` is the only crate that defines a macro. (ABI-21, AC 3)
Proof: run suite | test only_the_abi_authoring_crate_defines_a_macro ... ok

**C52** - The crate exports four attribute macros and accepts thirteen helper attributes, as annex E lists them. (ABI-22, AC 4)
Proof: run macros | test arguments::the_catalogue_is_four_macros_and_thirteen_helpers ... ok

**C53** - Each of the six twins has the shape of its hand-written form, entry by entry. (ABI-23, AC 5)
Proof: run twins | test every_twin_has_the_shape_of_its_hand_written_form ... ok

**C54** - Each twin, for the same read bytes, gives the writes, partials and commits of its hand-written form. (ABI-23, AC 6)
Proof: run twins | test every_twin_answers_as_its_hand_written_form ... ok

**C55** - An expansion holds only the author's items and inherent items on the author's type: no `unsafe`, `static`, trait implementation, new type, macro or foreign path. (ABI-24, AC 7)
Proof: run expansion | test an_expansion_holds_only_inherent_items_of_the_authors_type ... ok

**C56** - The expansion of each twin equals its golden file. (ABI-24, AC 8)
Proof: run expansion | test every_twin_expands_to_its_golden_file ... ok

**C57** - Each of the ten misuses stops the build with its message at the offending token. (ABI-25, AC 9)
Proof: run macros | test authoring_error::each_of_the_ten_misuses_stops_with_its_message_at_its_token ... ok

**C58** - An entry declared by attribute has the `file:line:column` of its declaration as origin. (ABI-26, AC 10)
Proof: run expansion | test an_entry_declared_by_attribute_has_the_origin_of_its_declaration ... ok

**C59** - An index of another table in an attribute is a type error at that expression. (ABI-27, AC 11)
Proof: run macrodoc | compile fail ... ok

**C60** - The guards pass and the feature added at most seven rows to the guards' lists: the four proc-macro entries and at most three reasoned `#[expect]` (T30, T40, T63). (ABI-28, AC 1)
Proof: `cargo test -p piperine-repository-guards`
Proof: `test "$(git diff device-abi-simplification-base HEAD -- crates/piperine-repository-guards/allowances | grep -c '^+[^+]')" -le 7`

**C61** - Both ABI crates build under `#![deny(missing_docs)]` with no `#[allow]`; every `#[expect]` of the tree is item-scoped, carries a reason and is listed by the guard. (ABI-28, AC 2)
Proof: run suite | test no_allow_hides_a_design_problem ... ok

**C62** - The decision is recorded with the user's words; the project's documents list the new crate. (ABI-29, AC 3)
Proof: run boundaries | test state_records_the_device_abi_decision ... ok
Proof: run boundaries | test instructions_list_the_kept_crates ... ok

**C63** - `GAPS.md` holds the row for `force` and `release`, with an owner. (ABI-29, AC 4)
Proof: run worklists | test the_unimplemented_capability_names_its_gap ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** none
**Walked:** none

---

## Phase Execution Map

```
Wave 0:  Phase 1
Wave 1:  Phase 3 (lane C)  ∥  Phase 2 (lane S)
Wave 2:  Phase 5 (lane C)  ∥  Phase 4 (lane S)
Wave 3:  Phase 7 (lane C)  ∥  Phase 6 (lane S)
Wave 4:  Phase 8 (lane C)  ∥  Phase 9 (lane S)
Wave 5:  Phase 10 (lane C) ∥  Phase 11 (lane S)
Wave 6:  Phase 12, then Phase 13
```

Within a lane execution is strictly sequential. Batches are whole phases; no phase is split across workers. Phase 3, 5, 7 and 8 are each one batch (tight chains); the others pack by the skill's budget.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: The baseline on the merged `main` | one inventory document and the corrections it implies | ⚠️ kept whole, with the reason |
| T11: The informative annexes A to D and F | five informative annexes, text only | ⚠️ kept whole, with the reason |
| T28: B1: the plan, the printer and the interpreter | three crates, one commit: the loader compares the printed shape with the plan's (proposal B1) | ⚠️ kept whole, with the reason |
| T47: C4: one binding vector per pin table | three crates, one commit: one type crosses them (proposal C4) | ⚠️ kept whole, with the reason |
| T53: The flip | one crate, one commit: the raw layout and everything in the crate that reads it | ⚠️ kept whole, with the reason |
| T80: Citations re-pointed: `piperine-abi`, `piperine-plan`, `piperine-ir`, `piperine-language`, `piperine-api`, `piperine-parity` | mechanical re-pointing over six crates | ⚠️ kept whole, with the reason |
| T82: Citations re-pointed: the three solver crates | mechanical re-pointing over three crates | ⚠️ kept whole, with the reason |
| T84: The project's documents | the project's documents, text only | ⚠️ kept whole, with the reason |
| every other task | one type, one expander, one table of the builder, one chapter section, one guard pair or one family of fixtures | ✅ Granular |

## Diagram-Definition Cross-Check

Every task depends on the task before it in its phase, and the first task of a phase on the tasks of earlier phases its body names. `validate_tasks.py` checks the arrows against the `Depends on` fields.

| Phase | First task depends on | Chain |
| ----- | --------------------- | ----- |
| 1 | None | T1 to T5, each on the one before |
| 2 | T2 | T6 to T12, each on the one before |
| 3 | T3 | T13 to T21, each on the one before |
| 4 | T4, T12, T21 | T22 to T27, each on the one before |
| 5 | T21 | T28 to T34, each on the one before |
| 6 | T27, T32 | T35 to T42, each on the one before |
| 7 | T34 | T43 to T49, each on the one before |
| 8 | T49, T10 | T50 to T58, each on the one before |
| 9 | T42 | T59 to T65, each on the one before |
| 10 | T58 | T66 to T71, each on the one before |
| 11 | T58, T65 | T72 to T77, each on the one before |
| 12 | T71, T77 | T78 to T83, each on the one before |
| 13 | T83 | T84 to T87, each on the one before |

## Test Co-location Validation

| Tasks | Code Layer | Matrix Requires | Task Says | Status |
| ----- | ---------- | --------------- | --------- | ------ |
| T13, T14, T15, T16, T17, T18, T19, T23, T24, T25, T26, T27, T29, T35, T36, T37, T38, T39, T43, T51, T52 | the contract, the authoring surface, the expanders | unit | unit | ✅ OK |
| T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T20, T21, T22, T28, T30, T31, T32, T33, T34, T40, T41, T42, T44, T45, T46, T47, T48, T49, T50, T53, T54, T55, T56, T57, T58, T59, T60, T61, T62, T63, T64, T65, T66, T67, T68, T69, T70, T71, T72, T73, T74, T75, T76, T77, T78, T79, T80, T81, T82, T83, T84, T85 | call path, migrations, bind checks, attributes in use, guards, the specification's text, the Book | integration | integration | ✅ OK |
| T1, T86, T87 | tracking files with no guard, publication, the final run | none | none | ✅ OK |

No task defers its tests: a guard lands in the task that creates what it guards (as a ratchet where later tasks extend it), and a refusal's test lands with the refusal.

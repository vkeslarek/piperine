# Code Clarity Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/code-clarity/design.md`
**Status**: Approved
**Approved**: 2026-11-25 - user: "Aprovo o code-clarity. Pode engatilhar ele para desenvolver em seguida das correçoes do nested-reach"
**Build**: `cargo build --workspace --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite`
**Run structure**: `cargo test -p piperine-repository-guards --test structure`
**Run boundaries**: `cargo test -p piperine-repository-guards --test boundaries`
**Run ownership**: `cargo test -p piperine-repository-guards --test ownership`
**Run naming**: `cargo test -p piperine-repository-guards --test naming`
**Run documentation**: `cargo test -p piperine-repository-guards --test documentation`
**Run suite**: `cargo test -p piperine-repository-guards --test suite`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`
**Run worklists**: `cargo test -p piperine-repository-guards --test worklists`
**Run refusals**: `cargo test -p piperine-language --test refusals`
**Run stddecl**: `cargo test -p piperine-language --test std_declarations`
**Run variadics**: `cargo test -p piperine-language --test named_variadics`
**Run specstructure**: `cargo test -p piperine-language --test spec_structure`
**Run srcio**: `cargo test -p piperine-parity --test source_io`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run golden**: `cargo test -p piperine-transpile --test golden`
**Run book**: `cargo test -p piperine --test book inventory_guard`

The `**Build**` line above is green at the base. Three tasks rewrite it in their own commit: T2 (no `--exclude`), T5 (`cargo test -p piperine-repository-guards --test suite` in place of the hygiene target), T139 (the crate's new name). The `Run` lines name the tree as it stands after T139 and T89; `check_proofs.py` reads them once, after T155.

Executors: Sonnet, except the tasks marked **Opus** (the state partitions, the grammar and solver splits with the most pinned behaviour, the language changes). Opus verifies the feature; an author never verifies their own work.

**Lanes.** design.md "Lanes" is the map: up to three lanes (T, F, K), each a worktree with its own `CARGO_TARGET_DIR` and `TMPDIR`, each owning the paths the table gives it for the wave. A phase merges into the integration branch when its last task is committed; a phase starts from the integration branch. Implementers can stop at any phase boundary: every guard is green there, because what is not done yet is an allowance row.

Rules every task follows (the user's rules, repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, the lane letter the coordinator gives. `/var/tmp/piperine-target` is a read-only trap and is never used. Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-23).** Per task: the `**Build**` line, `cargo test -p <crate>` for each crate the task touched, and what its `Gate` names. The full suite runs once, in T155. No task runs `cargo test --workspace` before it.
- **Behaviour does not change (CC-09).** Every twin golden, transpile golden, parity target and Book listing stays byte-identical, and no tolerance moves; `crates/piperine-parity/src/contract.rs` is not edited. Only what design.md "What may move" lists may move, under the task it names. A task that would move anything else stops and reports.
- **A refactor task edits no test.** Its proof is the touched crates' existing tests, unedited. The one exception is the call of a `pub` function that became a method, edited in the same commit and named in its body.
- **Tests keep their names.** A merge moves a test into a `mod` named for its former target; a removal has a `replace` or `retire` row in `.specs/features/code-clarity/repoints/T<n>.tsv`, applied with `cargo run -p piperine-repository-guards --bin repoint`. No assertion is weakened; the parity oracles are not reduced.
- **Closing a finding is deleting allowance rows.** A task deletes the rows of `crates/piperine-repository-guards/allowances/<crate>/` and `worklists/` it closed, and adds none.
- **The code follows the spec.** A language change (phases 15 to 17) amends `docs/spec/`, the Book, the corpora and the tests in the same commit, and adds the special case's id to the revision note of `docs/spec/index.md` (CC-11 AC 3).
- **Code clarity.** No free function, tests included; one purpose per type; full words; about 40 lines and 5 parameters is what a reviewer looks for, not a limit; a `///` on every new item; no macro; no `#[allow]`; no `unwrap()`/`expect()` on a path user input reaches.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge outside the integration branch.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Build and test", "Tests", "Code rules"), `AGENTS.md`, `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-28.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Repository guards (`piperine-repository-guards`) | integration | one test per rule of CC-01, CC-02, CC-03, CC-04, CC-05, CC-07, CC-08, CC-10, CC-12; each guard's own detector has a unit test of a violating and a clean source | `crates/piperine-repository-guards/tests/*.rs` | `cargo test -p piperine-repository-guards` |
| The scan and the ledger (`piperine-repository-guards/src`) | unit | each rule of findings.md Appendix G; each row kind of a repoint map | `crates/piperine-repository-guards/src/*.rs` | `cargo test -p piperine-repository-guards --lib` |
| A refactor of product code (B3, B5, B6, B9) | integration | the touched crates' existing targets, unedited; goldens where the Golden gate applies | `crates/<crate>/tests/*.rs` | `cargo test -p <crate>` |
| A fix of a user-input panic (T24, T26) | integration | one test per site that drives the input and reads the error | the crate's analyses or shape target | `cargo test -p <crate>` |
| A language change (B7, B8) | integration | one acceptance test per spec rule amended, at check, elaboration and run level; the records and generated Rust byte-identical | `crates/piperine-language/tests/{std_declarations,named_variadics}.rs`, `crates/piperine-parity/tests/source_io.rs`, `tests/book/` | `cargo test -p piperine-language --test std_declarations`, `--test named_variadics` |
| Test-suite surgery (B1, B2, B11) | integration | every proof and inventory row names an existing test; the parity oracles are a superset of the base | `crates/piperine-repository-guards/tests/proof_ledger.rs` | `cargo test -p piperine-repository-guards --test proof_ledger` |
| Doc comments, tracking files, `ARCHITECTURE.md` | none | - (build gate; the documentation and structure guards hold the result) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | every task that changes Rust | the `**Build**` line; `cargo test -p <crate>` for each crate whose `src/` or `tests/` the task changed; the `Run` lines its `Done when` names |
| Golden | a task on the path from MSL source to generated bytes, or one that renames a name generated code spells | Quick; `cargo test -p piperine-transpile --test golden`; `cargo test -p piperine-cli --test twins`; `cargo test -p piperine --test book_inventory` (`--test book inventory_guard` after T89) |
| Ledger | a task that removes, moves or merges a test, or renames a crate | Quick; `cargo test -p piperine-repository-guards --test proof_ledger` |
| Build | a task that changes no Rust | the `**Build**` line |
| Final | T155 only | `cargo test --workspace`; `cargo test -p piperine-parity -- --nocapture` with ngspice; `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`; the buck targets of `examples/buck/README.md`; `check_proofs.py code-clarity`; `check_proofs.py <feature>` for every feature with `## Checks` |

---

## Execution Plan

Phases are numbered so that every dependency points to a lower number. Phases of different lanes that share no dependency run at the same time (design.md "Lanes"); tasks within a phase execute in order.

| Wave | Lane T | Lane F | Lane K |
| ---- | ------ | ------ | ------ |
| 0 | | Phase 1 (single) | |
| 1 | Phase 2 | Phase 3 | Phase 4 |
| 2 | Phase 5 | Phases 6, 7 | Phase 8 |
| 3 | Phase 9 | Phase 10 | Phase 11 |
| 4 | Phase 12 | Phase 13 | |
| 5 | | Phase 14 (single), then Phases 15, 16 | Phase 17 (after Phase 14) |
| 6 | | Phase 18 (single), then Phase 19 | Phase 20 (after Phase 18) |
| 7 | | Phase 21 (single), then Phase 22 | Phase 23 (after Phase 21) |
| 8 | | Phases 24, 25 (single) | |

### Phase 1: B0 Ground: the language server leaves, the guards arrive

Runs in single lane.

```
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8
```

### Phase 2: B1 Lean tests I: prose pins, dead guards, uncited duplicates

Runs in lane T. Starts after T8.

```
T9 → T10 → T11 → T12 → T13 → T14
```

### Phase 3: B4 Refusals have an owner

Runs in lane F. Starts after T8.

```
T15 → T16 → T17 → T18 → T19 → T20 → T21 → T22
```

### Phase 4: B3 Small defects of structure, back end

Runs in lane K. Starts after T8.

```
T23 → T24 → T25 → T26 → T27 → T28 → T29 → T30 → T31 → T32
```

### Phase 5: B2a Device fixtures and the back-end merges

Runs in lane T. Starts after T14.

```
T33 → T34 → T35 → T36 → T37 → T38 → T39 → T40
```

### Phase 6: B6a Owners in `msl/`: lexer, printer, parser

Runs in lane F. Starts after T22.

```
T41 → T42 → T43 → T44 → T45 → T46
```

### Phase 7: B6a Owners in `pom/`, and the front end's small defects

Runs in lane F. Starts after T46.

```
T47 → T48 → T49 → T50 → T51 → T52
```

### Phase 8: B5a Owners in ir, plan, transpile, interp, project, api

Runs in lane K. Starts after T32.

```
T53 → T54 → T55 → T56 → T57 → T58 → T59 → T60 → T61 → T62
```

### Phase 9: B2b IR fixtures, one compile cache, more merges

Runs in lane T. Starts after T40, T62.

```
T63 → T64 → T65 → T66 → T67 → T68
```

### Phase 10: B6b `Checker` split, `types/` regrouped

Runs in lane F. Starts after T52.

```
T69 → T70 → T71 → T72 → T73 → T74 → T75 → T76
```

### Phase 11: B5b Owners in abi, analog-solver, digital-solver, solver

Runs in lane K. Starts after T40, T62.

```
T77 → T78 → T79 → T80 → T81 → T82 → T83 → T84 → T85
```

### Phase 12: B11a The remaining merges

Runs in lane T. Starts after T68.

```
T86 → T87 → T88 → T89 → T90 → T91
```

### Phase 13: B6c `Elaboration` split, `elab/` regrouped

Runs in lane F. Starts after T76.

```
T92 → T93 → T94 → T95 → T96 → T97 → T98 → T99
```

### Phase 14: CC-11 ground: how the compiler finds a `std` item

Runs in single lane. Starts after T91, T99.

```
T100 → T101 → T102
```

### Phase 15: B7 Declare what exists I: traits, core, carriers

Runs in lane F. Starts after T102.

```
T103 → T104 → T105 → T106 → T107 → T108 → T109
```

### Phase 16: B7 II with B8 (b), (c): `Range`, `Iterator`, `Option`, the method catalogue

Runs in lane F. Starts after T109.

```
T110 → T111 → T112 → T113 → T114
```

### Phase 17: B8 (a) Named variadic, `emit`, the reserved streams, `include_str`

Runs in lane K. Starts after T102.

```
T115 → T116 → T117 → T118 → T119 → T120 → T121 → T122 → T123
```

### Phase 18: B9 Names: glossary, one module layout, the IR

Runs in single lane. Starts after T85, T114, T123.

```
T124 → T125 → T126
```

### Phase 19: B9 Names, front end

Runs in lane F. Starts after T126.

```
T127 → T128 → T129 → T130 → T131
```

### Phase 20: B9 Names, back end

Runs in lane K. Starts after T126.

```
T132 → T133 → T134 → T135 → T136
```

### Phase 21: B9 Modules and crates

Runs in single lane. Starts after T131, T136.

```
T137 → T138 → T139 → T140
```

### Phase 22: B10 Words, front end

Runs in lane F. Starts after T140.

```
T141 → T142 → T143
```

### Phase 23: B10 Words, back end

Runs in lane K. Starts after T140.

```
T144 → T145 → T146 → T147
```

### Phase 24: B10 Structure words and the last guards

Runs in single lane. Starts after T143, T147.

```
T148 → T149 → T150 → T151
```

### Phase 25: B11 Close

Runs in single lane. Starts after T151.

```
T152 → T153 → T154 → T155
```

---

## Task Breakdown

### Phase 1 tasks: B0 Ground: the language server leaves, the guards arrive

### T1: Contract: the base, the decision, the numbers

**What**: Check that `main` holds `generics` phase E and that no other feature is landing code (spec Assumption 2); append the next free `MD-n` to `.specs/STATE.md` with the standing rules design.md "Project-level decision" lists; record on the Handoff the base commit and that design.md's five questions are answered (spec Assumptions 13 to 17).
**Where**: `.specs/STATE.md`
**Depends on**: None
**Reuses**: design.md "Tech Decisions"
**Requirement**: CC-09
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] The `MD-n` entry exists and names: the guards crate and its allowances, test support as dev-only crates, one module layout, `LanguageItems`, `Refusal`, MD-28 amended
- [x] The Handoff names the base commit and the five answers

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): code-clarity decision and base`

---

### T2: The language server and the Verilog-AMS extension leave the tree

**What**: Delete `crates/piperine-lang-server` and `editors/vscode` (Q1); remove the crate from `members`; update `CLAUDE.md`, `AGENTS.md`, `README.md`, `GAPS.md` (B3, H13) and `ROADMAP.md` (the `lang-server` feature rebuilds on the POM's reflection); rewrite this file's `**Build**` line to `cargo build --workspace --all-targets …` with no `--exclude`.
**Where**: `Cargo.toml`
**Depends on**: T1
**Reuses**: findings.md §4.3
**Requirement**: CC-03
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] `cargo build --workspace --all-targets` prints no warning and no error
- [x] No file of the tree names `piperine-lang-server` or `editors/vscode` outside `.specs/` and `ROADMAP.md`'s `lang-server` row
- [x] `CLAUDE.md` no longer says "never `--workspace` bare"
- [x] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `chore: remove the language server and the vscode extension`

---

### T3: The guards crate and the scan

**What**: Create the test-only crate `piperine-repository-guards` with `SourceTree` and `ItemScan` (design.md Components): one `ScannedItem` per file, module, type, field, variant, `impl`, method, free `fn`, nested `fn`, lint attribute and `unwrap`/`expect` call, with file, line, length, doc flag and product-or-test, following `#[cfg(test)] mod x;` and `#[path]` declarations to the file they reach.
**Where**: `crates/piperine-repository-guards/src/item_scan.rs`
**Depends on**: T2
**Reuses**: findings.md Appendix G
**Requirement**: CC-04
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] Unit tests over source strings hold each rule of findings.md Appendix G: a free `fn`, a nested `fn`, `main`, an `extern "C"` function, a `#[test]`, a method of a trait `impl`, a `#[cfg(test)]` inline module, a `#[cfg(test)] mod x;` file, a `#[path]` include, a closure of 15 lines, a documented and an undocumented item
- [x] The crate has no in-tree dependency and `#![deny(missing_docs)]`
- [x] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `test(guards): the repository scan as a test-only crate`

---

### T4: `ProofLedger`, the `repoint` binary and the ledger guard

**What**: Add `ProofLedger` (design.md Components): it reads every `Run` line, lone `Proof:` command and proof row of `.specs/features/*/{tasks,checks}*.md` and every `covered_by` of `tests/book/inventory.tsv`, applies a map of `move`, `rename-crate`, `replace` and `retire` rows, and is driven by `cargo run -p piperine-repository-guards --bin repoint -- <map.tsv>`; write `data/parity_oracles.tsv` from the base; add the target `proof_ledger`. Rows that are dangling at the base go to `allowances/proof_ledger.tsv` with their feature.
**Where**: `crates/piperine-repository-guards/src/proof_ledger.rs`
**Depends on**: T3
**Reuses**: the skill's `check_proofs.py` matching rule (design.md fact 1)
**Requirement**: CC-12 (AC 5)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] `every_proof_names_a_test_that_exists`, `every_inventory_row_names_a_test_that_exists`, `every_retired_check_is_recorded` and `the_parity_oracles_are_not_reduced` pass under `**Run ledger**`
- [x] Unit tests apply one row of each kind to a fixture feature folder and compare the rewritten files byte for byte
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(guards): the proof ledger and its repoint binary`

---

### T5: The workspace guards move to the guards crate

**What**: Move `crates/piperine-solver/tests/{workspace_shape,crate_boundaries}.rs` and `crates/piperine-lang/tests/suite_hygiene.rs` into the guards crate as the targets `boundaries` and `suite`, their free helpers as methods of `SourceTree` (M7); re-point with `repoints/T5.tsv`; rewrite this file's `**Build**` line to run `cargo test -p piperine-repository-guards --test suite`.
**Where**: `crates/piperine-repository-guards/tests/suite.rs`
**Depends on**: T4
**Reuses**: the three moved targets
**Requirement**: CC-03
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] `the_dependency_graph_is_the_documented_one` passes under `**Run boundaries**` and encodes the arrows of `CLAUDE.md` "Pipeline"
- [x] Every moved test keeps its name and passes; the hygiene walk now reaches `tests/support/` (the file-scope suppression of `transpile/tests/support/compiled.rs:4` is fixed, not exempted)
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(guards): workspace guards live in the guards crate`

---

### T6: Ratchet guards: owners and structure

**What**: Add `Allowance` and the targets `ownership` and `structure`, each test reading one allowance file per crate seeded by the scan: free functions, free helpers in test code, inherent `impl` blocks away from their type, `#[allow]` attributes, the owner table of design.md (`data/owners.tsv`), crate docs, module names.
**Where**: `crates/piperine-repository-guards/tests/ownership.rs`
**Depends on**: T5
**Reuses**: T3's scan
**Requirement**: CC-04, CC-02, CC-01
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] `no_free_function_outside_the_language_exceptions`, `no_free_helper_function_in_test_code`, `an_inherent_impl_lives_with_its_type`, `every_owner_of_the_design_exists_and_has_its_methods` and `no_allow_hides_a_design_problem` pass under `**Run ownership**`
- [x] `every_crate_states_its_purpose_and_module_map` and `no_module_is_named_for_no_concern` pass under `**Run structure**`
- [x] Each fails on a violation its allowance does not list and on a listed row that no longer exists (a unit test of `Allowance` proves both)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): ratchet guards for owners and structure`

---

### T7: Ratchet guards: names and doc comments

**What**: Add `Glossary` and `Dictionary` (`data/glossary.tsv` from findings.md §1.3 (a) with `eval`, `spec` and `param`, Q2) and the targets `naming` and `documentation`, with per-crate allowances seeded by the scan.
**Where**: `crates/piperine-repository-guards/tests/naming.rs`
**Depends on**: T6
**Reuses**: T6's `Allowance`
**Requirement**: CC-05, CC-10
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] `every_declared_name_is_words_or_glossary` and `no_bare_type_name_is_declared_twice` pass under `**Run naming**`
- [x] `every_item_is_documented` and `every_crate_denies_missing_docs` pass under `**Run documentation**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): ratchet guards for names and doc comments`

---

### T8: The numbers before: the suite report and the worklists

**What**: Add `SuiteCensus` and `Worklist`; write `.specs/features/code-clarity/suite-report.md` section `## Before` (targets and tests per crate from the census; run seconds per crate, one serial pass with ngspice, the before values taken from findings.md §3.1 at the inventory base, 2 202 s in all; no build time); seed `worklists/{long_functions,single_implementor_traits,test_only_items,duplicate_tests}.tsv` from the scan with no verdict yet (allowance: all rows open); add the targets `worklists` and the test `no_crate_has_more_targets_than_its_plan` (the plan starts at today's counts).
**Where**: `crates/piperine-repository-guards/tests/worklists.rs`
**Depends on**: T7
**Reuses**: findings.md §3.1, Appendix B
**Requirement**: CC-12 (AC 1, AC 6)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [x] `suite-report.md` `## Before` holds the census columns re-taken at this commit and the `Run s` column from findings.md §3.1 (no build time column)
- [x] `every_long_function_of_the_inventory_has_a_verdict`, `every_single_implementor_trait_is_removed_or_justified`, `every_test_only_item_has_a_verdict` and `every_duplicate_of_the_inventory_has_a_verdict` pass under `**Run worklists**` (open rows are allowance rows)
- [x] `no_crate_has_more_targets_than_its_plan` passes under `**Run suite**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): the suite census, the worklists and the before report`

---

### Phase 2 tasks: B1 Lean tests I: prose pins, dead guards, uncited duplicates

### T9: Guards against a retired architecture leave

**What**: Remove `crates/piperine-solver/tests/removed_surface.rs` (12 tests), the rows of `workspace_shape` about retired crates (7) and the hygiene tests `phdl_corpus_does_not_grow`, `lrm_covers_declarations_are_well_formed`, `no_dead_architecture_identifiers` with their helpers (P3); `repoints/T9.tsv` retires each cited row.
**Where**: `crates/piperine-repository-guards/tests/suite.rs`
**Depends on**: T8
**Reuses**: findings.md §3.4 P3
**Requirement**: CC-12 (AC 2)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] No test is named for a retired crate, PHDL or the Verilog-AMS front end
- [x] `every_retired_check_is_recorded` passes under `**Run ledger**`
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test: remove guards against a retired architecture`

---

### T10: Tracking-file pins leave `docs.rs`

**What**: Remove from `crates/piperine-cli/tests/docs.rs` the tests that pin a sentence of `GAPS.md`, `ROADMAP.md`, `.specs/STATE.md` or `CLAUDE.md` (51 of 60, P1); move the structural ones (`gaps_section_10_lists_exactly_the_owned_codes`, `the_roadmap_is_forward_looking`, the buck README's command lines) to the guards target `suite`; `repoints/T10.tsv` retires or moves each cited row; `GAPS.md` drops the note that its numbering is frozen for this target.
**Where**: `crates/piperine-cli/tests/docs.rs`
**Depends on**: T9
**Reuses**: findings.md §3.4 P1; Q13
**Requirement**: CC-12 (AC 2, AC 5)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-cli/tests/docs.rs` is gone; the kept tests pass under `**Run suite**` with their names
- [x] Each removed test has a `retired.tsv` row naming the feature and check that cited it
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test: remove pins on the wording of tracking files`

---

### T11: Wording pins leave `spec_text.rs`

**What**: Remove from `crates/piperine-lang/tests/spec_text.rs` the tests that assert the wording of `docs/spec/*.md` (P2); keep the structural guards (every error code defined once, every method called in a fence is catalogued, the construct count, the file map) as the target `spec_structure`; `repoints/T11.tsv` retires or moves each cited row.
**Where**: `crates/piperine-lang/tests/spec_structure.rs`
**Depends on**: T10
**Reuses**: findings.md §3.4 P2
**Requirement**: CC-12 (AC 2, AC 5)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] `spec_text.rs` is gone; `spec_structure` opens with a `//!` header and its tests keep their names
- [x] No remaining test asserts `contains(..)` on a sentence of the spec (reviewed in the commit body: the list of kept tests and why each is structural)
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(lang): keep the spec's structural guards, drop wording pins`

---

### T12: Uncited tests of the solver crates

**What**: For each `#[test]` of `piperine-solver`, `piperine-analog-solver` and `piperine-digital-solver` that no proof, task, inventory row or Book claim cites (126, findings.md §3.6): remove it when another test asserts the same rule on the same input (name the survivor in `worklists/duplicate_tests.tsv`), keep it otherwise with verdict `kept: <the rule it alone proves>`.
**Where**: `crates/piperine-repository-guards/worklists/duplicate_tests.tsv`
**Depends on**: T11
**Reuses**: findings.md §3.6
**Requirement**: CC-12 (AC 2)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] Every uncited test of the three crates has a verdict
- [x] No removed test was the only assertion of its rule (the commit body lists each removal with its survivor)
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(solver): remove uncited tests that duplicate another`

---

### T13: Uncited tests of the other crates

**What**: The same for the 207 uncited tests of lang, parity, api, transpile, root, interp, ir, cli, abi and plan.
**Where**: `crates/piperine-repository-guards/worklists/duplicate_tests.tsv`
**Depends on**: T12
**Reuses**: findings.md §3.6
**Requirement**: CC-12 (AC 2)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] Every uncited test of these crates has a verdict
- [x] The commit body lists each removal with its survivor
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test: remove uncited tests that duplicate another`

---

### T14: Test names defined in more than one target

**What**: Give each of the 57 names of findings.md Appendix E (140 definitions) a verdict: `removed` with a `replace` row in `repoints/T14.tsv` when two definitions assert the same thing, `kept: <what differs>` otherwise. The parity oracles and `tests/scenarios.rs` are `kept` (Q9).
**Where**: `crates/piperine-repository-guards/worklists/duplicate_tests.tsv`
**Depends on**: T13
**Reuses**: findings.md §3.3 T5, Appendix E
**Requirement**: CC-12 (AC 2, AC 5)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] `every_duplicate_of_the_inventory_has_a_verdict` passes under `**Run worklists**` with no open row
- [x] `the_parity_oracles_are_not_reduced` passes under `**Run ledger**`
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test: one definition per duplicated test name`

---

### Phase 3 tasks: B4 Refusals have an owner

### T15: `Refusal`: one place words `P0001`

**What**: Add `Refusal`, `RefusedForm`, `RefusalRow`, `RefusalOwner` (design.md Components) with `RefusedForm` holding one variant, `Text`, that carries today's free text; `Checker::unsupported` and `Elaboration::unsupported` build a `Refusal` and report `Refusal::diagnostic()`; `ConstFault::Unsupported` becomes `ConstFault::Refused(Refusal)` and `elab/diagnose.rs` places it. No message changes.
**Where**: `crates/piperine-lang/src/refusal.rs`
**Depends on**: T8
**Reuses**: `elab/unlowered.rs`
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `p0001_is_worded_in_one_place` passes under `**Run refusals**`: the literal `"P0001"` is in one file of `src/`
- [x] `cargo test -p piperine-lang` passes with no existing test edited
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(lang): one owner words every unsupported form`

---

### T16: `P0001` no longer names a milestone

**What**: Change the label of `Refusal::diagnostic` from "… is outside the M1 subset" to "not supported yet" (Q12); `GAPS.md` §4.8 drops its row; the comments that say "the M1 subset" in `src/` say what the code does.
**Where**: `crates/piperine-lang/src/refusal.rs`
**Depends on**: T15
**Reuses**: spec Assumption 8
**Requirement**: CC-09, CC-10 (AC 4)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `p0001_does_not_name_a_milestone` passes under `**Run refusals**`
- [x] No non-test file says "M1 subset"
- [x] A test that pinned the old label is re-pinned exactly, named in the commit body
- [x] Gate check passes: Quick, and `cargo test -p piperine-cli -p piperine-api`

**Tests**: integration
**Gate**: quick

**Commit**: `fix(lang): drop the milestone name from the unsupported-form text`

---

### T17: The checker's refused forms are named

**What**: Each of the 98 refusal sites of `types/` raises a `RefusedForm` variant of its family in place of text; the row's `text` is the message's bytes.
**Where**: `crates/piperine-lang/src/types`
**Depends on**: T16
**Reuses**: T15
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] No `Refusal` of `types/` is built from `RefusedForm::Text`
- [x] `cargo test -p piperine-lang` passes with no existing test edited
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(types): name each refused form`

---

### T18: The value and function lowering's refused forms are named

**What**: The same for the refusal sites of `elab/` that lower values, patterns, aggregates, iterators, closures, methods and functions.
**Where**: `crates/piperine-lang/src/elab`
**Depends on**: T17
**Reuses**: T15
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] No `Refusal` raised by these files is built from `RefusedForm::Text` (the commit body lists the files)
- [x] `cargo test -p piperine-lang` passes with no existing test edited
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(elab): name the refused value forms`

---

### T19: The analog, digital and structure lowering's refused forms are named

**What**: The same for the remaining refusal sites of `elab/` outside `consts/`.
**Where**: `crates/piperine-lang/src/elab`
**Depends on**: T18
**Reuses**: T15
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] No file of `elab/` outside `consts/` builds a `Refusal` from `RefusedForm::Text`
- [x] `cargo test -p piperine-lang` passes with no existing test edited
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(elab): name the refused hardware forms`

---

### T20: The constant evaluator's refused forms are named

**What**: The same for the 120 `ConstFault::Refused` sites of `elab/consts*`; `RefusedForm::Text` is deleted.
**Where**: `crates/piperine-lang/src/elab/consts`
**Depends on**: T19
**Reuses**: T15
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `RefusedForm` has no `Text` variant
- [x] `cargo test -p piperine-lang` passes with no existing test edited
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(elab): name the refused constant forms`

---

### T21: One method per repeated refusal

**What**: Each of the 25 messages raised from more than one site (68 sites, R3) is one named variant whose words live in one row of the refusal table, and each site builds that variant directly (coordinator decision, user AFK, pending review: the first attempt's one pass-through method per form, `refusal/repeated.rs`, was removed as indirection); the elaborator's copy of the two forms the checker refuses first (R4: `expect` with a non-literal message, a name as a pattern) becomes an internal guard `P0004`; `types/unsupported.rs` and `elab/unlowered.rs` fold into the table (R7).
**Where**: `crates/piperine-lang/src/refusal/forms.rs`
**Depends on**: T20
**Reuses**: findings.md §6 R3, R4, R7
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] A form's words are defined in exactly one row and each repeated form is one named variant, not one message per site: `each_form_words_live_in_one_row` passes under `**Run refusals**` (coordinator decision while the user is AFK, pending review: the per-form pass-through methods of the first attempt were removed as indirection)
- [x] `types/unsupported.rs` and `elab/unlowered.rs` are gone: `the_catch_all_refusal_files_are_gone` passes under `**Run refusals**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(lang): one method raises each repeated refusal`

---

### T22: The refusal table and its guard

**What**: Every `RefusedForm` has a `RefusalRow` with its owner: `Gap(id)` where a site already cites a `GAPS.md` row, `Unswept` otherwise; write `tests/support/unswept_count.txt`; `ROADMAP.md`'s `spec-closure` row gains "fills the refusal table (`refusal/forms.rs`), ends with no `Unswept`".
**Where**: `crates/piperine-lang/tests/refusals.rs`
**Depends on**: T21
**Reuses**: `tests/support/code_owners.tsv`
**Requirement**: CC-07
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `every_refused_form_has_a_row`, `every_gap_owner_names_a_gaps_row` and `the_unswept_count_only_shrinks` pass under `**Run refusals**`
- [x] `ROADMAP.md` hands the sweep to `spec-closure`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): the refusal table has an owner per form`

---

### Phase 4 tasks: B3 Small defects of structure, back end

### T23: A circuit of devices always has its binding

**What**: Replace `CircuitInstance.abi: Option<Bound>` by the type that cannot be unbound (the empty circuit of `CircuitInstance::new` becomes its own constructor's type or holds an empty `Bound`), removing the 28 `.expect("a circuit of devices…")` (A2).
**Where**: `crates/piperine-solver/src/core/circuit.rs`
**Depends on**: T8
**Reuses**: findings.md §1.6 A2
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] No `expect` on the binding remains in `analyses/` or `core/circuit.rs`
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(solver): a circuit of devices cannot be unbound`

**Notes**: one call changed in a test, because `CircuitInstance::from_netlist` (`pub(crate)`) now takes the binding: `sp.rs` `zero_ports_fails_loud` builds its empty circuit through `CircuitBuilder::new(..).build()`; its assertion is unchanged.

---

### T24: A NaN pole and an empty shoot are errors, not panics

**What**: `pz.rs:250` (`partial_cmp(..).unwrap()`) and `pss.rs:234,236,278,369` (`.last().unwrap()`) return the solver's `Error` with its `SolverDomain`.
**Where**: `crates/piperine-solver/src/analyses`
**Depends on**: T23
**Reuses**: findings.md §1.9
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] A test per site drives the input that panicked and reads the error (two new tests in the crate's analyses target: `a_matrix_with_an_infinite_entry_is_refused` in `pz.rs`, `a_result_with_no_step_has_no_final_step_error` in `tests/pss.rs` (the reader's claim: a shot always records its first step, so the shooting cannot be driven to it))
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(solver): return an error for a NaN pole and an empty shoot`

---

### T25: The analog solver's invariants are types or errors

**What**: Each of the 23 `unwrap`/`expect` of `piperine-analog-solver` (`math/newton_raphson.rs`, `netlist.rs`, `tolerances.rs`, `device/{plan,assembly,flow,watch}.rs`) becomes the type that makes the case impossible, or the crate's `Error`.
**Where**: `crates/piperine-analog-solver/src`
**Depends on**: T24
**Reuses**: findings.md Appendix D
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] `grep -rE 'unwrap\(\)|expect\(' crates/piperine-analog-solver/src` finds nothing outside `#[cfg(test)]`
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(analog-solver): no unwrap on a validated shape`

---

### T26: An `Ir` built by a host is refused, not panicked on

**What**: The 9 `panic!("…; check() refuses it")` of `piperine-plan`, the 2 of `piperine-transpile` and the 7 out-of-range panics of `ir/src/build.rs` return the crate's error; `ir::Checker::expect` is renamed (it reads as a panic and is not one).
**Where**: `crates/piperine-plan/src`
**Depends on**: T25
**Reuses**: findings.md §1.9
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] A test per crate builds the offending `Ir` through `IrBuilder` and reads the error: `an_own_terminal_on_a_branch_is_a_plan_error_not_a_panic` (`piperine-plan`, `tests/structure_shape.rs`), `a_probe_on_an_own_terminal_has_no_text_not_a_panic` (`piperine-transpile`, `print/analog.rs`) and `a_branch_on_an_own_terminal_is_labelled_not_a_panic` (`print/contribution.rs`); a checked `Ir` cannot reach those two print sites, so the tests hand the printer the terminal
- [x] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

- [ ] (6 of the 7 done in T55; the 7th, `IrBuilder::body_data`, is owned by T96 — coordinator decision, user AFK, pending review) the out-of-range panics of `ir/src/build.rs` return the builder's error

**Commit**: `fix(plan): refuse an unchecked ir with an error`

**Deferred (partly, to T55)**: the 7 out-of-range panics of `ir/src/build.rs` stay: `resolve_net`, `bind_nets`, `bind_reached_node`, `bind_reached_net` have 5 callers in `piperine-lang/src/elab/` (lane F's paths) and about 40 in tests; changing them to `Result` needs both. They are a builder-misuse panic (a handle of another builder), not a user-input path. Reported to the coordinator.

---

### T27: A trace sink replaces the debug prints

**What**: The `eprintln!` behind trace flags (`analog-solver/src/convergence.rs:543,569,625`, `solver/src/analyses/transient.rs:1756`) write to a `Trace` sink owned by `Policy` (A8); the unit test that greps its own source (`solver/src/analyses/tf.rs:455-500`, P4) leaves with a `retire` row.
**Where**: `crates/piperine-analog-solver/src/convergence.rs`
**Depends on**: T26
**Reuses**: findings.md §1.6 A8
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] No `eprintln!` in `src/` of the two crates; the trace a flag printed is the same bytes through the sink (one test: `a_trace_line_reaches_the_sink_with_the_bytes_the_flag_printed`)
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `refactor(solver): a trace sink owned by the policy`

---

### T28: `lib.rs` declares, it does not define

**What**: Move `Interpreter` and `Rejected` out of `crates/piperine-interp/src/lib.rs` (238 lines) and `Transpiler` out of `crates/piperine-transpile/src/lib.rs` (152) into modules named for them; each `lib.rs` keeps its doc, `mod` and `pub use` (A9).
**Where**: `crates/piperine-transpile/src/lib.rs`
**Depends on**: T27
**Reuses**: findings.md §1.6 A9
**Requirement**: CC-02
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] Both `lib.rs` hold no `struct`, `enum`, `impl` or `fn`; every public path is unchanged
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(transpile): lib.rs holds the map, modules hold the code`

---

### T29: One owner per rule written twice

**What**: The fatal-record rule lives in `piperine-api`'s `Verdict` and `cli/src/commands/run.rs:79` reads it; `Backend` is declared once (`piperine-api`) and `piperine-parity` uses it; the ground name is one `GroundName` of `piperine-solver` that `api/src/noise.rs:69` uses, and it accepts `gnd` only (S21, deferred to the user, see Deferred below: stop and report if a corpus, example or test uses `GND`, `vss` or `VSS` as ground). Adds `no_type_of_the_inventory_is_defined_in_two_crates` over the names of findings.md A4 and A6.
**Where**: `crates/piperine-api/src/verdict.rs`
**Depends on**: T28
**Reuses**: findings.md §1.6 A4, §2.2 S21
**Requirement**: CC-03
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] `no_type_of_the_inventory_is_defined_in_two_crates` passes under `**Run boundaries**` (the rows T51 and T52 close are allowance rows)
- [x] Gate check passes: Quick, and `cargo test -p piperine-cli -p piperine-parity`

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(api): one owner for the verdict, backend and ground rules`

**Deferred**: S21 (the `GND`, `vss` and `VSS` aliases) is deferred to the user: the ground net's public label is `GND` (`core/net.rs`), so removing the alias changes host behaviour. `GroundName` accepts the four names again and `edge_gnd_name_routes_to_ground` keeps its original assertion. (Coordinator decision, user AFK, pending review.)

**S21 decided by the user, 2026-11-27**: "Sim... um gnd só!". The ground reference stays (the solver needs the circuit's zero) and is spelled `gnd` only: the ground net's printed label becomes `gnd`, and `GND`, `vss`, `VSS` stop naming ground and are refused with an error that names the one spelling (never silently a new node). Owned by T83.

---

### T30: Dead items leave; test-only items say whom they serve

**What**: Remove the 12 never-referenced `pub` items of findings.md §4.1; give each of the 78 test-only `pub` items of §4.2 that sits in the back-end crates a verdict in `worklists/test_only_items.tsv`: `kept: <the host it serves>` with a doc comment saying so, or `removed` with its test (a `retire` or `replace` row).
**Where**: `crates/piperine-repository-guards/worklists/test_only_items.tsv`
**Depends on**: T29
**Reuses**: findings.md §4.1, §4.2
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The 8 back-end items of the 12 are gone and the workspace builds
- [x] The other 4 (`round_integer`, `GenericParam::default_type`, `Ty::has_repr`, `DeferReason::InferredGenerics`) are removed by T51, T50, T49 and T48 (their Done-when lines)
- [x] Every back-end row of the worklist has a verdict
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `refactor: remove dead public items`

---

### T31: Recorded deviations are closed

**What**: For each of the 9 `SPEC_DEVIATION` comments (M5: `solver/src/analyses/adapters.rs:8`, `analog-solver/src/device/assembly.rs:222,393,415,477`, `solver/src/analyses/transient.rs:1433,1564`, and the two of `lang/src/pom` handed to T49): move the code to where the feature's design put it, or amend that design's text with the reason; delete the comment.
**Where**: `crates/piperine-solver/src/analyses/adapters.rs`
**Depends on**: T30
**Reuses**: findings.md §1.10 M5
**Requirement**: CC-02
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] No `SPEC_DEVIATION` in the back-end crates; each closure is named in the commit body with the design file it touched
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(solver): close the recorded structure deviations`

---

### T32: A trait with one implementor earns its place or leaves

**What**: Give each of the 13 traits of findings.md A3 a verdict in `worklists/single_implementor_traits.tsv`: remove the linear-algebra layer over `faer` (`LinearSystem`, `SymbolicMatrix`, `SymbolicLinearSystem`, `FaerToNdarray`), `InitialValueApplyExt`, `Limbs`, `ElementSource`, `Factory`, `NetRead`, `StepperStrategy` and `HomotopyDriver` where the trait simplifies nothing; keep `DiscreteSide` (the protocol's seam) and the parser's `In` alias with the reason written on the trait.
**Where**: `crates/piperine-repository-guards/worklists/single_implementor_traits.tsv`
**Depends on**: T31
**Reuses**: findings.md §1.6 A3
**Requirement**: CC-07
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] `every_single_implementor_trait_is_removed_or_justified` passes under `**Run worklists**`
- [x] no open row: `lang::In` (T43) and `numbers::Limbs` (T51) are closed by their Done-when lines
- [x] Gate check passes: Quick for abi, numbers, analog-solver and solver

**Tests**: integration
**Gate**: quick

**Commit**: `refactor: remove traits with one implementor and no use`

**Deferred**: rows `lang::In` and `numbers::Limbs` stay open until T43 and T51 (outside lane K's paths). `LinearSystem`, `SymbolicMatrix` and `SymbolicLinearSystem` are removed (fix round 1: `NewtonRaphsonSolver` has no backend parameter; its methods are inherent on `FaerSparseLinearSystem` and `FaerSymbolicMatrix`; test edits: the import at `tests/support/system.rs`, the `Solver` alias there, three ascriptions in `faer.rs`'s unit tests). `NetRead` is kept: `DiscreteSide` has an associated `Mark` and is not object safe.

---

### Phase 5 tasks: B2a Device fixtures and the back-end merges

### T33: `Device::react` has a default

**What**: Give `Device::react` the default body `Ok(Flow::Continue)` (A1, Q8); the `#[repr(C)]` contract and the thunks are unchanged.
**Where**: `crates/piperine-abi/src/device.rs`
**Depends on**: T14
**Reuses**: findings.md §1.6 A1
**Requirement**: CC-07
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] A test implements `Device` with `eval` only and its `react` thunk returns `Continue`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): a default react for a device with no reaction`

---

### T34: The device fixtures crate

**What**: Create the dev-only crate `piperine-device-fixtures`: the hand-written ABI devices as types whose methods are today's helpers, from `abi/tests/support/examples.rs`, `analog-solver/tests/support/devices.rs`, `digital-solver/tests/support/devices.rs`, `solver/tests/helpers/{abi_devices,abi_mixed,partials}.rs` and `solver/src/analyses/test_devices.rs` (M6: `R`, `C`, `L` become `Resistor`, `Capacitor`, `Inductor`), each device once, analog-only ones with no `react`. Adds `no_test_support_is_shared_by_path` and `no_allow_in_test_code` to `suite` (ratchets).
**Where**: `crates/piperine-device-fixtures/src/lib.rs`
**Depends on**: T33
**Reuses**: root `tests/` fixture types
**Requirement**: CC-12 (AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] The crate builds with `#![deny(missing_docs)]`, no free function and one `PartialsCheck` over every device (`its_devices_pass_the_partials_verifier`, once)
- [x] `no_test_support_is_shared_by_path` and `no_allow_in_test_code` pass under `**Run suite**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test: hand-written devices as one fixtures crate`

---

### T35: `piperine-abi` tests: 11 targets become 2

**What**: Merge into `contract` and `thunks`, one `mod` per former target, on the fixtures crate; `repoints/T35.tsv`.
**Where**: `crates/piperine-abi/tests`
**Depends on**: T34
**Reuses**: T34
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 2 targets, each with a `//!` header; every test keeps its name; no `#[path]`, no `#[allow]`, no free helper
- [x] `no_crate_has_more_targets_than_its_plan` passes with the plan lowered to 2
- [x] Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(abi): two targets on the device fixtures`

---

### T36: `piperine-analog-solver` tests: 13 targets become 3

**What**: Merge into `assembly`, `numerics`, `boundary` on the fixtures crate; `repoints/T36.tsv`.
**Where**: `crates/piperine-analog-solver/tests`
**Depends on**: T35
**Reuses**: T34
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 3 targets; every test keeps its name; `tests/support/devices.rs` is gone
- [x] Plan lowered to 3; Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(analog-solver): three targets on the device fixtures`

---

### T37: `piperine-digital-solver` tests: 16 targets become 3

**What**: Merge into `engine`, `plan`, `boundary` on the fixtures crate; `repoints/T37.tsv`.
**Where**: `crates/piperine-digital-solver/tests`
**Depends on**: T36
**Reuses**: T34
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 3 targets; every test keeps its name; `tests/support/devices.rs` is gone
- [x] Plan lowered to 3; Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(digital-solver): three targets on the device fixtures`

---

### T38: `piperine-solver` tests I: protocol, devices, analyses

**What**: Merge the `protocol_*` targets into `protocol`, the `abi_*` into `abi_devices`, and the small-signal and periodic ones (ac, noise, pz, pss, sens, adjoint, tf) into `analyses`, on the fixtures crate; the 27 copies of `its_devices_pass_the_partials_verifier` leave with `replace` rows to the one of T34; `repoints/T38.tsv`.
**Where**: `crates/piperine-solver/tests`
**Depends on**: T37
**Reuses**: T34
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] The three targets pass with `--test-threads=1` and with the default
- [x] Every other test keeps its name; Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(solver): protocol, device and analysis targets merged`

---

### T39: `piperine-solver` tests II: transient, host

**What**: Merge the transient targets (reentry, truncation, checkpoint, landing, jump, crossing, stepper) into `transient` and the host ones (`host_*`, `live_params`, `probe_selection`, session) into `host`; `parity_baseline` stays; `tests/helpers/` is gone; `repoints/T39.tsv`.
**Where**: `crates/piperine-solver/tests`
**Depends on**: T38
**Reuses**: T34
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 7 targets in the crate (`context_init` stays its own: its header requires a fresh process); both merged targets pass with `--test-threads=1` and with the default
- [x] Plan lowered to 7 (6 plus `context_init`); Gate check passes: Ledger
- Coordinator amendment (user AFK, pending review): the approved text said 6 targets; `context_init` needs a fresh process, so the target count is 7.

**Tests**: integration
**Gate**: ledger

**Commit**: `test(solver): transient and host targets merged`

---

### T40: The four back-end crates' tests are clean

**What**: Close the allowance rows of abi, analog-solver, digital-solver and solver for `no_free_helper_function_in_test_code`, `no_test_support_is_shared_by_path` and `no_allow_in_test_code`: a remaining helper becomes a method of a fixture type.
**Where**: `crates/piperine-repository-guards/allowances`
**Depends on**: T39
**Reuses**: T34
**Requirement**: CC-12 (AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] The four crates have no row in the three allowances. Closed in fix round 1 of phase 5: the 36 `test_helper_functions` rows of `#[cfg(test)]` helpers in back-end `src/` (abi `raw.rs` 1, analog-solver 6, solver 29) are gone, each helper being a method of a `Cases` struct of its test module; solver's `analyses/test_devices.rs` now only places the fixtures crate's devices (coordinator decision, user AFK, pending review: lane T edited those `src/` helpers for this round, which breaks the circular wait with lane K)
- [x] Gate check passes: Quick for the four crates

**Tests**: integration
**Gate**: quick

**Commit**: `test: no free helper or path include in the solver crates`

---

### Phase 6 tasks: B6a Owners in `msl/`: lexer, printer, parser

### T41: `Lexer` and `NumberLiteral` own the lexer

**What**: The 27 free functions of `msl/lexer.rs` become methods of `Lexer` and `NumberLiteral` (`classify_number`, 96 lines, split by literal kind); the two `expect`s (`:310,619`) become total (F2, §1.9).
**Where**: `crates/piperine-lang/src/msl/lexer.rs`
**Depends on**: T22
**Reuses**: findings.md §1.2 F2
**Requirement**: CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] The crate's row for the file leaves the free-function allowance; `long_functions.tsv` rows of the file have a verdict
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the calls of `lex` and `lex_bytes`, now `Lexer::lex` and `Lexer::lex_bytes`)
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(msl): the lexer's functions have an owner`

---

### T42: `Printer` owns the printer; `print.rs` splits

**What**: The 9 free functions of `msl/print.rs` become methods of `Printer`; the file (1 028 lines) splits into `print/{items,expressions,hardware}.rs` (F5).
**Where**: `crates/piperine-lang/src/msl/print.rs`
**Depends on**: T41
**Reuses**: findings.md §1.2 F5
**Requirement**: CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] No free function in the printer; the formatter's output is byte-identical (the crate's `msl_*` tests, unedited but for the one exception: the calls of `print`, now `Printer::print`)
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(msl): the printer owns its helpers`

---

### T43: The parser's input, recovery and diagnostics have owners

**What**: The 57 free functions of `msl/parser/{entry,base}.rs` become methods of `TokenInput`, `Recovery`, `ParseDiagnostics` and the entry type `Parser`; the bookkeeping `unwrap`s (`entry.rs:388,557-560`) become total.
**Where**: `crates/piperine-lang/src/msl/parser`
**Depends on**: T42
**Reuses**: findings.md §1.2 F1; Q4
**Requirement**: CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] The two files leave the free-function allowance; `piperine_lang`'s public parse entry points are unchanged
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the calls of `parse` and `parse_bytes`, now `SourceParser::parse` and `SourceParser::parse_bytes`)
- [x] the `lang::In` row of `worklists/single_implementor_traits.tsv` (`msl/parser/base.rs`, a trait alias) has a verdict: removed, or kept with its reason written on the trait (T32): kept, reason on the trait
- [x] Gate check passes: Quick

**Deferred**: the six fragment entry points stay free functions in `entry.rs` (6 rows of `free_functions.tsv`, kept, none added) because as methods of `SourceParser` the worklist guard lists them in `test_only_items.tsv` (a re-export line counted as their one product use) and that would add six rows. They are owned by the grammar tasks that follow: T44 `parse_expr` (closed by T44: `ExpressionGrammar::parse`), T45 `parse_type`/`parse_pat`/`parse_generics`, T46 `parse_module_item`/`parse_block` (done: `ModuleGrammar::parse`, `ExpressionGrammar::parse_block`). All six are gone; the first box is ticked. The entry type is `SourceParser`, not `Parser`: `chumsky::Parser` is the trait every grammar file names through `use super::*`, and a local `Parser` shadows it.

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(parser): input, recovery and diagnostics have owners`

---

### T44: `ExpressionGrammar`

**What**: `msl/parser/expr.rs`: the 14 free functions become methods of `ExpressionGrammar`, a production being a method that returns its parser; `build` (430 lines) becomes one method per precedence level and form.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs`
**Depends on**: T43
**Reuses**: T43
**Requirement**: CC-04
**Lane**: F
**Executor**: Opus

**Done when**:

- [x] The file leaves the free-function allowance; `build`'s row of `long_functions.tsv` is `split` (the rows of `build`, `block_like` and `stmt` are deleted: every method of `ExpressionGrammar` is under 60 lines, and the guard fails a row whose item is gone)
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the calls of `parse_expr`, now `ExpressionGrammar::parse`)
- [x] Gate check passes: Quick

**Closed from T43**: `parse_expr` is `ExpressionGrammar::parse` (its `free_functions` and `undeclared_words` rows deleted, none added). `parse` is a word product code spells on many lines, so the worklist guard does not list it in `test_only_items.tsv`.

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(parser): the expression grammar is a type`

---

### T45: `TypeGrammar`, `PatternGrammar`, `ItemGrammar`

**What**: The 23 free functions of `msl/parser/{ty,pat,item}.rs` become methods of the three grammar types.
**Where**: `crates/piperine-lang/src/msl/parser`
**Depends on**: T44
**Reuses**: T44
**Requirement**: CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] The three files leave the free-function allowance
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the calls of `parse_type`, `parse_pat` and `parse_generics`, now `TypeGrammar::parse`, `PatternGrammar::parse` and `GenericsGrammar::parse`)
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(parser): type, pattern and item grammars are types`

---

### T46: `ModuleGrammar` and `ParseCheck`

**What**: The 13 free functions of `msl/parser/msl.rs` become methods of `ModuleGrammar` and the 16 of `check.rs` methods of `ParseCheck`; `msl/diag.rs`'s free functions get their owner.
**Where**: `crates/piperine-lang/src/msl/parser`
**Depends on**: T45
**Reuses**: T44
**Requirement**: CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `crates/piperine-lang/src/msl` has no row in the free-function allowance
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the calls of `parse_module_item`, `parse_block` and `render`, now `ModuleGrammar::parse`, `ExpressionGrammar::parse_block` and `Diagnostic::render`)
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(parser): the module grammar and the parse check are types`

---

### Phase 7 tasks: B6a Owners in `pom/`, and the front end's small defects

### T47: `Classifier` owns classification

**What**: The 32 free functions of `pom/resolve/classify.rs` become methods of `Classifier`, which holds the `Ctx` they thread (F3).
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs`
**Depends on**: T46
**Reuses**: findings.md §1.2 F3
**Requirement**: CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] The file leaves the free-function allowance
- [x] `cargo test -p piperine-lang` passes with no existing test edited
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(pom): the classification pass is a type`

---

### T48: Each resolution pass is a type

**What**: The passes whose entry is a free `run` (`pom/resolve/{mod,members,shape,leaves}.rs`) and the free functions of `scopes.rs` and `imports.rs` become `NameResolution`, `MemberResolution`, `ShapeCheck`, `LeafResolution` and methods of the scope and import types; `resolve/mod.rs` (167 lines) keeps declarations only and its hygiene exemption is deleted (M4).
**Where**: `crates/piperine-lang/src/pom/resolve`
**Depends on**: T47
**Reuses**: findings.md §1.2 F4
**Requirement**: CC-04, CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `pom/resolve` has no row in the free-function allowance and no hygiene exemption
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the hygiene registry entry of `resolve/mod.rs` in the guards' `suite_hygiene.rs`, removed with the exemption)
- [x] `DeferReason::InferredGenerics` (`lang/src/pom/resolution.rs`), never referenced (T30, findings 4.1), is removed
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(pom): each resolution pass is a type`

---

### T49: `BodyWalk`, and the rest of `pom/` has owners

**What**: `pom/body.rs:269-345` and `reflect/body.rs:621-660` become `BodyWalk`; the free functions of `pom/{index,to_ast,source,crates,design}.rs` and `pom/lower/mod.rs` get the owner of the data they read; `lower/mod.rs` (160 lines) keeps declarations only; the two `SPEC_DEVIATION` comments of `pom/{body,source}.rs` are closed as T31 does.
**Where**: `crates/piperine-lang/src/pom`
**Depends on**: T48
**Reuses**: findings.md §1.2 F4
**Requirement**: CC-04, CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `pom/` outside `builtin.rs` and `reflect/` has no row in the free-function allowance; no `SPEC_DEVIATION` in the crate
- [x] `cargo test -p piperine-lang` passes with no existing test edited (the one exception: the calls of `is_crate_name`, now `CrateGraph::is_crate_name`, in `lang/tests/pom_crates.rs`, `piperine-cli` and `piperine-project`; and the hygiene registry entry of `lower/mod.rs`)
- [x] `Ty::has_repr` (`lang/src/types/ty.rs`), never referenced (T30, findings 4.1), is removed
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(pom): walkers and lookups have owners`

---

### T50: Reflection: one file per handle family

**What**: `pom/reflect/mod.rs` (512 lines) keeps declarations only; `reflect/hardware.rs` (1 110 lines, 14 handles) splits into `module.rs`, `instance.rs`, `behavior.rs`, `nature.rs`; the free functions of `reflect/{mod,items,hardware,body}.rs` get owners; the last hygiene exemption is deleted. `docs/pom/` paths follow.
**Where**: `crates/piperine-lang/src/pom/reflect`
**Depends on**: T49
**Reuses**: findings.md §5, M4
**Requirement**: CC-02, CC-04
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] No registered oversized `mod.rs`; every public path of `piperine_lang::pom` is unchanged
- [x] Verdicts in `test_only_items.tsv` for the POM's reflection items (kept: the public reflection surface `docs/pom/` documents)
- [x] `GenericParam::default_type` (`lang/src/pom/reflect/items.rs`), never referenced (T30, findings 4.1), is removed
- [x] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(pom): reflection handles one family per file`

---

### T51: `DynamicInteger` lives in `piperine-numbers`

**What**: Move `lang/src/elab/consts/integers.rs` (`DynamicInteger`, 50 methods) to `piperine-numbers`, documented under its `#![deny(missing_docs)]` (A6); the constant evaluator imports it.
**Where**: `crates/piperine-numbers/src`
**Depends on**: T50
**Reuses**: findings.md §1.6 A6
**Requirement**: CC-03
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] The A6 row leaves the `boundaries` allowance; `crates/piperine-numbers/tests/purity.rs` passes
- [x] `BinaryFormat::round_integer` (`numbers/src/format.rs`), never referenced (T30, findings 4.1), is removed
- [x] the `numbers::Limbs` row of `worklists/single_implementor_traits.tsv` has a verdict: the trait is removed, or kept with its reason written on it (T32)
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(numbers): the dynamic integer belongs to the numbers crate`

---

### T52: One `LogicPattern`, one reading of an out-of-range float

**What**: `LogicPattern` is declared once (`piperine-lang`) and `api/src/logic_pattern.rs` is gone; the text of an out-of-range float is read by one type that `lang/src/elab/override_text.rs` and `api/src/param_text.rs` both use (A4).
**Where**: `crates/piperine-lang/src/types/logic_pattern.rs`
**Depends on**: T51
**Reuses**: findings.md §1.6 A4
**Requirement**: CC-03
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [x] `no_type_of_the_inventory_is_defined_in_two_crates` passes with no allowance row
- [x] Gate check passes: Quick, and `cargo test -p piperine-api`

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(lang): one logic pattern and one float-text reader`

---

### Phase 8 tasks: B5a Owners in ir, plan, transpile, interp, project, api

### T53: `ir::Checker` becomes three checks

**What**: `ir/src/check.rs` (2 411 lines): `IdCheck` (dangling ids), `TypeCheck` (one method per `Expr` variant in place of the 518-line `expr`), `ClassCheck`, under `check/{ids,types,classes,statements}.rs` (G4).
**Where**: `crates/piperine-ir/src/check`
**Depends on**: T32
**Reuses**: findings.md §1.4 G4
**Requirement**: CC-05 (AC 2), CC-06
**Lane**: K
**Executor**: Opus

**Done when**:

- [x] `Ir::check` reports the same errors in the same order (the crate's tests, unedited); the file's rows of `long_functions.tsv` have a verdict
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(ir): the check is three passes`

---

### T54: `ir::Printer` per section

**What**: `ir/src/print.rs` (1 626 lines) splits into `print/{body,expression,arenas}.rs`; `body` (241 lines) and `arenas` (130) become one method per section; the exhaustive `expr` table is `kept: one arm per variant` (G6).
**Where**: `crates/piperine-ir/src/print`
**Depends on**: T53
**Reuses**: findings.md §1.4 G6
**Requirement**: CC-06
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The printed IR is byte-identical (the crate's text tests, unedited)
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(ir): the printer has one file per section`

---

### T55: The IR model: one file per family

**What**: `ir/src/model.rs` (1 388 lines, 76 types) splits into `model/{body,analog,digital,value,function,types}.rs` along the comment banners the file already has; `model.rs` re-exports every path.
**Where**: `crates/piperine-ir/src/model`
**Depends on**: T54
**Reuses**: findings.md §5
**Requirement**: CC-02
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] Every `piperine_ir::model::…` path compiles unchanged across the workspace
- [x] `crates/piperine-ir/src/build.rs` holds no `panic!` on an out-of-range net, instance or slot: the 6 sites of `resolve_net`, `bind_nets`, `bind_reached_node` and `bind_reached_net` return the typed `BuildError`; the five call sites in `piperine-lang/src/elab/{place,resolvers}.rs` are **call-site changes only** (each reports `Elaboration::internal_error` with the error's text), and the test calls take `.unwrap()`
- [ ] (owned by T96, coordinator decision, user AFK, pending review: `DigitalLowering` moves most of the callers) the 7th site, `body_data` (`IrBuilder::body_data`, behind 22 builder methods such as `carrier`, `drive`, `event`): returning an error changes those 22 signatures and about 30 call sites in `piperine-lang/src/elab/{carriers,resolvers,body,children}.rs`, which is lane F's code, more than call-site edits; reported to the coordinator
- [x] The error path has a test: `build::tests::a_builder_call_with_an_id_it_never_handed_out_names_the_id_and_the_count` drives each of the four `BuildError` variants and reads its message (fix round 1, P8-5)
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(ir): the object model one family per file`

---

### T56: IR dependencies: the results apart from the walk

**What**: `ir/src/deps.rs` (1 320 lines): `Deps`, `BodyDeps`, `Sources` in their own files; `Walk::expr` (212 lines), `body` (144) and `finish` (110) split by form.
**Where**: `crates/piperine-ir/src/deps`
**Depends on**: T55
**Reuses**: findings.md §5
**Requirement**: CC-06
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The crate's dependency tests pass unedited; the file's rows of `long_functions.tsv` have a verdict
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(ir): dependency results apart from their walk`

---

### T57: `BodyPlanBuilder`; `plan/digital` by table

**What**: `BodyPlan::new` (292 lines) becomes `BodyPlanBuilder` with one method per table (terms, pins, sources, functions) (G10); `plan/src/digital.rs` (950 lines) splits into `digital/{carriers,records,reactions}.rs`.
**Where**: `crates/piperine-plan/src`
**Depends on**: T56
**Reuses**: findings.md §1.4 G10
**Requirement**: CC-06
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The crate's tests pass unedited; `BodyPlan::new`'s row is `split`
- [x] Gate check passes: Golden

- [x] `Digital::functions` verdict `split` (fix round 1, P8-2): one method per function family (`cross_functions`, `tolerance_functions`, `drive_functions`, `event_functions`, `delayed_functions`, `rearm_functions`) and `declare_function`; the row is gone because every part is under 60 lines

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(plan): a builder with one method per table`

---

### T58: `ValuePrinter` becomes three printers

**What**: `transpile/src/print/value.rs` (1 641 lines, 116 methods, 22 fields): `ExpressionPrinter`, `StatementPrinter`, `RecordPrinter`, each holding the fields it reads (G5); the unit tests of the file move with their printer.
**Where**: `crates/piperine-transpile/src/print`
**Depends on**: T57
**Reuses**: findings.md §1.4 G5
**Requirement**: CC-05 (AC 2)
**Lane**: K
**Executor**: Opus

**Done when**:

- [x] The transpile goldens are byte-identical
- [x] Gate check passes: Golden
- [x] Test calls edited because a `pub` fn changed owner: `ValuePrinter::function` is `ExpressionPrinter::function` in `crates/piperine-transpile/tests/{aggregates,control,generics,number_widths}.rs` (the import and each call); the file's own unit tests moved with it to `print/expression.rs` (`use super::ExpressionPrinter`), and the two proofs of `ir-rust` that name them (C4, C67) and their rows in `uncited_tests.tsv` follow the module path

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(transpile): expression, statement and record printers`

---

### T59: The interpreter: `walk/` by statement family

**What**: `interp/src/walk.rs` (896 lines) splits by statement family; the crate doc says what the crate is (Q10: the reference for the analog core; it refuses every later form by name and gains no refusal test).
**Where**: `crates/piperine-interp/src/walk`
**Depends on**: T58
**Reuses**: findings.md §5; Q10
**Requirement**: CC-02
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The crate's tests pass unedited
- [x] `Walk::form_name` verdict `kept: one arm per variant, every arm a one-line name` (fix round 1, P8-2): all 60 arms are `Expr::X { .. } => "X"`
- [x] Gate check passes: Quick, and `cargo test -p piperine-cli --test twins`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(interp): the walk one file per statement family`

---

### T60: `piperine-project`: five owners

**What**: The 20 free functions of `project/src/*.rs` become methods of `Resolver`, `Lockfile`, `Version`, `GitCheckout` and `Project` (F12).
**Where**: `crates/piperine-project/src`
**Depends on**: T59
**Reuses**: findings.md §1.2 F12
**Requirement**: CC-04
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The crate has no row in the free-function allowance; the cli's and api's calls follow in the same commit
- [x] `Walk::dependency` and `Walk::package` verdict `split` (fix round 1, P8-2): `dependency_source`, `apply_references` and `authors` are methods; both rows are gone
- [x] The fifth owner is `ProjectRoot`, not the design's `Project` (coordinator decision, pending review): `piperine-api` declares `Project` (`api/src/project.rs`) and `repeated_type_names` refuses a second type of that name; `ProjectRoot` holds the two F12 functions `find_root` and `load` (fix round 1, P8-4)
- [x] Gate check passes: Quick, and `cargo test -p piperine-cli`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(project): resolver, lockfile, version, checkout and project`

---

### T61: `Session`: one runner per analysis family

**What**: `api/src/session.rs` (1 087 lines): `Session::analysis` (179 lines) dispatches to one runner type per analysis family; opening (plan, transpile-or-interpret, bind) reads in the order of the data path (G8, M9).
**Where**: `crates/piperine-api/src/session`
**Depends on**: T60
**Reuses**: findings.md §1.4 G8
**Requirement**: CC-05 (AC 2), CC-08
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] `Session`'s public methods are unchanged; the crate's tests pass unedited
- [x] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(api): one runner per analysis family behind the session`

---

### T62: The run request: one reader per key group

**What**: `api/src/request.rs` (1 271 lines) splits into `request/{run_table,overrides,options,grid}.rs`; `Keys::spec` (185 lines, 38 fields) becomes one reader per key group; the 6 free functions become methods of `Keys` (G9, F13).
**Where**: `crates/piperine-api/src/request`
**Depends on**: T61
**Reuses**: findings.md §1.4 G9
**Requirement**: CC-04, CC-06
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [x] The crate has no row in the free-function allowance; the request tests pass unedited
- [x] `Source::key` verdict `split` (fix round 1, P8-2): the table arms are `numbers` and `simparam_table`, the refusal of an unknown key is `unknown_key`; the function is 59 lines and its row is gone
- [x] Gate check passes: Quick, and `cargo test -p piperine-cli`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(api): the run request reads one key group at a time`

---

### Phase 9 tasks: B2b IR fixtures, one compile cache, more merges

### T63: The IR fixtures crate

**What**: Create the dev-only crate `piperine-ir-fixtures` from `ir/tests/support/{digital,devices}.rs` and `transpile/tests/support/mixed.rs`: the hand-built bodies as types (`Buck::dpwm`, 173 lines, one method per block).
**Where**: `crates/piperine-ir-fixtures/src/lib.rs`
**Depends on**: T40, T62
**Reuses**: T34's pattern
**Requirement**: CC-12 (AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] The crate builds with `#![deny(missing_docs)]` and no free function; its bodies pass `Ir::check`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test: hand-built ir bodies as one fixtures crate`

---

### T64: ir, plan and interp tests merged

**What**: `piperine-ir` 15 → `model`, `text`; `piperine-plan` 9 → `shape`, `circuit`; `piperine-interp` 7 → `interpreter`, on the IR fixtures; the `#[cfg(test)] #[path]` include and its `#[allow(dead_code)]` in `interp/src/lib.rs` go; `repoints/T64.tsv`.
**Where**: `crates/piperine-ir/tests`
**Depends on**: T63
**Reuses**: T63
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 5 targets across the three crates; every test keeps its name
- [x] Plans lowered; Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(ir): ir, plan and interpreter targets merged`

---

### T65: `piperine-transpile` tests: 29 targets become 5

**What**: Merge into `print`, `compile` (cache, rustc, load, compiled), `digital`, `analog`, `golden`, on the IR fixtures; the three `#[cfg(test)] #[path]` includes of `transpile/src/lib.rs` go; `repoints/T65.tsv`.
**Where**: `crates/piperine-transpile/tests`
**Depends on**: T64
**Reuses**: T63
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 5 targets; every test keeps its name; the goldens are untouched
- [x] Plan lowered to 5; Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(transpile): five targets on the ir fixtures`

---

### T66: One compile cache per test run

**What**: `piperine-parity` gains `CompileCache::shared()`: one directory under the target directory for the whole run; the 32 `static CACHE: OnceLock<TempDir>` of api, cli, parity and the root use it. The task first proves the transpiler's cache is safe under two writers (a test with two processes) and adds the lock if it is not.
**Where**: `crates/piperine-parity/src/compile_cache.rs`
**Depends on**: T65
**Reuses**: findings.md §3.2
**Requirement**: CC-12 (AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] `grep -r 'static CACHE' crates tests` finds nothing; a run leaves no directory in `TMPDIR`
- [x] `two_processes_share_one_cache` passes in `piperine-parity`'s `harness` tests
- [x] Gate check passes: Quick for parity, api, cli and the root's `integrated`

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): one compile cache for a test run`

---

### T67: `piperine-api` tests: 27 targets become 5

**What**: Merge into `session`, `host`, `records`, `params`, `analyses`; shared helpers (`Bench::settings`, `Kept::record`, `Ignore::record`) are methods of one fixture type in `tests/support/`; `repoints/T67.tsv`.
**Where**: `crates/piperine-api/tests`
**Depends on**: T66
**Reuses**: findings.md §3.5
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 5 targets (the buck one stays `test = false`); every test keeps its name
  - Departure: 6 targets, `session`, `host`, `records`, `params`, `analyses` and `memory`. `memory` needs its own process because its counting `#[global_allocator]` measures the whole process: the two tests that measure take its turn (`Counting::alone`) and nothing else runs beside them. The shared sinks and settings are `piperine_parity::sinks::{Ignore, Kept}` and `CompileCache::settings`, not a support module of the crate. T67's commit subject says "five targets" for a commit that makes six.
- [x] Plan lowered; Gate check passes: Ledger
  - Departure: api went 26 to 6, not 5.

**Tests**: integration
**Gate**: ledger

**Commit**: `test(api): five targets and one fixture type`

---

### T68: cli, numbers and project tests merged

**What**: `piperine-cli` 10 → `commands`, `book`, `twins` (the two buck targets stay); `piperine-numbers` 8 → `integers`, `floats` with their optimised profile; `piperine-project` 5 → `project`; `repoints/T68.tsv`.
**Where**: `crates/piperine-cli/tests`
**Depends on**: T67
**Reuses**: findings.md §3.8
**Requirement**: CC-12 (AC 3)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 6 default targets across the three crates; every test keeps its name; `binary` and `shared_cache` exist once
  - Departure: they exist no more. A call is `PathBuf::from(env!("CARGO_BIN_EXE_piperine"))` or `CompileCache::shared().path()` (T66's one cache), so no copy of either is left. The cli has 3 targets (`commands`, `book`, `twins`), numbers 2, project 1.
- [x] Plans lowered; Gate check passes: Ledger

**Tests**: integration
**Gate**: ledger

**Commit**: `test(cli): cli, numbers and project targets merged`

---

### Phase 10 tasks: B6b `Checker` split, `types/` regrouped

### T69: `CheckState`: the checker's fields by concern

**What**: Partition the 68 fields of `Checker` into `ValueTables`, `PatternTables`, `ModuleTables`, `AnalogTables`, `CarrierTables`, `GenericsTables`, the existing `TraitTable` and the result `Types`, held by `CheckState`; `Checker` holds the state and the worklist. No method moves; every access goes through the table.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T52
**Reuses**: design.md "Splitting `Checker`"
**Requirement**: CC-05 (AC 2)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] `Checker` has two fields; each table struct documents what it holds
- [ ] `cargo test -p piperine-lang` passes with no existing test edited
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): the checker's state one table per concern`

---

### T70: `ExpressionTyping` and `types/value/`

**What**: Move the files design.md lists into `types/value/` and their methods from `impl Checker` to `impl ExpressionTyping`; `ValueTables`' fields become private to the directory.
**Where**: `crates/piperine-lang/src/types/value`
**Depends on**: T69
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Checker` in `types/value/`; the diagnostics of every test are unchanged in text and order
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): expression typing owns its methods`

---

### T71: `PatternTyping` and `types/pattern/`

**What**: The same for `patterns.rs`, `aggregate_patterns.rs`, `cover.rs`, `logic_pattern.rs`.
**Where**: `crates/piperine-lang/src/types/pattern`
**Depends on**: T70
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Checker` in `types/pattern/`
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): pattern typing owns its methods`

---

### T72: `GenericsSolver`, `TraitTable` and their directories

**What**: The same for `types/generics/` (generics, generic values, inference, substitution, obligations, bounds, module bounds, module generics) under `GenericsSolver`, and for `types/traits/` (traits, coherence, derives, repr) as methods of `TraitTable`.
**Where**: `crates/piperine-lang/src/types/generics`
**Depends on**: T71
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Checker` in the two directories
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): the generics solver and the trait table own their methods`

---

### T73: `ModuleTyping` and `types/module/`

**What**: The same for module, result, generation, templates, template items, param requirement, reach, bundle, bundle handles, handles and record.
**Where**: `crates/piperine-lang/src/types/module`
**Depends on**: T72
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Checker` in `types/module/`
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): module typing owns its methods`

---

### T74: `AnalogTyping` and `types/analog/`

**What**: The same for analog, continuity, nature and nature rules.
**Where**: `crates/piperine-lang/src/types/analog`
**Depends on**: T73
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Checker` in `types/analog/`
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): analog typing owns its methods`

---

### T75: `CarrierTyping` and `types/carrier/`

**What**: The same for carrier, definition, coverage, element target, event and reaction.
**Where**: `crates/piperine-lang/src/types/carrier`
**Depends on**: T74
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-02
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Checker` in `types/carrier/`
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(types): carrier typing owns its methods`

---

### T76: `Checker` keeps the worklist

**What**: `types/table.rs` splits into `result_table.rs` (`Types`) and `callee.rs`; `model.rs` becomes `whole_model.rs`; `check.rs` holds the declaration worklist and the pass order; the allowance row of `Checker` in `an_inherent_impl_lives_with_its_type` is deleted; `types/`'s rows of `long_functions.tsv` have verdicts.
**Where**: `crates/piperine-lang/src/types/check.rs`
**Depends on**: T75
**Reuses**: T69
**Requirement**: CC-05 (AC 2), CC-06
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `an_inherent_impl_lives_with_its_type` passes under `**Run ownership**` with no row for `Checker`
- [ ] `types/` holds 8 files at its root and 6 directories, as design.md's map
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `refactor(types): the checker is its worklist`

---

### Phase 11 tasks: B5b Owners in abi, analog-solver, digital-solver, solver

### T77: `Validator` and `Thunk` own the ABI's functions

**What**: The 32 free functions of `abi/src/{validate,thunk,raw,verify,caller,layout,device,shape}.rs` become methods: `Validator` (holding the `Rule` list it reports; `cell_bytes`, 97 lines, split), `Thunk<D>`, and the type each remaining one reads; the 11 `extern "C"` functions stay (the exceptions of CC-04) and the allowance lists them by name (F11).
**Where**: `crates/piperine-abi/src`
**Depends on**: T40, T62
**Reuses**: findings.md §1.2 F11
**Requirement**: CC-04
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] The crate's free-function allowance holds exactly the 10 `extern "C"` functions of findings.md §1.2; `unsafe` is still only in `raw`, `thunk`, `caller`
- [ ] The fixtures crate and the transpiler's printed calls follow in the same commit; goldens byte-identical
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(abi): a validator and a thunk type own the free functions`

---

### T78: `abi/raw` by section of the contract

**What**: `abi/src/raw.rs` (1 138 lines, 27 `#[repr(C)]` types, 97 constants) splits into `raw/{views,shape,calls,constants}.rs` following abi.md's sections; `raw.rs` re-exports every path.
**Where**: `crates/piperine-abi/src/raw`
**Depends on**: T77
**Reuses**: findings.md §5
**Requirement**: CC-02
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] Every `piperine_abi::raw::…` path compiles unchanged; the layout tests pass unedited
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(abi): the raw layout one file per contract section`

---

### T79: The analog capabilities own their stamping

**What**: The 28 free functions of `analog-solver/src/device/*.rs` become methods of `DdtStamp`, `FlowStamp`, `DelayStamp`, `LimitState`, `Watches` and `SmallSignal`, each holding the part of `Assembly` it reads; `stamp_validation.rs`, `crossing.rs` and `math/integration.rs` get their owners (F8).
**Where**: `crates/piperine-analog-solver/src/device`
**Depends on**: T78
**Reuses**: findings.md §1.2 F8
**Requirement**: CC-04
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The crate has no row in the free-function allowance
- [ ] Gate check passes: Quick, and `cargo test -p piperine-solver`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(analog-solver): each capability owns its stamps`

---

### T80: `AnalogPlan::bind_instance` by step

**What**: `bind_instance` (260 lines, `device/plan.rs:283`) becomes a method of `AnalogPlan` split by what it binds (terminals, branches, capability parts).
**Where**: `crates/piperine-analog-solver/src/device/plan.rs`
**Depends on**: T79
**Reuses**: findings.md §1.5
**Requirement**: CC-06
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] Its row of `long_functions.tsv` is `split`; the crate's rows all have verdicts
- [ ] Gate check passes: Quick, and `cargo test -p piperine-solver`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(analog-solver): binding an instance one step per method`

---

### T81: `ReactionRun` owns the digital execution

**What**: The 15 free functions of `digital-solver/src/run.rs` become methods of `ReactionRun`, which holds the engine parts the 8-to-10-parameter lists pass around; `#[allow(clippy::too_many_arguments)]` (`:764`) is deleted; `call_reaction` (148 lines) and `run_reactions` (123) split (F6).
**Where**: `crates/piperine-digital-solver/src/run.rs`
**Depends on**: T80
**Reuses**: findings.md §1.2 F6
**Requirement**: CC-04, CC-06
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] No free function and no `#[allow]` in the file; no function of it takes more than 5 parameters beyond `self`
- [ ] Gate check passes: Quick, and `cargo test -p piperine-solver`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(digital-solver): a reaction run owns the execution`

---

### T82: `DigitalPlan` and `Reachability`

**What**: The 12 free functions of `digital-solver/src/{plan,dag}.rs` become methods of `DigitalPlan` and `Reachability` (`analyze`, 102 lines, split) (F7).
**Where**: `crates/piperine-digital-solver/src/plan.rs`
**Depends on**: T81
**Reuses**: findings.md §1.2 F7
**Requirement**: CC-04
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The crate has no row in the free-function allowance
- [ ] Gate check passes: Quick, and `cargo test -p piperine-solver`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(digital-solver): the plan and its reachability are types`

---

### T83: `Bind::run` by pin role

**What**: `solver/src/core/bind.rs`: `bind` (371 lines) becomes `Bind::run` with one method per pin role (PORT, PARAM, GLOBAL, PRIVATE, the four the module doc names); the file's other 4 free functions get owners (F9).
**Where**: `crates/piperine-solver/src/core/bind.rs`
**Depends on**: T82
**Reuses**: findings.md §1.2 F9
**Requirement**: CC-04, CC-06
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] S21 (user, 2026-11-27): the ground is spelled `gnd` only. `GroundName` accepts `gnd`; the ground net's label (`core/net.rs`) prints `gnd`; `GND`, `vss` and `VSS` given as a node name are refused with an error naming `gnd`, in the builder and in a noise request (`api/src/noise.rs`); `edge_gnd_name_routes_to_ground` becomes a test of the refusal (a `replace` row)
- [ ] No free function in the file; `bind`'s row is `split`
- [ ] Gate check passes: Quick, and `cargo test -p piperine-api`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(solver): binding one method per pin role`

---

### T84: The analyses' helpers have owners

**What**: The 22 free functions of `solver/src/analyses/*.rs` and `solver/src/{core,protocol}/*.rs` (`solve_dense`, `dominant_monodromy_magnitude`, `restamp_jacobian`, `crossing_estimate`, the error builders) become methods of the solver type of their file (F10).
**Where**: `crates/piperine-solver/src/analyses`
**Depends on**: T83
**Reuses**: findings.md §1.2 F10
**Requirement**: CC-04
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The crate has no row in the free-function allowance
- [ ] Gate check passes: Quick, and `cargo test -p piperine-api`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(solver): each analysis owns its numeric helpers`

---

### T85: `transient/` by responsibility

**What**: `solver/src/analyses/transient.rs` (2 012 lines, 17 types) splits into `transient/{options,system,step_control,scheduled_sets,attempt}.rs`; `TransientSolver::solve` (210 lines) reads as its stages (G7).
**Where**: `crates/piperine-solver/src/analyses/transient`
**Depends on**: T84
**Reuses**: findings.md §1.4 G7
**Requirement**: CC-02, CC-06
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] Every public path is unchanged; the transient and parity-baseline targets pass unedited
- [ ] Gate check passes: Quick, and `cargo test -p piperine-api -p piperine-cli --test twins`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(solver): the transient solver one file per responsibility`

---

### Phase 12 tasks: B11a The remaining merges

### T86: `piperine-lang` tests I: msl, pom, std

**What**: Merge the `msl_*` targets into `msl`, the `pom_*` into `pom`, the `std_*` into `std`; `design_of`, `diagnostics` and `snippet` exist once as methods of a `Checked` source fixture in `tests/support/`; `repoints/T86.tsv`.
**Where**: `crates/piperine-lang/tests`
**Depends on**: T68
**Reuses**: findings.md §3.8
**Requirement**: CC-12 (AC 3, AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] Every test keeps its name; the three targets open with a `//!` header
  - Annotation: `buck_example` (a hand-merged sibling with no target of its own in the design table) became the module `buck_example` of `pom`; `msl` holds 115 tests, `pom` 295 (with `buck_example`), `std` 71 = 481, the count of `#[test]` of the 20 former targets. `std/library.rs`'s two source-reading tests (`the_fixture_prelude_is_gone`, `no_pom_test_was_removed`) now walk the new directory layout; their assertions are unchanged.
- [x] Gate check passes: Ledger
  - Annotation: the `Checked` fixture's single `design_of`/`diagnostics`/`snippet` (the **What** line) belongs to the `types_*` files that define them (fourteen copies); it lands with T87, which merges those files, so the files are touched once.

**Tests**: integration
**Gate**: ledger

**Commit**: `test(lang): msl, pom and std targets merged`

---

### T87: `piperine-lang` tests II: types, elab

**What**: Merge the 30 `types_*` targets into `types_values`, `types_patterns`, `types_modules`, `types_analog`, `types_carriers`, `types_generics` (the directories of T70 to T75) and the 26 `elab_*` into `elab_values`, `elab_analog`, `elab_digital`, `elab_structure`, on the `Checked` and `Elaborated` fixtures; `repoints/T87.tsv`.
**Where**: `crates/piperine-lang/tests`
**Depends on**: T86
**Reuses**: T86
**Requirement**: CC-12 (AC 3, AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 15 targets in the crate (design.md's table); every test keeps its name
  - Annotation: the crate held 31 `types_*` and 27 `elab_*` targets, not the 30 and 26 of the text. `book` and `spec_codes` (named by no task) became modules of `spec_structure`, and `elab_buck` is the module `buck_converter` (its support module `buck` keeps the name). The plan counts them: msl, pom, std, six types, four elab, spec_structure, refusals = 15. Counts per target equal the `#[test]` count of their former files: types 88/56/72/69/80/88, elab 75/49/95/193, spec_structure 29.
- [x] Plan lowered to 15; Gate check passes: Ledger
  - Annotation: the one `Checked` fixture for `design_of`/`diagnostics`/`snippet` (T86's What) is not done here: the fourteen copies differ in signature (some return the source with the diagnostics, some take items and a body) and are free helpers of the files, so they close with T91 as methods of fixture types, together with every other helper.

**Tests**: integration
**Gate**: ledger

**Commit**: `test(lang): types and elab targets merged`

---

### T88: `piperine-parity` tests: 61 targets become 11

**What**: Merge the `source_*` targets by Part of the spec into `source_values`, `source_carriers`, `source_structure`, `source_behavior`, `source_execution`, `source_io`, `source_stdlib`, `source_host`; the twelve `parity_*` into `parity` (first check ngspice's per-twin caching survives one process; stop if a comparison would be skipped); `harness`, `boundary`; the 7 buck targets stay `test = false`; `repoints/T88.tsv`.
**Where**: `crates/piperine-parity/tests`
**Depends on**: T87
**Reuses**: findings.md §3.8
**Requirement**: CC-12 (AC 3, AC 5)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] 11 default targets; every test keeps its name; `the_parity_oracles_are_not_reduced` passes
  - Annotation: the 62 targets of the plan were 41 `source_*`, 12 `parity_*`, `harness`, `boundary`, `backend`, `backends`, `bench`, `deviation`, `project`, `run_errors` and `twins`. `backend`, `backends` and `deviation` became modules of `parity`; `bench` and `twins` of `harness`; `project` and `run_errors` of `boundary`. Per-target tests equal the former files' counts: source 114/84/115/69/39/29/55/9, parity 57, harness 20, boundary 11. Three tests needed a path edit that changes no assertion: `harness::two_processes_share_one_cache` starts its writers with the module-qualified `--exact harness::a_process_that_writes_the_cache`, and the corpus guard reads `tests/parity/*.rs` (leaving out the backend, backends and deviation modules, which the `parity_*` file filter never covered).
- [x] `cargo test -p piperine-parity --test parity -- --nocapture` compares every twin with ngspice
  - Annotation: ngspice is installed; the only `SKIP` lines are the six tests that pass `Lookup::Skip` or print the skip line on purpose (`uic_rc`, `tran_rc`, `diode_rs`, three noise twins). The per-twin comparisons are not cached across tests, so one process changes nothing: 56 of 57 passed on the first full run, the one failure being the corpus guard's file filter above.
- [x] Plan lowered to 11; Gate check passes: Ledger
  - Annotation: `root-e2e`'s C16 Run line `--test 'parity_*'` became `--test parity`; two proof-ledger rows that recorded the targets `parity` and `source_io` as missing were deleted because those targets now exist.

**Tests**: integration
**Gate**: ledger

**Commit**: `test(parity): source targets by spec part, one parity target`

---

### T89: The Book is one target

**What**: The 23 `book_*` targets of the root become the target `book` with one module per chapter (Q11); `inventory.tsv`'s `covered_by` cells and every feature's `Run` lines follow through `repoints/T89.tsv`; `CLAUDE.md` says "one module per Book chapter".
**Where**: `Cargo.toml`
**Depends on**: T88
**Reuses**: findings.md §3.8; Q11
**Requirement**: CC-12 (AC 3, AC 5)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] The root has 4 default targets (`book`, `integrated`, `scenarios`, `host_ii`) and the 2 buck ones
  - Annotation: `book` holds 355 tests, the sum of the 21 chapters and the two guards (`inventory_guard`, `claims_guard`) it replaces; `book_buck` and `buck_scenarios` stay `test = false`. The Book inventory resolver learned `piperine/<target>::<module>::<fn>` for a root target whose path is a `main.rs`. `ROOT_TARGET_EXEMPTIONS` is empty now: the root of `book` names `piperine::`, so the two guard exemptions went with the two guard targets.
- [x] `every_row_names_an_existing_test` and `every_block_of_the_book_has_one_row` pass under `**Run book**`
- [x] Gate check passes: Ledger
  - Annotation: five proof-ledger rows that recorded `book` as a missing target or test were deleted because the target exists now.

**Tests**: integration
**Gate**: ledger

**Commit**: `test: the book's chapters are modules of one target`

---

### T90: A design proven twice is proven from source

**What**: Remove the four tests of `transpile`'s `commits` module whose twins in `parity`'s `source_commits` module run by default (`replace` rows in `repoints/T90.tsv`); keep every hand-built test whose source twin is a buck target or that reaches an IR form the front end cannot write (verdict `kept` with the form); record `parity_*` and `tests/scenarios.rs` as both kept (Q9).
**Where**: `crates/piperine-repository-guards/worklists/duplicate_tests.tsv`
**Depends on**: T89
**Reuses**: findings.md §3.3 T2
**Requirement**: CC-12 (AC 2, AC 5)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [x] Each removed test's proof rows name the source-level survivor
- [x] Gate check passes: Ledger
  - Annotation: the four tests (`two_commits_exchange_two_carriers`, `the_later_statement_wins`, `two_statements_make_a_pulse`, `an_unchanged_commit_fires_nothing`) were the whole of `transpile/tests/digital/commits.rs`, which went with them; their four worklist rows of `duplicate_tests.tsv` went because the names are no longer duplicated, and `ir-rust` C86 to C89 now run `piperine-parity --test source_execution` (`replace` rows of `repoints/T90.tsv`, applied by the tool). Kept with their verdicts: the hand-built twins whose source side is a buck target (`the_adc_commits_the_rounded_code_after_tconv` and the other rows still in the worklist) and `parity_*` (now the modules of `parity`) with `tests/scenarios.rs` as both kept (Q9, design.md).

**Tests**: integration
**Gate**: ledger

**Commit**: `test(transpile): drop commit tests the source run already proves`

---

### T91: No free helper, no path include, no `#[allow]` in any test

**What**: Close every remaining row of `no_free_helper_function_in_test_code`, `no_test_support_is_shared_by_path` and `no_allow_in_test_code` (lang, parity, transpile, api, cli, ir, plan, interp, root): each helper becomes a method of a fixture type (Q17).
**Where**: `crates/piperine-repository-guards/allowances`
**Depends on**: T90
**Reuses**: findings.md §3.5
**Requirement**: CC-12 (AC 4)
**Lane**: T
**Executor**: Sonnet

**Done when**:

- [ ] The three allowances are empty for every crate
- [ ] `no_crate_has_more_targets_than_its_plan` passes with the plan at design.md's table (73; phases 14 and 17 add one target each, raising it to 75)
- [ ] Gate check passes: Quick for the crates the task touched

**Tests**: integration
**Gate**: quick

**Commit**: `test: every test helper is a method of a fixture type`

---

### Phase 13 tasks: B6c `Elaboration` split, `elab/` regrouped

### T92: `ElaborationState`: the elaborator's fields by concern

**What**: Partition the 37 fields of `Elaboration` into `ValueTables`, `FunctionTables`, `AnalogSites`, `DigitalTables`, `StructureTables` held by `ElaborationState`, and the 42 fields of `Scope` into `ValueScope`, `AnalogScope`, `DigitalScope`. No method moves.
**Where**: `crates/piperine-lang/src/elab/elaborator.rs`
**Depends on**: T76
**Reuses**: design.md "Splitting `Elaboration`"
**Requirement**: CC-05 (AC 2)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] `Elaboration` holds the state only; `Scope` has three fields; each table documents what it holds
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): the elaborator's state one table per concern`

---

### T93: `ValueLowering` and `elab/value/`

**What**: Move the files design.md lists into `elab/value/` and their methods to `impl ValueLowering`; `lower.rs` becomes `expressions.rs` and `lower_expr_unchecked` (213 lines) one method per expression family.
**Where**: `crates/piperine-lang/src/elab/value`
**Depends on**: T92
**Reuses**: T92
**Requirement**: CC-05 (AC 2), CC-06
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Elaboration` in `elab/value/`; goldens, twins and the Book inventory are byte-identical
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): value lowering owns its methods`

---

### T94: `FunctionLowering` and `elab/functions/`

**What**: The same for functions, variants, instances, handles and event functions.
**Where**: `crates/piperine-lang/src/elab/functions`
**Depends on**: T93
**Reuses**: T92
**Requirement**: CC-05 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Elaboration` in `elab/functions/`
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): function lowering owns its methods`

---

### T95: `AnalogLowering` and `elab/analog/`

**What**: The same for the 16 files of the analog block (contributions, selectors, `ddt`, `delay`, `noise`, `ddx`, expansion, symbolic, unroll, session, inflow, signal flow, node choice, nature, host sources).
**Where**: `crates/piperine-lang/src/elab/analog`
**Depends on**: T94
**Reuses**: T92
**Requirement**: CC-05 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Elaboration` in `elab/analog/`
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_behavior`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): analog lowering owns its methods`

---

### T96: `DigitalLowering` and `elab/digital/`

**What**: The same for the 14 files of carriers, elements, drives, loops, events, reactions, records and resolvers.
**Where**: `crates/piperine-lang/src/elab/digital`
**Depends on**: T95
**Reuses**: T92
**Requirement**: CC-05 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Elaboration` in `elab/digital/`
- [ ] `IrBuilder::body_data` (`crates/piperine-ir/src/build.rs`) returns `Result<_, BuildError>` instead of panicking on an out-of-range body id, and the ~22 builder methods behind it and the elab callers (`carriers`, `resolvers`, `body`, `children`) propagate it as `Elaboration::internal_error`, the way T55 did for the other six sites (owner of T26's and T55's open box; coordinator decision, user AFK, pending review)
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_carriers`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): digital lowering owns its methods`

---

### T97: `StructureLowering` and `elab/structure/`

**What**: The same for body, children, placement, bundles, generation, generic bodies, templates, structural params, hoisting, the three `reach` files and override text; the two `#[allow(clippy::too_many_arguments)]` of `generation.rs` become a parameter object.
**Where**: `crates/piperine-lang/src/elab/structure`
**Depends on**: T96
**Reuses**: T92
**Requirement**: CC-05 (AC 2), CC-06
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] No `impl Elaboration` in `elab/structure/`; no `#[allow]` in `crates/piperine-lang/src`
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_structure`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): structure lowering owns its methods`

---

### T98: `ConstEval` by value family

**What**: `elab/consts.rs` (1 073 lines) becomes `consts/evaluator.rs` holding the entry and the dispatch; each value family's methods live in its file of `consts/` (G3); `consts/composite.rs` (977 lines) splits by aggregate kind.
**Where**: `crates/piperine-lang/src/elab/consts`
**Depends on**: T97
**Reuses**: findings.md §1.4 G3
**Requirement**: CC-06
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `ConstEval` stays one type; every `impl ConstEval` is below `elab/consts/`
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): the constant evaluator one file per value family`

---

### T99: `Elaboration` keeps the pass order

**What**: `elaborator.rs` holds `Elaborator`, `Elaboration::run` and the pass order; the allowance row of `Elaboration` is deleted; `elab/`'s rows of `long_functions.tsv` have verdicts.
**Where**: `crates/piperine-lang/src/elab/elaborator.rs`
**Depends on**: T98
**Reuses**: T92
**Requirement**: CC-05 (AC 2), CC-06
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `an_inherent_impl_lives_with_its_type` passes with no row for `Elaboration` or `ConstEval`
- [ ] `elab/` holds 3 files at its root and 6 directories, as design.md's map
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `refactor(elab): the elaboration is its pass order`

---

### Phase 14 tasks: CC-11 ground: how the compiler finds a `std` item

### T100: Contract: what `std` can declare today

**What**: For each form design.md "What `std` must be able to write" lists (an inherent `impl` on a primitive type, `extern "intrinsic"` on a method and on a module, a trait with default type parameters, a generic method with an `Fn` bound, a trait bound satisfied by the compiler), write one acceptance test in the new target `std_declarations` that declares it in a `std`-like crate; record on the Handoff each form as *checks today* or *missing*, and for each missing one the task below that implements it as a general form. Also record §19 rule 11's exact set of field types and check each is a `Repr` type or `str` (what `RecordValue` is implemented for).
**Where**: `crates/piperine-lang/tests/std_declarations.rs`
**Depends on**: T91, T99
**Reuses**: findings.md §2.2, last paragraph
**Requirement**: CC-11 (AC 1)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] One test per form, passing or asserting today's refusal by its code, each named on the Handoff; the target plan of the crate rises by one, named in the commit body
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): which declaration forms std can write today`

---

### T101: `LanguageItems`

**What**: Add `LanguageItem` and `LanguageItems` (design.md): each `std` item the compiler gives meaning to is found once, by its `std` path, at resolution. First users: the five intrinsics (`parse_table`, `parse_hex`, `matches_bits`, `logic`, `table_model`) and `Logic`, in place of `types/intrinsic.rs:30-40`'s `(namespace, name)` strings and the scan of `:123-130`.
**Where**: `crates/piperine-lang/src/pom/language_items.rs`
**Depends on**: T100
**Reuses**: `extern "intrinsic"` (§3 rule 10)
**Requirement**: CC-11 (AC 2)
**Lane**: single
**Executor**: Opus

**Done when**:

- [ ] `a_shadowed_intrinsic_is_the_models_own_function` and `a_std_without_a_language_item_is_an_internal_error` pass in `std_declarations`
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(lang): language items found by their std declaration`

---

### T102: The prelude-declaration guard, as a ratchet

**What**: Add to `std_declarations`: `every_builtin_is_declared_in_std` (allowance: today's `BUILTINS` rows outside the primitive list of S17), `no_std_item_is_found_by_its_spelling` (allowance: the string comparisons of findings.md §2.1, by file and count), `every_method_of_appendix_h_is_declared_in_std` (allowance: the 75 catalogue rows), `every_kept_special_case_has_its_written_reason` (S10, S17 to S20, each a doc comment in `std` naming the reason).
**Where**: `crates/piperine-lang/tests/std_declarations.rs`
**Depends on**: T101
**Reuses**: T6's `Allowance`
**Requirement**: CC-11 (AC 1, AC 2)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] The four tests pass under `**Run stddecl**`; each fails on a new builtin row, a new string comparison or a stale allowance row
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): builtins must be declared in std`

---

### Phase 15 tasks: B7 Declare what exists I: traits, core, carriers

### T103: `std/traits.msl`

**What**: Declare `Repr` (with `WIDTH`, `encode`, `decode`), `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Default`, `Debug`, `Hash`, `From`, the six operator traits and `Resolve` as the spec prints them; the checker reads each signature from the declaration (`types/operators.rs:195-210`, `bounds.rs:266-376`, `generics.rs:163-172` lose their name tables); 19 `BUILTINS` rows leave; Appendix H.8 and H.10 say "declared in `std`" (S8).
**Where**: `crates/piperine-lang/std/traits.msl`
**Depends on**: T102
**Reuses**: T101
**Requirement**: CC-11 (AC 2, AC 3)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] The trait rows leave the guard's allowances; `#[derive]` and operator `impl`s of every corpus check as before
- [ ] `docs/spec/index.md`'s revision note names S8
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): the library traits are declared in the prelude`

---

### T104: The bounds of the text parsers are traits

**What**: `pub trait TableRow` and `pub trait Integer` in `std/io.msl` carry the contracts that are comments today (`parse_table<Row: TableRow, …>`, `parse_hex<T: Integer, …>`); `types/intrinsic.rs:56-95`'s hand typing goes (S15).
**Where**: `crates/piperine-lang/std/io.msl`
**Depends on**: T103
**Reuses**: T101
**Requirement**: CC-11 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] A wrong row type is refused by the bound, with the code it had; §22.6.4 prints the bounds
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): the text parsers state their bounds`

---

### T105: `std/core.msl`: the functions of §14 and the enum `Analysis`

**What**: Declare `now`, `temperature`, `simparam`, `analysis`, `is_static`, `freq`, `gmin`, `source_scale`, `potential`, `flow`, `ddx`, `ac_stim` as `extern "intrinsic"` functions with §14's parameters (the probe parameter a `Probe` bound, the second node a trailing `Option`, the position class by the declaring module), and `pub enum Analysis { Dc, Transient, Ac, Noise }`; the prelude re-exports the type, not its variants. `Analysis::Dc` and `is_static()` work; the bare names and `analysis(Static)` still resolve until T106. The function rows of `CORE_ROWS` and the `match name` of `elab/analog.rs:174-200` go (S11, S7).
**Where**: `crates/piperine-lang/std/core.msl`
**Depends on**: T104
**Reuses**: T101
**Requirement**: CC-11 (AC 2, AC 3)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] The 11 function rows and the `Analysis` type row leave the allowances; `analysis(Analysis::Dc)` and `is_static()` lower to what `analysis(Dc)` and `analysis(Static)` lower to (one model written both ways gives identical generated Rust)
- [ ] Part IV §14 says each primitive is declared in `std::core`; the revision note names S11
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_behavior`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): the core functions and the analysis enum are declared`

---

### T106: An analysis kind is written `Analysis::Dc`; `Static` is `is_static()`

**What**: Rewrite every bare `Dc`, `Transient`, `Ac`, `Noise` to `Analysis::Dc`, … and every `analysis(Static)` to `is_static()`: `std` (7 lines of `analog.msl` and `monitor.msl`), the `.msl` corpora (29 lines in all), the Rust test strings (60 lines), the spec (53 lines; §11.1 and §14.4 say the variants are written with their enum and `Static` is not an analysis) and the Book (3 lines, `inventory.tsv` fingerprints following); delete the five bare variant rows of `BUILTINS`, so a bare name no longer resolves (spec Assumption 16). A model that wants the short names imports them itself.
**Where**: `crates/piperine-lang/src/pom/builtin.rs`
**Depends on**: T105
**Reuses**: T105
**Requirement**: CC-11 (AC 1, AC 3)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `an_analysis_kind_is_written_with_its_enum` passes under `**Run stddecl**`: `analysis(Dc)` is the ordinary unresolved-name error, `analysis(Analysis::Dc)` checks, and `use std::core::Analysis::*;` brings the bare names back in the file that writes it
- [ ] `grep -rnE 'analysis\((Dc|Transient|Ac|Noise|Static)\)'` over the tree finds nothing; twins, goldens and every parity source target are byte-identical
- [ ] The revision note names S7; `every_block_of_the_book_has_one_row` passes
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity -p piperine-api`, `cargo test -p piperine --test book`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): analysis kinds are written with their enum`

---

### T107: `std/core.msl`: the core modules, `settle`, `bound_step`

**What**: Declare `ddt`, `delay`, `limit`, `noise`, `cross`, `at` as `extern "intrinsic"` modules, the event `settle`, and `bound_step(dt: f64)` as an intrinsic function of the analog class; the parser's two peeks for `bound_step` (`parser/msl.rs:298`, `check.rs:378`) and `BuiltinKind::Statement` go (S11, S12).
**Where**: `crates/piperine-lang/std/core.msl`
**Depends on**: T106
**Reuses**: T105
**Requirement**: CC-11 (AC 2, AC 3)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] The 8 rows leave the allowances; `E0404`'s cases are refused by the function-class rule with their code
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_behavior`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): the core modules and bound_step are declared`

---

### T108: Carrier methods are declared; `rise` and `fall` compile from their source

**What**: `impl<T: Repr> Bit<T>` in `std/carriers.msl` declares `changed` and `prev`; `others` and `inflow` are declared on their bound; `Bit::rise` and `Bit::fall` compile from the bodies `std/values.msl:18-38` writes, and `Builtin::Rise`/`Fall`, `rise_guard`/`fall_guard` and the refusal of `types/call.rs:320` go (S13, S14). If `others` or `inflow` cannot be given by a bound, the task stops on that item and writes its reason in the allowance.
**Where**: `crates/piperine-lang/std/carriers.msl`
**Depends on**: T107
**Reuses**: T101
**Requirement**: CC-11 (AC 1, AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] The 4 method rows leave the allowances or carry a written reason; a model's own `rise` on its nature runs
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_carriers`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): carrier methods declared, rise and fall compiled`

---

### T109: One shadowing rule

**What**: A core name declared in `std` shadows like any prelude name; `E0457` narrows to `gnd`, `Nature` and `str` (`part_ii_carriers.md:438`, `part_iv_behavior.md:796`); `pom/builtin.rs:190` `is_core_primitive` goes (S16).
**Where**: `crates/piperine-lang/src/pom/builtin.rs`
**Depends on**: T108
**Reuses**: T105
**Requirement**: CC-11 (AC 3)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `a_model_may_declare_its_own_ddt` passes in `std_declarations`; the Book's `E0457` listing follows; the revision note names S16
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(lang): core names shadow like any prelude name`

---

### Phase 16 tasks: B7 II with B8 (b), (c): `Range`, `Iterator`, `Option`, the method catalogue

### T110: `Range<T>` and `Iterator<T>` are named

**What**: `extern "intrinsic"` may mark a `struct` (§3 rule 10, Appendix B); `std/sequences.msl` declares `Range<T>` and `Iterator<T>`; `a..b` has type `Range<T>` and `iter()` returns `Iterator<T>`; H.9 loses the unnameable types; naming them gives no new position (S5).
**Where**: `crates/piperine-lang/std/sequences.msl`
**Depends on**: T109
**Reuses**: T101
**Requirement**: CC-11 (AC 1, AC 3)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] `option_range_and_iterator_are_std_declarations` (its range and iterator halves) passes under `**Run stddecl**`
- [ ] A struct expression of either type and a parameter of either type are refused by name
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): ranges and iterators have named types`

---

### T111: `Option` is a `std` enum

**What**: `std/option.msl` declares `pub enum Option<T> { None, Some(T) }`; `?`, a left-out trailing parameter and the implicit `Some` name the declaration through `LanguageItems`, which maps it to the existing representation at one place; the 3 `BUILTINS` rows go; Appendix H.4 and §1 follow (S6). The IR does not change.
**Where**: `crates/piperine-lang/std/option.msl`
**Depends on**: T110
**Reuses**: T101
**Requirement**: CC-11 (AC 1, AC 3)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] `option_range_and_iterator_are_std_declarations` passes whole; reflection lists `Option` as a `std` item
- [ ] Transpile goldens and twins byte-identical
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): option is an enum of the library`

---

### T112: `std/numbers.msl`: the scalar methods

**What**: Declare the 32 methods of `f64`, the 13 of the integers (the signed-only rows under the `Int<N>` impl) and the 2 of `bool` as `extern "intrinsic"` methods; method lookup is the ordinary one and `IntrinsicBinding` maps each declaration to its `ir::Method` (S4).
**Where**: `crates/piperine-lang/std/numbers.msl`
**Depends on**: T111
**Reuses**: T101
**Requirement**: CC-11 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] 47 catalogue rows leave the allowance; `Lookup::Uncatalogued` for these receivers is the ordinary "no method" error with its code
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_values`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): scalar methods are declared in the library`

---

### T113: The methods of arrays, ranges, iterators and `Option`

**What**: Declare the 28 remaining rows of H.4 to H.7, the 14 typed from a payload or a closure as generic methods with an `Fn` bound (`map`, `filter`, `fold`, `any`, `all`, `zip`, `sum`, `then`, `and_then`, `unwrap_or_else`, …).
**Where**: `crates/piperine-lang/std/sequences.msl`
**Depends on**: T112
**Reuses**: T110, T111
**Requirement**: CC-11 (AC 2)
**Lane**: F
**Executor**: Opus

**Done when**:

- [ ] `every_method_of_appendix_h_is_declared_in_std` passes with no allowance row
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity --test source_values`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): sequence and option methods are declared`

---

### T114: The hidden tables go

**What**: `types/catalogue.rs` is deleted; `BUILTINS` holds the closed primitive set of S17 and `gnd`, each with a doc comment in `std` saying where its rules are; `types::Method`'s name table and the remaining string comparisons go; the guards are strict.
**Where**: `crates/piperine-lang/src/pom/builtin.rs`
**Depends on**: T113
**Reuses**: T103 to T113
**Requirement**: CC-11 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `every_builtin_is_declared_in_std` and `no_std_item_is_found_by_its_spelling` pass under `**Run stddecl**` with empty allowances
- [ ] `the_revision_note_names_every_special_case_removed` passes under `**Run specstructure**` for S3 to S16
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `refactor(lang): the builtin tables are the library's declarations`

---

### Phase 17 tasks: B8 (a) Named variadic, `emit`, the reserved streams, `include_str`

### T115: A named variadic parameter parses

**What**: `fn_param` gains `'..' IDENT ':' type`; a call's arguments gain `'..' IDENT`, and a named argument `name: value` or a bare identifier may be a field (design.md's grammar; the spelling is §3.2.4's, spec Assumption 13); the POM carries both; §3.2 and Appendix B.3 are amended with design.md's text. Until T116 the checker refuses the forms by name.
**Where**: `crates/piperine-lang/src/msl/parser`
**Depends on**: T102
**Reuses**: design.md "Named variadic parameters"
**Requirement**: CC-11 (AC 1, AC 3)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] `a_named_variadic_parameter_and_its_arguments_parse_and_print` passes in the new target `named_variadics` (round trip through the printer); the target plan of the crate rises by one
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(msl): a named variadic parameter and field arguments parse`

---

### T116: A named variadic is typed

**What**: `std/records.msl` declares `RecordValue` (no supertrait; the core implements it for the `Repr` types and `str`, spec Assumption 14) and `RecordFields`; the fields of a call form an anonymous record type; §3.2.11 to §3.2.13 and §3.2.15 hold for every `fn`: last parameter, at most one, bounded by `RecordFields`; a named argument that names no optional parameter is a field, and a name that is an optional parameter binds it; a bare identifier is `name: name`; distinct names (`E0604`); each field's type implements `RecordValue`; `..name` forwards. Two new codes take the next free numbers.
**Where**: `crates/piperine-lang/src/types/value`
**Depends on**: T115
**Reuses**: T115; spec Assumption 14
**Requirement**: CC-11 (AC 1, AC 3)
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] `a_user_function_declares_and_calls_a_named_variadic`, `a_named_variadic_must_be_last_and_bounded_by_record_fields`, `a_bare_identifier_is_a_field_of_its_own_name`, `an_optional_parameter_wins_over_a_field_of_its_name`, `a_repeated_field_is_refused` and `a_field_type_must_implement_record_value` pass under `**Run variadics**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): named variadic parameters for every function`

---

### T117: A named variadic runs

**What**: A function with a named variadic is instantiated once per record type (§3.2.14, MD-81) and `..name` forwards its fields.
**Where**: `crates/piperine-lang/src/elab/functions`
**Depends on**: T116
**Reuses**: T116
**Requirement**: CC-11 (AC 1)
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] `a_user_wrapper_forwards_its_fields` passes under `**Run variadics**` (elaboration) and `a_user_wrapper_over_emit_writes_the_record` in `source_io` (a run, after T118)
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(elab): a named variadic is instantiated per record type`

---

### T118: `emit` is a function of `std`

**What**: `pub extern "intrinsic" fn emit<F: RecordFields>(stream: str, ..fields: F);` in `std/records.msl`, found as `LanguageItem::Emit`; it lowers to the record the `emit!` form lowers to; `std/io.msl` calls it; §19's syntax, rule 11 and `E0602` are amended. `emit!` still parses.
**Where**: `crates/piperine-lang/std/records.msl`
**Depends on**: T117
**Reuses**: T117
**Requirement**: CC-11 (AC 1, AC 3, AC 4)
**Lane**: K
**Executor**: Opus

**Done when**:

- [ ] `emit_writes_the_records_it_wrote_before` passes under `**Run srcio**`: one model written both ways gives identical records and identical generated Rust
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): emit is an ordinary function with a named variadic`

---

### T119: `finish` and `check` write the reserved streams

**What**: `std/io.msl` declares the intrinsics `finish(failed, msg)` and `check(kind, name, pass, margin)`; `fatal`, `require`, `cover`, `target` call them; the reserved-field table (`types/record.rs:44-55`) goes; the backends find the two streams by declaration through one `ReservedStream` of `piperine-ir` (A5); an `emit` to `sim` or `checks` is `E0603` (S2).
**Where**: `crates/piperine-lang/std/io.msl`
**Depends on**: T118
**Reuses**: T118
**Requirement**: CC-11 (AC 1, AC 3, AC 4)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] `finish_and_check_are_the_writers_of_the_reserved_streams` passes under `**Run srcio**`; the texts `"sim"` and `"checks"` are in one file of `piperine-ir`
- [ ] Twins byte-identical
- [ ] Gate check passes: Golden, and `cargo test -p piperine-api -p piperine-cli`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): finish and check own the reserved streams`

---

### T120: The corpora call `emit`

**What**: Rewrite `emit!("s", a = b)` to `emit("s", a: b)` in the 65 `.msl` files (examples, twins' sources, parity models, e2e fixtures) and in the Rust test strings of every crate: the `!` goes and each field's `=` becomes `:`, found by the old production's spans, never by a textual replace, so no `=` inside a value is touched; a site that wrote `sim` or `checks` directly becomes `finish(..)` or `check(..)`, each listed in the commit body.
**Where**: `crates/piperine-parity/tests`
**Depends on**: T119
**Reuses**: T118, T119
**Requirement**: CC-11 (AC 3, AC 4)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] `grep -rn 'emit!' --include=*.msl .` and the same over `crates/*/tests tests` find nothing but the parse-error tests of T122
- [ ] No golden, twin output or expected record changes
- [ ] Gate check passes: Golden, and `cargo test -p piperine-parity -p piperine-api -p piperine-cli -p piperine --test integrated`

**Tests**: integration
**Gate**: golden

**Commit**: `refactor: models and tests call emit as a function`

---

### T121: The Book and the spec's lists call `emit`

**What**: Rewrite the 98 Book lines (`emit("s", a: b)`) and the remaining spec mentions (`index.md:27,175`, Appendix B's tables and counts, Appendix C `:49,60`, Appendix G `:37`, Part VII §22.6); `tests/book/inventory.tsv` takes the new fingerprints; the Book's prose on "the one output statement" says "function".
**Where**: `docs/book/content.html`
**Depends on**: T120
**Reuses**: T120
**Requirement**: CC-11 (AC 3)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] `every_block_of_the_book_has_one_row` and `every_row_names_an_existing_test` pass under `**Run book**`; no Book listing changes verdict
- [ ] `the_revision_note_names_every_special_case_removed` passes for S1 and S2
- [ ] Gate check passes: Golden, and `cargo test -p piperine --test book`

**Tests**: integration
**Gate**: golden

**Commit**: `docs(book): emit is a function in the book and the spec`

---

### T122: The `emit!` production leaves

**What**: Delete the `emit!` production (`parser/expr.rs:240-300`, `item.rs:489`, `msl.rs:299`) and its AST node; `emit!(` is A.1 rule 4's parse error with the help "write `emit(…)`".
**Where**: `crates/piperine-lang/src/msl/parser`
**Depends on**: T121
**Reuses**: T121
**Requirement**: CC-11 (AC 1)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] `a_bang_form_is_a_parse_error` passes under `**Run variadics**` for `emit!`
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `feat(msl): the emit form is gone from the grammar`

---

### T123: `include_str` is a function; no `!` form remains

**What**: `pub const extern "intrinsic" fn include_str(path: str) -> str;` beside `parse_table`; the production and its 6 `.msl`, 21 Rust-test, 27 spec and 11 Book uses follow; A.1 rules 4 and 5 become design.md's one rule; Appendix B loses the `!` forms row and constructs 44 and 45 (S3).
**Where**: `crates/piperine-lang/std/io.msl`
**Depends on**: T122
**Reuses**: T118
**Requirement**: CC-11 (AC 1, AC 3)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] `a_bang_form_is_a_parse_error` passes whole and `include_str_is_a_std_function` passes under `**Run stddecl**`
- [ ] The lexer has no token and the grammar no production for a `!` form; the revision note names S3
- [ ] Gate check passes: Golden, and `cargo test -p piperine --test book`

**Tests**: integration
**Gate**: golden

**Commit**: `feat(std): include_str is a function and no bang form remains`

---

### Phase 18 tasks: B9 Names: glossary, one module layout, the IR

### T124: The glossary and the rename map

**What**: Review `data/glossary.tsv` (findings.md §1.3 (a); Q2) and write `data/dictionary.txt`; write `.specs/features/code-clarity/renames.tsv` with every declared name of §1.3 (b) and its new name (re-taken by the scan on this commit), the bare type names of §1.3 with their subject, and the module names of §1.3 (c).
**Where**: `.specs/features/code-clarity/renames.tsv`
**Depends on**: T85, T114, T123
**Reuses**: findings.md §1.3
**Requirement**: CC-05
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] Every row of the naming allowances has a row in `renames.tsv` or a glossary term; no new name collides in its scope (a unit test of `Glossary` over the map)
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): the glossary and the rename map`

---

### T125: One module layout

**What**: Every `x/mod.rs` becomes `x.rs` beside `x/` (Q6); the hygiene test `mod_rs_declares_only` is replaced by `no_file_is_named_mod_rs`.
**Where**: `crates/piperine-repository-guards/tests/structure.rs`
**Depends on**: T124
**Reuses**: findings.md §1.10 M3
**Requirement**: CC-02
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `no_file_is_named_mod_rs` passes under `**Run structure**`; no `use` path changes
- [ ] Gate check passes: Quick for every crate that had a `mod.rs`

**Tests**: integration
**Gate**: quick

**Commit**: `refactor: one module layout, no file named mod.rs`

---

### T126: The IR's names

**What**: Rename the declared names of `piperine-ir` by `renames.tsv` (`Expr` → `Expression`, `Stmt` → `Statement`, …) and every use in the workspace.
**Where**: `crates/piperine-ir/src`
**Depends on**: T125
**Reuses**: T124
**Requirement**: CC-05
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `piperine-ir` has no row in the naming allowance; the printed IR text is unchanged (its keywords are not Rust names)
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(ir): spell out the abbreviated names`

---

### Phase 19 tasks: B9 Names, front end

### T127: The names of `msl/`

**What**: Rename by `renames.tsv` the declared names of `piperine-lang/src/msl` (`Kw` → `Keyword`, `Punct` → `Punctuation`, `Lit` → `Literal`, `Pat` → `Pattern`, `Attr` → `Attribute`, `Dir` → `Direction`, `ident` → `identifier`, …) and every use.
**Where**: `crates/piperine-lang/src/msl`
**Depends on**: T126
**Reuses**: T124
**Requirement**: CC-05
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `msl/` has no row in the naming allowance
- [ ] Gate check passes: Quick, and `cargo test -p piperine-api -p piperine-cli`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(msl): spell out the abbreviated names`

---

### T128: The names of `pom/`

**What**: The same for `pom/` (`Decl…` → `Declaration…`, `res` → `resolution`, `krate` → `crate_name`, …); `docs/pom/` follows.
**Where**: `crates/piperine-lang/src/pom`
**Depends on**: T127
**Reuses**: T124
**Requirement**: CC-05
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `pom/` has no row in the naming allowance; `docs/pom/` names existing items
- [ ] Gate check passes: Quick, and `cargo test -p piperine-api -p piperine-cli`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(pom): spell out the abbreviated names`

---

### T129: The names of `types/`

**What**: The same for `types/` (`Ty` → `CheckedType`, `Res` → `ResultType`, `Recv` → `Receiver`, `CoreSig` gone with T114, …).
**Where**: `crates/piperine-lang/src/types`
**Depends on**: T128
**Reuses**: T124
**Requirement**: CC-05
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `types/` has no row in the naming allowance
- [ ] Gate check passes: Quick

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(types): spell out the abbreviated names`

---

### T130: The names of `elab/`

**What**: The same for `elab/` (`Env` → `Environment`, `Deps` → `Dependencies`, `init` → `initial`, …).
**Where**: `crates/piperine-lang/src/elab`
**Depends on**: T129
**Reuses**: T124
**Requirement**: CC-05
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `elab/` has no row in the naming allowance
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(elab): spell out the abbreviated names`

---

### T131: A bare type name takes its subject, front end

**What**: Each type name `piperine-lang` declares more than once or shares with another crate (`Step`, `Walk`, `Scope`, `Ctx`, `Frame`, `Site`, `Shape`, `Value`, `Key`, `Row`, …) takes its subject by `renames.tsv`.
**Where**: `crates/piperine-lang/src`
**Depends on**: T130
**Reuses**: T124
**Requirement**: CC-05 (AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `no_bare_type_name_is_declared_twice` passes with no row for `piperine-lang`
- [ ] Gate check passes: Golden

**Tests**: integration
**Gate**: golden

**Commit**: `refactor(lang): each type name says its subject`

---

### Phase 20 tasks: B9 Names, back end

### T132: The names of `piperine-abi`

**What**: Rename by `renames.tsv` (`ptr`, `buf`, `idx`, … ; `Eval` and `…Spec` stay, Q2) and every use, the transpiler's printed text included; the goldens move by identifier only.
**Where**: `crates/piperine-abi/src`
**Depends on**: T126
**Reuses**: T124
**Requirement**: CC-05, CC-09
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The crate has no row in the naming allowance
- [ ] `git diff --word-diff` of `crates/piperine-transpile/tests/golden/` shows renamed identifiers and nothing else (pasted in the commit body)
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(abi): spell out the abbreviated names`

---

### T133: The names of the two sub-solvers

**What**: The same for `piperine-analog-solver` and `piperine-digital-solver` (`dt`, `h` → `time_step`, `jac` → `jacobian`, `coeffs`, `tol`, one-letter fields → the quantity's name).
**Where**: `crates/piperine-analog-solver/src`
**Depends on**: T132
**Reuses**: T124
**Requirement**: CC-05
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] Both crates have no row in the naming allowance
- [ ] Gate check passes: Quick, and `cargo test -p piperine-solver`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(analog-solver): spell out the abbreviated names`

---

### T134: The names of `piperine-solver`

**What**: The same for `piperine-solver`: `dt_min`, `prev_h` and their kin, the two `AnalysisKind`s of the crate take their subject, `Step`, `Source`, `Held`, `Outcome`, `Bound` take theirs.
**Where**: `crates/piperine-solver/src`
**Depends on**: T133
**Reuses**: T124
**Requirement**: CC-05
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The crate has no row in the naming allowance or in `no_bare_type_name_is_declared_twice`
- [ ] Gate check passes: Quick, and `cargo test -p piperine-api`

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: quick

**Commit**: `refactor(solver): spell out the abbreviated names`

---

### T135: The names of plan, project and numbers

**What**: The same for `piperine-plan` (`Stim` → `Stimulus`, …), `piperine-project` (`dir` → `directory`) and `piperine-numbers`.
**Where**: `crates/piperine-plan/src`
**Depends on**: T134
**Reuses**: T124
**Requirement**: CC-05
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The three crates have no row in the naming allowance
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(plan): spell out the abbreviated names`

---

### T136: The names of transpile, interp, api, cli, parity and the support crates

**What**: The same for the remaining crates. A renamed public item of `piperine-api` is followed by the Book, `examples/` and the tests in this commit (CC-09 AC 2).
**Where**: `crates/piperine-api/src`
**Depends on**: T135
**Reuses**: T124
**Requirement**: CC-05, CC-09 (AC 2)
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] No crate but `piperine-lang` has a row in the naming allowance
- [ ] `every_row_names_an_existing_test` passes under `**Run book**`; `cargo test -p piperine --test book` passes
- [ ] Gate check passes: Golden

**Tests**: integration (the existing targets of the touched crates, unedited)
**Gate**: golden

**Commit**: `refactor(api): spell out the abbreviated names`

---

### Phase 21 tasks: B9 Modules and crates

### T137: Module names say what they hold

**What**: Rename by `renames.tsv`: `elab` → `elaboration`, `rt` → `runtime`, `piperine-solver/src/core` → `circuit`, `deps` → `dependencies`, `diag` → `diagnostic`, `ty` → `checked_type`, `pat` → `pattern`, `iv` → `initial_value`, `num` → `number`, `sens.rs` → `finite_difference_sensitivity.rs`, parser `base.rs` → `tokens.rs`, `entry.rs` → `parse.rs`, `ir/src/{handle,written}.rs` by content (M2). The generated code's `rt::` path follows; goldens move by identifier only.
**Where**: `crates`
**Depends on**: T131, T136
**Reuses**: T124
**Requirement**: CC-02, CC-05
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `no_module_is_named_for_no_concern` passes under `**Run structure**` with no allowance row
- [ ] The golden word-diff holds identifiers only (pasted in the commit body); `CLAUDE.md`'s paths follow
- [ ] Gate check passes: Golden, and `cargo test -p piperine-api -p piperine-cli`

**Tests**: integration
**Gate**: golden

**Commit**: `refactor: module names say what they hold`

---

### T138: `piperine-interp` becomes `piperine-interpreter`

**What**: Rename the crate (directory, every `Cargo.toml`, every `use`), `repoint` with `rename-crate` over every feature and this file, the Book's organization chapter, `CLAUDE.md`, `AGENTS.md`, `README.md`, `GAPS.md`, `ROADMAP.md` (Q3).
**Where**: `crates/piperine-interpreter`
**Depends on**: T137
**Reuses**: T4's `rename-crate`
**Requirement**: CC-05
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `grep -rw piperine-interp` and `piperine_interp` find nothing outside `proofs*.md` and `retired.tsv`
- [ ] Gate check passes: Ledger, and `cargo test -p piperine-interpreter -p piperine-cli --test twins`

**Tests**: integration
**Gate**: ledger

**Commit**: `refactor: rename the interpreter crate`

---

### T139: `piperine-lang` becomes `piperine-language`

**What**: The same for `piperine-lang` (spec Assumption 15); this file's `**Build**` and `Run` lines follow.
**Where**: `crates/piperine-language`
**Depends on**: T138
**Reuses**: T138
**Requirement**: CC-05
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `grep -rw piperine-lang` and `piperine_lang` find nothing outside `proofs*.md` and `retired.tsv`
- [ ] Gate check passes: Ledger, and `cargo test -p piperine-language`

**Tests**: integration
**Gate**: ledger

**Commit**: `refactor: rename the language crate`

---

### T140: The naming guard is strict

**What**: Delete the naming allowances (every crate is empty); add `no_renamed_name_survives_in_a_comment` over `renames.tsv`'s old names in comments, doc comments and diagnostics' prose.
**Where**: `crates/piperine-repository-guards/tests/naming.rs`
**Depends on**: T139
**Reuses**: T7
**Requirement**: CC-05 (AC 1), CC-10 (AC 4)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `every_declared_name_is_words_or_glossary`, `no_bare_type_name_is_declared_twice` and `no_renamed_name_survives_in_a_comment` pass under `**Run naming**` with no allowance
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): names are words or glossary terms everywhere`

---

### Phase 22 tasks: B10 Words, front end

### T141: Doc comments: `msl/` and `pom/`

**What**: A `///` on every struct, enum, trait, alias, method and function of the two directories, and on each field and variant whose meaning its name and type do not give: what it is for; for a method, what it returns or changes; a non-evident block says why and cites its rule (CC-10 AC 3).
**Where**: `crates/piperine-language/src/pom`
**Depends on**: T140
**Reuses**: the scan's doc flag
**Requirement**: CC-10
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] The two directories have no row in the documentation allowance
- [ ] No comment restates its line (the Verifier reads a sample)
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(language): comment the syntax and the object model`

---

### T142: Doc comments: `types/`

**What**: The same for `types/`.
**Where**: `crates/piperine-language/src/types`
**Depends on**: T141
**Reuses**: T141
**Requirement**: CC-10
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `types/` has no row in the documentation allowance
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(language): comment the type checker`

---

### T143: Doc comments: `elaboration/`; the crate denies missing docs

**What**: The same for `elaboration/`, `refusal` and the crate root; `#![deny(missing_docs)]` in `lib.rs`.
**Where**: `crates/piperine-language/src/elaboration`
**Depends on**: T142
**Reuses**: T141
**Requirement**: CC-10 (AC 1, AC 2)
**Lane**: F
**Executor**: Sonnet

**Done when**:

- [ ] `piperine-language` has no row in the documentation allowance and builds under `#![deny(missing_docs)]`
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(language): comment the elaboration and deny missing docs`

---

### Phase 23 tasks: B10 Words, back end

### T144: Doc comments: `piperine-ir`

**What**: The same for `piperine-ir` (139 methods, 32 types, 393 fields and variants at the inventory; `model/` first); `#![deny(missing_docs)]`.
**Where**: `crates/piperine-ir/src`
**Depends on**: T140
**Reuses**: the scan's doc flag
**Requirement**: CC-10
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The crate has no row in the documentation allowance and denies missing docs
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(ir): comment the object model and deny missing docs`

---

### T145: Doc comments: abi, plan, project, numbers

**What**: The same for `piperine-abi` (the 89 role and flag constants of `raw/constants.rs` among them), `piperine-plan`, `piperine-project`; `#![deny(missing_docs)]` in each (`piperine-numbers` has it).
**Where**: `crates/piperine-abi/src`
**Depends on**: T144
**Reuses**: T144
**Requirement**: CC-10
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The four crates have no row in the documentation allowance and deny missing docs
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(abi): comment the contract and deny missing docs`

---

### T146: Doc comments: the three solver crates

**What**: The same for `piperine-analog-solver`, `piperine-digital-solver` and `piperine-solver` (whose `lib.rs` has no doc at all today); `#![deny(missing_docs)]` in each.
**Where**: `crates/piperine-solver/src`
**Depends on**: T145
**Reuses**: T144
**Requirement**: CC-10
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] The three crates have no row in the documentation allowance and deny missing docs
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(solver): comment the solvers and deny missing docs`

---

### T147: Doc comments: transpile, interpreter, api, cli, parity, the support crates

**What**: The same for the remaining crates; `#![deny(missing_docs)]` in each.
**Where**: `crates/piperine-api/src`
**Depends on**: T146
**Reuses**: T144
**Requirement**: CC-10
**Lane**: K
**Executor**: Sonnet

**Done when**:

- [ ] No crate but `piperine-language` has a row in the documentation allowance; every crate denies missing docs
- [ ] Gate check passes: Quick

**Tests**: none
**Gate**: quick

**Commit**: `docs(api): comment the host library and deny missing docs`

---

### Phase 24 tasks: B10 Structure words and the last guards

### T148: A comment cites a rule, not a feature

**What**: Replace each feature requirement id and task id in a doc comment by the spec rule (`§n`) where one exists (Q18); `MD-n` stays; an id with no spec rule is listed in `allowances/<crate>/requirement_tags.tsv` with the reason. Adds `no_feature_or_task_id_in_a_doc_comment`.
**Where**: `crates/piperine-repository-guards/tests/documentation.rs`
**Depends on**: T143, T147
**Reuses**: findings.md §1.3, last paragraph
**Requirement**: CC-10 (AC 3)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `no_feature_or_task_id_in_a_doc_comment` passes under `**Run documentation**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs: comments cite the spec rule in place of feature ids`

---

### T149: Every `lib.rs`: purpose, module map, data path

**What**: Each crate's `lib.rs` opens with its purpose in the macro flow (or the crates it supports), its module map and its data path (input type → stages → output type), and the entry point calls the stages in that order; the stale texts of `piperine-parity` and `piperine-ir` are corrected. Adds `every_crate_states_its_data_path_with_existing_types`; the `structure` allowances are deleted.
**Where**: `crates/piperine-repository-guards/tests/structure.rs`
**Depends on**: T148
**Reuses**: `piperine-transpile`'s and `piperine-plan`'s crate docs
**Requirement**: CC-01, CC-08 (AC 1)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `every_crate_states_its_purpose_and_module_map` and `every_crate_states_its_data_path_with_existing_types` pass under `**Run structure**` with no allowance
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs: every crate states its purpose, map and data path`

---

### T150: `ARCHITECTURE.md` and the Book's organization chapter

**What**: Write `ARCHITECTURE.md`: the whole path `.msl → POM → types → elaboration → IR → plan → transpile/interpreter → ABI → solver → api → cli` with the owner type of each step, the crates and their one purpose, the guards. The Book's "How Piperine is organized" and `CLAUDE.md`'s pipeline and crate table say the same names. Adds `the_architecture_document_names_every_step_and_owner`.
**Where**: `ARCHITECTURE.md`
**Depends on**: T149
**Reuses**: `CLAUDE.md` "Pipeline"
**Requirement**: CC-08 (AC 2)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `the_architecture_document_names_every_step_and_owner` passes under `**Run structure**`: every type the document names exists in the crate it says
- [ ] `every_block_of_the_book_has_one_row` passes under `**Run book**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs: the architecture, one path with an owner per step`

---

### T151: The doc guard is strict

**What**: Delete the documentation allowances; `every_item_is_documented` holds for every private and public item outside test code.
**Where**: `crates/piperine-repository-guards/tests/documentation.rs`
**Depends on**: T150
**Reuses**: T7
**Requirement**: CC-10 (AC 1, AC 2)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `every_item_is_documented` and `every_crate_denies_missing_docs` pass under `**Run documentation**` with no allowance
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): every item carries a doc comment`

---

### Phase 25 tasks: B11 Close

### T152: The allowances hold only the language's exceptions

**What**: Every allowance file is gone or holds only what the spec's edge cases name (`main`, the `extern "C"` functions, generated-code `#[allow]` strings, requirement tags with no spec rule); every worklist row has a verdict; `owners.tsv` matches the tree.
**Where**: `crates/piperine-repository-guards/allowances`
**Depends on**: T151
**Reuses**: T6, T8
**Requirement**: CC-04, CC-06, CC-07
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `no_free_function_outside_the_language_exceptions`, `an_inherent_impl_lives_with_its_type`, `every_owner_of_the_design_exists_and_has_its_methods`, `no_allow_hides_a_design_problem` pass under `**Run ownership**`
- [ ] The four `worklists` tests pass with no open row
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(guards): the allowances are the language's exceptions`

---

### T153: The numbers after

**What**: Write `suite-report.md` `## After` the way T8 took `## Before` (the census and the run seconds per crate, measured in the one full-suite run of T155), with the difference per crate; add `the_suite_report_has_before_and_after`.
**Where**: `.specs/features/code-clarity/suite-report.md`
**Depends on**: T152
**Reuses**: T8
**Requirement**: CC-12 (AC 6)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `the_suite_report_has_before_and_after` passes under `**Run suite**`: both tables, the same rows, test count, target count and run time
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): the suite before and after code-clarity`

---

### T154: Tracking files say what is true

**What**: `CLAUDE.md` and `AGENTS.md` (crates, pipeline, tests, the guards crate, gate commands), `ROADMAP.md` (code-clarity closed; `spec-closure` receives the refusal table, S9 and the items logged on this file's Handoff), `GAPS.md` (rows this feature closed leave), `.specs/STATE.md` Handoff, spec.md's traceability table (12 requirements, each mapped to its tasks).
**Where**: `CLAUDE.md`
**Depends on**: T153
**Reuses**: -
**Requirement**: CC-01
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] `cargo test -p piperine-repository-guards` passes (the structural tracking guards among them)
- [ ] spec.md's coverage line reads 12 total, 12 mapped
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs: tracking files after code-clarity`

---

### T155: Final verification: the full suite, once

**What**: Run the Final gate: `cargo test --workspace`; `cargo test -p piperine-parity -- --nocapture` with ngspice; `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`; the buck targets by name; `check_proofs.py code-clarity`; `check_proofs.py <feature>` for every feature that has `## Checks`. Record the results on the Handoff; a red proof of another feature is fixed by a `repoint` row, never by editing its claim.
**Where**: `.specs/features/code-clarity/tasks.md`
**Depends on**: T154
**Reuses**: the skill's `check_proofs.py`
**Requirement**: CC-09, CC-12 (AC 5)
**Lane**: single
**Executor**: Sonnet

**Done when**:

- [ ] Every command exits 0; the 7 twin goldens and the transpile goldens differ from the base only as design.md "What may move" lists (`git diff <base> --stat` of the golden directories in the commit body)
- [ ] No `ToleranceContract` or `contract.rs` line changed since the base
- [ ] Gate check passes: Final

**Tests**: integration
**Gate**: final

**Commit**: `test: code-clarity final verification`

---

## Checks

**C1** - Every crate's `lib.rs` opens with its purpose in the macro flow and its module map, and a test fails on a crate without it. (CC-01, AC 1; T6, T149)
Proof: run structure | test every_crate_states_its_purpose_and_module_map ... ok

**C2** - Each module holds one concern named by the module; none is `utils`, `helpers`, `misc`, `common`, `core`, `base` or `entry` outside test support, and no file is named `mod.rs`. (CC-02, AC 1; T125, T137)
Proof: run structure | test no_module_is_named_for_no_concern ... ok
Proof: run structure | test no_file_is_named_mod_rs ... ok

**C3** - The dependency graph is the documented one, and no type the inventory found in two crates is declared twice. (CC-03, AC 1; T5, T29, T51, T52)
Proof: run boundaries | test the_dependency_graph_is_the_documented_one ... ok
Proof: run boundaries | test no_type_of_the_inventory_is_defined_in_two_crates ... ok

**C4** - No non-test module defines a free `fn` other than `main` and the `extern "C"` functions of the ABI, and a new one fails the guard. (CC-04, AC 1; T6, T152)
Proof: run ownership | test no_free_function_outside_the_language_exceptions ... ok

**C5** - Each function the inventory moved is a method of the owner the design names. (CC-04, AC 2; T6, T152)
Proof: run ownership | test every_owner_of_the_design_exists_and_has_its_methods ... ok

**C6** - No declared type, method, field, variant, const or module name holds a part that is neither a word nor a glossary term. (CC-05, AC 1; T7, T140)
Proof: run naming | test every_declared_name_is_words_or_glossary ... ok

**C7** - A type has one purpose: its inherent `impl` blocks live with it (`Checker` and `Elaboration` are no longer spread over 54 and 67 files), and no bare type name is declared twice. (CC-05, AC 2; T76, T99, T131, T134)
Proof: run ownership | test an_inherent_impl_lives_with_its_type ... ok
Proof: run naming | test no_bare_type_name_is_declared_twice ... ok

**C8** - Every function over 60 lines of the inventory was split or kept with a written reason, and no `#[allow]` hides a design problem; there is no length limit (spec Assumption 9). (CC-06, AC 1; T8, T152)
Proof: run worklists | test every_long_function_of_the_inventory_has_a_verdict ... ok
Proof: run ownership | test no_allow_hides_a_design_problem ... ok

**C9** - A trait with one implementor is removed or carries its reason, and logic that was repeated has one owner: `P0001` is worded in one place and each refused form has one row. (CC-07, AC 1; T15, T21, T22, T32)
Proof: run worklists | test every_single_implementor_trait_is_removed_or_justified ... ok
Proof: run refusals | test p0001_is_worded_in_one_place ... ok
Proof: run refusals | test every_refused_form_has_a_row ... ok
Proof: run refusals | test the_unswept_count_only_shrinks ... ok

**C10** - Each crate's `lib.rs` states its data path with types that exist in the crate. (CC-08, AC 1; T149)
Proof: run structure | test every_crate_states_its_data_path_with_existing_types ... ok

**C11** - `ARCHITECTURE.md` shows the whole path from `.msl` to the cli with the owner type of each step, and each named type exists. (CC-08, AC 2; T150)
Proof: run structure | test the_architecture_document_names_every_step_and_owner ... ok

**C12** - The twin goldens, the transpile goldens and the Book's rows are what they were, outside design.md "What may move"; no tolerance changed. (CC-09, AC 1; T155)
Proof: run twins | test the_two_backends_print_the_same_bytes ... ok
Proof: run golden | test the_five_m1_bodies_print_as_the_golden_files ... ok
Proof: run book | test every_block_of_the_book_has_one_row ... ok
Proof: `cargo test -p piperine-parity --test parity`

**C13** - A renamed public item of `piperine-api` is followed by the Book, the examples and the tests. (CC-09, AC 2; T136)
Proof: run book | test every_row_names_an_existing_test ... ok
Proof: `cargo test -p piperine --test book`

**C14** - Every struct, enum, trait, alias, method and function outside test code carries a doc comment, public or private. (CC-10, AC 1; T151)
Proof: run documentation | test every_item_is_documented ... ok

**C15** - Every crate builds under `#![deny(missing_docs)]`. (CC-10, AC 2; T143, T147)
Proof: run documentation | test every_crate_denies_missing_docs ... ok

**C16** - A doc comment cites a spec rule or an `MD-n`, not a feature or task id. (CC-10, AC 3; T148)
Proof: run documentation | test no_feature_or_task_id_in_a_doc_comment ... ok

**C17** - No renamed name survives in a comment. (CC-10, AC 4; T140)
Proof: run naming | test no_renamed_name_survives_in_a_comment ... ok

**C18** - `P0001` does not name a milestone. (CC-09 as amended by spec Assumption 8; T16)
Proof: run refusals | test p0001_does_not_name_a_milestone ... ok

**C19** - A user function declares a named variadic parameter and is called with fields, by the rules `emit` follows, in the spelling every named argument has (`name: value`); an analysis kind is written `Analysis::Dc`; no `!` form parses; `Option`, `Range` and `Iterator` are declarations of `std`; each special case kept has its written reason. (CC-11, AC 1; T102, T110, T111, T116, T117, T122, T123)
Proof: run variadics | test a_user_function_declares_and_calls_a_named_variadic ... ok
Proof: run variadics | test a_user_wrapper_forwards_its_fields ... ok
Proof: run variadics | test a_bang_form_is_a_parse_error ... ok
Proof: run stddecl | test option_range_and_iterator_are_std_declarations ... ok
Proof: run stddecl | test every_kept_special_case_has_its_written_reason ... ok
Proof: run stddecl | test an_analysis_kind_is_written_with_its_enum ... ok
Proof: run variadics | test an_optional_parameter_wins_over_a_field_of_its_name ... ok

**C20** - Every builtin the user can name is declared in `std`; the compiler finds none by its spelling; every method of Appendix H resolves to a `std` declaration. (CC-11, AC 2; T114, T123)
Proof: run stddecl | test every_builtin_is_declared_in_std ... ok
Proof: run stddecl | test no_std_item_is_found_by_its_spelling ... ok
Proof: run stddecl | test every_method_of_appendix_h_is_declared_in_std ... ok
Proof: run stddecl | test include_str_is_a_std_function ... ok

**C21** - The spec's revision note names each special case removed, and the Book's rows follow. (CC-11, AC 3; T114, T121)
Proof: run specstructure | test the_revision_note_names_every_special_case_removed ... ok
Proof: run book | test every_row_names_an_existing_test ... ok

**C22** - `emit` as a function writes the records `emit!` wrote, and `finish` and `check` write `sim` and `checks` as before: no run computes anything else. (CC-11, AC 4; T118, T119)
Proof: run srcio | test emit_writes_the_records_it_wrote_before ... ok
Proof: run srcio | test finish_and_check_are_the_writers_of_the_reserved_streams ... ok
Proof: run srcio | test a_user_wrapper_over_emit_writes_the_record ... ok

**C23** - The inventory measured the suite: targets, tests, run time, duplicates, obsolete pins, uncited tests, duplicated helpers. (CC-12, AC 1; done in `findings.md` §3)
Proof: `grep -q '^## 3. The test suite (CC-12)' .specs/features/code-clarity/findings.md`

**C24** - Where two tests proved one rule one is kept, and a removed prose pin is recorded with the check it retired. (CC-12, AC 2; T9 to T14, T90)
Proof: run worklists | test every_duplicate_of_the_inventory_has_a_verdict ... ok
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C25** - No crate has more targets than the plan (75 link steps, from 375), and each target opens with its scope header. (CC-12, AC 3; T35 to T39, T64 to T68, T86 to T89)
Proof: run suite | test no_crate_has_more_targets_than_its_plan ... ok
Proof: run suite | test every_integration_target_declares_its_scope ... ok

**C26** - Test helpers are methods of fixture types and live once: no free helper, no support shared by `#[path]`, no `#[allow]` in test code. (CC-12, AC 4; T34, T63, T66, T91)
Proof: run ownership | test no_free_helper_function_in_test_code ... ok
Proof: run suite | test no_test_support_is_shared_by_path ... ok
Proof: run suite | test no_allow_in_test_code ... ok

**C27** - Every proof of every feature and every Book inventory row names a test that exists, and the parity oracles are not reduced. (CC-12, AC 5; T4, T155)
Proof: run ledger | test every_proof_names_a_test_that_exists ... ok
Proof: run ledger | test every_inventory_row_names_a_test_that_exists ... ok
Proof: run ledger | test the_parity_oracles_are_not_reduced ... ok

**C28** - The suite's test count, target count and test run time are reported before and after; build time is not measured (user, 2026-11-26: "O build não precisa de métrica... Tá de boa"; "O importante é menor tempo de testes (conjunto mais bem dimensionado deles)"). (CC-12, AC 6; T8, T153)
Proof: run suite | test the_suite_report_has_before_and_after ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** the suite report has no build-time column; test run time is the key metric (user 2026-11-26: "O build não precisa de métrica... Tá de boa"; "Tempo e build é muito relativo... O importante é menor tempo de testes (conjunto mais bem dimensionado deles)"). The before `Run s` values come from findings.md §3.1 (2 202 s); T155 measures the after value in its single full-suite run.
**Base:** `main` at 93c831ffc holds `generics` phase E (a8968de11) and no other feature is landing code; branch `feature/code-clarity` starts there (spec Assumption 2). The decision is MD-85 in `.specs/STATE.md`.
**Questions answered:** the five of design.md are answered (spec Assumptions rows 13 to 17): OQ1 `name: value` fields and forwarding; OQ2 `RecordValue` and `RecordFields`; OQ3 `piperine-language` and `piperine-interpreter`; OQ4 `Analysis::Dc` and `is_static()`; OQ5 the serde derives stay.
**Progress:** T1 to T8 done.
**Dispatched:** none
**Walked:** none - the feature has no screen; each story's Independent Test is a guard target of `## Checks`.

**Open questions:** none. OQ1 to OQ5 of design.md were answered by the user on 2026-11-25 (spec Assumptions rows 13 to 17) and are in the tasks: `name: value` fields and forwarding (T115 to T117, T120), `RecordValue` and `RecordFields` (T116), `piperine-language` (T139), `Analysis::Dc` and `is_static()` (T105, T106), the serde derives and the JSON export stay (no task).

---

## Phase Execution Map

```
Phase  1 [single]  T1 … T8  (8 tasks)
Phase  2 [  T   ]  T9 … T14  (6 tasks)
Phase  3 [  F   ]  T15 … T22  (8 tasks)
Phase  4 [  K   ]  T23 … T32  (10 tasks)
Phase  5 [  T   ]  T33 … T40  (8 tasks)
Phase  6 [  F   ]  T41 … T46  (6 tasks)
Phase  7 [  F   ]  T47 … T52  (6 tasks)
Phase  8 [  K   ]  T53 … T62  (10 tasks)
Phase  9 [  T   ]  T63 … T68  (6 tasks)
Phase 10 [  F   ]  T69 … T76  (8 tasks)
Phase 11 [  K   ]  T77 … T85  (9 tasks)
Phase 12 [  T   ]  T86 … T91  (6 tasks)
Phase 13 [  F   ]  T92 … T99  (8 tasks)
Phase 14 [single]  T100 … T102  (3 tasks)
Phase 15 [  F   ]  T103 … T109  (7 tasks)
Phase 16 [  F   ]  T110 … T114  (5 tasks)
Phase 17 [  K   ]  T115 … T123  (9 tasks)
Phase 18 [single]  T124 … T126  (3 tasks)
Phase 19 [  F   ]  T127 … T131  (5 tasks)
Phase 20 [  K   ]  T132 … T136  (5 tasks)
Phase 21 [single]  T137 … T140  (4 tasks)
Phase 22 [  F   ]  T141 … T143  (3 tasks)
Phase 23 [  K   ]  T144 … T147  (4 tasks)
Phase 24 [single]  T148 … T151  (4 tasks)
Phase 25 [single]  T152 … T155  (4 tasks)
```

A batch for one worker is one phase (3 to 10 tasks); two short consecutive phases of one lane may share a worker (6 and 7; 15 and 16; 18 and 19; 21 and 22; 24 and 25). A phase is never split across workers.

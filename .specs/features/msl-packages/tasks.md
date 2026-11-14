# MSL Packages Tasks

**Approved**: 2026-11-14 - user: "Dispara o fable para especificar o project e despachar"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-packages/design.md`
**Status**: Draft
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^warning')`
**Run spec**: `cargo test -p piperine-lang --test spec_text`
**Run errors**: `cargo test -p piperine-lang --test msl_errors`
**Run crates**: `cargo test -p piperine-lang --test pom_crates`
**Run docs**: `cargo test -p piperine-lang --test pom_docs`
**Run manifest**: `cargo test -p piperine-project --test manifest`
**Run paths**: `cargo test -p piperine-project --test paths`
**Run git**: `cargo test -p piperine-project --test git`
**Run lock**: `cargo test -p piperine-project --test lock`
**Run project**: `cargo test -p piperine-project --test project`

Executors: Sonnet sub-agents, one commit per task. Verifier: Opus. Work in `/home/keslarek/Git/piperine-msl-spec`, branch `feature/msl-packages`. Do not set `CARGO_TARGET_DIR` (the environment provides it). Never run `cargo clean`. Never pass `--workspace`: only `-p piperine-lang` and `-p piperine-project`; the other crates do not build and stay that way. `CLAUDE.md` still describes the retired Verilog-AMS pipeline; for this feature the facts are in `spec.md` and `design.md`.

`piperine-project` does not build when this feature starts (it imports the deleted `piperine_lang::SourceMap`), so the `**Build**` line covers `piperine-lang` only. From T15 on, every task also runs `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')` before its commit; C117 holds the feature to it at `HEAD`.

Rules every task follows:

- Tests first: write the tests the task's checks cite, run them red, then implement. A test function has exactly the name its check cites.
- A test asserts the spec's outcome on input written in the test (source text, a manifest, a repository built in a `tempdir`, or the text of `docs/spec/`); it never restates the implementation.
- A new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. New targets: `pom_crates` (T6), `project` (T15), `manifest` (T16), `paths` (T19), `git` (T22), `lock` (T23); `tests/support/mod.rs` of `piperine-project` is `#[path]`-included, never a target.
- An existing test is only strengthened. The one exception: a task's **Existing tests** line names an assertion that a decision of `spec.md` moves. Then exactly that assertion changes and the commit body names the test and the decision. An existing test that goes red for any other reason: stop and report, do not edit it.
- `docs/spec/` follows `.specs/features/msl-spec/STYLE.md`: numbered rules, `> **Resolution.**` notes, error tables `Code | Condition | Phase`, sentences under 30 words, no history. The content of §3.5 and §3.6 is fixed in `design.md` ("Specification text").
- A spec edit lands where its guard can pass (MD-57): a row of the removed-forms table of B.3.6 travels with its parser change (T4); a `rust` fence must parse and round-trip in the commit that adds it.
- A public POM type gets its `### ` heading and a public reflection method its row in `docs/pom/` in the task that adds it (`pom_docs` enforces both).
- No macro is written (`macro_rules!`, proc-macros): data and plain functions. Derives are fine. Storage types of the POM keep `Debug, Clone, PartialEq, Serialize, Deserialize`.
- Only lowering creates POM nodes; resolution writes slots and side tables (MD-54). No declaration of one crate is a child of another crate's (navigability rule). No POM function panics on any input.
- `piperine-lang` gains no dependency. No test contacts the network.
- Zero warnings in both crates at every commit that touches them.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Where the tests are": a target lives in the crate it exercises, one concern per target, a `//!` header; "Build and test": zero warnings), `tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope `allow`; it cannot run today because the root crate does not build, so its rules are applied by hand), `.specs/STATE.md` MD-57 (spec edits and their guards), the fifteen existing targets of `crates/piperine-lang/tests/` (integration tests over source text and over `docs/spec/`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Specification text (`docs/spec/*.md`) | integration | one assertion per decided wording: PKG-01 AC 1 to 8 and 10, PKG-02 | `crates/piperine-lang/tests/spec_text.rs` | run spec |
| Parser, removed forms (`msl/parser/item.rs`, `base.rs`) | integration | PKG-01 AC 9: code, message, help, one diagnostic | `crates/piperine-lang/tests/msl_errors.rs` | run errors |
| Crate graph (`pom/crates.rs`) | integration | PKG-03: every `GraphError`, each with the graph unchanged | `crates/piperine-lang/tests/pom_crates.rs` | run crates |
| Std and the prelude (`pom/stdlib.rs`, `std/*.msl`) | integration | PKG-06: each rule with its positive and its negative case | `crates/piperine-lang/tests/pom_crates.rs` | run crates |
| Crates in the design, lowering per crate (`pom/design.rs`, `pom/lower/`) | integration | PKG-04, PKG-07 AC 1 and 2: memory and disk, an unreadable root, the authored tree of every crate | `crates/piperine-lang/tests/pom_crates.rs` | run crates |
| Resolution across crates (`pom/resolve/`) | integration | PKG-05: all sixteen cases, each `E0161` and `E0163` counted | `crates/piperine-lang/tests/pom_crates.rs` | run crates |
| Reflection (`pom/reflect/`) | integration | PKG-07 AC 3 to 7 | `crates/piperine-lang/tests/pom_crates.rs` | run crates |
| Serialization (`pom/check.rs`) | integration | PKG-08: the round trip and each broken link | `crates/piperine-lang/tests/pom_crates.rs` | run crates |
| Reference docs (`docs/pom/`) | integration | PKG-09; the existing heading, method, fence and nav guards | `crates/piperine-lang/tests/pom_docs.rs`, doctests of `pom::guide` | run docs |
| Manifest reader (`piperine-project` `error.rs`, `manifest.rs`) | integration | PKG-10: every key, every fault with its span, the rendered report | `crates/piperine-project/tests/manifest.rs` | run manifest |
| Resolver (`resolver.rs`) | integration | PKG-11: relative paths at depth, shared packages, each of `E0172` to `E0175`, the eight requirement rows on both sides | `crates/piperine-project/tests/paths.rs` | run paths |
| Git dependencies (`git.rs`, `resolver.rs`) | integration | PKG-12: default branch, branch, tag, rev, failures, one and two commits; local repositories only | `crates/piperine-project/tests/git.rs` | run git |
| Lockfile (`lockfile.rs`) | integration | PKG-13: written, honoured, updated, pruned, invalid, offline | `crates/piperine-project/tests/lock.rs` | run lock |
| Entry point and hygiene (`lib.rs`, `Cargo.toml`) | integration | PKG-14, PKG-15: the loaded design, both failure kinds, the retired surface, the code guard | `crates/piperine-project/tests/project.rs` | run project |
| Data-only files (the placeholder `std/*.msl`, `Cargo.toml` edits) | none | build gate only | - | build |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | after a task that touches one layer and cannot move another target (spec text, the crate graph) | the task's `Run <name>` line(s); a task that edits `docs/spec/` adds `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test pom_docs --test msl_errors` |
| Full | after a task that changes the POM, the parser or `piperine-project` | `cargo test -p piperine-lang` (T4, T7 to T14); `cargo test -p piperine-project` (T15 to T25) |
| Build | every task, before its commit | the `**Build**` line above; from T15 on also `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')` |

---

## Execution Plan

Phases are ordered and run sequentially; tasks within a phase execute in order. Each phase starts from the last task of the one before.

### Phase 1: Specification: crates and packages

```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: Front end: a design over a crate graph

```
T5 → T6 → T7 → T8 → T9 → T10 → T11
```

### Phase 3: Front end: reflection, serialization, docs

```
T11 → T12 → T13 → T14
```

### Phase 4: Project: cut down, manifest

```
T14 → T15 → T16 → T17 → T18
```

### Phase 5: Project: resolution, git, lockfile, entry point

```
T18 → T19 → T20 → T21 → T22 → T23 → T24 → T25
```

---

## Task Breakdown

### T1: §3.5 Crates

**What**: A new section `### 3.5 Crates` in Part I with the eleven rules, the two `Resolution` notes, the error table (the defining row of `E0163` and the section-keyed rows) and a short example fence of `design.md` ("Specification text"); the lead of §3.4 points at §3.5 and §3.6; §3.4.2 defines the prelude as the `pub` items of `std::prelude`; the `E0161` row names a first segment that is no dependency; "Terms defined here" of Part I gains crate, crate root, dependency name. MD-58 and MD-59 are appended to `.specs/STATE.md` from `design.md`.
**Where**: `docs/spec/part_i_values.md` (+ `crates/piperine-lang/tests/spec_text.rs`, `.specs/STATE.md`)
**Depends on**: None
**Reuses**: `spec_text.rs` helpers `spec`, `section`, `table_rows`, `one_line`, `s34`
**Requirement**: PKG-01

**Done when**:

- [x] Test `s35_defines_crate_root_and_dependency_name` passes in `spec_text.rs` (C1: PKG-01 AC 1)
- [x] Test `s35_orders_the_first_segment_lookup` passes in `spec_text.rs` (C2: PKG-01 AC 2)
- [x] Test `s35_local_names_win_in_paths_and_clash_in_use` passes in `spec_text.rs` (C3: PKG-01 AC 3)
- [x] Test `s35_crate_and_super_are_per_crate` passes in `spec_text.rs` (C4: PKG-01 AC 4)
- [x] Test `s35_only_pub_crosses_a_crate` passes in `spec_text.rs` (C5: PKG-01 AC 5)
- [x] Test `std_is_implicit_and_the_prelude_is_std_prelude` passes in `spec_text.rs` (C6: PKG-01 AC 6)
- [x] Test `e0161_row_covers_an_unknown_crate` passes in `spec_text.rs` (C7: PKG-01 AC 7)
- [x] Test `e0163_is_defined_in_3_5` passes in `spec_text.rs` (C10: PKG-01 AC 10)
- [x] §3.5 follows `STYLE.md`: numbered rules `§3.5.n`, sentences under 30 words, each new term bold once
- [x] Any `rust` fence added parses today (`use util::cells::Cell;` is plain Rust path syntax); no fence uses `extern crate`
- [x] `E0163` is the one new defining row, of phase `elaboration`; the `E0161` row is reworded in place
- [x] MD-58 and MD-59 are in `.specs/STATE.md` with the text of `design.md`
- [x] Gate passes: run spec; `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test pom_docs --test msl_errors`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): §3.5 crates, dependency names and the implicit std`

---

### T2: The `package` phase; §3.6 layout and manifest keys

**What**: §0.3 lists the phase `package` (five phases; rule 3 rejects on it). A new section `### 3.6 Packages` with rules 1 to 5 of `design.md`: the manifest file, the crate root `src/lib.msl`, the extension `.msl`, the `mod` layout, the closed key table, the name rule and the version form; a `toml` example fence.
**Where**: `docs/spec/part_i_values.md` (+ `docs/spec/index.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T1
**Reuses**: T1 helpers; the key table of `design.md`
**Requirement**: PKG-02

**Done when**:

- [x] Test `phase_package_is_listed` passes in `spec_text.rs` (C11: PKG-02 AC 1)
- [x] Test `s36_fixes_the_package_layout` passes in `spec_text.rs` (C12: PKG-02 AC 2)
- [x] Test `s36_lists_every_manifest_key` passes in `spec_text.rs` (C13: PKG-02 AC 3)
- [x] Test `s36_names_are_identifiers` passes in `spec_text.rs` (C14: PKG-02 AC 4)
- [x] The example manifest is a `toml` fence (the corpus targets read `rust` fences only)
- [x] "Terms defined here" gains package and manifest
- [x] Gate passes: run spec; `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test pom_docs --test msl_errors`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): the package phase; §3.6 layout and manifest keys`

---

### T3: §3.6 dependencies, requirements, lockfile, error table

**What**: §3.6 rules 6 to 14 of `design.md`: the two sources, the three git references, `package`, the `version` requirement table, one crate per located package, no cycle, no other way to find a crate, the lockfile and its format; the error table `E0170` to `E0176` of phase `package`.
**Where**: `docs/spec/part_i_values.md` (+ `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T2
**Reuses**: T1 helpers; `every_error_code_is_defined_exactly_once` (stays green: each code has one defining row)
**Requirement**: PKG-02

**Done when**:

- [x] Test `s36_states_the_dependency_sources` passes in `spec_text.rs` (C15: PKG-02 AC 5)
- [x] Test `s36_gives_the_version_requirement_table` passes in `spec_text.rs` (C16: PKG-02 AC 6)
- [x] Test `s36_has_no_search_path_and_no_cycle` passes in `spec_text.rs` (C17: PKG-02 AC 7)
- [x] Test `s36_states_the_lockfile_rule` passes in `spec_text.rs` (C18: PKG-02 AC 8)
- [x] Test `s36_defines_the_package_codes` passes in `spec_text.rs` (C19: PKG-02 AC 9)
- [x] Every row of the error table ends in `| package |`; none ends in `| parse |`
- [x] "Terms defined here" gains dependency and lockfile
- [x] §3.5's section-keyed row for `E0174` now has its defining row
- [x] Gate passes: run spec; `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test pom_docs --test msl_errors`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): §3.6 dependencies, version requirements, the lockfile, E0170 to E0176`

---

### T4: `extern crate` is a removed form

**What**: A row `extern crate` items in the removed-forms table of B.3.6 (`E0900`; write instead: name the dependency under `[dependencies]`), the matching `RemovedForm` row, and a parser arm that reports it once at `extern` and skips to the `;`.
**Where**: `crates/piperine-lang/src/msl/parser/item.rs` (+ `src/msl/parser/base.rs`, `docs/spec/appendix_b_grammar.md`, `tests/spec_text.rs`, `tests/msl_errors.rs`)
**Depends on**: T3
**Reuses**: `skipped(first, row)` and `removed(row, span)` in `item.rs` / `base.rs`; the removed-form fixtures of `msl_errors.rs`
**Requirement**: PKG-01
**Existing tests**: `spec_text::REMOVED_FORMS` gains the 21st row and `msl_errors`' removed-form fixtures gain its entry (decision: `extern crate` is removed); no other assertion moves

**Done when**:

- [x] Test `b36_lists_extern_crate` passes in `spec_text.rs` (C8: PKG-01 AC 8)
- [x] Test `extern_crate_is_a_removed_form` passes in `msl_errors.rs` (C9: PKG-01 AC 9)
- [x] `extern "C" fn` items and `extern "abi" module` still parse; an `extern` block still reports its own row
- [x] The item is kept as `ItemKind::Error`: one fault, one diagnostic
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(msl): extern crate is a removed form with a targeted message`

---

### T5: Appendix G, glossary and file map

**What**: G.0 gains the rows for source search paths and a package registry; the closed table gains P1 to P6 (`design.md`) and its closing paragraph says what P numbers; §0.9 gains crate, crate root, dependency, package, manifest and lockfile; the Part I row of §0.8 names crates and packages.
**Where**: `docs/spec/appendix_g_scope.md` (+ `docs/spec/index.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T4
**Reuses**: `g_entry`, `table_rows`; `appendix_g_records_the_decisions` as the pattern
**Requirement**: PKG-02

**Done when**:

- [x] Test `appendix_g_records_the_package_decisions` passes in `spec_text.rs` (C20: PKG-02 AC 10)
- [x] Test `glossary_and_file_map_name_crates_and_packages` passes in `spec_text.rs` (C21: PKG-02 AC 11)
- [x] Each glossary row links to the defining section (§3.5 or §3.6); `prelude` still points at §3.4
- [x] G.38 is untouched (the library as a source is `msl-stdlib`)
- [x] Gate passes: run spec; `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test pom_docs --test msl_errors`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): Appendix G and the glossary record the package decisions`

---

### T6: `CrateGraph`: the crate graph as data

**What**: `CrateId`, `CrateGraph`, `CrateNode`, `DependencyData`, `GraphError` and `is_crate_name`, as in `design.md` ("Data Models"); a new test target `pom_crates`.
**Where**: `crates/piperine-lang/src/pom/crates.rs` (+ `src/pom/ids.rs`, `src/pom/mod.rs`, `tests/pom_crates.rs`, `docs/pom/model.md`)
**Depends on**: T5
**Reuses**: the id pattern of `ids.rs`; `msl::lexer::lex` for `is_crate_name`
**Requirement**: PKG-03

**Done when**:

- [x] Test `graph_keeps_crates_and_edges_as_given` passes in `pom_crates.rs` (C22: PKG-03 AC 1)
- [x] Test `graph_rejects_invalid_and_reserved_names` passes in `pom_crates.rs` (C23: PKG-03 AC 2)
- [x] Test `graph_rejects_a_cycle_and_names_it` passes in `pom_crates.rs` (C24: PKG-03 AC 3)
- [x] Test `graph_rejects_a_repeated_dependency_name` passes in `pom_crates.rs` (C25: PKG-03 AC 4)
- [x] Test `graph_rejects_a_foreign_id` passes in `pom_crates.rs` (C26: PKG-03 AC 5)
- [x] Test `piperine_lang_takes_the_graph_as_data` passes in `pom_crates.rs` (C27: PKG-03 AC 6)
- [x] `pom_crates.rs` opens with a `//!` scope header; no `#[ignore]`, no file-scope `#![allow]`
- [x] A rejected call leaves the graph equal to its clone taken before the call
- [x] The cycle check is a reachability walk over the stored edges; nothing recurses without bound
- [x] `docs/pom/model.md` has a `### ` heading for each new public type (`pom_docs` enforces it)
- [x] Gate passes: run crates, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(pom): CrateGraph, the crate graph as plain data`

---

### T7: The standard library is a source tree; the prelude is `std::prelude`

**What**: `crates/piperine-lang/std/lib.msl` (`pub mod prelude;`) and `std/prelude.msl` (empty but for its doc comment), embedded by `include_str!` in `src/pom/stdlib.rs` and lowered after the project's files in every build. `DesignBuilder::std_source(path, text)` replaces the embedded files. The implicit names are the `pub` items of the namespace `prelude` of that tree's root. `DesignBuilder::prelude` is removed.
**Where**: `crates/piperine-lang/src/pom/stdlib.rs` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/design.rs`, `src/pom/resolve/mod.rs`, `src/pom/resolve/imports.rs`, `tests/pom_crates.rs`, the tests and `docs/pom/` fences named below)
**Depends on**: T6
**Reuses**: `MemorySources`, `Design::add_file`, `Ctx::ns_item`; the `prelude: bool` of `SourceFileData` still marks the library's files until T8
**Requirement**: PKG-06
**Existing tests**: decision U2 moves these and nothing else: (a) the nine `.prelude(name, text)` call sites of `pom_reflect.rs`, `pom_classify.rs`, `pom_design.rs`, `pom_resolve.rs` and the five of `docs/pom/` become `.std_source("lib.msl", ..)` with the same items inside `pub mod prelude { .. }`; (b) `pom_design::prelude_files_are_numbered_first` asserts the new order (project files, then the library's) under the name `library_files_are_numbered_after_the_project`; (c) assertions that count or index all files (`files().len()`, `files().last()`) count the files that are not `is_prelude()`. No expected value of a resolution moves

**Done when**:

- [x] Test `embedded_std_builds_clean_and_has_a_prelude` passes in `pom_crates.rs` (C51: PKG-06 AC 1)
- [x] Test `prelude_items_resolve_without_an_import` passes in `pom_crates.rs` (C52: PKG-06 AC 2)
- [x] Test `only_pub_items_of_std_prelude_are_implicit` passes in `pom_crates.rs` (C53: PKG-06 AC 3)
- [x] Test `std_source_replaces_the_embedded_library` passes in `pom_crates.rs` (C54: PKG-06 AC 4)
- [x] Test `a_namespace_item_hides_a_prelude_item` passes in `pom_crates.rs` (C55: PKG-06 AC 5)
- [x] Test `std_files_are_numbered_last` passes in `pom_crates.rs` (C56: PKG-06 AC 6)
- [x] Test `std_files_see_the_prelude` passes in `pom_crates.rs` (C57: PKG-06 AC 7)
- [x] `std/lib.msl` and `std/prelude.msl` hold no item: the content is `msl-stdlib`
- [x] A design built with no `std_source` and no project source has zero diagnostics
- [x] The lookup order is unchanged but for its third step: namespace items and imports, the `pub` items of `std::prelude`, the built-in table
- [x] No doctest of `docs/pom/` is removed; `cargo test -p piperine-lang --doc` passes
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): std is an embedded source tree and the prelude is std::prelude`

---

### T8: Crates in the `Design`

**What**: `CrateData` storage, `Design::{crates, root_crate, std_crate}` fields, `SourceFileData::krate` in place of `prelude`; every build holds the root crate (named by the stem of its root file) and `std`. The handle `Crate` with `id`, `name`, `root`, `files`, `is_root`, `is_std`; `Design::crates`, `root_crate`, `std_crate`; `SourceFile::krate`; `Reflect::krate`. `SourceFile::is_prelude` and `Design::prelude_files` are removed.
**Where**: `crates/piperine-lang/src/pom/design.rs` (+ `src/pom/reflect/crates.rs`, `src/pom/reflect/body.rs`, `src/pom/reflect/mod.rs`, `src/pom/lower/files.rs`, `src/pom/check.rs`, `tests/pom_crates.rs`, `docs/pom/model.md`, `docs/pom/reflection.md`)
**Depends on**: T7
**Reuses**: the handle pattern of `reflect/body.rs` (`SourceFile`); `Design::authored`, `first_named`
**Requirement**: PKG-04, PKG-07
**Existing tests**: the renames of this task and nothing else: `is_prelude()` becomes `krate().is_std()`, `prelude_files()` becomes `std_crate().files()`, in `pom_reflect.rs`, `pom_resolve.rs`, `pom_design.rs`, the tests T7 added and `docs/pom/reflection.md`; the serialized form in any golden text gains `crates`, `root_crate`, `std_crate`, `krate` and loses `root_file`, `prelude`

**Done when**:

- [x] Test `single_source_design_has_a_root_crate_named_by_its_file` passes in `pom_crates.rs` (C28: PKG-04 AC 1)
- [x] Test `every_design_holds_std_last` passes in `pom_crates.rs` (C29: PKG-04 AC 2)
- [x] Test `a_crate_reports_its_name_root_and_files` passes in `pom_crates.rs` (C58: PKG-07 AC 1)
- [x] Test `declarations_and_files_know_their_crate` passes in `pom_crates.rs` (C59: PKG-07 AC 2)
- [x] A root file with no stem that is an identifier (`9.msl`, `a-b.msl`) gives the crate name with every other character replaced by `_` and a leading `_` before a digit
- [x] `Design::root()` is `root_crate().root()`; `SourceFileRepr` carries `krate`
- [x] Every new public type has its `### ` heading and every new method its row in `docs/pom/` (`pom_docs`)
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): a design holds its crates; the Crate handle`

---

### T9: The builder takes a crate graph

**What**: `DesignBuilder::crates(graph)` and `Design::load_crates(graph)`: one `CrateData` per crate of the graph in graph order (the first is the root crate), each lowered from its own root file with its own `mod` chain, then `std`. Root paths are read from the virtual sources when any was given, else from disk.
**Where**: `crates/piperine-lang/src/pom/design.rs` (+ `src/pom/lower/mod.rs`, `tests/pom_crates.rs`, `docs/pom/reflection.md`)
**Depends on**: T8
**Reuses**: `Design::add_file` (called once per crate root with `is_root = true`), `Lowerer::load_mod`, `DiskSources`, `MemorySources`
**Requirement**: PKG-04

**Done when**:

- [x] Test `graph_crates_become_design_crates_in_order` passes in `pom_crates.rs` (C30: PKG-04 AC 3)
- [x] Test `mod_files_load_beside_their_own_crate_root` passes in `pom_crates.rs` (C31: PKG-04 AC 4)
- [x] Test `an_unreadable_crate_root_is_one_e0161` passes in `pom_crates.rs` (C32: PKG-04 AC 5)
- [x] Test `every_crate_keeps_its_authored_tree` passes in `pom_crates.rs` (C33: PKG-04 AC 6)
- [x] Test `disk_and_memory_graphs_build_the_same_design` passes in `pom_crates.rs` (C34: PKG-04 AC 7)
- [x] An empty graph is treated as no graph
- [x] Resolution of dependency names is not part of this task: a path into another crate is still `E0161` here
- [x] The `E0161` of an unreadable crate root has the message form of today's unreadable root file
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): Design over a crate graph, each crate lowered as authored`

---

### T10: Dependency names start a path; `crate` is per crate

**What**: `scope_lookup` gains the dependency step between the namespace and the prelude: a dependency name of the crate that contains the path, `std` among them, resolves to the root namespace of that crate. `walk` resolves `crate` to the root of the containing crate. In a `use` path a first segment that names both a dependency and an item or import of the enclosing namespace is `E0163` (U6); in any other path the item or import wins. The prelude lookup of T7 now reads the `std` crate.
**Where**: `crates/piperine-lang/src/pom/resolve/imports.rs` (+ `src/pom/resolve/mod.rs`, `tests/pom_crates.rs`, `docs/pom/resolution.md`)
**Depends on**: T9
**Reuses**: `Ctx::walk`, `scope_lookup`, `ns_item`, `parent_namespace`
**Requirement**: PKG-05

**Done when**:

- [x] Test `a_dependency_name_starts_a_path` passes in `pom_crates.rs` (C35: PKG-05 AC 1)
- [x] Test `an_instance_links_to_a_module_of_a_dependency` passes in `pom_crates.rs` (C36: PKG-05 AC 2)
- [x] Test `crate_names_the_root_of_the_containing_crate` passes in `pom_crates.rs` (C37: PKG-05 AC 3)
- [x] Test `a_renamed_dependency_resolves_by_its_dependency_name` passes in `pom_crates.rs` (C38: PKG-05 AC 4)
- [x] Test `a_local_name_wins_over_a_dependency_in_a_path` passes in `pom_crates.rs` (C39: PKG-05 AC 5)
- [x] Test `a_use_path_that_names_a_local_and_a_dependency_is_e0163` passes in `pom_crates.rs` (C40: PKG-05 AC 6)
- [x] Test `crate_prefix_names_the_local_one_in_a_clash` passes in `pom_crates.rs` (C41: PKG-05 AC 7)
- [x] Test `std_is_a_dependency_of_every_crate` passes in `pom_crates.rs` (C42: PKG-05 AC 8)
- [x] Test `the_prelude_reaches_every_crate` passes in `pom_crates.rs` (C43: PKG-05 AC 9)
- [x] Test `a_dependency_name_hides_a_prelude_item` passes in `pom_crates.rs` (C44: PKG-05 AC 10)
- [x] `Ctx` holds the crate of each namespace and the dependency names of each crate, computed once
- [x] A crate name is `Resolution::Decl` of a `Namespace`: no new `Resolution` variant
- [x] `E0163` is reported once, at the `use` path, by a `WalkError` of its own; its help reads `rename the dependency in Piperine.toml (other = { package = "<name>", .. }) or write crate::<name> for the local one`; a block-level `use` follows the same rule
- [x] A dependency name against a prelude item is no clash: the dependency wins silently (C of PKG-05 AC 10)
- [x] The Diagnostics table of `docs/pom/resolution.md` gains the `E0163` row, defined in `§3.5` (`pom_docs::every_code_the_pom_reports_is_documented`)
- [x] `use util::*;`, `use util::{a, b};` and `use util as u;` resolve like any other import
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): dependency names resolve at the start of a path; crate is per crate`

---

### T11: Visibility and diagnostics across crates

**What**: The six cases of PKG-05 AC 11 to 16, each proven by a test written first. The one new behaviour is the help line of `E0161` when the missing first segment is the name of a crate of the design that is no dependency of the containing crate; the others hold by `within` and `ns_item` and are fixed here if a test shows otherwise.
**Where**: `crates/piperine-lang/src/pom/resolve/imports.rs` (+ `tests/pom_crates.rs`, `docs/pom/resolution.md`)
**Depends on**: T10
**Reuses**: `path_diagnostic`, `WalkError`, `within`
**Requirement**: PKG-05

**Done when**:

- [x] Test `a_crate_that_is_no_dependency_is_e0161_with_help` passes in `pom_crates.rs` (C45: PKG-05 AC 11)
- [x] Test `a_private_item_of_another_crate_is_e0161` passes in `pom_crates.rs` (C46: PKG-05 AC 12)
- [x] Test `a_pub_item_in_a_private_namespace_is_e0161` passes in `pom_crates.rs` (C47: PKG-05 AC 13)
- [x] Test `super_at_a_crate_root_is_e0161` passes in `pom_crates.rs` (C48: PKG-05 AC 14)
- [x] Test `a_glob_of_a_dependency_brings_only_pub_items` passes in `pom_crates.rs` (C49: PKG-05 AC 15)
- [x] Test `a_pub_use_re_exports_across_crates` passes in `pom_crates.rs` (C50: PKG-05 AC 16)
- [x] Each fault is one diagnostic: no `E0161` is doubled by the import and by its use
- [x] The help text is `name it under [dependencies] in Piperine.toml` and appears only for a first segment
- [x] The `E0161` row of the Diagnostics table of `docs/pom/resolution.md` names the crate case
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): pub is the only way across a crate; E0161 help for an undeclared crate`

---

### T12: Reflection over crates

**What**: `Design::krate(name)`; `Crate::dependencies`, `dependency`, `decls`, `modules`, `module`, `find`. The typed collections of `Design` list the root crate; the by-name lookups prefer the root crate, then crate order.
**Where**: `crates/piperine-lang/src/pom/reflect/crates.rs` (+ `src/pom/reflect/mod.rs`, `tests/pom_crates.rs`, `docs/pom/reflection.md`)
**Depends on**: T11
**Reuses**: `Design::authored`, `first_named`, `Design::find`, `Reflect::qualified_name`
**Requirement**: PKG-07
**Existing tests**: `pom_reflect::module_lookup_prefers_authored_files` and `by_name_lookups_prefer_project_files` keep their assertions; only their doc comments say `root crate` for `project files`

**Done when**:

- [x] Test `a_crate_lists_its_declared_dependencies` passes in `pom_crates.rs` (C60: PKG-07 AC 3)
- [x] Test `collections_list_the_root_crate_and_a_crate_lists_its_own` passes in `pom_crates.rs` (C61: PKG-07 AC 4)
- [x] Test `by_name_lookups_prefer_the_root_crate_then_crate_order` passes in `pom_crates.rs` (C62: PKG-07 AC 5)
- [x] Test `crates_are_found_by_name` passes in `pom_crates.rs` (C63: PKG-07 AC 6)
- [x] Test `crate_find_inverts_qualified_name` passes in `pom_crates.rs` (C64: PKG-07 AC 7)
- [x] `qualified_name()` is unchanged: relative to the declaration's own crate
- [x] Every new method has its row in `docs/pom/reflection.md` (`pom_docs`)
- [x] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): crates in reflection: dependencies, listings, lookups`

---

### T13: Serialization and `Design::check` over crates

**What**: `Design::check` verifies the crate invariants: every crate link in range, each crate's root declaration is the root namespace of its own root file, each file tagged with an existing crate, no dependency cycle. `Design::from_json` returns them as `Err`.
**Where**: `crates/piperine-lang/src/pom/check.rs` (+ `tests/pom_crates.rs`)
**Depends on**: T12
**Reuses**: `Design::check`, `Design::from_json`; the JSON-editing pattern of `pom_reflect.rs` (`serde_json::Value`)
**Requirement**: PKG-08

**Done when**:

- [ ] Test `a_multi_crate_design_round_trips_through_json` passes in `pom_crates.rs` (C65: PKG-08 AC 1)
- [ ] Test `from_json_rejects_out_of_range_crate_links` passes in `pom_crates.rs` (C66: PKG-08 AC 2)
- [ ] Test `from_json_rejects_a_dependency_cycle` passes in `pom_crates.rs` (C67: PKG-08 AC 3)
- [ ] Test `from_json_rejects_a_crate_whose_root_is_not_its_own` passes in `pom_crates.rs` (C68: PKG-08 AC 4)
- [ ] No function panics on any JSON: the tests feed each broken link through `from_json`
- [ ] The cycle check walks stored edges with a visited set
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): Design::check validates crates; multi-crate round trip`

---

### T14: `docs/pom`: crates

**What**: `resolution.md`: the scope order with the dependency names and the `std` prelude, a section `### Crates and the standard library` with a two-crate doctest in place of `Imports and the prelude`. `mapping.md`: a `crate` row. `index.md` and `model.md`: the build line of the overview, a `## Crates` storage section. No page mentions prelude sources.
**Where**: `docs/pom/resolution.md` (+ `docs/pom/mapping.md`, `docs/pom/index.md`, `docs/pom/model.md`, `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T13
**Reuses**: `pom_docs.rs` helpers `page_of`, `section_rows`, `pom_docs`
**Requirement**: PKG-09

**Done when**:

- [ ] Test `docs_describe_crates_and_no_prelude_sources` passes in `pom_docs.rs` (C69: PKG-09 AC 1)
- [ ] Test `mapping_has_a_crate_row` passes in `pom_docs.rs` (C70: PKG-09 AC 2)
- [ ] `cargo test -p piperine-lang --doc` exits 0 (C71: PKG-09 AC 3)
- [ ] `every_public_pom_type_has_a_heading`, `every_reflection_method_is_documented`, `msl_fences_parse_clean` and `mkdocs_nav_lists_every_pom_page` pass unchanged
- [ ] The two-crate doctest builds a `CrateGraph` with virtual sources and asserts zero diagnostics
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `docs(pom): crates, dependency names and the std prelude`

---

### T15: Cut `piperine-project` down

**What**: Delete `release.rs`, `source_map.rs`, `tests/release.rs`, the old `resolver.rs` and `lockfile.rs`, `GitSource` and its unit tests, `PiperineToml`, `Project`, `DependencySource` and the plugin sections. Keep `find_project_root` and `git::sync_and_checkout`. Drop `ureq`, `sha2`, `serde`, `serde_json` and `thiserror` from `Cargo.toml`. A new test target `project`.
**Where**: `crates/piperine-project/src/lib.rs` (+ `Cargo.toml`, `src/git.rs`, the deleted files, `tests/project.rs`)
**Depends on**: T14
**Reuses**: `find_project_root` as it is
**Requirement**: PKG-14, PKG-15
**Existing tests**: the unit tests inside the deleted modules go with their modules (U3: the surface they test is deleted); no test of surviving code is removed

**Done when**:

- [ ] Test `the_project_root_is_the_nearest_manifest` passes in `project.rs` (C111: PKG-14 AC 1)
- [ ] Test `the_crate_holds_no_retired_surface` passes in `project.rs` (C115: PKG-15 AC 1)
- [ ] `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')` exits 0
- [ ] `tests/project.rs` opens with a `//!` scope header and uses `tempfile::tempdir()`
- [ ] `get_current_project_root` is deleted with the rest (the CLI's helper)
- [ ] The other workspace crates are not touched
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `refactor(project): keep manifest, dependencies and lockfile; delete the rest`

---

### T16: `PackageError`; a manifest that cannot be read

**What**: `PackageError { file, text, diagnostic }` with `code()` and `render()` (the `miette` report of `piperine_lang::msl::render`, named by the file name). `Manifest::load(dir)` and `Manifest::parse(text)` up to a valid TOML table: `E0170` for a missing file, for text that is not UTF-8 and for a TOML syntax error at the span `toml` reports. A new test target `manifest`.
**Where**: `crates/piperine-project/src/error.rs` (+ `src/manifest.rs`, `src/lib.rs`, `tests/manifest.rs`)
**Depends on**: T15
**Reuses**: `msl::Diagnostic`, `msl::render`, `toml::de::DeTable::parse`, `toml::de::Error::span`
**Requirement**: PKG-10

**Done when**:

- [ ] Test `a_missing_manifest_is_e0170` passes in `manifest.rs` (C72: PKG-10 AC 1)
- [ ] Test `invalid_toml_is_e0170_at_its_span` passes in `manifest.rs` (C73: PKG-10 AC 2)
- [ ] Test `a_package_error_renders_as_a_miette_report` passes in `manifest.rs` (C74: PKG-10 AC 3)
- [ ] A missing file has an empty text and a zero span; its message carries the path
- [ ] The render test reads line and column from the span, as `msl_errors::rendered_output_is_miette_graphical` does
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): PackageError and E0170 for an unreadable manifest`

---

### T17: The `[package]` table

**What**: `Manifest`, `Package`, `Version`: a hand walk over the spanned table. `name` and `version` required, `authors` and `description` optional; any other table or key, a missing key, a wrong type, a bad name (`pom::is_crate_name`) and a bad version are each one `E0171` at their span; every fault is reported, in source order.
**Where**: `crates/piperine-project/src/manifest.rs` (+ `tests/manifest.rs`)
**Depends on**: T16
**Reuses**: `piperine_lang::pom::is_crate_name`; the spans of `DeTable`
**Requirement**: PKG-10

**Done when**:

- [ ] Test `a_valid_manifest_reads_every_package_field` passes in `manifest.rs` (C75: PKG-10 AC 4)
- [ ] Test `an_unknown_key_is_e0171_at_the_key` passes in `manifest.rs` (C76: PKG-10 AC 5)
- [ ] Test `a_missing_or_mistyped_package_key_is_e0171` passes in `manifest.rs` (C77: PKG-10 AC 6)
- [ ] Test `a_bad_package_name_is_e0171` passes in `manifest.rs` (C78: PKG-10 AC 7)
- [ ] Test `a_bad_version_is_e0171` passes in `manifest.rs` (C79: PKG-10 AC 8)
- [ ] Test `every_fault_of_a_manifest_is_reported` passes in `manifest.rs` (C80: PKG-10 AC 9)
- [ ] The message of an unknown key names the key and the table; its help lists the keys §3.6 allows there
- [ ] `[dependencies]` is accepted and left unread until T18
- [ ] Diagnostics are sorted by span start
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): the [package] table, validated with spans`

---

### T18: The `[dependencies]` table

**What**: `Dependency`, `Source`, `GitRef`, `Requirement`: each entry read with its name span; exactly one of `path` and `git`; at most one of `branch`, `tag`, `rev`, with `git` only; `version` in the forms `M`, `M.m`, `M.m.p`; `package` a valid name; each violation one `E0171`.
**Where**: `crates/piperine-project/src/manifest.rs` (+ `tests/manifest.rs`)
**Depends on**: T17
**Reuses**: T17's walk and helpers
**Requirement**: PKG-10

**Done when**:

- [ ] Test `dependencies_read_in_source_order` passes in `manifest.rs` (C81: PKG-10 AC 10)
- [ ] Test `a_dependency_needs_exactly_one_source` passes in `manifest.rs` (C82: PKG-10 AC 11)
- [ ] Test `git_refs_are_exclusive_and_git_only` passes in `manifest.rs` (C83: PKG-10 AC 12)
- [ ] Test `a_bad_dependency_name_or_requirement_is_e0171` passes in `manifest.rs` (C84: PKG-10 AC 13)
- [ ] Dependencies keep source order
- [ ] An unknown key inside a dependency (`subdir`, `features`) is `E0171` at the key
- [ ] A dependency written as a bare string (`util = "1.0"`) is `E0171`: there is no registry
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): the [dependencies] table: path, git, version, package`

---

### T19: Path dependencies become a `CrateGraph`

**What**: `resolve(dir, options) -> Result<PackageGraph, Vec<PackageError>>` for `path` dependencies: a depth-first walk from the root package; each path relative to the directory of the manifest that names it; a package identified by its canonical directory, so a shared dependency is one crate; `E0172` for a directory without a manifest, `E0175` for a package without `src/lib.msl`. A new test target `paths` and the shared fixture builders.
**Where**: `crates/piperine-project/src/resolver.rs` (+ `src/lib.rs`, `tests/paths.rs`, `tests/support/mod.rs`)
**Depends on**: T18
**Reuses**: `Manifest::load`, `CrateGraph::add_crate`, `add_dependency`
**Requirement**: PKG-11

**Done when**:

- [ ] Test `path_dependencies_are_relative_to_their_manifest` passes in `paths.rs` (C85: PKG-11 AC 1)
- [ ] Test `the_graph_mirrors_the_manifests` passes in `paths.rs` (C86: PKG-11 AC 2)
- [ ] Test `a_shared_dependency_is_one_crate` passes in `paths.rs` (C87: PKG-11 AC 3)
- [ ] Test `a_path_without_a_package_is_e0172` passes in `paths.rs` (C88: PKG-11 AC 4)
- [ ] Test `a_package_without_a_crate_root_is_e0175` passes in `paths.rs` (C89: PKG-11 AC 5)
- [ ] `tests/support/mod.rs` opens with a `//!` header; it writes packages into a `tempdir` (`package(dir, name, version, deps, lib_text)`): plain functions, no macro
- [ ] `E0172` and `E0175` point at the dependency's key in the manifest that names it; for the root package `E0175` has a zero span in its own manifest
- [ ] A git dependency is `E0172` "git dependencies are not resolved yet" until T22: fail loud, never skipped
- [ ] The first crate of the graph is the root package
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): path dependencies resolve into a CrateGraph`

---

### T20: The located package is the one asked for

**What**: `Requirement::admits(version)` by the table of §3.6; in the resolver, `E0173` when the located package's name is not the dependency's (or its `package`), or its version is not admitted; `package` names the edge by the key.
**Where**: `crates/piperine-project/src/resolver.rs` (+ `src/manifest.rs`, `tests/paths.rs`)
**Depends on**: T19
**Reuses**: T19's walk
**Requirement**: PKG-11

**Done when**:

- [ ] Test `a_package_of_another_name_is_e0173` passes in `paths.rs` (C90: PKG-11 AC 6)
- [ ] Test `package_key_renames_a_dependency` passes in `paths.rs` (C91: PKG-11 AC 7)
- [ ] Test `version_requirements_follow_the_caret_table` passes in `paths.rs` (C92: PKG-11 AC 8)
- [ ] Test `an_unsatisfied_requirement_is_e0173` passes in `paths.rs` (C93: PKG-11 AC 9)
- [ ] `version_requirements_follow_the_caret_table` holds the eight rows as data and checks, for each, the lower bound, the last admitted version, the one below and the first past
- [ ] The `E0173` message names what was asked and what was found; its span is the dependency's key
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): E0173 for a package of another name or version; package renames`

---

### T21: Cycles

**What**: The walk keeps the stack of packages being resolved: a dependency that locates a package on the stack is one `E0174` at that dependency's key, with the chain of package names in the message. An `Err` of `CrateGraph::add_dependency` is reported, never unwrapped.
**Where**: `crates/piperine-project/src/resolver.rs` (+ `tests/paths.rs`)
**Depends on**: T20
**Reuses**: T19's walk; `GraphError::Cycle`
**Requirement**: PKG-11

**Done when**:

- [ ] Test `a_dependency_cycle_is_e0174_with_its_chain` passes in `paths.rs` (C94: PKG-11 AC 10)
- [ ] Test `a_package_that_depends_on_itself_is_e0174` passes in `paths.rs` (C95: PKG-11 AC 11)
- [ ] Resolution terminates on every cyclic input the tests give (two, three packages, a self-dependency)
- [ ] The chain reads `a -> b -> c -> a`
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): E0174 for a dependency cycle, with its chain`

---

### T22: Git dependencies

**What**: `git` dependencies in the resolver: the reference resolved in `<root>/target/deps/<sanitized url>/db`, the commit checked out in `<root>/target/deps/<sanitized url>/<commit>`; a package identified by `(url, commit)`; `E0172` with git's message on any failure. `sync_and_checkout` resolves a branch as `refs/remotes/origin/<b>`, a tag as `refs/tags/<t>`, a rev as given, and fetches tags. A new test target `git`.
**Where**: `crates/piperine-project/src/git.rs` (+ `src/resolver.rs`, `tests/git.rs`, `tests/support/mod.rs`)
**Depends on**: T21
**Reuses**: `sync_and_checkout`; `git2::Repository::init` for the fixtures
**Requirement**: PKG-12

**Done when**:

- [ ] Test `a_git_dependency_checks_out_the_default_branch` passes in `git.rs` (C96: PKG-12 AC 1)
- [ ] Test `branch_tag_and_rev_select_their_commit` passes in `git.rs` (C97: PKG-12 AC 2)
- [ ] Test `an_unreachable_repository_or_ref_is_e0172` passes in `git.rs` (C98: PKG-12 AC 3)
- [ ] Test `two_commits_of_one_repository_are_two_crates` passes in `git.rs` (C99: PKG-12 AC 4)
- [ ] Test `one_commit_of_one_repository_is_one_crate` passes in `git.rs` (C100: PKG-12 AC 5)
- [ ] Test `a_git_package_resolves_its_own_dependencies` passes in `git.rs` (C101: PKG-12 AC 6)
- [ ] Every repository of the tests is created in a `tempdir` and named by its filesystem path: no test contacts the network
- [ ] A failed checkout leaves no `<commit>` directory behind
- [ ] The fixture helper `repo(dir) -> Repository` with `commit(repo, files, message) -> Oid`, `branch`, `tag` lives in `tests/support/mod.rs`
- [ ] The lockfile is not written yet (T23)
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): git dependencies: branch, tag, rev, one checkout per commit`

---

### T23: The lockfile is written and honoured

**What**: `Lockfile`, `LockedPackage`: read through the spanned table, written by a string builder in the format of §3.6 rule 14. The resolver records each git package, checks out the recorded commit while the entry's `source` equals the dependency's, writes the file only when its text changes, and creates none for a graph without git packages. A new test target `lock`.
**Where**: `crates/piperine-project/src/lockfile.rs` (+ `src/resolver.rs`, `tests/lock.rs`)
**Depends on**: T22
**Reuses**: T22's fixtures; T17's table walk
**Requirement**: PKG-13

**Done when**:

- [ ] Test `the_lockfile_records_each_git_package` passes in `lock.rs` (C102: PKG-13 AC 1)
- [ ] Test `a_locked_commit_is_kept_when_the_branch_moves` passes in `lock.rs` (C103: PKG-13 AC 2)
- [ ] Test `no_git_package_no_lockfile` passes in `lock.rs` (C104: PKG-13 AC 3)
- [ ] Test `the_lockfile_is_deterministic` passes in `lock.rs` (C105: PKG-13 AC 4)
- [ ] `source` is `git+<url>` followed by `?branch=<b>`, `?tag=<t>` or `?rev=<r>` when one was given
- [ ] Entries are sorted by `name`, then `source`; the file ends with one newline
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): Piperine.lock records and pins git packages`

---

### T24: Update, stale entries, an invalid lockfile, no network

**What**: `Options { update }` resolves every reference again and rewrites the lockfile; an entry no dependency asks for is dropped and one whose source changed is replaced; a lockfile that is not valid TOML or whose `version` is not `1` is one `E0176` before anything is fetched; with an entry and its checkout present the repository is not opened; a lockfile inside a dependency is not read.
**Where**: `crates/piperine-project/src/lockfile.rs` (+ `src/resolver.rs`, `tests/lock.rs`)
**Depends on**: T23
**Reuses**: T23
**Requirement**: PKG-13

**Done when**:

- [ ] Test `update_moves_the_lock_to_the_new_commit` passes in `lock.rs` (C106: PKG-13 AC 5)
- [ ] Test `stale_entries_leave_the_lockfile` passes in `lock.rs` (C107: PKG-13 AC 6)
- [ ] Test `an_invalid_lockfile_is_e0176` passes in `lock.rs` (C108: PKG-13 AC 7)
- [ ] Test `a_locked_checkout_needs_no_repository` passes in `lock.rs` (C109: PKG-13 AC 8)
- [ ] Test `only_the_root_lockfile_counts` passes in `lock.rs` (C110: PKG-13 AC 9)
- [ ] `a_locked_checkout_needs_no_repository` deletes the source repository and the `db` clone before the second resolution
- [ ] `E0176` has the lockfile as its `file` and the TOML error's span
- [ ] Gate passes: `cargo test -p piperine-project`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): lockfile update, pruning, E0176, offline resolution`

---

### T25: `load_project`; the code guard

**What**: `load_project(dir) -> Result<Design, Vec<PackageError>>`: `resolve` then `Design::load_crates`. The guard `the_crate_reports_exactly_the_package_phase_codes` compares the `"E01nn"` literals of `crates/piperine-project/src` with the rows of phase `package` of `docs/spec/`.
**Where**: `crates/piperine-project/src/lib.rs` (+ `tests/project.rs`)
**Depends on**: T24
**Reuses**: `resolve`, `Design::load_crates`; the scraping pattern of `pom_docs::every_code_the_pom_reports_is_documented`
**Requirement**: PKG-14, PKG-15

**Done when**:

- [ ] Test `load_project_builds_the_design_of_a_package_graph` passes in `project.rs` (C112: PKG-14 AC 2)
- [ ] Test `a_package_error_stops_load_project` passes in `project.rs` (C113: PKG-14 AC 3)
- [ ] Test `a_model_error_is_a_diagnostic_not_a_package_error` passes in `project.rs` (C114: PKG-14 AC 4)
- [ ] Test `the_crate_reports_exactly_the_package_phase_codes` passes in `project.rs` (C116: PKG-15 AC 2)
- [ ] `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')` exits 0 (C117: PKG-15 AC 3)
- [ ] The crate's public surface is `Manifest`, `Package`, `Version`, `Requirement`, `Dependency`, `Source`, `GitRef`, `Options`, `PackageGraph`, `ResolvedPackage`, `Lockfile`, `LockedPackage`, `PackageError`, `resolve`, `load_project`, `find_project_root`
- [ ] `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')` exits 0 (C117)
- [ ] `cargo test -p piperine-lang` and `cargo test -p piperine-project` both pass
- [ ] Gate passes: `cargo test -p piperine-project`; `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(project): load_project builds the design of a package graph`

---

## Checks

One check per acceptance criterion of `spec.md`. Each `run` proof names the line its test prints; the two lone proofs are judged by their exit code.

**C1** - §3.5 exists and defines crate, crate root and dependency name once each (PKG-01, AC 1)
Proof: run spec | test s35_defines_crate_root_and_dependency_name ... ok

**C2** - §3.5 lists the five lookup steps in that order (PKG-01, AC 2)
Proof: run spec | test s35_orders_the_first_segment_lookup ... ok

**C3** - §3.5 says the local name wins in an ordinary path and a `use` path is `E0163`; no `::` (PKG-01, AC 3)
Proof: run spec | test s35_local_names_win_in_paths_and_clash_in_use ... ok

**C4** - §3.5 makes `crate`, `self` and `super` relative to the containing crate (PKG-01, AC 4)
Proof: run spec | test s35_crate_and_super_are_per_crate ... ok

**C5** - §3.5 says only `pub` crosses a crate boundary, namespace by namespace (PKG-01, AC 5)
Proof: run spec | test s35_only_pub_crosses_a_crate ... ok

**C6** - §3.5 makes `std` implicit and reserved; §3.4.2 defines the prelude as `std::prelude` (PKG-01, AC 6)
Proof: run spec | test std_is_implicit_and_the_prelude_is_std_prelude ... ok

**C7** - the `E0161` row covers a path that starts with a crate that is no dependency (PKG-01, AC 7)
Proof: run spec | test e0161_row_covers_an_unknown_crate ... ok

**C8** - B.3.6 has a row for `extern crate` with `E0900` and its replacement (PKG-01, AC 8)
Proof: run spec | test b36_lists_extern_crate ... ok

**C9** - `extern crate util;` gives one `E0900` with the targeted message and help (PKG-01, AC 9)
Proof: run errors | test extern_crate_is_a_removed_form ... ok

**C10** - §3.5 defines `E0163` (elaboration) and names both remedies (PKG-01, AC 10)
Proof: run spec | test e0163_is_defined_in_3_5 ... ok

**C11** - §0.3 lists five phases, `package` among them, and rule 3 rejects on it (PKG-02, AC 1)
Proof: run spec | test phase_package_is_listed ... ok

**C12** - §3.6 names `Piperine.toml`, `src/lib.msl`, `.msl` and the two `mod` candidates (PKG-02, AC 2)
Proof: run spec | test s36_fixes_the_package_layout ... ok

**C13** - the manifest table of §3.6 has exactly those eleven key rows and closes the list with `E0171` (PKG-02, AC 3)
Proof: run spec | test s36_lists_every_manifest_key ... ok

**C14** - §3.6 states the name rule and the version form (PKG-02, AC 4)
Proof: run spec | test s36_names_are_identifiers ... ok

**C15** - §3.6 states the two sources and the three exclusive git references (PKG-02, AC 5)
Proof: run spec | test s36_states_the_dependency_sources ... ok

**C16** - §3.6 holds the eight-row requirement table with its ranges (PKG-02, AC 6)
Proof: run spec | test s36_gives_the_version_requirement_table ... ok

**C17** - §3.6 excludes every other way to find a crate and forbids cycles (PKG-02, AC 7)
Proof: run spec | test s36_has_no_search_path_and_no_cycle ... ok

**C18** - §3.6 states where the lockfile is, its keys and when its commit is used (PKG-02, AC 8)
Proof: run spec | test s36_states_the_lockfile_rule ... ok

**C19** - §3.6 defines the seven codes `E0170` to `E0176`, all of phase `package` (PKG-02, AC 9)
Proof: run spec | test s36_defines_the_package_codes ... ok

**C20** - G.0 excludes search paths and a registry; the closed table has rows P1 to P6 (PKG-02, AC 10)
Proof: run spec | test appendix_g_records_the_package_decisions ... ok

**C21** - §0.9 has the six terms and §0.8's Part I row names crates and packages (PKG-02, AC 11)
Proof: run spec | test glossary_and_file_map_name_crates_and_packages ... ok

**C22** - a graph of three crates reads back names, roots and edges in insertion order (PKG-03, AC 1)
Proof: run crates | test graph_keeps_crates_and_edges_as_given ... ok

**C23** - `9a`, `a-b`, `fn`, `module`, `std` and the empty name are `InvalidName`; the graph is unchanged (PKG-03, AC 2)
Proof: run crates | test graph_rejects_invalid_and_reserved_names ... ok

**C24** - `a -> b -> c -> a` and `a -> a` are `Cycle` with the chain named; no edge is added (PKG-03, AC 3)
Proof: run crates | test graph_rejects_a_cycle_and_names_it ... ok

**C25** - a second dependency named `util` on one crate is `DuplicateDependency` (PKG-03, AC 4)
Proof: run crates | test graph_rejects_a_repeated_dependency_name ... ok

**C26** - an id past the end is `UnknownCrate` as source and as target (PKG-03, AC 5)
Proof: run crates | test graph_rejects_a_foreign_id ... ok

**C27** - `crates/piperine-lang/Cargo.toml` names none of `toml`, `git2`, `piperine-project` (PKG-03, AC 6)
Proof: run crates | test piperine_lang_takes_the_graph_as_data ... ok

**C28** - `top.msl` alone gives a root crate `top` whose root file is `FileId(0)` (PKG-04, AC 1)
Proof: run crates | test single_source_design_has_a_root_crate_named_by_its_file ... ok

**C29** - the last crate of any design is `std`, and it is not the root crate (PKG-04, AC 2)
Proof: run crates | test every_design_holds_std_last ... ok

**C30** - a three-crate graph gives crates `app`, `util`, `base`, `std` with the graph's edges (PKG-04, AC 3)
Proof: run crates | test graph_crates_become_design_crates_in_order ... ok

**C31** - `mod cells;` in a dependency loads `cells.msl` from the dependency's directory (PKG-04, AC 4)
Proof: run crates | test mod_files_load_beside_their_own_crate_root ... ok

**C32** - a missing dependency root is one `E0161` with its path; the root crate still resolves (PKG-04, AC 5)
Proof: run crates | test an_unreadable_crate_root_is_one_e0161 ... ok

**C33** - no declaration of one crate has an ancestor in another; every file's `to_ast` equals its parse (PKG-04, AC 6)
Proof: run crates | test every_crate_keeps_its_authored_tree ... ok

**C34** - disk and memory builds of one two-crate tree give the same dump and crates (PKG-04, AC 7)
Proof: run crates | test disk_and_memory_graphs_build_the_same_design ... ok

**C35** - `util::cells::Cell` resolves segment 0 to `util`'s root and the last to `Cell` (PKG-05, AC 1)
Proof: run crates | test a_dependency_name_starts_a_path ... ok

**C36** - an instance of an imported module links to the declaration in the dependency crate (PKG-05, AC 2)
Proof: run crates | test an_instance_links_to_a_module_of_a_dependency ... ok

**C37** - `crate::K` inside `util` names `util`'s `K` while the root crate has its own `K` (PKG-05, AC 3)
Proof: run crates | test crate_names_the_root_of_the_containing_crate ... ok

**C38** - crate `util` declared as `u2`: `u2::X` resolves, `util::X` is `E0161` (PKG-05, AC 4)
Proof: run crates | test a_renamed_dependency_resolves_by_its_dependency_name ... ok

**C39** - `mod util { .. }` beside a dependency `util`: the type path `util::X` is the local one, zero diagnostics (PKG-05, AC 5)
Proof: run crates | test a_local_name_wins_over_a_dependency_in_a_path ... ok

**C40** - `use util::X;` beside `mod util` and a dependency `util` is one `E0163` with both remedies in the help, and no `E0161` (PKG-05, AC 6)
Proof: run crates | test a_use_path_that_names_a_local_and_a_dependency_is_e0163 ... ok

**C41** - `use crate::util::X;` in the same clash resolves to the local `X`, zero diagnostics (PKG-05, AC 7)
Proof: run crates | test crate_prefix_names_the_local_one_in_a_clash ... ok

**C42** - `std::prelude::Bit` resolves in the root crate and in a dependency with no edge to `std` (PKG-05, AC 8)
Proof: run crates | test std_is_a_dependency_of_every_crate ... ok

**C43** - a port `p: Bit` in a dependency's file resolves to the prelude's `Bit` (PKG-05, AC 9)
Proof: run crates | test the_prelude_reaches_every_crate ... ok

**C44** - a dependency named `filters` wins over a prelude item `filters` (PKG-05, AC 10)
Proof: run crates | test a_dependency_name_hides_a_prelude_item ... ok

**C45** - a transitive crate and the crate's own name are each one `E0161` with the manifest help (PKG-05, AC 11)
Proof: run crates | test a_crate_that_is_no_dependency_is_e0161_with_help ... ok

**C46** - `util::hidden` (no `pub`) is one `E0161` "`hidden` is private" (PKG-05, AC 12)
Proof: run crates | test a_private_item_of_another_crate_is_e0161 ... ok

**C47** - `util::inner::Cell` with `mod inner` private is one `E0161` naming `inner` (PKG-05, AC 13)
Proof: run crates | test a_pub_item_in_a_private_namespace_is_e0161 ... ok

**C48** - `super::X` in a dependency's root file is one `E0161` (PKG-05, AC 14)
Proof: run crates | test super_at_a_crate_root_is_e0161 ... ok

**C49** - after `use util::*;` the `pub` module resolves and the private one is `E0161` (PKG-05, AC 15)
Proof: run crates | test a_glob_of_a_dependency_brings_only_pub_items ... ok

**C50** - `util` has `pub use base::Res;`: `util::Res` resolves from the root crate to `base`'s `Res` (PKG-05, AC 16)
Proof: run crates | test a_pub_use_re_exports_across_crates ... ok

**C51** - a design of an empty source has zero diagnostics and `std`'s root holds `pub mod prelude` (PKG-06, AC 1)
Proof: run crates | test embedded_std_builds_clean_and_has_a_prelude ... ok

**C52** - with a prelude that declares `pub nature Bit`, a port `p: Bit` resolves to it (PKG-06, AC 2)
Proof: run crates | test prelude_items_resolve_without_an_import ... ok

**C53** - a private prelude item and a `pub` item of `std::other` are each `E0161` when used bare (PKG-06, AC 3)
Proof: run crates | test only_pub_items_of_std_prelude_are_implicit ... ok

**C54** - `std_source("lib.msl", ..)` replaces the embedded files: the design holds that text and no other `std` file (PKG-06, AC 4)
Proof: run crates | test std_source_replaces_the_embedded_library ... ok

**C55** - a local `nature Bit` wins over the prelude's `Bit`, with no diagnostic (PKG-06, AC 5)
Proof: run crates | test a_namespace_item_hides_a_prelude_item ... ok

**C56** - with root `top.msl` and `mod b;`, the files are `top.msl`, `b.msl`, then `std`'s (PKG-06, AC 6)
Proof: run crates | test std_files_are_numbered_last ... ok

**C57** - a module in `std`'s `lib.msl` with a port `p: Bit` resolves to the prelude's `Bit` (PKG-06, AC 7)
Proof: run crates | test std_files_see_the_prelude ... ok

**C58** - `design.root_crate()` reports name `top`, a root with the file's items, two files, `is_root` (PKG-07, AC 1)
Proof: run crates | test a_crate_reports_its_name_root_and_files ... ok

**C59** - a module, a nested signal and a `mod` file of the root crate report the root crate; a `std` item reports `std` (PKG-07, AC 2)
Proof: run crates | test declarations_and_files_know_their_crate ... ok

**C60** - `app` lists `(u2, util)` then `(base, base)`; `dependency("u2")` is `util`; `std` is not listed (PKG-07, AC 3)
Proof: run crates | test a_crate_lists_its_declared_dependencies ... ok

**C61** - `design.modules()` holds the root crate's modules; `util.modules()` holds `util`'s (PKG-07, AC 4)
Proof: run crates | test collections_list_the_root_crate_and_a_crate_lists_its_own ... ok

**C62** - `design.module("Cell")` is the root crate's when both declare one, `util`'s when only `util` and `std` do (PKG-07, AC 5)
Proof: run crates | test by_name_lookups_prefer_the_root_crate_then_crate_order ... ok

**C63** - `crates()` is `app`, `util`, `base`, `std`; `krate("util")`, `root_crate()`, `std_crate()` agree (PKG-07, AC 6)
Proof: run crates | test crates_are_found_by_name ... ok

**C64** - for every named declaration of a dependency, `krate.find(d.qualified_name())` is `d` (PKG-07, AC 7)
Proof: run crates | test crate_find_inverts_qualified_name ... ok

**C65** - a three-crate design round-trips: equal design, equal dump, equal crate list (PKG-08, AC 1)
Proof: run crates | test a_multi_crate_design_round_trips_through_json ... ok

**C66** - each of the six links set past its arena gives `Err` with the field named (PKG-08, AC 2)
Proof: run crates | test from_json_rejects_out_of_range_crate_links ... ok

**C67** - an edge `util -> app` added to the JSON of `app -> util` gives `Err` naming the cycle (PKG-08, AC 3)
Proof: run crates | test from_json_rejects_a_dependency_cycle ... ok

**C68** - swapping two crates' root files in the JSON gives `Err` (PKG-08, AC 4)
Proof: run crates | test from_json_rejects_a_crate_whose_root_is_not_its_own ... ok

**C69** - `resolution.md` lists dependency names and `std::prelude` in the scope order; no page says `prelude source` or `.prelude(` (PKG-09, AC 1)
Proof: run docs | test docs_describe_crates_and_no_prelude_sources ... ok

**C70** - `mapping.md` has a `crate` row naming `CrateData`, `Crate` and `design.crates()` (PKG-09, AC 2)
Proof: run docs | test mapping_has_a_crate_row ... ok

**C71** - every `docs/pom` example runs (PKG-09, AC 3)
Proof: `cargo test -p piperine-lang --doc`

**C72** - an empty directory gives one `E0170` with `<dir>/Piperine.toml` in the message (PKG-10, AC 1)
Proof: run manifest | test a_missing_manifest_is_e0170 ... ok

**C73** - `[package` (unclosed) is one `E0170` whose span lies on that line (PKG-10, AC 2)
Proof: run manifest | test invalid_toml_is_e0170_at_its_span ... ok

**C74** - a rendered `E0171` shows the code, `[Piperine.toml:3:1]`, the line and an underline of the key's length (PKG-10, AC 3)
Proof: run manifest | test a_package_error_renders_as_a_miette_report ... ok

**C75** - a full `[package]` table reads back its four fields; the optional two default to empty (PKG-10, AC 4)
Proof: run manifest | test a_valid_manifest_reads_every_package_field ... ok

**C76** - `[plugins]`, `[package] edition` and `[project]` are each one `E0171` at the key (PKG-10, AC 5)
Proof: run manifest | test an_unknown_key_is_e0171_at_the_key ... ok

**C77** - no `[package]`, no `name`, no `version` and `name = 3` are each one `E0171` (PKG-10, AC 6)
Proof: run manifest | test a_missing_or_mistyped_package_key_is_e0171 ... ok

**C78** - `my-pkg`, `9lives`, `module`, `fn` and `std` are each one `E0171` at the string (PKG-10, AC 7)
Proof: run manifest | test a_bad_package_name_is_e0171 ... ok

**C79** - `1.0`, `1.0.0-rc1`, `v1.0.0` and `1.0.x` are each one `E0171`; `0.1.0` and `10.20.30` are accepted (PKG-10, AC 8)
Proof: run manifest | test a_bad_version_is_e0171 ... ok

**C80** - a manifest with an unknown key, a bad name and a bad version gives three `E0171` in source order (PKG-10, AC 9)
Proof: run manifest | test every_fault_of_a_manifest_is_reported ... ok

**C81** - three dependencies (path, git with `tag`, git with `package` and `version`) read back in order (PKG-10, AC 10)
Proof: run manifest | test dependencies_read_in_source_order ... ok

**C82** - `a = {}` and `b = { path = "x", git = "y" }` are each one `E0171` at `a` and at `b` (PKG-10, AC 11)
Proof: run manifest | test a_dependency_needs_exactly_one_source ... ok

**C83** - `branch` with `rev`, and `tag` on a path dependency, are each one `E0171` (PKG-10, AC 12)
Proof: run manifest | test git_refs_are_exclusive_and_git_only ... ok

**C84** - dependencies `std`, `my-dep` and `version = ">=1.0"` are each one `E0171` (PKG-10, AC 13)
Proof: run manifest | test a_bad_dependency_name_or_requirement_is_e0171 ... ok

**C85** - `app -> ../util -> ../base` resolves from a working directory that is none of them (PKG-11, AC 1)
Proof: run paths | test path_dependencies_are_relative_to_their_manifest ... ok

**C86** - the graph of `app -> util -> base` has three crates, their `src/lib.msl` roots and two edges (PKG-11, AC 2)
Proof: run paths | test the_graph_mirrors_the_manifests ... ok

**C87** - `app -> util`, `app -> base`, `util -> base`: `base` is one crate with two incoming edges (PKG-11, AC 3)
Proof: run paths | test a_shared_dependency_is_one_crate ... ok

**C88** - `util = { path = "../nowhere" }` is one `E0172` at `util` in the root manifest (PKG-11, AC 4)
Proof: run paths | test a_path_without_a_package_is_e0172 ... ok

**C89** - a package without `src/lib.msl` is one `E0175` with that path (PKG-11, AC 5)
Proof: run paths | test a_package_without_a_crate_root_is_e0175 ... ok

**C90** - dependency `util` that locates package `utils` is one `E0173` naming `util` and `utils` (PKG-11, AC 6)
Proof: run paths | test a_package_of_another_name_is_e0173 ... ok

**C91** - `u2 = { path = "../util", package = "util" }` gives an edge `u2` to the crate `util` (PKG-11, AC 7)
Proof: run paths | test package_key_renames_a_dependency ... ok

**C92** - each of the eight forms accepts its lower bound and its last admitted version and rejects the one below and the first one past (PKG-11, AC 8)
Proof: run paths | test version_requirements_follow_the_caret_table ... ok

**C93** - `version = "1.2"` on a package `2.0.0` is one `E0173` naming `1.2` and `2.0.0` (PKG-11, AC 9)
Proof: run paths | test an_unsatisfied_requirement_is_e0173 ... ok

**C94** - `a -> b -> c -> a` is one `E0174` in `c`'s manifest with `a -> b -> c -> a` in the message (PKG-11, AC 10)
Proof: run paths | test a_dependency_cycle_is_e0174_with_its_chain ... ok

**C95** - `me = { path = "." }` in package `me` is one `E0174` (PKG-11, AC 11)
Proof: run paths | test a_package_that_depends_on_itself_is_e0174 ... ok

**C96** - a local repository checked out at its default branch head; the crate root exists under `target/deps` (PKG-12, AC 1)
Proof: run git | test a_git_dependency_checks_out_the_default_branch ... ok

**C97** - a branch, a tag and a full commit id each check out their own commit (three distinct file contents) (PKG-12, AC 2)
Proof: run git | test branch_tag_and_rev_select_their_commit ... ok

**C98** - a path that is no repository, and a branch that does not exist, are each one `E0172` (PKG-12, AC 3)
Proof: run git | test an_unreachable_repository_or_ref_is_e0172 ... ok

**C99** - `util@v1` and `util@v2` of one repository are two crates with different roots (PKG-12, AC 4)
Proof: run git | test two_commits_of_one_repository_are_two_crates ... ok

**C100** - two dependents of the same repository and tag share one crate (PKG-12, AC 5)
Proof: run git | test one_commit_of_one_repository_is_one_crate ... ok

**C101** - a repository package with `base = { path = "base" }` resolves `base` inside the checkout (PKG-12, AC 6)
Proof: run git | test a_git_package_resolves_its_own_dependencies ... ok

**C102** - after one resolution the lockfile holds `version = 1` and the entry with the checked-out commit (PKG-13, AC 1)
Proof: run lock | test the_lockfile_records_each_git_package ... ok

**C103** - a new commit on the branch does not move a locked dependency (PKG-13, AC 2)
Proof: run lock | test a_locked_commit_is_kept_when_the_branch_moves ... ok

**C104** - a path-only project has no `Piperine.lock` after resolution (PKG-13, AC 3)
Proof: run lock | test no_git_package_no_lockfile ... ok

**C105** - two resolutions give the same bytes; entries `base`, `util` are in that order (PKG-13, AC 4)
Proof: run lock | test the_lockfile_is_deterministic ... ok

**C106** - after a new commit, `update` moves the lock and the checkout to it (PKG-13, AC 5)
Proof: run lock | test update_moves_the_lock_to_the_new_commit ... ok

**C107** - removing the dependency empties the entries; changing `branch` replaces the entry's `source` and `commit` (PKG-13, AC 6)
Proof: run lock | test stale_entries_leave_the_lockfile ... ok

**C108** - a truncated lockfile and `version = 2` are each one `E0176`; `target/deps` stays absent (PKG-13, AC 7)
Proof: run lock | test an_invalid_lockfile_is_e0176 ... ok

**C109** - after the source repository is deleted, a second resolution still succeeds (PKG-13, AC 8)
Proof: run lock | test a_locked_checkout_needs_no_repository ... ok

**C110** - a lockfile committed in a dependency naming another commit is ignored (PKG-13, AC 9)
Proof: run lock | test only_the_root_lockfile_counts ... ok

**C111** - from `app/src/deep` the root is `app`; from a directory without a manifest above it, `None` (PKG-14, AC 1)
Proof: run project | test the_project_root_is_the_nearest_manifest ... ok

**C112** - `app -> util`: crates `app`, `util`, `std`; `Top`'s instance is `util`'s `Cell`; no diagnostic (PKG-14, AC 2)
Proof: run project | test load_project_builds_the_design_of_a_package_graph ... ok

**C113** - a root manifest with an unknown key gives `Err` with one `E0171` (PKG-14, AC 3)
Proof: run project | test a_package_error_stops_load_project ... ok

**C114** - `use nowhere::X;` in `src/lib.msl` gives `Ok(design)` with one `E0161` (PKG-14, AC 4)
Proof: run project | test a_model_error_is_a_diagnostic_not_a_package_error ... ok

**C115** - the crate's manifest and sources hold none of the retired names and files (PKG-15, AC 1)
Proof: run project | test the_crate_holds_no_retired_surface ... ok

**C116** - the `E01nn` literals of `crates/piperine-project/src` equal the codes of the rows of phase `package` (PKG-15, AC 2)
Proof: run project | test the_crate_reports_exactly_the_package_phase_codes ... ok

**C117** - `piperine-project` builds all targets with no warning and no error (PKG-15, AC 3)
Proof: `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 leaves `E0174` out of §3.5 rule 10 and its `[§3.6]` row (the code-defined-once guard); T3, which defines `E0174` in §3.6, also writes it into those two places (orchestrator, 2026-11-14)
**Dispatched:** B1 (T1-T5, Sonnet) at 5e0b75f3 - returned (T1-T5 cd533e88..ab419043; T1 tests written after text; phases glossary row fixed by orchestrator); B2 (T6-T11, Sonnet) at e10ac044 - returned (T6-T11 8e04a3b9..00bc5848; deviations: fixture prelude as 2nd std file, find() filters 'not in std', E0163 import cascade E0161; gaps: cycle name order, keyword/std root stems); B3 (T12-T18, Sonnet) at 3d3e963c
**Walked:** no screen in this feature; the stories' Independent Tests are the checks above

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4 ------→ T5
Phase 2:  T6 ------→ T7 ------→ T8 ------→ T9 ------→ T10 ------→ T11
Phase 3:  T12 ------→ T13 ------→ T14
Phase 4:  T15 ------→ T16 ------→ T17 ------→ T18
Phase 5:  T19 ------→ T20 ------→ T21 ------→ T22 ------→ T23 ------→ T24 ------→ T25
```

Execution is strictly sequential. 25 tasks pack into 4 batches of whole phases, one Sonnet worker per batch; the Verifier (Opus) runs once after T25, over `check_proofs.py msl-packages`.

| Batch | Phases | Tasks | Layer |
| ----- | ------ | ----- | ----- |
| 1 | 1: Specification: crates and packages | T1 to T5 (5) | `docs/spec/`, `src/msl/` (one row) |
| 2 | 2: Front end: a design over a crate graph | T6 to T11 (6) | `piperine-lang` `src/pom/` |
| 3 | 3, 4: Front end: reflection, serialization, docs; Project: cut down, manifest | T12 to T18 (7) | `src/pom/`, `docs/pom/`; `piperine-project` |
| 4 | 5: Project: resolution, git, lockfile, entry point | T19 to T25 (7) | `piperine-project` |

Batch 3 joins the last front-end phase and the first project phase: T15 to T18 need `pom::is_crate_name` (T6) and nothing later, and seven tasks fit one worker.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: §3.5 Crates | 1 concern, 8 criteria | ✅ Granular |
| T2: The `package` phase; §3.6 layout and manifest keys | 1 concern, 4 criteria | ✅ Granular |
| T3: §3.6 dependencies, requirements, lockfile, error table | 1 concern, 5 criteria | ✅ Granular |
| T4: `extern crate` is a removed form | 1 concern, 2 criteria | ✅ Granular |
| T5: Appendix G, glossary and file map | 1 concern, 2 criteria | ✅ Granular |
| T6: `CrateGraph`: the crate graph as data | 1 concern, 6 criteria | ✅ Granular |
| T7: The standard library is a source tree; the prelude is `std::prelude` | 1 concern, 7 criteria | ✅ Granular |
| T8: Crates in the `Design` | 1 concern, 4 criteria | ✅ Granular |
| T9: The builder takes a crate graph | 1 concern, 5 criteria | ✅ Granular |
| T10: Dependency names start a path; `crate` is per crate | 1 concern, 10 criteria | ✅ Granular |
| T11: Visibility and diagnostics across crates | 1 concern, 6 criteria | ✅ Granular |
| T12: Reflection over crates | 1 concern, 5 criteria | ✅ Granular |
| T13: Serialization and `Design::check` over crates | 1 concern, 4 criteria | ✅ Granular |
| T14: `docs/pom`: crates | 1 concern, 3 criteria | ✅ Granular |
| T15: Cut `piperine-project` down | 1 concern, 2 criteria | ✅ Granular |
| T16: `PackageError`; a manifest that cannot be read | 1 concern, 3 criteria | ✅ Granular |
| T17: The `[package]` table | 1 concern, 6 criteria | ✅ Granular |
| T18: The `[dependencies]` table | 1 concern, 4 criteria | ✅ Granular |
| T19: Path dependencies become a `CrateGraph` | 1 concern, 5 criteria | ✅ Granular |
| T20: The located package is the one asked for | 1 concern, 4 criteria | ✅ Granular |
| T21: Cycles | 1 concern, 2 criteria | ✅ Granular |
| T22: Git dependencies | 1 concern, 6 criteria | ✅ Granular |
| T23: The lockfile is written and honoured | 1 concern, 4 criteria | ✅ Granular |
| T24: Update, stale entries, an invalid lockfile, no network | 1 concern, 5 criteria | ✅ Granular |
| T25: `load_project`; the code guard | 1 concern, 5 criteria | ✅ Granular |

T7 and T8 each touch several files because one storage decision (the library as a tree; the crate table) moves call sites that must compile in the same commit. T10 and T22 carry ten and six criteria of one mechanism each.

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | None | ✅ Match |
| T2 | T1 | T1 | ✅ Match |
| T3 | T2 | T2 | ✅ Match |
| T4 | T3 | T3 | ✅ Match |
| T5 | T4 | T4 | ✅ Match |
| T6 | T5 | T5 | ✅ Match |
| T7 | T6 | T6 | ✅ Match |
| T8 | T7 | T7 | ✅ Match |
| T9 | T8 | T8 | ✅ Match |
| T10 | T9 | T9 | ✅ Match |
| T11 | T10 | T10 | ✅ Match |
| T12 | T11 | T11 | ✅ Match |
| T13 | T12 | T12 | ✅ Match |
| T14 | T13 | T13 | ✅ Match |
| T15 | T14 | T14 | ✅ Match |
| T16 | T15 | T15 | ✅ Match |
| T17 | T16 | T16 | ✅ Match |
| T18 | T17 | T17 | ✅ Match |
| T19 | T18 | T18 | ✅ Match |
| T20 | T19 | T19 | ✅ Match |
| T21 | T20 | T20 | ✅ Match |
| T22 | T21 | T21 | ✅ Match |
| T23 | T22 | T22 | ✅ Match |
| T24 | T23 | T23 | ✅ Match |
| T25 | T24 | T24 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | specification text | integration | integration | ✅ OK |
| T2 | specification text | integration | integration | ✅ OK |
| T3 | specification text | integration | integration | ✅ OK |
| T4 | parser, removed forms | integration | integration | ✅ OK |
| T5 | specification text | integration | integration | ✅ OK |
| T6 | crate graph | integration | integration | ✅ OK |
| T7 | std and the prelude | integration | integration | ✅ OK |
| T8 | crates in the design | integration | integration | ✅ OK |
| T9 | lowering per crate | integration | integration | ✅ OK |
| T10 | resolution across crates | integration | integration | ✅ OK |
| T11 | resolution across crates | integration | integration | ✅ OK |
| T12 | reflection | integration | integration | ✅ OK |
| T13 | serialization | integration | integration | ✅ OK |
| T14 | reference docs | integration | integration | ✅ OK |
| T15 | entry point and hygiene | integration | integration | ✅ OK |
| T16 | manifest reader | integration | integration | ✅ OK |
| T17 | manifest reader | integration | integration | ✅ OK |
| T18 | manifest reader | integration | integration | ✅ OK |
| T19 | resolver | integration | integration | ✅ OK |
| T20 | resolver | integration | integration | ✅ OK |
| T21 | resolver | integration | integration | ✅ OK |
| T22 | git dependencies | integration | integration | ✅ OK |
| T23 | lockfile | integration | integration | ✅ OK |
| T24 | lockfile | integration | integration | ✅ OK |
| T25 | entry point and hygiene | integration | integration | ✅ OK |

T7 also adds the data-only `std/*.msl` (matrix: none); the task takes the higher requirement of the layer it changes.

---

## Requirement coverage

| Requirement | Criteria | Checks | Tasks |
| ----------- | -------- | ------ | ----- |
| PKG-01 | 10 | C1 to C10 | T1, T4 |
| PKG-02 | 11 | C11 to C21 | T2, T3, T5 |
| PKG-03 | 6 | C22 to C27 | T6 |
| PKG-04 | 7 | C28 to C34 | T8, T9 |
| PKG-05 | 16 | C35 to C50 | T10, T11 |
| PKG-06 | 7 | C51 to C57 | T7 |
| PKG-07 | 7 | C58 to C64 | T8, T12 |
| PKG-08 | 4 | C65 to C68 | T13 |
| PKG-09 | 3 | C69 to C71 | T14 |
| PKG-10 | 13 | C72 to C84 | T16, T17, T18 |
| PKG-11 | 11 | C85 to C95 | T19, T20, T21 |
| PKG-12 | 6 | C96 to C101 | T22 |
| PKG-13 | 9 | C102 to C110 | T23, T24 |
| PKG-14 | 4 | C111 to C114 | T15, T25 |
| PKG-15 | 3 | C115 to C117 | T15, T25 |

117 criteria, 117 checks, 25 tasks, 5 phases.

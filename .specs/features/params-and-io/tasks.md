# Params and I/O Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/params-and-io/design.md`
**Status**: Approved
**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-project -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run speccodes**: `cargo test -p piperine-lang --test spec_codes`
**Run typesvalues**: `cargo test -p piperine-lang --test types_values`
**Run typesmodules**: `cargo test -p piperine-lang --test types_modules`
**Run typescarriers**: `cargo test -p piperine-lang --test types_carriers`
**Run typesresults**: `cargo test -p piperine-lang --test types_results`
**Run typesunknowns**: `cargo test -p piperine-lang --test types_unknowns`
**Run typesstructure**: `cargo test -p piperine-lang --test types_structure`
**Run elabvalues**: `cargo test -p piperine-lang --test elab_values`
**Run elabanalog**: `cargo test -p piperine-lang --test elab_analog`
**Run elabentry**: `cargo test -p piperine-lang --test elab_entry`
**Run elabinstances**: `cargo test -p piperine-lang --test elab_instances`
**Run elabparams**: `cargo test -p piperine-lang --test elab_params`
**Run elabfiles**: `cargo test -p piperine-lang --test elab_files`
**Run msformat**: `cargo test -p piperine-lang --test msl_format`
**Run stdgalleries**: `cargo test -p piperine-lang --test std_galleries`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run irchecks**: `cargo test -p piperine-ir --test check`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run apisources**: `cargo test -p piperine-api --test host_sources`
**Run apisession**: `cargo test -p piperine-api --test session`
**Run runerrors**: `cargo test -p piperine-parity --test run_errors`
**Run sourcehost**: `cargo test -p piperine-parity --test source_host`
**Run sourceparams**: `cargo test -p piperine-parity --test source_params`
**Run sourcetables**: `cargo test -p piperine-parity --test source_tables`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run clicommands**: `cargo test -p piperine-cli --test commands`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run bookdigital**: `cargo test -p piperine --test book_digital`
**Run bookcorestd**: `cargo test -p piperine --test book_core_std`
**Run bookpackages**: `cargo test -p piperine --test book_packages`
**Run bookcheatsheet**: `cargo test -p piperine --test book_cheatsheet`
**Run bookinventory**: `cargo test -p piperine --test book_inventory`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** The spec's open questions were answered 2026-11-18 (user: "Sobre as open questions, vou de aceito todas ok?"); the spec was amended 2026-11-20 under the AFK rule (spec.md "Amendments" A1 to A6). The defaults taken while the user was away are design.md "Decisions taken (user AFK)" D1 to D25; every task follows them as written.

**Base and order.** Batches B1 to B4 (phases 1 to 7, T1 to T28) start from `main` at `494a7f75` and need nothing that is not there. B5 and B6 (phases 8 to 12, T29 to T44) start only once `composite-values` phases A–B (its T1 to T63) are on `main`; T29 rebases B1 to B4 onto that tree. If they are not on `main` when B4 closes, the feature pauses there and reports (design.md D7).

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-b TMPDIR=/var/tmp/piperine-tmp-b CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command. Cargo with `-p <crate>`, or plain `cargo test` at the repository root (the default members); never `--workspace` (`piperine-lang-server` is postponed and known broken), never `cargo clean`. Long jobs run in the background with no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Parity is untouchable (ngspice first).** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. A new parity comparison uses an existing contract as it is (D11). Every buck target passes after every task with `Gate: full`.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both and the spec line.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority; where today's behaviour contradicts it for a construct this feature touches, the task fixes it (design.md "Divergences found"). A spec text that is wrong is amended explicitly with its `spec_text` guard, never left. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria and the rule each cites, or from a value Rust computes in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`) is part of every task's done-criteria.** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; object-oriented names with no abbreviation; no god method and no needless abstraction; the data path evident. Every new or changed struct, enum, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **The guard of codes.** From T2 on, a task that raises a code listed in `crates/piperine-lang/tests/support/code_owners.tsv` removes its row in the same commit (`no_owned_code_is_raised` fails otherwise).
- **Conventions.** New IR enum variants are appended last (msl-digital convention). The interpreter refuses every new IR form by name (codegen context R1) in the task that adds it. Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `inventory.tsv` rows are edited in place, never reordered.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend), MD-72 (root Book tests).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | every code with its span and named subject; one test per AC | `crates/piperine-lang/tests/types_*.rs` | `cargo test -p piperine-lang --test types_<area>` |
| Elaborator (`piperine-lang/src/elab`) | integration | the IR each form lowers to; every elaboration code | `crates/piperine-lang/tests/elab_*.rs` (new `elab_files.rs`) | `cargo test -p piperine-lang --test elab_<area>` |
| The guard of codes | integration | every code raised or owned; stale entries; no `P0002` | `crates/piperine-lang/tests/spec_codes.rs` | `cargo test -p piperine-lang --test spec_codes` |
| Formatter output of shipped sources | integration | each file a fixed point; printed tree unchanged | `crates/piperine-lang/tests/{msl_format,std_galleries}.rs` | `cargo test -p piperine-lang --test msl_format` / `--test std_galleries` |
| IR (`piperine-ir`) | integration | the new expressions built, checked, printed | `crates/piperine-ir/tests/check.rs` | `cargo test -p piperine-ir --test check` |
| Interpreter refusals | integration | each new form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan and printer | integration | from source on the Rust backend, values from the rule | `crates/piperine-parity/tests/{source_params,source_tables}.rs` | `cargo test -p piperine-parity --test source_params` / `--test source_tables` |
| Host library (`piperine-api`) | integration | each host action, its refusal and its code | `crates/piperine-api/tests/{host_sources,session}.rs` | `cargo test -p piperine-api --test host_sources` / `--test session` |
| ngspice parity | integration | each comparison under an existing contract | `crates/piperine-parity/tests/source_host.rs`, `tests/e2e/twins/` | `cargo test -p piperine-parity --test source_host` |
| Run faults | integration | the code of a run panic | `crates/piperine-parity/tests/run_errors.rs` | `cargo test -p piperine-parity --test run_errors` |
| CLI | integration | the binary's exit and its code | `crates/piperine-cli/tests/{run,commands,book}.rs` | `cargo test -p piperine-cli --test run` / `--test commands` / `--test book` |
| The Book | integration | each row this feature changes | `tests/book/*.rs`, `tests/book/inventory.tsv` | `cargo test -p piperine --test book_<chapter>` |
| Spec text and repository records | integration | one assertion per amended sentence or removed `GAPS.md` row | `crates/piperine-lang/tests/spec_text.rs`, `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-lang --test spec_text`, `cargo test -p piperine-cli --test docs` |
| Contract re-read (T1) | none | - (build gate; findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with checker or elaborator tests only | the `Run` line(s) the task's `Done when` names |
| Full | After a task that touches the host, the plan, the printer, the CLI, the shipped sources or the Book | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins --test commands`, `cargo test -p piperine-api --test buck_long`, `cargo test -p piperine-transpile`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members; `piperine-lang-server` is not one) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Contract and the guard of codes

```
T1 → T2
```

### Phase 2: Type errors carry spec codes (X5, L20, PI-06)

```
T3 → T4 → T5 → T6 → T7
```

### Phase 3: Codes of constructs that already run (PI-12)

```
T8 → T9 → T10 → T11 → T12 → T13 → T14 → T15
```

### Phase 4: The top's continuous ports (L39, PI-07)

```
T16 → T17 → T18 → T19 → T20
```

### Phase 5: Module-typed params (PI-04)

```
T21 → T22
```

### Phase 6: `include_str!` (PI-08)

```
T23 → T24
```

### Phase 7: Formatting, the spline note and the early records

```
T25 → T26 → T27 → T28
```

### Phase 8: Contract after `composite-values` (waits for its phases A–B on `main`)

```
T29
```

### Phase 9: Params of integer, enum and `Option` type (PI-01 to PI-03, PI-05)

```
T30 → T31 → T32 → T33 → T34 → T35
```

### Phase 10: `E0122` for named functions

```
T36
```

### Phase 11: Files and tables (PI-09, PI-10)

```
T37 → T38 → T39 → T40 → T41 → T42 → T43
```

### Phase 12: Closure

```
T44
```

---

## Task Breakdown

### T1: Re-read the anchors

**What**: Re-read every anchor of design.md "What the code does today" at `HEAD`; for `E0244`, `E0354` and `E0906` write the smallest source that reaches each condition and the diagnostic it gets today; run the formatter over `crates/piperine-lang/std/`, `examples/buck/src/` and the D/E/F fences and count the files it changes; say whether `tests/e2e/buck` is a copy of `examples/buck`. Every finding goes on the Handoff's `**Settled mid-build:**` line; a finding that contradicts design.md stops the batch and is reported.
**Where**: `.specs/features/params-and-io/tasks.md`
**Depends on**: None
**Reuses**: design.md
**Requirement**: PI-13
**Executor**: Sonnet

**Done when**:

- [x] The `**Build**` line is green at `HEAD` before any change (red stops the feature)
- [x] The Handoff names, for each of `E0244`, `E0354`, `E0906`, the source that reaches it and today's code
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(params-and-io): re-read the anchors`

---

### T2: The guard of spec codes

**What**: Create `crates/piperine-lang/tests/spec_codes.rs` (`CodeTable` from every `| \`Ennnn\` |` row of `docs/spec/*.md` through `support/spec_tables.rs`, `SourceSpellings` from every non-comment line of `crates/*/src/**/*.rs`, `CodeOwners` from `tests/support/code_owners.tsv`) and the allow-list `crates/piperine-lang/tests/support/code_owners.tsv` with the 68 rows of spec.md "Gaps closed" (code, owner feature, construct).
**Where**: `crates/piperine-lang/tests/spec_codes.rs`
**Depends on**: T1
**Reuses**: `crates/piperine-lang/tests/support/spec_tables.rs`
**Requirement**: PI-13
**Executor**: Sonnet

**Done when**:

- [x] `every_spec_code_is_raised_or_owned`, `no_owned_code_is_raised`, `every_owned_code_names_its_owner_and_construct` pass under `**Run speccodes**`
- [x] The target opens with a `//!` scope header; `suite_hygiene` passes
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(lang): the guard of spec codes`

---

### T3: `E0182` for `as`

**What**: `Checker::cast` reports `E0182` naming both types for an `as` between two types that are not a row of §1.4.5, in place of the mismatch text.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: None
**Reuses**: `Checker::coded`
**Requirement**: PI-06
**Executor**: Sonnet

**Done when**:

- [x] `an_as_between_types_without_a_row_is_e0182` passes under `**Run typesvalues**`
- [x] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a bad as is E0182`

---

### T4: `E0141` for the arity of a call through a value

**What**: `Checker::apply` reports `E0141` with the expected and found counts where it reported `P0002` (a call through a function value, a probe, a builtin).
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T3
**Reuses**: the `E0141` arm of `bind_arguments`
**Requirement**: PI-06
**Executor**: Sonnet

**Done when**:

- [x] `a_call_through_a_function_value_with_the_wrong_arity_is_e0141` passes under `**Run typesvalues**`
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): arity of a call through a value is E0141`

---

### T5: `E0180` at every other mismatch

**What**: `check` passes `E0180` to `check_as`; every remaining "mismatched types" site (`types/expr.rs`, `types/call.rs`, `types/analog.rs`) carries `E0180`; every test and Book listing that asserted `P0002` asserts the new code (design.md "may move"); `spec_codes.rs` gains `no_source_spells_p0002`.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T4
**Reuses**: `check_as`
**Requirement**: PI-06
**Executor**: Sonnet

**Done when**:

- [x] `a_mismatch_no_row_names_is_e0180_naming_both_types` passes under `**Run typesvalues**`
- [x] `no_source_spells_p0002` passes under `**Run speccodes**`
- [x] `no_book_listing_prints_p0002` passes under `**Run clibook**`
- [x] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [x] `tests/book/inventory.tsv` rows that printed `P0002` are corrected in place; `**Run bookinventory**` passes
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(types): a plain mismatch is E0180`

---

### T6: A param that hides a function (L20)

**What**: When a callee path resolves to a param, a `let` or a `const` that is not a function and a function of that name is in an enclosing namespace, the checker reports `E0180` "expected a function, found `<type>`" with a note at the function's declaration (§3.5.4), in place of `P0001` "call of this declaration".
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: T5
**Reuses**: the POM's name resolution of the enclosing namespace
**Requirement**: PI-06
**Executor**: Sonnet

**Done when**:

- [x] `a_param_hiding_a_function_is_e0180_with_a_note_at_the_function` passes under `**Run typesvalues**`
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a param hiding a function is E0180 with a note`

---

### T7: `E0188` for const expressions

**What**: `const_fault` reports `E0188` naming the `const` when the position is a `const` item: a `Dynamic` read (a probe, a param, a carrier), a `NoValue` outside a trait, and a `const` that depends on itself (§3.9.2); a param default keeps `E0321`.
**Where**: `crates/piperine-lang/src/elab/diagnose.rs`
**Depends on**: T6
**Reuses**: `ConstFault`
**Requirement**: PI-06
**Executor**: Sonnet

**Done when**:

- [x] `a_const_reading_a_run_value_is_e0188`, `a_const_that_depends_on_itself_is_e0188` pass under `**Run elabvalues**`
- [x] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [x] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(elab): a const that is not a const expression is E0188`

---

### T8: `E0204` and `E0205` on natures

**What**: A quantity attribute other than `access`, `units`, `abstol`, one of the wrong type, or an `abstol` that is not positive is `E0204` naming it; equal `access` names of one nature are `E0205` naming them.
**Where**: `crates/piperine-lang/src/elab/nature.rs`
**Depends on**: None
**Reuses**: `Elaboration::report`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_quantity_attribute_outside_the_three_is_e0204`, `a_non_positive_abstol_is_e0204`, `equal_access_names_are_e0205` pass under `**Run elabanalog**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(elab): nature attribute codes E0204 and E0205`

---

### T9: `E0315` on a carrier port that is not `in`

**What**: `Checker::connect` reports `E0315` naming the port when an expression is connected to a carrier port that is not an `in` port (`R(2.0 * v(a), gnd)`).
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: T8
**Reuses**: `Checker::connect`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `an_expression_on_a_carrier_port_that_is_not_in_is_e0315` passes under `**Run typesmodules**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): an expression on a carrier port that is not in is E0315`

---

### T10: `E0333` on an instance used as a value

**What**: The name of an instance of a module without a result, read as a value, is `E0333` naming the instance (§8.2.14).
**Where**: `crates/piperine-lang/src/types/result.rs`
**Depends on**: T9
**Reuses**: `Checker::placement_type`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `an_instance_without_a_result_used_as_a_value_is_e0333` passes under `**Run typesmodules**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): an instance without a result used as a value is E0333`

---

### T11: `E0904` from the attribute table of A.1

**What**: `types/attributes.rs` (`AttributeCatalogue`): A.1's table as data (attribute name, the item kinds it may sit on); one walk over every attribute of the design reports `E0904` naming the attribute and the item for one A.1 does not list (`#[cfg(test)]`, `#[allow(..)]`) or one on the wrong kind of item (`#[link]` on a `fn`, `#[repr(u2)]` on a `struct`).
**Where**: `crates/piperine-lang/src/types/attributes.rs`
**Depends on**: T10
**Reuses**: `types/compound.rs` attribute reads
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `an_attribute_a1_does_not_list_is_e0904`, `a_listed_attribute_on_the_wrong_item_is_e0904` pass under `**Run typesstructure**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Every stdlib, gallery and Book source still checks (no shipped source carries an unlisted attribute); `**Run stdgalleries**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(types): an attribute A.1 does not list is E0904`

---

### T12: `E0244` on a node the module cannot reach

**What**: A probe or a contribution naming a node §6 rule 7 does not let the module reach (a child's node that is not `pub`; the form T1 recorded) is `E0244` naming the node.
**Where**: `crates/piperine-lang/src/types/analog.rs`
**Depends on**: T11
**Reuses**: the `inst.name` resolution of `analog-structure`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_probe_on_a_childs_private_node_is_e0244` passes under `**Run typescarriers**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a probe on a node the module cannot reach is E0244`

---

### T13: `E0354` on a result read out of place

**What**: A continuous result read where probes are not allowed, or an `Event` result outside an event position, is `E0354` naming the placement (§8.3.10).
**Where**: `crates/piperine-lang/src/types/result.rs`
**Depends on**: T12
**Reuses**: `Checker::placement_type`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_continuous_result_read_where_probes_are_not_allowed_is_e0354`, `an_event_result_outside_an_event_position_is_e0354` pass under `**Run typesresults**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a result read out of place is E0354`

---

### T14: `E0906` on `Unknown` with no type

**What**: `Unknown` where no type can be inferred (`let x = Unknown;`) is `E0906`.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T13
**Reuses**: the `Unknown` arm of `infer`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `unknown_with_no_inferable_type_is_e0906` passes under `**Run typesunknowns**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): Unknown with no type is E0906`

---

### T15: `E0105` on a run panic

**What**: `Failure::from_fault` (one decoding place, D19): a device fault whose message is a model panic under known control is a `Failure` with code `E0105` naming the operation, the instance and the time; a payload that starts with a spec code keeps that code.
**Where**: `crates/piperine-api/src/failure.rs`
**Depends on**: T14
**Reuses**: `piperine_abi` thunk fault text, `rt::Panic`
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_division_by_zero_under_known_control_fails_the_run_with_e0105`, `an_unwrap_of_none_under_known_control_fails_the_run_with_e0105` pass under `**Run runerrors**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a run panic carries E0105`

---

### T16: Spec text: the host drives a top's node ports

**What**: `docs/spec/part_iii_structure.md` §8.1.14 and `part_viii_host.md` §23.1 rule 1 state what the host does with a node port (a potential source to `gnd` per port, DC, AC and piecewise-linear) and with a value input; §23.1's errors table gains `E0810` (host) and `E0811` (elaboration) (D10), each asserted in `spec_text.rs` (appended).
**Where**: `docs/spec/part_viii_host.md`
**Depends on**: None
**Reuses**: `spec_text.rs` conventions (MD-57)
**Requirement**: PI-07
**Executor**: Sonnet

**Done when**:

- [ ] `the_host_drives_a_tops_node_ports_e0810_e0811` passes under `**Run spectext**`
- [ ] The guard sees `E0810` and `E0811` as spec codes; they are raised by T17 and T18, so they enter `code_owners.tsv` under `params-and-io` here and leave it there
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): the host drives a top's node ports`

---

### T17: Elaborator: one host source per node port of the top

**What**: `elab/host_sources.rs` (`HostSources`): when the top has a node port, the IR gains the body `HostSource` (design.md, built from IR expressions, no MSL source, D8) and one instance per node port at `<top>.<port>.source`, its node bound to the port; a value input of the top is `E0811` naming it; the `P0001` "continuous port" refusal leaves; the POM is unchanged.
**Where**: `crates/piperine-lang/src/elab/host_sources.rs`
**Depends on**: T16
**Reuses**: `IrBuilder`, the analog lowering's contribution and `ac_stim` IR forms
**Requirement**: PI-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_top_node_port_gets_one_host_source_instance`, `a_top_value_input_is_e0811`, `the_pom_keeps_a_top_with_node_ports_as_written` pass under `**Run elabentry**`
- [ ] The interpreter refuses a `HostSource` body by name if it cannot run it (design.md R2)
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): a host source per node port of the top`

---

### T18: Host: a DC and AC source per node port, `E0810`

**What**: `piperine-api/src/source.rs` (`NodeSource`, `NodeSources`): `Session::source(port, NodeSource)` before a run sets the source instance's params (DC: `v0 = v1 = dc`; AC: `mag`, `phase`); a run started with a node port without a source is `E0810` naming it. Create `crates/piperine-api/tests/host_sources.rs` and `crates/piperine-parity/tests/source_host.rs` (the RC of `ac_rc.cir`/`tran_rc.cir` as a top with the port `vin`, D11).
**Where**: `crates/piperine-api/src/source.rs`
**Depends on**: T17
**Reuses**: `Session::set_param`, `ParamPath`, `ToleranceContract::AC_MAGNITUDE`/`AC_PHASE`/`TRAN_VOLTAGE`
**Requirement**: PI-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_dc_source_sets_the_port_at_the_operating_point`, `an_undriven_node_port_is_e0810`, `a_node_port_tied_to_gnd_inside_is_still_refused_undriven` pass under `**Run apisources**`
- [ ] `a_host_driven_rc_matches_ac_rc_in_ac`, `a_host_driven_rc_step_matches_tran_rc` pass under `**Run sourcehost**`
- [ ] No `ToleranceContract` and no twin golden changes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): host sources on a top's node ports`

---

### T19: Host: piecewise-linear sources and jumps

**What**: `NodeSources` turns a `NodeSource`'s corners into timed sets of the segment params at each corner's time (D9), refusing corners not strictly increasing in time with `E0802`; `Session::source_at(port, value, at)` is a jump (a timed set with `v0 = v1`). Add the twin `tests/e2e/twins/tran_pwl_rc.cir` (no `.command`) and its golden.
**Where**: `crates/piperine-api/src/source.rs`
**Depends on**: T18
**Reuses**: `Session::set_at`
**Requirement**: PI-07
**Executor**: Sonnet

**Done when**:

- [ ] `a_pwl_source_lands_on_every_corner`, `a_timed_source_change_is_a_jump` pass under `**Run apisources**`
- [ ] `a_host_pwl_into_the_rc_matches_tran_pwl_rc` passes under `**Run sourcehost**`
- [ ] The CLI twin count of `crates/piperine-cli/tests/twins.rs` is unchanged
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): piecewise-linear host sources and jumps`

---

### T20: CLI: a top with a node port is `E0810`

**What**: `piperine run` and `piperine test` of a top with a node port fail with `E0810` naming the port (no manifest key drives a port, X8); the tests that pinned the old `P0001` move (design.md).
**Where**: `crates/piperine-cli/tests/run.rs`
**Depends on**: T19
**Reuses**: the CLI's failure printing
**Requirement**: PI-07
**Executor**: Sonnet

**Done when**:

- [ ] `run_of_a_top_with_a_node_port_is_e0810` passes under `**Run clirun**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(cli): a top with a node port is E0810`

---

### T21: Checker: a param of module type

**What**: `Checker::param` accepts a module type; its default or binding is a template value `Name { params }` typed as that module, a template of another module `E0180` naming both; a placement whose head is such a param is checked against the module's ports, and a param block on it is `E0361`; the `P0001` "placement through a template value" leaves for this form.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: None
**Reuses**: `Checker::placement`, `Checker::connections`
**Requirement**: PI-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_module_typed_param_takes_a_template`, `a_param_block_on_a_placement_through_a_param_is_e0361`, `a_template_of_another_module_is_e0180_naming_both` pass under `**Run typesmodules**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): params of module type`

---

### T22: Elaborator: placement through a module-typed param

**What**: The param is structural; the elaborator folds its template (module and param block, `E0319` for a required param left unbound) and places the concrete module where `cell(c)` is written; a parent's binding replaces the default; the POM keeps `cell(c)`, the IR holds the concrete instance (MD-25); a host `--param`/`set` of it is `E0802` (D13).
**Where**: `crates/piperine-lang/src/elab/children.rs`
**Depends on**: T21
**Reuses**: `ChildValue`, the structural-param path of `analog-structure`
**Requirement**: PI-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_placement_through_a_module_param_places_its_template`, `a_parent_binding_replaces_the_template`, `a_template_leaving_a_required_param_unbound_is_e0319`, `the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module` pass under `**Run elabinstances**`
- [ ] `a_module_typed_param_is_not_set_by_text` passes under `**Run apisession**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): placement through a module-typed param`

---

### T23: Checker: `include_str!` in a `const` only

**What**: `include_str!(..)` is `&str` in the initializer of a `const` item; anywhere else `E0610`; the `P0001` "`include_str!`" leaves.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: None
**Reuses**: `Context::Const`
**Requirement**: PI-08
**Executor**: Sonnet

**Done when**:

- [ ] `include_str_outside_a_const_is_e0610`, `include_str_in_a_const_is_a_str` pass under `**Run typesvalues**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): include_str in a const`

---

### T24: Elaborator: files read once at elaboration

**What**: `elab/files.rs` (`IncludedFiles`): the path resolved against the directory of the source file (absolute as it is; a source with no directory `E0612`, D14), each resolved path read once and cached, `E0612` naming the resolved path, `E0613` with the byte offset of the first invalid sequence (D15); the text is the `const`'s `Constant::Text`. Create `crates/piperine-lang/tests/elab_files.rs` with a fixture package under `crates/piperine-lang/tests/fixtures/files/`.
**Where**: `crates/piperine-lang/src/elab/files.rs`
**Depends on**: T23
**Reuses**: `ConstEval`, the `SourceMap` paths
**Requirement**: PI-08
**Executor**: Sonnet

**Done when**:

- [ ] `include_str_reads_the_file_beside_the_source`, `an_unreadable_file_is_e0612_naming_the_path`, `a_file_that_is_not_utf8_is_e0613_with_the_offset`, `a_changed_file_needs_a_new_elaboration`, `one_file_named_twice_is_read_once` pass under `**Run elabfiles**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): include_str reads its file once`

---

### T25: The stdlib in the formatter's form

**What**: Every file of `crates/piperine-lang/std/` rewritten by `Formatter::format`; `msl_format` gains the guard and the printed-tree comparison.
**Where**: `crates/piperine-lang/std/`
**Depends on**: None
**Reuses**: `Formatter::format`, `msl::print`
**Requirement**: PI-11
**Executor**: Sonnet

**Done when**:

- [ ] `every_stdlib_file_is_in_the_formatters_form`, `formatting_keeps_the_printed_tree_of_every_shipped_source` pass under `**Run msformat**`
- [ ] `std_galleries`, `std_excerpts`, `std_library` and `spec_text` pass unedited
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `style(std): the stdlib in the formatter's form`

---

### T26: The buck and the galleries in the formatter's form

**What**: `examples/buck/src/*.msl` (and `tests/e2e/buck` if T1 found it a copy, D20) and every `rust` fence of Appendices D, E and F rewritten by the formatter; `std_galleries` guards the fences; `piperine fmt --check` exits 0 on the stdlib and in `examples/buck`.
**Where**: `examples/buck/src/`
**Depends on**: T25
**Reuses**: `Formatter::format`
**Requirement**: PI-11
**Executor**: Sonnet

**Done when**:

- [ ] `every_gallery_fence_is_in_the_formatters_form` passes under `**Run stdgalleries**`
- [ ] `fmt_check_passes_on_every_stdlib_file`, `fmt_check_passes_in_examples_buck` pass under `**Run clicommands**`
- [ ] Every buck target passes with unchanged assertions; the printed tree of each buck file is unchanged
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `style(buck): the buck and the galleries in the formatter's form`

---

### T27: Appendix C.2: the spline end conditions

**What**: Appendix C.2 documents `table_model`'s end conditions (cubic not-a-knot; quadratic first segment through the first three points) and the derivative at a sample (D18), asserted in `spec_text.rs` (appended).
**Where**: `docs/spec/appendix_c_implementation.md`
**Depends on**: T26
**Reuses**: `spec_text.rs`
**Requirement**: PI-10
**Executor**: Sonnet

**Done when**:

- [ ] `appendix_c2_states_the_spline_end_conditions` passes under `**Run spectext**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): table_model spline end conditions`

---

### T28: Early records

**What**: `GAPS.md` drops X5, L20, L39 and H12 (keeping its numbering) and §10 lists exactly the rows of `code_owners.tsv`; ROADMAP item 10 row says what B1 to B4 delivered and that B5–B6 wait on `composite-values` A–B; `.specs/STATE.md` gets the `MD-` entry (next free number, D21); `docs.rs` assertions appended.
**Where**: `GAPS.md`
**Depends on**: T27
**Reuses**: `crates/piperine-cli/tests/docs.rs`
**Requirement**: PI-14
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_no_longer_lists_x5_l20_l39_h12` passes under `**Run docs**`
- [ ] If `composite-values` A–B is not on `main`, the Handoff says the feature pauses here (D7)
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: params-and-io early records`

---

### T29: Rebase and re-read the anchors

**What**: Rebase the branch on `main` once `composite-values` A–B is there; re-read `CompositeLayout`, `TypeArena`, `ConstValue`/`Constant::Aggregate`, `types/class.rs`, the host's `set` by leaf path; every `P0002` the rebase brings in becomes the code `docs/spec` names; `code_owners.tsv` loses every row the guard now reports stale (`E0102`, `E0181`, `E0183` to `E0186`, and any other); add the check of the handed-over `E0102`.
**Where**: `crates/piperine-lang/tests/support/code_owners.tsv`
**Depends on**: None
**Reuses**: design.md
**Requirement**: PI-13
**Executor**: Sonnet

**Done when**:

- [ ] `every_spec_code_is_raised_or_owned`, `no_owned_code_is_raised`, `no_source_spells_p0002` pass under `**Run speccodes**`
- [ ] `a_borrow_outside_a_call_argument_is_e0102` passes under `**Run typesvalues**`
- [ ] The `**Build**` line and every buck target pass at the rebased `HEAD`; the findings go on the Handoff
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `chore(params-and-io): rebase on composite-values`

---

### T30: Checker: integer, enum and `Option` params

**What**: `Checker::param` accepts `UInt<N>`/`Int<N>` (1..=64), unit enums and `Option<T>` of these, with defaults and `where`; a `where` reading an `Option` param as a number is `E0180`; the `P0001` "param of type" leaves for these types.
**Where**: `crates/piperine-lang/src/types/module.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `composite-values`' param typing (its T47)
**Requirement**: PI-01
**Executor**: Sonnet

**Done when**:

- [ ] `integer_enum_and_option_params_type_check`, `an_option_param_read_as_a_number_is_e0180` pass under `**Run typesmodules**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): integer, enum and Option params`

---

### T31: Elaborator: integer, enum and `Option` params

**What**: Defaults and bindings fold to `Constant::Int`, the IR enum constant and the IR `Option` constant (an `as` in a default under Part I §1 rule 1); reads in reactions and `analog` (`n as f64`, `match md`, `rs.unwrap_or(..)`, `if let Some(x) = rs`).
**Where**: `crates/piperine-lang/src/elab/body.rs`
**Depends on**: T30
**Reuses**: `composite-values`' composite params (its T58)
**Requirement**: PI-01
**Executor**: Sonnet

**Done when**:

- [ ] `an_integer_param_is_bound_and_read`, `n_as_f64_reaches_the_contribution`, `an_integer_default_converts_with_as`, `an_enum_param_selects_its_arm`, `an_option_param_left_out_is_none`, `an_option_param_bound_to_a_value_is_some`, `option_param_methods_read_the_bound_case` pass under `**Run elabparams**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): integer, enum and Option params`

---

### T32: Plan and printer: the param pins of the new types

**What**: The plan lays integer, enum and `Option` params out in their `PARAM` pins (`CompositeLayout` for `Option`), the netlist passes them, and the generated Rust reads them; a run from source reads each value.
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: T31
**Reuses**: `CompositeLayout`, `Netlist::circuit`
**Requirement**: PI-01
**Executor**: Sonnet

**Done when**:

- [ ] `an_integer_param_runs_with_its_value`, `an_enum_param_runs_its_arm`, `an_option_param_runs_its_case` pass under `**Run sourceparams**`
- [ ] The transpile goldens and every buck target are byte-identical
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): param pins of integer, enum and Option params`

---

### T33: Host: the text of the new param types

**What**: `--param` and `set` parse a value by the param's type (D24): an integer literal of the type and range, a variant name, `None` or a value of `T`; anything else `E0802` naming the param and the value, listing the variants for an enum.
**Where**: `crates/piperine-api/src/params.rs`
**Depends on**: T32
**Reuses**: `composite-values`' set by leaf path (its T59)
**Requirement**: PI-01
**Executor**: Sonnet

**Done when**:

- [ ] `an_integer_param_set_by_text_in_range`, `an_integer_out_of_range_is_e0802`, `an_enum_param_set_by_variant_name`, `an_unknown_variant_is_e0802_listing_the_variants`, `an_option_param_set_to_a_value_or_none` pass under `**Run apisession**`
- [ ] `param_of_an_integer_from_the_command_line_reads_from_the_start` passes under `**Run clirun**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): host text of integer, enum and Option params`

---

### T34: Interpreter: params of other types are refused

**What**: The interpreter refuses a design with a param that is not `f64` or `bool`, naming the param and the transpiled backend (R1).
**Where**: `crates/piperine-interp/src/walk.rs`
**Depends on**: T33
**Reuses**: the interpreter's refusal pattern
**Requirement**: PI-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_param_of_another_type_is_refused_naming_the_transpiled_backend` passes under `**Run interp**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(interp): refuse params of other types by name`

---

### T35: The Book's debouncer, PWM, check and watchdog

**What**: The Book rows of L28 (digital#7, #11, #14, the `~` claims, core-std#10) run and print the records the Book shows; `inventory.tsv` rows corrected in place; the Book's own model text is not changed.
**Where**: `tests/book/digital.rs`
**Depends on**: T34
**Reuses**: `tests/book/support`
**Requirement**: PI-01
**Executor**: Sonnet

**Done when**:

- [ ] `the_debouncer_pwm_and_pwm_check_print_their_records` passes under `**Run bookdigital**`
- [ ] `the_watchdog_builds_with_its_u16_param` passes under `**Run bookcorestd**`
- [ ] `every_block_of_the_book_has_one_row` passes under `**Run bookinventory**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(book): the debouncer, PWM and watchdog run`

---

### T36: `E0122` for named functions

**What**: A named emitting function called where `emit!` is not allowed, or a named solve-reading function called in a const context, is `E0122` naming the function and its class (§3, V3), using `composite-values`' function classes; `E0601` stays for `emit!` written directly.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: None
**Reuses**: `types/class.rs` (`composite-values` T16)
**Requirement**: PI-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_named_emitting_fn_where_emit_is_not_allowed_is_e0122`, `a_named_solve_reading_fn_in_a_const_is_e0122` pass under `**Run typesvalues**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(types): a named function called outside its class is E0122`

---

### T37: `parse_table`

**What**: `elab/tables.rs` (`TableText`): `parse_table` typed from the `const`'s declared type (D16) folds to `Constant::Aggregate`; rows of whitespace-separated real literals (SI suffixes, blank lines and `#` comments skipped); a wrong row count or column count `E0614` naming file and row; a token that is not a real literal, or a value whose nearest `f64` is infinite, `E0706` naming file, row and token.
**Where**: `crates/piperine-lang/src/elab/tables.rs`
**Depends on**: None
**Reuses**: `ConstEval`, `IncludedFiles`
**Requirement**: PI-09
**Executor**: Sonnet

**Done when**:

- [ ] `parse_table_reads_64_rows_in_order`, `a_table_of_the_wrong_shape_is_e0614_naming_file_and_row`, `a_token_that_is_not_a_real_is_e0706`, `an_infinite_row_value_is_e0706` pass under `**Run elabfiles**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): parse_table`

---

### T38: `parse_hex`

**What**: `HexText`: a `$readmemh` text (`_` inside a word, `//` and `/* */`, `@address`) into `[T; N]`, a digit `x` or `z` making its four levels unknown; an address the text does not write `E0614`; a word that is not hexadecimal `E0706`.
**Where**: `crates/piperine-lang/src/elab/tables.rs`
**Depends on**: T37
**Reuses**: `TableText`'s row reading
**Requirement**: PI-09
**Executor**: Sonnet

**Done when**:

- [ ] `parse_hex_reads_words_at_their_addresses`, `x_and_z_digits_are_unknown_levels`, `an_address_the_text_does_not_write_is_e0614`, `a_word_that_is_not_hex_is_e0706` pass under `**Run elabfiles**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): parse_hex`

---

### T39: `table_model`: control and table at elaboration

**What**: `elab/table_model.rs` (`TableControl`): the checker types `table_model(x, &T, ctrl)` from the catalogue; the elaborator parses the control (`E0703` naming it) and validates the table (`E0704` naming the first offending row: abscissae not strictly increasing, fewer points than the degree needs, one row under `D`).
**Where**: `crates/piperine-lang/src/elab/table_model.rs`
**Depends on**: T38
**Reuses**: the intrinsic catalogue
**Requirement**: PI-10
**Executor**: Sonnet

**Done when**:

- [ ] `a_bad_control_string_is_e0703`, `non_increasing_abscissae_are_e0704`, `too_few_points_for_the_degree_is_e0704`, `a_one_row_table_under_d_is_e0704` pass under `**Run elabfiles**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): table_model control and table`

---

### T40: IR `Table` and `TableSlope`

**What**: `Expr::Table` and `Expr::TableSlope` appended last (D17); `Ir::check` accepts them; the derivative pass gives `TableSlope` as the derivative of `Table`; the structural printer prints both; the interpreter refuses both by name.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T39
**Reuses**: the `MathFn` arms of `check.rs` and `print.rs`
**Requirement**: PI-10
**Executor**: Sonnet

**Done when**:

- [ ] `a_table_expression_is_built_checked_and_printed` passes under `**Run irchecks**`
- [ ] `a_table_expression_is_refused_by_name` passes under `**Run interp**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): table_model expressions`

---

### T41: `rt::TableModel` and its printer arm

**What**: `piperine-transpile/src/rt/table.rs` (`TableModel`): degrees `D`, `1`, `2`, `3` with the end conditions of D18, extrapolation `C`, `L`, `E` at each end, the slope of the interpolant, coefficients computed once per table; `E` outside the table raises `Panic` coded `E0705` (D19); the printer emits calls for `Table`/`TableSlope`. Create `crates/piperine-parity/tests/source_tables.rs` with models under `crates/piperine-parity/tests/tables/`.
**Where**: `crates/piperine-transpile/src/rt/table.rs`
**Depends on**: T40
**Reuses**: `rt::Panic`, the printer's `MathFn` arm
**Requirement**: PI-10
**Executor**: Sonnet

**Done when**:

- [ ] `linear_table_model_is_the_interpolation_at_op_and_dc_sweep`, `discrete_control_gives_the_closest_sample`, `cubic_reproduces_a_cubic_polynomial`, `extrapolation_c_and_l_at_each_end`, `extrapolation_e_outside_is_e0705` pass under `**Run sourcetables**`
- [ ] The code's row leaves `crates/piperine-lang/tests/support/code_owners.tsv` in this commit and `**Run speccodes**` passes
- [ ] The transpile goldens are byte-identical
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): table_model at run time`

---

### T42: The table diode against the partials verifier

**What**: A fixture package with `diode_iv.tbl` and `boot.hex` (spec Independent Test): the measured-table diode's device passes `piperine_abi::verify`, the partial being the interpolant's derivative.
**Where**: `crates/piperine-parity/tests/source_tables.rs`
**Depends on**: T41
**Reuses**: `piperine_abi::verify`
**Requirement**: PI-10
**Executor**: Sonnet

**Done when**:

- [ ] `the_table_diode_passes_the_partials_verifier` passes under `**Run sourcetables**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the table diode passes the partials verifier`

---

### T43: The Book's file rows

**What**: packages#12, cheatsheet#10 and packages~64f2121d run or raise as the Book says (`E0610` in a module); `inventory.tsv` rows corrected in place.
**Where**: `tests/book/packages.rs`
**Depends on**: T42
**Reuses**: `tests/book/support`
**Requirement**: PI-09
**Executor**: Sonnet

**Done when**:

- [ ] `the_include_str_table_runs`, `e0610_is_raised_for_include_str_in_a_module` pass under `**Run bookpackages**`
- [ ] `the_files_card_runs` passes under `**Run bookcheatsheet**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(book): the file rows run`

---

### T44: Closing records

**What**: `GAPS.md` drops L28 and L57 and §10 lists exactly `code_owners.tsv`; `spec_codes.rs` gains `this_features_codes_are_all_raised` (none of the 22 codes, nor `E0810`/`E0811`, in the allow-list); spec.md traceability marks every requirement Done; ROADMAP item 10 row and the `MD-` entry completed.
**Where**: `GAPS.md`
**Depends on**: None
**Reuses**: `docs.rs`
**Requirement**: PI-14
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_no_longer_lists_this_features_ids`, `gaps_section_10_lists_exactly_the_owned_codes` pass under `**Run docs**`
- [ ] `this_features_codes_are_all_raised` passes under `**Run speccodes**`
- [ ] Code clarity: every new or changed struct, enum, method, field and variant has a `///` doc comment citing its spec rule; no free function; object-oriented names without abbreviations; no god method; the data path readable in one place
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: params-and-io closing records`

---

## Checks

**C1** - `n: u8 = 4` read by a reaction runs with `n` equal to 4 of type `u8`. (PI-01, AC 1; T32)
Proof: run sourceparams | test an_integer_param_runs_with_its_value ... ok

**C2** - A placement binding `{ n: 7 }` gives the instance 7. (PI-01, AC 2; T31)
Proof: run elabparams | test an_integer_param_is_bound_and_read ... ok

**C3** - `analog` reading `n as f64` contributes 7.0 for that instance. (PI-01, AC 3; T31)
Proof: run elabparams | test n_as_f64_reaches_the_contribution ... ok

**C4** - A host value that is not an integer literal of the type, or out of its range, is `E0802` naming the param and the value. (PI-01, AC 4; T33)
Proof: run apisession | test an_integer_out_of_range_is_e0802 ... ok

**C5** - `--param Top.db.n=6` reads 6 from the start of the run. (PI-01, AC 5; T33)
Proof: run clirun | test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok

**C6** - The Book's `Debounce`, `Pwm` and `PwmCheck` build and print the Book's records. (PI-01, AC 6; T35)
Proof: run bookdigital | test the_debouncer_pwm_and_pwm_check_print_their_records ... ok

**C7** - A `match` on `md: Mode = Mode::Fast` runs the bound variant's arm. (PI-02, AC 1; T32)
Proof: run sourceparams | test an_enum_param_runs_its_arm ... ok

**C8** - A placement binding `{ md: Mode::Slow }` reads `Mode::Slow`. (PI-02, AC 2; T31)
Proof: run elabparams | test an_enum_param_selects_its_arm ... ok

**C9** - `--param Top.md=Slow` reads `Mode::Slow`. (PI-02, AC 3; T33)
Proof: run apisession | test an_enum_param_set_by_variant_name ... ok

**C10** - A variant name the enum lacks is `E0802` listing the variants. (PI-02, AC 4; T33)
Proof: run apisession | test an_unknown_variant_is_e0802_listing_the_variants ... ok

**C11** - An `Option<f64>` param left out reads `None`. (PI-03, AC 1; T31)
Proof: run elabparams | test an_option_param_left_out_is_none ... ok

**C12** - `{ rs: 10.0 }` reads `Some(10.0)` and `{ rs: None }` reads `None`. (PI-03, AC 2; T31)
Proof: run elabparams | test an_option_param_bound_to_a_value_is_some ... ok

**C13** - `unwrap_or`, `is_some`, `match` and `if let Some` give the bound case. (PI-03, AC 3; T31)
Proof: run elabparams | test option_param_methods_read_the_bound_case ... ok

**C14** - A host value of `T` sets `Some` of it; `None` sets `None`. (PI-03, AC 4; T33)
Proof: run apisession | test an_option_param_set_to_a_value_or_none ... ok

**C15** - `cell: Dram1T1C = Dram1T1C { cs: 30f }` and `cell(c)` place a `Dram1T1C` with `cs` 30 fF. (PI-04, AC 1; T22)
Proof: run elabinstances | test a_placement_through_a_module_param_places_its_template ... ok

**C16** - A parent's `{ cell: Dram1T1C { cs: 10f } }` places `cs` 10 fF. (PI-04, AC 2; T22)
Proof: run elabinstances | test a_parent_binding_replaces_the_template ... ok

**C17** - A param block on a placement through a module-typed param is `E0361`. (PI-04, AC 3; T21)
Proof: run typesmodules | test a_param_block_on_a_placement_through_a_param_is_e0361 ... ok

**C18** - A template leaving a required param unbound is `E0319` naming it. (PI-04, AC 4; T22)
Proof: run elabinstances | test a_template_leaving_a_required_param_unbound_is_e0319 ... ok

**C19** - A template of another module is `E0180` naming both modules. (PI-04, AC 5; T21)
Proof: run typesmodules | test a_template_of_another_module_is_e0180_naming_both ... ok

**C20** - The POM keeps `cell(c)` as written and the IR holds the concrete module. (PI-04, AC 6; T22)
Proof: run elabinstances | test the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module ... ok

**C21** - The interpreter refuses a param that is not `f64` or `bool`, naming it and the transpiled backend. (PI-05, AC 1; T34)
Proof: run interp | test a_param_of_another_type_is_refused_naming_the_transpiled_backend ... ok

**C22** - A mismatch no other row names is `E0180` naming the expected and found types. (PI-06, AC 1; T5)
Proof: run typesvalues | test a_mismatch_no_row_names_is_e0180_naming_both_types ... ok

**C23** - No non-test source spells `P0002`. (PI-06, AC 2; T5)
Proof: run speccodes | test no_source_spells_p0002 ... ok

**C24** - A call through a param hiding a function is `E0180` with a note at the function. (PI-06, AC 3; T6)
Proof: run typesvalues | test a_param_hiding_a_function_is_e0180_with_a_note_at_the_function ... ok

**C25** - No Book listing prints `P0002`. (PI-06, AC 4; T5)
Proof: run clibook | test no_book_listing_prints_p0002 ... ok

**C26** - An `as` between types without a §1.4.5 row is `E0182` naming both. (PI-06, AC 5; T3)
Proof: run typesvalues | test an_as_between_types_without_a_row_is_e0182 ... ok

**C27** - A call through a function value with the wrong arity is `E0141` with both counts. (PI-06, AC 6; T4)
Proof: run typesvalues | test a_call_through_a_function_value_with_the_wrong_arity_is_e0141 ... ok

**C28** - A `const` initializer reading a run value is `E0188` naming the `const`. (PI-06, AC 7; T7)
Proof: run elabvalues | test a_const_reading_a_run_value_is_e0188 ... ok

**C29** - A host DC source of 1.5 V on node port `a` gives `v(a)` = 1.5 V at `op`. (PI-07, AC 1; T18)
Proof: run apisources | test a_dc_source_sets_the_port_at_the_operating_point ... ok

**C30** - A PWL source makes the transient land on each corner with `v(a)` equal to the waveform there. (PI-07, AC 2; T19)
Proof: run apisources | test a_pwl_source_lands_on_every_corner ... ok

**C31** - A source's AC magnitude and phase drive `ac`: the host-driven RC matches `ac_rc.cir` under `AC_MAGNITUDE` and `AC_PHASE`. (PI-07, AC 3; T18)
Proof: run sourcehost | test a_host_driven_rc_matches_ac_rc_in_ac ... ok

**C32** - A host change of the source at a time is a jump at that time. (PI-07, AC 4; T19)
Proof: run apisources | test a_timed_source_change_is_a_jump ... ok

**C33** - A run with an undriven node port is `E0810` naming the port. (PI-07, AC 5; T18)
Proof: run apisources | test an_undriven_node_port_is_e0810 ... ok

**C34** - A top with a value input is `E0811` naming it. (PI-07, AC 6; T17)
Proof: run elabentry | test a_top_value_input_is_e0811 ... ok

**C35** - Q1 is (a): a value input of the top is refused, never bound by the host (AC 7 applies only under (c)). (PI-07, AC 7; T17)
Proof: run elabentry | test a_top_value_input_is_e0811 ... ok

**C36** - §8.1.14 and §23.1 state the host's sources and carry `E0810` and `E0811`. (PI-07, AC 8; T16)
Proof: run spectext | test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok

**C37** - `include_str!` in a `const` gives the file's text, resolved beside the source. (PI-08, AC 1; T24)
Proof: run elabfiles | test include_str_reads_the_file_beside_the_source ... ok

**C38** - `include_str!` outside a `const` item is `E0610`. (PI-08, AC 2; T23)
Proof: run typesvalues | test include_str_outside_a_const_is_e0610 ... ok

**C39** - An unreadable file is `E0612` naming the resolved path. (PI-08, AC 3; T24)
Proof: run elabfiles | test an_unreadable_file_is_e0612_naming_the_path ... ok

**C40** - A file that is not UTF-8 is `E0613` with the path and the byte offset. (PI-08, AC 4; T24)
Proof: run elabfiles | test a_file_that_is_not_utf8_is_e0613_with_the_offset ... ok

**C41** - A file changed after elaboration does not change the run until re-elaboration. (PI-08, AC 5; T24)
Proof: run elabfiles | test a_changed_file_needs_a_new_elaboration ... ok

**C42** - `parse_table` gives the 64 pairs of `diode_iv.tbl` in order. (PI-09, AC 1; T37)
Proof: run elabfiles | test parse_table_reads_64_rows_in_order ... ok

**C43** - `parse_hex` gives the words at their addresses. (PI-09, AC 2; T38)
Proof: run elabfiles | test parse_hex_reads_words_at_their_addresses ... ok

**C44** - A table of the wrong shape is `E0614` naming the file and the row. (PI-09, AC 3; T37)
Proof: run elabfiles | test a_table_of_the_wrong_shape_is_e0614_naming_file_and_row ... ok

**C45** - A token that is not a real literal is `E0706` naming file, row and token. (PI-09, AC 4; T37)
Proof: run elabfiles | test a_token_that_is_not_a_real_is_e0706 ... ok

**C46** - `table_model(.., "1C")` equals the linear interpolation at `op` and over a DC sweep to 1e-15 relative. (PI-10, AC 1; T41)
Proof: run sourcetables | test linear_table_model_is_the_interpolation_at_op_and_dc_sweep ... ok

**C47** - Control `D` gives the closest sample. (PI-10, AC 2; T41)
Proof: run sourcetables | test discrete_control_gives_the_closest_sample ... ok

**C48** - Control `3` reproduces a cubic sampled at 4 or more points to 1e-12 relative. (PI-10, AC 3; T41)
Proof: run sourcetables | test cubic_reproduces_a_cubic_polynomial ... ok

**C49** - Extrapolation `C` holds the end value, `L` the end slope's line, per end. (PI-10, AC 4; T41)
Proof: run sourcetables | test extrapolation_c_and_l_at_each_end ... ok

**C50** - `E` outside the table under known control fails the analysis with `E0705` naming the instance and the call. (PI-10, AC 5; T41)
Proof: run sourcetables | test extrapolation_e_outside_is_e0705 ... ok

**C51** - A control string not of §22.2.15's form is `E0703`. (PI-10, AC 6; T39)
Proof: run elabfiles | test a_bad_control_string_is_e0703 ... ok

**C52** - Abscissae not strictly increasing are `E0704` naming the first offending row. (PI-10, AC 7; T39)
Proof: run elabfiles | test non_increasing_abscissae_are_e0704 ... ok

**C53** - The table diode passes the partials verifier. (PI-10, AC 8; T42)
Proof: run sourcetables | test the_table_diode_passes_the_partials_verifier ... ok

**C54** - Appendix C.2 states the spline end conditions. (PI-10, AC 9; T27)
Proof: run spectext | test appendix_c2_states_the_spline_end_conditions ... ok

**C55** - `piperine fmt --check` exits 0 over each stdlib file. (PI-11, AC 1; T26)
Proof: run clicommands | test fmt_check_passes_on_every_stdlib_file ... ok

**C56** - `piperine fmt --check` exits 0 in `examples/buck`. (PI-11, AC 2; T26)
Proof: run clicommands | test fmt_check_passes_in_examples_buck ... ok

**C57** - Every `rust` fence of Appendices D, E and F equals its formatter output. (PI-11, AC 3; T26)
Proof: run stdgalleries | test every_gallery_fence_is_in_the_formatters_form ... ok

**C58** - Formatting keeps the printed tree of every shipped source. (PI-11, AC 4; T25)
Proof: run msformat | test formatting_keeps_the_printed_tree_of_every_shipped_source ... ok

**C59** - `&` outside a call argument or a parameter type is `E0102` (raised by `composite-values`). (PI-12, AC 1; T29)
Proof: run typesvalues | test a_borrow_outside_a_call_argument_is_e0102 ... ok

**C60** - An integer division by zero under known control fails the run with `E0105` naming the operation, the instance and the time. (PI-12, AC 2; T15)
Proof: run runerrors | test a_division_by_zero_under_known_control_fails_the_run_with_e0105 ... ok

**C61** - A named emitting function where `emit!` is not allowed is `E0122` naming it and its class. (PI-12, AC 3; T36)
Proof: run typesvalues | test a_named_emitting_fn_where_emit_is_not_allowed_is_e0122 ... ok

**C62** - A quantity attribute other than `access`, `units`, `abstol` is `E0204` naming it. (PI-12, AC 4; T8)
Proof: run elabanalog | test a_quantity_attribute_outside_the_three_is_e0204 ... ok

**C63** - Equal `access` names of one nature are `E0205`. (PI-12, AC 5; T8)
Proof: run elabanalog | test equal_access_names_are_e0205 ... ok

**C64** - An expression on a carrier port that is not `in` is `E0315` naming the port. (PI-12, AC 6; T9)
Proof: run typesmodules | test an_expression_on_a_carrier_port_that_is_not_in_is_e0315 ... ok

**C65** - An instance without a result used as a value is `E0333`. (PI-12, AC 7; T10)
Proof: run typesmodules | test an_instance_without_a_result_used_as_a_value_is_e0333 ... ok

**C66** - An attribute A.1 does not list is `E0904` naming it and the item. (PI-12, AC 8; T11)
Proof: run typesstructure | test an_attribute_a1_does_not_list_is_e0904 ... ok

**C67** - The file and table codes `E0610` to `E0706` are raised (none left in the allow-list). (PI-12, AC 9; T44)
Proof: run speccodes | test this_features_codes_are_all_raised ... ok

**C68** - A probe on a child's node that is not `pub` is `E0244` naming the node. (PI-12, AC 10; T12)
Proof: run typescarriers | test a_probe_on_a_childs_private_node_is_e0244 ... ok

**C69** - A continuous result read where probes are not allowed is `E0354`. (PI-12, AC 11; T13)
Proof: run typesresults | test a_continuous_result_read_where_probes_are_not_allowed_is_e0354 ... ok

**C70** - `Unknown` with no inferable type is `E0906`. (PI-12, AC 12; T14)
Proof: run typesunknowns | test unknown_with_no_inferable_type_is_e0906 ... ok

**C71** - Every code of the spec's tables is spelled in a source or listed in the allow-list. (PI-13, AC 1; T2)
Proof: run speccodes | test every_spec_code_is_raised_or_owned ... ok

**C72** - Every allow-list entry names its owner feature and construct. (PI-13, AC 2; T2)
Proof: run speccodes | test every_owned_code_names_its_owner_and_construct ... ok

**C73** - A listed code that a source spells fails the guard naming the stale entry. (PI-13, AC 3; T2)
Proof: run speccodes | test no_owned_code_is_raised ... ok

**C74** - At merge the allow-list holds none of this feature's 22 codes, nor `E0810` or `E0811`. (PI-13, AC 4; T44)
Proof: run speccodes | test this_features_codes_are_all_raised ... ok

**C75** - `GAPS.md` no longer lists L20, L28, L39, L57, X5 and H12, and §10 lists only the allow-list. (PI-14, AC 1; T44)
Proof: run docs | test gaps_no_longer_lists_this_features_ids ... ok

**C76** - `n: u8 = K as u8` evaluates the `as` under Part I §1 rule 1. (edge case; T31)
Proof: run elabparams | test an_integer_default_converts_with_as ... ok

**C77** - A `where` reading an `Option` param as a number is `E0180`. (edge case; T30)
Proof: run typesmodules | test an_option_param_read_as_a_number_is_e0180 ... ok

**C78** - A top node port tied to `gnd` inside the design is still refused when undriven. (edge case; T18)
Proof: run apisources | test a_node_port_tied_to_gnd_inside_is_still_refused_undriven ... ok

**C79** - One file named by two `const`s is read once. (edge case; T24)
Proof: run elabfiles | test one_file_named_twice_is_read_once ... ok

**C80** - A `parse_table` value whose nearest `f64` is infinite is `E0706` naming the row. (edge case; T37)
Proof: run elabfiles | test an_infinite_row_value_is_e0706 ... ok

**C81** - A one-row table under `D` is `E0704`. (edge case; T39)
Proof: run elabfiles | test a_one_row_table_under_d_is_e0704 ... ok

**C82** - A cyclic `const` is `E0188`. (PI-06, AC 7; T7)
Proof: run elabvalues | test a_const_that_depends_on_itself_is_e0188 ... ok

**C83** - The host-driven RC's DC step matches `tran_rc.cir` under `TRAN_VOLTAGE`. (D11; T18)
Proof: run sourcehost | test a_host_driven_rc_step_matches_tran_rc ... ok

**C84** - A host PWL into the RC matches the new twin `tran_pwl_rc.cir` under `TRAN_VOLTAGE`. (D11; T19)
Proof: run sourcehost | test a_host_pwl_into_the_rc_matches_tran_pwl_rc ... ok

**C85** - A top with a node port run from the CLI is `E0810` naming the port (X8). (PI-07, AC 5; T20)
Proof: run clirun | test run_of_a_top_with_a_node_port_is_e0810 ... ok

**C86** - A non-positive `abstol` is `E0204`. (PI-12, AC 4; T8)
Proof: run elabanalog | test a_non_positive_abstol_is_e0204 ... ok

**C87** - A listed attribute on the wrong kind of item is `E0904`. (PI-12, AC 8; T11)
Proof: run typesstructure | test a_listed_attribute_on_the_wrong_item_is_e0904 ... ok

**C88** - An `Event` result outside an event position is `E0354`. (PI-12, AC 11; T13)
Proof: run typesresults | test an_event_result_outside_an_event_position_is_e0354 ... ok

**C89** - A named solve-reading function in a const context is `E0122`. (PI-12, AC 3; T36)
Proof: run typesvalues | test a_named_solve_reading_fn_in_a_const_is_e0122 ... ok

**C90** - A model `unwrap` of `None` under known control fails the run with `E0105`. (PI-12, AC 2; T15)
Proof: run runerrors | test an_unwrap_of_none_under_known_control_fails_the_run_with_e0105 ... ok

**C91** - A top with a node port gets one `HostSource` instance per port in the IR. (PI-07, AC 1; T17)
Proof: run elabentry | test a_top_node_port_gets_one_host_source_instance ... ok

**C92** - The POM keeps a top with node ports as written (MD-25). (PI-07, AC 1; T17)
Proof: run elabentry | test the_pom_keeps_a_top_with_node_ports_as_written ... ok

**C93** - A module-typed param takes a template as its default. (PI-04, AC 1; T21)
Proof: run typesmodules | test a_module_typed_param_takes_a_template ... ok

**C94** - A host `--param`/`set` of a module-typed param is `E0802` (D13). (D13; T22)
Proof: run apisession | test a_module_typed_param_is_not_set_by_text ... ok

**C95** - `include_str!` in a `const` is typed `&str`. (PI-08, AC 1; T23)
Proof: run typesvalues | test include_str_in_a_const_is_a_str ... ok

**C96** - Every stdlib file is a fixed point of the formatter. (PI-11, AC 1; T25)
Proof: run msformat | test every_stdlib_file_is_in_the_formatters_form ... ok

**C97** - `GAPS.md` no longer lists X5, L20, L39 and H12 after B4. (PI-14, AC 1; T28)
Proof: run docs | test gaps_no_longer_lists_x5_l20_l39_h12 ... ok

**C98** - Integer, enum and `Option` params type check with defaults and `where`. (PI-01, AC 1; T30)
Proof: run typesmodules | test integer_enum_and_option_params_type_check ... ok

**C99** - An `Option` param runs its bound case from source. (PI-03, AC 3; T32)
Proof: run sourceparams | test an_option_param_runs_its_case ... ok

**C100** - An integer param set by text in range reads that value. (PI-01, AC 5; T33)
Proof: run apisession | test an_integer_param_set_by_text_in_range ... ok

**C101** - The Book's `Watchdog` builds with `limit: u16` and ends the run at the fourth edge. (PI-01, AC 6; T35)
Proof: run bookcorestd | test the_watchdog_builds_with_its_u16_param ... ok

**C102** - Every Book block keeps one inventory row after the L28 rows change. (PI-01, AC 6; T35)
Proof: run bookinventory | test every_block_of_the_book_has_one_row ... ok

**C103** - `x` and `z` hex digits make their four levels unknown. (PI-09, AC 2; T38)
Proof: run elabfiles | test x_and_z_digits_are_unknown_levels ... ok

**C104** - An address the hex text does not write is `E0614`. (PI-09, AC 3; T38)
Proof: run elabfiles | test an_address_the_text_does_not_write_is_e0614 ... ok

**C105** - A word that is not hexadecimal is `E0706`. (PI-09, AC 4; T38)
Proof: run elabfiles | test a_word_that_is_not_hex_is_e0706 ... ok

**C106** - Fewer points than the degree needs is `E0704`. (PI-10, AC 7; T39)
Proof: run elabfiles | test too_few_points_for_the_degree_is_e0704 ... ok

**C107** - The IR builds, checks and prints `Table` and `TableSlope`. (PI-10, AC 8; T40)
Proof: run irchecks | test a_table_expression_is_built_checked_and_printed ... ok

**C108** - The interpreter refuses `Table` by name. (PI-10, AC 8; T40)
Proof: run interp | test a_table_expression_is_refused_by_name ... ok

**C109** - The Book's `include_str!` table (packages#12) runs. (PI-09, AC 1; T43)
Proof: run bookpackages | test the_include_str_table_runs ... ok

**C110** - The Book's `include_str!` in a module raises `E0610`. (PI-08, AC 2; T43)
Proof: run bookpackages | test e0610_is_raised_for_include_str_in_a_module ... ok

**C111** - The Book's files card (cheatsheet#10) runs. (PI-09, AC 1; T43)
Proof: run bookcheatsheet | test the_files_card_runs ... ok

**C112** - `GAPS.md` §10 lists exactly the allow-list's codes. (PI-14, AC 1; T44)
Proof: run docs | test gaps_section_10_lists_exactly_the_owned_codes ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-20): the user was away and asked for the most reasonable default on every open point ("Eu gostaria que engatilhasse até o 12"); the defaults are design.md D1 to D25 and the spec amendments A1 to A6. T1 re-read (2026-11-20, `HEAD` 79fd5ca9, Build green): `E0244` - the source `module Child(p: Electrical) { signal hidden: Electrical; }` placed as `let c = Child(x);` in `Step`, with `analog for Step { i(x) +> v(c.hidden); }` (also `v(m.c.hidden)` and `i(m.c.hidden) +> 1.0`) is `E0343` "`hidden` is not `pub` in module `Child`" from the name resolver (`pom/resolve/members.rs:84`), before the checker runs; with `pub` it is accepted; `v(c.p)` and `i(c.p) +> 1.0` on a child's port are accepted, so no form reaches `E0244` today and T12 raises it where the resolver's `E0343` names a probe or contribution operand (the same form, a different code at that position). `E0354` - a continuous result read where probes are not allowed (`module Step() { signal a: Electrical; let g = ddt(v(a)); q: f64 = g; }`) is `P0001` "a name whose type is unknown" at `g`; an `Event` read outside an event position (`i(a) +> g + e;` with `let e = cross(v(a));`) is already `E0441` (§13 rule 8) and `if e` is `P0001` "an event read as a `bool`"; T13 gives the first form `E0354`. `E0906` - `let x = Unknown;` in a module body and in a reaction is `P0001` "`Unknown` literal where no carried type is expected"; T14 gives it `E0906`. Formatter: `piperine fmt --check` would change 10 of 10 files of `crates/piperine-lang/std/`, 9 of 9 files of `examples/buck` (8 under `src/`, `benches/startup.msl`) and 18 of 18 `rust` fences of Appendices D (6), E (7) and F (5); `tests/e2e/buck` is not a copy of `examples/buck` (`src/lib.msl` and `Piperine.toml` differ, and it lacks `control`, `dpwm`, `power`, `sense` and the benches). Nothing in T1 contradicts design.md. B1 defaults (T2 to T7, 2026-11-20): T2 - `spec_codes.rs` counts a line as a comment when it starts with `//` after trimming; a code is a word `E` or `P` plus four digits. T3 - `E0182` is raised by `Checker::cast` only; the tuple form `(1.0, 2.0) as f64` is `P0001` (tuples are outside the subset), so the test uses `true as f64` and `1.5 as bool`. T4 - the arity sites that were `P0002` (`apply`, the one-argument `Option` rows) share one `arity_mismatch`, and a call through a function value (a `fn`-typed param or local) with the wrong count is `E0141` where the right count stays `P0001`; the Book's `compiler` test proved `P0002` with a count, so it moved to a type mismatch until T5 removed the row. T5 - the closure parameter count and `None` with no type are `E0180`; the Book's compiler chapter drops its `P0002` row ("Three more belong to the compiler"), its claim row `compiler~79f450b1` leaves `inventory.tsv` and `docs/book/index.html` is rebuilt by `build.py`; tests named `*_is_p0002` were renamed `*_is_e0180`. T6 - the hiding forms are a param and a module `let` (a `const` cannot sit in a module body, `E0900`); the note is a secondary label on the `fn`'s name, written only when the `fn` is in the same file. T7 - a fault found folding a `const` item is `E0188` (new `ConstFault::{NotConstant, Cyclic}`, `ConstEval::open_constants`); a `const` without a value outside a trait is `E0188` from `Checker::constant` (a trait's stays `P0001`); a probe, a param or a carrier cannot reach a `const` initializer (the checker and the resolver refuse them with `E0323`, `E0352`, `E0161` first), so the reachable run-value form is a call of a function that reads the environment (it was `E0321`); a param default keeps `E0321`. Commit trailer: tasks.md's rule (`Claude Opus 5.5`) was followed over the harness attribution reminder. Not run in full: the final root `cargo test --no-fail-fast` was stopped when `df /` passed 90 % (other lanes share the disk) and re-run after freeing `/var/tmp/piperine-tmp-b`.
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 1 → Phase 2 → … → Phase 12

Phase 1:   T1 ------→ T2
Phase 2:   T3 ------→ T4 ------→ T5 ------→ T6 ------→ T7
Phase 3:   T8 ------→ T9 ------→ T10 ------→ T11 ------→ T12 ------→ T13 ------→ T14 ------→ T15
Phase 4:   T16 ------→ T17 ------→ T18 ------→ T19 ------→ T20
Phase 5:   T21 ------→ T22
Phase 6:   T23 ------→ T24
Phase 7:   T25 ------→ T26 ------→ T27 ------→ T28
Phase 8:   T29
Phase 9:   T30 ------→ T31 ------→ T32 ------→ T33 ------→ T34 ------→ T35
Phase 10:  T36
Phase 11:  T37 ------→ T38 ------→ T39 ------→ T40 ------→ T41 ------→ T42 ------→ T43
Phase 12:  T44
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | Depends on | Notes |
| ----- | ------ | ----- | ---------- | ----- |
| B1 | 1, 2 | T1 to T7 | `main` at `494a7f75` | the guard of codes; X5 and L20 closed (`E0180`, `E0182`, `E0141`, `E0188`) |
| B2 | 3 | T8 to T15 | B1 | `E0204`, `E0205`, `E0315`, `E0333`, `E0904`, `E0244`, `E0354`, `E0906`, `E0105` |
| B3 | 4, 5 | T16 to T22 | B2 | the host drives a top's node ports (L39, `E0810`, `E0811`, ngspice parity); module-typed params (`E0361`) |
| B4 | 6, 7 | T23 to T28 | B3 | `include_str!` (`E0610`, `E0612`, `E0613`); H12; Appendix C.2; early records. The feature pauses here if `composite-values` A–B is not on `main` |
| B5 | 8, 9, 10 | T29 to T36 | B4 **and** `composite-values` A–B on `main` (**waiting**) | rebase; integer, enum and `Option` params (L28 closed); `E0122` for named functions |
| B6 | 11, 12 | T37 to T44 | B5 (**waiting** with it) | `parse_table`, `parse_hex`, `table_model` (L57 closed); closing records |

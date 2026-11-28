# Value Semantics Tasks

**Approved**: 2026-11-20 - user: "Ok, perfeito!" (to the sequencing and to D1, D2, D5, D7: composite-values pauses after B7, value-semantics B1-B2 run on its tip, then composite-values B8-B10 resume with amended tasks)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/value-semantics/design.md`
**Status**: Approved
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite suite_hygiene::`
**Run errors**: `cargo test -p piperine-lang --test msl errors::`
**Run typesvs**: `cargo test -p piperine-lang --test types_values value_semantics::`
**Run srcvs**: `cargo test -p piperine-parity --test source_values value_semantics::`
**Run iragg**: `cargo test -p piperine-ir --test model aggregates::`
**Run excerpts**: `cargo test -p piperine-lang --test std excerpts::`
**Run stdlib**: `cargo test -p piperine-lang --test std library::`
**Run hygiene**: `cargo test -p piperine-repository-guards --test suite suite_hygiene::`
**Run print**: `cargo test -p piperine-lang --test msl print::`
**Run resolve**: `cargo test -p piperine-lang --test pom resolve::`
**Run buckcli**: `cargo test -p piperine-cli --test buck`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run goldens**: `cargo test -p piperine-transpile --test golden`
**Run booklang**: `cargo test -p piperine --test book_language`
**Run booktypes**: `cargo test -p piperine --test book_types`
**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-c CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-c` before any cargo command (the `composite-values` lane: the two features run one after the other on one line, never at once). Cargo with `-p <crate>` or plain at the root, never `--workspace`, never `cargo clean`. Long commands run in the background and are waited for; no polling loop; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens and every parity target stay byte-identical; no tolerance moves. Every buck target passes after every task that touches the elaborator, the IR, the printer or `std`.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` at the amended text is the authority. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the same computation in Rust in the test, or the spec's own value; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`).** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type; no abbreviated name; no god method and no needless abstraction; the data path evident. Every struct, enum, trait, method, field and variant a task adds or changes carries a `///` doc comment saying what it is for; a non-evident block cites its spec rule. No macro. No `unwrap()`/`expect()` on a path user input reaches. A removal removes the whole concept: no dead field, no unused variant, no comment about what used to be there.
- **Conventions.** Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models go in `crates/piperine-parity/tests/value_semantics/*.msl`.
- Zero warnings on every crate the `**Build**` line names.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>` as the session's attribution reminder spells it. No push, no merge.
- **`lessons.py` is not run** during this feature.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Build and test", "Files not to edit casually"), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Parser (`piperine-lang/src/msl`) | integration | each removed form: its code, its span, its help, one diagnostic per use; the neighbouring forms still accepted | `crates/piperine-lang/tests/msl_errors.rs` | `cargo test -p piperine-lang --test msl_errors` |
| Printer and formatter (`msl/print.rs`, `msl/format.rs`) | integration | round trip of the corpus; no removed word printed | `crates/piperine-lang/tests/msl_print.rs` | `cargo test -p piperine-lang --test msl_print` |
| Checker (`piperine-lang/src/types`) | integration | every AC the checker decides; each code with its span and the name it gives | `crates/piperine-lang/tests/types_value_semantics.rs` | `cargo test -p piperine-lang --test types_value_semantics` |
| Elaborator and runs | integration | one test per run-time AC, from MSL source on the Rust backend, expected values computed in the test | `crates/piperine-parity/tests/source_value_semantics.rs` | `cargo test -p piperine-parity --test source_value_semantics` |
| IR (`piperine-ir`) | integration | no slice node or type is constructible; the assignable flag follows the body | `crates/piperine-ir/tests/aggregates.rs` | `cargo test -p piperine-ir --test aggregates` |
| Standard library | integration | builds clean; Part VII fences are excerpts; no removed form | `crates/piperine-lang/tests/std_excerpts.rs`, `std_library.rs` | `cargo test -p piperine-lang --test std_excerpts`, `--test std_library` |
| Tree hygiene | integration | no `.msl` under `examples/`, `crates/*/tests/`, `std/` uses a removed form outside a refusal test | `crates/piperine-lang/tests/suite_hygiene.rs` | `cargo test -p piperine-lang --test suite_hygiene` |
| Spec text and records | integration | one assertion per record changed | `spec_text.rs`, `crates/piperine-cli/tests/docs.rs`, `pom_resolve.rs` | `cargo test -p piperine-lang --test spec_text`, `cargo test -p piperine-cli --test docs`, `cargo test -p piperine-lang --test pom_resolve` |
| Book | integration | each code the Book names is the code the front end reports | `tests/book/language.rs`, `tests/book/types.rs` | `cargo test -p piperine --test book_language`, `--test book_types` |
| Parity and generated code | integration | twins and goldens byte-identical | `crates/piperine-cli/tests/twins.rs`, `crates/piperine-transpile/tests/golden.rs` | `cargo test -p piperine-cli --test twins`, `cargo test -p piperine-transpile --test golden` |
| Contract re-check (T1) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with one or two integration targets | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the elaborator, the IR, the printer, the parser or `std` | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins`, `cargo test -p piperine-transpile`, `cargo test -p piperine-lang`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Contract

```
T1
```

### Phase 2: The amended forms are accepted

```
T2 → T3 → T4 → T5
```

### Phase 3: The tree is rewritten

```
T6 → T7 → T8
```

### Phase 4: The earlier forms are refused and removed

```
T9 → T10 → T11 → T12 → T13
```

### Phase 5: Closure

```
T14 → T15
```

---

## Task Breakdown

### T1: Rebase onto the `composite-values` B7 tip and re-read the anchors

**What**: Rebase `feature/value-semantics` (the spec amendment `cc3599bf`, the Book commit, this plan) onto `feature/composite-values` once B7 has stopped. Resolve `docs/book/content.html`, `tests/book/*`, `GAPS.md`, `spec_text.rs` and `docs.rs` by design.md "Risks" (the amended wording for the language, `composite-values`' facts for what runs). Re-read every file design.md "What exists" names at the new `HEAD` and write on the Handoff: whether T42 and T43 of `composite-values` were committed, and each anchor that moved with the task it affects. The Book status boxes then say `let` assignment and loops run (they do after B5) and keep the "earlier spelling" sentences.
**Where**: `.specs/features/value-semantics/tasks.md`
**Depends on**: None
**Reuses**: design.md "Where it runs"
**Requirement**: VS-11
**Executor**: Sonnet

**Done when**:

- [x] The rebase is clean or its conflicts are resolved without changing behaviour; the `**Build**` line and `cargo test` at the root pass
- [x] The Handoff names what B7 left of T42 and T43 and each anchor that moved
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): value-semantics - contract re-read after the rebase`

---

### T2: Checker: every local is a place, no parameter is

**What**: In `types/assign.rs`, `Places` replaces the `mutable` set: a `DeclKind::Binding` introduced by a `let` or by the pattern of a `match` arm, `if let`, `while let` or `for` is a place with or without `mut`; a function parameter, a closure parameter, a name a closure captures, a body `let`, a `const`, a param and a carrier are not, each `E0183` naming the target and saying what it is (§1.5.12, §3.1.10). `let x;` may be assigned more than once; a read before assignment on some path stays `E0183` (§1.7.5). `Checker::assigned` records the bindings a body assigns. `mut` is still parsed and ignored. Create `types_value_semantics.rs`. `types_control.rs::assigning_a_non_place_is_e0183_naming_the_target` loses its "local without `mut`" case (design "may move").
**Where**: `crates/piperine-lang/src/types/assign.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Checker::let_statement`, composite-values T11
**Requirement**: VS-03
**Executor**: Sonnet

**Done when**:

- [x] `a_let_without_mut_is_assigned_with_every_operator`, `a_parameter_a_capture_and_a_body_let_are_not_places`, `a_pattern_bound_name_is_a_place` and `a_deferred_let_is_assigned_twice_and_never_read_first` pass under `**Run typesvs**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): every local is a place, no parameter is`

---

### T3: Elaborator: the assignable flag follows the body; pattern-bound locals; captures are copies

**What**: In `elab/control.rs`, a local's `piperine_ir::Local::mutable` is set from `Checker::assigned`, whatever the source wrote, and its doc comment in `piperine-ir/src/model.rs` says so. `PatternLocal` in `elab/patterns.rs` copies a pattern-bound name the body assigns into a fresh assignable local at the head of the arm or the loop body, so a `for` counter is never written. A closure reads the copy taken where it was written (§1.5.14). Source tests against the same computation in Rust: a reaction, a function and an `analog` block that assign a `let` without `mut`; an assigned `for` variable; a local assigned after a closure captured it; a caller's local unchanged after the callee rebinds and changes its argument.
**Where**: `crates/piperine-lang/src/elab/control.rs`
**Depends on**: T2
**Reuses**: composite-values T19, T22, T23
**Requirement**: VS-03, VS-04, VS-12
**Executor**: Sonnet

**Done when**:

- [x] `a_let_without_mut_runs_in_a_function_a_reaction_and_analog`, `an_assigned_loop_variable_does_not_change_the_loop`, `a_closure_keeps_the_value_it_captured`, `a_callee_cannot_change_the_callers_local` and `a_value_is_read_twice` pass under `**Run srcvs**`
- [x] `a_local_is_assignable_exactly_when_its_body_assigns_it` passes under `**Run iragg**`
- [x] `**Run goldens**` is green with no golden edited
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): the assignable flag follows the body`

---

### T4: Checker and elaborator: `str` is a value type

**What**: `Ty::Str` is a value type without `Repr` (§1.2.9): the type of a string literal, of a `str` parameter, `const`, local, `Option<str>` payload, struct field and record field; `==` and `!=` compare it; `#[derive(Repr)]` on a struct with a `str` field is `E0110`. The intrinsic signatures of `types/intrinsic.rs` and `elab/intrinsics.rs` match `str` and, until T6, `&str` as well. Source test: `fn tag(name: str) -> bool { name == "a" }` and `log`, `fatal`, `require` records unchanged.
**Where**: `crates/piperine-lang/src/types/ty.rs`
**Depends on**: T3
**Reuses**: the string handling of `types/record.rs`
**Requirement**: VS-07
**Executor**: Sonnet

**Done when**:

- [x] `str_is_a_value_type_in_every_position` passes under `**Run typesvs**`
- [x] `a_string_is_passed_and_compared_by_value` and `the_reporting_functions_emit_the_same_records` pass under `**Run srcvs**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): str is a value type`

---

### T5: Checker: range indexes and array rest patterns are arrays; an array is passed as itself

**What**: `Checker::range_index` in `types/lengths.rs`: `e[a..b]`, `e[a..=b]`, `e[..b]`, `e[a..]`, `e[..]` on an array give `[T; b - a]` with the bounds folded (§1.5.9); a bound that is not a const expression is `E0188` naming it; a range that ends beyond the length or starts after its end is refused at elaboration (§1.5.25, `E0104`). In `types/aggregate_patterns.rs`, `name @ ..` binds `[T; N - k]` and a pattern with more sub-patterns than elements is `E0180` (§1.6.7). An array argument is accepted for a `[T; N]` parameter with no `&`; `[x; N]` takes an `x` of any type (§1.2.4).
**Where**: `crates/piperine-lang/src/types/lengths.rs`
**Depends on**: T4
**Reuses**: composite-values T38, T42 (as B7 left it)
**Requirement**: VS-06
**Executor**: Sonnet

**Done when**:

- [x] `a_range_index_is_an_array_of_the_folded_length`, `a_range_bound_that_is_not_const_is_e0188`, `a_range_index_out_of_the_array_is_refused`, `an_array_rest_pattern_binds_an_array` and `an_array_is_passed_as_itself` pass under `**Run typesvs**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): range indexes and rest patterns are arrays`

---

### T6: `std` and the Part VII fences in the amended spelling

**What**: Rewrite `crates/piperine-lang/std/*.msl` by design.md "The spelling map" (`io.msl`, `nets.msl`, `analog.msl`, `values.msl`): `str`, arrays by value under `const N`, no `*x`, no `mut`, no `Clone`/`Copy`; `wired`, `tri` and `sum` as design.md gives them. Move each Part VII fence with its source in the same commit (`std_excerpts` holds both ways), delete §22's Resolution about the earlier spelling, and make Part VI's and Part VII's `MeasuredDiode` lines agree. The intrinsic signature matches of T4 drop `&str` and `&[..]`. `std` builds with no diagnostic; a model that used `table_model`, `noise_table`, `laplace_zp`, `zi_zp`, `logic` or `matches_bits` gives what it gave.
**Where**: `crates/piperine-lang/std/`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `std_excerpts.rs`, `std_library.rs`, `StandardLibrary::normalized` of the Book tests
**Requirement**: VS-08
**Executor**: Sonnet

**Done when**:

- [x] `every_part_vii_fence_is_in_its_file` and `every_std_item_is_in_a_fence` (the existing two guards) pass under `**Run excerpts**`
- [x] `embedded_std_has_no_diagnostics` and `std_holds_no_removed_form` pass under `**Run stdlib**`
- [x] `section_22_no_longer_says_std_is_in_the_earlier_spelling` passes under `**Run spectext**`
- [x] `the_library_items_that_took_borrows_give_the_same_results` passes under `**Run srcvs**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(std): the library in value semantics`

---

### T7: Examples and gallery fixtures in the amended spelling

**What**: Apply the spelling map to `examples/**/*.msl` (`examples/buck/src/supervisor.msl` derives), to `crates/piperine-lang/tests/fixtures/**` and to the MSL examples of `docs/pom/`. `examples/buck` and `examples/rc` print what they printed.
**Where**: `examples/`
**Depends on**: T6
**Reuses**: `crates/piperine-cli/tests/buck.rs`, `std_galleries.rs`, `pom_docs.rs`
**Requirement**: VS-09
**Executor**: Sonnet

**Done when**:

- [x] `**Run buckcli**` is green with no expected output edited
- [x] The buck targets pass
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(examples): the examples and fixtures in value semantics`

---

### T8: The test corpora in the amended spelling, and the hygiene walk

**What**: Apply the spelling map to every MSL source a test holds: `crates/*/tests/**/*.msl` and the MSL written inside `crates/*/tests/*.rs` and `tests/book/*.rs` (the `composite-values` targets `types_control`, `types_aggregates`, `elab_control`, `source_control` use `let mut` throughout). A test that asserted a refusal the amendment removed is listed on the Handoff and replaced by the criterion that supersedes it; a test that asserts the refusal of a removed form keeps it. Add to `suite_hygiene.rs` a walk, `no_msl_file_uses_a_removed_form`, over `std/`, `examples/` and `crates/*/tests/**/*.msl` for `let mut`, a `&` or `*` in operand-start position, `Clone`/`Copy` in a derive and `'static`.
**Where**: `crates/piperine-lang/tests/suite_hygiene.rs`
**Depends on**: T7
**Reuses**: the walkers of `suite_hygiene.rs`
**Requirement**: VS-09
**Executor**: Sonnet

**Done when**:

- [x] `no_msl_file_uses_a_removed_form` passes under `**Run hygiene**` and fails on a planted file
- [x] `cargo test` at the root passes
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `test: the corpora in value semantics, and a walk that keeps them so`

---

### T9: Parser: `mut`, `ref` and `move` are reserved words; `E0331` is gone

**What**: The three words leave the value keywords of the lexer's table and join the reserved words (A.1). The removed-forms table of `msl/parser/entry.rs` gains the row "the markers `mut`, `ref` and `move`": `E0903` at the word, once, help per word (`let x`: every local is assignable; a pattern binds a copy; a closure captures by value), with recovery that reads the binding or the closure so nothing else is reported. `msl/parser/check.rs` loses the `E0331` rule. Fixtures: the `E0331` fixtures and `types_control.rs::let_mut_in_a_module_body_is_e0331` are replaced by `let_mut_in_a_module_body_is_e0903`; `RETIRED_AHEAD_OF_THE_PARSER` is deleted; "`mut` and pattern parameters" becomes the row "pattern parameters".
**Where**: `crates/piperine-lang/src/msl/parser/entry.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the `REMOVED` fixture table of `msl_errors.rs`
**Requirement**: VS-02
**Executor**: Sonnet

**Done when**:

- [x] `the_markers_are_reserved_words_with_a_help_each`, `let_mut_in_a_module_body_is_e0903` and `e0102_and_e0331_are_raised_nowhere` pass under `**Run errors**`
- [x] `every_parse_phase_row_has_a_fixture` passes with no exception list
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(parser): mut, ref and move are reserved words`

---

### T10: Parser: reference forms are `E0101`, a lifetime is `E0905`, a slice type does not parse

**What**: The removed-forms table gains the row "references and borrows": `E0101` at `&T`, `&mut T`, `&'static str`, a prefix `&`, `&&` or `*` at the start of an operand, a `&p` pattern, `&self` and `&mut self`, once per use, help "pass the value itself: every value is copied" (and "`self`" for a receiver); the rows "`&self` and `&mut self` receivers" and "double borrows" go. `'static` is `E0905` like any lifetime. `[T]` as a type is a syntax error at the type. Binary `&`, `&&` and `*` are untouched: fixtures for `a & -b`, `a && !b`, `a * (b)`. `ROWS_AHEAD_OF_THE_PARSER` and `FIXTURES_BEHIND_THE_SPEC` are deleted.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs`
**Depends on**: T9
**Reuses**: T9's row mechanism
**Requirement**: VS-01
**Executor**: Sonnet

**Done when**:

- [x] `every_reference_form_is_e0101_once`, `a_lifetime_is_e0905_static_included`, `a_slice_type_does_not_parse` and `binary_ampersand_and_star_still_parse` pass under `**Run errors**`
- [x] `every_b36_row_has_a_fixture` passes with no exception list
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(parser): reference forms are E0101`

---

### T11: The syntax tree, the object model, the printer and the formatter lose the removed forms

**What**: Delete `ast::Type::Ref`, `Type::Slice`, `Pat::Ref`, the `by_ref` and `mutable` fields of `Pat::Ident`, `UnOp::Deref`, `UnOp::Ref` and the closure's `move` flag, and their mirrors in `pom/body.rs`, `pom/lower`, `pom/to_ast.rs` and `pom/reflect`; `msl/print.rs` and `msl/format.rs` lose their arms; `Pat::Slice` is renamed `Pat::Array`. `docs/pom/` names the types that remain (`pom_docs` guard).
**Where**: `crates/piperine-lang/src/msl/ast.rs`
**Depends on**: T10
**Reuses**: `msl_print.rs`, `msl_format.rs`, `pom_corpus.rs`, `pom_docs.rs`
**Requirement**: VS-09
**Executor**: Sonnet

**Done when**:

- [x] `the_printer_and_the_object_model_hold_no_removed_form` passes under `**Run print**`
- [x] `cargo test -p piperine-lang` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(lang): the tree holds no reference, marker or slice`

---

### T12: The checker and the IR lose borrows and slices

**What**: Delete `Ty::Ref`, the borrow, dereference and slice rules of `types/aggregate.rs` (whatever `composite-values` T42 and T43 left, per the Handoff of T1) and `Checker::mutable`'s remains. Delete `Expr::SliceOf` and `ValueType::Slice` from `piperine-ir` (`model.rs`, `build.rs`, `check/aggregates.rs`, `print.rs`, `deps.rs`), their rows in `piperine-interp`'s refusal list and `frozen.rs`, and their arms in `piperine-transpile/src/print/aggregate.rs`.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T11
**Reuses**: `crates/piperine-ir/tests/aggregates.rs`
**Requirement**: VS-12
**Executor**: Sonnet

**Done when**:

- [x] `the_ir_holds_no_slice` passes under `**Run iragg**`
- [x] `cargo test -p piperine-ir -p piperine-interp -p piperine-transpile -p piperine-lang` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(ir): no slice type and no slice expression`

---

### T13: `Clone` and `Copy` leave the built-in table

**What**: Delete the two rows of `pom/builtin.rs`. A derive or a bound naming either, and a call of `clone()`, is `E0161` through the path of any unresolved name; the unit that builds `E0161` adds the help "every value is copied where it is used; delete the name" for these three names. `pom_resolve.rs::builtin_table_matches_appendix_h` loses the line that adds them. A `#[derive(Repr, PartialEq)]` enum is carried, compared and printed as before.
**Where**: `crates/piperine-lang/src/pom/builtin.rs`
**Depends on**: T12
**Reuses**: `pom_resolve.rs`
**Requirement**: VS-05
**Executor**: Sonnet

**Done when**:

- [x] `builtin_table_matches_appendix_h` passes under `**Run resolve**` with no added name
- [x] `clone_and_copy_are_e0161_with_a_help` and `clone_as_a_method_is_e0161` pass under `**Run typesvs**`
- [x] `an_enum_without_copy_is_carried_and_compared` passes under `**Run srcvs**`
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(lang): Clone and Copy are not names of the language`

---

### T14: The Book states the codes the binary reports

**What**: Chapters 2 and 3: the status-box sentences about the earlier spellings (`language~8883ef8c`, `types~ee044194` and their neighbours) leave; the "Common mistakes" tables gain `E0903` for `let mut x = 0;`, `E0101` for `fn mean(xs: &[f64])` and `E0161` for `#[derive(Clone, Copy)]`; the modules chapter's `E0331` row becomes the `E0903` row. `tests/book/inventory.tsv` follows; the four tests design.md "may move" names are replaced by `e0903_mut_is_not_a_word_of_the_language`, `e0101_nothing_is_lent` and `e0161_copying_is_not_a_trait`.
**Where**: `docs/book/content.html`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Faulty::elaborating`, the claim guard
**Requirement**: VS-10
**Executor**: Sonnet

**Done when**:

- [x] `e0903_mut_is_not_a_word_of_the_language` passes under `**Run booklang**`
- [x] `e0101_nothing_is_lent` and `e0161_copying_is_not_a_trait` pass under `**Run booktypes**`
- [x] `cargo test -p piperine --test 'book_*'` and `cargo test -p piperine-cli --test book` pass
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs(book): the removed spellings by the codes the compiler reports`

---

### T15: Records: G.71, `GAPS.md` S7, the decision log

**What**: Appendix G.71 loses its last paragraph on the reference implementation and its status becomes Closed in the entry and the summary (the For-review line stays as a Provisional note if the user has not confirmed D5 and D7); `GAPS.md` drops S7, restores the counts and the code list (`E0331` no longer raised); `spec_text.rs` and `docs.rs` follow; `.specs/STATE.md` gains the next free `MD-` entry (pure value semantics, D1 to D8); `ROADMAP.md` names the feature delivered; `composite-values/tasks.md` and `spec.md` take the amendments of design.md "Changes to `composite-values`" so B8 starts from them.
**Where**: `GAPS.md`
**Depends on**: T14
**Reuses**: `docs.rs`, `spec_text.rs`
**Requirement**: VS-10, VS-11
**Executor**: Sonnet

**Done when**:

- [x] `gaps_drops_the_value_semantics_gap` passes under `**Run docs**`
- [x] `g71_is_closed_and_names_no_implementation_gap` passes under `**Run spectext**`
- [x] `**Run twins**` is green with no golden edited
- [x] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: value semantics delivered`

---

## Checks

**C1** - Each reference form is `E0101` at the form, once, with its help. (VS-01, AC 1; T10)
Proof: run errors | test every_reference_form_is_e0101_once ... ok

**C2** - A lifetime token, `'static` included, is `E0905`. (VS-01, AC 2; T10)
Proof: run errors | test a_lifetime_is_e0905_static_included ... ok

**C3** - A slice type `[T]` is a parse error at the type. (VS-01, AC 3; T10)
Proof: run errors | test a_slice_type_does_not_parse ... ok

**C4** - Binary `&`, `&&` and `*` parse as before. (VS-01, AC 4; T10)
Proof: run errors | test binary_ampersand_and_star_still_parse ... ok

**C5** - `mut`, `ref` and `move` are each `E0903` at the word, once, with a help naming the plain form. (VS-02, AC 1; T9)
Proof: run errors | test the_markers_are_reserved_words_with_a_help_each ... ok

**C6** - `let mut` in a module body is `E0903` and not `E0331`. (VS-02, AC 2; T9)
Proof: run errors | test let_mut_in_a_module_body_is_e0903 ... ok

**C7** - No source raises `E0102` or `E0331`. (VS-02, AC 3; T9)
Proof: run errors | test e0102_and_e0331_are_raised_nowhere ... ok

**C8** - A `let` without `mut` is assigned with `=` and every compound operator, and runs with Rust's values in a function, a reaction and `analog`. (VS-03, AC 1; T2, T3)
Proof: run srcvs | test a_let_without_mut_runs_in_a_function_a_reaction_and_analog ... ok

**C9** - An assignment to a parameter, a closure parameter, a captured name, a body `let`, a `const`, a param or a carrier is `E0183` naming the target. (VS-03, AC 2; T2)
Proof: run typesvs | test a_parameter_a_capture_and_a_body_let_are_not_places ... ok

**C10** - A name bound by a `match` arm, `if let`, `while let` or `for` is assignable, and assigning a `for` variable does not change the loop. (VS-03, AC 3; T2, T3)
Proof: run srcvs | test an_assigned_loop_variable_does_not_change_the_loop ... ok

**C11** - `let x;` may be assigned more than once and is `E0183` when read before it is assigned. (VS-03, AC 4; T2)
Proof: run typesvs | test a_deferred_let_is_assigned_twice_and_never_read_first ... ok

**C12** - A closure keeps the value it captured when the local is assigned afterwards. (VS-04, AC 1; T3)
Proof: run srcvs | test a_closure_keeps_the_value_it_captured ... ok

**C13** - A callee that rebinds and changes its argument leaves the caller's local as it was. (VS-04, AC 2; T3)
Proof: run srcvs | test a_callee_cannot_change_the_callers_local ... ok

**C14** - A value of any type is read twice with no refusal. (VS-04, AC 3; T3)
Proof: run srcvs | test a_value_is_read_twice ... ok

**C15** - `Clone` or `Copy` in a derive or a bound is `E0161` at the name with the help. (VS-05, AC 1; T13)
Proof: run typesvs | test clone_and_copy_are_e0161_with_a_help ... ok

**C16** - An enum deriving `Repr` without `Copy` is carried, compared and printed as before. (VS-05, AC 2; T13)
Proof: run srcvs | test an_enum_without_copy_is_carried_and_compared ... ok

**C17** - `clone()` is `E0161` naming the method and the type. (VS-05, AC 3; T13)
Proof: run typesvs | test clone_as_a_method_is_e0161 ... ok

**C18** - A range index with const bounds has the type `[T; b - a]`. (VS-06, AC 1; T5)
Proof: run typesvs | test a_range_index_is_an_array_of_the_folded_length ... ok

**C19** - A range bound that is not a const expression is `E0188` naming it. (VS-06, AC 2; T5)
Proof: run typesvs | test a_range_bound_that_is_not_const_is_e0188 ... ok

**C20** - A const range index beyond the array or with its start after its end is refused at elaboration. (VS-06, AC 3; T5)
Proof: run typesvs | test a_range_index_out_of_the_array_is_refused ... ok

**C21** - `name @ ..` binds an array, and too many sub-patterns are `E0180`. (VS-06, AC 4; T5)
Proof: run typesvs | test an_array_rest_pattern_binds_an_array ... ok

**C22** - An array is the argument of a `[T; N]` parameter, with no `&`. (VS-06, AC 5; T5)
Proof: run typesvs | test an_array_is_passed_as_itself ... ok

**C23** - `str` is accepted as a parameter, a `const`, a local, an `Option` payload and a record field, and compares with `==`. (VS-07, AC 1; T4)
Proof: run typesvs | test str_is_a_value_type_in_every_position ... ok

**C24** - `log`, `fatal`, `require`, `cover` and `target` emit the records they emitted. (VS-07, AC 2; T4)
Proof: run srcvs | test the_reporting_functions_emit_the_same_records ... ok

**C25** - `std` holds no removed form, and its resolvers take `[T; N]`. (VS-08, AC 1; T6)
Proof: run stdlib | test std_holds_no_removed_form ... ok

**C26** - `std` builds with no diagnostic and every Part VII fence is an excerpt of it. (VS-08, AC 2; T6)
Proof: run excerpts | test every_part_vii_fence_is_in_its_file ... ok

**C27** - §22 no longer says `std` is in the earlier spelling. (VS-08, AC 3; T6) (retired by code-clarity T11: a pin on a sentence of the specification; the specification is the authority)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C28** - The library items that took borrows give the results they gave. (VS-08, AC 4; T6)
Proof: run srcvs | test the_library_items_that_took_borrows_give_the_same_results ... ok

**C29** - No `.msl` file of `std`, `examples` or a test directory uses a removed form. (VS-09, AC 1; T8)
Proof: run hygiene | test no_msl_file_uses_a_removed_form ... ok

**C30** - `examples/buck` prints what it printed. (VS-09, AC 2; T7)
Proof: run buckcli | test startup_prints_the_whole_start_on_its_grid ... ok

**C31** - The printer, the formatter and the object model hold no removed form. (VS-09, AC 3; T11)
Proof: run print | test the_printer_and_the_object_model_hold_no_removed_form ... ok

**C32** - No test lists a specification row or code as ahead of the implementation. (VS-10, AC 1; T9, T10, T13)
Proof: run resolve | test builtin_table_matches_appendix_h ... ok

**C33** - `GAPS.md` has no S7 and G.71 names no implementation gap. (VS-10, AC 2; T15) (retired by code-clarity T10: a pin on a sentence of a tracking file; the tracking file is not behaviour)
Proof: run ledger | test every_retired_check_is_recorded ... ok

**C34** - The Book names `E0903` for `let mut`, held by the front end. (VS-10, AC 3; T14)
Proof: run booklang | test e0903_mut_is_not_a_word_of_the_language ... ok

**C35** - The Book names `E0101` for a lent table and `E0161` for a `Copy` derive, held by the front end. (VS-10, AC 3; T14)
Proof: run booktypes | test e0101_nothing_is_lent ... ok

**C36** - The 7 twins print the bytes they printed, with no contract edited. (VS-11, AC 1; T15)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C37** - The transpile goldens are byte-identical. (VS-11, AC 2, AC 3; T3)
Proof: run goldens | test the_five_m1_bodies_print_as_the_golden_files ... ok

**C38** - The IR holds no slice type and no slice expression. (VS-12, AC 1; T12)
Proof: run iragg | test the_ir_holds_no_slice ... ok

**C39** - An IR local is assignable exactly when its body assigns it. (VS-12, AC 2; T3)
Proof: run iragg | test a_local_is_assignable_exactly_when_its_body_assigns_it ... ok

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 (2026-11-21, batch B1, branch `feature/composite-values`, merge `309eec36` of `origin/main` `f9a29dd0` into the B7 tip; no rebase, the Book/spec/GAPS conflicts resolved by design.md "Risks"). Anchors at the merged `HEAD`: (1) `composite-values` T42 was **narrowed** and T43 **dropped** before B7 stopped: `Ty::Slice`, `Ty::Ref`, `E0102` and the borrow expressions were never committed, a range index `e[a..b]` is refused by name (`types/aggregate.rs`: "a slice `e[a..b]`"), the checker knows `Ty::Array` and slice *patterns* over arrays only; so T12's checker part removes only `Checker::mutable`'s remains (already gone in T2) and the IR part stays (`Expr::SliceOf`, `ValueType::Slice` were built by T31). (2) `Checker::mutable` was a `BTreeSet<DeclId>` in `types/check.rs`; it became `Places` in `types/assign.rs` (T2). (3) The elaborator's `Elaboration::declared_mutable` (`elab/control.rs`) reads the source's `mut`; T3 replaces it by `Types::is_assigned`. (4) `E0183` for an immutable local and the at-most-once `let x;` rule lived in `types/assign.rs` (`assignable_once`, `Assigned::possibly`): both removed in T2. (5) Parser anchors unchanged: `msl/parser/entry.rs` (removed-forms table), `check.rs` (`E0331`, line `PatKind::Ident { mutable: true, .. }`). (6) The Book status boxes keep "earlier spelling" sentences, re-pointed to what the binary reports: until T3 an assignment to a `let` without `mut` is `P0001` ("an assignment to a local that is not mutable here"), `language#13` is a `refused` row owned by `value-semantics` (GAPS section 11), and T3 flips it back to `run`. (7) The tests named in the Handoff "To confirm" line still exist under those names. B1 settled items (T2 to T8, 2026-11-21, user AFK, defaults taken): (1) `Places` (`types/assign.rs`) holds the closure-span stack and the assigned set; `Types::is_assigned` hands the set to the elaborator; a closure parameter, a body `let`, a generation `for` binding, `self` and a name a closure captures are `E0183` with the wording "only a local that a `let` or a pattern binds is a place"; `Assigned::possible` and the at-most-once rule are gone. (2) Between T2 and T3 the Book pinned `language#13` as a `refused` row (`P0001`, S7) and T3 flipped it back to `run`; the status-box sentence about `let mut` now says it is accepted where it changes nothing. (3) T3: a pattern-bound name the body assigns is the fresh local `binding_lets` already writes at the head of the arm; only a `for` variable needed `assignable_copy` (`elab/patterns.rs`); an assigned `Option` local is `P0001` ("an assignment to a local of type `Option`"). (4) T4: a `str` is text known at elaboration (`text_of`, `lower_text_let`, `lower_text_comparison` in `elab/records.rs`); a call of a function that takes a `str` is inlined like an emitting function (`is_inlined`); `Option<str>` and a `str` struct field are typed only (the aggregates are B8); a module-body `let s = "a"` stays `P0001`; `E0110` is raised for a `str` field under `#[derive(Repr)]` only. Superseded pins replaced: two `types_subset` rows, `types_values` and `elab_results` wording. (5) T5: a range index out of the array or starting after its end is `E0104` (checker), a pattern with the wrong number of sub-patterns is `E0180`; the elaborator still refuses the range index by name until composite-values T51'; `arrays_indexing_and_slices_type_check` is now `arrays_and_indexing_type_check`. (6) T6: the member resolver read an array of structs as the struct (`d.iter()` on `[Logic; N]` was unresolved): `Recv::StructArray` and `Recv::InstanceArray` fix it; `std` and the Part VII fences moved together, §22's Resolution and G.71's sentence about the spelling are gone. (7) T7: only `examples/buck/src/supervisor.msl` and one `docs/pom` example held a removed form. (8) T8: every `.msl` under `crates/*/tests` rewritten; Rust test files rewritten only inside their MSL string literals and only in `piperine-lang/tests` (`types_*`, `elab_*`) and `tests/book/types.rs`; `piperine-transpile` and `piperine-ir` tests (generated Rust and IR text keep `let mut`/`Clone, Copy` by design), the parser/printer/POM tests (`msl_*`, `pom_*`, `spec_text`) and the Book tests that pin the earlier spellings stay for B2 (T9 to T14); the hygiene walk `no_msl_file_uses_a_removed_form` is `RemovedForms` in `suite_hygiene.rs`. Left for B2: Book chapters after 2 and 3 still say `let mut` in prose (digital chapter), `parse_table`/`parse_hex` run nowhere (`include_str!` is refused), `table_model` is not run from source. B2 settled items (T9 to T15, 2026-11-21, user AFK, defaults taken): (1) the markers are reserved words the parser still reads (`ReservedMarker`, `msl/parser/base.rs`) so a `let mut x`, `ref y`, `move |a|` or `mut x: T` gives one `E0903` at the word and is read as the plain form; (2) `reference_head` reads `&`, `&&`, `&mut`: one `E0101` per use, read as the operand (type, pattern, expression, receiver, with the receiver's help naming `self`); the lifetime after a `&` is one `E0905` and no token (`Token::StaticLifetime` is gone), other lifetimes keep the error token; (3) a slice type `[T]` is E0900 at the type and stands as `_`, so `&[T]` still reports its reference (found by the Book test `e0101_nothing_is_lent`, committed in T14); (4) `Pat::Slice` is `Pat::Array`; the printer, `docs/pom` and the resolver lost every removed form; (5) T12 removed `SliceOf`, `ValueType::Slice`, the slice-position rule, the interpreter row (the refusal list renumbered) and `rt::Index::slice`; the transpile fixture `span` takes `[u8; 4]`; (6) `clone()` is E0161 for every receiver type (it was P0001 for an uncatalogued one); `COPY_HELP` lives in `msl::diag`; (7) Book: the status sentences left, the tables gained the three rows, the claim rows `modules~80317e1d` and `types~399f441d` follow; one Book test cites the shared code cells (`language~80317e1d`, `types~39a5d01d`); (8) G.71 is Closed with a Provisional note (D5, D7 unconfirmed); MD-76 records it; `composite-values` tasks and spec carry the amendments of design.md (T42 and T43 included, as the B7 tip never had them); (9) per the user's policy of 2026-11-21 the per-task Full gate ran as Build plus the touched crates' tests, and the full root run once at the batch end. Planned 2026-11-20 on `feature/value-semantics` (from `origin/main` `45737a10`): spec amendment `cc3599bf`, the Book commit after it, then this plan. Not approved: spec.md D1 to D8 wait for the user.
**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

**To confirm in T1:** C26, C30, C36 and C37 cite tests that exist on `main` today (`every_part_vii_fence_is_in_its_file`, `startup_prints_the_whole_start_on_its_grid`, `every_twin_prints_the_bytes_it_printed_before_the_move`, `the_five_m1_bodies_print_as_the_golden_files`); T1 reads them again on the rebased tree and corrects a name that moved.

## Dependency diagram

```
Phase 1: T1
Phase 2: T2 -----→ T3 -----→ T4 -----→ T5
Phase 3: T6 -----→ T7 -----→ T8
Phase 4: T9 -----→ T10 ----→ T11 ----→ T12 ----→ T13
Phase 5: T14 ----→ T15
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| B1 | 1, 2, 3 | T1 to T8 | starts when `composite-values` B7 has stopped; ends with the whole tree in the amended spelling and the earlier one still parsed |
| B2 | 4, 5 | T9 to T15 | the parser refuses the earlier spelling; closes `GAPS.md` S7; `composite-values` B8 starts on its tip with the amended tasks |

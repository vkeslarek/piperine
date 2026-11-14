# MSL Standard Library Tasks

**Approved**: 2026-11-14 - user: "Approve, run after merge (Recommended)"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-stdlib/design.md`
**Status**: Draft
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run spec**: `cargo test -p piperine-lang --test spec_text`
**Run resolve**: `cargo test -p piperine-lang --test pom_resolve`
**Run docs**: `cargo test -p piperine-lang --test pom_docs`
**Run std**: `cargo test -p piperine-lang --test std_library`
**Run excerpts**: `cargo test -p piperine-lang --test std_excerpts`
**Run galleries**: `cargo test -p piperine-lang --test std_galleries`
**Run book**: `cargo test -p piperine-lang --test book`

**Start condition.** Execution starts after `feature/msl-packages` is merged to `main` and this branch (`feature/msl-stdlib`) is rebased onto it. The first worker re-reads `crates/piperine-lang/src/pom/stdlib.rs`, `src/pom/design.rs` and `src/pom/resolve/` before T5: they may have moved since this plan was written.

Executors: Sonnet sub-agents, one commit per task. Verifier: Opus. Work in `/home/keslarek/Git/piperine-msl-stdlib`, branch `feature/msl-stdlib`. Do not set `CARGO_TARGET_DIR` (the environment provides it). Never run `cargo clean`. Never pass `--workspace`: only `-p piperine-lang`; the other crates do not build against the MSL front end and stay that way. `CLAUDE.md` still describes the retired Verilog-AMS front end; `docs/spec/` and `docs/pom/` are the reference for MSL.

Rules every task follows:

- Tests first: write the tests the task's checks cite, run them red, then implement. A test function has exactly the name its check cites.
- A test asserts the spec's outcome on input written in the test, on the text of `docs/spec/`, on the files of `std/` or on the embedded design; it never restates the implementation.
- A new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. New targets: `std_excerpts` (T8), `std_library` (T9), `std_galleries` (T18), `book` (T22). Files under `tests/support/` are `#[path]`-included, never targets.
- An existing test is only strengthened. The one exception: a task's **Existing tests** line names an assertion a decision of `spec.md` moves. Then exactly that assertion changes and the commit body names the test and the decision. An existing test that goes red for any other reason: stop and report, do not edit it.
- `docs/spec/` follows `.specs/features/msl-spec/STYLE.md`: numbered rules, `> **Resolution.**` notes, error tables `Code | Condition | Phase`, sentences under 30 words, no history.
- A spec edit lands where its guard can pass (MD-57): a `rust` fence must parse and round-trip in the commit that adds or edits it, and from T8 on a `rust` fence of a covered section of Part VII must be text of its `std` file in the same commit.
- The text of `std` is Part VII's text. Where this feature writes a body, the text is the one in `design.md` ("Bodies written by this feature"). A body the parser or the POM rejects is a stop: report the form and the diagnostic. Turning a body into an `extern "intrinsic"` declaration, or adding a sixth intrinsic, is never a fix.
- A `std` file holds no `use`: it reaches the other namespaces through the prelude (§3.5 rule 9).
- A public POM type gets its `### ` heading and a public reflection method its row in `docs/pom/` in the task that adds it (`pom_docs` enforces both).
- No macro is written (`macro_rules!`, proc-macros): data and plain functions. Derives are fine.
- Only lowering creates POM nodes; resolution writes slots and side tables (MD-54). No POM function panics on any input.
- `piperine-lang` gains no dependency. No test contacts the network.
- Zero warnings in `piperine-lang` at every commit.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Where the tests are": a target lives in the crate it exercises, one concern per target, a `//!` header; "Build and test": zero warnings), `tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope `allow`; the root crate does not build today, so its rules are applied by hand), `.specs/STATE.md` MD-57 (spec edits and their guards). Sampled: `spec_text.rs`, `msl_corpus.rs`, `pom_corpus.rs`, `pom_crates.rs`, `pom_resolve.rs`, `pom_docs.rs`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Specification text (`docs/spec/*.md`) | integration | one assertion per decided wording: STD-01, STD-02 (a row, a code, a short phrase; never a sentence) | `crates/piperine-lang/tests/spec_text.rs` | run spec |
| POM resolution (`src/pom/resolve/`, `src/pom/builtin.rs`, `src/pom/reflect/`) | integration | STD-04, STD-05, STD-06, STD-15: each rule with its positive and its negative case, each diagnostic with its code and span | `crates/piperine-lang/tests/pom_resolve.rs` | run resolve |
| Reference docs (`docs/pom/`) | integration | STD-17; the existing heading, method, fence and nav guards; the doctests | `crates/piperine-lang/tests/pom_docs.rs`, doctests of `pom::guide` | run docs |
| The library (`std/*.msl`, `src/pom/stdlib.rs`) | integration | STD-07 to STD-10, STD-16: layout, content, the intrinsic list, cleanliness, the structure of each new body | `crates/piperine-lang/tests/std_library.rs` | run std |
| Fences of Part VII against `std` | integration | STD-11, STD-12: both directions, each with a control on text written in the test, and a floor | `crates/piperine-lang/tests/std_excerpts.rs` | run excerpts |
| Galleries (Appendices D, E, F) and their fixtures | integration | STD-13, STD-14: all 18 fences, the four senses of clean, the control against an empty prelude | `crates/piperine-lang/tests/std_galleries.rs` | run galleries |
| The MSL Book (`docs/book/`) | integration | STD-18: every block parses, the chapter names every core name and export, the build runs | `crates/piperine-lang/tests/book.rs` | run book |
| Test support moved between files (`tests/support/`) | none | build gate; the targets that include it keep their test counts | - | build |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | after a task that edits `docs/spec/` or `docs/book/` and no Rust source outside its test target | the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text` |
| Full | after a task that changes `src/`, `std/`, shared test support or `docs/pom/` | `cargo test -p piperine-lang` (it includes the doctests) |
| Build | after each phase | the `**Build**` line, then `cargo test -p piperine-lang` |

---

## Execution Plan

Phases are ordered and run sequentially; tasks within a phase execute in order. Each phase starts from the last task of the one before.

### Phase 1: Specification

```
T1 → T2 → T3 → T4
```

### Phase 2: POM: the forms the library uses

```
T4 → T5 → T6 → T7
```

### Phase 3: Library I: guards, prelude, values, analog

```
T7 → T8 → T9 → T10 → T11 → T12
```

### Phase 4: Library II: nets, converters, monitors, output, completeness

```
T12 → T13 → T14 → T15 → T16 → T17
```

### Phase 5: Gates

```
T17 → T18 → T19 → T20 → T21
```

### Phase 6: The MSL Book

```
T21 → T22 → T23 → T24
```

---

## Task Breakdown

### T1: §3 defines `extern "intrinsic" fn`

**What**: §3 rule 2 and rule 10 of Part I name the second ABI string `"intrinsic"`: accepted only in the crate `std`, no body, no scalar restriction, no `#[derivative]`, the contract is the item's entry in Part VII. The `E0125` row gains the condition "the ABI `"intrinsic"` outside the crate `std`". A `> **Resolution.**` note says why (an item MSL cannot express; Rust's `extern "rust-intrinsic"`).
**Where**: `docs/spec/part_i_values.md` (+ `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: None
**Reuses**: `spec_text.rs`: `section`, `table_rows`, `one_line`, the existing `e0125_covers_bodies`
**Requirement**: STD-02

**Done when**:

- [x] Test `s3_lists_the_intrinsic_abi` passes in `spec_text.rs` (C11: STD-02 AC 1)
- [x] Test `s3_states_the_intrinsic_contract` passes in `spec_text.rs` (C12: STD-02 AC 2)
- [x] Test `e0125_covers_intrinsic_outside_std` passes in `spec_text.rs` (C13: STD-02 AC 3)
- [x] Test `b1_still_counts_65_constructs` passes in `spec_text.rs` (C16: STD-02 AC 6)
- [x] No grammar production changes (B.3.2 `fn_decl` already takes `extern STRING`); Appendix B.1 is not edited
- [x] The existing test `e0125_covers_bodies` still passes unchanged
- [x] Gate passes: the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text`; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): extern "intrinsic" fn is the form of an engine-implemented std item`

---

### T2: §22 says the library is source: rules, "Sources", "Implemented by the engine"

**What**: The preamble of §22: rule 1 and rule 4 reworded (the library is the crate `std`, its files are normative, a `rust` fence of this part is an excerpt; an item given as a signature and a contract is declared `extern "intrinsic"`). Two tables: "Sources" (section, namespace, file: the eight rows of `design.md`) and "Implemented by the engine" (item, entry, reason: `matches_bits`, `logic`, `table_model`, `parse_table`, `parse_hex`, each "reads the characters of a `str`"). The Resolution "Listings that omit `pub` ... are read as if written with `pub`" is removed.
**Where**: `docs/spec/part_vii_stdlib.md` (+ `tests/spec_text.rs`)
**Depends on**: T1
**Reuses**: `design.md`, "The crate `std`" (the file table) and "The split"
**Requirement**: STD-01, STD-02

**Done when**:

- [ ] Test `s22_says_std_is_the_normative_source` passes in `spec_text.rs` (C1: STD-01 AC 1)
- [ ] Test `s22_maps_sections_to_namespaces` passes in `spec_text.rs` (C2: STD-01 AC 2)
- [ ] Test `s22_drops_the_implied_pub_resolution` passes in `spec_text.rs` (C3: STD-01 AC 3)
- [ ] Test `s22_lists_the_engine_items` passes in `spec_text.rs` (C14: STD-02 AC 4)
- [ ] Test `s22_rule_4_names_extern_intrinsic` passes in `spec_text.rs` (C15: STD-02 AC 5)
- [ ] No fence of Part VII is edited in this task (fences travel with their `std` file, T9 to T16)
- [ ] The Resolution that an implementation MAY implement an item natively stays
- [ ] Gate passes: the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): Part VII is the text of std; sources and engine-implemented tables`

---

### T3: The analysis kind is `Transient`

**What**: Rename the analysis kind `Tran` to `Transient` everywhere it is the kind, and nowhere it is the module of §22.3.7: the table of occurrences in `design.md` ("Spec edits") lists each file and line. The built-in row `Tran` becomes `Transient` (same kind, same section) and the two `Variant` rows of `docs/pom/` follow.
**Where**: `docs/spec/part_iv_behavior.md` (+ `part_v_execution.md`, `part_vii_stdlib.md`, `appendix_b_grammar.md`, `appendix_d_gallery_analog.md`, `appendix_f_gallery_mixed.md`, `appendix_g_scope.md`, `docs/pom/resolution.md`, `docs/pom/model.md`, `crates/piperine-lang/src/pom/builtin.rs`, `tests/spec_text.rs`, `tests/pom_resolve.rs`)
**Depends on**: T2
**Reuses**: `grep -n '\bTran\b' docs/spec docs/pom crates/piperine-lang` as the worklist; `spec_corpus::corpus` for the fence test
**Requirement**: STD-01, STD-15
**Existing tests**: decision Q3 moves the spelling of one built-in row: assertions that spell the variant `Tran` (`pom_resolve.rs`, `pom_docs`, doctests of `docs/pom/`) change to `Transient`

**Done when**:

- [ ] Test `analysis_kinds_list_transient` passes in `spec_text.rs` (C7: STD-01 AC 7)
- [ ] Test `no_analysis_tran_remains` passes in `spec_text.rs` (C8: STD-01 AC 8)
- [ ] Test `tran_in_fences_is_the_module` passes in `spec_text.rs` (C9: STD-01 AC 9)
- [ ] Test `transient_is_a_builtin_variant` passes in `pom_resolve.rs` (C90: STD-15 AC 3)
- [ ] The module `Tran` keeps its name in §22.3.7, §22.3.8, the §22 Index, `part_iii_structure.md` (the fence near line 454), Appendix C (line 120), Appendix F ("Models referenced", F.4) and G.38
- [ ] The prose "transient", "transient step", "a transient" is not touched
- [ ] Any existing test that names the built-in `Tran` is updated to `Transient` and nothing else in it changes; the commit body lists each
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `refactor(spec,pom): the analysis kind is Transient; Tran is the pass switch`

---

### T4: Appendix G records S1 to S5; §1 places the constants in `std::constants`

**What**: Appendix G's decision table gains S1 to S5 (Q1 to Q5 as answered, one line each, with the sections they touch). §1 rule 3 and the caption of "Prelude constants" say the six constants are `pub const` items of `std::constants`, re-exported by the prelude.
**Where**: `docs/spec/appendix_g_scope.md` (+ `docs/spec/part_i_values.md`, `tests/spec_text.rs`)
**Depends on**: T3
**Reuses**: the P1 to P6 rows of Appendix G as the pattern; `appendix_g_records_the_package_decisions` as the test pattern
**Requirement**: STD-01

**Done when**:

- [ ] Test `s1_constants_are_std_items` passes in `spec_text.rs` (C6: STD-01 AC 6)
- [ ] Test `appendix_g_records_the_stdlib_decisions` passes in `spec_text.rs` (C10: STD-01 AC 10)
- [ ] G.38 keeps the status `Open` (T19 closes it)
- [ ] No fence is added in this task (the §1 fence of the constants is T9)
- [ ] Gate passes: the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): stdlib decisions S1 to S5; the prelude constants are std items`

---

### T5: The POM accepts `extern "intrinsic" fn` in `std` only

**What**: Stage 8 reports `E0125` at the name of a function whose ABI is `intrinsic` when its crate is not `std`, with the help line of `design.md`. `Function::is_intrinsic()` is added. `docs/pom/resolution.md` gains the condition in the `E0125` row and names `is_intrinsic`; `docs/pom/reflection.md` gets the method's row.
**Where**: `crates/piperine-lang/src/pom/resolve/shape.rs` (+ `src/pom/reflect/items.rs`, `docs/pom/resolution.md`, `docs/pom/reflection.md`, `tests/pom_resolve.rs`, `tests/pom_docs.rs`)
**Depends on**: T4
**Reuses**: the existing `E0125` branches of `shape.rs`; `Function::abi()`; `Crate::is_std()`; `std_source` to put an intrinsic in a test library
**Requirement**: STD-04, STD-17

**Done when**:

- [ ] Test `intrinsic_fn_is_accepted_in_std` passes in `pom_resolve.rs` (C20: STD-04 AC 1)
- [ ] Test `intrinsic_fn_outside_std_is_e0125` passes in `pom_resolve.rs` (C21: STD-04 AC 2)
- [ ] Test `intrinsic_fn_with_a_body_is_e0125` passes in `pom_resolve.rs` (C22: STD-04 AC 3)
- [ ] Test `is_intrinsic_reads_the_abi` passes in `pom_resolve.rs` (C23: STD-04 AC 4)
- [ ] Test `extern_c_fn_is_not_intrinsic` passes in `pom_resolve.rs` (C24: STD-04 AC 5)
- [ ] Test `a_call_to_an_intrinsic_resolves` passes in `pom_resolve.rs` (C25: STD-04 AC 6)
- [ ] Test `resolution_doc_describes_intrinsics` passes in `pom_docs.rs` (C96: STD-17 AC 2)
- [ ] An `extern "C" fn` is reported exactly as before
- [ ] `pom_docs` passes: the new method has its row, the new condition its text
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): extern "intrinsic" fn is legal in std and E0125 elsewhere`

---

### T6: `Self::NAME` in a trait resolves through its supertraits

**What**: `Self::NAME` inside a trait looks in the trait's items, then in each supertrait in written order: a user trait's items, or the `Repr` member rows. Another built-in supertrait gives `Deferred(BuiltinTraitMember)`. Nothing found is `E0161`. `docs/pom/resolution.md` states the rule beside the rule for a generic's bounds.
**Where**: `crates/piperine-lang/src/pom/resolve/names.rs` (+ `docs/pom/resolution.md`, `tests/pom_resolve.rs`, `tests/pom_docs.rs`)
**Depends on**: T5
**Reuses**: the code path that resolves `A::WIDTH` through the bounds of a generic `A`; `builtin_member`
**Requirement**: STD-05, STD-17

**Done when**:

- [ ] Test `self_assoc_resolves_through_repr_supertrait` passes in `pom_resolve.rs` (C26: STD-05 AC 1)
- [ ] Test `self_assoc_resolves_through_a_user_supertrait` passes in `pom_resolve.rs` (C27: STD-05 AC 2)
- [ ] Test `self_assoc_missing_everywhere_is_e0161` passes in `pom_resolve.rs` (C28: STD-05 AC 3)
- [ ] Test `resolution_doc_describes_the_supertrait_rule` passes in `pom_docs.rs` (C97: STD-17 AC 3)
- [ ] Resolution writes the slot only; no node is created (MD-54)
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): Self::NAME in a trait resolves through supertraits`

---

### T7: A carrier's method resolves through a blanket implementation

**What**: A fourth step of member resolution for a carrier receiver and for `self` in an `impl` of a nature: an `impl<T: B> Tr for T` whose `B` is absent or the core `Repr`, for a visible trait `Tr` that has the method, resolves the call to the trait's method and records the reference. The Members table of `docs/pom/resolution.md` gains the step.
**Where**: `crates/piperine-lang/src/pom/resolve/members.rs` (+ `docs/pom/resolution.md`, `tests/pom_resolve.rs`, `tests/pom_docs.rs`)
**Depends on**: T6
**Reuses**: the three existing steps of `members.rs`; the index writer that records method references
**Requirement**: STD-06, STD-17

**Done when**:

- [ ] Test `blanket_method_resolves_on_a_carrier` passes in `pom_resolve.rs` (C29: STD-06 AC 1)
- [ ] Test `blanket_method_resolves_on_self_in_a_nature_impl` passes in `pom_resolve.rs` (C30: STD-06 AC 2)
- [ ] Test `a_nature_impl_wins_over_a_blanket` passes in `pom_resolve.rs` (C31: STD-06 AC 3)
- [ ] Test `an_unlinked_member_keeps_its_slot` passes in `pom_resolve.rs` (C32: STD-06 AC 4)
- [ ] Test `a_blanket_call_is_a_reference_of_the_trait_method` passes in `pom_resolve.rs` (C33: STD-06 AC 5)
- [ ] Test `members_doc_lists_the_blanket_step` passes in `pom_docs.rs` (C98: STD-17 AC 4)
- [ ] A value receiver stays `Deferred(ReceiverTypeUnknown)`: no trait solving is added
- [ ] An `impl<T: Other> Tr for T` with a bound that is not `Repr` is not searched
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(pom): a carrier method resolves through a blanket impl bounded by Repr`

---

### T8: The excerpt guards exist and are honest on an empty library

**What**: New target `std_excerpts` with both guards and their controls. Forward: each `rust` fence of Part VII, normalized, is consecutive lines of the file its section maps to in the "Sources" table; it runs over the rows whose file exists. Reverse: each top-level item and behavior block of each `std` file but `lib.msl` and `prelude.msl` is a substring of a `rust` fence of `docs/spec/`. The helpers `section` and `table_rows` move from `spec_text.rs` to `tests/support/spec_tables.rs`, and `fragment_diagnostics` from `msl_corpus.rs` to `tests/support/spec_corpus.rs`, both `#[path]`-included by their old users.
**Where**: `crates/piperine-lang/tests/std_excerpts.rs` (+ `tests/support/spec_tables.rs`, `tests/support/spec_corpus.rs`, `tests/spec_text.rs`, `tests/msl_corpus.rs`)
**Depends on**: T7
**Reuses**: `spec_corpus::{corpus, extract, Class}`; the control pattern of `msl_corpus::every_compile_fail_block_diagnoses`
**Requirement**: STD-11, STD-12

**Done when**:

- [ ] Test `every_part_vii_fence_is_in_its_file` passes in `std_excerpts.rs` (C66: STD-11 AC 1)
- [ ] Test `a_missing_fence_is_named` passes in `std_excerpts.rs` (C69: STD-11 AC 4)
- [ ] Test `a_fence_one_character_off_is_not_found` passes in `std_excerpts.rs` (C70: STD-11 AC 5)
- [ ] Test `every_std_item_is_in_a_fence` passes in `std_excerpts.rs` (C71: STD-12 AC 1)
- [ ] Test `every_std_behavior_block_is_in_a_fence` passes in `std_excerpts.rs` (C72: STD-12 AC 2)
- [ ] Test `a_missing_item_is_named` passes in `std_excerpts.rs` (C74: STD-12 AC 4)
- [ ] The guards are plain functions returning a list of failures (spec line, first unmatched line; file, line, item); the tests assert the list is empty and the controls assert its content on text written in the test
- [ ] With the placeholder library both guards pass over zero rows and zero items; no floor is asserted yet (T17)
- [ ] `spec_text` and `msl_corpus` pass with the same number of tests as before
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `test(lang): excerpt guards between Part VII and std, with controls`

---

### T9: `std::constants`, `std::carriers`, the prelude of re-exports

**What**: `std/constants.msl` (the six `pub const`, `inf` as `1.0 / 0.0`), `std/carriers.msl` (`pub nature Bit<T: Repr = bool>`, `Electrical`, `Thermal`, `VSignal`, `pub module Short`), `std/lib.msl` declaring `constants`, `carriers`, `prelude`, and `std/prelude.msl` with one `pub use crate::<ns>::*;` each. `EMBEDDED` lists the four files. The fences of §1 (a new `rust` fence under the constants table), §4 and §11 show those items with `pub`, text equal to the files. New target `std_library`.
**Where**: `crates/piperine-lang/std/carriers.msl` (+ `std/constants.msl`, `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_i_values.md`, `docs/spec/part_ii_carriers.md`, `docs/spec/part_iv_behavior.md`, `tests/std_library.rs`, `tests/std_excerpts.rs`)
**Depends on**: T8
**Reuses**: `EMBEDDED` of `src/pom/stdlib.rs`; the §4 examples fence as the text of the natures; `pom_crates.rs` for the prelude tests' shape
**Requirement**: STD-07, STD-08, STD-09, STD-10, STD-12

**Done when**:

- [ ] Test `every_std_file_is_embedded` passes in `std_library.rs` (C38: STD-07 AC 5)
- [ ] Test `constants_have_the_values_of_s1` passes in `std_library.rs` (C45: STD-08 AC 7)
- [ ] Test `a_std_item_resolves_by_its_single_name` passes in `std_library.rs` (C52: STD-09 AC 1)
- [ ] Test `no_export_has_a_core_name` passes in `std_library.rs` (C56: STD-09 AC 5)
- [ ] Test `embedded_std_has_no_diagnostics` passes in `std_library.rs` (C61: STD-10 AC 1)
- [ ] Test `std_has_no_unresolved_member` passes in `std_library.rs` (C62: STD-10 AC 2)
- [ ] Test `embedded_std_passes_check` passes in `std_library.rs` (C63: STD-10 AC 3)
- [ ] Test `a_replaced_std_without_an_item_is_e0161` passes in `std_library.rs` (C64: STD-10 AC 4)
- [ ] Test `prelude_items_outside_part_vii_are_shown_pub` passes in `std_excerpts.rs` (C73: STD-12 AC 3)
- [ ] The built-in `Const` rows stay until T20: a prelude item is found before a core name (§3.5 rule 4), so `K_B` already resolves to `std::constants`
- [ ] If the prelude step does not see a `pub use` glob, extending it to imports is part of this task (§3.4.2: the prelude "re-exports"); items are never declared in `prelude.msl`
- [ ] A two-file test proves a `std` file sees another namespace's item through the prelude without `E0162`
- [ ] `pom_crates::embedded_std_builds_clean_and_has_a_prelude` still passes
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): constants, carriers and the prelude of re-exports`

---

### T10: `std::values` (§22.1)

**What**: `std/values.msl`: `Known` and its blanket `impl`, `merge`, `cast`, `same` (body of `design.md`), `matches_bits` and `logic` as `extern "intrinsic"`, `impl Bit` (`rise`, `fall`, `high`), `init`, `every`, `since` with their `digital` blocks. `lib.msl` and `prelude.msl` gain the namespace, `EMBEDDED` the file, the "Sources" row is live. The fences of §22.1 become excerpts.
**Where**: `crates/piperine-lang/std/values.msl` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_vii_stdlib.md`, `tests/std_library.rs`)
**Depends on**: T9
**Reuses**: the fences of §22.1 as the text; T5 to T7 for the forms they need
**Requirement**: STD-09

**Done when**:

- [ ] Test `bit_rise_resolves_into_std_values` passes in `std_library.rs` (C57: STD-09 AC 6)
- [ ] Test `known_resolves_into_std_values` passes in `std_library.rs` (C58: STD-09 AC 7)
- [ ] `std_excerpts` passes with §22.1 covered in both directions
- [ ] `std_library::embedded_std_has_no_diagnostics` and `std_has_no_unresolved_member` pass with this file: `self.known_or(false)` in `impl Bit` and `t.known_or(-inf)` in `since` are linked
- [ ] A body the parser or the POM rejects is a stop: report, do not turn it into an intrinsic
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): values, Known, Bit edges, init, every, since`

---

### T11: `std::analog`, part one (§22.2.1 to §22.2.11, §22.2.15 to §22.2.17)

**What**: `std/analog.msl`: `State`, `vt`, `white_noise`, `flicker_noise`, `noise_table` (body), `limexp`, `pnjlim`, `fetlim`, `limvds` (transliterated from `crates/piperine-ir/src/limit.rs`), `deltalim`, `above`, `absdelta`, `last_crossing`, `idt`, `idtmod`, `Slewing` and `slew`, `transition`, `table_model` (`extern "intrinsic"`), `db10`, `db20`, `uniform` and `normal` (SplitMix64 and Box-Muller), with their behavior blocks. The fences of those entries become excerpts; the `Pi` and `MeasuredDiode` examples become `rust,fragment`; the Resolutions that say "signature inferred" are reworded to describe the body; G.64 names the generator.
**Where**: `crates/piperine-lang/std/analog.msl` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_vii_stdlib.md`, `docs/spec/appendix_g_scope.md`, `tests/std_library.rs`, `tests/spec_text.rs`)
**Depends on**: T10
**Reuses**: `design.md`, "Bodies written by this feature"; `crates/piperine-ir/src/limit.rs`; the fences of §22.2
**Requirement**: STD-02, STD-08, STD-09

**Done when**:

- [ ] Test `g64_names_the_normal_generator` passes in `spec_text.rs` (C19: STD-02 AC 9)
- [ ] Test `noise_table_is_noise_of_table_model` passes in `std_library.rs` (C49: STD-08 AC 11)
- [ ] Test `pure_bodies_name_no_primitive` passes in `std_library.rs` (C50: STD-08 AC 12)
- [ ] Test `a_private_std_item_is_e0161` passes in `std_library.rs` (C55: STD-09 AC 4)
- [ ] The three limiters match `limit.rs` branch by branch; the commit body says so
- [ ] `std_excerpts` and the two cleanliness tests of `std_library` pass with this file
- [ ] `pure_bodies_name_no_primitive` covers `same` (T10) and the seven functions of this task
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): analog functions and modules, limiters and normal written in MSL`

---

### T12: `std::analog`, part two: `zi_*` and `laplace_*` (§22.2.12 to §22.2.14)

**What**: `zi_nd` (Part VII's text, `analysis(Transient)`), the private `conjugate_closed`, `poly_q`, `den_q`, `poly_s`, the modules `zi_zp`, `zi_zd`, `zi_np`, `laplace_nd` with its `analog` block, `laplace_zp`, `laplace_zd`, `laplace_np`, as in `design.md`, placed in section order. §22.2.13 and §22.2.14: the fences become excerpts, "heads and params only" goes, the rules give `E0317` for a broken root list or a zero denominator and `E0304` for `D == 0` or `N > D`; the `E0707` row is removed from the error table.
**Where**: `crates/piperine-lang/std/analog.msl` (+ `docs/spec/part_vii_stdlib.md`, `tests/std_library.rs`, `tests/spec_text.rs`)
**Depends on**: T11
**Reuses**: `design.md`, "Bodies written by this feature"; `zi_nd`'s own text for the loop and param-block forms
**Requirement**: STD-02, STD-08

**Done when**:

- [ ] Test `e0707_is_replaced_by_e0317` passes in `spec_text.rs` (C17: STD-02 AC 7)
- [ ] Test `laplace_states_the_proper_rule` passes in `spec_text.rs` (C18: STD-02 AC 8)
- [ ] Test `root_forms_place_their_coefficient_form` passes in `std_library.rs` (C46: STD-08 AC 8)
- [ ] Test `root_params_carry_a_where` passes in `std_library.rs` (C47: STD-08 AC 9)
- [ ] Test `laplace_nd_has_state_signals_and_an_analog_block` passes in `std_library.rs` (C48: STD-08 AC 10)
- [ ] `std_excerpts` and the two cleanliness tests of `std_library` pass with the file complete
- [ ] No other part of `docs/spec/` still cites `E0707` (`every_error_code_is_defined_exactly_once` passes)
- [ ] A body the parser or the POM rejects is a stop: report the form and the diagnostic, do not turn the module into an intrinsic or back into a head
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): zi and laplace filters written in MSL; E0707 becomes a where`

---

### T13: `std::nets` (§22.3)

**What**: `std/nets.msl`: `Strength`, `Logic` and its two `impl` blocks, `Wire`, `tri`, `Tri`, `sum`, `SumNet`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, each `pub`, with their `digital` blocks. The fences of §22.3 become excerpts (with `pub`).
**Where**: `crates/piperine-lang/std/nets.msl` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_vii_stdlib.md`, `docs/spec/part_ii_carriers.md`, `tests/std_library.rs`)
**Depends on**: T12
**Reuses**: the fences of §22.3; the `Wire` line of the §4 examples fence (it shows `pub nature Wire` now)
**Requirement**: STD-08, STD-09, STD-10

**Done when**:

- [ ] Test `tgate_places_two_tran` passes in `std_library.rs` (C51: STD-08 AC 13)
- [ ] Test `a_std_path_names_the_same_declaration` passes in `std_library.rs` (C54: STD-09 AC 3)
- [ ] Test `transient_is_the_kind_and_tran_the_module` passes in `std_library.rs` (C65: STD-10 AC 5)
- [ ] `no_export_has_a_core_name` passes with `Tran` exported: the built-in row is `Transient` since T3
- [ ] `std_excerpts` and the two cleanliness tests of `std_library` pass with this file
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): resolved nets, Logic, Wire, Tran and Tgate`

---

### T14: `std::convert` (§22.4)

**What**: `std/convert.msl`: `D2A`, `A2D`, `Bidir`, each `pub`, with their `analog` and `digital` blocks. The fences of §22.4 become excerpts.
**Where**: `crates/piperine-lang/std/convert.msl` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_vii_stdlib.md`)
**Depends on**: T13
**Reuses**: the fences of §22.4
**Requirement**: STD-09

**Done when**:

- [ ] Test `converters_resolve_into_std_convert` passes in `std_library.rs` (C59: STD-09 AC 8)
- [ ] `std_excerpts` passes with §22.4 covered in both directions (C-level proof: the guards of T8)
- [ ] `std_library::embedded_std_has_no_diagnostics` and `std_has_no_unresolved_member` pass with this file
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): converters D2A, A2D and Bidir`

---

### T15: `std::monitor` (§22.5)

**What**: `std/monitor.msl`: `AckWithin`, `Never`, `Stable`, `Setup`, `Hold`, `MinPulse`, `inertial`, `TribufDelayed`, each `pub`, with their `digital` blocks. The fences of §22.5 become excerpts.
**Where**: `crates/piperine-lang/std/monitor.msl` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_vii_stdlib.md`)
**Depends on**: T14
**Reuses**: the fences of §22.5
**Requirement**: STD-09

**Done when**:

- [ ] Test `monitors_resolve_into_std_monitor` passes in `std_library.rs` (C60: STD-09 AC 9)
- [ ] `std_excerpts` passes with §22.5 covered in both directions
- [ ] `std_library::embedded_std_has_no_diagnostics` and `std_has_no_unresolved_member` pass with this file
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): monitors and inertial delay`

---

### T16: `std::io` (§22.6)

**What**: `std/io.msl`: `Severity`, `log`, `finish`, `fatal`, `require`, `cover`, `target`, and `parse_table`, `parse_hex` as `pub const extern "intrinsic" fn`. The fences of §22.6 become excerpts; the `IV`/`ROM` example becomes `rust,fragment`.
**Where**: `crates/piperine-lang/std/io.msl` (+ `std/lib.msl`, `std/prelude.msl`, `src/pom/stdlib.rs`, `docs/spec/part_vii_stdlib.md`, `tests/std_excerpts.rs`)
**Depends on**: T15
**Reuses**: the fences of §22.6
**Requirement**: STD-11

**Done when**:

- [ ] Test `part_vii_examples_are_fragments` passes in `std_excerpts.rs` (C67: STD-11 AC 2)
- [ ] `std_excerpts` passes with all six sections covered
- [ ] `std_library::embedded_std_has_no_diagnostics` and `std_has_no_unresolved_member` pass with this file
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(std): output functions and the two text parsers as intrinsics`

---

### T17: The library is complete: layout, Index, intrinsic list, floors

**What**: The tests that can only pass with every file present: the root declares the nine namespaces and nothing else; each namespace holds its section; the prelude is eight re-exports; the Index and the prelude's exports are equal sets; every function but the five intrinsics has a body and every module with a result type a result expression; the intrinsic set equals the "Implemented by the engine" table and each member takes a `&str`; the floors of both excerpt guards. The §22 Index is corrected where it lags the library (rows for the helper-free exports, `Tran`). The file comment of `std/lib.msl` names the namespaces.
**Where**: `crates/piperine-lang/tests/std_library.rs` (+ `tests/std_excerpts.rs`, `std/lib.msl`, `docs/spec/part_vii_stdlib.md`)
**Depends on**: T16
**Reuses**: `spec_tables::{section, table_rows}` to read the Index and the engine table; reflection over the embedded design
**Requirement**: STD-07, STD-08, STD-09, STD-11, STD-12, STD-17

**Done when**:

- [ ] Test `std_root_declares_nine_namespaces` passes in `std_library.rs` (C34: STD-07 AC 1)
- [ ] Test `each_namespace_holds_its_section` passes in `std_library.rs` (C35: STD-07 AC 2)
- [ ] Test `prelude_is_eight_reexports` passes in `std_library.rs` (C36: STD-07 AC 3)
- [ ] Test `embedded_list_names_every_file_once` passes in `std_library.rs` (C37: STD-07 AC 4)
- [ ] Test `every_index_item_is_declared` passes in `std_library.rs` (C39: STD-08 AC 1)
- [ ] Test `every_item_but_the_intrinsics_has_a_body` passes in `std_library.rs` (C40: STD-08 AC 2)
- [ ] Test `intrinsics_are_exactly_the_engine_table` passes in `std_library.rs` (C41: STD-08 AC 3)
- [ ] Test `every_intrinsic_reads_a_str` passes in `std_library.rs` (C42: STD-08 AC 4)
- [ ] Test `every_index_item_is_pub` passes in `std_library.rs` (C43: STD-08 AC 5)
- [ ] Test `std_declares_no_core_name` passes in `std_library.rs` (C44: STD-08 AC 6)
- [ ] Test `prelude_exports_equal_the_index` passes in `std_library.rs` (C53: STD-09 AC 2)
- [ ] Test `the_forward_guard_covers_part_vii` passes in `std_excerpts.rs` (C68: STD-11 AC 3)
- [ ] Test `the_reverse_guard_covers_std` passes in `std_excerpts.rs` (C75: STD-12 AC 5)
- [ ] Test `std_root_comment_names_the_namespaces` passes in `std_library.rs` (C100: STD-17 AC 6)
- [ ] No `std` item is added or changed in this task but the comment of `lib.msl`; a missing item found here is fixed in the file and fence of its section, in this commit, and named in the commit body
- [ ] The Index and the library agree in both directions; a name in one and not the other fails `prelude_exports_equal_the_index`
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `test(std): the library is complete: layout, index, intrinsic list and guard floors`

---

### T18: The gallery fixtures

**What**: `tests/fixtures/gallery/<Model>.msl` for the twelve models, each line taken from `docs/spec/` (a `rust` fence, or the code span of Appendix D's table for the declarations the galleries only assume). New target `std_galleries` with the fixture checks and the table (gallery section, fixtures appended, `std` items it must reference), filled from each fence's own text.
**Where**: `crates/piperine-lang/tests/std_galleries.rs` (+ `tests/fixtures/gallery/*.msl`)
**Depends on**: T17
**Reuses**: `spec_corpus::corpus`; `design.md`, "Gallery gate" for the table's shape
**Requirement**: STD-13

**Done when**:

- [ ] Test `gallery_fixtures_exist` passes in `std_galleries.rs` (C76: STD-13 AC 1)
- [ ] Test `every_fixture_line_is_spec_text` passes in `std_galleries.rs` (C77: STD-13 AC 2)
- [ ] Test `the_fixture_table_covers_every_gallery` passes in `std_galleries.rs` (C78: STD-13 AC 3)
- [ ] Test `the_fixture_table_names_only_used_models` passes in `std_galleries.rs` (C79: STD-13 AC 4)
- [ ] A fixture is never listed for the fence that defines the same model (`Vco` in D.3, `RSwitch` in D.6)
- [ ] The table is data in the test: plain arrays, no macro
- [ ] Gate passes: the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `test(lang): gallery fixtures, each line text of the specification`

---

### T19: The galleries build clean; G.38 is closed

**What**: The gate: each of the 18 gallery fences, with its fixtures appended, built against the embedded library, is clean in the four senses of `design.md`; the control builds the same sources against an empty prelude. G.38 becomes `Closed` and names `std_excerpts` and `std_galleries`; the line of Appendix C says the library and the galleries are parsed and resolved and nothing is executed.
**Where**: `crates/piperine-lang/tests/std_galleries.rs` (+ `docs/spec/appendix_g_scope.md`, `docs/spec/appendix_c_implementation.md`, `tests/spec_text.rs`)
**Depends on**: T18
**Reuses**: T18's table and assembly; `Decl::references()`, `Crate::is_std()`, `Design::check()`
**Requirement**: STD-14, STD-01

**Done when**:

- [ ] Test `g38_is_closed` passes in `spec_text.rs` (C4: STD-01 AC 4)
- [ ] Test `appendix_c_says_parsed_and_resolved` passes in `spec_text.rs` (C5: STD-01 AC 5)
- [ ] Test `every_gallery_builds_with_no_diagnostics` passes in `std_galleries.rs` (C80: STD-14 AC 1)
- [ ] Test `every_gallery_passes_check` passes in `std_galleries.rs` (C81: STD-14 AC 2)
- [ ] Test `no_gallery_has_an_unresolved_member` passes in `std_galleries.rs` (C82: STD-14 AC 3)
- [ ] Test `every_gallery_references_its_std_items` passes in `std_galleries.rs` (C83: STD-14 AC 4)
- [ ] Test `the_gate_builds_18_fences` passes in `std_galleries.rs` (C84: STD-14 AC 5)
- [ ] Test `a_dirty_gallery_is_reported_by_fence` passes in `std_galleries.rs` (C85: STD-14 AC 6)
- [ ] Test `galleries_fail_against_an_empty_prelude` passes in `std_galleries.rs` (C86: STD-14 AC 7)
- [ ] Test `every_gallery_lists_a_std_item` passes in `std_galleries.rs` (C87: STD-14 AC 8)
- [ ] A gallery that is not clean is a finding about `std`, the POM or the gallery: fix it where it is wrong and say which in the commit body; the gate's assertions are not loosened
- [ ] A gallery fence is edited only for the rename of T3 or a defect the gate proves; each edit is listed in the commit body
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `test(lang): the galleries build clean against std; G.38 closed`

---

### T20: The built-in table holds core names only

**What**: Remove the six `Const` rows and `BuiltinKind::Const`. `docs/pom/resolution.md` and `docs/pom/model.md` follow: the table without the row, a sentence that the constants are items of `std::constants`.
**Where**: `crates/piperine-lang/src/pom/builtin.rs` (+ `docs/pom/resolution.md`, `docs/pom/model.md`, `tests/pom_resolve.rs`, `tests/pom_docs.rs`)
**Depends on**: T19
**Reuses**: `BUILTINS`; the family-row constants `UINT_ROW`, `INT_ROW`, `FLOAT_ROW` (their indexes do not move: the removed rows are after them)
**Requirement**: STD-15, STD-17
**Existing tests**: decision Q5 moves these: `pom_resolve.rs:20` (the loop over `Q` .. `inf` as built-ins) asserts the six resolve to declarations of `std::constants`; `pom_resolve.rs:311` drops `K_B` from its list of built-in sections; the doctest of `docs/pom/resolution.md` keeps its assertions (none names a constant)

**Done when**:

- [ ] Test `the_constants_are_not_builtin` passes in `pom_resolve.rs` (C88: STD-15 AC 1)
- [ ] Test `k_b_resolves_to_std_constants` passes in `pom_resolve.rs` (C89: STD-15 AC 2)
- [ ] Test `the_core_rows_are_unchanged` passes in `pom_resolve.rs` (C91: STD-15 AC 4)
- [ ] Test `a_model_constant_hides_the_prelude_constant` passes in `pom_resolve.rs` (C92: STD-15 AC 5)
- [ ] Test `the_builtin_table_doc_has_no_const_row` passes in `pom_docs.rs` (C95: STD-17 AC 1)
- [ ] Every match on `BuiltinKind` in `src/pom/` compiles without the variant; no `_ =>` arm is added to hide it
- [ ] A test that used a constant with a replaced library declares it in its own `std_source`
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `refactor(pom): the prelude constants are std items, not built-in rows`

---

### T21: The fixture prelude is retired

**What**: Delete `tests/fixtures/pom/prelude.msl`. `pom_classify`, `pom_reflect` and `pom_resolve` build against the embedded library where they used the fixture; a test that proves a rule of the prelude itself keeps a library of its own, written in the test with `std_source`. The examples of `docs/pom/` use the embedded library where they declared stand-ins of `std` items.
**Where**: `crates/piperine-lang/tests/pom_resolve.rs` (+ `tests/pom_classify.rs`, `tests/pom_reflect.rs`, `tests/fixtures/pom/prelude.msl`, `docs/pom/*.md`, `tests/std_library.rs`)
**Depends on**: T20
**Reuses**: the `std_lib` helpers of the three targets (kept for tests with their own library)
**Requirement**: STD-16, STD-17
**Existing tests**: decision D4 (A-42) moves the assertions that read a stand-in of the fixture: the three stand-ins `State`, `Logic`, `init`, and any count of prelude files or items. Each changes to the real item's value; nothing else in those tests changes

**Done when**:

- [ ] Test `the_fixture_prelude_is_gone` passes in `std_library.rs` (C93: STD-16 AC 1)
- [ ] Test `no_pom_test_was_removed` passes in `std_library.rs` (C94: STD-16 AC 2)
- [ ] `cargo test -p piperine-lang --doc` passes (C99: STD-17 AC 5)
- [ ] No test is deleted; one that asserted a property of a stand-in (`init` as `settle`, `Logic` as a four-variant enum, `State` with `lvl`) asserts the same property of the real item, and the commit body lists each
- [ ] `cargo test -p piperine-lang --doc` passes
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `test(lang): pom tests build against the real std; fixture prelude deleted`

---

### T22: The Book's source lives in `docs/book/` and every `msl` block parses

**What**: Copy `content.html`, `template.html` and `build.py` from the session scratchpad (path in `design.md`) to `docs/book/`. `build.py` writes `docs/book/index.html` and knows `Transient`. `.gitignore` ignores the output; `mkdocs.yml` excludes `docs/book/`. New target `book`: the three files exist with the twelve chapters, the build runs, there are at least 40 `msl` blocks and each parses as a file or a fragment.
**Where**: `docs/book/content.html` (+ `docs/book/template.html`, `docs/book/build.py`, `.gitignore`, `mkdocs.yml`, `crates/piperine-lang/tests/book.rs`)
**Depends on**: T21
**Reuses**: `spec_corpus::fragment_diagnostics` (moved in T8); plain string functions for sections, blocks and code spans
**Requirement**: STD-18

**Done when**:

- [ ] Test `the_book_source_is_in_docs_book` passes in `book.rs` (C101: STD-18 AC 1)
- [ ] Test `build_py_writes_an_ignored_index` passes in `book.rs` (C102: STD-18 AC 2)
- [ ] Test `every_msl_block_parses` passes in `book.rs` (C109: STD-18 AC 9)
- [ ] Test `the_book_has_at_least_40_msl_blocks` passes in `book.rs` (C111: STD-18 AC 11)
- [ ] If the scratchpad files are gone, stop and ask the orchestrator for them; the Book is not rewritten from memory
- [ ] A block that is wrong MSL is fixed in place; a block that illustrates an error is marked `data-lang="msl-error"` and the check requires at least one diagnostic from it; each edit is listed in the commit body
- [ ] The test runs `python3 docs/book/build.py` and fails loud when Python is missing; it never skips
- [ ] Gate passes: the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(book): the MSL Book's source moves into the repository, with a parse check`

---

### T23: The chapter "Core and standard library"

**What**: Chapter 10 of the Book, id `core-std`, after `packages`, with the six parts of `design.md`: the idea; core (the core names, the §14 primitives by kind, the `!` forms); `std::prelude`; one part per namespace with one sentence per item and one example; "implemented by the engine"; replacing a library item. The later chapters are renumbered 11 to 13.
**Where**: `docs/book/content.html` (+ `crates/piperine-lang/tests/book.rs`)
**Depends on**: T22
**Reuses**: the Book's own chapter markup (`<section id=".." class="chapter">`, `<p class="ch-num">`, `<pre data-lang="msl">`); `BUILTINS` and the embedded design as the lists the check reads
**Requirement**: STD-18

**Done when**:

- [ ] Test `the_core_std_chapter_follows_packages` passes in `book.rs` (C103: STD-18 AC 3)
- [ ] Test `the_chapter_names_every_core_name` passes in `book.rs` (C104: STD-18 AC 4)
- [ ] Test `the_chapter_has_a_part_per_namespace` passes in `book.rs` (C105: STD-18 AC 5)
- [ ] Test `the_chapter_names_every_prelude_export` passes in `book.rs` (C106: STD-18 AC 6)
- [ ] Test `the_chapter_explains_the_prelude_and_the_engine_items` passes in `book.rs` (C107: STD-18 AC 7)
- [ ] Test `chapter_blocks_build_against_std` passes in `book.rs` (C108: STD-18 AC 8)
- [ ] Test `a_missing_name_is_listed` passes in `book.rs` (C112: STD-18 AC 12)
- [ ] The lists of names come from the code (`BUILTINS`, the embedded design), never from an array in the test
- [ ] Plain language: each item gets what it is for, not its signature; the specification stays the reference and the chapter links to Part VII by section number
- [ ] Every `msl` block of the chapter is a whole file that builds against the embedded library
- [ ] Gate passes: the task's `Run` line(s), then `cargo test -p piperine-lang --test msl_corpus --test pom_corpus --test spec_text`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(book): chapter 10, core and standard library`

---

### T24: The packages note and the cheat sheet follow the library

**What**: Chapter 9 (`packages`) says `std` is a dependency of every crate and that `std::prelude` is implied in every file. The cheat sheet gains a block "where names come from" with the eight `std::<name>` namespaces and the core names. Every `analysis(Tran)` of the Book becomes `analysis(Transient)`.
**Where**: `docs/book/content.html` (+ `crates/piperine-lang/tests/book.rs`)
**Depends on**: T23
**Reuses**: T23's chapter as the place the cheat sheet links to
**Requirement**: STD-18

**Done when**:

- [ ] Test `packages_and_cheatsheet_name_std` passes in `book.rs` (C110: STD-18 AC 10)
- [ ] Test `the_book_says_transient` passes in `book.rs` (C113: STD-18 AC 13)
- [ ] `python3 docs/book/build.py` runs and the output holds 13 chapters in its navigation
- [ ] Publishing the artifact is not part of this task: the orchestrator does it after the merge
- [ ] Gate passes: `cargo test -p piperine-lang`; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `docs(book): packages note and cheat sheet name std and the prelude`

---

## Checks

One check per acceptance criterion of `spec.md`. Each `run` proof names the line its test prints; the one lone proof is judged by its exit code.

**C1** - The section §22 state that the standard library is the crate `std`, that its source files are normative, and that each `rust` fence of Part VII is an excerpt of one of them. (STD-01, AC 1)
Proof: run spec | test s22_says_std_is_the_normative_source ... ok

**C2** - The section §22 hold a table "Sources" that maps each of §22.1 to §22.6 to one namespace of `std`, and one row each for `std::constants` (§1) and `std::carriers` (§4, §11). (STD-01, AC 2)
Proof: run spec | test s22_maps_sections_to_namespaces ... ok

**C3** - The section §22 does not contain the Resolution that reads listings without `pub` "as if written with `pub`". (STD-01, AC 3)
Proof: run spec | test s22_drops_the_implied_pub_resolution ... ok

**C4** - The entry G.38 of Appendix G have the status `Closed` and name the two excerpt guards and the gallery gate. (STD-01, AC 4)
Proof: run spec | test g38_is_closed ... ok

**C5** - The line of Appendix C that says no part of the specification has been compiled say instead that the library and the galleries are parsed and resolved, and that nothing is executed. (STD-01, AC 5)
Proof: run spec | test appendix_c_says_parsed_and_resolved ... ok

**C6** - The table "Prelude constants" of §1 say that the six constants are `pub const` items of `std::constants`. (STD-01, AC 6)
Proof: run spec | test s1_constants_are_std_items ... ok

**C7** - The lists of analysis kinds in §11.1.1 and in row 23 of Appendix B.1 be `Dc`, `Transient`, `Ac`, `Noise`, `Static`. (STD-01, AC 7)
Proof: run spec | test analysis_kinds_list_transient ... ok

**C8** - No file of `docs/spec/` contain the text `analysis(Tran)`. (STD-01, AC 8)
Proof: run spec | test no_analysis_tran_remains ... ok

**C9** - WHEN the token `Tran` occurs in a `rust` or `rust,fragment` fence of `docs/spec/` THEN the next character be `(`. (STD-01, AC 9)
Proof: run spec | test tran_in_fences_is_the_module ... ok

**C10** - Appendix G record the five decisions Q1 to Q5 in its decision table, as S1 to S5. (STD-01, AC 10)
Proof: run spec | test appendix_g_records_the_stdlib_decisions ... ok

**C11** - §3 rule 10 list two ABI strings for a function, `"C"` and `"intrinsic"`, and say that `"intrinsic"` is accepted only in the crate `std`. (STD-02, AC 1)
Proof: run spec | test s3_lists_the_intrinsic_abi ... ok

**C12** - §3 state that an `extern "intrinsic" fn` has no body, is not restricted to scalar types, takes no `#[derivative]`, and has the contract its entry in Part VII gives. (STD-02, AC 2)
Proof: run spec | test s3_states_the_intrinsic_contract ... ok

**C13** - The `E0125` row of §3 include the condition "the ABI `"intrinsic"` outside the crate `std`". (STD-02, AC 3)
Proof: run spec | test e0125_covers_intrinsic_outside_std ... ok

**C14** - §22 hold a table "Implemented by the engine" whose rows are exactly `matches_bits`, `logic`, `table_model`, `parse_table` and `parse_hex`, each with its entry and a reason. (STD-02, AC 4)
Proof: run spec | test s22_lists_the_engine_items ... ok

**C15** - §22 rule 4 say that an item given as a signature and a contract is declared `extern "intrinsic"`. (STD-02, AC 5)
Proof: run spec | test s22_rule_4_names_extern_intrinsic ... ok

**C16** - The count of constructs in Appendix B.1 stay 65. (STD-02, AC 6)
Proof: run spec | test b1_still_counts_65_constructs ... ok

**C17** - The error table of Part VII have no `E0707` row, and §22.2.13 and §22.2.14 give `E0317` for a complex root without its conjugate and for a zero denominator. (STD-02, AC 7)
Proof: run spec | test e0707_is_replaced_by_e0317 ... ok

**C18** - §22.2.14 state that `laplace_nd` requires `D >= 1` and `N <= D`, and that a placement that breaks it is `E0304`. (STD-02, AC 8)
Proof: run spec | test laplace_states_the_proper_rule ... ok

**C19** - The entry G.64 of Appendix G say that the generator of `normal` is the one written in `std::analog`. (STD-02, AC 9)
Proof: run spec | test g64_names_the_normal_generator ... ok

**C20** - WHEN a file of the crate `std` declares `extern "intrinsic" fn f(..) -> T;` THEN the POM report no diagnostic for it. (STD-04, AC 1)
Proof: run resolve | test intrinsic_fn_is_accepted_in_std ... ok

**C21** - IF a file of any other crate declares an `extern "intrinsic" fn` THEN the POM report `E0125` at the function's name. (STD-04, AC 2)
Proof: run resolve | test intrinsic_fn_outside_std_is_e0125 ... ok

**C22** - IF an `extern "intrinsic" fn` has a body THEN the POM report `E0125` at the function's name. (STD-04, AC 3)
Proof: run resolve | test intrinsic_fn_with_a_body_is_e0125 ... ok

**C23** - The handle `Function` answer `is_intrinsic()`, true exactly when the ABI string is `intrinsic`. (STD-04, AC 4)
Proof: run resolve | test is_intrinsic_reads_the_abi ... ok

**C24** - WHEN a file of any crate declares `extern "C" fn g(x: f64) -> f64;` THEN the POM report no diagnostic and `is_intrinsic()` be false. (STD-04, AC 5)
Proof: run resolve | test extern_c_fn_is_not_intrinsic ... ok

**C25** - WHEN a model calls an intrinsic function of `std` THEN the POM classify the call `CallClass::Function` and resolve the callee to that declaration. (STD-04, AC 6)
Proof: run resolve | test a_call_to_an_intrinsic_resolves ... ok

**C26** - WHEN a trait declared `trait Known: Repr` writes `Self::WIDTH` in a method body THEN the POM resolve it to the built-in member `Repr::WIDTH` and report no diagnostic. (STD-05, AC 1)
Proof: run resolve | test self_assoc_resolves_through_repr_supertrait ... ok

**C27** - WHEN a trait with a user supertrait writes `Self::K`, where the supertrait declares `const K`, THEN the POM resolve it to that item. (STD-05, AC 2)
Proof: run resolve | test self_assoc_resolves_through_a_user_supertrait ... ok

**C28** - IF `Self::NAME` in a trait names no item of the trait or of any supertrait THEN the POM report `E0161` at the path. (STD-05, AC 3)
Proof: run resolve | test self_assoc_missing_everywhere_is_e0161 ... ok

**C29** - WHEN a method is called on a carrier, no carrier built-in and no `impl` of the nature or of its value type has that name, and a visible trait has it and has an implementation `impl<T: Repr> Tr for T`, THEN the POM resolve the call to the trait's method. (STD-06, AC 1)
Proof: run resolve | test blanket_method_resolves_on_a_carrier ... ok

**C30** - WHEN `self.known()` is called inside an `impl` of a nature THEN the POM resolve it to `Known::known` by the same rule. (STD-06, AC 2)
Proof: run resolve | test blanket_method_resolves_on_self_in_a_nature_impl ... ok

**C31** - WHEN a nature's own `impl` and a blanket implementation both have the method name THEN the POM resolve the call to the nature's function. (STD-06, AC 3)
Proof: run resolve | test a_nature_impl_wins_over_a_blanket ... ok

**C32** - IF no rule of the member table links the method THEN the slot stay `Unresolved` for a known receiver and `Deferred` otherwise, with no diagnostic. (STD-06, AC 4)
Proof: run resolve | test an_unlinked_member_keeps_its_slot ... ok

**C33** - WHEN a `known`, `known_or` or `is_known` call is linked THEN the trait's method list the call in `references()`. (STD-06, AC 5)
Proof: run resolve | test a_blanket_call_is_a_reference_of_the_trait_method ... ok

**C34** - The root `std/lib.msl` declare these `pub` namespaces and no other item: `constants`, `carriers`, `values`, `analog`, `nets`, `convert`, `monitor`, `io`, `prelude`. (STD-07, AC 1)
Proof: run std | test std_root_declares_nine_namespaces ... ok

**C35** - The file of each namespace hold the items of its section: `constants` §1 rule 3; `carriers` the natures `Bit`, `Electrical`, `Thermal`, `VSignal` (§4) and the module `Short` (§11); `values` §22.1; `analog` §22.2; `nets` §22.3; `convert` §22.4; `monitor` §22.5; `io` §22.6. (STD-07, AC 2)
Proof: run std | test each_namespace_holds_its_section ... ok

**C36** - The file `std/prelude.msl` hold one `pub use crate::<namespace>::*;` per namespace of rule 1 other than `prelude`, and no other item. (STD-07, AC 3)
Proof: run std | test prelude_is_eight_reexports ... ok

**C37** - The embedded list of `std` files name the root first and every other file once. (STD-07, AC 4)
Proof: run std | test embedded_list_names_every_file_once ... ok

**C38** - WHEN a `.msl` file exists under `crates/piperine-lang/std/` that the embedded list does not name THEN a test fail and name the file. (STD-07, AC 5)
Proof: run std | test every_std_file_is_embedded ... ok

**C39** - WHEN the §22 Index names an item THEN the crate `std` declare it in the namespace of its entry's section. (STD-08, AC 1)
Proof: run std | test every_index_item_is_declared ... ok

**C40** - WHEN a function of `std` is not in the table "Implemented by the engine" THEN it have a body, and every module of `std` with a result type have a result expression. (STD-08, AC 2)
Proof: run std | test every_item_but_the_intrinsics_has_a_body ... ok

**C41** - The set of functions of `std` for which `is_intrinsic()` is true equal the rows of the table "Implemented by the engine" of §22. (STD-08, AC 3)
Proof: run std | test intrinsics_are_exactly_the_engine_table ... ok

**C42** - WHEN a function of `std` is intrinsic THEN it have a parameter of type `&str`. (STD-08, AC 4)
Proof: run std | test every_intrinsic_reads_a_str ... ok

**C43** - WHEN the §22 Index names an item THEN its declaration in `std` carry `pub`. (STD-08, AC 5)
Proof: run std | test every_index_item_is_pub ... ok

**C44** - The crate `std` declare no item named like a core name of §14 rule 1. (STD-08, AC 6)
Proof: run std | test std_declares_no_core_name ... ok

**C45** - The six constants of `std::constants` have the values of the table of §1: `Q` 1.602176634e-19, `K_B` 1.380649e-23, `EPS0` 8.8541878128e-12, `PI` and `E` equal to Rust's `f64` constants, `inf` written `1.0 / 0.0`. (STD-08, AC 7)
Proof: run std | test constants_have_the_values_of_s1 ... ok

**C46** - The modules `zi_np`, `zi_zd` and `zi_zp` each place exactly one `zi_nd`, and `laplace_np`, `laplace_zd` and `laplace_zp` exactly one `laplace_nd`. (STD-08, AC 8)
Proof: run std | test root_forms_place_their_coefficient_form ... ok

**C47** - WHEN a param of a `zi_*` or `laplace_*` module is named `zeros` or `poles` THEN it carry a `where` condition. (STD-08, AC 9)
Proof: run std | test root_params_carry_a_where ... ok

**C48** - The module `laplace_nd` declare a signal that is an array of `State` and have one `analog` block. (STD-08, AC 10)
Proof: run std | test laplace_nd_has_state_signals_and_an_analog_block ... ok

**C49** - The module `noise_table` place `noise` and call `table_model`. (STD-08, AC 11)
Proof: run std | test noise_table_is_noise_of_table_model ... ok

**C50** - The bodies of `same`, `deltalim`, `pnjlim`, `fetlim`, `limvds`, `db10`, `db20` and `normal` name no core primitive of §14 other than `level`. (STD-08, AC 12)
Proof: run std | test pure_bodies_name_no_primitive ... ok

**C51** - The module of §22.3.7 be declared `Tran`, and `Tgate` place two `Tran`. (STD-08, AC 13)
Proof: run std | test tgate_places_two_tran ... ok

**C52** - WHEN a file of any crate uses by its single name a `pub` item of a namespace of `std` THEN the POM resolve it to that item through `std::prelude`. (STD-09, AC 1)
Proof: run std | test a_std_item_resolves_by_its_single_name ... ok

**C53** - The set of names `std::prelude` exports equal the names of the first column of the §22 Index, plus `Bit`, `Electrical`, `Thermal`, `VSignal`, `Short` and the six constants. (STD-09, AC 2)
Proof: run std | test prelude_exports_equal_the_index ... ok

**C54** - WHEN a path is written `std::nets::Wire` THEN the POM resolve it to the same declaration as `Wire`. (STD-09, AC 3)
Proof: run std | test a_std_path_names_the_same_declaration ... ok

**C55** - IF a file of another crate names a private item of a `std` namespace (`Slewing`) THEN the POM report `E0161`. (STD-09, AC 4)
Proof: run std | test a_private_std_item_is_e0161 ... ok

**C56** - No name exported by `std::prelude` equal the name of a row of the built-in table. (STD-09, AC 5)
Proof: run std | test no_export_has_a_core_name ... ok

**C57** - WHEN a model calls `clk.rise()` on a port `clk: Bit` THEN the POM resolve the call to the function `rise` of the `impl Bit` of `std::values`. (STD-09, AC 6)
Proof: run std | test bit_rise_resolves_into_std_values ... ok

**C58** - WHEN a model calls `d.known()` on a port `d: Bit<u8>` THEN the POM resolve the call to the method `known` of the trait `Known` of `std::values`. (STD-09, AC 7)
Proof: run std | test known_resolves_into_std_values ... ok

**C59** - WHEN a model places `D2A(d, a, vdd)` and `A2D(a, q)` THEN the POM link each instance to the module of `std::convert`. (STD-09, AC 8)
Proof: run std | test converters_resolve_into_std_convert ... ok

**C60** - WHEN a model places `Setup(clk, d)` with a param block and reads `inertial(x, tpd)` THEN the POM link each to the module of `std::monitor`. (STD-09, AC 9)
Proof: run std | test monitors_resolve_into_std_monitor ... ok

**C61** - WHEN a design is built from one empty source THEN `Design::diagnostics()` be empty. (STD-10, AC 1)
Proof: run std | test embedded_std_has_no_diagnostics ... ok

**C62** - WHEN that design is built THEN no `Field` or `MethodCall` slot of a `std` file be `Unresolved`. (STD-10, AC 2)
Proof: run std | test std_has_no_unresolved_member ... ok

**C63** - WHEN that design is built THEN `Design::check()` return no problem. (STD-10, AC 3)
Proof: run std | test embedded_std_passes_check ... ok

**C64** - IF `std_source` replaces the library with one that lacks an item THEN a model that names the item get `E0161` and the build complete. (STD-10, AC 4)
Proof: run std | test a_replaced_std_without_an_item_is_e0161 ... ok

**C65** - WHEN a model writes `analysis(Transient)` and places `Tran(a, b, en)` THEN `Transient` resolve to the built-in variant and `Tran` to the module of `std::nets`. (STD-10, AC 5)
Proof: run std | test transient_is_the_kind_and_tran_the_module ... ok

**C66** - WHEN a fence of Part VII has the class `rust` THEN its text, with trailing whitespace of each line and leading and trailing blank lines removed, occur as consecutive whole lines of the `std` file its section maps to. (STD-11, AC 1)
Proof: run excerpts | test every_part_vii_fence_is_in_its_file ... ok

**C67** - WHEN a fence of Part VII declares a module `Pi` or `MeasuredDiode`, or names `boot.hex`, THEN the fence have the class `rust,fragment`. (STD-11, AC 2)
Proof: run excerpts | test part_vii_examples_are_fragments ... ok

**C68** - The guard count the fences it checked and fail when that count is under 45. (STD-11, AC 3)
Proof: run excerpts | test the_forward_guard_covers_part_vii ... ok

**C69** - IF a `rust` fence of Part VII is not in its file THEN the guard fail and name the spec line of the fence and its first line that has no match. (STD-11, AC 4)
Proof: run excerpts | test a_missing_fence_is_named ... ok

**C70** - IF the control text of the guard, a fence that differs from the file by one character, is checked THEN the guard report it as not found. (STD-11, AC 5)
Proof: run excerpts | test a_fence_one_character_off_is_not_found ... ok

**C71** - WHEN a `std` file other than `lib.msl` and `prelude.msl` declares an item at its top level THEN the source text of the item, attributes and doc comments included, occur verbatim inside a `rust` fence of `docs/spec/`. (STD-12, AC 1)
Proof: run excerpts | test every_std_item_is_in_a_fence ... ok

**C72** - WHEN a `std` file holds a behavior block THEN the text of the block occur verbatim inside a `rust` fence of Part VII. (STD-12, AC 2)
Proof: run excerpts | test every_std_behavior_block_is_in_a_fence ... ok

**C73** - The fences of §1, §4 and §11 that show `Q` to `inf`, the four natures and `Short` show them with `pub`. (STD-12, AC 3)
Proof: run excerpts | test prelude_items_outside_part_vii_are_shown_pub ... ok

**C74** - IF an item of `std` is in no fence THEN the guard fail and name the `std` file, the line and the item. (STD-12, AC 4)
Proof: run excerpts | test a_missing_item_is_named ... ok

**C75** - The guard count the items it checked and fail when that count is under 80. (STD-12, AC 5)
Proof: run excerpts | test the_reverse_guard_covers_std ... ok

**C76** - The fixture directory `tests/fixtures/gallery/` hold one file for each of `Resistor`, `Capacitor`, `Switch`, `SampleHold`, `Comparator`, `Cell`, `Dram1T1C`, `MemoryBank`, `Stream`, `Tx`, `Vco`, `RSwitch`. (STD-13, AC 1)
Proof: run galleries | test gallery_fixtures_exist ... ok

**C77** - WHEN a fixture file is read THEN each of its lines occur in `docs/spec/`, in a `rust` fence or in a code span of the table "Models referenced from the normative parts" of Appendix D. (STD-13, AC 2)
Proof: run galleries | test every_fixture_line_is_spec_text ... ok

**C78** - The gate hold a table with one row for each of the 18 gallery sections, naming the fixture models appended to its fence. (STD-13, AC 3)
Proof: run galleries | test the_fixture_table_covers_every_gallery ... ok

**C79** - IF the table names a model a gallery does not use, or a model the gallery defines itself, THEN the gate fail and name the pair. (STD-13, AC 4)
Proof: run galleries | test the_fixture_table_names_only_used_models ... ok

**C80** - WHEN a `rust` fence of Appendix D, E or F is built as the root file of a crate, with its fixture models appended and the embedded `std`, THEN `Design::diagnostics()` be empty. (STD-14, AC 1)
Proof: run galleries | test every_gallery_builds_with_no_diagnostics ... ok

**C81** - WHEN that design is built THEN `Design::check()` return no problem. (STD-14, AC 2)
Proof: run galleries | test every_gallery_passes_check ... ok

**C82** - WHEN that design is built THEN no `Field` or `MethodCall` slot of the root file be `Unresolved`. (STD-14, AC 3)
Proof: run galleries | test no_gallery_has_an_unresolved_member ... ok

**C83** - WHEN a gallery names a `std` item the gate's table lists for it THEN that item's `references()` include a reference in the root file, and the item's crate answer `is_std()`. (STD-14, AC 4)
Proof: run galleries | test every_gallery_references_its_std_items ... ok

**C84** - The gate build 18 fences and fail when it finds another number. (STD-14, AC 5)
Proof: run galleries | test the_gate_builds_18_fences ... ok

**C85** - IF a gallery is not clean THEN the gate fail and print, per fence, the spec file and line, each diagnostic's code and source text, and each unresolved member's name. (STD-14, AC 6)
Proof: run galleries | test a_dirty_gallery_is_reported_by_fence ... ok

**C86** - IF the same fences are built against a `std` of an empty `prelude` THEN the gate's control observe at least one `E0161` in every fence. (STD-14, AC 7)
Proof: run galleries | test galleries_fail_against_an_empty_prelude ... ok

**C87** - The gate's table of rule 4 hold at least one `std` item for each of the 18 fences. (STD-14, AC 8)
Proof: run galleries | test every_gallery_lists_a_std_item ... ok

**C88** - The built-in table hold no row for `Q`, `K_B`, `EPS0`, `PI`, `E` or `inf`. (STD-15, AC 1)
Proof: run resolve | test the_constants_are_not_builtin ... ok

**C89** - WHEN a model names `K_B` THEN the POM resolve it to `Resolution::Decl` of the constant of `std::constants`. (STD-15, AC 2)
Proof: run resolve | test k_b_resolves_to_std_constants ... ok

**C90** - The built-in table hold a row `Transient` of kind `Variant` and no row `Tran`. (STD-15, AC 3)
Proof: run resolve | test transient_is_a_builtin_variant ... ok

**C91** - The rows of kind `Type`, `Trait`, `Fn`, `Module`, `Event`, `Statement`, `Node`, `ImplicitParam`, `Member` and `Method`, and the variants `Some`, `None`, `Dc`, `Ac`, `Noise`, `Static`, be the rows the table has today. (STD-15, AC 4)
Proof: run resolve | test the_core_rows_are_unchanged ... ok

**C92** - IF a model declares a namespace item named `Q` THEN the POM report no `E0457`, and the model's item hide the prelude's. (STD-15, AC 5)
Proof: run resolve | test a_model_constant_hides_the_prelude_constant ... ok

**C93** - The file `tests/fixtures/pom/prelude.msl` does not exist, and no test file of `piperine-lang` name it. (STD-16, AC 1)
Proof: run std | test the_fixture_prelude_is_gone ... ok

**C94** - The targets `pom_classify`, `pom_reflect` and `pom_resolve` hold at least 39, 31 and 48 tests. (STD-16, AC 2)
Proof: run std | test no_pom_test_was_removed ... ok

**C95** - The page `docs/pom/resolution.md` list the built-in table without a `Const` row and say that the six constants are items of `std::constants`. (STD-17, AC 1)
Proof: run docs | test the_builtin_table_doc_has_no_const_row ... ok

**C96** - The page `docs/pom/resolution.md` name `extern "intrinsic"` and `is_intrinsic` where it describes `E0125`. (STD-17, AC 2)
Proof: run docs | test resolution_doc_describes_intrinsics ... ok

**C97** - The page `docs/pom/resolution.md` say that `Self::NAME` in a trait resolves through the supertraits. (STD-17, AC 3)
Proof: run docs | test resolution_doc_describes_the_supertrait_rule ... ok

**C98** - The Members table of `docs/pom/resolution.md` list the blanket implementation as the last step for a carrier. (STD-17, AC 4)
Proof: run docs | test members_doc_lists_the_blanket_step ... ok

**C99** - WHEN the examples of `docs/pom/` run as doctests THEN every example pass against the embedded library. (STD-17, AC 5)
Proof: `cargo test -p piperine-lang --doc`

**C100** - The file comment of `std/lib.msl` name the nine namespaces and does not say the content is future work. (STD-17, AC 6)
Proof: run std | test std_root_comment_names_the_namespaces ... ok

**C101** - The directory `docs/book/` hold the Book's source, `content.html`, `template.html` and `build.py`, with the twelve chapters it has today. (STD-18, AC 1)
Proof: run book | test the_book_source_is_in_docs_book ... ok

**C102** - WHEN `python3 docs/book/build.py` runs THEN it write `docs/book/index.html`, and that file be ignored by git. (STD-18, AC 2)
Proof: run book | test build_py_writes_an_ignored_index ... ok

**C103** - The file `docs/book/content.html` hold a chapter with the id `core-std` and the title "Core and standard library", placed after the chapter `packages`, and the chapter numbers be consecutive from 1. (STD-18, AC 3)
Proof: run book | test the_core_std_chapter_follows_packages ... ok

**C104** - WHEN a name is a core name (a row of the built-in table of kind `Fn`, `Module`, `Event`, `Statement`, `Method`, `Node` or `ImplicitParam`, or `Nature`, `str`, `emit!`, `include_str!`) THEN the chapter name it in a code span. (STD-18, AC 4)
Proof: run book | test the_chapter_names_every_core_name ... ok

**C105** - WHEN a namespace of `std` is `pub` THEN the chapter name it as `std::<name>` in a code span inside a heading of its own. (STD-18, AC 5)
Proof: run book | test the_chapter_has_a_part_per_namespace ... ok

**C106** - WHEN a name is exported by `std::prelude` THEN the chapter name it in a code span. (STD-18, AC 6)
Proof: run book | test the_chapter_names_every_prelude_export ... ok

**C107** - The chapter name `std::prelude` and each of the five items declared `extern "intrinsic"` under a heading that contains the word "engine". (STD-18, AC 7)
Proof: run book | test the_chapter_explains_the_prelude_and_the_engine_items ... ok

**C108** - WHEN a block of the chapter is marked `data-lang="msl"` THEN its text build as one file against the embedded library with zero diagnostics. (STD-18, AC 8)
Proof: run book | test chapter_blocks_build_against_std ... ok

**C109** - WHEN a block of any chapter is marked `data-lang="msl"` THEN its text, HTML entities decoded, parse with zero diagnostics as a file or as one of the fragment forms `msl_corpus` accepts. (STD-18, AC 9)
Proof: run book | test every_msl_block_parses ... ok

**C110** - The chapter `packages` name `std::prelude` in a code span, and the chapter `cheatsheet` name every `pub` namespace of `std` as `std::<name>`. (STD-18, AC 10)
Proof: run book | test packages_and_cheatsheet_name_std ... ok

**C111** - The check count the `msl` blocks of the Book and fail when it finds fewer than 40. (STD-18, AC 11)
Proof: run book | test the_book_has_at_least_40_msl_blocks ... ok

**C112** - IF the chapter omits a core name or an export THEN the check fail and list every missing name. (STD-18, AC 12)
Proof: run book | test a_missing_name_is_listed ... ok

**C113** - No block of the Book contain `analysis(Tran)`. (STD-18, AC 13)
Proof: run book | test the_book_says_transient ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** B1 (T1-T7, Sonnet) at 0ae471d1
**Walked:** no screen in this feature; the stories' Independent Tests are the checks above. The Book is a page, but it is published by the orchestrator after the merge and has no binding capture.

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4
Phase 2:  T5 ------→ T6 ------→ T7
Phase 3:  T8 ------→ T9 ------→ T10 ------→ T11 ------→ T12
Phase 4:  T13 ------→ T14 ------→ T15 ------→ T16 ------→ T17
Phase 5:  T18 ------→ T19 ------→ T20 ------→ T21
Phase 6:  T22 ------→ T23 ------→ T24
```

Execution is strictly sequential. 24 tasks pack into 4 batches of whole phases, one Sonnet worker per batch; the Verifier (Opus) runs once after T24, over `check_proofs.py msl-stdlib`. Besides judging each check, the Verifier hand-traces `poly_q`, `poly_s` and `laplace_nd` on one example each and compares the three limiters with `crates/piperine-ir/src/limit.rs` (`design.md`, Risks).

| Batch | Phases | Tasks | Layer |
| ----- | ------ | ----- | ----- |
| 1 | 1, 2: Specification; POM | T1 to T7 (7) | `docs/spec/`, `src/pom/` |
| 2 | 3: Library I | T8 to T12 (5) | `tests/`, `std/`, Part VII fences |
| 3 | 4: Library II | T13 to T17 (5) | `std/`, Part VII fences, `tests/std_library.rs` |
| 4 | 5, 6: Gates; the Book | T18 to T24 (7) | `tests/`, `src/pom/builtin.rs`, `docs/book/` |

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: §3 defines `extern "intrinsic" fn` | one rule and one error row of §3 | ✅ Granular |
| T2: §22 says the library is source: rules, "Sources", "Implemented by the engine" | the preamble of one section | ✅ Granular |
| T3: The analysis kind is `Transient` | one rename, every file that spells it | ⚠️ Cohesive: one concept across files |
| T4: Appendix G records S1 to S5; §1 places the constants in `std::constants` | one decision table and one caption | ✅ Granular |
| T5: The POM accepts `extern "intrinsic" fn` in `std` only | one check and one method | ✅ Granular |
| T6: `Self::NAME` in a trait resolves through its supertraits | one lookup rule | ✅ Granular |
| T7: A carrier's method resolves through a blanket implementation | one lookup step | ✅ Granular |
| T8: The excerpt guards exist and are honest on an empty library | one test target and the helpers it shares | ✅ Granular |
| T9: `std::constants`, `std::carriers`, the prelude of re-exports | two small namespaces and the prelude that exports them | ⚠️ Cohesive: one concept across files |
| T10: `std::values` (§22.1) | one namespace | ✅ Granular |
| T11: `std::analog`, part one (§22.2.1 to §22.2.11, §22.2.15 to §22.2.17) | one namespace file, first part | ✅ Granular |
| T12: `std::analog`, part two: `zi_*` and `laplace_*` (§22.2.12 to §22.2.14) | one namespace file, second part | ✅ Granular |
| T13: `std::nets` (§22.3) | one namespace | ✅ Granular |
| T14: `std::convert` (§22.4) | one namespace | ✅ Granular |
| T15: `std::monitor` (§22.5) | one namespace | ✅ Granular |
| T16: `std::io` (§22.6) | one namespace | ✅ Granular |
| T17: The library is complete: layout, Index, intrinsic list, floors | the completeness tests of one target | ✅ Granular |
| T18: The gallery fixtures | one fixture set and its table | ✅ Granular |
| T19: The galleries build clean; G.38 is closed | one gate | ✅ Granular |
| T20: The built-in table holds core names only | one table | ✅ Granular |
| T21: The fixture prelude is retired | one fixture retired | ⚠️ Cohesive: one concept across files |
| T22: The Book's source lives in `docs/book/` and every `msl` block parses | one move and its check | ✅ Granular |
| T23: The chapter "Core and standard library" | one chapter | ✅ Granular |
| T24: The packages note and the cheat sheet follow the library | two notes in one file | ✅ Granular |

T3, T9 and T21 touch several files for one concept each (a rename; two namespaces that only resolve with the prelude that exports them; one fixture's users). Splitting any of them leaves a commit that does not build clean. Every `Where` names its primary file first; the files in parentheses are the fences, tests and lists that must move in the same commit (MD-57 rule 2).

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

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: §3 defines `extern "intrinsic" fn` | specification text | integration | integration | ✅ OK |
| T2: §22 says the library is source: rules, "Sources", "Implemented by the engine" | specification text | integration | integration | ✅ OK |
| T3: The analysis kind is `Transient` | specification text, POM resolution, reference docs | integration | integration | ✅ OK |
| T4: Appendix G records S1 to S5; §1 places the constants in `std::constants` | specification text | integration | integration | ✅ OK |
| T5: The POM accepts `extern "intrinsic" fn` in `std` only | POM resolution, reference docs | integration | integration | ✅ OK |
| T6: `Self::NAME` in a trait resolves through its supertraits | POM resolution, reference docs | integration | integration | ✅ OK |
| T7: A carrier's method resolves through a blanket implementation | POM resolution, reference docs | integration | integration | ✅ OK |
| T8: The excerpt guards exist and are honest on an empty library | fences against `std`, test support | integration | integration | ✅ OK |
| T9: `std::constants`, `std::carriers`, the prelude of re-exports | the library, specification text | integration | integration | ✅ OK |
| T10: `std::values` (§22.1) | the library | integration | integration | ✅ OK |
| T11: `std::analog`, part one (§22.2.1 to §22.2.11, §22.2.15 to §22.2.17) | the library, specification text | integration | integration | ✅ OK |
| T12: `std::analog`, part two: `zi_*` and `laplace_*` (§22.2.12 to §22.2.14) | the library, specification text | integration | integration | ✅ OK |
| T13: `std::nets` (§22.3) | the library | integration | integration | ✅ OK |
| T14: `std::convert` (§22.4) | the library, fences against `std` | integration | integration | ✅ OK |
| T15: `std::monitor` (§22.5) | the library, fences against `std` | integration | integration | ✅ OK |
| T16: `std::io` (§22.6) | the library, fences against `std` | integration | integration | ✅ OK |
| T17: The library is complete: layout, Index, intrinsic list, floors | the library, fences against `std` | integration | integration | ✅ OK |
| T18: The gallery fixtures | galleries and fixtures | integration | integration | ✅ OK |
| T19: The galleries build clean; G.38 is closed | galleries, specification text | integration | integration | ✅ OK |
| T20: The built-in table holds core names only | POM resolution, reference docs | integration | integration | ✅ OK |
| T21: The fixture prelude is retired | the library (fixture), reference docs | integration | integration | ✅ OK |
| T22: The Book's source lives in `docs/book/` and every `msl` block parses | the Book | integration | integration | ✅ OK |
| T23: The chapter "Core and standard library" | the Book | integration | integration | ✅ OK |
| T24: The packages note and the cheat sheet follow the library | the Book | integration | integration | ✅ OK |

T8 also moves test support (matrix: none); the task takes the higher requirement of the layers it changes.

---

## Requirement coverage

| Requirement | Criteria | Checks | Tasks |
| ----------- | -------- | ------ | ----- |
| STD-01 | 10 | C1 to C10 | T2, T3, T4, T19 |
| STD-02 | 9 | C11 to C19 | T1, T2, T11, T12 |
| STD-04 | 6 | C20 to C25 | T5 |
| STD-05 | 3 | C26 to C28 | T6 |
| STD-06 | 5 | C29 to C33 | T7 |
| STD-07 | 5 | C34 to C38 | T9, T17 |
| STD-08 | 13 | C39 to C51 | T9, T11, T12, T13, T17 |
| STD-09 | 9 | C52 to C60 | T9, T10, T11, T13, T14, T15, T17 |
| STD-10 | 5 | C61 to C65 | T9, T13 |
| STD-11 | 5 | C66 to C70 | T8, T16, T17 |
| STD-12 | 5 | C71 to C75 | T8, T9, T17 |
| STD-13 | 4 | C76 to C79 | T18 |
| STD-14 | 8 | C80 to C87 | T19 |
| STD-15 | 5 | C88 to C92 | T3, T20 |
| STD-16 | 2 | C93 to C94 | T21 |
| STD-17 | 6 | C95 to C100 | T5, T6, T7, T17, T20, T21 |
| STD-18 | 13 | C101 to C113 | T22, T23, T24 |

113 criteria, 113 checks, 24 tasks, 6 phases.

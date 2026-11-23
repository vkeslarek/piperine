# Explicit Unknowns Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/explicit-unknowns/design.md`
**Status**: In Progress
**Approved**: 2026-11-21 - user: "Aceito os recomendados" (Q4: no separate strength trait, `impl Resolve<Driven>` carries it; Q5: Strength set B, five levels HighZ < Weak < Pull < Strong < Supply), after R1–R10, Q1–Q3 and the ABI (major 1, edited in place) the same day; D1–D21 stand.
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run bookvalues**: `cargo test -p piperine --test book_values`
**Run booklang**: `cargo test -p piperine --test book_language`
**Run booknatures**: `cargo test -p piperine --test book_natures`
**Run bookabi**: `cargo test -p piperine --test book_abi`
**Run bookinv**: `cargo test -p piperine --test book_inventory`
**Run types**: `cargo test -p piperine-lang --test types_explicit`
**Run src**: `cargo test -p piperine-parity --test source_explicit`
**Run ir**: `cargo test -p piperine-ir --test digital`
**Run abi**: `cargo test -p piperine-abi --test layouts`
**Run plan**: `cargo test -p piperine-plan --test digital_shape`
**Run cache**: `cargo test -p piperine-transpile --test cache`
**Run abicontract**: `cargo test -p piperine-abi --test contract`
**Run relax**: `cargo test -p piperine-digital-solver --test relax`
**Run records**: `cargo test -p piperine-api --test records`
**Run session**: `cargo test -p piperine-api --test session`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run hygiene**: `cargo test -p piperine-lang --test suite_hygiene`
**Run excerpts**: `cargo test -p piperine-lang --test std_excerpts`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run buckcli**: `cargo test -p piperine-cli --test buck`
**Run integrated**: `cargo test -p piperine --test integrated`
**Run cliintegrated**: `cargo test -p piperine-cli --test integrated`
**Run ops**: `cargo test -p piperine-lang --test types_operators`
**Run logic**: `cargo test -p piperine-parity --test source_logic`

Executors: **every task is Sonnet**. Opus verifies each batch and the feature.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-a TMPDIR=/var/tmp/piperine-tmp-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` before any cargo command; never `/var/tmp/piperine-target`, never a target directory another worktree is building in. Cargo with `-p <crate>` or plain at the root, never `cargo clean`. Long commands run in the background and are waited for. If `df /` passes 90 %, stop and report. Never `git stash`.
- **Gate policy (user, 2026-11-21).** Per task: the `**Build**` line and the tests of the crates the task touched. At the end of each batch and at each merge: `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` at the root, `cargo test -p piperine --test integrated`, `cargo test -p piperine-cli --test integrated`, the parity source targets (`cargo test -p piperine-parity`) and the ngspice twins (`cargo test -p piperine-cli --test twins`, `cargo test -p piperine-parity --test twins`). The buck runs on demand: only in a task whose `Done when` says "with the buck", because the buck's sources, the generated code or the layouts moved.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens stay byte-identical; no tolerance moves.
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names. A test is never deleted: it is replaced by the test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** After B1 to B3, `docs/spec` is the authority. A divergence a task cannot fix in its scope stops the task and is reported.
- **Proof lines are real lines.** A check that names a test of an existing target (`twins`, `buck`, `integrated`) or of a target a task creates is confirmed against a real run in the task that first makes it pass; a line that differs from what the runner prints is corrected in that task's commit, never the test renamed to fit.
- **Tests state the spec, not the code.** Expected values are the specification's own tables or the same computation written in the test; each assertion cites its rule. Test functions have exactly the names `## Checks` cites.
- **The Book.** Part 1 Fundamentals before Part 2 In depth; self-contained (never "same as Rust" or "as in Rust"); a respectful voice; every listing has a row in `tests/book/inventory.tsv` and a test; a listing that cannot run yet names its `GAPS.md` row and its test asserts today's refusal, so it fails loud the day it runs.
- **Code clarity (`.specs/features/code-clarity/spec.md`).** No free function; names are whole words; one purpose per type; no macro; every item a task adds or changes carries a `///` comment; a removal removes the whole concept, with no dead field, no unused variant and no comment about what used to be there. No `unwrap()` or `expect()` on a path user input reaches.
- **Conventions.** Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- One atomic commit per task, message as given, lowercase subject of at most 72 characters, ending with `Co-Authored-By: Claude Sonnet <noreply@anthropic.com>` as the session's attribution reminder spells it. No push and no merge without the user's word.
- **`lessons.py` is not run** during this feature.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Build and test", "Where the tests are"), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-77 (the buck runs on demand; the integrated suite is the gate).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Specification text (`docs/spec`) | integration | one assertion per rule written; one walk that the removed words are absent | `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-lang --test spec_text` |
| `abi.md`, `STATE.md`, `GAPS.md` | integration | one assertion per record changed | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| Book (`docs/book`) | integration | every listing and every claim has a row and a test; refused listings assert the refusal | `tests/book/*.rs`, `tests/book/inventory.tsv` | `cargo test -p piperine --test book_<chapter>` |
| Parser (`piperine-lang/src/msl`) | integration | the removed token: its code, its span, its help | `crates/piperine-lang/tests/msl_errors.rs` | `cargo test -p piperine-lang --test msl_errors` |
| Checker (`piperine-lang/src/types`) | integration | every AC the checker decides; each code with its span and the name it gives | `crates/piperine-lang/tests/types_explicit.rs`, `types_operators.rs` | `cargo test -p piperine-lang --test types_explicit` |
| Elaborator and runs | integration | one test per run-time AC, from source on the Rust backend, expected values from the specification | `crates/piperine-parity/tests/source_explicit.rs`, `source_logic.rs` | `cargo test -p piperine-parity --test source_explicit` |
| IR (`piperine-ir`) | integration | no flagged type is constructible | `crates/piperine-ir/tests/digital.rs` | `cargo test -p piperine-ir --test digital` |
| Generated code (`piperine-transpile`) | integration | one path per control; goldens moved once | `crates/piperine-transpile/tests/reaction.rs`, `golden.rs` | `cargo test -p piperine-transpile` |
| ABI (`piperine-abi`) | integration | each layout byte for byte; the refusal of major 1 | `crates/piperine-abi/tests/layouts.rs`, `contract.rs` | `cargo test -p piperine-abi` |
| Engines | integration | relaxation and its failure | `crates/piperine-digital-solver/tests/relax.rs` | `cargo test -p piperine-digital-solver --test relax` |
| Host and CLI | integration | each cell form in each output format | `crates/piperine-api/tests/records.rs`, `session.rs`, `crates/piperine-cli/tests/run.rs` | `cargo test -p piperine-api`, `cargo test -p piperine-cli` |
| Standard library | integration | builds clean; Part VII fences are excerpts | `crates/piperine-lang/tests/std_excerpts.rs` | `cargo test -p piperine-lang --test std_excerpts` |
| Tree hygiene | integration | no removed name in any `.msl`; every replaced test has a successor | `crates/piperine-lang/tests/suite_hygiene.rs` | `cargo test -p piperine-lang --test suite_hygiene` |
| Parity | integration | twins byte-identical; integrated suites green | `crates/piperine-cli/tests/twins.rs`, `tests/integrated` | `cargo test -p piperine-cli --test twins` |
| Contract re-reads (T19, T41) | none | - (build gate; findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task that changes text, or one or two test targets of one crate | the `**Build**` line, then the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the parser, the checker, the elaborator, the IR, the printer, the ABI, an engine, the host or `std` | Quick, then `cargo test -p <crate>` for every crate the task touched; "with the buck" adds the buck targets of `piperine-parity` and `cargo test -p piperine-cli --test buck` |
| Batch | After the last task of each batch, and at each merge | Full, then `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` at the root, `cargo test -p piperine --test integrated`, `cargo test -p piperine-cli --test integrated`, `cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins` |
| Build | After a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. One phase is one batch.

**Batches** (every worker Sonnet, every batch verified by Opus):

| Batch | Tasks | Starts on | Delivers |
| ----- | ----- | --------- | -------- |
| B1 | T1 to T6 | `main` at `8b5596ed`, once the spec is approved | the specification's normative parts: the explicit model, traits as capabilities, `Resolve<X, V>` |
| B2 | T7 to T11 | B1 | Part VII (`Logic`, strengths, net modules), the appendices, the four worked nets in the gallery, `abi.md` edited in place, MD-78, `GAPS.md` S8 |
| B3 | T12 to T18 | B2 | the Book, with "Driving with strength". **B1 to B3 merge to `main` together, ahead of every code batch** |
| B4 | T19 to T22 | B3 on `main` **and** `composite-values` on `main` | plain analog reads, the `Default` table, initial values |
| B5 | T23 to T27 | B4 | `std`, the buck, the e2e packages, the fixtures and the unknown tests in the explicit form |
| B6 | T28 to T33 | B5 | the names refused; no flag in the elaborator, the IR and the generated code |
| B7 | T34 to T40 | B6 | records, the ABI without known bytes (still major 1), relaxation and `E0511`, host, CLI; S8 closed. **Phase A merges to `main` here, before `generics`** |
| B8 | T41 to T48 | B7 on `main` **and** `generics` phase A on `main` | operator traits, `Logic<N>` from source, four-state cells |

### Phase 1: The specification: values, carriers, structure, behavior (B1)

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: The specification: library, appendices, ABI, records (B2)

```
T7 → T8 → T9 → T10 → T11
```

### Phase 3: The Book (B3)

```
T12 → T13 → T14 → T15 → T16 → T17 → T18
```

### Phase 4: Phase A: contract and the accepted forms (B4)

```
T19 → T20 → T21 → T22
```

### Phase 5: Phase A: the tree is rewritten (B5)

```
T23 → T24 → T25 → T26 → T27
```

### Phase 6: Phase A: the names are refused, the flag leaves the front end and the IR (B6)

```
T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 7: Phase A: records, the ABI, engines, host (B7)

```
T34 → T35 → T36 → T37 → T38 → T39 → T40
```

### Phase 8: Phase B: `Logic` from library source (B8)

```
T41 → T42 → T43 → T44 → T45 → T46 → T47 → T48
```

---

## Task Breakdown

### T1: Part I: §3.3 is the explicit model, a nature takes trait impls

**What**: Amend §3.1 (the table, §3.1.9, `E0134` for a bundle only), §3.7 (the sentence "a trait is a capability", §3.7.12 lists `Resolve`, the `E0134` row) as design.md "Traits are capabilities". Rewrite `part_i_values.md` §3.3 as design.md "Part I §3.3" (title "Uncertainty and absence", rules §3.3.1 to §3.3.6, no error of its own, new examples). Amend §1 rule 13, §2's opening and rules 7 to 11, the §1.5 and §1.8 sentences on unknown operands and control, and §3.7.5's `Known` example. Replace the `spec_text.rs` assertions that quote the earlier §3.3.
**Where**: `docs/spec/part_i_values.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md "The explicit model"; `value-semantics` `cc3599bf` as the form of an amendment
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] §3.3 holds the seven rules, each numbered, and the sentence that `bool`, `UInt<N>`, `Int<N>`, `Float<N>` and `str` do not implement `Default` and that this differs from Rust
- [x] `spec_text.rs::section_3_3_states_the_explicit_model` and `a_nature_takes_trait_impls` pass; the tests it replaces are named in the commit body
- [x] Every anchor `#33-unknown-values` in `docs/spec` still resolves (the heading keeps an explicit anchor or every link moves)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): part i section 3.3 is the explicit model`

---

### T2: Appendix H: `Default`, `From`, `PartialEq<Rhs, Out>`, operator traits

**What**: In `appendix_h_library.md`: remove H rule 5 (all-or-nothing methods); H.8 `Default` row and rules 4 and 7 as D4; `PartialEq<Rhs = Self, Out = bool>`; `From<T>`; new H.10 with the table and the five rules of design.md.
**Where**: `docs/spec/appendix_h_library.md`
**Depends on**: T1
**Reuses**: design.md "Appendix H"
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] H.8 and H.10 read as design.md gives them
- [x] `spec_text.rs::appendix_h_holds_default_from_and_the_operator_traits` passes
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): appendix h, default table and operator traits`

---

### T3: Part II: carriers start with a value, `Resolve` is a capability

**What**: Amend `part_ii_carriers.md` as design.md "Traits are capabilities" and "Carriers". §4: the `nature_field` production loses `resolve`; the kinds table; rules 1, 2, 6, 8 (the trait `Resolve<X: Repr, V: Repr = X>`), 15; `E0203`. §5.2: the two definitions, §5.2.1 to §5.2.3, §5.2.9, §5.2.10, §5.2.12, new §5.2.15, `E0220` reworded, new `E0225`. §5.1.1, §5.1.5, §5.1.6, §5.1.8, the §5.1 examples and error table (`E0187`), §5.2.8, the §5.3 table, §5.3.4 to §5.3.8, §5.3.11, §5.3.12 removed, §6 rule 16, §7 rule 2 (a field's direction by whether its nature implements `Resolve`).
**Where**: `docs/spec/part_ii_carriers.md`
**Depends on**: T2
**Reuses**: design.md "Traits are capabilities", "Carriers"; spec.md D8, D9, D17 to D21
**Requirement**: EU-11
**Executor**: Sonnet

**Done when**:

- [x] The rules read as design.md gives them; no sentence mentions a flag or a `resolve` field
- [x] `spec_text.rs::resolve_is_a_trait_of_two_parameters`, `many_drives_need_the_resolve_capability`, `a_drive_picks_its_impl_by_its_type`, `resolve_receives_plain_values_for_every_count` and `a_drive_without_a_value_starts_at_its_default` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): part ii, resolve is a capability of a nature`

---

### T4: Part III: structure without unknown levels

**What**: Amend `part_iii_structure.md` by design.md "Other parts": §8.1 (a level no definition names), §8.2.3 steps 1 and 4 and the error table (`E0511`, phase run), §8.2.28, §8.2.30, §8.3.18, §8.5, the two `known_or` examples (`DigiPot`, `LaggedResistor` read `r` directly).
**Where**: `docs/spec/part_iii_structure.md`
**Depends on**: T3
**Reuses**: design.md "Other parts"
**Requirement**: EU-11
**Executor**: Sonnet

**Done when**:

- [x] `E0511` is a row of the §8.2 error table and of Part V's run codes
- [x] `spec_text.rs::a_relaxing_drive_starts_absent` and `a_loop_that_does_not_settle_is_e0511` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): part iii, a loop that does not settle is e0511`

---

### T5: Part IV: behavior reads values

**What**: Amend `part_iv_behavior.md`: §11 rule 13 and its resolution (the text of §3.3.6), §12.1.19 removed, the `SampleHold` example (`held` read directly, with an initial value), the `clk.prev().is_known()` sentence removed, §13 rules 1, 5, 6 and 16, `at` and `cross` arguments, the §14 tables (`x.level(k)` leaves), the `E0150` row.
**Where**: `docs/spec/part_iv_behavior.md`
**Depends on**: T4
**Reuses**: design.md "Other parts"
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] §13 rule 6 says the initial value's commit at `(0, 0)` is a change of every carrier
- [x] The file holds no removed word; `spec_text.rs` assertions that quoted the earlier rules are replaced and pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): part iv, analog and events read values`

---

### T6: Parts V, VI and VIII: execution, output and the host

**What**: Amend `part_v_execution.md` (§16 step 2, §17.3 rule 5 and its resolution, `E0511` among the run codes), `part_vi_io.md` (§19 rules 1 and 11, the unknown `pass`, `E0604`) and `part_viii_host.md` (§23.1 rule 9, §23.2 rules 3 and 4, §23.3 rule 4g). The three files are one task because each change is under ten lines.
**Where**: `docs/spec/part_vi_io.md`
**Depends on**: T5
**Reuses**: design.md "Other parts"
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] A record is its declared fields; a `require` is a pass or a fail
- [x] §23.1 rule 9 reads as design.md gives it
- [x] `spec_text.rs` assertions for §19 and §23 are replaced and pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): parts v, vi and viii without unknown values`

---

### T7: Part VII: `Logic`, the tables, strengths and the net modules

**What**: Rewrite `part_vii_stdlib.md` §22.1 (remove `Known` and `merge`; `cast`, `same`, `matches_bits`, `logic`, `impl Bit`, `impl Bit<Logic>`; new units for `Logic`, `LogicState`, the operator `impl`s and the printed tables) and §22.3 as design.md "Part VII §22.3": `Strength`, `Driven`, `Driven::strongest` and its table, `sum`, the eight modules generic over `W: Nature + Resolve<Driven>`; `Wire`, `Tri`, `tri`, `SumNet` and `Logic::wired` removed. Under §22.3, a rationale of at most fifteen lines for the set of `Strength`, citing the survey of spec.md Q5: **each citation is checked against its document first** (IEEE 1364 §7, IEEE 1800 §6.6.7 and §28, IEEE 1164, IEEE 1666, the ngspice manual, the Logisim and Digital documentation), and one that cannot be confirmed is dropped and reported, never kept. Amend §22.4 converters and §22.5 monitors where they read a flag or name `Wire`, the index, and the error table. The fences of units that `std` does not hold yet are marked by the section's own convention, and `std_excerpts.rs` lists them as ahead of `std` until T23 and T44.
**Where**: `docs/spec/part_vii_stdlib.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md "Part VII §22.1", "Part VII §22.3"; Q4 = A, Q5 = B
**Requirement**: EU-12
**Executor**: Sonnet

**Done when**:

- [x] §22.1 prints the four tables beside the source
- [x] `spec_text.rs::section_22_1_holds_the_four_state_tables`, `strongest_resolves_by_strength`, `the_library_declares_no_net_nature` pass
- [x] `std_excerpts` passes with the fences ahead of `std` listed by name
- [x] The commit body lists each citation and where it was confirmed
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): part vii, logic, strengths and net modules`

---

### T8: Appendices A, B, C and the index

**What**: `appendix_a_lexical.md`: remove A.3, retire `E0906` and `E0908`, the `IDENT` and literal productions. `appendix_b_grammar.md`: row 21, the literal count, the placement-matrix row, the `literal` production; row 15 (`resolve`), the contextual-keyword list and the `nature_field` production lose `resolve`, as does A.1's list. `appendix_c_implementation.md`: the `signal` row of C.1, the "unknown control" row of C.2, the loop bound of C.2.1. `index.md`: the concept and glossary rows (known, unknown, flag-total, merge leave; `Logic`, absence, `Default` enter).
**Where**: `docs/spec/appendix_a_lexical.md`
**Depends on**: T7
**Reuses**: spec.md "Inventory"
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] `Unknown` and `resolve` are in no production and no keyword list
- [x] Appendix B's count of additions is two lower and its sum still matches its rows
- [x] The `spec_text.rs` tests `appendix_c_states_the_loop_bound_under_unknown_control` and `appendix_c_lists_the_loop_bound_among_the_defaults` are replaced by a test that C.2 holds no such row
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): appendices a, b, c and the index lose unknown`

---

### T9: Galleries D, E, F and Appendix G

**What**: Rewrite every listing and note of `appendix_d_gallery_analog.md`, `appendix_e_gallery_digital.md` and `appendix_f_gallery_mixed.md` that reads a flag or leaves a carrier without a value (E.5's unknown index becomes a plain index). Rewrite F.4 as the four worked nets of design.md "The worked nets", with the `DataBus` of D12 as a fifth listing: each with its nature, its `impl`, its module code and its table of outcomes; every gallery that used `Wire` declares its own nature. `appendix_g_scope.md`: new G.72 (what was removed, R1 to R10, "the reference implementation does not meet this yet"), G.29, G.46 and G.57 moved to the closed items. Add the absence test over the whole of `docs/spec`.
**Where**: `docs/spec/appendix_g_scope.md`
**Depends on**: T8
**Reuses**: G.71 as the form of an amendment entry
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] `spec_text.rs::the_removed_words_are_absent_from_the_specification` walks every file of `docs/spec` and exempts G.72 only
- [x] `appendix_g_records_the_explicit_model` and `the_gallery_works_four_nets` pass
- [x] `std_galleries` passes, with the gallery fixtures that changed regenerated
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): galleries in the explicit model, appendix g.72`

---

### T10: `abi.md`: the contract edited in place

**What**: Rewrite `.specs/features/device-abi/abi.md` as design.md "`abi.md`, still major 1, edited in place": the unreleased sentence at the top and in §12, §2 (six entries), §3 (`Field`, constants, `MAJOR` 1, `MINOR` 0), P3, P5, the `TRACE` and C5 presence fields, C7 removed, D1 (drive pins of `X`, value pin of `V`, `E0511`), the worked examples of §11 that show a known byte, §14 point 6 removed, §15 rows 6 and 7.
**Where**: `.specs/features/device-abi/abi.md`
**Depends on**: T9
**Reuses**: design.md "`abi.md`, still major 1, edited in place"
**Requirement**: EU-17
**Executor**: Sonnet

**Done when**:

- [x] `docs.rs::abi_md_describes_the_unreleased_contract` passes: the symbol `piperine_abi_1`, minor 0, the four-field `Field`, the three shipped layouts, the `driving` byte, the unreleased sentence, and the absence of `HAS_KNOWN`, `commit_with` and `known byte`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(abi): no known byte, no commit_with, still major 1`

---

### T11: Records of the amendment

**What**: `.specs/STATE.md`: append MD-78 (spec.md "MD entry"; renumber if `composite-values` took the number), mark MD-70 superseded and MD-64 superseded in part. `GAPS.md`: S6 and G6 closed by redesign, new S8 ("the implementation still carries the known flag", with the layers of spec.md's code inventory), the rows of spec.md "`GAPS.md`". `.specs/features/unknown-semantics-per-level/spec.md`: a first line that says dropped, by the user, 2026-11-21, MD-78. `ROADMAP.md`: the `KNOWN_LEVELS` line removed. `crates/piperine-lang/tests/support/code_owners.tsv`: `E0511` and `E0225` owned by `explicit-unknowns`, `E0150`, `E0151`, `E0906`, `E0908` out.
**Where**: `.specs/STATE.md`
**Depends on**: T10
**Reuses**: MD-76 as the form of an entry; `spec_codes.rs`
**Requirement**: EU-19
**Executor**: Sonnet

**Done when**:

- [x] `docs.rs::state_holds_the_explicit_unknowns_decision` and `the_per_level_feature_is_recorded_as_dropped` pass
- [x] `spec_codes` passes
- [x] `GAPS.md`'s counts line matches its rows
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): md-78, s6 closed by redesign, per-level dropped`

---

### T12: Book: "Values in hardware" teaches the explicit model

**What**: In `docs/book/content.html`, replace the sections "Known and unknown", "Catching an unknown with `emit!`" and "Analog code has no unknowns" by three sections in the Book's own voice: (1) what a binary type holds and what `Logic` adds, with the four tables; (2) `Option` for absence, and `Default` deciding whether a carrier may start without a value (`signal en: Bit;` refused, `= false` accepted, `Bit<Option<u8>>` starting `None`, `Bit<Logic<4>>` starting `xxxx`); (3) deciding on a `Logic` (`==` gives a `Logic`, `if` takes a `bool`, `is_high`, `same`, `match x.at(0)`), the conversions, and analog reading a `Logic` through `to_volts`. Self-contained, respectful, Part 1 Fundamentals before Part 2 In depth, never "same as Rust". Each listing gets its inventory row; a listing that needs phase A or B names S8 or its L row and its test asserts today's refusal.
**Where**: `docs/book/content.html`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the chapter's existing sections; `tests/book/values.rs`
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_values::the_values_chapter_teaches_the_explicit_model` passes
- [x] `the_flag_is_read_with_known_known_or_and_is_known` and `analog_reads_a_digital_carrier_through_known_or` are replaced, both names in the commit body
- [x] `book_inventory` and `book_claims` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): values in hardware, logic, option and default`

---

### T13: Book: language basics, types and traits

**What**: "Language basics": the list of rules that differ gains "a number has no default value: `u8::default()` does not exist, and a register of a number must be given its first value", in the Book's words. "Your own types": "A value that may be absent: `Option`" and "Optional parameters" say absence is always an `Option`. "Traits and generics": the `Default` row (line 1199) as D4; a short section on operator traits (`impl BitAnd for ..`) with one worked type. `docs/book/build.py`: `Unknown` leaves `CONSTS`.
**Where**: `docs/book/content.html`
**Depends on**: T12
**Reuses**: `tests/book/language.rs`, `types.rs`, `generics.rs`
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_language::the_book_says_numbers_have_no_default` passes
- [x] `book_types`, `book_generics`, `book_inventory` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): numbers have no default, operators are traits`

---

### T14: Book: natures, capabilities and driving with strength

**What**: "Natures and carriers": the start of a carrier, `prev()` and edges as D8 and D9 (a clock that starts high has not risen; `init()` for an action at the start); a section "What a nature can do: traits as capabilities" (an `impl` on a nature gives its carriers methods, `abc.celsius()`; a trait a nature implements is something its carriers can do; one driver unless the nature implements `Resolve`, with the `E0220` message quoted). "Digital behavior", "Many drivers: resolved nets" is rewritten around a nature the reader declares, and gains the section "Driving with strength" with the four worked nets of design.md, in this order: a plain bit into `AndWire`; the open-drain line with its pull-up (the table of outcomes, and what the line reads without the pull-up); the tri-state bus with two drivers and a keeper; the conflict of equal strengths, and beside it the loop that does not settle (`E0511`), so the reader sees a conflict is a value and an instability an error. Prose answers "why is `resolve` a function and not a module" and "why is there no `Wire` in the library". `Pfd3` with an initial `state`. Fundamentals first (examples 1 and 2), In depth after (3, 4, `Resolve<X, V>` with two types).
**Where**: `docs/book/content.html`
**Depends on**: T13
**Reuses**: `tests/book/natures.rs`, `digital.rs`; `GAPS.md` L21 rows natures#3, digital#16; design.md "The worked nets"
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_natures::the_book_drives_nets_with_strength` passes
- [x] Each worked net has its inventory row; all four name L21 and their tests assert today's refusal text
- [x] `book_digital`, `book_inventory`, `book_claims` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): natures have capabilities, driving with strength`

---

### T15: Book: analog, mixed-signal and simulation listings

**What**: "Analog behavior", "Mixed-signal" and "Inside a simulation": every listing with `known_or`, `known()`, `is_known` or `high()` reads the carrier directly and gives it an initial value; the sample-and-hold; the prose around `on settle` and around records loses "unknown" and `maybe`; the solver-options tables are untouched.
**Where**: `docs/book/content.html`
**Depends on**: T14
**Reuses**: `tests/book/analog.rs`, `mixed.rs`, `simulation.rs`
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_analog`, `book_mixed`, `book_simulation`, `book_inventory`, `book_claims` pass
- [x] Each replaced test is named in the commit body
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): analog and mixed listings read values directly`

---

### T16: Book: standard library, errors, CLI and cheat sheet

**What**: "Core and standard library": "`std::values`: unknown values, bit patterns and clocks" becomes "`std::values` and `std::logic`" (`same`, `cast`, `logic`, `Logic`, `rise`, `fall`, `init`, `every`). "Reading errors": `E0150`, `E0151`, `E0906` leave; `E0187` for a missing initial value and `E0511` enter. "The command line" and "Cheat sheet": how `None` and a `Logic` print; no `maybe` column.
**Where**: `docs/book/content.html`
**Depends on**: T15
**Reuses**: `tests/book/core_std.rs`, `errors.rs`, `cli.rs`, `cheatsheet.rs`; `crates/piperine-cli/tests/book.rs`
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_core_std`, `book_errors`, `book_cli`, `book_cheatsheet` pass
- [x] `cargo test -p piperine-cli --test book` passes
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): std, errors and cli pages in the explicit model`

---

### T17: Book: device ABI, solver and compiler chapters

**What**: "Device ABI": the contract as T10 wrote it, still major 1 and said to be unreleased (the four-field `Field`, the one-byte `BIT`, `Option` as presence and payload, `Logic` as two planes, the `driving` byte, one `commit`). "The solver" and "From source to solver": relaxation without zeroing, no merge of commits, no flagged type in the IR walk-through. "The object model": the `Unknown` literal leaves.
**Where**: `docs/book/content.html`
**Depends on**: T16
**Reuses**: `tests/book/abi.rs`, `solver.rs`, `compiler.rs`, `pom.rs`
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_abi::the_abi_chapter_describes_the_unreleased_contract` passes
- [x] `book_solver`, `book_compiler`, `book_pom` pass
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(book): the abi without known bytes, the solver without flags`

---

### T18: Book: the inventory holds, the removed names are fenced

**What**: Add to `tests/book/inventory_guard.rs` a test that no block and no claim of `inventory.tsv`, and no text of `content.html`, uses `Unknown`, `known_or`, `known()`, `is_known`, `level(`, `high()` or `maybe`; and one that every row with verdict "refused" or "unraised" names a `GAPS.md` row that exists. Regenerate the fingerprints. Rebuild the Book with `build.py`.
**Where**: `tests/book/inventory_guard.rs`
**Depends on**: T17
**Reuses**: `tests/book/inventory_guard.rs`, `claims_guard.rs`
**Requirement**: EU-18
**Executor**: Sonnet

**Done when**:

- [x] `book_inventory::every_listing_that_does_not_run_names_its_gap` passes, and `the_book_uses_no_removed_name` passes with its two named `E0150` exemptions, which T30 removes (C79 is proved there)
- [x] All 24 `book_*` targets pass
- [x] Gate check passes: Batch

**Tests**: integration
**Gate**: full

**Commit**: `test(book): no removed name, every refused listing names its gap`

---

### T19: Contract: rebase onto `main` with `composite-values`

**What**: Rebase the phase A branch onto `main` after `composite-values` merges. Resolve `content.html`, `inventory.tsv`, `GAPS.md` and `spec_text.rs` by design.md "Risks". Re-read every file of design.md "What exists" at the new `HEAD` and write on the Handoff each anchor that moved, with the task it affects, and whether `composite-values` T52 was amended in place.
**Where**: `.specs/features/explicit-unknowns/tasks.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: design.md "Sequencing"
**Requirement**: EU-21
**Executor**: Sonnet

**Done when**:

- [x] The `**Build**` line passes (zero warnings); `cargo test` at the root runs once at the end of B5 (user gate policy)
- [x] The Handoff names every moved anchor
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): explicit-unknowns contract re-read after the rebase`

---

### T20: Checker: `analog` reads a binary carrier directly

**What**: Delete `types/flag_total.rs` and the rule of `types/analog.rs` that refuses a read outside `level` or a flag-total function (`E0150`). A discrete carrier read in `analog` is typed like any expression. Create `types_explicit.rs` from `types_unknowns.rs`: its `E0150` tests become tests that the same sources are accepted.
**Where**: `crates/piperine-lang/src/types/analog.rs`
**Depends on**: T19
**Reuses**: `Checker` in `types/check.rs`
**Requirement**: EU-01
**Executor**: Sonnet

**Done when**:

- [x] `types_explicit.rs` exists with a `//!` header; `types_unknowns.rs` is gone; each replaced test is named in the commit body
- [x] `source_explicit::a_binary_carrier_is_read_as_its_value` passes (the target is created here with this one test)
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): analog reads a discrete carrier as its value`

---

### T21: Checker: the `Default` table

**What**: `Defaults`: `Default` holds for `()`, `Option<T>`, tuples and arrays of types that have it, and a struct that derives it; it does not hold for `bool`, the integers, the floats and `str`. `#[derive(Default)]` over a field without it, and `T::default()` on a type without it, are `E0187` naming the field or the type.
**Where**: `crates/piperine-lang/src/types/compound.rs`
**Depends on**: T20
**Reuses**: the derive table `composite-values` T44 built
**Requirement**: EU-05
**Executor**: Sonnet

**Done when**:

- [x] `types_explicit::the_types_that_have_a_default`, `deriving_default_over_a_number_is_e0187`, `no_primitive_numeric_type_has_a_default` pass
- [x] Every source in the tree that called a numeric `default()` is rewritten with a literal, listed in the commit body
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): no primitive numeric type implements default`

---

### T22: Elaborator: a carrier starts at its written value or its default

**What**: `InitialValue`: every carrier pin gets init bytes, from `= value` or from `Default::default()` of its value type (`None`, the derived struct). An `Option<T>` param no instance binds is `None`. An array element never committed reads its initial value (sparse storage kept). The commit of the initial value at `(0, 0)` is a change, and the `PREVIOUS` part of a carrier is seeded with the same bytes, so `prev()` reads the initial value until a later change (D8). A carrier of a type without `Default` and no value still elaborates here, with zero bytes; T30 refuses it. A composite carrier (`Bit<Pair>`, `Bit<(u8, u8)>`, `Bit<[T; N]>`, `Bit<Option<T>>`) is committed its initial value the same way, so the run fault of `GAPS.md` L64 cannot arise on a plain read.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: T21
**Reuses**: `elab/consts.rs`; the `HAS_INIT` pins of `piperine-plan`
**Requirement**: EU-04
**Executor**: Sonnet

**Done when**:

- [x] `source_explicit::a_written_initial_value_is_the_value_at_the_start`, `a_carrier_without_a_value_starts_at_its_default`, `an_element_never_committed_reads_its_initial_value`, `an_option_carrier_is_read_by_matching`, `the_first_commit_is_a_change`, `prev_is_the_initial_value_before_a_change` pass
- [x] C20, `source_explicit::an_unbound_option_param_is_none`, passes (delivered by `params-and-io` B5, GAPS L28; the interim `an_unbound_option_param_waits_for_params_and_io` is replaced by it)
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): a carrier starts at its value or its default`

---

### T23: `std` in the explicit form

**What**: `std/values.msl`: `Known` and `merge` deleted; `same` and `cast` as D10, D11. `std/carriers.msl`: `rise` and `fall` as D9, `high` deleted. `std/nets.msl`, `convert.msl`, `monitor.msl`, `analog.msl`: the text of Part VII as amended: `nets.msl` holds `Strength`, `Driven`, `strongest`, `sum` and the generic modules, and no `Wire`, `Tri` or `SumNet` (they stay refused with `P0001` until `resolved-nets`). Regenerate `fixtures/preformat/crates/piperine-lang/std/*.pre`. Write on the Handoff every model in the tree whose output moves because an edge no longer fires at `(0, 0)`.
**Where**: `crates/piperine-lang/std/carriers.msl`
**Depends on**: None (its phase follows the previous one)
**Reuses**: Part VII §22.1.5 as T7 wrote it
**Requirement**: EU-13
**Executor**: Sonnet

**Done when**:

- [x] `source_explicit::a_change_is_a_different_value`, `an_edge_is_a_transition`, `a_carrier_that_starts_high_has_not_risen` pass
- [x] `std_excerpts::every_fence_is_an_excerpt_of_std` and `std_library` pass
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(std): edges are transitions, the flag traits are gone`

---

### T24: `examples/buck` in the explicit form

**What**: Rewrite `examples/buck/src/{control,dpwm,power,supervisor,top}.msl` and `benches/startup.msl`: each carrier read through `known_or(d)` gets `= d` as its initial value and is read directly; `high()` becomes the read; a `known()` match becomes the read. Regenerate the buck's preformat and elab fixtures. Run every buck target (the on-demand suite): no waveform assert moves.
**Where**: `examples/buck/src/supervisor.msl`
**Depends on**: T23
**Reuses**: design.md "Risks" row 2
**Requirement**: EU-21
**Executor**: Sonnet

**Done when**:

- [x] Every buck target of `piperine-parity` and `cargo test -p piperine-cli --test buck` pass with no expected value edited
- [x] The only asserts rewritten are on a known byte, listed in the commit body
- [x] Gate check passes: Full, with the buck

**Tests**: integration
**Gate**: full

**Commit**: `refactor(buck): carriers start with values, reads are direct`

---

### T25: The e2e packages in the explicit form

**What**: Rewrite `tests/e2e/integrated/src/{lib,pi_loop,state_machine,threshold}.msl` and the buck scenarios under `tests/e2e/buck`: initial values for what was read through `known_or`, `&& x.prev().is_known()` deleted (D9). The twins under `tests/e2e/twins` are not edited.
**Where**: `tests/e2e/integrated/src/lib.msl`
**Depends on**: T24
**Reuses**: T23's Handoff list
**Requirement**: EU-21
**Executor**: Sonnet

**Done when**:

- [x] `cargo test -p piperine --test integrated` and `cargo test -p piperine-cli --test integrated` pass with no expected value edited
- [x] `git diff --stat tests/e2e/twins` is empty
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(e2e): the integrated models in the explicit form`

---

### T26: The parity and CLI fixtures in the explicit form

**What**: Rewrite the models under `crates/piperine-parity/tests/{digital,events,fixes,structure,natures,run_errors,buck}/`, `crates/piperine-cli/tests/projects/`, and `crates/piperine-lang/tests/fixtures/gallery/` that use a removed name or leave a binary carrier without a value. Regenerate `fixtures/preformat/**` and `fixtures/elab/buck.txt`.
**Where**: `crates/piperine-parity/tests/digital/time.msl`
**Depends on**: T25
**Reuses**: the spelling of T24 and T25
**Requirement**: EU-20
**Executor**: Sonnet

**Done when**:

- [x] Every `source_*` target of `piperine-parity` and `cargo test -p piperine-lang` pass
- [x] No expected value of a source test is edited; a test whose meaning was an unknown is listed on the Handoff for T27
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(tests): fixtures give carriers their first value`

---

### T27: `source_unknowns` becomes `source_explicit`

**What**: Move the 15 models of `crates/piperine-parity/tests/unknowns/` to `tests/explicit/` and fold `source_unknowns.rs` into `source_explicit.rs`: each test is replaced by the test of the criterion that supersedes it (a merge of two arms becomes the one-path test on the same source; a `known_or` read becomes the direct read). Add to `suite_hygiene.rs` the walk for removed names and the table of successors (old test name, new test name).
**Where**: `crates/piperine-parity/tests/source_explicit.rs`
**Depends on**: T26
**Reuses**: design.md "Goldens, pins and tests that may move"
**Requirement**: EU-20
**Executor**: Sonnet

**Done when**:

- [x] `suite_hygiene::no_source_uses_a_removed_unknown_form` and `every_unknown_test_has_a_successor` pass
- [x] `source_unknowns.rs` and `tests/unknowns/` are gone; the commit body lists every old and new name
- [x] Gate check passes: Batch

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the unknown tests become explicit-model tests`

---

### T28: Lexer and parser: `Unknown` is no token

**What**: Remove `Token::Unknown`, `LitKind::Unknown` and their arms from `msl/lexer.rs`, `msl/ast.rs`, the parser, the printer, the formatter and `pom`. `Unknown` lexes as an identifier and resolves to nothing: `E0161`, with the `RemovedName` help. `E0906` and `E0908` are raised nowhere. The `LitKind::Unknown` statement of `docs/pom/mapping.md` leaves with it, and so does its entry in `pom_docs::DocAheadOfTheCode::WAITING`.
**Where**: `crates/piperine-lang/src/msl/lexer.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the `Clone`/`Copy` help of `value-semantics` T13
**Requirement**: EU-02
**Executor**: Sonnet

**Done when**:

- [x] `types_explicit::unknown_is_a_name_that_does_not_resolve` passes
- [x] `msl_lexer`, `msl_parser`, `msl_errors`, `msl_print`, `pom_resolve` pass with their `Unknown` cases replaced
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(parser): unknown is a name, not a literal`

---

### T29: Checker: the flag methods are gone

**What**: Delete `level`, `known`, `known_or`, `is_known`, `high` and `merge` from `types/intrinsic.rs` and `elab/intrinsics.rs`; a call is `E0161` naming the method and the type, with the help. `logic` and `matches_bits` are refused by name with `P0001` (`GAPS.md` L58) until T45. The flag reads of a composite carrier and of an `Option` (`composite-values` T57, `Typing::read_of` in `piperine-ir/src/typing.rs`, `lower_flag_receiver` in `elab/known.rs`, `GAPS.md` L65) leave with the methods. The `level` statement of `docs/pom/mapping.md` and its `pom_docs::DocAheadOfTheCode::WAITING` entry leave with it.
**Where**: `crates/piperine-lang/src/types/intrinsic.rs`
**Depends on**: T28
**Reuses**: `RemovedName`
**Requirement**: EU-02
**Executor**: Sonnet

**Done when**:

- [x] `types_explicit::the_flag_methods_do_not_resolve` passes
- [x] `cargo test -p piperine-lang` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): level, known_or and merge do not resolve`

---

### T30: Checker: a missing initial value is `E0187`

**What**: Refill the two E0150 output blocks of the Book (welcome#19 and the buck's) with the real run now that the front end no longer prints `E0150`, and take the exemption out of `the_book_uses_no_removed_name` (its exempt count goes to 0, so EU-18 AC 3 holds). A signal, an `out` port, a drive of a resolved carrier, and a one-driver carrier with no drive, whose value type does not implement `Default` and that has no initial value: `E0187` at the declaration, naming the carrier and `Default`, once per declaration (an array of carriers is one). A top-level `in` port is left to the host (T38).
**Where**: `crates/piperine-lang/src/types/carrier.rs`
**Depends on**: T29
**Reuses**: `Defaults` (T21), `InitialValue` (T22)
**Requirement**: EU-04
**Executor**: Sonnet

**Done when**:

- [x] `types_explicit::a_missing_initial_value_is_e0187_naming_the_carrier` and `an_undriven_carrier_without_default_is_e0187` pass
- [x] `types_explicit::a_definition_or_a_child_out_port_specifies_the_value` passes: no `E0187` for a carrier an undelayed definition drives or a child's `out` port holds, `E0187` for a delayed definition without an initial value (C92)
- [x] `book_inventory::the_book_uses_no_removed_name` passes with no exempt block (C79)
- [x] The tree builds: T23 to T27 left no such carrier
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): a carrier without a value needs default`

---

### T31: Elaborator: no expression is flagged

**What**: Delete `elab/known.rs` and every use: the merge lowering in `compound.rs`, `lower.rs`, `events.rs`, `variants.rs`, `selectors.rs`, `derivatives.rs`; the unknown index; `E0151`; the refusal of an unknown struct part; `guard_may_be_unknown` and `decides_on_unknown` in `piperine-plan/src/digital.rs`. `elab_unknowns.rs` becomes `elab_explicit.rs`. The elaborator builds no `ValueType::Flagged`.
**Where**: `crates/piperine-lang/src/elab/known.rs`
**Depends on**: T30
**Reuses**: design.md "What exists"
**Requirement**: EU-01
**Executor**: Sonnet

**Done when**:

- [x] `source_explicit::control_on_a_carrier_runs_one_path`, `settle_runs_as_before`, `an_event_read_is_a_bool`, `a_false_guard_runs_nothing` pass
- [x] `suite_hygiene::retired_unknown_codes_are_raised_nowhere` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(elab): no expression may be unknown`

---

### T32: IR: `ValueType::Flagged` is removed

**What**: Remove `ValueType::Flagged` and its rules from `model.rs`, `typing.rs`, `check.rs`, `deps.rs`, `print.rs`, and the flag forms from `piperine-interp`'s refusal list. Regenerate `crates/piperine-ir/tests/golden/buck/**` and `fixtures/elab/buck.txt` once: the only diff allowed is a flagged type becoming its plain type.
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: T31
**Reuses**: `value-semantics` T12 as the form of an IR removal
**Requirement**: EU-01
**Executor**: Sonnet

**Done when**:

- [x] `cargo test -p piperine-ir` passes; `digital::no_value_type_is_flagged` passes
- [x] The golden diff is reviewed line by line and described in the commit body
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(ir): no flagged value type`

---

### T33: Generated code: no flag and no merge

**What**: Delete `rt/flag.rs` (with `Flagged::recast` and `known_or_refuse`, and its test `a_tag_naming_no_variant_reads_unknown`: a payload enum cast is plain) and the merge of `rt/control.rs`; the composite read prints the value, never `.known_or(Default::default())` and never `Flagged::<T>::decode(..).known_or_refuse(..)` (`print/value.rs`, `print/digital.rs`, `print/analog.rs`); remove the flagged paths of `print/value.rs`, `shape.rs`, `cross.rs`, `digital.rs`, `reaction.rs`, `analog.rs`, `site.rs`, `contribution.rs` and of `rt/real.rs`. `analog_merge.rs` and the merge tests of `reaction.rs` and `amended.rs` are replaced by one-path tests. Regenerate the transpile goldens once: the only diff allowed is the removal of flag and merge code.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T32
**Reuses**: `crates/piperine-transpile/tests/golden.rs`
**Requirement**: EU-01
**Executor**: Sonnet

**Done when**:

- [x] `cargo test -p piperine-transpile` passes
- [x] The golden diff is described in the commit body; every buck target passes
- [x] Gate check passes: Batch, with the buck (the generated code moved)

**Tests**: integration
**Gate**: full

**Commit**: `refactor(transpile): generated code runs one path`

---

### T34: Records hold their declared fields

**What**: Remove the `maybe` field from `RecordLayout` (`piperine-plan/src/digital.rs`), `Record::surely` and the unknown verdict from `piperine-api/src/{record,verdict,recorder}.rs`, the column from `piperine-cli/src/render/{record,json}.rs`, and the reservation of the name from `types/record.rs` (`E0604` keeps "repeated").
**Where**: `crates/piperine-api/src/record.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `crates/piperine-api/tests/records.rs`
**Requirement**: EU-15
**Executor**: Sonnet

**Done when**:

- [x] `records::a_record_holds_its_declared_fields_only`, `a_require_is_a_pass_or_a_fail`, `a_fatal_record_ends_the_run` pass
- [x] `types_explicit::maybe_is_an_ordinary_field_name` passes
- [x] Record goldens lose the `maybe` column and nothing else
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(records): a record holds its declared fields only`

---

### T35: ABI: one `commit`

**What**: Remove `Host.commit_with`, `commit_flag` and their thunks from `piperine-abi/src/{raw,caller,thunk,device}.rs`; the `MAYBE` conversion and `MERGE` application from `piperine-digital-solver/src/queue.rs`; `layout::Merge`; the callers in `piperine-transpile/src/rt/commits.rs`.
**Where**: `crates/piperine-abi/src/raw.rs`
**Depends on**: T34
**Reuses**: abi.md C1 to C6
**Requirement**: EU-16
**Executor**: Sonnet

**Done when**:

- [x] `contract::the_host_has_one_commit` passes
- [x] `cargo test -p piperine-abi -p piperine-digital-solver` passes with the `MERGE` tests of `queue.rs` and `instant.rs` replaced by plain-commit tests
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `refactor(abi): one commit entry, no commit flags`

---

### T36: ABI: layouts hold the value only, still major 1

**What**: `Field` becomes `{ name, leaf, offset, count }`; `HAS_KNOWN` and `known_of` go; `BIT`, `BYTE` and `F64` are the shipped layouts and `REAL` is deleted; `MAJOR` stays 1 and the symbol `piperine_abi_1`, `MINOR` becomes 0; `validate.rs` and `verify.rs` follow. `piperine-plan` lays every carrier as its value leaf and `Option<T>` as presence then payload; `CompositeLayout` (`piperine-plan/src/composite.rs`) drops its one trailing known byte: a composite pin is the parts' value bytes packed, alignment 1, size = width, and every field of it keeps its own offset (`composite-values` T56 laid it as width + 1); the composite `Params` bytes (`ParamValue::Bytes` of a composite param) follow; the printer's shapes, `piperine-solver/src/core/bind.rs` and every hand-written test device follow. The cache key of `piperine-transpile/src/cache.rs` holds a hash of the contract's source, so an entry built against the earlier contract is rebuilt. This is the one task whose change spans crates: no layer above produces a flag any more, so it is one byte dropped per leaf.
**Where**: `crates/piperine-abi/src/layout.rs`
**Depends on**: T35
**Reuses**: abi.md as T10 wrote it; `piperine-transpile/src/abi_source.rs`
**Requirement**: EU-16
**Executor**: Sonnet

**Done when**:

- [x] `layouts::the_shipped_layouts_hold_the_value_only` and `an_option_pin_is_presence_then_payload` pass
- [x] `cache::an_entry_of_the_earlier_contract_is_rebuilt` passes
- [x] The `**Build**` line passes; every buck target passes with its known-byte asserts rewritten to the value
- [x] Gate check passes: Full, with the buck

**Tests**: integration
**Gate**: full

**Commit**: `feat(abi): a pin holds the bytes of its type, still major 1`

---

### T37: Digital solver: relaxation on a plain `driving` byte

**What**: A drive pin is the bytes of its driven type and then `driving: BOOL`, 0 for absent. Each pass of a relaxing cycle starts with `driving` 0 on the cycle's drives. A net that still moves after the bounded passes is a `FAULT` the owner reports as `E0511` naming the loop's drives; `ObservationKind::Unsettled` and the zeroing of a moving field go.
**Where**: `crates/piperine-digital-solver/src/settle.rs`
**Depends on**: T36
**Reuses**: `resolved-nets` T7 to T9
**Requirement**: EU-16
**Executor**: Sonnet

**Done when**:

- [x] `relax::a_drive_pin_ends_with_a_plain_driving_byte` passes
- [x] The `relax.rs` tests of zeroing are replaced by the `E0511` test, both names in the commit body
- [x] `cargo test -p piperine-digital-solver -p piperine-solver` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(digital-solver): a loop that does not settle is e0511`

---

### T38: Host: a cell is a value

**What**: `piperine-api/src/{table,probe,plot,request,backend}.rs`: no unknown cell, and `CellShape::decode` (`piperine-api/src/composite.rs`) reads the composite pin without its known byte; `None` is a cell of its own. A top-level `in` port with no initial value, no `Default` and no host value when an analysis starts refuses the analysis with `E0187` naming the port.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T37
**Reuses**: `crates/piperine-api/tests/session.rs`, `probes.rs`
**Requirement**: EU-04
**Executor**: Sonnet

**Done when**:

- [x] `session::an_undriven_top_port_without_default_refuses_the_analysis` passes
- [x] `cargo test -p piperine-api` passes with the unknown-cell tests replaced
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a cell is a value, a top port needs its first value`

---

### T39: CLI: output without unknown

**What**: `piperine-cli/src/{cli,commands/run}.rs` and `render/*`: every format prints `None` for an absent value and has no unknown cell and no unknown verdict; the help text and `docs.rs` follow.
**Where**: `crates/piperine-cli/src/render/json.rs`
**Depends on**: T38
**Reuses**: `crates/piperine-cli/tests/run.rs`
**Requirement**: EU-15
**Executor**: Sonnet

**Done when**:

- [x] `run::an_option_cell_prints_none_or_the_value` passes
- [x] `cargo test -p piperine-cli` passes
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(cli): none prints as none, no unknown cell`

---

### T40: Phase A closure

**What**: `GAPS.md`: S8 removed; the clauses of L3, L5, L24 about unknown indexes and loops removed; L64 (a plain read of an unknown composite carrier is a run fault) and L65 (a flag read on an `Option`) removed, both owned by this phase (`composite-values` verifier: T22 starts every carrier at a value, T29 deletes the flag reads); L59 keeps its non-literal `map`/`and_then`/`then` closure and non-literal `expect` message clauses (the code still refuses them with `P0001`), its unknown clause is gone; the conformance rows updated; counts. Appendix G.72 loses "the reference implementation does not meet this yet" for everything but `Logic`, which names phase B. Book status boxes: the listings that now run flip, each with its test. **The Book rows T40 flips, exactly these** (inventory verdict `refused` or `unraised` becomes `run` or `holds`, the GAPS.md section 11 row is deleted, the test that asserted the refusal asserts the run): the E0150 listings welcome#17, analog#9, modules#9, mixed#8, mixed#10, mixed#14, values#21 (T20; their `Model::earlier` edits; the two E0150 output blocks welcome#19 and the buck's were refilled in T30); the E0187 and Default rows values~23c03fc8, values~39ba3a1d, errors~39ba3a1d, errors~385c354c (T21, T22, T30) and values~37a548f2 (T38: the analysis refuses an undriven top `in` port without a `Default`); and the `Option` listings values#14 and its sentence values~2bbc575e (an `Option` carried on a signal). `.specs/STATE.md`: MD-78's status line. `code_owners.tsv`.
**Where**: `GAPS.md`
**Depends on**: T39
**Reuses**: `value-semantics` T15
**Requirement**: EU-19
**Executor**: Sonnet

**Done when**:

- [x] `docs.rs::gaps_closes_s6_and_tracks_the_flag` passes
- [x] `twins::every_twin_matches_its_golden` and both integrated suites pass
- [x] `grep -rE 'Flagged|HAS_KNOWN|commit_with|MAYBE' crates/*/src` finds nothing
- [x] Gate check passes: Batch

**Tests**: integration
**Gate**: full

**Commit**: `docs(specs): explicit-unknowns phase a closes s8`

---

### T41: Contract: rebase onto `main` with `generics` phase A

**What**: Rebase the phase B branch onto `main` after `generics` phase A merges. Re-read `types/` (the `ImplTable`, trait dispatch, generic structs) and write on the Handoff where an operator is typed today and which `generics` unit T42 extends.
**Where**: `.specs/features/explicit-unknowns/tasks.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `generics` T24, T28, T31, T36
**Requirement**: EU-21
**Executor**: Sonnet

**Done when**:

- [ ] The `**Build**` line and `cargo test` at the root pass
- [ ] The Handoff names the units
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): explicit-unknowns phase b contract re-read`

---

### T42: Checker: operators through the operator traits

**What**: `OperatorTraits`: `&`, `|`, `^`, `!`, `<<`, `>>`, `==`, `!=` on a struct or an enum operand resolve to the one `impl` of the H.10 trait for the two operand types and take its `Out`; none is `E0180`; two `impl`s for one type and one `Rhs` are `E0187`. A trait may have type parameters with defaults; `PartialEq` alone is `PartialEq<Self, bool>`. `From<T>` joins the built-in traits. The built-in operand types keep the §1.5 path. Create `types_operators.rs`.
**Where**: `crates/piperine-lang/src/types/expr.rs`
**Depends on**: T41
**Reuses**: the `ImplTable` of `generics` T24
**Requirement**: EU-08
**Executor**: Sonnet

**Done when**:

- [ ] `types_operators::control_refuses_a_logic_and_an_option`, `analog_refuses_a_bare_logic`, `arithmetic_and_ordering_refuse_a_logic`, `deriving_partial_eq_over_a_logic_is_e0187` pass (against a test-local four-state struct until T44)
- [ ] `cargo test -p piperine-lang` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(types): operators on a library type are trait functions`

---

### T43: Elaborator: an operator on a library type is a call

**What**: An operator the checker resolved to a trait `impl` lowers to the call of that instance's function (`generics` T36's dispatch); a compound assignment lowers through the binary form.
**Where**: `crates/piperine-lang/src/elab/compound.rs`
**Depends on**: T42
**Reuses**: `generics` T36
**Requirement**: EU-08
**Executor**: Sonnet

**Done when**:

- [ ] A source test in `source_logic.rs` (created here) runs a test-local struct with `impl BitAnd` and reads the result
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): an operator on a library type lowers to its call`

---

### T44: `std/logic.msl`: `Logic<N>` from source

**What**: Add `std/logic.msl` with the source of design.md "Part VII §22.1": `Logic`, `LogicState`, `Default`, the consts, the tests, `at`, `to_bits`, `to_bit`, `to_volts`, the operator `impl`s and the mixed forms, `From`. Re-export from the prelude. `Logic` is in no built-in table. `std_excerpts.rs` drops the §22.1 fences from its ahead-of-`std` list.
**Where**: `crates/piperine-lang/std/logic.msl`
**Depends on**: T43
**Reuses**: Part VII §22.1 as T7 wrote it
**Requirement**: EU-07
**Executor**: Sonnet

**Done when**:

- [ ] `types_operators::logic_is_compiled_from_the_library_source` passes
- [ ] `source_logic::logic_lays_out_two_planes` and `at_gives_the_state_of_one_level` pass
- [ ] `types_explicit::the_types_that_have_a_default` gains the `Logic<N>` case
- [ ] `std_excerpts` and `std_library` pass
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(std): logic with four states, from library source`

---

### T45: `logic` and `matches_bits` over `Logic<N>`

**What**: `logic(p)` gives a `Logic<N>` from a pattern of `0`, `1`, `x`, `z`, highest level first; `matches_bits(s, p)` takes a `Logic<N>`, where `x` in the pattern matches any state. Both stay `extern "intrinsic"` (they read the characters of a `str`); any other character or length is `E0702`.
**Where**: `crates/piperine-lang/src/elab/intrinsics.rs`
**Depends on**: T44
**Reuses**: §22.1.4
**Requirement**: EU-07
**Executor**: Sonnet

**Done when**:

- [ ] `source_logic::logic_reads_a_four_state_pattern` passes
- [ ] `GAPS.md` L68 (`Logic<N>`, `logic`, `matches_bits`) is closed
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): logic patterns build four-state values`

---

### T46: `Logic` runs from source: tables, comparison, conversions, analog

**What**: The models of `crates/piperine-parity/tests/logic/*.msl` and their tests: the three binary tables over the 16 pairs of states, `!`, shifts, the mixed forms, `==`, `same`, the four tests, `From`, `to_bits`, `to_bit`, `to_volts` driving a node, `Option<Logic<4>>`, a `Bit<Logic<4>>` that starts `xxxx`, for `N` of 1, 4, 64 and 100. Expected values are the tables of the specification written in the test.
**Where**: `crates/piperine-parity/tests/source_logic.rs`
**Depends on**: T45
**Reuses**: `source_explicit.rs`
**Requirement**: EU-08
**Executor**: Sonnet

**Done when**:

- [ ] Every `source_logic` test that `## Checks` assigns to T46 passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): logic follows its four-state tables from source`

---

### T47: Host and plan: a `Logic` pin and a `Logic` cell

**What**: `LogicCell`: the host prints a `Logic<N>` as `N` characters, highest level first, in tables, records and JSON Lines, and parses the same pattern for a host write. The plan lays a `Logic<N>` as the struct it is (two `UInt<N>` leaves); a test pins the layout for `N` of 4 and 64.
**Where**: `crates/piperine-api/src/table.rs`
**Depends on**: T46
**Reuses**: the struct layout of `composite-values` T56
**Requirement**: EU-15
**Executor**: Sonnet

**Done when**:

- [ ] `run::a_logic_cell_prints_its_pattern` passes
- [ ] `layouts::a_logic_pin_is_two_planes` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a logic cell prints its four-state pattern`

---

### T48: Phase B closure

**What**: Remove the checker's own `cast` and `same` (`Intrinsic::{Same, Cast}` in `types/intrinsic.rs`, `elab/intrinsics.rs`, `lower_cast`) and compile them from `std/values.msl` now that `generics` A is on `main`, a differential test running the stdlib source against the lowering first; the `cast` to a tuple or an array holding an enum then runs. `GAPS.md`: L58's `cast` and `same` entries and L68 (`Logic`, `logic`, `matches_bits`); counts. Appendix G.72 loses its last "does not meet" sentence. Book: **exactly these rows flip** to running, each with its test and its GAPS.md section 11 row deleted: values#18 and its sentences values~c30bad4d, (the three E0180 rows values~13ffaea6, analog~39ba351d, mixed~39ba351d already hold on `main`: `Logic` is a struct type since `composite-values` CV-09, so a `Logic` where an `f64` or a `bool` is required is `E0180` today and no row of T48 flips them); the `Logic<4>` and `Option<Logic<N>>` lines of natures#2 run with the rest of that block only when `resolved-nets` has flipped it (it needs the array of `composite-values` and `Sda`'s `Resolve<Driven>`); T48 re-pins the row. `.specs/STATE.md`: MD-78 status delivered. A note in `.specs/features/resolved-nets/tasks.md` Handoff that `Logic` runs, that its spec is to be revised for `Resolve<X, V>` before B3, and that T27 starts from `Driven`.
**Where**: `GAPS.md`
**Depends on**: T47
**Reuses**: T40
**Requirement**: EU-19
**Executor**: Sonnet

**Done when**:

- [ ] `cast` and `same` compile from `std/values.msl`; `Intrinsic::Cast` and `Intrinsic::Same` are gone from the tree
- [ ] All `book_*` targets pass
- [ ] `twins::every_twin_matches_its_golden` and both integrated suites pass
- [ ] Gate check passes: Batch

**Tests**: integration
**Gate**: full

**Commit**: `docs(specs): explicit-unknowns phase b, logic runs`

---

## Checks

One check per acceptance criterion. `(task)` is the task that makes the check pass. A check of phase B is red until B8; a check of phase A until its batch.

**C1** - A plain read of a `Bit<u8>`, a struct and an array carrier yields the value in a reaction, a function and `analog` (EU-01, AC 1; T20)
Proof: run src | test a_binary_carrier_of_any_shape_is_read_as_its_value_everywhere ... ok

**C2** - An `if`, a `match`, a guard and `&&` on a carrier's value run one path (EU-01, AC 2; T31)
Proof: run src | test control_on_a_carrier_runs_one_path ... ok

**C3** - The IR builds no flagged value type and the generated reaction holds no merge (EU-01, AC 3; T32)
Proof: run src | test the_buck_elaborated_from_source_holds_no_flag_and_no_merge ... ok

**C4** - `Unknown` is `E0161` with a help naming an initial value, `Option<T>` and `Logic` (EU-02, AC 1; T28)
Proof: run types | test unknown_is_a_name_that_does_not_resolve ... ok

**C5** - `level`, `known`, `known_or`, `is_known`, `high` and `merge` are `E0161` naming the method and the type (EU-02, AC 2; T29)
Proof: run types | test the_flag_methods_do_not_resolve ... ok

**C6** - An `emit!` with a field named `maybe` elaborates and the record holds it (EU-02, AC 3; T34)
Proof: run src | test maybe_is_an_ordinary_field_of_a_record ... ok

**C7** - No source raises `E0150`, `E0151`, `E0906` or `E0908` (EU-02, AC 4; T31)
Proof: run hygiene | test retired_unknown_codes_are_raised_nowhere ... ok

**C8** - A `Logic` or an `Option` as a condition, a guard or a short-circuit operand is `E0180` naming the type and `bool` (EU-03, AC 1; T42)
Proof: run ops | test control_refuses_a_logic_and_an_option ... ok

**C9** - `analog` reads a `Logic` carrier through a function that returns `f64` or `bool` (EU-03, AC 2; T46)
Proof: run logic | test analog_reads_a_logic_through_a_function ... ok

**C10** - A `Logic` where `analog` needs an `f64` or a `bool` is `E0180` (EU-03, AC 3; T42)
Proof: run ops | test analog_refuses_a_bare_logic ... ok

**C11** - A written initial value is committed at `(0, 0)` (EU-04, AC 1; T22)
Proof: run src | test a_written_initial_value_is_the_value_at_the_start ... ok

**C12** - A carrier of `Bit<Option<u8>>` with no initial value starts at `None` (EU-04, AC 2; T22)
Proof: run src | test a_carrier_without_a_value_starts_at_its_default ... ok

**C13** - An element of `[Bit<Option<u8>>; 1 << 20]` never committed reads as its initial value, with storage for the written elements only (EU-04, AC 3; T22; proved on the real array `[Bit<Option<u8>>; 1 << 20]` by composite-values phase C, T86 (`test(parity): EU-04 AC 3 on an array of carriers`, GAPS L24 closed); its storage half is `piperine-api`'s `carrier_array_storage`)
Proof: run src | test an_element_never_committed_reads_its_initial_value ... ok

**C14** - `signal en: Bit;` and an `out` port with no value on a type without `Default` are `E0187` naming the carrier and `Default` (EU-04, AC 4; T30; the drive half of the criterion, a drive of a resolved carrier, is owed by `resolved-nets` under GAPS L21: no resolved nature runs yet, so none can be tested here)
Proof: run types | test a_missing_initial_value_is_e0187_naming_the_carrier ... ok

**C15** - A one-driver carrier with no drive and no `Default` is `E0187` (EU-04, AC 5; T30)
Proof: run types | test an_undriven_carrier_without_default_is_e0187 ... ok

**C16** - A top-level `in` port with no value, no `Default` and no host value refuses the analysis with `E0187` naming the port (EU-04, AC 6; T38)
Proof: run session | test an_undriven_top_port_without_default_refuses_the_analysis ... ok

**C17** - `()`, `Option<T>`, tuples and arrays of those, and a deriving struct have `Default`; `Logic<N>` has it in phase B (EU-05, AC 1; T21)
Proof: run types | test the_types_that_have_a_default ... ok

**C18** - `#[derive(Default)]` on a struct with a `u8` field is `E0187` naming the field and its type (EU-05, AC 2; T21)
Proof: run types | test deriving_default_over_a_number_is_e0187 ... ok

**C19** - `u8::default()`, `bool::default()`, `f64::default()` and `str::default()` are `E0187` (EU-05, AC 3; T21)
Proof: run types | test no_primitive_numeric_type_has_a_default ... ok

**C20** - An `Option<T>` param no instance binds is `None` (EU-06, AC 1; T22; closed by `params-and-io` T31 and T32, GAPS L28: the criterion's own test runs `Param` placed with no value for `limit` and reads `given = false`; the elaborated form is `elab_params::an_unbound_option_param_is_none`)
Proof: run src | test an_unbound_option_param_is_none ... ok

**C21** - A `Bit<Option<u8>>` carrier is read with `match`, `if let` and `unwrap_or` (EU-06, AC 2; T22)
Proof: run src | test an_option_carrier_is_read_by_matching ... ok

**C22** - A carrier of `Option<Logic<4>>` holds `None` or `Some` of four four-state levels (EU-06, AC 3; T46)
Proof: run logic | test an_option_of_logic_is_none_or_a_whole_vector ... ok

**C23** - `Logic<N>` is declared in `std/logic.msl` as two `UInt<N>` planes and compiled from that source; no built-in table names it (EU-07, AC 1; T44)
Proof: run ops | test logic_is_compiled_from_the_library_source ... ok

**C24** - `Logic<4>` has `Repr` width 8: `binary` at levels 0 to 3, `value` at levels 4 to 7 (EU-07, AC 2; T44)
Proof: run logic | test logic_lays_out_two_planes ... ok

**C25** - `logic("10xz")` gives those four states; a pattern with another character or length is `E0702` (EU-07, AC 3; T45)
Proof: run logic | test logic_reads_a_four_state_pattern ... ok

**C26** - `x.at(k)` gives the `LogicState` of level `k` and a `match` covers it with four arms (EU-07, AC 4; T44)
Proof: run logic | test at_gives_the_state_of_one_level ... ok

**C27** - `&` on `Logic<4>` follows its table over the 16 pairs of states (EU-08, AC 1; T46)
Proof: run logic | test and_follows_the_four_state_table ... ok

**C28** - `|` on `Logic<4>` follows its table over the 16 pairs of states (EU-08, AC 2; T46)
Proof: run logic | test or_follows_the_four_state_table ... ok

**C29** - `^` on `Logic<4>` follows its table over the 16 pairs of states (EU-08, AC 3; T46)
Proof: run logic | test xor_follows_the_four_state_table ... ok

**C30** - `!` on `Logic<4>` gives 1, 0, X, X for 0, 1, X, Z (EU-08, AC 4; T46)
Proof: run logic | test not_follows_the_four_state_table ... ok

**C31** - `<<` and `>>` move the states and shift in 0 (EU-08, AC 5; T46)
Proof: run logic | test shifts_move_states_and_fill_with_zero ... ok

**C32** - `Logic<4> & u4`, `u4 & Logic<4>`, `Logic & bool` and `bool & Logic` give the table's `Logic` (EU-08, AC 6; T46)
Proof: run logic | test a_logic_and_a_binary_value_combine_level_by_level ... ok

**C33** - `+` and `<` with a `Logic` operand are `E0180` (EU-08, AC 7; T42)
Proof: run ops | test arithmetic_and_ordering_refuse_a_logic ... ok

**C34** - `a == b` on `Logic<4>` is X with any X or Z level, else 1 or 0; `!=` is its `!` (EU-09, AC 1; T46)
Proof: run logic | test equality_of_logic_is_a_logic ... ok

**C35** - `same(a, b)` is true exactly when the four states agree on every level (EU-09, AC 2; T46)
Proof: run logic | test same_compares_the_four_states ... ok

**C36** - `is_high`, `is_low`, `is_z` and `is_x` are true exactly for all-1, all-0, all-Z and all-X (EU-09, AC 3; T46)
Proof: run logic | test the_four_tests_of_a_logic ... ok

**C37** - `#[derive(PartialEq)]` on a struct with a `Logic` field is `E0187` naming the field (EU-09, AC 4; T42)
Proof: run ops | test deriving_partial_eq_over_a_logic_is_e0187 ... ok

**C38** - `Logic::from(0b1010u4)` and `Logic::from(true)` give binary levels (EU-10, AC 1; T46)
Proof: run logic | test a_binary_value_converts_to_logic ... ok

**C39** - `to_bits()` is `Some` for all-binary levels and `None` with an X or a Z; `to_bit()` alike (EU-10, AC 2; T46)
Proof: run logic | test a_logic_converts_to_an_option_of_bits ... ok

**C40** - `to_volts(1.8)` gives 1.8, 0.0 and 0.9 for 1, 0 and X or Z, and drives a node from `analog` (EU-10, AC 3; T46)
Proof: run logic | test to_volts_gives_the_level_of_each_state ... ok

**C41** - §3.1 and §3.7 say a nature takes trait `impl`s, that a trait is a capability, and that `E0134` is for a bundle only (EU-11, AC 1; T1)
Proof: run spectext | test a_nature_takes_trait_impls ... ok

**C42** - §4 declares `Resolve<X: Repr, V: Repr = X>` with its one function, `V` the nature's value (`E0203`), and no `resolve` field or keyword is left (EU-11, AC 2; T3)
Proof: run spectext | test resolve_is_a_trait_of_two_parameters ... ok

**C43** - §5.2 says many drives need `Resolve`, and that `E0220` names the nature and the trait (EU-11, AC 3; T3)
Proof: run spectext | test many_drives_need_the_resolve_capability ... ok

**C44** - §5.2.15 says a drive picks its `impl` by its type, with `E0225` and `E0180` (EU-11, AC 4; T3)
Proof: run spectext | test a_drive_picks_its_impl_by_its_type ... ok

**C45** - §4 rule 15 and §5.2.10 say `resolve` receives the present drives as plain values, for every count from zero (EU-11, AC 5; T3)
Proof: run spectext | test resolve_receives_plain_values_for_every_count ... ok

**C46** - §5.2.12 says a drive with no initial value starts at its type's default or is `E0187`, and that a resolved carrier needs none (EU-11, AC 6; T3)
Proof: run spectext | test a_drive_without_a_value_starts_at_its_default ... ok

**C47** - §8.2.3 says a relaxing drive starts each relaxation absent (the passes of one relaxation build on the previous pass), and that absence is not a value (EU-11, AC 7; T4)
Proof: run spectext | test a_relaxing_drive_starts_absent ... ok

**C48** - §8.2.3 step 4 and its error row say a loop that still moves is `E0511` naming its drives (EU-11, AC 8; T4)
Proof: run spectext | test a_loop_that_does_not_settle_is_e0511 ... ok

**C49** - §22.3 defines `Strength`, `Driven` and `Driven::strongest` with its four-row table (EU-12, AC 1; T7)
Proof: run spectext | test strongest_resolves_by_strength ... ok

**C50** - §22.3 declares no `Wire`, `Tri` or `SumNet`, and its modules are generic over `W: Nature + Resolve<Driven>` (EU-12, AC 2; T7)
Proof: run spectext | test the_library_declares_no_net_nature ... ok

**C51** - Appendix F.4 holds the four worked nets, each with nature, `impl` and module code (EU-12, AC 3; T9)
Proof: run spectext | test the_gallery_works_four_nets ... ok

**C52** - A commit of another value is a change; a commit of the held value is not (EU-13, AC 1; T23)
Proof: run src | test a_change_is_a_different_value ... ok

**C53** - `changed()` fires at `(0, 0)` for a carrier with a written value and for one at its default (EU-13, AC 2; T22)
Proof: run src | test the_first_commit_is_a_change ... ok

**C54** - `prev()` is the initial value until the first later change (EU-13, AC 3; T22)
Proof: run src | test prev_is_the_initial_value_before_a_change ... ok

**C55** - `rise()` fires on false to true and `fall()` on true to false (EU-13, AC 4; T23)
Proof: run src | test an_edge_is_a_transition ... ok

**C56** - A `Bit` that starts `true` fires no `rise()` at `(0, 0)`, and one that starts `false` no `fall()` (EU-13, AC 5; T23)
Proof: run src | test a_carrier_that_starts_high_has_not_risen ... ok

**C57** - `on settle` runs once per time point and its commits land at the next instant (EU-14, AC 1; T31)
Proof: run src | test settle_runs_as_before ... ok

**C58** - An event read as a `bool` is whether it fired in this microstep (EU-14, AC 2; T31)
Proof: run src | test an_event_read_is_a_bool ... ok

**C59** - A reaction whose guard is false does not run, and the plan marks no reaction as a maybe (EU-14, AC 3; T31)
Proof: run src | test a_false_guard_runs_nothing ... ok

**C60** - A stream's record layout is its declared fields, with no `maybe` (EU-15, AC 1; T34)
Proof: run records | test a_record_holds_its_declared_fields_only ... ok

**C61** - A `require` record is a pass or a fail (EU-15, AC 2; T34)
Proof: run records | test a_require_is_a_pass_or_a_fail ... ok

**C62** - A `fatal` record ends the run (EU-15, AC 3; T34)
Proof: run records | test a_fatal_record_ends_the_run ... ok

**C63** - A `Logic<4>` cell prints `10xz` in a table, a record and JSON Lines (EU-15, AC 4; T47)
Proof: run clirun | test a_logic_cell_prints_its_pattern ... ok

**C64** - An `Option` cell prints `None` or the value; no output format has an unknown cell (EU-15, AC 5; T39)
Proof: run clirun | test an_option_cell_prints_none_or_the_value ... ok

**C65** - The library exports `piperine_abi_1` with major 1, minor 0; a cache entry keyed by the earlier contract is rebuilt, never loaded (EU-16, AC 1; T36)
Proof: run cache | test an_entry_of_the_earlier_contract_is_rebuilt ... ok

**C66** - `Field` is `{ name, leaf, offset, count }`; `BIT` is size 1, `BYTE` size 1, `F64` size 8 align 8 (EU-16, AC 2; T36)
Proof: run abi | test the_shipped_layouts_hold_the_value_only ... ok

**C67** - `Host` offers `commit` and no `commit_with`; no `commit_flag` exists (EU-16, AC 3; T35)

*Amendment (composite-values phase C, QC1, approved by the user 2026-11-23):* `Host` is 24 bytes: it gains `commit_element` (an array pin's element commit, `piperine-abi` `the_host_has_one_commit` pins it), so EU-16 AC 2's "Host is 16 bytes, one commit" is superseded.
Proof: run abicontract | test the_host_has_one_commit ... ok

**C68** - An `Option<u8>` pin is a `BOOL` presence field then `U8` (EU-16, AC 4; T36)
Proof: run plan | test an_option_carrier_pin_is_presence_then_payload ... ok

**C69** - A `Logic<4>` pin is two `U8` fields, `binary` at 0 and `value` at 1, size 2, align 1; `Logic<64>` is size 16, align 8 (EU-16, AC 5; T47)
Proof: run abi | test a_logic_pin_is_two_planes ... ok

**C70** - A drive pin is the driven type's layout then `driving: BOOL`; 0 leaves the drive out of the resolver (EU-16, AC 6; T37)
Proof: run relax | test a_drive_pin_ends_with_a_plain_driving_byte ... ok

**C71** - Part I §3.3 holds the six rules of the explicit model and says no primitive numeric type implements `Default`, unlike Rust (EU-17, AC 1; T1)
Proof: run spectext | test section_3_3_states_the_explicit_model ... ok

**C72** - Part VII §22.1 holds the tables of `&`, `|`, `^`, `!`, `==` beside the `Logic` source (EU-17, AC 2; T7)
Proof: run spectext | test section_22_1_holds_the_four_state_tables ... ok

**C73** - No file of `docs/spec` outside G.72 holds a removed word (EU-17, AC 3; T9)
Proof: run spectext | test the_removed_words_are_absent_from_the_specification ... ok

**C74** - Appendix G holds G.72 and marks G.29, G.46, G.57 closed by it (EU-17, AC 4; T9)
Proof: run spectext | test appendix_g_records_the_explicit_model ... ok

**C75** - `abi.md` describes the contract without known bytes, with C7 and weak point 6 removed, and says the ABI is unreleased and changes in place (EU-17, AC 5; T10)
Proof: run docs | test abi_md_describes_the_unreleased_contract ... ok

**C76** - Appendix H holds the `Default` table, `From`, `PartialEq<Rhs, Out>` and H.10 (EU-17, AC 6; T2)
Proof: run spectext | test appendix_h_holds_default_from_and_the_operator_traits ... ok

**C77** - The Book's "Values in hardware" teaches the explicit model in its three rewritten sections (EU-18, AC 1; T12)
Proof: run bookvalues | test the_values_chapter_teaches_the_explicit_model ... ok

**C78** - The Book lists "no primitive numeric type has a default" among its differing rules, without "same as Rust" (EU-18, AC 2; T13)
Proof: run booklang | test the_book_says_numbers_have_no_default ... ok

**C79** - No Book listing or sentence uses a removed name (EU-18, AC 3; T30)
Proof: run bookinv | test the_book_uses_no_removed_name ... ok

**C80** - The Book teaches a trait as a capability, `impl Resolve<X> for` a nature, and "Driving with strength" with the four worked nets; it names no library `Wire` (EU-18, AC 4; T14)
Proof: run booknatures | test the_book_drives_nets_with_strength ... ok

**C81** - Every Book listing that cannot run names its `GAPS.md` row and its test fails loud on any change (EU-18, AC 5; T18)
Proof: run bookinv | test every_listing_that_does_not_run_names_its_gap ... ok

**C82** - The Book's "Device ABI" chapter describes the contract without known bytes and says it is unreleased (EU-18, AC 6; T17)
Proof: run bookabi | test the_abi_chapter_describes_the_unreleased_contract ... ok

**C83** - `STATE.md` holds MD-78 and marks MD-70 superseded by it (EU-19, AC 1; T11)
Proof: run docs | test state_holds_the_explicit_unknowns_decision ... ok

**C84** - `GAPS.md` closes S6 and G6 by redesign; S8 exists after the amendment and is gone after phase A (EU-19, AC 2; T40)
Proof: run docs | test gaps_closes_s6_and_tracks_the_flag ... ok

**C85** - The per-level feature's spec opens with its dropped line (EU-19, AC 3; T11)
Proof: run docs | test the_per_level_feature_is_recorded_as_dropped ... ok

**C86** - No `.msl` under `std`, `examples`, `tests` or `crates/*/tests` uses a removed name outside a refusal test (EU-20, AC 1; T27)
Proof: run hygiene | test no_source_uses_a_removed_unknown_form ... ok

**C87** - Every test that pinned an unknown behavior has a named successor (EU-20, AC 2; T27)
Proof: run hygiene | test every_unknown_test_has_a_successor ... ok

**C88** - `std` builds with no diagnostic and every Part VII fence is consecutive lines of its file (EU-20, AC 3; T23)
Proof: run excerpts | test every_part_vii_fence_is_in_its_file ... ok

**C89** - The 7 twins print the bytes they printed before (EU-21, AC 1; T40)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

**C90** - Every buck target passes with its waveform asserts unchanged (EU-21, AC 2; T24)
Proof: run buckcli | test piperine_test_passes_on_the_buck ... ok

**C91** - Both integrated suites pass (EU-21, AC 3; T40)
Proof: run integrated | test the_runs_execute_at_once_in_their_own_build_directories ... ok
Proof: run cliintegrated | test piperine_test_passes_pi_loop_and_threshold ... ok

**C92** - A carrier driven by an undelayed definition, or by a child's `out` port that gives the initial value, is not `E0187`; one driven only by a delayed definition is (EU-04, AC 7; T30)
Proof: run types | test a_definition_or_a_child_out_port_specifies_the_value ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Delivered (batches B1 to B3, T1 to T18, 2026-11-21; one commit per task on `feature/explicit-unknowns`).** The specification, `abi.md`, the records and the Book state the explicit model; the code is untouched. S8 in `GAPS.md` tracks the implementation; MD-78 is in `.specs/STATE.md`.

**Settled mid-build (defaults a later task starts from):**

- **Appendix B counts 62 constructs, not 63.** `level` leaves with `Unknown` and `resolve` (D7): three rows, not two. `b1_counts_62_constructs` replaces `b1_still_counts_65_constructs`.
- **Retired section and codes.** Appendix A.3 stays as a retired stub (section numbers are not reused); `E0150`, `E0151`, `E0906` and `E0908` are in `RETIRED_CODES` of `spec_text.rs`. `E0225` and `E0511` are owned by `explicit-unknowns` in `code_owners.tsv` and listed in `GAPS.md` section 10.
- **E0187 does not apply to a carrier an undelayed definition drives, nor to a signal whose child `out` port holds the initial value** (§3.3.4 and §5.1's error row): the driver that gives none is the one refused.
- **Part VII fences are ahead of `std`.** `std_excerpts.rs` holds two lists, `AHEAD_OF_STD` (first unmatched line of each fence) and `BEHIND_SPEC` (std units with no fence). T23 empties the binary part, T44 the `Logic` part; `the_ahead_lists_hold_only_what_still_differs` fails on a stale entry. `Driven` has a hand-written `Default` (`Driven::Z`) so `inertial<T: Repr + Default>` serves `TribufDelayed`. `inertial` is bound by `Default`; `Dff`, `DLatch`, `Slice` and `Delay` are too.
- **The strength rationale (22.3.3).** Confirmed: Verilog-AMS LRM 2.4 section 9.22.1 to 9.22.3 (read from `VAMS-LRM-2-4.md` in git history), citing IEEE 1364-2005 subclauses 7.10 and 7.11 and the eight levels of its Figure 9-3; the ngspice manual 8.6.1 and 8.6.2; IEEE 1164 and IEEE 1800 section 6.6.7 by secondary sources only (no clause text reachable). Dropped: IEEE 1364 7.9 to 7.13 as a range, IEEE 1800 28.11 to 28.15, IEEE 1666 clause numbers, Logisim, Digital (H. Neemann).
- **Spec listings ahead of the code** are `rust` fences listed in `AHEAD_OF_STD`; E.7 and F.4 are `rust,fragment` (they need `Resolve`), so `std_galleries` builds 16 fences (T9 changed its table and fixtures; T45 or the last of B8 flips them back).
- **The Book registers what cannot run the way it already does:** the block row is `refused` (or the claim `unraised`) with gap `S8`, a `GAPS.md` section 11 row (owner `explicit-unknowns`), and a test that asserts today's refusal with `Refusal::expect` (it panics "now runs" the day the listing runs). The nets of "Driving with strength" (digital#17 to digital#20, new; the old digital#17 and digital#18 became digital#21 and digital#22), the `Option` and `Logic` listings of chapter 5, the alarm of chapter 6 and `core-std#6` are E0161 for `Driven` and `Resolve` or E0112 and P0001 for `Option` and `Logic`. Listings that read a discrete carrier in `analog` with no gate (welcome#17, analog#9, modules#9, mixed#8, mixed#10, mixed#14, values#21) are E0150 today; their tests also run the Book's physics through `Model::earlier` (`Earlier::gate` in `tests/book/support/model.rs`), which adds the gate the earlier text had, so every number stays held.
- **The Book may print the earlier front end's own words only in two listings** (the E0150 refusals welcome#19 and the buck's, held to the binary by `piperine-cli`'s book target): `the_book_uses_no_removed_name` lists the exception and counts it; T30 refills both and the test's exempt count goes to 0.
- **`examples/buck/benches/startup.msl` already reads `n` directly** (the Book's cli#44 is held equal to the file, so the line moved in T16; its preformat twin too). T24 has that file done.
- **Tests that still pin the earlier behavior and stay until their code task:** `e0150_*` in `tests/book/analog.rs`, `mixed.rs`, `errors.rs`; `a_reaction_on_a_changed_unknown_emits_the_inputs` and `without_an_initial_value_...` neighbors in `errors.rs`; the unknown tests of `crates/*/tests` (types_unknowns, elab_unknowns, source_unknowns, abi layouts) are untouched.

**Rows other features must flip (ROADMAP.md, "Every Book listing runs before item 13"; each names its owner in GAPS.md section 11):** `resolved-nets` B3 and later (T27 to T32) flips digital#16, digital#17, digital#18, digital#19, digital#20, natures#3 with its sentences natures~e99c1920 and natures~91c3bffe, core-std#6 and natures#2 (gap L21: `Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`; natures#2's `Sda` is the chapter's `impl Resolve<Driven>` nature, and its `[Bit; 8]`, `Option` and `Logic` lines need composite-values and explicit-unknowns phases A and B first, which only re-pin the row). `composite-values` re-pins natures#2, it does not flip it. When a row flips its test fails with "now runs": make the row `run`, delete its section 11 line.

**Spec ahead of the code, registered (stale entries fail loud):** `AHEAD_OF_STD`/`BEHIND_SPEC` in `std_excerpts.rs`; `INDEX_AHEAD_OF_STD`, `INDEX_MEMBERS_AHEAD_OF_STD`, `STD_BEHIND_THE_INDEX` in `std_library.rs` (T23, T44); `STD_BEHIND_THE_BOOK` and `BLOCKS_AHEAD_OF_STD` in `book.rs` (T23; resolved-nets); `SPEC_TRAITS_AHEAD_OF_THE_TABLE` (`From`) in `pom_resolve.rs` (T42); `RETIRED_AHEAD_OF_THE_PARSER` gains `E0908` (T28); `docs/pom/mapping.md` lists 62 constructs while the POM still maps `resolve`, `Unknown`, `level` (T28, T29).

**T19 (B4, 2026-11-22): rebase and re-read at `origin/main` `20c285b8`.** Nothing was unmerged (`git log origin/main..HEAD` empty), so the branch was set with `git checkout -B feature/explicit-unknowns origin/main`; no conflict to resolve in `content.html`, `inventory.tsv`, `GAPS.md` or `spec_text.rs`. What `composite-values` (merged, MD-79) changed for this phase, and where each lands: (1) `GAPS.md` L64 (a plain read of a composite carrier holding no value yet fails the run) and L65 (a flag read on an `Option`) are owned by this phase: T22 removes the case (every carrier starts at a value), T29 deletes the flag reads, and **T40 removes both rows**. (2) The three E0180 rows (values~13ffaea6, analog~39ba351d, mixed~39ba351d) already hold on `main` (`Logic` is a struct type since CV-09): **removed from T48's flip list**. (3) The composite pin layout is packed, alignment 1, plus ONE trailing known byte per composite (`CompositeLayout`, `piperine-plan/src/composite.rs`): T36 removes that byte. (4) Every place that reaches the hidden flag of a composite, and its owner: `CompositeLayout` and the composite `Params` bytes (T36), `CellShape::decode` in `piperine-api/src/composite.rs` (T38), `Typing::read_of` in `piperine-ir/src/typing.rs` (T32), `lower_flag_receiver` in `elab/known.rs` (T29 for the methods, deleted with the file in T31), the printer's `.known_or(Default::default())` for a composite read and `Flagged::<T>::decode(..).known_or_refuse(..)` in `print/{value,digital,analog}.rs` (T33), `Flagged::recast` and `known_or_refuse` in `rt/flag.rs` (T33, the file goes). (5) `composite-values` T52 was NOT amended in place: `Default` for `bool` and the numbers still holds on `main`; T21 changes the table. (6) Anchors that moved: none of design.md's "What exists" paths; `Defaults` lives with the derive table in `types/derives.rs` (`composite-values` T44), not `types/compound.rs`; the E0150 rule is in `types/result.rs` (`analog_reads`), not `types/analog.rs`, and the elaborator's twin is `known_in_analog` in `elab/known.rs`; `elab/carriers.rs` has `Frame` at line 48. (7) The two doc fixes the orchestrator made on `main` without a review are right: `docs/pom/mapping.md` says "The 62 constructs" and Appendix B.1.2 holds exactly 62 numbered rows; the `pom_docs.rs` WAITING entry for the `resolution.md` E0261 row ("a discrete nature without `resolve`", line 338) names a statement that is on the page and a `Resolve { meta, expr }` form that is in `pom/decl.rs`, so the entry is live and owned by `resolved-nets` T16.

**Delivered (batches B4 and B5, T19 to T27, 2026-11-22).** Settled defaults a later task starts from:

- **Reads in `analog` (T20).** The checker has no flag-total rule; the elaborator lowers a carrier read or `prev()` in an analog scope as `known_or(zero)` of its type (`Elaboration::analog_value`, `elab/known.rs`) while the flag is carried, and a flag read keeps the raw carrier (`lower_flag_receiver`). `E0150` stays only for a `const` holding `Unknown` (T28). The IR rule `analog_reads` of `piperine-ir/src/check.rs` still holds (T32 removes it with the flag). Book rows welcome#17, analog#9, modules#9, mixed#8, mixed#10, mixed#14, values#21 flipped to run (T20's CLI listings welcome#19 and the buck's were refilled in the T22 commit).
- **`Default` (T21).** `bool`, integers, floats and `str` have none; E0187 names the field and type (derive) or the type (`T::default()`); `str::default()` was silently untyped and is now E0187. The `generics` row of `code_owners.tsv` for E0187 was dropped (a source raises it now).
- **Initial values (T22).** `default_initial_value`/`default_constant` in `elab/carriers.rs`: `Option` is `None`, tuples, arrays and structs deriving `Default` hold their parts; a type without `Default` keeps zero bytes until T30. A hoisted module's `in` port is driven by its caller and gets none. An `in` port may carry `= value` (types/module.rs; was P0001; T24). `Option<T>` params are still P0001 (L28, `params-and-io`): `an_unbound_option_param_is_none` cannot run, `an_unbound_option_param_waits_for_params_and_io` asserts today's refusal. Arrays of carriers are L24: the "never committed element" test reads a composite `Bit<[u8; 4]>`.
- **D8/D9 in the engine.** `Store::expect_initial` (piperine-digital-solver): the next write to a net is its initial value, a change whatever the bytes, `PREVIOUS` the same bytes. Set in `Digital::start_at` for every net `start` queues an init for and for every undelayed-definition net with no initial value (T25). `rise()`/`fall()` keep their elaborator forms (`rise_guard`, `fall_guard`: flag-based) while the std text is D9; they agree on known values. Models that moved: two (the two `Script`s of the parity buck fixtures, whose `on clk.fall()` relied on the (0, 0) fall: `|| init()`); one golden line (`source_natures`: `before` 255 to 0). Not many: no stop.
- **std (T23).** `Known`, `merge` and `high` STAY in std until T29; the library nets, `Bidir`, `TribufDelayed` and the old `Logic` stay (flag-free bodies) until `resolved-nets` and T44 because `Driven`, `Logic<N>` and `Resolve` cannot compile yet: `AHEAD_OF_STD` has 13 entries, `BEHIND_SPEC` 22, `INDEX_AHEAD_OF_STD` 3. The task text asked for `Driven`, `strongest` and the generic net modules in `nets.msl`: not buildable now. Part VII 22.4.1's D2A fence is the formatter's form. `std_has_no_unresolved_member` allows `base.unwrap_or(start)`.
- **Buck (T24).** The hand fixtures `crates/piperine-ir/tests/golden/buck/**` are untouched; `elab_buck.rs`'s listed differences grew (plain reads, `dithered_width~11`). `fixtures/elab/buck.txt` moved 57 lines (plain reads, `flagged<..>` locals, `init 0.0` on `transition`'s carriers).
- **Tests pending a task:** `PENDING` in `suite_hygiene::RemovedUnknownForms` (std/values.msl T29; aggregates/carriers.msl and normal.msl T31: a plain carrier read inside a struct, a variant or a tuple-returning call is refused while the flag is carried); `SUCCESSORS` marks the `level`/`merge`/`same`/`cast`/`logic`/`matches_bits` tests Some("T45"/"T46"). `pom_reflect::nature_impls_lists_its_impl_blocks` expects two `impl Bit` until T29 removes `high`. T36 must keep a zero initial value a change (the store marker already does).
- **Generated code and IR without a flag (T32, T33, B6).** `ValueType::Flagged` and `Expr::{Unknown, KnownOr, IsKnown, High, Merge, Level}` are gone from the IR; `rt/flag.rs` is deleted and what it held that is no flag stays as `rt/pin.rs` (`Carried`, `LevelWords`, `Field`, `Tag`: the pin codec, with `from_pin`, `pin` and `write_pin`, which still set the known byte to 1 until T36). `rt/commits.rs` is `Commits::{commit, record, arm, finish, flow, flush}` and nothing else (no `Due`, no join, no `controlled`, no `maybe`); `rt/control.rs` keeps `Panic`, `Trips`, `Depth`, `Index::{of, at}`. `same` and `cast` print as `to_levels` compared and `from_levels` moved (a tag naming no variant decodes as the enum's `Default`, a payload enum cast is plain); a `cast` result no longer carries `tags` in its print. `Source::Resolution` and `Pins.resolution` are removed from `piperine-plan` (and the interpreter's arm). A record's `maybe` field is still in the layout (T34) and the generated code writes it false. The array-length check of the type checker raised the retired `E0151`: it is `E0188` now (the code the spec gives an array length that is no const), `suite_hygiene::retired_unknown_codes_are_raised_nowhere` guards it. The hand fixtures of the buck (`crates/piperine-ir/tests/support/digital.rs`) follow the migrated source: `&&`/`||` of the dpwm and the pid windup are `And`/`Or`, `rev3~1` and `dithered_width~01` are gone, `elab_buck.rs` lists only the rise guard, the pid fold, the GateDriver initial values and the A2D/Never additions; `buck::golden` no longer strips a flag. `crates/piperine-transpile/tests/analog_merge.rs` became `analog_if.rs` (one path). Tests that pinned a flag are listed in the commit body.
- **B6 batch gate, 2026-11-22 (T32 and T33 committed; first run red, closed below: the gate is GREEN, B7 starts from `origin` of this branch).** Closed after the first run, one commit per group: the Book (`test(book)`: welcome#21 and pom#2 give their out port a first value, inline models read directly, E0187 rows become `holds`, core-std~6e04c51d `unraised` under T45), the T45 successor row for `the_library_items_that_took_borrows_give_the_same_results`, `fix(api)` (a leaf of a variant the value does not hold has no cell yet: `CellShape::can_hold_leaf`), and the cli run/docs targets (GAPS section 10 lists E0702). Final run `env -u PIPERINE_NGSPICE cargo test --workspace --exclude piperine-lang-server --no-fail-fast`: every target ok. The list that was red at the first run: Green: the `**Build**` line (zero warnings), `piperine --test integrated`, `piperine-cli --test integrated`, `piperine-cli --test twins` and `piperine-parity --test twins` (ngspice present, no SKIP line), `piperine-cli --test buck` (8 tests), every `buck_*` target of `piperine-parity`, `piperine-ir`, `-transpile`, `-plan`, `-interp`, `-lang` (except `piperine-lang-server`, which does not build on `main` either: it names `piperine_lang::ast` and friends). Fixed after the gate, one commit per group: the transpiler texts of an emit value, a commit value and a last assignment (`cbfa7d44`), the parity models and helpers (`ae92deca`, `7c51857f`: every `out` port and `signal` starts at a value, an initial value goes on the declaring `out` port and never on the signal handed to it (E0223), `first_edge_value` reads the value after the first edge), the api host tests (`4e199fbc`). Still red, none of them a flag in the code: (1) the root Book targets `book_cheatsheet`, `book_cli`, `book_compiler`, `book_core_std`, `book_digital`, `book_errors`, `book_mixed`, `book_natures`, `book_pom`, `book_solver`, `book_values`, `book_welcome` and `piperine-cli --test book`: Book listings and the inline sources of `tests/book/*.rs` still spell `.high()`, `known_or`, a carrier with no initial value (e.g. welcome#21's `Inv` out port `y` with a delayed definition, the SAR ADC, the digital card) so they fail E0161 or E0187; each needs its listing refilled in `docs/book/content.html` and `inventory.tsv`, with the output blocks regenerated, in one commit per chapter; (2) `piperine-cli --test docs` (`gaps_no_longer_lists_x5_l20_l39_h12`: GAPS section 10 against `code_owners.tsv`) and `piperine-cli --test run` (three tests, inline models); (3) `piperine-api --test composites::a_carried_enum_reads_as_its_variant_name` ("the value has no leaf `Data.n`": the printed `tx` is not the committed `Data { n: 5 }`; not diagnosed); (4) `piperine-parity --test source_value_semantics::the_library_items_that_took_borrows_give_the_same_results` and the Book test `e0702_is_raised_for_a_logic_pattern_with_z`: `logic` and `matches_bits` are generic `std` items refused `P0001` until T45. `tools` used to fix the models were throwaway scripts (not kept): the rule they applied is E0187 -> `= zero` on the `out` port (or the `signal` when no child out port holds it), E0223 -> no initial value on the placing module's declaration.
- **Process notes.** One shell call without the lane A environment ran two `cargo` commands in `/var/tmp/piperine-target` early in T20 (no build was left running there). T20's CLI Book listings and T22's `source_natures` compile fix are folded into T22 (the first as part of its commit, the second as a fixup squashed into it).

- **Delivered (batch B7, T34 to T40, 2026-11-22; phase A closes).** Settled defaults a later task starts from:
  - **T34.** A record is its declared fields: `RecordLayout::maybe_offset`, `Record::maybe`, `Record::surely` (now `Record::holds`) and the `maybe` column of both renderers are gone; `E0604` keeps "repeated". A stream with no value field prints `let record_`, not `let mut`.
  - **T35.** The host is `{ size, commit }`, minor 0: `commit_with`, `commit_flag`, `Commit::flags`, `layout::Merge`, `DigitalError::Merge` and the `MAYBE`/`MERGE` queue code are removed.
  - **T36.** `Field` is `{ name, leaf, offset, count }`; `BIT` and `BYTE` are one byte, `F64` eight, `REAL` is deleted; `is_canonical` and the P3 canonical check go with the byte. A composite pin is its parts' value bytes (alignment 1, size = width), an `Option`'s presence field is a `BOOL`, `rt::pin::Carried` has no `SIZE`. The cache key already hashes the contract's source (`cache::an_entry_of_the_earlier_contract_is_rebuilt`). The host's write before a run is the carrier's initial value (`Digital::expect_initial` at `start`). The hand-written test devices of the solver crates lost their three-valued logic; `ROpt`'s `w` is an `F64` with a default (an `Option` param is still L28). Tests the removed byte deleted are listed in `solver-abi-integration/removed.txt`.
  - **T37.** `DigitalError::Unsettled` (`E0511`, naming the loop's drives in name order); `Observation::Unsettled` and `ObservationKind::Unsettled` are gone. A relaxing cycle still starts with every drive absent (zeroed once, then relaxed to a fixed point), not at each pass.
  - **T38.** `Cell::Unknown` and `Field::Unknown` are gone. A leaf a value does not hold shows an empty text; `CellShape::decode` fails on a tag that names no variant. A top `in` port with no initial value, `Default` or host write refuses `Session::run` with `E0187` (`CircuitInstance::net_written`); the `in` ports of the test designs got initial values. values~37a548f2 flipped to `holds` here (its test failed with the refusal), and the three phase B rows (values#18, values~c30bad4d, core-std~6e04c51d) point at L58.
  - **T39.** The renderers have no unknown arm; an absent `Option` prints `None` (table, record line), `null` (JSON Lines) or an empty `value` leaf (CSV). An `emit!` field of type `Option` is outside the M1 subset ("an expression the type checker gave no value type"): the record forms are held by the render unit tests.
  - **T40.** `GAPS.md`: S8, L64 and L65 removed (1 silent, 2 broken, 28 loud); G.72 says the implementation meets the item in every layer and names `Logic<N>` (phase B, L58) and `Resolve` (`resolved-nets`, L21); `docs.rs::gaps_closes_s6_and_tracks_the_flag` replaces `gaps_closes_s6_and_g6_by_the_redesign`; the `E0511` row left `code_owners.tsv` (a source raises it); MD-78's status line says phase A is delivered. The Book rows of the T40 list were all flipped by their own tasks (T20, T21, T22, T30, T38); T40 re-checked each against `inventory.tsv`.
  - **Gate (B7), 2026-11-22.** `env -u PIPERINE_NGSPICE cargo test --workspace --exclude piperine-lang-server --no-fail-fast`: 314 targets, every one `ok` after one fix commit. The first run was red in five targets, all of one cause (a top `in` port with no initial value now refuses the run, E0187): `book_analog`, `book_digital`, `book_mixed`, `book_modules` and `host_ii`; their wrapped tops give the port a first value and only those five targets were rerun. The integrated suites, `piperine-cli --test twins`, `piperine-parity --test twins`, the buck (`piperine-cli --test buck`, every `buck_*` target of `piperine-parity`) and the parity tests against ngspice ran in the workspace run (ngspice present: the only SKIP lines are the two skip-line tests). The Build line is clean (zero warnings). Twin goldens and `contract.rs` did not move.

**Dispatched:** none. **Walked:** none (the verifier of B1 to B3 is the caller's).

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4 ------→ T5 ------→ T6
Phase 2:  T7 ------→ T8 ------→ T9 ------→ T10 ------→ T11
Phase 3:  T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16 ------→ T17 ------→ T18
Phase 4:  T19 ------→ T20 ------→ T21 ------→ T22
Phase 5:  T23 ------→ T24 ------→ T25 ------→ T26 ------→ T27
Phase 6:  T28 ------→ T29 ------→ T30 ------→ T31 ------→ T32 ------→ T33
Phase 7:  T34 ------→ T35 ------→ T36 ------→ T37 ------→ T38 ------→ T39 ------→ T40
Phase 8:  T41 ------→ T42 ------→ T43 ------→ T44 ------→ T45 ------→ T46 ------→ T47 ------→ T48
```

Execution is strictly sequential. Phases 1 to 3 merge to `main` first. Phases 4 to 7 wait for `composite-values`. Phase 8 waits for `generics` phase A.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: Part I: §3.3 is the explicit model, a nature takes trait impls | 1 file or 1 unit, 1 concern | ✅ Granular |
| T2: Appendix H: `Default`, `From`, `PartialEq<Rhs, Out>`, operator traits | 1 file or 1 unit, 1 concern | ✅ Granular |
| T3: Part II: carriers start with a value, `Resolve` is a capability | 1 file or 1 unit, 1 concern | ✅ Granular |
| T4: Part III: structure without unknown levels | 1 file or 1 unit, 1 concern | ✅ Granular |
| T5: Part IV: behavior reads values | 1 file or 1 unit, 1 concern | ✅ Granular |
| T6: Parts V, VI and VIII: execution, output and the host | three files, each under ten lines of change | ⚠️ cohesive, kept whole |
| T7: Part VII: `Logic`, the tables, strengths and the net modules | 1 file or 1 unit, 1 concern | ✅ Granular |
| T8: Appendices A, B, C and the index | three appendices and the index, small edits of one concern | ⚠️ cohesive, kept whole |
| T9: Galleries D, E, F and Appendix G | three galleries and Appendix G: one concern, the listings | ⚠️ cohesive, kept whole |
| T10: `abi.md`: the contract edited in place | 1 file or 1 unit, 1 concern | ✅ Granular |
| T11: Records of the amendment | 1 file or 1 unit, 1 concern | ✅ Granular |
| T12: Book: "Values in hardware" teaches the explicit model | 1 file or 1 unit, 1 concern | ✅ Granular |
| T13: Book: language basics, types and traits | 1 file or 1 unit, 1 concern | ✅ Granular |
| T14: Book: natures, capabilities and driving with strength | 1 file or 1 unit, 1 concern | ✅ Granular |
| T15: Book: analog, mixed-signal and simulation listings | 1 file or 1 unit, 1 concern | ✅ Granular |
| T16: Book: standard library, errors, CLI and cheat sheet | 1 file or 1 unit, 1 concern | ✅ Granular |
| T17: Book: device ABI, solver and compiler chapters | 1 file or 1 unit, 1 concern | ✅ Granular |
| T18: Book: the inventory holds, the removed names are fenced | 1 file or 1 unit, 1 concern | ✅ Granular |
| T19: Contract: rebase onto `main` with `composite-values` | 1 file or 1 unit, 1 concern | ✅ Granular |
| T20: Checker: `analog` reads a binary carrier directly | 1 file or 1 unit, 1 concern | ✅ Granular |
| T21: Checker: the `Default` table | 1 file or 1 unit, 1 concern | ✅ Granular |
| T22: Elaborator: a carrier starts at its written value or its default | 1 file or 1 unit, 1 concern | ✅ Granular |
| T23: `std` in the explicit form | 1 file or 1 unit, 1 concern | ✅ Granular |
| T24: `examples/buck` in the explicit form | 1 file or 1 unit, 1 concern | ✅ Granular |
| T25: The e2e packages in the explicit form | 1 file or 1 unit, 1 concern | ✅ Granular |
| T26: The parity and CLI fixtures in the explicit form | 1 file or 1 unit, 1 concern | ✅ Granular |
| T27: `source_unknowns` becomes `source_explicit` | 1 file or 1 unit, 1 concern | ✅ Granular |
| T28: Lexer and parser: `Unknown` is no token | 1 file or 1 unit, 1 concern | ✅ Granular |
| T29: Checker: the flag methods are gone | 1 file or 1 unit, 1 concern | ✅ Granular |
| T30: Checker: a missing initial value is `E0187` | 1 file or 1 unit, 1 concern | ✅ Granular |
| T31: Elaborator: no expression is flagged | one deleted analysis and its uses | ⚠️ cohesive, kept whole |
| T32: IR: `ValueType::Flagged` is removed | 1 file or 1 unit, 1 concern | ✅ Granular |
| T33: Generated code: no flag and no merge | one deleted runtime unit and its printer arms | ⚠️ cohesive, kept whole |
| T34: Records hold their declared fields | 1 file or 1 unit, 1 concern | ✅ Granular |
| T35: ABI: one `commit` | 1 file or 1 unit, 1 concern | ✅ Granular |
| T36: ABI: layouts hold the value only, still major 1 | one layout change across the crates that share it (design.md Risks) | ⚠️ cohesive, kept whole |
| T37: Digital solver: relaxation on a plain `driving` byte | 1 file or 1 unit, 1 concern | ✅ Granular |
| T38: Host: a cell is a value | 1 file or 1 unit, 1 concern | ✅ Granular |
| T39: CLI: output without unknown | 1 file or 1 unit, 1 concern | ✅ Granular |
| T40: Phase A closure | 1 file or 1 unit, 1 concern | ✅ Granular |
| T41: Contract: rebase onto `main` with `generics` phase A | 1 file or 1 unit, 1 concern | ✅ Granular |
| T42: Checker: operators through the operator traits | 1 file or 1 unit, 1 concern | ✅ Granular |
| T43: Elaborator: an operator on a library type is a call | 1 file or 1 unit, 1 concern | ✅ Granular |
| T44: `std/logic.msl`: `Logic<N>` from source | 1 file or 1 unit, 1 concern | ✅ Granular |
| T45: `logic` and `matches_bits` over `Logic<N>` | 1 file or 1 unit, 1 concern | ✅ Granular |
| T46: `Logic` runs from source: tables, comparison, conversions, analog | 1 file or 1 unit, 1 concern | ✅ Granular |
| T47: Host and plan: a `Logic` pin and a `Logic` cell | 1 file or 1 unit, 1 concern | ✅ Granular |
| T48: Phase B closure | 1 file or 1 unit, 1 concern | ✅ Granular |

## Diagram-Definition Cross-Check

Every phase is a linear chain: each task's `Depends on` is the task before it, and the first task of a phase depends on nothing inside its phase. The diagrams show exactly those arrows.

## Test Co-location Validation

Every task that changes a layer of the matrix names its integration target in `Done when`. T19 and T41 are contract re-reads with `Tests: none`, the matrix's last row. No test is deferred to a later task: a criterion whose behavior needs two tasks is asserted in the second, and the first asserts what it alone delivers.

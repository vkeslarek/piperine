# MSL Polish Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-polish/design.md`
**Status**: Approved
**Approved**: 2026-11-14 - user: "sim"
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^warning')`
**Run spec**: `cargo test -p piperine-lang --test spec_text`
**Run lexer**: `cargo test -p piperine-lang --test msl_lexer`
**Run parser**: `cargo test -p piperine-lang --test msl_parser`
**Run errors**: `cargo test -p piperine-lang --test msl_errors`
**Run print**: `cargo test -p piperine-lang --test msl_print`
**Run mslcorpus**: `cargo test -p piperine-lang --test msl_corpus`
**Run design**: `cargo test -p piperine-lang --test pom_design`
**Run tree**: `cargo test -p piperine-lang --test pom_tree`
**Run body**: `cargo test -p piperine-lang --test pom_body`
**Run pomcorpus**: `cargo test -p piperine-lang --test pom_corpus`
**Run resolve**: `cargo test -p piperine-lang --test pom_resolve`
**Run classify**: `cargo test -p piperine-lang --test pom_classify`
**Run reflect**: `cargo test -p piperine-lang --test pom_reflect`
**Run docs**: `cargo test -p piperine-lang --test pom_docs`

Executors: Sonnet sub-agents, one commit per task. Verifier: Opus. Work in `/home/keslarek/Git/piperine-msl-spec`, branch `feature/msl-polish`. Do not set `CARGO_TARGET_DIR` (the environment provides it). Never pass `--workspace`: the other crates do not build and are out of scope. `CLAUDE.md` still describes the retired Verilog-AMS pipeline; for this feature the facts are in `spec.md` and `design.md`.

Rules every task follows:

- A new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. `spec_text` is the one new target of this feature.
- A test asserts the spec's outcome on a source text written in the test (or on the text of `docs/spec/`); it never restates the implementation. The test function has exactly the name its check cites.
- An existing test is only strengthened. The one exception: a decision of `spec.md` (D1 to D17, or a finding's fix named in the task's **Existing tests** line) changes what a test must expect. Then exactly the affected assertion changes and the commit body names the test and the decision. An existing test that goes red for any other reason: stop and report, do not edit it.
- `docs/spec/` follows `.specs/features/msl-spec/STYLE.md`: numbered rules, `> **Resolution.**` notes for what the decisions fix, error tables `Code | Condition | Phase`, sentences under 30 words, no history. A syntax excerpt in a part repeats B.3.2 verbatim.
- Three guards tie the spec to the code, so three kinds of spec edit land only in the commit that satisfies them: a row of phase `parse` in an error table (needs its fixture in `msl_errors.rs`), a `rust` or `rust,fragment` fence (must parse and round-trip now), a new production name in B.3.2 (needs its row in `docs/pom/mapping.md`).
- A public POM type gets its `### ` heading and a public reflection method its mention in `docs/pom/` in the task that adds it (`pom_docs` enforces both).
- No macro is written for tables, handles or accessors: data and plain functions. Storage types keep the standard derives (`Debug, Clone, PartialEq, Serialize, Deserialize`).
- Only lowering (`src/pom/lower/`) creates POM nodes; resolution writes slots and side tables (MD-54). No POM function panics on any input.
- The parser's sources (`src/msl/`) are written from `docs/spec/` only.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Where the tests are": a target lives in the crate it exercises, one concern per target, `//!` header), `tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope `allow`; it cannot run today because the root crate does not build, so its rules are applied by hand), the thirteen existing `crates/piperine-lang/tests/{msl,pom}_*.rs` targets (integration tests over source text; corpus from `docs/spec/`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Specification text (`docs/spec/*.md`, `mkdocs.yml`) | integration | one assertion per decided wording (POL-01 to POL-08, POL-13 AC 1, POL-15 AC 1); the code and construct-count guards | `crates/piperine-lang/tests/spec_text.rs` | run spec |
| Lexer (`msl/lexer.rs`) | integration | POL-10 AC 1, POL-13 AC 2, 3, 5 and 7: each boundary value on both sides | `crates/piperine-lang/tests/msl_lexer.rs` | run lexer |
| Parser, accepted forms (`msl/parser/`, `msl/ast.rs`) | integration | POL-09 AC 1 to 3, POL-11 AC 1, POL-12 AC 1 and 3: each form in every position the criterion lists | `crates/piperine-lang/tests/msl_parser.rs` | run parser |
| Parser, diagnostics and recovery (`msl/parser/check.rs`, `entry.rs`) | integration | POL-09 AC 5 to 7, POL-10 AC 2, POL-12 AC 2, POL-13 AC 4, POL-14 AC 1 to 7, POL-15, POL-16: code, span and message; one fault, one diagnostic | `crates/piperine-lang/tests/msl_errors.rs` | run errors |
| Printer (`msl/print.rs`) | integration | round trip and idempotence of every new form | `crates/piperine-lang/tests/msl_print.rs` | run print |
| Spec corpus through the parser | integration | every fence, after the spec edits | `crates/piperine-lang/tests/msl_corpus.rs` | run mslcorpus |
| POM lowering and `to_ast` (`pom/lower/`, `pom/to_ast.rs`, `pom/body.rs`) | integration | every new node form lowers and converts back; body navigation over every `ExprKind` | `crates/piperine-lang/tests/pom_body.rs`, `pom_corpus.rs`, `pom_tree.rs` | run body, run pomcorpus, run tree |
| Scopes, imports, names, built-ins (`pom/resolve/scopes.rs`, `imports.rs`, `names.rs`, `pom/builtin.rs`) | integration | POL-02 AC 5 and 6, POL-03, POL-04, POL-06, POL-10 AC 4, POL-14 AC 8, POL-16 AC 4, POL-17: each rule with its positive and its negative case | `crates/piperine-lang/tests/pom_resolve.rs` | run resolve |
| Classification, members, structural checks (`pom/resolve/classify.rs`, `members.rs`, `behaviors.rs`, `shape.rs`) | integration | POL-05 AC 3 and 4, POL-11 AC 3, POL-18, POL-19 AC 1 and 2, POL-20 AC 7 and 8: each code at its span, and no `E0161` beside it | `crates/piperine-lang/tests/pom_classify.rs` | run classify |
| Reflection, reverse index, serialization (`pom/reflect/`, `pom/index.rs`, `pom/design.rs`) | integration | POL-13 AC 8 and 9, POL-19 AC 3, POL-20, POL-21: each new method on a source that exercises it | `crates/piperine-lang/tests/pom_reflect.rs`, `pom_design.rs` | run reflect, run design |
| Reference docs (`docs/pom/`) | integration | POL-22: construct, code, built-in and method coverage | `crates/piperine-lang/tests/pom_docs.rs` | run docs |
| Data-only definitions (enum variants, table rows, `Cargo.toml`) | none | build gate only | - | build |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | after a task that touches one layer and cannot move another target (spec text with no fence, one resolver rule) | that task's `Run <name>` line(s), plus `run mslcorpus` and `run pomcorpus` when it edits `docs/spec/` |
| Full | after a task that changes the AST, the lexer, the parser, lowering or a resolver stage others read | `cargo test -p piperine-lang` |
| Build | every task, before its commit | the `**Build**` line above |

---

## Execution Plan

Phases are ordered and run sequentially; tasks within a phase execute in order. Each phase starts from the last task of the one before.

### Phase 1: Spec: the Rust boundary and names

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Spec: statements, docs, checks, new surface

```
T6 → T7 → T8 → T9 → T10
```

### Phase 3: Parser: accepted forms

```
T10 → T11 → T12 → T13 → T14 → T15 → T16
```

### Phase 4: Parser: literals and strictness

```
T16 → T17 → T18 → T19 → T20 → T21 → T22
```

### Phase 5: Parser: limits and recovery

```
T22 → T23 → T24 → T25 → T26
```

### Phase 6: POM: names

```
T26 → T27 → T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 7: POM: the spec's codes

```
T33 → T34 → T35 → T36 → T37 → T38 → T39
```

### Phase 8: POM: reflection and robustness

```
T39 → T40 → T41 → T42 → T43 → T44 → T45 → T46
```

### Phase 9: Reference docs and closure

```
T46 → T47 → T48 → T49
```

---

## Task Breakdown

### T1: `spec_text` target and the exhaustive B.3.6

**What**: A new test target `spec_text` that reads `docs/spec/*.md` (and `mkdocs.yml`) and asserts the wording the decisions fix; B.3.6 rewritten around one table of removed Rust forms (columns: Removed, Reported as, Write instead; the code never in the first cell) plus a `Taken from Rust` list (function-pointer types, a negative literal as a const generic argument); the two old rows that no parser can reject (struct-update syntax in a template, blocking writes) move to a closing sentence citing §8.4 and §12.2; B.3.1 item 1 and §0.4 point at B.3.6.
**Where**: `docs/spec/appendix_b_grammar.md` (+ `docs/spec/index.md` §0.4, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: None
**Reuses**: `tests/support/spec_corpus.rs` (fence extraction), `msl_errors.rs::spec_parse_phase_codes` (table-row scraping pattern)
**Requirement**: POL-01, POL-08
**Closes**: A-17, A-28

**Done when**:

- [x] Test `b36_lists_every_removed_form` passes in `spec_text.rs` (C1: POL-01 AC 1)
- [x] Test `b36_names_the_forms_taken_in` passes in `spec_text.rs` (C2: POL-01 AC 2)
- [x] Test `b36_rows_carry_a_code_and_a_replacement` passes in `spec_text.rs` (C3: POL-01 AC 3)
- [x] Test `every_error_code_is_defined_exactly_once` passes in `spec_text.rs` (C45: POL-08 AC 7)
- [x] Test `b1_still_counts_65_constructs` passes in `spec_text.rs` (C46: POL-08 AC 8)
- [x] `spec_text.rs` opens with a `//!` scope header and has helpers `spec(file)`, `section(text, heading)`, `table_rows(section)`: plain functions, no macro
- [x] The B.3.6 table has no error code in its first cell (the parse-phase fixture guard of `msl_errors` scrapes first cells)
- [x] No `rust` fence is added that the parser does not yet accept (function-pointer examples arrive in T12)
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): B.3.6 lists every removed Rust form; spec_text target`

---

### T2: Appendix H: the inherited library

**What**: A new normative page `appendix_h_library.md` with the closed catalogue of D2 (one table per group; each row: name, signature, note), built from the methods the spec's own fences call plus the numeric, `Option`, array/slice, range and iterator families of the decision; §0.4, §1 rule 1 and the `#[derive]` row of A.1 point at it; file map (§0.8) and `mkdocs.yml` nav list it.
**Where**: `docs/spec/appendix_h_library.md` (+ `docs/spec/index.md`, `docs/spec/part_i_values.md`, `docs/spec/appendix_a_lexical.md`, `mkdocs.yml`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T1
**Reuses**: the lexer (`piperine_lang::msl::lexer`) to find `.name(` sequences in fences
**Requirement**: POL-02
**Closes**: A-18

**Done when**:

- [x] Test `appendix_h_has_every_group` passes in `spec_text.rs` (C4: POL-02 AC 1)
- [x] Test `every_method_called_in_a_spec_fence_is_catalogued` passes in `spec_text.rs` (C5: POL-02 AC 2)
- [x] Test `catalogue_is_closed_by_e0161` passes in `spec_text.rs` (C6: POL-02 AC 3)
- [x] Test `appendix_h_is_in_the_file_map_and_nav` passes in `spec_text.rs` (C7: POL-02 AC 4)
- [x] The method scan tokenizes each fence with the crate's lexer (a `.`, an identifier, a `(`), not with a regular expression
- [x] Appendix H holds no row the decision does not name or a spec fence does not call: it is a closed list, not Rust's library
- [x] The derivable traits of A.1 are replaced by a reference to Appendix H
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): Appendix H, the closed catalogue of the inherited library`

---

### T3: Core names in the text

**What**: §6 rule 11 reworded (`gnd` is a core name), A.1 rule 8 and §3.4.2 name the core names (`gnd`, `Nature`, `m`, `dtemp`, `str` beside the core primitives of §14); a Resolution note says none of them can be declared in the language.
**Where**: `docs/spec/part_ii_carriers.md` (+ `docs/spec/appendix_a_lexical.md`, `docs/spec/part_i_values.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T2
**Reuses**: T1 helpers
**Requirement**: POL-03
**Closes**: A-01

**Done when**:

- [x] Test `gnd_and_the_implicit_params_are_core_names` passes in `spec_text.rs` (C10: POL-03 AC 1)
- [x] The B.1 count is untouched: all five names are already rows of B.1.2
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): gnd, Nature, m, dtemp and str are core names`

---

### T4: `E0162`: duplicate names, colliding imports, ambiguous globs

**What**: §3.4 gains the namespace-table rules of D5 (Rust's: one name per table per namespace; an item or explicit `use` shadows a glob; two globs clash only when the name is used) and the `E0162` row; the `E0143` row notes that it keeps the two-functions case.
**Where**: `docs/spec/part_i_values.md` (+ `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T3
**Reuses**: T1 helpers
**Requirement**: POL-04
**Closes**: A-10, A-40, A-46

**Done when**:

- [x] Test `e0162_is_defined_in_3_4` passes in `spec_text.rs` (C19: POL-04 AC 1)
- [x] Test `glob_shadowing_and_e0143_are_stated` passes in `spec_text.rs` (C20: POL-04 AC 2)
- [x] `E0162` is phase `elaboration` (no parse-phase fixture is owed)
- [x] `E0330` (module members) is left as it is
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): E0162 for a name declared or imported twice`

---

### T5: Part I: borrows as arguments, identifier patterns

**What**: §1 rule 2 and the `E0102` row allow a shared borrow of any value as a call argument (D7); a new rule of Part I states Rust's identifier-pattern rule (D10) with a Resolution note.
**Where**: `docs/spec/part_i_values.md` (+ `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T4
**Reuses**: T1 helpers
**Requirement**: POL-06
**Closes**: A-02, A-19

**Done when**:

- [x] Test `shared_borrow_of_any_argument_is_allowed` passes in `spec_text.rs` (C31: POL-06 AC 1)
- [x] Test `identifier_pattern_rule_is_stated` passes in `spec_text.rs` (C32: POL-06 AC 2)
- [x] The two `(0.1..=10.0).contains(&n)` examples (§8, §23) are left as written: they are now conforming
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): shared borrow as an argument; identifier patterns`

---

### T6: Behavior blocks: repeated generic names, namespace scope

**What**: §10 rule 6 and its Resolution, B.3.3 item 14 (D6: the names are the module's, repeated in order); B.2.1 note 3, §8 rule 1, §3.4.3 and the `E0401` row (D13: namespace scope, any namespace of the module's file).
**Where**: `docs/spec/part_iv_behavior.md` (+ `docs/spec/appendix_b_grammar.md`, `docs/spec/part_iii_structure.md`, `docs/spec/part_i_values.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T5
**Reuses**: T1 helpers
**Requirement**: POL-05
**Closes**: A-20, A-21

**Done when**:

- [x] Test `behavior_generics_are_repeated_not_positional` passes in `spec_text.rs` (C27: POL-05 AC 1)
- [x] Test `items_sit_at_namespace_scope` passes in `spec_text.rs` (C28: POL-05 AC 2)
- [x] `E0401` still reads `the same file`
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): behavior generics are repeated; items sit at namespace scope`

---

### T7: Misplaced statements and places, in the text

**What**: The `E0404` and `E0121` rows and §10 rule 8 / §3 rule 6 (D14: `signal` and the module body included; `E0121` wins in a `fn`); B.3.4 states that a value block may hold a contribution or a commit (A-35); B.3.3 states that a place takes no parentheses (D17).
**Where**: `docs/spec/part_iv_behavior.md` (+ `docs/spec/part_i_values.md`, `docs/spec/appendix_b_grammar.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T6
**Reuses**: T1 helpers
**Requirement**: POL-07
**Closes**: A-35, A-36, A-39

**Done when**:

- [x] Test `e0404_and_e0121_state_their_precedence` passes in `spec_text.rs` (C36: POL-07 AC 1)
- [x] Test `value_blocks_may_hold_contributions_and_commits` passes in `spec_text.rs` (C37: POL-07 AC 2)
- [x] Test `place_has_no_parentheses` passes in `spec_text.rs` (C38: POL-07 AC 3)
- [x] Both rows stay phase `parse`; their existing fixtures still hold
- [x] The `E0907` row is unchanged
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): E0404 and E0121 precedence; places take no parentheses`

---

### T8: Doc comments and attributes: where they go

**What**: A.4 rule 1 lists every position of D9; B.3.2 puts `attrs` in front of `reaction`, the definition alternative of `module_item`, `gen<X>`, `nature_field`, `contribution` and `commit` (existing productions edited in place, no new production name); §21 and A.4 say which docs are metadata.
**Where**: `docs/spec/appendix_a_lexical.md` (+ `docs/spec/appendix_b_grammar.md`, `docs/spec/part_vi_io.md`, `docs/spec/part_iv_behavior.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T7
**Reuses**: T1 helpers
**Requirement**: POL-08
**Closes**: A-07

**Done when**:

- [x] Test `attrs_are_allowed_before_statements_and_hardware_items` passes in `spec_text.rs` (C39: POL-08 AC 1)
- [x] Test `quantity_and_reaction_docs_are_metadata` passes in `spec_text.rs` (C40: POL-08 AC 2)
- [x] The syntax excerpts of §4, §8 and §10 repeat the edited productions verbatim
- [x] No `rust` fence with a doc comment before a statement is added yet (T15, T16)
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): doc comments and attributes before statements and hardware items`

---

### T9: Phases of the post-parse constraints; `E0125` and bodies

**What**: B.3.3 names the phase of constraints 8 to 14 (D11: item 8 parse, the others elaboration at name resolution); the `E0125` row and §3 rule 2 cover an `extern` function with a body and a bodiless plain function (A-34).
**Where**: `docs/spec/appendix_b_grammar.md` (+ `docs/spec/part_i_values.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T8
**Reuses**: T1 helpers
**Requirement**: POL-08
**Closes**: A-34, A-38

**Done when**:

- [x] Test `post_parse_constraints_name_their_phase` passes in `spec_text.rs` (C41: POL-08 AC 3)
- [x] Test `e0125_covers_bodies` passes in `spec_text.rs` (C42: POL-08 AC 4)
- [x] `E0125`, `E0370`, `E0320`, `E0140`, `E0201` and `E0263` keep phase `elaboration`
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): phases of the grammar's constraints; E0125 covers bodies`

---

### T10: Raw identifiers and the field shorthand, in the text

**What**: A.1 rule 6 and the Resolution on `out` (D15: `r#name` for the seven added keywords only); B.3.2 `instance` and `template` fields as `IDENT ( ':' expr )?` and B.3.3 on what the shorthand means (D16); Appendix G marks G.14 and G.15 Closed in the summary table and in their entries.
**Where**: `docs/spec/appendix_a_lexical.md` (+ `docs/spec/appendix_b_grammar.md`, `docs/spec/appendix_g_scope.md`, `docs/spec/part_iii_structure.md`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T9
**Reuses**: T1 helpers
**Requirement**: POL-08
**Closes**: A-23

**Done when**:

- [x] Test `raw_identifiers_are_specified_and_g15_is_closed` passes in `spec_text.rs` (C43: POL-08 AC 5)
- [x] Test `field_shorthand_is_specified_and_g14_is_closed` passes in `spec_text.rs` (C44: POL-08 AC 6)
- [x] No `rust` fence with `r#` is added yet (T13)
- [x] The §8 syntax excerpts repeat the edited `instance` and `template` productions
- [x] Gate passes: run spec, run mslcorpus, run pomcorpus, run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(spec): raw identifiers and field-init shorthand; G.14 and G.15 closed`

---

### T11: A negative literal as a const generic argument

**What**: `generic_args` accepts `'-' literal` as a const argument (A.5 rule 4).
**Where**: `crates/piperine-lang/src/msl/parser/ty.rs` (+ `crates/piperine-lang/tests/msl_parser.rs`)
**Depends on**: T10
**Reuses**: the existing `{ expr }` const-argument path
**Requirement**: POL-09
**Closes**: A-06

**Done when**:

- [x] Test `negative_literal_is_a_const_generic_argument` passes in `msl_parser.rs` (C48: POL-09 AC 1)
- [x] Test `commit_token_in_generic_arguments_is_still_an_error` passes in `msl_parser.rs` (C49: POL-09 AC 2)
- [x] The argument is the same `Unary(Neg, Lit)` expression the braced spelling gives
- [x] Gate passes: run parser; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/msl): a negative literal is a const generic argument`

---

### T12: Function-pointer types

**What**: `TypeKind::FnPtr { params, ret }` in the AST, parsed where a type is expected, printed, lowered to a POM `TypeKind::FnPtr` and converted back; §4 rule 8 gains a `rust` example that writes the type.
**Where**: `crates/piperine-lang/src/msl/parser/ty.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/body.rs`, `crates/piperine-lang/src/pom/lower/body.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `docs/spec/part_ii_carriers.md`, `docs/pom/model.md`, `crates/piperine-lang/tests/msl_parser.rs`, `crates/piperine-lang/tests/pom_body.rs`)
**Depends on**: T11
**Reuses**: `TypeKind::Tuple` lowering as the pattern
**Requirement**: POL-09
**Closes**: A-17

**Done when**:

- [x] Test `function_pointer_types_parse` passes in `msl_parser.rs` (C50: POL-09 AC 3)
- [x] Test `function_pointer_type_round_trips` passes in `pom_body.rs` (C51: POL-09 AC 4)
- [x] `fn` at item position is unchanged: the type form is taken only where a type is expected
- [x] `type_kind_name` of `pom_body.rs` and the `TypeKind` section of `docs/pom/model.md` know the new variant
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang): function-pointer types`

---

### T13: Raw identifiers

**What**: The lexer reads `r#word` as one identifier token for the seven added keywords (`Ident.raw`), reports any other `r#` with its own message; the printer writes the prefix; the POM `Name` drops it and keeps the flag for `to_ast`; A.1 gains a `rust` example.
**Where**: `crates/piperine-lang/src/msl/lexer.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/decl.rs`, `crates/piperine-lang/src/pom/lower/mod.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `docs/spec/appendix_a_lexical.md`, `crates/piperine-lang/tests/msl_lexer.rs`, `crates/piperine-lang/tests/msl_errors.rs`, `crates/piperine-lang/tests/msl_print.rs`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T12
**Reuses**: the raw-string lexing path (`r#"`), `Name` in `pom/decl.rs`
**Requirement**: POL-10
**Closes**: A-28

**Done when**:

- [x] Test `raw_identifier_of_an_added_keyword_is_an_identifier` passes in `msl_lexer.rs` (C55: POL-10 AC 1)
- [x] Test `raw_identifier_of_another_word_is_e0900` passes in `msl_errors.rs` (C56: POL-10 AC 2)
- [x] Test `raw_identifiers_print_with_their_prefix` passes in `msl_print.rs` (C57: POL-10 AC 3)
- [x] Test `raw_identifier_names_drop_the_prefix` passes in `pom_resolve.rs` (C58: POL-10 AC 4)
- [x] A bare added keyword is still `E0903` (`keyword_as_identifier_reports_e0903` unchanged)
- [x] Name lookup compares names without the prefix: `r#out` and a path segment `r#out` are the same name
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang): raw identifiers for the seven added keywords`

---

### T14: Field-init shorthand keeps its form

**What**: `FieldInit.shorthand` in the AST (struct expressions, bundle literals, param blocks, templates), the printer writes the authored form, the POM `FieldInit` carries the flag and `to_ast` restores it.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/body.rs`, `crates/piperine-lang/src/pom/lower/body.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `crates/piperine-lang/src/pom/reflect/hardware.rs`, `docs/pom/model.md`, `crates/piperine-lang/tests/msl_parser.rs`, `crates/piperine-lang/tests/msl_print.rs`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T13
**Reuses**: the existing shorthand parse of struct expressions (`struct_expression_field_shorthand`)
**Requirement**: POL-11
**Closes**: A-23
**Existing tests**: if a test of `msl_print.rs` expects `{ r }` to print as `{ r: r }`, that one assertion changes to the authored form (D16)

**Done when**:

- [x] Test `param_block_and_template_shorthand_keep_their_form` passes in `msl_parser.rs` (C59: POL-11 AC 1)
- [x] Test `field_shorthand_prints_as_written` passes in `msl_print.rs` (C60: POL-11 AC 2)
- [x] Test `shorthand_field_binds_param_and_reads_the_local` passes in `pom_classify.rs` (C61: POL-11 AC 3)
- [x] `struct_expression_field_shorthand` (msl_parser) is strengthened with the flag
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang): field-init shorthand keeps its authored form`

---

### T15: Docs and attributes before statements

**What**: `Stmt.meta` in the AST: a block statement, a contribution and a commit take outer doc comments and attributes; a doc comment that documents nothing gets its own `E0900`; the printer writes them; the POM `StmtNode` stores them and `to_ast` restores them.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/body.rs`, `crates/piperine-lang/src/pom/lower/body.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `docs/pom/model.md`, `crates/piperine-lang/tests/msl_parser.rs`, `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T14
**Reuses**: `Meta` and the item-level `attrs` parser (`parser/item.rs`), `pom/lower/meta.rs`
**Requirement**: POL-12
**Closes**: A-07

**Done when**:

- [x] Test `docs_and_attrs_attach_to_statements` passes in `msl_parser.rs` (C62: POL-12 AC 1)
- [x] Test `dangling_doc_comment_has_its_own_message` passes in `msl_errors.rs` (C63: POL-12 AC 2)
- [x] An attribute name is not validated here (`E0904` is elaboration)
- [x] `pom_corpus` and `msl_print` round trips stay green: the metadata is part of tree equality
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang): doc comments and attributes before statements`

---

### T16: Docs and attributes before hardware items

**What**: `meta` on reactions, definitions, generation `for`/`if` (module and digital) and nature fields: parsed, printed, stored by the POM lowering and restored by `to_ast`; §12.1 gains a `rust` example with a documented reaction.
**Where**: `crates/piperine-lang/src/msl/parser/msl.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/decl.rs`, `crates/piperine-lang/src/pom/lower/hardware.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `docs/spec/part_iv_behavior.md`, `docs/pom/model.md`, `crates/piperine-lang/tests/msl_parser.rs`, `crates/piperine-lang/tests/msl_print.rs`, `crates/piperine-lang/tests/pom_corpus.rs`)
**Depends on**: T15
**Reuses**: T15's statement metadata
**Requirement**: POL-12
**Closes**: A-07

**Done when**:

- [x] Test `docs_and_attrs_attach_to_hardware_items` passes in `msl_parser.rs` (C64: POL-12 AC 3)
- [x] Test `statement_and_hardware_docs_round_trip` passes in `msl_print.rs` (C65: POL-12 AC 4)
- [x] Test `docs_at_every_position_round_trip` passes in `pom_corpus.rs` (C66: POL-12 AC 5)
- [x] The six misleading diagnostics of A-07 are gone: each of its six sources parses with no diagnostic (asserted in `docs_and_attrs_attach_to_hardware_items`)
- [x] `E0434` and `E0903` are no longer reported after a documented reaction or definition
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang): doc comments and attributes before hardware items`

---

### T17: `E0909`: literals out of range

**What**: The lexer reports a float literal whose nearest `f64` is infinite and an integer literal past its type's range; the A.6 row, A.2 rule 6 and §1 rule 7 state the rule; the parse-phase fixture table gains its `E0909` entry.
**Where**: `crates/piperine-lang/src/msl/lexer.rs` (+ `docs/spec/appendix_a_lexical.md`, `docs/spec/part_i_values.md`, `crates/piperine-lang/tests/msl_lexer.rs`, `crates/piperine-lang/tests/msl_errors.rs`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T16
**Reuses**: `parse_f64` and the suffix reader of the lexer
**Requirement**: POL-13
**Closes**: A-11, A-37

**Done when**:

- [x] Test `e0909_is_defined_for_out_of_range_literals` passes in `spec_text.rs` (C68: POL-13 AC 1)
- [x] Test `float_literal_out_of_range_is_e0909` passes in `msl_lexer.rs` (C69: POL-13 AC 2)
- [x] Test `integer_literal_out_of_range_is_e0909` passes in `msl_lexer.rs` (C70: POL-13 AC 3)
- [x] Test `every_parse_phase_row_has_a_fixture` still passes in `msl_errors.rs` (C71: POL-13 AC 4)
- [x] The spec row and its fixture land in this one commit (`every_parse_phase_row_has_a_fixture` is green before and after)
- [x] A Resolution note says why `iN` admits `2^(N-1)`: the minus sign is an operator (A.2 rule 7)
- [x] The token is still produced after the diagnostic, so the parse goes on
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): E0909 for a literal out of range`

---

### T18: String escapes, raw strings, literal accessors

**What**: The lexer validates string escapes (`E0900` at the escape), a raw string keeps its spelling (`LitKind::Str { text, raw_hashes }`) so the printer writes it back, and `Lit` gains `str_value()`, `int_value()` and `suffix()`.
**Where**: `crates/piperine-lang/src/msl/lexer.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `crates/piperine-lang/tests/msl_lexer.rs`, `crates/piperine-lang/tests/msl_print.rs`)
**Depends on**: T17
**Reuses**: `string_literals_are_printed_verbatim` (msl_print) as the baseline
**Requirement**: POL-13
**Closes**: A-27

**Done when**:

- [x] Test `invalid_string_escape_is_e0900` passes in `msl_lexer.rs` (C72: POL-13 AC 5)
- [x] Test `raw_strings_print_as_written` passes in `msl_print.rs` (C73: POL-13 AC 6)
- [x] Test `literal_accessors_return_cooked_values` passes in `msl_lexer.rs` (C74: POL-13 AC 7)
- [x] `string_literals_are_printed_verbatim` still passes unchanged
- [x] The three accessors are plain methods on `Lit`; `int_value()` is none for a literal `E0909` rejected
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): validated escapes, raw strings as written, literal accessors`

---

### T19: A commit target is a place; a place takes no parentheses

**What**: The `<-` tail validates its left side with the check the definition uses and reports `E0900` `expected a place`; a parenthesized place is rejected in definitions and commits alike.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs` (+ `crates/piperine-lang/src/msl/parser/msl.rs`, `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T18
**Reuses**: the definition's place check in `parser/msl.rs`
**Requirement**: POL-14
**Closes**: A-04, A-39, A-42

**Done when**:

- [x] Test `commit_target_must_be_a_place` passes in `msl_errors.rs` (C77: POL-14 AC 1)
- [x] Test `parenthesized_place_is_rejected_in_both_forms` passes in `msl_errors.rs` (C78: POL-14 AC 2)
- [x] One function decides `is this expression a place` for both forms
- [x] The commit node is kept in the tree after the diagnostic (recovery), with its left side as written
- [x] Gate passes: run errors; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/msl): a commit target must be a place`

---

### T20: Misplaced statements: `E0404` everywhere but a `fn`

**What**: `signal` in `analog`, a reaction or a closure is `E0404`; a commit, a contribution and a reaction written in a module body are parsed as statements and reported `E0404` with help, instead of `E0907`/`E0900`.
**Where**: `crates/piperine-lang/src/msl/parser/check.rs` (+ `crates/piperine-lang/src/msl/parser/msl.rs`, `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T19
**Reuses**: `misplaced(ctx, ..)` and the `Ctx` enum of `check.rs`
**Requirement**: POL-14
**Closes**: A-05, A-24, A-36

**Done when**:

- [x] Test `signal_outside_a_module_body_is_e0404` passes in `msl_errors.rs` (C79: POL-14 AC 3)
- [x] Test `behavior_statements_in_a_module_body_are_e0404` passes in `msl_errors.rs` (C80: POL-14 AC 4)
- [x] Test `function_body_reports_e0121_only` passes in `msl_errors.rs` (C81: POL-14 AC 5)
- [x] `statements_of_the_wrong_block_kind_report_e0404` and `forbidden_forms_in_a_function_body_report_e0121` still pass unchanged
- [x] `a_statement_outside_a_reaction_in_digital_reports_e0434` still passes: the top level of a `digital` block keeps `E0434`
- [x] Gate passes: run errors; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/msl): E0404 for signal and for behavior statements in a module body`

---

### T21: `pub` on a behavior block; forms outside the grammar

**What**: `pub analog for` / `pub digital for` report `E0900` with help; an empty nature, a tuple signal type and a reference signal type are parse errors.
**Where**: `crates/piperine-lang/src/msl/parser/msl.rs` (+ `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T20
**Reuses**: `sigtype` and `nature_decl` in `parser/msl.rs`
**Requirement**: POL-14
**Closes**: A-22, A-29
**Existing tests**: a test source that used an empty `nature X { }` as a stand-in gets one field (A-29: the grammar requires one); nothing else in that test changes

**Done when**:

- [x] Test `pub_on_a_behavior_block_is_e0900` passes in `msl_errors.rs` (C82: POL-14 AC 6)
- [x] Test `forms_outside_the_grammar_are_parse_errors` passes in `msl_errors.rs` (C83: POL-14 AC 7)
- [x] A port of value type `(Bit, Bit)` is unaffected: only `signal` and carrier positions use `sigtype`
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/msl): reject pub on a behavior block, empty natures, non-carrier signal types`

---

### T22: Removed Rust forms get targeted messages

**What**: One recognizer per row of B.3.6 (data: form, code, message, help) so each removed form gives exactly one diagnostic that names it; a body `let` with a type annotation says so.
**Where**: `crates/piperine-lang/src/msl/parser/item.rs` (+ `crates/piperine-lang/src/msl/parser/ty.rs`, `crates/piperine-lang/src/msl/parser/pat.rs`, `crates/piperine-lang/src/msl/parser/msl.rs`, `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T21
**Reuses**: the `E0903` path for reserved keywords; `fixtures()` of `msl_errors.rs` as the table shape
**Requirement**: POL-09
**Closes**: A-17, A-28

**Done when**:

- [x] Test `removed_rust_forms_report_targeted_messages` passes in `msl_errors.rs` (C52: POL-09 AC 5)
- [x] Test `every_b36_row_has_a_fixture` passes in `msl_errors.rs` (C53: POL-09 AC 6)
- [x] Test `body_let_annotation_has_its_own_message` passes in `msl_errors.rs` (C54: POL-09 AC 7)
- [x] The table of removed forms is a `const` slice of plain rows; the guard reads B.3.6's first cells from the spec
- [x] The existing `E0903` fixtures for `type` and `static` still hold (same code, same span)
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): targeted messages for the Rust forms B.3.6 removes`

---

### T23: `E0910`: nesting and flat chains counted apart

**What**: The depth pre-pass keeps two counts: nesting (256) and chain links on one path (4096, `MAX_CHAIN`); the recursive walkers of the crate (printer, POM lowering, resolver, `to_ast`) run through one stack-growing helper; the A.6 row is rewritten.
**Where**: `crates/piperine-lang/src/msl/parser/entry.rs` (+ `crates/piperine-lang/src/msl/print.rs`, `crates/piperine-lang/src/pom/lower/body.rs`, `crates/piperine-lang/src/pom/to_ast.rs`, `crates/piperine-lang/src/pom/resolve/names.rs`, `crates/piperine-lang/Cargo.toml`, `docs/spec/appendix_a_lexical.md`, `crates/piperine-lang/tests/msl_errors.rs`, `crates/piperine-lang/tests/msl_print.rs`, `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T22
**Reuses**: `is_nesting` and `skip_overflow` of `entry.rs`; `stacker` (already built as a chumsky feature) as a direct dependency
**Requirement**: POL-15
**Closes**: A-25
**Existing tests**: `nesting_without_delimiters_is_also_bounded` drops `binary +1`, `cast chain` and `else-if chain` from `CHAINS`, and `long_chains_print_without_overflow` (msl_print) moves its sum and `else if` cases to the 4096 bound: D8 says a flat chain is not nesting

**Done when**:

- [x] Test `e0910_counts_nesting_and_bounds_chains` passes in `spec_text.rs` (C85: POL-15 AC 1)
- [x] Test `flat_chains_up_to_4096_links_are_not_nesting` passes in `msl_errors.rs` (C86: POL-15 AC 2)
- [x] Test `flat_chain_past_4096_links_is_e0910` passes in `msl_errors.rs` (C87: POL-15 AC 3)
- [x] Test `nesting_without_delimiters_is_also_bounded` still passes in `msl_errors.rs` (C88: POL-15 AC 4)
- [x] `MAX_DEPTH` stays 256 and `depth_over_256_is_diagnostic_not_overflow` passes unchanged
- [x] The helper is one plain function (`deep(|| ..)`), called at the recursion points; no macro
- [x] If 4096 links cannot be met on an 8 MiB debug stack, stop and report: the bound is not lowered silently
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): E0910 counts nesting; flat chains bounded at 4096 links`

---

### T24: Recovery: a bad `where` list

**What**: Recovery inside `<..>` and after `where` resynchronizes on closing delimiters, not on item keywords, so a comma in a `where` is one diagnostic.
**Where**: `crates/piperine-lang/src/msl/parser/item.rs` (+ `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T23
**Reuses**: the recovery strategies of `parser/base.rs`
**Requirement**: POL-16
**Closes**: A-26

**Done when**:

- [x] Test `bad_where_list_is_one_diagnostic` passes in `msl_errors.rs` (C89: POL-16 AC 1)
- [x] `one_fault_is_one_diagnostic` and `multiple_errors_in_source_order_with_partial_ast` still pass unchanged
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/msl): one diagnostic for a comma in a where clause`

---

### T25: Recovery: statements inside a block

**What**: A statement that fails to parse is skipped to its `;` (or the block's `}`) and becomes an error statement; the block and its item survive.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs` (+ `crates/piperine-lang/tests/msl_errors.rs`)
**Depends on**: T24
**Reuses**: `error_expressions_become_error_nodes` (pom_body) for the node the POM expects
**Requirement**: POL-16
**Closes**: A-26

**Done when**:

- [x] Test `statement_errors_recover_inside_the_block` passes in `msl_errors.rs` (C90: POL-16 AC 2)
- [x] An unclosed `(` inside the statement is still reported at its opener (`unclosed_delimiter_is_reported_at_the_opener` unchanged)
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/msl): recover a bad statement inside its block`

---

### T26: Recovery: one port of a port list

**What**: A port that fails to parse is skipped to the next `,` or the closing `)`; the module keeps its other ports and its body.
**Where**: `crates/piperine-lang/src/msl/parser/msl.rs` (+ `crates/piperine-lang/tests/msl_errors.rs`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T25
**Reuses**: T25's skip-to-separator recovery
**Requirement**: POL-16
**Closes**: A-26

**Done when**:

- [x] Test `a_bad_port_keeps_the_module` passes in `msl_errors.rs` (C91: POL-16 AC 3)
- [x] Test `module_with_a_bad_port_still_resolves` passes in `pom_resolve.rs` (C92: POL-16 AC 4)
- [x] The bad port is not in `ports()`; no placeholder port is invented
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/msl): a bad port no longer loses the module`

---

### T27: Core names in the built-in table

**What**: `BuiltinKind::Node` and `BuiltinKind::ImplicitParam`; rows `str` (Type), `Nature` (Trait), `gnd` (Node), `m` and `dtemp` (ImplicitParam, in scope only inside a module body and the blocks bound to a module); the port classifier reads the built-in `Nature` bound; the fixture prelude loses its `Nature` stand-in.
**Where**: `crates/piperine-lang/src/pom/builtin.rs` (+ `crates/piperine-lang/src/pom/resolve/names.rs`, `crates/piperine-lang/src/pom/resolve/classify.rs`, `crates/piperine-lang/tests/fixtures/pom/prelude.msl`, `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_resolve.rs`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T26
**Reuses**: `builtin(name)` and the scope walk of `resolve/names.rs`
**Requirement**: POL-03
**Closes**: A-01, A-42

**Done when**:

- [x] Test `str_is_a_builtin_type` passes in `pom_resolve.rs` (C11: POL-03 AC 2)
- [x] Test `nature_bound_is_builtin_and_classifies_the_port` passes in `pom_classify.rs` (C12: POL-03 AC 3)
- [x] Test `gnd_is_a_builtin_node` passes in `pom_resolve.rs` (C13: POL-03 AC 4)
- [x] Test `m_and_dtemp_resolve_inside_a_module` passes in `pom_resolve.rs` (C14: POL-03 AC 5)
- [x] Test `m_and_dtemp_are_unknown_outside_a_module` passes in `pom_resolve.rs` (C15: POL-03 AC 6)
- [x] Test `a_local_shadows_an_implicit_param` passes in `pom_resolve.rs` (C16: POL-03 AC 7)
- [x] Test `core_names_cannot_be_redeclared` passes in `pom_resolve.rs` (C17: POL-03 AC 8)
- [x] Test `fixture_prelude_declares_no_core_name` passes in `pom_resolve.rs` (C18: POL-03 AC 9)
- [x] `fixture_prelude_builds_clean` and `builtin_table_has_every_core_primitive` still pass
- [x] `m` and `dtemp` are not reported by `E0457`: their rule is `E0375` (§8.5.2), left to elaboration
- [x] The stand-ins `State`, `Logic` and `init` stay in the fixture: they are standard-library items (`msl-stdlib`)
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): str, Nature, gnd, m and dtemp are built in`

---

### T28: Catalogue traits and types in the built-in table

**What**: One `Trait` or `Type` row per trait and type of Appendix H (section `App. H`), and a guard that keeps the table and the appendix equal.
**Where**: `crates/piperine-lang/src/pom/builtin.rs` (+ `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T27
**Reuses**: T27's rows; `spec_text.rs` table scraping (copied as a small helper, no shared macro)
**Requirement**: POL-02
**Closes**: A-18

**Done when**:

- [x] Test `catalogue_traits_and_types_resolve` passes in `pom_resolve.rs` (C8: POL-02 AC 5)
- [x] Test `builtin_table_matches_appendix_h` passes in `pom_resolve.rs` (C9: POL-02 AC 6)
- [x] Methods of the catalogue get no row: a method on a value needs the receiver's type and stays `Deferred(ReceiverTypeUnknown)`
- [x] The existing rows `Clone`, `Copy` move under the `App. H` section label
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): the traits and types of Appendix H are built in`

---

### T29: `use a::{self}` and leaf spans

**What**: A `self` leaf imports the namespace its prefix names (under its `as` name); every leaf path carries the span of its own segments.
**Where**: `crates/piperine-lang/src/pom/resolve/imports.rs` (+ `crates/piperine-lang/src/pom/lower/items.rs`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T28
**Reuses**: `use_tree_flattens_to_imports` (pom_tree) for the import shape
**Requirement**: POL-17
**Closes**: A-08, A-43

**Done when**:

- [x] Test `use_self_imports_the_namespace` passes in `pom_resolve.rs` (C93: POL-17 AC 1)
- [x] Test `use_self_with_alias_and_nesting` passes in `pom_resolve.rs` (C94: POL-17 AC 2)
- [x] Test `use_leaf_text_is_its_own_segments` passes in `pom_resolve.rs` (C95: POL-17 AC 3)
- [x] `use_and_qualified_paths_resolve` and `use_tree_flattens_to_imports` still pass unchanged
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/pom): a self leaf imports its namespace; leaf paths span their own text`

---

### T30: Members of `Repr` and of built-in bounds

**What**: `BuiltinKind::Member` rows `Repr::WIDTH`, `Repr::encode`, `Repr::decode`; an associated segment after a generic resolves through its bounds: a `Repr` member row, `E0161` when `Repr` is the only bound and has no such member, `Deferred(BuiltinTraitMember)` through any other built-in trait.
**Where**: `crates/piperine-lang/src/pom/resolve/names.rs` (+ `crates/piperine-lang/src/pom/builtin.rs`, `crates/piperine-lang/src/pom/resolution.rs`, `docs/pom/resolution.md`, `docs/pom/model.md`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T29
**Reuses**: `associated_paths_resolve_to_variant_const_fn` (pom_resolve) for the user-trait path
**Requirement**: POL-17
**Closes**: A-09

**Done when**:

- [x] Test `repr_members_resolve_through_a_bound` passes in `pom_resolve.rs` (C96: POL-17 AC 4)
- [x] Test `unknown_repr_member_is_e0161` passes in `pom_resolve.rs` (C97: POL-17 AC 5)
- [x] Test `member_through_a_builtin_bound_is_deferred` passes in `pom_resolve.rs` (C98: POL-17 AC 6)
- [x] A user trait bound still resolves to the trait's own item, as today
- [x] `level` stays a `Method` row (it is a value method, §3.3.4)
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/pom): associated items through a built-in bound`

---

### T31: `E0162` in the scopes stage

**What**: The scopes stage keeps one type table and one value table per namespace and reports the three conditions of §3.4; `E0143` keeps the function pair.
**Where**: `crates/piperine-lang/src/pom/resolve/scopes.rs` (+ `crates/piperine-lang/src/pom/resolve/imports.rs`, `crates/piperine-lang/src/pom/resolve/names.rs`, `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T30
**Reuses**: the `E0330` member check of `scopes.rs` as the reporting pattern
**Requirement**: POL-04
**Closes**: A-10, A-40, A-46
**Existing tests**: a fixture of `pom_resolve.rs`/`pom_classify.rs` that declared one name twice by accident is renamed (D5); the test's own assertions do not change

**Done when**:

- [x] Test `duplicate_item_is_e0162_and_both_are_kept` passes in `pom_resolve.rs` (C21: POL-04 AC 3)
- [x] Test `import_colliding_with_an_item_is_e0162` passes in `pom_resolve.rs` (C22: POL-04 AC 4)
- [x] Test `ambiguous_glob_is_e0162_only_when_used` passes in `pom_resolve.rs` (C23: POL-04 AC 5)
- [x] Test `two_globs_of_one_declaration_are_clean` passes in `pom_resolve.rs` (C24: POL-04 AC 6)
- [x] Test `two_functions_of_one_name_stay_e0143` passes in `pom_resolve.rs` (C25: POL-04 AC 7)
- [x] Test `one_name_in_two_tables_is_clean` passes in `pom_resolve.rs` (C26: POL-04 AC 8)
- [x] Both declarations are kept; resolution adds no node and removes none (`resolution_creates_no_node` unchanged)
- [x] `local_item_wins_over_glob` still passes unchanged
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): E0162 for duplicate names, colliding imports and ambiguous globs`

---

### T32: Identifier patterns that name a constant

**What**: `PatKind::Ident` gains a `res: Resolution` slot; resolution fills it when the plain name resolves to a const, a unit variant, a unit struct or a built-in variant, does not enter the binding in scope, and indexes the reference; `Binding::is_binding()` answers.
**Where**: `crates/piperine-lang/src/pom/resolve/names.rs` (+ `crates/piperine-lang/src/pom/body.rs`, `crates/piperine-lang/src/pom/index.rs`, `crates/piperine-lang/src/pom/reflect/hardware.rs`, `docs/pom/model.md`, `docs/pom/reflection.md`, `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T31
**Reuses**: `binders_scope_their_names` (pom_resolve) for the binder walk
**Requirement**: POL-06
**Closes**: A-02, A-42

**Done when**:

- [x] Test `identifier_pattern_naming_none_is_a_path` passes in `pom_resolve.rs` (C33: POL-06 AC 3)
- [x] Test `identifier_pattern_naming_a_const_variant_or_unit_struct_is_a_path` passes in `pom_resolve.rs` (C34: POL-06 AC 4)
- [x] Test `other_identifier_patterns_stay_bindings` passes in `pom_resolve.rs` (C35: POL-06 AC 5)
- [x] The `Binding` declaration and the pattern node stay (MD-54 rule 1: resolution adds links only)
- [x] `to_ast` is unchanged: the AST keeps `PatKind::Ident`
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/pom): an identifier pattern naming a constant is a path, not a binding`

---

### T33: Behavior names by name; a misplaced `signal` declares nothing

**What**: On `E0403` the block's names link to the module's generics by name; a `signal` statement the parser reported as misplaced is lowered (the tree stays lossless) but entered in no scope; a test pins the cross-namespace binding of D13.
**Where**: `crates/piperine-lang/src/pom/resolve/behaviors.rs` (+ `crates/piperine-lang/src/pom/resolve/scopes.rs`, `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_classify.rs`, `crates/piperine-lang/tests/pom_resolve.rs`)
**Depends on**: T32
**Reuses**: `behavior_names_mismatch_is_e0403` (pom_classify)
**Requirement**: POL-05, POL-14
**Closes**: A-05, A-20, A-21

**Done when**:

- [x] Test `mismatched_behavior_names_bind_by_name` passes in `pom_classify.rs` (C29: POL-05 AC 3)
- [x] Test `behavior_binds_across_namespaces_of_one_file` passes in `pom_classify.rs` (C30: POL-05 AC 4)
- [x] Test `misplaced_signal_declares_nothing` passes in `pom_resolve.rs` (C84: POL-14 AC 8)
- [x] `behavior_names_mismatch_is_e0403` still passes unchanged
- [x] The misplaced `Signal` declaration stays in `decls()` with `module() == None`
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/pom): behavior generics link by name; a misplaced signal is not in scope`

---

### T34: `E0320` and `E0140` for param blocks and named arguments

**What**: The classifier reports an unknown or repeated param name as `E0320` and a named argument that names no optional parameter, or a bound one, as `E0140`.
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs` (+ `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T33
**Reuses**: the `no param` / `no port` / `no parameter` messages of `classify.rs`
**Requirement**: POL-18
**Closes**: A-14
**Existing tests**: `named_binding_without_a_target_is_e0161` (pom_classify) is renamed `named_binding_without_a_target_reports_the_spec_code` and its expected codes change from `E0161` to `E0320`/`E0140`, as do the `["E0161"]` assertions of that file whose source is an unknown param, port or parameter name (D11)

**Done when**:

- [x] Test `param_block_errors_are_e0320` passes in `pom_classify.rs` (C99: POL-18 AC 1)
- [x] Test `named_argument_errors_are_e0140` passes in `pom_classify.rs` (C100: POL-18 AC 2)
- [x] `optional` is syntactic: the declared type is written `Option<..>`
- [x] The slot of the offending name stays `Unresolved`
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/pom): E0320 and E0140 where the spec assigns them`

---

### T35: `E0263` for bundle literals; struct fields in literals and patterns

**What**: An unknown or repeated field of a bundle literal is `E0263`; an unknown field of a struct pattern is reported like one of a struct literal (`E0161`).
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs` (+ `crates/piperine-lang/src/pom/resolve/members.rs`, `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T34
**Reuses**: T34's reporting helper
**Requirement**: POL-18
**Closes**: A-14, A-45

**Done when**:

- [x] Test `bundle_literal_field_errors_are_e0263` passes in `pom_classify.rs` (C101: POL-18 AC 3)
- [x] Test `unknown_struct_field_is_reported_in_literals_and_patterns` passes in `pom_classify.rs` (C102: POL-18 AC 4)
- [x] The shorthand pattern `S { a }` still binds `a` and resolves the field
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/pom): E0263 for bundle literals; unknown fields in struct patterns`

---

### T36: `E0370`, `E0125` and `E0201`

**What**: Three structural checks in a new resolver pass: the shape of an `extern` module, the body rule of functions, and the fields of a nature.
**Where**: `crates/piperine-lang/src/pom/resolve/shape.rs` (+ `crates/piperine-lang/src/pom/resolve/mod.rs`, `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T35
**Reuses**: `NatureKind` computation in `reflect/hardware.rs`; `behaviors.rs` as the pass shape
**Requirement**: POL-18
**Closes**: A-29, A-34, A-38
**Existing tests**: a test whose source holds an invalid nature or a bodiless plain function on purpose now also sees `E0201`/`E0125`: its diagnostics assertion gains that code (D11), nothing else changes

**Done when**:

- [x] Test `extern_module_shape_is_e0370` passes in `pom_classify.rs` (C103: POL-18 AC 5)
- [x] Test `function_body_rule_is_e0125` passes in `pom_classify.rs` (C104: POL-18 AC 6)
- [x] Test `nature_field_errors_are_e0201` passes in `pom_classify.rs` (C105: POL-18 AC 7)
- [x] The pass reads declarations only: no types, no const evaluation
- [x] A trait's bodiless `fn` is not reported
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): E0370, E0125 and E0201 as structural checks`

---

### T37: An access call resolves by the nature of its first argument

**What**: `probe_callee` never falls back to the first quantity of that name: with a known nature that lacks the access it reports `E0206` and leaves the callee `Unresolved`.
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs` (+ `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T36
**Reuses**: `access_alias_resolves_by_first_argument` (pom_classify)
**Requirement**: POL-19
**Closes**: A-15

**Done when**:

- [x] Test `access_of_another_nature_is_e0206` passes in `pom_classify.rs` (C106: POL-19 AC 1)
- [x] `access_alias_resolves_by_first_argument` and `access_name_resolves_to_its_quantity` still pass unchanged
- [x] An argument whose nature needs inference still gives `Deferred(AmbiguousAccess)` and no diagnostic
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/pom): an access call never resolves to another nature's quantity`

---

### T38: A carrier method on an array of carriers

**What**: `receiver` looks through an array only after an index; a carrier method called on the array itself is `E0211`.
**Where**: `crates/piperine-lang/src/pom/resolve/members.rs` (+ `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_classify.rs`)
**Depends on**: T37
**Reuses**: `carrier_nature_through_array_and_flip` (pom_classify)
**Requirement**: POL-19
**Closes**: A-16

**Done when**:

- [x] Test `carrier_method_on_an_array_is_e0211` passes in `pom_classify.rs` (C107: POL-19 AC 2)
- [x] A name that is not a carrier method (`arr.len()`) stays `Deferred(ReceiverTypeUnknown)` with no diagnostic
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/pom): a carrier method on an array is E0211, not the element's impl`

---

### T39: Writers through a resolved member

**What**: The reverse index records a definition or commit whose place ends in a resolved member (`c.w`) as a writer of that member's declaration.
**Where**: `crates/piperine-lang/src/pom/index.rs` (+ `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_reflect.rs`)
**Depends on**: T38
**Reuses**: `writers_are_definitions_and_commits`, `port_writers_and_field_roots` (pom_reflect)
**Requirement**: POL-19
**Closes**: A-30

**Done when**:

- [x] Test `parent_drive_is_a_writer_of_the_child_carrier` passes in `pom_reflect.rs` (C108: POL-19 AC 3)
- [x] Both existing writer tests still pass unchanged
- [x] The root `c` is not listed as written by `c.w = ..`
- [x] Gate passes: run reflect; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `fix(lang/pom): a parent's drive is a writer of the child's carrier`

---

### T40: `Design` collections: one precedence, one inclusion rule

**What**: Every by-name lookup prefers project files; every typed collection lists project files only; new `enums()`, `traits()`, `consts()`, `impls()`, `struct_named(name)`, `function(name)`, `prelude_files()`.
**Where**: `crates/piperine-lang/src/pom/reflect/mod.rs` (+ `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_reflect.rs`)
**Depends on**: T39
**Reuses**: `Design::module` (already project-first)
**Requirement**: POL-20
**Closes**: A-12
**Existing tests**: tests that counted prelude items through `modules()` or `natures()` read them from `prelude_files()` instead (A-12: one inclusion rule); the counts they assert do not change

**Done when**:

- [x] Test `by_name_lookups_prefer_project_files` passes in `pom_reflect.rs` (C109: POL-20 AC 1)
- [x] Test `collections_list_project_files_only` passes in `pom_reflect.rs` (C110: POL-20 AC 2)
- [x] Test `design_has_a_collection_and_lookup_per_item_kind` passes in `pom_reflect.rs` (C111: POL-20 AC 3)
- [x] `module_lookup_prefers_authored_files` still passes unchanged
- [x] `decls()` and `files()` still list everything, prelude included
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): Design collections and lookups follow one rule`

---

### T41: `Expr::children`, `Expr::parent`, `Decl::exprs`

**What**: Child and parent navigation over every `ExprKind`, and the pre-order list of a declaration's own body expressions. The parent table is a side table filled by lowering.
**Where**: `crates/piperine-lang/src/pom/reflect/body.rs` (+ `crates/piperine-lang/src/pom/body.rs`, `crates/piperine-lang/src/pom/lower/body.rs`, `crates/piperine-lang/src/pom/check.rs`, `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_body.rs`)
**Depends on**: T40
**Reuses**: `every_expression_form_has_its_kind` (pom_body) for one source per `ExprKind`
**Requirement**: POL-20
**Closes**: A-13

**Done when**:

- [x] Test `expr_children_and_parent_are_inverse` passes in `pom_body.rs` (C112: POL-20 AC 4)
- [x] Test `decl_exprs_is_preorder_over_the_body` passes in `pom_body.rs` (C113: POL-20 AC 5)
- [x] One function enumerates the children of an `ExprKind`; `parent()` is checked against it by `check()`
- [x] The parent table serializes with the design (`gallery_design_round_trips_through_json` still passes)
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): walk expressions by children, parent and pre-order`

---

### T42: `Block` and statement handles

**What**: A `Block` handle (`stmts()`, `value()`, `span()`), `Stmt::children()`, `Expr::stmt()`; `Function::body()` and `Reaction::body()` return the handle (the raw block stays reachable as `node()`).
**Where**: `crates/piperine-lang/src/pom/reflect/body.rs` (+ `crates/piperine-lang/src/pom/reflect/items.rs`, `crates/piperine-lang/src/pom/reflect/hardware.rs`, `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_body.rs`)
**Depends on**: T41
**Reuses**: T41's child enumeration
**Requirement**: POL-20
**Closes**: A-13

**Done when**:

- [x] Test `block_and_stmt_handles_walk_a_body` passes in `pom_body.rs` (C114: POL-20 AC 6)
- [x] Call sites in the existing tests move from the raw block to the handle; their assertions do not change
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): Block handle and statement navigation`

---

### T43: Argument and field handles; `Instance::builtin`; impls of a type

**What**: Handles `Argument` and `FieldBinding` (`name()`, `value()`, `target()`), `Expr::args()`, `Expr::fields()`, `Instance::param_bindings()` returning handles, `Instance::builtin()`, `Struct::impls()`/`method(name)`, `Enum::impls()`/`method(name)`, `Trait::impls()`.
**Where**: `crates/piperine-lang/src/pom/reflect/hardware.rs` (+ `crates/piperine-lang/src/pom/reflect/body.rs`, `crates/piperine-lang/src/pom/reflect/items.rs`, `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_classify.rs`, `crates/piperine-lang/tests/pom_reflect.rs`)
**Depends on**: T42
**Reuses**: `Nature::impls`, `instance_connection_by_port` (pom_reflect)
**Requirement**: POL-20
**Closes**: A-13, A-44

**Done when**:

- [x] Test `arg_and_field_handles_expose_name_value_target` passes in `pom_classify.rs` (C115: POL-20 AC 7)
- [x] Test `builtin_instance_names_its_primitive` passes in `pom_classify.rs` (C116: POL-20 AC 8)
- [x] Test `types_list_their_impls_and_methods` passes in `pom_reflect.rs` (C117: POL-20 AC 9)
- [x] `target()` reads the slots the classifier already fills; this task resolves nothing new
- [x] `nature_impls_lists_its_impl_blocks` still passes unchanged
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): handles for arguments and fields; builtin instances; impls of a type`

---

### T44: `Generation::items` over every arm; `docs()` on the new positions

**What**: `Generation::items()` of an `if` returns every arm's items; `Reaction::docs()`, `Quantity::docs()`, `Definition::docs()`, `Generation::docs()` and `Stmt::docs()` read the metadata T15 and T16 store.
**Where**: `crates/piperine-lang/src/pom/reflect/hardware.rs` (+ `crates/piperine-lang/src/pom/reflect/body.rs`, `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_tree.rs`)
**Depends on**: T43
**Reuses**: `doc_comments_reach_docs` (pom_tree), `Reflect::docs`
**Requirement**: POL-12, POL-20
**Closes**: A-07, A-31
**Existing tests**: if `generation_if_chain_exposes_three_arms` or `generation_body_is_not_unrolled` asserts that `items()` of an `if` is the first arm only, that one assertion changes to every arm (A-31)

**Done when**:

- [x] Test `docs_reach_reactions_quantities_definitions_and_statements` passes in `pom_tree.rs` (C67: POL-12 AC 6)
- [x] Test `generation_if_items_cover_every_arm` passes in `pom_tree.rs` (C118: POL-20 AC 10)
- [x] `generation_if_chain_exposes_three_arms` still passes: `arms()` is unchanged
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): Generation::items covers every arm; docs() on reactions, quantities, definitions, statements`

---

### T45: Float literals from their text; `Design::from_json`

**What**: `LitKind::Float` serializes its raw text only and recomputes the value on read; `Design::from_json(text)` deserializes, runs `check()` and returns `Err(Vec<String>)` on any problem.
**Where**: `crates/piperine-lang/src/pom/design.rs` (+ `crates/piperine-lang/src/msl/ast.rs`, `docs/pom/reflection.md`, `docs/pom/model.md`, `crates/piperine-lang/tests/pom_reflect.rs`)
**Depends on**: T44
**Reuses**: `check()` and `check_reports_every_kind_of_broken_link` (pom_corpus), `gallery_design_round_trips_through_json`
**Requirement**: POL-13, POL-21
**Closes**: A-11, A-33, A-42

**Done when**:

- [x] Test `float_literals_round_trip_through_json` passes in `pom_reflect.rs` (C75: POL-13 AC 8)
- [x] Test `float_literal_is_serialized_from_its_text` passes in `pom_reflect.rs` (C76: POL-13 AC 9)
- [x] Test `from_json_reads_back_a_valid_design` passes in `pom_reflect.rs` (C119: POL-21 AC 1)
- [x] Test `from_json_rejects_a_corrupted_design` passes in `pom_reflect.rs` (C120: POL-21 AC 2)
- [x] `gallery_design_round_trips_through_json` and `json_is_deterministic` still pass unchanged
- [x] `docs/pom` says that the derived `Deserialize` is unvalidated and `from_json` is the entry point
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `fix(lang/pom): floats serialize from their text; Design::from_json validates`

---

### T46: Sorted name tables

**What**: Member tables of the scopes stage and the by-name lookups of `Design` and `Module` use `BTreeMap`s built once; the two scaling tests pin it.
**Where**: `crates/piperine-lang/src/pom/resolve/scopes.rs` (+ `crates/piperine-lang/src/pom/design.rs`, `crates/piperine-lang/src/pom/reflect/mod.rs`, `crates/piperine-lang/src/pom/reflect/hardware.rs`, `crates/piperine-lang/tests/pom_design.rs`)
**Depends on**: T45
**Reuses**: the `BTreeMap` side tables of the design (MD-54)
**Requirement**: POL-21
**Closes**: A-32

**Done when**:

- [x] Test `member_tables_scale_subquadratically` passes in `pom_design.rs` (C121: POL-21 AC 3)
- [x] Test `name_lookups_do_not_scan` passes in `pom_design.rs` (C122: POL-21 AC 4)
- [x] `two_builds_dump_identical` still passes: iteration order is the key order or source order, never a hash order
- [x] The lookup tables are `#[serde(skip)]` and rebuilt on read, or serialized deterministically
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `perf(lang/pom): name tables are maps, not scans`

---

### T47: `docs/pom/mapping.md`: the 65 constructs of B.1

**What**: A table `Constructs of B.1` with one row per number of B.1.2, each naming the POM type or method that represents it, and the guard that keeps it complete.
**Where**: `docs/pom/mapping.md` (+ `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T46
**Reuses**: `every_b3_production_is_mapped` (pom_docs) as the guard shape
**Requirement**: POL-22
**Closes**: A-41

**Done when**:

- [x] Test `every_b1_construct_is_mapped` passes in `pom_docs.rs` (C123: POL-22 AC 1)
- [x] The guard reads the row numbers of B.1.2 from the spec; a construct added there without a row fails
- [x] Gate passes: run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(pom): map the 65 constructs of B.1`

---

### T48: `docs/pom`: places, diagnostics, built-ins

**What**: The `place` row of mapping.md and `Signal::writers` of reflection.md lose `(s)`; the Diagnostics table of resolution.md lists every code `src/pom` reports; its built-in table names every row; three guards.
**Where**: `docs/pom/resolution.md` (+ `docs/pom/mapping.md`, `docs/pom/reflection.md`, `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T47
**Reuses**: `reflect_methods()` scraping in `pom_docs.rs`
**Requirement**: POL-22
**Closes**: A-39

**Done when**:

- [x] Test `place_has_no_parenthesized_form_in_the_docs` passes in `pom_docs.rs` (C124: POL-22 AC 2)
- [x] Test `every_code_the_pom_reports_is_documented` passes in `pom_docs.rs` (C125: POL-22 AC 3)
- [x] Test `every_builtin_row_is_documented` passes in `pom_docs.rs` (C126: POL-22 AC 4)
- [x] Test `every_reflection_method_is_documented` still passes in `pom_docs.rs` (C127: POL-22 AC 5)
- [x] `every_public_pom_type_has_a_heading` and `msl_fences_parse_clean` pass
- [x] The diagnostics guard scrapes `Diagnostic::new("E` in `src/pom/**/*.rs`
- [x] Gate passes: run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(pom): places, every reported code, every built-in row`

---

### T49: Appendix G records the decisions

**What**: `Closed in this specification` gains one row per decision D1 to D17 except D4 (question, decision, deciding section); G.38 stays Open and names the standard-library sources as the open part; a last guard.
**Where**: `docs/spec/appendix_g_scope.md` (+ `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: T48
**Reuses**: the existing `Closed in this specification` table
**Requirement**: POL-08, POL-23

**Done when**:

- [x] Test `appendix_g_records_the_decisions` passes in `spec_text.rs` (C47: POL-08 AC 9)
- [x] Test `every_valid_spec_block_parses_clean` still passes in `msl_corpus.rs` (C128: POL-23 AC 1)
- [x] Test `every_spec_block_round_trips_to_its_ast` still passes in `pom_corpus.rs` (C129: POL-23 AC 2)
- [x] Every `**Run**` line of this file is green (`cargo test -p piperine-lang`)
- [x] No `Resolution` note of an earlier task is left contradicting a decision: `grep -n positionally docs/spec` and `grep -n "prelude name" docs/spec/part_ii_carriers.md` are empty
- [x] Gate passes: `cargo test -p piperine-lang` (0 failed); no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `docs(spec): Appendix G records the decisions of the msl audit`

---

## Checks

One check per acceptance criterion of `spec.md`. Each proof names the line its test prints.

**C1** - B.3.6 holds a row for each of the 17 removed forms and says the list is complete (POL-01, AC 1)
Proof: run spec | test b36_lists_every_removed_form ... ok

**C2** - B.3.6 names function-pointer types and the negative literal const argument as taken in (POL-01, AC 2)
Proof: run spec | test b36_names_the_forms_taken_in ... ok

**C3** - every row of the removed-forms table carries one of the four codes and a replacement, and no row starts with a code (POL-01, AC 3)
Proof: run spec | test b36_rows_carry_a_code_and_a_replacement ... ok

**C4** - Appendix H exists with its nine groups and the ten iterator adaptors (POL-02, AC 1)
Proof: run spec | test appendix_h_has_every_group ... ok

**C5** - every method called in a spec fence is a row of Appendix H, a B.1 method or a function a fence declares (POL-02, AC 2)
Proof: run spec | test every_method_called_in_a_spec_fence_is_catalogued ... ok

**C6** - §0.4 and Appendix H both say a library name outside the catalogue is `E0161` (POL-02, AC 3)
Proof: run spec | test catalogue_is_closed_by_e0161 ... ok

**C7** - the §0.8 file map and the mkdocs nav both list `appendix_h_library.md` (POL-02, AC 4)
Proof: run spec | test appendix_h_is_in_the_file_map_and_nav ... ok

**C8** - `T: PartialOrd`, `#[derive(Default)]` and a catalogue type resolve to built-in rows with no `E0161` (POL-02, AC 5)
Proof: run resolve | test catalogue_traits_and_types_resolve ... ok

**C9** - the `Trait` and `Type` rows of `BUILTINS` equal Appendix H's lists plus `Repr`, `Nature`, `Fn` and the §1 types (POL-02, AC 6)
Proof: run resolve | test builtin_table_matches_appendix_h ... ok

**C10** - §6 rule 11, A.1 rule 8 and §3.4.2 say `core name`; no sentence calls `gnd` a prelude name (POL-03, AC 1)
Proof: run spec | test gnd_and_the_implicit_params_are_core_names ... ok

**C11** - `&'static str` resolves `str` to a built-in `Type` with zero diagnostics (POL-03, AC 2)
Proof: run resolve | test str_is_a_builtin_type ... ok

**C12** - `N: Nature` resolves to the built-in trait and port `g: N` is `PortKind::Carrier` (POL-03, AC 3)
Proof: run classify | test nature_bound_is_builtin_and_classifies_the_port ... ok

**C13** - `gnd` resolves to a built-in of kind `Node` in a body, an `analog` block and a reaction (POL-03, AC 4)
Proof: run resolve | test gnd_is_a_builtin_node ... ok

**C14** - `m` and `dtemp` resolve to built-ins of kind `ImplicitParam` in a body and in a block (POL-03, AC 5)
Proof: run resolve | test m_and_dtemp_resolve_inside_a_module ... ok

**C15** - outside a module `m` is `Unresolved` with one `E0161` (POL-03, AC 6)
Proof: run resolve | test m_and_dtemp_are_unknown_outside_a_module ... ok

**C16** - a local `let m` shadows the implicit param (POL-03, AC 7)
Proof: run resolve | test a_local_shadows_an_implicit_param ... ok

**C17** - an item named `gnd`, `Nature` or `str` is `E0457` (POL-03, AC 8)
Proof: run resolve | test core_names_cannot_be_redeclared ... ok

**C18** - the fixture prelude declares none of the five core names and still builds clean (POL-03, AC 9)
Proof: run resolve | test fixture_prelude_declares_no_core_name ... ok

**C19** - §3.4 defines `E0162` (elaboration) with its three conditions (POL-04, AC 1)
Proof: run spec | test e0162_is_defined_in_3_4 ... ok

**C20** - §3.4 states glob shadowing, the two-table names and that `E0143` keeps the function case (POL-04, AC 2)
Proof: run spec | test glob_shadowing_and_e0143_are_stated ... ok

**C21** - each later duplicate is one `E0162` at its name, all declarations stay, the name resolves to the first (POL-04, AC 3)
Proof: run resolve | test duplicate_item_is_e0162_and_both_are_kept ... ok

**C22** - an explicit import that collides with an item or another explicit import is one `E0162` at the later one (POL-04, AC 4)
Proof: run resolve | test import_colliding_with_an_item_is_e0162 ... ok

**C23** - an ambiguous glob name is silent until used, then one `E0162` at the use and no `E0161` (POL-04, AC 5)
Proof: run resolve | test ambiguous_glob_is_e0162_only_when_used ... ok

**C24** - two globs that reach the same declaration resolve cleanly (POL-04, AC 6)
Proof: run resolve | test two_globs_of_one_declaration_are_clean ... ok

**C25** - two functions of one name give `E0143` and no `E0162` (POL-04, AC 7)
Proof: run resolve | test two_functions_of_one_name_stay_e0143 ... ok

**C26** - one name in the type table and in the value table is clean (POL-04, AC 8)
Proof: run resolve | test one_name_in_two_tables_is_clean ... ok

**C27** - §10 says the names are repeated in order and no longer says `positionally` (POL-05, AC 1)
Proof: run spec | test behavior_generics_are_repeated_not_positional ... ok

**C28** - B.2.1 note 3, §8 rule 1 and §3.4.3 say `namespace scope` and allow any namespace of the file (POL-05, AC 2)
Proof: run spec | test items_sit_at_namespace_scope ... ok

**C29** - on `E0403` the block's names link by name (`N` to the const generic `N`), never by position (POL-05, AC 3)
Proof: run classify | test mismatched_behavior_names_bind_by_name ... ok

**C30** - a block binds to its module from another namespace of the same file (POL-05, AC 4)
Proof: run classify | test behavior_binds_across_namespaces_of_one_file ... ok

**C31** - §1 rule 2 and the `E0102` row allow `&value` as a call argument (POL-06, AC 1)
Proof: run spec | test shared_borrow_of_any_argument_is_allowed ... ok

**C32** - Part I states the identifier-pattern rule (POL-06, AC 2)
Proof: run spec | test identifier_pattern_rule_is_stated ... ok

**C33** - the pattern `None` resolves to the built-in variant and is not a binding (POL-06, AC 3)
Proof: run resolve | test identifier_pattern_naming_none_is_a_path ... ok

**C34** - patterns naming a const, an imported unit variant, a unit struct and `Dc` resolve to them and are listed as references (POL-06, AC 4)
Proof: run resolve | test identifier_pattern_naming_a_const_variant_or_unit_struct_is_a_path ... ok

**C35** - every other identifier pattern stays a binding (POL-06, AC 5)
Proof: run resolve | test other_identifier_patterns_stay_bindings ... ok

**C36** - the `E0404` and `E0121` rows cover `signal` and the module body and state that `E0121` wins in a `fn` (POL-07, AC 1)
Proof: run spec | test e0404_and_e0121_state_their_precedence ... ok

**C37** - B.3.4 says a value block may hold a contribution or a commit (POL-07, AC 2)
Proof: run spec | test value_blocks_may_hold_contributions_and_commits ... ok

**C38** - B.3.3 says a place is never parenthesized (POL-07, AC 3)
Proof: run spec | test place_has_no_parentheses ... ok

**C39** - A.4 and B.3.2 put `attrs` before statements, reactions, definitions, generation, nature fields, contributions and commits (POL-08, AC 1)
Proof: run spec | test attrs_are_allowed_before_statements_and_hardware_items ... ok

**C40** - A.4 and §21 route quantity and reaction docs to metadata and statement docs nowhere (POL-08, AC 2)
Proof: run spec | test quantity_and_reaction_docs_are_metadata ... ok

**C41** - B.3.3 names the phase of constraints 8 to 14 (POL-08, AC 3)
Proof: run spec | test post_parse_constraints_name_their_phase ... ok

**C42** - the `E0125` row covers an `extern` function with a body and a bodiless plain function (POL-08, AC 4)
Proof: run spec | test e0125_covers_bodies ... ok

**C43** - A.1 rule 6 specifies raw identifiers and G.15 is Closed (POL-08, AC 5)
Proof: run spec | test raw_identifiers_are_specified_and_g15_is_closed ... ok

**C44** - B.3.2 specifies the field shorthand and G.14 is Closed (POL-08, AC 6)
Proof: run spec | test field_shorthand_is_specified_and_g14_is_closed ... ok

**C45** - no code is mentioned without a defining row and none is defined twice (POL-08, AC 7)
Proof: run spec | test every_error_code_is_defined_exactly_once ... ok

**C46** - B.1 still counts 65 constructs in 65 rows (POL-08, AC 8)
Proof: run spec | test b1_still_counts_65_constructs ... ok

**C47** - Appendix G records the sixteen decisions with their sections and G.38 stays Open (POL-08, AC 9)
Proof: run spec | test appendix_g_records_the_decisions ... ok

**C48** - `Int< -1 >` and `Int<{ -1 }>` parse clean to a const argument `-1` and print as written (POL-09, AC 1)
Proof: run parser | test negative_literal_is_a_const_generic_argument ... ok

**C49** - `Int<-1>` is still an error at `<-` (POL-09, AC 2)
Proof: run parser | test commit_token_in_generic_arguments_is_still_an_error ... ok

**C50** - function-pointer types parse to `TypeKind::FnPtr` in four positions and print as written (POL-09, AC 3)
Proof: run parser | test function_pointer_types_parse ... ok

**C51** - a function-pointer type lowers to `TypeKind::FnPtr` and converts back losslessly (POL-09, AC 4)
Proof: run body | test function_pointer_type_round_trips ... ok

**C52** - each removed form gives one diagnostic with its row's code, a message naming the form and a help naming the replacement (POL-09, AC 5)
Proof: run errors | test removed_rust_forms_report_targeted_messages ... ok

**C53** - the fixture table has one entry per B.3.6 row (POL-09, AC 6)
Proof: run errors | test every_b36_row_has_a_fixture ... ok

**C54** - a body `let` with an annotation gives one `E0900` saying so (POL-09, AC 7)
Proof: run errors | test body_let_annotation_has_its_own_message ... ok

**C55** - each of the seven `r#` spellings lexes to a raw identifier and `signal r#out` parses clean (POL-10, AC 1)
Proof: run lexer | test raw_identifier_of_an_added_keyword_is_an_identifier ... ok

**C56** - `r#type`, `r#in` and `r#foo` each give one `E0900` listing the seven words (POL-10, AC 2)
Proof: run errors | test raw_identifier_of_another_word_is_e0900 ... ok

**C57** - the printer keeps `r#out` and round-trips (POL-10, AC 3)
Proof: run print | test raw_identifiers_print_with_their_prefix ... ok

**C58** - a raw-named signal is named `out`, resolves, and converts back losslessly (POL-10, AC 4)
Proof: run resolve | test raw_identifier_names_drop_the_prefix ... ok

**C59** - `{ r }` parses with `shorthand == true` and `{ r: r }` with `false` (POL-11, AC 1)
Proof: run parser | test param_block_and_template_shorthand_keep_their_form ... ok

**C60** - the printer writes each field in its authored form (POL-11, AC 2)
Proof: run print | test field_shorthand_prints_as_written ... ok

**C61** - a shorthand field binds the param, reads the local `r`, and converts back losslessly (POL-11, AC 3)
Proof: run classify | test shorthand_field_binds_param_and_reads_the_local ... ok

**C62** - docs and attributes before a `let`, an expression statement, a contribution and a commit attach to the statement (POL-12, AC 1)
Proof: run parser | test docs_and_attrs_attach_to_statements ... ok

**C63** - a dangling doc comment is one `E0900` at the comment with its own message (POL-12, AC 2)
Proof: run errors | test dangling_doc_comment_has_its_own_message ... ok

**C64** - docs and attributes attach to reactions, definitions, generation and nature fields (POL-12, AC 3)
Proof: run parser | test docs_and_attrs_attach_to_hardware_items ... ok

**C65** - docs in every new position round-trip through the printer (POL-12, AC 4)
Proof: run print | test statement_and_hardware_docs_round_trip ... ok

**C66** - docs in every new position convert back losslessly from the POM (POL-12, AC 5)
Proof: run pomcorpus | test docs_at_every_position_round_trip ... ok

**C67** - `docs()` of a reaction, a quantity, a definition, a generation and a statement return the authored text (POL-12, AC 6)
Proof: run tree | test docs_reach_reactions_quantities_definitions_and_statements ... ok

**C68** - A.6 defines `E0909` (parse) and A.2 and §1 rule 7 state the range rule (POL-13, AC 1)
Proof: run spec | test e0909_is_defined_for_out_of_range_literals ... ok

**C69** - `1e999` and an SI literal past the range are `E0909`; the largest finite `f64` is clean (POL-13, AC 2)
Proof: run lexer | test float_literal_out_of_range_is_e0909 ... ok

**C70** - integer literals past their type's range are `E0909` and the boundary values are clean (POL-13, AC 3)
Proof: run lexer | test integer_literal_out_of_range_is_e0909 ... ok

**C71** - the parse-phase rows of the spec, `E0909` included, each have a fixture (POL-13, AC 4)
Proof: run errors | test every_parse_phase_row_has_a_fixture ... ok

**C72** - an undefined string escape is one `E0900` at the escape (POL-13, AC 5)
Proof: run lexer | test invalid_string_escape_is_e0900 ... ok

**C73** - a raw string prints in its raw spelling (POL-13, AC 6)
Proof: run print | test raw_strings_print_as_written ... ok

**C74** - `str_value()`, `int_value()` and `suffix()` return the cooked string, the integer and the type suffix (POL-13, AC 7)
Proof: run lexer | test literal_accessors_return_cooked_values ... ok

**C75** - a design with out-of-range and extreme float literals round-trips through JSON (POL-13, AC 8)
Proof: run reflect | test float_literals_round_trip_through_json ... ok

**C76** - the JSON of a float literal carries the raw text and no number (POL-13, AC 9)
Proof: run reflect | test float_literal_is_serialized_from_its_text ... ok

**C77** - six non-place commit targets each give one `E0900` with the place message; four places are clean (POL-14, AC 1)
Proof: run errors | test commit_target_must_be_a_place ... ok

**C78** - `(q) = 1;` and `(q) <- 1;` both give the place `E0900` (POL-14, AC 2)
Proof: run errors | test parenthesized_place_is_rejected_in_both_forms ... ok

**C79** - `signal` in `analog`, in a reaction and in a closure is `E0404` (POL-14, AC 3)
Proof: run errors | test signal_outside_a_module_body_is_e0404 ... ok

**C80** - a commit, a contribution and a reaction in a module body are each exactly one `E0404` with help (POL-14, AC 4)
Proof: run errors | test behavior_statements_in_a_module_body_are_e0404 ... ok

**C81** - the five forms in a `fn` body, closures included, are `E0121` and never `E0404` (POL-14, AC 5)
Proof: run errors | test function_body_reports_e0121_only ... ok

**C82** - `pub analog for M {}` is one `E0900` at `pub` and the block is kept (POL-14, AC 6)
Proof: run errors | test pub_on_a_behavior_block_is_e0900 ... ok

**C83** - an empty nature, a tuple signal type and a reference signal type are parse errors (POL-14, AC 7)
Proof: run errors | test forms_outside_the_grammar_are_parse_errors ... ok

**C84** - a misplaced `signal` declares nothing: its name is `E0161` and no module lists it (POL-14, AC 8)
Proof: run resolve | test misplaced_signal_declares_nothing ... ok

**C85** - the `E0910` row separates 256 nesting levels from 4096 chain links (POL-15, AC 1)
Proof: run spec | test e0910_counts_nesting_and_bounds_chains ... ok

**C86** - four flat chains of 4096 links parse clean and print, build and convert back on an 8 MiB stack (POL-15, AC 2)
Proof: run errors | test flat_chains_up_to_4096_links_are_not_nesting ... ok

**C87** - a chain of 4097 or 20000 links is one `E0910` naming the chain bound (POL-15, AC 3)
Proof: run errors | test flat_chain_past_4096_links_is_e0910 ... ok

**C88** - prefix, closure, `return` and generic nesting past 256 levels is still `E0910` (POL-15, AC 4)
Proof: run errors | test nesting_without_delimiters_is_also_bounded ... ok

**C89** - a comma in a module `where` is one diagnostic at the comma and the module is kept (POL-16, AC 1)
Proof: run errors | test bad_where_list_is_one_diagnostic ... ok

**C90** - two bad statements cost their own diagnostics, the valid `let` is kept, and `f` stays one function (POL-16, AC 2)
Proof: run errors | test statement_errors_recover_inside_the_block ... ok

**C91** - one bad port is one diagnostic and the module keeps its other port and its body (POL-16, AC 3)
Proof: run errors | test a_bad_port_keeps_the_module ... ok

**C92** - a module with a bad port is still found and its instances still resolve (POL-16, AC 4)
Proof: run resolve | test module_with_a_bad_port_still_resolves ... ok

**C93** - `use a::{self, K}` makes both `a::K` and `K` resolve (POL-17, AC 1)
Proof: run resolve | test use_self_imports_the_namespace ... ok

**C94** - a `self` leaf with an alias and a nested `self` leaf import their namespaces (POL-17, AC 2)
Proof: run resolve | test use_self_with_alias_and_nesting ... ok

**C95** - the text and span of a use leaf are its own segments, never the brace hull (POL-17, AC 3)
Proof: run resolve | test use_leaf_text_is_its_own_segments ... ok

**C96** - `A::WIDTH` and `B::decode` resolve to `Repr`'s built-in members through the bound (POL-17, AC 4)
Proof: run resolve | test repr_members_resolve_through_a_bound ... ok

**C97** - `A::NOPE` with `A: Repr` is one `E0161` (POL-17, AC 5)
Proof: run resolve | test unknown_repr_member_is_e0161 ... ok

**C98** - a member through a catalogue trait bound is `Deferred(BuiltinTraitMember)`, silently (POL-17, AC 6)
Proof: run resolve | test member_through_a_builtin_bound_is_deferred ... ok

**C99** - an unknown or repeated param name is `E0320`, not `E0161` (POL-18, AC 1)
Proof: run classify | test param_block_errors_are_e0320 ... ok

**C100** - a named argument that names no optional parameter, or a bound one, is `E0140` (POL-18, AC 2)
Proof: run classify | test named_argument_errors_are_e0140 ... ok

**C101** - an unknown or repeated bundle-literal field is `E0263` (POL-18, AC 3)
Proof: run classify | test bundle_literal_field_errors_are_e0263 ... ok

**C102** - an unknown field is one `E0161` in a struct literal and in a struct pattern alike (POL-18, AC 4)
Proof: run classify | test unknown_struct_field_is_reported_in_literals_and_patterns ... ok

**C103** - a signal, a result and a value input on an `extern` module are each one `E0370` (POL-18, AC 5)
Proof: run classify | test extern_module_shape_is_e0370 ... ok

**C104** - an `extern` function with a body and a bodiless plain function are each one `E0125` (POL-18, AC 6)
Proof: run classify | test function_body_rule_is_e0125 ... ok

**C105** - a repeated, mixed or baseless nature is one `E0201` and `Invalid` (POL-18, AC 7)
Proof: run classify | test nature_field_errors_are_e0201 ... ok

**C106** - `i(t)` on a nature without `i` is `Unresolved`, not a probe, with one `E0206` (POL-19, AC 1)
Proof: run classify | test access_of_another_nature_is_e0206 ... ok

**C107** - `arr.rise()` is `Unresolved` with `E0211` while `arr[0].rise()` resolves (POL-19, AC 2)
Proof: run classify | test carrier_method_on_an_array_is_e0211 ... ok

**C108** - a parent's definition and commit of `c.w` are writers of `C.w` (POL-19, AC 3)
Proof: run reflect | test parent_drive_is_a_writer_of_the_child_carrier ... ok

**C109** - every by-name lookup returns the project's item over the prelude's (POL-20, AC 1)
Proof: run reflect | test by_name_lookups_prefer_project_files ... ok

**C110** - the nine collections list project files only and `prelude_files()` gives the rest (POL-20, AC 2)
Proof: run reflect | test collections_list_project_files_only ... ok

**C111** - `enums()`, `traits()`, `consts()`, `impls()`, `struct_named` and `function` return the declared items (POL-20, AC 3)
Proof: run reflect | test design_has_a_collection_and_lookup_per_item_kind ... ok

**C112** - `children()` and `parent()` are inverse over every expression form (POL-20, AC 4)
Proof: run body | test expr_children_and_parent_are_inverse ... ok

**C113** - `exprs()` lists a body's expressions in pre-order and skips nested items (POL-20, AC 5)
Proof: run body | test decl_exprs_is_preorder_over_the_body ... ok

**C114** - a body is walked through `Block`, `Stmt::children()` and `Expr::stmt()` without raw ids (POL-20, AC 6)
Proof: run body | test block_and_stmt_handles_walk_a_body ... ok

**C115** - argument and field handles expose name, value and the declaration they bind (POL-20, AC 7)
Proof: run classify | test arg_and_field_handles_expose_name_value_target ... ok

**C116** - `Instance::builtin()` names `cross` and `ddt` and is none for a user module (POL-20, AC 8)
Proof: run classify | test builtin_instance_names_its_primitive ... ok

**C117** - structs, enums and traits list their `impl` blocks and find a method by name (POL-20, AC 9)
Proof: run reflect | test types_list_their_impls_and_methods ... ok

**C118** - `items()` of a generation `if` covers every arm (POL-20, AC 10)
Proof: run tree | test generation_if_items_cover_every_arm ... ok

**C119** - `from_json` reads back a valid design with an identical dump (POL-21, AC 1)
Proof: run reflect | test from_json_reads_back_a_valid_design ... ok

**C120** - `from_json` returns `Err` with the `check()` problems for a corrupted design, without panicking (POL-21, AC 2)
Proof: run reflect | test from_json_rejects_a_corrupted_design ... ok

**C121** - building 8000 signals costs under 8 times the build of 2000 (POL-21, AC 3)
Proof: run design | test member_tables_scale_subquadratically ... ok

**C122** - by-name lookups at 4000 entries cost under 4 times the lookups at 500 (POL-21, AC 4)
Proof: run design | test name_lookups_do_not_scan ... ok

**C123** - mapping.md maps all 65 constructs of B.1.2 to a POM type or method (POL-22, AC 1)
Proof: run docs | test every_b1_construct_is_mapped ... ok

**C124** - no page of docs/pom lists `(s)` as a place (POL-22, AC 2)
Proof: run docs | test place_has_no_parenthesized_form_in_the_docs ... ok

**C125** - resolution.md lists every code the POM reports (POL-22, AC 3)
Proof: run docs | test every_code_the_pom_reports_is_documented ... ok

**C126** - resolution.md names every built-in row (POL-22, AC 4)
Proof: run docs | test every_builtin_row_is_documented ... ok

**C127** - every reflection method, the new ones included, is documented under its type (POL-22, AC 5)
Proof: run docs | test every_reflection_method_is_documented ... ok

**C128** - every valid spec fence still parses clean after the spec edits (POL-23, AC 1)
Proof: run mslcorpus | test every_valid_spec_block_parses_clean ... ok

**C129** - every spec fence still round-trips through the POM (POL-23, AC 2)
Proof: run pomcorpus | test every_spec_block_round_trips_to_its_ast ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** B1 (Phase 1) Sonnet worker at 8700cc5f - returned (HEAD d6591429, T1–T6); B2 (Phase 2) Sonnet worker at 6778bd6a - returned (HEAD 47f5fc3e, T7–T10); B3 (Phase 3) Sonnet worker at 81417beb - returned (HEAD b45aa82a, T11–T16); B4 (Phase 4) Sonnet worker at 6222563b - returned (HEAD 246c2928, T17–T22); B5 (Phase 5) Sonnet worker at e8b660ba - returned (HEAD 7353d8a1, T23–T26); B6 (Phase 6) Sonnet worker at 3334a5b4 - returned (HEAD deed40d2, T27–T33, 319 tests); B7 (Phase 7) Sonnet worker at 649b963b - returned (HEAD 31af0cfe, T34–T39); B8 (Phases 8–9) Sonnet worker at 020f143e - returned (HEAD dd4de682, T40–T49, 355 tests)
**B1 notes:** B.3.6 assigns codes for removed Rust forms: E0903 type aliases/associated types/static, E0905 loop labels, E0900 the rest — T22 must match. §14 rule 1 reworded (core names). T3–T6 spec edits written before tests (no red run).
**B2 notes:** B.3.3 shorthand note is an unnumbered paragraph after item 7 (items 8–14 referenced by number).
**B3 notes:** lexer token is Token::RawIdent(String) (design said Ident{name,raw}); dangling doc comments dropped by a pre-parse pass with one E0900 per run (any position); statement span excludes its docs/attrs; printer writes `Int< -1 >` to avoid `<-` lexing.
**B4 notes:** Str tokens/literals are struct variants {text, raw_hashes} (mechanical test pattern edits, no expected value moved); integer suffixes now all uN/iN 1..=128; loop label + colon dropped after E0905; pattern params only for mut/ref/(/[/&/_ starts; Lit::suffix() for floats returns f32/f64 or SI letter (spec silent); str_value None on undefined escape.
**B5 notes:** src/stack.rs `deep()` (stacker) wraps printer/lowering/resolver/to_ast/walk_expr and a manual Clone for ast::Expr — reuse it for any new recursive walker; chain bound read as 4096 links (AC wording said 4097 terms); `<` after an identifier tentatively counts as nesting (pathological false E0910 possible with 256+ `a<b` in one arg list).
**B6 notes:** derive argument paths stay unresolved (AC 5 met as 'no E0161' only); `use a::{self}` naming the item it meets is not an E0162 collision; import collisions reported in imports.rs; E0162 span = whole path; Reflect::module None for signal outside a module body; new API: builtin_implicit/builtin_member, BuiltinKind::{Node,ImplicitParam,Member}, DeferReason::BuiltinTraitMember, Reference::Pattern, PatKind::Ident.res, Binding::is_binding.
**B7 notes:** ORCHESTRATOR CONFIRMED: three fixtures that connected a carrier port by name (invalid per §8.3.9) now name an added `tc: Option<f64>` value port instead (pom_classify, pom_reflect, reflection.md doctest). shape.rs is resolve stage 8 (Index stage 9). 'Optional' is syntactic (last segment Option). T39 root-not-writer clause implemented but not assertable (no Binding::writers, index crate-private). T35/T39 red confirmed by stash.
**B8 notes:** block handle exported as BlockHandle (SPEC_DEVIATION reflect/body.rs); Function::body/Reaction::body/param_bindings now return handles (call sites moved, meaning kept); code guard scrapes every "E####" literal in src/pom; App. G rows labelled D1..D17; serde_json now a normal dependency (Design::from_json); Decl::exprs semantics documented in reflection.md.
**Walked:** no screen in this feature; the stories' Independent Tests are the checks above

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4 ------→ T5 ------→ T6
Phase 2:  T7 ------→ T8 ------→ T9 ------→ T10
Phase 3:  T11 ------→ T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16
Phase 4:  T17 ------→ T18 ------→ T19 ------→ T20 ------→ T21 ------→ T22
Phase 5:  T23 ------→ T24 ------→ T25 ------→ T26
Phase 6:  T27 ------→ T28 ------→ T29 ------→ T30 ------→ T31 ------→ T32 ------→ T33
Phase 7:  T34 ------→ T35 ------→ T36 ------→ T37 ------→ T38 ------→ T39
Phase 8:  T40 ------→ T41 ------→ T42 ------→ T43 ------→ T44 ------→ T45 ------→ T46
Phase 9:  T47 ------→ T48 ------→ T49
```

Execution is strictly sequential. 49 tasks pack into 9 batches of whole phases (4 to 7 tasks each), one Sonnet worker per phase; the Verifier (Opus) runs once after T49, over `check_proofs.py msl-polish`.

| Batch | Phase | Tasks | Layer |
| ----- | ----- | ----- | ----- |
| 1 | Spec: the Rust boundary and names | T1 to T6 (6) | `docs/spec/` |
| 2 | Spec: statements, docs, checks, new surface | T7 to T10 (4) | `docs/spec/` |
| 3 | Parser: accepted forms | T11 to T16 (6) | `src/msl/` |
| 4 | Parser: literals and strictness | T17 to T22 (6) | `src/msl/` |
| 5 | Parser: limits and recovery | T23 to T26 (4) | `src/msl/` |
| 6 | POM: names | T27 to T33 (7) | `src/pom/` |
| 7 | POM: the spec's codes | T34 to T39 (6) | `src/pom/` |
| 8 | POM: reflection and robustness | T40 to T46 (7) | `src/pom/` |
| 9 | Reference docs and closure | T47 to T49 (3) | `docs/pom/`, `docs/spec/` |

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: `spec_text` target and the exhaustive B.3.6 | 1 concern, 5 criteria | ✅ Granular |
| T2: Appendix H: the inherited library | 1 concern, 4 criteria | ✅ Granular |
| T3: Core names in the text | 1 concern, 1 criterion | ✅ Granular |
| T4: `E0162`: duplicate names, colliding imports, ambiguous globs | 1 concern, 2 criteria | ✅ Granular |
| T5: Part I: borrows as arguments, identifier patterns | 1 concern, 2 criteria | ✅ Granular |
| T6: Behavior blocks: repeated generic names, namespace scope | 1 concern, 2 criteria | ✅ Granular |
| T7: Misplaced statements and places, in the text | 1 concern, 3 criteria | ✅ Granular |
| T8: Doc comments and attributes: where they go | 1 concern, 2 criteria | ✅ Granular |
| T9: Phases of the post-parse constraints; `E0125` and bodies | 1 concern, 2 criteria | ✅ Granular |
| T10: Raw identifiers and the field shorthand, in the text | 1 concern, 2 criteria | ✅ Granular |
| T11: A negative literal as a const generic argument | 1 concern, 2 criteria | ✅ Granular |
| T12: Function-pointer types | 1 concern, 2 criteria | ✅ Granular |
| T13: Raw identifiers | 1 concern, 4 criteria | ✅ Granular |
| T14: Field-init shorthand keeps its form | 1 concern, 3 criteria | ✅ Granular |
| T15: Docs and attributes before statements | 1 concern, 2 criteria | ✅ Granular |
| T16: Docs and attributes before hardware items | 1 concern, 3 criteria | ✅ Granular |
| T17: `E0909`: literals out of range | 1 concern, 4 criteria | ✅ Granular |
| T18: String escapes, raw strings, literal accessors | 1 concern, 3 criteria | ✅ Granular |
| T19: A commit target is a place; a place takes no parentheses | 1 concern, 2 criteria | ✅ Granular |
| T20: Misplaced statements: `E0404` everywhere but a `fn` | 1 concern, 3 criteria | ✅ Granular |
| T21: `pub` on a behavior block; forms outside the grammar | 1 concern, 2 criteria | ✅ Granular |
| T22: Removed Rust forms get targeted messages | 1 concern, 3 criteria | ✅ Granular |
| T23: `E0910`: nesting and flat chains counted apart | 1 concern, 4 criteria | ✅ Granular |
| T24: Recovery: a bad `where` list | 1 concern, 1 criterion | ✅ Granular |
| T25: Recovery: statements inside a block | 1 concern, 1 criterion | ✅ Granular |
| T26: Recovery: one port of a port list | 1 concern, 2 criteria | ✅ Granular |
| T27: Core names in the built-in table | 1 concern, 8 criteria | ✅ Granular |
| T28: Catalogue traits and types in the built-in table | 1 concern, 2 criteria | ✅ Granular |
| T29: `use a::{self}` and leaf spans | 1 concern, 3 criteria | ✅ Granular |
| T30: Members of `Repr` and of built-in bounds | 1 concern, 3 criteria | ✅ Granular |
| T31: `E0162` in the scopes stage | 1 concern, 6 criteria | ✅ Granular |
| T32: Identifier patterns that name a constant | 1 concern, 3 criteria | ✅ Granular |
| T33: Behavior names by name; a misplaced `signal` declares nothing | 1 concern, 3 criteria | ✅ Granular |
| T34: `E0320` and `E0140` for param blocks and named arguments | 1 concern, 2 criteria | ✅ Granular |
| T35: `E0263` for bundle literals; struct fields in literals and patterns | 1 concern, 2 criteria | ✅ Granular |
| T36: `E0370`, `E0125` and `E0201` | 1 concern, 3 criteria | ✅ Granular |
| T37: An access call resolves by the nature of its first argument | 1 concern, 1 criterion | ✅ Granular |
| T38: A carrier method on an array of carriers | 1 concern, 1 criterion | ✅ Granular |
| T39: Writers through a resolved member | 1 concern, 1 criterion | ✅ Granular |
| T40: `Design` collections: one precedence, one inclusion rule | 1 concern, 3 criteria | ✅ Granular |
| T41: `Expr::children`, `Expr::parent`, `Decl::exprs` | 1 concern, 2 criteria | ✅ Granular |
| T42: `Block` and statement handles | 1 concern, 1 criterion | ✅ Granular |
| T43: Argument and field handles; `Instance::builtin`; impls of a type | 1 concern, 3 criteria | ✅ Granular |
| T44: `Generation::items` over every arm; `docs()` on the new positions | 1 concern, 2 criteria | ✅ Granular |
| T45: Float literals from their text; `Design::from_json` | 1 concern, 4 criteria | ✅ Granular |
| T46: Sorted name tables | 1 concern, 2 criteria | ✅ Granular |
| T47: `docs/pom/mapping.md`: the 65 constructs of B.1 | 1 concern, 1 criterion | ✅ Granular |
| T48: `docs/pom`: places, diagnostics, built-ins | 1 concern, 4 criteria | ✅ Granular |
| T49: Appendix G records the decisions | 1 concern, 3 criteria | ✅ Granular |

A task's `Where` names one primary file and, in parentheses, the files the same change must touch in the same commit (its test target, the `docs/` page a guard ties to it, the lowering of a new AST field). `validate_tasks.py` warns on these; they are not separable: a new AST field without its lowering does not compile, and a spec row without its fixture turns a guard red.

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
| T26 | T25 | T25 | ✅ Match |
| T27 | T26 | T26 | ✅ Match |
| T28 | T27 | T27 | ✅ Match |
| T29 | T28 | T28 | ✅ Match |
| T30 | T29 | T29 | ✅ Match |
| T31 | T30 | T30 | ✅ Match |
| T32 | T31 | T31 | ✅ Match |
| T33 | T32 | T32 | ✅ Match |
| T34 | T33 | T33 | ✅ Match |
| T35 | T34 | T34 | ✅ Match |
| T36 | T35 | T35 | ✅ Match |
| T37 | T36 | T36 | ✅ Match |
| T38 | T37 | T37 | ✅ Match |
| T39 | T38 | T38 | ✅ Match |
| T40 | T39 | T39 | ✅ Match |
| T41 | T40 | T40 | ✅ Match |
| T42 | T41 | T41 | ✅ Match |
| T43 | T42 | T42 | ✅ Match |
| T44 | T43 | T43 | ✅ Match |
| T45 | T44 | T44 | ✅ Match |
| T46 | T45 | T45 | ✅ Match |
| T47 | T46 | T46 | ✅ Match |
| T48 | T47 | T47 | ✅ Match |
| T49 | T48 | T48 | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | specification text | integration | integration | ✅ OK |
| T2 | specification text | integration | integration | ✅ OK |
| T3 | specification text | integration | integration | ✅ OK |
| T4 | specification text | integration | integration | ✅ OK |
| T5 | specification text | integration | integration | ✅ OK |
| T6 | specification text | integration | integration | ✅ OK |
| T7 | specification text | integration | integration | ✅ OK |
| T8 | specification text | integration | integration | ✅ OK |
| T9 | specification text | integration | integration | ✅ OK |
| T10 | specification text | integration | integration | ✅ OK |
| T11 | lexer / parser / printer | integration | integration | ✅ OK |
| T12 | lexer / parser / printer | integration | integration | ✅ OK |
| T13 | lexer / parser / printer | integration | integration | ✅ OK |
| T14 | lexer / parser / printer | integration | integration | ✅ OK |
| T15 | lexer / parser / printer | integration | integration | ✅ OK |
| T16 | lexer / parser / printer | integration | integration | ✅ OK |
| T17 | lexer / parser / printer | integration | integration | ✅ OK |
| T18 | lexer / parser / printer | integration | integration | ✅ OK |
| T19 | lexer / parser / printer | integration | integration | ✅ OK |
| T20 | lexer / parser / printer | integration | integration | ✅ OK |
| T21 | lexer / parser / printer | integration | integration | ✅ OK |
| T22 | lexer / parser / printer | integration | integration | ✅ OK |
| T23 | lexer / parser / printer | integration | integration | ✅ OK |
| T24 | lexer / parser / printer | integration | integration | ✅ OK |
| T25 | lexer / parser / printer | integration | integration | ✅ OK |
| T26 | lexer / parser / printer | integration | integration | ✅ OK |
| T27 | POM | integration | integration | ✅ OK |
| T28 | POM | integration | integration | ✅ OK |
| T29 | POM | integration | integration | ✅ OK |
| T30 | POM | integration | integration | ✅ OK |
| T31 | POM | integration | integration | ✅ OK |
| T32 | POM | integration | integration | ✅ OK |
| T33 | POM | integration | integration | ✅ OK |
| T34 | POM | integration | integration | ✅ OK |
| T35 | POM | integration | integration | ✅ OK |
| T36 | POM | integration | integration | ✅ OK |
| T37 | POM | integration | integration | ✅ OK |
| T38 | POM | integration | integration | ✅ OK |
| T39 | POM | integration | integration | ✅ OK |
| T40 | POM | integration | integration | ✅ OK |
| T41 | POM | integration | integration | ✅ OK |
| T42 | POM | integration | integration | ✅ OK |
| T43 | POM | integration | integration | ✅ OK |
| T44 | POM | integration | integration | ✅ OK |
| T45 | POM | integration | integration | ✅ OK |
| T46 | POM | integration | integration | ✅ OK |
| T47 | reference docs | integration | integration | ✅ OK |
| T48 | reference docs | integration | integration | ✅ OK |
| T49 | specification text | integration | integration | ✅ OK |

---

## Requirement coverage

| Requirement | Criteria | Checks | Tasks |
| ----------- | -------- | ------ | ----- |
| POL-01 | 3 | C1 to C3 | T1 |
| POL-02 | 6 | C4 to C9 | T2, T28 |
| POL-03 | 9 | C10 to C18 | T3, T27 |
| POL-04 | 8 | C19 to C26 | T4, T31 |
| POL-05 | 4 | C27 to C30 | T6, T33 |
| POL-06 | 5 | C31 to C35 | T5, T32 |
| POL-07 | 3 | C36 to C38 | T7 |
| POL-08 | 9 | C39 to C47 | T1, T8, T9, T10, T49 |
| POL-09 | 7 | C48 to C54 | T11, T12, T22 |
| POL-10 | 4 | C55 to C58 | T13 |
| POL-11 | 3 | C59 to C61 | T14 |
| POL-12 | 6 | C62 to C67 | T15, T16, T44 |
| POL-13 | 9 | C68 to C76 | T17, T18, T45 |
| POL-14 | 8 | C77 to C84 | T19, T20, T21, T33 |
| POL-15 | 4 | C85 to C88 | T23 |
| POL-16 | 4 | C89 to C92 | T24, T25, T26 |
| POL-17 | 6 | C93 to C98 | T29, T30 |
| POL-18 | 7 | C99 to C105 | T34, T35, T36 |
| POL-19 | 3 | C106 to C108 | T37, T38, T39 |
| POL-20 | 10 | C109 to C118 | T40, T41, T42, T43, T44 |
| POL-21 | 4 | C119 to C122 | T45, T46 |
| POL-22 | 5 | C123 to C127 | T47, T48 |
| POL-23 | 2 | C128 to C129 | T49 |

129 criteria, 129 checks, 49 tasks, 9 phases. The finding-to-task table is in `spec.md` (Requirement Traceability).

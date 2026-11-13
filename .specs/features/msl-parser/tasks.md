# MSL Parser Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-parser/design.md`
**Status**: Approved
**Approved**: 2026-11-13 - user: "Faz uma limpeza antes de iniciar para limpar lixinhos. Aprovo e pode iniciar"
**Build**: `export CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target; cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^warning'); cargo test -p piperine --test suite_hygiene`
**Run lexer**: `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target cargo test -p piperine-lang --test msl_lexer`
**Run parser**: `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target cargo test -p piperine-lang --test msl_parser`
**Run errors**: `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target cargo test -p piperine-lang --test msl_errors`
**Run corpus**: `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target cargo test -p piperine-lang --test msl_corpus`
**Run print**: `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target cargo test -p piperine-lang --test msl_print`

Executors: Sonnet sub-agents. Verifier: Opus. Every cargo command uses `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target` (disk at 92%). Work in `/home/keslarek/Git/piperine-msl-spec`, branch `feature/msl-parser`.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Where the tests are", MD-28 placement), `tests/suite_hygiene.rs` (`//!` header, no `#[ignore]`, no file-scope `allow`), `.github/workflows/ci.yml` (`RUSTFLAGS=-D warnings`, `cargo test --workspace`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Lexer (`msl/lexer.rs`) | integration (crate `tests/`) | 1:1 to spec MSLP-01 ACs; every Appendix A token class and every lexical error code | `crates/piperine-lang/tests/msl_lexer.rs` | run lexer |
| Parser (`msl/parser/*`) | integration | every B.3 production and every inherited form class has a fixture; MSLP-02 ACs 1:1 | `crates/piperine-lang/tests/msl_parser.rs` | run parser |
| Diagnostics / recovery | integration | every parse-phase error row (17) has a fixture asserting code + span; MSLP-04 ACs 1:1 | `crates/piperine-lang/tests/msl_errors.rs` | run errors |
| Spec corpus | integration | every ` ```rust* ` fence in `docs/spec/*.md` | `crates/piperine-lang/tests/msl_corpus.rs` | run corpus |
| Printer (`msl/print.rs`) | integration | round-trip + idempotence over the whole corpus; docs/attrs fixture | `crates/piperine-lang/tests/msl_print.rs` | run print |
| AST types, span, module wiring | none | build gate only | - | build |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | after a task that adds/changes tests in one target | the task's own `Run <target>` line |
| Full | after a parser task (parser + errors + corpus may move together) | `CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target cargo test -p piperine-lang --test msl_lexer --test msl_parser --test msl_errors` (+ `--test msl_corpus`/`--test msl_print` once they exist) |
| Build | every task before commit | `**Build**` line above |

---

## Execution Plan

### Phase 1: Foundation (lexing)

```
T1 → T2 → T3
```

### Phase 2: Rust value grammar

```
T3 → T4 → T5 → T6 → T7
```

### Phase 3: MSL additions and diagnostics

```
T7 → T16 → T8 → T9 → T10 → T11
```

### Phase 4: Corpus, error fixtures, printer

```
T11 → T12 → T13 → T14 → T15
```

---

## Task Breakdown

### T1: Module skeleton, dependencies, spans and diagnostics

**What**: Add `logos` and `chumsky` (latest stable 0.x, exact version pinned and recorded in the commit body) to `piperine-lang`; create `src/msl/mod.rs` with `span.rs` + `diag.rs` (`Span`, `Diagnostic` implementing `miette::Diagnostic`, `render()`), export `pub mod msl` from `lib.rs`.
**Where**: `crates/piperine-lang/src/msl/diag.rs` (+ `mod.rs`, `span.rs`, `Cargo.toml`, one line in `lib.rs`)
**Depends on**: None
**Reuses**: `miette` already in `Cargo.toml`
**Requirement**: MSLP-04

**Done when**:
- [x] `cargo build -p piperine-lang --all-targets` has zero warnings
- [x] `Diagnostic` carries code, message, primary label span, optional help
- [x] Build gate passes

**Tests**: none
**Gate**: build
**Commit**: `feat(lang/msl): module skeleton, spans and diagnostics`

---

### T2: Token set and keyword classification

**What**: `Token` enum via logos: identifiers, reserved keywords (Rust 2024 strict + MSL `module nature signal on out flip after`), `Unknown`, punctuation incl. `+>` and `<-`, line/block comments skipped, `///`/`//!` doc-comment tokens; contextual keywords stay `Ident`. `lex()` returns tokens with spans.
**Where**: `crates/piperine-lang/src/msl/lexer.rs`
**Depends on**: T1
**Reuses**: Appendix A token tables
**Requirement**: MSLP-01

**Done when**:
- [x] Tests `contribution_and_commit_operators_are_single_tokens`, `keywords_reserved_vs_contextual`, `doc_comments_are_tokens` pass
- [x] Run lexer passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): lexer token set and keywords`

---

### T3: Literals and lexical errors

**What**: integer/float/string literals; SI-suffixed literals (value scaled, raw text kept); `E0902` malformed SI, `E0905` dropped token classes, unterminated block comment/string (one diagnostic, continue), non-UTF-8 entry `lex_bytes`.
**Where**: `crates/piperine-lang/src/msl/lexer.rs` (modify)
**Depends on**: T2
**Reuses**: Appendix A §A.2 SI table
**Requirement**: MSLP-01, MSLP-06

**Done when**:
- [x] Tests `si_literal_scaled_value_and_raw_text`, `malformed_si_literal_reports_e0902`, `dropped_token_class_reports_e0905`, `unterminated_comment_and_string_one_diagnostic`, `non_utf8_one_diagnostic` pass
- [x] Run lexer passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): literals, SI suffixes and lexical errors`

---

### T4: AST node types

**What**: all AST types (items, Rust value forms, MSL additions per B.3) with `span` on every node, and a `strip_spans()` (or span-insensitive equality) used by round-trip tests.
**Where**: `crates/piperine-lang/src/msl/ast.rs`
**Depends on**: T3
**Reuses**: B.3 production list; design §Components
**Requirement**: MSLP-02

**Done when**:
- [x] One AST type or variant per B.3 production (listed in a doc comment table mapping production → type)
- [x] Build gate passes

**Tests**: none
**Gate**: build
**Commit**: `feat(lang/msl): AST`

---

### T5: Types, paths, generics, patterns

**What**: chumsky parsers for types (incl. `UInt<{N+1}>`, arrays, tuples, `&'static str`, `flip T`, `Option<sigtype>`), paths with generic args/turbofish, generics + `where`, patterns.
**Where**: `crates/piperine-lang/src/msl/parser/ty.rs` (+ `pat.rs`, `mod.rs` entry `parse_type`/`parse_pat`)
**Depends on**: T4
**Reuses**: lexer tokens
**Requirement**: MSLP-02

**Done when**:
- [x] Test `inherited_rust_forms_parse` covers the type/pattern classes (extended in T6/T7)
- [x] Run parser passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): types, paths, generics, patterns`

---

### T6: Expressions and statements

**What**: Pratt expressions (Rust precedence), literals incl. `Unknown`, calls with named args, method calls, closures, blocks, `if`/`match`/`for`/`while`/`loop`, `let`; statements `contribution` (`expr +> expr;`) and `commit` (`place <- expr [after expr];`) at any depth; `E0907` for `+>`/`<-` inside an expression; nesting depth limit (diagnostic, no overflow).
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs`
**Depends on**: T5
**Reuses**: T5 type/pattern parsers
**Requirement**: MSLP-02, MSLP-04

**Done when**:
- [x] Tests `contribution_commit_at_any_depth`, `depth_over_256_is_diagnostic_not_overflow`, `unknown_as_value_ok_as_name_e0908` (value half) pass
- [x] Run parser + run errors pass; build gate passes

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): expressions and statements`

---

### T7: Rust items, attributes, doc comments

**What**: `fn` (incl. `extern "C" fn`, `where`, optional trailing params), `struct` (incl. carrier fields with `in`/`out`, making bundles), `enum`, `impl` (fns + associated consts), `const`, `use`, `mod`, attributes, doc comments attached to the next item/field/port; `E0121` (forbidden forms in fn body), `E0901` (macro other than `emit!`/`include_str!`), `E0903` (keyword as identifier), `E0908` (`Unknown` as name).
**Where**: `crates/piperine-lang/src/msl/parser/item.rs`
**Depends on**: T6
**Reuses**: T5/T6 parsers
**Requirement**: MSLP-02, MSLP-04

**Done when**:
- [x] Tests `inherited_rust_forms_parse` (complete), `doc_comments_and_attrs_attach`, `unknown_as_value_ok_as_name_e0908` pass
- [x] Run parser + run errors pass; build gate passes

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): Rust items, attributes, doc comments`

---

### T16: Bounded stack usage

**What**: reduce per-nesting-level stack use so 256 levels parse in a debug build on an 8 MiB caller stack; remove the 512 MiB parser thread (parse runs on the caller's thread). Typical levers: box/erase recursive chumsky parsers (`.boxed()`, `Recursive` declared once per grammar area), fewer combinator layers per precedence level, `#[inline(never)]` on hot recursion points; measure before/after.
**Where**: `crates/piperine-lang/src/msl/parser/expr.rs` (+ the thread spawn site in `parser/mod.rs`)
**Depends on**: T7
**Reuses**: existing parser
**Requirement**: MSLP-04

**Done when**:
- [x] Test `depth_limit_fits_8mib_stack_in_debug` passes (spawns an 8 MiB thread, parses 256 nested parens/blocks clean and 300 with E0910, no overflow)
- [x] no thread with a stack > 8 MiB is created by `msl::parse`
- [x] Run errors + run parser pass; build gate passes

**Tests**: integration
**Gate**: full
**Commit**: `perf(lang/msl): bound parser stack depth`

---

### T8: Natures and signals

**What**: `nature_decl` (discrete `value`/`resolve`, continuous `potential`/`flow` with attribute blocks, generics), `signal_decl`, carrier-typed struct fields (bundles), `flip` sigtypes.
**Where**: `crates/piperine-lang/src/msl/parser/msl.rs`
**Depends on**: T16
**Reuses**: B.3 `nature_decl`, `signal_decl`, `sigtype`
**Requirement**: MSLP-02

**Done when**:
- [x] `every_b3_production_parses_with_span` covers nature/signal/sigtype productions
- [x] Run parser passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): natures, signals, bundles`

---

### T9: Modules, ports, params, instances, templates, generation

**What**: `module_decl` (incl. `extern "abi"`, `-> T` result, `where`), ports with directions/defaults, params, `let`, definitions `place = expr [after expr];`, `gen<X>` (`for`/`if` over params), `instance` (positional + named args + param block), `template`.
**Where**: `crates/piperine-lang/src/msl/parser/msl.rs` (modify)
**Depends on**: T8
**Reuses**: B.3 `module_decl`, `port`, `module_item`, `instance`, `template`, `gen<X>`
**Requirement**: MSLP-02

**Done when**:
- [x] `every_b3_production_parses_with_span` covers module productions
- [x] Run parser passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): modules, ports, params, instances`

---

### T10: Behavior blocks

**What**: `analog [<names>] for Path { … }`, `digital [<names>] for Path { … }` with `reaction` (`on expr { … }`) and `gen<digital_item>`; `emit!`/`include_str!` invocation forms; contextual `analog`/`digital` only before `for` or `<`.
**Where**: `crates/piperine-lang/src/msl/parser/msl.rs` (modify)
**Depends on**: T9
**Reuses**: B.3 `behavior_block`, `reaction`, `digital_item`
**Requirement**: MSLP-02, MSLP-06

**Done when**:
- [x] `every_b3_production_parses_with_span` complete (all B.3 productions); `contextual_keywords_are_identifiers_outside_nature` passes
- [x] Run parser passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): analog and digital behavior blocks`

---

### T11: Recovery and professional messages

**What**: recovery at `;`/`}`/item starts so every error in a file is reported in source order with a partial AST; generic syntax errors as `E0900` naming found vs expected; depth limit code `E0910`; `parse()`/`parse_bytes()` public entry; empty source → empty file; add `E0900`/`E0910` rows to `docs/spec/appendix_a_lexical.md`'s error table.
**Where**: `crates/piperine-lang/src/msl/parser/mod.rs` (+ the Appendix A table row)
**Depends on**: T10
**Reuses**: chumsky `Rich` errors, `msl::diag`
**Requirement**: MSLP-04, MSLP-06

**Done when**:
- [x] Tests `multiple_errors_in_source_order_with_partial_ast`, `unexpected_token_names_found_and_expected`, `diagnostic_has_code_message_label_help`, `empty_source_empty_file` pass
- [x] Run errors passes; build gate passes

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/msl): error recovery and diagnostics`

---

### T12: Spec corpus test

**What**: test that extracts every ` ```rust* ` fence from `docs/spec/*.md`; ` ```rust ` must parse with zero diagnostics, ` ```rust,compile_fail ` must report ≥1, ` ```rust,fragment ` is parsed with the item/statement entry. Re-fence the spec's error-illustrating and fragment blocks (fence info string only, no wording change). Fix parser gaps the corpus exposes (each fix stays in this task's commit; if a gap needs > ~150 lines, stop and report).
**Where**: `crates/piperine-lang/tests/msl_corpus.rs` (+ fence-only edits in `docs/spec/*.md`, parser fixes)
**Depends on**: T11
**Reuses**: T11 `parse()`
**Requirement**: MSLP-03

**Done when**:
- [x] Tests `every_valid_spec_block_parses_clean`, `every_compile_fail_block_diagnoses` pass; the test prints the count of blocks per class
- [x] Run corpus passes; build gate passes

**Tests**: integration
**Gate**: full
**Commit**: `test(lang/msl): spec corpus parses`

---

### T13: One fixture per parse-phase error row

**What**: a table-driven test: for each `docs/spec` error row of phase `parse` (17 + `E0900`/`E0910`), a minimal source asserting that code at the expected span; a guard that fails when the spec gains a parse-phase row with no fixture.
**Where**: `crates/piperine-lang/tests/msl_errors.rs` (modify)
**Depends on**: T12
**Reuses**: spec error tables (read at test time)
**Requirement**: MSLP-04

**Done when**:
- [x] Tests `parse_phase_rows_report_their_codes`, `every_parse_phase_row_has_a_fixture` pass
- [x] Run errors passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `test(lang/msl): every parse-phase error code has a fixture`

---

### T14: Canonical printer

**What**: `print(&File) -> String`, canonical layout (design §print), doc comments and attributes preserved; round-trip and idempotence tests over the corpus.
**Where**: `crates/piperine-lang/src/msl/print.rs` (+ `tests/msl_print.rs`)
**Depends on**: T13
**Reuses**: T12 corpus extraction (shared helper in `tests/support/` if needed)
**Requirement**: MSLP-05

**Done when**:
- [x] Tests `roundtrip_ast_equal_over_corpus`, `print_is_idempotent_over_corpus`, `printer_preserves_docs_and_attrs` pass
- [x] Run print passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/msl): canonical printer`

---

### T15: Rendered diagnostics

**What**: test that renders 3 representative diagnostics (`E0907`, `E0900`, `E0902`) with `msl::render` (miette graphical, no color) and asserts the output contains the code, the source line, the underline at the span and the help line.
**Where**: `crates/piperine-lang/tests/msl_errors.rs` (modify)
**Depends on**: T14
**Reuses**: `msl::diag::render`
**Requirement**: MSLP-04

**Done when**:
- [x] Test `rendered_output_is_miette_graphical` passes
- [x] Run errors passes; build gate passes

**Tests**: integration
**Gate**: quick
**Commit**: `test(lang/msl): rendered diagnostics`

---

## Checks

**C1** - `50m` lexes to one float token with value 0.05 and raw text `50m` (MSLP-01, AC 1)
Proof: run lexer | test si_literal_scaled_value_and_raw_text ... ok

**C2** - `1e3m`, `5mf64`, `0x10m` each report `E0902` at the literal (MSLP-01, AC 2)
Proof: run lexer | test malformed_si_literal_reports_e0902 ... ok

**C3** - `'a'`, `b'a'`, `b"x"`, `'a` each report `E0905` at the token (MSLP-01, AC 3)
Proof: run lexer | test dropped_token_class_reports_e0905 ... ok

**C4** - `+>` and `<-` each lex as one token (MSLP-01, AC 4)
Proof: run lexer | test contribution_and_commit_operators_are_single_tokens ... ok

**C5** - every reserved keyword lexes as a keyword and every contextual keyword as an identifier (MSLP-01, AC 5)
Proof: run lexer | test keywords_reserved_vs_contextual ... ok

**C6** - `///` and `//!` produce doc-comment tokens with their text (MSLP-01, AC 6)
Proof: run lexer | test doc_comments_are_tokens ... ok

**C7** - an unterminated `/*` or `"` yields exactly one diagnostic at its opening span and lexing reaches EOF (MSLP-01, AC 7)
Proof: run lexer | test unterminated_comment_and_string_one_diagnostic ... ok

**C8** - each B.3 production has a fixture that parses to its AST node with a span equal to the fixture's text range (MSLP-02, AC 1)
Proof: run parser | test every_b3_production_parses_with_span ... ok

**C9** - inherited Rust forms (fn/struct/enum/impl/const/use/mod, closures, match, generics, where, attributes) parse to their nodes (MSLP-02, AC 2)
Proof: run parser | test inherited_rust_forms_parse ... ok

**C10** - every ` ```rust ` fence in `docs/spec/*.md` parses with zero diagnostics (MSLP-03, AC 3)
Proof: run corpus | test every_valid_spec_block_parses_clean ... ok

**C11** - every ` ```rust,compile_fail ` fence yields at least one diagnostic (MSLP-03, AC 4)
Proof: run corpus | test every_compile_fail_block_diagnoses ... ok

**C12** - `+>` and `<-` statements nested in `if`/`match`/`for` parse as statements (MSLP-02, AC 5)
Proof: run parser | test contribution_commit_at_any_depth ... ok

**C13** - doc comments and attributes attach to the following item, field and port (MSLP-02, AC 6)
Proof: run parser | test doc_comments_and_attrs_attach ... ok

**C14** - each parse-phase error row's fixture reports that row's code at the expected span (MSLP-04, AC 1)
Proof: run errors | test parse_phase_rows_report_their_codes ... ok

**C15** - a parse-phase row without a fixture fails the suite (MSLP-04, AC 1)
Proof: run errors | test every_parse_phase_row_has_a_fixture ... ok

**C16** - every diagnostic has a code, a message, a primary label at a span, and help where the spec names a correct form (MSLP-04, AC 2)
Proof: run errors | test diagnostic_has_code_message_label_help ... ok

**C17** - a source with 3 independent errors yields 3 diagnostics in source order and a partial AST holding the valid items (MSLP-04, AC 3)
Proof: run errors | test multiple_errors_in_source_order_with_partial_ast ... ok

**C18** - an unexpected token diagnostic names the found token and the expected set (MSLP-04, AC 4)
Proof: run errors | test unexpected_token_names_found_and_expected ... ok

**C19** - rendered output shows code, source line, underline and help (MSLP-04, AC 5)
Proof: run errors | test rendered_output_is_miette_graphical ... ok

**C20** - 300 nested parentheses yield a depth diagnostic and no stack overflow (MSLP-04, AC 6)
Proof: run errors | test depth_over_256_is_diagnostic_not_overflow ... ok

**C28** - 256 nesting levels parse in debug on an 8 MiB caller-thread stack; no larger-stack thread is spawned (MSLP-04, AC 7)
Proof: run errors | test depth_limit_fits_8mib_stack_in_debug ... ok

**C21** - for every corpus file, strip_spans(parse(print(ast))) == strip_spans(ast) (MSLP-05, AC 1)
Proof: run print | test roundtrip_ast_equal_over_corpus ... ok

**C22** - print(parse(print(ast))) is byte-identical to print(ast) for every corpus file (MSLP-05, AC 2)
Proof: run print | test print_is_idempotent_over_corpus ... ok

**C23** - printed output keeps every doc comment and attribute of the input (MSLP-05, AC 3)
Proof: run print | test printer_preserves_docs_and_attrs ... ok

**C24** - an empty source yields an empty file and no diagnostics (MSLP-06)
Proof: run errors | test empty_source_empty_file ... ok

**C25** - non-UTF-8 bytes yield exactly one diagnostic and no AST (MSLP-06)
Proof: run lexer | test non_utf8_one_diagnostic ... ok

**C26** - `Unknown` parses as a value; as a binding, item or field name it reports `E0908` (MSLP-06)
Proof: run parser | test unknown_as_value_ok_as_name_e0908 ... ok

**C27** - `potential`, `flow`, `value`, `resolve`, `analog`, `digital` parse as identifiers outside their positions (MSLP-06)
Proof: run parser | test contextual_keywords_are_identifiers_outside_nature ... ok

---

## Handoff

**Settled mid-build:** F7 - user: "Declarar stacker e aceitar" (heap stack growth allowed, feature declared); F8 - user: "Corrigir os exemplos na spec" (split/complete the 4 elided spec examples, re-fence as rust); stack fix as new T16 - user: "2" (option: fix in B2, target 8 MiB debug); sub-agents accepted - user: "Lança sempre Sonnet para implementação, Opus para verifier"
**Dispatched:** B1 (T1–T7) Sonnet worker at 3f0d8b50 - returned (HEAD 1642e69c, 19 tests green); B2 (T16, T8–T15) Sonnet worker at b489121c - returned (HEAD 6eda5251, 39 tests green); Verifier (Opus) at 6eda5251 + proofs.md commit
**B1 gaps carried to B2:** parser runs on a 512 MiB-stack thread (debug build overflowed 2 MB at ~50 nested parens); tuple index `x.0.1` not parsed (lexer reads `0.1` as float); raw strings / `r#ident` not lexed; `check.rs` must extend placement checks (E0404) to analog/reaction/closure contexts in T10/T11; E0900 rows for unterminated/non-UTF-8 to add in T11; E0900 expected-token lists need summarizing (T11); E0121 vs E0404 and E0101 vs E0903 overlaps in the spec.
**Fix round:** Sonnet fix worker closed F1-F13 in 6 commits (see verification.md `Closed`); spec has no `compile_fail` fence left after F8, so C11 checks the mechanism with a control fence.
**Walked:** n/a - no screen; Independent Tests are the integration targets above

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1:  T1 → T2 → T3
Phase 2:  T4 → T5 → T6 → T7
Phase 3:  T16 → T8 → T9 → T10 → T11
Phase 4:  T12 → T13 → T14 → T15
```

Batches (~7 tasks, whole phases): **B1** = Phase 1 + 2 (T1–T7), **B2** = Phase 3 + 4 (T16, T8–T15). Each batch is one Sonnet worker; Opus Verifier after T15.

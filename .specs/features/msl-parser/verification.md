# MSL Parser verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 6eda5251
**Verifier**: independent sub-agent (author != verifier)

Probed at HEAD f8dc49da (code 6eda5251) with a throwaway binary crate linked to
`piperine-lang` by path (not committed), on the main thread's default 8 MiB stack, debug
build. Inputs: every `rust` block of `docs/spec` under ~1450 random mutations (deleted `;`/`}`,
duplicated lines, inserted/deleted characters, swapped tokens; seeds 1-3), with no panic,
crash or out-of-order/out-of-range span. Also tried: deep nesting of every kind, round-trips of
about 40 tricky forms, every parse-phase row against inputs the fixtures do not name, the Rust 2024
keyword list in binding/item positions, SI literal edge cases, and recovery orders.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `msl_lexer.rs` `si_literal_scaled_value_and_raw_text` | yes - asserts `Float { 0.05, "50m" }` exactly, plus the full suffix table. Its `2.2u` "rule 6" case does not discriminate (`2.2*1e-6 == 2.2e-6` in f64; `1.1p` would). That is not this criterion, and the lexer gets `1.1p` right |
| C2 | `msl_lexer.rs` `malformed_si_literal_reports_e0902` | yes - one diagnostic, code and exact span for `1e3m`, `5mf64`, `0x10m` and more |
| C3 | `msl_lexer.rs` `dropped_token_class_reports_e0905` | yes - all four classes, exact spans |
| C4 | `msl_lexer.rs` `contribution_and_commit_operators_are_single_tokens` | yes |
| C5 | `msl_lexer.rs` `keywords_reserved_vs_contextual` | yes - every Rust 2024 strict and reserved word (`gen` included), the added words, the contextual words |
| C6 | `msl_lexer.rs` `doc_comments_are_tokens` | yes for `///` and `//!` (the AC names only those). Block doc comments are F11 |
| C7 | `msl_lexer.rs` `unterminated_comment_and_string_one_diagnostic` | yes at the lexer layer the AC names. Through `parse()` a second, cascading diagnostic follows (F10) |
| C8 | `msl_parser.rs` `every_b3_production_parses_with_span` | no - the span is asserted equal to the text for about two thirds of the B.3 productions. `instance`, `callee`, `call`, `method_call`, `sigtype`, `names`, `place` and `attr_*` are only kind-checked (F13) |
| C9 | `msl_parser.rs` `inherited_rust_forms_parse` | no - green while inherited Rust forms are rejected: raw strings, `1.`, non-ASCII identifiers, a BOM, `let … else` (F6) |
| C10 | `msl_corpus.rs` `every_valid_spec_block_parses_clean` | yes - each `rust` block is parsed as a file with zero diagnostics. The 12 `rust,fragment` re-fences are genuine pieces (module-body items, statements, a reaction, expression lists) and each parses in a context. 4 blocks were moved out by `compile_fail` (F8) |
| C11 | `msl_corpus.rs` `every_compile_fail_block_diagnoses` | no - vacuous. The only 4 `compile_fail` blocks are the spec-bug examples. Each one diagnoses on `…` or on a context mix, not on an error it illustrates, and every part of each parses cleanly when split (verified). The filter also passes a block when either entry diagnoses (F8) |
| C12 | `msl_parser.rs` `contribution_commit_at_any_depth` | yes - `if`/`else`/`match`/`for`/`while`/`loop`/nested blocks, with counts per kind |
| C13 | `msl_parser.rs` `doc_comments_and_attrs_attach` | yes - item, variant, field and impl item here. Ports are asserted in `every_b3_production_parses_with_span` in the same `run parser` group |
| C14 | `msl_errors.rs` `parse_phase_rows_report_their_codes` | no - E0903's fixture covers only "keyword used as an identifier". The row's other half, "a Rust keyword where no production accepts it", reports E0900, and `let self = 1;` reports nothing (F4). E0602 and E0908 have untested shapes that report E0900 (F9) |
| C15 | `msl_errors.rs` `every_parse_phase_row_has_a_fixture` | yes - scans every spec table for `parse` rows, both directions (missing and stale) |
| C16 | `msl_errors.rs` `diagnostic_has_code_message_label_help` | yes |
| C17 | `msl_errors.rs` `multiple_errors_in_source_order_with_partial_ast` | yes for the cases it names. An unclosed opening delimiter is reported at end of input, after later items' errors (F5) |
| C18 | `msl_errors.rs` `unexpected_token_names_found_and_expected` | yes |
| C19 | `msl_errors.rs` `rendered_output_is_miette_graphical` | yes - code line, message, `[file:line:col]`, source line, an underline exactly the span's length at the span's column, and the help line |
| C20 | `msl_errors.rs` `depth_over_256_is_diagnostic_not_overflow` | no - only delimiter nesting is bounded. Unary/`as`/closure/`return`/else-if/generic-angle/binary-operator nesting gives no E0910 and crashes the process at depth ~12k-20k (F1) |
| C28 | `msl_errors.rs` `depth_limit_fits_8mib_stack_in_debug` | yes, on the AC's literal terms: the caller's 8 MiB thread, no thread spawned (none anywhere in `src/msl`). The fit comes from chumsky's default `stacker` feature growing the stack in heap segments. Raw debug cost for 256 calls is 10 MiB (commit 2f94bf37), and the test cannot detect a raw overrun (F7) |
| C21 | `msl_print.rs` `roundtrip_ast_equal_over_corpus` | yes over the corpus, the AC's scope. `File: PartialEq` ignores only spans (`Loc::eq` is `true`). Literals compare value and raw text, and `Paren` is kept. The corpus has no multi-line string, so the printer's corruption of one goes unseen (F3) |
| C22 | `msl_print.rs` `print_is_idempotent_over_corpus` | yes |
| C23 | `msl_print.rs` `printer_preserves_docs_and_attrs` | yes for `///`, `//!` and `#[..]` on every node kind (F11 for block docs) |
| C24 | `msl_errors.rs` `empty_source_empty_file` | yes |
| C25 | `msl_lexer.rs` `non_utf8_one_diagnostic` | yes (with `parse_bytes` also covered in `empty_source_empty_file`) |
| C26 | `msl_parser.rs` `unknown_as_value_ok_as_name_e0908` | yes for the ten positions named. A module-body param `Unknown: f64 = 1.0;` gets E0900 (F9) |
| C27 | `msl_parser.rs` `contextual_keywords_are_identifiers_outside_nature` | yes |

### Points the dispatch asked about

- **C28 / 8 MiB.** The literal AC holds: 256 levels of every delimiter shape parse on the caller's
  8 MiB thread, and no thread is spawned. Stacker does not spawn a thread. It switches the same
  thread onto mmap'd segments past a 64 KiB red zone. The user's T16 intent ("reduce per-level
  stack use so 256 levels parse … on an 8 MiB caller stack") is met for parens, blocks,
  brackets and `if` (raw 4.9-7.7 MiB) but not for calls (10.0 MiB raw). Whether heap growth is
  acceptable is the user's call (F7).
- **The 4 `compile_fail` spec-bug fences.** Each is genuinely not one parse unit: `…` (U+2026)
  stands for an elided body in AsyncFifo and noise_table, and file-scope `let` is mixed with
  items in Dram1T1C/MemoryBank and idt. B.3 `item` has no `let`. They are not invalid MSL,
  though: every piece parses cleanly once split (module head, the `let`s inside a module body,
  `analog for Pi {…}`, the `MemoryBank::<_, 64, 32>(…){…}` and `nch(…)(…)` instances). The
  parser has no gap here. The relabel misuses the class and adds HTML comment lines (F8).
- **Other re-fences.** All 12 `rust,fragment` changes are info-string only and legitimate. The
  E0900/E0910 rows added to Appendix A were authorized by T11/design.md. The E0910 row words the
  limit as "delimiters", which is narrower than AC 6's "nesting" (F1).
- **Code choices.** E0121 (fn body) over E0404, and E0101 (`unsafe`/`async`/`await`/`dyn`/`&mut`)
  over E0903, take the more specific row. Both are defensible. E0142, E0143, E0302, E0303, E0331,
  E0602, E0611 and E0434 are not extras: each is a `parse`-phase row in the spec (Part I lines
  400-401, Part III 78-79/416, Part VI 111/181, Part IV 457), part of the 17. No
  elaboration-phase code is emitted (`src/msl` emits only the 17 parse codes plus E0900/E0910).
- **Hygiene.** The diff touches no pre-existing test. The new targets carry `//!` headers, with no
  `#[ignore]`, `cfg(any())` or `allow`. The fixtures assert exact codes and spans.

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 [Major] AC 6 is held only for delimiter nesting. Other nesting has no E0910 and overflows the stack | 8 MiB main thread, debug: `fn f() -> f64 { ` + `-`×20000 + `1 }` (also `as f64`×20000, `&#124;x&#124; `×20000, `return `×20000, `else if a {}`×12000, `Option<`×20000, `1` + `+1`×12000) → `fatal runtime error: stack overflow`. `parse_expr` survives 50k unary, so the overflow is in `check::file`'s recursive walk; `parse_expr` itself dies at 200k. 257-5000 unary or generic levels give no diagnostic. C20 tests only `( { [ f( if` | bound the parser's depth for prefix/postfix/cast/closure/`return`/else-if/generic-angle/binary chains (or make `check.rs` iterative or stacker-guarded), word E0910 as nesting rather than delimiters, and add those shapes to C20's test on an 8 MiB thread | 18134b0f |
| F2 [Major] `msl::print` overflows the stack on input the parser accepts | `fn f() -> f64 { 1` + `+1`×1300 + ` }`: `parse` gives 0 diagnostics, then `print` hits `thread 'main' has overflowed its stack` (1200 terms pass). An else-if chain of 2000 also dies | make the printer iterative over left-nested binary chains and else-if, or stacker-guard it, and add a print test for a long sum and a long else-if chain | b977fdef |
| F3 [Major] The printer changes string literal contents: continuation lines of a multi-line string are re-indented | `fn f() { let s = "a\n  b"; }` prints `"a\n      b"`. Re-parse gives `ast_eq=false`, and print is not idempotent. The same for a `\`-continued string | emit string literal text verbatim, with no indentation inside the quotes, and add multi-line and `\`-continued strings to the print fixture | b977fdef |
| F4 [Major] E0903's second half is not implemented, and `self`/`Self`/`super`/`crate` as bindings are accepted silently | `type A = u8;`, `static X: u8 = 1;`, `fn f() { yield 1; }` → `E0900 unexpected …` (row: "a Rust keyword where no production accepts it" is E0903). `fn f() { let self = 1; }`, `let Self`, `let super`, `let crate`, `for self in x {}` → 0 diagnostics. The C14 fixture covers only `fn out()` | map a keyword token found where no production accepts it to E0903, reject path keywords in binding patterns, and add fixtures for both halves of the row | 6193b6a0 |
| F5 [Major] An unclosed opening delimiter is reported at end of input, not at the delimiter | `fn f() { g(1, 2; h(3); }` → only `E0900 @24..24 unexpected end of input`; nothing points at `(` (10) or `;` (15). `fn f() { let x = [1, 2; let y = 3; }\nfn g() { let = 1; }` → g's error at 50 comes first, f's at 56 (EOF). The same for `(1; }` | report the unclosed opener (primary at it, secondary at the mismatched closer) in place, and add a case to C17's test | 6193b6a0 |
| F6 [Major] Inherited Rust lexical and value forms are rejected (A.1: "identifiers, comments, attributes, literals and the value syntax are Rust's") | `r"raw"`, `r#"x"#` → E0900 (a B1 handoff gap carried to B2 and not closed). `let x = 1.;` → E0900. `let café = 1;` and `let µ = 1;` → "unexpected character". A leading BOM → E0900. `let Some(x) = o else { return 0; };` → E0900. C9's test omits all of these | lex raw strings and non-ASCII XID identifiers, accept the `1.` float literal, skip a BOM, parse `let-else`, and add them to C9's test | 3c7f9bc6 |
| F7 [Minor] The 8 MiB fit depends on an undeclared default feature, and C28's test cannot see a raw overrun | `Cargo.toml`: `chumsky = { version = "=0.13.0", features = ["pratt"] }`; `stacker` comes only from chumsky's defaults (`recursive.rs:149` `maybe_grow(64 KiB, 1 MiB)`). Raw debug cost for 256 calls is 10.0 MiB (commit 2f94bf37). The no-thread guard greps only `entry.rs` | list `"stacker"` explicitly with a comment saying the depth guarantee relies on it, widen the guard to `src/msl/**`, and confirm with the user that heap-grown stack meets the T16 intent | 08a488e4 |
| F8 [Minor] The 4 `compile_fail` re-fences mark valid-but-elided spec examples, so C11 is vacuous and their content is untested | `part_iii_structure.md:223`, `:645` and `part_vii_stdlib.md:317`, `:480`: every piece parses with 0 diagnostics once split (checked one by one). No block in the corpus illustrates an error. The `<!-- spec-bug: … -->` lines go beyond "fence info string only" | user decision: fix the four examples in the spec (split the blocks, replace `…`) and re-fence them `rust`/`rust,fragment`, or record the class and the comment lines as approved spec edits | 135b26a5 |
| F9 [Minor] Parse rows report E0900 for shapes the row names | `emit!("s", a: 1)` → E0900 (E0602: "a field is not `IDENT = expr`"). `module M() { Unknown: f64 = 1.0; }` → E0900 (E0908: binding or item name). `1meg`, `1kHz`, `1mm` → E0902 "has no type suffix", although by A.2 rule 3 the letter is not an SI suffix, so this is an unknown literal suffix (E0900 row) | map these shapes to their row's code and add each one to the fixtures | 6193b6a0 |
| F10 [Minor] Cascading diagnostics after a reported error | `fn f() { let s = "abc` → the E0900 unterminated string plus `E0900 unexpected end of input`. The same for `/* abc` inside an item, and `fn c() { 1 + ; }` adds an EOF error. `digital for M { if c { on e {} } else { y = 2; } }` → a spurious `E0900 unexpected else` beside the correct E0434 | suppress an EOF error after a lexer error token or a recovered item, and let `gen<digital_item>`'s `else` recover to E0434 alone | 6193b6a0 |
| F11 [Minor] Block doc comments `/** … */` and `/*! … */` are treated as plain comments and lost by the printer | `/** doc */\nfn f() {}` prints `fn f() {}` | lex them as outer/inner doc tokens like `///`/`//!` | b977fdef |
| F12 [Minor] Comparison chains that Rust rejects are accepted | `a < b < c` → `(Lt (Lt a b) c)`, `a == b == c` → `(Eq (Eq a b) c)`, `a as u8 < b` → `(Lt (as a) b)`, all with 0 diagnostics | make comparison operators non-associative (E0900), as in Rust 2024 | 6193b6a0 |
| F13 [Minor] C8's test leaves about a third of the B.3 productions without a span assertion | `msl_parser.rs` `modules_ports_items_and_generation` (instance/callee section) and `behavior_blocks_and_builtin_forms` (call, method_call, attrs) use `matches!` with no `text(src, &span)` | assert `text(src, &node.span)` for instance, callee, call, method_call, sigtype, names, place and attr_* | 34257c6b |

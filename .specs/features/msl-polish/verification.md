# msl-polish verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at f9d533ae (130 pass, uncommitted, not rerun)
**Verifier**: independent sub-agent (author != verifier)

Method: read spec.md, tasks.md `## Checks`, the audit report and the test of every check; re-ran the audit's own evidence and about 300 adversarial sources at f9d533ae through `parse`, `print` (round trip), `Design::build`, `to_ast`, `Design::from_json` and the reflection handles, from a throwaway target (`tests/zz_verify_probe.rs`, removed; a copy is at the scratchpad `verify-polish/zz_verify_probe.rs`). Cargo ran with `-p piperine-lang` only.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `spec_text.rs` `b36_lists_every_removed_form` | yes |
| C2 | `spec_text.rs` `b36_names_the_forms_taken_in` | yes |
| C3 | `spec_text.rs` `b36_rows_carry_a_code_and_a_replacement` | yes |
| C4 | `spec_text.rs` `appendix_h_has_every_group` | yes |
| C5 | `spec_text.rs` `every_method_called_in_a_spec_fence_is_catalogued` | yes |
| C6 | `spec_text.rs` `catalogue_is_closed_by_e0161` | yes |
| C7 | `spec_text.rs` `appendix_h_is_in_the_file_map_and_nav` | yes |
| C8 | `pom_resolve.rs` `catalogue_traits_and_types_resolve` | no - the derive half asserts only "no diagnostic" (its own comment: attribute paths are never resolved); `#[derive(Bogus, Iterator)] struct S;` is also clean, so a derive name is neither resolved to a row nor ever `E0161` (F4) |
| C9 | `pom_resolve.rs` `builtin_table_matches_appendix_h` | yes |
| C10 | `spec_text.rs` `gnd_and_the_implicit_params_are_core_names` | yes |
| C11 | `pom_resolve.rs` `str_is_a_builtin_type` | yes |
| C12 | `pom_classify.rs` `nature_bound_is_builtin_and_classifies_the_port` | yes |
| C13 | `pom_resolve.rs` `gnd_is_a_builtin_node` | yes |
| C14 | `pom_resolve.rs` `m_and_dtemp_resolve_inside_a_module` | yes |
| C15 | `pom_resolve.rs` `m_and_dtemp_are_unknown_outside_a_module` | yes |
| C16 | `pom_resolve.rs` `a_local_shadows_an_implicit_param` | yes |
| C17 | `pom_resolve.rs` `core_names_cannot_be_redeclared` | yes |
| C18 | `pom_resolve.rs` `fixture_prelude_declares_no_core_name` | yes |
| C19 | `spec_text.rs` `e0162_is_defined_in_3_4` | yes |
| C20 | `spec_text.rs` `glob_shadowing_and_e0143_are_stated` | yes |
| C21 | `pom_resolve.rs` `duplicate_item_is_e0162_and_both_are_kept` | yes |
| C22 | `pom_resolve.rs` `import_colliding_with_an_item_is_e0162` | yes |
| C23 | `pom_resolve.rs` `ambiguous_glob_is_e0162_only_when_used` | yes |
| C24 | `pom_resolve.rs` `two_globs_of_one_declaration_are_clean` | yes |
| C25 | `pom_resolve.rs` `two_functions_of_one_name_stay_e0143` | yes |
| C26 | `pom_resolve.rs` `one_name_in_two_tables_is_clean` | yes |
| C27 | `spec_text.rs` `behavior_generics_are_repeated_not_positional` | yes |
| C28 | `spec_text.rs` `items_sit_at_namespace_scope` | yes |
| C29 | `pom_classify.rs` `mismatched_behavior_names_bind_by_name` | yes |
| C30 | `pom_classify.rs` `behavior_binds_across_namespaces_of_one_file` | yes |
| C31 | `spec_text.rs` `shared_borrow_of_any_argument_is_allowed` | yes |
| C32 | `spec_text.rs` `identifier_pattern_rule_is_stated` | yes |
| C33 | `pom_resolve.rs` `identifier_pattern_naming_none_is_a_path` | yes |
| C34 | `pom_resolve.rs` `identifier_pattern_naming_a_const_variant_or_unit_struct_is_a_path` | yes |
| C35 | `pom_resolve.rs` `other_identifier_patterns_stay_bindings` | yes |
| C36 | `spec_text.rs` `e0404_and_e0121_state_their_precedence` | yes |
| C37 | `spec_text.rs` `value_blocks_may_hold_contributions_and_commits` | yes |
| C38 | `spec_text.rs` `place_has_no_parentheses` | yes |
| C39 | `spec_text.rs` `attrs_are_allowed_before_statements_and_hardware_items` | yes |
| C40 | `spec_text.rs` `quantity_and_reaction_docs_are_metadata` | yes |
| C41 | `spec_text.rs` `post_parse_constraints_name_their_phase` | yes |
| C42 | `spec_text.rs` `e0125_covers_bodies` | yes (text); the POM messages for the two `E0125` faults state the rule, so a bodiless `fn a();` reads "... has a body" (F9) |
| C43 | `spec_text.rs` `raw_identifiers_are_specified_and_g15_is_closed` | yes |
| C44 | `spec_text.rs` `field_shorthand_is_specified_and_g14_is_closed` | yes |
| C45 | `spec_text.rs` `every_error_code_is_defined_exactly_once` | yes |
| C46 | `spec_text.rs` `b1_still_counts_65_constructs` | yes |
| C47 | `spec_text.rs` `appendix_g_records_the_decisions` | yes |
| C48 | `msl_parser.rs` `negative_literal_is_a_const_generic_argument` | yes |
| C49 | `msl_parser.rs` `commit_token_in_generic_arguments_is_still_an_error` | yes |
| C50 | `msl_parser.rs` `function_pointer_types_parse` | yes |
| C51 | `pom_body.rs` `function_pointer_type_round_trips` | yes |
| C52 | `msl_errors.rs` `removed_rust_forms_report_targeted_messages` | no - one position per row; the same rows in other positions get generic messages: `use ::a::b;` (`unexpected ::`), `struct S<T> where T: Tr`, `impl<T> S<T> where T: Tr`, `trait Tr<T> where T: U` (`unexpected where`) (F5) |
| C53 | `msl_errors.rs` `every_b36_row_has_a_fixture` | yes |
| C54 | `msl_errors.rs` `body_let_annotation_has_its_own_message` | yes |
| C55 | `msl_lexer.rs` `raw_identifier_of_an_added_keyword_is_an_identifier` | yes |
| C56 | `msl_errors.rs` `raw_identifier_of_another_word_is_e0900` | yes |
| C57 | `msl_print.rs` `raw_identifiers_print_with_their_prefix` | yes |
| C58 | `pom_resolve.rs` `raw_identifier_names_drop_the_prefix` | yes |
| C59 | `msl_parser.rs` `param_block_and_template_shorthand_keep_their_form` | yes |
| C60 | `msl_print.rs` `field_shorthand_prints_as_written` | yes |
| C61 | `pom_classify.rs` `shorthand_field_binds_param_and_reads_the_local` | yes |
| C62 | `msl_parser.rs` `docs_and_attrs_attach_to_statements` | yes |
| C63 | `msl_errors.rs` `dangling_doc_comment_has_its_own_message` | yes |
| C64 | `msl_parser.rs` `docs_and_attrs_attach_to_hardware_items` | yes |
| C65 | `msl_print.rs` `statement_and_hardware_docs_round_trip` | yes |
| C66 | `pom_corpus.rs` `docs_at_every_position_round_trip` | yes |
| C67 | `pom_tree.rs` `docs_reach_reactions_quantities_definitions_and_statements` | yes |
| C68 | `spec_text.rs` `e0909_is_defined_for_out_of_range_literals` | yes |
| C69 | `msl_lexer.rs` `float_literal_out_of_range_is_e0909` | yes |
| C70 | `msl_lexer.rs` `integer_literal_out_of_range_is_e0909` | yes |
| C71 | `msl_errors.rs` `every_parse_phase_row_has_a_fixture` | yes |
| C72 | `msl_lexer.rs` `invalid_string_escape_is_e0900` | yes |
| C73 | `msl_print.rs` `raw_strings_print_as_written` | yes |
| C74 | `msl_lexer.rs` `literal_accessors_return_cooked_values` | yes |
| C75 | `pom_reflect.rs` `float_literals_round_trip_through_json` | yes |
| C76 | `pom_reflect.rs` `float_literal_is_serialized_from_its_text` | yes |
| C77 | `msl_errors.rs` `commit_target_must_be_a_place` | yes - six non-places and four places at the parser; `q.0 <- 1;` is accepted while `q.0 = 1;` is rejected and B.3.2 `place` has no tuple index (F8) |
| C78 | `msl_errors.rs` `parenthesized_place_is_rejected_in_both_forms` | yes |
| C79 | `msl_errors.rs` `signal_outside_a_module_body_is_e0404` | yes |
| C80 | `msl_errors.rs` `behavior_statements_in_a_module_body_are_e0404` | yes for the three forms it names; the spec's own `E0404` row also lists `bound_step` in a module body, which gives a generic `E0900` (F7) |
| C81 | `msl_errors.rs` `function_body_reports_e0121_only` | yes |
| C82 | `msl_errors.rs` `pub_on_a_behavior_block_is_e0900` | yes |
| C83 | `msl_errors.rs` `forms_outside_the_grammar_are_parse_errors` | yes |
| C84 | `pom_resolve.rs` `misplaced_signal_declares_nothing` | yes |
| C85 | `spec_text.rs` `e0910_counts_nesting_and_bounds_chains` | yes |
| C86 | `msl_errors.rs` `flat_chains_up_to_4096_links_are_not_nesting` | yes on its 8 MiB stack; no case puts comparisons in a list, where D8 is broken (F1), and on a default 2 MiB thread a 4096-term sum aborts the process (F3) |
| C87 | `msl_errors.rs` `flat_chain_past_4096_links_is_e0910` | yes - asserts one `E0910` naming 4096 at 4097 and 20000 links (AC text says "4097 terms", which is 4096 links and is clean; the test follows the bound) |
| C88 | `msl_errors.rs` `nesting_without_delimiters_is_also_bounded` | yes |
| C89 | `msl_errors.rs` `bad_where_list_is_one_diagnostic` | yes |
| C90 | `msl_errors.rs` `statement_errors_recover_inside_the_block` | yes |
| C91 | `msl_errors.rs` `a_bad_port_keeps_the_module` | yes |
| C92 | `pom_resolve.rs` `module_with_a_bad_port_still_resolves` | yes |
| C93 | `pom_resolve.rs` `use_self_imports_the_namespace` | yes |
| C94 | `pom_resolve.rs` `use_self_with_alias_and_nesting` | yes |
| C95 | `pom_resolve.rs` `use_leaf_text_is_its_own_segments` | yes |
| C96 | `pom_resolve.rs` `repr_members_resolve_through_a_bound` | yes |
| C97 | `pom_resolve.rs` `unknown_repr_member_is_e0161` | yes |
| C98 | `pom_resolve.rs` `member_through_a_builtin_bound_is_deferred` | yes |
| C99 | `pom_classify.rs` `param_block_errors_are_e0320` | yes |
| C100 | `pom_classify.rs` `named_argument_errors_are_e0140` | yes |
| C101 | `pom_classify.rs` `bundle_literal_field_errors_are_e0263` | yes |
| C102 | `pom_classify.rs` `unknown_struct_field_is_reported_in_literals_and_patterns` | yes |
| C103 | `pom_classify.rs` `extern_module_shape_is_e0370` | yes |
| C104 | `pom_classify.rs` `function_body_rule_is_e0125` | yes - asserts code and span; the messages read as the opposite fault (F9) |
| C105 | `pom_classify.rs` `nature_field_errors_are_e0201` | yes |
| C106 | `pom_classify.rs` `access_of_another_nature_is_e0206` | yes |
| C107 | `pom_classify.rs` `carrier_method_on_an_array_is_e0211` | yes |
| C108 | `pom_reflect.rs` `parent_drive_is_a_writer_of_the_child_carrier` | yes |
| C109 | `pom_reflect.rs` `by_name_lookups_prefer_project_files` | yes |
| C110 | `pom_reflect.rs` `collections_list_project_files_only` | yes |
| C111 | `pom_reflect.rs` `design_has_a_collection_and_lookup_per_item_kind` | yes |
| C112 | `pom_body.rs` `expr_children_and_parent_are_inverse` | yes |
| C113 | `pom_body.rs` `decl_exprs_is_preorder_over_the_body` | yes |
| C114 | `pom_body.rs` `block_and_stmt_handles_walk_a_body` | yes - BlockHandle, not the `Block` name the criterion and design.md give (documented deviation, F11) |
| C115 | `pom_classify.rs` `arg_and_field_handles_expose_name_value_target` | yes |
| C116 | `pom_classify.rs` `builtin_instance_names_its_primitive` | yes |
| C117 | `pom_reflect.rs` `types_list_their_impls_and_methods` | yes |
| C118 | `pom_tree.rs` `generation_if_items_cover_every_arm` | yes |
| C119 | `pom_reflect.rs` `from_json_reads_back_a_valid_design` | yes |
| C120 | `pom_reflect.rs` `from_json_rejects_a_corrupted_design` | no - corrupts four id fields only; `expr_parent`/`expr_stmt` entries out of range panic inside `from_json` (`check.rs:386`, `design.rs:228`), and an out-of-range `index.writers` id is accepted and panics on use (F2) |
| C121 | `pom_design.rs` `member_tables_scale_subquadratically` | yes |
| C122 | `pom_design.rs` `name_lookups_do_not_scan` | yes |
| C123 | `pom_docs.rs` `every_b1_construct_is_mapped` | yes |
| C124 | `pom_docs.rs` `place_has_no_parenthesized_form_in_the_docs` | yes |
| C125 | `pom_docs.rs` `every_code_the_pom_reports_is_documented` | no - asserts a row per scraped code but never the spec-section column the criterion requires (F10) |
| C126 | `pom_docs.rs` `every_builtin_row_is_documented` | yes |
| C127 | `pom_docs.rs` `every_reflection_method_is_documented` | yes |
| C128 | `msl_corpus.rs` `every_valid_spec_block_parses_clean` | yes |
| C129 | `pom_corpus.rs` `every_spec_block_round_trips_to_its_ast` | yes |

## Audit closure

| Finding | Status | Evidence at f9d533ae |
| --- | --- | --- |
| A-01 | closed | `const S: &'static str` clean; `module M<N: Nature>(a: N, g: N)` with `gnd`, `m * dtemp` clean (only `R` unknown, no prelude); items named `gnd`, `Nature`, `str` give `E0457`; `m` in a `fn` or `const` is `E0161` |
| A-02 | closed | all five audit sources clean, `None`, `ZERO`, `A`/`B` via `use E::*`, `Dc`, `U` resolve as paths (C33, C34); a const declared after the `match` resolves too |
| A-03 | deferred | `msl-stdlib` (D4) |
| A-04 | closed | the eight audit targets: `-q`, `a + b`, `f(q)`, `q.x()`, `*q`, `q as u8`, `(q)` are one `E0900` each; also `q?`, `&q`, `(q)[0]`, `q[0].x()`. `q[1..2]` is a place by B.3.2. See F8 for `q.0` |
| A-05 | closed | `signal` in `analog`, a reaction, a nested block, a closure: one `E0404`; in a `fn` and a closure in it: `E0121` |
| A-06 | closed | `Int< -1 >`, `Int< - 1 >`, `Int< -1i8 >`, `f::< -1 >()` clean and round-trip; `Int<-1>` is `E0907` at `<-` |
| A-07 | closed | docs before `let`, contribution, reaction, definition, nature field, generation, tail expression, match arm: clean and round-trip; dangling doc at `}` and EOF has its own message. See F6, F14 for positions outside D9's list |
| A-08 | closed | `use a::{self, K}` and `use a::{self as aa, n::{self, J as JJ}}` clean |
| A-09 | closed | `cast` clean; `A::NOPE` is one `E0161`; `T: PartialOrd` member defers |
| A-10 | closed | audit source gives four `E0162`; import vs item, two explicit imports, used ambiguous glob all `E0162`; unused globs and glob-vs-item silent. Block scope stays silent (F12) |
| A-11 | closed | `1e999` is `E0909`; design JSON round-trips equal (C75, own probe) |
| A-12 | closed | prelude/project `Bit`, `Buf`, `S`, `f`: every lookup returns the project item; collections hold one each |
| A-13 | closed | `children`/`parent`/`stmt`/`exprs` over a fn with closure and nested fn: pre-order, nested item skipped |
| A-14 | closed | `zz` and repeated `r` are `E0320`; `d`, repeated `k`, `q`, carrier `n` are `E0140` |
| A-15 | closed | `i(t)` on `Th` is `E0206`; a generic-nature port is not reported |
| A-16 | closed | `arr.rise()` and `arr.changed()` are `E0211`; `arr[0].rise()` resolves |
| A-17 | open | 15 of 17 forms targeted, but two forms of the audit's own evidence still get generic messages: `use ::a::b;` and `where` on `struct`/`impl`/`trait` (F5); B.3.6's completeness sentence is contradicted by Rust forms it does not list (F6) |
| A-18 | closed | Appendix H exists and is closed by `E0161` in text; the POM side of derives is F4 |
| A-19 | closed | §1 rule 2 and `E0102` allow `&value` as an argument; `(0.1..=10.0).contains(&n)` parses |
| A-20 | closed | `analog<X> for M<X>`, reordered and shorter lists are `E0403`; names link by name (C29) |
| A-21 | closed | text in B.2.1 note 3, §8 rule 1, §3.4.3; a block in another namespace binds to a `pub module` (a private one is `E0161 private`, by Rust's path rules) |
| A-22 | closed | `pub analog for`/`pub digital for`: one `E0900` at `pub`, block kept |
| A-23 | closed | `{ r }`, `{ r, }`, `{ r#out }`, `R { r, c: 1p }` keep their form through print and POM |
| A-24 | closed | `q <- 1;`, `i(a) +> 1.0;`, `on e {}` in a body: one `E0404` with help. `bound_step` in a body is F7 |
| A-25 | open | flat sums, method, cast, `else if` chains to 4096 links clean, 4097 is one `E0910`; but a list of comparisons is still counted as nesting (F1) |
| A-26 | closed | the three audit sources: one diagnostic at the comma, `f` one item with `let c`, `M` kept with `b` and `s` |
| A-27 | closed | `\q`, `\x9`, `\x80`, `\u{110000}`, `\u{D800}`, `\u{}` are `E0900` at the escape; raw strings print raw; huge integers are `E0909` |
| A-28 | closed | `static`, `r#out`, body `let` annotation have targeted messages. Stale `r#` help elsewhere is F14 |
| A-29 | closed | empty nature, tuple, reference and fn-pointer signal types are `E0900`; `extern "C" fn` with body and bodiless `fn` are `E0125` |
| A-30 | closed | parent `c.w = true;` and `c.w <- true;` give `C.w.writers().len() == 2` |
| A-31 | closed | `items()` of an `if`/`else if`/`else` generation lists `a`, `b`, `c` |
| A-32 | closed | C121, C122 green (not re-timed) |
| A-33 | open | `from_json` panics on 14 of 440 single-field corruptions and accepts one that panics on use (F2) |
| A-34 | closed | `E0125` row extended; POM reports both forms |
| A-35 | closed | B.3.4 states it (C37) |
| A-36 | closed | rows state precedence; parser follows it for all five forms |
| A-37 | closed | `E0909` in A.6, A.2, §1 rule 7; boundaries `255u8`, `128i8`, `2^128-1`, `2^127 i128`, `1.7976931348623158e308` clean, one past each is `E0909` |
| A-38 | closed | audit source gives three `E0370` (result, signal, definition); value input too |
| A-39 | closed | `(q) = 1;` and `(q) <- 1;` same `E0900`; docs fixed (C124). F8 is a sibling inconsistency |
| A-40 | closed | see A-10 |
| A-41 | closed | 65-row table, guard C123 |
| A-42 | closed (in scope) / deferred (stdlib half) | `Nature` stand-in gone; place, serde and identifier-pattern tests exist; `State`, `Logic`, `init` and the gallery-clean test go to `msl-stdlib` |
| A-43 | closed | leaf `text()`/`span()` are own segments (C95) |
| A-44 | closed | `cross(..)` and `ddt(..)` instances answer `builtin()`; a user module answers none |
| A-45 | closed | `S { a, q }`, `S { q, .. }` and `S { q }` literal all one `E0161` |
| A-46 | closed | `E0162` covers modules, consts, mods; two `fn` stay `E0143` only |

Count: 42 closed (A-42 counted for its in-scope half), 3 open (A-17, A-25, A-33), 1 deferred (A-03, plus the stdlib half of A-42).

## Decisions

| Decision | Stated normatively | Code matches |
| --- | --- | --- |
| D1 | B.3.6 table and "Taken from Rust" (C1 to C3) | partly: F5, F6 |
| D2 | Appendix H, §0.4, §1 rule 1 | partly: bound and type names resolve; derive names are never resolved or rejected (F4) |
| D3 | §6 rule 11, A.1 rule 8, §3.4.2 | yes |
| D5 | §3.4 `E0162` row and rules | yes at namespace scope; block-level items silent (F12) |
| D6 | §10 rule 6, Resolution, B.3.3 item 14 | yes |
| D7 | §1 rule 2, `E0102` | yes (parse); `&&x` rejected (F6) |
| D8 | A.6 `E0910` row | no for comparisons in lists (F1) |
| D9 | A.4, B.3.2, §21 | yes for the listed positions; Rust outer-attribute positions outside the list rejected (F6) |
| D10 | Part I identifier-pattern rule | yes |
| D11 | codes in POM | yes |
| D12 | A.2, A.6 `E0909`, §1 rule 7 | yes |
| D13 | B.2.1 note 3, §8 rule 1, §3.4.3 | yes |
| D14 | `E0404`/`E0121` rows, §10 rule 8 | partly: `bound_step` in a module body is generic `E0900` (F7) |
| D15 | A.1 rule 6, G.15 Closed | yes for the lexer and parser; reserved-keyword help still says raw identifiers are not supported (F14) |
| D16 | B.3.2 fields, G.14 Closed | yes |
| D17 | B.3.3 place note | yes for parentheses; tuple index accepted in a commit only (F8) |

## Handoff items scrutinized

- B1 (T3 to T6 spec edits with no red run): the text tests (C10 to C35 text rows) assert the decided wording by substring and each would fail on the old text; acceptable.
- B4 (`Str` struct variant): every edited assertion keeps its expected string; only the variant shape moved. Legitimate.
- B5 (CHAINS, print limits): `binary +1`, `cast chain`, `else-if chain` left CHAINS because D8 says they are not nesting; print round-trips rose from 250 to 4000 links (stronger). Legitimate. The tentative `<` count is a real false `E0910` (F1).
- B6 (`Nature` to `Nope`): the unresolved-bound case still asserts `Unknown`, now with a name that is not built in. Legitimate. Derive args unresolved: C8 does not hold its criterion (F4).
- B7 (carrier-port-by-name fixtures): §8.3.9 forbids naming a carrier port; the replacement names an `Option` value port, and `named_binding_without_a_target_is_e0161` was replaced by C99/C100/C102, which assert the new codes and the `Unresolved` slots. Legitimate.
- B8 (call sites to handles, `BlockHandle`): assertions read the same values through `node()`; the name departs from the criterion's `Block` (F11).

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 MAJOR - a list of comparisons is counted as nesting: false `E0910` on valid flat source (D8, A-25, POL-15) | `fn f() { let v = [x < 1, x < 1, ...]; }` with 255 elements: `E0910 nesting deeper than 256 levels`; same for a call `g(a < b, ...)`, a tuple, a struct literal of 300 fields `aN: x < 1`, 300 match arms `1 => a < b,`, a module-body `let`. `a < b && ...` x300 is clean | release the tentative `<` count at `,`, `;`, `=>` and the closing delimiter of the enclosing list, or count `<` only in type and turbofish position; add a check with 300 comparisons in each list form | 76367f6d |
| F2 MAJOR - `Design::from_json` panics on corrupted ids (POL-21 AC 2, A-33) | setting one numeric JSON leaf to 99999: 11 `expr_parent.N` and 3 `expr_stmt.N` corruptions panic inside `from_json` (index out of bounds at `pom/check.rs:386`, `pom/design.rs:228`); `index.writers.N.0.Definition=99999` is accepted and panics in `Signal::writers()`. C120 corrupts only `exprs.owner`, `decls.children`, `files.root`, `stmts.owner` | bound-check `expr_parent`, `expr_stmt` and every `index` id in `check()` before dereferencing; extend C120 to a per-leaf corruption sweep that asserts `Err` or a usable design, never a panic | cfc4f226 |
| F3 MAJOR - parsing a spec-valid 4096-link chain aborts the process on a default 2 MiB thread | `parse("fn f() -> f64 { 1+1...+1 }")` with 4096 terms in a `std::thread` of 2 MiB: `has overflowed its stack`, SIGABRT (2000 terms pass). The parser itself is not under `stack::deep()`; the bound of D8 was sized for an 8 MiB stack, which spawned threads, test harness threads and LSP workers do not have | wrap the parser's recursive entry points in `deep()` (as the printer and lowering are), or parse binary and postfix chains iteratively; add a 2 MiB-thread case to C86 | 76367f6d |
| F4 MAJOR - derive names are never resolved: C8 does not hold POL-02 AC 5, and D2's "everything else is `E0161`" does not apply to derives | `#[derive(Bogus, Iterator)] struct S;` builds with zero diagnostics; C8's derive half asserts only an empty diagnostic list ("attribute paths are never resolved") | resolve `#[derive(..)]` paths against the built-in trait rows and user traits; `E0161` for an unknown one; assert the resolution in C8 | d2031202 |
| F5 MINOR - two forms of A-17's own evidence still get generic messages (POL-09 AC 5) | `use ::a::b;` gives `unexpected ::, expected {, identifier or *`; `struct S<T> where T: Tr`, `impl<T> S<T> where T: Tr`, `trait Tr<T> where T: U` give `unexpected where`. The fixture table has one position per row (`let x = ::a::b;`, `fn ... where T: Tr`) | targeted message in every position of the row; add these fixtures | 91038e8e |
| F6 MINOR - B.3.6 says its table is the complete list of what is rejected, but Rust forms it does not list are rejected | `fn(x: f64) -> f64` (named fn-pointer params), `fn f(#[a] x: f64)`, `fn f<#[a] T>()`, `S { #[a] x: 1 }`, `fn f(/// x` doc on a param, `g(&&a)` (double borrow) all `E0900` with generic text; D9 says docs and attributes go everywhere Rust takes an outer attribute | either accept them or add rows to B.3.6 with targeted messages | 91038e8e |
| F7 MINOR - `bound_step` in a module body is a generic `E0900`, while the spec's `E0404` row and §10 rule 8 (D14) list it | `module M() { bound_step(1.0); }` gives `E0900 unexpected (, expected an operator, :: or :`; in a reaction or closure it is `E0404` | report `E0404` with help for a `bound_step` call in a module body; add it to C80 | bfca400b |
| F8 MINOR - tuple index accepted as a commit place only | `q.0 <- 1;` (and `q.0 <- 1 after 1n;`) is clean; `q.0 = 1;` is `unexpected 0, expected identifier`; B.3.2 `place` is `path ( [expr] / .IDENT )*` | apply the same place check to a numeric field in a commit | bfca400b |
| F9 MINOR - `E0125` messages state the rule, so they read as the opposite fault | `fn a();` and `impl S { fn a(self); }` report "a function that is neither `extern` nor a trait item has a body"; `extern "C" fn a(..) { .. }` reports "an `extern` function has no body" | phrase the fault: "this function has no body ..." / "an `extern` function cannot have a body" | 8dffd7c6 |
| F10 MINOR - C125 does not check the spec section of each code | `every_code_the_pom_reports_is_documented` compares first cells only; POL-22 AC 3 requires the defining section in each row | assert a non-empty section cell that names a `§` or Appendix | b2d7e987 |
| F11 MINOR - the block handle is `BlockHandle`, not the `Block` POL-20 AC 6 and design.md name | `pom/mod.rs:54` exports `BlockHandle`; other handles take the concept name (`Expr`, `Stmt`) with `*Node` storage | rename storage to `BlockNode` and the handle to `Block`, or amend the criterion | 63a365ef |
| F12 MINOR - duplicate items in a block are silent | `fn f() { fn g() {} fn g() {} }` builds with zero diagnostics (no `E0143`, no `E0162`); §3.4 defines namespaces as files and `mod`s and says nothing of block items | state the rule for items in a block and report it | 512e7057 |
| F13 MINOR - Appendix H tables render broken code spans | H.8 "Written as" cells are ``#[derive(Clone)]`, `T: Clone`` (double-backtick span with inner backticks); H.9 "What it is" wraps prose in code with nested backticks (`unsigned integer of `N` levels; ...`) | write each form as its own code span and the prose plain | ac18ac91 |
| F14 MINOR - stale and misleading help texts | the `E0903` reserved-keyword help still says "raw identifiers (`r#name`) are not supported", false since D15; a doc comment before `else` reports "`else` is a reserved keyword and cannot be used as an identifier" | drop the stale clause (or name the seven words); report a doc comment before `else` as documenting nothing | bfca400b |

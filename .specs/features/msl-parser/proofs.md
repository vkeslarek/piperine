# msl-parser proofs

**HEAD**: 6eda5251
**Tree**: clean
**Shell**: /bin/sh
**Result**: 31 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run lexer / test si_literal_scaled_value_and_raw_text ... ok` | pass |
| C2 | `run lexer / test malformed_si_literal_reports_e0902 ... ok` | pass |
| C3 | `run lexer / test dropped_token_class_reports_e0905 ... ok` | pass |
| C4 | `run lexer / test contribution_and_commit_operators_are_single_tokens ... ok` | pass |
| C5 | `run lexer / test keywords_reserved_vs_contextual ... ok` | pass |
| C6 | `run lexer / test doc_comments_are_tokens ... ok` | pass |
| C7 | `run lexer / test unterminated_comment_and_string_one_diagnostic ... ok` | pass |
| C8 | `run parser / test every_b3_production_parses_with_span ... ok` | pass |
| C9 | `run parser / test inherited_rust_forms_parse ... ok` | pass |
| C10 | `run corpus / test every_valid_spec_block_parses_clean ... ok` | pass |
| C11 | `run corpus / test every_compile_fail_block_diagnoses ... ok` | pass |
| C12 | `run parser / test contribution_commit_at_any_depth ... ok` | pass |
| C13 | `run parser / test doc_comments_and_attrs_attach ... ok` | pass |
| C14 | `run errors / test parse_phase_rows_report_their_codes ... ok` | pass |
| C15 | `run errors / test every_parse_phase_row_has_a_fixture ... ok` | pass |
| C16 | `run errors / test diagnostic_has_code_message_label_help ... ok` | pass |
| C17 | `run errors / test multiple_errors_in_source_order_with_partial_ast ... ok` | pass |
| C18 | `run errors / test unexpected_token_names_found_and_expected ... ok` | pass |
| C19 | `run errors / test rendered_output_is_miette_graphical ... ok` | pass |
| C20 | `run errors / test depth_over_256_is_diagnostic_not_overflow ... ok` | pass |
| C28 | `run errors / test depth_limit_fits_8mib_stack_in_debug ... ok` | pass |
| C21 | `run print / test roundtrip_ast_equal_over_corpus ... ok` | pass |
| C22 | `run print / test print_is_idempotent_over_corpus ... ok` | pass |
| C23 | `run print / test printer_preserves_docs_and_attrs ... ok` | pass |
| C24 | `run errors / test empty_source_empty_file ... ok` | pass |
| C25 | `run lexer / test non_utf8_one_diagnostic ... ok` | pass |
| C26 | `run parser / test unknown_as_value_ok_as_name_e0908 ... ok` | pass |
| C27 | `run parser / test contextual_keywords_are_identifiers_outside_nature ... ok` | pass |
| Build | `export CARGO_TARGET_DIR=/home/keslarek/Git/piperine/target` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^warning')` | pass |
| Build | `cargo test -p piperine --test suite_hygiene` | pass |

## Output

`Run lexer` - exit 0

```
running 8 tests
test contribution_and_commit_operators_are_single_tokens ... ok
test doc_comments_are_tokens ... ok
test malformed_si_literal_reports_e0902 ... ok
test dropped_token_class_reports_e0905 ... ok
test non_utf8_one_diagnostic ... ok
test unterminated_comment_and_string_one_diagnostic ... ok
test keywords_reserved_vs_contextual ... ok
test si_literal_scaled_value_and_raw_text ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/msl_lexer.rs (/home/keslarek/Git/piperine/target/debug/deps/msl_lexer-563f0fc6aee4f0c2)
```

`Run parser` - exit 0

```
test struct_expression_field_shorthand ... ok
test nested_tuple_index ... ok
test doc_comments_and_attrs_attach ... ok
test contextual_keywords_are_identifiers_outside_nature ... ok
test unknown_as_value_ok_as_name_e0908 ... ok
test contribution_commit_at_any_depth ... ok
test inherited_rust_forms_parse_items ... ok
test inherited_rust_forms_parse ... ok
test every_b3_production_parses_with_span ... ok
test inherited_rust_forms_parse_expressions ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/msl_parser.rs (/home/keslarek/Git/piperine/target/debug/deps/msl_parser-e1520082db1c95f5)
```

`Run corpus` - exit 0

```
running 2 tests
test every_compile_fail_block_diagnoses ... ok
test every_valid_spec_block_parses_clean ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/msl_corpus.rs (/home/keslarek/Git/piperine/target/debug/deps/msl_corpus-4f11e8d1ec55e1d4)
```

`Run errors` - exit 0

```
test keyword_as_identifier_reports_e0903 ... ok
test unexpected_token_names_found_and_expected ... ok
test multiple_errors_in_source_order_with_partial_ast ... ok
test every_parse_phase_row_has_a_fixture ... ok
test statements_of_the_wrong_block_kind_report_e0404 ... ok
test forbidden_forms_in_a_function_body_report_e0121 ... ok
test parse_phase_rows_report_their_codes ... ok
test diagnostic_has_code_message_label_help ... ok
test depth_limit_fits_8mib_stack_in_debug ... ok
test depth_over_256_is_diagnostic_not_overflow ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/msl_errors.rs (/home/keslarek/Git/piperine/target/debug/deps/msl_errors-6c979a0a775241ba)
```

`Run print` - exit 0

```
running 3 tests
test printer_preserves_docs_and_attrs ... ok
test roundtrip_ast_equal_over_corpus ... ok
test print_is_idempotent_over_corpus ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/msl_print.rs (/home/keslarek/Git/piperine/target/debug/deps/msl_print-50016ff773bc3cb9)
```

# composite-values phase D proofs (tasks-d.md)

**HEAD**: ce94b0e5
**Tree**: clean
**Shell**: /bin/sh
**Result**: 50 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C146 | `run srcw / test f32_computes_what_rust_f32_computes ... ok` | pass |
| C147 | `run srcw / test a_carried_f32_is_the_f32_leaf ... ok` | pass |
| C148 | `run typesw / test an_f32_where_an_f64_is_required_is_a_mismatch ... ok` | pass |
| C149 | `run srcw / test f16_is_ieee_binary16_bit_for_bit ... ok` | pass |
| C150 | `run half / test every_unary_f16_operation_is_correctly_rounded_over_all_inputs ... ok` | pass |
| C151 | `run half / test binary_f16_operations_are_correctly_rounded ... ok` | pass |
| C152 | `run transcendental / test every_f16_transcendental_is_correctly_rounded_over_all_inputs ... ok` | pass |
| C153 | `run srcw / test f128_is_ieee_binary128_bit_for_bit_in_every_operation_and_method ... ok` | pass |
| C154 | `run quad / test f128_arithmetic_and_square_root_are_correctly_rounded ... ok` | pass |
| C155 | `run quad / test f128_exact_methods_follow_h2 ... ok` | pass |
| C156 | `run conversions / test every_as_conversion_follows_the_table_of_1_4_5 ... ok` | pass |
| C157 | `run transcendental / test exp_and_logarithms_match_the_reference_vectors_bit_for_bit ... ok` | pass |
| C158 | `run transcendental / test powers_match_the_reference_vectors_bit_for_bit ... ok` | pass |
| C159 | `run transcendental / test circular_functions_match_the_reference_vectors_bit_for_bit ... ok` | pass |
| C160 | `run transcendental / test inverse_circular_functions_match_the_reference_vectors_bit_for_bit ... ok` | pass |
| C161 | `run transcendental / test hyperbolic_functions_match_the_reference_vectors_bit_for_bit ... ok` | pass |
| C162 | `run transcendental / test the_reference_vectors_are_complete_and_well_formed ... ok` | pass |
| C163 | `run srcw / test wide_integers_wrap_modulo_two_to_the_n_in_every_operation ... ok` | pass |
| C164 | `run integers / test wide_integers_up_to_128_bits_agree_with_u128_and_i128 ... ok` | pass |
| C165 | `run integers / test wide_integers_above_128_bits_agree_with_big_integers_modulo_two_to_the_n ... ok` | pass |
| C166 | `run typesw / test a_literal_out_of_its_type_is_e0909 ... ok` | pass |
| C167 | `run srcw / test a_wide_literal_past_its_type_is_e0909 ... ok` | pass |
| C168 | `run apiw / test the_host_reads_each_width_as_one_exact_value ... ok` | pass |
| C169 | `run clirun / test number_widths_print_exactly_in_every_format ... ok` | pass |
| C170 | `run typesw / test a_width_outside_the_rules_is_e0103 ... ok` | pass |
| C171 | `run typesw / test every_float_and_integer_width_type_checks ... ok` | pass |
| C172 | `run interp / test each_number_width_is_refused_by_name ... ok` | pass |
| C173 | `run elabw / test const_contexts_compute_every_width_as_the_run_does ... ok` | pass |
| C174 | `run elabw / test a_const_panic_of_a_new_width_names_the_expression ... ok` | pass |
| C175 | `run elabw / test number_widths_lower_to_their_ir_types_and_constants ... ok` | pass |
| C176 | `run purity / test the_numeric_crate_is_embeddable_source ... ok` | pass |
| C177 | `run integers / test limbs_agree_with_big_integers_on_every_operation ... ok` | pass |
| C178 | `run formats / test rounding_to_every_format_is_nearest_ties_to_even ... ok` | pass |
| C179 | `run formats / test decimal_text_reads_to_the_nearest_value_and_prints_the_shortest ... ok` | pass |
| C180 | `run precision / test directed_rounding_brackets_the_exact_result ... ok` | pass |
| C181 | `run precision / test reference_constants_contain_their_published_digits ... ok` | pass |
| C182 | `run precision / test reduction_keeps_full_precision_for_every_finite_argument ... ok` | pass |
| C183 | `run precision / test ziv_rounding_raises_precision_until_the_bounds_agree ... ok` | pass |
| C184 | `run irw / test number_width_types_and_constants_are_checked_printed_and_serialized ... ok` | pass |
| C185 | `run planw / test each_new_scalar_has_its_leaf_count_and_bytes ... ok` | pass |
| C186 | `run tw / test a_design_without_number_widths_gets_todays_runtime_files ... ok` | pass |
| C187 | `run tw / test number_widths_compile_and_compute_as_their_library ... ok` | pass |
| C188 | `run book / test a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it ... ok` | pass |
| C188 | `run book / test floats_of_every_width_and_wide_integers_run ... ok` | pass |
| C189 | `run docs / test gaps_drops_what_composite_values_phase_d_closed ... ok` | pass |
| C190 | `cargo test -p piperine-cli --test twins` | pass |
| C192 | `run rtw / test widths::tests::the_runtime_raises_msl_panics_and_carries_every_width ... ok` | pass |
| C191 | `cargo build -p piperine-numbers -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperi` | pass |
| Build | `cargo build -p piperine-numbers -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperi` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run srcw` - exit 0

```
   Compiling toml v1.1.2+spec-1.1.0
   Compiling fastrand v2.4.1
   Compiling tempfile v3.27.0
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-lang)
   Compiling git2 v0.21.0
   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-plan)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-transpile)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-interp)
   Compiling piperine-project v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-project)
   Compiling piperine-api v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-api)
   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-parity)
    Finished `release` profile [optimized + debuginfo] target(s) in 5m 23s
     Running tests/source_number_widths.rs (/var/tmp/piperine-target-a/release/deps/source_number_widths-64bfffe9cbe14502)
```

`Run typesw` - exit 0

```
running 4 tests
test an_f32_where_an_f64_is_required_is_a_mismatch ... ok
test every_float_and_integer_width_type_checks ... ok
test a_width_outside_the_rules_is_e0103 ... ok
test a_literal_out_of_its_type_is_e0909 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/types_number_widths.rs (/var/tmp/piperine-target-a/debug/deps/types_number_widths-59189c564e7af7bc)
```

`Run half` - exit 0

```
running 2 tests
test every_unary_f16_operation_is_correctly_rounded_over_all_inputs ... ok
test binary_f16_operations_are_correctly_rounded ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.76s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/half.rs (/var/tmp/piperine-target-a/debug/deps/half-24cc1f4db1b6780c)
```

`Run transcendental` - exit 0

```
test inverse_circular_functions_match_the_reference_vectors_bit_for_bit ... ok
test exp_and_logarithms_match_the_reference_vectors_bit_for_bit ... ok
test powers_match_the_reference_vectors_bit_for_bit ... ok
test circular_functions_match_the_reference_vectors_bit_for_bit ... ok
test every_f16_transcendental_is_correctly_rounded_over_all_inputs has been running for over 60 seconds
test every_f16_transcendental_is_correctly_rounded_over_all_inputs ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 154.21s

   Compiling num-traits v0.2.19
   Compiling num-integer v0.1.46
   Compiling num-bigint v0.4.4
   Compiling piperine-numbers v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-numbers)
    Finished `release` profile [optimized + debuginfo] target(s) in 13.56s
     Running tests/transcendental.rs (/var/tmp/piperine-target-a/release/deps/transcendental-e91be4380dd5ef3c)
```

`Run quad` - exit 0

```
running 2 tests
test f128_exact_methods_follow_h2 ... ok
test f128_arithmetic_and_square_root_are_correctly_rounded ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.29s

    Finished `test` profile [unoptimized] target(s) in 0.06s
     Running tests/quad.rs (/var/tmp/piperine-target-a/debug/deps/quad-3bf936535e3a83fa)
```

`Run conversions` - exit 0

```
running 1 test
test every_as_conversion_follows_the_table_of_1_4_5 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.72s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/conversions.rs (/var/tmp/piperine-target-a/debug/deps/conversions-09076d37bf586588)
```

`Run integers` - exit 0

```
running 3 tests
test wide_integers_up_to_128_bits_agree_with_u128_and_i128 ... ok
test wide_integers_above_128_bits_agree_with_big_integers_modulo_two_to_the_n ... ok
test limbs_agree_with_big_integers_on_every_operation ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.81s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/integers.rs (/var/tmp/piperine-target-a/debug/deps/integers-41498923065a97f4)
```

`Run apiw` - exit 0

```
running 2 tests
test the_host_reads_each_width_as_one_exact_value ... ok
test a_param_of_each_width_shows_its_default_and_is_set_by_text_in_its_own_format ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.16s

    Finished `test` profile [unoptimized] target(s) in 0.28s
     Running tests/number_widths.rs (/var/tmp/piperine-target-a/debug/deps/number_widths-d2f98dace357bb94)
```

`Run clirun` - exit 0

```
test an_array_of_carriers_prints_by_element_and_whole_in_every_format ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test an_option_cell_prints_none_or_the_value ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test records_stay_when_the_analysis_fails ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test number_widths_print_exactly_in_every_format ... ok
test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.69s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target-a/debug/deps/run-0b8ee2012e535de3)
```

`Run interp` - exit 0

```
test a_digital_body_is_rejected_by_name ... ok
test a_table_expression_is_refused_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_analog_operator_by_name ... ok
test a_param_of_another_type_is_refused_naming_the_transpiled_backend ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_level_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/frozen.rs (/var/tmp/piperine-target-a/debug/deps/frozen-1d51dac525b72719)
```

`Run elabw` - exit 0

```
running 4 tests
test a_const_fn_and_an_array_length_fold_over_the_new_widths ... ok
test const_contexts_compute_every_width_as_the_run_does ... ok
test number_widths_lower_to_their_ir_types_and_constants ... ok
test a_const_panic_of_a_new_width_names_the_expression ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.32s

    Finished `test` profile [unoptimized] target(s) in 0.48s
     Running tests/elab_number_widths.rs (/var/tmp/piperine-target-a/debug/deps/elab_number_widths-6874ed078b779d35)
```

`Run purity` - exit 0

```
running 1 test
test the_numeric_crate_is_embeddable_source ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.35s
     Running tests/purity.rs (/var/tmp/piperine-target-a/debug/deps/purity-aa64dac4aea7242f)
```

`Run formats` - exit 0

```
running 2 tests
test rounding_to_every_format_is_nearest_ties_to_even ... ok
test decimal_text_reads_to_the_nearest_value_and_prints_the_shortest ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.71s

    Finished `test` profile [unoptimized] target(s) in 0.20s
     Running tests/formats.rs (/var/tmp/piperine-target-a/debug/deps/formats-99af29e828a8afb5)
```

`Run precision` - exit 0

```
running 4 tests
test ziv_rounding_raises_precision_until_the_bounds_agree ... ok
test reference_constants_contain_their_published_digits ... ok
test directed_rounding_brackets_the_exact_result ... ok
test reduction_keeps_full_precision_for_every_finite_argument ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.55s

    Finished `test` profile [unoptimized] target(s) in 0.29s
     Running tests/precision.rs (/var/tmp/piperine-target-a/debug/deps/precision-ab8c1ff7b849bf1f)
```

`Run irw` - exit 0

```
running 4 tests
test a_malformed_number_width_form_is_refused ... ok
test a_math_function_is_typed_by_its_operand ... ok
test a_design_without_number_widths_serializes_without_the_arena ... ok
test number_width_types_and_constants_are_checked_printed_and_serialized ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/number_widths.rs (/var/tmp/piperine-target-a/debug/deps/number_widths-7b43f31435419019)
```

`Run planw` - exit 0

```
running 3 tests
test a_record_field_of_a_new_scalar_is_laid_out_after_its_alignment ... ok
test each_new_scalar_has_its_leaf_count_and_bytes ... ok
test a_new_scalar_inside_a_composite_is_one_leaf_with_its_count ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.20s
     Running tests/number_widths.rs (/var/tmp/piperine-target-a/debug/deps/number_widths-1c47ebee4d33c690)
```

`Run tw` - exit 0

```
running 2 tests
test a_design_without_number_widths_gets_todays_runtime_files ... ok
test number_widths_compile_and_compute_as_their_library ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.83s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/number_widths.rs (/var/tmp/piperine-target-a/debug/deps/number_widths-79e1a125b5bb51fc)
```

`Run book` - exit 0

```
test the_sum_below_listing_runs_with_the_literal_typed_by_its_use ... ok
test e0903_mut_is_not_a_word_of_the_language ... ok
test e0909_e0905_and_e0903_a_literal_too_large_a_character_and_a_keyword_as_a_name ... ok
test the_literal_forms_have_the_values_their_comments_give ... ok
test blocks_ifs_and_matches_are_expressions_with_one_value ... ok
test a_fault_ends_the_analysis_or_the_build_and_a_non_finite_value_ends_the_solve ... ok
test constants_and_functions_compute_before_and_during_the_run ... ok
test floats_of_every_width_and_wide_integers_run ... ok
test arithmetic_keeps_the_low_bits_and_as_converts_without_failing ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.85s

    Finished `test` profile [unoptimized] target(s) in 0.20s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-4e223d7a754dd3b2)
     Running tests/book/language.rs (/var/tmp/piperine-target-a/debug/deps/book_language-336ea49cde2e9a88)
```

`Run docs` - exit 0

```
test state_records_the_unknown_semantics_decision ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test gaps_drops_what_composite_values_phase_d_closed ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test the_roadmap_is_forward_looking ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test transient_performance_closes_v12 ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.16s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run rtw` - exit 0

```
running 1 test
test rt::widths::tests::the_runtime_raises_msl_panics_and_carries_every_width ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 51 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target-a/debug/deps/piperine_transpile-bd455ffa969581ab)
```

`cargo test -p piperine-cli --test twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 50.21s

    Finished `test` profile [unoptimized] target(s) in 0.18s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-89feed4c96f5a89f)
```

`cargo build -p piperine-numbers -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperi` - exit 0

```
Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-cv-d/crates/piperine-solver)
    Finished `dev` profile [unoptimized] target(s) in 1.70s
```

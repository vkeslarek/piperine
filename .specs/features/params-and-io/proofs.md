# params-and-io proofs

**HEAD**: bec863ca
**Tree**: clean
**Shell**: /bin/sh
**Result**: 63 pass, 52 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourceparams / test an_integer_param_runs_with_its_value ... ok` | **no test ran** |
| C2 | `run elabparams / test an_integer_param_is_bound_and_read ... ok` | **no test ran** |
| C3 | `run elabparams / test n_as_f64_reaches_the_contribution ... ok` | **no test ran** |
| C4 | `run apisession / test an_integer_out_of_range_is_e0802 ... ok` | **no test ran** |
| C5 | `run clirun / test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok` | **no test ran** |
| C6 | `run bookdigital / test the_debouncer_pwm_and_pwm_check_print_their_records ... ok` | **no test ran** |
| C7 | `run sourceparams / test an_enum_param_runs_its_arm ... ok` | **no test ran** |
| C8 | `run elabparams / test an_enum_param_selects_its_arm ... ok` | **no test ran** |
| C9 | `run apisession / test an_enum_param_set_by_variant_name ... ok` | **no test ran** |
| C10 | `run apisession / test an_unknown_variant_is_e0802_listing_the_variants ... ok` | **no test ran** |
| C11 | `run elabparams / test an_option_param_left_out_is_none ... ok` | **no test ran** |
| C12 | `run elabparams / test an_option_param_bound_to_a_value_is_some ... ok` | **no test ran** |
| C13 | `run elabparams / test option_param_methods_read_the_bound_case ... ok` | **no test ran** |
| C14 | `run apisession / test an_option_param_set_to_a_value_or_none ... ok` | **no test ran** |
| C15 | `run elabinstances / test a_placement_through_a_module_param_places_its_template ... ok` | pass |
| C16 | `run elabinstances / test a_parent_binding_replaces_the_template ... ok` | pass |
| C17 | `run typesmodules / test a_param_block_on_a_placement_through_a_param_is_e0361 ... ok` | pass |
| C18 | `run elabinstances / test a_template_leaving_a_required_param_unbound_is_e0319 ... ok` | pass |
| C19 | `run typesmodules / test a_template_of_another_module_is_e0180_naming_both ... ok` | pass |
| C20 | `run elabinstances / test the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module ... ok` | pass |
| C21 | `run interp / test a_param_of_another_type_is_refused_naming_the_transpiled_backend ... ok` | **no test ran** |
| C22 | `run typesvalues / test a_mismatch_no_row_names_is_e0180_naming_both_types ... ok` | pass |
| C23 | `run speccodes / test no_source_spells_p0002 ... ok` | pass |
| C24 | `run typesvalues / test a_param_hiding_a_function_is_e0180_with_a_note_at_the_function ... ok` | pass |
| C25 | `run clibook / test no_book_listing_prints_p0002 ... ok` | pass |
| C26 | `run typesvalues / test an_as_between_types_without_a_row_is_e0182 ... ok` | pass |
| C27 | `run typesvalues / test a_call_through_a_function_value_with_the_wrong_arity_is_e0141 ... ok` | pass |
| C28 | `run elabvalues / test a_const_reading_a_run_value_is_e0188 ... ok` | pass |
| C29 | `run apisources / test a_dc_source_sets_the_port_at_the_operating_point ... ok` | pass |
| C30 | `run apisources / test a_pwl_source_lands_on_every_corner ... ok` | pass |
| C31 | `run sourcehost / test a_host_driven_rc_matches_ac_rc_in_ac ... ok` | pass |
| C32 | `run apisources / test a_timed_source_change_is_a_jump ... ok` | pass |
| C33 | `run apisources / test an_undriven_node_port_is_e0810 ... ok` | pass |
| C34 | `run elabentry / test a_top_value_input_is_e0811 ... ok` | pass |
| C35 | `run elabentry / test a_top_value_input_is_e0811 ... ok` | pass |
| C36 | `run spectext / test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok` | pass |
| C37 | `run elabfiles / test include_str_reads_the_file_beside_the_source ... ok` | pass |
| C38 | `run typesvalues / test include_str_outside_a_const_is_e0610 ... ok` | pass |
| C39 | `run elabfiles / test an_unreadable_file_is_e0612_naming_the_path ... ok` | pass |
| C40 | `run elabfiles / test a_file_that_is_not_utf8_is_e0613_with_the_offset ... ok` | pass |
| C41 | `run elabfiles / test a_changed_file_needs_a_new_elaboration ... ok` | pass |
| C42 | `run elabfiles / test parse_table_reads_64_rows_in_order ... ok` | **no test ran** |
| C43 | `run elabfiles / test parse_hex_reads_words_at_their_addresses ... ok` | **no test ran** |
| C44 | `run elabfiles / test a_table_of_the_wrong_shape_is_e0614_naming_file_and_row ... ok` | **no test ran** |
| C45 | `run elabfiles / test a_token_that_is_not_a_real_is_e0706 ... ok` | **no test ran** |
| C46 | `run sourcetables / test linear_table_model_is_the_interpolation_at_op_and_dc_sweep ... ok` | **fail** |
| C47 | `run sourcetables / test discrete_control_gives_the_closest_sample ... ok` | **fail** |
| C48 | `run sourcetables / test cubic_reproduces_a_cubic_polynomial ... ok` | **fail** |
| C49 | `run sourcetables / test extrapolation_c_and_l_at_each_end ... ok` | **fail** |
| C50 | `run sourcetables / test extrapolation_e_outside_is_e0705 ... ok` | **fail** |
| C51 | `run elabfiles / test a_bad_control_string_is_e0703 ... ok` | **no test ran** |
| C52 | `run elabfiles / test non_increasing_abscissae_are_e0704 ... ok` | **no test ran** |
| C53 | `run sourcetables / test the_table_diode_passes_the_partials_verifier ... ok` | **fail** |
| C54 | `run spectext / test appendix_c2_states_the_spline_end_conditions ... ok` | pass |
| C55 | `run clicommands / test fmt_check_passes_on_every_stdlib_file ... ok` | pass |
| C56 | `run clicommands / test fmt_check_passes_in_examples_buck ... ok` | pass |
| C57 | `run stdgalleries / test every_gallery_fence_is_in_the_formatters_form ... ok` | pass |
| C58 | `run msformat / test formatting_keeps_the_printed_tree_of_every_shipped_source ... ok` | pass |
| C59 | `run typesvalues / test a_borrow_outside_a_call_argument_is_e0102 ... ok` | **no test ran** |
| C60 | `run runerrors / test a_division_by_zero_under_known_control_fails_the_run_with_e0105 ... ok` | pass |
| C61 | `run typesvalues / test a_named_emitting_fn_where_emit_is_not_allowed_is_e0122 ... ok` | **no test ran** |
| C62 | `run elabanalog / test a_quantity_attribute_outside_the_three_is_e0204 ... ok` | pass |
| C63 | `run elabanalog / test equal_access_names_are_e0205 ... ok` | pass |
| C64 | `run typesmodules / test an_expression_on_a_carrier_port_that_is_not_in_is_e0315 ... ok` | pass |
| C65 | `run typesmodules / test an_instance_without_a_result_used_as_a_value_is_e0333 ... ok` | pass |
| C66 | `run typesstructure / test an_attribute_a1_does_not_list_is_e0904 ... ok` | pass |
| C67 | `run speccodes / test this_features_codes_are_all_raised ... ok` | **no test ran** |
| C68 | `run typescarriers / test a_probe_on_a_childs_private_node_is_e0244 ... ok` | pass |
| C69 | `run typesresults / test a_continuous_result_read_where_probes_are_not_allowed_is_e0354 ... ok` | pass |
| C70 | `run typesunknowns / test unknown_with_no_inferable_type_is_e0906 ... ok` | pass |
| C71 | `run speccodes / test every_spec_code_is_raised_or_owned ... ok` | pass |
| C72 | `run speccodes / test every_owned_code_names_its_owner_and_construct ... ok` | pass |
| C73 | `run speccodes / test no_owned_code_is_raised ... ok` | pass |
| C74 | `run speccodes / test this_features_codes_are_all_raised ... ok` | **no test ran** |
| C75 | `run docs / test gaps_no_longer_lists_this_features_ids ... ok` | **no test ran** |
| C76 | `run elabparams / test an_integer_default_converts_with_as ... ok` | **no test ran** |
| C77 | `run typesmodules / test an_option_param_read_as_a_number_is_e0180 ... ok` | **no test ran** |
| C78 | `run apisources / test a_node_port_tied_to_gnd_inside_is_still_refused_undriven ... ok` | pass |
| C79 | `run elabfiles / test one_file_named_twice_is_read_once ... ok` | pass |
| C80 | `run elabfiles / test an_infinite_row_value_is_e0706 ... ok` | **no test ran** |
| C81 | `run elabfiles / test a_one_row_table_under_d_is_e0704 ... ok` | **no test ran** |
| C82 | `run elabvalues / test a_const_that_depends_on_itself_is_e0188 ... ok` | pass |
| C83 | `run sourcehost / test a_host_driven_rc_step_matches_tran_rc ... ok` | pass |
| C84 | `run sourcehost / test a_host_pwl_into_the_rc_matches_tran_pwl_rc ... ok` | pass |
| C85 | `run clirun / test run_of_a_top_with_a_node_port_is_e0810 ... ok` | pass |
| C86 | `run elabanalog / test a_non_positive_abstol_is_e0204 ... ok` | pass |
| C87 | `run typesstructure / test a_listed_attribute_on_the_wrong_item_is_e0904 ... ok` | pass |
| C88 | `run typesresults / test an_event_result_outside_an_event_position_is_e0354 ... ok` | pass |
| C89 | `run typesvalues / test a_named_solve_reading_fn_in_a_const_is_e0122 ... ok` | **no test ran** |
| C90 | `run runerrors / test an_unwrap_of_none_under_known_control_fails_the_run_with_e0105 ... ok` | pass |
| C91 | `run elabentry / test a_top_node_port_gets_one_host_source_instance ... ok` | pass |
| C92 | `run elabentry / test the_pom_keeps_a_top_with_node_ports_as_written ... ok` | pass |
| C93 | `run typesmodules / test a_module_typed_param_takes_a_template ... ok` | pass |
| C94 | `run apisession / test a_module_typed_param_is_not_set_by_text ... ok` | pass |
| C95 | `run typesvalues / test include_str_in_a_const_is_a_str ... ok` | pass |
| C96 | `run msformat / test every_stdlib_file_is_in_the_formatters_form ... ok` | pass |
| C97 | `run docs / test gaps_no_longer_lists_x5_l20_l39_h12 ... ok` | pass |
| C98 | `run typesmodules / test integer_enum_and_option_params_type_check ... ok` | **no test ran** |
| C99 | `run sourceparams / test an_option_param_runs_its_case ... ok` | **no test ran** |
| C100 | `run apisession / test an_integer_param_set_by_text_in_range ... ok` | **no test ran** |
| C101 | `run bookcorestd / test the_watchdog_builds_with_its_u16_param ... ok` | **no test ran** |
| C102 | `run bookinventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C103 | `run elabfiles / test x_and_z_digits_are_unknown_levels ... ok` | **no test ran** |
| C104 | `run elabfiles / test an_address_the_text_does_not_write_is_e0614 ... ok` | **no test ran** |
| C105 | `run elabfiles / test a_word_that_is_not_hex_is_e0706 ... ok` | **no test ran** |
| C106 | `run elabfiles / test too_few_points_for_the_degree_is_e0704 ... ok` | **no test ran** |
| C107 | `run irchecks / test a_table_expression_is_built_checked_and_printed ... ok` | **no test ran** |
| C108 | `run interp / test a_table_expression_is_refused_by_name ... ok` | **no test ran** |
| C109 | `run bookpackages / test the_include_str_table_runs ... ok` | **no test ran** |
| C110 | `run bookpackages / test e0610_is_raised_for_include_str_in_a_module ... ok` | **no test ran** |
| C111 | `run bookcheatsheet / test the_files_card_runs ... ok` | **no test ran** |
| C112 | `run docs / test gaps_section_10_lists_exactly_the_owned_codes ... ok` | **no test ran** |
| Run sourcetables | `cargo test -p piperine-parity --test source_tables` | **fail** - exit 101 |
| Build | `cargo build -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourceparams` - exit 0

```
running 3 tests
test a_body_let_as_a_param_value_sets_the_corner ... ok
test a_node_initial_value_over_a_param_starts_newton_in_an_operating_point ... ok
test a_node_initial_value_over_a_param_holds_the_operating_point ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.08s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_params.rs (/var/tmp/piperine-target-b/debug/deps/source_params-a40c966bba2b4980)
```

`Run elabparams` - exit 0

```
test an_environment_function_is_dynamic ... ok
test a_param_read_with_no_binding_is_not_constant ... ok
test an_unset_param_takes_its_default ... ok
test division_by_zero_is_inf_not_an_error ... ok
test a_param_read_is_never_folded ... ok
test a_const_read_is_folded_once ... ok
test functions_methods_and_logic_fold ... ok
test a_false_where_is_e0317_with_the_path ... ok
test a_required_param_left_out_is_e0319 ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.54s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.60s
     Running tests/elab_params.rs (/var/tmp/piperine-target-b/debug/deps/elab_params-89d1e9c8fef61684)
```

`Run apisession` - exit 0

```
running 9 tests
test the_interpreter_refusal_names_the_body_and_the_rust_backend ... ok
test the_top_key_names_the_module ... ok
test a_top_with_a_node_port_is_refused_by_the_interpreter_naming_the_host_source ... ok
test the_one_portless_module_is_the_top ... ok
test a_missing_rustc_names_rustc_and_the_interpreter ... ok
test no_top_or_several_is_p0011_with_candidates ... ok
test a_module_typed_param_is_not_set_by_text ... ok
test a_run_compiles_once_and_never_during_an_analysis ... ok
test a_top_with_discrete_in_ports_runs_with_them_unknown ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.78s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/session.rs (/var/tmp/piperine-target-b/debug/deps/session-79e63c113c6b08bd)
```

`Run clirun` - exit 0

```
test records_come_before_the_table ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test run_prints_header_records_blank_line_and_table ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test records_stay_when_the_analysis_fails ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test a_first_compile_is_announced_on_stderr ... ok
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.93s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/run.rs (/var/tmp/piperine-target-b/debug/deps/run-3deb22af66054c5e)
```

`Run bookdigital` - exit 0

```
test e0430_a_bit_is_a_level_not_an_event ... ok
test the_check_passes_every_cycle_and_ends_the_run_at_the_twentieth ... ok
test a_press_released_before_the_fourth_sample_gives_no_flip ... ok
test the_debouncer_is_refused_for_its_integer_param_alone ... ok
test the_debouncer_flips_once_at_the_fourth_clean_sample ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.92s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/digital.rs (/var/tmp/piperine-target-b/debug/deps/book_digital-9dc58690984a778f)
```

`Run elabinstances` - exit 0

```
test a_placement_through_a_module_param_places_its_template ... ok
test the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module ... ok
test a_parent_binding_replaces_the_template ... ok
test an_unnamed_placement_is_module_hash_k ... ok
test instances_come_in_pre_order ... ok
test electrical_is_lowered_once_with_its_tolerances ... ok
test one_body_per_module_one_instance_per_placement ... ok
test paths_start_at_the_top_module ... ok
test signals_are_nodes_and_ports_bind_the_parents ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.51s
     Running tests/elab_instances.rs (/var/tmp/piperine-target-b/debug/deps/elab_instances-c286917d2cae5fdd)
```

`Run typesmodules` - exit 0

```
test a_where_that_is_not_bool_is_e0318 ... ok
test an_instance_without_a_result_used_as_a_value_is_e0333 ... ok
test an_expression_on_a_carrier_port_that_is_not_in_is_e0315 ... ok
test ports_and_signals_are_nodes_and_gnd_fits_any ... ok
test a_where_on_a_module_or_fn_is_bool ... ok
test params_are_f64_or_bool ... ok
test a_placement_checks_nodes_count_and_params ... ok
test an_environment_function_in_a_const_context_is_e0323 ... ok
test a_core_primitive_is_checked_where_it_is_written ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.50s
     Running tests/types_modules.rs (/var/tmp/piperine-target-b/debug/deps/types_modules-e42f34873c4ac40f)
```

`Run interp` - exit 0

```
test a_digital_body_is_rejected_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_unknown_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 13.84s
     Running tests/frozen.rs (/var/tmp/piperine-target-b/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run typesvalues` - exit 0

```
test catalogue_methods_on_f64_are_f64 ... ok
test every_h2_row_types_with_its_signature ... ok
test include_str_outside_a_const_is_e0610 ... ok
test diagnostics_come_in_source_order_one_per_expression ... ok
test comparing_an_enum_without_partial_ord_is_e0180 ... ok
test operators_type_as_rust_types_them ... ok
test a_call_through_a_function_value_with_the_wrong_arity_is_e0141 ... ok
test the_std_pnjlim_checks_clean ... ok
test include_str_in_a_const_is_a_str ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.66s
     Running tests/types_values.rs (/var/tmp/piperine-target-b/debug/deps/types_values-65c645751b306c9f)
```

`Run speccodes` - exit 0

```
running 4 tests
test every_owned_code_names_its_owner_and_construct ... ok
test no_owned_code_is_raised ... ok
test no_source_spells_p0002 ... ok
test every_spec_code_is_raised_or_owned ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/spec_codes.rs (/var/tmp/piperine-target-b/debug/deps/spec_codes-86724e06e4c5ed72)
```

`Run clibook` - exit 0

```
test the_clipper_runs_as_the_book_prints ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test the_buck_block_is_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 173.30s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-a1f18a8423c7ed6b)
```

`Run elabvalues` - exit 0

```
test a_const_that_depends_on_itself_is_e0188 ... ok
test option_rows_lower_on_the_pair ... ok
test a_carrier_read_is_flagged_and_the_flag_reads_are_not ... ok
test a_function_gets_one_variant_per_flag_pattern ... ok
test an_enum_is_its_tag_and_a_match_is_an_if_chain ... ok
test an_option_is_a_flag_and_a_payload ... ok
test a_constant_division_by_zero_is_e0104 ... ok
test numeric_rows_lower_to_methods_and_checked_ops_to_pairs ... ok
test a_partial_right_operand_under_a_flagged_short_circuit_is_an_if ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.86s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.84s
     Running tests/elab_values.rs (/var/tmp/piperine-target-b/debug/deps/elab_values-044391ce0c7e05d6)
```

`Run apisources` - exit 0

```
running 6 tests
test a_node_port_tied_to_gnd_inside_is_still_refused_undriven ... ok
test a_pwl_source_lands_on_every_corner ... ok
test a_dc_source_sets_the_port_at_the_operating_point ... ok
test corners_that_do_not_increase_are_e0802_and_need_a_transient ... ok
test an_undriven_node_port_is_e0810 ... ok
test a_timed_source_change_is_a_jump ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.17s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/host_sources.rs (/var/tmp/piperine-target-b/debug/deps/host_sources-846e2e82a8773a49)
```

`Run sourcehost` - exit 0

```
running 3 tests
test a_host_driven_rc_step_matches_tran_rc ... ok
test a_host_driven_rc_matches_ac_rc_in_ac ... ok
test a_host_pwl_into_the_rc_matches_tran_pwl_rc ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.76s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_host.rs (/var/tmp/piperine-target-b/debug/deps/source_host-480988613ba19c2b)
```

`Run elabentry` - exit 0

```
test a_top_value_input_is_e0811 ... ok
test device_bodies_match_the_ir_goldens ... ok
test front_end_diagnostics_are_returned_without_an_ir ... ok
test a_top_node_port_gets_one_host_source_instance ... ok
test every_diagnostic_has_code_span_and_path ... ok
test the_design_is_unchanged_by_elaboration ... ok
test the_twins_elaborate_without_diagnostics ... ok
test elaborating_twice_prints_the_same_text ... ok
test a_twin_elaborates_to_a_checked_ir ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.92s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.28s
     Running tests/elab_entry.rs (/var/tmp/piperine-target-b/debug/deps/elab_entry-0a4be7f7e45bf048)
```

`Run spectext` - exit 0

```
test gnd_and_the_implicit_params_are_core_names ... ok
test s3_states_the_intrinsic_contract ... ok
test time_point_runs_the_procedure ... ok
test tran_in_fences_is_the_module ... ok
test the_commit_of_an_event_read_as_a_bool_is_stated ... ok
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test zero_bytes_are_the_unknown_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok

test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/spec_text.rs (/var/tmp/piperine-target-b/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run elabfiles` - exit 0

```
running 5 tests
test a_file_that_is_not_utf8_is_e0613_with_the_offset ... ok
test a_changed_file_needs_a_new_elaboration ... ok
test one_file_named_twice_is_read_once ... ok
test include_str_reads_the_file_beside_the_source ... ok
test an_unreadable_file_is_e0612_naming_the_path ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.51s
     Running tests/elab_files.rs (/var/tmp/piperine-target-b/debug/deps/elab_files-78593e794374e4b0)
```

`Run sourcetables` - exit 101

```
    project
    run_errors
    source_catalogue
    source_derivative
    source_environment
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_params
    source_reach
    source_switch
    source_unknowns
    twins
```

`Run clicommands` - exit 0

```
test build_prints_one_line_per_run ... ok
test a_second_build_is_cached_and_compiles_nothing ... ok
test test_prints_pass_with_the_check_count ... ok
test test_prints_no_table_and_no_other_record ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_fail_and_the_failing_records ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test clean_cache_removes_the_compiled_cache ... ok
test verbose_prints_a_source_path_that_exists ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.18s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/commands.rs (/var/tmp/piperine-target-b/debug/deps/commands-e62622f9813395ac)
```

`Run stdgalleries` - exit 0

```
test the_fixture_table_covers_every_gallery ... ok
test galleries_fail_against_an_empty_prelude ... ok
test every_gallery_fence_is_in_the_formatters_form ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test every_gallery_references_its_std_items ... ok
test the_gate_builds_18_fences ... ok
test every_gallery_passes_check ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test no_gallery_has_an_unresolved_member ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.73s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.55s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-b/debug/deps/std_galleries-20eb711d5dacf84a)
```

`Run msformat` - exit 0

```
test every_plain_comment_comes_out_once_in_order_unchanged ... ok
test the_guard_refuses_a_text_that_lost_a_comment ... ok
test the_formatted_tree_equals_the_sources ... ok
test formatting_twice_changes_nothing ... ok
test every_stdlib_file_is_in_the_formatters_form ... ok
test formatting_keeps_the_printed_tree_of_every_shipped_source ... ok
test the_corpus_keeps_every_comment ... ok
test the_corpus_keeps_every_tree ... ok
test the_corpus_is_stable_under_a_second_format ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.71s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.50s
     Running tests/msl_format.rs (/var/tmp/piperine-target-b/debug/deps/msl_format-c16dd70a423cc656)
```

`Run runerrors` - exit 0

```
running 5 tests
test a_floating_node_from_source_is_e0503 ... ok
test a_loop_of_two_vsources_from_source_is_e0503 ... ok
test a_nan_contribution_from_source_is_e0507 ... ok
test an_unwrap_of_none_under_known_control_fails_the_run_with_e0105 ... ok
test a_division_by_zero_under_known_control_fails_the_run_with_e0105 ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.73s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/run_errors.rs (/var/tmp/piperine-target-b/debug/deps/run_errors-d0ffd0a2802d9e56)
```

`Run elabanalog` - exit 0

```
test freq_analysis_and_bound_step_lower ... ok
test probes_and_environment_lower_to_their_nodes ... ok
test the_device_bodies_equal_the_structural_goldens ... ok
test sites_are_numbered_in_source_order_and_named ... ok
test value_code_lowers_to_the_same_forms_and_functions_once ... ok
test two_analog_blocks_keep_their_locals_apart ... ok
test bound_step_under_an_if_is_p0001 ... ok
test a_ddt_off_the_top_of_a_flow_contribution_is_refused ... ok
test a_quantity_attribute_outside_the_three_is_e0204 ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.55s
     Running tests/elab_analog.rs (/var/tmp/piperine-target-b/debug/deps/elab_analog-52fe00f5ce65d9cd)
```

`Run typesstructure` - exit 0

```
test inst_name_of_an_item_that_is_not_pub_is_e0343 ... ok
test inst_name_types_as_the_pub_item ... ok
test feedback_through_an_unknown_of_a_module_is_accepted ... ok
test an_attribute_a1_does_not_list_is_e0904 ... ok
test a_listed_attribute_on_the_wrong_item_is_e0904 ... ok
test a_continuous_argument_read_by_a_callee_definition_is_e0336 ... ok
test a_cycle_with_a_direct_dependence_at_every_result_is_e0353 ... ok
test a_discrete_argument_read_by_a_callee_definition_is_accepted ... ok
test an_initial_value_that_reads_a_carrier_is_e0210 ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.33s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.56s
     Running tests/types_structure.rs (/var/tmp/piperine-target-b/debug/deps/types_structure-78e56499b09a151b)
```

`Run typescarriers` - exit 0

```
test a_carrier_of_another_value_type_is_e0251 ... ok
test an_initial_value_without_the_drive_is_e0223 ... ok
test prev_is_typed_on_a_carrier ... ok
test two_definitions_of_one_carrier_are_e0335 ... ok
test an_out_port_takes_only_a_carrier_the_parent_hands_over ... ok
test a_target_the_module_may_not_drive_is_e0222 ... ok
test an_initial_value_of_the_wrong_kind_is_e0210 ... ok
test the_flag_reads_are_typed ... ok
test a_continuous_definition_is_e0336 ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.56s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.62s
     Running tests/types_carriers.rs (/var/tmp/piperine-target-b/debug/deps/types_carriers-e1157a56d14a4b3b)
```

`Run typesresults` - exit 0

```
test a_continuous_result_read_where_probes_are_not_allowed_is_e0354 ... ok
test a_placement_has_its_result_type ... ok
test an_event_result_outside_an_event_position_is_e0354 ... ok
test a_placement_in_a_const_context_is_e0352 ... ok
test a_let_that_reads_itself_is_e0332_or_e0353 ... ok
test trailing_option_inputs_may_be_left_out ... ok
test a_module_with_a_result_is_typed ... ok
test now_and_analysis_static_are_typed ... ok
test an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150 ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.49s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.55s
     Running tests/types_results.rs (/var/tmp/piperine-target-b/debug/deps/types_results-debe08770933098b)
```

`Run typesunknowns` - exit 0

```
test the_intrinsics_are_typed_at_the_concrete_type ... ok
test cast_between_widths_is_e0701 ... ok
test width_is_typed_u32_on_a_concrete_type ... ok
test level_is_typed_on_every_repr_type ... ok
test a_project_function_named_like_an_intrinsic_is_an_ordinary_function ... ok
test unknown_with_no_inferable_type_is_e0906 ... ok
test a_bad_pattern_is_e0702 ... ok
test a_function_that_reads_its_parameter_bare_is_not_flag_total ... ok
test a_flag_total_function_may_take_a_carrier_in_analog ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.49s
     Running tests/types_unknowns.rs (/var/tmp/piperine-target-b/debug/deps/types_unknowns-ab8a4e7b58f8b925)
```

`Run docs` - exit 0

```
test state_records_the_cli_decision ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test state_records_the_derived_param_decision ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test state_records_the_unknown_semantics_decision ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test the_roadmap_and_state_record_analog_structure ... ok
test the_roadmap_and_state_record_root_e2e ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-93bbd657b5852db6)
```

`Run bookcorestd` - exit 0

```
test the_probe_writes_a_check_and_a_log_record ... ok
test e0161_the_inherited_library_is_a_closed_list ... ok
test the_library_declares_the_noise_sources_and_the_monitors_only_report ... ok
test the_dac_ramp_scales_its_code_by_the_reference ... ok
test the_pad_drives_its_node_and_reads_it_back ... ok
test a_crates_own_vt_replaces_the_library_one ... ok
test the_probes_have_the_short_names_the_nature_gives ... ok
test the_tour_module_runs_its_analog_and_digital_blocks ... ok
test the_watchdog_ends_the_run_at_the_fourth_edge_with_the_button_held ... ok
test the_latch_takes_its_input_at_the_rising_edge_or_zero_when_unknown ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.03s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/core_std.rs (/var/tmp/piperine-target-b/debug/deps/book_core_std-a129f930e05c2854)
```

`Run bookinventory` - exit 0

```
test the_owner_and_disputed_rules_name_what_they_break ... ok
test every_block_of_the_book_has_one_row ... ok
test a_model_across_blocks_names_one_test ... ok
test every_gaps_row_names_its_owner ... ok
test a_verdict_fits_its_block ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-b/debug/deps/book_inventory-fa5abaa513c18484)
```

`Run irchecks` - exit 0

```
test a_top_alone_prints_and_checks ... ok
test a_recursive_call_or_an_unbound_local_is_named ... ok
test a_cyclic_expression_is_named_without_hanging ... ok
test an_instance_with_wrong_arity_is_named ... ok
test a_duplicated_path_is_named ... ok
test a_wrong_type_in_an_analog_position_is_named ... ok
test an_out_of_range_id_is_named ... ok
test a_misplaced_or_duplicated_site_is_named ... ok
test a_built_ir_checks_clean ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-ir)
    Finished `test` profile [unoptimized] target(s) in 3.19s
     Running tests/check.rs (/var/tmp/piperine-target-b/debug/deps/check-accb0dc311a55cae)
```

`Run bookpackages` - exit 0

```
test e0610_include_str_in_a_module_is_refused ... ok
test e0162_one_name_in_one_namespace_is_declared_once ... ok
test a_crate_reaches_another_only_through_its_manifest ... ok
test e0401_a_behavior_block_lives_in_the_file_of_its_module ... ok
test the_include_str_table_is_refused_for_its_generic_function ... ok
test e0161_a_private_item_and_a_module_file_that_is_not_there_or_is_there_twice ... ok
test a_function_named_like_a_dependency_does_not_clash_with_it ... ok
test the_crate_root_of_the_bigger_project_loads_its_files_and_reaches_its_dependency ... ok
test a_local_namespace_and_a_dependency_of_one_name_make_the_use_ambiguous ... ok
test the_lowpass_project_runs_as_the_book_assembles_it ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/packages.rs (/var/tmp/piperine-target-b/debug/deps/book_packages-edf02e018ff89bf3)
```

`Run bookcheatsheet` - exit 0

```
test the_extern_card_is_refused ... ok
test the_template_card_is_refused_for_its_generic_module_and_its_placements ... ok
test the_module_card_is_refused_for_its_const_generic ... ok
test the_body_items_card_is_refused_for_its_generation ... ok
test the_debug_prints_card_is_refused_for_the_analog_trace ... ok
test the_files_card_is_refused_for_its_table_and_its_statement ... ok
test the_nature_card_is_the_standard_librarys ... ok
test the_analog_card_runs ... ok
test the_digital_card_runs ... ok
test a_name_of_yours_hides_the_library_but_not_the_core ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.03s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/cheatsheet.rs (/var/tmp/piperine-target-b/debug/deps/book_cheatsheet-4fb8d98fae564693)
```

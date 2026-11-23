# params-and-io proofs

**HEAD**: e5a515a8
**Tree**: clean
**Shell**: /bin/sh
**Result**: 114 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourceparams / test an_integer_param_runs_with_its_value ... ok` | pass |
| C2 | `run elabparams / test an_integer_param_is_bound_and_read ... ok` | pass |
| C3 | `run elabparams / test n_as_f64_reaches_the_contribution ... ok` | pass |
| C4 | `run apisession / test a_param_flag_is_read_by_the_params_type_before_the_run ... ok` | pass |
| C5 | `run clirun / test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok` | pass |
| C6 | `run bookdigital / test the_debouncer_pwm_and_pwm_check_print_their_records ... ok` | pass |
| C7 | `run sourceparams / test an_enum_param_runs_its_arm ... ok` | pass |
| C8 | `run elabparams / test an_enum_param_selects_its_arm ... ok` | pass |
| C9 | `run apisession / test an_enum_param_set_by_variant_name ... ok` | pass |
| C10 | `run apisession / test an_unknown_variant_is_e0802_listing_the_variants ... ok` | pass |
| C11 | `run elabparams / test an_option_param_left_out_is_none ... ok` | pass |
| C12 | `run elabparams / test an_option_param_bound_to_a_value_is_some ... ok` | pass |
| C13 | `run sourceparams / test every_read_of_an_option_param_gives_the_bound_case ... ok` | pass |
| C14 | `run apisession / test an_option_param_set_to_a_value_or_none ... ok` | pass |
| C15 | `run elabinstances / test a_placement_through_a_module_param_places_its_template ... ok` | pass |
| C16 | `run elabinstances / test a_parent_binding_replaces_the_template ... ok` | pass |
| C17 | `run typesmodules / test a_param_block_on_a_placement_through_a_param_is_e0361 ... ok` | pass |
| C18 | `run elabinstances / test a_template_leaving_a_required_param_unbound_is_e0319 ... ok` | pass |
| C19 | `run typesmodules / test a_template_of_another_module_is_e0180_naming_both ... ok` | pass |
| C20 | `run elabinstances / test the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module ... ok` | pass |
| C21 | `run interp / test a_param_of_another_type_is_refused_naming_the_transpiled_backend ... ok` | pass |
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
| C42 | `run elabfiles / test parse_table_reads_64_rows_in_order ... ok` | pass |
| C43 | `run elabfiles / test parse_hex_reads_words_at_their_addresses ... ok` | pass |
| C44 | `run elabfiles / test a_table_of_the_wrong_shape_is_e0614_naming_file_and_row ... ok` | pass |
| C45 | `run elabfiles / test a_token_that_is_not_a_real_is_e0706 ... ok` | pass |
| C46 | `run sourcetables / test linear_table_model_is_the_interpolation_at_op_and_dc_sweep ... ok` | pass |
| C47 | `run sourcetables / test discrete_control_gives_the_closest_sample ... ok` | pass |
| C48 | `run sourcetables / test cubic_reproduces_a_cubic_polynomial ... ok` | pass |
| C49 | `run sourcetables / test extrapolation_c_and_l_at_each_end ... ok` | pass |
| C50 | `run sourcetables / test extrapolation_e_outside_is_e0705 ... ok` | pass |
| C51 | `run elabfiles / test a_bad_control_string_is_e0703 ... ok` | pass |
| C52 | `run elabfiles / test non_increasing_abscissae_are_e0704 ... ok` | pass |
| C53 | `run sourcetables / test the_table_diode_passes_the_partials_verifier ... ok` | pass |
| C54 | `run spectext / test appendix_c2_states_the_spline_end_conditions ... ok` | pass |
| C55 | `run clicommands / test fmt_check_passes_on_every_stdlib_file ... ok` | pass |
| C56 | `run clicommands / test fmt_check_passes_in_examples_buck ... ok` | pass |
| C57 | `run stdgalleries / test every_gallery_fence_is_in_the_formatters_form ... ok` | pass |
| C58 | `run msformat / test formatting_keeps_the_printed_tree_of_every_shipped_source ... ok` | pass |
| C59 | `run typesvalues / test a_borrow_outside_a_call_argument_is_not_e0102 ... ok` | pass |
| C60 | `run runerrors / test a_division_by_zero_under_known_control_fails_the_run_with_e0105 ... ok` | pass |
| C61 | `run typesvalues / test a_named_emitting_fn_where_emit_is_not_allowed_is_e0122 ... ok` | pass |
| C62 | `run elabanalog / test a_quantity_attribute_outside_the_three_is_e0204 ... ok` | pass |
| C63 | `run elabanalog / test equal_access_names_are_e0205 ... ok` | pass |
| C64 | `run typesmodules / test an_expression_on_a_carrier_port_that_is_not_in_is_e0315 ... ok` | pass |
| C65 | `run typesmodules / test an_instance_without_a_result_used_as_a_value_is_e0333 ... ok` | pass |
| C66 | `run typesstructure / test an_attribute_a1_does_not_list_is_e0904 ... ok` | pass |
| C67 | `run speccodes / test this_features_codes_are_all_raised ... ok` | pass |
| C68 | `run typescarriers / test a_probe_on_a_childs_private_node_is_e0244 ... ok` | pass |
| C69 | `run typesresults / test a_continuous_result_read_where_probes_are_not_allowed_is_e0354 ... ok` | pass |
| C70 | `run hygiene / test retired_unknown_codes_are_raised_nowhere ... ok` | pass |
| C71 | `run speccodes / test every_spec_code_is_raised_or_owned ... ok` | pass |
| C72 | `run speccodes / test every_owned_code_names_its_owner_and_construct ... ok` | pass |
| C73 | `run speccodes / test no_owned_code_is_raised ... ok` | pass |
| C74 | `run speccodes / test this_features_codes_are_all_raised ... ok` | pass |
| C75 | `run docs / test gaps_no_longer_lists_this_features_ids ... ok` | pass |
| C76 | `run elabparams / test an_integer_default_converts_with_as ... ok` | pass |
| C77 | `run typesmodules / test an_option_param_read_as_a_number_is_e0180 ... ok` | pass |
| C78 | `run apisources / test a_node_port_tied_to_gnd_inside_is_still_refused_undriven ... ok` | pass |
| C79 | `run elabfiles / test one_file_named_twice_is_read_once ... ok` | pass |
| C80 | `run elabfiles / test an_infinite_row_value_is_e0706 ... ok` | pass |
| C81 | `run elabfiles / test a_one_row_table_under_d_is_e0704 ... ok` | pass |
| C82 | `run elabvalues / test a_const_that_depends_on_itself_is_e0188 ... ok` | pass |
| C83 | `run sourcehost / test a_host_driven_rc_step_matches_tran_rc ... ok` | pass |
| C84 | `run sourcehost / test a_host_pwl_into_the_rc_matches_tran_pwl_rc ... ok` | pass |
| C85 | `run clirun / test run_of_a_top_with_a_node_port_is_e0810 ... ok` | pass |
| C86 | `run elabanalog / test a_non_positive_abstol_is_e0204 ... ok` | pass |
| C87 | `run typesstructure / test a_listed_attribute_on_the_wrong_item_is_e0904 ... ok` | pass |
| C88 | `run typesresults / test an_event_result_outside_an_event_position_is_e0354 ... ok` | pass |
| C89 | `run typesvalues / test a_named_solve_reading_fn_in_a_const_is_e0122 ... ok` | pass |
| C90 | `run runerrors / test an_unwrap_of_none_under_known_control_fails_the_run_with_e0105 ... ok` | pass |
| C91 | `run elabentry / test a_top_node_port_gets_one_host_source_instance ... ok` | pass |
| C92 | `run elabentry / test the_pom_keeps_a_top_with_node_ports_as_written ... ok` | pass |
| C93 | `run typesmodules / test a_module_typed_param_takes_a_template ... ok` | pass |
| C94 | `run apisession / test a_module_typed_param_is_not_set_by_text ... ok` | pass |
| C95 | `run typesvalues / test include_str_in_a_const_is_a_str ... ok` | pass |
| C96 | `run msformat / test every_stdlib_file_is_in_the_formatters_form ... ok` | pass |
| C97 | `run docs / test gaps_no_longer_lists_x5_l20_l39_h12 ... ok` | pass |
| C98 | `run typesmodules / test integer_enum_and_option_params_type_check ... ok` | pass |
| C99 | `run sourceparams / test an_option_param_runs_its_case ... ok` | pass |
| C100 | `run apisession / test an_integer_param_set_by_text_in_range ... ok` | pass |
| C101 | `run bookcorestd / test the_watchdog_builds_with_its_u16_param ... ok` | pass |
| C102 | `run bookinventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C103 | `run elabfiles / test x_and_z_digits_are_e0706_integers_hold_no_unknown ... ok` | pass |
| C104 | `run elabfiles / test an_address_the_text_does_not_write_is_e0614 ... ok` | pass |
| C105 | `run elabfiles / test a_word_that_is_not_hex_is_e0706 ... ok` | pass |
| C106 | `run elabfiles / test too_few_points_for_the_degree_is_e0704 ... ok` | pass |
| C107 | `run irchecks / test a_table_expression_is_built_checked_and_printed ... ok` | pass |
| C108 | `run interp / test a_table_expression_is_refused_by_name ... ok` | pass |
| C109 | `run bookpackages / test the_include_str_table_runs ... ok` | pass |
| C110 | `run bookpackages / test e0610_is_raised_for_include_str_in_a_module ... ok` | pass |
| C111 | `run bookcheatsheet / test the_files_card_runs ... ok` | pass |
| C112 | `run docs / test gaps_section_10_lists_exactly_the_owned_codes ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourceparams` - exit 0

```
test a_body_let_as_a_param_value_sets_the_corner ... ok
test a_composite_param_needs_no_repr_and_no_read_by_analog ... ok
test an_enum_param_runs_its_arm ... ok
test a_node_initial_value_over_a_param_starts_newton_in_an_operating_point ... ok
test a_payload_enum_param_runs_its_variant ... ok
test an_integer_param_runs_with_its_value ... ok
test a_unit_enum_with_no_derive_is_a_param ... ok
test an_option_param_runs_its_case ... ok
test a_node_initial_value_over_a_param_holds_the_operating_point ... ok
test every_read_of_an_option_param_gives_the_bound_case ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.58s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/source_params.rs (/var/tmp/piperine-target-d/debug/deps/source_params-a40c966bba2b4980)
```

`Run elabparams` - exit 0

```
test an_unset_param_takes_its_default ... ok
test division_by_zero_is_inf_not_an_error ... ok
test an_enum_param_selects_its_arm ... ok
test a_required_param_left_out_is_e0319 ... ok
test functions_methods_and_logic_fold ... ok
test n_as_f64_reaches_the_contribution ... ok
test an_integer_param_is_bound_and_read ... ok
test option_param_methods_read_the_bound_case ... ok
test an_option_param_bound_to_a_value_is_some ... ok
test an_option_param_bound_over_the_parents_param_reads_some_of_it ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_params.rs (/var/tmp/piperine-target-d/debug/deps/elab_params-89d1e9c8fef61684)
```

`Run apisession` - exit 0

```
test an_integer_out_of_range_is_e0802 ... ok
test a_payload_enum_param_is_set_by_its_leaves_not_by_text ... ok
test a_refused_text_is_named_as_the_user_wrote_it ... ok
test an_option_param_set_to_a_value_or_none ... ok
test an_undriven_top_port_without_default_refuses_the_analysis ... ok
test an_unknown_variant_is_e0802_listing_the_variants ... ok
test an_integer_param_set_by_text_in_range ... ok
test params_passed_through_a_wrappers_placement_run ... ok
test an_integer_above_2_pow_53_is_set_exactly ... ok
test a_structural_params_bad_text_is_e0802 ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 58.47s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/session.rs (/var/tmp/piperine-target-d/debug/deps/session-b8872f77132e3cb3)
```

`Run clirun` - exit 0

```
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test composites_print_in_each_output_format ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test records_stay_when_the_analysis_fails ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.62s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.44s
     Running tests/run.rs (/var/tmp/piperine-target-d/debug/deps/run-3deb22af66054c5e)
```

`Run bookdigital` - exit 0

```
test the_debouncer_flips_once_at_the_fourth_clean_sample ... ok
test the_library_declares_the_monitors_and_never_runs ... ok
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_phase_detector_runs_assignment_and_events_read_as_bools ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_debouncer_pwm_and_pwm_check_print_their_records ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.85s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-params-and-io)
    Finished `test` profile [unoptimized] target(s) in 1.00s
     Running tests/book/digital.rs (/var/tmp/piperine-target-d/debug/deps/book_digital-98cddfefc66a9ac1)
```

`Run elabinstances` - exit 0

```
test a_raw_signal_name_is_a_node_without_the_prefix ... ok
test instances_come_in_pre_order ... ok
test one_body_per_module_one_instance_per_placement ... ok
test electrical_is_lowered_once_with_its_tolerances ... ok
test signals_are_nodes_and_ports_bind_the_parents ... ok
test paths_start_at_the_top_module ... ok
test a_placement_through_a_module_param_places_its_template ... ok
test a_parent_binding_replaces_the_template ... ok
test an_unnamed_placement_is_module_hash_k ... ok
test the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.97s

    Finished `test` profile [unoptimized] target(s) in 0.17s
     Running tests/elab_instances.rs (/var/tmp/piperine-target-d/debug/deps/elab_instances-c286917d2cae5fdd)
```

`Run typesmodules` - exit 0

```
test an_option_param_read_as_a_number_is_e0180 ... ok
test integer_enum_and_option_params_type_check ... ok
test a_node_of_another_nature_is_e0251 ... ok
test params_are_f64_or_bool ... ok
test an_expression_on_a_carrier_port_that_is_not_in_is_e0315 ... ok
test ports_and_signals_are_nodes_and_gnd_fits_any ... ok
test a_placement_checks_nodes_count_and_params ... ok
test an_environment_function_in_a_const_context_is_e0323 ... ok
test a_core_primitive_is_checked_where_it_is_written ... ok
test a_where_on_a_module_or_fn_is_bool ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_modules.rs (/var/tmp/piperine-target-d/debug/deps/types_modules-e42f34873c4ac40f)
```

`Run interp` - exit 0

```
test a_settle_event_is_refused_by_name ... ok
test a_host_source_body_is_rejected_by_name ... ok
test a_table_expression_is_refused_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/frozen.rs (/var/tmp/piperine-target-d/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run typesvalues` - exit 0

```
test comparing_an_enum_without_partial_ord_is_e0180 ... ok
test literals_are_f64_or_bool ... ok
test include_str_outside_a_const_is_e0610 ... ok
test diagnostics_come_in_source_order_one_per_expression ... ok
test an_option_before_a_required_parameter_is_positional_only ... ok
test an_unsupported_form_does_not_hide_an_independent_error ... ok
test operators_type_as_rust_types_them ... ok
test a_call_through_a_function_value_with_the_wrong_arity_is_e0141 ... ok
test the_std_pnjlim_checks_clean ... ok
test include_str_in_a_const_is_a_str ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_values.rs (/var/tmp/piperine-target-d/debug/deps/types_values-65c645751b306c9f)
```

`Run speccodes` - exit 0

```
running 5 tests
test every_owned_code_names_its_owner_and_construct ... ok
test this_features_codes_are_all_raised ... ok
test no_owned_code_is_raised ... ok
test no_source_spells_p0002 ... ok
test every_spec_code_is_raised_or_owned ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/spec_codes.rs (/var/tmp/piperine-target-d/debug/deps/spec_codes-86724e06e4c5ed72)
```

`Run clibook` - exit 0

```
test the_chapter_shows_the_session_options ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 49.88s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.49s
     Running tests/book.rs (/var/tmp/piperine-target-d/debug/deps/book-a1f18a8423c7ed6b)
```

`Run elabvalues` - exit 0

```
test integers_lower_to_typed_ir ... ok
test folded_casts_shifts_and_sums_wrap_as_compiled_code_does ... ok
test integer_constants_fold ... ok
test option_rows_lower_on_the_pair ... ok
test an_enum_is_its_tag_and_a_match_is_an_if_chain ... ok
test a_const_reading_a_run_value_is_e0188 ... ok
test a_const_that_depends_on_itself_is_e0188 ... ok
test an_option_is_a_flag_and_a_payload ... ok
test numeric_rows_lower_to_methods_and_checked_ops_to_pairs ... ok
test a_constant_division_by_zero_is_e0104 ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_values.rs (/var/tmp/piperine-target-d/debug/deps/elab_values-044391ce0c7e05d6)
```

`Run apisources` - exit 0

```
running 6 tests
test a_pwl_source_lands_on_every_corner ... ok
test a_node_port_tied_to_gnd_inside_is_still_refused_undriven ... ok
test an_undriven_node_port_is_e0810 ... ok
test corners_that_do_not_increase_are_e0802_and_need_a_transient ... ok
test a_timed_source_change_is_a_jump ... ok
test a_dc_source_sets_the_port_at_the_operating_point ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.23s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/host_sources.rs (/var/tmp/piperine-target-d/debug/deps/host_sources-cd95ed2520357e17)
```

`Run sourcehost` - exit 0

```
running 4 tests
test a_host_driven_rc_step_matches_tran_rc ... ok
test a_host_driven_rc_matches_ac_rc_in_ac ... ok
test a_host_pwl_into_the_rc_matches_tran_pwl_rc ... ok
test a_host_square_into_the_rc_matches_tran_square_rc ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.73s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 0.76s
     Running tests/source_host.rs (/var/tmp/piperine-target-d/debug/deps/source_host-480988613ba19c2b)
```

`Run elabentry` - exit 0

```
test an_unknown_top_is_one_diagnostic ... ok
test the_pom_keeps_a_top_with_node_ports_as_written ... ok
test device_bodies_match_the_ir_goldens ... ok
test front_end_diagnostics_are_returned_without_an_ir ... ok
test a_top_node_port_gets_one_host_source_instance ... ok
test every_diagnostic_has_code_span_and_path ... ok
test the_design_is_unchanged_by_elaboration ... ok
test elaborating_twice_prints_the_same_text ... ok
test the_twins_elaborate_without_diagnostics ... ok
test a_twin_elaborates_to_a_checked_ir ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.42s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_entry.rs (/var/tmp/piperine-target-d/debug/deps/elab_entry-0a4be7f7e45bf048)
```

`Run spectext` - exit 0

```
test tran_in_fences_is_the_module ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test the_library_declares_no_net_nature ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test zero_bytes_are_the_default_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok

test result: ok. 132 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-d/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run elabfiles` - exit 0

```
test one_file_named_twice_is_read_once ... ok
test a_table_of_the_wrong_shape_is_e0614_naming_file_and_row ... ok
test an_address_the_text_does_not_write_is_e0614 ... ok
test a_word_that_is_not_hex_is_e0706 ... ok
test parse_table_reads_64_rows_in_order ... ok
test a_token_that_is_not_a_real_is_e0706 ... ok
test parse_hex_reads_words_at_their_addresses ... ok
test too_few_points_for_the_degree_is_e0704 ... ok
test x_and_z_digits_are_e0706_integers_hold_no_unknown ... ok
test a_bad_control_string_is_e0703 ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_files.rs (/var/tmp/piperine-target-d/debug/deps/elab_files-78593e794374e4b0)
```

`Run sourcetables` - exit 0

```
running 6 tests
test cubic_reproduces_a_cubic_polynomial ... ok
test the_table_diode_passes_the_partials_verifier ... ok
test discrete_control_gives_the_closest_sample ... ok
test extrapolation_c_and_l_at_each_end ... ok
test extrapolation_e_outside_is_e0705 ... ok
test linear_table_model_is_the_interpolation_at_op_and_dc_sweep ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.36s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 0.76s
     Running tests/source_tables.rs (/var/tmp/piperine-target-d/debug/deps/source_tables-ccb6e9eb56f17034)
```

`Run clicommands` - exit 0

```
test test_prints_pass_with_the_check_count ... ok
test test_prints_target_margins_after_the_run_line ... ok
test test_prints_no_table_and_no_other_record ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_fail_and_the_failing_records ... ok
test clean_cache_removes_the_compiled_cache ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test verbose_prints_a_source_path_that_exists ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.38s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.47s
     Running tests/commands.rs (/var/tmp/piperine-target-d/debug/deps/commands-e62622f9813395ac)
```

`Run stdgalleries` - exit 0

```
test the_fixture_table_names_only_used_models ... ok
test galleries_fail_against_an_empty_prelude ... ok
test every_gallery_fence_is_in_the_formatters_form ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test the_fences_composite_values_lifted_are_checked ... ok
test every_gallery_references_its_std_items ... ok
test no_gallery_has_an_unresolved_member ... ok
test every_gallery_passes_check ... ok
test the_gate_builds_16_fences ... ok
test every_gallery_builds_with_no_diagnostics ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.74s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-d/debug/deps/std_galleries-20eb711d5dacf84a)
```

`Run msformat` - exit 0

```
test a_comment_ending_a_block_stays_inside_it ... ok
test blank_lines_collapse_to_one ... ok
test the_guard_refuses_a_text_that_lost_a_comment ... ok
test the_formatted_tree_equals_the_sources ... ok
test formatting_twice_changes_nothing ... ok
test every_stdlib_file_is_in_the_formatters_form ... ok
test formatting_keeps_the_printed_tree_of_every_shipped_source ... ok
test the_corpus_keeps_every_comment ... ok
test the_corpus_keeps_every_tree ... ok
test the_corpus_is_stable_under_a_second_format ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.34s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/msl_format.rs (/var/tmp/piperine-target-d/debug/deps/msl_format-c16dd70a423cc656)
```

`Run runerrors` - exit 0

```
running 5 tests
test a_loop_of_two_vsources_from_source_is_e0503 ... ok
test a_floating_node_from_source_is_e0503 ... ok
test a_nan_contribution_from_source_is_e0507 ... ok
test a_division_by_zero_under_known_control_fails_the_run_with_e0105 ... ok
test an_unwrap_of_none_under_known_control_fails_the_run_with_e0105 ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-params-and-io/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 0.70s
     Running tests/run_errors.rs (/var/tmp/piperine-target-d/debug/deps/run_errors-d0ffd0a2802d9e56)
```

`Run elabanalog` - exit 0

```
test two_analog_blocks_keep_their_locals_apart ... ok
test potential_and_flow_on_one_branch_is_e0415 ... ok
test freq_analysis_and_bound_step_lower ... ok
test probes_and_environment_lower_to_their_nodes ... ok
test sites_are_numbered_in_source_order_and_named ... ok
test the_device_bodies_equal_the_structural_goldens ... ok
test value_code_lowers_to_the_same_forms_and_functions_once ... ok
test bound_step_under_an_if_is_p0001 ... ok
test a_ddt_off_the_top_of_a_flow_contribution_is_refused ... ok
test a_quantity_attribute_outside_the_three_is_e0204 ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_analog.rs (/var/tmp/piperine-target-d/debug/deps/elab_analog-52fe00f5ce65d9cd)
```

`Run typesstructure` - exit 0

```
test inst_name_types_as_the_pub_item ... ok
test inst_name_of_an_item_that_is_not_pub_is_e0343 ... ok
test a_commit_to_a_childs_carrier_is_e0222 ... ok
test an_attribute_a1_does_not_list_is_e0904 ... ok
test feedback_through_an_unknown_of_a_module_is_accepted ... ok
test a_continuous_argument_read_by_a_callee_definition_is_e0336 ... ok
test a_listed_attribute_on_the_wrong_item_is_e0904 ... ok
test a_discrete_argument_read_by_a_callee_definition_is_accepted ... ok
test a_cycle_with_a_direct_dependence_at_every_result_is_e0353 ... ok
test an_initial_value_that_reads_a_carrier_is_e0210 ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_structure.rs (/var/tmp/piperine-target-d/debug/deps/types_structure-78e56499b09a151b)
```

`Run typescarriers` - exit 0

```
test a_loop_of_definitions_is_e0339 ... ok
test an_initial_value_under_an_undelayed_definition_is_e0338 ... ok
test a_carrier_of_another_value_type_is_e0251 ... ok
test a_continuous_expression_at_an_in_port_is_e0316 ... ok
test prev_is_typed_on_a_carrier ... ok
test an_out_port_takes_only_a_carrier_the_parent_hands_over ... ok
test a_target_the_module_may_not_drive_is_e0222 ... ok
test two_definitions_of_one_carrier_are_e0335 ... ok
test an_initial_value_of_the_wrong_kind_is_e0210 ... ok
test a_continuous_definition_is_e0336 ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.68s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_carriers.rs (/var/tmp/piperine-target-d/debug/deps/types_carriers-e1157a56d14a4b3b)
```

`Run typesresults` - exit 0

```
test a_bare_analog_read_of_a_carrier_has_no_diagnostic ... ok
test a_continuous_result_read_where_probes_are_not_allowed_is_e0354 ... ok
test a_placement_has_its_result_type ... ok
test an_event_result_outside_an_event_position_is_e0354 ... ok
test a_let_that_reads_itself_is_e0332_or_e0353 ... ok
test a_placement_in_a_const_context_is_e0352 ... ok
test trailing_option_inputs_may_be_left_out ... ok
test a_module_with_a_result_is_typed ... ok
test now_and_analysis_static_are_typed ... ok
test an_analog_read_of_a_carrier_is_the_value ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.86s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_results.rs (/var/tmp/piperine-target-d/debug/deps/types_results-debe08770933098b)
```

`Run hygiene` - exit 0

```
test mod_rs_declares_only ... ok
test the_walk_covers_the_three_solver_crates ... ok
test root_targets_match_the_naming_rule ... ok
test no_file_scope_lint_suppression ... ok
test no_ignored_tests ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_dead_architecture_identifiers ... ok
test no_disabled_test_code ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.35s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-d/debug/deps/suite_hygiene-354b8333c6c0effe)
```

`Run docs` - exit 0

```
test state_holds_the_explicit_unknowns_decision ... ok
test state_records_the_derived_param_decision ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test transient_performance_closes_v12 ... ok
test the_roadmap_is_forward_looking ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-93bbd657b5852db6)
```

`Run bookcorestd` - exit 0

```
test the_library_declares_the_noise_sources_and_the_monitors_only_report ... ok
test the_dac_ramp_scales_its_code_by_the_reference ... ok
test the_probe_writes_a_check_and_a_log_record ... ok
test the_pad_drives_its_node_and_reads_it_back ... ok
test the_tour_module_runs_its_analog_and_digital_blocks ... ok
test the_watchdog_builds_with_its_u16_param ... ok
test a_crates_own_vt_replaces_the_library_one ... ok
test the_probes_have_the_short_names_the_nature_gives ... ok
test the_watchdog_ends_the_run_at_the_fourth_edge_with_the_button_held ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.43s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-params-and-io)
    Finished `test` profile [unoptimized] target(s) in 0.94s
     Running tests/book/core_std.rs (/var/tmp/piperine-target-d/debug/deps/book_core_std-732527cde9b72258)
```

`Run bookinventory` - exit 0

```
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test every_block_of_the_book_has_one_row ... ok
test every_listing_that_does_not_run_names_its_gap ... ok
test run_rows_name_tests_that_run_their_model ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test every_row_names_an_existing_test ... ok
test the_book_uses_no_removed_name ... ok
test the_book_reads_every_block ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-params-and-io)
    Finished `test` profile [unoptimized] target(s) in 0.54s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-d/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

`Run irchecks` - exit 0

```
test a_top_alone_prints_and_checks ... ok
test a_table_expression_is_built_checked_and_printed ... ok
test a_recursive_call_or_an_unbound_local_is_named ... ok
test a_cyclic_expression_is_named_without_hanging ... ok
test a_duplicated_path_is_named ... ok
test an_instance_with_wrong_arity_is_named ... ok
test a_wrong_type_in_an_analog_position_is_named ... ok
test an_out_of_range_id_is_named ... ok
test a_misplaced_or_duplicated_site_is_named ... ok
test a_built_ir_checks_clean ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized] target(s) in 0.06s
     Running tests/check.rs (/var/tmp/piperine-target-d/debug/deps/check-accb0dc311a55cae)
```

`Run bookpackages` - exit 0

```
test e0610_is_raised_for_include_str_in_a_module ... ok
test e0162_one_name_in_one_namespace_is_declared_once ... ok
test e0401_a_behavior_block_lives_in_the_file_of_its_module ... ok
test e0161_a_private_item_and_a_module_file_that_is_not_there_or_is_there_twice ... ok
test the_crate_root_of_the_bigger_project_loads_its_files_and_reaches_its_dependency ... ok
test a_function_named_like_a_dependency_does_not_clash_with_it ... ok
test a_local_namespace_and_a_dependency_of_one_name_make_the_use_ambiguous ... ok
test the_include_str_table_runs ... ok
test the_lowpass_project_runs_as_the_book_assembles_it ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.86s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-params-and-io)
    Finished `test` profile [unoptimized] target(s) in 0.89s
     Running tests/book/packages.rs (/var/tmp/piperine-target-d/debug/deps/book_packages-e41ca7ef020ae985)
```

`Run bookcheatsheet` - exit 0

```
test the_module_card_is_refused_for_its_const_generic ... ok
test the_template_card_is_refused_for_its_generic_module_and_its_placements ... ok
test the_debug_prints_card_is_refused_for_the_analog_trace ... ok
test the_body_items_card_is_refused_for_its_generation ... ok
test the_nature_card_is_the_standard_librarys ... ok
test the_analog_card_runs ... ok
test the_digital_card_runs ... ok
test the_files_card_runs ... ok
test a_name_of_yours_hides_the_library_but_not_the_core ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.58s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-params-and-io)
    Finished `test` profile [unoptimized] target(s) in 0.90s
     Running tests/book/cheatsheet.rs (/var/tmp/piperine-target-d/debug/deps/book_cheatsheet-c7d06684e7366c2e)
```

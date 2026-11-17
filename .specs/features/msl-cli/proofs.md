# msl-cli proofs

**HEAD**: 772c790f
**Tree**: clean
**Shell**: /bin/sh
**Result**: 203 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run shape / test default_members_are_the_green_crates ... ok` | pass |
| C2 | `run usage / test the_binary_is_named_piperine ... ok` | pass |
| C3 | `run shape / test the_root_package_is_a_re_export_and_nothing_else ... ok` | pass |
| C4 | `run shape / test host_crates_name_no_deleted_surface ... ok` | pass |
| C5 | `run usage / test help_lists_exactly_the_seven_commands ... ok` | pass |
| C6 | `run clilib / test cli::tests::every_command_is_built ... ok` | pass |
| C7 | `run usage / test version_prints_the_workspace_version ... ok` | pass |
| C8 | `run usage / test an_unknown_command_or_flag_exits_2 ... ok` | pass |
| C9 | `run project / test discover_takes_the_nearest_manifest_upward ... ok` | pass |
| C10 | `run run / test the_project_flag_uses_that_directory_and_never_searches ... ok` | pass |
| C11 | `run run / test no_manifest_is_e0170_and_exit_1 ... ok` | pass |
| C12 | `run manifest / test a_run_table_is_accepted_and_located ... ok` | pass |
| C13 | `run spectext / test the_manifest_rule_admits_the_run_table ... ok` | pass |
| C14 | `run project / test a_dependencys_run_table_is_ignored ... ok` | pass |
| C15 | `run run / test a_package_error_is_rendered_with_code_file_and_line ... ok` | pass |
| C16 | `run project / test a_bench_is_the_root_crate_over_the_package_and_its_dependencies ... ok` | pass |
| C17 | `run project / test a_run_without_a_bench_roots_at_the_package ... ok` | pass |
| C18 | `run project / test a_lone_file_with_no_manifest_is_a_crate_alone ... ok` | pass |
| C19 | `run project / test a_file_under_a_package_is_its_bench ... ok` | pass |
| C20 | `run project / test a_file_under_src_is_refused_with_p0010 ... ok` | pass |
| C21 | `run apilib / test request::tests::a_run_table_reads_its_nineteen_keys ... ok` | pass |
| C22 | `run clilib / test cli::tests::every_run_key_has_its_flag ... ok` | pass |
| C23 | `run apilib / test request::tests::a_flag_overrides_a_key ... ok` | pass |
| C24 | `run apilib / test request::tests::an_unknown_key_or_a_wrong_type_is_p0010_at_its_span ... ok` | pass |
| C25 | `run apilib / test request::tests::analysis_is_one_of_four ... ok` | pass |
| C26 | `run apilib / test request::tests::a_transient_needs_a_positive_stop ... ok` | pass |
| C27 | `run apilib / test request::tests::a_key_outside_its_analysis_is_p0010 ... ok` | pass |
| C28 | `run project / test a_missing_bench_file_is_p0010_at_the_value ... ok` | pass |
| C29 | `run project / test the_only_run_is_run_without_a_name ... ok` | pass |
| C30 | `run run / test no_name_with_no_run_or_several_lists_them_and_exits_2 ... ok` | pass |
| C31 | `run run / test an_unknown_run_name_lists_the_runs_and_exits_2 ... ok` | pass |
| C32 | `run run / test a_lone_file_without_analysis_exits_2 ... ok` | pass |
| C33 | `run apilib / test request::tests::a_run_name_is_an_identifier ... ok` | pass |
| C34 | `run run / test a_single_valued_flag_given_twice_exits_2 ... ok` | pass |
| C35 | `run session / test the_top_key_names_the_module ... ok` | pass |
| C36 | `run session / test the_one_portless_module_is_the_top ... ok` | pass |
| C37 | `run session / test no_top_or_several_is_p0011_with_candidates ... ok` | pass |
| C38 | `run clilib / test render::diagnostic::tests::a_diagnostic_has_code_file_line_and_excerpt ... ok` | pass |
| C39 | `run run / test a_type_error_is_rendered_with_its_stage_and_exits_1 ... ok` | pass |
| C40 | `run session / test a_missing_rustc_names_rustc_and_the_interpreter ... ok` | pass |
| C41 | `run session / test the_interpreter_refusal_names_the_body_and_the_rust_backend ... ok` | pass |
| C42 | `run apilib / test backend::tests::the_flag_then_the_variable_then_the_key_then_rust ... ok` | pass |
| C43 | `run run / test a_bad_backend_exits_2_for_the_flag_and_1_otherwise ... ok` | pass |
| C44 | `run session / test a_run_compiles_once_and_never_during_an_analysis ... ok` | pass |
| C45 | `run session / test a_top_with_discrete_in_ports_runs_with_them_unknown ... ok` | pass |
| C46 | `run session / test a_top_with_a_continuous_port_is_the_front_ends_diagnostic ... ok` | pass |
| C47 | `run analyses / test an_operating_point_is_signal_and_value_in_print_order ... ok` | pass |
| C48 | `run analyses / test a_transient_has_one_row_per_accepted_step ... ok` | pass |
| C49 | `run probes / test the_default_is_the_tops_own_signals_in_order ... ok` | pass |
| C50 | `run probes / test a_node_a_net_and_a_flow_resolve_relative_or_full ... ok` | pass |
| C51 | `run probes / test a_column_is_named_by_the_full_path ... ok` | pass |
| C52 | `run probes / test an_entry_that_names_nothing_is_e0801_before_any_analysis ... ok` | pass |
| C53 | `run analyses / test the_divider_mid_is_7_5_on_both_backends ... ok` | pass |
| C54 | `run analyses / test the_rc_step_follows_one_minus_exp_on_both_backends ... ok` | pass |
| C55 | `run run / test a_solver_failure_prints_no_table_and_exits_1 ... ok` | pass |
| C56 | `run clilib / test render::number::tests::numbers_have_one_text_each ... ok` | pass |
| C57 | `run probes / test a_signal_printed_twice_is_two_columns ... ok` | pass |
| C58 | `run records / test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok` | pass |
| C59 | `run clilib / test render::record::tests::a_record_is_one_line ... ok` | pass |
| C60 | `run records / test records_arrive_in_time_order ... ok` | pass |
| C61 | `run records / test a_text_field_is_the_sources_literal ... ok` | pass |
| C62 | `run records / test an_unknown_field_is_unknown ... ok` | pass |
| C63 | `run records / test a_failed_require_is_one_checks_record_with_pass_false ... ok` | pass |
| C64 | `run run / test records_come_before_the_table ... ok` | pass |
| C65 | `run records / test a_design_without_emit_gives_no_record ... ok` | pass |
| C66 | `run records / test a_reserved_stream_keeps_its_name_and_its_site ... ok` | pass |
| C67 | `run run / test run_prints_header_records_blank_line_and_table ... ok` | pass |
| C68 | `run clilib / test render::table::tests::a_table_is_left_aligned_two_spaces_no_trailing_space ... ok` | pass |
| C69 | `run run / test a_completed_run_exits_0 ... ok` | pass |
| C70 | `run run / test a_failure_before_the_analysis_prints_nothing_on_stdout ... ok` | pass |
| C71 | `run run / test records_stay_when_the_analysis_fails ... ok` | pass |
| C72 | `run run / test stdout_is_the_same_from_two_caches_and_two_directories ... ok` | pass |
| C73 | `run run / test a_lone_file_run_is_named_by_its_stem ... ok` | pass |
| C74 | `run run / test a_first_compile_is_announced_on_stderr ... ok` | pass |
| C75 | `run run / test a_closed_stdout_ends_with_exit_1_and_no_panic ... ok` | pass |
| C76 | `run buck / test the_buck_has_its_bench_and_two_runs ... ok` | pass |
| C77 | `run buck / test startup_ends_with_the_reference_at_128 ... ok` | pass |
| C78 | `run buck / test startup_prints_no_failed_check ... ok` | pass |
| C79 | `run buck / test idle_keeps_the_mode_off ... ok` | pass |
| C80 | `run buck / test piperine_test_passes_on_the_buck ... ok` | pass |
| C81 | `run analyses / test a_dc_sweep_has_one_row_per_point ... ok` | pass |
| C82 | `run analyses / test an_ac_sweep_has_mag_and_phase_in_degrees ... ok` | pass |
| C83 | `run apilib / test request::tests::a_dc_sweep_needs_its_four_keys_and_a_step_toward_to ... ok` | pass |
| C84 | `run apilib / test request::tests::per_decade_gives_round_n_log10_plus_one_points ... ok` | pass |
| C85 | `run apilib / test request::tests::an_ac_grid_is_exactly_one_of_two_and_its_range_is_ordered ... ok` | pass |
| C86 | `run analyses / test points_sweeps_linearly ... ok` | pass |
| C87 | `run analyses / test the_rc_corner_is_minus_3_db_and_minus_45_degrees ... ok` | pass |
| C88 | `run analyses / test the_diode_sweep_has_37_rows ... ok` | pass |
| C89 | `run analyses / test a_sweep_of_no_param_is_e0801 ... ok` | pass |
| C90 | `run analyses / test params_are_set_in_order_table_then_flags ... ok` | pass |
| C91 | `run analyses / test a_param_path_that_names_nothing_is_e0801 ... ok` | pass |
| C92 | `run analyses / test a_refused_value_is_e0802_and_nothing_runs ... ok` | pass |
| C93 | `run options / test the_divider_with_r2_at_1k_gives_5 ... ok` | pass |
| C94 | `run options / test csv_writes_the_table_and_prints_wrote ... ok` | pass |
| C95 | `run options / test an_existing_csv_is_replaced ... ok` | pass |
| C96 | `run options / test an_unwritable_csv_names_the_path_and_exits_1 ... ok` | pass |
| C97 | `run apilib / test table::tests::a_print_grid_has_one_row_per_multiple ... ok` | pass |
| C98 | `run apilib / test table::tests::continuous_is_interpolated_and_discrete_is_held ... ok` | pass |
| C99 | `run options / test a_relative_output_path_is_under_the_project ... ok` | pass |
| C100 | `run apilib / test table::tests::a_grid_wider_than_the_run_is_the_row_at_zero ... ok` | pass |
| C101 | `run records / test keep_matches_a_stream_name_or_its_full_path ... ok` | pass |
| C102 | `run records / test a_filter_entry_that_matches_nothing_is_e0801 ... ok` | pass |
| C103 | `run options / test records_go_to_the_file_as_json_lines_and_not_to_stdout ... ok` | pass |
| C104 | `run options / test an_existing_records_file_is_replaced ... ok` | pass |
| C105 | `run clilib / test render::json::tests::each_value_kind_has_its_json ... ok` | pass |
| C106 | `run options / test csv_and_records_leave_the_header_and_the_wrote_line ... ok` | pass |
| C107 | `run options / test the_stream_flag_keeps_only_that_stream ... ok` | pass |
| C108 | `run twins / test the_three_op_twins_run_on_both_backends ... ok` | pass |
| C109 | `run twins / test the_sweep_and_transient_twins_run_on_both_backends ... ok` | pass |
| C110 | `run twins / test the_two_backends_print_the_same_bytes ... ok` | pass |
| C111 | `run commands / test check_runs_every_stage_and_no_backend ... ok` | pass |
| C112 | `run commands / test check_prints_ok_with_the_counts ... ok` | pass |
| C113 | `run commands / test check_renders_every_diagnostic_and_exits_1 ... ok` | pass |
| C114 | `run commands / test check_without_a_run_says_type_checking_needs_one ... ok` | pass |
| C115 | `run commands / test check_takes_a_lone_file ... ok` | pass |
| C116 | `run commands / test build_prints_one_line_per_run ... ok` | pass |
| C117 | `run commands / test a_second_build_is_cached_and_compiles_nothing ... ok` | pass |
| C118 | `run commands / test verbose_prints_a_source_path_that_exists ... ok` | pass |
| C119 | `run commands / test build_goes_on_after_a_failing_run_and_exits_1 ... ok` | pass |
| C120 | `run commands / test test_runs_every_run_or_the_named_ones ... ok` | pass |
| C121 | `run commands / test test_prints_pass_with_the_check_count ... ok` | pass |
| C122 | `run apilib / test verdict::tests::no_false_and_no_unknown_pass_is_a_pass ... ok` | pass |
| C123 | `run commands / test test_prints_fail_and_the_failing_records ... ok` | pass |
| C124 | `run apilib / test verdict::tests::an_unknown_pass_fails ... ok` | pass |
| C125 | `run commands / test test_prints_no_table_and_no_other_record ... ok` | pass |
| C126 | `run commands / test a_run_that_does_not_complete_is_a_fail ... ok` | pass |
| C127 | `run commands / test the_last_line_counts_and_the_exit_code_follows ... ok` | pass |
| C128 | `run commands / test a_project_without_a_run_passes_with_zero ... ok` | pass |
| C129 | `run commands / test new_creates_the_four_files_and_says_created ... ok` | pass |
| C130 | `run commands / test new_writes_the_rc_example_under_the_new_name ... ok` | pass |
| C131 | `run commands / test new_refuses_an_existing_directory ... ok` | pass |
| C132 | `run commands / test new_refuses_a_name_the_manifest_rule_forbids ... ok` | pass |
| C133 | `run commands / test a_new_project_checks_runs_and_tests ... ok` | pass |
| C134 | `run commands / test new_without_a_name_exits_2 ... ok` | pass |
| C135 | `run commands / test clean_removes_the_target_directory ... ok` | pass |
| C136 | `run commands / test clean_cache_removes_the_compiled_cache ... ok` | pass |
| C137 | `run commands / test clean_with_nothing_to_remove_exits_0 ... ok` | pass |
| C138 | `run commands / test clean_touches_nothing_else ... ok` | pass |
| C139 | `run book / test the_cli_chapter_sits_between_compiler_and_cheatsheet ... ok` | pass |
| C140 | `run clibook / test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok` | pass |
| C141 | `run clibook / test the_chapter_shows_fmt_and_says_comments_stay ... ok` | pass |
| C142 | `run book / test the_welcome_chapter_says_a_bench_is_msl ... ok` | pass |
| C143 | `run rcpkg / test the_rc_example_resolves_with_no_diagnostic ... ok` | pass |
| C144 | `run clibook / test the_chapter_shows_the_rc_project_file_for_file ... ok` | pass |
| C145 | `run clibook / test the_rc_output_blocks_are_what_the_binary_prints ... ok` | pass |
| C146 | `run clibook / test the_buck_block_is_what_the_binary_prints ... ok` | pass |
| C147 | `run clibook / test the_chapter_explains_emit_as_print ... ok` | pass |
| C148 | `run clibook / test the_chapter_lists_commands_keys_flags_codes ... ok` | pass |
| C149 | `run clibook / test the_chapter_stands_alone ... ok` | pass |
| C150 | `run book / test every_msl_block_parses ... ok` | pass |
| C151 | `run book / test the_compiler_chapter_says_the_command_line_exists ... ok` | pass |
| C152 | `run book / test the_cheat_sheet_lists_the_seven_commands ... ok` | pass |
| C153 | `run docs / test the_instruction_files_say_the_host_crates_build ... ok` | pass |
| C154 | `run docs / test gaps_drops_what_closed_and_lists_what_is_left ... ok` | pass |
| C155 | `run docs / test the_roadmap_marks_the_cli_item_done ... ok` | pass |
| C156 | `run docs / test state_records_the_cli_decision ... ok` | pass |
| C157 | `run docs / test the_buck_readme_says_how_to_run_it ... ok` | pass |
| C158 | `run spectext / test part_viii_defines_a_bench ... ok` | pass |
| C159 | `run spectext / test the_index_says_benches_are_msl_and_sweeps_are_host ... ok` | pass |
| C160 | `run spectext / test the_testbench_table_splits_stimulus_and_host_work ... ok` | pass |
| C161 | `run spectext / test no_spec_text_says_stimulus_is_host_only ... ok` | pass |
| C162 | `run spectext / test every_error_code_is_defined_exactly_once ... ok` | pass |
| C163 | `run options / test plot_writes_an_svg_and_prints_wrote ... ok` | pass |
| C164 | `run clilib / test render::svg::tests::one_path_per_signal_with_its_name_and_a_legend ... ok` | pass |
| C165 | `run apilib / test plot::tests::a_value_maps_linearly_onto_the_plot_area ... ok` | pass |
| C166 | `run clilib / test render::svg::tests::each_axis_has_ticks_labels_and_a_title ... ok` | pass |
| C167 | `run apilib / test plot::tests::ticks_are_1_2_5_times_a_power_of_ten ... ok` | pass |
| C168 | `run apilib / test plot::tests::a_discrete_signal_gets_a_lane_and_no_series ... ok` | pass |
| C169 | `run apilib / test plot::tests::an_ac_plot_has_a_db_panel_over_a_degree_panel_and_a_log_axis ... ok` | pass |
| C170 | `run options / test a_plot_draws_every_accepted_step_whatever_the_print_step ... ok` | pass |
| C171 | `run apilib / test plot::tests::an_operating_point_has_nothing_to_plot ... ok` | pass |
| C172 | `run apilib / test plot::tests::an_unknown_value_breaks_the_run ... ok` | pass |
| C173 | `run clilib / test render::svg::tests::the_same_plot_is_the_same_bytes ... ok` | pass |
| C174 | `run options / test an_unwritable_plot_names_the_path_and_exits_1 ... ok` | pass |
| C175 | `run format / test plain_comments_are_listed_with_span_text_and_line_start ... ok` | pass |
| C176 | `run format / test every_plain_comment_comes_out_once_in_order_unchanged ... ok` | pass |
| C177 | `run format / test the_guard_refuses_a_text_that_lost_a_comment ... ok` | pass |
| C178 | `run format / test the_formatted_tree_equals_the_sources ... ok` | pass |
| C179 | `run format / test formatting_twice_changes_nothing ... ok` | pass |
| C180 | `run format / test a_comment_is_anchored_before_after_or_inside ... ok` | pass |
| C181 | `run format / test a_comment_on_its_own_line_stays_before_its_node ... ok` | pass |
| C182 | `run format / test a_trailing_comment_stays_on_its_line ... ok` | pass |
| C183 | `run format / test a_comment_ending_a_block_stays_inside_it ... ok` | pass |
| C184 | `run format / test an_unreachable_comment_marks_its_statement_verbatim ... ok` | pass |
| C185 | `run format / test a_statement_with_an_inner_comment_is_kept_as_written ... ok` | pass |
| C186 | `run format / test blank_lines_collapse_to_one ... ok` | pass |
| C187 | `run format / test a_source_that_does_not_parse_gives_diagnostics_and_no_text ... ok` | pass |
| C188 | `run format / test the_corpus_keeps_every_comment ... ok` | pass |
| C189 | `run format / test the_corpus_keeps_every_tree ... ok` | pass |
| C190 | `run format / test the_corpus_is_stable_under_a_second_format ... ok` | pass |
| C191 | `run print / test print_is_idempotent_over_corpus ... ok` | pass |
| C192 | `run print / test roundtrip_ast_equal_over_corpus ... ok` | pass |
| C193 | `run commands / test fmt_rewrites_what_differs_and_names_it ... ok` | pass |
| C194 | `run commands / test fmt_takes_files ... ok` | pass |
| C195 | `run commands / test fmt_check_writes_nothing_and_exits_1_on_a_difference ... ok` | pass |
| C196 | `run commands / test fmt_skips_a_file_that_does_not_parse_and_exits_1 ... ok` | pass |
| C197 | `run commands / test fmt_on_formatted_files_is_silent ... ok` | pass |
| C198 | `run commands / test fmt_check_passes_on_the_rc_example_and_on_a_new_project ... ok` | pass |
| Build | `cargo build -p piperine-project -p piperine-lang -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! ` | pass |
| Build | `test ! -f crates/piperine-api/src/failure.rs // cargo build -p piperine-api -p piperine --all-targets 2>&1 / t` | pass |
| Build | `test ! -f crates/piperine-cli/src/cli.rs // cargo build -p piperine-cli --all-targets 2>&1 / tee /dev/stderr /` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run shape` - exit 0

```
test retired_crates_are_gone ... ok
test instructions_list_the_kept_crates ... ok
test solver_manifest_has_no_in_tree_dev_dependency ... ok
test root_tests_hold_no_end_to_end_target ... ok
test state_records_the_solver_structure_decision ... ok
test the_root_package_is_a_re_export_and_nothing_else ... ok
test workflows_name_no_retired_crate ... ok
test host_crates_name_no_deleted_surface ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-solver)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.98s
     Running tests/workspace_shape.rs (/var/tmp/piperine-target/debug/deps/workspace_shape-3039e2667bd94175)
```

`Run usage` - exit 0

```
running 4 tests
test the_binary_is_named_piperine ... ok
test version_prints_the_workspace_version ... ok
test help_lists_exactly_the_seven_commands ... ok
test an_unknown_command_or_flag_exits_2 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.15s
     Running tests/usage.rs (/var/tmp/piperine-target/debug/deps/usage-098867c83cf58e24)
```

`Run clilib` - exit 0

```
test render::json::tests::each_value_kind_has_its_json ... ok
test render::number::tests::numbers_have_one_text_each ... ok
test render::record::tests::a_record_is_one_line ... ok
test render::table::tests::a_table_is_left_aligned_two_spaces_no_trailing_space ... ok
test render::svg::tests::one_path_per_signal_with_its_name_and_a_legend ... ok
test render::svg::tests::the_same_plot_is_the_same_bytes ... ok
test render::svg::tests::each_axis_has_ticks_labels_and_a_title ... ok
test cli::tests::every_command_is_built ... ok
test cli::tests::every_run_key_has_its_flag ... ok
test render::diagnostic::tests::a_diagnostic_has_code_file_line_and_excerpt ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_cli-f58330f3a788d0cd)
```

`Run project` - exit 0

```
running 9 tests
test a_file_under_src_is_refused_with_p0010 ... ok
test a_missing_bench_file_is_p0010_at_the_value ... ok
test discover_takes_the_nearest_manifest_upward ... ok
test the_only_run_is_run_without_a_name ... ok
test a_file_under_a_package_is_its_bench ... ok
test a_dependencys_run_table_is_ignored ... ok
test a_lone_file_with_no_manifest_is_a_crate_alone ... ok
test a_run_without_a_bench_roots_at_the_package ... ok
test a_bench_is_the_root_crate_over_the_package_and_its_dependencies ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/project.rs (/var/tmp/piperine-target/debug/deps/project-211f5cbd2c80f082)
```

`Run run` - exit 0

```
test a_solver_failure_prints_no_table_and_exits_1 ... ok
test a_lone_file_run_is_named_by_its_stem ... ok
test records_come_before_the_table ... ok
test a_completed_run_exits_0 ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test a_closed_stdout_ends_with_exit_1_and_no_panic ... ok
test run_prints_header_records_blank_line_and_table ... ok
test a_first_compile_is_announced_on_stderr ... ok
test records_stay_when_the_analysis_fails ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.75s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target/debug/deps/run-8bc9482a472d21be)
```

`Run manifest` - exit 0

```
test a_bad_dependency_name_or_requirement_is_e0171 ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling serde v1.0.228
   Compiling miette v7.6.0
   Compiling log v0.4.29
   Compiling once_cell v1.21.4
   Compiling git2 v0.21.0
   Compiling tempfile v3.27.0
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-ir)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-lang)
   Compiling piperine-project v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-project)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 25.73s
     Running tests/manifest.rs (/var/tmp/piperine-target/debug/deps/manifest-1930747ca54e3474)
```

`Run spectext` - exit 0

```
test tran_in_fences_is_the_module ... ok
test value_blocks_may_hold_contributions_and_commits ... ok
test every_error_code_is_defined_exactly_once ... ok
test s36_defines_the_package_codes ... ok
test zero_bytes_are_the_unknown_value ... ok
test no_spec_text_says_stimulus_is_host_only ... ok

test result: ok. 84 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-ir)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-plan)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-lang)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.53s
     Running tests/spec_text.rs (/var/tmp/piperine-target/debug/deps/spec_text-b62cdc0a876d2a27)
```

`Run apilib` - exit 0

```
test table::tests::a_print_grid_has_one_row_per_multiple ... ok
test request::tests::a_transient_needs_a_positive_stop ... ok
test table::tests::continuous_is_interpolated_and_discrete_is_held ... ok
test request::tests::analysis_is_one_of_four ... ok
test verdict::tests::an_unknown_pass_fails ... ok
test request::tests::a_run_table_reads_its_nineteen_keys ... ok
test request::tests::a_dc_sweep_needs_its_four_keys_and_a_step_toward_to ... ok
test verdict::tests::no_false_and_no_unknown_pass_is_a_pass ... ok
test request::tests::an_unknown_key_or_a_wrong_type_is_p0010_at_its_span ... ok
test request::tests::an_ac_grid_is_exactly_one_of_two_and_its_range_is_ordered ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_api-f46f164108a134e4)
```

`Run session` - exit 0

```
running 8 tests
test a_missing_rustc_names_rustc_and_the_interpreter ... ok
test the_one_portless_module_is_the_top ... ok
test the_interpreter_refusal_names_the_body_and_the_rust_backend ... ok
test a_top_with_a_continuous_port_is_the_front_ends_diagnostic ... ok
test the_top_key_names_the_module ... ok
test no_top_or_several_is_p0011_with_candidates ... ok
test a_run_compiles_once_and_never_during_an_analysis ... ok
test a_top_with_discrete_in_ports_runs_with_them_unknown ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.10s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/session.rs (/var/tmp/piperine-target/debug/deps/session-ef7a3bd1d4c33533)
```

`Run analyses` - exit 0

```
test a_param_path_that_names_nothing_is_e0801 ... ok
test the_diode_sweep_has_37_rows ... ok
test an_operating_point_is_signal_and_value_in_print_order ... ok
test the_divider_mid_is_7_5_on_both_backends ... ok
test a_refused_value_is_e0802_and_nothing_runs ... ok
test the_rc_corner_is_minus_3_db_and_minus_45_degrees ... ok
test an_ac_sweep_has_mag_and_phase_in_degrees ... ok
test a_dc_sweep_has_one_row_per_point ... ok
test the_rc_step_follows_one_minus_exp_on_both_backends ... ok
test params_are_set_in_order_table_then_flags ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.60s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/analyses.rs (/var/tmp/piperine-target/debug/deps/analyses-b506f377e01c0549)
```

`Run probes` - exit 0

```
running 5 tests
test a_column_is_named_by_the_full_path ... ok
test a_signal_printed_twice_is_two_columns ... ok
test a_node_a_net_and_a_flow_resolve_relative_or_full ... ok
test the_default_is_the_tops_own_signals_in_order ... ok
test an_entry_that_names_nothing_is_e0801_before_any_analysis ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/probes.rs (/var/tmp/piperine-target/debug/deps/probes-f0711161f95d64f9)
```

`Run records` - exit 0

```
running 9 tests
test a_filter_entry_that_matches_nothing_is_e0801 ... ok
test a_design_without_emit_gives_no_record ... ok
test a_text_field_is_the_sources_literal ... ok
test records_arrive_in_time_order ... ok
test a_reserved_stream_keeps_its_name_and_its_site ... ok
test a_failed_require_is_one_checks_record_with_pass_false ... ok
test an_unknown_field_is_unknown ... ok
test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok
test keep_matches_a_stream_name_or_its_full_path ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.17s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/records.rs (/var/tmp/piperine-target/debug/deps/records-2db83e7d73620fa9)
```

`Run buck` - exit 0

```
test the_buck_has_its_bench_and_two_runs ... ok
test idle_keeps_the_mode_off ... ok
test piperine_test_passes_on_the_buck has been running for over 60 seconds
test startup_ends_with_the_reference_at_128 has been running for over 60 seconds
test startup_prints_no_failed_check has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines has been running for over 60 seconds
test startup_ends_with_the_reference_at_128 ... ok
test the_buck_runs_print_at_most_16_lines ... ok
test piperine_test_passes_on_the_buck ... ok
test startup_prints_no_failed_check ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 258.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/buck.rs (/var/tmp/piperine-target/debug/deps/buck-a661d56f54666ee6)
```

`Run options` - exit 0

```
test an_unwritable_plot_names_the_path_and_exits_1 ... ok
test csv_and_records_leave_the_header_and_the_wrote_line ... ok
test an_existing_csv_is_replaced ... ok
test plot_writes_an_svg_and_prints_wrote ... ok
test an_existing_records_file_is_replaced ... ok
test records_go_to_the_file_as_json_lines_and_not_to_stdout ... ok
test csv_writes_the_table_and_prints_wrote ... ok
test a_relative_output_path_is_under_the_project ... ok
test a_passing_check_is_hidden_a_failing_one_shown_and_all_checks_shows_both ... ok
test the_stream_flag_keeps_only_that_stream ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.59s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/options.rs (/var/tmp/piperine-target/debug/deps/options-b939bfd130e3db2e)
```

`Run twins` - exit 0

```
running 3 tests
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.94s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target/debug/deps/twins-2aa9f00922f0fbd5)
```

`Run commands` - exit 0

```
test test_prints_no_table_and_no_other_record ... ok
test a_second_build_is_cached_and_compiles_nothing ... ok
test build_prints_one_line_per_run ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_fail_and_the_failing_records ... ok
test verbose_prints_a_source_path_that_exists ... ok
test test_prints_pass_with_the_check_count ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test clean_cache_removes_the_compiled_cache ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.95s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/commands.rs (/var/tmp/piperine-target/debug/deps/commands-e67a6e5d751dd843)
```

`Run book` - exit 0

```
test the_welcome_chapter_says_a_bench_is_msl ... ok
/home/keslarek/Git/piperine-msl-cli/docs/book/index.html 422337
test build_py_writes_an_ignored_index ... ok
test the_chapter_explains_the_prelude_and_the_engine_items ... ok
test the_chapter_has_a_part_per_namespace ... ok
test packages_and_cheatsheet_name_std ... ok
test the_chapter_names_every_prelude_export ... ok
test every_msl_block_parses ... ok
test chapter_blocks_build_against_std ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.57s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running tests/book.rs (/var/tmp/piperine-target/debug/deps/book-dabbd8a363fa42aa)
```

`Run clibook` - exit 0

```
test the_chapter_explains_emit_as_print ... ok
test the_chapter_stands_alone ... ok
test the_chapter_shows_the_rc_project_file_for_file ... ok
test the_chapter_lists_commands_keys_flags_codes ... ok
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 130.63s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target/debug/deps/book-41913a260b16a2a6)
```

`Run rcpkg` - exit 0

```
running 7 tests
test the_project_root_is_the_nearest_manifest ... ok
test a_package_error_stops_load_project ... ok
test the_crate_holds_no_retired_surface ... ok
test the_crate_reports_exactly_the_package_phase_codes ... ok
test a_model_error_is_a_diagnostic_not_a_package_error ... ok
test load_project_builds_the_design_of_a_package_graph ... ok
test the_rc_example_resolves_with_no_diagnostic ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

   Compiling piperine-project v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-project)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.53s
     Running tests/project.rs (/var/tmp/piperine-target/debug/deps/project-2fb90affb5253166)
```

`Run docs` - exit 0

```
running 5 tests
test the_buck_readme_says_how_to_run_it ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test gaps_drops_what_closed_and_lists_what_is_left ... ok
test state_records_the_cli_decision ... ok
test the_instruction_files_say_the_host_crates_build ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target/debug/deps/docs-7e405e0698d57fd4)
```

`Run format` - exit 0

```
test a_trailing_comment_stays_on_its_line ... ok
test a_comment_on_its_own_line_stays_before_its_node ... ok
test blank_lines_collapse_to_one ... ok
test the_guard_refuses_a_text_that_lost_a_comment ... ok
test the_formatted_tree_equals_the_sources ... ok
test formatting_twice_changes_nothing ... ok
test the_corpus_keeps_every_comment ... ok
test the_corpus_keeps_every_tree ... ok
test the_corpus_is_stable_under_a_second_format ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.98s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.43s
     Running tests/msl_format.rs (/var/tmp/piperine-target/debug/deps/msl_format-901b80bf791fef75)
```

`Run print` - exit 0

```
test inherited_lexical_forms_roundtrip ... ok
test string_literals_are_printed_verbatim ... ok
test statement_and_hardware_docs_round_trip ... ok
test field_shorthand_prints_as_written ... ok
test raw_strings_print_as_written ... ok
test printer_preserves_docs_and_attrs ... ok
test roundtrip_ast_equal_over_corpus ... ok
test print_is_idempotent_over_corpus ... ok
test long_chains_print_without_overflow ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.09s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-cli/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.47s
     Running tests/msl_print.rs (/var/tmp/piperine-target/debug/deps/msl_print-a9e8d8de6f1a48b8)
```

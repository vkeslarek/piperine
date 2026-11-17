# msl-cli verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 772c790f (HEAD dbcca0d3 adds docs only)
**Verifier**: independent sub-agent (author != verifier)

Three defects ship with green proofs. Two are the silent gaps batch 7 recorded in `GAPS.md`. Both
are cheap to refuse at the CLI, and one of them changes a `piperine test` verdict, so neither stays
a recorded gap. The third is `clean --cache` deleting a project's sources. The formatter held on
every adversarial input I wrote. Passing-check hiding never changes a `test` verdict or count.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `workspace_shape.rs` `default_members_are_the_green_crates` | yes |
| C2 | `piperine-cli usage.rs` `the_binary_is_named_piperine` | yes |
| C3 | `workspace_shape.rs` `the_root_package_is_a_re_export_and_nothing_else` | yes |
| C4 | `workspace_shape.rs` `host_crates_name_no_deleted_surface` | yes |
| C5 | `piperine-cli usage.rs` `help_lists_exactly_the_seven_commands` | yes |
| C6 | `piperine-cli lib` `every_command_is_built` | yes - exact strings |
| C7 | `piperine-cli usage.rs` `version_prints_the_workspace_version` | yes |
| C8 | `piperine-cli usage.rs` `an_unknown_command_or_flag_exits_2` | yes |
| C9 | `piperine-api project.rs` `discover_takes_the_nearest_manifest_upward` | yes |
| C10 | `piperine-cli run.rs` `the_project_flag_uses_that_directory_and_never_searches` | yes - real binary, exit code and stream asserted |
| C11 | `piperine-cli run.rs` `no_manifest_is_e0170_and_exit_1` | yes - real binary, exit code and stream asserted |
| C12 | `piperine-project manifest.rs` `a_run_table_is_accepted_and_located` | yes |
| C13 | `piperine-lang spec_text.rs` `the_manifest_rule_admits_the_run_table` | yes - the sentence or row named |
| C14 | `piperine-api project.rs` `a_dependencys_run_table_is_ignored` | yes |
| C15 | `piperine-cli run.rs` `a_package_error_is_rendered_with_code_file_and_line` | yes - real binary, exit code and stream asserted |
| C16 | `piperine-api project.rs` `a_bench_is_the_root_crate_over_the_package_and_its_dependencies` | yes |
| C17 | `piperine-api project.rs` `a_run_without_a_bench_roots_at_the_package` | yes |
| C18 | `piperine-api project.rs` `a_lone_file_with_no_manifest_is_a_crate_alone` | yes |
| C19 | `piperine-api project.rs` `a_file_under_a_package_is_its_bench` | yes |
| C20 | `piperine-api project.rs` `a_file_under_src_is_refused_with_p0010` | yes |
| C21 | `piperine-api lib` `a_run_table_reads_its_nineteen_keys` | yes - exact values |
| C22 | `piperine-cli lib` `every_run_key_has_its_flag` | yes - exact strings |
| C23 | `piperine-api lib` `a_flag_overrides_a_key` | yes - exact values |
| C24 | `piperine-api lib` `an_unknown_key_or_a_wrong_type_is_p0010_at_its_span` | yes - exact values |
| C25 | `piperine-api lib` `analysis_is_one_of_four` | yes - exact values |
| C26 | `piperine-api lib` `a_transient_needs_a_positive_stop` | yes - exact values |
| C27 | `piperine-api lib` `a_key_outside_its_analysis_is_p0010` | yes - exact values |
| C28 | `piperine-api project.rs` `a_missing_bench_file_is_p0010_at_the_value` | yes |
| C29 | `piperine-api project.rs` `the_only_run_is_run_without_a_name` | yes |
| C30 | `piperine-cli run.rs` `no_name_with_no_run_or_several_lists_them_and_exits_2` | yes - real binary, exit code and stream asserted |
| C31 | `piperine-cli run.rs` `an_unknown_run_name_lists_the_runs_and_exits_2` | yes - real binary, exit code and stream asserted |
| C32 | `piperine-cli run.rs` `a_lone_file_without_analysis_exits_2` | yes - real binary, exit code and stream asserted |
| C33 | `piperine-api lib` `a_run_name_is_an_identifier` | yes - exact values |
| C34 | `piperine-cli run.rs` `a_single_valued_flag_given_twice_exits_2` | yes - real binary, exit code and stream asserted |
| C35 | `piperine-api session.rs` `the_top_key_names_the_module` | yes |
| C36 | `piperine-api session.rs` `the_one_portless_module_is_the_top` | yes |
| C37 | `piperine-api session.rs` `no_top_or_several_is_p0011_with_candidates` | yes |
| C38 | `piperine-cli lib` `a_diagnostic_has_code_file_line_and_excerpt` | yes - exact strings |
| C39 | `piperine-cli run.rs` `a_type_error_is_rendered_with_its_stage_and_exits_1` | yes - real binary, exit code and stream asserted |
| C40 | `piperine-api session.rs` `a_missing_rustc_names_rustc_and_the_interpreter` | yes |
| C41 | `piperine-api session.rs` `the_interpreter_refusal_names_the_body_and_the_rust_backend` | yes |
| C42 | `piperine-api lib` `the_flag_then_the_variable_then_the_key_then_rust` | yes - exact values |
| C43 | `piperine-cli run.rs` `a_bad_backend_exits_2_for_the_flag_and_1_otherwise` | yes - real binary, exit code and stream asserted |
| C44 | `piperine-api session.rs` `a_run_compiles_once_and_never_during_an_analysis` | yes |
| C45 | `piperine-api session.rs` `a_top_with_discrete_in_ports_runs_with_them_unknown` | yes |
| C46 | `piperine-api session.rs` `a_top_with_a_continuous_port_is_the_front_ends_diagnostic` | yes |
| C47 | `piperine-api analyses.rs` `an_operating_point_is_signal_and_value_in_print_order` | yes - spec tolerances, both backends where named |
| C48 | `piperine-api analyses.rs` `a_transient_has_one_row_per_accepted_step` | yes for its case (tran_rc reaches `stop`), but the criterion fails on a design with no analog node: F2 |
| C49 | `piperine-api probes.rs` `the_default_is_the_tops_own_signals_in_order` | yes |
| C50 | `piperine-api probes.rs` `a_node_a_net_and_a_flow_resolve_relative_or_full` | yes |
| C51 | `piperine-api probes.rs` `a_column_is_named_by_the_full_path` | yes |
| C52 | `piperine-api probes.rs` `an_entry_that_names_nothing_is_e0801_before_any_analysis` | yes |
| C53 | `piperine-api analyses.rs` `the_divider_mid_is_7_5_on_both_backends` | yes - spec tolerances, both backends where named |
| C54 | `piperine-api analyses.rs` `the_rc_step_follows_one_minus_exp_on_both_backends` | yes - spec tolerances, both backends where named |
| C55 | `piperine-cli run.rs` `a_solver_failure_prints_no_table_and_exits_1` | yes - real binary, exit code and stream asserted |
| C56 | `piperine-cli lib` `numbers_have_one_text_each` | yes - exact strings |
| C57 | `piperine-api probes.rs` `a_signal_printed_twice_is_two_columns` | yes |
| C58 | `piperine-api records.rs` `an_emit_gives_one_record_with_time_instance_stream_and_fields` | yes - decoded fields asserted exactly |
| C59 | `piperine-cli lib` `a_record_is_one_line` | yes - exact strings |
| C60 | `piperine-api records.rs` `records_arrive_in_time_order` | yes - decoded fields asserted exactly |
| C61 | `piperine-api records.rs` `a_text_field_is_the_sources_literal` | yes - decoded fields asserted exactly |
| C62 | `piperine-api records.rs` `an_unknown_field_is_unknown` | yes - decoded fields asserted exactly |
| C63 | `piperine-api records.rs` `a_failed_require_is_one_checks_record_with_pass_false` | yes - decoded fields asserted exactly |
| C64 | `piperine-cli run.rs` `records_come_before_the_table` | yes - real binary, exit code and stream asserted |
| C65 | `piperine-api records.rs` `a_design_without_emit_gives_no_record` | yes - decoded fields asserted exactly |
| C66 | `piperine-api records.rs` `a_reserved_stream_keeps_its_name_and_its_site` | yes - decoded fields asserted exactly |
| C67 | `piperine-cli run.rs` `run_prints_header_records_blank_line_and_table` | yes - real binary, exit code and stream asserted |
| C68 | `piperine-cli lib` `a_table_is_left_aligned_two_spaces_no_trailing_space` | yes - exact strings |
| C69 | `piperine-cli run.rs` `a_completed_run_exits_0` | yes - real binary, exit code and stream asserted |
| C70 | `piperine-cli run.rs` `a_failure_before_the_analysis_prints_nothing_on_stdout` | yes - real binary, exit code and stream asserted |
| C71 | `piperine-cli run.rs` `records_stay_when_the_analysis_fails` | yes - real binary, exit code and stream asserted |
| C72 | `piperine-cli run.rs` `stdout_is_the_same_from_two_caches_and_two_directories` | yes - real binary, exit code and stream asserted |
| C73 | `piperine-cli run.rs` `a_lone_file_run_is_named_by_its_stem` | yes - real binary, exit code and stream asserted |
| C74 | `piperine-cli run.rs` `a_first_compile_is_announced_on_stderr` | partial - asserts one stderr line naming the run, cannot tell that it is printed after the compile, not while (F4); check text says `compiling`/`step`, the test reads `compiled`/`op` |
| C75 | `piperine-cli run.rs` `a_closed_stdout_ends_with_exit_1_and_no_panic` | yes - real binary, exit code and stream asserted |
| C76 | `piperine-cli buck.rs` `the_buck_has_its_bench_and_two_runs` | yes (read, not rerun) |
| C77 | `piperine-cli buck.rs` `startup_ends_with_the_reference_at_128` | yes (read, not rerun) |
| C78 | `piperine-cli buck.rs` `startup_prints_no_failed_check` | yes (read, not rerun) |
| C79 | `piperine-cli buck.rs` `idle_keeps_the_mode_off` | yes (read, not rerun) |
| C80 | `piperine-cli buck.rs` `piperine_test_passes_on_the_buck` | yes (read, not rerun) |
| C81 | `piperine-api analyses.rs` `a_dc_sweep_has_one_row_per_point` | yes - on 0..10 by 2.5 (and 3.0, and a downward sweep) rather than the 0..1 by 0.25 the check names; first column and every point asserted |
| C82 | `piperine-api analyses.rs` `an_ac_sweep_has_mag_and_phase_in_degrees` | no - asserts the 1 kHz phase in (-2, 0), not within 1e-2 of -0.5729 (F5); degrees are held by C87 and the 10 MHz bound |
| C83 | `piperine-api lib` `a_dc_sweep_needs_its_four_keys_and_a_step_toward_to` | yes - exact values |
| C84 | `piperine-api lib` `per_decade_gives_round_n_log10_plus_one_points` | yes - exact values |
| C85 | `piperine-api lib` `an_ac_grid_is_exactly_one_of_two_and_its_range_is_ordered` | yes - exact values |
| C86 | `piperine-api analyses.rs` `points_sweeps_linearly` | yes - spec tolerances, both backends where named |
| C87 | `piperine-api analyses.rs` `the_rc_corner_is_minus_3_db_and_minus_45_degrees` | yes - spec tolerances, both backends where named |
| C88 | `piperine-api analyses.rs` `the_diode_sweep_has_37_rows` | yes - spec tolerances, both backends where named |
| C89 | `piperine-api analyses.rs` `a_sweep_of_no_param_is_e0801` | yes - spec tolerances, both backends where named |
| C90 | `piperine-api analyses.rs` `params_are_set_in_order_table_then_flags` | yes - order within the list here, table-before-flags in C23s test |
| C91 | `piperine-api analyses.rs` `a_param_path_that_names_nothing_is_e0801` | yes - spec tolerances, both backends where named |
| C92 | `piperine-api analyses.rs` `a_refused_value_is_e0802_and_nothing_runs` | yes - spec tolerances, both backends where named |
| C93 | `piperine-cli options.rs` `the_divider_with_r2_at_1k_gives_5` | yes |
| C94 | `piperine-cli options.rs` `csv_writes_the_table_and_prints_wrote` | yes |
| C95 | `piperine-cli options.rs` `an_existing_csv_is_replaced` | yes |
| C96 | `piperine-cli options.rs` `an_unwritable_csv_names_the_path_and_exits_1` | yes |
| C97 | `piperine-api lib` `a_print_grid_has_one_row_per_multiple` | yes - exact values |
| C98 | `piperine-api lib` `continuous_is_interpolated_and_discrete_is_held` | yes - exact values |
| C99 | `piperine-cli options.rs` `a_relative_output_path_is_under_the_project` | yes |
| C100 | `piperine-api lib` `a_grid_wider_than_the_run_is_the_row_at_zero` | yes - exact values |
| C101 | `piperine-api records.rs` `keep_matches_a_stream_name_or_its_full_path` | yes - decoded fields asserted exactly |
| C102 | `piperine-api records.rs` `a_filter_entry_that_matches_nothing_is_e0801` | yes - decoded fields asserted exactly |
| C103 | `piperine-cli options.rs` `records_go_to_the_file_as_json_lines_and_not_to_stdout` | yes |
| C104 | `piperine-cli options.rs` `an_existing_records_file_is_replaced` | yes |
| C105 | `piperine-cli lib` `each_value_kind_has_its_json` | yes - exact strings |
| C106 | `piperine-cli options.rs` `csv_and_records_leave_the_header_and_the_wrote_line` | yes |
| C107 | `piperine-cli options.rs` `the_stream_flag_keeps_only_that_stream` | yes |
| C108 | `piperine-cli twins.rs` `the_three_op_twins_run_on_both_backends` | yes - exit 0, header and a table on both backends |
| C109 | `piperine-cli twins.rs` `the_sweep_and_transient_twins_run_on_both_backends` | yes |
| C110 | `piperine-cli twins.rs` `the_two_backends_print_the_same_bytes` | yes - byte equality, no tolerance |
| C111 | `piperine-cli commands.rs` `check_runs_every_stage_and_no_backend` | yes - exact stdout and exit code |
| C112 | `piperine-cli commands.rs` `check_prints_ok_with_the_counts` | yes - exact stdout and exit code |
| C113 | `piperine-cli commands.rs` `check_renders_every_diagnostic_and_exits_1` | yes - exact stdout and exit code |
| C114 | `piperine-cli commands.rs` `check_without_a_run_says_type_checking_needs_one` | yes - exact stdout and exit code |
| C115 | `piperine-cli commands.rs` `check_takes_a_lone_file` | yes - exact stdout and exit code |
| C116 | `piperine-cli commands.rs` `build_prints_one_line_per_run` | yes - the test asserts `built step: cached` (both runs share `Step`), the check text says `compiled`; the test is right |
| C117 | `piperine-cli commands.rs` `a_second_build_is_cached_and_compiles_nothing` | yes - exact stdout and exit code |
| C118 | `piperine-cli commands.rs` `verbose_prints_a_source_path_that_exists` | yes - exact stdout and exit code |
| C119 | `piperine-cli commands.rs` `build_goes_on_after_a_failing_run_and_exits_1` | yes - exact stdout and exit code |
| C120 | `piperine-cli commands.rs` `test_runs_every_run_or_the_named_ones` | yes - exact stdout and exit code |
| C121 | `piperine-cli commands.rs` `test_prints_pass_with_the_check_count` | yes - exact stdout and exit code |
| C122 | `piperine-api lib` `no_false_and_no_unknown_pass_is_a_pass` | yes - exact values |
| C123 | `piperine-cli commands.rs` `test_prints_fail_and_the_failing_records` | yes - exact stdout and exit code |
| C124 | `piperine-api lib` `an_unknown_pass_fails` | yes - exact values |
| C125 | `piperine-cli commands.rs` `test_prints_no_table_and_no_other_record` | yes - exact stdout and exit code |
| C126 | `piperine-cli commands.rs` `a_run_that_does_not_complete_is_a_fail` | yes - exact stdout and exit code |
| C127 | `piperine-cli commands.rs` `the_last_line_counts_and_the_exit_code_follows` | yes - exact stdout and exit code |
| C128 | `piperine-cli commands.rs` `a_project_without_a_run_passes_with_zero` | yes - exact stdout and exit code |
| C129 | `piperine-cli commands.rs` `new_creates_the_four_files_and_says_created` | yes - exact stdout and exit code |
| C130 | `piperine-cli commands.rs` `new_writes_the_rc_example_under_the_new_name` | yes - exact stdout and exit code |
| C131 | `piperine-cli commands.rs` `new_refuses_an_existing_directory` | yes - exact stdout and exit code |
| C132 | `piperine-cli commands.rs` `new_refuses_a_name_the_manifest_rule_forbids` | yes - exact stdout and exit code |
| C133 | `piperine-cli commands.rs` `a_new_project_checks_runs_and_tests` | yes - exact stdout and exit code |
| C134 | `piperine-cli commands.rs` `new_without_a_name_exits_2` | yes - exact stdout and exit code |
| C135 | `piperine-cli commands.rs` `clean_removes_the_target_directory` | yes - exact stdout and exit code |
| C136 | `piperine-cli commands.rs` `clean_cache_removes_the_compiled_cache` | yes - exact stdout and exit code |
| C137 | `piperine-cli commands.rs` `clean_with_nothing_to_remove_exits_0` | yes - exact stdout and exit code |
| C138 | `piperine-cli commands.rs` `clean_touches_nothing_else` | partial - covers a sibling of the cache, not a cache directory that is or holds the project (F3) |
| C139 | `piperine-lang book.rs` `the_cli_chapter_sits_between_compiler_and_cheatsheet` | yes |
| C140 | `piperine-cli book.rs` `the_plot_in_the_chapter_is_the_file_the_binary_writes` | yes |
| C141 | `piperine-cli book.rs` `the_chapter_shows_fmt_and_says_comments_stay` | yes |
| C142 | `piperine-lang book.rs` `the_welcome_chapter_says_a_bench_is_msl` | yes |
| C143 | `piperine-project project.rs` `the_rc_example_resolves_with_no_diagnostic` | yes |
| C144 | `piperine-cli book.rs` `the_chapter_shows_the_rc_project_file_for_file` | yes |
| C145 | `piperine-cli book.rs` `the_rc_output_blocks_are_what_the_binary_prints` | yes - the four full blocks; see F6 for blocks the guard does not reach |
| C146 | `piperine-cli book.rs` `the_buck_block_is_what_the_binary_prints` | yes |
| C147 | `piperine-cli book.rs` `the_chapter_explains_emit_as_print` | yes |
| C148 | `piperine-cli book.rs` `the_chapter_lists_commands_keys_flags_codes` | yes |
| C149 | `piperine-cli book.rs` `the_chapter_stands_alone` | yes |
| C150 | `piperine-lang book.rs` `every_msl_block_parses` | yes |
| C151 | `piperine-lang book.rs` `the_compiler_chapter_says_the_command_line_exists` | yes |
| C152 | `piperine-lang book.rs` `the_cheat_sheet_lists_the_seven_commands` | yes |
| C153 | `piperine-cli docs.rs` `the_instruction_files_say_the_host_crates_build` | yes - text test |
| C154 | `piperine-cli docs.rs` `gaps_drops_what_closed_and_lists_what_is_left` | yes - text test |
| C155 | `piperine-cli docs.rs` `the_roadmap_marks_the_cli_item_done` | yes - text test |
| C156 | `piperine-cli docs.rs` `state_records_the_cli_decision` | yes - text test |
| C157 | `piperine-cli docs.rs` `the_buck_readme_says_how_to_run_it` | yes - text test |
| C158 | `piperine-lang spec_text.rs` `part_viii_defines_a_bench` | yes - the sentence or row named |
| C159 | `piperine-lang spec_text.rs` `the_index_says_benches_are_msl_and_sweeps_are_host` | yes - the sentence or row named |
| C160 | `piperine-lang spec_text.rs` `the_testbench_table_splits_stimulus_and_host_work` | yes - the sentence or row named |
| C161 | `piperine-lang spec_text.rs` `no_spec_text_says_stimulus_is_host_only` | yes - the sentence or row named |
| C162 | `piperine-lang spec_text.rs` `every_error_code_is_defined_exactly_once` | yes - the sentence or row named |
| C163 | `piperine-cli options.rs` `plot_writes_an_svg_and_prints_wrote` | yes |
| C164 | `piperine-cli lib` `one_path_per_signal_with_its_name_and_a_legend` | yes - exact strings |
| C165 | `piperine-api lib` `a_value_maps_linearly_onto_the_plot_area` | yes - exact values |
| C166 | `piperine-cli lib` `each_axis_has_ticks_labels_and_a_title` | yes - exact strings |
| C167 | `piperine-api lib` `ticks_are_1_2_5_times_a_power_of_ten` | yes - exact values |
| C168 | `piperine-api lib` `a_discrete_signal_gets_a_lane_and_no_series` | yes - exact values |
| C169 | `piperine-api lib` `an_ac_plot_has_a_db_panel_over_a_degree_panel_and_a_log_axis` | yes - exact values |
| C170 | `piperine-cli options.rs` `a_plot_draws_every_accepted_step_whatever_the_print_step` | partial - asserts `points > 20` against 6 printed rows, not "as many as the run without print_step" (F5); still fails if the plot drew the grid |
| C171 | `piperine-api lib` `an_operating_point_has_nothing_to_plot` | yes - exact values |
| C172 | `piperine-api lib` `an_unknown_value_breaks_the_run` | yes - exact values |
| C173 | `piperine-cli lib` `the_same_plot_is_the_same_bytes` | yes - exact strings |
| C174 | `piperine-cli options.rs` `an_unwritable_plot_names_the_path_and_exits_1` | yes |
| C175 | `piperine-lang msl_format.rs` `plain_comments_are_listed_with_span_text_and_line_start` | yes - hand-written expected text |
| C176 | `piperine-lang msl_format.rs` `every_plain_comment_comes_out_once_in_order_unchanged` | yes - hand-written expected text |
| C177 | `piperine-lang msl_format.rs` `the_guard_refuses_a_text_that_lost_a_comment` | yes - the guard refuses a doctored text missing one comment, P0004, span of the lost comment |
| C178 | `piperine-lang msl_format.rs` `the_formatted_tree_equals_the_sources` | yes - hand-written expected text |
| C179 | `piperine-lang msl_format.rs` `formatting_twice_changes_nothing` | yes - hand-written expected text |
| C180 | `piperine-lang msl_format.rs` `a_comment_is_anchored_before_after_or_inside` | yes - hand-written expected text |
| C181 | `piperine-lang msl_format.rs` `a_comment_on_its_own_line_stays_before_its_node` | yes - hand-written expected text |
| C182 | `piperine-lang msl_format.rs` `a_trailing_comment_stays_on_its_line` | yes - hand-written expected text |
| C183 | `piperine-lang msl_format.rs` `a_comment_ending_a_block_stays_inside_it` | yes - hand-written expected text |
| C184 | `piperine-lang msl_format.rs` `an_unreachable_comment_marks_its_statement_verbatim` | yes - hand-written expected text |
| C185 | `piperine-lang msl_format.rs` `a_statement_with_an_inner_comment_is_kept_as_written` | yes - hand-written expected text |
| C186 | `piperine-lang msl_format.rs` `blank_lines_collapse_to_one` | yes - hand-written expected text |
| C187 | `piperine-lang msl_format.rs` `a_source_that_does_not_parse_gives_diagnostics_and_no_text` | yes - hand-written expected text |
| C188 | `piperine-lang msl_format.rs` `the_corpus_keeps_every_comment` | yes - 277 sources; my 50 adversarial inputs (comments before braces, in params, after `else`, in calls, strings with `//`, CRLF, nested blocks, EOF without newline, tabs, unicode) kept every comment, the tree and the fixed point, none refused |
| C189 | `piperine-lang msl_format.rs` `the_corpus_keeps_every_tree` | yes - hand-written expected text |
| C190 | `piperine-lang msl_format.rs` `the_corpus_is_stable_under_a_second_format` | yes - hand-written expected text |
| C191 | `piperine-lang msl_print.rs` `print_is_idempotent_over_corpus` | yes - target unedited |
| C192 | `piperine-lang msl_print.rs` `roundtrip_ast_equal_over_corpus` | yes - target unedited |
| C193 | `piperine-cli commands.rs` `fmt_rewrites_what_differs_and_names_it` | yes - exact stdout and exit code |
| C194 | `piperine-cli commands.rs` `fmt_takes_files` | yes - exact stdout and exit code |
| C195 | `piperine-cli commands.rs` `fmt_check_writes_nothing_and_exits_1_on_a_difference` | yes - exact stdout and exit code |
| C196 | `piperine-cli commands.rs` `fmt_skips_a_file_that_does_not_parse_and_exits_1` | yes - exact stdout and exit code |
| C197 | `piperine-cli commands.rs` `fmt_on_formatted_files_is_silent` | yes - exact stdout and exit code |
| C198 | `piperine-cli commands.rs` `fmt_check_passes_on_the_rc_example_and_on_a_new_project` | yes - exact stdout and exit code |

## Findings

Severity order: silent > broken > loud.

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (silent, CLI-09, CLI-16 AC 2/3): a `dc` run never delivers its records, so `piperine test` passes a dc run whose `require` fails at every point. Must fail loud now, not stay a gap | Scratch project, `Zero` monitor `on at(0.0) { require("at_zero", v(a) <= 0.1) }` over a divider at 0.5 V: `piperine test` prints `FAIL op (1 of 1 checks failed)`, `PASS sweep (0 checks)`, `FAIL tr (...)`. `run sweep` prints no record and exits 0. `Session::analysis` `Analysis::Dc` solves `circuit.dc(Context::default())` and drops `delivery` | Deliver each point's records through the sink (a solver with the sink per point), or refuse in `Session::open` a `dc` run of a design with any record stream (`Streams` not empty), with a code and a message naming the gap. Add a check: a dc run with a failing `require` is FAIL or refused, never PASS | |
| F2 (silent, CLI-05 AC 2): a transient of a design with no analog node prints one row at t=0 holding the end-of-run values and exits 0. Must fail loud now | Book `blinky.msl`, `piperine run blinky.msl --analysis transient --stop 3e-6`: table `0.000000e0  false  3`, exit 0. `--plot` also exits 0 with a one-point plot. C48 tests only `tran_rc`. The Book listing shows lines 1-4 only (`data-cli-lines`), so the guard never sees the table | In `Session` transient: when the last kept step is before `stop` (beyond the grid's slack), return a `Failure` naming the gap, so no table and exit 1. Change the Book listing to the refusal or drop the table claim. Add a check over a digital-only design | |
| F3 (broken, data loss, CLI-18 AC 4): `piperine clean --cache` runs `remove_dir_all` on whatever `PIPERINE_CACHE_DIR` names, the project included | Copy of `examples/rc`, `PIPERINE_CACHE_DIR=. piperine clean --cache` in it: every file of the project is gone, then `error: .: Invalid argument`, exit 1. `PIPERINE_CACHE_DIR=$HOME` would empty the home directory. C138 covers only a sibling of the cache | Canonicalize the cache path. Refuse (exit 1, nothing removed) when it equals or contains the project directory or the working directory, or lacks the transpiler's cache layout (its `rust/` entry). Add the case to `clean_touches_nothing_else` | |
| F4 (loud, CLI-07 AC 8): the compile line is printed after `rustc` finishes, not while it runs | `run.rs` writes `compiled run ... and cached it` after `Session::open` returns. Its comment says so. On the buck the user waits minutes with no line. C74 cannot tell the order | Print `compiling run <name> ...` before the build on a cache miss: a probe of the cache or a callback from `Session::open`. The test reads the line from a wrapper `rustc` that writes a marker, and asserts the line comes first | |
| F5 (assertions looser than their check): C82 and C170 | C82: `phase < 0.0 && phase > -2.0` at 1 kHz for "within 1e-2 of -0.5729". C170: `points > 20` for "as many as the run without `print_step`" | Assert `(phase + 0.5729).abs() <= 1e-2`. Count the points of a run without `--print-step` and assert equality | |
| F6 (latent escape, Book guard): `book.rs` checks a `data-cli-output` block inside chapter `cli` only when its project is `rc` or `buck`. The outside test skips the chapter. `data-cli-lines` can cut a wrong part out of a listing | `the_rc_output_blocks_are_what_the_binary_prints` and `the_buck_block_is_what_the_binary_prints` filter by project. `every_cli_listing_of_the_book_is_what_the_binary_prints` excludes `[from, to)` of `cli`. Today every block of the chapter is `rc` or `buck`. The Blinky table (F2) is cut out by `data-cli-lines="1-4"` | One test checks every `data-cli-output` block of the whole Book, any project. Keep the per-AC presence assertions. Record the line ranges that are allowed to exclude a table, or fix F2 so none needs to | |

Not findings, read and accepted:

- **Out-of-scope edits in `piperine-transpile`.** `d53ed59e` `has_digital_form` also counts drives, events, reactions and streams. It is one predicate that both `new` and `digital` use, and `carrierless.rs` holds it. `bb81fbb8` makes the probe name unique per call through the existing `STAGING` counter. Both are minimal and correct.
- **`suite_hygiene` floor.** The floor dropped from 15 to 10 `mod.rs` files after T4 deleted five. The test now also requires three named files, so the guard is no weaker.
- **Tolerances.** No ngspice tolerance changed. `tests/support/ngspice.rs` and `piperine-parity` are not in the diff. The twins compare bytes.
- **Records and filtering.** `test` collects every record, clears `streams` and judges with `Verdict` (`pass != true` fails). The terminal hides `pass == true` only, the exact complement, so hiding never moves a verdict or a count. `--records` bypasses the terminal filter. `--stream` filters at decode, for the terminal and the file alike, as CLI-12 AC 3 says.
- **Exit codes and streams.** Probed: no command, an extra argument, a repeated flag and `new` with no name each exit 2 on stderr. An unknown run in `test` or `run` exits 2. `P0010`, `E0170`, a refused interpreter and a missing file exit 1 with empty stdout. A failure after records keeps them.
- **Formatter.** Two own-example rounds with 50 inputs (see the C188 row): every output kept the comment list, kept the tree and reached its fixed point. Nothing was refused. `fmt` rewrites CRLF files to LF, which no criterion forbids.

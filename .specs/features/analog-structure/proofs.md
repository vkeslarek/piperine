# analog-structure proofs

**HEAD**: 89c9ec3d
**Tree**: clean
**Shell**: /bin/sh
**Result**: 97 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sswitch / test a_flow_contribution_under_an_if_on_a_probe_gives_one_volt ... ok` | pass |
| C2 | `run sswitch / test a_match_on_a_carrier_adds_only_the_selected_arm ... ok` | pass |
| C3 | `run elabstruct / test a_contribution_in_a_nested_block_lowers_as_at_the_top ... ok` | pass |
| C4 | `run sswitch / test an_untaken_contribution_leaves_the_branch_with_zero_flow ... ok` | pass |
| C5 | `run sswitch / test a_bound_step_under_an_if_caps_only_the_steps_that_executed_it ... ok` | pass |
| C5 | `run watch / test a_cap_of_infinity_caps_nothing ... ok` | pass |
| C6 | `run elabstruct / test a_contribution_in_a_closure_is_refused_at_parse ... ok` | pass |
| C7 | `run elabstruct / test a_probe_in_the_selector_of_a_potential_capable_branch_is_e0414 ... ok` | pass |
| C8 | `run elabstruct / test a_selector_through_a_value_input_is_judged_per_placement ... ok` | pass |
| C9 | `run elabstruct / test params_only_selectors_fix_the_kind_and_mark_the_params_structural ... ok` | pass |
| C10 | `run elabstruct / test a_fixed_zero_potential_branch_merges_its_nodes ... ok` | pass |
| C10 | `run parop / test diode_rs_with_rs_zero_merges_and_matches_rdiode ... ok` | pass |
| C11 | `run elabstruct / test params_only_selectors_enabling_both_kinds_are_e0415 ... ok` | pass |
| C12 | `run elabstruct / test a_selector_on_a_carrier_or_the_environment_keeps_the_flow_unknown ... ok` | pass |
| C13 | `run sswitch / test the_switch_shorts_at_one_microsecond ... ok` | pass |
| C14 | `run sswitch / test a_switch_changes_kind_at_the_step_after_the_instant ... ok` | pass |
| C15 | `run sswitch / test both_kinds_enabled_in_one_solve_fail_with_e0418 ... ok` | pass |
| C15 | `run tstruct / test both_kinds_enabled_return_e0418_naming_the_branch ... ok` | pass |
| C16 | `run sswitch / test an_ac_sweep_takes_the_switch_kind_with_static_false ... ok` | pass |
| C17 | `run partran / test uic_rc ... ok` | pass |
| C18 | `run sswitch / test capic_has_the_admittance_of_a_capacitor_in_ac ... ok` | pass |
| C19 | `run sderiv / test bounded_holds_its_state_at_the_bound ... ok` | pass |
| C20 | `run smodules / test integ_holds_ic_while_reset_then_integrates ... ok` | pass |
| C21 | `run smodules / test integ_is_one_over_j_omega_in_ac ... ok` | pass |
| C22 | `run smodules / test integ_holds_its_result_at_ic_at_the_operating_point ... ok` | pass |
| C23 | `run elabstruct / test the_stdlib_idt_waits_for_generics ... ok` | pass |
| C24 | `run elabstruct / test the_water_model_waits_for_generics ... ok` | pass |
| C25 | `run elabstruct / test the_section_18_diode_waits_for_params_and_io ... ok` | pass |
| C25 | `run parop / test diode_rs ... ok` | pass |
| C26 | `run docs / test gaps_keeps_l40_for_the_spec_models_that_wait ... ok` | pass |
| C27 | `run sswitch / test a_limit_under_an_untaken_branch_neither_vetoes_nor_moves ... ok` | pass |
| C28 | `run sswitch / test a_junction_enabled_after_the_operating_point_reaches_the_rdiode_solution ... ok` | pass |
| C29 | `run sswitch / test a_limit_reads_a_carrier_through_known_or ... ok` | pass |
| C30 | `run sswitch / test an_ac_stim_under_an_if_drives_only_when_enabled ... ok` | pass |
| C31 | `run sswitch / test an_ac_stim_reads_its_magnitude_from_a_carrier ... ok` | pass |
| C32 | `run partran / test tran_rc_with_a_scaled_derivative ... ok` | pass |
| C32 | `run parac / test ac_rc_with_a_scaled_derivative ... ok` | pass |
| C33 | `run sderiv / test a_derivative_times_a_probe_is_k_v_dv_dt ... ok` | pass |
| C34 | `run elabstruct / test a_body_let_of_ddt_read_as_an_addend_gives_the_inline_rows ... ok` | pass |
| C35 | `run sderiv / test an_event_reads_the_derivative_of_the_step ... ok` | pass |
| C36 | `run plan / test a_derivative_abstol_names_its_pin ... ok` | pass |
| C36 | `run sderiv / test a_looser_derivative_abstol_takes_fewer_steps ... ok` | pass |
| C37 | `run smodules / test a_value_input_of_a_module_without_a_result_drives_its_contribution ... ok` | pass |
| C38 | `run smodules / test a_value_input_is_differentiated_in_ac ... ok` | pass |
| C39 | `run smodules / test a_value_input_reads_the_callers_carrier_from_its_change ... ok` | pass |
| C40 | `run typestruct / test a_continuous_argument_read_by_a_callee_definition_is_e0336 ... ok` | pass |
| C41 | `run smodules / test a_value_input_read_in_a_reaction_is_the_callers_value_at_that_instant ... ok` | pass |
| C42 | `run smodules / test lag_follows_one_minus_exp ... ok` | pass |
| C43 | `run smodules / test lag_is_a_first_order_low_pass_in_ac ... ok` | pass |
| C44 | `run elabstruct / test two_placements_with_equal_arguments_are_two_instances ... ok` | pass |
| C45 | `run smodules / test a_lag_under_an_untaken_branch_keeps_integrating ... ok` | pass |
| C46 | `run smodules / test a_named_lag_is_one_instance_for_analog_and_its_event ... ok` | pass |
| C47 | `run smodules / test slew_bounds_the_rate_and_tracks_at_the_operating_point ... ok` | pass |
| C48 | `run smodules / test feedback_through_integ_decays_exponentially ... ok` | pass |
| C49 | `run typestruct / test a_cycle_with_a_direct_dependence_at_every_result_is_e0353 ... ok` | pass |
| C50 | `run typestruct / test feedback_through_an_unknown_of_a_module_is_accepted ... ok` | pass |
| C51 | `run sreach / test the_parent_probes_a_pub_node_of_its_child ... ok` | pass |
| C52 | `run sreach / test the_parent_loads_a_pub_node_of_its_child ... ok` | pass |
| C53 | `run sreach / test the_parent_reads_a_pub_carrier_of_its_child ... ok` | pass |
| C54 | `run sreach / test the_parent_reads_a_pub_param_and_a_pub_let_of_its_child ... ok` | pass |
| C55 | `run typestruct / test inst_name_of_an_item_that_is_not_pub_is_e0343 ... ok` | pass |
| C56 | `run typestruct / test a_commit_to_a_childs_carrier_is_e0222 ... ok` | pass |
| C57 | `run sparams / test a_body_let_as_a_param_value_sets_the_corner ... ok` | pass |
| C58 | `run elabstruct / test a_param_value_let_may_read_another_let ... ok` | pass |
| C59 | `run elabstruct / test a_param_value_let_that_reads_a_probe_is_e0321 ... ok` | pass |
| C60 | `run apibind / test a_binding_of_k_recomputes_the_scale_a_body_let_derives ... ok` | pass |
| C61 | `run sreach / test inflow_sums_the_flows_of_the_modules_branches_at_the_port ... ok` | pass |
| C62 | `run sreach / test inflow_includes_a_childs_branch_on_the_port ... ok` | pass |
| C63 | `run sreach / test inflow_is_differentiated_in_ac ... ok` | pass |
| C64 | `run typestruct / test inflow_of_a_node_that_is_not_a_port_is_e0243 ... ok` | pass |
| C65 | `run typestruct / test inflow_on_the_left_of_a_contribution_is_e0410 ... ok` | pass |
| C66 | `run apibind / test a_timed_set_fires_changed_once_at_its_time ... ok` | pass |
| C67 | `run apibind / test prev_of_a_param_reads_its_own_value_then_the_value_before_a_set ... ok` | pass |
| C68 | `run apibind / test a_binding_before_the_run_fires_no_changed ... ok` | pass |
| C69 | `run apibind / test an_initial_value_over_a_param_follows_its_binding ... ok` | pass |
| C70 | `run sparams / test a_node_initial_value_over_a_param_holds_the_operating_point ... ok` | pass |
| C71 | `run typestruct / test an_initial_value_that_reads_a_carrier_is_e0210 ... ok` | pass |
| C72 | `run elabstruct / test a_childs_own_signal_is_bound_to_own_in_the_authored_child ... ok` | pass |
| C73 | `run irstruct / test an_own_terminal_prints_and_round_trips ... ok` | pass |
| C74 | `run elabstruct / test a_design_with_own_terminals_passes_check ... ok` | pass |
| C75 | `run frozen / test the_interpreter_refuses_each_analog_structure_form_by_name ... ok` | pass |
| C76 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C76 | `run twins / test the_two_backends_print_the_same_bytes ... ok` | pass |
| C76 | `run tgolden / test the_five_m1_bodies_print_as_the_golden_files ... ok` | pass |
| C77 | `run partran / test tran_rc ... ok` | pass |
| C77 | `run parac / test ac_rc ... ok` | pass |
| C77 | `run parop / test divider ... ok` | pass |
| C77 | `run pardc / test diode_iv ... ok` | pass |
| C77 | `run corpus / test every_twin_has_a_cir_and_a_test ... ok` | pass |
| C77 | `git diff --exit-code origin/main -- crates/piperine-parity/src/contract.rs` | pass |
| C78 | `run partran / test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok` | pass |
| C78 | `run parop / test diode_rs_runs_and_prints_the_skip_line_without_ngspice ... ok` | pass |
| C79 | `run docs / test gaps_drops_what_analog_structure_closed ... ok` | pass |
| C79 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| Build | `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piper` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run sswitch` - exit 0

```
test both_kinds_enabled_in_one_solve_fail_with_e0418 ... ok
test an_ac_stim_under_an_if_drives_only_when_enabled ... ok
test capic_has_the_admittance_of_a_capacitor_in_ac ... ok
test a_flow_ac_stim_under_a_statement_if_drives_only_when_enabled ... ok
test a_limit_under_an_untaken_branch_neither_vetoes_nor_moves ... ok
test two_enabled_flow_arms_sum ... ok
test a_bound_step_under_an_if_caps_only_the_steps_that_executed_it ... ok
test a_switch_changes_kind_at_the_step_after_the_instant ... ok
test a_junction_enabled_after_the_operating_point_reaches_the_rdiode_solution ... ok
test the_switch_shorts_at_one_microsecond ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.76s

    Finished `test` profile [unoptimized] target(s) in 0.23s
     Running tests/source_switch.rs (/var/tmp/piperine-target-b/debug/deps/source_switch-f80a8a81fdfe14c7)
```

`Run elabstruct` - exit 0

```
test two_instances_with_different_key_values_get_two_bodies ... ok
test params_only_selectors_enabling_both_kinds_are_e0415 ... ok
test two_placements_with_equal_arguments_are_two_instances ... ok
test params_only_selectors_fix_the_kind_and_mark_the_params_structural ... ok
test a_probe_in_the_selector_of_a_potential_capable_branch_is_e0414 ... ok
test a_selector_on_a_carrier_or_the_environment_keeps_the_flow_unknown ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.79s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-ir)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-plan)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-lang)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 23.49s
     Running tests/elab_structure.rs (/var/tmp/piperine-target-b/debug/deps/elab_structure-8d084d852506f5e7)
```

`Run watch` - exit 0

```
test a_cap_of_minus_infinity_or_nan_is_e0509 ... ok
test accept_fails_with_e0509_on_an_invalid_cap ... ok
test accept_reports_the_smallest_cap ... ok
test an_instance_lists_its_terms_and_var_watches_and_reads_a_term ... ok
test an_infinite_cross_watch_fails_the_solve ... ok
test a_nan_cross_watch_is_unknown_and_never_fires ... ok
test a_tolerance_the_discrete_side_has_not_written_is_the_default ... ok
test rising_reports_a_sign_change_without_accepting ... ok
test samples_are_the_continuous_pins_reactions_read ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-analog-solver)
    Finished `test` profile [unoptimized] target(s) in 5.98s
     Running tests/assembly_watch.rs (/var/tmp/piperine-target-b/debug/deps/assembly_watch-eea1d8e19770cb42)
```

`Run parop` - exit 0

```
running 6 tests
test diode_rs ... ok
test isrc_r ... ok
test diode_rs_runs_and_prints_the_skip_line_without_ngspice ... ok
test divider ... ok
test diode_series ... ok
test diode_rs_with_rs_zero_merges_and_matches_rdiode ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.92s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_op.rs (/var/tmp/piperine-target-b/debug/deps/parity_op-eac8afb1dbfbcbd7)
```

`Run tstruct` - exit 0

```
test a_derivative_abstol_function_writes_its_pin ... ok
test a_derivative_read_is_its_rate_pin ... ok
test a_switch_with_neither_kind_has_zero_flow ... ok
test a_param_change_fires_its_event ... ok
test a_disabled_limit_writes_its_last_value ... ok
test a_derivative_state_is_written_outside_every_condition ... ok
test both_kinds_enabled_return_e0418_naming_the_branch ... ok
test an_executed_cap_that_is_not_finite_and_positive_is_e0509 ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.96s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 1.03s
     Running tests/structure.rs (/var/tmp/piperine-target-b/debug/deps/structure-681ad2ab176c04ac)
```

`Run partran` - exit 0

```
running 9 tests
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_corner ... ok
test tran_jump ... ok
test tran_floor ... ok
test uic_rc ... ok
test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
test tran_rc_with_a_scaled_derivative ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.46s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/parity_tran.rs (/var/tmp/piperine-target-b/debug/deps/parity_tran-1f68c93d92106e3e)
```

`Run sderiv` - exit 0

```
running 4 tests
test bounded_holds_its_state_at_the_bound ... ok
test a_derivative_times_a_probe_is_k_v_dv_dt ... ok
test an_event_reads_the_derivative_of_the_step ... ok
test a_looser_derivative_abstol_takes_fewer_steps ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.44s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_derivative.rs (/var/tmp/piperine-target-b/debug/deps/source_derivative-20faf13e9bf3104f)
```

`Run smodules` - exit 0

```
test lag_is_a_first_order_low_pass_in_ac ... ok
test integ_holds_ic_while_reset_then_integrates ... ok
test a_value_input_reads_the_callers_carrier_from_its_change ... ok
test feedback_through_integ_decays_exponentially ... ok
test a_named_lag_event_lands_within_ttol_of_the_computed_crossing ... ok
test lag_follows_one_minus_exp ... ok
test a_value_input_read_in_a_reaction_is_the_callers_value_at_that_instant ... ok
test a_lag_under_an_untaken_branch_keeps_integrating ... ok
test a_named_lag_is_one_instance_for_analog_and_its_event ... ok
test slew_bounds_the_rate_and_tracks_at_the_operating_point ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.28s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_modules.rs (/var/tmp/piperine-target-b/debug/deps/source_modules-196f4d33439a0ca3)
```

`Run docs` - exit 0

```
test the_roadmap_lists_what_the_long_run_leaves ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test gaps_drops_what_closed_and_lists_what_is_left ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test the_roadmap_and_state_record_analog_structure ... ok
test gaps_drops_what_root_e2e_closed ... ok
test gaps_drops_what_analog_structure_closed ... ok
test the_roadmap_and_state_record_root_e2e ... ok
test gaps_drops_what_msl_fixes_closed ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-93bbd657b5852db6)
```

`Run parac` - exit 0

```
running 2 tests
test ac_rc_with_a_scaled_derivative ... ok
test ac_rc ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-b/debug/deps/parity_ac-ddd91a46c3793786)
```

`Run plan` - exit 0

```
test a_derivative_abstol_names_its_pin ... ok
test a_derivative_read_is_a_differentiable_pin_read ... ok
test a_derivative_site_has_a_rate_pin_and_two_terms ... ok
test a_switch_equation_is_not_affine ... ok
test a_guarded_limit_reads_its_enabled_sources ... ok
test a_switch_branch_has_a_flow_pin_and_an_equation_term ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 10.00s
     Running tests/structure_shape.rs (/var/tmp/piperine-target-b/debug/deps/structure_shape-cd758639902ad757)
```

`Run typestruct` - exit 0

```
test inflow_on_the_left_of_a_contribution_is_e0410 ... ok
test inflow_of_a_node_that_is_not_a_port_is_e0243 ... ok
test inst_name_of_an_item_that_is_not_pub_is_e0343 ... ok
test a_commit_to_a_childs_carrier_is_e0222 ... ok
test a_continuous_argument_read_by_a_callee_definition_is_e0336 ... ok
test feedback_through_an_unknown_of_a_module_is_accepted ... ok
test a_cycle_with_a_direct_dependence_at_every_result_is_e0353 ... ok
test a_discrete_argument_read_by_a_callee_definition_is_accepted ... ok
test an_initial_value_that_reads_a_carrier_is_e0210 ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.85s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.52s
     Running tests/types_structure.rs (/var/tmp/piperine-target-b/debug/deps/types_structure-78e56499b09a151b)
```

`Run sreach` - exit 0

```
running 9 tests
test inflow_includes_a_childs_branch_on_the_port ... ok
test the_parent_reads_a_pub_param_and_a_pub_let_of_its_child ... ok
test the_parent_probes_a_pub_node_of_its_child ... ok
test inflow_sums_the_flows_of_the_modules_branches_at_the_port ... ok
test the_parent_loads_a_pub_node_of_its_child ... ok
test inflow_is_differentiated_in_ac ... ok
test inflow_of_a_port_held_at_zero_is_not_merged_away ... ok
test the_parent_reads_a_pub_carrier_of_its_child_in_a_reaction_and_a_definition ... ok
test the_parent_reads_a_pub_carrier_of_its_child ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.23s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_reach.rs (/var/tmp/piperine-target-b/debug/deps/source_reach-e3e0d3243e8f2da5)
```

`Run sparams` - exit 0

```
running 3 tests
test a_body_let_as_a_param_value_sets_the_corner ... ok
test a_node_initial_value_over_a_param_starts_newton_in_an_operating_point ... ok
test a_node_initial_value_over_a_param_holds_the_operating_point ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_params.rs (/var/tmp/piperine-target-b/debug/deps/source_params-a40c966bba2b4980)
```

`Run apibind` - exit 0

```
running 8 tests
test prev_of_a_param_reads_its_own_value_then_the_value_before_a_set ... ok
test a_timed_set_fires_changed_once_at_its_time ... ok
test a_binding_before_the_run_fires_no_changed ... ok
test a_before_run_binding_of_a_plain_param_is_still_the_hosts_write ... ok
test a_binding_of_k_recomputes_the_scale_a_body_let_derives ... ok
test a_write_of_a_structural_param_after_open_is_e0803 ... ok
test a_before_run_binding_of_a_structural_param_selects_its_variant ... ok
test an_initial_value_over_a_param_follows_its_binding ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.04s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/bindings.rs (/var/tmp/piperine-target-b/debug/deps/bindings-91de6fac1a40cbdb)
```

`Run irstruct` - exit 0

```
test a_limit_under_a_condition_needs_enabled ... ok
test an_own_terminal_that_names_a_port_is_broken ... ok
test merging_a_node_into_ground_removes_it ... ok
test merging_two_nodes_leaves_one_and_rewrites_every_instance ... ok
test a_param_previous_read_is_typed_as_the_param ... ok
test a_top_level_limit_has_no_enabled ... ok
test contributions_lists_each_with_its_enclosing_conditions ... ok
test an_own_terminal_prints_and_round_trips ... ok
test the_structure_records_print_and_round_trip ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-ir)
    Finished `test` profile [unoptimized] target(s) in 2.84s
     Running tests/structure.rs (/var/tmp/piperine-target-b/debug/deps/structure-32d631cbe4c999d2)
```

`Run frozen` - exit 0

```
running 5 tests
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 0.90s
     Running tests/frozen.rs (/var/tmp/piperine-target-b/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.13s

    Finished `test` profile [unoptimized] target(s) in 0.23s
     Running tests/twins.rs (/var/tmp/piperine-target-b/debug/deps/twins-9b01bd7200d69219)
```

`Run tgolden` - exit 0

```
running 7 tests
test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok
test a_body_without_analog_prints_no_terms_function ... ok
test every_item_sits_under_its_path_and_origin ... ok
test the_five_m1_bodies_print_as_the_golden_files ... ok
test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok
test transpiling_twice_gives_the_same_bytes ... ok
test a_design_with_nothing_to_run_has_no_module ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-analog-structure/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 0.63s
     Running tests/golden.rs (/var/tmp/piperine-target-b/debug/deps/golden-ab85f3ee5d658ee1)
```

`Run pardc` - exit 0

```
running 2 tests
test diode_iv ... ok
test diode_rev ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.54s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/parity_dc.rs (/var/tmp/piperine-target-b/debug/deps/parity_dc-f35f75664572bfd4)
```

`Run corpus` - exit 0

```
running 8 tests
test tests_ngspice_holds_no_twin ... ok
test a_literal_tolerance_in_a_parity_target_is_reported ... ok
test the_guards_report_planted_text ... ok
test no_parity_target_compares_against_a_literal ... ok
test every_twin_has_its_files_in_one_scenario_directory ... ok
test a_scenario_with_benches_holds_a_manifest ... ok
test the_layout_guards_report_planted_trees ... ok
test every_twin_has_a_cir_and_a_test ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-b/debug/deps/parity_corpus-0b598b4d30604325)
```

`Run clibook` - exit 0

```
test blinky_runs_to_its_stop_in_the_book ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test the_buck_block_is_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 119.03s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-a1f18a8423c7ed6b)
```

`git diff --exit-code origin/main -- crates/piperine-parity/src/contract.rs` - exit 0

```
(no output)
```

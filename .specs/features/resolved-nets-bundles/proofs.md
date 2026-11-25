# resolved-nets-bundles proofs

**HEAD**: f87c79299
**Tree**: clean
**Shell**: /bin/sh
**Result**: 72 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run pombundles / test a_struct_of_carriers_is_a_bundle ... ok` | pass |
| C2 | `run pombundles / test one_driver_fields_carry_a_direction_and_no_other_field_does ... ok` | pass |
| C3 | `run pombundles / test a_struct_mixing_carrier_and_value_fields_is_e0260 ... ok` | pass |
| C4 | `run pombundles / test a_misplaced_or_missing_field_direction_is_e0261 ... ok` | pass |
| C5 | `run pombundles / test flip_out_and_nested_directions_are_e0262 ... ok` | pass |
| C6 | `run pombundles / test a_bundle_declared_as_a_value_is_e0264 ... ok` | pass |
| C7 | `run typesbundles / test a_producer_drives_out_fields_and_reads_in_fields ... ok` | pass |
| C8 | `run typesbundles / test flip_swaps_every_one_driver_direction_nested_included ... ok` | pass |
| C9 | `run typesbundles / test an_observer_reads_every_one_driver_field ... ok` | pass |
| C10 | `run elabbundles / test two_producers_of_one_field_is_e0220_naming_its_path ... ok` | pass |
| C11 | `run typesbundles / test driving_an_input_field_is_e0222 ... ok` | pass |
| C12 | `run sourcebundles / test a_producer_consumer_and_observer_run_three_beats ... ok` | pass |
| C13 | `run sourcebundles / test a_bundle_carrier_connects_field_by_field ... ok` | pass |
| C14 | `run sourcebundles / test a_cell_literal_connects_each_named_field ... ok` | pass |
| C15 | `run typesbundles / test a_literal_field_of_another_nature_is_e0263 ... ok` | pass |
| C16 | `run typesbundles / test a_literal_outside_a_connection_or_a_bundle_operand_is_e0264 ... ok` | pass |
| C17 | `run sourcebundles / test a_diff_bundle_connects_its_nodes_like_ports ... ok` | pass |
| C18 | `run sourcebundles / test field_commits_definitions_and_methods_follow_section_5 ... ok` | pass |
| C19 | `run typesbundles / test a_carrier_method_on_a_field_applies_to_that_field ... ok` | pass |
| C20 | `run pombundles / test a_carrier_method_on_a_whole_bundle_is_e0211 ... ok` | pass |
| C21 | `run typesbundles / test self_on_a_bundle_is_a_read_only_handle ... ok` | pass |
| C22 | `run sourcebundles / test vd_on_a_diff_bundle_gives_the_differential_voltage ... ok` | pass |
| C23 | `run sourcebundles / test beat_fires_on_rising_edges_with_valid_and_ready ... ok` | pass |
| C24 | `run typesbundles / test a_write_through_a_bundle_handle_is_e0133 ... ok` | pass |
| C25 | `run sourcebundles / test nested_bundles_apply_flip_recursively ... ok` | pass |
| C26 | `run sourcebundles / test every_view_may_drive_a_resolved_field ... ok` | pass |
| C27 | `run sourcebundles / test a_generic_bundle_is_instantiated_per_type_argument ... ok` | pass |
| C28 | `run sourcebundles / test an_array_of_bundles_connects_each_element_field_by_field ... ok` | pass |
| C29 | `run apimetadata / test a_bundle_field_is_named_by_its_dotted_path ... ok` | pass |
| C30 | `run pomreflect / test a_bundle_port_reflects_its_type_and_view ... ok` | pass |
| C31 | `run docs / test gaps_no_longer_lists_bundles ... ok` | pass |
| C32 | `run speccodes / test no_owned_code_is_raised ... ok` | pass |
| C33 | `run typesbundles / test a_bundle_port_is_not_a_value_input ... ok` | pass |
| C34 | `run booknatures / test the_stream_bundle_and_its_slice_run ... ok` | pass |
| C37 | `run bookmodules / test the_memory_bank_of_bundle_cells_runs ... ok` | pass |
| C37 | `run bookcheatsheet / test the_template_card_runs ... ok` | pass |
| C35 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C36 | `run elabbundles / test a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path ... ok` | pass |
| C38 | `run typesbundles / test an_element_index_that_is_a_loop_variable_is_an_element_of_the_array ... ok` | pass |
| C38 | `run elabbundles / test a_loop_variable_index_folds_to_the_element_of_each_iteration ... ok` | pass |
| C38 | `run elabbundles / test a_loop_that_gives_one_element_twice_is_e0220_naming_it ... ok` | pass |
| C38 | `run sourcebundles / test an_element_index_that_is_a_loop_variable_folds_per_iteration ... ok` | pass |
| C39 | `run typesbundles / test an_array_of_carriers_as_a_field_is_driven_and_read_by_index ... ok` | pass |
| C39 | `run elabbundles / test an_array_field_of_carriers_binds_whole_and_is_written_by_element ... ok` | pass |
| C39 | `run sourcebundles / test an_array_of_carriers_as_a_field_is_driven_and_read_by_index ... ok` | pass |
| C40 | `run typesbundles / test a_constant_parameter_in_an_array_length_of_a_field_is_replaced_by_its_argument ... ok` | pass |
| C40 | `run elabbundles / test a_constant_parameter_in_an_array_length_of_a_field_is_the_argument_of_each_port ... ok` | pass |
| C40 | `run sourcebundles / test a_constant_parameter_in_an_array_length_of_a_field_gives_its_argument_of_carriers ...` | pass |
| C41 | `run typesbundles / test an_array_of_bundles_of_a_generic_length_is_typed_per_instance ... ok` | pass |
| C41 | `run elabbundles / test an_array_of_bundles_of_a_generic_length_has_the_length_of_each_instance ... ok` | pass |
| C41 | `run elabbundles / test a_computed_length_of_an_array_of_bundles_is_folded_per_instance ... ok` | pass |
| C41 | `run sourcebundles / test an_array_of_bundles_of_a_generic_length_runs_with_each_length ... ok` | pass |
| C41 | `run sourcebundles / test a_generation_loop_drives_each_element_of_an_array_of_bundles_by_its_index ... ok` | pass |
| C41 | `run sourcebundles / test an_array_of_bundles_of_a_computed_length_runs ... ok` | pass |
| C41 | `run sourcebundles / test an_array_of_flipped_streams_completes_a_handshake_per_lane ... ok` | pass |
| C42 | `run typesbundles / test two_producers_of_one_element_of_an_array_of_bundles_is_e0220_naming_it ... ok` | pass |
| C42 | `run typesbundles / test an_element_of_an_array_of_bundles_no_producer_drives_is_e0187_naming_it ... ok` | pass |
| C42 | `run elabbundles / test a_second_producer_of_one_element_in_a_generic_instance_is_e0220_naming_it ... ok` | pass |
| C42 | `run elabbundles / test an_element_no_iteration_drives_in_a_generic_instance_is_e0187_naming_it ... ok` | pass |
| C43 | `run apimetadata / test a_field_of_an_element_of_an_array_of_bundles_is_named_element_first ... ok` | pass |
| C43 | `run elabbundles / test a_node_field_of_an_array_of_bundles_is_one_array_of_nodes ... ok` | pass |
| C43 | `run elabbundles / test a_loop_variable_index_folds_to_the_element_of_each_iteration ... ok` | pass |
| C44 | `run msl_parser / test a_struct_field_takes_an_initial_value_after_its_type ... ok` | pass |
| C44 | `run pombundles / test an_initial_value_belongs_to_an_out_field_of_a_bundle ... ok` | pass |
| C44 | `run typesbundles / test a_field_default_is_the_initial_value_of_the_carrier_its_producer_commits ... ok` | pass |
| C44 | `run typesbundles / test a_consumer_commits_the_in_field_its_flip_view_drives_from_the_field_default ... ok` | pass |
| C44 | `run elabbundles / test a_field_default_is_the_initial_value_of_the_leaf_a_view_makes_out ... ok` | pass |
| C44 | `run elabbundles / test an_in_field_default_is_the_initial_value_of_the_consumers_leaf ... ok` | pass |
| C44 | `run sourcebundles / test a_bit_field_with_a_default_is_written_by_commits ... ok` | pass |
| C44 | `run sourcebundles / test a_consumer_commits_a_bit_field_its_flip_view_drives_from_the_field_default ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run pombundles` - exit 0

```
running 8 tests
test a_misplaced_or_missing_field_direction_is_e0261 ... ok
test a_struct_mixing_carrier_and_value_fields_is_e0260 ... ok
test flip_out_and_nested_directions_are_e0262 ... ok
test one_driver_fields_carry_a_direction_and_no_other_field_does ... ok
test a_carrier_method_on_a_whole_bundle_is_e0211 ... ok
test a_struct_of_carriers_is_a_bundle ... ok
test an_initial_value_belongs_to_an_out_field_of_a_bundle ... ok
test a_bundle_declared_as_a_value_is_e0264 ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.41s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-bundles/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.51s
     Running tests/pom_bundles.rs (/var/tmp/piperine-target-d/debug/deps/pom_bundles-394d332a0379c3c4)
```

`Run typesbundles` - exit 0

```
test a_literal_field_of_another_nature_is_e0263 ... ok
test a_field_default_is_the_initial_value_of_the_carrier_its_producer_commits ... ok
test bundles_of_different_types_do_not_connect ... ok
test an_element_index_that_is_a_loop_variable_is_an_element_of_the_array ... ok
test driving_an_input_field_is_e0222 ... ok
test two_producers_of_one_element_of_an_array_of_bundles_is_e0220_naming_it ... ok
test self_on_a_bundle_is_a_read_only_handle ... ok
test an_array_of_bundles_of_a_generic_length_is_typed_per_instance ... ok
test flip_swaps_every_one_driver_direction_nested_included ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-bundles/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.70s
     Running tests/types_bundles.rs (/var/tmp/piperine-target-d/debug/deps/types_bundles-a47d12df74ee9013)
```

`Run elabbundles` - exit 0

```
test a_loop_variable_index_folds_to_the_element_of_each_iteration ... ok
test a_field_default_is_the_initial_value_of_the_leaf_a_view_makes_out ... ok
test an_observer_view_adds_no_drive ... ok
test an_array_field_of_carriers_binds_whole_and_is_written_by_element ... ok
test an_element_no_iteration_drives_in_a_generic_instance_is_e0187_naming_it ... ok
test a_constant_parameter_in_an_array_length_of_a_field_is_the_argument_of_each_port ... ok
test an_in_field_default_is_the_initial_value_of_the_consumers_leaf ... ok
test two_producers_of_one_field_is_e0220_naming_its_path ... ok
test an_array_of_bundles_of_a_generic_length_has_the_length_of_each_instance ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-bundles/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.63s
     Running tests/elab_bundles.rs (/var/tmp/piperine-target-d/debug/deps/elab_bundles-a444a815002d0bc6)
```

`Run sourcebundles` - exit 0

```
test vd_on_a_diff_bundle_gives_the_differential_voltage ... ok
test a_generic_bundle_is_instantiated_per_type_argument ... ok
test an_array_of_flipped_streams_completes_a_handshake_per_lane ... ok
test nested_bundles_apply_flip_recursively ... ok
test an_array_of_carriers_as_a_field_is_driven_and_read_by_index ... ok
test an_element_index_that_is_a_loop_variable_folds_per_iteration ... ok
test beat_fires_on_rising_edges_with_valid_and_ready ... ok
test field_commits_definitions_and_methods_follow_section_5 ... ok
test an_array_of_bundles_of_a_generic_length_runs_with_each_length ... ok
test every_view_may_drive_a_resolved_field ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.86s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_bundles.rs (/var/tmp/piperine-target-d/debug/deps/source_bundles-3539f11f66e6b81d)
```

`Run apimetadata` - exit 0

```
running 8 tests
test a_flow_of_several_contributions_is_e0801 ... ok
test metadata_lists_every_param_with_type_value_where_and_structural ... ok
test metadata_runs_no_analysis ... ok
test the_flow_of_a_single_flow_contribution_prints ... ok
test a_field_of_an_element_of_an_array_of_bundles_is_named_element_first ... ok
test a_param_prints_its_value_in_every_row ... ok
test metadata_lists_ports_and_streams ... ok
test a_bundle_field_is_named_by_its_dotted_path ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.84s

    Finished `test` profile [unoptimized] target(s) in 0.93s
     Running tests/metadata.rs (/var/tmp/piperine-target-d/debug/deps/metadata-cfe14844878cb381)
```

`Run pomreflect` - exit 0

```
test sar_adc_instances_name_their_modules ... ok
test writers_are_definitions_and_commits ... ok
test types_list_their_impls_and_methods ... ok
test json_is_deterministic ... ok
test from_json_reads_back_a_valid_design ... ok
test from_json_rejects_a_corrupted_design ... ok
test gallery_signals_are_declared_in_their_module ... ok
test from_json_never_panics_on_a_corrupted_field ... ok
test resolution_creates_no_node ... ok

test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.93s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-bundles/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 6.66s
     Running tests/pom_reflect.rs (/var/tmp/piperine-target-d/debug/deps/pom_reflect-38924400c596ee62)
```

`Run docs` - exit 0

```
test the_instruction_files_say_the_host_crates_build ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test the_roadmap_is_forward_looking ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test transient_performance_closes_v12 ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.19s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run speccodes` - exit 0

```
running 5 tests
test every_owned_code_names_its_owner_and_construct ... ok
test no_source_spells_p0002 ... ok
test no_owned_code_is_raised ... ok
test this_features_codes_are_all_raised ... ok
test every_spec_code_is_raised_or_owned ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-bundles/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/spec_codes.rs (/var/tmp/piperine-target-d/debug/deps/spec_codes-a942a887390f8334)
```

`Run booknatures` - exit 0

```
test the_slice_listing_bounds_t_by_default_as_gallery_e6_does ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok
test an_out_port_gives_the_initial_value ... ok
test behavior_crosses_freely_inside_one_module ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test the_alarm_prints_three_records_of_the_line ... ok
test the_stream_bundle_and_its_slice_run ... ok
test the_pulldown_alarm_prints_the_exercise_answer ... ok
test signals_of_every_family_run ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.49s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/natures.rs (/var/tmp/piperine-target-d/debug/deps/book_natures-573ee03fdb953ba2)
```

`Run bookmodules` - exit 0

```
test e0340_a_generation_range_read_from_a_signal ... ok
test e0320_a_block_names_a_param_the_module_does_not_have ... ok
test the_half_reactions_report_each_stage_in_time_order ... ok
test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok
test a_ladder_written_out_runs_an_operating_point_and_a_dc_sweep ... ok
test the_pi_controller_runs_with_its_generic_idt ... ok
test the_memory_bank_of_bundle_cells_runs ... ok
test the_multiplicity_listing_runs_as_four_in_parallel ... ok
test the_switched_capacitor_integrator_is_a_netlist_that_runs ... ok
test the_swappable_mosfet_listing_runs ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.29s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book/modules.rs (/var/tmp/piperine-target-d/debug/deps/book_modules-f54c744dbbe5981c)
```

`Run bookcheatsheet` - exit 0

```
test the_nature_card_is_the_standard_librarys ... ok
test the_analog_card_runs ... ok
test the_module_card_runs_with_its_value_input_and_its_out_carrier ... ok
test the_digital_card_runs ... ok
test the_body_items_card_runs_with_its_generation ... ok
test the_template_card_runs ... ok
test the_module_card_runs_for_its_const_generic_without_its_value_input ... ok
test the_debug_prints_card_runs_with_its_analog_trace ... ok
test the_files_card_runs ... ok
test a_name_of_yours_hides_the_library_but_not_the_core ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.16s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/cheatsheet.rs (/var/tmp/piperine-target-d/debug/deps/book_cheatsheet-1449ebc6201d4733)
```

`Run bookinventory` - exit 0

```
test every_gaps_row_names_its_owner ... ok
test no_block_row_is_pending ... ok
test a_marked_file_joins_its_blocks ... ok
test a_verdict_fits_its_block ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test disputed_rows_are_listed_for_the_user ... ok
test the_book_uses_no_removed_name ... ok
test the_book_reads_every_block ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s

    Finished `test` profile [unoptimized] target(s) in 0.19s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-d/debug/deps/book_inventory-4e223d7a754dd3b2)
```

`Run msl_parser` - exit 0

```
test negative_literal_is_a_const_generic_argument ... ok
test contextual_keywords_are_identifiers_outside_nature ... ok
test contribution_commit_at_any_depth ... ok
test function_pointer_types_parse ... ok
test unknown_is_an_ordinary_name ... ok
test inherited_rust_forms_parse_items ... ok
test inherited_rust_forms_parse ... ok
test inherited_rust_forms_parse_expressions ... ok
test every_b3_production_parses_with_span ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-bundles/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.14s
     Running tests/msl_parser.rs (/var/tmp/piperine-target-d/debug/deps/msl_parser-27277862c4e427ef)
```

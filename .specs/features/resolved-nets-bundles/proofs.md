# resolved-nets-bundles proofs

**HEAD**: 73cef16b
**Tree**: clean
**Shell**: /bin/sh
**Result**: 10 pass, 31 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run pombundles / test a_struct_of_carriers_is_a_bundle ... ok` | pass |
| C2 | `run pombundles / test one_driver_fields_carry_a_direction_and_no_other_field_does ... ok` | pass |
| C3 | `run pombundles / test a_struct_mixing_carrier_and_value_fields_is_e0260 ... ok` | pass |
| C4 | `run pombundles / test a_misplaced_or_missing_field_direction_is_e0261 ... ok` | pass |
| C5 | `run pombundles / test flip_out_and_nested_directions_are_e0262 ... ok` | pass |
| C6 | `run pombundles / test a_bundle_declared_as_a_value_is_e0264 ... ok` | pass |
| C7 | `run typesbundles / test a_producer_drives_out_fields_and_reads_in_fields ... ok` | **fail** |
| C8 | `run typesbundles / test flip_swaps_every_one_driver_direction_nested_included ... ok` | **fail** |
| C9 | `run typesbundles / test an_observer_reads_every_one_driver_field ... ok` | **fail** |
| C10 | `run elabbundles / test two_producers_of_one_field_is_e0220_naming_its_path ... ok` | **fail** |
| C11 | `run typesbundles / test driving_an_input_field_is_e0222 ... ok` | **fail** |
| C12 | `run sourcebundles / test a_producer_consumer_and_observer_run_three_beats ... ok` | **fail** |
| C13 | `run sourcebundles / test a_bundle_carrier_connects_field_by_field ... ok` | **fail** |
| C14 | `run sourcebundles / test a_cell_literal_connects_each_named_field ... ok` | **fail** |
| C15 | `run typesbundles / test a_literal_field_of_another_nature_is_e0263 ... ok` | **fail** |
| C16 | `run typesbundles / test a_literal_outside_a_connection_or_a_bundle_operand_is_e0264 ... ok` | **fail** |
| C17 | `run sourcebundles / test a_diff_bundle_connects_its_nodes_like_ports ... ok` | **fail** |
| C18 | `run sourcebundles / test field_commits_definitions_and_methods_follow_section_5 ... ok` | **fail** |
| C19 | `run typesbundles / test a_carrier_method_on_a_field_applies_to_that_field ... ok` | **fail** |
| C20 | `run pombundles / test a_carrier_method_on_a_whole_bundle_is_e0211 ... ok` | **no test ran** |
| C21 | `run typesbundles / test self_on_a_bundle_is_a_read_only_handle ... ok` | **fail** |
| C22 | `run sourcebundles / test vd_on_a_diff_bundle_gives_the_differential_voltage ... ok` | **fail** |
| C23 | `run sourcebundles / test beat_fires_on_rising_edges_with_valid_and_ready ... ok` | **fail** |
| C24 | `run typesbundles / test a_write_through_a_bundle_handle_is_e0133 ... ok` | **fail** |
| C25 | `run sourcebundles / test nested_bundles_apply_flip_recursively ... ok` | **fail** |
| C26 | `run sourcebundles / test every_view_may_drive_a_resolved_field ... ok` | **fail** |
| C27 | `run sourcebundles / test a_generic_bundle_is_instantiated_per_type_argument ... ok` | **fail** |
| C28 | `run sourcebundles / test an_array_of_bundles_connects_each_element_field_by_field ... ok` | **fail** |
| C29 | `run apimetadata / test a_bundle_field_is_named_by_its_dotted_path ... ok` | **no test ran** |
| C30 | `run pomreflect / test a_bundle_port_reflects_its_type_and_view ... ok` | **no test ran** |
| C31 | `run docs / test gaps_no_longer_lists_bundles ... ok` | **no test ran** |
| C32 | `run speccodes / test no_owned_code_is_raised ... ok` | pass |
| C33 | `run typesbundles / test a_bundle_port_is_not_a_value_input ... ok` | **fail** |
| C34 | `run booknatures / test the_stream_bundle_and_its_slice_run ... ok` | **no test ran** |
| C35 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C36 | `run elabbundles / test a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path ... ok` | **fail** |
| Run typesbundles | `cargo test -p piperine-lang --test types_bundles` | **fail** - exit 101 |
| Run elabbundles | `cargo test -p piperine-lang --test elab_bundles` | **fail** - exit 101 |
| Run sourcebundles | `cargo test -p piperine-parity --test source_bundles` | **fail** - exit 101 |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run pombundles` - exit 0

```
running 6 tests
test a_struct_mixing_carrier_and_value_fields_is_e0260 ... ok
test flip_out_and_nested_directions_are_e0262 ... ok
test a_misplaced_or_missing_field_direction_is_e0261 ... ok
test one_driver_fields_carry_a_direction_and_no_other_field_does ... ok
test a_struct_of_carriers_is_a_bundle ... ok
test a_bundle_declared_as_a_value_is_e0264 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.95s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.38s
     Running tests/pom_bundles.rs (/var/tmp/piperine-target-a/debug/deps/pom_bundles-69496b0fdf495fef)
```

`Run typesbundles` - exit 101

```
    suite_hygiene
    types_analog
    types_buck
    types_carriers
    types_compound
    types_entry
    types_events
    types_integers
    types_modules
    types_natures
    types_results
    types_structure
    types_subset
    types_unknowns
    types_values
```

`Run elabbundles` - exit 101

```
    suite_hygiene
    types_analog
    types_buck
    types_carriers
    types_compound
    types_entry
    types_events
    types_integers
    types_modules
    types_natures
    types_results
    types_structure
    types_subset
    types_unknowns
    types_values
```

`Run sourcebundles` - exit 101

```
    source_derivative
    source_environment
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_switch
    source_time
    source_unknowns
    twins
```

`Run apimetadata` - exit 0

```
running 6 tests
test a_flow_of_several_contributions_is_e0801 ... ok
test metadata_lists_every_param_with_type_value_where_and_structural ... ok
test metadata_runs_no_analysis ... ok
test the_flow_of_a_single_flow_contribution_prints ... ok
test a_param_prints_its_value_in_every_row ... ok
test metadata_lists_ports_and_streams ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.07s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/metadata.rs (/var/tmp/piperine-target-a/debug/deps/metadata-1de86750d8a04096)
```

`Run pomreflect` - exit 0

```
test json_is_deterministic ... ok
test sar_adc_instances_name_their_modules ... ok
test types_list_their_impls_and_methods ... ok
test writers_are_definitions_and_commits ... ok
test from_json_reads_back_a_valid_design ... ok
test from_json_rejects_a_corrupted_design ... ok
test gallery_signals_are_declared_in_their_module ... ok
test from_json_never_panics_on_a_corrupted_field ... ok
test resolution_creates_no_node ... ok

test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.95s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.22s
     Running tests/pom_reflect.rs (/var/tmp/piperine-target-a/debug/deps/pom_reflect-dd6c6bfb385a2d5b)
```

`Run docs` - exit 0

```
test state_records_the_unknown_semantics_decision ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test gaps_drops_what_msl_fixes_closed ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test the_roadmap_and_state_record_analog_structure ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test the_roadmap_and_state_record_root_e2e ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run speccodes` - exit 0

```
running 4 tests
test every_owned_code_names_its_owner_and_construct ... ok
test every_spec_code_is_raised_or_owned ... ok
test no_source_spells_p0002 ... ok
test no_owned_code_is_raised ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_codes.rs (/var/tmp/piperine-target-a/debug/deps/spec_codes-86724e06e4c5ed72)
```

`Run booknatures` - exit 0

```
test e0222_a_module_cannot_drive_its_own_input ... ok
test e0310_a_one_driver_port_needs_a_direction ... ok
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test signals_of_every_family_are_refused_for_the_arrays_and_the_wire ... ok
test the_open_drain_line_is_refused_while_resolved_natures_are_not_compiled ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test behavior_crosses_freely_inside_one_module ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test an_out_port_gives_the_initial_value ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.37s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/natures.rs (/var/tmp/piperine-target-a/debug/deps/book_natures-84da6f1f33ea55da)
```

`Run bookinventory` - exit 0

```
test every_block_of_the_book_has_one_row ... ok
test a_verdict_fits_its_block ... ok
test disputed_rows_are_listed_for_the_user ... ok
test a_model_across_blocks_names_one_test ... ok
test every_gaps_row_names_its_owner ... ok
test run_rows_name_tests_that_run_their_model ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

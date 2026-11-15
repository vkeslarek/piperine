# msl-typeck proofs

**HEAD**: bfa13f35
**Tree**: clean
**Shell**: /bin/sh
**Result**: 44 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run entry / test the_reachable_set_of_a_twin_is_typed ... ok` | pass |
| C2 | `run entry / test the_design_is_unchanged_by_the_check ... ok` | pass |
| C3 | `run values / test a_deferred_method_is_resolved ... ok` | pass |
| C4 | `run entry / test checking_twice_gives_equal_tables ... ok` | pass |
| C5 | `run values / test literals_are_f64_or_bool ... ok` | pass |
| C6 | `run values / test operators_type_as_rust_types_them ... ok` | pass |
| C7 | `run values / test if_and_block_take_their_value_type ... ok` | pass |
| C8 | `run values / test a_let_takes_its_initializer_type ... ok` | pass |
| C9 | `run values / test a_call_takes_its_declared_result ... ok` | pass |
| C10 | `run values / test catalogue_methods_on_f64_are_f64 ... ok` | pass |
| C11 | `run values / test a_closure_takes_its_types_from_the_parameter ... ok` | pass |
| C12 | `run values / test a_mismatch_names_both_types ... ok` | pass |
| C13 | `run modules / test params_are_f64_or_bool ... ok` | pass |
| C14 | `run modules / test a_default_of_the_wrong_type_is_rejected ... ok` | pass |
| C15 | `run modules / test a_where_that_is_not_bool_is_e0318 ... ok` | pass |
| C16 | `run modules / test ports_and_signals_are_nodes_and_gnd_fits_any ... ok` | pass |
| C17 | `run modules / test a_node_init_that_is_not_f64_is_e0210 ... ok` | pass |
| C18 | `run modules / test a_placement_checks_nodes_count_and_params ... ok` | pass |
| C19 | `run modules / test a_node_of_another_nature_is_e0251 ... ok` | pass |
| C20 | `run analog / test potential_probes_are_f64 ... ok` | pass |
| C21 | `run analog / test probe_nodes_of_two_natures_are_e0240 ... ok` | pass |
| C22 | `run analog / test a_contribution_target_that_is_no_probe_is_e0410 ... ok` | pass |
| C23 | `run analog / test a_contribution_that_is_not_f64_is_e0417 ... ok` | pass |
| C24 | `run analog / test ddt_ac_stim_and_environment_are_f64 ... ok` | pass |
| C25 | `run analog / test limit_checks_its_input_function_and_seed ... ok` | pass |
| C26 | `run analog / test a_core_primitive_in_a_fn_is_e0450 ... ok` | pass |
| C27 | `run subset / test a_construct_outside_the_subset_is_p0001 ... ok` | pass |
| C28 | `run subset / test p0001_names_the_construct ... ok` | pass |
| C29 | `run subset / test unreached_items_are_not_reported ... ok` | pass |
| C30 | `run entry / test a_design_with_diagnostics_gives_them_back ... ok` | pass |
| C31 | `run entry / test an_unknown_top_is_one_diagnostic ... ok` | pass |
| C32 | `run values / test diagnostics_come_in_source_order_one_per_expression ... ok` | pass |
| C33 | `run entry / test every_diagnostic_has_a_span_and_a_code ... ok` | pass |
| C34 | `run values / test a_recursive_function_is_typed_from_its_signature ... ok` | pass |
| C35 | `run values / test a_let_without_type_or_value_is_p0001 ... ok` | pass |
| C36 | `run entry / test a_const_is_typed_once_by_its_declared_type ... ok` | pass |
| C37 | `run values / test a_closure_outside_an_argument_is_p0001 ... ok` | pass |
| C38 | `run modules / test a_where_on_a_module_or_fn_is_bool ... ok` | pass |
| C39 | `run modules / test a_core_primitive_is_checked_where_it_is_written ... ok` | pass |
| C40 | `run modules / test an_environment_function_in_a_const_context_is_e0323 ... ok` | pass |
| C41 | `run values / test a_function_is_a_value_only_as_an_argument ... ok` | pass |
| C42 | `run values / test an_unsupported_form_does_not_hide_an_independent_error ... ok` | pass |
| C43 | `run subset / test std_types_outside_the_subset_are_p0001 ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run entry` - exit 0

```
running 7 tests
test checking_twice_gives_equal_tables ... ok
test a_design_with_diagnostics_gives_them_back ... ok
test an_unknown_top_is_one_diagnostic ... ok
test a_const_is_typed_once_by_its_declared_type ... ok
test every_diagnostic_has_a_span_and_a_code ... ok
test the_design_is_unchanged_by_the_check ... ok
test the_reachable_set_of_a_twin_is_typed ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-typeck/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.73s
     Running tests/types_entry.rs (/var/tmp/piperine-target/debug/deps/types_entry-7c107df262610793)
```

`Run values` - exit 0

```
test literals_are_f64_or_bool ... ok
test a_recursive_function_is_typed_from_its_signature ... ok
test diagnostics_come_in_source_order_one_per_expression ... ok
test a_function_is_a_value_only_as_an_argument ... ok
test an_unsupported_form_does_not_hide_an_independent_error ... ok
test a_closure_takes_its_types_from_the_parameter ... ok
test a_call_takes_its_declared_result ... ok
test operators_type_as_rust_types_them ... ok
test the_std_pnjlim_checks_clean ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-typeck/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.38s
     Running tests/types_values.rs (/var/tmp/piperine-target/debug/deps/types_values-f8763260a09ed093)
```

`Run modules` - exit 0

```
test a_node_init_that_is_not_f64_is_e0210 ... ok
test a_default_of_the_wrong_type_is_rejected ... ok
test a_node_of_another_nature_is_e0251 ... ok
test params_are_f64_or_bool ... ok
test ports_and_signals_are_nodes_and_gnd_fits_any ... ok
test a_where_on_a_module_or_fn_is_bool ... ok
test a_placement_checks_nodes_count_and_params ... ok
test an_environment_function_in_a_const_context_is_e0323 ... ok
test a_core_primitive_is_checked_where_it_is_written ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-typeck/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.35s
     Running tests/types_modules.rs (/var/tmp/piperine-target/debug/deps/types_modules-248adbc18878d535)
```

`Run analog` - exit 0

```
running 8 tests
test a_contribution_that_is_not_f64_is_e0417 ... ok
test potential_probes_are_f64 ... ok
test a_core_primitive_in_a_fn_is_e0450 ... ok
test probe_nodes_of_two_natures_are_e0240 ... ok
test a_contribution_target_that_is_no_probe_is_e0410 ... ok
test limit_checks_its_input_function_and_seed ... ok
test ddt_ac_stim_and_environment_are_f64 ... ok
test the_std_vt_checks_clean ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-typeck/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.36s
     Running tests/types_analog.rs (/var/tmp/piperine-target/debug/deps/types_analog-560800edb3a3438e)
```

`Run subset` - exit 0

```
running 4 tests
test unreached_items_are_not_reported ... ok
test p0001_names_the_construct ... ok
test a_construct_outside_the_subset_is_p0001 ... ok
test std_types_outside_the_subset_are_p0001 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-typeck/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.32s
     Running tests/types_subset.rs (/var/tmp/piperine-target/debug/deps/types_subset-010120436d0f2935)
```

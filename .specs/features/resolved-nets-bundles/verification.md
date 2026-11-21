# Resolved Nets: Bundles verification

**Verdict**: PASS
**Proofs**: `proofs.md` at 73cef16b
**Verifier**: independent sub-agent (author != verifier)
**Scope**: partial merge, batch B1 (T1-T4) only. Every check owned by T5-T21 is deferred (it waits on `composite-values` A-B, `generics`, `resolved-nets` T22); its proof is red or has no test at this HEAD, as expected, and is not judged here.

Test runs at 22fc40b4: `pom_bundles` 6/6. In the root `cargo test --no-fail-fast` run, 3186 passed and 0 failed. `check_proofs.py`: C1-C6, C32 and C35 pass, and the build passes. D16: the rows for E0260, E0261, E0262 and E0264 are deleted from `code_owners.tsv`. GAPS §10 drops them, and the list now holds 44 of 189 codes (recounted). The replaced fixtures are legitimate under spec §7: `pom_classify` `Port2` lost its `gain: f64` field, because §7 rule 1 makes that struct E0260 and the test stays about E0263. `types_buck` `Bn { a: Bit }` became `out a: Bit`, because §7 rule 2 makes the undirected field E0261 and the row stays about the bundle's `P0001`. Neither appears in design.md "Goldens"; both are named in the T1 and T2 commit bodies. Probes: a direction on a `VSignal` field is E0261; a value field beside a nested bundle is E0260; a tuple field is E0260; `[flip Bit; 2]` is E0262; `[S; 2]` as a param type is E0264.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `pom_bundles.rs` `a_struct_of_carriers_is_a_bundle` | partly - asserts only that no diagnostic is raised, which also holds if the struct were classified as a plain value struct; that `Stream` is a bundle (`StructData::bundle == Some(true)`) is never asserted |
| C2 | `pom_bundles.rs` `one_driver_fields_carry_a_direction_and_no_other_field_does` | yes |
| C3 | `pom_bundles.rs` `a_struct_mixing_carrier_and_value_fields_is_e0260` | yes - span `gain`, the message names `hi` and `gain`, and the plain struct is untouched |
| C4 | `pom_bundles.rs` `a_misplaced_or_missing_field_direction_is_e0261` | yes - exact span list `valid`, `bus`, `node`, with `ok` and `good` accepted |
| C5 | `pom_bundles.rs` `flip_out_and_nested_directions_are_e0262` | yes - exact span list for the three refusals, with the three accepted forms beside them |
| C6 | `pom_bundles.rs` `a_bundle_declared_as_a_value_is_e0264` | yes for the declaration positions (one span per case, plus a clean control); the value-operator position belongs to T7 |
| C7 | deferred (T6) `a_producer_drives_out_fields_and_reads_in_fields` | deferred |
| C8 | deferred (T6) `flip_swaps_every_one_driver_direction_nested_included` | deferred |
| C9 | deferred (T6) `an_observer_reads_every_one_driver_field` | deferred |
| C10 | deferred `two_producers_of_one_field_is_e0220_naming_its_path` | deferred |
| C11 | deferred `driving_an_input_field_is_e0222` | deferred |
| C12 | deferred `a_producer_consumer_and_observer_run_three_beats` | deferred |
| C13 | deferred `a_bundle_carrier_connects_field_by_field` | deferred |
| C14 | deferred `a_cell_literal_connects_each_named_field` | deferred |
| C15 | deferred `a_literal_field_of_another_nature_is_e0263` | deferred |
| C16 | deferred `a_literal_outside_a_connection_or_a_bundle_operand_is_e0264` | deferred |
| C17 | deferred `a_diff_bundle_connects_its_nodes_like_ports` | deferred |
| C18 | deferred `field_commits_definitions_and_methods_follow_section_5` | deferred |
| C19 | deferred `a_carrier_method_on_a_field_applies_to_that_field` | deferred |
| C20 | deferred `a_carrier_method_on_a_whole_bundle_is_e0211` | deferred |
| C21 | deferred `self_on_a_bundle_is_a_read_only_handle` | deferred |
| C22 | deferred `vd_on_a_diff_bundle_gives_the_differential_voltage` | deferred |
| C23 | deferred `beat_fires_on_rising_edges_with_valid_and_ready` | deferred |
| C24 | deferred `a_write_through_a_bundle_handle_is_e0133` | deferred |
| C25 | deferred `nested_bundles_apply_flip_recursively` | deferred |
| C26 | deferred `every_view_may_drive_a_resolved_field` | deferred |
| C27 | deferred `a_generic_bundle_is_instantiated_per_type_argument` | deferred |
| C28 | deferred `an_array_of_bundles_connects_each_element_field_by_field` | deferred |
| C29 | deferred `a_bundle_field_is_named_by_its_dotted_path` | deferred |
| C30 | deferred `a_bundle_port_reflects_its_type_and_view` | deferred |
| C31 | deferred (T21) `gaps_no_longer_lists_bundles` | deferred |
| C32 | deferred (T21) `no_owned_code_is_raised` | deferred (green today: E0260-E0262 and E0264 rows deleted) |
| C33 | deferred `a_bundle_port_is_not_a_value_input` | deferred |
| C34 | deferred (T21) `the_stream_bundle_and_its_slice_run` | deferred |
| C35 | deferred (T21) `refused_rows_and_gaps_section_11_agree` | deferred (green today) |
| C36 | deferred `a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path` | deferred |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1. C1's test does not assert that the struct is a bundle; it passes if classification treated it as a value struct | `pom_bundles.rs` `a_struct_of_carriers_is_a_bundle` asserts only `codes == []` | assert `StructData::bundle == Some(true)` (through the POM reflection) for `Stream` and for a struct whose only field is an array of carriers, and for one whose only field is a nested bundle | cf54677e: C1 asserts `StructData::bundle` is `Some(true)` for `Stream`, an array-of-carriers struct and a nested bundle |

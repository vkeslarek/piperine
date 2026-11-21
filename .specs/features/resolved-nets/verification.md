# Resolved Nets verification

**Verdict**: PASS
**Proofs**: `proofs.md` at 73cef16b
**Verifier**: independent sub-agent (author != verifier)
**Scope**: partial merge (fix round 2026-11-21: F1-F7 and orchestrator decisions 9 and 10 closed below; verdict PASS for the in-scope checks), batches B1-B2 (T1-T10) only. Every check owned by T11-T36 is deferred (it waits on `composite-values` A-B, `value-semantics`, `generics` A); its proof is red or has no test at this HEAD, as expected, and is not judged here.

Test runs at 22fc40b4 (`CARGO_TARGET_DIR=/var/tmp/piperine-target-a`): `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` at the root, exit 0, 308 targets, 3186 passed, 0 failed; `cargo test -p piperine-digital-solver --test relax` 4/4; `cargo test -p piperine-parity --test source_natures` 5/5; `types_natures` 8/8, `elab_natures` 2/2. `check_proofs.py`: every in-scope check passes, the build passes, and the 49 failing rows all belong to deferred tasks. No contract or tolerance edits: `git diff origin/main -- crates/piperine-parity/src tests/support crates/piperine-transpile/tests/golden` is empty. GAPS counts recomputed from the files: 36 `| L` rows in §4, and the §10 list holds 44 codes, none of which `crates/*/src` raises (189 codes in `docs/spec`). abi.md §9 D1 contains the D3 `driving` sentence. D16: the rows for E0224, E0242 and E0413 are deleted; E0203 stays with T16.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `source_natures.rs` `a_counter_over_a_project_byte_nature_commits_and_records` | no - commits and records are proven, but the model has no definition of a `Byte`, no `changed()` and no `prev()` on one (the only `changed()` is on the `Bit` clock); three of the five behaviours the criterion names are untested |
| C2 | `source_natures.rs` `a_project_nature_carrier_connects_to_a_port_of_the_same_nature` | partly - it compares the counter net between two benches, which also matches if the reader is not connected; C1's `seen` records (0..5) are what show the reader reads the net |
| C3 | `types_natures.rs` `a_project_discrete_port_without_a_direction_is_e0310` | yes |
| C4 | `types_natures.rs` `connecting_byte_to_bit_u8_is_e0251_naming_both_natures` | yes, but it pins a wrong spelling: the message names `Byte<u8>`, a type argument that the non-generic `Byte` does not take (probe: `Byte` to `Byte2` reports "`Byte<u8>` ... `Byte2<u8>`") |
| C5 | `types_natures.rs` `a_nature_value_without_repr_is_e0112` | yes |
| C6 | `types_natures.rs` `a_second_driver_of_a_project_discrete_carrier_is_e0220` | yes |
| C7 | `types_natures.rs` `a_generic_project_nature_takes_its_argument_or_its_default` | yes - asserts `UInt(8)` and `UInt(16)`; probe: `Word` into `Word<u16>` is E0251 and `Word` into `Word<u8>` is accepted |
| C23 | `types_natures.rs` `others_on_a_one_driver_carrier_is_e0224` | yes - code, carrier name and span `s.others()`, for `Bit<u8>` and a project nature |
| C26 | `relax.rs` `a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes` | partly - "starts absent" holds (first pass sees 1 drive), but the loop settles in 2 passes, so `counts.len() <= drives + 1` would also pass with a bound of `drives + 5`, or with no bound; the non-settling loop of T8 never has its pass count asserted |
| C30 | `relax.rs` `a_released_loop_keeps_no_earlier_level` | yes - if the RELAX nets were not zeroed at the start of the microstep, the keeper's drive would stay present and `v` would latch HIGH |
| C31 | `relax.rs` `an_absent_drive_is_left_out_and_an_unknown_one_is_passed` | yes as scoped (below the language): the presence rule lives in the test's stub `Resolver`/`Drive::is_present`, so the test proves the D3 layout round-trips through the solver, not a generated resolver; the generated resolver's proof comes with T14 (C61) |
| C46 | `source_natures.rs` `a_signal_flow_node_with_one_driver_solves_and_loads_nothing` | yes after F3 (was: no) - an ideal potential driver fixes `v(a)` whatever its readers draw, so "1.5 V with one reader or two" also holds if the readers loaded the node; "loads it with no flow" is not observed |
| C47 | `source_natures.rs` `a_gain_chain_over_vsignal_gives_the_product_of_the_gains` | partly - op 9 V, AC magnitude 6 (`AC_MAGNITUDE`, an existing contract) and transient 9 V at every step all match the closed form 1.5*2*3 and 1*2*3; the `dc` sweep the criterion names is not run |
| C48 | `elab_natures.rs` `a_signal_flow_node_with_no_or_two_drivers_is_e0242` | yes - code, node `Top.s`, count and both driver paths; probe: a second driver nested one level down (`Top.w.x`) is also E0242. Reported at the top module's name (Handoff default) |
| C49 | `types_natures.rs` `a_flow_contribution_to_a_signal_flow_node_is_e0413` | yes - `flow(o, gnd) +>` is E0413 and `v(o) +>` is accepted; probe: `flow(o) +>` is E0413 too |
| C50 | `source_natures.rs` `vsignal_connected_to_an_electrical_port_is_conservative` | yes - 0.5 V = 1 V * 1k/(1k+1k); together with `a_node_meeting_a_nature_with_flow_is_not_counted` |
| C51 | `types_natures.rs` `a_flow_probe_of_a_signal_flow_node_is_e0206` | yes - `flow(a, gnd)` from the checker, `i(a)` from the POM |
| C62 | `protocol_transient.rs` `an_unsettled_loop_reaches_the_owner_once_per_step` | yes - one `Unsettled { nets: ["x"] }` at the owner, no Record, and no known level of `x` |
| C64 | `tests/book/modules.rs` `the_pi_controller_is_refused_only_for_its_generic_idt` | yes - the generic-module refusal remains, and no message mentions signal-flow |
| C67 | `tests/book/inventory_guard.rs` `refused_rows_and_gaps_section_11_agree` | yes - the inventory row `modules#8` moved to L30 together with GAPS §11 |
| C68 | `source_natures.rs` `an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run` | yes - `E0810` naming `a` without a host source, then `v(c)` = 9 V with one (added by the fix round, RN-08 AC 7) |
| C8 | deferred (T16) `a_const_generic_resolve_over_an_array_is_accepted` | deferred |
| C9 | deferred (T16) `resolve_may_name_an_associated_function_or_a_const` | deferred |
| C10 | deferred (T16) `a_resolve_of_another_shape_is_e0203` | deferred |
| C11 | deferred (T16) `resolve_without_value_or_with_potential_is_e0201` | deferred |
| C12 | deferred (T16) `a_direction_on_a_resolved_port_is_e0310` | deferred |
| C13 | deferred (T22) `an_initial_value_is_the_declaring_modules_first_drive` | deferred |
| C14 | deferred (T19) `resolve_is_instantiated_once_per_drive_count` | deferred |
| C15 | deferred (T22) `three_sumnet_drivers_give_their_sum_in_the_microstep_any_changes` | deferred |
| C16 | deferred (T22) `an_undriven_resolved_carrier_holds_resolve_of_no_drive` | deferred |
| C17 | deferred (T22) `resolve_receives_the_drives_in_instance_path_order` | deferred |
| C18 | deferred (T22) `a_drive_without_initial_value_is_unknown_until_its_first_commit` | deferred |
| C19 | deferred (T22) `a_drive_change_that_leaves_the_value_fires_no_changed` | deferred |
| C20 | deferred (T17) `a_definition_and_a_commit_of_one_resolved_carrier_is_e0221` | deferred |
| C21 | deferred (T23) `others_is_resolve_of_every_drive_but_the_readers` | deferred |
| C22 | deferred (T23) `others_of_a_module_without_a_drive_is_the_value` | deferred |
| C24 | deferred (T19) `others_depends_on_the_other_drives_only` | deferred |
| C25 | deferred (T24) `a_loop_of_resolved_drives_is_accepted_and_settles` | deferred |
| C27 | deferred (T31) `two_parallel_trans_carry_a_strong_level_and_stay_two_carriers` | deferred |
| C28 | deferred (T25) `a_loop_that_does_not_settle_is_unknown_and_logged_once` | deferred |
| C29 | deferred (T21) `a_loop_through_a_one_driver_carrier_across_instances_is_e0339` | deferred |
| C32 | deferred (T27) `wired_gives_the_table_of_22_3_2` | deferred |
| C33 | deferred (T27) `wired_of_equal_drives_is_that_drive_in_any_order` | deferred |
| C34 | deferred (T28) `tri_gives_the_table_of_22_3_3` | deferred |
| C35 | deferred (T29) `tribuf_drives_strong_z_or_their_merge` | deferred |
| C36 | deferred (T29) `a_pullup_wins_only_while_every_stronger_drive_is_released` | deferred |
| C37 | deferred (T30) `a_keeper_holds_the_last_driven_level_weakly` | deferred |
| C38 | deferred (T31) `tran_drives_each_side_with_the_others_of_the_other` | deferred |
| C39 | deferred (T31) `tgate_joins_while_s_high_or_sn_low` | deferred |
| C40 | deferred (T26) `resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac` | deferred |
| C41 | deferred (T32) `bidir_drives_the_node_while_the_wire_is_driven` | deferred |
| C42 | deferred (T32) `bidir_drives_the_wire_from_the_node_when_released` | deferred |
| C43 | deferred (T32) `bidir_acts_as_driven_under_an_unknown_strength` | deferred |
| C44 | deferred (T32) `a_change_of_driving_is_a_time_point_and_a_jump` | deferred |
| C45 | deferred (T32) `two_bidirs_alone_make_the_wire_unknown_and_log_the_loop` | deferred |
| C52 | deferred (T34) `the_alarm_prints_three_records_of_the_line` | deferred |
| C53 | deferred (T34) `every_cli_listing_of_the_book_is_what_the_binary_prints` | deferred (green today on the unchanged listing) |
| C54 | deferred (T34) `the_pulldown_alarm_prints_the_exercise_answer` | deferred |
| C55 | deferred (T36) `gaps_lists_no_resolved_net_gap` | deferred (T10's partial `gaps_no_longer_lists_project_or_signal_flow_natures` passes and checks L22, L23 and modules#8) |
| C56 | deferred (T36) `c_2_3_and_g_38_say_tran_on_wires_runs` | deferred |
| C57 | deferred (T36) `no_owned_code_is_raised` | deferred (green today: E0224, E0242, E0413 rows deleted; E0203 not yet raised) |
| C58 | deferred (T12) `a_resolved_net_is_built_checked_and_printed` | deferred |
| C59 | deferred (T12) `resolved_forms_are_refused_by_name` | deferred |
| C60 | deferred (T13) `a_resolved_carrier_has_value_drive_and_others_pins` | deferred |
| C61 | deferred (T14) `resolve_calls_the_instance_of_the_present_count` | deferred |
| C63 | deferred (T25) `an_unsettled_loop_is_one_log_record_per_analysis` | deferred |
| C65 | deferred (T35) `the_irq_line_runs_on_its_resolved_wire` | deferred |
| C66 | deferred (T35) `the_resolved_line_runs_on_its_wire` | deferred |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1. C1's test exercises only commits and records; definitions, `changed()` and `prev()` on a project-nature carrier are untested | `natures/byte_counter.msl`: `count <- count + 1` and `emit!` only; `changed()` sits on the `Bit` clock | add to the model a `Byte` defined with `=`, a reaction `on count.changed()` and a read of `count.prev()`, and assert each value from §5.1/§5.3 | 38b78143: C1 now asserts `twice = 2k` (definition) and every `changed()`/`prev()` pair of `count` |
| F2. C47 does not run the `dc` sweep that RN-08 AC 2 names | `a_gain_chain_over_vsignal_gives_the_product_of_the_gains` runs op, ac and transient only | sweep the source `level` (for example 0..2 V) in `dc` and assert `v(c) = 6 * level` at each point under `DC_VOLTAGE` | 26feb863: C47 sweeps `source.level` 0..2 V in `dc` and asserts `v(c) = 6 * level` under `DC_VOLTAGE` |
| F3. C46 cannot fail if readers load the node: an ideal potential driver holds `v(a)` whatever the load | `a_signal_flow_node_with_one_driver_solves_and_loads_nothing` compares `v(a)` = 1.5 V with one reader and with two | observe "no flow" directly, for example assert that the source's potential-branch current is 0 with two readers, or drive the node through a finite source impedance whose voltage would move under any load | 60bbd637: C46 asserts every branch current of the loadless bench is 0 A |
| F4. C26's "within drives + 1 passes" bound is never reached, so a wrong bound passes | the settled loop takes 2 passes; `counts.len() <= drive_count() + 1` holds for any bound of 2 or more; the oscillating loop's pass count is never asserted | on `Loop::oscillating()`, assert that the resolver runs exactly `drive_count() + 1` passes in the step | 673d50fe: C26 asserts exactly `drive_count() + 1` passes on `Loop::oscillating()` |
| F5. E0251 spells a non-generic project nature with a type argument (`Byte<u8>`), and C4's test pins that spelling | `types_natures.rs:80` asserts `message.contains("Byte<u8>")`; probe: "`Byte<u8>` ... `Byte2<u8>`" | name a non-generic nature bare (`Byte`) in `Ty::name` for carriers, and assert `` `Byte` `` and `` `Bit<u8>` `` | dd3d96ce: C4 asserts `` `Byte` `` and `` `Bit<u8>` `` and no `Byte<..>` |
| F6. C2's test would pass with the reader unconnected | it compares `CounterBench.count` with `ConnectedBench.count`; both are written by the counter alone | assert that the reader observes the net (as C1's `seen` stream does) in C2's own test | 49bb11db: C2 asserts the reader records 0..5 on the connected net |
| F7. Cleanliness: `NatureRules::one_driver` contradicts spec §5.2's definition, and `Loop` fields lack doc comments | `types/nature_rules.rs`: SignalFlow has `one_driver: true`, while §5.2 says "one-driver carrier is a carrier of a discrete nature without `resolve`" and `pom/resolve/shape.rs` uses that meaning; `relax.rs` `struct Loop { plan, store, dirty, obs, step }` has no `///` on its fields | make `one_driver` mean discrete-without-`resolve` and express a signal-flow node as `!flow && !carries_values` (or give it its own field); add the field doc comments | 631775a1: `one_driver` is discrete-without-`resolve`, `signal_flow` is its own flag, `Loop` fields documented |
| F8. Orchestrator decision 9: `i(o) +> x` on a `VSignal` node is `E0413`, not `E0206` (RN-08 AC 4) | `classify.rs` took the contribution for a probe of an access the nature lacks | contribution targets raise `E0413`, reads keep `E0206` | 53031a87: C49 asserts `i(o) +> v(a)` is `E0413`; `resolution.md` row added |
| F9. Orchestrator decision 10: an undriven top `VSignal` port | accepted at elaboration, the host drives top node ports | RN-08 AC 7 and check C68 | c14312a2: C68 asserts `E0810` naming `a` without a host source and `v(c)` = 9 V with one |

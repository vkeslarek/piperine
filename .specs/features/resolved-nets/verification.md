# Resolved Nets verification

**Verdict**: PASS
**Proofs**: `proofs.md` at 42a063dea (98 pass, 0 fail; build pass)
**Verifier**: independent sub-agent (author != verifier), Opus, round 5 (F23 fix b77ca5bbb; round 4 judged `e031b90c5..38c2d0dcd`)
**Round 5**: `check_proofs.py` at 42a063dea, 98 pass; `cargo test -p piperine --test book_welcome --test book_claims --test book_inventory` 13 + 7 + 18 pass. The welcome test now runs a module template used as a value (`R { r: 2.0 }`) and asserts its `P0001`: when `generics` phase D makes it run, `p0001` finds no refusal and the test fails, so the sentence is rewritten at that merge. Every finding is closed.
**Scope (round 4)**: targeted only (user gate policy): `check_proofs.py` once at 38c2d0dcd with the `**Build**` line, `validate_tasks.py` (0 errors), and probes with the debug `piperine` binary (throwaway files, not committed). The merge of `origin/main` and the final full suite were NOT run: they follow a PASS only.

Round 4 judgements. The leaf exactness change is consistent with `params-and-io`: an integer leaf is now read in the range of its declared type (a `u4` field, a `Logic<60>` plane), as a whole integer param and a host drive already are (`integer_range` of the type), where before it took the range of its byte storage (0..255 for a `u4`), which let a host write bits a value of the type cannot hold; nothing goes through an `f64` any more. The `Logic` over 64 levels printing as its pattern matches how narrower ones print. The §4 rule 2 sentence (AFK default, pending user review) is what `elab/resolvers.rs` does: body carrier index is port declaration order. book_claims natures~1197ebef and the welcome test's summing net are correct; the welcome claim is F23.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | sourcenatures `a_counter_over_a_project_byte_nature_commits_and_records` | yes - after round-1 F1 (definition, `changed()` and `prev()` on a `Byte`) |
| C2 | sourcenatures `a_project_nature_carrier_connects_to_a_port_of_the_same_nature` | yes - after round-1 F6 (the reader records 0..5) |
| C3 | typesnatures `a_project_discrete_port_without_a_direction_is_e0310` | yes |
| C4 | typesnatures `connecting_byte_to_bit_u8_is_e0251_naming_both_natures` | yes - after round-1 F5 (bare `Byte`) |
| C5 | typesnatures `a_nature_value_without_repr_is_e0112` | yes |
| C6 | typesnatures `a_second_driver_of_a_project_discrete_carrier_is_e0220` | yes |
| C7 | typesnatures `a_generic_project_nature_takes_its_argument_or_its_default` | yes |
| C8 | typesnatures `an_impl_resolve_makes_a_discrete_nature_resolved` | yes |
| C9 | typesnatures `resolve_may_call_a_library_or_project_function` | yes |
| C10 | typesnatures `an_impl_resolve_of_another_shape_is_e0203` | yes - after F11: nine shapes, the three impure ones included |
| C11 | typesnatures `a_resolve_field_is_not_a_nature_field` | yes |
| C12 | typesnatures `a_direction_on_a_resolved_port_is_e0310` | yes |
| C13 | sourceresolved `an_initial_value_is_the_declaring_modules_first_drive` | yes - 13 = the initial 3 plus the other drives, one `changed` |
| C14 | elabnatures `resolve_is_instantiated_once_per_drive_count` | yes - `<0>`..`<2>` and one body for two nets of two drives |
| C15 | sourceresolved `three_summing_drivers_give_their_sum_in_the_microstep_any_changes` | yes - one change, to 7 |
| C16 | sourceresolved `an_undriven_resolved_carrier_holds_resolve_of_no_drive` | yes - 0.0 and `true` |
| C17 | sourceresolved `resolve_receives_the_drives_in_instance_path_order` | yes - after F18: name order (123) and twelve generated drivers in numeric index order (12) |
| C18 | sourceresolved `a_drive_without_initial_value_is_its_types_default_until_its_first_commit` | yes - 7 (the default) then 2 |
| C19 | sourceresolved `a_drive_change_that_leaves_the_value_fires_no_changed` | yes |
| C20 | typesnatures `a_definition_and_a_commit_of_one_resolved_carrier_is_e0221` | yes |
| C21 | sourceresolved `others_is_resolve_of_every_drive_but_the_readers` | yes - each reader's (own, others) pair, three readers |
| C22 | sourceresolved `others_of_a_module_without_a_drive_is_the_value` | yes |
| C23 | typesnatures `others_on_a_one_driver_carrier_is_e0224` | yes |
| C24 | elabnatures `others_depends_on_the_other_drives_only` | yes - only the reader's drive has an others net, resolved with `except` |
| C25 | sourceresolved `a_loop_of_resolved_drives_is_accepted_and_settles` | yes |
| C26 | relax `a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes` | yes - after round-1 F4 |
| C27 | sourcenets `two_parallel_trans_carry_a_strong_level_and_stay_two_carriers` | yes |
| C28 | sourceresolved `a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives` | yes - code, both holders in order, no `$drive` leak |
| C29 | elabnatures `a_loop_through_a_one_driver_carrier_across_instances_is_e0339` | yes |
| C30 | relax `a_released_loop_keeps_no_earlier_level` | yes |
| C31 | relax `an_absent_drive_is_left_out_and_a_present_one_is_passed` | yes (below the language; the generated resolver is C61) |
| C32 | sourcenets `strongest_gives_the_table_of_22_3_2` | yes |
| C33 | sourcenets `strongest_of_equal_drives_is_that_drive_in_any_order` | yes |
| C34 | sourcenets `a_typed_bus_resolves_to_free_byte_or_contended` | yes - after F18: no drive and two released talkers are `Free` |
| C35 | sourcenets `tribuf_drives_strong_or_high_impedance` | yes |
| C36 | sourcenets `a_pull_wins_only_while_every_stronger_drive_is_released` | yes - eight benches, pull-up and pull-down both ways |
| C37 | sourcenets `a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it` | yes - histories, the contended X not kept |
| C38 | sourcenets `tran_drives_each_side_with_the_others_of_the_other` | yes - a to b, b to a, en low |
| C39 | sourcenets `tgate_joins_while_s_high_or_sn_low` | yes |
| C40 | sourceresolved `resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac` | yes |
| C41 | sourcenets `bidir_drives_the_node_while_the_wire_is_driven` | yes - node under `TRAN_VOLTAGE` (existing contract), net unchanged |
| C42 | sourcenets `bidir_drives_the_wire_from_the_node_when_released` | yes |
| C43 | sourcenets `bidir_acts_as_driven_on_a_contended_net` | yes |
| C44 | sourcenets `a_change_of_driving_is_a_time_point_and_a_jump` | yes - a time point at 5 ns and the jump |
| C45 | sourcenets `two_bidirs_alone_stop_the_run_with_e0511` | yes |
| C46 | sourcenatures `a_signal_flow_node_with_one_driver_solves_and_loads_nothing` | yes - after round-1 F3 |
| C47 | sourcenatures `a_gain_chain_over_vsignal_gives_the_product_of_the_gains` | yes - after round-1 F2 (dc sweep) |
| C48 | elabnatures `a_signal_flow_node_with_no_or_two_drivers_is_e0242` | yes |
| C49 | typesnatures `a_flow_contribution_to_a_signal_flow_node_is_e0413` | yes |
| C50 | sourcenatures `vsignal_connected_to_an_electrical_port_is_conservative` | yes |
| C51 | typesnatures `a_flow_probe_of_a_signal_flow_node_is_e0206` | yes |
| C52 | booknatures `the_alarm_prints_three_records_of_the_line` | yes - three records, times, levels and strengths, none at 2 us |
| C53 | clibook `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes - natures#6 held text re-hashed and compared |
| C54 | booknatures `the_pulldown_alarm_prints_the_exercise_answer` | yes |
| C55 | docs `gaps_lists_no_resolved_net_gap` | yes |
| C56 | spectext `c_2_3_and_g_38_say_tran_on_a_resolved_nature_runs` | yes |
| C57 | speccodes `no_owned_code_is_raised` | yes |
| C58 | irresolved `a_resolved_net_is_built_checked_and_printed` | yes |
| C59 | interp `resolved_forms_are_refused_by_name` | yes |
| C60 | planshape `a_resolved_carrier_has_value_drive_and_others_pins` | yes |
| C61 | transresolve `resolve_calls_the_instance_of_the_present_count` | yes |
| C62 | prototran `an_unsettled_loop_is_e0511_naming_its_drives` | yes - one `Error::Unsettled` with tick and drives as data (F10 re-pointed) |
| C63 | elabnatures `a_resolved_drive_without_default_or_initial_value_is_e0187` | yes |
| C64 | bookmodules `the_pi_controller_runs_with_its_generic_idt` | yes - restated for modules#8 running since `generics` C (F10) |
| C65 | bookdigital `the_irq_line_runs_on_its_resolved_nature` | yes - after F14: the listing's `listed` is false, true at 3 us, false at 5 us |
| C66 | bookcorestd `the_resolved_line_runs_on_its_nature` | yes - after F14: `Top.board.irq_n` every half microsecond |
| C67 | bookinventory `refused_rows_and_gaps_section_11_agree` | yes |
| C68 | sourcenatures `an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run` | yes |
| C69 | typesnatures `a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve` | yes |
| C70 | sourcenets `a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type` | yes - four benches and the two-net bench |
| C71 | typesnatures `a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181` | yes |
| C72 | elabnatures `two_driven_types_on_one_net_are_e0225` | yes - code, both paths, both types |
| C73 | sourcenets `one_drive_still_goes_through_resolve` | yes - 5 + 100 |
| C74 | typesnatures `a_library_module_on_a_nature_without_resolve_driven_is_e0187` | yes |
| C75 | booknatures `the_book_no_longer_says_the_alarm_is_refused` | yes |
| C76 | bookdigital `the_four_worked_nets_give_the_tables_of_f_4` | yes - after F14: each listing's own net column |
| C77 | booknatures `signals_of_every_family_run` | yes - judged by elaboration (amendment): E0503 alone, every first value once tied |
| C78 | docs `gaps_section_11_holds_no_net_row` | yes |
| C79 | typesgenmod `a_nature_parameter_bound_by_resolve_is_a_resolved_carrier_in_the_body` | yes |
| C80 | typesgenmod `a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound` | yes |
| C81 | sourcenets `a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature` | yes |
| C82 | typesnatures `an_impure_resolve_is_e0203_naming_its_class` | yes - five cases, the class named |
| C83 | typesaggregates `a_param_of_the_librarys_logic_type_checks` | yes |
| C84 | sourcelogic `a_logic_param_binds_a_pattern_and_a_reaction_reads_it` | yes - both planes of the default and of the bound pattern, written from §22.1.4 |
| C85 | hostpatterns `a_logic_param_is_set_by_its_pattern` | yes - after F19, `Logic<4>`; any width is C92 and C93 |
| C86 | hostpatterns `the_host_drives_a_four_state_in_port` | yes |
| C87 | hostpatterns `a_bad_character_and_a_wrong_length_are_e0702` | yes - `q`, upper-case `X`, short, long, empty |
| C88 | sourcenets `two_carriers_of_one_instance_on_one_net_hold_two_drives` | yes - 12 and 312; the carrier order it pins is not in the spec (F21) |
| C89 | sourcenets `a_tran_with_both_sides_on_one_net_settles_on_the_level` | yes |
| C90 | sourcenets `a_match_over_options_and_a_resolve_walking_option_drives_run` | yes |
| C91 | irresolved `a_resolved_signal_is_no_port_of_a_child_binding` | yes - signal and port, too many and too few |
| C92 | hostpatterns `a_logic_param_of_any_width_is_set_exactly_by_its_pattern` | yes - 60 and 100 levels, by `set_text` and by `--param`, read back exactly |
| C93 | hostpatterns `a_logic_param_of_any_width_is_set_exactly_by_its_leaves` | yes - bits above 2^53 and in the second word, by text and by number; 2^60 into a 60-level plane is E0802. Round-4 probe: leaf ranges follow the declared type (`i8` takes -128 and `0x7f`, refuses -129; `u4` takes 15, refuses 16), as a whole integer param does |
| C94 | irresolved `a_resolution_whose_holders_do_not_fit_its_drives_is_refused` | yes - five holder faults, each message |
| C95 | spectext `rule_2_orders_the_drives_of_one_instance_by_carrier_declaration` | yes |
| C96 | elabgenmod `a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F10. (Major) The proofs of C62 and C64 are red at HEAD: no test ran | `proofs.md` at dfa149812: 81 pass, 2 fail. tasks.md:2171 names `an_unsettled_loop_reaches_the_owner_as_one_e0511_fault` (T11 renamed it `an_unsettled_loop_is_e0511_naming_its_drives`, `crates/piperine-solver/tests/protocol_transient.rs:711`, and left C62 unedited although T11's Done-when said to); tasks.md:2177 names `the_pi_controller_is_refused_only_for_its_generic_idt`, gone since `generics` C made modules#8 run | C62's proof names `an_unsettled_loop_is_e0511_naming_its_drives`; C64 is restated for modules#8 running (`the_pi_controller_runs_with_its_generic_idt`) or retired by a recorded amendment | 20acc35e3: C62 names `an_unsettled_loop_is_e0511_naming_its_drives`, C64 `the_pi_controller_runs_with_its_generic_idt`; both green in `proofs.md` at a132da196 (93 pass) |
| F11. (Major) A `resolve` that is not pure is not `E0203`: a solve-reading one reaches rustc as a transpiler defect, an emitting one is `P0004` | §4 rule 8 (`part_ii_carriers.md:85`): "The function MUST be of class pure"; RN-02 AC 3 / C10 name "an impure class". Probe: a fold of the drives plus `now()` passes `piperine check`, and `piperine run` stops with "rustc refused the generated source ... cannot find value `time_`"; calling an `emit!`ing fn gives "P0004 internal error: a trace outside the analog block of a body (function Total.resolve::<f64>::<0>)". C10's test (`types_natures.rs:171`) has no impure case | `E0203` naming nature and `impl` for a `resolve` whose class is not pure (`types/nature.rs` shape rule); add a solve-reading and an emitting case to C10's test | b98acbda8: `ResolveImpls::check_purity` raises E0203 naming the class; C10 gains three impure cases, C82 names the class; round-3 probe: `now()` and an `emit!`ing fn are E0203 |
| F12. (Major) Part VIII §23.1 rule 13 says "A `Logic` is no param", which §8.1.15 contradicts; the struct-param refusal it hides lost its GAPS row | `docs/spec/part_viii_host.md:96` vs `part_iii_structure.md:173` ("A param's type is a value type"; `Logic<N>` is a struct, a value type). Probe: `pat: Logic<4> = logic("10xz");` in a module body is `P0001` "param of type `struct Logic<4>` is outside the M1 subset". dfa149812 deleted L71 from GAPS, whose first half was exactly this; no other GAPS row names a struct-typed param. The amendment was written by the implementer to fit the subset, with no user or coordinator ruling recorded (user rule: code follows spec) | drop the sentence (or record a ruling that changes §8.1.15); reopen a GAPS §4 row for a struct-typed param (and the host's pattern write of a `Logic` param) with its owner | aa16aef3e: the sentence is reverted; a `Logic<N>` param checks and runs (C83, C84, C85). The >53-level limit it left is F19 (round 3) |
| F13. (Major) An instance whose two resolved carriers sit on one net and both drive it is a bind failure, not two drives | `crates/piperine-plan/src/netlist.rs:183`: `holders.position(instance)` binds every carrier of the instance to its first drive slot (the Handoff's stated LIMIT). Probe: `module Two(in a: Bit, in b: Bit, x: AndWire, y: AndWire) { x = !a; y = !b; }` placed as `Two(a, b, w, w)` stops the build with "Digital: bind rule P1 broken: net `Top.w$drive0` has two writers: Top.t.x.drive ... Top.t.y.drive". Loud, not silent (a one-driving-one-reading instance on one net was probed and resolves correctly), but §5.2 gives each written carrier a drive, the message is an internal bind rule rather than a coded diagnostic, and GAPS does not list it. The same holds for `Tran(w, w, en)` | key a holder by (instance, carrier) so each carrier gets its own drive slot, or refuse the form at elaboration with `P0001` naming it and add a GAPS row | 3633739fc: `Resolution::holders` is `Vec<Option<Holder { instance, carrier }>>`; C88 (12, 312) and C89 (`Tran(w, w, en)`); round-3 probe `Two(a, b, w, w)` resolves (false, true at 3 us) |
| F14. (Minor) The Book's net rows assert the bench's copy of each board, not the listing's `Board` | `tests/book/digital.rs:447-450`: `signal listed` takes the listing's `Board`'s `out pending` and no stream or assertion reads it; `tests/book/core_std.rs:52-56` and the digital#17 bench do the same with `Watched`. Probe at HEAD: the listing's `Board` with the digital#16 stimulus records `listed` false at 0, true at 3 us, false at 5 us, so the code is right and only the proof is loose | record and assert `listed` in digital#16 (it equals `pending`); observe the listing's own line (`Top.board.irq_n` or a watcher bound through a reach) for core-std#6 and digital#17 | 2e0710c1c: digital#16 asserts `listed`; digital#17-#19 and core-std#6 assert the listing's own net column (C65, C66, C76) |
| F15. (Minor) tasks.md is corrupted and the records are stale | 0910f7193 inserted a second copy of tasks.md lines 1014-1962 (the tail of the Execution Protocol through T39), starting with the broken line 1013 "## Checks` cites...": every task appears twice; T36's two Done-when boxes are unchecked (tasks.md:1975-1976, `- [ ]`); spec.md Requirement Traceability still says RN-02 "In Tasks", RN-03 "T33 edge cases remain", RN-05 "T31 remains", RN-06 "Started", RN-07, RN-09, RN-10 "In Tasks"; T36's What still says `std_galleries` builds 18 (the 20 is recorded in Handoff B7 (5) and in G.38, which is an accepted amendment) | delete the duplicate block and restore T36's heading position; mark T36; update the traceability table; note the 18 -> 20 amendment in T36 | 20acc35e3, 2dd7beffd: tasks.md holds one copy of each task, T36 marked, traceability updated |
| F16. (Minor) Code cleanliness | free functions: `shape_width` (`crates/piperine-api/src/logic_pattern.rs:76`), `pad_node` and `edge_values` (`crates/piperine-parity/tests/source_nets.rs:290`, `:351`); `Error::Unsettled`'s fields `domain`, `tick`, `drives` carry no `///` (`crates/piperine-analog-solver/src/error.rs:131-135`), against the tasks.md rule "every new or changed ... field ... carries a `///`" | make each an associated function of an owner type (`LogicPattern`, the test's `Model`/`Held`); document the three fields | e4b982d4b: `shape_width`, `pad_node`, `edge_values` are associated functions; `Error::Unsettled` fields documented |
| F17. (Minor) Compiler changes proven only through the Book, and L71 has no check | the digital#20 fix (`elab/aggregate.rs` `field_subject`, `elab/iterators.rs` `bind_pattern` over `Option`) and `CarrierRole::Resolved { port: false }` in the IR child-binding count (`piperine-ir/src/check.rs:559`) have no test in their own crate (MD-28); `Session::drive_pattern` (L71) has tests in `crates/piperine-api/tests/host_patterns.rs` but no `## Checks` row, so no proof runs it. Probes at HEAD passed: a tuple `match` over two `Option`s (3, 104, 3, 7) and a `resolve` with `for` and `fold` over `Option` drives (12, then 120) | a `piperine-lang` (or parity) test for each lowering and an IR test of a child with a resolved signal; a check for L71's tests | e4b982d4b, d843e017c: C90 (Option lowerings), C91 (resolved signal is no port), C86/C87 (L71) |
| F18. (Minor) C17 and C34 prove their rules on part of the cases | C17: name order only, the numeric index order of §4 rule 2 is untested (probe: holds); C34: "`Free` for ... only `None`s" has no bench (`nets/bus.msl:139` has no drive at all) | add a generated bench of 11+ drivers to C17's test and a two-released-talkers bench to C34's | e4b982d4b: C17 adds `IndexBench` (12 generated drivers, 12.0), C34 adds two released talkers (`Free`) |
| F19. (Major) A `Logic<N>` param of more than 53 levels cannot be set by the host at all, and Part VIII rule 13 recommends a path that is refused too | `crates/piperine-api/src/param_text.rs` `Reading::pattern`: `width > 53` is E0802 "set the leaves `binary` and `value` as numbers". Probe at 97842adab, `pat: Logic<60>`: `--param Top.l.pat=1000...0001` (60 levels) is E0802, and `--param Top.l.pat.binary=1152921504606846977` is E0802 "an `f64` cannot hold it exactly" (`Params::leaf_number`/`leaf_change` take an `f64`). The limit is not real: `Params::changes_composite` (`params.rs:486`) already writes a composite param's exact bytes, `LogicPattern::bytes` gives exactly those bytes, and `params-and-io` made integer params exact (`changes_exact`, `changes_integer`). `docs/spec/part_viii_host.md` rule 13 ("a pattern of more than 53 levels is set by its leaves") states a limit and a workaround that do not hold. No test covers N > 53 (C85 is `Logic<4>` only) | write the pattern's bytes through `changes_composite` for every `N` (no f64 round trip), drop the 53-level sentence from rule 13, and test `Logic<60>` and `Logic<100>` patterns in C85's test | 9f20c2ac9: a pattern is written as the pin's exact bytes through `changes_composite` for every N, and a plane leaf through `LeafEncoding` (`leaf_input.rs`) in its own type's range; rule 13 drops the 53-level sentence; C92, C93; round-4 probe: `--param Top.l.pat=` a 60-level pattern reads back exactly |
| F20. (Minor) The IR checks of a net's `Resolution` are untested, the new `Holder` carrier check included | `crates/piperine-ir/src/check/resolved.rs:61-72`: "holders for drives", "holder instance out of range" and the new "holder carrier {} out of range for instance" are raised by no test (`grep` of `crates/*/tests`). The check also accepts a holder whose carrier is in range but not `CarrierRole::Resolved` | a negative test in `crates/piperine-ir/tests/resolved.rs` for each message; refuse a holder carrier whose role is not `Resolved` | cfb472e9d: a holder carrier that is no `Resolved` carrier is refused; C94 raises every holder check (count, instance, carrier range, role) |
| F21. (Minor) The order of two drives of one instance is observable and pinned by C88, but the spec does not state it | §4 rule 2's Resolution orders drives by the holders' instance paths only (`docs/spec/part_ii_carriers.md:129-132`); `elab/resolvers.rs` breaks the tie by carrier index, and `resolve` sees it (`Twin(n, n)` gives 12, not 21). Code follows spec: the order is a rule that needs its sentence | add to §4 rule 2's Resolution that the drives of one instance follow its carriers' declaration order (with `spec_text.rs`), or record the question in GAPS §5 | fb203add7: §4 rule 2 Resolution states carrier declaration order (AFK default, pending user review), guarded by C95; round-4 probe: `Twin(y, x)` declared y first gives 21 |
| F22. (Minor) `validate_tasks.py` still fails, and T29a's elaboration test has no check | `validate_tasks.py resolved-nets`: "ERROR T28 declares `Depends on: T28`": the heading `### T29a:` is no task ID the validator reads, so T29a's body (its `Depends on: T28`, its `Where` list) is parsed as T28's; WARN "`Run elabgenmod` is read by no proof": `a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature` (T29a's Done-when) has no check | give T29a a plain task ID (T40) with the phase diagram arrow, and add a check for the elaboration test | d4b915498: T29a renumbered T40 with its arrow; C96 checks its elaboration test; `validate_tasks.py` 0 errors |
| F23. (Minor) The welcome status sentence's "templates" half has no test, so it will go stale silently when `generics` phase D merges | `tests/book/welcome.rs` `the_status_paragraph_lists_what_is_still_refused_and_one_item_runs` (claim `welcome~84a34395`: "refused by name with P0001: templates and bundles") asserts `P0001` for a bundle (`struct Pair` with carrier fields) only; no template is run. Once phase D makes templates run, the sentence is false and this test stays green (only cheatsheet#8 and modules#10 flip). origin/main is still 0bf08c637, so the sentence is true today | add a `p0001(row, <a module template>, <its message>)` line, so the test fails with "now runs" at the phase D merge and the sentence is rewritten there | b77ca5bbb: `the_status_paragraph_lists_what_is_still_refused_and_one_item_runs` asserts `P0001` for a module template value beside the bundle; book_welcome 13, book_claims 7, book_inventory 18 pass |

## Round 3 (F10-F18 fixes, 2026-11-24)

**Verdict (round 3)**: FAIL
**Proofs (round 3)**: `proofs.md` at 97842adab (93 pass, 0 fail; build pass)
**Verifier (round 3)**: independent sub-agent (author != verifier), Opus, round 3 (re-verification of the F10-F18 fixes, `53b501777..97842adab`)
**Scope (round 3)**: targeted only (user gate policy): `check_proofs.py` once at 97842adab with the `**Build**` line, and probes with the debug `piperine` binary (throwaway files, not committed). The merge of `origin/main` and the final full suite were NOT run: they follow a PASS only.

Round 3 probes at 97842adab: F11's `now()` and `emit!`ing `resolve` are E0203 naming the class; F13's `Two(a, b, w, w)` resolves both drives (false, true at 3 us); a `Logic<60>` param set by pattern and by leaf (F19, both refused).

Judged and accepted: the `Holder { instance, carrier }` IR change is applied at every reader (`netlist.rs` drive pins, `loops.rs` own/other drives, `failure.rs` E0511 names, the printer's `instance#carrier`), and a reached carrier (a parent's drive of a child's `pub` signal) keys by the parent's carrier index, which is what `resolved_ports` looks up; F10-F18 are closed (see their `Closed` cells); tasks.md holds one copy of each task.

### Checks (round 3)

| Check | Test | Holds |
| --- | --- | --- |
| C1 | sourcenatures `a_counter_over_a_project_byte_nature_commits_and_records` | yes - after round-1 F1 (definition, `changed()` and `prev()` on a `Byte`) |
| C2 | sourcenatures `a_project_nature_carrier_connects_to_a_port_of_the_same_nature` | yes - after round-1 F6 (the reader records 0..5) |
| C3 | typesnatures `a_project_discrete_port_without_a_direction_is_e0310` | yes |
| C4 | typesnatures `connecting_byte_to_bit_u8_is_e0251_naming_both_natures` | yes - after round-1 F5 (bare `Byte`) |
| C5 | typesnatures `a_nature_value_without_repr_is_e0112` | yes |
| C6 | typesnatures `a_second_driver_of_a_project_discrete_carrier_is_e0220` | yes |
| C7 | typesnatures `a_generic_project_nature_takes_its_argument_or_its_default` | yes |
| C8 | typesnatures `an_impl_resolve_makes_a_discrete_nature_resolved` | yes |
| C9 | typesnatures `resolve_may_call_a_library_or_project_function` | yes |
| C10 | typesnatures `an_impl_resolve_of_another_shape_is_e0203` | yes - after F11: nine shapes, the three impure ones included |
| C11 | typesnatures `a_resolve_field_is_not_a_nature_field` | yes |
| C12 | typesnatures `a_direction_on_a_resolved_port_is_e0310` | yes |
| C13 | sourceresolved `an_initial_value_is_the_declaring_modules_first_drive` | yes - 13 = the initial 3 plus the other drives, one `changed` |
| C14 | elabnatures `resolve_is_instantiated_once_per_drive_count` | yes - `<0>`..`<2>` and one body for two nets of two drives |
| C15 | sourceresolved `three_summing_drivers_give_their_sum_in_the_microstep_any_changes` | yes - one change, to 7 |
| C16 | sourceresolved `an_undriven_resolved_carrier_holds_resolve_of_no_drive` | yes - 0.0 and `true` |
| C17 | sourceresolved `resolve_receives_the_drives_in_instance_path_order` | yes - after F18: name order (123) and twelve generated drivers in numeric index order (12) |
| C18 | sourceresolved `a_drive_without_initial_value_is_its_types_default_until_its_first_commit` | yes - 7 (the default) then 2 |
| C19 | sourceresolved `a_drive_change_that_leaves_the_value_fires_no_changed` | yes |
| C20 | typesnatures `a_definition_and_a_commit_of_one_resolved_carrier_is_e0221` | yes |
| C21 | sourceresolved `others_is_resolve_of_every_drive_but_the_readers` | yes - each reader's (own, others) pair, three readers |
| C22 | sourceresolved `others_of_a_module_without_a_drive_is_the_value` | yes |
| C23 | typesnatures `others_on_a_one_driver_carrier_is_e0224` | yes |
| C24 | elabnatures `others_depends_on_the_other_drives_only` | yes - only the reader's drive has an others net, resolved with `except` |
| C25 | sourceresolved `a_loop_of_resolved_drives_is_accepted_and_settles` | yes |
| C26 | relax `a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes` | yes - after round-1 F4 |
| C27 | sourcenets `two_parallel_trans_carry_a_strong_level_and_stay_two_carriers` | yes |
| C28 | sourceresolved `a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives` | yes - code, both holders in order, no `$drive` leak |
| C29 | elabnatures `a_loop_through_a_one_driver_carrier_across_instances_is_e0339` | yes |
| C30 | relax `a_released_loop_keeps_no_earlier_level` | yes |
| C31 | relax `an_absent_drive_is_left_out_and_a_present_one_is_passed` | yes (below the language; the generated resolver is C61) |
| C32 | sourcenets `strongest_gives_the_table_of_22_3_2` | yes |
| C33 | sourcenets `strongest_of_equal_drives_is_that_drive_in_any_order` | yes |
| C34 | sourcenets `a_typed_bus_resolves_to_free_byte_or_contended` | yes - after F18: no drive and two released talkers are `Free` |
| C35 | sourcenets `tribuf_drives_strong_or_high_impedance` | yes |
| C36 | sourcenets `a_pull_wins_only_while_every_stronger_drive_is_released` | yes - eight benches, pull-up and pull-down both ways |
| C37 | sourcenets `a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it` | yes - histories, the contended X not kept |
| C38 | sourcenets `tran_drives_each_side_with_the_others_of_the_other` | yes - a to b, b to a, en low |
| C39 | sourcenets `tgate_joins_while_s_high_or_sn_low` | yes |
| C40 | sourceresolved `resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac` | yes |
| C41 | sourcenets `bidir_drives_the_node_while_the_wire_is_driven` | yes - node under `TRAN_VOLTAGE` (existing contract), net unchanged |
| C42 | sourcenets `bidir_drives_the_wire_from_the_node_when_released` | yes |
| C43 | sourcenets `bidir_acts_as_driven_on_a_contended_net` | yes |
| C44 | sourcenets `a_change_of_driving_is_a_time_point_and_a_jump` | yes - a time point at 5 ns and the jump |
| C45 | sourcenets `two_bidirs_alone_stop_the_run_with_e0511` | yes |
| C46 | sourcenatures `a_signal_flow_node_with_one_driver_solves_and_loads_nothing` | yes - after round-1 F3 |
| C47 | sourcenatures `a_gain_chain_over_vsignal_gives_the_product_of_the_gains` | yes - after round-1 F2 (dc sweep) |
| C48 | elabnatures `a_signal_flow_node_with_no_or_two_drivers_is_e0242` | yes |
| C49 | typesnatures `a_flow_contribution_to_a_signal_flow_node_is_e0413` | yes |
| C50 | sourcenatures `vsignal_connected_to_an_electrical_port_is_conservative` | yes |
| C51 | typesnatures `a_flow_probe_of_a_signal_flow_node_is_e0206` | yes |
| C52 | booknatures `the_alarm_prints_three_records_of_the_line` | yes - three records, times, levels and strengths, none at 2 us |
| C53 | clibook `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes - natures#6 held text re-hashed and compared |
| C54 | booknatures `the_pulldown_alarm_prints_the_exercise_answer` | yes |
| C55 | docs `gaps_lists_no_resolved_net_gap` | yes |
| C56 | spectext `c_2_3_and_g_38_say_tran_on_a_resolved_nature_runs` | yes |
| C57 | speccodes `no_owned_code_is_raised` | yes |
| C58 | irresolved `a_resolved_net_is_built_checked_and_printed` | yes |
| C59 | interp `resolved_forms_are_refused_by_name` | yes |
| C60 | planshape `a_resolved_carrier_has_value_drive_and_others_pins` | yes |
| C61 | transresolve `resolve_calls_the_instance_of_the_present_count` | yes |
| C62 | prototran `an_unsettled_loop_is_e0511_naming_its_drives` | yes - one `Error::Unsettled` with tick and drives as data (F10 re-pointed) |
| C63 | elabnatures `a_resolved_drive_without_default_or_initial_value_is_e0187` | yes |
| C64 | bookmodules `the_pi_controller_runs_with_its_generic_idt` | yes - restated for modules#8 running since `generics` C (F10) |
| C65 | bookdigital `the_irq_line_runs_on_its_resolved_nature` | yes - after F14: the listing's `listed` is false, true at 3 us, false at 5 us |
| C66 | bookcorestd `the_resolved_line_runs_on_its_nature` | yes - after F14: `Top.board.irq_n` every half microsecond |
| C67 | bookinventory `refused_rows_and_gaps_section_11_agree` | yes |
| C68 | sourcenatures `an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run` | yes |
| C69 | typesnatures `a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve` | yes |
| C70 | sourcenets `a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type` | yes - four benches and the two-net bench |
| C71 | typesnatures `a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181` | yes |
| C72 | elabnatures `two_driven_types_on_one_net_are_e0225` | yes - code, both paths, both types |
| C73 | sourcenets `one_drive_still_goes_through_resolve` | yes - 5 + 100 |
| C74 | typesnatures `a_library_module_on_a_nature_without_resolve_driven_is_e0187` | yes |
| C75 | booknatures `the_book_no_longer_says_the_alarm_is_refused` | yes |
| C76 | bookdigital `the_four_worked_nets_give_the_tables_of_f_4` | yes - after F14: each listing's own net column |
| C77 | booknatures `signals_of_every_family_run` | yes - judged by elaboration (amendment): E0503 alone, every first value once tied |
| C78 | docs `gaps_section_11_holds_no_net_row` | yes |
| C79 | typesgenmod `a_nature_parameter_bound_by_resolve_is_a_resolved_carrier_in_the_body` | yes |
| C80 | typesgenmod `a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound` | yes |
| C81 | sourcenets `a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature` | yes |
| C82 | typesnatures `an_impure_resolve_is_e0203_naming_its_class` | yes - five cases, the class named |
| C83 | typesaggregates `a_param_of_the_librarys_logic_type_checks` | yes |
| C84 | sourcelogic `a_logic_param_binds_a_pattern_and_a_reaction_reads_it` | yes - both planes of the default and of the bound pattern, written from §22.1.4 |
| C85 | hostpatterns `a_logic_param_is_set_by_its_pattern` | partly - proven at `Logic<4>` only; the >53-level refusal and the leaf write it recommends are untested, and the leaf write fails (F19) |
| C86 | hostpatterns `the_host_drives_a_four_state_in_port` | yes |
| C87 | hostpatterns `a_bad_character_and_a_wrong_length_are_e0702` | yes - `q`, upper-case `X`, short, long, empty |
| C88 | sourcenets `two_carriers_of_one_instance_on_one_net_hold_two_drives` | yes - 12 and 312; the carrier order it pins is not in the spec (F21) |
| C89 | sourcenets `a_tran_with_both_sides_on_one_net_settles_on_the_level` | yes |
| C90 | sourcenets `a_match_over_options_and_a_resolve_walking_option_drives_run` | yes |
| C91 | irresolved `a_resolved_signal_is_no_port_of_a_child_binding` | yes - signal and port, too many and too few |

### Findings (round 3; closed in the round 4 table above)

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F10. (Major) The proofs of C62 and C64 are red at HEAD: no test ran | `proofs.md` at dfa149812: 81 pass, 2 fail. tasks.md:2171 names `an_unsettled_loop_reaches_the_owner_as_one_e0511_fault` (T11 renamed it `an_unsettled_loop_is_e0511_naming_its_drives`, `crates/piperine-solver/tests/protocol_transient.rs:711`, and left C62 unedited although T11's Done-when said to); tasks.md:2177 names `the_pi_controller_is_refused_only_for_its_generic_idt`, gone since `generics` C made modules#8 run | C62's proof names `an_unsettled_loop_is_e0511_naming_its_drives`; C64 is restated for modules#8 running (`the_pi_controller_runs_with_its_generic_idt`) or retired by a recorded amendment | 20acc35e3: C62 names `an_unsettled_loop_is_e0511_naming_its_drives`, C64 `the_pi_controller_runs_with_its_generic_idt`; both green in `proofs.md` at a132da196 (93 pass) |
| F11. (Major) A `resolve` that is not pure is not `E0203`: a solve-reading one reaches rustc as a transpiler defect, an emitting one is `P0004` | §4 rule 8 (`part_ii_carriers.md:85`): "The function MUST be of class pure"; RN-02 AC 3 / C10 name "an impure class". Probe: a fold of the drives plus `now()` passes `piperine check`, and `piperine run` stops with "rustc refused the generated source ... cannot find value `time_`"; calling an `emit!`ing fn gives "P0004 internal error: a trace outside the analog block of a body (function Total.resolve::<f64>::<0>)". C10's test (`types_natures.rs:171`) has no impure case | `E0203` naming nature and `impl` for a `resolve` whose class is not pure (`types/nature.rs` shape rule); add a solve-reading and an emitting case to C10's test | b98acbda8: `ResolveImpls::check_purity` raises E0203 naming the class; C10 gains three impure cases, C82 names the class; round-3 probe: `now()` and an `emit!`ing fn are E0203 |
| F12. (Major) Part VIII §23.1 rule 13 says "A `Logic` is no param", which §8.1.15 contradicts; the struct-param refusal it hides lost its GAPS row | `docs/spec/part_viii_host.md:96` vs `part_iii_structure.md:173` ("A param's type is a value type"; `Logic<N>` is a struct, a value type). Probe: `pat: Logic<4> = logic("10xz");` in a module body is `P0001` "param of type `struct Logic<4>` is outside the M1 subset". dfa149812 deleted L71 from GAPS, whose first half was exactly this; no other GAPS row names a struct-typed param. The amendment was written by the implementer to fit the subset, with no user or coordinator ruling recorded (user rule: code follows spec) | drop the sentence (or record a ruling that changes §8.1.15); reopen a GAPS §4 row for a struct-typed param (and the host's pattern write of a `Logic` param) with its owner | aa16aef3e: the sentence is reverted; a `Logic<N>` param checks and runs (C83, C84, C85). The >53-level limit it left is F19 (round 3) |
| F13. (Major) An instance whose two resolved carriers sit on one net and both drive it is a bind failure, not two drives | `crates/piperine-plan/src/netlist.rs:183`: `holders.position(instance)` binds every carrier of the instance to its first drive slot (the Handoff's stated LIMIT). Probe: `module Two(in a: Bit, in b: Bit, x: AndWire, y: AndWire) { x = !a; y = !b; }` placed as `Two(a, b, w, w)` stops the build with "Digital: bind rule P1 broken: net `Top.w$drive0` has two writers: Top.t.x.drive ... Top.t.y.drive". Loud, not silent (a one-driving-one-reading instance on one net was probed and resolves correctly), but §5.2 gives each written carrier a drive, the message is an internal bind rule rather than a coded diagnostic, and GAPS does not list it. The same holds for `Tran(w, w, en)` | key a holder by (instance, carrier) so each carrier gets its own drive slot, or refuse the form at elaboration with `P0001` naming it and add a GAPS row | 3633739fc: `Resolution::holders` is `Vec<Option<Holder { instance, carrier }>>`; C88 (12, 312) and C89 (`Tran(w, w, en)`); round-3 probe `Two(a, b, w, w)` resolves (false, true at 3 us) |
| F14. (Minor) The Book's net rows assert the bench's copy of each board, not the listing's `Board` | `tests/book/digital.rs:447-450`: `signal listed` takes the listing's `Board`'s `out pending` and no stream or assertion reads it; `tests/book/core_std.rs:52-56` and the digital#17 bench do the same with `Watched`. Probe at HEAD: the listing's `Board` with the digital#16 stimulus records `listed` false at 0, true at 3 us, false at 5 us, so the code is right and only the proof is loose | record and assert `listed` in digital#16 (it equals `pending`); observe the listing's own line (`Top.board.irq_n` or a watcher bound through a reach) for core-std#6 and digital#17 | 2e0710c1c: digital#16 asserts `listed`; digital#17-#19 and core-std#6 assert the listing's own net column (C65, C66, C76) |
| F15. (Minor) tasks.md is corrupted and the records are stale | 0910f7193 inserted a second copy of tasks.md lines 1014-1962 (the tail of the Execution Protocol through T39), starting with the broken line 1013 "## Checks` cites...": every task appears twice; T36's two Done-when boxes are unchecked (tasks.md:1975-1976, `- [ ]`); spec.md Requirement Traceability still says RN-02 "In Tasks", RN-03 "T33 edge cases remain", RN-05 "T31 remains", RN-06 "Started", RN-07, RN-09, RN-10 "In Tasks"; T36's What still says `std_galleries` builds 18 (the 20 is recorded in Handoff B7 (5) and in G.38, which is an accepted amendment) | delete the duplicate block and restore T36's heading position; mark T36; update the traceability table; note the 18 -> 20 amendment in T36 | 20acc35e3, 2dd7beffd: tasks.md holds one copy of each task, T36 marked, traceability updated |
| F16. (Minor) Code cleanliness | free functions: `shape_width` (`crates/piperine-api/src/logic_pattern.rs:76`), `pad_node` and `edge_values` (`crates/piperine-parity/tests/source_nets.rs:290`, `:351`); `Error::Unsettled`'s fields `domain`, `tick`, `drives` carry no `///` (`crates/piperine-analog-solver/src/error.rs:131-135`), against the tasks.md rule "every new or changed ... field ... carries a `///`" | make each an associated function of an owner type (`LogicPattern`, the test's `Model`/`Held`); document the three fields | e4b982d4b: `shape_width`, `pad_node`, `edge_values` are associated functions; `Error::Unsettled` fields documented |
| F17. (Minor) Compiler changes proven only through the Book, and L71 has no check | the digital#20 fix (`elab/aggregate.rs` `field_subject`, `elab/iterators.rs` `bind_pattern` over `Option`) and `CarrierRole::Resolved { port: false }` in the IR child-binding count (`piperine-ir/src/check.rs:559`) have no test in their own crate (MD-28); `Session::drive_pattern` (L71) has tests in `crates/piperine-api/tests/host_patterns.rs` but no `## Checks` row, so no proof runs it. Probes at HEAD passed: a tuple `match` over two `Option`s (3, 104, 3, 7) and a `resolve` with `for` and `fold` over `Option` drives (12, then 120) | a `piperine-lang` (or parity) test for each lowering and an IR test of a child with a resolved signal; a check for L71's tests | e4b982d4b, d843e017c: C90 (Option lowerings), C91 (resolved signal is no port), C86/C87 (L71) |
| F18. (Minor) C17 and C34 prove their rules on part of the cases | C17: name order only, the numeric index order of §4 rule 2 is untested (probe: holds); C34: "`Free` for ... only `None`s" has no bench (`nets/bus.msl:139` has no drive at all) | add a generated bench of 11+ drivers to C17's test and a two-released-talkers bench to C34's | e4b982d4b: C17 adds `IndexBench` (12 generated drivers, 12.0), C34 adds two released talkers (`Free`) |
| F19. (Major) A `Logic<N>` param of more than 53 levels cannot be set by the host at all, and Part VIII rule 13 recommends a path that is refused too | `crates/piperine-api/src/param_text.rs` `Reading::pattern`: `width > 53` is E0802 "set the leaves `binary` and `value` as numbers". Probe at 97842adab, `pat: Logic<60>`: `--param Top.l.pat=1000...0001` (60 levels) is E0802, and `--param Top.l.pat.binary=1152921504606846977` is E0802 "an `f64` cannot hold it exactly" (`Params::leaf_number`/`leaf_change` take an `f64`). The limit is not real: `Params::changes_composite` (`params.rs:486`) already writes a composite param's exact bytes, `LogicPattern::bytes` gives exactly those bytes, and `params-and-io` made integer params exact (`changes_exact`, `changes_integer`). `docs/spec/part_viii_host.md` rule 13 ("a pattern of more than 53 levels is set by its leaves") states a limit and a workaround that do not hold. No test covers N > 53 (C85 is `Logic<4>` only) | write the pattern's bytes through `changes_composite` for every `N` (no f64 round trip), drop the 53-level sentence from rule 13, and test `Logic<60>` and `Logic<100>` patterns in C85's test | |
| F20. (Minor) The IR checks of a net's `Resolution` are untested, the new `Holder` carrier check included | `crates/piperine-ir/src/check/resolved.rs:61-72`: "holders for drives", "holder instance out of range" and the new "holder carrier {} out of range for instance" are raised by no test (`grep` of `crates/*/tests`). The check also accepts a holder whose carrier is in range but not `CarrierRole::Resolved` | a negative test in `crates/piperine-ir/tests/resolved.rs` for each message; refuse a holder carrier whose role is not `Resolved` | |
| F21. (Minor) The order of two drives of one instance is observable and pinned by C88, but the spec does not state it | §4 rule 2's Resolution orders drives by the holders' instance paths only (`docs/spec/part_ii_carriers.md:129-132`); `elab/resolvers.rs` breaks the tie by carrier index, and `resolve` sees it (`Twin(n, n)` gives 12, not 21). Code follows spec: the order is a rule that needs its sentence | add to §4 rule 2's Resolution that the drives of one instance follow its carriers' declaration order (with `spec_text.rs`), or record the question in GAPS §5 | |
| F22. (Minor) `validate_tasks.py` still fails, and T29a's elaboration test has no check | `validate_tasks.py resolved-nets`: "ERROR T28 declares `Depends on: T28`": the heading `### T29a:` is no task ID the validator reads, so T29a's body (its `Depends on: T28`, its `Where` list) is parsed as T28's; WARN "`Run elabgenmod` is read by no proof": `a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature` (T29a's Done-when) has no check | give T29a a plain task ID (T40) with the phase diagram arrow, and add a check for the elaboration test | |

## Round 2 (B3-B7, T29a, L71, 2026-11-24)

**Verdict (round 2)**: FAIL
**Proofs (round 2)**: `proofs.md` at dfa149812 (81 pass, 2 fail: C62, C64; build pass)
**Verifier (round 2)**: independent sub-agent (author != verifier), Opus, round 2 (batches B3-B7, T29a, L71; B1-B2 judged in round 1 below)
**Scope (round 2)**: `a97f306bb..dfa149812` (the feature's own commits; merges of `main` read, not judged). Targeted tests only, per the user's gate policy: `check_proofs.py` ran every `Run` group once at HEAD with the `**Build**` line (zero warnings, `suite_hygiene` green). The final full suite and the merge of `origin/main` were NOT run: the verdict is FAIL, and the brief runs them only after a PASS.

Probes run at HEAD with the debug `piperine` binary (throwaway files outside the tree, not committed): two driving carriers of one instance on one net (F13); one driving and one reading carrier of one instance on one net (resolves correctly); the listing's own `Board` of digital#16 (F14, correct); a `Logic<4>` param (F12); an impure `resolve` (F11); a tuple `match` over `Option`s and `for`/`fold` over `Option` drives in a `resolve` (F17, correct); 12 generated drivers in index order (F18, correct).

Judged and accepted: T29a follows spec (`W: Nature + Resolve<Driven>` is in Part I §3.7.13 and Part VII §22.3; `Bound::Resolve`, `ResolveImpls::of_parameter`, `concrete_implementation` picking the instance nature's `impl` by `X`); a parent driving a child's `pub` resolved signal is right for the single-carrier case (F13 is the multi-carrier limit); the `relax()` order fix (functions writing no `RELAX` net first) matches §8.2.3 steps 1-2; `Error::Unsettled` carries tick and drives as data and `LoopDrives` reads them, E0511 text unchanged; `CarrierRole::Resolved { port }` fixes the child-binding count (P0004 of digital#16), the moved fixtures say `port: true` for ports; core-std#6's E0187 decision is legitimate (Part VIII rule 9, H.8 rule 4: `bool` has no `Default`; the listing has `in` ports and is not a top), the bench wrappers are the right vehicle (F14 is only about what they assert); the gallery count 20 is recorded (Handoff B7 (5), G.38, commit 0910f7193) and the fences are real; natures#2 by elaboration is the recorded amendment; GAPS L21, L22, L23, the L58 net group and its counts, STATE MD-83, ROADMAP and README are consistent. No tolerance or parity contract edit: `git diff a97f306bb..HEAD -- crates/piperine-parity/src tests/support` holds no `ToleranceContract` change.

### Checks (round 2)

| Check | Test | Holds |
| --- | --- | --- |
| C1 | sourcenatures `a_counter_over_a_project_byte_nature_commits_and_records` | yes - after round-1 F1 (definition, `changed()` and `prev()` on a `Byte`) |
| C2 | sourcenatures `a_project_nature_carrier_connects_to_a_port_of_the_same_nature` | yes - after round-1 F6 (the reader records 0..5) |
| C3 | typesnatures `a_project_discrete_port_without_a_direction_is_e0310` | yes |
| C4 | typesnatures `connecting_byte_to_bit_u8_is_e0251_naming_both_natures` | yes - after round-1 F5 (bare `Byte`) |
| C5 | typesnatures `a_nature_value_without_repr_is_e0112` | yes |
| C6 | typesnatures `a_second_driver_of_a_project_discrete_carrier_is_e0220` | yes |
| C7 | typesnatures `a_generic_project_nature_takes_its_argument_or_its_default` | yes |
| C8 | typesnatures `an_impl_resolve_makes_a_discrete_nature_resolved` | yes |
| C9 | typesnatures `resolve_may_call_a_library_or_project_function` | yes |
| C10 | typesnatures `an_impl_resolve_of_another_shape_is_e0203` | no - six shapes, but the criterion's "impure class" is not among them, and it fails (F11) |
| C11 | typesnatures `a_resolve_field_is_not_a_nature_field` | yes |
| C12 | typesnatures `a_direction_on_a_resolved_port_is_e0310` | yes |
| C13 | sourceresolved `an_initial_value_is_the_declaring_modules_first_drive` | yes - 13 = the initial 3 plus the other drives, one `changed` |
| C14 | elabnatures `resolve_is_instantiated_once_per_drive_count` | yes - `<0>`..`<2>` and one body for two nets of two drives |
| C15 | sourceresolved `three_summing_drivers_give_their_sum_in_the_microstep_any_changes` | yes - one change, to 7 |
| C16 | sourceresolved `an_undriven_resolved_carrier_holds_resolve_of_no_drive` | yes - 0.0 and `true` |
| C17 | sourceresolved `resolve_receives_the_drives_in_instance_path_order` | partly - name order (c, a, b written; 123) only; the numeric index order is untested (probe: 12 generated drivers arrive `cell[0]`..`cell[11]`, holds) and host-first lives in C-less `the_hosts_drive_of_a_top_port_comes_first` (F18) |
| C18 | sourceresolved `a_drive_without_initial_value_is_its_types_default_until_its_first_commit` | yes - 7 (the default) then 2 |
| C19 | sourceresolved `a_drive_change_that_leaves_the_value_fires_no_changed` | yes |
| C20 | typesnatures `a_definition_and_a_commit_of_one_resolved_carrier_is_e0221` | yes |
| C21 | sourceresolved `others_is_resolve_of_every_drive_but_the_readers` | yes - each reader's (own, others) pair, three readers |
| C22 | sourceresolved `others_of_a_module_without_a_drive_is_the_value` | yes |
| C23 | typesnatures `others_on_a_one_driver_carrier_is_e0224` | yes |
| C24 | elabnatures `others_depends_on_the_other_drives_only` | yes - only the reader's drive has an others net, resolved with `except` |
| C25 | sourceresolved `a_loop_of_resolved_drives_is_accepted_and_settles` | yes |
| C26 | relax `a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes` | yes - after round-1 F4 |
| C27 | sourcenets `two_parallel_trans_carry_a_strong_level_and_stay_two_carriers` | yes |
| C28 | sourceresolved `a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives` | yes - code, both holders in order, no `$drive` leak |
| C29 | elabnatures `a_loop_through_a_one_driver_carrier_across_instances_is_e0339` | yes |
| C30 | relax `a_released_loop_keeps_no_earlier_level` | yes |
| C31 | relax `an_absent_drive_is_left_out_and_a_present_one_is_passed` | yes (below the language; the generated resolver is C61) |
| C32 | sourcenets `strongest_gives_the_table_of_22_3_2` | yes |
| C33 | sourcenets `strongest_of_equal_drives_is_that_drive_in_any_order` | yes |
| C34 | sourcenets `a_typed_bus_resolves_to_free_byte_or_contended` | partly - `Free` is proven for no drive only; "only `None`s" (two released talkers) has no bench (F18) |
| C35 | sourcenets `tribuf_drives_strong_or_high_impedance` | yes |
| C36 | sourcenets `a_pull_wins_only_while_every_stronger_drive_is_released` | yes - eight benches, pull-up and pull-down both ways |
| C37 | sourcenets `a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it` | yes - histories, the contended X not kept |
| C38 | sourcenets `tran_drives_each_side_with_the_others_of_the_other` | yes - a to b, b to a, en low |
| C39 | sourcenets `tgate_joins_while_s_high_or_sn_low` | yes |
| C40 | sourceresolved `resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac` | yes |
| C41 | sourcenets `bidir_drives_the_node_while_the_wire_is_driven` | yes - node under `TRAN_VOLTAGE` (existing contract), net unchanged |
| C42 | sourcenets `bidir_drives_the_wire_from_the_node_when_released` | yes |
| C43 | sourcenets `bidir_acts_as_driven_on_a_contended_net` | yes |
| C44 | sourcenets `a_change_of_driving_is_a_time_point_and_a_jump` | yes - a time point at 5 ns and the jump |
| C45 | sourcenets `two_bidirs_alone_stop_the_run_with_e0511` | yes |
| C46 | sourcenatures `a_signal_flow_node_with_one_driver_solves_and_loads_nothing` | yes - after round-1 F3 |
| C47 | sourcenatures `a_gain_chain_over_vsignal_gives_the_product_of_the_gains` | yes - after round-1 F2 (dc sweep) |
| C48 | elabnatures `a_signal_flow_node_with_no_or_two_drivers_is_e0242` | yes |
| C49 | typesnatures `a_flow_contribution_to_a_signal_flow_node_is_e0413` | yes |
| C50 | sourcenatures `vsignal_connected_to_an_electrical_port_is_conservative` | yes |
| C51 | typesnatures `a_flow_probe_of_a_signal_flow_node_is_e0206` | yes |
| C52 | booknatures `the_alarm_prints_three_records_of_the_line` | yes - three records, times, levels and strengths, none at 2 us |
| C53 | clibook `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes - natures#6 held text re-hashed and compared |
| C54 | booknatures `the_pulldown_alarm_prints_the_exercise_answer` | yes |
| C55 | docs `gaps_lists_no_resolved_net_gap` | yes |
| C56 | spectext `c_2_3_and_g_38_say_tran_on_a_resolved_nature_runs` | yes |
| C57 | speccodes `no_owned_code_is_raised` | yes |
| C58 | irresolved `a_resolved_net_is_built_checked_and_printed` | yes |
| C59 | interp `resolved_forms_are_refused_by_name` | yes |
| C60 | planshape `a_resolved_carrier_has_value_drive_and_others_pins` | yes |
| C61 | transresolve `resolve_calls_the_instance_of_the_present_count` | yes |
| C62 | prototran `an_unsettled_loop_reaches_the_owner_as_one_e0511_fault` | no - no test of that name exists at HEAD (its successor is `an_unsettled_loop_is_e0511_naming_its_drives`): the proof is red (F10) |
| C63 | elabnatures `a_resolved_drive_without_default_or_initial_value_is_e0187` | yes |
| C64 | bookmodules `the_pi_controller_is_refused_only_for_its_generic_idt` | no - no test of that name exists at HEAD: modules#8 runs since `generics` C (`the_pi_controller_runs_with_its_generic_idt`); the claim itself is obsolete (F10) |
| C65 | bookdigital `the_irq_line_runs_on_its_resolved_nature` | partly - the outcomes are asserted on the bench's copy `Watched`; the listing's own `Board` is placed with `out pending` on `listed`, which is never asserted (probe: the listing gives false, true at 3 us, false at 5 us - the code is right) (F14) |
| C66 | bookcorestd `the_resolved_line_runs_on_its_nature` | partly - as C65: the records come from the bench's copy `Watched`, the listing's `Board` only has to run (F14) |
| C67 | bookinventory `refused_rows_and_gaps_section_11_agree` | yes |
| C68 | sourcenatures `an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run` | yes |
| C69 | typesnatures `a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve` | yes |
| C70 | sourcenets `a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type` | yes - four benches and the two-net bench |
| C71 | typesnatures `a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181` | yes |
| C72 | elabnatures `two_driven_types_on_one_net_are_e0225` | yes - code, both paths, both types |
| C73 | sourcenets `one_drive_still_goes_through_resolve` | yes - 5 + 100 |
| C74 | typesnatures `a_library_module_on_a_nature_without_resolve_driven_is_e0187` | yes |
| C75 | booknatures `the_book_no_longer_says_the_alarm_is_refused` | yes |
| C76 | bookdigital `the_four_worked_nets_give_the_tables_of_f_4` | partly - the F.4 tables are asserted on the bench's `Watched` copies; the listing's `Board` is placed but unobserved (F14) |
| C77 | booknatures `signals_of_every_family_run` | yes - judged by elaboration (amendment): E0503 alone, every first value once tied |
| C78 | docs `gaps_section_11_holds_no_net_row` | yes |
| C79 | typesgenmod `a_nature_parameter_bound_by_resolve_is_a_resolved_carrier_in_the_body` | yes |
| C80 | typesgenmod `a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound` | yes |
| C81 | sourcenets `a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature` | yes |

### Findings (round 2; closed in the round 3 table above)

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F10. (Major) The proofs of C62 and C64 are red at HEAD: no test ran | `proofs.md` at dfa149812: 81 pass, 2 fail. tasks.md:2171 names `an_unsettled_loop_reaches_the_owner_as_one_e0511_fault` (T11 renamed it `an_unsettled_loop_is_e0511_naming_its_drives`, `crates/piperine-solver/tests/protocol_transient.rs:711`, and left C62 unedited although T11's Done-when said to); tasks.md:2177 names `the_pi_controller_is_refused_only_for_its_generic_idt`, gone since `generics` C made modules#8 run | C62's proof names `an_unsettled_loop_is_e0511_naming_its_drives`; C64 is restated for modules#8 running (`the_pi_controller_runs_with_its_generic_idt`) or retired by a recorded amendment |  |
| F11. (Major) A `resolve` that is not pure is not `E0203`: a solve-reading one reaches rustc as a transpiler defect, an emitting one is `P0004` | §4 rule 8 (`part_ii_carriers.md:85`): "The function MUST be of class pure"; RN-02 AC 3 / C10 name "an impure class". Probe: a fold of the drives plus `now()` passes `piperine check`, and `piperine run` stops with "rustc refused the generated source ... cannot find value `time_`"; calling an `emit!`ing fn gives "P0004 internal error: a trace outside the analog block of a body (function Total.resolve::<f64>::<0>)". C10's test (`types_natures.rs:171`) has no impure case | `E0203` naming nature and `impl` for a `resolve` whose class is not pure (`types/nature.rs` shape rule); add a solve-reading and an emitting case to C10's test |  |
| F12. (Major) Part VIII §23.1 rule 13 says "A `Logic` is no param", which §8.1.15 contradicts; the struct-param refusal it hides lost its GAPS row | `docs/spec/part_viii_host.md:96` vs `part_iii_structure.md:173` ("A param's type is a value type"; `Logic<N>` is a struct, a value type). Probe: `pat: Logic<4> = logic("10xz");` in a module body is `P0001` "param of type `struct Logic<4>` is outside the M1 subset". dfa149812 deleted L71 from GAPS, whose first half was exactly this; no other GAPS row names a struct-typed param. The amendment was written by the implementer to fit the subset, with no user or coordinator ruling recorded (user rule: code follows spec) | drop the sentence (or record a ruling that changes §8.1.15); reopen a GAPS §4 row for a struct-typed param (and the host's pattern write of a `Logic` param) with its owner |  |
| F13. (Major) An instance whose two resolved carriers sit on one net and both drive it is a bind failure, not two drives | `crates/piperine-plan/src/netlist.rs:183`: `holders.position(instance)` binds every carrier of the instance to its first drive slot (the Handoff's stated LIMIT). Probe: `module Two(in a: Bit, in b: Bit, x: AndWire, y: AndWire) { x = !a; y = !b; }` placed as `Two(a, b, w, w)` stops the build with "Digital: bind rule P1 broken: net `Top.w$drive0` has two writers: Top.t.x.drive ... Top.t.y.drive". Loud, not silent (a one-driving-one-reading instance on one net was probed and resolves correctly), but §5.2 gives each written carrier a drive, the message is an internal bind rule rather than a coded diagnostic, and GAPS does not list it. The same holds for `Tran(w, w, en)` | key a holder by (instance, carrier) so each carrier gets its own drive slot, or refuse the form at elaboration with `P0001` naming it and add a GAPS row |  |
| F14. (Minor) The Book's net rows assert the bench's copy of each board, not the listing's `Board` | `tests/book/digital.rs:447-450`: `signal listed` takes the listing's `Board`'s `out pending` and no stream or assertion reads it; `tests/book/core_std.rs:52-56` and the digital#17 bench do the same with `Watched`. Probe at HEAD: the listing's `Board` with the digital#16 stimulus records `listed` false at 0, true at 3 us, false at 5 us, so the code is right and only the proof is loose | record and assert `listed` in digital#16 (it equals `pending`); observe the listing's own line (`Top.board.irq_n` or a watcher bound through a reach) for core-std#6 and digital#17 |  |
| F15. (Minor) tasks.md is corrupted and the records are stale | 0910f7193 inserted a second copy of tasks.md lines 1014-1962 (the tail of the Execution Protocol through T39), starting with the broken line 1013 "## Checks` cites...": every task appears twice; T36's two Done-when boxes are unchecked (tasks.md:1975-1976, `- [ ]`); spec.md Requirement Traceability still says RN-02 "In Tasks", RN-03 "T33 edge cases remain", RN-05 "T31 remains", RN-06 "Started", RN-07, RN-09, RN-10 "In Tasks"; T36's What still says `std_galleries` builds 18 (the 20 is recorded in Handoff B7 (5) and in G.38, which is an accepted amendment) | delete the duplicate block and restore T36's heading position; mark T36; update the traceability table; note the 18 -> 20 amendment in T36 |  |
| F16. (Minor) Code cleanliness | free functions: `shape_width` (`crates/piperine-api/src/logic_pattern.rs:76`), `pad_node` and `edge_values` (`crates/piperine-parity/tests/source_nets.rs:290`, `:351`); `Error::Unsettled`'s fields `domain`, `tick`, `drives` carry no `///` (`crates/piperine-analog-solver/src/error.rs:131-135`), against the tasks.md rule "every new or changed ... field ... carries a `///`" | make each an associated function of an owner type (`LogicPattern`, the test's `Model`/`Held`); document the three fields |  |
| F17. (Minor) Compiler changes proven only through the Book, and L71 has no check | the digital#20 fix (`elab/aggregate.rs` `field_subject`, `elab/iterators.rs` `bind_pattern` over `Option`) and `CarrierRole::Resolved { port: false }` in the IR child-binding count (`piperine-ir/src/check.rs:559`) have no test in their own crate (MD-28); `Session::drive_pattern` (L71) has tests in `crates/piperine-api/tests/host_patterns.rs` but no `## Checks` row, so no proof runs it. Probes at HEAD passed: a tuple `match` over two `Option`s (3, 104, 3, 7) and a `resolve` with `for` and `fold` over `Option` drives (12, then 120) | a `piperine-lang` (or parity) test for each lowering and an IR test of a child with a resolved signal; a check for L71's tests |  |
| F18. (Minor) C17 and C34 prove their rules on part of the cases | C17: name order only, the numeric index order of §4 rule 2 is untested (probe: holds); C34: "`Free` for ... only `None`s" has no bench (`nets/bus.msl:139` has no drive at all) | add a generated bench of 11+ drivers to C17's test and a two-released-talkers bench to C34's |  |

## Round 1 (B1-B2, 2026-11-21)

**Verdict (round 1)**: PASS
**Proofs (round 1)**: `proofs.md` at 73cef16b
**Verifier (round 1)**: independent sub-agent (author != verifier)
**Scope (round 1)**: partial merge (fix round 2026-11-21: F1-F7 and orchestrator decisions 9 and 10 closed below; verdict PASS for the in-scope checks), batches B1-B2 (T1-T10) only. Every check owned by T11-T36 is deferred (it waits on `composite-values` A-B, `value-semantics`, `generics` A); its proof is red or has no test at this HEAD, as expected, and is not judged here.

Test runs at 22fc40b4 (`CARGO_TARGET_DIR=/var/tmp/piperine-target-a`): `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` at the root, exit 0, 308 targets, 3186 passed, 0 failed; `cargo test -p piperine-digital-solver --test relax` 4/4; `cargo test -p piperine-parity --test source_natures` 5/5; `types_natures` 8/8, `elab_natures` 2/2. `check_proofs.py`: every in-scope check passes, the build passes, and the 49 failing rows all belong to deferred tasks. No contract or tolerance edits: `git diff origin/main -- crates/piperine-parity/src tests/support crates/piperine-transpile/tests/golden` is empty. GAPS counts recomputed from the files: 36 `| L` rows in §4, and the §10 list holds 44 codes, none of which `crates/*/src` raises (189 codes in `docs/spec`). abi.md §9 D1 contains the D3 `driving` sentence. D16: the rows for E0224, E0242 and E0413 are deleted; E0203 stays with T16.

### Checks (round 1)

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

### Findings (round 1)

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

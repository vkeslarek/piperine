# Analog Structure verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 24a19f26 (HEAD bade8806 adds only `proofs.md`)
**Verifier**: independent sub-agent (author != verifier)
**Fix round**: F1 to F9 closed by the commits of the `Closed` column; `check_proofs.py` rerun at the fix head (round 2).

Range judged: `git diff origin/main...HEAD` (132 files). Own examples run at HEAD on the Rust
backend (throwaway, not committed): a flow `ac_stim` under a statement `if` (AS-06 AC 4 as the spec
writes it: |v(a)| = 1 / 0, holds), `i(gnd, a)` under an arm beside `i(a, gnd)` at the top (edge case:
one branch, holds), a zero potential written `0.0 * 5.0` (merges, holds), the named `Lag` of AS-09
AC 5 (one hoisted frame `l`, holds), a parent reading `c.state` in a reaction and a definition
(holds), E0415 through a value input bound to a param (fails, F2), a DC sweep of a structural key
param (fails silently, F1), `--print` of a merged node (fails, F3). Solver touch (`NetSpec::param`,
`Store::seed`, `Digital::seed_param`, `DigitalSide::start`): every analysis starts through the one
`DigitalSide::start`, a set between analyses is a host write of `on_start` (seeded, no change), a timed
`set_at` stays a `host_commit` (a change); the analog copy is filled from `host_writes`/`init`
independently of the seed. Correct as written. `contract.rs` and the transpile goldens are unchanged
(`git diff --stat` empty); `parity_corpus` 16 -> 18 is an equality on the two new twins;
`COMPILED_ONLY` gains `uic_rc`/`diode_rs`, which the interpreter refuses by name (checked:
`DiodeRs~rs=100.0 holds a contribution under a selector`). User rulings applied: AS-09 AC 5 amended
to the numerical crossing with the Part V §17.3 Resolution; AC 6 narrowed with `GAPS.md` L60 and
AO-13 in `analog-operators/spec.md`. Lifted tests keep their names and assert the lowering the spec
now requires; `buck.txt` `gnd` -> `own:<name>` is exactly S4/AS-16. No new free function
(`pad_child_nodes` is a method of `Elaboration`).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `piperine-parity/tests/source_switch.rs` `a_flow_contribution_under_an_if_on_a_probe_gives_one_volt` | yes |
| C2 | `source_switch.rs` `a_match_on_a_carrier_adds_only_the_selected_arm` | yes |
| C3 | `piperine-lang/tests/elab_structure.rs` `a_contribution_in_a_nested_block_lowers_as_at_the_top` | yes |
| C4 | `source_switch.rs` `an_untaken_contribution_leaves_the_branch_with_zero_flow` | yes |
| C5 | `source_switch.rs` `a_bound_step_under_an_if_caps_only_the_steps_that_executed_it`; `assembly_watch.rs` `a_cap_of_infinity_caps_nothing` | yes |
| C6 | `elab_structure.rs` `a_contribution_in_a_closure_is_refused_at_parse` | yes - fixed in 94d1e0f4: asserts E0404 and the span of the contribution |
| C7 | `elab_structure.rs` `a_probe_in_the_selector_of_a_potential_capable_branch_is_e0414` | yes |
| C8 | `elab_structure.rs` `a_selector_through_a_value_input_is_judged_per_placement` | yes - fixed in 60ee0c33: asserts the fixed kind and `k` structural, plus the E0415 and merge cases |
| C9 | `elab_structure.rs` `params_only_selectors_fix_the_kind_and_mark_the_params_structural` | yes |
| C10 | `elab_structure.rs` `a_fixed_zero_potential_branch_merges_its_nodes`; `parity_op.rs` `diode_rs_with_rs_zero_merges_and_matches_rdiode` | yes (the block by a flow read and by `inflow()` is held by `a_zero_potential_branch_whose_flow_is_read_is_not_merged` and `source_reach::inflow_of_a_port_held_at_zero_is_not_merged_away`) |
| C11 | `elab_structure.rs` `params_only_selectors_enabling_both_kinds_are_e0415` | yes (direct params only; see F2 for a value input) |
| C12 | `elab_structure.rs` `a_selector_on_a_carrier_or_the_environment_keeps_the_flow_unknown` | yes |
| C13 | `source_switch.rs` `the_switch_shorts_at_one_microsecond` | yes - fixed in 94d1e0f4: reads the branch flow under DC_CURRENT |
| C14 | `source_switch.rs` `a_switch_changes_kind_at_the_step_after_the_instant` | yes |
| C15 | `source_switch.rs` `both_kinds_enabled_in_one_solve_fail_with_e0418`; `piperine-transpile/tests/structure.rs` `both_kinds_enabled_return_e0418_naming_the_branch` | yes - fixed in 94d1e0f4: the run-level text names `BothBench.both` and `branch v(a)` |
| C16 | `source_switch.rs` `an_ac_sweep_takes_the_switch_kind_with_static_false` | yes |
| C17 | `parity_tran.rs` `uic_rc` | yes |
| C18 | `source_switch.rs` `capic_has_the_admittance_of_a_capacitor_in_ac` | yes |
| C19 | `source_derivative.rs` `bounded_holds_its_state_at_the_bound` | yes |
| C20 | `source_modules.rs` `integ_holds_ic_while_reset_then_integrates` | yes |
| C21 | `source_modules.rs` `integ_is_one_over_j_omega_in_ac` | yes |
| C22 | `source_modules.rs` `integ_holds_its_result_at_ic_at_the_operating_point` | yes |
| C23 | `elab_structure.rs` `the_stdlib_idt_waits_for_generics` | yes (WHERE clause; refusal by name) |
| C24 | `elab_structure.rs` `the_water_model_waits_for_generics` | yes (WHERE clause; refusal by name) |
| C25 | `elab_structure.rs` `the_section_18_diode_waits_for_params_and_io`; `parity_op.rs` `diode_rs` | yes |
| C26 | `piperine-cli/tests/docs.rs` `gaps_keeps_l40_for_the_spec_models_that_wait` | yes |
| C27 | `source_switch.rs` `a_limit_under_an_untaken_branch_neither_vetoes_nor_moves` | yes - fixed in 94d1e0f4: no veto proven; the held value stays unobservable in the bench (settled in tasks.md) |
| C28 | `source_switch.rs` `a_junction_enabled_after_the_operating_point_reaches_the_rdiode_solution` | yes - fixed in 94d1e0f4: compares the closed form of the rdiode circuit (and ngspice when present) |
| C29 | `source_switch.rs` `a_limit_reads_a_carrier_through_known_or` | no - the limiter is the identity `|new, old| yes - fixed in 94d1e0f4: limits the sum `lv + 0.5 v(a)` through a damped step (a clamp never converges, settled) |
| C30 | `source_switch.rs` `an_ac_stim_under_an_if_drives_only_when_enabled` | yes - fixed in 94d1e0f4: adds the AC statement form `if en.high() { i(a, gnd) +> ac_stim(1m); }` |
| C31 | `source_switch.rs` `an_ac_stim_reads_its_magnitude_from_a_carrier` | yes |
| C32 | `parity_tran.rs` `tran_rc_with_a_scaled_derivative`; `parity_ac.rs` `ac_rc_with_a_scaled_derivative` | yes |
| C33 | `source_derivative.rs` `a_derivative_times_a_probe_is_k_v_dv_dt` | yes |
| C34 | `elab_structure.rs` `a_body_let_of_ddt_read_as_an_addend_gives_the_inline_rows` | yes |
| C35 | `source_derivative.rs` `an_event_reads_the_derivative_of_the_step` | yes - fixed in 94d1e0f4: asserts 0 at an equilibrium |
| C36 | `piperine-plan/tests/structure_shape.rs` `a_derivative_abstol_names_its_pin`; `source_derivative.rs` `a_looser_derivative_abstol_takes_fewer_steps` | yes |
| C37 | `source_modules.rs` `a_value_input_of_a_module_without_a_result_drives_its_contribution` | yes |
| C38 | `source_modules.rs` `a_value_input_is_differentiated_in_ac` | yes |
| C39 | `source_modules.rs` `a_value_input_reads_the_callers_carrier_from_its_change` | yes |
| C40 | `piperine-lang/tests/types_structure.rs` `a_continuous_argument_read_by_a_callee_definition_is_e0336` | yes |
| C41 | `source_modules.rs` `a_value_input_read_in_a_reaction_is_the_callers_value_at_that_instant` | yes |
| C42 | `source_modules.rs` `lag_follows_one_minus_exp` | yes |
| C43 | `source_modules.rs` `lag_is_a_first_order_low_pass_in_ac` | yes |
| C44 | `elab_structure.rs` `two_placements_with_equal_arguments_are_two_instances` | yes |
| C45 | `source_modules.rs` `a_lag_under_an_untaken_branch_keeps_integrating` | yes |
| C46 | `source_modules.rs` `a_named_lag_is_one_instance_for_analog_and_its_event` | yes - fixed in 94d1e0f4: asserts `hoisted()` is `["l"]` |
| C47 | `source_modules.rs` `slew_bounds_the_rate_and_tracks_at_the_operating_point` | yes (finite edge, as amended) |
| C48 | `source_modules.rs` `feedback_through_integ_decays_exponentially` | yes |
| C49 | `types_structure.rs` `a_cycle_with_a_direct_dependence_at_every_result_is_e0353` | yes |
| C50 | `types_structure.rs` `feedback_through_an_unknown_of_a_module_is_accepted` | yes (incl. a mixed cycle `Integ(twice(y))`) |
| C51 | `source_reach.rs` `the_parent_probes_a_pub_node_of_its_child` | yes |
| C52 | `source_reach.rs` `the_parent_loads_a_pub_node_of_its_child` | yes |
| C53 | `source_reach.rs` `the_parent_reads_a_pub_carrier_of_its_child` | yes - fixed in 94d1e0f4: runs the reaction and the definition reads |
| C54 | `source_reach.rs` `the_parent_reads_a_pub_param_and_a_pub_let_of_its_child` | yes |
| C55 | `types_structure.rs` `inst_name_of_an_item_that_is_not_pub_is_e0343` | yes |
| C56 | `types_structure.rs` `a_commit_to_a_childs_carrier_is_e0222` | yes |
| C57 | `source_params.rs` `a_body_let_as_a_param_value_sets_the_corner` | yes |
| C58 | `elab_structure.rs` `a_param_value_let_may_read_another_let` | yes |
| C59 | `elab_structure.rs` `a_param_value_let_that_reads_a_probe_is_e0321` | yes - fixed in 94d1e0f4: adds `let p = Lag(1.0);` |
| C60 | `piperine-api/tests/bindings.rs` `a_binding_of_k_recomputes_the_scale_a_body_let_derives` | yes (a before-run host set; the name says capacitance, the model a scale) |
| C61 | `source_reach.rs` `inflow_sums_the_flows_of_the_modules_branches_at_the_port` | yes |
| C62 | `source_reach.rs` `inflow_includes_a_childs_branch_on_the_port` | yes |
| C63 | `source_reach.rs` `inflow_is_differentiated_in_ac` | yes |
| C64 | `types_structure.rs` `inflow_of_a_node_that_is_not_a_port_is_e0243` | yes |
| C65 | `types_structure.rs` `inflow_on_the_left_of_a_contribution_is_e0410` | yes |
| C66 | `bindings.rs` `a_timed_set_fires_changed_once_at_its_time` | yes |
| C67 | `bindings.rs` `prev_of_a_param_reads_its_own_value_then_the_value_before_a_set` | yes |
| C68 | `bindings.rs` `a_binding_before_the_run_fires_no_changed` | yes |
| C69 | `bindings.rs` `an_initial_value_over_a_param_follows_its_binding` | yes - fixed in 94d1e0f4: reads `s` at `at(0)` |
| C70 | `source_params.rs` `a_node_initial_value_over_a_param_holds_the_operating_point` | yes - fixed in cc55b74e: an `op` of a three-equilibrium cell starts Newton at +-0.6 V (and `DcSolver::start_values` now does) |
| C71 | `types_structure.rs` `an_initial_value_that_reads_a_carrier_is_e0210` | yes |
| C72 | `elab_structure.rs` `a_childs_own_signal_is_bound_to_own_in_the_authored_child` | yes (for `Signal` nodes; see F4 for reached nodes) |
| C73 | `piperine-ir/tests/structure.rs` `an_own_terminal_prints_and_round_trips` | yes |
| C74 | `elab_structure.rs` `a_design_with_own_terminals_passes_check` | yes |
| C75 | `piperine-interp/tests/frozen.rs` `the_interpreter_refuses_each_analog_structure_form_by_name` | yes |
| C76 | `piperine-cli/tests/twins.rs` `every_twin_prints_the_bytes_it_printed_before_the_long_run`, `the_two_backends_print_the_same_bytes`; `piperine-transpile/tests/golden.rs` `the_five_m1_bodies_print_as_the_golden_files` | yes |
| C77 | `parity_tran.rs` `tran_rc`, `parity_ac.rs` `ac_rc`, `parity_op.rs` `divider`, `parity_dc.rs` `diode_iv`, `parity_corpus.rs` `every_twin_has_a_cir_and_a_test`; `git diff --exit-code origin/main -- contract.rs` | yes |
| C78 | `parity_tran.rs` `uic_rc_runs_and_prints_the_skip_line_without_ngspice`; `parity_op.rs` `diode_rs_runs_and_prints_the_skip_line_without_ngspice` | yes |
| C79 | `docs.rs` `gaps_drops_what_analog_structure_closed`; `piperine-cli/tests/book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 [Blocker, silent] A host write of a structural key param after elaboration runs the variant of the OLD value: a DC sweep of `Top.m.short` prints the wrong voltage and no error | `Sw` of `piperine-api/tests/bindings.rs` (`if short > 0.5 { v(a, b) +> 0.25; } else { i(a, b) +> 0.0; }`): `piperine run sw.msl --analysis dc --sweep Top.m.short --from 0 --to 1 --step 1 --print a` gives `v(a)` = 0.25 at `short` = 0; `--param Top.m.short=0` gives the right 1.0. `CircuitInstance::set` rebinds a `STRUCTURAL` param with the new value, but the compiled body is `Sw~short=1.0`, folded at elaboration. Every key class (selector, initial value, reached param) is marked `structural` (`elab/body.rs:150`), so the same holds for an initial value | refuse loud (E0803 or a new code naming the param and "fixes the body at elaboration") any host write of a key param after `Session::open` (sweep, `set` between analyses), as `set_at` already is; add a test that a sweep of `short` fails loud |  e505b21a |
| F2 [Major] A selector through a value input bound to a param is params-only after substitution but stays a run-time `Switch`: no fixed kind, no structural param, no E0415, no merge (AS-02 AC 2 + AC 3, AC 4, AC 5); C8's test asserts the deviation | `Gate(a: Electrical, s: f64)` with `if s > 0.5 { v(a) +> 1.0; } if s > 0.5 { i(a, gnd) +> 1m; }` placed `Gate(a, k)`: elaborates, the run fails `E0418` at the operating point; the same body with `s` a param is `E0415` at elaboration. `elab_structure.rs::a_selector_through_a_value_input_is_judged_per_placement` asserts `[BranchKind::Switch]` "a param selects at run time" | treat a hoisted frame's selector that reads only the caller's params (after substitution) as a params-only selector of the caller: fold it per variant, mark the caller's params key/structural, run E0415 and the merge on it; rewrite C8's test to assert the fixed kind and `k` structural, and add the E0415 case |  60ee0c33 |
| F3 [Major] A merged node loses its authored name: `--print b` of a `signal b` the user wrote is E0801 "names nothing in the design" | `Zero(a, b)` with `v(a, b) +> 0.0` placed between `ZeroBench.a` and `ZeroBench.b`: `ir.node_at("ZeroBench.b")` is `None`, `piperine run .. --print b` fails E0801; `elab_structure.rs::a_merge_with_ground_binds_the_node_to_ground` asserts `node_at("Top.s").is_none()`. The navigability of authored names should not depend on an internal merge | keep each merged node's path as an alias of its survivor (`IrBuilder::apply_merges` records it, `Ir::node_at` resolves it), so `v(b)` prints the survivor's value (or ground's 0); assert it in the merge tests |  7afe5c8c |
| F4 [Minor] S4 remains for reached nodes: `pad_child_nodes` pads a child's `Reached` node slot with `Terminal::Ground` | `crates/piperine-lang/src/elab/children.rs:36-60` ("A node of the child that is no signal ... its slot is ground"); the user story of AS-16 is "never shows ground where a node exists" | name the slot (`Own(k)` for any non-port node of the child's body, or a `Reached` terminal), and add a case with a child that reads a grandchild's `pub` node |  ff9eb92b |
| F5 [Minor] C13 and C15 assert less than their criteria | `source_switch.rs` `the_switch_shorts_at_one_microsecond` reads only `v(a)` (the branch flow 0 / 1 mA under `DC_CURRENT` is in the AC); `both_kinds_enabled_in_one_solve_fail_with_e0418` asserts `E0418` and "both", not the instance `BothBench.both` and the branch `v(a)` | read the flow of `(a, gnd)` (or the series resistor's) before and after 1 us under `DC_CURRENT`; assert the instance path and the branch in the E0418 text |  94d1e0f4 |
| F6 [Minor] AS-06 tests prove weaker or different forms | C29: identity limiter on the carrier alone, so "limit the sum" is unobservable; C30: the test's form is an `if` expression inside a potential contribution, not the AC's `if en.high() { i(a, gnd) +> ac_stim(1m); }`; C28: expected value is Piperine's own ungated junction bounded to 0.6..0.8 V, local copy instead of the `devices.msl` diode; C27: "keeps its last value" unobservable; C35: "0 at an equilibrium solution" not asserted; C28/C29 comments mislabel AC 1/AC 2 and C29's comment arithmetic is garbled | C29: limit `lv.known_or(0.0) + v(a)` with a clamping closure and assert the clamped value; C30: add the AC's statement form (it gives 1 / 0 by own example); C28: compare with the `rdiode` golden `v(out)` under `DC_VOLTAGE`; C27: read the held value after re-enabling; C35: assert the record at an operating point is 0 |  94d1e0f4 |
| F7 [Minor] Cases of a criterion with no assertion | C46: one instance is inferred from timing, which two identical instances also give; C53: reaction and definition reads of `c.state` not run; C59: "an instance result" case missing; C69: `s` read at 0.5 ms, not at (0, 0); C70: the `op` Newton start from `v0` not tested | C46: assert `hoisted()` of `NamedBench` is `["l"]`; C53: emit `c.state` in a reaction and define a carrier from it; C59: add `let p = Lag(v(a));`; C69: record `s` at `at(0)`; C70: an `op` case whose solution depends on the start (two solutions, or a Newton count) |  94d1e0f4, cc55b74e |
| F8 [Minor] C6 accepts any diagnostic | `elab_structure.rs::a_contribution_in_a_closure_is_refused_at_parse` asserts `!design.diagnostics().is_empty()` | assert the code and span of the parse refusal (the contribution inside the closure) |  94d1e0f4 |
| F9 [Cosmetic] Code rules: two new constructors without a doc comment; a test name that does not say what it tests | `crates/piperine-ir/src/contributions.rs:55` `Contributions::new`, `crates/piperine-lang/src/elab/structural.rs:107` `ParamReads::new`; `bindings.rs::a_set_of_r_recomputes_the_capacitance_it_derives` sets `k` and checks a scale | add `///` to both; rename the test to what it sets and reads |  0edc961f |

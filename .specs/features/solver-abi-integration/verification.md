# Solver ABI Integration verification

**Verdict**: PASS. Round 2 closes all 9 round-1 findings at 9806e693. It records 3 new findings, all Low, which are left open for the user (none moves a number).
**Round**: 2
**Proofs**: `proofs.md` at 6d73aa03 (34 pass). This round re-ran the four suites at fcc38c6e: all green, zero warnings (`cargo build --all-targets`, four crates).
**Verifier**: independent sub-agent (author != verifier), Opus
**Range**: 43876aec..fcc38c6e (round 1); f928e331..331883bb (round 2, fixes 294bbc05..9806e693). `parity_baseline` literals diffed against 98b30ad6.

## Numerics

- **Literal diff across the whole test tree.** A script compared the numeric literals on assertion lines of every `#[test]` in `crates/*/tests/*.rs` and `crates/piperine-solver/src`, 43876aec against HEAD, matching tests by name: 1278 tests at the base, 1210 at HEAD, 19 with different literals. Each of the 19 was read:
  - Added only: `parity_rlc_ac_resonant_peak`, `workspace_shape`, `failure_rules`, `probe_selection`, `checkpoint_restore::transient_reject_drives_restore_accept_discards`, the `temperature` targets, and `assembly_limit` (an `abi_tran` or `abi_dc` twin shares the name).
  - Removed with their subject: `device_introspect_*` (`Introspect`), `active_limiter_*` (the consultation counter), `test_setup_error_propagates`, `transient_step_lookup_*` (`LogicValue`). See F6 and F7.
  - One intentional change: `live_params::scheduled_op_strength_set_resolves_consistently_at_the_breakpoint`. It now reads 5 V at the 5 µs landing and 10 V after it, the boundary moved from `< 5e-6 - 1e-18` to `< 5e-6 + 1e-18`, and the 1e-9 tolerance is unchanged. This matches the user's order (context.md lines 185-189: solve on the left value at t, then react).
  - No other surviving numeric assertion or tolerance changed.
- **`parity_baseline`.** Every `assert`, `abs()` and `len()` line matches the 98b30ad6 text. Only the device construction differs. `res.len() == 161` holds.
- **The differential oracle.** It was recorded at a9a23f55, which is before Phase 3 put the transient loop through the protocol. To check the protocol-wired path, `tests/differential.rs` was run at 951fb9ac, the last commit before T22 deleted it: 6 of 6 passed, with time within 1e-15 relative, unknowns within 1e-12 and equal step and Newton counts. The test file did not change between a9a23f55 and the deletion.
  - Nothing in today's tree checks bit-identity, because `Element` is gone.
  - What still guards the numbers: `parity_baseline` (its literals plus 161 steps) and the tolerances of every ported test, which are unchanged.
- **The numeric core.** `math/{newton_raphson,integration}.rs` and `stamp_validation.rs` changed only in comments and the `SolverDomain::Device` rename.

## Probes (scratchpad `verify-abi-integration/`, not committed)

| Probe | Result |
| --- | --- |
| Stub protocol, 1 fs/tick, proposal 1.000000001 s + 0.4 tick | the attempt ends 9.95e-14 s (99.5 ticks) past the proposal: F1 |
| Device transient with `with_start(1e-4)` and no captured step | fails loud: "starts at 1e-4 s only from a captured step" (the accepted gap) |
| Sine through a comparator for 2 ms, 4 crossings | up/down/up/down at accepted times. Adding a free-running clock leaves every accepted time and crossing identical. Two comparators on one node fire 8 crossings, in pairs at equal times |
| Partials verifier on `parity_baseline`'s own Diode, SineVsrc, Comparator, GatedISink | all `Exact` |

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `bind.rs` `pins_bind_by_role` | yes |
| C2 | `bind.rs` `ports_on_one_net_share_one_unknown` | yes |
| C3 | `abi_tran.rs` `a_continuous_init_starts_newton_and_is_held_through_the_operating_point` | yes |
| C4 | `bind.rs` `a_pin_abstol_tightens_its_node` | yes |
| C5 | `protocol_real.rs` `a_clock_nothing_analog_reads_adds_no_step` | yes - equal step counts and every accepted time; my probe repeated it with crossings |
| C6 | `protocol_real.rs` `a_boundary_change_is_delivered_after_the_landing_and_restarts_the_next` | yes. Round 2: the claim text and the test name now state the left-value order (F2 closed) |
| C7 | `protocol_real.rs` `a_rejected_step_leaves_no_trace` | yes - the store holds 50 edges, the instants replayed, and the final value is within 1e-4 of a run with no fault |
| C8 | `host_surface.rs` `an_irreversible_reaction_runs_once_at_an_accepted_time` | yes |
| C9 | `host_surface.rs` `finish_ends_the_analysis_at_an_accepted_time` | yes |
| C10 | `protocol_real.rs` `a_design_without_analog_runs_every_instant_with_no_solve` | yes |
| C11 | `differential.txt` and the tag `solver-abi-integration-differential` (a9a23f55) | yes as a record; re-run green at 951fb9ac after the protocol wiring. It can no longer fail, by design |
| C12 | `operating_point.rs` `a_mixed_loop_settles_at_its_operating_point` | yes |
| C13 | `operating_point.rs` `operating_point_rounds_beyond_the_bound_are_e0506` | yes |
| C14 | `host_surface.rs` `the_sink_receives_everything_in_instant_order` | yes - order is checked by time only, not by microstep within an instant (loose, accepted) |
| C15 | `host_surface.rs` `records_of_an_undone_instant_do_not_reach_the_sink` | yes |
| C16 | `host_set.rs` `a_false_check_refuses_a_set` | yes |
| C17 | `host_set.rs` `reshape_is_refused_in_a_transient_and_rebinds_between_analyses` | yes |
| C18 | `host_set.rs` `a_scheduled_host_write_lands_and_restarts_small` | yes |
| C19 | `operating_point.rs` `globals_hold_the_session_values` | yes for TEMPERATURE, RESOLUTION, DC and NAMED; TRAN, AC and NOISE are not read |
| C20 | `abi_sens.rs` `sensitivity_uses_declared_partials_and_falls_back_to_a_finite_difference` | yes |
| C21 | shell proof over `abi_dc`/`abi_tran`/`abi_ac`/`abi_noise`/`limiting_report` (7 named tests, `gmin_stepping_reaches_a_device_through_the_ambient_input` and `source_stepping_scales_sources_through_the_ambient_input` among them) | yes. Round 2: each of the seven tests must print its pass line (F3 closed) |
| C22 | `protocol_crossings.rs` `a_cap_watch_bounds_the_next_step`, `a_fired_crossing_is_a_commit_at_the_landing` | yes. Round 2: caps of 4 ns and 1 ns in both orders. Every step is at most 1 ns, the widest step exceeds 0.99 ns, so the 4 ns cap alone would fail it (F4 closed) |
| C23 | `parity_baseline.rs`, diffed against 98b30ad6 | yes |
| C24 | `removed_surface.rs` `removed_tests_are_listed_with_their_item` | yes. Round 2: `base_tests.txt` holds 527 names, every base test of the four crates (I counted 44+69+120+296 from 43876aec). Every name that vanished in either round is in it or in `removed.txt` (F5 closed) |
| C25 | `removed_surface.rs` `the_element_contract_is_gone` | yes - a grep of src and tests of the four crates finds no name |
| C26 | `removed_surface.rs` `the_old_digital_engine_is_gone` | yes |
| C27 | shell proof: `every_device_function_passes_the_partials_verifier`, `every_helper_device_passes_at_generic_points`, `every_hand_written_device_is_in_a_partials_check` and one `its_devices_pass_the_partials_verifier` per target (more than 20) | yes. The guard matches a device by its label string, not its module: N3 (F9 closed) |
| C28 | `crate_boundaries.rs` `analog_sources_name_no_digital_type` | yes |
| C29 | `crate_boundaries.rs` `digital_sources_name_no_analog_type` | yes |
| C30 | the four suites | yes - re-run green, zero warnings |

## Round 2

- **Numerics unchanged.**
  - The literal diff f928e331..HEAD over every test only adds: the 0.99e-9 floor in `a_cap_watch_bounds_the_next_step`, the AGE asserts, and the `transient_step_lookup` bytes. No surviving literal or tolerance was removed or changed.
  - Against 43876aec the only removed literal is still the intended `live_params` 10 V to 5 V.
  - `parity_baseline` asserts match 98b30ad6 and all 7 pass with 161 steps.
  - The four suites are green, with zero warnings under `--all-targets`.
- **F1, the cap at half a tick, is sound.**
  - A proposal less than half a tick past tick k ends at k. One between half a tick and a tick past ends at k+1, at most half a tick after the proposal.
  - `analyses/transient.rs:1215` then ends the analog step at the proposal itself, because it lies within one tick of the tick. So no accepted step passes the proposal, and none passes the `CAP` bound the proposal honours. The instant runs on its tick at most half a tick later, as the design's tick-grid tolerance allows.
  - A few ulps short of a tick still rounds to that tick: the ulp of `ticks` stays below 0.5 up to about 2^51 ticks, which is 2 s at 1 fs per tick.
  - My round-1 probe now ends 0.4 ticks past the proposal, below f64 resolution at 1 s. The new stub test fails with the old formula (99.5 ticks past).
- **F6 is real now.** The glitch test asserts that `c` reads HIGH inside the flip, using a lag inverter so the glitch exists. The homotopy test asserts the limit's AGE sequence 0, >0, >0, >0, 0, >0..., which fails if there is no restore, or if the converging attempt is restored.
- **F7 is restored.**
  - Every step carries its digital net bytes and names. The value is the settled one after that time's instant, so a landing step pairs the analog left value with the post-instant digital value. That is documented.
  - Re-entry works from any recorded step. `reentry_from_any_captured_step_matches_the_continuous_run` re-enters at every 7th step plus the last and checks the digital end state exactly.
- **Probes re-run.** No regressions: the round-1 crossings, the free clock, simultaneous crossings, the non-zero start and the `parity_baseline` partials probes all hold.
- **New re-entry probe.** Re-entering at each clock-edge step gives the results in N1.

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 - The `floor_tick` relative slack lets a transient step end past the controller's proposal by up to 1e-13·t. At the default 1 fs per tick that exceeds one tick once t > 10 ms. This breaks the function's own contract ("a step never ends past what was proposed"), and the step can pass a `CAP` bound by that amount. | Probe: SPT 1e-15, proposal 1.000000001 s: the attempt ends 9.95e-14 s (99.5 ticks) past it. `protocol/transient.rs:141` adds `ticks.abs() * 1e-13`. `analyses/transient.rs:1215` then ends the step at the tick, not the proposal, because the gap is more than one tick. | Bound the slack below one tick (for example `min(ticks.abs() * 1e-13, 0.5)`). Add a stub test at t ≈ 1 s with 1 fs ticks that asserts the attempt is at most one tick past the proposal. | 294bbc05 |
| F2 - The C6 claim text in tasks.md `## Checks` and the test name still state the superseded order ("read by the arriving step"). The test asserts the left value. | tasks.md C6; `protocol_real.rs:84`, which asserts "the landing is solved on the left value" | Restate C6 as the user's order and rename the test (for example `a_boundary_change_is_delivered_after_the_landing_and_restarts_the_next`). | 8e6c008b |
| F3 - C21's proof covers less than its claim: the veto, the GMIN/SOURCE_SCALE continuation and the transient/AC/noise drivers are not in the proof. | `abi_dc.rs:32` is one linear DC solve. Covering tests exist but are not named: `limiting_report.rs` `active_limiter_vetoes_newton_convergence`, `abi_dc.rs` `source_stepping_scales_sources_through_the_ambient_input`, and the `abi_tran`/`abi_ac`/`abi_noise` targets. | Make C21 a Run group that names those tests. | d71101fc |
| F4 - C22 does not prove "the smallest CAP" or "a fired CROSS is committed at the landing". | `protocol_crossings.rs:111` places one capped device. The crossing half is `a_fired_crossing_is_a_commit_at_the_landing`, which the proof does not name. | Add a second CAP device with a smaller bound and assert that bound. Add the crossing test to the C22 proof. | 8c377cea |
| F5 - Three base tests disappeared without a `removed.txt` line. Two are renames, which the spec forbids for ported tests (Port AC 1). | Name diff 43876aec..HEAD: `abi_compiles_an_element` became `abi_compiles_a_device`, `owner_depends_on_both_sub_solvers_only` became `owner_depends_on_the_abi_and_both_sub_solvers_only`, and `checkpoint_restore::_context_default_compiles_for_later_tests` was deleted. The C24 guard cannot see this. | List the three in `removed.txt` with their item (the `Element` surface; the owner's new `piperine-abi` dependency; a compile stub). | 3e4b400c |
| F6 - Two ported tests lost the assertion that gave their name meaning. They pass without proving it. | `digital_settle_subscriber.rs` `the_subscriber_sees_only_the_settled_value_never_the_mid_delta_cycle_glitch` dropped "the fixture must actually glitch", so nothing shows that a glitch exists to suppress. `checkpoint_restore.rs` `dc_homotopy_fallthrough_drives_restore_between_strategies` no longer observes a restore. It compares the final value to a plain solve within a new 1e-4 tolerance, which a run with no restore also meets. | Assert the intermediate HIGH of `c` (a sink Change inside the instant) and observe the restore (assembly snapshot or limit state between strategies). Otherwise rename both and list the dropped claims in `removed.txt`. | afb35119 |
| F7 - Two host-visible capabilities were narrowed with no `removed.txt` line and no MD-62 text. `TransientStep` no longer carries per-step digital net values. A transient can now re-enter only from the first or the last step; before, every step was a full state. | `core/result.rs`: `digital`, `with_digital`, `digital(idx)` and `digital_hidden` are removed. `transient_step_lookup_by_net_returns_analog_and_digital_values` now asserts `get_net(digital) == None`, the opposite of its name. `transient_reentry.rs` `reentry_needs_a_captured_device_state`. | Record both in MD-62 (the sink's Change observations replace the digital waveform; re-entry only from the first or last step), add a ROADMAP line if they are gaps, and rename the lookup test. | 7dbfbd79 |
| F8 - The CLAUDE.md crate table says `piperine-solver` "depends on no other in-tree crate" besides the sub-solvers, but it depends on `piperine-abi`. | `crates/piperine-solver/Cargo.toml:17`; `crate_boundaries.rs` `owner_depends_on_the_abi_and_both_sub_solvers_only`. CLAUDE.md line 38 has the same omission. | Name `piperine-abi` in both places, in CLAUDE.md and AGENTS.md. | 92da5f96 |
| F9 - C27 claims "every function of every test device", but the table holds one function per helper module. It omits `noise_pair` fn 0 "a" and every device defined inside a target (about 25, including the six in `parity_baseline`). | `abi_devices.rs:49` `table()`. My probe found `parity_baseline`'s Diode, SineVsrc, Comparator and GatedISink `Exact`, so no mismatch was found. | Add `noise_pair` fn 0 and the target-local devices with declared partials, or narrow the C27 claim to the helper library. | e663a22b |
| N1 (Low) - Re-entry approximates the continuous run; it is not a replay. The captured state does not hold the controller's step history, nor the restart pending after a landing's delivery, so the continuation takes another step grid. | Probe: re-entering `build_clocked_rc` at each of its 9 clock-edge steps ends with v(c) up to 7.3e-4 relative from the continuous run (worst at 0.9 ms), with 15 to 154 steps against 21 to 151. `reentry_from_any_captured_step_matches_the_continuous_run` allows 1e-3 relative, which only just covers this. | Carry the last step and a pending `Restart` in `ReentryState`, then tighten the test. Or state the tolerance class of re-entry in MD-62. Left to the user. | |
| N2 (Low) - Every transient step now keeps a full re-entry state: the digital checkpoint, every net's bytes and the assembly snapshot. `DigitalSide::mark` also copies every net's bytes on each attempt, rejected ones included. Memory and time grow with steps times state, and there is no opt-out. | `adapters.rs` `mark` and `record_settled`; `analyses/transient.rs` `device_capture` runs for every step | Make per-step re-entry state opt-in (like `record_device_state`), or share unchanged net bytes. Left to the user. | |
| N3 (Low) - `every_hand_written_device_is_in_a_partials_check` matches a device by the label string passed to `verify_every_function`, not by the module verified. `verify_every_function("X", other_module)` satisfies the guard for device `X`. | `abi_devices.rs:192-199` | Match the module expression (`module::<X>`), or the struct name in the call's second argument. Left to the user. | |

Also checked, no finding:

- One `create` per placement, shared by both sides (`bind_creates_each_placement_once_for_both_sides`). The shared handle locks each call. A reaction's host callback only records commits, so it never re-enters `eval` on the same instance and cannot deadlock.
- Removed-name grep over src and tests of the four crates is clean.
- The analog/digital boundary guards hold.
- MD-62 is in STATE.md.
- The 25 removed `mixed_signal` tests tested the `comb_phase` of test doubles. Their solver-level behaviour is still covered: crossings in both directions (`a_comparator_and_toggle_divide_the_clock`, my probe), simultaneous crossings (probe), D2A/A2D loops (`operating_point.rs`, `parity_mixed_signal_dc_settle`) and sampled reactions (`a_reaction_reads_the_accepted_solution`). Hysteresis and X/Z on a switch were properties of the doubles, not of the solver.

# Solver Hygiene verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at b623dd0e (the tree judged is 137244f6, which adds only the dispatch record)
**Verifier**: independent sub-agent (author != verifier)

The numerics hold where the checks look: the deviation table at HEAD equals the T2 baseline (09d0cf3c)
except the authorized moves, re-entry is bit-exact on cases nobody named, and every mutation of a core
rule (HYG-05 history, stepper/restart restore, restart cap, state bound, store undo) turns its check red.
The failures are a plan the transient operating point ignores (silent), an `E0508` rule that fails
circuits ngspice-47 completes, a retry path AC 2 promises but the code skips, an untested stop snap, and
three looser-than-claimed points.

## Evidence run at 137244f6

- `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test deviation -- --nocapture`, against the
  T2 baseline (09d0cf3c body), identical on interp and rust:

  | Twin, quantity | Baseline | HEAD | Authorized |
  | --- | --- | --- | --- |
  | ac_rc both | 0.0000 | 0.0000 | - |
  | diode_iv i(v1) | 0.3685 | 0.3686 | Q4 (probe predicted 0.3685 to 0.3686) |
  | diode_rev i(v1) | 0.0000 | 0.0000 | - |
  | diode_series v(a) / v(b) | 0.0008 / 0.0006 | 0.0005 / 0.0003 | Q4, closer |
  | divider, isrc_r | 0.0000 | 0.0000 | - |
  | tran_rc v(out) | 0.5123 | 0.5123 | - (1000 steps, 0 rejected, 0 floor hits) |
  | tran_dq v(a) | (new) | 0.5123 | HYG-06 (1046 steps, 0 rejected) |

  No ratio is worse than baseline beyond the authorized 1e-4 on `diode_iv`. `diode_series.txt` moved
  6.884155e-1 to 6.884153e-1; ngspice-47 at `numdgt=10` gives 6.8841508e-1, so the new digit is closer.
- Whole suites green: `cargo test -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver`
  (526 passed), `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --no-fail-fast` (97 passed, all
  buck targets included), `cargo test -p piperine-cli -p piperine-api --no-fail-fast` (220 passed),
  `cargo test -p piperine-solver --test pss` (3 passed: the shooting map still
  converges through the restart path of `device_resume`).
- Own examples (throwaway, removed): re-entry of `tran_dq` (diode, `limit`, nonlinear `ddt`) at steps
  1, 2, 3, 5, n/4, n/2, n-3, n-2, both at the 10 ns cap (1047 steps) and free-running (238 steps, the
  controller shaping the grid): every leg equals the continuous run bit for bit. Hard DC with the new
  no-damping default, each against ngspice-47: a 9-diode chain from 20 V through 10 Ω (v(n0) 7.5550005
  vs 7.554998), a diode at 100 V through 1 Ω (1.0717329 vs 1.071733), anti-parallel diodes at -50 V
  (-2.2058496 / -1.3911580 vs -2.20585 / -1.39116), a cross-coupled behavioral latch (9.900990e-3 / 5.0
  vs 9.900990e-3 / 5.0): all converge, all agree. Nested journal marks (40 seeds, depth 3, inner marks
  released without rewind then the outer rewound, double rewind to one mark, look-ahead `advance` mixed
  in): every rewind equals the checkpoint taken at its mark.
- Mutations (scratch, reverted, tree clean): listed in the `Holds` column.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `deviation::tests::the_worst_ratio_is_the_largest_difference_over_its_bound` | yes - worst row in the middle, value 0.5 asserted |
| C2 | `deviation::tests::a_pair_at_the_bound_has_ratio_one` | yes - ratio 1 to 1e-12 and the contract's own `holds` agrees just inside |
| C3 | `deviation.rs` `the_table_covers_every_twin` | yes - one row per `tests/ngspice/*.msl` on each backend; numbers asserted nowhere, as designed |
| C4 | `assembly_ddt.rs` `a_nonlinear_charge_through_a_limit_keeps_the_operating_point` | yes - red with `push_stage` back on `evaluate` (mutation) |
| C5 | `assembly_ddt.rs` `the_ddt_history_holds_the_charge_at_the_solution` | yes - red under the same mutation |
| C6 | `parity_tran.rs` `tran_dq` | yes - red under the same mutation; worst 0.5123 of `TRAN_VOLTAGE` |
| C7 | `parity_corpus.rs` `every_twin_has_a_cir_and_a_test` | yes |
| C8 | `convergence::tests::the_pi_controller_resumes_its_memory` | yes - bits compared after 1 to 6 accepts, and warm != cold shown |
| C9 | `transient_reentry.rs` `a_reentry_from_every_recorded_step_continues_bit_for_bit` | yes - red without the stepper resume and without the restart restore (mutations) |
| C10 | `transient_reentry.rs` `a_reentry_after_a_jump_takes_the_same_restart` | yes - red without `restart_pending` restored |
| C11 | `transient_reentry.rs` `the_reentry_state_holds_the_newton_history_and_the_controller` | yes - red without the stepper resume |
| C12 | `transient_reentry.rs` `reentry_last_step_attaches_state_to_the_last_step_only` | yes - every middle step refused, last re-entered, run bits unchanged |
| C13 | `transient_reentry.rs` `the_default_reentry_keeps_every_step_state` | yes |
| C14 | `error::tests::a_run_error_displays_its_code_time_and_names` | yes |
| C15 | `error::tests::a_run_error_without_names_shows_its_unknowns` | yes |
| C16 | `singular::tests::a_loop_of_two_potential_sources_is_singular_in_both_flows` | yes |
| C17 | `singular::tests::a_node_reached_only_through_capacitors_is_singular_at_dc` | yes |
| C18 | `singular::tests::a_singular_system_above_the_dense_bound_says_it_skipped` | yes |
| C19 | `newton_raphson::tests::a_non_finite_solution_of_finite_stamps_is_e0503` | yes |
| C20 | `newton_raphson::tests::exhaustion_names_the_unknowns_that_failed` | yes |
| C21 | `failure_rules.rs` `a_singular_solve_names_the_pins_of_its_node` | yes - exact `node of c1.p, c2.p`; DC only (see F8) |
| C22 | `failure_rules.rs` `a_loop_of_sources_names_both_instances` | yes - DC only (see F8) |
| C23 | `failure_rules.rs` `a_non_finite_contribution_is_e0507_and_names_the_instance` | yes - code, instance, and no `7.3.2.1` |
| C24 | `stamp_validation::tests::a_non_finite_stamp_is_e0507_with_its_row` | yes |
| C25 | `failure_rules.rs` `an_equilibrium_that_never_converges_is_e0505_with_its_unknowns` | yes - exact names |
| C26 | `failure_rules.rs` `a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time` | yes - code, time window, exact names |
| C27 | `failure_rules.rs` `a_jump_step_that_fails_at_the_floor_is_e0504_naming_the_carrier` | yes |
| C28 | `run_errors.rs` `a_loop_of_two_vsources_from_source_is_e0503` | yes - both backends, both paths |
| C29 | `run_errors.rs` `a_floating_node_from_source_is_e0503` | yes |
| C30 | `run_errors.rs` `a_nan_contribution_from_source_is_e0507` | yes |
| C31 | `spec_text.rs` `part_v_states_the_solver_hygiene_rules` | yes |
| C32 | `spec_text.rs` `appendix_c_states_the_solver_hygiene_numerics` | yes |
| C33 | `integration::tests::the_state_estimate_reads_chgtol` | yes - red with the bound loosened tenfold (mutation) |
| C34 | `integration::tests::the_state_estimate_is_zero_for_a_linear_charge` | yes |
| C35 | `integration::tests::a_rise_from_flat_is_measured_and_rejected` | yes - unit layer, as the matrix names; the skip has no code left to return to |
| C36 | `assembly_ddt.rs` `the_assembly_reports_each_ddt_state_after_a_step` | yes |
| C37 | `truncation.rs` `a_source_driven_nonlinear_capacitor_meets_the_current_tolerance` | yes - every accepted step against the amended `trtol·tol`; red with the bound loosened tenfold |
| C38 | `truncation.rs` `a_larger_chgtol_takes_fewer_steps` | yes - red with the bound loosened tenfold |
| C39 | `parity_tran.rs` `tran_rc` | yes for its claim; it cannot see the basis (every step is at the 10 ns cap), see F7 |
| C40 | `truncation.rs` `an_unannounced_corner_fails_e0508_at_the_floor` | yes for the code; the behaviour itself is F2 |
| C41 | `truncation.rs` `the_floor_is_retried_once_before_failing` | partly - proves the retry on the rejection path only, through the error text; the floor reached by a proposal is not retried (F3) |
| C42 | `twins` `every_twin_prints_the_bytes_it_printed_before_the_long_run` | yes |
| C43 | `convergence::tests::node_damping_follows_ngspice` | yes - each condition off, 20 V and 200 V factors, branch untouched |
| C44 | `convergence::tests::no_damping_leaves_the_update` | yes |
| C45 | `abi_dc.rs` `the_dc_driver_iterates_with_the_plans_strategy` | yes - one call per iteration, equality asserted |
| C46 | `abi_tran.rs` `the_transient_driver_iterates_with_the_plans_strategy` | no - `seen >= 2·(steps-1)` counts the time steps only; the transient's operating point iterates with the default plan and the test cannot see it (F1) |
| C47 | `parity_dc.rs` `diode_iv` | yes - 0.3686 of `DC_CURRENT` at every point |
| C48 | `protocol_transient.rs` `the_proposal_sees_the_next_due_time` | yes - the time in seconds, and none when nothing is queued |
| C49 | `grep` over `GAPS.md` | yes |
| C50 | `grep` over `.specs/STATE.md` | yes |
| C51 | `abi_tran.rs` `the_step_after_a_jump_is_capped_at_a_tenth_of_the_last` | yes - red with `JUMP_FRACTION` 1.0 (mutation) |
| C52 | `boundary.rs` `a_carrier_change_is_a_time_point_and_starts_the_response` | yes - red with the restart cap removed (mutation); green with the fraction at 1.0, so it proves a cap, not the tenth (C51 does) |
| C53 | `host_set.rs` `a_scheduled_host_write_lands_and_restarts_small` | yes |
| C54 | `instant.rs` `start_at_commits_init_at_its_tick` | yes |
| C55 | `transient_reentry.rs` `a_late_start_solves_the_operating_point_at_its_start` | yes - a ramp reads 2 V at 2 ms, so the OP saw the time |
| C56 | `transient_reentry.rs` `a_late_start_runs_no_instant_before_it` | yes |
| C57 | `transient_reentry.rs` `a_start_that_is_not_finite_fails_before_the_operating_point` | yes - NaN, inf and negative, value named |
| C58 | `removed_surface.rs` `the_dead_newton_hooks_are_gone` | yes |
| C59 | `removed_surface.rs` `the_phdl_port_text_is_gone` | yes |
| C60 | `abi_devices.rs` `every_hand_written_device_is_in_a_partials_check` | yes |
| C61 | `abi_devices.rs` `a_check_of_another_module_does_not_count` | yes - planted label over another module, directly and through a helper |
| C62 | `checkpoint.rs` `a_rewind_equals_the_checkpoint_taken_at_the_mark` | partly - red when the store loses `changed_at`, green when the queue's `Removed` undo is dropped: the random sequence never removes a queued commit (F6) |
| C63 | `checkpoint.rs` `nested_marks_rewind_in_order` | yes for one nesting; the release-without-rewind order the protocol uses is covered only by the Verifier's throwaway (green) |
| C64 | `checkpoint.rs` `a_mark_journals_only_the_nets_that_changed` | yes - 1 entry of 1000 nets |
| C65 | `git log` grep on the basis commit | yes - aacda903 carries the before/after table (identical) |
| C66 | `transient_reentry.rs` `a_reentry_with_another_stop_lands_on_the_old_stop` | yes |
| C67 | `transient_reentry.rs` `a_middle_step_under_last_step_cannot_be_reentered` | yes |
| C68 | `failure_rules.rs` `an_instance_is_named_once_in_a_singular_report` | yes |
| C69 | `truncation.rs` `the_floor_at_the_stop_time_names_that_time` | yes |
| C70 | `abi_tran.rs` `two_jumps_in_one_restart_step_restart_from_the_later` | yes - red with `JUMP_FRACTION` 1.0 |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (silent, HYG-10 AC 1) - the transient's operating point ignores the transient's `ConvergencePlan`. A host that opts into `NodeDamping` with `TransientSolver::with_plan`/`set_plan` gets no damping at all: `NodeDamping` acts only in static solves, and the only static solve of a transient is built with the default plan. The plan's homotopy list is ignored there too | `analyses/transient.rs` operating point: `DcSolver::new(self.system.circuit, context)?` with no `with_plan`; C46 asserts `seen >= 2·(steps-1)`, so it stays green whatever the operating point used | pass the transient's plan (or its Newton strategy and homotopies) to that `DcSolver`; make C46 assert the counting strategy saw every iteration, operating point included (`seen == stats.newton_iterations`), on a circuit whose operating point needs several | |
| F2 (loud, Q3 premise) - `E0508` fails circuits ngspice-47 completes. Q3(a) was chosen as "as ngspice" | a 0 to 1 V step at 1.03 us computed from `TIME` across 1 nF (`truncation.rs` `run_corner`, width 0, default `dt_min` 1e-15) fails `E0508` at t = 1.03e-6; ngspice-47 on `B1 top 0 V=u(time-1.03u)`, `C1 top 0 1n`, `.tran 0.1u 2u` completes (61 points, no rejection). A 5 ns `tanh` corner with `dt_min` 1 ns fails `E0508` 31 ns before the corner; ngspice with `delmin=1n` completes. Before this feature both runs completed with a warning | put this evidence to the user and record the answer: keep Q3(a) knowingly (and drop "as ngspice" from the spec's rationale), or fail only where ngspice's own step-size rule would | |
| F3 (loud, HYG-09 AC 2) - the floor is retried only when a truncation rejection drives the step there. When the controller's proposal after an accepted step (or a Newton cut) first lands on the floor, that first floor attempt fails the run with no retry | `run_corner` with width 0.5, 1, 2 and 5 ns and `dt_min` 1 ns: `E0508 ... at a floor-sized step with no retry before it`; `device_reject_lte` fails any attempt within a tick of `dt_min` whatever `floor_retried` says. C41 exercises only the rejection path, and only through the error text | retry once whenever the failing attempt is the first at the floor (the previous attempt was larger), or amend AC 2 to ngspice's `olddelta` rule; add a test of the proposal path that reads `dt_min_floor_hits` or a retry counter, not the wording | |
| F4 (untested rule, no spec) - the stop snap in `device_propose` (an end within 2 ticks short of the stop moves to the stop) is in no criterion and no test, and it can stretch a step past `now + cap` (a `CAP` bound) by up to 2 ticks. It cannot swallow a discrete event (the protocol's `advance` lands after the proposal) nor a located crossing (`LandedEarlier` retries inside the step) | mutation `if false && ...` on the snap: every `piperine-solver` target and `boundary`, `parity_tran`, `buck_boundary`, `buck_time` stay green | state it in Appendix C with its reason, add a test (a capped restart step that would end 1 tick short of the stop ends at the stop), and take `min` with the `CAP` bound after the snap | |
| F5 (cost) - the solver default still copies every net per attempt. With `Keep::Every` and `Reentry::EveryStep`, `DigitalSide::mark` takes a full `checkpoint()` and every net's bytes at every protocol checkpoint, rejected attempts included; the journal spares only `Keep::Last`/`LastStep` and the look-ahead | `analyses/adapters.rs` `DigitalSide::mark`: `if let Some(settled) = &self.settled { ... self.digital.checkpoint() ... }` before `self.digital.mark()` | take the settled copy once per accepted step (or skip it when no instant ran since the last copy) | |
| F6 (assertion looser than its criterion, HYG-14 AC 4) - C62's random sequence never removes a queued commit, so the queue's `Removed` undo is unproven by any check | mutation dropping the `Undo::Removed` reinsertion: `checkpoint.rs` (C62, C63, C64) green; only `queue.rs` `a_queue_rewind_restores_its_pending_commits`, which no check names, turns red | add device commits that replace or cancel (`REPLACE`, overlapping commits) to the random sequence, or promote the queue test to a check | |
| F7 (cost hidden by the cap) - `parity_rc_transient`'s re-pin is accuracy-neutral but not cost-neutral: 161 to 259 steps (+61 %) for an error that moved 2.89e-5 to 2.95e-5. ngspice-47 on the same circuit takes 112 points with error 5.4e-5, so Piperine is still the more accurate, at 2.3x ngspice's points. The error does not fall with 61 % more steps, so it is not truncation-bound and the pin cannot judge the basis; the twins run at a 10 ns cap where every step sits at the cap, so no twin measures the step cost either | `parity_baseline.rs` comment and step pin; `ngspice -b` of `V1 top 0 SIN(0 1 1k)`, 1 kΩ, 100 nF, `.tran 1e-5 1e-3`: 112 points, v(mid) = -0.450511 against the converged -0.4504568 | not a regression of accuracy; record the step cost: have `tests/deviation.rs` also print a free-running run (ngspice's own max step) of `tran_rc` and `tran_dq` with its step count next to ngspice's | |
| F8 (coverage, HYG-01 AC 1) - AC 1 names four solve kinds (operating point, DC point, transient step, AC point); the checks raise `E0503` from operating points only | C16 to C22, C28, C29 all solve an operating point; `ac.rs` and the transient driver map the error through the same `singular` path but no test reaches it | add one transient-step and one AC-point singular case | |

## Not judged

- The Book listings re-pasted (docs only).
- Re-entry from a state captured mid-microstep (a suspended instant with host waiters): not built.
  Re-entry right after a floor retry cannot be captured: capture happens at accepted steps and the
  retry flag clears on accept.
- ngspice's `niiter.c` node-damping detail (whether `maxdiff` is signed): `NodeDamping` is opt-in and
  unreachable from a transient today (F1); not checked against the C source.

# Solver Crossings verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 601264c4 (rerun by the Verifier at 6877d414: every `Run` group green; root `cargo test`, default members, ngspice at `/usr/bin/ngspice`, `PIPERINE_NGSPICE` unset: 1415 passed, 0 failed)
**Verifier**: independent sub-agent (author != verifier)

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `piperine-digital-solver/tests/instant.rs` `two_commits_to_one_net_settle_to_one_write` | yes - fails with `instant.rs` reverted to origin/main |
| C2 | `piperine-digital-solver/tests/instant.rs` `a_commit_undone_in_its_microstep_is_no_change` | yes - fails with the revert |
| C3 | `piperine-digital-solver/tests/instant.rs` `a_settled_change_is_one_observation` | yes, weak - fails with the revert, but HIGH, LOW, HIGH over LOW leaves the last intermediate value equal to the value before the microstep, so `PREVIOUS` = "before the microstep" is not told apart from `PREVIOUS` = "the previous commit" |
| C4 | `piperine-solver/tests/settled_value.rs` `x_changed_does_not_fire_when_the_settled_value_is_unchanged` | yes - fails with the revert |
| C5 | `piperine-solver/tests/protocol_crossings.rs` `a_ramp_lands_within_ttol_plus_one_tick` | yes - fails without the tick grid for `CROSS` designs |
| C6 | `piperine-solver/tests/protocol_crossings.rs` `a_sine_crossing_is_located` | yes |
| C7 | `piperine-solver/tests/protocol_crossings.rs` `an_rc_charge_trips_at_its_threshold` | yes |
| C8 | `piperine-solver/tests/protocol_crossings.rs` `a_fired_crossing_is_a_commit_at_the_landing` | yes - the replaced assertion is stricter than the old one (exact tick, exact accepted step, located within etol or ttol plus one tick of the analytic time); a faithful move to §17.3, not a weakening |
| C9 | `piperine-solver/tests/protocol_crossings.rs` `the_earliest_of_several_crossings_lands_first` | yes - taking the latest estimate instead of the earliest survives, but only costs one more retake: the satisfied watch drops out and the loop still lands the earlier one first (an equivalent mutant) |
| C10 | `piperine-solver/tests/protocol_crossings.rs` `crossings_on_one_tick_fire_together` | yes |
| C11 | `piperine-solver/tests/protocol_crossings.rs` `the_session_ttol_applies_to_a_watch_without_a_ttol_pin` | yes - fails without the tick grid |
| C12 | `piperine-solver/tests/protocol_crossings.rs` `a_watch_without_an_etol_pin_takes_the_potential_abstol` | yes - fails without the tick grid |
| C13 | `piperine-solver/tests/protocol_crossings.rs` `a_crossing_in_the_step_after_a_jump_is_not_located` | no - with the `!post_set_step` guard removed (rule 8 gone) it still passes: the mutant locates the crossing and lands it at edge + 1.908 ns, which is still more than `ttol` plus one tick after the edge and still the first accepted step after it |
| C14 | `piperine-solver/tests/crossing_rules.rs` `a_crossing_between_operating_point_rounds_fires_in_the_later_round` | yes |
| C15 | `piperine-solver/tests/crossing_rules.rs` `a_crossing_between_two_operating_points_fires_at_the_later` | yes |
| C16 | `piperine-solver/tests/crossing_rules.rs` `an_invalid_ttol_or_etol_fails_the_analysis_with_e0510` | yes - 0, −1 ps, NaN and +inf on both pins, with the code, instance, watch and pin named |
| C17 | `piperine-solver/tests/crossing_rules.rs` `an_invalid_step_cap_fails_the_analysis_with_e0509` | yes, weak - the instance and its watch are both named `cap`, so `contains("`cap`")` does not prove the watch is named (AC 9) |
| C18 | `piperine-solver/tests/parity_baseline.rs` `parity_rc_transient` | yes - 161 steps, unedited |
| C19 | `piperine-analog-solver/tests/assembly_watch.rs` `rising_reports_a_sign_change_without_accepting` | yes |
| C20 | `piperine-abi/tests/validate.rs` `a_reaction_may_read_gmin_and_source_scale` | yes |
| C21 | `piperine-digital-solver/tests/functions.rs` `reactions_and_functions_read_the_continuation_values` | yes - uses distinct values (2e-12, 0.25), so a hard-coded 1.0 would fail |
| C22 | `piperine-solver/tests/environment.rs` `a_reaction_reads_temperature_gmin_and_source_scale` | yes - a session `gmin` of 3e-12, not the default |
| C23 | `piperine-solver/tests/environment.rs` `a_discrete_function_reads_gmin_and_source_scale` | yes |
| C24 | `piperine-solver/tests/parity_baseline.rs` `parity_rc_transient` | yes |

Counts: 22 hold (2 of them weak), 1 does not hold (C13). C9 holds against an equivalent mutant.

**Parity evidence beyond the checks.** A throwaway target ran six designs with no `CROSS` watch (an RC on a sine, an RC driven by a clock edge, a diode rectifier, a ramp RC with a `CAP` watch, an RLC, and a sine RC with a digital `delay` kick) against this branch and against origin/main's `src/` for `piperine-{solver,analog-solver,digital-solver,abi}`. It hashed every accepted (time, node value) pair to the bit. Step counts (161, 269, 397, 104, 564, 161) and hashes matched exactly. Designs with no `CROSS` watch take the old steps.

**Own examples.** (1) Locator accuracy on a saturating, lopsided `e = tanh(1e4·(v − 0.5)) + 0.999` on a 1 kV/s ramp, with `etol` out of play (`vntol` 1e-15): the run lands 0.73 fs after the analytic crossing for initial steps of 10 µs and 100 µs, with no rejection blow-up. (2) A crossing exactly on a step end (a ramp through 0.5 V, steps of 100 µs): `e = 0` at 0.5 ms does not fire, and it fires one tick later. This matches the spec's edge case. (3) A falling sign change does not fire a `CROSS` watch: that is §17.3 ("`≤ 0` to `> 0`"), and a falling edge is a second watch on `−e`, as the comparator helper does.

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| Major: C13's test passes with rule 8 removed. A locator that runs on the step that reads a jump goes undetected. | Mutant `if run.locates && !post_set_step` changed to `if run.locates` (`crates/piperine-solver/src/analyses/transient.rs:1339`): `protocol_crossings` 12/12 green. Probe: the mutant lands the clock-edge crossing at 2.50000001908e-4 s instead of the controller's 2.54e-4 s. Both lie more than `ttol` plus one tick after the edge, and both are "the first accepted step after it". | Assert that no retake happened. Either the fired time equals the fired time of the same circuit run with `o.ttol = 1.0` (unlocated), or `fired − edge` equals the controller's step, here more than 1 µs. | |
| Major (ENV-03 consistency, pre-existing line): the transient's operating point solves the analog side with `Context::default()`, so analog functions read the default `gmin` (and the default tolerances) there. Reactions in that same operating point now read the session `gmin`. | `transient.rs:1120` `DcSolver::new(self.system.circuit, Context::default())` (unchanged from origin/main). Probe: a device whose `FLOW` reads the ambient `GMIN` (`I = gmin·1e9` into 1 kΩ), session `gmin` = 3e-12. `dc()` gives v(out) = 3.0, but the transient gives v = 1.0 at t = 0 and 3.0 from the first step on. In that same transient operating point, `environment.rs` shows a reaction reading 3e-12. | Pass `self.system.context.clone()` to the transient operating point's `DcSolver`. Then add a check that an analog function and a reaction read the same `GMIN` at t = 0 of a transient, and rerun parity (a parity case that sets a non-default context would move). | |
| Major (performance regression, `CROSS` designs only): after a located landing, the step controller grows again from the landing's tiny final step, at most ×1.5 per step. Every located crossing costs about 25 to 60 extra accepted steps. | dt right after the landing on a 10 kHz sine: 1.68e-11 s, then 2.5e-11, 3.8e-11 and so on, back to about 1e-6 s about 28 steps later. Step counts against origin/main: 10 kHz sine with 10 crossings, 934 → 1652; 1 kHz sine, 102 → 133; 1 kHz with a 100 µs step, 280 → 494. A ramp crossing exactly on a step end: about 11 → 72 steps (a 1 fs landing step, then 60 steps of regrowth). Mixed-signal designs with frequent comparator edges (ADCs, PWM, buck) pay for this on every edge. No result is wrong. | Seed the step after an accepted retake from the step size proposed before the retake, for example `st.dt = max(propose_dt(..), dt proposed before the LandedEarlier)`, in the `run.locates` path only, so parity is untouched. Pin it with a step-count bound on a multi-crossing sine. | |
| Minor: the two-point secant, which the design says keeps a curved `e` from stalling, has no test. | Mutant: pass `prev = None` to `crossing_estimate` (`transient.rs:887`, bracket secant only). `protocol_crossings` 12/12 green. | Add a curved-`e` case where the bracket-only secant needs visibly more retakes or steps, and bound its retake or step count. | |
| Minor: the controller-growth rule (`grown_from`, `transient.rs:1454`) is guarded only by a hang. | Mutant `if false && …`: `protocol_crossings` does not finish. It ran over 3 min of CPU before I killed it, crawling toward `max_steps`. No assertion names the rule, and a CI timeout is the only signal. | Add a test with an explicit step-count bound on a `CROSS` design whose steps are a few ticks long. | |
| Minor: C17 cannot tell the watch name from the instance name. | `crossing_rules.rs:272`: `capped()`'s instance is `cap` and its watch `cap`, and the test checks `contains("`cap`")` once. | Give the instance another path (for example `cc`) and assert both `` `cc` `` and the watch name. | |
| Minor: C3 does not tell `PREVIOUS` = "before the microstep" from `PREVIOUS` = "the last intermediate commit". | `instant.rs` test `a_settled_change_is_one_observation`: in HIGH, LOW, HIGH over LOW, the commit before the last is LOW, which is also the value before the microstep. | Use a multi-valued net (for example an `F64` holding 1.0 with commits 2.0, 3.0) and assert `PREVIOUS` reads 1.0. | |
| Info: within one microstep, `Fire` observations now precede every `Change` observation. `apply_due` fires event nets inline and writes discrete nets after the loop (`crates/piperine-digital-solver/src/instant.rs:44`). Before, observations followed C2 commit order. | No spec clause binds observation order inside a microstep, and every digital, protocol and parity test is green. | Write the order down in Part V §15 or abi.md, or keep C2 order in the observation stream. | |
| Info (accepted scoping): sweep-point crossings (SCX-02's "sweep points") are proven only as two operating points of one circuit, because the solver has no DC sweep driver. | Out of Scope row naming SCT-10 / `solver-time-point`; C15 covers the comparison that persists across points. | None. SCT-10 has to carry a check that the comparison persists across sweep points. | |

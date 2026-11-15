# Spec ABI Amendments verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 3e77174d (13 pass); judged at f89a33b9
**Verifier**: independent sub-agent (author != verifier)

The seven ABI amendments are written as abi.md §15 words them. Amendments 5, 7 and 3b are not
applied: `part_vi_io.md` and §8.2.3 are untouched, and §8.5.11/15/16 are unchanged (G.65 says
`"cosim"` stays discrete-only). §17.1 rule 2 matches `Tolerances::residual_test`: node rows use
`vntol_for` (per-node potential `abstol`, default 1e-6), branch rows use `abstol` (1e-12), and the
test runs on every Newton solve (`convergence.rs:121`). The `gmin`/`source_scale` built-in rows and
their two `pom_resolve` tests are correct: either test fails if a row is removed.

The §15.2 rewrite is not true to the solver, and it does not agree with the rest of Part V. The
text reads as if every jump goes through the digital-event path of `transient.rs`. Two other paths
behave differently: host `set`s, and changes made after the solve.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `spec_text.rs` `later_commit_is_not_a_time_point` | yes - both phrases are new to §17.2 (the only earlier "at that instant" is in §16.4) |
| C2 | `spec_text.rs` `time_point_runs_the_procedure` | yes |
| C3 | `spec_text.rs` `extern_c_interface_is_the_device_abi` | yes for `piperine_abi_1` and G.65; the `"cosim"` irreversible / `"osdi"` adapter sentences are unguarded |
| C4 | `spec_text.rs` `structure_changing_set_is_refused` | no - `re-elaborates` was already in rule 7 ("the host re-elaborates (§8.1.2)"); deleting the `Dc`-sweep clause leaves the test green (tried) |
| C5 | `spec_text.rs` `zero_bytes_are_the_unknown_value` | yes |
| C6 | `spec_text.rs` `continuation_variables_are_core_functions` | yes - asserts both table rows and their class |
| C7 | `spec_text.rs` `records_travel_as_quiet_commits` | yes - `no microstep` is new; `quiet` already appeared once in the file (C.2 table) |
| C8 | `spec_text.rs` `jump_has_no_solve_of_its_own` | partly - holds AC 1 and 3; AC 2 (landing step exempt, backward-Euler restart) is unasserted |
| C9 | `spec_text.rs` `reference_rows_describe_the_kept_jump` | no - `restart` contains `set` through `` `sets_just_applied` ``; deleting "after a host `set` only" leaves it green (tried); G.6 removal (AC 7) is unasserted |
| C10 | `spec_text.rs` `residual_test_uses_the_present_mapping` | yes - the base text has no "node's row is tested" sentence, so a revert fails; the ngspice-parity sentence is unasserted |
| C11 | `spec_text.rs` `jump_solve_is_a_transient_step` | yes for AC 9; F.5 (AC 10) is unasserted |
| C12 | `cargo test -p piperine-lang` (all targets, incl. `pom_resolve` `continuation_variable{s_are_builtin_functions,_redefined_is_e0457}`) | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1. The time-point procedure still solves *before* microstep 0, which contradicts the new §15.2 | `part_v_execution.md` step 1 says "Solve at `t`. The result is the **left value**" and then applies `(t, 0)` commits in step 3.1; step 3.3 says `n = 0` reads "the left value". The new §15.2 applies what is due at microstep 0 before the step that arrives at `t`, and defines **left value** a second time (index.md: "one defining occurrence", pointing at step 1). S2/S3 still treat a settle commit as the special case ("no second solve, no jump, no integrator restart"), while §15.2 rule 6 restarts after every jump. §16's intro (l. 285) gives "the first solve comes after the commits of microstep 0" as the operating point's *one difference*. The code (`attempt_step`: `drain_due` + `run_digital_at(t_next)` before `execute_timestep`) matches §15.2, so step 1 is the stale text | Rewrite step 1/2/3.3 and S2/S3 so that microstep-0 changes are applied before the step to `t`; keep one definition of left value; rewrite or drop §16's "one difference" sentence | |
| F2. §15.2 is false for host `set`s | `accept_step` applies scheduled sets in `apply_scheduled_sets` only after the step to `t` is accepted, so the step that arrives at `t` does not read the set. `predict_step` marks `landed_on_breakpoint` only for `Digital \| Breakpoint`, never `ScheduledSet`, so the landing step is LTE-tested. The exempt step is the next one (`post_set_step`), and it starts at 10⁻³. A set of `OperatingPoint` strength re-solves the closed step to `t` (`execute_timestep(t − dt, dt)`): a second solve at the same `t`, which contradicts "no solve of its own" | State the host-`set` path in §15.2/C.2: the set applies after the step to `t` is accepted (an operating-point-strength set re-solves that step); the next step reads it, is exempt, and restarts at 10⁻³. Or report it as a deviation | |
| F3. "The previous derivative is dropped after every jump" and "a change in a later microstep is read by the first step after `t`" are both false in part | `prev_h = 0` is set only for a landed digital event or breakpoint, an applied set, or a device `discontinuity()` (`accept_step` l. 1029-1048). A carrier change made in `settle_digital` (A2D accept hooks or a `cross` at an ordinary accepted point) neither restarts the integrator nor exempts the reading step from the LTE test. In the other direction, `run_digital_at` (`evaluate_until_stable` / `evaluate_dag_ordered`) runs the zero-delay delta cycles at `t` to quiescence before the solve. Those later-microstep changes are read by the step that arrives at `t`, not by the first step after it. Affects §15.2 body and rule 6, C.2 rows "the jump" and "restart", F.5 "Restart" | Split by cause: changes produced before the solve (digital events at `t` and their delta cycles) versus reactions that read the solution; state when the restart and the exemption actually happen | |
| F4. A crossing caused by a jump has no stated microstep | The row "`cross` crossed between the two sides of the jump → `(t, n + 1)`" and its Resolution were deleted. The new §13 rule 5 row says "located by the solver (§17.3)", and §15's table puts a located crossing at `(t, 0)`. For a `(t, 0)` change, that places the crossing in the same microstep as its cause, and §17.3's bracket `(t_prev, t]` would locate it before the cause. Today the A2D/event accept hooks fire it after the reading step is accepted (post-solve `settle_digital` at that step's time), as C.2's crossing gap says | Give the jump row of §13 rule 5 and the §15 event table the present behaviour: it fires at the time point of the step that reads the change, after that step's solve, and is not located before the change | |
| F5. Stale dependents of §15.2 | §14.5.2 "the test of whether a jump moved it (§15.2.6)"; §14.5.3 "the history is dropped only when the jump moved it (§15.2.6)"; App. D l. 237 "derivative differs across the jump, so the integrator restarts for it"; App. F l. 268 "so a jump is solved"; §15 "There are four kinds" next to "not a fourth kind of solve on its own"; F.5 node rows printed in backward-Euler form `(v − 1)/h` while the landing step is TR-BDF2 (charge is still conserved; label it or show the TR stage) | Update each sentence to the kept algorithm | |
| F6. Three guards are looser than their criteria | C4 (finding row above, mutation green); C9 (`set` matched by `sets_just_applied`, mutation green; AC 7 unasserted); C8 does not assert AC 2; C11 does not assert AC 10 | C4: assert `Dc` inside the rule-7 slice. C9: assert "host `set`" and `10⁻³` in the restart row, and that G.6's gap table has no `jump` row. C8: assert `exempt` and `backward Euler`. C11: assert F.5 "first step that reads the jump" | |
| F7 (minor). §14.4.4 does not give the value of `gmin()`/`source_scale()` in a reaction | §14.4.4 lists `now()`, `freq()`, `analysis(Static)` only. §14.4.7 speaks of solves | Add "at rest" values for reactions to §14.4.4 | |

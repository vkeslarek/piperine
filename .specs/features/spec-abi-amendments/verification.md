# Spec ABI Amendments verification

**Verdict**: PASS (round 2); N1 goes to the user
**Round**: 2
**Proofs**: `proofs.md` round 2 at 374a188e (13 pass); judged at d08fee24 (round 1: 3e77174d / f89a33b9)
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
| C4 | `spec_text.rs` `structure_changing_set_is_refused` | yes (round 2) - asserts "in a `Dc` sweep ... re-elaborates the instance" in rule 7 and in §8.5.10; deleting either clause turns it red (tried) |
| C5 | `spec_text.rs` `zero_bytes_are_the_unknown_value` | yes |
| C6 | `spec_text.rs` `continuation_variables_are_core_functions` | yes - asserts both table rows and their class |
| C7 | `spec_text.rs` `records_travel_as_quiet_commits` | yes - `no microstep` is new; `quiet` already appeared once in the file (C.2 table) |
| C8 | `spec_text.rs` `jump_has_no_solve_of_its_own` | yes (round 2) - rule 6 must say `backward Euler` and `drops the previous derivative`, and the before-the-solve case must say it is exempt; removing either turns it red (tried) |
| C9 | `spec_text.rs` `reference_rows_describe_the_kept_jump` | yes (round 2) - asserts `10⁻³ of the last step` and "after a host `set` only" in the restart row, and no G.6 gap row names a jump; removing the clause or planting a G.6 jump row turns it red (tried) |
| C10 | `spec_text.rs` `residual_test_uses_the_present_mapping` | yes - the base text has no "node's row is tested" sentence, so a revert fails; the ngspice-parity sentence is unasserted |
| C11 | `spec_text.rs` `jump_solve_is_a_transient_step` | yes (round 2) - also asserts F.5's "right values are those of the first step that reads the jump"; removing it turns it red (tried) |
| C12 | `cargo test -p piperine-lang` (all targets, incl. `pom_resolve` `continuation_variable{s_are_builtin_functions,_redefined_is_e0457}`) | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1. The time-point procedure still solves *before* microstep 0, which contradicts the new §15.2 | `part_v_execution.md` step 1 says "Solve at `t`. The result is the **left value**" and then applies `(t, 0)` commits in step 3.1; step 3.3 says `n = 0` reads "the left value". The new §15.2 applies what is due at microstep 0 before the step that arrives at `t`, and defines **left value** a second time (index.md: "one defining occurrence", pointing at step 1). S2/S3 still treat a settle commit as the special case ("no second solve, no jump, no integrator restart"), while §15.2 rule 6 restarts after every jump. §16's intro (l. 285) gives "the first solve comes after the commits of microstep 0" as the operating point's *one difference*. The code (`attempt_step`: `drain_due` + `run_digital_at(t_next)` before `execute_timestep`) matches §15.2, so step 1 is the stale text | Rewrite step 1/2/3.3 and S2/S3 so that microstep-0 changes are applied before the step to `t`; keep one definition of left value; rewrite or drop §16's "one difference" sentence || 63d43683, 57f9df3a |
| F2. §15.2 is false for host `set`s | `accept_step` applies scheduled sets in `apply_scheduled_sets` only after the step to `t` is accepted, so the step that arrives at `t` does not read the set. `predict_step` marks `landed_on_breakpoint` only for `Digital \| Breakpoint`, never `ScheduledSet`, so the landing step is LTE-tested. The exempt step is the next one (`post_set_step`), and it starts at 10⁻³. A set of `OperatingPoint` strength re-solves the closed step to `t` (`execute_timestep(t − dt, dt)`): a second solve at the same `t`, which contradicts "no solve of its own" | State the host-`set` path in §15.2/C.2: the set applies after the step to `t` is accepted (an operating-point-strength set re-solves that step); the next step reads it, is exempt, and restarts at 10⁻³. Or report it as a deviation || 1da9e362 |
| F3. "The previous derivative is dropped after every jump" and "a change in a later microstep is read by the first step after `t`" are both false in part | `prev_h = 0` is set only for a landed digital event or breakpoint, an applied set, or a device `discontinuity()` (`accept_step` l. 1029-1048). A carrier change made in `settle_digital` (A2D accept hooks or a `cross` at an ordinary accepted point) neither restarts the integrator nor exempts the reading step from the LTE test. In the other direction, `run_digital_at` (`evaluate_until_stable` / `evaluate_dag_ordered`) runs the zero-delay delta cycles at `t` to quiescence before the solve. Those later-microstep changes are read by the step that arrives at `t`, not by the first step after it. Affects §15.2 body and rule 6, C.2 rows "the jump" and "restart", F.5 "Restart" | Split by cause: changes produced before the solve (digital events at `t` and their delta cycles) versus reactions that read the solution; state when the restart and the exemption actually happen || 9ba45bd0 |
| F4. A crossing caused by a jump has no stated microstep | The row "`cross` crossed between the two sides of the jump → `(t, n + 1)`" and its Resolution were deleted. The new §13 rule 5 row says "located by the solver (§17.3)", and §15's table puts a located crossing at `(t, 0)`. For a `(t, 0)` change, that places the crossing in the same microstep as its cause, and §17.3's bracket `(t_prev, t]` would locate it before the cause. Today the A2D/event accept hooks fire it after the reading step is accepted (post-solve `settle_digital` at that step's time), as C.2's crossing gap says | Give the jump row of §13 rule 5 and the §15 event table the present behaviour: it fires at the time point of the step that reads the change, after that step's solve, and is not located before the change || d36ceedf |
| F5. Stale dependents of §15.2 | §14.5.2 "the test of whether a jump moved it (§15.2.6)"; §14.5.3 "the history is dropped only when the jump moved it (§15.2.6)"; App. D l. 237 "derivative differs across the jump, so the integrator restarts for it"; App. F l. 268 "so a jump is solved"; §15 "There are four kinds" next to "not a fourth kind of solve on its own"; F.5 node rows printed in backward-Euler form `(v − 1)/h` while the landing step is TR-BDF2 (charge is still conserved; label it or show the TR stage) | Update each sentence to the kept algorithm || 42748430 |
| F6. Three guards are looser than their criteria | C4 (finding row above, mutation green); C9 (`set` matched by `sets_just_applied`, mutation green; AC 7 unasserted); C8 does not assert AC 2; C11 does not assert AC 10 | C4: assert `Dc` inside the rule-7 slice. C9: assert "host `set`" and `10⁻³` in the restart row, and that G.6's gap table has no `jump` row. C8: assert `exempt` and `backward Euler`. C11: assert F.5 "first step that reads the jump" || 6cf7fb04 |
| F7 (minor). §14.4.4 does not give the value of `gmin()`/`source_scale()` in a reaction | §14.4.4 lists `now()`, `freq()`, `analysis(Static)` only. §14.4.7 speaks of solves | Add "at rest" values for reactions to §14.4.4 || ca86b529 |

## Round 2

I checked each fix (63d43683..f0cfb5a1, f7c48623) against `crates/piperine-solver/src/analyses/transient.rs` and `core/circuit.rs`.

- **F1 holds.** `attempt_step` runs `drain_due` and `run_digital_at(t_next)` (scheduler to quiescence, `analog_slice = &[]`) before `execute_timestep`. That is the new order: steps 1, 3.3 and 3.4 of §15, I5/I6, S2 and the reads table. Left value now has one definition (§15.2), and index.md points at it.
- **F2 holds.** `predict_step` lands on a `ScheduledSet` without setting `landed_on_breakpoint`, so the landing step is LTE-tested. `apply_scheduled_sets` runs in `accept_step` after `settle_digital`, which is §15 step 5. An `OperatingPoint` set retakes the step with `execute_timestep(t − dt, dt)` and replaces the recorded point. The next step has `post_set_step`, so it is exempt. Because `landed_on_breakpoint = true`, `prev_h = 0`, so the integrator restarts. `propose_dt` sets the next size to `1e-3 · dt_actual` floored at `dt_min`. §15.2 "Host `set`", §9 rule 4, §23.1 rule 4 and C.2 all say this.
- **F3 holds.** `prev_h = 0` is set only after a landed digital event or breakpoint, after an applied set, or after `discontinuity()`. A change made in `settle_digital` restarts nothing and is not exempt. §15.2 (before/after the solve), rule 6 and the C.2 rows "the jump" and "restart" all say so.
- **F4 holds.** Accept hooks detect a crossing by comparing the previous accepted solution with the new one. It fires in `settle_digital` after the reading step, never before its cause. §13 rule 5, the §15 table, §15.2.4, §17.2 A4 and §17.3 rules 4 and 8 agree. The C.2 crossing gap (no location) is unchanged.
- **F5 holds.** §14.5.2, §14.5.3, App. D (Pwl corner, DeltaSigma), App. F (Dram, F.5 now in TR-stage form) and "three kinds" of solve are all fixed. F.5's derivation checks out: with no earlier derivative, the TR stage and the BDF2 stage each conserve the cut's charge.
- **F6 holds.** All seven planted removals now turn their test red; see the Checks rows above.
- **F7 holds.** §14.4.4 now gives the at-rest values.
- **Sweep.** I grepped every page for `n − 1`, "solved first", `§15 step 1`, "second solve", "moved it", "fire together at `(t, 0)`", `h → 0`, "impulsive" and "repeated once", and checked every citation of `§15.2.4` and `§15.2.6`. Nothing stale remains except N2.

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| N1 (Major, user decision). The amended order contradicts the device ABI the feature set out to agree with | abi.md §9 loop step 5: "accept the analog solution at t_c; run the instant t_c". abi.md row 11: a reaction "reads ... the accepted solution". The spec now solves after the pre-solve microsteps, and its reactions before step 3.4 read the previous time point. The new §15.2 `> **Note.**` names the conflict and hands it to `solver-abi`. The text is true to the solver, as Q7 asks. Under it, a probing reaction at a scheduled `t` (a sampler) reads the solution of the previous time point, even though §17.2.2 still makes `t` a time point for it | The user chooses whether the ABI loop or the solver changes order. This feature's text cannot settle it || 0e9f7651 |
| N2 (Minor). §16 H2 cites §15.2.6 for the restart at the first step, but rule 6 now lists only two restarting steps | `part_v_execution.md` l. 378. The code does restart there (`prev_h: 0.0` at `transient.rs` l. 574), so only the citation is wrong | Add "and the first step after the operating point" to rule 6, or cite C.2 || 0f59e34d |
| N3 (Minor, unverified). "Reactions before step 3.4 read the previous time point's solution" is not established by the solver crate | The pre-solve `run_digital_at` passes an empty `analog_slice`. What a `SAMPLES_ANALOG` element reads there depends on its own cache, and that code lives in codegen, which is outside this worktree | Confirm against `piperine-codegen`'s digital device, or word §15 step 3.3 as "the last accepted solution the element holds" || 1a3b3fce |
| N4 (Minor, unverified). The claim in S3 / §17.2.2 that a settle commit is read by a step that is not exempt and does not restart has no present code path | The solver has no settle commit. A commit at a fixed `t_next` would enter the queue as a `Digital` event, which `predict_step` marks as landed: exempt, and restart. S3 says the opposite | Mark S3 as target semantics for `solver-abi`, or make it agree with how a digital event at `t_next` is treated || ef2ae630 |

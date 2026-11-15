# Solver ABI Digital verification

**Verdict**: FAIL
**Round**: 2
**Proofs**: `proofs.md` at d75324d6 (47 pass; HEAD 3e4cde2e adds docs only). Round 1 was at c2872a20 (46 pass)
**Verifier**: independent sub-agent (author != verifier), Opus, rounds 1 and 2
**Probes**: `/tmp/claude-1000/-home-keslarek-Git-piperine/570f7032-f229-4af2-937c-a6378b382406/scratchpad/verify-abi-digital/zz_verify_probe.rs` (symlinked into `crates/piperine-digital-solver/tests/` to run, removed afterwards, never committed). Five of nine probes fail at HEAD.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `ticks.rs` `a_delay_is_due_at_the_nearest_tick` | yes - the half-tick boundary on an exact 1 s tick, both sides |
| C2 | `ticks.rs` `a_time_outside_the_tick_range_is_e0502` | yes - both signs, NaN, inf, and an overflowing due |
| C3 | `ticks.rs` `a_nan_delay_is_e0435` | yes - at the tick layer; the site (instance, statement) is asserted in `reactions.rs` `an_infinite_delay_is_never_stored_and_a_nan_delay_is_e0435` |
| C4 | `ticks.rs` `an_infinite_delay_never_comes_due` | yes - but see finding 4: an infinite commit is also skipped by C3 removal |
| C5 | `store.rs` `a_net_starts_as_zero_bytes` | yes |
| C6 | `store.rs` `a_change_keeps_the_previous_bytes` | yes - with `a_write_of_equal_bytes_is_not_a_change` (-0.0, NaN bytes) |
| C7 | `store.rs` `changed_reads_one_for_exactly_one_microstep` | yes |
| C8 | `bind.rs` `two_writers_of_one_net_are_refused_naming_both` | yes - function/function and function/statement, both pins asserted |
| C9 | `bind.rs` `unequal_layouts_on_one_net_are_refused` | yes |
| C10 | `dag.rs` `a_cycle_through_no_relax_net_is_refused_naming_its_functions` | yes for the cases it names; but the rule it proves is component-level and lets an inner non-RELAX cycle through (finding 3) |
| C11 | `functions.rs` `a_function_runs_when_a_read_net_changed` | yes - with `dag.rs` `a_function_runs_after_every_writer_of_its_reads` for the order half |
| C12 | `functions.rs` `a_function_with_unchanged_inputs_is_not_called` | yes at `run_functions`; at the engine a relaxed group is left dirty and re-runs in a second instant at the same tick (finding 2) |
| C13 | `queue.rs` `commits_apply_in_made_statement_call_order` | yes - each key of the triple decides one pair |
| C14 | `queue.rs` `transport_removes_later_commits_of_the_same_statement_only` | yes - earlier, later, equal due, other statement, other reaction |
| C15 | `queue.rs` `replace_removes_every_pending_commit_of_the_statement` | yes at the queue; a `REPLACE` commit with `+inf` removes nothing at the engine (finding 4) |
| C16 | `functions.rs` `a_relax_group_settles_within_its_bound` | yes - asserts two passes exactly and one observed change |
| C17 | `functions.rs` `a_moving_known_field_is_zeroed_and_observed` | yes - with `a_moving_field_without_a_known_byte_fails` for AC 5 |
| C18 | `bind.rs` `later_is_dropped_when_its_bound_is_under_one_tick` | yes - own probe: a bound of exactly one tick keeps the flag, one tick plus 1e-7 drops it |
| C19 | `reactions.rs` `a_reaction_runs_once_when_its_event_fired` | yes - including two of its events in one microstep |
| C20 | `reactions.rs` `reactions_of_one_microstep_read_one_snapshot` | yes - a swap ring of two flip-flops |
| C21 | `reactions.rs` `a_zero_delay_commit_applies_at_the_next_microstep` | yes - 0, negative, sub-half-tick |
| C22 | `reactions.rs` `a_commit_outside_its_statement_range_is_a_fault` | yes - with `a_later_commit_below_its_bound_is_a_fault` for AC 13 |
| C23 | `instant.rs` `the_traced_instant_of_section_15_1` | yes - matches the table of `part_v_execution.md` §15.1 row by row |
| C24 | `instant.rs` `an_instant_that_never_goes_quiet_is_e0501_naming_the_nets` | yes |
| C25 | `functions.rs` `a_function_fault_carries_code_tick_and_instance` | yes - code, message, tick, instance as typed fields; `RESHAPE` outside the cone |
| C26 | `reactions.rs` `finish_marks_the_instant_final` | yes - at the reaction layer; the instant's `Outcome.finish` is asserted by C33's test |
| C27 | `instant.rs` `a_quiet_commit_does_not_start_a_microstep` | yes |
| C28 | `instant.rs` `start_commits_inits_and_fires_start` | yes |
| C29 | `instant.rs` `a_settle_commit_with_no_delay_is_refused` | yes |
| C30 | `checkpoint.rs` `restore_returns_store_queue_and_observations_byte_for_byte` | yes - checkpoint equality, zero device calls, and the replay matches a fresh run |
| C31 | `checkpoint.rs` `two_runs_give_identical_observations` | yes - two insertion orders and a rerun |
| C32 | `advance.rs` `advance_stops_before_the_first_time_point_and_leaves_it_untouched` | yes |
| C33 | `instant.rs` `an_instant_reports_boundary_fired_and_final` | yes |
| C34 | `advance.rs` `each_clause_of_the_time_point_test_stops_advance` | yes - one case per clause; own probe: a statement reaching a sampling reaction through its `on` (timer tick to sampler) also lands at tick 0 |
| C35 | `advance.rs` `horizon_is_before_the_earliest_pending_commit_and_the_limit` | yes for the AC text; it never has a second reaction committing in the same microstep, so it does not catch finding 1 |
| C36 | `advance.rs` `a_commit_due_after_the_target_stays_pending` | yes - with `times_that_round_to_one_tick_are_one_instant` for AC 9 |
| C37 | `checkpoint.rs` `every_change_commit_and_record_is_observed_in_order` | yes |
| C38 | `checkpoint.rs` `observations_after_a_checkpoint_are_gone_after_restore` | yes |
| C39 | `host.rs` `a_host_write_is_a_commit_at_microstep_zero` | yes |
| C40 | `host.rs` `a_structural_param_write_during_a_run_is_refused` | yes - checkpoint equality proves nothing changed |
| C41 | `host.rs` `a_false_check_refuses_the_write_and_changes_nothing` | yes |
| C42 | `host.rs` `reshape_on_the_shadow_names_the_instance_and_changes_nothing` | yes |
| C44 | `boundary.rs` `abi_modules_name_no_analog_or_owner_item` | yes - with `the_crate_depends_on_the_abi_crate_only` and `abi_modules_hold_no_unsafe` |
| C46 | `round1.rs` `horizon_counts_the_commits_other_reactions_make_in_the_same_microstep` | yes for two reversible reactions in both call orders (round 1 gave `i64::MAX - 1`, so it fails if the rule is removed); it has no `IRREVERSIBLE` reaction, and SAD-23 AC 11 fails there (finding 7) |
| C45 | `cargo test -p piperine-digital-solver --lib` (27 passed) | yes - and `git diff 2974b806..HEAD -- src/{drivers,events,interface,scheduler,state,topology}.rs` is empty; the only feature commit outside the crate and `.specs` touches `Cargo.lock` |

## Known deviations, judged

- **`advance` stops strictly before `to`**: accepted. Under "ABI order: solve, then react" the instant at `t_c` is step 5's `run_instant`; probe `probe_g` confirms it is left untouched and `next_due` still reports it.
- **RELAX rule "at least one RELAX net" vs design "every internal net RELAX"**: neither matches abi.md §9 D1, which refuses *a cycle* through no `RELAX` net. The design's rule would refuse the resolver loop itself (its value pin is not `RELAX`); the implementation's rule, applied per strongly connected component, accepts an inner cycle through no `RELAX` net. Finding 3.
- **Host commits are never transported**: accepted. C3 is stated per statement and the host has none; two scheduled host writes both apply, the later-made one wins by C2.
- **Extra fixtures** (`Succ` to `Clockwatch`): accepted; each is used by a test.
- **P3 canonical form checked only in debug, only on function outputs**: not enough. abi.md P3 says "debug sessions check this canonical form on every write"; reaction commits and host commits are writes and are not checked, and no test exercises even the function-output check. Finding 6.
- **Old engine files**: untouched (see C45).

## Round 2

Fixes e01e8c0b..dba31035 (after the ABI-view adaptation a7e34a77). Every round-1 probe was re-run at HEAD, together with a new probe `probe_j`.

- **Finding 1 (re-call fixed point)**: sound for reversible reactions. R3 says a reaction is a function of its reads, and `HORIZON` is one of those reads, so the engine can call a reaction again and drop the earlier calls. Nothing is applied or observed until the final calls: commits, records and `FINISH` all come from them. The loop terminates: each horizon only decreases, at most `reversible + 1` passes run, and then every horizon falls back to the instant itself, which is always sound. A commit due at the next microstep caps the horizon at the current tick, so the cascade through later microsteps is also covered. The `probe_b` horizon is now 20 ns - 1. One gap remains: the `IRREVERSIBLE` reaction runs last and its commits never bound the horizons of the reversible reactions (finding 7). Two minor costs, not findings: (a) a reaction that faults on a discarded call aborts the instant; (b) the bound applies whatever the reach, which is conservative.
- **Finding 2**: closed. In `probe_a`, `next_due` is `None` and the resolver is not called again.
- **Finding 3**: closed. Removing the edges through `RELAX` nets and requiring the rest to be acyclic is exactly abi.md D1. In `probe_i`, bind refuses the design and names g1 and g2; `dag.rs` `an_inner_cycle_through_no_relax_net_inside_a_relax_component_is_refused` holds the case.
- **Finding 4**: closed. `probe_c` still fails, but the probe was wrong: it also expected the non-`REPLACE` `out` edge at 10 ns to go, and C3 keeps that edge for a never-due commit. The fix's test (`round1.rs` `replace_with_an_infinite_delay_removes_the_pending_commits`) separates the two statements correctly.
- **Finding 5**: closed. In `probe_d`, the write is refused with `HostWrite` and nothing panics; the test also covers an event net with bytes and a net that does not exist.
- **Finding 6**: closed. The canonical-form check now runs after every applied commit (reaction and host, the quiet exit included) and on function outputs, in release builds too. The `sloppy` fixture faults through a function, a commit and the host.
- **Regressions**: `probe_e` to `probe_h` still pass. The digital crate's proofs are green at d75324d6 (47).

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1. `HORIZON` is not sound (abi.md R5): it ignores commits made by other reactions in the same microstep, so a reaction can be told it may run its clock past an event already scheduled for one of its pins. The spec's own premise ("a conservative horizon is always sound") does not hold for this one | `probe_b`: timer and clockwatch both fire on `t` at 10 ns; the timer commits `t` again at 20 ns in that microstep; the clockwatch reads `HORIZON = i64::MAX - 1` (`instant.rs` computes it once, before `run_reactions`) | when any other reaction fires in the microstep, or anything is due later in this tick, bound the `HORIZON` of every reaction to the current tick (or compute reach from the other firing reactions' statements); add a criterion (new SAD id) and a check with two reactions in one microstep | dba31035 |
| 2. A relaxed group is left dirty after its instant, so `next_due` reports the tick just run and the engine runs a second instant at that tick, calling the group's functions again (F3; observations can then go back from `(t, n)` to `(t, 0)`) | `probe_a`: keeper loop (resolver + and2), after `run_instant(0)` `next_due() == Some(Tick(0))`; `advance(100)` re-enters tick 0 and calls the resolver twice more. Cause: `run.rs` `relax` ends with `write_net`, which re-marks the group's own members after `walk` cleared them | after the closing `write_net`s of `relax`, clear the dirty flags of the group's members (they have just settled); add a check that `next_due` is `None` after an instant with a settled RELAX group | e01e8c0b |
| 3. A cycle through no `RELAX` net is accepted when it shares a component with a cycle through one (abi.md §9 D1, spec SAD-05 AC 10) | `probe_i`: g1 (reads n2, writes n1), g2 (reads n1, n3, writes n2), r (reads n2, writes n3), only n3 `RELAX`: `Dag::build` returns one `Relax` step; the g1-g2 cycle passes through n1, n2 only | within each component, drop the edges through `RELAX` nets and refuse if the rest still has a cycle, naming its functions; add the case to C10's test | a68dad5f |
| 4. A `REPLACE` commit with delay `+inf` removes no pending commit of its statement (abi.md §6 C3) | `probe_c`: timer pending `tick` at 10 ns, then a `START` at 5 ns with `half = +inf`: after the instant `next_due() == Some(10 ns)`. `run.rs` skips `queue.push` when `due_at` is `None` | apply the C3 removal for a never-due commit too (remove, do not insert); add the case to C15's test | 48a5ed55 |
| 5. An out-of-range host write is accepted and later panics inside the store instead of failing loud | `probe_d`: `host_commit(n, 1, &[1,1,1], ..)` on a 2-byte net returns `Ok(())`; `run_instant` panics at `store.rs:135` (slice index) | validate `offset + len <= size` (and empty bytes for an event net) in `host_commit`, returning `DigitalError::HostWrite`; add a check | 50b71e87 |
| 6. The P3 canonical-form check covers function outputs only, and nothing tests it (abi.md §3.1 P3: "on every write"; spec Goals name P1 to P7) | `run.rs:202` is the only `is_canonical` call; `run_reactions` and `host_commit` write unchecked; no test in `tests/` names P3 or non-canonical bytes | in debug builds check the net's bytes after each applied reaction or host commit; add a test with a device that writes non-canonical bytes through a function and through a commit | e0293f51 |
| 7. (Medium, round 2) An `IRREVERSIBLE` reaction's commits do not bound the `HORIZON` of the other reactions in its microstep (abi.md R5; spec SAD-23 AC 11, "another reaction of the microstep") | `probe_j`: an irreversible reaction on `a` re-fires `a` 5 ticks later, and a clockwatch also on `a` reads `HORIZON = i64::MAX - 1` in both instance orders. Cause: `run.rs` `run_reactions` computes the reversible fixed point before the irreversible reaction runs, and never re-calls a reversible reaction after it | after the irreversible reaction runs, cap the horizon of every reversible reaction by its commits and re-call the ones that were cut. This is sound because they are pure (R3), but it can move their commits below the irreversible reaction's own horizon. Simplest sound rule: when an irreversible reaction fires together with others, give every reaction of the microstep the current tick as its horizon. Add an irreversible case to C46's test | fd84748f |

# Unknown Semantics: Events verification

**Verdict**: FAIL
**Proofs**: `proofs.md` not re-run at 35266266 (see the B5 round: the root filesystem was at 93 %, past the 90 % stop line, before any build, so no cargo command ran)
**Verifier**: independent sub-agent (author != verifier)

## B5 round (T24 to T31, closure) - HEAD 35266266

Range judged: `git diff f8595df9..35266266` (15 files). Read against `docs/spec/part_iv_behavior.md` §13 and
`docs/spec/part_i_values.md` §3 and §3.2. No own example could run: `df /` reported 93 % (11 GB free) at the
start of the round, past the stop line the dispatch set, so the review is by reading and no targeted test,
`check_proofs.py` or suite ran.

What holds: the inliner (`elab/event_functions.rs`) is wired at the one place events are walked
(`events.rs` `disjuncts`, `Call` arm), restores the scope through `Environment` so two calls never see each
other's arguments, nests, and evaluates every argument before binding any. The `bool` read is found by the
declared result (`is_event_function`), not the call's type, which is right since §13 rule 2 types the read
`bool`. The Settled B5 amendments are honoured: no `HandleBinding`, no `let mut` dependency, T27 tests-only.
`docs.rs` counts (20 to 19 loud) match the removed L53 row; values#8 and values~c389077a flip to `run`/`holds`
on `a_function_has_the_class_its_body_gives`, which asserts one handshake at exactly 1 us (the 0.2 us rise
with `ack` low excluded); digital#6 points at `the_phase_detector_runs_assignment_and_events_read_as_bools`;
MD-74 records the B5 decision; ROADMAP marks the feature delivered. No tolerance or parity contract moved
(`crates/piperine-parity/src`, `tests/support` untouched).

### B5 checks

| Check | Test | Holds |
| --- | --- | --- |
| C3 | `source_events.rs` `pfd3_rising_together_leaves_state_unchanged` | yes - asserts no `up`/`dn` move at 40 ns and the full `up` sequence (the 80 ns step from state 1) |
| C23 | `types_events.rs` `a_function_returning_event_is_accepted_in_every_event_position`; `source_events.rs` `an_event_function_in_every_event_position` | no - AC 1 names "the result of a module"; neither test places the call as a module's `Event` result (F8) |
| C24 | `source_events.rs` `handshake_fires_when_req_rises_with_ack_high` | yes - exact instants 40, 60, 110, 130 ns; 10, 20, 80 ns excluded by the equality |
| C25 | `source_events.rs` `an_event_function_read_as_a_bool_says_whether_it_fired` | yes - every record of either edge with its exact flag |
| C26 | `types_events.rs` `a_function_returning_event_outside_an_event_position_is_e0441` | yes - code and span, and the `bool` read in body and guard |
| C36 | `docs.rs` `gaps_no_longer_lists_settle_or_limexp`, `gaps_no_longer_lists_l51_or_l53` | yes for the text it asserts; but closing L53 leaves refused, spec-allowed forms with no row (F7) |
| C40 | `source_events.rs` `an_event_function_calling_another_fires_under_the_outer_guard` | yes - exactly 110 and 130 ns |
| C45 (third proof) | `elab_digital.rs` `an_event_function_call_lowers_to_its_body_event` | yes - structural print equal to the hand-written event, swapped arguments swap it |
| C46 (B5 proofs) | `book_digital.rs` `the_phase_detector_runs_assignment_and_events_read_as_bools`; `book_values.rs` `a_function_has_the_class_its_body_gives` | yes |

### B5 findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F6 (Major, silent) - an event function binds its arguments by position only: a named argument binds the wrong parameter, a left-out optional one is never bound, and an `Option<T>` parameter is not given `Some`/`None` | `crates/piperine-lang/src/elab/event_functions.rs:83-87`: `params.iter().zip(arguments)` over `args.iter().map(\|arg\| arg.value)` (:48) ignores `arg.name` and the checker's per-call slots (`Types::binding`, `types/table.rs:408`, filled by `bind_arguments`, `types/call.rs:147`). §3.2.2 to §3.2.4 and §3.2.9 allow `fn edge(c: Bit, a: Option<bool>, b: Option<bool>) -> Event { c.rise() && a.unwrap_or(true) }` called `edge(clk, b: Some(false))`: the checker binds `b`, the inliner binds `Some(false)` to `a`, so the event never fires where the spec says it fires on every rise. The ordinary call path does it right (`expansion.rs:183` `expanded_arguments`: slots, `lower_option_slot`) | bind through `self.types.binding(call)` slots as `expanded_arguments` does (an `Option` parameter through `lower_option_slot`, a left-out one as `None`); a test with a named, a left-out and a wrapped `Option` argument | |
| F7 (Major) - three forms the spec allows are refused `P0001` with no test and no GAPS row, while L53 is closed as if functions returning `Event` all run | `event_functions.rs:29-31` refuses recursion, but §3 rule 12 allows it (unrolled while decided by params and constants, as `expansion.rs` `may_expand` already does elsewhere); `:68-73` refuses a body with statements besides its event, but §3 rules 6 and 15 allow locals (`let`) in a function body, and the event position list of §13 includes "the return value of a function", not only a tail expression; `:61-64` refuses a bodiless one, reachable by a trait-declared `fn ... -> Event;` (§3 rule 2 exempts trait items from E0125). `grep` finds no test naming any of the three messages | either make them run (recursion through the expansion's unrolling, `let`s through the scope's `values` before the tail) or reopen a GAPS §4 row naming the three forms with a real owner (`spec-closure`) and a test per refusal pinning `P0001` and its span; adjust `gaps_no_longer_lists_l51_or_l53` | |
| F8 (Minor) - C23 does not prove UE-04 AC 1's "result of a module" position, nor the generic-over-a-nature path | `types_events.rs` rows and `handshake.msl` cover head, body `let`, guard, `\|\|`, `&&`, nested function; no `module M(..) -> Event { handshake(req, ack) }`. `event_functions.rs:35-41` (`concrete_instance` + substitution for a generic event function) is reached by no test | add a module-result row to both tests and one generic event function (`fn edge<N: ..>(c: Bit<N>) -> Event`) from source | |
| F9 (Minor, cleanliness) - inline path and abbreviated binding; a stale task line | `event_functions.rs:75` `crate::msl::Span` written inline instead of imported; `:22`, `:60` bind the function data as `f`; `tasks.md:768` T28 still says "Reuses: `HandleBinding`" though the amendment (`:766`) removed it | import `Span`; name the binding `function_data` (or similar); strike `HandleBinding` from T28's Reuses | |

### B5 final run

Not run. The verdict is FAIL, and the disk was past the stop line (93 %) before any build. The full suite,
parity, buck and twins targets are for the PASS round after the fixes.

## Round 1 (B1 to B4)

**Round 1 verdict (B1 to B4)**: PASS
**Proofs**: `proofs.md` at 3ed72693 (60 pass; the 10 failing rows are the B5 proofs, deferred)
**Verifier**: independent sub-agent (author != verifier)
**Scope**: batches B1 to B4 (T1 to T23). B5 (T24 to T31: `Pfd3`, functions that return `Event`, the closing of L51 and L53) is deliberately not built yet; its checks (C3, C23 to C26, C40, the second proof of C36, the third of C45, the `bookdigital`/`bookvalues` proofs of C46) are listed `deferred`, not judged as failures.

Range judged: `git diff origin/main...HEAD` (origin/main f9a29dd0, 80 files). Own examples run at
HEAD on the Rust backend (throwaway, not committed, files restored):

- S4 from source on `commits.msl` `BetweenBench`: the first time point is at 37 ns, before the
  first clock edge (0.37 us); every `read` record is after it and reads 2 kOhm. No event time lies
  strictly between the settle commit and the next time point, so C16's source test checks nothing
  of S4 (F1).
- S6 from source on `LagBench` through `DcSolver::followed_by_another_point`: one point alone keeps
  `r` at 1 kOhm (dropped); two points land 2 kOhm at the second point's `(0, 0)`. Holds.
- §22.2.4 rule 3 on the compiled `DiodeLimexp` limit function, cases the test does not name:
  `(-30, old -5) -> -6`, `(1, 0) -> 1`, `(1.0000001, 0) -> 1`, `(-40, 39.5) -> 38.5`. Holds.

Parity: `git diff origin/main -- crates/piperine-parity/src tests/support` is empty; no
`ToleranceContract`, contract or golden is edited anywhere in the range. `source_limexp`,
`parity_tran` and `source_events` ran with ngspice present (no SKIP line, 10 + 20 + 5 passed). The
limexp `diode_series` comparison is an operating point on both sides. The `tran_clipper` twin uses
one step configuration on both sides: ngspice `tran 20u 2m 0 2u` with `.options interp`, piperine
`dt_max` = print step / 10 = 2 us on the export's 20 us grid; the model's `bound_step(0.02 / f)`
= 20 us is looser than the 2 us cap and never binds. The `.msl` twin is the Book's analog part
verbatim (the Book's diode has no `gmin` shunt, like ngspice's `rs=0` model less its internal
gmin: 1e-12 S, below the contract).

Spec agreement, judged:

- §13 D2 Resolution (`docs/spec/part_iv_behavior.md`) and the checker agree: in a reaction body an
  `Event` where a `bool` is expected types `bool` and is recorded (`Types::event_read`); onto a
  carrier of another type it is `E0441` at the event (`check_as` -> `event_where_another_type_is_expected`);
  outside a guard or a body it stays `E0441`. The `E0441` row of the §13 error table still says
  "an `Event` committed or carried" unqualified (F4).
- Settle commits: held in `SettleCommits` with no due instant, landed at `(t_next, 0)` only by an
  instant that is a time point (`Protocol::land_settle_commits`), delivered to the analog side with
  `Restart::Integrator` and the next step exempt (`exempt_next`, kept across a rejected retry),
  dropped at the stop (`drop_settle_commits_at_stop`) and after a sweep's last point
  (`DcSolver::followed_by_another_point`, passed by the api `Dc` driver). `Checkpoint` and `Mark`
  carry them, so a restore or a rewind brings back a held commit and takes back a landing (S5). S6's
  "unless the transient is continued" is not implemented; no host continues a transient past its
  stop today, and G.34 stays open as the spec says.
- bc0633c4 (`a record from the settle phase is no settle commit`, 9ec9bb84 before the rebase): what
  the spec says. §15 defines a settle commit as a commit (`<-`) of a reaction in the settle phase
  due at the next microstep; a record is an `emit!`, not a commit, has no effect on any carrier
  (§19 rule 9) and the §19 Resolution stamps a settle-phase record `(t, last microstep)`. Holding it
  to `t_next` would contradict that Resolution.
- `GAPS.md`: 40 loud after the rebase holds (L1 to L62 less 9 + 1 + 2 + 9 + 1 = 40; `docs.rs` counts
  the rows, not the sentence); "about 29 of 75" holds (`settle` is a §13/§14.9 core event, not a §22
  index name; `limexp` is). The L51 row and the §9 IV §12–§13 coverage cell still list "an event
  read as a `bool`" as refused, though both of the row's examples run at HEAD (F3).
- Code cleanliness of non-test code: every new type, method and field is doc-commented, no free
  function was added (`fired_reactions`/`run_reactions` are pre-existing free functions, only
  re-signed), names have no abbreviation, `settle_phase` keeps `run_instant` from growing. The new
  test files break the tasks' own "no free test helper" rule (F5).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `source_events.rs` `an_event_read_as_a_bool_says_whether_it_fired` | yes - `a` alone true at 10 ns, `b` alone true at 20 ns, no record at the falling 30 ns |
| C2 | `source_events.rs` `two_events_in_one_microstep_run_the_reaction_once_both_true` | yes - exactly one record at 40 ns with both true |
| C3 | `source_events.rs` `pfd3_rising_together_leaves_state_unchanged` | deferred - B5 (T25) |
| C4 | `source_events.rs` `a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it` | yes - `settle` true exactly at the time points, the edge exactly the reverse, both kinds of instant asserted to occur |
| C5 | `source_events.rs` `a_guard_reading_events_fires_only_when_d_changed_with_en_high_and_still` | yes - exactly 20 ns and 60 ns, the three excluded combinations named |
| C6 | `source_events.rs` `a_guard_reads_what_the_reaction_would_read` | yes - fires at the joint microstep only, not when `y` follows `x` one microstep later |
| C7 | `types_events.rs` `negating_an_event_outside_a_guard_or_body_is_e0440` | yes - code and span on a head, an `||` operand and a module `let` |
| C8 | `types_events.rs` `a_bool_and_an_event_in_an_event_position_is_e0440` | yes - and the same `&&` accepted in a body |
| C9 | `types_events.rs` `an_event_where_another_type_is_expected_is_e0441` | yes - a commit onto `Bit<f64>` and a module-body value, code and span |
| C10 | `source_events.rs` `a_committed_event_read_commits_whether_it_fired` | yes - `q` true, false, true at 10/20/40 ns: a constant commit fails it |
| C11 | `spec_text.rs` `the_commit_of_an_event_read_as_a_bool_is_stated` | yes - the three sentences of the Resolution (the error table row is F4) |
| C12 | `source_events.rs` `settle_runs_once_per_accepted_time_point` | yes - record instants equal the accepted steps exactly |
| C13 | `source_events.rs` `settle_does_not_run_at_an_event_time_that_is_not_a_time_point` | yes - asserts an off-grid clock edge exists, then records only at steps |
| C14 | `source_events.rs` `a_settle_commit_is_read_at_the_next_time_point`; `protocol_transient.rs` `a_settle_commit_lands_at_the_next_time_point` | yes - the carrier changes at the first time point and not at the operating point; the protocol lands before the instant runs |
| C15 | `protocol_transient.rs` `the_step_after_a_settle_commit_is_exempt_and_restarts` | yes - exempt, then not exempt; one `Restart::Integrator` delivery |
| C16 | `protocol_transient.rs` `an_event_time_before_the_next_time_point_reads_the_old_value` | yes - the stub test is the sole proof of S4 (F1 closed by decision); the source test is renamed `an_event_after_the_landing_reads_the_settle_commit` and proves S3 only |
| C17 | `instant.rs` `a_settle_commit_with_an_advancing_delay_is_an_ordinary_commit`; `source_events.rs` `a_delayed_settle_commit_is_ordinary` | yes - queued at 1 ns / changes at 250 ns, never held |
| C18 | `protocol_transient.rs` `a_rejected_step_moves_the_settle_commit`; `checkpoint.rs` `a_restore_brings_back_the_held_settle_commits` | yes - restore before the one landing at the retried point; checkpoint and journal mark both bring the commit back |
| C19 | `protocol_transient.rs` `a_settle_commit_at_the_stop_time_is_dropped`; `source_events.rs` `a_settle_commit_at_the_stop_time_is_dropped_from_source` | yes - the source test also shows the same commit lands when the run goes on |
| C20 | `instant.rs` `settle_records_carry_the_last_microstep_index`; `source_events.rs` `settle_records_follow_every_microstep_of_their_time_point` | yes - the cited instant test stamps fires and commits at `(t, 1)`; the record itself is stamped by its sibling `a_record_from_the_settle_phase_is_stamped_with_the_last_microstep` |
| C21 | `source_events.rs` `settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant`; `protocol_operating_point.rs` `a_settle_commit_of_a_dc_point_lands_at_the_next_point` | yes - one record per point and at the `finish` instant; landing order on stubs, confirmed from source by an own example |
| C22 | `source_events.rs` `settle_combined_with_an_event_fires_by_rule_1`; `instant.rs` `a_reaction_in_the_settle_phase_is_on_settle_whatever_its_head` | yes - `settle || ev` from source; `settle && g` in `instant.rs` `a_guarded_settle_event_fires_in_the_settle_phase` |
| C23 | `types_events.rs` `a_function_returning_event_is_accepted_in_every_event_position`; `source_events.rs` `an_event_function_in_every_event_position` | deferred - B5 (T27, T29) |
| C24 | `source_events.rs` `handshake_fires_when_req_rises_with_ack_high` | deferred - B5 (T29) |
| C25 | `source_events.rs` `an_event_function_read_as_a_bool_says_whether_it_fired` | deferred - B5 (T29) |
| C26 | `types_events.rs` `a_function_returning_event_outside_an_event_position_is_e0441` | deferred - B5 (T27) |
| C27 | `types_analog.rs` `limexp_checks_with_its_closure_bindings`; `source_limexp.rs` `limexp_elaborates_from_the_stdlib_with_its_closure` | yes - over the real stdlib |
| C28 | `source_limexp.rs` `limexp_and_pnjlim_diodes_agree_at_the_operating_point` | yes - existing `DC_VOLTAGE` |
| C29 | `source_limexp.rs` `limexp_moves_its_argument_at_most_one_per_iteration` | yes - expected values from `clamp(old - 1, old + 1)`, both signs; own cases hold |
| C30 | `source_limexp.rs` `the_derivative_of_limexp_is_limexp` | yes - partial `is * exp(x)` and, chained, `is * exp(x) / nvt`, relative 1e-9 |
| C31 | `source_limexp.rs` `the_limexp_diode_series_twin_agrees_with_ngspice` | yes - panics if ngspice is skipped; ran with ngspice |
| C32 | `tests/book/analog.rs` `the_clipper_prints_one_clip_record_per_period_near_0_72_volts` | yes - two `clip` records within 50 mV of 0.72 V, `wave` count equal to the table's rows |
| C33 | `tests/book/mixed.rs` `the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28` | yes - non-empty, every `regulated` record after 10 ms `pass = true` |
| C34 | `piperine-cli/tests/book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints`, `the_clipper_runs_as_the_book_prints` | yes |
| C35 | `parity_tran.rs` `tran_clipper` | yes - 101 rows under `TRAN_VOLTAGE`, same step cap on both sides |
| C36 | `docs.rs` `gaps_no_longer_lists_settle_or_limexp`; `gaps_no_longer_lists_l51_or_l53` | yes for the B4 half; `gaps_no_longer_lists_l51_or_l53` deferred - B5 (T31) |
| C37 | `docs.rs` `the_book_no_longer_says_settle_is_not_compiled` | yes |
| C38 | `source_events.rs` `a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record` | yes - no record, no `$settle`, the rest's observations equal with and without the tap |
| C39 | `source_events.rs` `settle_runs_once_per_time_point_across_a_jump` | yes - the 2.5 us jump asserted a time point, one record per step |
| C40 | `source_events.rs` `an_event_function_calling_another_fires_under_the_outer_guard` | deferred - B5 (T29) |
| C41 | `source_events.rs` `a_probing_settle_reaction_reads_the_time_point_solution` | yes - exact equality with the accepted solution, five records for five steps |
| C42 | `source_events.rs` `settle_in_a_design_with_nothing_analog_runs_at_the_operating_point_and_the_stop` | yes |
| C43 | `source_events.rs` `an_event_not_in_the_head_reads_whether_it_fired` | yes |
| C44 | `digital.rs` `a_settle_event_is_built_checked_and_printed`; `frozen.rs` `a_settle_event_is_refused_by_name`; `digital_shape.rs` `a_settle_event_reads_the_global_settle_pin`; `reaction.rs` `a_settle_event_function_fires_on_the_settle_pin_and_its_guard`; `types_events.rs` `settle_is_an_event_in_every_event_position`; `elab_digital.rs` `settle_lowers_to_the_settle_event_source` | yes |
| C45 | `elab_digital.rs` `an_event_read_as_a_bool_in_a_body_lowers_to_fired`; `digital_shape.rs` `an_event_function_reading_another_event_runs_after_it`; `elab_digital.rs` `an_event_function_call_lowers_to_its_body_event` | yes - the plan test builds the elaborator's order (F2 fixed, d0a89fee); the lowering test holds; the third proof deferred - B5 (T28) |
| C46 | `tests/book/mixed.rs` `the_comparator_without_hysteresis_reads_its_event_as_a_bool`; `book_inventory.rs` `every_block_of_the_book_has_one_row`, `refused_rows_and_gaps_section_11_agree`; `the_phase_detector_tells_its_two_edges_apart`, `a_function_has_the_class_its_body_gives` | yes for the B3/B4 rows (both comparator polarities); the `Pfd3` and `handshake` proofs deferred - B5 (T26, T30) |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 - C16's source test proves nothing of S4: no event time lies between the settle commit and the next time point | own example at HEAD: `BetweenBench` first time point 37 037 037 fs (37 ns), reads at 0.37, 1.11, 1.85, 2.59 us, all 2 kOhm; the test comment assumes the first time point at 1 us; it would pass if S4 were false | make the model's first time point fall after an edge (or drive the edge before it) and assert at least one `read` with `t < first` reading 1 kOhm, plus one after it reading 2 kOhm |  closed by decision, no commit: the source-level version is impossible, the engine's first step is 1/10 of the distance to the first pending edge, so no edge can precede the first time point and edges fall in long gaps only after unchanged commits; the stub test `an_event_time_before_the_next_time_point_reads_the_old_value` is the sole proof of S4; C16 re-pointed, source test renamed (1f384eca) |
| F2 - the plan test of D8 cannot fail on order | `digital_shape.rs` `Guarding::body` declares `en.changed()` (event 0) before the guarded `d.changed()` (event 1) that reads it, so `position(event:0) < position(event:1)` is declaration order; the elaborator appends read events after the head's (`event_reads.rs`), the reverse case | build the fixture as the elaborator does (the guarded event first, the read event appended) and assert the read event's function runs first through whatever orders it (shape order or the engine's dependency order) |  d0a89fee |
| F3 - `GAPS.md` still lists L51 as refused, which is false once B1 to B4 merge | the L51 row's two examples (`if fref.rise()` in a reaction, `on d.changed() && en && !en.changed()`) run at HEAD (C1, C5); §9 "IV §12–§13" still names "an event as a `bool`"; digital#6, its last user, is re-pointed to L5 | rewrite L51 to what is still missing (only `Pfd3`, which waits on L5 `let mut`, UE-01 AC 3) or close it now and track UE-01 AC 3 under B5; drop "an event as a `bool`" from the §9 row; `docs.rs` asserts it |  0208118e |
| F4 - the §13 error table contradicts the new D2 Resolution | `docs/spec/part_iv_behavior.md` `E0441` row: "an `Event` committed or carried (§13 rule 8)", unqualified; the Resolution says a commit onto a `bool` carrier is the rule 2 read and `E0441` is "committed where a type other than `bool` is expected" | qualify the row ("an `Event` committed where a type other than `bool` is expected, or carried") with a `spec_text.rs` assertion |  fee8922f |
| F5 - the new test files break the tasks' own "no free test helper" rule and leave fields undocumented | `source_events.rs`: free `ticks`, `known_real`, `same_instants`, `real`, `flag`, `flags` (T11: "test fixtures are methods of a named scenario type; no free test helper"); `source_limexp.rs`: free `compiled_backends` (T18: "no free test helper"); undocumented fields `Tape.0`, `Run`'s three, `Scenario`'s two, `CompiledDiode.handle`, `Call.writes`/`partials`; stale comment in `tests/book/welcome.rs` ("the sentence's prose is left for T21", done by T22) | move the helpers onto `Run`/`Scenario`/a record reader type and `SeriesModel`; doc the fields; drop the stale comment |  3ed72693 |

## Final run (HEAD 3ed72693)

`git rebase origin/main`: up to date. Root `cargo test --no-fail-fast` (PIPERINE_NGSPICE unset, every .rs touched): 3097 passed, 0 failed. `source_limexp`, `parity_tran`, `source_events` with ngspice present (no SKIP): 5 + 10 + 20 passed. `check_proofs.py`: 60 pass, 10 fail, all ten the deferred B5 rows (C3, C23 (2), C24, C25, C26, C36 second, C40, C45 third, C46 B5 proofs). Verdict PASS for B1 to B4; B5 rows stay deferred.

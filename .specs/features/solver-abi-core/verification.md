# Solver ABI Core verification

**Verdict**: FAIL - F1-F13 closed and re-judged; three new findings (N1 medium, N2 medium, N3 low) stay open for the user
**Round**: 2
**Proofs**: `proofs.md` at 1fedaa8d (51 pass); round 2 judged at a1512672 (docs-only on top). Round 1: 60a542da (41 pass)
**Verifier**: independent sub-agent (author != verifier), rounds 1 and 2
**Range**: round 1 82151641..daa75d37; round 2 157c700d..a1512672 (fixes 3967a634..f1414d16, closure 19073264)

Probes were throwaway test targets copied into `tests/`, run, and removed (sources kept in the
session scratchpad `verify-abi-core/zz_probe_protocol.rs` and `zz_probe_abi.rs`). Nothing here is
committed but this file.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `contract.rs` `struct_layouts_match_the_abi` | yes - every struct's size, align and offsets against a hand table (64-bit) |
| C2 | `contract.rs` `list_reads_by_the_producers_stride` | yes - for a larger stride, as the AC states; the smaller-stride case is unguarded (F8) |
| C3 | `contract.rs` `crate_declares_no_dependency` | yes |
| C4 | `layouts.rs` `shipped_layouts_have_their_stated_fields` | yes |
| C5 | `layouts.rs` `layout_equality_ignores_names` | yes - each of leaf, offset, count, known, size, align, field count flipped alone |
| C6 | `layouts.rs` `a_valueless_field_with_value_bytes_is_not_canonical` | yes |
| C7 | `shape.rs` `resistor_shape_reads_back_through_raw_pointers` | yes |
| C8 | `validate.rs` `every_shape_local_rule_is_enforced` | yes - one planted defect per rule, rule, entry and origin asserted; the table is forced to cover `Rule::ALL`. Unlisted §3 invariants pass (F12) |
| C9 | `thunks.rs` `module_pointers_call_the_device` | yes |
| C10 | `thunks.rs` `destroy_drops_the_device_and_releases_the_shape` | yes - drop count and `Weak::strong_count` 0 |
| C11 | `thunks.rs` `a_panic_is_a_fault_with_its_message` | yes - an unwind across `extern "C"` would abort the test binary |
| C12 | `thunks.rs` `a_fault_carries_its_code_and_a_cut_message` | yes - code 77, cut inside `é` back to 247 |
| C13 | `device.rs` (unit) `typed_access_that_does_not_fit_is_none` | yes |
| C14 | `thunks.rs` `commit_reaches_the_host_once` | yes |
| C15 | `thunks.rs` `finish_is_a_status` | yes |
| C16 | `caller.rs` `handle_builds_the_frames_and_returns_results` | yes - frame mismatch refused before the call |
| C17 | `verify.rs` `exact_partials_verify` | yes |
| C18 | `verify.rs` `a_wrong_partial_is_reported_with_its_index` | yes - index, declared and finite asserted; a NaN partial is not caught (F7) |
| C19 | `verify.rs` `a_point_where_the_limit_is_active_is_skipped` | yes |
| C20 | `examples.rs` `worked_examples_produce_their_stated_values` | yes - hand-computed values to 1e-12 and exact partials |
| C21 | `examples.rs` `worked_examples_pass_validation` | yes |
| C22 | `crate_boundaries.rs` `sub_solvers_depend_on_the_abi_crate_only` | yes |
| C23 | `crate_boundaries.rs` `abi_crate_is_a_leaf` | yes |
| C24 | `workspace_shape.rs` `instruction_files_and_state_name_the_abi_crate` | yes |
| C25 | `protocol_seam.rs` `the_protocol_module_names_no_sub_solver_item` | yes, against the design; the scan exempts `piperine_analog_solver::error::`, an item of the analog crate that spec AC 1 literally forbids (design allows it; accepted, not a finding) |
| C26 | `protocol_transient.rs` `no_landing_means_one_attempt_to_the_proposed_time` | yes, on tick-aligned times only (every probe time is a multiple of 0.5 s); off-grid steps break the protocol (F2) |
| C27 | `protocol_transient.rs` `a_landing_is_solved_on_the_left_value_then_its_instant_runs` | round 2: superseded by C44; the preview test is gone - yes |
| C28 | `protocol_transient.rs` `a_rejected_attempt_restores_both_sides_and_replays` | yes (with `an_earlier_landing_restores_and_replays` for the shortened case) |
| C29 | `protocol_transient.rs` `acceptance_fires_crossings_runs_the_instant_and_asks_for_restart` | yes |
| C30 | `protocol_transient.rs` `a_host_param_write_asks_for_the_small_restart` (+ `protocol_host.rs` `a_waiter_param_write_asks_for_the_small_restart`) | round 2: yes - goes through `Protocol::set` between runs, asserts the instant at now, `Deliver(.., IntegratorSmall)` / `Integrator` per case and the next attempt going forward (round 1: no, F5) |
| C31 | `protocol_host.rs` `an_irreversible_landing_runs_once_after_acceptance_and_does_not_settle` | yes |
| C32 | `protocol_host.rs` `finish_ends_after_its_instant_and_is_dropped_on_an_earlier_landing` | yes for the transient; a `FINISH` at the operating point is dropped and no check names it (F3) |
| C33 | `protocol_transient.rs` `no_unknowns_means_no_attempt` | yes |
| C34 | `protocol_operating_point.rs` `the_operating_point_follows_the_call_sequence` | yes |
| C35 | `protocol_operating_point.rs` `rounds_beyond_the_bound_are_e0506_naming_the_nets` | round 2: yes - asserts `nets still changing: top.cmp.out` from `DiscreteSide::net_name` (round 1: no, F10) |
| C40 | `verify.rs` `a_non_finite_partial_is_a_mismatch` | yes - NaN declared and NaN finite (x - h < 0) each a `Mismatch` with the index; my round-1 probes now give `Mismatch` |
| C41 | `contract.rs` `a_list_whose_stride_is_below_its_element_is_refused` | yes - short stride, misaligned stride, misaligned pointer each refused, `iter()` empty; `UnreadableList` is a C8 row. Silent empty iteration elsewhere: N3 |
| C42 | `contract.rs` `raw_views_have_no_public_pointer_field` | yes for its literal claim (no `pub` pointer field, four unsafe constructors); SAC-21's "no safe accessor can reach UB" is not held - N2 |
| C43 | `contract.rs` `no_unsafe_outside_raw_thunk_and_caller` | yes - scans every other `src/` file, self-tests the scanner |
| C44 | `protocol_transient.rs` `a_landing_is_solved_on_the_left_value_then_its_instant_runs` | yes - exact sequence: `Attempt{1.0, exempt}`, `Accept`, `RunInstant` with the 1 s samples, then `Deliver(.., Integrator)`; no delivery before the solve |
| C45 | `protocol_transient.rs` `an_off_grid_step_lands_on_the_tick_grid` | yes for proposals and for a crossing rounding to an earlier tick; a crossing within half a tick of the target loops - N1 |
| C46 | `protocol_operating_point.rs` `finish_at_the_operating_point_ends_the_run` | yes - `Finish(0.0)` and nothing advanced, proposed or attempted after |
| C47 | `protocol_transient.rs` `consecutive_rejections_beyond_the_cap_fail_loud` | yes - default 64 and a set cap, exact attempt count, reset on acceptance. `LandedEarlier` is not counted - N1 |
| C48 | `protocol_host.rs` `irreversible_reactions_are_bounded_by_the_current_instant` | yes - `Horizon{stop, crossing: tick_c}` with unknowns, `crossing = stop` without |
| C49 | `protocol_sink.rs` `traces_reach_the_sink_in_time_order` | yes - a rejected attempt's trace waits for its time; traces precede the instant at one time |
| C36 | `protocol_sink.rs` `observations_reach_the_sink_after_acceptance_in_instant_order` | yes |
| C37 | `protocol_sink.rs` `an_undone_instant_delivers_nothing` | yes |
| C38 | `protocol_host.rs` `a_host_waiter_runs_once_and_its_writes_are_due_next_microstep` | yes |
| C39 | `protocol_host.rs` `a_write_is_committed_only_after_the_shadow_accepts` | yes |

## Judgement of the brief's points

**`unsafe` (raw.rs, thunk.rs, caller.rs only - confirmed by grep; no test enforces it, F11).**
`pod_ref`/`pod_mut` check length and alignment before the cast; `pod_bytes` is sound for `Pod`.
Every thunk (`create`, `destroy`, `eval`, `react`) runs the device inside `catch_unwind`; the
caller's `trampoline` catches a panic of the solver's closure and resumes it only after the
device returned - no unwind crosses `extern "C"`. `cut`/`cut_mut` take lengths from the
instance's own shape and `Handle::eval/react` check every buffer is at least that long before
the frame is built; `&[&[u8]]` and `&mut [&mut [u8]]` make aliasing between reads and writes
impossible from safe code. `ShapeRef = Arc<ShapeBuf>`: the raw `Shape` lives in the `Arc`
allocation, every list in a `Box`/`Vec` heap block refreshed after each add; `Created.shape` and
`Handle::shape()` stay valid while the instance holds its `Arc`. `Handle` is `Send`, not `Sync`;
`react` takes `&mut self`. Two SAFETY arguments do not hold as written: `List::get`'s
("the producer compiled at least the fields of `T`") fails for an older-minor producer (F8), and
the safe accessors over public raw fields let safe code reach UB (F9). A panic payload whose own
`Drop` panics aborts (Rust >= 1.81) rather than unwinding - acceptable.

**`validate` (15 rules).** Each rule rejects its planted defect (C8). Extra planted shapes
nobody named: a zeroed entry, `u32::MAX` indices, an unknown statement flag - all refused. Four
§3 invariants outside the list pass (F12).

**FD verifier.** Catches a 1 % error at the right index; no false alarm on the six examples, on
`exp`-shaped diode partials, or on exact-zero partials (the `4ε·max|f|/2h` floor works; the
step uses `xp − xm`, not `2h`). It passes NaN (F7).

**Owner protocol.** Checkpoint = (discrete mark, analog mark, sink length); a rejected or
shortened attempt restores all three; the pending restart is taken only at acceptance, so a
rejection cannot lose it; discrete observations of an undone instant never reach the sink
(I-no-trace, C37). Sub-checkpoint `c_s` per instant is delegated to `DiscreteSide::advance`
(documented on `Advance::Landing`); it cannot be proven on stubs and is carried to
`solver-abi-digital`. Landing, irreversible instant (runs once, only after acceptance, never in
`advance`), host waiter as an irreversible reaction (R7, called from `run_whole_instant` only),
`FINISH` with its `final` mark dropped on an earlier landing, and `Shadow` handling
(`Accept` commits, `Refused` and in-transient `Reshape` commit nothing, outside a transient
`Reshape` is reported) are as abi.md §5/P6 say. Determinism: plain `Vec`s, waiters in
registration order, no hash map, no clock.

**The user's order (2026-11-15): solve at t on the left value, then run the instant, reactions
read the solution at t.** Not implemented - F1.

**The implementer's choices.**

| Choice | Judgement |
| --- | --- |
| Every accepted analog step is a time point and settles | accepted - amendment 2 lets a simulator accept other solutions, it does not oblige it; settling at each accepted step is what `on settle` means in the language |
| An irreversible landing does not settle | accepted - R6 verbatim |
| With no unknown, settle at the stop time (and at R4 landings) only | accepted - no analog solution exists elsewhere |
| Traces after every attempt | accepted - spec AC 12 and A5; ordering note F13 |
| `run_instant` limit = stop tick | finding F6 |
| `transient()` does not run the operating point | accepted - §9: an analysis from a transient's last instant keeps everything; a guard against a never-started discrete side would be cheap but is not required |
| No cap on consecutive rejected attempts | finding F4 |
| `E0506` names slots, not nets | finding F10 |

**Seam additions.** `DiscreteSide::wait`, `Outcome::host_events` (suspend, call waiters, resume
`run_instant` at the same tick), `HostCx::set -> SetOutcome`, `Protocol::buffered` are coherent
and minimal. Gap: `Advance` has no channel for a host-param change, which is why a `set` made
between runs has no restart path (F5).

## Round 2

Every fix re-read against its finding and the round-1 probes re-run, adapted to the new API
(`List::from_raw_parts`, `Horizon`, `deliver(boundary, restart)`, `accept()`); probe sources in
the session scratchpad `verify-abi-core/zz_probe_protocol_r2.rs` and `zz_probe_abi_r2.rs`.

| Finding | Re-judged |
| --- | --- |
| F1 | closed - the attempt runs on the current copy (left value), exempt at a landing; accept, fire crossings, run the instant with the samples at t, then `deliver(boundary, restart)`. `preview_boundary` is gone from the seam. This is the user's "solve, then react" |
| F2 | closed - proposals floored to a tick, at least one past `now`; probe 1.6 s / 0.3 s gives `Attempt 1.0, Run(1), Attempt 2.0, Run(2), ...`. Crossing rounding: N1 |
| F3 | closed - probe `op=Finish(0.0) tran=Finish(0.0)` |
| F4 | closed - 64 rejections then a transient error naming t = 0; the `Mark` doc now keeps the step reduction across `restore` |
| F5 | closed - `set` between runs runs the instant at now and delivers `IntegratorSmall`; the next attempt goes to 2.0; a landing at the current tick now fails loud (`a landing at tick 0 does not advance`) |
| F6 | closed - `Horizon { stop, crossing }`, crossing = the current tick while unknowns exist |
| F7 | closed - both probes now `Mismatch` |
| F8 | closed - `is_readable` gates `get`; `validate` refuses `UnreadableList` before walking anything |
| F9 | partly - pointer fields are private and foreign views need `unsafe from_raw_parts`, `Handle::create` takes only a sound `Module`; but lifetime-less `Copy` views still let safe code outlive their owner - N2 |
| F10 | closed - `net_name(slot)` |
| F11 | closed - scan test C43 |
| F12 | closed - all four round-1 shapes now refused with their rule (`EventNotEmpty`, `ContinuousNotF64`, `CheckNotBool`, `QuietChangeRead`) |
| F13 | closed - one stream sorted by time, traces first at one time, rejected traces held to their time |

Soundness spot-check of the new code: `Module` accessors deref `prototype` under the invariant
both constructors keep; `check_lists` runs before any nested list is walked and the `check`
pin's fields are read only after it; `Delivery::flush` sorts stably, so the discrete side's
order inside one time is kept. No new `unsafe` outside the three files. Determinism unchanged.

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (high). The protocol solves the landing on the right value: `transient.rs` calls `preview_boundary` and `deliver`s microstep 0 of `t_c` before `attempt(t_c)`, so the solve at t reads the instant's commits. The user decided (2026-11-15) the ABI's order: solve at t on the left value, then run the instant; reactions read the solution at t | `transient_loop` steps 3-4; C27's test asserts `Preview(2), Deliver, Attempt{exempt:true}`; design Risks row "preview_boundary ... before the attempt"; spec AC 3 and the assumption row "follows Q7" | drop the preview from the loop (and `preview_boundary` from the seam): attempt to `t_c` on the current copy, accept, run the instant, deliver its whole boundary (microstep 0 included) and ask the next acceptance for the restart; decide what `exempt` means then. AC 3 is approved text: append a new requirement for the order and a check, do not edit AC 3 || 67a89e9f |
| F2 (high). `t_c` is never put on the tick grid: an off-grid step runs the instant `round(t_c / spt)`, which is up to half a tick in the future (run early, with the samples of an earlier time) or in the past (run late, attributed to a later accepted time) | probe `probe_off_grid_step_runs_a_later_instant_early`: spt 1 s, propose 1.6 → `Advance(2), Attempt{to:1.6}, RunInstant{at:2}`; every protocol test uses times that are multiples of 0.5 s | snap `t_c` to a tick before `advance` (floor, at least one tick past `now`), or run an instant only when `t_c == time(tick_c)`; add a check with an off-grid proposal || d15edccf |
| F3 (medium). A `FINISH` at the operating point is dropped: `operating_point` ignores `outcome.finish` / `settled.finish` and returns `Ok(())`; a following `transient` runs to the stop time | probe `probe_finish_at_operating_point_is_dropped`: `op=Ok(()) tran=Ok(StopTime)` | return the finish from `operating_point` (e.g. `Result<Ended>` or a flag) and end the analysis; add a check || 8a513d23 |
| F4 (medium). No bound on consecutive rejections, and `AnalogSide::Mark` is documented as "solution, histories and step control": an adapter that restores its step control on `restore` re-proposes the rejected step and the loop never ends | probe `probe_unbounded_rejections`: 20 001 attempts, `Ok(StopTime)`; `sides.rs` `AnalogSide::Mark` doc; `transient_loop` `Rejected => restore; continue` | state in the seam that the step reduction of a rejection survives `restore` (or pass the rejection to `propose`), and fail loud (a typed error at the time) after a bounded run of rejections or below a minimum step || b15b3aa6 |
| F5 (medium). A host write through `Protocol::set` between runs has no restart path: its commit at `tick(now)` is a landing at `now`, so the next transient attempts a zero-length step and accepts it with `Restart::None`; AC 6's small restart only exists for writes made by a waiter inside an instant | probe `probe_host_set_between_runs_lands_on_now`: after `set` at 1 s, `Attempt{to:1.0, exempt:true}, Accept(None)`; the `t_c <= now` guard runs before `advance` only | outside an instant, run the instant at `now` after the commit (`run_whole_instant`), deliver its boundary and set `IntegratorSmall`; guard `t_c > now` after `advance`; give C30 a test through `Protocol::set` || 4ee84932 |
| F6 (medium). `run_instant`'s `limit` is the stop tick, but R5 bounds `HORIZON` by the end of the tentative analog step when a crossing can reach the reaction; at step 5 the next step is not proposed yet, so an `IRREVERSIBLE` reaction (an emulator, the host) reachable from a `CROSS` watch can run its clock past a crossing that later fires | `transient_loop` passes `stop_tick`; C29/C38 assert `limit: 4` (the stop) | pass the discrete side a bound it can honour (the current tick when a crossing can reach the reaction, or propose the next step first and pass its tick), and state the rule on `run_instant` || 4e088f2f |
| F7 (medium). `verify_partials` passes NaN: a NaN declared partial or a NaN finite difference makes `>` false, so the verdict is `Exact` | probes: `NanJac` (jac = NaN) → `Ok(Exact)`; `ln(x)` with a 5× wrong partial at x = 1e-7 (x − h < 0, finite NaN) → `Ok(Exact)`, at x = 2 → `Mismatch` | treat a non-finite `declared` or `finite` as a mismatch (or its own verdict); add a check || 3967a634 |
| F8 (medium). `List::get` does not check `stride >= size_of::<T>()` or alignment; its SAFETY argument ("the producer compiled at least the fields of T") is false for an older-minor producer, which abi.md §12 "Versioning" admits (a minor appends fields), so a newer consumer reads past each element | probe: `List<[u8;8]> { len: 4, stride: 4 }` over 16 bytes → `get(3)` is `Some` and reads bytes 12..20 | refuse (or read by prefix) a list whose stride is below `size_of::<T>()` or not a multiple of `align_of::<T>()`, or whose `ptr` is misaligned; check it where a foreign library is first read || 1a4935c3 |
| F9 (low). Safe functions dereference raw pointers from public fields: `Str::as_bytes/as_str`, `Bytes::as_slice`, `List::get`, `validate(&Shape)`, and `Handle::create(&'static Module)` (a `Module` from `module::<D>()` with `prototype` replaced makes the `create` thunk dereference it). abi.md §7's own rule - "a safe call over raw pointers the caller supplies would be unsound" - was applied to `Handle::eval/react` only | `raw.rs` `pub` fields with safe accessors; `caller.rs` `Handle::create` | make `Handle::create` an `unsafe fn` (or take a wrapper whose construction is `unsafe`, as a loader's `dlopen` is), and give the raw views an `unsafe` constructor with the trust boundary documented || 68d42414 |
| F10 (low). `E0506` names analog-side boundary slots, not nets, so the user cannot read which nets keep changing; C35 asserts the slot text | `operating_point.rs` `too_many_rounds`; C35 `"slots): 7"` | carry the net (id or path) with each `Boundary` or let the adapter map slots to net names; assert a net in C35 || 7693b027 |
| F11 (low). The design's mitigation "no `unsafe` outside raw/thunk/caller (a scan test in `tests/contract.rs`)" is not implemented; only `lib.rs` prose states it | `grep unsafe crates/piperine-abi/tests/contract.rs`: no scan | add the scan test || 46629d5b |
| F12 (low). `validate` accepts four §3 invariants broken: an `EVENT` pin with a non-empty layout and a 2-byte statement on it (C4: `len = 0`); a `CONTINUOUS` pin whose layout is not one `F64`; `HAS_CHECK` naming a continuous pin (the check is a discrete `BOOL`); a reaction reading `PREVIOUS` of a `QUIET` pin (P7) | probe `probe_validate_gaps`: all four `Ok(())` | add them as rules with planted defects (new requirement and check) || b02ee1a7 |
| F13 (low). The sink receives the traces of the attempt to `t_c` before the observations of the instants in `(t, t_c)`, so one stream is not in time order | probe `probe_trace_order_vs_instants`: `[("trace", 1.0), ("instant", 0.5)]` | state on `Sink` that traces are delivered per attempt ahead of the buffered observations (or stamp a sequence the host can sort by) || f1414d16 |
| N1 (medium, round 2). A located crossing whose nearest tick is the step's own target (within half a tick before it), or a `LandedEarlier` at or past the target, is clamped back to `tick_c` and the same step is attempted again; `LandedEarlier` is not counted toward `max_rejections`, so an analog side that relocates the same crossing loops with no bound | probe `r2_crossing_within_half_a_tick_of_the_target_loops`: spt 0.5 s, target 1.0 s, `LandedEarlier(0.9)` → 5 001 attempts all `to: 1.0`, ended only when the script ran out; `LandedEarlier(7.0)` → 5 002 attempts | when the forced tick equals `tick_c`, accept there with the crossing fired at `(tick_c, 0)` (the seam must say how), or count these retries toward the cap and fail loud; add a check | |
| N2 (medium, round 2). SAC-21 ("no safe accessor can reach undefined behaviour") does not hold: `List`, `Str`, `Bytes`, `Shape` are `Copy` with no lifetime and `List::from_slice(&[T])` is safe for any borrow, so safe code builds a list that outlives its slice (or copies a `Shape` out of a `ShapeRef` and drops it) and `get` reads freed memory | probe `probe_from_slice_outlives_its_slice_in_safe_code`: a list from a dropped `Vec` reads back the next allocation's `0xdeadbeef`, all safe code | make `from_slice` take `&'static [T]` (keep a crate-private variant for `ShapeBuf`) and give the views a lifetime (`PhantomData<&'a T>`, `repr(C)` unchanged), or narrow SAC-21 and mark the dereferencing accessors' contract explicitly | |
| N3 (low, round 2). `List::iter` skips an unreadable list silently (`filter_map(get)`), so code that iterates without `validate` sees an empty list: the `create` thunk reads `values.iter()` from the foreign caller, so an older host's `List<Value>` drops every param value and the device is created on its defaults with status `OK`; `ShapeBuf` copies an unreadable layout as field-less | `thunk.rs` `create` (`for v in values.iter()`), `raw.rs` `iter`; `shape.rs` `layout` | fail loud: `create` returns `FAULT` when `!values.is_readable()`; `ShapeBuf::layout` refuses an unreadable field list | |

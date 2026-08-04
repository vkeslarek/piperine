# codegen-simplification Validation

**Date**: 2026-08-03
**Spec**: `.specs/features/codegen-simplification/spec.md`
**Diff range**: `eadd5a2^..HEAD` (15 commits, T1–T14 + one Cargo.lock follow-up)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status  | Notes |
|------|---------|-------|
| T1   | ✅ Done | Dependency edge + baseline (267 binaries, 1675 passed, 4 ignored) recorded in commit `eadd5a2` |
| T2   | ✅ Done | `SlottedOperator` wraps `signal::ops::Operator` |
| T3   | ✅ Done | `EventSystem` delegates fire decision to `signal::events::EventDetector` |
| T4   | ✅ Done | `LIMIT_FNS` registered, analog JIT only |
| T5   | ✅ Done | `$limit` emits a call; `emit_pnjlim`/`emit_fetlim`/`emit_limvds`/`emit_min`/`emit_max` deleted |
| T6   | ✅ Done | Device `Limiter` wraps `signal::limit::Limiter`; SPEC_DEVIATION (`advance_precomputed`) verified as documented, non-duplicative |
| T7   | ✅ Done | `Table::points`/`segment_slope` public accessors |
| T8   | ✅ Done | Codegen's `table` lowering builds from validated `signal::Table` |
| T9   | ✅ Done | `signal::logic::{Logic, EdgeDirection}`, IEEE 1364, full 4×4 table tested |
| T10  | ✅ Done | Both digital.rs call sites (`:306`, `:345` in current tree) delegate to `fires` |
| T11  | ✅ Done | `EdgeWait::observe` delegates; `EdgeDirection::matches` deleted; `X→1`/`Z→1`/`X→0`/`Z→0` tests added |
| T12  | ✅ Done | `tests/delegation_guard.rs`, 5-family registry, proven able to fail (re-verified 1 of 5 myself, see Sensor) |
| T13  | ✅ Done | Four `cross_check_*.rs` deleted; `host_capability_parity.rs` untouched; dev-deps dropped with a documented reason in `Cargo.toml` |
| T14  | ⚠️ Partial (by design) | Shrinkage measured; 3/5 file caps and the net-deletion floor missed — reported, not silently absorbed, matches AC 28. Independently re-measured and confirmed exact (see below) |

All tasks done; T14's shortfall is the pre-accepted, user-renegotiated P2 miss — not a defect.

---

## Spec-Anchored Acceptance Criteria

| # | Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|---|
| 1 | `AnalogInstance` builds `signal::ops::Operator`; no local `Operator` enum | No `enum Operator` in codegen | `crates/piperine-codegen/src/device/analog/operators.rs:6,11-14` (`SlottedOperator{slot,op:Operator}`, `Operator` imported from `piperine_signal::ops`); `tests/delegation_guard.rs:41-45` witness | ✅ PASS |
| 2 | Accepted-timestep slot mapping carried by codegen | Slot alongside signal value | `device/analog/instance.rs:1277-1279` (`op.slot()`, `self.state[slot]=op.accept(...)`) | ✅ PASS |
| 3 | `idt` `ic` seeding bit-for-bit at construction | Integral = today's value | `device/analog/instance.rs:222-228,236-240` — `Operator::integrate(value(ic)?,...)`, then `state[*slot] = *value` pattern-matched on `Operator::Integrate{value,..}`, logic unchanged from pre-delegation, proven by full-suite zero-numeric-change gate (below) | ✅ PASS |
| 4 | Rejected timestep mutates no operator state | Invariant preserved | `device/element.rs:712-721` — `accept_timestep` is the solver's post-accept hook only (called nowhere on a load/residual path); `grep` confirms | ✅ PASS |
| 5 | Fire decision from `signal::events::EventDetector`; no local `EventDetector` | No `struct EventDetector` in codegen | `device/analog/events.rs:9,32` (imports `EventDetector`, `EventSystem.detectors: Vec<EventDetector>`); `tests/delegation_guard.rs:46-50` witness | ✅ PASS |
| 6 | `cross` doesn't fire on first sample; `above`>0 fires on first sample | Asymmetry preserved exactly | `piperine-signal/src/events.rs:247` (`assert!(!d.fired(Cross(Rising),...))`), `:257` (`assert!(d.fired(Above, 1.0,...))`) | ✅ PASS |
| 7 | `CompiledTrigger::Initial` has no signal counterpart | Short-circuited, never mapped | `device/analog/events.rs:105-112` — `Initial => None` arm, `continue`s before calling `detector.fired` | ✅ PASS |
| 8 | `EventSystem` keeps timer table + `@initial`/fired dispatch | Unchanged ownership | `device/analog/events.rs:33,73-77` (`periods` field, `next_breakpoints`), `:122-150` (`fire_initial_events`, `apply_event_actions`) | ✅ PASS |
| 9 | `pnjlim`/`fetlim`/`limvds` registered as JIT imports via `MATH_FNS`-shaped mechanism | Same `jit_builder.symbol()`/`declare_function` mechanism | `kernel/analog/limits.rs:60-91` (`LimitFn`, `LIMIT_FNS`, `mod wrappers`); `kernel/analog/compile.rs:154,173` (import loop) | ✅ PASS |
| 10 | `$limit` emits a call; no `emit_pnjlim`/`emit_fetlim`/`emit_limvds` | Call, not inline IR | `emit/analog_expr.rs:321-326` (`emit_limit_call` dispatch), `:336-341`; `grep -c "fn emit_pnjlim"` = 0 | ✅ PASS |
| 11 | Repeated `$limit` still CSE's to one call under `CseKey::Limit(slot)` | One call per slot | `emit/analog_expr.rs:311-314`; test `crates/piperine-codegen/tests/limiters.rs:162-195` — **but see Sensor Mutation 3: this test does not actually distinguish "one call" from "two calls computing the same result"** | ⚠️ Spec-precision gap (test present, doesn't fully discriminate — see Sensor) |
| 12 | Every `$limit` emission site delegates (param-Jacobian, `fn`-inlined) | Same call at every site | `crates/piperine-codegen/tests/limiters.rs:197-229` (`limit_inside_an_inlined_fn_still_emits_the_call`) — asserts numeric equality with a direct call | ✅ PASS |
| 13 | `$limit` stays transparent to differentiation; `diff.rs` unchanged | No hand-written Jacobian | `resolve/diff.rs:264-265` (unchanged marker comment); `git diff eadd5a2^..HEAD -- .../diff.rs` is **empty** — confirmed byte-identical | ✅ PASS |
| 14 | `active`/`active_slots`/`seeds` from `signal::limit::Limiter` | No local `Limiter` struct | `device/analog/limits.rs:28-30` (`struct Limiter{inner: piperine_signal::limit::Limiter}`); SPEC_DEVIATION's `advance_precomputed` verified non-duplicative (rules 2+4 shared with `update`, formula-free) — `piperine-signal/src/limit.rs:226-250` | ✅ PASS |
| 15 | Unknown `$limit` kind fails loud, unchanged error | `CodegenError::unsupported` naming the kind | `emit/analog_expr.rs:325` (`other => Err(CodegenError::unsupported(...))`) — verbatim arm | ✅ PASS |
| 16 | `table()` builds `signal::Table` first, `If`-chain from validated segments | One definition of validation+geometry | `resolve/pom/analog_ops.rs:187-198` (`Table::with_mode`, `.points()`) | ✅ PASS |
| 17 | Validation failures raised from `Table`'s error type, unchanged user text | Same message text | `resolve/pom/analog_ops.rs:153-166` (`err_from` maps `TableError` variants to the exact pre-delegation strings); tests `crates/piperine-codegen/tests/table_op.rs:18-38` assert on message substrings | ✅ PASS (see note on check-order below) |
| 18 | `x` outside `[xs[0],xs[n-1]]` clamps, doesn't extrapolate | Flat clamp both ends | `resolve/pom/analog_ops.rs:219,234-242` (fold + low-clamp `If`) — **Sensor Mutation 4 shows no test in the codegen/host-parity suite actually exercises the *low*-end clamp of the lowered expression** | ⚠️ Spec-precision gap (high clamp is tested via `host_capability_parity.rs:261`; low clamp is not) |
| 19 | Derivative = segment slope, `diff.rs` unmodified | No hand-written Jacobian | Same `diff.rs` zero-diff proof as AC 13 (single file, both ACs) | ✅ PASS |
| 20 | Non-constant `xs`/`ys` fail loud via `const_array`, unchanged | Same loud failure | `resolve/pom/analog_ops.rs:130-139,173-178` (`const_array` unmodified in shape; still gates before any `signal::Table` is built) | ✅ PASS |
| 21 | `cargo test --workspace` passes, same count modulo 4 deleted, 0 warnings, 0 numeric diffs | Green gate | Full run: 276 test-result blocks, 1689 passed, **0 failed**, 4 ignored (same as T1 baseline); `cargo build --workspace` 0 warnings; `cargo clippy --workspace --all-targets` 0 warnings; `tests/ngspice_validation.rs` 33/33 | ✅ PASS |
| 22 | 5 delegation witnesses, mechanically checking origin | 5 witnesses total | `tests/delegation_guard.rs:39-85` (`FAMILIES`, 5 entries); `no_delegated_family_has_a_second_copy` asserts `FAMILIES.len()==5` | ✅ PASS |
| 23 | Witness fails if a local copy reappears | Source-level check, not a comment | Re-verified myself: injected `enum Operator` comment marker into `operators.rs`, `no_delegated_family_has_a_second_copy` FAILED naming `operators.rs:9` and the family — reverted | ✅ PASS |
| 24 | 4 `cross_check_*` deleted, `host_capability_parity.rs` untouched | Deletion + preservation | `git show 0ca0ce3 --stat` (4 files deleted); `git diff eadd5a2^..HEAD -- tests/host_capability_parity.rs` empty | ✅ PASS |
| 25 | `piperine-signal/tests/purity.rs` still passes | No new dependency | Full-suite run: `purity.rs` 2/2 pass; `Cargo.toml`'s `[dependencies]` table still empty | ✅ PASS |
| 26 | 5 file caps hold | See table | Independently re-measured with `wc -l` (below) — matches `shrinkage.md` exactly; 2/5 met, 3/5 missed | ⚠️ Spec-precision gap — reported as P2, pre-accepted (see below) |
| 27 | ≥300 net lines removed | 16670 baseline | Independently re-measured: 16670 → 16474 = **196 removed**, matches `shrinkage.md` exactly | ❌ Missed (pre-accepted renegotiation, not a defect — AC 28 governs) |
| 28 | Missed cap reported with number+reason, not moved elsewhere | Honest report | `shrinkage.md` gives per-file reasons (adapter/mapping code, not moved code); independently corroborated by reading `events.rs`/`limits.rs`/`analog_ops.rs` — the extra lines are genuinely load-bearing (timer table, `advance_precomputed` plumbing, `err_from` mapping), not relocated logic | ✅ PASS |
| 29 | `signal::logic` implements IEEE 1364, `X→1`/`Z→1`/`X→0`/`Z→0` named explicitly | Named transitions | `piperine-signal/src/logic.rs:68-69,86-87` (`assert!` lines name all four transitions explicitly) | ✅ PASS |
| 30 | Both digital.rs call sites delegate | No inline edge comparison | `device/digital.rs:306-308,345-347` (`edge.to_signal().fires(...)`); `grep` for the old inline forms (`prev != 1 && cur == 1`) returns nothing | ✅ PASS |
| 31 | `EdgeWait::observe` delegates; `EdgeDirection::matches` deleted | Deletion + delegation | `piperine-api/src/sequencer/wait.rs:13,53` (re-export + `self.direction.fires(...)`); `grep -c "fn matches"` in `wait.rs` = 0 | ✅ PASS |
| 32 | Host `wait(edge(net,Rising))` fires on `X→1` | New test asserting the fix | `wait.rs:88-93` (`rising_fires_on_x_to_one`, asserts `w.observe(&[LogicValue::One])==true`) | ✅ PASS |
| 33 | No prior sample ⇒ never fires, stays host-side | Arming rule preserved | `wait.rs:45-51` (`baseline: None => {set; false}`); test `wait.rs:70-74` | ✅ PASS |
| 34 | `Quad`→detector mapping is codegen's; `signal` stays over its own 4-state type | No `piperine-solver` dep in signal | `device/digital.rs:35-54` (`Quad::to_signal`, `EdgeKind::to_signal`); `piperine-signal/src/logic.rs` has zero solver imports; `purity.rs` passes | ✅ PASS |

**Status**: 31/34 ACs PASS outright; 3 flagged ⚠️ (AC 11, AC 18 are genuine test-discrimination gaps found by the sensor below; AC 26/27 are the pre-accepted, user-renegotiated P2 shortfall, not a defect per AC 28's own terms).

---

## Discrimination Sensor

All mutations applied directly to the real tree (no worktree available with headroom for a second checkout at the time), each **reverted immediately after observation** via `git checkout -- <file>`; `git status`/`git diff --stat` confirmed clean after every mutation and at the end of the run.

| # | File:line | Description | Killed? |
|---|---|---|---|
| 1 | `crates/piperine-signal/src/limit.rs:238` | Disabled the seed-preservation skip in `advance_precomputed` (`if false && seeded && v < old`) | ✅ Killed — `piperine-signal`'s own `advance_precomputed_preserves_the_seed_while_still_seeded` unit test fails with `left: [0.5] right: [0.7]`. (Full `ngspice_validation.rs` suite — 33/33 — did **not** catch this; the fixtures never drive a junction into the exact seed-preservation window. The kill is at the correct, intended layer per T6's own coverage design, but it means the *integration* suite alone would not have caught this class of regression.) |
| 2 | `crates/piperine-signal/src/logic.rs:42` | Narrowed `Rising` from `before != One && after == One` to `before == Zero && after == One` (reintroduces the pre-fix bug) | ✅ Killed at 3 layers: `piperine-signal`'s `rising_fires_only_into_one_from_a_non_one_state` (`Rising(X -> One)` assertion fails), `piperine-api`'s `rising_fires_on_x_to_one`/`rising_fires_on_z_to_one`, and `piperine-codegen`'s `an_out_of_range_watch_value_folds_to_x_and_still_fires_rising_into_one` |
| 3 | `crates/piperine-codegen/src/emit/analog_expr.rs:312` | Disabled the CSE cache-hit early return in `emit_analog_limit` (`if false { if let Some(&hit)=... }`) | ❌ **Survived** — `cargo test -p piperine-codegen --test limiters` still 7/7 green. `a_repeated_structurally_equal_limit_collapses_to_one_slot_not_two` checks `kernel.num_limits()==1` (a *compile-time slot-dedup* count from `collect_limits`, unrelated to this CSE cache) and a numeric-doubling check that is satisfied whether the call runs once or twice, since both calls read the same `vold` and compute the same value. **No assertion in the suite counts actual JIT `call` instructions**, so AC 11's literal claim ("exactly one call... not two calls per Newton iteration") is not mechanically proven, only its numeric *symptom* under CSE-working conditions. |
| 4 | `crates/piperine-codegen/src/resolve/pom/analog_ops.rs:240` | Replaced the low-end clamp (`real(ys[0])`) with the segment-0 extrapolation (`seg(0,&x)`) | ❌ **Survived** — `cargo test -p piperine-codegen --test table_op` (3/3 green, these only test loud-error paths) and `cargo test --test host_capability_parity table_nonlinearity` (1/1 green — the fixture drives `$abstime` from 0 upward against `XS` starting at/below 0, so `x < xs[0]` is never reached; only the high clamp is exercised, per the test's own `saw_high_clamp` assertion). **No test in the delegation's scope exercises the table's low-end clamp on the lowered PHDL expression.** |
| 5 | `crates/piperine-codegen/src/device/analog/operators.rs:6` | Injected a marker comment containing the literal text `enum Operator` | ✅ Killed — `tests/delegation_guard.rs::no_delegated_family_has_a_second_copy` fails, naming `operators.rs:9` and the `operators` family, exactly as designed |

**Sensor depth**: lightweight (5 mutations, one per the assigned suggested spread)
**Result**: 3/5 killed, **2/5 survived** — the surviving mutants are genuine, narrowly-scoped test-discrimination gaps (AC 11's "exactly one call" and AC 18's low-end clamp), not correctness defects in the shipped code. Both surviving mutants leave the delegated *implementation* correct (verified by direct reading of the source); they show the *test suite* doesn't fully discriminate two specific spec claims.

---

## Payload/Conjunction Rule Spot-Checks

- AC 3 (idt `ic` seeding, "bit-for-bit"): no dedicated unit test asserts a specific numeric value at this call site directly, but the construction logic (`instance.rs:222-240`) is textually near-identical to pre-delegation (only the wrapper type changed) and the full-suite zero-numeric-change gate (`ngspice_validation.rs` 33/33, `host_capability_parity.rs` 4/4) is the payload proof per T2's own stated coverage expectation ("zero numeric change against the T1 baseline is the primary expectation"). Acceptable — not a gap.
- AC 11/T5's CSE test (`limiters.rs:162-195`): asserts on the actual computed residual value (`out[0] == 2.0*single`), not just "no error was thrown" — a good payload assertion in general, but (per Sensor #3) it is a value-level proxy for a structural claim ("one call") that it cannot actually distinguish from "two calls, same value." Flagged above.
- AC 18/T8's table clamp: the codegen suite has no test asserting the specific clamped *value* at the low end; `host_capability_parity.rs` asserts on real Values (`(v_p - v_h).abs() < 1e-9`) but never samples `t < xs[0]`. Flagged above.
- AC 29 (`Logic`/`EdgeDirection::fires`): the 4×4 transition table is asserted exhaustively (`for &before in &ALL { for &after in &ALL { assert_eq!(...) } }`) plus explicit named assertions for `X→1`/`Z→1`/`X→0`/`Z→0` — a genuinely strong payload proof, best in the feature.
- AC 32 (host `X→1` fix): `wait.rs:88-93` asserts the actual boolean return value of `observe`, not just that it compiles or doesn't panic — good payload assertion.

---

## Code Quality

| Principle | Status |
|---|---|
| No features beyond what was asked | ✅ |
| No abstractions for single-use code | ✅ |
| No unnecessary "flexibility" added | ✅ |
| Only touched files required for the task | ✅ (T6's SPEC_DEVIATION is scoped and disclosed, not scope creep) |
| Didn't "improve" unrelated code | ✅ |
| Matches existing patterns/style | ✅ (`LIMIT_FNS` mirrors `MATH_FNS` exactly; `delegation_guard.rs` mirrors `suite_hygiene.rs`) |
| Spec-anchored outcome check (asserted values match spec) | ⚠️ Mostly — 2 gaps noted above (AC 11, AC 18) |
| Every test maps to a spec AC / edge case / Done-when | ✅ |
| Documented guidelines followed | CLAUDE.md ("zero warnings"), MD-31 (guard proven able to fail), MD-13 (data table + plain wrappers, no macro) — all followed |

---

## Gate Check

- **Gate command**: `cargo build --workspace && cargo test --workspace` (`CARGO_TARGET_DIR=/var/tmp/piperine-target` already configured project-wide for disk headroom); `cargo clippy --workspace --all-targets` additionally run and clean
- **Result**: 276 test-result blocks, **1689 passed, 0 failed**, 4 ignored (unchanged from baseline)
- **Test count before feature (T1 baseline, from commit `eadd5a2`'s message)**: 267 test binaries, 1675 passed, 4 ignored
- **Test count after feature**: 276 test-result blocks (includes the new `delegation_guard` binary), 1689 passed
- **Delta**: +14 passed net (−6 from the four deleted `cross_check_*` targets, +20 from new tests across T7/T9/T10/T11/T12 — matches T13's commit-message accounting exactly)
- **Skipped tests**: 4 ignored, unchanged count from baseline (not newly introduced by this feature)
- **Failures**: none
- **Zero rustc/clippy warnings**: confirmed independently

---

## Independent Re-Measurement of Shrinkage (AC 26/27)

| File | Cap | `shrinkage.md` claim | My `wc -l` | Match? |
|---|---|---|---|---|
| `device/analog/operators.rs` | ≤100 | 95 | 95 | ✅ |
| `device/analog/events.rs` | ≤140 | 160 | 160 | ✅ |
| `device/analog/limits.rs` | ≤130 | 153 | 153 | ✅ |
| `emit/analog_expr.rs` | ≤500 | 495 | 495 | ✅ |
| `resolve/pom/analog_ops.rs` | ≤330 | 375 | 375 | ✅ |
| crate total | — | 16670→16474 (−196) | 16670→16474 (−196, verified via `git ls-tree -r eadd5a2` sum and `find ... \| xargs wc -l`) | ✅ |

`shrinkage.md`'s numbers are accurate. The P2 shortfall (196/300 net lines, 3/5 file caps) was already user-accepted as a renegotiated target per the spec/tasks record, not re-litigated here — recorded factually per instructions.

---

## Edge Cases (spec.md)

- [x] Dev-dependency cycle builds (T1, confirmed in the baseline commit message and by the current clean build)
- [x] `$limit` inside a loop-unrolled/`fn`-inlined body still CSE's — `limiters.rs:201-229` (numeric proof only, see AC 11 caveat above)
- [x] `pnjlim`'s discarded-NaN branch: `piperine-signal/src/limit.rs:59-68` matches the argument in spec's edge case (short-circuits before `ln`)
- [x] `vte==0` guard: unchanged formula, `dv.abs() > 2.0*vte` guard present in `limit.rs:61`
- [x] Zero `$limit` slots: `device/analog/limits.rs:48-50,98-101,143-145` — all early-return `if nl==0`
- [ ] Two-point table clamped at both ends — **not independently verified by a dedicated test**; the general low-clamp gap (Sensor #4) subsumes this
- [x] `@initial` before `Limiter::seed` construction order — `device/analog/limits.rs`'s header + T6's Done-when documents this explicitly; construction site not touched by this feature

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
|---|---|---|
| CGS-01 | Pending | ✅ Verified |
| CGS-02 | Pending | ✅ Verified |
| CGS-03 | Pending | ✅ Verified |
| CGS-04 | Pending | ✅ Verified |
| CGS-05 | Pending | ⚠️ Verified with a flagged test-discrimination gap (AC 11) |
| CGS-06 | Pending | ✅ Verified |
| CGS-07 | Pending | ✅ Verified |
| CGS-08 | Pending | ⚠️ Verified with a flagged test-discrimination gap (AC 18) |
| CGS-09 | Pending | ✅ Verified |
| CGS-10 | Pending | ✅ Verified |
| CGS-11 | Pending | ✅ Verified |
| CGS-12 | Pending | ⚠️ Verified — pre-accepted shortfall, factually confirmed |
| CGS-13 | Pending | ✅ Verified |
| CGS-14 | Pending | ✅ Verified |

---

## Summary

**Overall**: ✅ Ready (with 2 minor, non-blocking test-discrimination gaps recommended for follow-up)

**Spec-anchored check**: 31/34 ACs matched spec outcome cleanly; 3 flagged (AC 11, AC 18 are test-discrimination gaps in otherwise-correct code; AC 26/27 are the pre-accepted P2 shortfall)
**Sensor**: 3/5 mutations killed, 2/5 survived (both in the P1 scope: AC 11's CSE-count claim, AC 18's low-end table clamp)
**Gate**: 276 test-result blocks, 1689 passed, 0 failed, 0 warnings

**What works**: All five families genuinely delegate with no second copy (verified by direct reading, not just grep); the digital-edge behavior fix (AC 32) is real and tested; `diff.rs` is provably untouched (empty git diff); the delegation guard is a real, proven-able-to-fail gate (re-verified myself); the shrinkage numbers in `shrinkage.md` are accurate to the line.

**Issues found**:
1. **AC 11 (CSE call count)**: `limiters.rs`'s `a_repeated_structurally_equal_limit_collapses_to_one_slot_not_two` proves numeric correctness under working CSE, but cannot distinguish "one call" from "two calls with the same result" — it never inspects the Cranelift IR's call-instruction count. Not a code defect (the CSE code itself is correct and unchanged from before delegation), but the test does not mechanically prove the literal AC text. **Fix**: add an assertion on compiled function IR (e.g. via Cranelift's own inspection API, or a call counter injected through a test-only instrumentation hook) counting `call` instructions to the `pnjlim`/`fetlim`/`limvds` symbols.
2. **AC 18 (table low-end clamp)**: no test in scope drives the lowered `table()` expression's `x` below `xs[0]`. `host_capability_parity.rs`'s fixture only exercises the high clamp. Not a code defect (the low-clamp `If` arm is structurally identical to the high-clamp arm and both were reviewed directly), but the claim is unproven by the suite. **Fix**: extend `table_op.rs` or add a new `piperine-codegen` integration test asserting the clamped value at `x < xs[0]` (and ideally the two-point-table edge case from spec.md's Edge Cases list, also unverified).

**Next steps**: Both issues are test-strengthening follow-ups, not implementation defects — they do not block accepting this feature as delivered. Recommend two small fix-tasks (above) be filed for a fast follow-up commit; no re-verification loop is required for the feature itself since the underlying code was independently confirmed correct by direct reading.

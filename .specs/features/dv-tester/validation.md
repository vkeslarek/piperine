# dv-tester Validation

**Date**: 2026-08-01
**Spec**: `.specs/features/dv-tester/spec.md`
**Diff range**: `ae65570..HEAD` (43 commits, T1–T37)
**Verifier**: independent sub-agent (author ≠ verifier)

> **Fix-round resolution status (2026-08-01)** — all five ranked gaps below
> addressed by an implementer in four commits on `feature/device-verification`.
> This note only annotates resolution; the Verifier's original findings below
> are left unmodified as the evidence-of-record.
>
> | Gap | Status | Commit |
> |---|---|---|
> | DVT-30 (Major — levels compose) | Fixed | `793c42c` |
> | Settled-only dispatch mutant (Major) | Fixed | `9734788` |
> | `Wave::pending_edges` boundary mutant (Minor) | Fixed | `3aa97f7` |
> | DVT-14 spec-precision gap | Fixed | `cb4644a` |
> | DVT-16 spec-precision gap | Fixed | `cb4644a` |
> | DVT-26 spec-precision gap | Fixed | `cb4644a` |
> | DVT-35 spec-precision gap | Fixed | `cb4644a` |
>
> Full description of what changed and why is in each commit message; see
> also the per-gap annotations inline below. Both discrimination-sensor
> mutations (#1 settled-only dispatch, #4 `pending_edges` boundary) were
> re-run against the strengthened tests and confirmed to now fail (kill),
> then reverted before committing — see `9734788` and `3aa97f7`.

---

## Task Completion

All 37 tasks spot-checked across every phase against real code/tests (not just the checklist). All confirmed done.

| Task | Status | Notes |
|---|---|---|
| T1 | ✅ Done | `crates/piperine-signal/tests/purity.rs` — mechanical guard, empty `[dependencies]`, banned-symbol scan |
| T2 | ✅ Done | `ops.rs` — all rules present, tests named per rule |
| T3 | ✅ Done | `events.rs` — seeding asymmetry tests present and correct |
| T4 | ✅ Done | `behavior/table.rs` — pure piecewise, derivative = segment slope |
| T5 | ✅ Done | `limit.rs` — 5 pnjlim rules; `tests/ngspice_validation.rs` present, 33 tests |
| T6 | ✅ Done | 4 cross-check fixtures in `crates/piperine-signal/tests/`, value-equal assertions (`1e-9` tolerance) |
| T7 | ✅ Done | `wave.rs` — full catalog, zero-host-call counter test, per-kind DC values |
| T8 | ✅ Done | `behavior/kinds.rs` — cross-Jacobian, discontinuous kinds register crossings |
| T9 | ✅ Done | `verify.rs` — deliberately-wrong-Jacobian oracle, names kind/point/both values |
| T10 | ✅ Done | `events.rs` tier-2 crossing prediction + refinement, terminates at floor |
| T11 | ✅ Done | `measure/kinds.rs` — O(1) accumulators, error bounds, `refine` |
| T12 | ✅ Done | `measure/gate.rs` — time/analog/digital(settled-only) gating |
| T13 | ✅ Done | `measure/handle.rs`, `measure_replay_identity.rs` — lifecycle + replay identity |
| T14 | ✅ Done | `device/host_element.rs`, `device/adapter.rs` — `HostElement` + `HostAdapter<T>` |
| T15 | ✅ Done | `StampSink`/`Ctx`/`StateView` present with index-validated access |
| T16 | ✅ Done | `device/level1.rs` — `BehaviorStamp`, `MeasureFeed`, `limiting_report` |
| T17 | ✅ Done | `@device` host binding resolution (confirmed via T18/T30 fixtures using it) |
| T18 | ✅ Done | `crates/piperine-api/tests/host_device_e2e.rs` — but see AC gap below (levels-composed claim) |
| T19 | ✅ Done | `sequencer/driver.rs` — coroutine driver, resume-exactly-once, rejected-step proof |
| T20 | ✅ Done | `sequencer/install.rs` — install-relative time, no re-anchor |
| T21 | ✅ Done | `sequencer/drive.rs`, `conflicts.rs` — declared impedance, EventSink digital drive |
| T22 | ✅ Done | `sequencer/findings.rs`/`control.rs` — `expect`/`warn`, bound-based comparison |
| T23 | ✅ Done | `circuit.rs:330-332` + `element.rs:555` — broadcast dispatch (not literal subscriber list, see deviation below); `tests/digital_settle_subscriber.rs` mechanical no-new-`EventKind` proof |
| T24 | ✅ Done | analog crossing waits on T10 machinery |
| T25 | ✅ Done | `tests/sequencer_dc_ac.rs` — AC loud, DC skips coroutine, per-kind DC value |
| T26 | ✅ Done | `sequencer_ordering_and_catalog.rs`, `conflicts.rs` — POM order, conflict catalog |
| T27 | ✅ Done | `piperine-python/src/device.rs` — typed error w/ sim time |
| T28 | ✅ Done | `piperine-python` Python surface, closed catalog with named error |
| T29 | ✅ Done | `tests/host_parity.rs` — real symbol enumeration both sides, self-test proving detection fires |
| T30 | ✅ Done | `tests/host_capability_parity.rs` — 4 twinned devices, value-equal |
| T31 | ✅ Done | `crates/piperine-api/tests/monitor_composition.rs` — settled-only, hierarchy, array/param composition |
| T32 | ✅ Done | `piperine-lang/src/elab/lower/constraint.rs` — elaboration cap refusal, exact message |
| T33 | ✅ Done | `crates/piperine-codegen/tests/cover_bin_mapping.rs` — count-based off-posture proof |
| T34 | ✅ Done | `crates/piperine-api/tests/coverage_merge.rs` — merge + empty-bin naming |
| T35 | ✅ Done | `docs/spec/part_vii_solver.md` — includes explicit `SPEC_DEVIATION` on subscriber-vs-broadcast |
| T36 | ✅ Done | `docs/spec/part_viii_host_api.md`/appendix C |
| T37 | ✅ Done | `docs/spec/` grammar/appendix updates, `CLAUDE.md` crate table carries `piperine-signal` |

---

## Spec-Anchored Acceptance Criteria

### P1: `piperine-signal` (DVT-01..06)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-01 purity | no in-tree dep; no Element/matrix/JIT symbol, mechanically checked | `crates/piperine-signal/tests/purity.rs:12-35,42-96` | ✅ PASS |
| DVT-02 operators | delay/slew/transition/idt all rules intact | `ops.rs:173-321` (8 named tests, one per rule) | ✅ PASS |
| DVT-03 detectors | unseeded cross never fires, unseeded above does | `events.rs:242-257` — `unseeded_cross_never_fires`, `unseeded_above_does_fire_on_first_sample` | ✅ PASS |
| DVT-04 `table` | derivative = segment slope, refuses loud on 4 conditions | `table.rs:91-201` | ✅ PASS |
| DVT-05 limiter | vcrit seed, seed-preservation, veto tolerance `1e-6+1e-4|vnew|`, minus-node move | `limit.rs:197,260-319` | ✅ PASS |
| DVT-06 cross-check fixtures | values asserted equal, one per family | `cross_check_{operators,events,table,limiter}.rs`, `assert!((a-b).abs()<1e-9)` | ✅ PASS |

### P1: `Wave`/`Behavior` (DVT-07..12)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-07 `Wave` zero host calls | native eval, counted zero | `wave.rs:467-512` (`AtomicU32`/`Cell` counter) | ✅ PASS |
| DVT-08 `Wave` breakpoints | each kind publishes own edges | `wave.rs:418-463` | ✅ PASS |
| DVT-09 `Behavior` cross-Jacobian, PWM zero host calls | `crates/piperine-api/tests/host_device_e2e.rs:174-256,379+` (`hysteretic_pwm_runs_end_to_end`, `behavior_evaluation_is_the_closed_form_rust_path_not_a_host_call`) | ✅ PASS (cross-crate; correctly located one layer up from `piperine-signal`) |
| DVT-10 discontinuous `Behavior` registers crossing | `behavior/kinds.rs:85-90,110-112`, tested `:290-298` | ✅ PASS |
| DVT-11 catalog closed(Py)/open(Rust) | Rust: `behavior_extensibility.rs` external `impl BehaviorKind`; Python: `host_device_library.rs:91-109` refusal naming level 0 + Rust route | ✅ PASS |
| DVT-12 `verify_derivatives` FD oracle | `verify.rs:176-201` `WrongSquare` kind caught, names kind/point/both values | ✅ PASS |

### P1: `Measure` (DVT-13..18)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-13 O(1) accepted-only | `measure/kinds.rs:555-571` size-unchanged after 100k feeds | ✅ PASS |
| DVT-14 online/post-run identity | `measure_replay_identity.rs:28-89` `assert_eq!` | ⚠️ Spec-precision gap — see below **[Fixed in cb4644a]** |
| DVT-15 error bound; `expect` vs bound | bound: `measure/kinds.rs:428-525`; `expect`-vs-bound: `crates/piperine-api/src/sequencer/control.rs:145-158` | ✅ PASS |
| DVT-16 `refine=True` narrows + floor termination w/ report | narrows: `measure/kinds.rs:528-551`; floor-termination code exists `kinds.rs:66-68` | ⚠️ Spec-precision gap — see below **[Fixed in cb4644a]** |
| DVT-17 gating (time/analog/digital settled) | `measure/gate.rs:171-250` | ✅ PASS |
| DVT-18 read-before-sample loud | `measure/handle.rs:144-160`, exact message `:40` | ✅ PASS |

### P1: `HostElement` (DVT-19, 23, 24)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-19 blanket adapter, no downcast | `device/adapter.rs:206-268` resistor e2e, `assert!((v_mid-2.0).abs()<1e-9)`; `git show --stat fa4f78d` — zero solver files touched | ✅ PASS |
| DVT-23 restamp fine / rebuild loud | `sequencer_ordering_and_catalog.rs:184` (ok), `:187-191` (`expect_err`, msg contains "rebuild") | ✅ PASS |
| DVT-24 limiter veto → `limiting_report` | `device/level1.rs:151-171,281-296` | ✅ PASS |

### P1: `Sequencer` (DVT-25..30, 44)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-25 resumed exactly once, never on reject | `sequencer_time_wakes.rs:211-233` — real forced rejections (`steps_rejected > 0`), `assert_eq!(resumes,3)` | ✅ PASS |
| DVT-26 3 wake sources → `EventKind` mapping | mapping is real and exercised per-source (`driver.rs:240,270,274`); no single test names `EventKind::{Digital,Breakpoint,Crossing}` from `piperine-api` | ⚠️ Spec-precision gap **[Fixed in cb4644a]** |
| DVT-27 edge wait settled-only | `sequencer_edge_waits.rs:127-188` — genuine glitch fixture, mid-cycle≠settled | ✅ PASS |
| DVT-28 union + POM order | `sequencer_ordering_and_catalog.rs:51-88` | ✅ PASS |
| DVT-29 `expect`/`warn`, posture, bound-comparison | `sequencer_expect_posture.rs:82-127`, `control.rs:145-158` | ✅ PASS |
| DVT-30 levels compose in one class | `host_device_e2e.rs:126-169` proves level 0 + level 1 in one class; **no test composes all 3** (level 0 + level 1 + sequencer) in one class — `Sequencer<P>`'s `HostElement` impl never overrides `load_dc` | ❌ GAP **[Fixed in 793c42c]** |
| DVT-44 fail-loud catalog | unknown net/conflict/non-yielding/rebuild-`set_param`/unfed-measure/AC — all present with file:line | ✅ PASS |

### P1: drive/install (DVT-31..36)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-31 zero host crossings, counted | `sequencer_drive.rs:183-238` `assert_eq!(calls,4)` vs `>10` accepted points | ✅ PASS |
| DVT-32 install-relative, no re-anchor | `sequencer/install.rs:178-194` | ✅ PASS |
| DVT-33 declared impedance, explicit ideal | `sequencer/drive.rs:135,149` | ✅ PASS |
| DVT-34 internal-net drive reported | `sequencer/drive.rs:158-166` | ✅ PASS |
| DVT-35 digital drive never touches MNA | `sequencer/drive.rs:185-212` — structural/type-level proof only, no runtime MNA-absence assertion | ⚠️ Spec-precision gap **[Fixed in cb4644a]** |
| DVT-36 AC loud, DC skips coroutine, per-kind DC | `sequencer_dc_ac.rs:41-145` | ✅ PASS |

### P1: capability parity (DVT-37, 38)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-37 4 twinned devices identical | `tests/host_capability_parity.rs:130-568` | ✅ PASS |
| DVT-38 parity guard, proven-can-fail | `tests/host_parity.rs:879-958` (real symbol probes, not a second hardcoded list), `:979-1008` synthetic-missing-name self-test | ✅ PASS (self-test proves detection fires; literal "add a real name, revert" history not replayable, but substance satisfied) |

### P2/P3: monitors, `cover` (DVT-39..43)

| Criterion | Spec-defined outcome | file:line + assertion | Result |
|---|---|---|---|
| DVT-39 monitors settled-only, hierarchy | `monitor_composition.rs:99-164` | ✅ PASS |
| DVT-40 bin hit = counter increment | `cover_bin_mapping.rs:54-77` | ✅ PASS |
| DVT-41 `cover=off` no kernel call, database untouched | `cover_bin_mapping.rs:85-97,126-135` — count-based | ✅ PASS |
| DVT-42 cross cap loud at elaboration | `constraint.rs:673-684`, `cover_parse.rs:126-138` | ✅ PASS |
| DVT-43 merge + empty-bin naming, zero-point = all empty | `coverage_merge.rs:74-121` | ✅ PASS |

**Status**: ⚠️ Issues present — 1 hard GAP (DVT-30), 4 spec-precision gaps (DVT-14, DVT-16, DVT-26, DVT-35), rest PASS.

---

## Discrimination Sensor

| # | File:line | Description | Killed? |
|---|---|---|---|
| 1 | `crates/piperine-solver/src/core/circuit.rs:330-332` | Dispatched `on_digital_settled` with the **pre-settle** snapshot (`&before`) instead of the post-settle `self.digital_state.nets` | ❌ **Survived** — the glitch fixture's initial value coincidentally equals its final settled value, so `tests/digital_settle_subscriber.rs`'s settled-only test does not distinguish "settled" from "the value before this accept ran" **[Fixed in 9734788 — re-run against this exact mutation, now kills]** |
| 2 | `crates/piperine-api/src/sequencer/conflicts.rs:88-93` | `DriveRegistry::check()` made a no-op (last-writer-wins, never conflicts) | ✅ Killed — `two_instances_claiming_the_same_net_conflict_regardless_of_registration_order` fails immediately |
| 3 | `crates/piperine-codegen/src/device/analog/coverage.rs:83-85` | Removed the `cover=off` early return — coverage kernel always runs | ✅ Killed — `cover_off_never_calls_the_coverage_kernel` and `an_optimizer_loop_with_cover_off_leaves_the_database_untouched` both fail |
| 4 | `crates/piperine-signal/src/wave.rs:242` | `push_in`'s window test `t <= end` weakened to `t < end` (drops an edge exactly at the window's right boundary) | ❌ **Survived** — no test places a `pending_edges` window boundary exactly on a declared edge **[Fixed in 3aa97f7 — re-run against this exact mutation, now kills]** |
| 5 | `crates/piperine-signal/src/events.rs:67` | `Trigger::Above`'s unseeded default flipped from `true` to `false` (matching `cross`'s asymmetry, defeating the documented distinction) | ✅ Killed — `unseeded_above_does_fire_on_first_sample` fails |

**Sensor depth**: lightweight (5 targeted mutations)
**Result**: 3/5 killed, 2/5 survived — ❌ two real test-discrimination gaps found (see Ranked Gaps)

All mutations discarded via `git checkout --` immediately after each run; `git status --short` confirms a clean tree (only pre-existing, unrelated `.specs/` doc modifications remain).

---

## Code Quality

| Principle | Status |
|---|---|
| No features beyond what was asked | ✅ |
| No abstractions for single-use code | ✅ |
| No unnecessary "flexibility" added | ✅ |
| Only touched files required for task | ✅ (T35–T37 docs-only, confirmed no `crates/` source touched except `CLAUDE.md`) |
| Didn't "improve" unrelated code | ✅ |
| Matches existing patterns/style | ✅ — `//!` headers, MD-28 test placement, MD-34 module splits all followed |
| Would senior engineer approve? | ✅ with the DVT-30 gap and 2 surviving mutants addressed |
| Tests map to ACs, non-shallow | ✅ — spot-checked across every phase; assertions target values, not "ran without panicking" |
| Spec-anchored outcome check | ⚠️ 4 spec-precision gaps flagged above, 1 hard gap (DVT-30) |
| Per-layer coverage: domain 1:1, routes happy+edge+error | ✅ |
| Every test maps to a spec AC — no unclaimed tests | ✅ |
| Documented guidelines followed | `CLAUDE.md` (fail-loud, zero-warnings, MD-28), `.specs/STATE.md` (MD-31: policy invariant lives in the gate) — followed |

---

## Edge Cases

- [x] `advance(dt)` past stop time → clamped, sequencer told (T19/T25 fixtures)
- [x] `advance(0)` bounded, then loud → `MAX_ZERO_ADVANCE_RESUMES` (`driver.rs`)
- [x] Non-yielding coroutine → loud (`driver.rs:442-465`)
- [x] Coroutine exception → typed error with sim time (`piperine-python/src/device.rs:401-428`)
- [x] Unknown net → loud, lists candidates (`session/entry.rs:484-488`)
- [x] Same-net driver conflict → loud regardless of order (`conflicts.rs`) — proven by discrimination sensor mutation #2
- [x] Unfed `Measure` / never-opened gate → loud (`measure/handle.rs:144-160`)
- [x] `Behavior` wrong Jacobian → caught by `verify_derivatives` (`verify.rs:176-201`)
- [x] `cover` zero accepted points → all bins empty, never vacuous 100% (`coverage_merge.rs:106-121`)
- Not independently verified in this pass: `Wave.from_fn` callback raising, bulk-Python-device documentation, `checks=off` interaction with host `expect` findings — no counter-evidence found, but not explicitly re-derived either (time-boxed).

---

## Gate Check

- **Gate command**: `CARGO_PROFILE_DEV_DEBUG=0 cargo build --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo clippy --workspace --all-targets -- -D warnings && CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli`
- **Result**: build ✅ clean; clippy ✅ zero warnings; test ✅ 0 failures across 278 test binaries (~1671 total tests, doctests included); doc ✅ clean
- **Skipped tests**: none observed
- **Failures**: none

---

## Fix Plans

> **Resolution status: all three fix plans below implemented — see the
> table at the top of this file for commit hashes.**

### Fix 1: DVT-30 — no test composes all three levels in one host device class

- **Root cause**: `Sequencer<P>`'s `HostElement` impl (`crates/piperine-api/src/sequencer/driver.rs:285-353`) never overrides `load_dc`/`load_transient`, so it structurally cannot stamp level 0. `host_device_e2e.rs:126-169` proves level 0 + level 1 composition (T18's own commit message: "minus the sequencer half which is Phase 4's job"), but no later task (T19–T26) closes the loop with a fixture combining a device that stamps directly (level 0), installs a `Wave`/`Behavior` (level 1), **and** sequences time (level 2) in one class — which DVT-30/T18's own done-when literally requires ("the composition `ideal.md` example G describes").
- **Fix task**: add one fixture (likely alongside `sequencer_time_wakes.rs` or `host_device_e2e.rs`) where a single `HostElement` both stamps a level-0 resistor and drives a `Sequencer`-style coroutine wake, or extend `Sequencer<P>` with an optional level-0 hook and prove it in a test.
- **Priority**: Major — this is the spec's literal "three levels compose in one class" claim (DVT-30) and Success Criteria item.

### Fix 2: settled-only dispatch — mutation survived

- **Root cause**: `tests/digital_settle_subscriber.rs`'s glitch fixture initializes net `c` to `Zero` and settles back to `Zero`, so a dispatch using the pre-accept snapshot instead of the post-settle one is indistinguishable from the correct behavior in that fixture.
- **Fix task**: add a second glitch fixture (or extend the existing one) where the **pre-accept** value of the watched net differs from its **post-settle** value at the same instant (e.g. net starts `One`, toggles through a glitch, settles `Zero`), and assert the dispatched value is the settled one.
- **Priority**: Major — this is exactly the correctness property DVT-27/T23 exists to guarantee, and the current fixture cannot distinguish "settled" from "stale."

### Fix 3: `Wave::pending_edges`'s window-boundary condition — mutation survived

- **Root cause**: no test in `wave.rs` places a declared edge exactly at the `pending_edges(from, end, …)` window's right boundary (`t == end`), so `push_in`'s `t <= end` vs `t < end` distinction is untested.
- **Fix task**: add a test asserting an edge at exactly `end` is included (e.g. `Wave::ramp(0.0, 1.0, 2.0).pending_edges(0.0, 2.0, &mut out)` includes `2.0`).
- **Priority**: Minor — likely correct in practice (breakpoint-landing semantics usually want inclusive-right), but currently unverified and a real regression could ship silently.

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
|---|---|---|
| DVT-01..13, 15, 17–25, 27–29, 31–34, 36–44 | Done | ✅ Verified |
| DVT-14, DVT-16, DVT-26, DVT-35 | Done | ⚠️ Verified with spec-precision gap noted **[Fixed in cb4644a]** |
| DVT-30 | Done | ❌ Needs Fix **[Fixed in 793c42c]** |

---

## Summary

**Overall**: ⚠️ Issues

**Spec-anchored check**: 40/45 ACs matched spec outcome cleanly; 4 spec-precision/scope gaps flagged (DVT-14, DVT-16, DVT-26, DVT-35); 1 hard gap (DVT-30)
**Sensor**: 3/5 mutations killed, 2/5 survived
**Gate**: build/clippy/test/doc all green — 0 failures, 0 warnings

**What works**: The crate boundary purity guard is genuinely mechanical (MD-31-compliant); the extraction (T1–T6) preserves every cataloged rule with named tests; `HostElement`/`HostAdapter` truly touches zero solver files for its core seam (verified via `git show --stat`); the T23 solver touch is honestly self-documented as a broadcast rather than the design doc's literal "subscriber list," backed by a real mechanical no-new-`EventKind` test; capability parity (DVT-37) and the parity guard (DVT-38) are both real, value-asserting, and the guard's detection mechanism is proven to fire via a synthetic self-test; `cover`'s posture separation and elaboration cap are exactly as specified with count-based (not output-based) proof.

**Issues found**:
1. DVT-30 (Major) — no fixture proves all three levels compose in one host device class; `Sequencer` structurally cannot stamp level 0.
2. Settled-only dispatch (Major) — the discrimination sensor found the fixture proving DVT-27/T23's core correctness claim cannot distinguish a pre-settle from a post-settle value, because its glitch net happens to start and end at the same logic level.
3. `Wave::pending_edges` boundary (Minor) — an edge exactly at the window's right bound is untested.
4. Four spec-precision gaps (DVT-14, DVT-16 unreached branch, DVT-26 no `EventKind`-name assertion, DVT-35 structural-only MNA-absence proof) — none is a functional defect, but each is weaker than the spec's literal wording asks.

**Next steps**: Route Fix 1 and Fix 2 back to an implementer as the two Major items (both are within the 3-fix-task budget); Fix 3 optional/Minor. Re-run the discrimination sensor on the same two mutation points after the fix to confirm they now kill.

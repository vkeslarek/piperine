# solver-live-params Validation

**Date**: 2026-07-17
**Spec**: `.specs/features/solver-live-params/spec.md`
**Diff range**: `d92fbbb..HEAD` (107bf40 … 07a6110, branch `feature/solver-live-params`)
**Verifier**: independent sub-agent (author ≠ verifier); coverage re-derived evidence-or-zero

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1–T5 (batch 1) | ✅ Done | commits 107bf40, f15273a, 8d5c73d, 62486d8, d400973 |
| T6–T11 (batch 2) | ✅ Done | commits e74bb94, 7d1a47b, c5f4b3f, 35ba4e8, 47d7dfa, 07a6110 |

---

## Spec-Anchored Acceptance Criteria

All file paths absolute from repo root `/home/keslarek/Git/piperine`.

| AC | Spec-defined outcome | Evidence (`file:line` + assertion) | Result |
| -- | -------------------- | ---------------------------------- | ------ |
| LIVE-01 parity | Same path+param addresses same instance via POM and solver | `crates/piperine-codegen/tests/live_params.rs:69` `assert_eq!(labels, pom_paths)`; `:84–105` live set of bundle field `model_r0` vs POM-staged rebuild oracle, `(v_live − v_staged).abs() < 1e-9`; scalar param `:108–110` | ✅ PASS (see spec-precision note 1) |
| LIVE-02 zero recompile | `Invalidation::Restamp/Temperature`, compile count unchanged | `crates/piperine-codegen/tests/live_set_never_recompiles.rs:157,164` `assert_eq!(inv, Restamp/Temperature)`; `:176–180` `assert_eq!(AnalogKernel::compile_count(), after_build)` across 10 set+solve cycles | ✅ PASS |
| LIVE-03 unknown param loud + candidates | Error names element, lists params | `crates/piperine-solver/tests/live_params.rs:195–205` asserts `r1`, `bogus`, `"available parameters"`, candidate `r`; JIT devices `crates/piperine-codegen/tests/live_params.rs:116–135` lists flattened `model_r0`+`model_k`; impl `crates/piperine-solver/src/core/circuit.rs:143–159` | ✅ PASS |
| LIVE-04 unknown path loud | Error carries the path | `crates/piperine-solver/tests/live_params.rs:184–190` `err.to_string().contains("nope")` | ✅ PASS |
| LIVE-05 bypass invalidation | Next iteration re-evaluates the device | `crates/piperine-solver/tests/live_params.rs:280–298` set through held `DcSolver`, re-solve gives 7.5 V (stale bypass would freeze 5 V); impl `crates/piperine-solver/src/solver/dc.rs` `set_element_param` → `invalidate_bypass()` — mutation-verified (sensor #3) | ✅ PASS |
| LIVE-06 next-step + breakpoint | Step lands exactly on t, LTE skipped at edge; RC 2k→1k@5µs closed form reltol 1e-3 | `crates/piperine-codegen/tests/live_params.rs:226–317` — `:271–277` exactly one landing at each set time (`abs < 1e-18`); `:293–301` closed-form τ-switch reference within `1e-3·5 + 1e-6`; `:306–308` trajectories separated > 0.05; `:311–316` no rejection storm. Breakpoint landing mutation-verified (sensor #5) | ✅ PASS |
| LIVE-07 reactive discontinuity | No NaN, no dt collapse, no rejection storm, reltol 1e-3 vs fresh-from-pre-set-state | C-jump `crates/piperine-codegen/tests/live_params.rs:326–405` (`is_finite`, closed form, `steps_rejected ≤ accepted/5+5`, `accepted < 5000`); L-jump `:411–492` (flux leg, same gates) | ✅ PASS |
| LIVE-08 idle set next run | Applies to next analysis; sets ≤ start drain pre-OP, no breakpoint | `crates/piperine-solver/tests/live_params.rs:249–275` (`Restamp` then 7.5 V; `OperatingPoint` then 15 V); `:375–385` set at t=0 applies to whole run (every step 7.5 V) | ✅ PASS |
| LIVE-09 ≥OP re-solve at t | Recorded point AT t reflects the post-set value | `crates/piperine-solver/tests/live_params.rs:315–344` — landing at 5 µs asserted `(v_at − 10.0).abs() < 1e-9` (post-set), all pre/post points partitioned | ✅ PASS |
| LIVE-10 python single compile | Elaboration+JIT once; set+op loop adds zero compilations | `crates/piperine-python/tests/live_session.rs:45–86` isolated binary: `assert_eq!(loop_compiles, per_build)` for 10×(set+op) | ✅ PASS |
| LIVE-11 python parity | Same errors as Rust path (exact messages), same invalidation semantics | `crates/piperine-python/tests/live_facade.rs:67–139` `e.args[0] == {solver oracle string:?}` for unknown label and param (`KeyError`); `src/live.rs` in-crate tests `:712–727` KeyError + candidates; `schedule_set` mid-tran RC from Python with exact landing + closed form | ✅ PASS |
| LIVE-12 optimization loop perf | ≥100 iters == fresh builds reltol 1e-3; ≥10× faster | `examples/live_optimize.py:67–70` per-point `abs(a−b) ≤ 1e-3·max + 1e-9` over 100 points, `assert speedup >= 10.0`; `crates/piperine-python/tests/live_optimize_example.rs:44–53` whole example JITs exactly `101·per_build` (live loop = 0 extra) | ✅ PASS |
| LIVE-13 result-shape uniformity | Same pyclass types as `_Module` | `crates/piperine-python/src/live.rs:667–674` asserts `_OpResult`/`_Trace`/`_AcTrace`/`_NoiseTrace`; facade `tests/live_facade.rs:82–84` `type(s).__name__ == "LiveSession"` | ✅ PASS |
| LIVE-14 auto re-elab + notice | Rebuild automatic, visible flag, behavior appears | `crates/piperine-python/src/live.rs:809–827` dio `ns none→1.2`: `rebuilds == 1`, sidewall lowers v(out), equals fresh staged build reltol 1e-3; solver half `crates/piperine-codegen/tests/live_params.rs:145–216` typed `Invalidation::Rebuild` with **no partial apply** (post-set solve still 5 V) | ✅ PASS |
| LIVE-15 state carry by net name | Carried voltages = next solve's initial guess | `crates/piperine-python/src/live.rs:830–838` `warm < cold` Newton iterations vs identical cold build — mutation-verified (sensor #4) | ✅ PASS |
| LIVE-16 mid-tran rebuild restart | Restart from t with carried ICs; waveform continuous at t | `crates/piperine-python/src/live.rs:916–994` — strictly increasing stitched axis, exactly one point at split, run reaches stop, two-phase closed form within `1e-3·5 + 1e-6` at **every** point (continuity at t is enforced by the reference), tail at new structure `(v_end − 2.5) < 0.02`; idle pre-run set survives the rebuild (dirty-ledger replay) | ✅ PASS |
| LIVE-17 re-elab failure keeps old circuit | Error surfaced, previous circuit usable | `crates/piperine-python/src/live.rs:851–865` failing set → `KeyError`, `rebuilds` unchanged, subsequent `op()` matches pre-failure result reltol 1e-3 | ✅ PASS |

**Status**: 17/17 ACs covered with spec-matching assertions. Two spec-precision notes below.

### Spec-precision notes (not failures)

1. **LIVE-01 grammar scope**: parity is asserted over the *actual* compilable grammar — flat instance labels + `{param}_{field}` bundle flattening. Dotted hierarchical paths (`"x1.d1"`) are not compilable today (nested hierarchy fail-louds in codegen: "flatten during elaboration"), documented in the test header (`crates/piperine-codegen/tests/live_params.rs:5–11`) and tasks.md T2. The spec's `"x1.d1"` example is aspirational; parity over the full grammar the POM accepts *for a compilable design* holds. Recommend a spec wording update when nested hierarchy lands.
2. **Digital-element edge case**: the "fails loud if the element declares none" branch is covered by a generic param-less element (`crates/piperine-solver/tests/live_params.rs:208–226`, message "declares no writable parameters") — behaviorally identical to a digital-only `PiperineDevice` (`crates/piperine-codegen/src/device/mod.rs` `set_param` returns `ParamError::Unknown` when `analog` is `None`, and `list_params` is empty). No test exercises a literal `DIGITAL`-capability element; the surface is uniform (`Element::set_param`), so this is a minor coverage nicety, not a gap in behavior.

---

## Edge Cases

- [x] **Last-write-wins, one recorded landing**: `crates/piperine-solver/tests/live_params.rs:347–372` (two sets same param same t → 1 landing, final value = later call, `assert_eq!(r2.get_param("r"), Some(Value::Real(3000.0)))`); unit `crates/piperine-solver/src/solver/transient.rs` `drain_preserves_scheduling_order_for_last_write_wins` — mutation-verified (sensor #2).
- [x] **Out-of-bounds, no partial apply**: `crates/piperine-solver/tests/live_params.rs:231–244` — 1e-12 is element-acceptable but below `ParamDescriptor` bounds (1e-9); only the central bounds gate can reject it; value asserted unchanged. Gate implemented before the element sees the write (`crates/piperine-solver/src/core/circuit.rs:125–141`).
- [x] **Temperature invalidation recompute**: `crates/piperine-codegen/tests/live_set_never_recompiles.rs:159–173` — temp set reports `Invalidation::Temperature`, next DC solve asserted against the temp-scaled closed form (`rt_eff = r0·(1 + tc·(T − TNOM))`), no stale tnom value, and no recompilation.
- [x] **Digital element param**: see spec-precision note 2 (same surface; loud when no params declared).
- [x] **Scheduled set with bad addressing fails the run**: `crates/piperine-solver/tests/live_params.rs:388–395`.
- [x] **Structural set at solver level is fail-loud** (MD-18 boundary): `transient.rs` returns typed error "re-elaborate at the host layer"; probe path in `live.rs` keeps structural entries out of the solver queue.

---

## Gate Check

- **Build**: `cargo build --workspace` — success, **0 warnings**.
- **Full**: `cargo test --workspace` — **472 passed, 0 failed** (baseline 445 pre-feature, 465 after batch 1; +27 total). No tests deleted; no weakened assertions found in the diff.
- **Python examples**: 24/24 green via `./target/debug/piperine run` (23 numbered + `live_optimize.py`; `*_plot.py` variants excluded as before).

---

## Discrimination Sensor

Scratch-state mutations (edit → targeted tests → `git checkout` restore). A control mutation (`phase_coeffs(phase, 2·h)` garbage in the same function) failed 3 tests, validating the pipeline picks up solver-crate mutations.

| # | Mutation | File | Killed? |
| - | -------- | ---- | ------- |
| 1 | Remove `TrBdf2::stage_coeffs` backward-Euler degradation at `prev_h ≤ 0` (return full trapezoid always) | `crates/piperine-solver/src/math/integration.rs:197–203` | ❌ **SURVIVED** — `cargo test -p piperine-codegen --test live_params` 6/6 ok, `-p piperine-python` all ok, `-p piperine-solver --test live_params` 11/11 ok |
| 2 | Reverse `SetQueue::drain_due` application order (breaks last-write-wins) | `crates/piperine-solver/src/solver/transient.rs:136–141` | ✅ Killed (`drain_preserves_scheduling_order_for_last_write_wins`, `drain_takes_only_due_entries…`) |
| 3 | Drop `invalidate_bypass()` in `DcSolver::set_element_param` | `crates/piperine-solver/src/solver/dc.rs` | ✅ Killed (`set_through_held_dc_analysis_invalidates_bypass_cache`) |
| 4 | Drop the net-name carry in `_LiveSession::auto_rebuild` (`carry = {}`) | `crates/piperine-python/src/live.rs:207–212` | ✅ Killed (`structural_set_auto_rebuilds…` warm<cold + `mid_transient_structural_set_restarts…`) |
| 5 | Disable scheduled-set breakpoint landing in the transient driver | `crates/piperine-solver/src/solver/transient.rs` solve loop | ✅ Killed (`scheduled_op_strength_set_resolves_consistently_at_the_breakpoint`) |

**Sensor depth**: 5 behavior-level mutations + 1 pipeline control.
**Result**: 4/5 killed — ❌ one surviving mutant → fix task below.

### Surviving mutant analysis

`stage_coeffs`'s prev_h=0 degradation was introduced as a T10 "solver fix en route" (tasks.md: the full `2/(γh)` weight with assumed-zero previous current "doubled the first-step derivative after any discontinuity"). No test discriminates it because every discontinuity restart in the suite begins at `1e-3·dt` (the post-set convention), which shrinks the O(h)·i_n error far below the reltol-1e-3 assertions; and the very first step of a run starts from a DC OP where the true reactive current is 0, making the missing term exact by accident. The claimed bug is plausible but has **zero discriminating evidence** in the test suite — evidence-or-zero says this behavior is unverified.

---

## Code Quality

| Principle | Status |
| --------- | ------ |
| Minimum code / no scope creep | ✅ diff surface matches T1–T11 exactly; no unrelated edits |
| Surgical changes | ✅ solver core additions are additive (`SetQueue`, `start_time`, snapshot/restore); MD-18 boundary respected (solver never re-elaborates, typed Rebuild fail-loud) |
| Matches patterns | ✅ isolated compile-count binaries follow `compile_once_sweep.rs` precedent; result mapping reuses `module.rs` plumbing; no macros (user preference) |
| Spec-anchored outcomes | ✅ every assertion targets the spec value (exact landings, closed forms, exact error strings, compile-count deltas) |
| Per-layer coverage expectation | ✅ matrix in tasks.md satisfied: solver unit+integration, codegen JIT integration, python integration+e2e, example e2e |
| No unclaimed tests | ✅ all new tests map to LIVE-01..17 or listed edge cases (headers cite the AC) |
| Guidelines followed | ✅ CLAUDE.md (zero warnings, `--workspace`), MD-13, MD-18 |

---

## Requirement Traceability

spec.md marks LIVE-01..17 Verified; this validation independently confirms all 17 (evidence table above). No status change required. LIVE-01's evidence note ("actual flat grammar") is accurate as written.

---

## Fix Plan

### Fix 1: discriminating test for the TR-stage restart convention (surviving mutant)

- **Root cause**: `TrBdf2::stage_coeffs(TR, h, prev_h=0)` backward-Euler degradation has no test that fails when it is removed — all restart scenarios use a tiny first step that masks the doubled-derivative error.
- **Fix task**: add a unit test in `crates/piperine-solver/src/math/integration.rs` asserting `stage_coeffs(Trapezoidal, h, 0.0) == (1/(γh), −1/(γh), 0.0)` and `stage_coeffs(Trapezoidal, h, prev_h>0) == phase_coeffs(...)` (kills the mutant directly); optionally an integration test where a mid-tran set is followed by a *non-tiny* first step (e.g. dt_min forced up) with a tightened tolerance at the first post-edge point, to pin the behavior end-to-end.
- **Priority**: Minor (behavior believed correct; test-strength gap only — everything user-visible is green).

---

## Summary

**Overall**: ⚠️ Issues — PASS on all 17 ACs, all edge cases, and all gates; one sensor gap (surviving mutant) requires a test-strengthening fix task before final closure.

**Spec-anchored check**: 17/17 ACs matched spec outcomes; 2 spec-precision notes (LIVE-01 flat-grammar scope — documented, judged in-scope; digital-element edge covered generically).
**Gate**: build 0 warnings; 472 passed, 0 failed; python examples 24/24.
**Sensor**: 4/5 killed, 1 survived (`stage_coeffs` restart convention) → Fix 1.

**What works**: solver live set with loud errors/bounds/bypass invalidation; scheduled sets with exact breakpoint landing, last-write-wins, ≥OP re-solve; reactive C/L jumps storm-free at reltol 1e-3; MD-18 zero-recompile proven at solver and python layers; LiveSession with error parity, single compile, ≥10× optimization loop; auto re-elab with notice, net-name carry, mid-tran restart stitching, and failure-keeps-old-circuit.

**Next steps**: execute Fix 1 (one unit test, ~10 lines) and re-run the sensor on mutation 1.

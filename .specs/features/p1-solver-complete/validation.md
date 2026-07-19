# p1-solver-complete Validation

**Date**: 2026-07-18
**Spec**: `.specs/features/p1-solver-complete/spec.md`
**Diff range**: `fe2fe82..HEAD` on `feature/bench-removal` (30 commits; T21/T23/T24 core changes pre-landed in d400973/2403e29/1857df5 outside the range, their evidence in-range)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

T1–T15, T17–T27 all marked ✅ Done in tasks.md and verified in-tree. T16 (urc) ⛔ BLOCKED on external `codegen-parametric-devices` — out of scope per brief; noted, not failed.

---

## Spec-Anchored Acceptance Criteria

| AC | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| SC-01 sens values | ∂V(mid)/∂R2 = V·R1/(R1+R2)² = 2.5e-3 to 1e-6 rel; diode vs two-sided FD to 1e-3 | `tests/sens.rs:99` `((d_r2 - analytic)/analytic).abs() < 1.0e-6` (+`:104` ∂V/∂v1.dc = 0.5); `tests/sens.rs:124` `((a-b)/a).abs() < 1.0e-3` step-independence + `:123` sign | ⚠️ Divider ✅ exact analytic. Diode ⚠️ spec-precision: spec says "matching a two-sided FD reference `(V(p+h)−V(p−h))/2h`"; test asserts FD step-size self-consistency (1e-6 vs 1e-4 agree to 1e-3), not an independent re-solve reference (deviation documented in commit b2fdc60 message) |
| SC-02 sens loud errors | Rebuild-class OR nonexistent param → loud error naming param | `tests/sens.rs:142` unknown element names `nope`; `:152` unknown param names `bogus` | ❌ PARTIAL — Rebuild-class branch (`crates/piperine-solver/src/solver/sens.rs:59`) has NO test; mutant M8 survived |
| SC-03 sens python surface | same values as Rust API, map (out, label.param)→f64 | `crates/piperine-python/tests/sens_parity.rs:78-82` `assert_eq!(py_r2, rust_r2)` + analytic anchor 1e-6 | ✅ PASS |
| SC-04 PSS periodic trace | \|x(T)−x(0)\| < shoot_tol (1e-6); phasor amplitude within 1% | `tests/pss.rs:87` `result.stats.residual < 1.0e-6`; `:100` `((amplitude - analytic)/analytic).abs() < 1.0e-2` with analytic `5/√(1+(ωRC)²)` | ✅ PASS |
| SC-05 PSS loud non-convergence | `SolverDomain::Pss`, iterations + residual in message; digital k·T diagnostic for dividers | `tests/pss.rs:110` period≤0 names "period"; `:126-131` ramp non-periodic asserts "did not converge"/"singular"/"does not repeat" | ❌ PARTIAL — analog loud paths ✅; the mixed-signal digital-periodicity guard + "circuit period appears to be k·T" diagnostic (`crates/piperine-solver/src/solver/pss.rs:174,294`) has NO test anywhere (T5 Done-when required the divider-by-2 case) |
| SC-06 PSS tstab + rectifier | tstab equivalence; rectifier ripple vs settled transient within 10·reltol | `tests/pss_host.rs:97-109` mean+peak_to_peak vs 14-period settled ref within `10·reltol·mean`, `mean > 2.0`; `:129` tstab amplitudes equal to 1e-9+1e-3 rel; `:140` settle estimate within 20% of analytic | ✅ PASS |
| SC-07 .dc host-proof | nested + source sweeps == fresh-build solve (exact), compile count 1 | `tests/dc_host_proof.rs:110` `assert_eq!(sweep_compiles, 0)` after 1 build; `:118,125` `restamped == fresh` (bit-exact) for 12 nested + 4 source points | ✅ PASS |
| SC-08 table operator | linear interp + end clamp; Jacobian = segment slope; non-monotonic loud | `crates/piperine-lang/tests/spec_simulation.rs:1105` V(mid)=1.75 to 1e-9 (Newton needs correct slope); `:1131` clamp V(a)=9.991 to 1e-9; `crates/piperine-codegen/tests/table_op.rs:19,25,31` loud paths | ⚠️ PASS with surviving mutant — value/clamp asserted exactly, but slope-division arithmetic is NOT discriminated (all test vectors use unit-spaced xs; mutant M4 survived) |
| SC-09 transition operator | linear ramp, breakpoints, ramped edge in trace | `spec_simulation.rs:1148` RuntimeState::Transition wiring; `:1202-1209` pre-rail 0, mid-ramp ∈ (0.3,0.6), settled 1.0±0.03; `:1433-1437` state survives rejected steps | ✅ PASS (edge rise/fall=0 untested — minor) |
| SC-10 idt AC stamp | X/(jω): −20 dB/dec, −90° across 4 decades | `spec_simulation.rs:1377` \|H(1kHz)\| = 1/(2π·1e6) to 1e-3; `:1382` phase = −π/2 ± 1e-3 ×9 pts; `:1388` decade ratio 0.1 ± 1e-3 | ✅ PASS |
| SC-11 multiple ac_stim | phasor superposition vs two-source circuit | `spec_simulation.rs:1325-1329` `(got-want).norm() < 1e-12` per point + anchor (1.5, √3/2) to 1e-9 | ✅ PASS |
| SC-12 @initial branch + UIC | t=0 = ic exactly; 5·e^(−t/RC) within 10·reltol | `spec_simulation.rs:1252` `(v0-5.0).abs() < 1e-6`; `:1259` per-step `\|got−5e^(−t/τ)\| ≤ 10·reltol·want + 1e-6` | ✅ PASS (mutant M6 killed) |
| SC-13 digital fused + identical | fused DigitalNetwork proven active (instrumentation), bit-identical | `crates/piperine-codegen/tests/digital_fusion.rs:110-121` fused_networks==1, 13 devices→1, adder sum bits asserted; `:57-70` per-step full state-vector equality fused vs per-device (static adder, clocked cone, NBA pipeline) | ✅ PASS |
| SC-14 MOS2+MOS3 parity | ngspice goldens within harness tolerance, live-or-SKIP | `tests/ngspice_validation.rs:304-321` OP goldens (nmos2/3 fixed+load); `:381-424` 4 sweep goldens (id_vds/id_vgs × mos2/3) | ✅ PASS — ngspice 44 on PATH, goldens ran LIVE in gate |
| SC-15 tline ideal | matched: delayed td, <1% reflection; open: doubling at 2·td; bad params loud | `tests/tline.rs:65-68` quiet before td, 0.5±0.02 after, ripple < 0.005; `:84-86` doubles to 1.0±0.03; `:91,110` loud z0/td | ⚠️ PASS — analytic cases green; the spec assumption's "ngspice `tra` golden" has no `.cir` pair in `tests/ngspice/` (never landed) |
| SC-16 urc | — | — | ⛔ BLOCKED (external dep, out of scope) |
| SC-17 transformer | AC ratio ≈ k·√(L2/L1); coupled-LC energy transfer | `tests/xfmr.rs:45-48` ratio 1.998±0.01 at 3 frequencies; `:122-139` secondary peak ≥0.7, primary collapse <0.35, peak-time window (1.1,1.55)µs | ✅ PASS |
| SC-18 stdlib off sentinels | no `1e99`/`$param_given` in models; suites green | grep of `headers/spice/`: zero code matches (one stale doc comment in `constants.phdl:18` still lists `$param_given` as an "ideal" system function — cosmetic); full model suites green in gate | ✅ PASS (comment nit noted) |
| SC-19 fetlim/limvds | ngspice DEVfetlim/DEVlimvds reference values | `crates/piperine-codegen/tests/limiters.rs:95,120` JIT limiter vs C-ported reference across vnew/vold/vto grid | ✅ PASS (mutant M1 killed) |
| SC-20 temperature | tnom rescaling uniform; diode ≈ −2 mV/°C | `crates/piperine-api/tests/temp_sweep.rs:72` monotonic fall ×10 steps; `:77-80` coefficient ∈ [−2.5,−1.5] mV/K | ✅ PASS |
| SC-21 inductor TR dual | previous-voltage dual form; coupled-LC/RL unchanged or tighter | `tests/xfmr.rs:136-139` peak-time window discriminates dual vs pure backward difference (window sits between the two); `crates/piperine-codegen/tests/live_params.rs:411` L-jump accuracy + storm-free | ✅ PASS (mutant M2 killed on both) |
| SC-22 IntegrationMethod removal | grep-clean; suite green | `grep -rn IntegrationMethod crates/ src/` → exit 1 (zero matches); gate green | ✅ PASS |
| SC-23 scheduler split + SignalBridge | behavior unchanged; split modules | `crates/piperine-solver/src/digital/{topology,state,scheduler}.rs` exist; `SignalBridge` at `core/circuit.rs:23` (task text said `core/bridge.rs` — placement deviation, AC met); full suite green | ✅ PASS |
| SC-24 as_iv + Integrator | new signatures, identical numerics | `analog/netlist.rs:228` `initial_values` owns the mapping; `math/integration.rs:167` `Integrator::trapezoid` used by `solver/noise.rs:284`; unit tests `integration.rs:190-204` | ✅ PASS |
| SC-25 Trace.i state recording | opt-in → recomputed current; off → loud error | `tests/session.rs:239` per-step KCL i_L==i_R to 1e-6·max + settled analytic 4.9876mA to 1e-3; `:274` disabled → error names device + `record_device_state` | ✅ PASS (mutant M7 killed) |
| SC-26 init_global ownership | Context::default() no global init | `crates/piperine-solver/tests/context_init.rs:12-31` process-isolated negative assertion (faer parallelism untouched) + `init_global` positive control | ✅ PASS |

**Status**: ❌ 23/25 fully matched; 2 partial (SC-02 Rebuild branch, SC-05 k·T diagnostic — no evidence); 2 spec-precision notes (SC-01 diode FD reference shape, SC-15 missing ngspice golden pair); SC-16 blocked out-of-scope.

---

## Discrimination Sensor

| # | Mutation | File:line | Expected discriminator | Killed? |
|---|----------|-----------|------------------------|---------|
| M1 | limvds `3*vold+2` → `2*vold+3` | `codegen/analog_emit.rs:452-453` | `cargo test -p piperine-codegen --test limiters` | ✅ Killed (`limvds_matches_ngspice_reference`) |
| M2 | TR-stage flux `if v_prev != 0.0` → `if false && …` | `codegen/device/analog.rs:1086` | `cargo test -p piperine --test xfmr` + `--test live_params mid_transient_l_jump` | ✅ Killed (both) |
| M3 | PSS 2nd-period guard tol `1e-9+1e-3·max` → `1e9` | `solver/pss.rs:157` | `cargo test -p piperine --test pss` | ✅ Killed (`non_periodic_circuit_fails_loud`) |
| M4 | table slope `(Δy)/(Δx)` → `Δy·Δx` | `lower/pom/analog_ops.rs:180` | `cargo test -p piperine-lang --test spec_simulation sim_dc_table` | ❌ **SURVIVED** — every table test vector uses unit-spaced xs (Δx=1), so the divide is invisible |
| M5 | table high clamp removed (extrapolate last segment) | `lower/pom/analog_ops.rs:196` | same | ✅ Killed (`sim_dc_table_clamps_at_the_ends`) |
| M6 | UIC clamp never released (`if false` around `uic_hold = false`) | `solver/transient.rs:561` | `spec_simulation sim_tran_initial_branch_force` | ✅ Killed |
| M7 | Trace.i loud error guarded off (`false && needs_banks…`) | `piperine-api/src/waveform.rs:245` | `cargo test -p piperine --test session trace_i_on_stateful` | ✅ Killed (disabled-path test) |
| M8 | sens Rebuild-invalidation guard off (`false &&`) | `solver/sens.rs:59` | `cargo test -p piperine --test sens` + full `piperine-solver` | ❌ **SURVIVED** — no test trips the Rebuild branch (SC-02 partial) |

**Sensor depth**: expanded (solver-critical tier) — 8 mutations
**Result**: 6/8 killed — FAIL (2 survivors)

---

## Payload/Conjunction Spot-Check

Result-bearing assertions assert VALUES, not call success: sens analytic values (`sens.rs:99,104`), PSS `stats.residual < 1e-6` + amplitude + `estimated_settle_time` vs analytic (`pss.rs:87,100`; `pss_host.rs:140`), Trace.i per-step KCL + settled analytic (`session.rs:255-268`), table/clamp exact equilibria, idt phase/slope anchors, ac_stim phasor anchor (1.5, √3/2), fused-digital per-step full state-vector equality. No conjunction-only assertions found in the diff surface.

---

## Edge Cases

- [x] PSS period ≤ 0 loud — `tests/pss.rs:107`
- [~] PSS tstab < 0 loud — code exists (`solver/pss.rs:45`), no test
- [x] table xs/ys length mismatch loud — `table_op.rs:25`
- [~] transition rise/fall = 0 instantaneous+breakpoint — no test found
- [x] tline td ≤ 0 / Z0 ≤ 0 loud — `tline.rs:91,110`
- [x] ngspice absent → SKIP with notice — `ngspice_validation.rs:443` (live here, so golden path also exercised)
- [~] .sens non-addressable output net — `sens.rs:127` docstring claims "digital/pseudo output" case but the test body has only unknown-element + unknown-param asserts

---

## Gate Check

- **Gate command**: `cargo build --workspace` + `cargo test --workspace`
- **Result**: 504 passed, 0 failed, 5 ignored (matches expected ≈504/0/5)
- **Zero-warnings check**: `cargo build --workspace 2>&1 | grep -cE "^warning: unused|^error"` → **0**
- **Skipped tests**: 5 ignored (pre-existing baseline, unchanged by this feature)
- **ngspice**: present (`/usr/bin/ngspice`) — MOS2/MOS3 goldens ran LIVE
- **Failures**: none

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code / surgical changes | ✅ |
| No scope creep | ✅ |
| Matches patterns (fail-loud, MD-13 idiom) | ✅ |
| Spec-anchored outcome check | ❌ SC-02/SC-05 partial branches untested |
| Every test maps to a spec AC | ✅ |
| Documented guidelines followed (AGENTS.md, CLAUDE.md zero-warnings) | ✅ |

---

## Fix Plans

### Fix 1: table slope discrimination (surviving mutant M4)
- **Root cause**: all table test vectors use unit-spaced breakpoints; slope division never changes the result.
- **Fix task**: add a non-uniform xs case (e.g. xs=[0.0, 0.5, 3.0]) asserting interpolated value AND segment-slope Jacobian; kills the Δy·Δx mutant class.
- **Priority**: Major

### Fix 2: sens Rebuild-class loud test (surviving mutant M8, SC-02)
- **Root cause**: `sens_error_paths_are_loud` covers unknown element/param only; the `Invalidation::Rebuild` guard at `solver/sens.rs:59` is untested.
- **Fix task**: request sensitivity on a presence-flipping (Rebuild) param; assert loud error naming the param and "Rebuild"/"restampable".
- **Priority**: Major

### Fix 3: PSS mixed-signal k·T diagnostic test (SC-05 second half, T5 Done-when)
- **Root cause**: `verify_digital_periodicity` (`solver/pss.rs:294`, "circuit period appears to be k·T") shipped with no exercising test.
- **Fix task**: divider-by-2 digital circuit under PSS at T; assert loud `Pss` error naming "period appears to be 2·T" (per T5's original Done-when).
- **Priority**: Major

### Fix 4: minor edge tests
- tstab < 0 loud; transition rise/fall = 0 instantaneous+breakpoint; sens non-analog output (or fix the `sens.rs:127` docstring overpromise).
- **Priority**: Minor

---

## Requirement Traceability Update

| Requirement | Previous | New |
|---|---|---|
| SC-01,03,04,06–15,17–26 | Done | ✅ Verified |
| SC-02, SC-05 | Done | ❌ Needs Fix (untested branches — Fixes 2, 3) |
| SC-16 | Blocked | ⛔ Blocked (external, unchanged) |

---

## Summary

**Overall**: ❌ Not Ready (sensor found 2 surviving mutants; 2 AC branches without evidence)

**Spec-anchored check**: 23/25 ACs matched spec outcome; 2 partial (SC-02, SC-05); 2 spec-precision notes (SC-01 diode FD shape, SC-15 ngspice golden pair absent)
**Sensor**: 8 injected, 6 killed, 2 survived (M4 table slope, M8 sens Rebuild guard)
**Gate**: 504 passed, 0 failed, 5 ignored; 0 warnings; ngspice goldens live

**What works**: every shipped analysis/operator/model behaves per spec — the full suite is green including live ngspice goldens, and 6 targeted mutations across limiters, TR-flux, PSS guard, table clamp, UIC hold, and Trace.i were all caught.

**Issues found**: test-discrimination gaps only (no functional bugs): table slope arithmetic untested (M4), sens Rebuild guard untested (M8 = SC-02), PSS digital k·T diagnostic untested (SC-05), three minor edge-case test absences.

**Next steps**: route Fixes 1–3 (Major) and Fix 4 (Minor) to an implementer; re-verify.

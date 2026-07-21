# p1-solver-complete Validation — Round 2

**Date**: 2026-07-18
**Spec**: `.specs/features/p1-solver-complete/spec.md`
**Round**: 2 of max 3 (round 1: FAIL — 4 ranked gaps; fixes `d3693a9`, `5dc84a9`, `038d5f8`, `4c178b1`)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Gap Closure (round-1 ranked gaps)

| # | Round-1 gap | Fix commit | Verdict | Evidence |
|---|---|---|---|---|
| 1 | M4 — table slope undiscriminated | `038d5f8` | ✅ CLOSED | `sim_dc_table_nonuniform_spacing_slope` (`spec_simulation.rs:1137`) asserts the exact equilibrium V = 22/9 (xs=[0,0.5,3], segment slope 2m/2.5; 1.8·V = 4.4) to 1e-9 — the value only solves with the `Δy/Δx` division. M4 mutant (`Δy/Δx` → `Δy·Δx` at `lower/pom/analog_ops.rs:184`) re-run: **KILLED** — only the new test fails; the two unit-spaced tests still pass (discrimination is exact) |
| 2 | M8 / SC-02 — Rebuild guard unreachable | `d3693a9` | ✅ CLOSED | Root cause fixed, not just tested: `list_params` hardcoded Restamp; descriptor now classifies via `analog.set_flips_presence(name)` (`device/mod.rs:208`) — the *same predicate* as `set_param`'s LIVE-14 write path (`device/mod.rs:232`). `sens_rebuild_class_param_fails_loud` (`tests/sens.rs:194`) trips the guard naming param `w` + "Rebuild"; the given-at-build scalar stays Restamp and solves. Digital-net output loud case ("not a solved analog net", `sens.rs:97`) landed in `sens_error_paths_are_loud:173` — closes the docstring overpromise. Discriminated both ways (sensor table) |
| 3 | SC-05 — PSS digital k·T broken + untested | `5dc84a9` | ✅ CLOSED | Root cause fixed: re-entry restored nets only and `TransientSolver::new` re-ran `init_digital` per shot. `TransientStep` now always carries hidden digital state (module vars + edge memory) via the new `Element::digital_hidden_snapshot/restore` ABI; restore runs after the constructor's init so the restore wins, unknown labels start fresh. `digital_divider_reports_k_times_period` (`tests/pss.rs:185`) fails loud with "appears to be 2·T" + suggested "2.000000e-3". Discriminated both directions (sensor table) |
| 4 | Minor edges (tstab<0, transition rise/fall=0, digital output) | `4c178b1` + above | ✅ CLOSED | `negative_tstab_is_loud` (`tests/pss.rs:149`) names "tstab"; `sim_tran_transition_zero_rise_fall_is_an_instantaneous_step` (`spec_simulation.rs:1247`) asserts rails-only at every recorded step (a ramp would show intermediates), a step landing exactly on the 1e-3 edge (±1e-12), all values finite, flip within 3·dt_max — matches spec outcome (instantaneous = rails only; breakpoint = exact landing) |

**Closure: 4/4.**

---

## Spec-Anchored Acceptance Criteria

| AC | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| SC-01 sens values | ∂V(mid)/∂R2 = 2.5e-3 to 1e-6 rel; diode vs two-sided FD to 1e-3 | `tests/sens.rs:99,104` analytic 1e-6; `:123-124` sign + step-independence 1e-3 | ⚠️ Divider ✅. Diode ⚠️ spec-precision note carried from round 1 (FD step self-consistency, not independent re-solve; documented in b2fdc60) — unchanged |
| SC-02 sens loud errors | Rebuild-class OR nonexistent param → loud error naming param | `tests/sens.rs:194-213` Rebuild-class `w` fails loud naming param + class; given scalar `r` solves; `:142,152` unknown element/param; `:173-186` digital-net output "not a solved analog net" | ✅ PASS (round-1 partial closed; guard now reachable via descriptor fix) |
| SC-03 sens python surface | same values as Rust API | `crates/piperine-python/tests/sens_parity.rs:78-82` `assert_eq!` + analytic anchor | ✅ PASS |
| SC-04 PSS periodic trace | ‖x(T)−x(0)‖ < shoot_tol; phasor within 1% | `tests/pss.rs:87` residual < 1e-6; `:100` amplitude vs analytic | ✅ PASS |
| SC-05 PSS loud non-convergence | `SolverDomain::Pss`, iterations + residual; digital k·T diagnostic | `tests/pss.rs:110` period≤0; `:149` tstab<0; `:126-131` ramp "did not converge"/"does not repeat"; `:185-195` divider "circuit period appears to be 2·T" + suggested period | ✅ PASS (round-1 partial closed — mechanism fixed + tested) |
| SC-06 PSS tstab + rectifier | tstab equivalence; ripple vs settled within 10·reltol | `tests/pss_host.rs:97-109,129,140` | ✅ PASS |
| SC-07 .dc host-proof | sweeps == fresh-build, compile count 1 | `tests/dc_host_proof.rs:110,118,125` | ✅ PASS |
| SC-08 table operator | interp + clamp; Jacobian = segment slope; non-monotonic loud | `spec_simulation.rs:1137` non-uniform slope exact equilibrium 22/9 to 1e-9; `:1105,1131` interp/clamp exact; `table_op.rs:19,25,31` loud paths | ✅ PASS (round-1 surviving mutant M4 now killed) |
| SC-09 transition operator | ramp, breakpoints, edge in trace | `spec_simulation.rs:1202-1209` ramp; `:1247-1305` rise/fall=0 rails-only + exact-edge + finite; `:1433` rejected-step survival | ✅ PASS (edge case now covered) |
| SC-10 idt AC stamp | −20 dB/dec, −90° ×4 decades | `spec_simulation.rs:1377,1382,1388` | ✅ PASS |
| SC-11 multiple ac_stim | phasor superposition | `spec_simulation.rs:1325-1329` | ✅ PASS |
| SC-12 @initial + UIC | t=0 = ic; RC decay | `spec_simulation.rs:1252,1259` | ✅ PASS |
| SC-13 digital fused + identical | fused proven active, bit-identical | `digital_fusion.rs:110-121,57-70` | ✅ PASS |
| SC-14 MOS2+MOS3 parity | ngspice goldens, live-or-SKIP | `ngspice_validation.rs:304-321,381-424` | ✅ PASS (ngspice 44 live in gate) |
| SC-15 tline ideal | matched/open/bad-params | `tests/tline.rs:65-68,84-86,91,110` | ⚠️ PASS — ngspice `.cir` golden pair still absent (spec-precision note, carried) |
| SC-16 urc | — | — | ⛔ BLOCKED (external dep, out of scope) |
| SC-17 transformer | AC ratio; energy transfer | `tests/xfmr.rs:45-48,122-139` | ✅ PASS |
| SC-18 stdlib off sentinels | no `1e99`/`$param_given` in models | grep-clean; suites green | ✅ PASS (stale doc comment nit carried) |
| SC-19 fetlim/limvds | ngspice reference values | `limiters.rs:95,120` | ✅ PASS |
| SC-20 temperature | tnom rescaling; −2 mV/°C | `temp_sweep.rs:72,77-80` | ✅ PASS |
| SC-21 inductor TR dual | dual form; coupled unchanged | `xfmr.rs:136-139`; `live_params.rs:411` | ✅ PASS |
| SC-22 IntegrationMethod removal | grep-clean | zero matches | ✅ PASS |
| SC-23 scheduler split + SignalBridge | behavior unchanged | `digital/{topology,state,scheduler}.rs`; `core/circuit.rs:23` | ✅ PASS |
| SC-24 as_iv + Integrator | identical numerics | `analog/netlist.rs:228`; `math/integration.rs:167` | ✅ PASS |
| SC-25 Trace.i state recording | opt-in current; off → loud | `tests/session.rs:239,274` | ✅ PASS |
| SC-26 init_global ownership | no global init on default | `context_init.rs:12-31` | ✅ PASS |

**Status**: ✅ 24/25 matched (2 spec-precision notes carried: SC-01 diode FD shape, SC-15 ngspice golden pair); SC-16 blocked out-of-scope. Zero partials.

---

## Discrimination Sensor

| # | Mutation | File:line | Result |
|---|----------|-----------|--------|
| M1–M3, M5–M7 | round-1 killed mutants (limiters, TR-flux, PSS guard, table clamp, UIC hold, Trace.i) | — | ✅ Still killed (discriminating tests green in the 509 gate) |
| M4 (re-run) | table slope `(Δy)/(Δx)` → `Δy·Δx` | `lower/pom/analog_ops.rs:184` | ✅ **KILLED** — `sim_dc_table_nonuniform_spacing_slope` fails; the two unit-spaced table tests still pass (exact discrimination) |
| M8 (re-run) | sens Rebuild guard off (`false &&`) | `solver/sens.rs:59` | ✅ **KILLED** — `sens_rebuild_class_param_fails_loud` fails |
| M9 (new) | descriptor scratch-reverted to always-Restamp (pre-fix behavior) | `device/mod.rs:208` | ✅ **KILLED** — `sens_rebuild_class_param_fails_loud` fails (proves the test exercises the new classification, not just the guard) |
| M10 (new) | `digital_hidden_restore` call removed from full-state re-entry | `solver/transient.rs:280-285` | ✅ **KILLED** — divider test fails with the exact expected signature: "digital state is not periodic at T (and does not close within 4·T)" (register state reset per shot ⇒ no closure) |
| M11 (new) | `verify_digital_periodicity` neutered (early `return Ok(())`) | `solver/pss.rs:272` | ✅ **KILLED** — divider test fails at `expect_err` (shooting returns Ok); other pss tests unaffected |

**Sensor depth**: 6 mutations re-run/injected this round
**Result**: 6/6 killed — PASS

---

## New-ABI Spot-Check (MD-13 contract coherence)

- `Element::digital_hidden_snapshot/restore` (`core/element.rs:366-376`): trait methods with documented defaults (`None` = stateless combinational); contract states *what* (register state round-trips with digital nets on full-state re-entry), not how. No loose functions, no macros. ✅
- Always-on vs opt-in separation: `snapshot()` (`solver/transient.rs:717-723`) applies `with_digital_hidden` **before** the `if !record_device_state { return }` gate — hidden digital state is always recorded (full-state contract), device banks stay opt-in. Distinct fields (`digital_hidden` vs `device_state`), distinct collectors (`collect_digital_hidden` vs `collect_device_banks`). No leak into the opt-in path. ✅
- Restore ordering documented: restored after the constructor's `init_digital` so the snapshot wins over power-on reset; unknown labels (rebuilt circuits) skipped → start fresh (`transient.rs:275-285`). `DigitalInstance::hidden_restore` splits the int carrier by current kernel-fixed layout with a length guard. ✅

---

## Payload/Conjunction Spot-Check

New assertions assert VALUES/outcomes: exact equilibrium 22/9 to 1e-9 (table slope), error-message content naming param + invalidation class (sens Rebuild), k=2 and the suggested period 2.000000e-3 (PSS divider), rails-only membership at every recorded step + exact-edge landing ±1e-12 (transition zero). No conjunction-only assertions introduced.

---

## Edge Cases

- [x] PSS period ≤ 0 loud — `tests/pss.rs:110`
- [x] PSS tstab < 0 loud — `tests/pss.rs:149` (round-1 gap closed)
- [x] table xs/ys length mismatch loud — `table_op.rs:25`
- [x] transition rise/fall = 0 instantaneous+breakpoint — `spec_simulation.rs:1247` (round-1 gap closed)
- [x] tline td ≤ 0 / Z0 ≤ 0 loud — `tline.rs:91,110`
- [x] ngspice absent → SKIP with notice — `ngspice_validation.rs:443` (live here)
- [x] .sens non-addressable output net — `tests/sens.rs:173-186` (round-1 docstring overpromise closed)

---

## Gate Check

- **Gate command**: `cargo build --workspace` + `cargo test --workspace`
- **Result**: **509 passed, 0 failed, 5 ignored** (round-1 baseline 504 + exactly the 5 new test fns: table-slope, sens-Rebuild, tstab, divider-k·T, transition-zero)
- **Zero-warnings check**: `cargo build --workspace 2>&1 | grep -cE "^warning: unused|^error"` → **0**
- **ngspice**: present — MOS2/MOS3 goldens ran LIVE
- **Failures**: none

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code / surgical changes | ✅ |
| No scope creep | ✅ |
| Matches patterns (fail-loud, MD-13 idiom) | ✅ |
| Spec-anchored outcome check | ✅ all branches evidenced |
| Every test maps to a spec AC | ✅ |
| Documented guidelines followed | ✅ |

---

## Requirement Traceability (report-only; orchestrator updates spec.md)

| Requirement | Round-1 | Round-2 |
|---|---|---|
| SC-01,03,04,06–15,17–26 | ✅ Verified | ✅ Verified (unchanged) |
| SC-02, SC-05 | ❌ Needs Fix | ✅ **Verified** |
| SC-16 | ⛔ Blocked | ⛔ Blocked (external, unchanged) |

---

## Summary

**Overall**: ✅ Ready (PASS)

**Gap closure**: 4/4 — table slope discriminated (M4 killed), sens Rebuild guard reachable + tested via descriptor fix (M8 + M9 killed), PSS digital k·T mechanism fixed + tested (M10 + M11 killed), minor edges covered.

**Spec-anchored check**: 24/25 ACs matched (2 carried spec-precision notes: SC-01 diode FD reference shape, SC-15 missing ngspice golden pair); SC-16 blocked out-of-scope.

**Sensor**: 6 mutations this round (2 re-run survivors + 1 re-run killed + 3 new), 6/6 killed.

**Gate**: 509 passed, 0 failed, 5 ignored; 0 warnings; ngspice goldens live.

**What changed since round 1**: two root-cause fixes (descriptor classification mirrors LIVE-14; PSS shots round-trip hidden digital state via a new `Element` ABI pair) plus five discriminating tests. The new ABI is contract-coherent and keeps the always-on full-state recording separate from the opt-in `record_device_state` path.

**Residual notes** (non-blocking, carried from round 1): SC-01 diode FD reference is step-self-consistency rather than an independent re-solve; SC-15 has no ngspice `.cir` golden pair; one stale `$param_given` doc comment in `constants.phdl:18`.

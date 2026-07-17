# spice-stdlib Validation

**Date**: 2026-07-16
**Spec**: `.specs/features/spice-stdlib/spec.md`
**Diff range**: `f53398a..d92fbbb` (branch `feature/spice-stdlib`, 14 commits; batch 1 `1492a04…9653b3f`, batch 2 `271454b…d92fbbb`)
**Verifier**: independent sub-agent (author ≠ verifier), evidence re-derived from scratch; ngspice-46 live at `/usr/bin/ngspice`

---

## Task Completion

| Task | Status | Notes |
|---|---|---|
| T1 headers/spice + namespace | ✅ Done | 10 `.phdl` in `crates/piperine-lang/headers/spice/`; namespace in both source maps |
| T2 smoke tests | ✅ Done | `spice_smoke.rs` (junction + validate), both pass |
| T3 shadowing | ✅ Done | insert-if-absent in `piperine-project/src/source_map.rs:60` + test |
| T4 docs + retirement | ✅ Done | CLAUDE.md:62, README.md:49, `docs/spec/part_v_builtins.md:229`; external repo README deprecated (verified on disk); stale fork absent on disk |
| T5 harness + OP circuits | ✅ Done | `NgspiceHarness`, 8 OP circuit pairs, all failure modes tested |
| T6 sweep (wrdata CSV) | ✅ Done | `diode_iv` 37 pts, strict parser + parser tests |
| T7 MOS1 fix | ✅ Done | `nmos_load`/`nmos_fixed` pass; 2 sweeps (21+26 pts) pass |
| T8 JFET fix | ✅ Done | `jfet_bias` + `jfet_id_vds` (26 pts) pass |
| T9 source stepping | ✅ Done | 4 unit tests (ramp/back-off/give-up/warm-start); `bjt_ce` passes |
| T10 BJT mirror | ✅ Done | `bjt_mirror` passes |
| T11 full green, zero ignores | ✅ Done | zero `#[ignore]` in harness (grep); the 5 workspace-wide ignores are pre-existing `piperine-codegen/tests/ppr_ir.rs` (untouched by diff) |
| T12 compile-once sweeps (MD-18) | ✅ Done | `compile_once_sweep.rs` own binary, compile-count delta assertion |

---

## Spec-Anchored Acceptance Criteria

All ngspice golden values were independently re-derived by running `/usr/bin/ngspice -b` on the checked-in `.cir` fixtures: `nmos_load` v(d)=3.000000, `bjt_ce` v(col)=1.112314e-01 / v(base)=8.159876e-01, `jfet_bias` v(d)=1.381966, `bjt_mirror` v(ref)=7.709873e-01 / v(out)=8.539091e-01. Harness run with `--nocapture` shows all 12 circuit PASS lines live (19/19 tests in `ngspice_validation`).

| AC | Spec-defined outcome | Evidence (`file:line` + assertion) | Result |
|---|---|---|---|
| SPICE-01 use spice:: resolves from headers/spice | elaboration succeeds via builtin path, no Piperine.toml | `crates/piperine-lang/tests/spice_headers.rs:10-26` — `parse_and_elaborate(src, &SourceMap::dummy()).expect(…)` + `design.module("Top").is_some()`; project path: `crates/piperine-project/src/source_map.rs:93-115` — `map.namespaces.get("spice")` + full elaboration through `project_source_map` | ✅ PASS |
| SPICE-02 every headers/spice file elaborates | no parse/elab errors, all files | `crates/piperine-lang/tests/spice_headers.rs:30-49` — `assert_eq!(files.len(), 10)` + per-file `parse_and_elaborate` with `failures.is_empty()` | ✅ PASS |
| SPICE-03 migrated smoke tests pass | junction + validate benches pass | `crates/piperine-bench/tests/spice_smoke.rs:46-54` — `spice_junction_devices_converge`, `spice_validate_smoke_passes`; `run_fixture` asserts non-empty results + `failures.is_empty()` (:30,:40) | ✅ PASS |
| SPICE-04 project `spice` shadows builtin | project `src/` wins over headers | `crates/piperine-project/src/source_map.rs:120-134` — `assert_eq!(spice_dir, &src_dir, "project `spice` package must win…")`; branch at `source_map.rs:60` (`!namespaces.contains_key("spice")`) | ✅ PASS |
| SPICE-05 each circuit pair compared per node | `\|Δ\| ≤ 1e-6 + 1e-3·max(\|a\|,\|b\|)` for every shared node | `ngspice_validation.rs:52-54` (formula), `:124-154` (`compare_op` every golden node), `:279-317` (8 OP `#[test]`s); formula itself pinned by `ngspice_tolerance_contract` `:422-430` with boundary cases | ✅ PASS |
| SPICE-06 no ngspice → skip + pass | explicit skip message, test passes | `ngspice_validation.rs:376-384` — `detect_with_path(empty).is_none()` + `detect_with_path(None).is_none()`; skip arm prints `SKIP {circuit}: ngspice not on PATH` and passes (`:270-275`). Note: skip verified via the injectable PATH seam, not an end-to-end PATH-less run (process-global PATH mutation avoided by design) | ✅ PASS |
| SPICE-07 mismatch names circuit/node/values/Δ | loud failure with all four | `ngspice_validation.rs:410-418` — asserts err contains `"offby"`, `"v(out)"`, `"3.0"`+`"1.92"`, `"Δ="` | ✅ PASS |
| SPICE-08 sweeps via wrdata CSV point-by-point | same tolerance, per point | `ngspice_validation.rs:172-193` (wrdata export), `:197-219` (strict parser), `:226-266` (`sweep_case` per-point compare); parser edge cases `:435-449`; `ngspice_diode_iv_sweep` `:325-332` (37 pts live, ≥20 asserted `:228-232`) | ✅ PASS |
| SPICE-09 nmos_load v(d)=3.0 V ± tol | 3.0 V within reltol 1e-3/abstol 1e-6 | `ngspice_validation.rs:300-302` (`ngspice_nmos_load`) via `op_case` compare vs live golden; ngspice independently prints `v(d) = 3.000000e+00` | ✅ PASS |
| SPICE-10 NMOS Id–Vgs/Id–Vds sweeps ≥10 pts, lin+sat | every point within reltol 1e-3 + abstol 1e-9 A | `ngspice_validation.rs:338-345` (Id–Vgs, 21 pts: cutoff→sat→linear, body effect) and `:350-357` (Id–Vds, 26 pts: linear→sat, rd/rs=100Ω); `ABSTOL_I = 1e-9` `:34` | ✅ PASS |
| SPICE-11 jfet_bias within tol | 1.381966 V golden | `ngspice_validation.rs:305-307` + `ngspice_jfet_id_vds_sweep` `:362-369` (26 pts) | ✅ PASS |
| SPICE-12 bjt_ce converges to saturated point | Vce ≈ 0.11 V (golden 0.1112 V), not the KCL-violating active point | `ngspice_validation.rs:310-312` — compare vs live golden v(col)=1.112314e-01 within 1e-6+1e-3·max; an active-region answer (~ volts) is far outside tolerance | ✅ PASS |
| SPICE-13 bjt_mirror converges + matches | v(ref)=0.7710, v(out)=0.8539 | `ngspice_validation.rs:315-317` — compare vs live golden, both nodes | ✅ PASS |
| SPICE-14 retirement, no path references | piperine references neither external path; external repo deprecated | grep over repo: zero hits for `Git/piperine-spice` / `plugins/piperine-spice` local paths outside `.specs/` (historical `piperine-spice` package-name mentions in ROADMAP_REFINEMENT.md / resolver test fixtures are name/URL references, not the retired paths); `~/Git/plugins/piperine-spice/README.md` carries the DEPRECATED 2026-07-16 notice (verified on disk); stale fork `~/Git/piperine-spice` absent on disk | ✅ PASS |
| SPICE-15 docs describe builtin stdlib | CLAUDE.md/README/spec | `CLAUDE.md:62` (crate table), `README.md:49-53`, `docs/spec/part_v_builtins.md:229-244` (`### The spice namespace`, full module list) | ✅ PASS |
| MD-18 / T12 single JIT across sweep | one build's worth of kernel compiles for an N-point sweep; results match staged path | `crates/piperine-bench/tests/compile_once_sweep.rs:67-71` — `assert_eq!(sweep_compiles, per_build, "MD-18: …")` with `AnalogKernel::compile_count()` deltas; per-point equivalence `:73-79` (`\|i−r\| ≤ 1e-9 + 1e-3·max`); loud unknown-label/param errors `:85-93`; isolated test binary so counts aren't polluted | ✅ PASS |

**Status**: ✅ 16/16 covered (15 SPICE ACs + MD-18), 0 gaps.

### Edge cases (spec)

- [x] `Real?` optional params through builtin path — covered by `spice_junction_devices_converge` (junction.phdl exercises the migrated optional-param models via `use spice::…`).
- [x] Unparseable ngspice output fails loud with raw excerpt — `ngspice_validation.rs:389-394`.
- [x] 0 shared nodes = contract violation — `ngspice_validation.rs:399-405`.
- [~] Non-convergence names circuit + solver error — implemented (`ngspice_validation.rs:118-119`, `"{circuit}: piperine DC solve failed: {e}"`) but no test exercises this error path (no intentionally non-converging fixture). Minor untested-error-path note; not an AC.

### Additional regression guard (unclaimed-test check)

`ngspice_series_junctions_are_self_consistent` (`ngspice_validation.rs:456-468`) maps to T5's per-variable bypass fix — claimed. Solver unit tests `source_stepping_{ramp_schedule,backs_off_after_failure,gives_up_when_backoff_exhausts,warm_start_chain}` (`convergence.rs:504-582`) map to T9 Done-when. No unclaimed tests found in the diff surface.

---

## Discrimination Sensor

Scratch-state workflow: edit → run targeted tests → `git checkout -- <file>` → tree verified clean after each. Real tree never committed to a mutated state; `git status` clean at end.

| # | Mutation | File:line | Description | Killed? |
|---|---|---|---|---|
| 1 | Tolerance formula | `ngspice_validation.rs:53` | `max(\|a\|,\|b\|)` → `(\|a\|+\|b\|)` (looser) | ✅ Killed — `ngspice_tolerance_contract` FAILED |
| 2 | MOS1 saturation current | `headers/spice/mos.phdl:256` | `betap·vgst²·0.5` → `·0.75` | ✅ Killed — 4 tests FAILED (`nmos_load`, `nmos_fixed`, both sweeps) |
| 3 | DC bypass threshold | `solver/dc.rs:73` | per-variable scale → floored at global 5 V (re-opens freeze window) | ✅ Killed — `ngspice_bjt_ce` + `ngspice_diode_iv_sweep` FAILED |
| 4 | −R branch-current stamp | `codegen/device/analog.rs:524` | `-r * sign` → `r * sign` | ✅ Killed — `ngspice_nmos_id_vds_sweep` + `ngspice_jfet_id_vds_sweep` FAILED |
| 5 | srcfact ramp exactness | `solver/convergence.rs:458` | final `set_gmin_extra(0.0)` → `(1e-9)` (residual shunt in the "exact" solve) | ✅ Killed — `source_stepping_ramp_schedule` + `source_stepping_backs_off_after_failure` FAILED |

**Sensor depth**: 5 behavior-level mutations (P0-adjacent correctness paths)
**Result**: 5/5 killed — PASS ✅

---

## Gate Check

- **Build**: `cargo build --workspace` after touching every crate root — 0 warnings, exit 0. ✅
- **Test**: `cargo test --workspace` — **445 passed, 0 failed, 5 ignored**. ✅
- Ignored: all 5 pre-existing (`piperine-codegen/tests/ppr_ir.rs` "pending POM Stmt rewrite" + siblings), outside the diff surface; zero `#[ignore]` in the harness (grep-verified).
- Test count: baseline 391 (tasks.md) → 445 (**+54**); matches tasks.md batch record (432 after batch 1, 445 after batch 2). No deleted tests observed in the diff.
- Live harness: `cargo test -p piperine-bench --test ngspice_validation -- --nocapture` → 19/19, PASS lines for all 12 circuits (8 OP + 4 sweeps: 37/21/26/26 points).

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code / no scope creep | ✅ — `current_terms` mirrors existing `flux_terms` machinery; fail-loud guard for non-forced-branch probes (`flatten.rs`, GAPS-conformant) |
| Surgical changes | ✅ — diff confined to feature surface + T11 warning fix (`python_setup.rs` cfg-split, sanctioned by tasks.md) |
| Matches patterns | ✅ — harness follows bench e2e `elab` pattern; homotopy tests use scripted-driver seam per MD-05 |
| Spec-anchored outcomes | ✅ — golden values re-derived from live ngspice, tolerance formula pinned by its own boundary test |
| Per-layer coverage expectation (tasks.md matrix) | ✅ — every matrix row has tests at the stated location |
| No unclaimed tests | ✅ |
| Project guidelines followed | ✅ — `CLAUDE.md` zero-warnings bar + `--workspace` rule honored |

---

## Requirement Traceability Update

SPICE-01..15: Pending/Implementing → ✅ Verified (all 15). MD-18 (T12): ✅ Verified.

---

## Summary

**Overall**: ✅ Ready

**Spec-anchored check**: 16/16 ACs matched spec outcomes (15 SPICE + MD-18), 0 spec-precision gaps
**Sensor**: 5/5 mutations killed
**Gate**: 445 passed, 0 failed, zero warnings

**What works**: builtin `use spice::` (both source maps, shadowing), 8 OP + 4 sweep golden circuits green vs live ngspice-46, all four transistor-correctness fixes land at the golden values, source-stepping homotopy unit-tested, compile-once sweeps enforced by compile-counter.

**Notes (non-blocking)**:
1. SPICE-06 skip path verified via the injectable-PATH seam rather than an end-to-end PATH-less run — acceptable design (avoids process-global env mutation), the skip arm itself is trivial.
2. The piperine non-convergence error path (`ngspice_validation.rs:118`) has no exercising test — minor, not an AC.

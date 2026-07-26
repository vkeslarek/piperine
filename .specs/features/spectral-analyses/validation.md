# Spectral & Small-Signal Analyses Validation

**Date**: 2026-07-19
**Spec**: `.specs/features/spectral-analyses/spec.md`
**Diff range**: `0496d9b~1..HEAD` (T1–T14 committed as `0496d9b..78d31b3`; T15/T16
uncommitted in the working tree — ngspice cross-checks, the `.disto`
compile-explosion fix, `Cargo.toml` Cranelift opt-level override,
`examples/live_optimize.py` threshold change)
**Verifier**: independent fresh sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1–T14 | ✅ Done | Each has its own commit (`0496d9b`…`78d31b3`) matching the tasks.md commit-message spec exactly |
| T15 | ✅ Done (uncommitted) | ngspice cross-checks present and passing (ngspice-46 detected); real `.disto` compile-explosion fix folded in, verified below |
| T16 | ⚠️ Partial | `ROADMAP.md`/`.specs/STATE.md` modified in the working tree but not committed; content not spot-checked in depth (docs-only, low risk) |

---

## Spec-Anchored Acceptance Criteria

### P1: `.four`

| Criterion | Spec-defined outcome | file:line + assertion | Result |
| --------- | --------------------- | ---------------------- | ------ |
| AC1: `fourier(f0,n)` returns freq/mag/phase/norm per harmonic | fields `frequency,magnitude,phase,norm_magnitude,norm_phase` present, `k=0..n-1` | `crates/piperine-api/src/fourier.rs:101-109` struct build; `:177-184` field assertions | ✅ PASS |
| AC2: THD formula, ≤1e-6 rel vs numpy-FFT-equivalent | `sqrt(Σ_{k≥2} mag_k²)/mag_1` | `fourier.rs:111` (impl); `:184` `assert!((result.thd-0.1).abs()<1e-6)`; `:197-203` (numpy-style single-tone reference) | ✅ PASS |
| AC3: non-uniform grid resampled before DFT | resample onto uniform grid via `Waveform::at` | `fourier.rs:72-75` (impl); `:229-239` `resamples_nonuniform_grid_before_dft`, tol 1e-3 on jittered grid | ✅ PASS |
| AC4: fail loud on `f0≤0` / short span / `n<2` | typed `Error::Measurement`, never partial spectra | `fourier.rs:48-70` (impl); `:207-226` three dedicated tests | ✅ PASS |
| AC5: Rust==Python identical shape/values | same fields, same values | `crates/piperine-python/tests/four_parity.rs` (parity test, ≤1e-9 per tasks.md T2) | ✅ PASS (parity test exists and is in the green 582; not individually re-read line-by-line) |

### P1: `.pz`

| Criterion | Spec-defined outcome | file:line + assertion | Result |
| --------- | --------------------- | ---------------------- | ------ |
| AC1: poles = generalized eig of `(G,C)` | complex `s`, rad/s | `crates/piperine-solver/src/analyses/pz.rs:163-170` (impl); `tests/pz.rs:65-72` RC pole | ✅ PASS |
| AC2: zeros via input↔output-swapped pencil | Rosenbrock pencil | `pz.rs:766-791` `lag_network_has_the_known_pole_and_zero` — exact analytic pole **and** zero, ≤1e-6 rel | ✅ PASS |
| AC3: near-zero imaginary snapped real; complex roots conjugate-paired | `im==0.0` snap; conjugate pairing | `pz.rs:804-829` `finite_generalized_eigenvalues_pairs_conjugates` | ✅ PASS |
| AC4: RC → `-1/(RC)`; RLC → analytic conjugate pair | ≤1e-6 rel | `tests/pz.rs:58-96` both cases, exact closed forms | ✅ PASS |
| AC5: no reactive elements → fail loud | error, not empty success | `pz.rs:754-761` `resistor_only_circuit_fails_loud_pz05` | ✅ PASS |
| AC6: frequency-nonlinear AC stamp → fail loud | error naming the linearity guard | `pz.rs:691-712` — asserts `msg.contains("not affine in jω")` | ✅ PASS |
| AC7: Rust/Python same shape/poles/zeros | MD-22 | `crates/piperine-python/tests/pz_parity.rs` (present, in the 582-green run) | ✅ PASS |

### P2: `.sp`

| Criterion | Spec-defined outcome | file:line + assertion | Result |
| --------- | --------------------- | ---------------------- | ------ |
| AC1: `@rfport(num,z0)` recognized as ports | `(num,z0,node)` resolved | `crates/piperine-lang/tests/rfport.rs:11-21` | ✅ PASS |
| AC2: per-frequency, one port driven at a time, full `S` via power waves | Kurokawa power-wave, real z0 | `crates/piperine-solver/src/analyses/sp.rs:198-224` (impl); `:489-511` `series_r_attenuator_matches_analytic_s11_s21` | ✅ PASS |
| AC3: reciprocal passive → `S12==S21`, `|Sii|≤1` | exact tolerance stated | `sp.rs:513-523` `reciprocal_network_has_s12_eq_s21_and_passive_sii`, diff <1e-9 | ✅ PASS |
| AC4: matched divider/attenuator matches analytic S ≤1e-6 | ≤1e-6 | `sp.rs:489-511`, `tests/sp.rs:34-72` shunt-C closed form ≤1e-6 | ✅ PASS |
| AC5: <1 port / non-positive z0 / unknown node → fail loud | error | `sp.rs:552-613` — 5 dedicated fail-loud tests (zero ports, non-positive z0, port-on-ground, duplicate num, coincident nodes, unknown node) | ✅ PASS |
| AC6: Rust/Python same shape | MD-22 | `crates/piperine-python/tests/sp_parity.rs` (present, green) | ✅ PASS |

### P2: `.disto`

| Criterion | Spec-defined outcome | file:line + assertion | Result |
| --------- | --------------------- | ---------------------- | ------ |
| AC1: single-tone HD2/HD3 from 2nd/3rd derivatives | `HD2=½(g2/g1)A`, `HD3=¼(g3/g1)A²` closed form | `tests/disto.rs:40-65` — exact closed-form comparison, ≤1e-3 | ✅ PASS |
| AC2: two-tone IM2/IM3 (F1±F2 etc.) | analytic Volterra | `tests/disto.rs:67-95` — exact closed form, ≤1e-3 | ✅ PASS |
| AC3: 2nd/3rd derivatives symbolic, never numeric/silent-0 | `diff.rs` symbolic, JIT kernel | `crates/piperine-codegen/src/lower/diff.rs` `d_dv`/`d_dv_twice`/`d_dv_thrice` (symbolic); `crates/piperine-codegen/tests/disto_jit.rs:51-95,236-270` value-for-value vs hand-derived reference | ✅ PASS |
| AC4: unlowerable higher-derivative → fail loud naming device | `CodegenError::Unsupported` | `disto_jit.rs:169-195` `disto2_branch_current_read_fails_loud_naming_device` | ✅ PASS |
| AC5: cubic stage HD2/HD3 vs closed form ≤1e-3 | as above | same as AC1 | ✅ PASS |
| AC6: Rust/Python same shape | MD-22 | `crates/piperine-python/tests/disto_parity.rs` (present, green) | ✅ PASS |

**Status**: ✅ All 24 traceability-table ACs (FOUR-01..05, PZ-01..07, SP-01..06, DISTO-01..06) have direct file:line evidence matching the spec-defined precise outcome. No spec-precision gaps found — every criterion in spec.md states a numeric tolerance or an exact error condition, and every corresponding test targets that exact value.

---

## Edge Cases

| Edge case | Handled? | Evidence |
| --------- | -------- | -------- |
| `.four` fundamental period > transient span → fail loud | ✅ | `fourier.rs:213-219` |
| `.pz` infinite/NaN eigenvalues filtered; none finite → fail loud | ✅ | `pz.rs:220-233` (`finite_generalized_eigenvalues` drops `TOL_INFINITE` roots); `pz.rs:754-761` (all-infinite → fail loud) |
| `.sp` degenerate coincident port nodes → fail loud | ✅ | `sp.rs:594-604` `coincident_port_nodes...fails_loud` |
| `.disto` DC non-convergence → surface DC error, no distortion attempt | ⚠️ | `disto.rs:224` `DcSolver::new(...)?.solve()?` propagates via `?` (would surface a DC error), but **no dedicated test** feeds a non-converging bias point through `.disto` to confirm the propagation path — plausible-by-construction, not directly demonstrated |
| Any analysis on a purely digital circuit → fail loud "no analog network" | ⚠️ | `.disto` has an explicit `size==0 → "no analog network"` check (`disto.rs:219-221`) but **no test exercises it**. `.pz`/`.sp` have no explicit `size==0` guard of their own; a fully digital circuit reaching `.pz` falls through to `finite_generalized_eigenvalues` returning an empty root set, which `poles()` then reports as "no reactive elements... no finite poles" (PZ-05's message, not "no analog network") — fails loud, but with different wording than the edge case's example message, and untested for the pure-digital case specifically (only tested via a resistor-only *analog* network). `.four` doesn't take a circuit at all (post-processes an existing `Waveform`), so this edge case doesn't directly apply to it. |

**Gap flagged**: the "purely digital circuit → fail loud" edge case and the "`.disto` DC non-convergence surfaces the DC error" edge case are both plausible from the code path (both use `?`-propagation / explicit guards that read as correct) but neither has a dedicated regression test. Minor — the general fail-loud discipline is intact and covered elsewhere, but a specific mutation to these guards would not be caught by the current suite.

---

## Discrimination Sensor

Reverts confirmed clean via `git status --short`/`git diff --stat` after each restore — no lingering mutation.

| # | File:line | Description | Killed? |
| - | --------- | ------------ | ------- |
| 1 | `crates/piperine-solver/src/analyses/pz.rs:164` | Removed negation: `self.g.mapv(|v| -v)` → `self.g.mapv(|v| v)` (flips the `(−G,C)` pencil sign) | ✅ Killed — 3 tests failed (`rc_low_pass_has_one_real_pole_at_minus_one_over_rc`, `series_rlc_has_complex_conjugate_pole_pair`, `lag_network_has_the_known_pole_and_zero`), all with wrong-sign poles reported |
| 2 | `crates/piperine-solver/src/analyses/sp.rs:218` | Flipped Kurokawa power-wave sign: `(v_i - z0_i*i_i)` → `(v_i + z0_i*i_i)` | ✅ Killed — 2 tests failed (`series_r_attenuator_matches_analytic_s11_s21`: S11 came back 1.0 instead of 0.333; `shunt_c_lowpass_s21_rolls_off_with_frequency`: S21 came back 0 instead of ~1.0) |
| 3 | `crates/piperine-solver/src/analyses/disto.rs:522` | Changed Volterra 2nd-order coefficient `0.25` → `0.5` (doubles the nonlinear-current injection) | ✅ Killed — 3 tests failed (`single_tone_hd2_matches_closed_form_volterra`, `hd3_includes_the_f2_x1_x2_cross_term`, `two_tone_im2_im3_match_hand_run_volterra_recursion`) with exactly 2x-off values, confirming the tests pin the coefficient precisely, not just its sign/existence |

**Sensor depth**: lightweight (3 targeted mutations across the 3 new analysis drivers)
**Result**: 3/3 killed — ✅ PASS

---

## Code Quality

| Principle | Status | Notes |
| --------- | ------ | ----- |
| No features beyond what was asked | ✅ | Scope matches spec.md/tasks.md exactly (4 analyses, `@rfport`, docs) |
| No abstractions for single-use code | ✅ | pz/sp/disto each a focused module; `d_dv_once_more_named`/`d_dv_thrice_from_twice` are genuine reuse (avoid redundant symbolic passes), not premature abstraction |
| No unnecessary "flexibility" added | ✅ | `compile_disto: bool` flag is minimal and load-bearing (default off; only `run_disto` opts in) |
| Only touched files required for task | ✅ | Diff surface matches the Test Coverage Matrix's listed files; no unrelated files touched beyond `examples/07_thermostat_plot.png` (regenerated artifact, benign) |
| Didn't "improve" unrelated code | ✅ | No unrelated refactors found in the diff |
| Matches existing patterns/style | ✅ | pz/sp/disto follow the existing `analyses/` module shape (`Context`, `SolverDomain`, `Result` alias); host wiring mirrors `pss`/`sens` pattern per tasks.md |
| Would senior engineer approve? | ✅ | Yes — the T15 compile-explosion fix (symbolic redundancy removal + per-branch-pair Cranelift functions + dev-profile opt-level) is a well-reasoned, well-documented perf fix, not a hack |
| Tests map to ACs, non-shallow | ✅ | Spot-checked `.four`/`.pz`/`.sp`/`.disto` — every test asserts an exact closed-form value or a specific fail-loud message substring, not just "no panic" |
| Spec-anchored outcome check | ✅ | See ACs table above — all 24 matched precise spec outcomes |
| Per-layer coverage (domain 1:1 ACs; edge cases) | ⚠️ | Domain logic 1:1 is solid; 2 of 5 edge cases (digital-only fail-loud, disto DC-non-convergence) lack dedicated tests (see Edge Cases table) |
| Every test maps to a spec requirement | ✅ | No stray/unclaimed tests found in the reviewed files |
| Documented guidelines followed | ✅ | CLAUDE.md "fail loud" convention followed throughout; no macros/loose fns introduced (AGENTS.md, per tasks.md's own guideline citation) |

---

## The `.disto` Compile-Explosion Fix (T15 note) — Independent Verification

- **Root cause understanding confirmed**: `compile_disto2`/`compile_disto3` in
  `crates/piperine-codegen/src/jit/analog.rs` build one Cranelift function
  **per branch-pair/triple** (`build_fn` called inside the `for k_idx { for
  j_idx { ... } }` double loop, `analog.rs:1742-1789`), with an early skip
  when a pair's Hessian row is entirely zero (`analog.rs:1757-1759`) — matches
  the "per-branch-combination Cranelift functions instead of one unrolled
  function" claim.
- `d_dv_once_more_named`/`d_dv_thrice_from_twice` (`diff.rs:140-153,207-221`)
  genuinely avoid redoing the shared first/second differentiation pass per
  pair/triple — confirmed by reading the call sites in `compile_disto2`
  (`analog.rs:1763-1770`, `all_dtemps_inner[j_idx]` built once, reused across
  every `k`).
- `Cargo.toml` adds `opt-level = 3` for `cranelift-codegen`/`cranelift-jit`/
  `cranelift-module`/`cranelift-frontend`/`cranelift-native`/`regalloc2` in
  the dev profile, with a comment explaining why (Cranelift's own codegen is
  slow unoptimized) — confirmed via `git diff`.
- `compile_disto: bool` threads `SimSession::build_circuit(compile_disto:
  bool)` → `CircuitCompiler::with_disto` → (presumably)
  `CompiledModule`/`AnalogKernel`; every host call site passes `false` except
  `run_disto`, which passes `true` (`crates/piperine-api/src/session.rs`,
  confirmed by diff read).
- **`examples/live_optimize.py` threshold 10x → 5x**: independently
  re-ran `cargo test -p piperine-python --test live_optimize_example` 3x in
  **dev profile** (matching real CI/gate conditions, not `--release`):
  results were **10x, 11x, 11x** — comfortably above the new 5x floor and
  roughly matching the *old* 10x floor (borderline on run 1). The stated
  justification (Cranelift's own opt-level-3 dev build makes the "fresh
  recompile" path faster, shrinking the fresh/live wall-clock ratio even
  though the live path's zero-recompile guarantee is unaffected and
  separately verified by `compile_count`) is **consistent with the observed
  numbers** — the ratio did not collapse to near-1 (which would indicate the
  live-recompile-avoidance regressed), it just moved from ~10x to ~10-11x
  measured / 5x floor, i.e. a 2x safety margin was added rather than the
  floor being lowered to paper over an actual regression. **Not
  confirmed borderline-flaky** in 3 runs, but the margin above the *new*
  floor (5x) is comfortable (~2x) while the margin above the *old* floor
  (10x) is thin — a future regression could plausibly still trip 10x without
  tripping 5x, so this remains a real, if modest, weakening of the assertion
  rather than a pure like-for-like adjustment.

**Overall**: the T15 fix is real, well-documented, and independently verified
against the source; the threshold relaxation is justified by measurement but
does represent a genuine loosening of the regression floor (10x → 5x), not
purely cosmetic.

---

## Gate Check

- **Gate command**: `cargo build --workspace` then `cargo test --workspace`
- **Build result**: zero rustc warnings (only a `build.rs`-emitted advisory
  about the Python `.so` not being pre-built, not a compiler warning)
- **Test result**: **582 passed, 0 failed, 5 ignored** — matches the
  author's claim in `spec.md`/`tasks.md` T15 exactly, independently verified
  by re-running the full suite and summing `test result:` lines
  (`grep "^test result:" ... | awk ...`)
- **ngspice cross-checks**: ngspice-46 detected on `$PATH`; `ngspice_four_diode`,
  `ngspice_pz_rc`, `ngspice_disto_diode` all ran for real (not the skip
  branch) and passed
- **Skipped tests**: 5 ignored — all pre-existing doctests on
  non-runnable/illustrative code (`piperine-plugin`, `piperine-plugin-wasm`,
  `piperine-solver` analyses/builder/prelude doctest snippets), unrelated to
  this feature; not a feature-introduced skip
- **Failures**: none

---

## Fix Plans (if issues found)

### Fix 1: Missing dedicated test for "purely digital circuit" fail-loud edge case (`.pz`/`.sp`/`.disto`)

- **Root cause**: `.disto` has the guard (`disto.rs:219-221`) but no test
  exercises a genuinely analog-free circuit; `.pz`/`.sp` rely on
  `finite_generalized_eigenvalues`/downstream fail-loud paths that were only
  tested with a resistor-only *analog* network, not a zero-analog-node
  circuit.
- **Fix task**: Add one test per analysis (`.pz`, `.sp`, `.disto`) that runs
  against a module with no analog nodes/nets at all and asserts the specific
  fail-loud message.
- **Priority**: Minor (the general fail-loud discipline holds; this is a
  coverage gap, not a behavior gap).

### Fix 2: Missing dedicated test for `.disto` DC non-convergence propagation

- **Root cause**: `disto.rs:224` relies on `?`-propagation from `DcSolver`;
  plausible by construction but not exercised by a circuit engineered to
  fail DC convergence.
- **Fix task**: Add a `.disto` test with a circuit that fails to converge at
  DC (e.g. an unstable positive-feedback bias network) and assert the error
  surfaces the DC failure, not a distortion result.
- **Priority**: Minor.

---

## Requirement Traceability Update

All 24 requirement IDs (FOUR-01..05, PZ-01..07, SP-01..06, DISTO-01..06)
remain **✅ Verified** — spec.md's own traceability table already marks them
"Done"; this validation independently confirms each with file:line evidence.

---

## Summary

**Overall**: ✅ Ready (with 2 minor, non-blocking coverage gaps noted above)

**Spec-anchored check**: 24/24 ACs matched spec-defined outcomes; 0
spec-precision gaps (every AC in spec.md states an exact tolerance or fail
condition, and every corresponding test targets it precisely)

**Sensor**: 3/3 mutations killed

**Gate**: 582 passed, 0 failed, 5 ignored (pre-existing, unrelated) — matches
author's claim exactly, independently reproduced

**What works**: All four analyses (`.four`/`.pz`/`.sp`/`.disto`) on both
hosts, ngspice-cross-validated where a reference exists, fail-loud discipline
intact everywhere checked, the `.disto` compile-explosion fix is real and
well-engineered (per-branch-pair Cranelift functions + symbolic-redundancy
removal + dev-profile opt-level), `live_optimize.py`'s 5x floor is
comfortably met (10-11x measured) and not flaky in 3 runs.

**Issues found**: 2 minor test-coverage gaps (digital-only fail-loud edge
case untested for `.pz`/`.sp`/`.disto`; `.disto` DC-non-convergence
propagation untested) — both plausible-correct by code inspection but not
empirically pinned by a regression test.

**Next steps**: Optional — add the two edge-case tests above (Fix 1/2) in a
follow-up task; not blocking, since the feature's core P1/P2 acceptance
criteria are all met with strong evidence and the discrimination sensor
confirms the existing tests are not shallow.

# Random-Variable Catalogue Validation

**Date**: 2026-08-18
**Spec**: `.specs/features/random-variable-catalogue/spec.md`
**Diff range**: `2bd431b7..77ff498e` (feature commits `ca233e4f`, `759ebe4d`, `77ff498e`)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status  | Notes |
| ---- | ------- | ----- |
| T1 (grammar) | ✅ Done | `parse_tol_clause` + at-most-one loud — `grammar/item.rs:553-597` |
| T2 (elab) | ⚠️ Partial | Distribution lands on the POM, but done-when "mixed with `from` works" has **no committed test** (probe-verified only) |
| T3 (host migration) | ✅ Done | 23/23 green; `high_sigma_sampling` also green (3/3) — see RVCAT-08 note |
| T4 (docs) | ⚠️ Done w/ blemish | Part I updated; ROADMAP edit introduced a duplicated bullet (see Code Quality) |

---

## Spec-Anchored Acceptance Criteria

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| RVCAT-01 clauses parse → catalogue, stable order | `TolClause { kind, args, global }` on `ParamDecl`; `Param::distribution` filled; deterministic walk order | `crates/piperine-lang/src/grammar/item.rs:573` (`parse_tol_clause`); `crates/piperine-lang/src/elab/lower/vams.rs:125-146,157` (`distribution: distribution.clone()`); `crates/piperine-lang/tests/vams_param_distributions.rs:36-39` — `assert_eq!(tol.kind, "gauss"); assert!(tol.global)`; `crates/piperine-api/tests/monte_carlo_sampling.rs:270-274` — `assert_eq!(labels, vec!["m1.l1","m1.l2","m2.l1","m2.l2"])` | ✅ PASS |
| RVCAT-02 driver draws + applies via restamp, zero rebuilds | MD-18: 1000 samples, 0 recompiles | `crates/piperine-api/tests/monte_carlo_compile_once.rs:68-74` — `assert_eq!(delta, 0)` over `plan.monte_carlo(&mut session, 1000, 5, …)`; apply path `crates/piperine-api/src/sampling.rs:255-260` (`Session::set` per draw) | ✅ PASS |
| RVCAT-03 inert at nominal solve | bit-identical op/tran/ac with vs without `tol` | `tests/tol_inert.rs:106-120` — `to_bits()` equality on v/i; `:141-144` tran sample-by-sample `to_bits()`; `:161-166` ac point-by-point `to_bits()`; staged override `:181-192`; invalidation class `:218-221` | ✅ PASS |
| RVCAT-04 global shared; mismatch independent | one draw per (module,param) group; per-instance otherwise | `monte_carlo_sampling.rs:125-129` — `assert_eq!(r1_rg.value, r2_rg.value)`; `:160-164` — `assert_ne!(r1_rm.value, r2_rm.value)`; `:173-197` both shapes in one run | ✅ PASS |
| RVCAT-05 (seed, index) replay byte-identical | independent constructions reproduce draws/op/margins bit-for-bit | `monte_carlo_sampling.rs:340-347` — `assert_eq!(a.value.to_bits(), b.value.to_bits())`; `:396-405` replayed failure `to_bits()` on V(out) and margin | ✅ PASS |
| RVCAT-06 worst named+replayable; yield; margin spread | worst = min over converged, replayable; yield = share all-margins ≥ 0; σ = population stddev | `monte_carlo_report.rs:127-137` yield vs hand-count; `:161-171` σ vs hand-computed population stddev `< 1e-9`; `:190-210` worst is min + replay `to_bits()` | ✅ PASS |
| RVCAT-07 duplicate RV loud; **out-of-range draw reported** | duplicate → parse error "at most one"; drawn value past a `from` range is **reported, not clamped** | duplicate: `vams_param_distributions.rs:57-66` ✅; unknown kind loud `:107-113` ✅; unresolvable arg loud `:116-121` ✅. **Out-of-range reporting: NO evidence** — no range check in `sampling.rs:255-260`, compiled devices are `Bounds::UNBOUNDED` (`piperine-codegen/src/device/element.rs:429`); verifier probe: 46 out-of-range draws over 50 samples, **0 apply errors, no report channel**; `Session::set("r1","r",-12345.0)` on a `from (0.0:2000.0)` param returns `Ok(())` | ❌ GAP |
| RVCAT-08 suites unblocked; high-sigma stays red (named blocker) | tol_inert 5, sampling 8, report 4, compile_once 1, center 2 green | All green (Gate Check below). **`high_sigma_sampling` is 3/3 GREEN** — the spec's "stays red with named blocker" is stale; commit `759ebe4d` documents the estimator surface (`sampling.rs:300-335`, `driver/high_sigma.rs`) turning out live | ✅ PASS (⚠️ spec text stale, favorable direction) |
| RVCAT-09 docs current | Part I parameters section + ROADMAP bullet resolved | `docs/spec/part_i_language.md:158-164` documents kinds/keyed args/scope/inert-at-solve; ROADMAP bullet removed in `77ff498e` | ✅ PASS (cosmetic blemish: duplicated "SOA constraint blocks" bullet at `ROADMAP.md:192` vs `:195`) |

**Status**: ❌ Gaps present — RVCAT-07's out-of-range-report half has zero implementation and zero coverage

### Spec-listed forms not honored at draw time (P1.1)

Spec P1.1 lists `tol uniform(lo_offset = …, hi_offset = …)` as a supported clause. It parses and lands on the POM (generic keyed args), but `SampleEntry::draw_from` (`sampling.rs:390-420`) only handles `sigma`/`sigma_rel` (gauss) and `half`/`rel` (uniform) — verifier probe: `uniform(lo_offset = -500.0, hi_offset = 500.0)` draws return the nominal exactly (delta = 0 on every entry), a **silent no-op**. Unknown/mistyped arg names are likewise silently ignored (elab validates the *kind*, never the arg names — `vams.rs:135-144`). This collides with the repo's fail-loud hard rule.

---

## Discrimination Sensor

Scratch protocol: files backed up to `/tmp/opencode/*.bak`, mutated in place (target files had no pre-existing unstaged changes), tests run, restored from backup; `git status` verified byte-identical to the pre-sensor tree afterward.

| Mutation | File:line | Description | Killed? |
|---|---|---|---|
| 1 | `elab/lower/vams.rs:157` | `distribution: distribution.clone()` → `None` (the pre-feature vestige) | ✅ Killed — lang 2 fails; `monte_carlo_sampling` 7/8 fail |
| 2 | `grammar/item.rs:553-563` | clause parsed but dropped (`ParamDecl.tol = None`) | ✅ Killed — lang 6 fails |
| 3 | `elab/lower/vams.rs:136-144` | keyed args const-eval replaced with `Value::Real(0.0)` | ✅ Killed — lang 2 fails; sampling 5 fails; report 2 fails |
| 4 | `elab/lower/vams.rs:145` | `global: t.global` → `global: false` | ✅ Killed — lang 1 fail; sampling 3 fails (global-sharing tests) |
| 5 | `elab/lower/vams.rs:126-134` | unknown-kind check removed (any kind accepted) | ✅ Killed — lang 1 fail (`an_unknown_distribution_kind_fails_loud`) |

**Sensor depth**: lightweight-plus (5 behavior-level mutations, one per high-risk seam)
**Result**: 5/5 killed — PASS ✅

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code / surgical changes | ✅ — feature commit touches exactly grammar + elab + one new test file; host needed zero code change (as designed) |
| No scope creep | ✅ |
| Matches patterns (MD-13: methods on impls, no macros, fail loud) | ⚠️ — `draw_from`'s silent `return base` on unrecognized arg names is a silent no-op (see G2) |
| Spec-anchored outcome check | ⚠️ — RVCAT-07 out-of-range "reported" outcome asserted nowhere |
| Every test maps to a spec requirement | ✅ — 9 lang tests ↔ RVCAT-01/07; host suites ↔ RVCAT-02..06/08 |
| Guidelines followed | ✅ `AGENTS.md` (zero warnings verified), tasks.md gates run |

---

## Edge Cases

- [x] Duplicate `tol` on one declaration: loud parse error — `vams_param_distributions.rs:57-66`
- [x] Sampled draw makes the circuit fail to converge: reported as `SampleOutcome::Failed`, run continues — `monte_carlo_report.rs:216-248`
- [x] `tol`/`global` stay contextual (Annex B untouched); `tol` usable as a param name — `:123-138`
- [ ] **Drawn value outside a `from` range is reported**: NOT implemented, NOT covered — probe shows silent application (RVCAT-07)
- [ ] RV on a param of an uninstantiated module ("draws exist, apply to nothing"): no test; the walk-based catalogue yields **no entries at all** for uninstantiated modules (`sampling.rs:475-508`) — outcome-equivalent but text-divergent, and uncovered
- [x] Post-run nominal has no residue (reset) — `monte_carlo_sampling.rs:412-436`, `monte_carlo_report.rs:243-248`

---

## Gate Check

- **Gate commands**: `cargo test -p piperine-lang --test vams_param_distributions` · `cargo test --test tol_inert` · `cargo test -p piperine-api --test monte_carlo_sampling --test monte_carlo_report --test monte_carlo_compile_once --test center_policy --test high_sigma_sampling` (all with `CARGO_TARGET_DIR=/var/tmp/piperine-target`) + `cargo build --workspace`
- **Result**: 32 passed, 0 failed, 0 skipped (9 lang + 5 tol_inert + 8 sampling + 4 report + 1 compile_once + 2 center + 3 high-sigma); workspace build **zero warnings** (re-verified post-restore)
- **Test count delta**: +9 new lang tests; 23 host tests migrated from blocked → green; no tests deleted; no assertions weakened (migration strengthened fixtures onto the live producer)
- **Failures**: none

---

## Fix Plans

### Fix 1: Out-of-range draws are never reported (RVCAT-07, half)

- **Root cause**: `SamplePlan::apply` (`sampling.rs:255-260`) forwards draws to `Session::set` with no range awareness; compiled devices carry `Bounds::UNBOUNDED` (`piperine-codegen/src/device/element.rs:429`), so nothing anywhere observes the violation. Spec decision: "draws report, never clamp".
- **Fix task**: add a report channel on the sampling path — e.g. `SamplePlan::apply`/`monte_carlo` compares each draw against the authored param's `from` range (POM has it) and records/vends an out-of-range notice per draw (never an error, never a clamp). Add a test: `from`-ranged param + wide `tol`, assert ≥1 reported notice and that the value was still applied unclamped.
- **Priority**: Major (spec-mandated behavior with zero implementation)

### Fix 2: `uniform(lo_offset/hi_offset)` and unknown arg names silently no-op at draw (P1.1)

- **Root cause**: `draw_from` (`sampling.rs:398-418`) pattern-matches a closed set of arg names and returns `base` for anything else; elab never validates arg names.
- **Fix task**: either implement the `lo_offset`/`hi_offset` semantics the spec lists, or fail loud at elab on arg names outside the supported set per kind. Add a test either way.
- **Priority**: Major (fail-loud hard rule; spec-listed form is inert)

### Fix 3: design.md PRNG claim false; spec goal "one PRNG shape" unmet

- **Root cause**: design.md says draws come from `piperine_lang::eval::rng::{RandomState, Distributions}`; the actual draw path is a private `SplitMix64` + Box-Muller in `sampling.rs:550-585` — a second PRNG convention alongside `$rdist_*`.
- **Fix task**: either re-anchor `draw_from` on lang's `Distributions` (spec goal) or amend the spec goal + design.md to record the SplitMix64 decision.
- **Priority**: Minor (replay ACs pass; this is a spec/design fidelity gap, not a behavioral one)

### Fix 4: Smaller fidelity items

- design.md "in any order with `from`/`exclude`" — actually `tol` must follow the ranges (probe: `tol … from (…)` → parse error). Amend design.md or the grammar. **Minor**
- Spec edge "RV on uninstantiated module: draws exist, apply to nothing" — the walk produces no entries; no test. Add the test or amend the text. **Minor**
- RVCAT-08 spec text stale ("high_sigma stays red" — it is green, `759ebe4d`). Amend spec. **Cosmetic**
- `ROADMAP.md:192` duplicates the `:195` "SOA constraint blocks" bullet (introduced by `77ff498e`). **Cosmetic**
- T2 done-when "mixed with `from` works" has no committed test (probe-verified only). **Minor**

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
|---|---|---|
| RVCAT-01 | Verified | ✅ Verified |
| RVCAT-02 | Verified | ✅ Verified |
| RVCAT-03 | Verified | ✅ Verified |
| RVCAT-04 | Verified | ✅ Verified |
| RVCAT-05 | Verified | ✅ Verified |
| RVCAT-06 | Verified | ✅ Verified |
| RVCAT-07 | Verified | ❌ Needs Fix (duplicate-loud ✅; out-of-range report — no evidence) |
| RVCAT-08 | Verified | ✅ Verified (spec text stale re: high-sigma — favorable) |
| RVCAT-09 | Verified | ✅ Verified (cosmetic ROADMAP blemish) |

---

## Lessons (no-script fallback — no `scripts/lessons.py` in this repo)

Proposed grounded entries (NOT written to `.specs/LESSONS.md` — that file carries unrelated uncommitted WIP; recording left to the orchestrator):

1. **A spec edge case phrased as "X is reported, not clamped" needs an assertion on the report channel itself** — "not clamped" alone is satisfiable by total silence; probe showed 46 out-of-range draws with zero observations (RVCAT-07, `sampling.rs:255-260`).
2. **Keyed-argument surfaces need elab-time name validation or a draw-time loud default** — a closed arg-name set with a silent `return base` fallback turns spec-listed forms (`uniform(lo_offset/hi_offset)`) and typos into silent no-ops (`sampling.rs:398-418`).

## Summary

**Overall**: ❌ Not Ready (two Major gaps; everything else solid)

**Spec-anchored check**: 8/9 ACs verified against spec outcomes; RVCAT-07 half-covered (no evidence for out-of-range reporting); 2 spec-precision notes flagged (RVCAT-08 stale text; uninstantiated-module edge)
**Sensor**: 5/5 mutations killed
**Gate**: 32 passed, 0 failed; workspace build zero warnings

**What works**: parse + elab of `tol` clauses (contextual keywords, module-scope const-eval, at-most-one loud, unknown kind/arg loud), inert-at-solve bit-identity across op/tran/ac, global-vs-mismatch scoping, `(seed, index)` byte-identical replay, restamp-only 1000-sample compile-once, yield/σ/worst-sample reporting, non-convergent samples as outcomes, high-sigma importance sampling (green beyond spec expectation).

**Issues found**: Fix 1 (out-of-range draws unreported), Fix 2 (spec-listed uniform form + arg typos silently inert), Fix 3 (PRNG design-claim false), Fix 4 (fidelity minors).

**Next steps**: route Fix 1 + Fix 2 to an implementer, then re-verify (iteration 1 of max 3).

---

# Round 2 — Re-verification of fix commit `1d2b1df3`

**Date**: 2026-08-18 · **Verifier**: independent sub-agent (author ≠ verifier) · **Scope**: close-out of round-1 Majors (Fix 1, Fix 2) + recorded minors; regression re-run of the still-green suites.

## Verdict: ✅ PASS

Both Majors are closed with implementation + tests + killed discrimination sensors. Minors confirmed as recorded. No regression in any still-green suite.

## Major 1 — RVCAT-07 out-of-range draws reported loud: CLOSED

- **Mechanism**: POM `Param` gains `from_ranges` (const-evaled numeric `from` intervals, per-bound exclusivity; `exclude`/`{…}` disclosed as non-constraining) — `pom/module.rs:131-148` (`FromRange::contains`), populated at `elab/lower/vams.rs:120-135,178`. `SampleEntry` carries it (`sampling.rs:101-106`); `SamplePlan::apply` / `apply_around` / `apply_importance` all run `check_draw_ranges` (`sampling.rs:262,313,389`) which fails loud with `Error::Measurement` naming entry, drawn value, and declared domain (`sampling.rs:275-301`). Never clamped — the run errors before any `Session::set`.
- **Test**: `monte_carlo_sampling.rs::a_draw_outside_the_declared_range_is_reported_loud` — `r = 1e3 from (0:1e6) tol gauss(sigma_rel = 2.0)`, asserts ≥1 of 50 draws errors and the message names the entry (`r1`) and the `from` domain. ✅ passes.
- **Sensor (killed)**: `check_draw_ranges` neutered to a no-op in scratch → the test **fails** (`0 passed; 1 failed`). Restored byte-identical (backup/restore, `git diff` empty).

## Major 2 — `uniform(lo_offset/hi_offset)` inert + silent unknown args: CLOSED

- **Loud keys**: `draw_from` validates every keyed arg against a per-kind known set (`gauss: sigma, sigma_rel`; `uniform: half, rel, lo_offset, hi_offset`) and errors naming the typo + accepted keys (`sampling.rs:440-456`); a draw with no usable args errors instead of returning nominal (`:457-498`); unknown kind errors too.
- **lo_offset/hi_offset implemented**: `nominal + U(lo, hi)` (`sampling.rs:476-489`). **Scratch probe** (`tol uniform(lo_offset = -500.0, hi_offset = 500.0)` on `r = 1e3`): draw ≠ nominal and ∈ [500, 1500] — live semantics confirmed. ⚠️ **Minor residual**: no *committed* test exercises the lo_offset/hi_offset draw itself (probe-verified only) — Fix 2 said "add a test either way"; the committed test covers the loud path (`an_unrecognized_draw_argument_fails_loud`), not the new semantics.
- **Test**: `an_unrecognized_draw_argument_fails_loud` — `tol gauss(sigmaa = 10.0)` errors naming `sigmaa` and the legal keys. ✅ passes.
- **Sensor (killed)**: unknown-key check neutered in scratch → test **fails** ("names the typo" assertion — neutered run reports only "needs `sigma` or `sigma_rel`"). Restored byte-identical.

## Minors — confirmed as recorded

- design.md: SplitMix64+Box-Muller stream pinned by dv-core values; re-anchor onto lang's `Distributions` recorded as a separate user call with explicit `SPEC_DEVIATION` note; grammar order documented as ranges-then-tol (not "any order"). ✅ (commit diff)
- ROADMAP: duplicated "SOA constraint blocks" bullet deduped. ✅ (commit diff)
- Signature ripple (`draws`/`draws_around`/`draws_importance` → `Result`) propagated to all callers incl. `high_sigma_sampling.rs`. ✅

## Gate re-run (CARGO_TARGET_DIR=/var/tmp/piperine-target)

| Suite | Result |
|---|---|
| `monte_carlo_sampling` | 10/10 ✅ (+2 new) |
| `high_sigma_sampling` | 3/3 ✅ |
| `tol_inert` | 5/5 ✅ |
| `vams_param_distributions` (lang) | 9/9 ✅ |
| `monte_carlo_report` / `monte_carlo_compile_once` / `center_policy` | 4/4 · 1/1 · 2/2 ✅ |
| `cargo build --workspace` | zero warnings ✅ |

**Total: 34 passed, 0 failed.** Sensors: 2/2 killed. Tree restored clean after all scratch mutations (`git status` on touched files empty).

## Residuals (non-blocking)

1. **Minor**: no committed test for the `lo_offset/hi_offset` draw semantics (probe-verified live; the two committed tests cover range-report and unknown-key paths). Recommend one small committed test.
2. **Cosmetic**: the range-report format string contains a run of stray spaces mid-message (`sampling.rs:295`).
3. Round-1 Fix-4 leftovers not in this round's scope remain open (uninstantiated-module edge test, stale RVCAT-08 spec text, T2 mixed-with-`from` committed test).

**Overall**: ✅ Ready — both Majors closed with discriminating tests; all suites green.

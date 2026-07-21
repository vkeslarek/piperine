# solver-simplification Validation

**Date**: 2026-07-19
**Spec**: `.specs/features/solver-simplification/spec.md`
**Diff range**: `4565f9e^..dc4fa07` (41 commits, branch `feature/bench-removal`; 66 files, +4501/−2689)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

All 35 tasks (T1–T35, batches 1–6) marked done in `tasks.md` Progress Log. No
blocked or partial tasks. Commit hashes in the log verified against
`git log 4565f9e^..HEAD` (all present).

---

## Spec-Anchored Acceptance Criteria (requirements SS-01..SS-18)

| Req | Spec-defined outcome | `file:line` evidence | Result |
|-----|----------------------|----------------------|--------|
| SS-01 | One module per analysis (request + driver + result reachable from one module); old `analysis/`+`solver/` trees gone | `crates/piperine-solver/src/analyses/{dc,ac,tf,transient,noise,sens,pss}.rs`; `analyses/dc.rs:24` (`── request/state ──`), `analyses/dc.rs:75` (`── driver ──`); `ls src/` shows no `analysis/` or `solver/` dir | ✅ PASS |
| SS-02 | Function-named modules; no `contracts`/`traits`/`models`/`utils` dirs; each layer contracted | `src/{analyses,core,analog,digital,math}` — all system-function names; `analyses/dc.rs:1-6` `//!` contract naming contents + must-not-contain | ✅ PASS |
| SS-03 | `Element` = composed concern supertraits, no downcast, no `Any` | `core/element.rs:106` (`AnalogDevice`), `:225` (`DigitalDevice`), `:280` (`Introspect`), `:352` (`pub trait Element: AnalogDevice + DigitalDevice + Introspect`); grep for `Any`/downcast in dispatch paths clean (only doc mentions) | ✅ PASS |
| SS-04 | Every element implements composed surface; suite green | `tests/composed_element.rs:100-125` (analog-only double solves through `dyn Element`); parity devices in `tests/parity_baseline.rs:80-95,130-135` use the split impls; gate 520/0 | ✅ PASS |
| SS-05 | `transient::solve()` decomposed into named phase methods, none > ~60 lines | `analyses/transient.rs:1118` `solve()` = 40-line phase loop over `begin_run`(:1002) `predict_step`(:746) `attempt_step`(:790) `assess_step`(:810) `accept_step`(:837) `reject_lte_step`(:1069) `reject_step`(:1105) `settle_digital`(:927) `record_step`(:941) `propose_dt`(:963) `finish_run`(:1050); scripted size audit: max driver method 47 lines | ✅ PASS |
| SS-06 | Transient parity bit-identical | `tests/parity_baseline.rs:404-428` pins RC tran exact value `−0.450_485_218_772_388_9` + step count 386; gate green; GAMMA mutant killed (sensor #2) | ✅ PASS |
| SS-07 | Homotopy/stepper numerics read from typed config, zero behavior-affecting literals in bodies | `analyses/config.rs:12-91` (`GminSchedule`/`SourceSchedule`), `:107-132` (`StepperGains`); `analyses/convergence.rs:397-431` and `:453-501` bodies read only `schedule.*` fields; `:200-228` `PiController` reads only `self.gains.*`; grep for former literals in bodies clean | ✅ PASS |
| SS-08 | Trace toggles through typed config, no hot-path env reads | `analyses/config.rs:139-158` (`TraceFlags` + `from_env`); `analyses/mod.rs:143` single env read seeding `Policy`; grep `env::var` shows no other site in the crate | ✅ PASS |
| SS-09 | Config defaults == former literals exactly | `analyses/config.rs:167-202` test asserts every default (`start_g==0.1`, `knee_gmin==1e-6`, `reject_divisor==8.0`, …); parity baselines green on defaults | ✅ PASS |
| SS-10 | `LINEAR`/`ANALYTIC_JACOBIAN`/`STAMPS_CHARGE` removed (+ producers + asserts); every flag has both ends | Workspace grep: only comments referencing the removal; `tests/capabilities_contract.rs:52-65` (every surviving flag documented-consumer) + `:67-77` (reintroduction guard); `core/element.rs:34-82` table | ✅ PASS |
| SS-11 | Phantom rollback doc corrected; no doc references non-existent methods | `core/element.rs:65-73` `SUPPORTS_ROLLBACK` reserved with explicit "No method is promised"; grep `checkpoint_state`/`rollback_state`/`commit_state` clean in crate and Part VII | ✅ PASS |
| SS-12 | `SignalBridge` zero-field struct gone; methods folded into caller | Grep: no `SignalBridge` type anywhere (comments only); `core/circuit.rs:158-293` seam section incl. `seed_digital_from_accept_hooks`(:279) | ✅ PASS |
| SS-13 | STATE.md MD-05 done + MD-01 amendment with date | `.specs/STATE.md:46-54` MD-05 "Done (2026-07-19)" listing shipped+wired strategies; `:8-21` MD-01 "(amended 2026-07-19)" supertrait amendment with C-ABI rationale | ✅ PASS |
| SS-14 | Every solver module carries a one-line `//!` responsibility contract | All `core/*`, `analyses/*`, `math/*`, `analog/*`, `digital/*` modules carry `//!` (scripted check). ⚠️ `src/error.rs:1` and `src/result.rs:1` (and crate root `lib.rs`) have no `//!` — both are outside design §1's layer table but inside spec.md's "each solver module" wording | ⚠️ Minor gap |
| SS-15 | `CircuitInstance` grouped into five contracted responsibilities | `core/circuit.rs:1-4` struct contract; sections at `:52` (Circuit state), `:99` (Analysis entry), `:158` (Mixed-signal seam), `:294` (Live mutation), `:384` (Construction — builder-output seam only, documented) | ✅ PASS |
| SS-16 | Mixed-signal seam one home, behavior unchanged | `core/circuit.rs:158-175` call-order contract; `tests/parity_baseline.rs:562-587` mixed-signal DC settle pinned `1.666…7` — green | ✅ PASS |
| SS-17 | `math/unit.rs` gone, aliases inlined to `f64`, no `Second` re-export | `src/math/` listing: no `unit.rs`; grep `math::unit`/`crate::math::unit` clean workspace-wide; grep `\bSecond\b` in `abi.rs`/`prelude.rs` clean | ✅ PASS |
| SS-18 | Part VII canonical, code-consistent, no phantom constructs | `docs/spec/part_vii_solver.md`: §2 five responsibilities (:78-104) matches `circuit.rs`; §3 capabilities table (:150-161) matches `element.rs:34-82` incl. reserved-bit wording; §10.2 phase loop matches `transient.rs`; §15.9 `bound_step_hint` no-consumer note (:1032); §16 failure rows (:1052-1078); §17 sens + §18 PSS present; grep for `LINEAR`/`ANALYTIC_JACOBIAN`/`STAMPS_CHARGE`/`SignalBridge`/`math::unit`/rollback methods in Part VII: zero hits | ✅ PASS |

**Status**: ✅ 17/18 requirements fully covered · ⚠️ 1 minor gap (SS-14: `error.rs`/`result.rs` `//!` missing)

---

## Discrimination Sensor

Scratch-state mutations applied in place on tracked files, restored with
`git checkout --`; tree verified clean after (`git status` shows only the
pre-existing `examples/07_thermostat_plot.png` modification + untracked
`.specs/features/spectral-analyses/`, both untouched by this verification).

| # | Mutation | File:line | Test run | Killed? |
|---|----------|-----------|----------|---------|
| 1 | `start_g: 0.1` → `0.2` (config default flip) | `analyses/config.rs:37` | `cargo test -p piperine-solver defaults_equal` | ✅ Killed — `defaults_equal_the_literals_they_replace` FAILED (assertion: `0.2 == 0.1`). Note: `parity_diode_dc_point` survived because that circuit converges under plain Newton (gmin stepping never fires) — the config-defaults test is the discriminator, as designed |
| 2 | `GAMMA` `0.58578…` → `0.48578…` (TR-BDF2 γ, parity-critical driver value) | `math/integration.rs:55` | `cargo test -p piperine-solver --test parity_baseline` | ✅ Killed — `parity_rc_transient` FAILED (5 other baselines unaffected, as expected for a transient-only constant) |
| 3 | LTE reject flipped `milne > trtol` → `milne < trtol` (extracted `assess_step` phase) | `analyses/transient.rs:830` | `cargo test -p piperine-solver --test parity_baseline parity_rc` | ✅ Killed — dt collapses to the `dt_min` floor and the run livelocks (1e12 steps at floor; "accepting at dt_min" warnings confirm the mutated path). Kill mode is suite-level timeout, not an assertion — the suite cannot pass with the mutant, which is the sensor criterion; noted as a slightly less crisp kill than an assertion failure |

**Sensor depth**: lightweight (3 mutations) — appropriate for a behavior-preserving refactor whose oracle is parity + contract assertions
**Result**: 3/3 killed — PASS ✅

---

## Gate Check

- **Gate command**: `cargo test --workspace` (full) + `cargo build --workspace 2>&1 | grep -cE "^warning:|^error"` (build)
- **Result**: **520 passed, 0 failed, 5 ignored** across 51+ targets
- **rustc warnings**: **0** (piperine-cli build-script `cargo:warning=` notes about the Python `.so` are pre-existing noise, not rustc `warning:` lines — excluded by the anchored pattern)
- **Test count before feature**: 509 (spec.md baseline)
- **Test count after feature**: 520
- **Delta**: +11 (6 parity baselines + 2 capabilities contract + 2 composed-element + 1 config-defaults) — matches Progress Log exactly
- **Ignored**: 5 — pre-existing ignored doctests (`AcSweepAnalysisOptions::generate_frequencies`, `CircuitBuilder`, `prelude`) and 2 pre-existing ignored integration tests; none introduced by this feature
- **Failures**: none

---

## Known Gaps Reported by the Last Worker — Verification

| # | Claim | Verdict | Evidence |
|---|-------|---------|----------|
| i | `bound_step_hint` has a producer but no solver consumer | **CONFIRMED** | Producers: `crates/piperine-codegen/src/device/mod.rs:150-153`, `device/analog.rs:1363`. No call site anywhere in `piperine-solver` (grep clean outside the trait default `core/element.rs:121`). Honestly documented in `docs/spec/part_vii_solver.md:1032` ("the current stepper does not consult [it]") — a documented dangling contract end, not a hidden one |
| ii | Three §16 contract rows not enforced at runtime | **CONFIRMED** | (a) §2 "element declares no capability → device-load error": no empty-capabilities check in `core/builder.rs` (only the `HAS_INTERNAL_UNKNOWNS` allocation check at `:149`); (b) §4 "digital event targets nonexistent net → analysis error": `QueueSink::emit` (`digital/interface.rs:82-88`) pushes events with no net-existence validation; (c) §4 "digital boundary changes during analysis → analysis error": `boundary()` is re-read per iteration (`digital/scheduler.rs:170,192`) and never compared against the initial boundary. All three are doc-stated rules with no runtime enforcement |

**Scope judgment**: both are pre-existing conditions outside spec.md's
behavior-preserving scope ("input validation — unchanged (same loud errors)";
out-of-scope table excludes new capability work). The spec required removing
*dangling* flags (SS-E) — done; it did not require wiring `bound_step_hint`
or adding §16 runtime checks. Classified as roadmap observations, not feature
failures.

---

## Code Quality

| Principle | Status |
|-----------|--------|
| Minimum code / surgical changes | ✅ — diff is exactly the refactor surface (solver crate + codegen seam + Part VII + STATE.md); no unrelated edits |
| No scope creep | ✅ — out-of-scope table respected (no retuning, no new analyses, osdi untouched) |
| Matches patterns / MD-13 | ✅ — trait/struct-owned methods only, no macros (grep `macro_rules!` in solver crate: none new), modules named by system function |
| Tests map to ACs, non-shallow | ✅ — parity tests pin exact solved values to 1e-9..1e-12; contract tests enumerate the flag registry exhaustively and assert the reintroduction guard |
| Spec-anchored outcome check | ✅ — every AC above carries `file:line`; config-defaults test asserts the exact spec-named literals |
| Documented guidelines followed | ✅ — `AGENTS.md` hard rules (fail loud, no unwrap on user paths, MD-13) respected |

---

## Edge Cases (spec.md)

- [x] Single-concern element (analog-only) leaves other concerns at defaults — `tests/composed_element.rs:127-145` (`defaulted_concerns_are_inert`)
- [x] External `impl Element` ergonomics — single conjunction bound preserved; codegen `PiperineDevice` regrouped into four explicit blocks, full gate green
- [x] Config defaults indistinguishable from literals — config parity test + parity baselines + sensor mutation 1

---

## Ranked Gaps

1. **Minor** — SS-14 partial: `src/error.rs:1` and `src/result.rs:1` lack the
   `//!` one-line responsibility contract the spec requires of "each solver
   module" (both sit outside design §1's layer table, hence missed by T32's
   audit). Fix: add two `//!` lines.
2. **Roadmap (out of scope)** — `bound_step_hint` producer without consumer
   (gap i above); documented in Part VII. Owner: `solver-performance` /
   future stepper work.
3. **Roadmap (out of scope)** — three §16 failure rows unenforced at runtime
   (gap ii above): empty-capabilities check, event-net validation, boundary
   stability check. Owner: a validation-hardening feature.

---

## Requirement Traceability Update

| Requirement | New Status |
|-------------|------------|
| SS-01..SS-13, SS-15..SS-18 | ✅ Verified |
| SS-14 | ⚠️ Verified with minor gap (error.rs/result.rs `//!`) |

---

## Summary

**Overall**: ✅ Ready (one Minor gap, non-blocking)

**Spec-anchored check**: 17/18 requirements matched with `file:line` evidence; 1 minor gap
**Sensor**: 3/3 mutations killed
**Gate**: 520 passed, 0 failed, 5 ignored (pre-existing); 0 rustc warnings; +11 tests vs 509 baseline

**What works**: the full refactor end-state exists in code — `analyses/`
Scheme-B layout with contracted layers, composed `Element` supertraits with a
green composed-surface contract test, a 40-line phase-loop `solve()` with all
driver methods ≤ 60 lines, a single config home whose defaults are pinned to
the former literals, dead flags/units/`SignalBridge` removed with regression
guards, five-section `CircuitInstance`, STATE.md current, and a Part VII that
matches the code section-for-section with zero phantom references. Parity
baselines and contract tests demonstrably discriminate (sensor 3/3).

**Issues found**: one Minor doc-contract gap (SS-14: `error.rs`/`result.rs`
missing `//!`); two pre-existing out-of-scope observations confirmed and
routed to the roadmap.

**Next steps**: fix SS-14 gap (two lines) at convenience; record gaps 2–3 in
`ROADMAP.md` if not already tracked.

---

## Addendum (2026-07-19)

- SS-14 gap FIXED — `error.rs:1` and `result.rs:1` now carry `//!` module
  contracts (`183dd44`); build gate 0 rustc warnings, solver suite green.
- Roadmap gaps 2–3 recorded in `ROADMAP.md` → "Minor refactor leftovers".
- **Final verdict: PASS — 18/18 requirements covered.**

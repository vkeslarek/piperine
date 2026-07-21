# Codegen Architecture Refactor Validation

**Date**: 2026-07-21
**Spec**: `.specs/features/codegen-architecture/spec.md`
**Diff range**: `78d31b3..HEAD` (14 commits, T1–T18) on `feature/bench-removal`
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1–T18 | ✅ Done | All 18 tasks present as individual commits (`cdc4201` … `7c90684`), each claim spot-checked against the actual tree (see AC table below), not taken on faith from tasks.md. |

---

## Scope Check

`git diff --stat 78d31b3..HEAD` touches `crates/piperine-codegen/` plus `CLAUDE.md`/`.specs/STATE.md` as expected, **plus** 6 files outside the crate:
`crates/piperine-api/src/error.rs`, `crates/piperine-lang/tests/{bundle_param,run_examples,spec_simulation}.rs`,
`crates/piperine-python/{src/live.rs,tests/live_facade.rs}`, `tests/{dc_host_proof,pss,sens,transient_reentry}.rs`.

All are 1–2 line diffs changing `piperine_codegen::ir::…` → `piperine_codegen::resolve::…` (dropping the legacy alias). This is the exact cross-crate call-site fixup the spec's own edge case requires ("WHEN the refactor moves a `pub` item that a sibling crate imports THEN the cross-crate call site SHALL be updated in the same commit") — **not a scope violation**, it is CGA-02's mandated consequence. No other unrelated content crept in.

---

## Spec-Anchored Acceptance Criteria (CGA-01 .. CGA-10)

| AC | Spec-defined outcome | Evidence | Result |
| -- | --------------------- | -------- | ------ |
| CGA-01 (pipeline module tree) | Top-level modules name pipeline stages; no `jit`/inner `codegen` | `ls crates/piperine-codegen/src/` → `device emit error.rs flatten kernel lib.rs resolve`. No `jit/`, `lower/`, `codegen/` directories exist. | ✅ PASS |
| CGA-02 (drop `ir` alias) | `pub use lower as ir` gone; all call sites use `resolve::…` | `grep -rn 'as ir\b\|::ir::' crates/ tests/ --include=*.rs` → only hits are Cranelift's own `cranelift_codegen::ir::*` module (unrelated false positives — `cranelift_codegen::ir` is the Cranelift IR crate, not piperine's dropped alias). No `piperine_codegen::ir::` or `as ir` remains. | ✅ PASS |
| CGA-03 (zero-warning, suite-green) | `cargo build --workspace` zero warnings; full suite green, unchanged test count | `cargo build --workspace` emits only the pre-existing, unrelated `piperine-cli` "python .so not found" notices — zero codegen warnings. `cargo test --workspace`: 99 `test result:` blocks, summed `passed` = **582**, zero `FAILED`/`failures:` lines. Matches tasks.md's claimed 582 both before and after every task. | ✅ PASS |
| CGA-04 (`AnalogKernel` capability sub-structs) | `core: AnalogCore` + `Option<Capability>` per optional capability; `has_*` are presence checks | `crates/piperine-codegen/src/kernel/analog/mod.rs:164-184` — struct is exactly `core: AnalogCore, reactive: Option<Reactive>, forces: Option<Forces>, limits: Option<Limits>, noise: Option<Noise>, ac_stim: Option<AcStim>, ac_idt_jacobian: Option<AnalogFn>, runtime_states, …`. `has_reactive()` at `mod.rs:464-466` is `self.reactive.is_some()`; `has_force_flux`/`has_force_current`/`has_force_ac_stim`/`has_ac_idt` (lines 358,376,544,585) are likewise presence/emptiness checks, no separately-stored bool. One file per capability: `kernel/analog/{reactive,forces,limits,noise,ac_stim}.rs` present. | ✅ PASS |
| CGA-05 (split `emit_analog`/`builder.rs`) | Each of `emit/{analog_expr,builder,resolver,cse,stmt}.rs` exists, reasonably scoped | `ls crates/piperine-codegen/src/emit/` → `abi.rs analog_expr.rs builder.rs cse.rs digital_expr.rs mod.rs resolver.rs stmt.rs`. Sizes: `analog_expr.rs` 619, `builder.rs` 820, `cse.rs` 148, `resolver.rs` 78, `stmt.rs` 266 LOC — each a bounded, single-purpose file, not a copy-paste dump (verified `emit_analog` in `analog_expr.rs` is a dispatcher calling category helpers, per T6's own note). | ✅ PASS |
| CGA-06 (every helper keeps an owner, MD-13 r2) | No new module-scope loose `pub fn`/`pub(crate) fn` in changed files | `grep -n '^pub fn \|^pub(crate) fn '` in `kernel/analog/mod.rs`, `device/analog/*.rs`, `device/circuit.rs`, `device/builder.rs` → **zero hits** (all are impl-block methods). Loose free fns found elsewhere (`resolve/diff.rs`, `emit/cse.rs::expr_structural_eq`) are confirmed pre-existing via `git show 78d31b3:…` — identical signatures existed before the refactor, so they are not new violations (matches T4's documented deviation). | ✅ PASS |
| CGA-07 (`CodegenError` home + messages) | Lives in crate-root `error.rs`; `ModuleNotFound` message no longer says "IrProgram" | `crates/piperine-codegen/src/error.rs:10` — `ModuleNotFound(String)` variant; `rg IrProgram` across the crate shows the string survives only in unrelated doc comments (`device/circuit.rs:8`, `resolve/pom/mod.rs:3`) describing the historical absence of the type — not in the error message. One doc-comment nit found: `device/mod.rs:8` still reads `walks an [\`crate::resolve::IrProgram\`]'s top module` — a broken intra-doc link, mechanically rewritten from `crate::ir::IrProgram` during T2's alias-drop rename without noticing `IrProgram` itself is a stale type name (confirmed via `git show 78d31b3:…/device/mod.rs` — pre-existing, not newly introduced). Flagged as a minor doc-accuracy gap, not a functional violation of CGA-07 (the AC is specifically about the error *message*, which is fixed). | ✅ PASS (minor doc nit noted, not blocking) |
| CGA-08 (`SimCtx` home) | Lives in `emit/abi.rs`, not a `jit/mod.rs` dumping ground | `crates/piperine-codegen/src/emit/abi.rs:10` — `pub struct SimCtx`. `src/jit/` does not exist. | ✅ PASS |
| CGA-09 (`device/analog` capability split + internal trait) | `forces.rs`/`limits.rs`/`operators.rs`/`events.rs` exist; `Element`/`AnalogDevice`/`DigitalDevice`/`Introspect` unchanged/flat; no capability trait leaks into the `Element` ABI | `ls crates/piperine-codegen/src/device/analog/` → `events.rs forces.rs limits.rs mod.rs operators.rs`. `device/mod.rs:153,259,324,379` — `impl AnalogDevice for PiperineDevice`, `impl DigitalDevice for PiperineDevice`, `impl Introspect for PiperineDevice`, `impl Element for PiperineDevice` — all still flat inherent impls on the single struct, no trait objects. `Stamps` trait (`device/analog/mod.rs:64`) and `AnalogCapability` trait (`kernel/analog/mod.rs:64`) are both un-exported (no `pub` on the trait, not re-exported through `lib.rs`) — confirmed internal-only, never crossing into the solver-facing `Element` surface. | ✅ PASS |
| CGA-10 (`device/circuit` responsibility split) | `device/builder.rs` (`InstanceBuilder`), `device/fusion.rs`, `device/plugin.rs` exist; `device/provider.rs` gone; `circuit.rs` holds only compiler API | `ls crates/piperine-codegen/src/device/` → `analog builder.rs circuit.rs digital.rs fusion.rs mod.rs plugin.rs` — no `provider.rs`. `circuit.rs` is 212 LOC (down from 888), `builder.rs` 428, `fusion.rs` 185, `plugin.rs` 166. | ✅ PASS |

**Status**: ✅ All 10 ACs confirmed by direct evidence (1 minor non-blocking doc-accuracy nit noted under CGA-07).

---

## Discrimination Sensor

Adapted for a refactor: proves the **existing** suite still catches regressions in the moved/split code (not new tests). One mutation at a time; each reverted with `git checkout --` immediately after its test run, verified clean before the next.

| # | File:line | Description | Test run | Result |
| - | --------- | ------------ | -------- | ------ |
| 1 | `crates/piperine-codegen/src/device/analog/forces.rs:86` (`ForceStamper::stamp`, moved in T12) | Flipped KCL sign on the `plus`-terminal branch-current matrix stamp: `Stamp::Matrix(p, branch, 1.0)` → `-1.0` | `cargo test -p piperine-codegen --test codegen_ir --test silent_bugs --test from_ir` (pass) then `cargo test --test ngspice_validation` | ❌→✅ **Killed** — 14/30 ngspice cross-checks fail (`ngspice_bjt_ce`, `ngspice_nmos_fixed`, `ngspice_rdiode`, …), Newton non-convergence on several |
| 2 | `crates/piperine-codegen/src/device/analog/limits.rs:122` (`Limiter::update`, moved in T12) | Loosened the "still limiting" veto tolerance from `1e-6` to `1e6` (effectively disables the Newton-convergence veto for `$limit` junctions) | `cargo test --test ngspice_validation` (30/30 pass) + `cargo test --workspace` (582/582 pass, 0 failed) | ⚠️ **Survived** — see note below |
| 3 | `crates/piperine-codegen/src/kernel/analog/mod.rs:528` (`AnalogKernel::eval_charge`, regrouped in T10) | Wrapped the reactive/charge accumulation body in `if false { … }`, making `eval_charge` a permanent no-op regardless of `self.reactive` | `cargo test --test transient_reentry` | ✅ **Killed** — `reentry_from_captured_state_matches_continuous_run` fails: `"Failed to converge after 500 iterations"` |

**Note on mutation 2 (survived):** confirmed via `git show 78d31b3:crates/piperine-codegen/src/device/analog.rs` that the exact tolerance expression (`1e-6 + 1e-4 * vnew[i].abs()`) is byte-identical to the pre-refactor code — this is a **pre-existing test-coverage gap**, not something the refactor introduced or made worse. The existing ngspice/junction fixtures happen to converge within the 500-iteration Newton cap even without the limiting-active veto actively firing, so no test currently exercises the veto's specific effect on convergence. This is worth a follow-up test (e.g. a fixture engineered to overshoot without the veto), but it is out of scope for this feature (a pure structural refactor must not add new tests to "cover" moved code, per the feature's own coverage-matrix note) and does not indicate the refactor changed behavior — the code moved verbatim.

**Sensor depth**: lightweight (3 targeted mutations across kernel/analog and device/analog capability code, per the tiering table for standard features).
**Result**: 2/3 killed, 1/3 survived (pre-existing gap, not refactor-introduced) — sensor confirms the suite is discriminating for the refactor's own changes; the one survivor is a documented, pre-existing gap outside this feature's remit.

Mid-sensor incident: mutation 3's first run (`cargo test --workspace`) hung indefinitely (10+ min, near-zero CPU, consistent with a stalled Newton loop elsewhere in the suite blocking on the broken charge path) and was killed (`kill -9`) rather than left to complete; the mutation was then re-verified with a fast, narrowly-scoped test (`transient_reentry`) which killed it cleanly in 0.43s. The file was reverted immediately after in both cases; `crates/piperine-codegen/` was confirmed clean before proceeding.

---

## Code Quality

| Principle | Status |
| --------- | ------ |
| No features beyond what was asked | ✅ — pure move/split/rename, no new behavior |
| No abstractions for single-use code | ✅ — `AnalogCapability`/`Stamps` traits each have multiple implementors (5 and 2+ respectively) |
| Only touched files required for task | ✅ — scope check above; cross-crate touches are the mandated CGA-02 fixups |
| Matches existing patterns/style | ✅ |
| Spec-anchored outcome check | ✅ — all 10 ACs traced to file:line |
| No unclaimed tests | ✅ — zero new tests added (582 passed both before and after, confirmed independently) |
| Documented guidelines followed | CLAUDE.md §Build and test, `.specs/STATE.md` MD-13 — followed |

---

## Gate Check

- **Gate command**: `cargo build --workspace` (zero warnings) + `cargo test --workspace`
- **Result**: 582 passed, 0 failed, 0 unexpected skips (a handful of pre-existing `1 ignored`/`3 ignored` blocks are unrelated to this feature)
- **Test count before feature**: 582 (per tasks.md, claimed at T1 and every subsequent task)
- **Test count after feature**: 582 (independently re-counted from a fresh `cargo test --workspace` run post-revert)
- **Delta**: 0 — confirms zero functional change invariant held
- **Failures**: none

---

## Edge Cases (from spec.md)

- [x] Moved `pub` item cross-crate import updated same-commit — confirmed via the 6 non-codegen files in the diff, all `ir::` → `resolve::` fixups, no broken build at any point (workspace builds clean at HEAD).
- [x] `None` capability sub-struct behaves identically to old `has_*==false` branch — confirmed by the full ngspice/solver suite staying green (582/0) and by mutation 3 above (disabling a capability path does break a real transient test, proving the dispatch is live and correctly gated).
- [x] No item became more public than needed — T17's done-when documents a grep-verified visibility sweep (`emit`/`flatten`/`error` narrowed to crate-private, `device`/`kernel`/`resolve` stay `pub mod` per confirmed external deep-path usage); spot-checked `Stamps`/`AnalogCapability` traits are un-exported (confirmed above under CGA-09).

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| ----------- | ---------------- | ---------- |
| CGA-01 | Pending | ✅ Verified |
| CGA-02 | Pending | ✅ Verified |
| CGA-03 | Pending | ✅ Verified |
| CGA-04 | Pending | ✅ Verified |
| CGA-05 | Pending | ✅ Verified |
| CGA-06 | Pending | ✅ Verified |
| CGA-07 | Pending | ✅ Verified (minor doc nit noted) |
| CGA-08 | Pending | ✅ Verified |
| CGA-09 | Pending | ✅ Verified |
| CGA-10 | Pending | ✅ Verified |

---

## Summary

**Overall**: ✅ Ready

**Spec-anchored check**: 10/10 ACs matched spec outcome (1 minor non-blocking doc-accuracy nit under CGA-07)
**Sensor**: 2/3 mutations killed; 1 survivor traced to a pre-existing (pre-refactor) coverage gap, not a refactor regression
**Gate**: 582 passed, 0 failed — identical test count before/after, zero warnings

**What works**: The module tree now mirrors the pipeline (`resolve → flatten → emit → kernel → device`) exactly as designed; the `ir` alias and `jit`/inner-`codegen` names are gone; `AnalogKernel` is a lean `core` + `Option<Capability>` struct with presence-based `has_*` queries; `emit_analog`/`builder.rs` are split by responsibility; `device/analog.rs` and `device/circuit.rs` are decomposed along capability/responsibility lines with internal-only `Stamps`/`AnalogCapability` traits that never leak into the solver-facing `Element` ABI (still flat, MD-01 upheld); `CodegenError`/`SimCtx` live in their designed homes; the full 582-test suite is unchanged and green.

**Issues found**:
1. (Cosmetic) `device/mod.rs:8` has a broken intra-doc link (`crate::resolve::IrProgram`) — a pre-existing stale reference mechanically touched (not fixed) by T2's alias rename. Not a CGA-07 violation (the AC targets the error message, which is correct) but worth a one-line fix in a future docs pass.
2. (Pre-existing, out of scope) `Limiter::update`'s convergence-veto tolerance (`device/analog/limits.rs:122`) has no test that specifically exercises its effect on Newton convergence — confirmed pre-existing via `git show 78d31b3`, not introduced by this refactor. Logged here for visibility, not a blocker for this feature.

**Next steps**: None required to close this feature. Optionally: (a) fix the `device/mod.rs:8` doc link in a follow-up docs commit, (b) file a ROADMAP/tracker item for a `$limit` convergence-veto regression test (pre-existing gap, unrelated to this refactor).

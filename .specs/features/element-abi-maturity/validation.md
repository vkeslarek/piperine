# Element ABI Maturity — Validation

**Date**: 2026-07-23
**Spec**: `.specs/features/element-abi-maturity/spec.md`
**Diff range**: `2617c3f^..HEAD` (30 commits, 32 tasks T1–T32)
**Verifier**: independent sub-agent (author ≠ verifier)
**Method**: evidence-or-zero — every AC traced to a `file:line` + assertion expression

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1   | ✅ Done | `ElementCheckpoint` + `checkpoint_state`/`restore_state` defaults on `Element` (`element.rs:117,508,515`) |
| T2   | ✅ Done | Transient reject path calls `restore_device_checkpoints` (`transient.rs:1262,1282`) |
| T3   | ✅ Done | DC homotopy bracketed by `checkpoint_devices`/`restore_devices` (`convergence.rs:398-418`) |
| T4   | ✅ Done | PiperineDevice limiter checkpoint/restore (`device/mod.rs:655,681`) |
| T5   | ✅ Done | PiperineDevice digital register checkpoint/restore (same methods) |
| T6   | ✅ Done | `LimitingReport`/`LimitReason` + `limiting_report` default-None (`element.rs:131,171`) |
| T7   | ✅ Done | Newton gate + DC bypass route through `any_limiting_report`/`apply_limiting_reports` |
| T8   | ✅ Done | Codegen produces `LimitingReport` with `"pnjlim"`/`VoltageStep`; dead methods removed (`grep` zero) |
| T9   | ✅ Done | `setup_all` calls `set_temperature` after `allocate_unknowns`; sweep honors `Invalidation::Temperature` |
| T10  | ✅ Done | PiperineDevice composes `t_nominal + dtemp` (`device/mod.rs:167-172`) |
| T11  | ✅ Done | `HAS_DISTO2/HAS_DISTO3/NUMERIC_JACOBIAN` bits defined (`element.rs:94-106`) |
| T12  | ✅ Done | `capabilities()` mirrors `kernel.has_disto2()/has_disto3()` (`device/mod.rs:605-613`) |
| T13  | ✅ Done | `capability_prescan` warns + fails loud (`disto.rs:324-355`) |
| T14  | ✅ Done | `TerminalKind { External, Internal, Auxiliary }` (`introspect.rs:175-183`) |
| T15  | ✅ Done | Analog kernel → `list_terminals` bridge (`device/mod.rs:439`) |
| T16  | ✅ Done | Digital kernel → `list_terminals` extension |
| T17  | ✅ Done | Opvar compilation path; `eval_opvars` reads module vars |
| T18  | ✅ Done | `read_opvars`/`list_queries` bridges (`device/mod.rs:353,365`) |
| T19  | ✅ Done | `ModelDescriptor` + state/force/noise catalogs (`device/mod.rs:488,502,511,520`) |
| T20  | ✅ Done | `ConvergencePlan::stepper` field + `with_stepper` (`convergence.rs:296,342`) |
| T21  | ✅ Done | Parity baselines unchanged + custom-stepper test |
| T22  | ✅ Done | `EventEntry/EventKind/RollbackBehavior` types (`events.rs:37-137`) |
| T23  | ✅ Done | `EventQueue` with checkpoint/rollback (`events.rs:267-422`) |
| T24  | ✅ Done | Digital adapter (`events.rs:295-301`) |
| T25  | ✅ Done | Breakpoint/scheduled-set/step-hint adapters (`events.rs:308-331`) |
| T26  | ✅ Done | `predict_step` reads from `EventQueue` (`transient.rs:849-918`) |
| T27  | ✅ Done | `rollback()` honors per-entry `RollbackBehavior` (`events.rs:383-406`; `transient.rs:1247,1287`) |
| T28  | ✅ Done | `ObservableDescriptor` + `ProbeSelection` (`introspect.rs:213-277`) |
| T29  | ✅ Done | `collect_device_banks` filters by `ProbeSelection` (`transient.rs:1094-1105`) |
| T30  | ✅ Done | Fail-loud unknown device/observable at setup (`probe_selection.rs:296-321`) |
| T31  | ✅ Done | Part VII §19 lifecycle chart + algorithm flow (`docs/spec/part_vii_solver.md:1224-1521`) |
| T32  | ✅ Done | Lifecycle contract test asserting hook ordering per analysis (`lifecycle.rs:347-531`) |

**All 32 tasks marked done.** No partials, no blockers.

---

## Spec-Anchored Acceptance Criteria

### P1: Rollback lifecycle (ABI-01..08)

| Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-01: solver calls `checkpoint_state` before each attempt | `Option<ElementCheckpoint>` per element, before settle+solve | `crates/piperine-solver/tests/checkpoint_restore.rs:119` — `ckpt = dev.checkpoint_state().expect(...)`; `transient.rs:935` — `snapshot_device_checkpoints()` | ✅ PASS |
| ABI-02: rejected step calls `restore_state` | restore BEFORE retry, on every checkpointed element | `checkpoint_restore.rs:331` — `assert!(n_restores > 0)`; `checkpoint_restore.rs:333` — `assert_eq!(n_restores, res.stats.steps_rejected)` | ✅ PASS |
| ABI-03: accepted step discards checkpoint | NO restore on accept | `checkpoint_restore.rs:339` — `assert!(n_checkpoints > n_restores)` | ✅ PASS |
| ABI-04: limiter state rewound | `active`, `seeds`, vold restored to pre-attempt | `crates/piperine-codegen/tests/checkpoint_limiter.rs:122-129` — `assert_eq!(restored_ckpt.real_state.first(), active0)` + `assert_eq!(dev.runtime_banks().0, vold0)` | ✅ PASS |
| ABI-05: digital registers rewound | `vars_int`, `vars_real`, `prev_watch` restored | `crates/piperine-codegen/tests/checkpoint_digital.rs:87-91` — `assert_eq!(restored_hidden, hidden0, "registers + watch memory restored")` | ✅ PASS |
| ABI-06: default None = zero cost | `checkpoint_state()` returns `None` by default | `checkpoint_restore.rs:32` — `assert!(dev.checkpoint_state().is_none())` | ✅ PASS |
| ABI-07: DC homotopy retry calls restore | restore before next strategy attempt | `checkpoint_restore.rs:476-478` — `assert!(n_restores >= 1)` + `472` — `assert_eq!(res.stats.homotopy_strategy.as_deref(), Some("gmin-stepping"))`; `convergence.rs:404,415` | ✅ PASS |
| ABI-08: multiple rejects, fresh checkpoint each | `n_restores == steps_rejected`, no stacking | `checkpoint_restore.rs:333-336` — `assert_eq!(n_restores, res.stats.steps_rejected)` + `345` — `assert!(n_checkpoints >= accepted + rejected)` | ✅ PASS |

### P2: Formal limiting API (ABI-09..13)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-09: `LimitingReport` struct replaces boolean | `{ net, proposed, limited_value, limiter_name, reason }` | `checkpoint_limiter.rs:185-198` — `report = dev.limiting_report().expect(...)`; asserts `limiter_name == "pnjlim"`, `reason == VoltageStep`, `net.idx().is_some()`, `limited_value != proposed` | ✅ PASS |
| ABI-10: solver applies `limited_value` to Newton guess | guess reaches `lim` within first 3 loads | `crates/piperine-solver/tests/limiting_report.rs:144-149` — `assert!(seq.iter().take(3).any(|&v| (v - 0.7).abs() < 1e-9))` | ✅ PASS |
| ABI-11: None when inactive | default `None` | `checkpoint_restore.rs:95` — `assert!(dev.limiting_report().is_none())`; `checkpoint_limiter.rs:217` — `assert!(dev.limiting_report().is_none(), "idle limiter reports None")` | ✅ PASS |
| ABI-12: host-readable diagnostics | limiter name + reason exposed | `checkpoint_limiter.rs:186-187` — `assert_eq!(report.limiter_name, "pnjlim")` + `assert_eq!(report.reason, LimitReason::VoltageStep)` | ✅ PASS |
| ABI-13: no dead methods left behind | grep `limiting_active`/`convergence_hint`/`ConvergenceHint` = 0 | `grep -rn "limiting_active\|convergence_hint\|ConvergenceHint" crates/ src/` returns **zero** | ✅ PASS |

### P3: Lifecycle contract (ABI-14..18)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-14: documented hook chart per analysis | Part VII §19.2–19.7 each has chart | `crates/piperine-solver/tests/lifecycle.rs:528` — `assert!(body.contains("Hook ordering table"))` for all 6 analyses | ✅ PASS |
| ABI-15: algorithm flow description per analysis | "Algorithm flow" header in each section | `lifecycle.rs:524` — `assert!(body.contains("Algorithm flow"))` | ✅ PASS |
| ABI-16: executable contract test | recording Element; one test per analysis | `lifecycle.rs:349,366,391,417,449,466` — six `*_hook_ordering_matches_chart` tests | ✅ PASS |
| ABI-17: rollback hooks in chart + algorithm | `checkpoint_state` in tran chart | `lifecycle.rs:402-404` — assert ordered subsequence includes `"checkpoint_state"`; Part VII §19.4 chart rows 5 + 11 (`ckpt`/`restore`) | ✅ PASS |
| ABI-18: temperature in chart | `set_temperature` after `setup`, before first load | `lifecycle.rs:359` — DC ordering includes `"set_temperature"` between `"setup"` and `"load_dc"`; Part VII §19.2 chart row 2 | ✅ PASS |

### P4: Temperature protocol (ABI-19..22)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-19: set_temperature in setup | called after `allocate_unknowns`, before first load | `crates/piperine-solver/tests/temperature.rs:174-186` — `temps` contains `run_temperature` + `assert!(set_at < load_at)` | ✅ PASS |
| ABI-20: sweep drives invalidation | `set_temperature(t_new)` honored → `Invalidation::Temperature` | `temperature.rs:222-232` — `assert_eq!(inv, Invalidation::Temperature)` + `temps` contains `sweep_t` | ✅ PASS |
| ABI-21: per-instance dtemp | effective `t_nominal + dtemp` | `crates/piperine-codegen/tests/temperature_dtemp.rs:75-79` — `assert!((cached - (tnom + dtemp)).abs() < 1e-9)` | ✅ PASS |
| ABI-22: Rebuild fail-loud | never silently no-ops | `temperature_dtemp.rs:196-200` — `assert_eq!(inv, Invalidation::Temperature, "never Rebuild")` | ✅ PASS |

### P5: Jacobian/stamp capability (ABI-23..26)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-23: capability descriptor | `HAS_DISTO2/HAS_DISTO3/NUMERIC_JACOBIAN` bits | `crates/piperine-codegen/tests/disto_capabilities.rs:48-54` — `caps.contains(HAS_DISTO2)` + `HAS_DISTO3` | ✅ PASS |
| ABI-24: .disto warns when absent | named diagnostic, not silent zero | `crates/piperine-solver/src/analyses/disto.rs:1237-1245` (inline test) — `assert!(result.warnings.iter().any(|w| w.contains("no device provides disto2 capability") && w.contains("HD2")))` | ✅ PASS |
| ABI-25: numeric fail-loud | `Err` naming the offending device | `disto.rs:1305-1308` (inline test) — `msg.contains("numeric_dev")` + `msg.contains("numeric-only Jacobian")` + `msg.contains("analytic derivatives")` | ✅ PASS |
| ABI-26: JIT devices declare analytic | MOSFET declares both; resistor neither; never NUMERIC | `disto_capabilities.rs:67-128` — resistor `!HAS_DISTO2`, `!HAS_DISTO3`; `disto_capabilities.rs:124` — `!NUMERIC_JACOBIAN` | ✅ PASS |

### P6: Terminal descriptors + opvar catalog (ABI-27..31)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-27: analog kernel → ABI | named descriptors per terminal | `crates/piperine-codegen/tests/terminal_bridge.rs:69-77` — `by_name.get("p") == Some(&(External, Analog))`; cp/bp/ep marked Internal | ✅ PASS |
| ABI-28: digital kernel → ABI | per input/output with correct direction | `terminal_bridge.rs:184-198` — inputs `External/Digital`; `204-225` — `Direction::In`/`Out` mapping | ✅ PASS |
| ABI-29: internal/auxiliary kind | `TerminalKind::Internal` for non-port wires | `terminal_bridge.rs:79-83` — `by_name.get("mid") == Some(&(Internal, Analog))` | ✅ PASS |
| ABI-30: read_opvars populated | post-solve values, not empty | `crates/piperine-codegen/tests/opvar_bridge.rs:55-59` — `(g - 1.0e-3).abs() < 1.0e-12` | ✅ PASS |
| ABI-31: list_queries catalog | typed `QueryDescriptor` per opvar | `opvar_bridge.rs:145-146` — `by_name.get("g") == Some(&QueryKind::OperatingVariable)` | ✅ PASS |

### P7: Save/probe selection (ABI-32..35)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-32: observable catalog | `ObservableDescriptor { name, kind, cost }` | `crates/piperine-codegen/tests/observable_catalog.rs:41-44` — `o.kind == State && o.name.starts_with("ddt[")`; `154-176` — `cost ∈ [0,1]` | ✅ PASS |
| ABI-33: ProbeSelection in options | field + builder | `crates/piperine-solver/tests/probe_selection.rs:118-120` — `opts.probe_selection.requests.len() == 2` + `contains(...)` | ✅ PASS |
| ABI-34: per-observable recording | only requested observables in `device_state` | `probe_selection.rs:171-176` — `banks.0.len() == 1, banks.1.is_empty()`; `257-268` — 10 devices → only `dev3` recorded | ✅ PASS |
| ABI-35: fail-loud unknown | named error at setup | `probe_selection.rs:303-306` — `msg.contains("device `stub` has no observable `nonexistent`")`; `316-319` — `msg.contains("device `ghost` not found")` | ✅ PASS |

### P8: Unified event model (ABI-36..41)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-36: unified queue type | `EventQueue` with `(time, priority)` ordering | `crates/piperine-solver/src/analyses/events.rs:450-454` (inline test) — `early < late`, `Exact < Advisory`; `458-471` peek returns earliest | ✅ PASS |
| ABI-37: digital events in queue | `kind=Digital`, `priority=Exact`, `rollback=Restore` | `crates/piperine-solver/tests/digital_events_unified.rs:32-36` — all three fields asserted | ✅ PASS |
| ABI-38: breakpoints + scheduled sets in queue | `kind=Breakpoint`, `Exact`, breakpoint=RePoll / set=Restore | `crates/piperine-solver/tests/event_adapters.rs:26-31` (breakpoint); `63-67` (set: `Restore` + `ScheduledSet`) | ✅ PASS |
| ABI-39: $bound_step in queue | `kind=StepHint`, `Advisory`, `Discard` | `event_adapters.rs:98-103` — all three fields asserted | ✅ PASS |
| ABI-40: analog crossings in queue | `kind=Crossing`, `Advisory`, `Discard`; new capability | `crates/piperine-solver/src/analyses/events.rs:517-520` (inline test) — constructor sets Crossing/Advisory/Discard | ✅ PASS (type/queue path; A2D crossing detector is a separate future feature — spec marks it "new capability") |
| ABI-41: per-entry rollback behavior | Restore returns; RePoll/Discard stay out | `crates/piperine-solver/tests/event_rollback.rs:184-203` (mixed sources survive rejects); `event_adapters.rs:38-49,120-132` (per-kind behavior); `events.rs:581-583` (digital+set return, breakpoint+hint stay out) | ✅ PASS |

### P9: Stepper strategy composition (ABI-42..45)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-42: StepperStrategy in ConvergencePlan | field + builder + accessor | `crates/piperine-solver/tests/stepper_fold.rs:270-271` — `plan.stepper_mut().propose_dt(...)` | ✅ PASS |
| ABI-43: rejection routes through plan | delegate reject_dt to `plan.stepper()` | `transient.rs:1248,1284` — `self.plan.stepper_mut().reject_dt(...)`; `stepper_fold.rs:257-260` — `n_propose > 0` | ✅ PASS |
| ABI-44: parity baselines bit-identical | unchanged step sequence | `crates/piperine-solver/tests/parity_baseline.rs:425-428` — `(t_last - 1e-3).abs() < 1e-12`, `(v_last - (-0.450_485_218_772_388_9)).abs() < 1e-9`, `res.len() == 386` | ✅ PASS |
| ABI-45: custom stepper via plan | test double routed through plan | `stepper_fold.rs:241-252,258-260` — `HalvingStepper` counters > 0 via `with_stepper` | ✅ PASS |

### P10: Introspect leftovers (ABI-46..48)

| Criterion | Spec outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- |
| ABI-46: ModelDescriptor (type id/version) | `{ type_id, version }` from kernel | `crates/piperine-codegen/tests/model_descriptor.rs:49-50` — `assert_eq!(descriptor.type_id, "RCap")` + `version == ""`; default `72-74` | ✅ PASS |
| ABI-47: named state/force/noise catalogs | non-empty for reactive/noisy devices | `model_descriptor.rs:93-98` (state: `ddt[`), `125-127` (vold), `146-147` (force `(p,n)`), `170-172` (noise `(p,n)`) | ✅ PASS |
| ABI-48: unified "what can I report" | union of terminals+opvars+state+force+noise | `model_descriptor.rs:201-208` — all non-empty where the device has data; identity present | ✅ PASS |

**Spec-anchored status**: ✅ All 48 ACs covered. 0 spec-precision gaps. ABI-40's analog-crossing detector is noted as a forward-declared future feature (the spec itself calls it "new capability"); the queue + entry path is fully covered.

---

## Edge Cases

- [x] **SUPPORTS_ROLLBACK but `checkpoint_state` returns None** → stateless path. `checkpoint_restore.rs:30-33` asserts default None; `transient.rs` restore loop iterates `Vec<Option<ElementCheckpoint>>` honoring `None`.
- [x] **Checkpoint taken but element destroyed mid-step** → live-param rebuild mid-step fails loud (`transient.rs:1054` — `Err` on `Invalidation::Rebuild`); checkpoint vector is dropped with the solver.
- [x] **`.disto` with some disto2 but no disto3** → disto2 results valid; disto3 named warning. `disto.rs:341-353` (capability_prescan emits each warning independently).
- [x] **Temperature sweep hits default-no-op device** → backward compatible. `temperature.rs:238-252` (`default_set_temperature_is_noop`).
- [x] **Empty event queue** → fall back to PI-proposed dt. `transient.rs:892` guarded by `if let Some(front) = self.event_queue.peek()`.
- [x] **ProbeSelection on device with `record_device_state = false`** → per-device selection wins. `probe_selection.rs:161-176` (default `false` + non-empty selection records); `transient.rs:1097` — gate is `record_device_state || !probe_selection.requests.is_empty()`.

---

## Gate Check

- **Gate command**: `cargo build --workspace && cargo test --workspace`
- **Build**: clean (only unrelated pre-existing `piperine-python .so not found` warning).
- **Result**: **814 passed, 0 failed, 5 ignored**.
- **Test count before feature**: ~705
- **Test count after feature**: 814
- **Delta**: **+109 new tests** (matches "~814+" expectation).
- **Skipped (5)**: 1 plugin doctest (`#[ignore]`), 1 wasm doctest, 3 solver doctests (`#[ignore]`). All pre-existing — unrelated to this feature.
- **Failures**: none.

New test files added by this feature:
- `crates/piperine-solver/tests/checkpoint_restore.rs` (8 tests)
- `crates/piperine-solver/tests/limiting_report.rs` (2 tests)
- `crates/piperine-solver/tests/temperature.rs` (3 tests)
- `crates/piperine-solver/tests/probe_selection.rs` (10 tests)
- `crates/piperine-solver/tests/stepper_fold.rs` (3 tests)
- `crates/piperine-solver/tests/digital_events_unified.rs` (6 tests)
- `crates/piperine-solver/tests/event_adapters.rs` (8 tests)
- `crates/piperine-solver/tests/event_rollback.rs` (2 tests)
- `crates/piperine-codegen/tests/checkpoint_limiter.rs` (5 tests)
- `crates/piperine-codegen/tests/checkpoint_digital.rs` (4 tests)
- `crates/piperine-codegen/tests/disto_capabilities.rs` (3 tests)
- `crates/piperine-codegen/tests/terminal_bridge.rs` (5 tests)
- `crates/piperine-codegen/tests/opvar_bridge.rs` (5 tests)
- `crates/piperine-codegen/tests/observable_catalog.rs` (5 tests)
- `crates/piperine-codegen/tests/model_descriptor.rs` (6 tests)
- `crates/piperine-codegen/tests/temperature_dtemp.rs` (4 tests)
- Plus inline tests in `disto.rs` (3) and `events.rs` (9)

---

## Discrimination Sensor

Five behavior-level mutations injected in a `git worktree` scratch state (never the real tree), each followed by the targeted test run, then discarded.

| # | Mutation site | Fault injected | Killed by | Killed? |
| - | ------------- | -------------- | --------- | ------- |
| 1 | `transient.rs:1262` | Commented out `self.restore_device_checkpoints(&device_checkpoints)` in `reject_lte_step` (replaced with `let _ = &device_checkpoints;`) | `checkpoint_restore::transient_reject_drives_restore_accept_discards` panicked at line 331 ("expected rejections to call restore_state") | ✅ Killed |
| 2 | `disto.rs:329-337` | Commented out the `NUMERIC_JACOBIAN` fail-loud block in `capability_prescan` | `analyses::disto::tests::numeric_jacobian_device_fails_loud_at_prescan` panicked at line 1303 ("must fail loud at the .disto pre-scan") | ✅ Killed |
| 3 | `device/mod.rs:167-172` | Made `set_temperature` a no-op (commented the body, kept signature) | `temperature_dtemp::dtemp_instance_composes_into_effective_temperature` + `no_dtemp_param_caches_received_temperature` panicked (no cached temperature) | ✅ Killed |
| 4 | `events.rs:232-238` | Flipped the time comparison in `Ord::cmp` (`self.time.total_cmp` → `other.time.total_cmp`) | 4 events tests panicked: `event_entry_orders_by_time_then_priority`, `binary_heap_peek_returns_earliest_via_reverse`, `drain_due_returns_earliest_entries_in_time_order`, `all_four_sources_coexist_in_unified_queue` | ✅ Killed |
| 5 | `device/mod.rs:178-182` | Made `limiting_report()` always return `None` (commented the analog delegation) | `checkpoint_limiter::piperine_device_produces_limiting_report_when_clamping` panicked at line 185 ("limiter produced a report while clamping") | ✅ Killed |

**Sensor depth**: lightweight (5 mutations targeting the highest-risk new code).
**Result**: **5/5 killed — PASS ✅**. The test suite is discriminating for all five behavior-level regressions.

---

## Code Quality

| Principle | Status |
| --- | --- |
| No features beyond what was asked | ✅ |
| No abstractions for single-use code | ✅ (e.g. `EventQueue::push_*` adapters are one-liners over `push`; no over-engineering) |
| No unnecessary "flexibility" added | ✅ |
| Only touched files required for task | ✅ (42 files; surgical across 30 commits) |
| Didn't "improve" unrelated code | ✅ |
| Matches existing patterns/style | ✅ (matches `bitflags!` pattern, `DigitalState::Checkpoint` one-deep semantics, `HomotopyStrategy` trait shape) |
| Would senior engineer approve? | ✅ |
| Tests map to ACs and are non-shallow | ✅ (spot-check: `transient_reject_drives_restore_accept_discards` asserts `n_restores == steps_rejected`, not just `> 0`) |
| Spec-anchored outcome check | ✅ (every AC traced to a spec-outcome-targeted assertion) |
| Per-layer Coverage Expectation met | ✅ (1:1 AC↔test mapping; happy+edge+error paths covered) |
| Every test in scope maps to a spec AC | ✅ |
| Documented guidelines followed | ✅ (`AGENTS.md` — zero rustc warnings, `piperine-solver` never depends on `piperine-codegen`, fail-loud at host boundary) |

**AGENTS.md rule compliance:**

- **No `unwrap()`/`expect()` on user paths** ✅ — grep over the diff's `src/` additions returns 8 hits, ALL inside `#[cfg(test)] mod tests` blocks (disto.rs:687, events.rs:426). Production paths return `crate::result::Result`.
- **No loose functions** ✅ — `git diff` grep for `^\+pub(crate) fn` / `^\+fn` in events.rs/element.rs/introspect.rs/device-mod.rs returns zero. Every new function is a method on a struct/enum or a trait method.
- **No macros** ✅ — zero `macro_rules!` added (the existing `bitflags!` use is the established pattern for capability flags).
- **Modules organized by system function** ✅ — new module `analyses/events.rs` (named for what it does); types added to `core/element.rs` (`ElementCheckpoint`, `LimitingReport`) and `core/introspect.rs` (`ModelDescriptor`, `TerminalKind`, `ObservableDescriptor`, `ProbeSelection`) in the right place.
- **Fail loud** ✅ — `CodegenError::Unsupported`, `Error::simple(SolverDomain::Element, ...)` with named diagnostics everywhere the spec demands it; never silent `0.0`/no-op.

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| ----------- | --------------- | ---------- |
| ABI-01..48 (all) | Design / Pending | ✅ Verified |

---

## Summary

**Overall**: ✅ **Ready**

**Spec-anchored check**: 48/48 ACs matched spec outcome; 0 spec-precision gaps.
**Sensor**: 5/5 mutations killed.
**Gate**: 814 passed, 0 failed, 5 ignored (pre-existing).

**What works**:
- Rollback lifecycle (transient reject + DC homotopy) correctly rewinds limiter + digital registers; default `None` is zero-cost.
- `LimitingReport` replaces `limiting_active`/`convergence_hint` with no dead methods.
- `.disto` pre-scan warns on missing derivative orders and fails loud on `NUMERIC_JACOBIAN`.
- Temperature protocol wires `set_temperature` into setup + sweep, composes per-instance `dtemp`.
- JIT device surfaces terminals, opvars, observables, state/force/noise catalogs, and model identity through the standard `Introspect` trait.
- `ProbeSelection` records O(requested × steps) memory; fail-loud on typo.
- Unified `EventQueue` with per-entry `RollbackBehavior`; `predict_step` reads one queue.
- `StepperStrategy` folded into `ConvergencePlan`; parity baselines bit-identical.
- Part VII §19 documents hook chart + algorithm flow per analysis; executable contract test enforces it.

**Issues found**: none.

**Next steps**: none — feature complete and verified.

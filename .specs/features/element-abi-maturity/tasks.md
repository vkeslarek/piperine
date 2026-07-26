# Element ABI Maturity Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/element-abi-maturity/design.md`
**Spec**: `.specs/features/element-abi-maturity/spec.md`
**Status**: Draft

---

## Test Coverage Matrix

> Generated from `AGENTS.md` (Test placement table), `CLAUDE.md`, and codebase sampling. Guidelines found: `AGENTS.md`, `CLAUDE.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver ABI (Element trait, ElementCapabilities, Introspect, UnknownAllocator) | unit | All branches; 1:1 to spec ACs; all listed edge cases | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Solver analyses (transient, dc, disto, convergence, noise) | integration | All paths in scope: happy + reject + edge + error | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Solver digital (state, events, scheduler) | integration | All event kinds + rollback paths | `crates/piperine-solver/tests/{mixed_signal,digital_topology}.rs` | `cargo test -p piperine-solver` |
| Codegen device (PiperineDevice, AnalogInstance, Limiter, DigitalInstance) | unit | All branches; 1:1 to spec ACs; all listed edge cases | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Codegen kernel (AnalogKernel, DigitalKernel catalogs) | unit | Bridge correctness + accessor parity | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Documentation (Part VII lifecycle chart) | none | Completeness check in contract test | `docs/spec/part_vii_solver.md` | build gate only |

## Gate Check Commands

> Generated from `Cargo.toml` workspace + `AGENTS.md` build/test instructions.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick (solver) | After solver-only tasks | `cargo test -p piperine-solver` |
| Quick (codegen) | After codegen-only tasks | `cargo test -p piperine-codegen` |
| Full | After cross-crate tasks (solver+codegen) | `cargo test --workspace` |
| Build | After phase completion or doc-only tasks | `cargo build --workspace && cargo test --workspace` |

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before the next begins, and tasks within a phase execute in order.

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7
```

### Phase 1: Rollback + Limiting (ABI-01..13)

Core correctness fix (rollback) + limiting API replacement. Both touch the Newton convergence gate.

```
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8
```

### Phase 2: Temperature + Jacobian (ABI-19..26)

Independent ABI additions: temperature protocol wiring + derivative capability declarations.

```
T9 → T10 → T11 → T12 → T13
```

### Phase 3: Terminals + Opvars + Introspect (ABI-27..31, 46..48)

Codegen→ABI bridge: kernel catalogs surfaced through the introspection surface.

```
T14 → T15 → T16 → T17 → T18 → T19
```

### Phase 4: Stepper fold (ABI-42..45)

Pure refactor: `StepperStrategy` into `ConvergencePlan`. Parity-gated.

```
T20 → T21
```

### Phase 5: Unified event model (ABI-36..41)

One typed queue replacing four ad-hoc sources. Depends on Phase 1 (per-entry rollback behavior).

```
T22 → T23 → T24 → T25 → T26 → T27
```

### Phase 6: Save/probe selection (ABI-32..35)

Per-observable recording. Independent, lowest risk.

```
T28 → T29 → T30
```

### Phase 7: Lifecycle contract (ABI-14..18)

Documents the final state. Must land last.

```
T31 → T32
```

---

## Task Breakdown

### T1: ElementCheckpoint type + checkpoint/restore hooks

**What**: Add `ElementCheckpoint` struct + `checkpoint_state()`/`restore_state()` default methods on `Element`; activate `SUPPORTS_ROLLBACK` from "Reserved".
**Where**: `crates/piperine-solver/src/core/element.rs`
**Depends on**: None
**Reuses**: `(Vec<i64>, Vec<f64>)` carrier from `digital_hidden_snapshot` (`element.rs:289`)
**Requirement**: ABI-01, ABI-06

**Done when**:
- [ ] `ElementCheckpoint { int_state: Vec<i64>, real_state: Vec<f64> }` defined + exported in `abi`
- [ ] `fn checkpoint_state(&self) -> Option<ElementCheckpoint> { None }` on `Element`
- [ ] `fn restore_state(&mut self, _ckpt: &ElementCheckpoint) {}` on `Element`
- [ ] `SUPPORTS_ROLLBACK` doc updated: no longer "Reserved" — describes the checkpoint/restore lifecycle
- [ ] Unit test: default `checkpoint_state` returns `None`; default `restore_state` is a no-op
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T2: Wire checkpoint/restore into transient reject path

**What**: Call `checkpoint_state()` in `attempt_step` before the digital settle; call `restore_state()` in `reject_step` + `reject_lte_step`; discard on `accept_step`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (`attempt_step:790`, `reject_lte_step:1069`, `reject_step:1105`, `accept_step:837`)
**Depends on**: T1
**Reuses**: `StepAttempt` struct (`transient.rs:426`) — add a `device_checkpoints: Vec<Option<ElementCheckpoint>>` field
**Requirement**: ABI-02, ABI-03, ABI-08

**Done when**:
- [ ] `attempt_step` calls `checkpoint_state()` per element before digital settle + Newton solve
- [ ] `reject_step` + `reject_lte_step` call `restore_state()` per checkpointed element before retry
- [ ] `accept_step` discards the checkpoints (no restore called)
- [ ] Multiple rejects: each attempt takes a fresh checkpoint, each reject restores from it
- [ ] Integration test: a circuit forced through step rejection (tight LTE tol) — assert `restore_state` was called on reject, not on accept
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T3: Wire checkpoint/restore into DC homotopy retry

**What**: Call `checkpoint_state()` before `ConvergencePlan::solve()`; call `restore_state()` on homotopy strategy fallthrough (before next strategy attempt).
**Where**: `crates/piperine-solver/src/analyses/dc.rs` (homotopy cascade around `convergence.rs:298-368`)
**Depends on**: T1
**Reuses**: `ConvergencePlan::solve` return (`PlanOutcome`) — fallthrough is the trigger
**Requirement**: ABI-07

**Done when**:
- [ ] DC solver checkpoints before each homotopy attempt
- [ ] On strategy fallthrough, `restore_state()` called before next strategy
- [ ] Integration test: a circuit with gmin-stepping fallthrough (hard convergence) — assert `restore_state` fired between strategies
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T4: PiperineDevice checkpoint limiter state

**What**: Override `checkpoint_state`/`restore_state` on `PiperineDevice` to snapshot/restore the limiter (`active`, `seeds`) and the vold slots (`state[limit_base..]`).
**Where**: `crates/piperine-codegen/src/device/mod.rs` (PiperineDevice impl), `crates/piperine-codegen/src/device/analog/mod.rs` (AnalogInstance checkpoint method)
**Depends on**: T1
**Reuses**: `Limiter { active, seeds }` (`limits.rs:13-19`), `AnalogInstance::state` (`analog/mod.rs:84`)
**Requirement**: ABI-04

**Done when**:
- [ ] `PiperineDevice::checkpoint_state` returns `Some(ElementCheckpoint)` when the analog instance has a limiter with active state
- [ ] `PiperineDevice::restore_state` rewinds `limiter.active`, `limiter.seeds`, and the vold slots to the checkpoint
- [ ] `capabilities()` includes `SUPPORTS_ROLLBACK` when the device has a limiter
- [ ] Unit test: limiter state after a rejected step matches the pre-attempt state
- [ ] Gate: `cargo test -p piperine-codegen`

**Tests**: unit
**Gate**: quick (codegen)

---

### T5: PiperineDevice checkpoint digital registers

**What**: Extend `PiperineDevice::checkpoint_state`/`restore_state` to snapshot/restore digital `vars_int`, `vars_real`, and `prev_watch`.
**Where**: `crates/piperine-codegen/src/device/digital.rs` (DigitalInstance checkpoint method)
**Depends on**: T4
**Reuses**: `DigitalInstance { vars_int, vars_real, prev_watch }` (`digital.rs:97-100`)
**Requirement**: ABI-05

**Done when**:
- [ ] Checkpoint includes digital `vars_int`/`vars_real`/`prev_watch` when the device has a digital instance
- [ ] Restore rewinds all three to the checkpoint
- [ ] Unit test: digital register state after a rejected settle matches the pre-settle state
- [ ] Gate: `cargo test -p piperine-codegen`

**Tests**: unit
**Gate**: quick (codegen)

---

### T6: LimitingReport type + limiting_report() hook

**What**: Add `LimitingReport`/`LimitReason` structs + `limiting_report()` default-None method on `AnalogDevice`.
**Where**: `crates/piperine-solver/src/core/element.rs`
**Depends on**: None
**Reuses**: `AnalogReference` (existing), `ConvergenceHint` shape (being replaced)
**Requirement**: ABI-09, ABI-11

**Done when**:
- [ ] `LimitingReport { net, proposed, limited_value, limiter_name, reason }` defined + exported in `abi`
- [ ] `LimitReason` enum defined (`VoltageStep`, `VdsStep`, `Other`)
- [ ] `fn limiting_report(&self) -> Option<LimitingReport> { None }` on `AnalogDevice`
- [ ] Unit test: default returns `None`
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T7: Wire LimitingReport into Newton convergence + DC bypass

**What**: Replace `any_limiting()`/`apply_convergence_hints()` with `any_limiting_report()`/`apply_limiting_reports()`; update the Newton gate (`newton_raphson.rs:371-383`) and DC bypass gate (`dc.rs:123`).
**Where**: `crates/piperine-solver/src/core/circuit.rs`, `analyses/dc.rs`, `analyses/transient.rs`, `math/newton_raphson.rs`
**Depends on**: T6
**Reuses**: Existing `apply_convergence_hints` hard-overwrite semantics (`circuit.rs:404`)
**Requirement**: ABI-10

**Done when**:
- [ ] `NonLinearSystem::any_limiting_report()` replaces `any_limiting()` — scans `limiting_report().is_some()`
- [ ] `CircuitInstance::apply_limiting_reports()` replaces `apply_convergence_hints()` — applies `report.limited_value` to `report.net`
- [ ] Newton gate (`newton_raphson.rs:375`) uses `any_limiting_report()`
- [ ] DC bypass gate (`dc.rs:123`) uses `!self.any_limiting_report()`
- [ ] `apply_limiting_reports` called at `newton_raphson.rs:371` before the convergence test
- [ ] Integration test: MOSFET DC sweep through subthreshold — limiting gate fires, limited value applied
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T8: Codegen Limiter produces LimitingReport; remove dead methods

**What**: Override `limiting_report()` on `PiperineDevice` to produce a `LimitingReport` from the `Limiter` state; remove `limiting_active()`, `convergence_hint()`, `ConvergenceHint` struct.
**Where**: `crates/piperine-codegen/src/device/mod.rs` (PiperineDevice), `analog/limits.rs` (Limiter), `crates/piperine-solver/src/core/element.rs` (remove old methods + struct), `prelude.rs`/`abi.rs` (remove re-exports)
**Depends on**: T7
**Reuses**: `Limiter::update` already computes the clamped values (`limits.rs:95-128`)
**Requirement**: ABI-12, ABI-13

**Done when**:
- [ ] `PiperineDevice::limiting_report()` returns `Some(LimitingReport)` when `limiter.active` — with `net`, `proposed`, `limited_value`, `limiter_name = "pnjlim"`, `reason = VoltageStep`
- [ ] `limiting_active()` removed from `AnalogDevice` trait
- [ ] `convergence_hint()` removed from `AnalogDevice` trait
- [ ] `ConvergenceHint` struct removed from `element.rs`
- [ ] All re-exports updated (`prelude.rs`, `abi.rs`)
- [ ] No dead methods left behind (grep: zero references to removed items)
- [ ] Existing MOSFET/diode test suite green (limiting still works through new API)
- [ ] Gate: `cargo test --workspace`

**Tests**: unit + integration (cross-crate: codegen produces, solver consumes)
**Gate**: full

---

### T9: Wire set_temperature into setup + sweep invalidation

**What**: Call `set_temperature(t_nominal)` on every analog element in `CircuitBuilder::build` / `CircuitInstance` setup (after `allocate_unknowns`, before first `load_*`); wire temperature sweep to call `set_temperature(t_new)` and honor `Invalidation::Temperature`.
**Where**: `crates/piperine-solver/src/core/builder.rs` (build path), `analyses/dc.rs` + `transient.rs` (sweep path)
**Depends on**: None
**Reuses**: `Invalidation::Temperature` (`introspect.rs:71-82`), `Tolerances.temperature` (`analyses/mod.rs:39`)
**Requirement**: ABI-19, ABI-20

**Done when**:
- [ ] `CircuitBuilder::build` calls `set_temperature(tolerances.temperature)` per element after `allocate_unknowns`
- [ ] Temperature sweep calls `set_temperature(t_new)` and honors `Invalidation::Temperature` (recompute constants → restamp)
- [ ] Default no-op devices unaffected (backward compatible)
- [ ] Integration test: a test device asserts `set_temperature` was called with the correct value before the first load
- [ ] Existing temperature-sweep proof (diode −2 mV/°C) still passes
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T10: Per-instance dtemp effective temperature

**What**: Compose `t_nominal + dtemp_instance` into the effective temperature; `set_temperature` receives the effective value.
**Where**: `crates/piperine-codegen/src/device/mod.rs` (PiperineDevice override), `crates/piperine-solver/src/core/element.rs` (set_temperature)
**Depends on**: T9
**Reuses**: Existing `dtemp` instance param in stdlib models (`mos.phdl`, `bjt.phdl`)
**Requirement**: ABI-21, ABI-22

**Done when**:
- [ ] `PiperineDevice` overrides `set_temperature` to compute `t_effective = t_nominal + dtemp` and cache it
- [ ] The effective temperature is used in temperature-dependent parameter evaluation
- [ ] `Invalidation::Rebuild` from a temperature change fails loud if rebuild is not possible (same rule as param invalidation)
- [ ] Unit test: instance with `dtemp=10` receives `set_temperature(tnom+10)`
- [ ] Gate: `cargo test --workspace`

**Tests**: unit
**Gate**: full

---

### T11: Add Jacobian/stamp capability bits

**What**: Add `HAS_DISTO2`, `HAS_DISTO3`, `NUMERIC_JACOBIAN` bits to `ElementCapabilities`.
**Where**: `crates/piperine-solver/src/core/element.rs`
**Depends on**: None
**Reuses**: Existing bitflags pattern
**Requirement**: ABI-23

**Done when**:
- [ ] `HAS_DISTO2 = 1 << 12`, `HAS_DISTO3 = 1 << 13`, `NUMERIC_JACOBIAN = 1 << 14` defined
- [ ] Doc comments explain each bit
- [ ] Unit test: capability flags compose correctly
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T12: Codegen kernels declare disto capability

**What**: `PiperineDevice::capabilities()` sets `HAS_DISTO2`/`HAS_DISTO3` based on `AnalogKernel::has_disto2()`/`has_disto3()`.
**Where**: `crates/piperine-codegen/src/device/mod.rs` (PiperineDevice capabilities)
**Depends on**: T11
**Reuses**: `AnalogKernel::has_disto2()`/`has_disto3()` (`kernel/analog/mod.rs:657,695`)
**Requirement**: ABI-26

**Done when**:
- [ ] A device with a compiled disto2 kernel declares `HAS_DISTO2`
- [ ] A device with a compiled disto3 kernel declares `HAS_DISTO3`
- [ ] A linear device (no disto kernels) declares neither
- [ ] Unit test: MOSFET declares disto2+disto3; resistor declares neither
- [ ] Gate: `cargo test -p piperine-codegen`

**Tests**: unit
**Gate**: quick (codegen)

---

### T13: `.disto` fail-loud checks

**What**: When `.disto` runs and no device declares `HAS_DISTO2`/`HAS_DISTO3`, emit a named warning; when a device has `NUMERIC_JACOBIAN`, fail loud.
**Where**: `crates/piperine-solver/src/analyses/disto.rs` (before the `let Some(d2) = … else { continue }` at lines 375, 414, 510, 560)
**Depends on**: T11, T12
**Reuses**: `SolverDomain::Element` error domain
**Requirement**: ABI-24, ABI-25

**Done when**:
- [ ] Pre-scan: if no device has `HAS_DISTO2`, emit a warning (`SolverDomain::Element`, "no device provides disto2 capability; HD2 results will be zero")
- [ ] Pre-scan: if any device has `NUMERIC_JACOBIAN`, return `Err` ("device `{name}` has numeric-only Jacobian; .disto requires analytic derivatives")
- [ ] A purely linear circuit running `.disto` produces the named warning
- [ ] A circuit with one nonlinear device runs `.disto` normally (no warning)
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T14: Add TerminalKind to TerminalDescriptor

**What**: Add `TerminalKind { External, Internal, Auxiliary }` enum + `kind` field to `TerminalDescriptor`.
**Where**: `crates/piperine-solver/src/core/introspect.rs`
**Depends on**: None
**Reuses**: Existing `TerminalDescriptor` struct (`introspect.rs:172-191`)
**Requirement**: ABI-29

**Done when**:
- [ ] `TerminalKind` enum defined with `External`, `Internal`, `Auxiliary`
- [ ] `TerminalDescriptor` gains `kind: TerminalKind` field (default `External` in `TerminalDescriptor::new`)
- [ ] Unit test: descriptors carry the correct kind
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T15: PiperineDevice bridges analog kernel terminals

**What**: Override `list_terminals()` on `PiperineDevice` to return one `TerminalDescriptor` per `AnalogKernel::terminals()`, populated from the symbol table (names, not just `NodeId` indices); mark internal `wire` nodes as `TerminalKind::Internal`.
**Where**: `crates/piperine-codegen/src/device/mod.rs` (PiperineDevice Introspect impl)
**Depends on**: T14
**Reuses**: `AnalogKernel::terminals()`, `num_ports()`, `num_terminals()` (`kernel/analog/mod.rs:267-285`), symbol table for names
**Requirement**: ABI-27

**Done when**:
- [ ] `list_terminals()` returns one descriptor per kernel terminal with the correct name
- [ ] Port terminals are `TerminalKind::External`; internal `wire` nodes are `TerminalKind::Internal`
- [ ] BJT test: `list_terminals()` returns c/b/e (external) + cp/bp/ep (internal)
- [ ] Gate: `cargo test -p piperine-codegen`

**Tests**: unit
**Gate**: quick (codegen)

---

### T16: PiperineDevice bridges digital kernel terminals

**What**: Extend `list_terminals()` to also cover `DigitalKernel::inputs()`/`outputs()`.
**Where**: `crates/piperine-codegen/src/device/mod.rs`
**Depends on**: T15
**Reuses**: `DigitalKernel::inputs()`/`outputs()` (`kernel/digital/abi.rs:134-140`)
**Requirement**: ABI-28

**Done when**:
- [ ] A mixed-signal device's `list_terminals()` returns both analog + digital terminals
- [ ] Digital terminals carry `Domain::Digital` + correct direction
- [ ] Unit test: a gate device returns correct input/output terminals
- [ ] Gate: `cargo test -p piperine-codegen`

**Tests**: unit
**Gate**: quick (codegen)

---

### T17: Codegen opvar compilation path

**What**: Compile an opvar-evaluation `AnalogFn` alongside the residual, reading the same state/var banks; the function evaluates declared opvar expressions (`var gm = …`).
**Where**: `crates/piperine-codegen/src/kernel/analog/mod.rs` (compile path), `lower/` (lowering)
**Depends on**: None
**Reuses**: Existing `AnalogFn` compilation pattern, existing `var` expressions in stdlib
**Requirement**: ABI-30

**Done when**:
- [ ] `AnalogKernel` exposes an opvar-evaluation function (or vector thereof)
- [ ] The function reads `state`/`vars` and returns `(name, value)` pairs
- [ ] Devices without opvars compile an empty path (zero overhead)
- [ ] Unit test: a MOSFET kernel evaluates `gm` post-solve
- [ ] **Risk flag**: if this task proves larger than expected, split into "terminal bridge ships, opvars defer" — discuss before extending scope
- [ ] Gate: `cargo test -p piperine-codegen`

**Tests**: unit
**Gate**: quick (codegen)

---

### T18: PiperineDevice read_opvars/list_queries bridge

**What**: Override `read_opvars()`/`list_queries()` on `PiperineDevice` to call the compiled opvar-eval function and return real data.
**Where**: `crates/piperine-codegen/src/device/mod.rs`
**Depends on**: T17
**Reuses**: `QueryDescriptor`/`QueryKind` (`introspect.rs:114-151`)
**Requirement**: ABI-31

**Done when**:
- [ ] `read_opvars()` returns the evaluated opvars (e.g., `gm`, `vbe`) from the compiled function
- [ ] `list_queries()` returns the declared query catalog (names, kinds, units)
- [ ] A MOSFET after DC returns at least `gm` and `vbe` (or whatever the kernel declares)
- [ ] Gate: `cargo test --workspace`

**Tests**: integration
**Gate**: full

---

### T19: ModelDescriptor + kernel named catalogs

**What**: Add `ModelDescriptor { type_id, version }` + `model_descriptor()` on `Introspect`; surface kernel named catalogs (state/force/noise terminal names) through the ABI.
**Where**: `crates/piperine-solver/src/core/introspect.rs` (trait + struct), `crates/piperine-codegen/src/device/mod.rs` (bridge)
**Depends on**: T15, T16
**Reuses**: `AnalogKernel` slot counts + names, `DigitalKernel::layout()`
**Requirement**: ABI-46, ABI-47, ABI-48

**Done when**:
- [ ] `Introspect::model_descriptor() -> ModelDescriptor` with default `{ type_id: "", version: "" }`
- [ ] `PiperineDevice` overrides it with the kernel's model name + version
- [ ] Kernel named catalogs (state slot names, force terminal names, noise terminal names) surfaced
- [ ] Unit test: MOSFET reads `{ type: "mos", version: "3" }`; state/noise catalogs non-empty
- [ ] Gate: `cargo test --workspace`

**Tests**: integration
**Gate**: full

---

### T20: Fold StepperStrategy into ConvergencePlan

**What**: Add `stepper: Box<dyn StepperStrategy>` to `ConvergencePlan`; transient delegates `propose_dt`/`reject_dt` to `plan.stepper()`.
**Where**: `crates/piperine-solver/src/analyses/convergence.rs` (ConvergencePlan), `analyses/transient.rs` (delegate)
**Depends on**: T2 (checkpoint wiring must be stable before moving reject logic)
**Reuses**: `PiController` (already implements `StepperStrategy`), `ConvergencePlan::newton` pattern
**Requirement**: ABI-42, ABI-43

**Done when**:
- [ ] `ConvergencePlan` has `stepper: Box<dyn StepperStrategy>` field + `with_stepper()` builder + `stepper()` accessor
- [ ] `TransientSolver` delegates `propose_dt` + reject decisions to `plan.stepper()` instead of `self.stepper`
- [ ] `PiController` impl unchanged (same behavior)
- [ ] Default `ConvergencePlan::default()` includes `PiController::default()`
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T21: Parity baseline verification + custom stepper test

**What**: Verify parity baselines are bit-identical through the fold; add a test double `StepperStrategy` that routes through the plan.
**Where**: `crates/piperine-solver/tests/parity_baseline.rs` (verify), `crates/piperine-solver/tests/` (new test)
**Depends on**: T20
**Reuses**: `parity_baseline.rs` existing baselines
**Requirement**: ABI-44, ABI-45

**Done when**:
- [ ] Parity baselines (`parity_baseline.rs`) bit-identical through the stepper fold
- [ ] Test double `StepperStrategy` (halves dt on reject) produces a deterministic step sequence through the plan
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T22: EventEntry/EventKind/RollbackBehavior types

**What**: Define the unified event types: `EventEntry`, `EventKind`, `EventTarget`, `EventPriority`, `EventSource`, `RollbackBehavior`.
**Where**: `crates/piperine-solver/src/analyses/events.rs` (new module)
**Depends on**: None
**Reuses**: `DigitalEvent`/`DigitalNet` (existing)
**Requirement**: ABI-36

**Done when**:
- [ ] All types defined with doc comments
- [ ] `EventEntry` implements `Ord` (by `(time, priority)`) for `BinaryHeap`
- [ ] Unit test: ordering by time then priority
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T23: EventQueue type

**What**: Define `EventQueue` — a `BinaryHeap<Reverse<EventEntry>>` with `push`, `peek_next_time`, `drain_due(time)`, `rollback(entries_to_restore)`.
**Where**: `crates/piperine-solver/src/analyses/events.rs`
**Depends on**: T22
**Reuses**: `DigitalState::event_queue` pattern (`digital/state.rs:19-27`)
**Requirement**: ABI-36

**Done when**:
- [ ] `EventQueue` struct with push/peek/drain/rollback methods
- [ ] One-deep checkpoint semantics (matching `DigitalState::Checkpoint`)
- [ ] Unit test: push multiple events, peek returns earliest, drain_due returns due events
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T24: Digital events → unified queue adapter

**What**: Adapter that pushes `DigitalEvent`s into the `EventQueue` with `kind=Digital`, `rollback=Restore`.
**Where**: `crates/piperine-solver/src/digital/state.rs` (adapter) or `analyses/transient.rs`
**Depends on**: T23
**Reuses**: `DigitalEvent` → `EventEntry` conversion
**Requirement**: ABI-37

**Done when**:
- [ ] Digital events enter the unified queue with correct kind + rollback behavior
- [ ] `DigitalState::event_queue` either becomes the backing store or is replaced by the unified queue
- [ ] Existing digital scheduler tests green
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T25: Breakpoints + scheduled sets + $bound_step → unified queue

**What**: Adapters for the remaining three sources: `next_breakpoints()` → `kind=Breakpoint/RePoll`, `SetQueue` → unified, `bound_step_hint()` → `kind=StepHint/Advisory`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (adapter wiring)
**Depends on**: T24
**Reuses**: `SetQueue` (`transient.rs:374-408`), `Element::next_breakpoints` (`element.rs:132`)
**Requirement**: ABI-38, ABI-39

**Done when**:
- [ ] Breakpoint times enter the queue with `kind=Breakpoint`, `rollback=RePoll`
- [ ] Scheduled live-set times enter the queue with correct kind
- [ ] `$bound_step` enters as `kind=StepHint`, `priority=Advisory`
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T26: predict_step reads from EventQueue

**What**: `predict_step` reads the next event time from the unified `EventQueue` instead of manually merging four sources.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (`predict_step:734-782`)
**Depends on**: T25
**Reuses**: Existing `StepPrediction`/`landed_on_breakpoint` logic
**Requirement**: ABI-36

**Done when**:
- [ ] `predict_step` calls `event_queue.peek_next_time()` instead of four-source merge
- [ ] `landed_on_breakpoint` set correctly from the event kind
- [ ] Empty queue falls back to PI-proposed dt
- [ ] Parity baselines green (step sequence unchanged for existing circuits)
- [ ] Gate: `cargo test --workspace`

**Tests**: integration
**Gate**: full

---

### T27: Per-entry rollback behavior on step rejection

**What**: On step rejection, honor each `EventEntry`'s `rollback` field: `Restore` for digital, `RePoll` for breakpoints, `Discard` for crossings.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (reject path), `analyses/events.rs`
**Depends on**: T26, T2 (checkpoint infrastructure)
**Reuses**: `EventQueue::rollback` from T23
**Requirement**: ABI-40, ABI-41

**Done when**:
- [ ] Reject path calls `event_queue.rollback()` which honors per-entry `RollbackBehavior`
- [ ] Digital events restored, breakpoints discarded (re-polled next step), crossings discarded
- [ ] Integration test: mixed-signal circuit with digital clock + pulse source — rejected step restores digital events, re-polls breakpoints
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T28: ObservableDescriptor + ProbeSelection types

**What**: Define `ObservableDescriptor { name, kind, cost }`, `ObservableKind`, `ProbeSelection`; `PiperineDevice` declares its observables.
**Where**: `crates/piperine-solver/src/core/introspect.rs` (types), `crates/piperine-codegen/src/device/mod.rs` (declare)
**Depends on**: None
**Reuses**: `runtime_banks()` existing state/var banks
**Requirement**: ABI-32, ABI-33

**Done when**:
- [ ] `ObservableDescriptor` + `ObservableKind` defined in introspect
- [ ] `Introspect::list_observables() -> Vec<ObservableDescriptor>` default empty
- [ ] `ProbeSelection { requests: Vec<(String, String)> }` defined
- [ ] `PiperineDevice` overrides `list_observables()` from kernel state/var slot names
- [ ] Unit test: a device declares branch currents + state observables
- [ ] Gate: `cargo test --workspace`

**Tests**: unit
**Gate**: full

---

### T29: collect_device_banks filters by ProbeSelection

**What**: `TransientAnalysisOptions` gains `probe_selection: ProbeSelection`; `collect_device_banks` records only requested observables.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (options + collect)
**Depends on**: T28
**Reuses**: Existing `collect_device_banks` (`transient.rs:1192-1203`)
**Requirement**: ABI-34

**Done when**:
- [ ] `TransientAnalysisOptions` has `probe_selection: ProbeSelection` field
- [ ] `collect_device_banks` filters: only devices + observables in the selection are recorded
- [ ] Empty `ProbeSelection` = no recording (today's default-off behavior)
- [ ] Global `record_device_state: bool = true` = "all observables on all devices" shorthand
- [ ] Integration test: 100-step transient with 10 devices, `ProbeSelection` requests 1 observable on 1 device — only that data is in `TransientStep::device_state`
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: quick (solver)

---

### T30: Fail-loud on unknown observable

**What**: When `ProbeSelection` requests an observable a device doesn't declare, return a named error at setup time.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (setup validation)
**Depends on**: T29
**Reuses**: `SolverDomain::Element` error domain
**Requirement**: ABI-35

**Done when**:
- [ ] Unknown device label in `ProbeSelection` → `Err("device `{label}` not found")`
- [ ] Unknown observable name on a known device → `Err("device `{label}` has no observable `{name}`")`
- [ ] Unit test: requesting `"nonexistent"` on a resistor fails loud at setup
- [ ] Gate: `cargo test -p piperine-solver`

**Tests**: unit
**Gate**: quick (solver)

---

### T31: Write Part VII lifecycle chart + algorithm flow

**What**: Document, for each analysis (DC/AC/tran/noise/PSS/`.sens`), the ordered hook chart (preconditions/postconditions) AND the algorithm flow (main loop structure, phases per iteration, convergence/rejection criteria, where each hook sits).
**Where**: `docs/spec/part_vii_solver.md`
**Depends on**: T8, T10, T13, T19, T21, T27, T30 (all hook changes must land first)
**Reuses**: Design doc component mapping, audit findings
**Requirement**: ABI-14, ABI-15, ABI-17, ABI-18

**Done when**:
- [ ] For each of the 6 analyses: a hook ordering table + an algorithm flow description
- [ ] Rollback hooks (`checkpoint_state`/`restore_state`) documented in tran + DC algorithm flow
- [ ] Temperature (`set_temperature`) documented in setup position
- [ ] LimitingReport documented in Newton convergence flow
- [ ] Each algorithm flow covers: main loop, phases within one iteration, convergence/rejection criteria, hook positions
- [ ] Completeness check: no analysis section is empty
- [ ] Gate: build gate only (documentation)

**Tests**: none
**Gate**: build

---

### T32: Lifecycle contract test — executable hook ordering

**What**: Instrument a recording test Element; run each analysis; assert the hook ordering matches the documented chart. Include an algorithm-description completeness assertion.
**Where**: `crates/piperine-solver/tests/lifecycle.rs` (extend existing)
**Depends on**: T31
**Reuses**: Existing `LifecycleTestDevice` (`lifecycle.rs:5-38`)
**Requirement**: ABI-16

**Done when**:
- [ ] Test Element records every hook call with its analysis + relative timestamp
- [ ] One test per analysis (DC, AC, tran, noise, PSS, `.sens`): assert hook order matches Part VII chart
- [ ] Algorithm-description completeness assertion: Part VII has non-empty algorithm flow for each analysis
- [ ] Gate: `cargo test --workspace`

**Tests**: integration
**Gate**: full

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7

Phase 1:  T1 ──→ T2 ──→ T3 ──→ T4 ──→ T5 ──→ T6 ──→ T7 ──→ T8
Phase 2:  T9 ──→ T10 ──→ T11 ──→ T12 ──→ T13
Phase 3:  T14 ──→ T15 ──→ T16 ──→ T17 ──→ T18 ──→ T19
Phase 4:  T20 ──→ T21
Phase 5:  T22 ──→ T23 ──→ T24 ──→ T25 ──→ T26 ──→ T27
Phase 6:  T28 ──→ T29 ──→ T30
Phase 7:  T31 ──→ T32
```

Execution is strictly sequential — there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batch packing** (32 tasks, ~7 per batch → 5 batches):
- Batch 1: Phase 1 (8 tasks) — rollback + limiting
- Batch 2: Phases 2+3 (11 tasks) — temperature + jacobian + terminals + introspect
- Batch 3: Phases 4+5 (8 tasks) — stepper fold + unified events
- Batch 4: Phase 6 (3 tasks) — save/probe
- Batch 5: Phase 7 (2 tasks) — lifecycle contract

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| T1: ElementCheckpoint + hooks | 1 type + 2 trait methods | ✅ Granular |
| T2: Transient reject wiring | 1 code path (3 methods) | ✅ Granular |
| T3: DC homotopy wiring | 1 code path | ✅ Granular |
| T4: Limiter checkpoint | 1 impl override | ✅ Granular |
| T5: Digital register checkpoint | 1 impl extension | ✅ Granular |
| T6: LimitingReport type + hook | 1 type + 1 trait method | ✅ Granular |
| T7: Newton gate wiring | 2 call sites (gate + bypass) | ✅ Granular |
| T8: Codegen limiter + remove dead | 1 impl + dead code removal | ✅ Granular |
| T9: set_temperature wiring | 2 call sites (setup + sweep) | ✅ Granular |
| T10: Per-instance dtemp | 1 override | ✅ Granular |
| T11: Capability bits | 3 bitflags | ✅ Granular |
| T12: Codegen disto capability | 1 capabilities override | ✅ Granular |
| T13: .disto fail-loud | 1 analysis driver | ✅ Granular |
| T14: TerminalKind | 1 enum + 1 field | ✅ Granular |
| T15: Analog terminal bridge | 1 override | ✅ Granular |
| T16: Digital terminal bridge | 1 override extension | ✅ Granular |
| T17: Opvar compilation path | 1 compile path | ⚠️ Risk flag — may split |
| T18: Opvar bridge | 2 overrides | ✅ Granular |
| T19: ModelDescriptor + catalogs | 1 type + 1 override + bridge | ✅ Granular |
| T20: Stepper fold | 1 struct field + delegation | ✅ Granular |
| T21: Parity + custom stepper test | 1 verification + 1 test | ✅ Granular |
| T22: Event types | 1 module (6 enums/structs) | ⚠️ 6 types, same file — cohesive |
| T23: EventQueue | 1 struct + methods | ✅ Granular |
| T24: Digital adapter | 1 adapter | ✅ Granular |
| T25: Breakpoint/set/bound adapters | 3 adapters, same wiring site | ⚠️ 3 things, same file — cohesive |
| T26: predict_step unification | 1 method rewrite | ✅ Granular |
| T27: Per-entry rollback | 1 reject-path extension | ✅ Granular |
| T28: Observable types + declare | 1 type module + 1 override | ⚠️ 2 things, cohesive |
| T29: ProbeSelection filtering | 1 options field + 1 collect change | ✅ Granular |
| T30: Fail-loud unknown observable | 1 validation | ✅ Granular |
| T31: Part VII documentation | 1 doc file | ✅ Granular |
| T32: Lifecycle contract test | 1 test file | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
|------|----------------------|---------------|--------|
| T1 | None | Phase 1 start | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T1 | T2 → T3 (within phase) | ✅ Match (T1 is the dependency, T2 is ordering) |
| T4 | T1 | T3 → T4 | ✅ Match (T1 dep; ordering within phase) |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | None | T5 → T6 | ✅ Match (no dep; ordering within phase) |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | None | Phase 2 start | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | None | T10 → T11 | ✅ Match (no dep; ordering) |
| T12 | T11 | T11 → T12 | ✅ Match |
| T13 | T11, T12 | T12 → T13 | ✅ Match |
| T14 | None | Phase 3 start | ✅ Match |
| T15 | T14 | T14 → T15 | ✅ Match |
| T16 | T15 | T15 → T16 | ✅ Match |
| T17 | None | T16 → T17 | ✅ Match (no dep; ordering) |
| T18 | T17 | T17 → T18 | ✅ Match |
| T19 | T15, T16 | T18 → T19 | ✅ Match (T15/T16 deps; T18 ordering) |
| T20 | T2 | Phase 4 start | ✅ Match (T2 from Phase 1) |
| T21 | T20 | T20 → T21 | ✅ Match |
| T22 | None | Phase 5 start | ✅ Match |
| T23 | T22 | T22 → T23 | ✅ Match |
| T24 | T23 | T23 → T24 | ✅ Match |
| T25 | T24 | T24 → T25 | ✅ Match |
| T26 | T25 | T25 → T26 | ✅ Match |
| T27 | T26, T2 | T26 → T27 | ✅ Match (T26 dep; T2 from Phase 1) |
| T28 | None | Phase 6 start | ✅ Match |
| T29 | T28 | T28 → T29 | ✅ Match |
| T30 | T29 | T29 → T30 | ✅ Match |
| T31 | T8,T10,T13,T19,T21,T27,T30 | Phase 7 start | ✅ Match (all prior phases) |
| T32 | T31 | T31 → T32 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
|------|---------------------------|-----------------|-----------|--------|
| T1 | Solver ABI | unit | unit | ✅ OK |
| T2 | Solver analyses | integration | integration | ✅ OK |
| T3 | Solver analyses | integration | integration | ✅ OK |
| T4 | Codegen device | unit | unit | ✅ OK |
| T5 | Codegen device | unit | unit | ✅ OK |
| T6 | Solver ABI | unit | unit | ✅ OK |
| T7 | Solver analyses | integration | integration | ✅ OK |
| T8 | Codegen device + Solver ABI | unit + integration | unit + integration | ✅ OK (full gate) |
| T9 | Solver analyses | integration | integration | ✅ OK |
| T10 | Codegen device + Solver ABI | unit | unit | ✅ OK (full gate) |
| T11 | Solver ABI | unit | unit | ✅ OK |
| T12 | Codegen kernel | unit | unit | ✅ OK |
| T13 | Solver analyses | integration | integration | ✅ OK |
| T14 | Solver ABI | unit | unit | ✅ OK |
| T15 | Codegen device | unit | unit | ✅ OK |
| T16 | Codegen device | unit | unit | ✅ OK |
| T17 | Codegen kernel | unit | unit | ✅ OK |
| T18 | Codegen device + Solver ABI | integration | integration | ✅ OK (full gate) |
| T19 | Codegen device + Solver ABI | integration | integration | ✅ OK (full gate) |
| T20 | Solver analyses | integration | integration | ✅ OK |
| T21 | Solver analyses (test) | integration | integration | ✅ OK |
| T22 | Solver ABI (new module) | unit | unit | ✅ OK |
| T23 | Solver ABI (new module) | unit | unit | ✅ OK |
| T24 | Solver digital | integration | integration | ✅ OK |
| T25 | Solver analyses | integration | integration | ✅ OK |
| T26 | Solver analyses | integration | integration | ✅ OK (full gate) |
| T27 | Solver analyses + digital | integration | integration | ✅ OK |
| T28 | Solver ABI + Codegen device | unit | unit | ✅ OK (full gate) |
| T29 | Solver analyses | integration | integration | ✅ OK |
| T30 | Solver analyses | unit | unit | ✅ OK |
| T31 | Documentation | none | none | ✅ OK (build gate) |
| T32 | Solver analyses (test) | integration | integration | ✅ OK |

# solver-simplification Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Design**: `.specs/features/solver-simplification/design.md`
**Status**: In Progress — batch 2 (P2+P3) DONE

## Progress Log
- **Batch 1 (P0+P1)** ✅ — T1 `4565f9e` analog parity baselines · T2 `0912915` mixed-signal+digital baselines · T3 `cba1783` remove `LINEAR` · T4 `2a521cb` remove `ANALYTIC_JACOBIAN`/`STAMPS_CHARGE`+producers+asserts, add `capabilities_contract.rs` · T5 `9e324ec` phantom rollback doc removed, `SUPPORTS_QUERIES`/`SUPPORTS_ROLLBACK` kept as reserved bits (no method promise) · T6 `50225cf` `SignalBridge` folded into `CircuitInstance`. +8 tests, all gates green, baselines bit-identical.
- **Batch 2 (P2+P3)** ✅ — T7 `4735157` `math::unit` aliases inlined as `f64`, `unit.rs` deleted · T8 `7471d71` `Second` dropped from the `abi` surface (codegen consumer to `f64`) · T9 `7f438d0` config home `solver/config.rs` (`GminSchedule`/`SourceSchedule`/`StepperGains`/`TraceFlags`; defaults == former literals, contract test) · T10 `e595347` `Schedules` owned by `ConvergencePlan`, homotopy bodies de-literaled · T11 `2184b5e` `StepperGains` wired into `PiController` · T12 `06c0275` `PIPERINE_TRACE_*` routed through `TraceFlags` (`Policy` seeds from env, single env read left). +1 test (518 total), all gates green, baselines bit-identical.
**Invariant**: behavior-preserving refactor. Every task keeps
`cargo test --workspace` green with **bit-identical numerics** on the P0 parity
baselines. A task that changes a solved value is a defect, not a deviation.

**External scope (locked):** `piperine-osdi` is frozen/external (redesign
later) and there are no plugins today. Do **not** touch or coordinate external
crates. In-workspace `impl Element` sites (codegen `PiperineDevice`, solver
test doubles) adapt within their tasks.

---

## Test Coverage Matrix

> Generated from codebase + `CLAUDE.md`/`AGENTS.md` guidelines + spec. Guidelines
> found: `CLAUDE.md`, `.specs/STATE.md` (MD-13 idiom rules), `AGENTS.md`. Gate is
> `cargo`. This is a **behavior-preserving refactor**: the existing suite (509
> green) is the invariance oracle; new tests are parity baselines + contract
> assertions, never re-specifications of behavior.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Analysis drivers (`analyses/`, formerly `solver/`+`analysis/`) | integration (parity) | Existing suite stays green + pinned exact-value baselines unchanged; every touched analysis has a parity anchor | `crates/piperine-solver/tests/*.rs`, `crates/piperine-codegen/tests/*.rs`, root `tests/*.rs` | `cargo test --workspace` |
| ABI / contracts (`core/element.rs`, capabilities, `config.rs`) | unit (contract) | Every capability flag has a producer+consumer; each config default equals its former literal; composed-trait surface compiles for a minimal double | `crates/piperine-solver/tests/*.rs`, in-crate `#[cfg(test)]` | `cargo test -p piperine-solver` |
| Pure structural moves (module relocation, impl regrouping, doc) | none | Build gate only — behavior proven by the unchanged suite | — | build gate |
| Docs (`docs/spec/part_vii_solver.md`) | none | Manual stated-vs-code audit (SS-I); build gate | — | build gate + audit |

## Gate Check Commands

> Confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a solver-only task with unit/contract tests | `cargo test -p piperine-solver` |
| Full | After a task touching cross-crate behavior or parity | `cargo test --workspace` |
| Build | After the last task in a phase, or a structural/doc-only task | `cargo build --workspace 2>&1 \| grep -cE "^warning\|^error"` must be `0`, then `cargo test --workspace` |

**Tools per task:** MCP: NONE (Rust workspace, `cargo` gate). Skill:
`tlc-spec-driven` (the executing flow) — no other skills.

---

## Execution Plan

Phases run sequentially; tasks within a phase run in order. Ordered low-risk →
high-churn; Part VII last (describes the built solver).

### Phase P0: Parity safety net
```
T1 → T2
```
### Phase P1: Dead surface
```
T3 → T4 → T5 → T6
```
### Phase P2: Remove math/unit.rs
```
T7 → T8
```
### Phase P3: Config home
```
T9 → T10 → T11 → T12
```
### Phase P4: Element decomposition
```
T13 → T14 → T15 → T16 → T17 → T18
```
### Phase P5: CircuitInstance grouped
```
T19 → T20 → T21
```
### Phase P6: Module layout (Scheme B)
```
T22 → T23 → T24 → T25 → T26
```
### Phase P7: Transient loop
```
T27 → T28 → T29 → T30
```
### Phase P8: Map matches territory
```
T31 → T32
```
### Phase P9: Part VII canonical rewrite (finalization)
```
T33 → T34 → T35
```

---

## Task Breakdown

### T1: Analog parity baselines
**What**: Pin exact solved-value regression tests for divider op, clipper tran (2 iters/step landmark), coupled-LC transfer peak, diode DC point — the analog refactor oracle.
**Where**: `crates/piperine-solver/tests/parity_baseline.rs` (new)
**Depends on**: None · **Requirement**: SS-06/SS-09 (oracle)
**Done when**:
- [ ] Each baseline asserts the current exact value(s) to tight tolerance (capture from a run first)
- [ ] Full gate passes: `cargo test --workspace`
- [ ] Test count recorded

**Tests**: integration · **Gate**: full

### T2: Mixed-signal + digital parity baselines
**What**: Pin bit-identical snapshots for a mixed-signal DC settle circuit and a digital-topology example (adder/mux) — the mixed-signal/digital oracle before `SignalBridge` fold and module moves.
**Where**: `crates/piperine-solver/tests/parity_baseline.rs` (extend)
**Depends on**: T1 · **Requirement**: SS-16 (oracle)
**Done when**:
- [ ] Mixed-signal settled node voltages + final digital net snapshot asserted exactly
- [ ] Full gate passes
**Tests**: integration · **Gate**: full

### T3: Remove `ElementCapabilities::LINEAR`
**What**: Delete the never-produced, never-read `LINEAR` flag.
**Where**: `crates/piperine-solver/src/core/element.rs`
**Depends on**: T2 · **Requirement**: SS-10
**Done when**:
- [ ] `LINEAR` gone; workspace grep clean; build gate 0 warnings
- [ ] `cargo test --workspace` green
**Tests**: none (build gate) · **Gate**: build

### T4: Remove `ANALYTIC_JACOBIAN` + `STAMPS_CHARGE` (flags + producers + asserts) + consumer contract test
**What**: Delete both flags, the codegen producers (`codegen/device/mod.rs:163,165`), and the asserts (`codegen/tests/codegen_api.rs`). Add a contract test asserting every remaining `ElementCapabilities` flag has a solver consumer.
**Where**: `core/element.rs`, `crates/piperine-codegen/src/device/mod.rs`, `crates/piperine-codegen/tests/codegen_api.rs`, new contract test in `crates/piperine-solver/tests/capabilities_contract.rs`
**Depends on**: T3 · **Requirement**: SS-10 (SC: every flag has both ends)
**Done when**:
- [ ] Flags + producers + asserts removed
- [ ] Contract test enumerates surviving flags and asserts a documented consumer for each (fails if a write-only flag is reintroduced)
- [ ] Full gate green
**Tests**: unit (contract) · **Gate**: full

### T5: Correct the phantom rollback contract; audit `SUPPORTS_QUERIES`
**What**: Delete the `SUPPORTS_ROLLBACK` doc block referencing non-existent `checkpoint_state`/`rollback_state`/`commit_state`; either drop the flag or reduce to a one-line `// reserved: solver-commit-rollback` with no method promise. Audit `SUPPORTS_QUERIES`: keep only if a consumer reads it, else same treatment.
**Where**: `core/element.rs`
**Depends on**: T4 · **Requirement**: SS-11
**Done when**:
- [ ] No `Element` doc references a non-existent method
- [ ] `SUPPORTS_QUERIES` has a consumer or is removed/marked reserved (decision noted in commit)
- [ ] Build gate green
**Tests**: none (build gate) · **Gate**: build

### T6: Fold `SignalBridge` into `CircuitInstance`
**What**: Move `build_accept_state`/`settle` onto `CircuitInstance` as named mixed-signal-seam methods; delete the zero-field `SignalBridge` struct and its field.
**Where**: `crates/piperine-solver/src/core/circuit.rs`
**Depends on**: T5 · **Requirement**: SS-12/SS-16
**Done when**:
- [ ] `SignalBridge` gone; seam methods live on `CircuitInstance`
- [ ] T2 mixed-signal baseline bit-identical; full gate green
**Tests**: integration (parity via T2) · **Gate**: full

### T7: Inline `f64` for all `math::unit` aliases
**What**: Replace every `unit::{Volt,Ohm,Second,Siemens,Farad,…}` use with `f64`; delete `math/unit.rs` and its `mod unit`.
**Where**: `crates/piperine-solver/src/**` (esp. `solver/mod.rs`, `math/constant.rs`, `core/element.rs`), delete `math/unit.rs`
**Depends on**: T6 · **Requirement**: SS-17
**Done when**:
- [ ] No `crate::math::unit` reference remains in the solver crate
- [ ] Build gate 0 warnings; parity baselines bit-identical
**Tests**: none (build gate + parity) · **Gate**: build

### T8: Drop `Second` from the `abi`/`prelude` surface; fix in-workspace consumers
**What**: Remove the `Second` re-export; update any in-workspace crate (codegen/api) that imported it to `f64`. Confirm no external-only breakage (osdi excluded).
**Where**: `crates/piperine-solver/src/abi.rs`, `prelude.rs`, downstream in-workspace uses
**Depends on**: T7 · **Requirement**: SS-17
**Done when**:
- [ ] `Second` no longer exported; workspace builds 0 warnings
- [ ] Full gate green
**Tests**: none (build gate) · **Gate**: full

### T9: Create `config.rs` — schedule/gain/trace structs
**What**: Add `GminSchedule`, `SourceSchedule`, `StepperGains`, `TraceFlags` (design §5) with `Default` impls whose values **equal today's literals exactly**.
**Where**: `crates/piperine-solver/src/solver/config.rs` (new; moves to `analyses/config.rs` in P6)
**Depends on**: T8 · **Requirement**: SS-07
**Done when**:
- [ ] Structs + documented fields + defaults defined
- [ ] Unit test asserts each default equals the literal it replaces (e.g. `GminSchedule::default().start_g == 0.1`)
- [ ] Quick gate green
**Tests**: unit (contract) · **Gate**: quick

### T10: Wire gmin/source schedules into homotopy strategies
**What**: `GminStepping`/`SourceStepping` read `GminSchedule`/`SourceSchedule` fields; remove every inline literal (`0.1`,`1.3`,`3.0`,`0.5`,`0.7`,`200`,`300`,`knee_gmin`,`1e-6`,…).
**Where**: `solver/convergence.rs`, ownership on `ConvergencePlan`
**Depends on**: T9 · **Requirement**: SS-07
**Done when**:
- [ ] No behavior-affecting numeric literal in `GminStepping`/`SourceStepping` bodies
- [ ] Existing convergence tests (`source_stepping_*`) + junction DC parity unchanged; full gate green
**Tests**: integration (parity) · **Gate**: full

### T11: Wire `StepperGains` into `PiController`
**What**: `PiController` reads `grow_factor`/`reject_divisor`/`factor_clamp`/`kp`/`ki` from `StepperGains`; remove inline `1.5`,`8.0`,`0.2`,`0.7`,`0.4`.
**Where**: `solver/convergence.rs`, ownership on the transient stepper
**Depends on**: T10 · **Requirement**: SS-07
**Done when**:
- [ ] No timestep literal in `PiController::{propose_dt,reject_dt}`
- [ ] Transient parity (clipper/coupled-LC dt schedule) bit-identical; full gate green
**Tests**: integration (parity) · **Gate**: full

### T12: Route trace toggles through `TraceFlags`
**What**: Replace `PIPERINE_TRACE_{GMIN,SRC,TRAN}` env reads with `TraceFlags` fields (env may seed the defaults, but the hot path reads the typed field).
**Where**: `solver/convergence.rs`, `solver/transient.rs`, `TraceFlags` on `Context`/`Policy`
**Depends on**: T11 · **Requirement**: SS-08
**Done when**:
- [ ] No `std::env::var("PIPERINE_TRACE*")` in a hot path; typed field read instead
- [ ] Build gate green (behavior default-off unchanged)
**Tests**: none (build gate) · **Gate**: build

### T13: Extract `AnalogDevice` supertrait
**What**: Define `trait AnalogDevice: Send + Sync` with the analog methods (load_*, noise, limiting/hint/bound_step/next_breakpoints/initial_conditions, allocate_unknowns, set_temperature, update, suggest_transient_step), all defaulted (design §3).
**Where**: `crates/piperine-solver/src/core/element.rs`
**Depends on**: T12 · **Requirement**: SS-03
**Done when**:
- [ ] `AnalogDevice` defined; methods moved off `Element` (compile may break until T16 — acceptable within phase, gate at T18)
- [ ] Crate compiles OR breakage is confined to the impl-regroup tasks (state in commit)
**Tests**: none (interim) · **Gate**: build (allow the four-task compile unit to close at T16/T17/T18 — see note)

> **Phase P4 compile note:** T13–T16 restructure one trait; the crate may not
> compile between them. Treat T13–T16 as one compile unit — each commits its
> slice, and the **first green gate is at T16** (trait redefinition complete),
> with T17/T18 restoring all impls. Workers: run `cargo build -p piperine-solver`
> after T16, not T13–T15. This is the one legitimate multi-task compile chain
> (tasks.md granularity guidance: a tight dependency chain).

### T14: Extract `DigitalDevice` supertrait
**What**: Define `trait DigitalDevice: Send + Sync` with boundary/init/seq_phase/comb_phase/evaluate/has_input_on/digital_hidden_snapshot/restore (design §3).
**Where**: `core/element.rs`
**Depends on**: T13 · **Requirement**: SS-03
**Done when**:
- [ ] `DigitalDevice` defined; digital methods moved off `Element`
**Tests**: none (interim) · **Gate**: build (at T16)

### T15: Extract `Introspect` supertrait
**What**: Define `trait Introspect: Send + Sync` with list_params/get_param/set_param/list_queries/query/list_terminals/read_opvars (design §3).
**Where**: `core/element.rs`
**Depends on**: T14 · **Requirement**: SS-03
**Done when**:
- [ ] `Introspect` defined; introspection methods moved off `Element`
**Tests**: none (interim) · **Gate**: build (at T16)

### T16: Redefine `Element` as the conjunction + update `abi`
**What**: `trait Element: AnalogDevice + DigitalDevice + Introspect` keeping identity/lifecycle (name, capabilities, setup, destroy, accept_timestep, runtime_banks). Export the sub-traits from `abi.rs`.
**Where**: `core/element.rs`, `abi.rs`
**Depends on**: T15 · **Requirement**: SS-03/SS-04
**Done when**:
- [ ] `Element` compiles as the conjunction; `abi` exports the three sub-traits
- [ ] `cargo build -p piperine-solver` green (first green gate of P4)
**Tests**: none · **Gate**: build

### T17: Re-group the codegen `PiperineDevice` impl
**What**: Split codegen's single `impl Element for PiperineDevice` into `impl AnalogDevice` + `impl DigitalDevice` + `impl Introspect` + `impl Element` (mechanical; no logic change).
**Where**: `crates/piperine-codegen/src/device/mod.rs`
**Depends on**: T16 · **Requirement**: SS-04
**Done when**:
- [ ] Codegen compiles against the composed surface; codegen tests green
- [ ] Full gate green
**Tests**: integration (existing codegen suite) · **Gate**: full

### T18: Re-group in-workspace test doubles + composed-surface contract test; MD-01 amendment stub
**What**: Regroup any solver-crate `impl Element` test doubles into the four blocks. Add a contract test: a minimal double implementing only `AnalogDevice` non-trivially (rest defaulted) compiles and solves. Mark MD-01 amendment intent (full STATE.md write in T31).
**Where**: `crates/piperine-solver/tests/*.rs`, new `crates/piperine-solver/tests/composed_element.rs`
**Depends on**: T17 · **Requirement**: SS-04
**Done when**:
- [ ] All in-workspace `impl Element` compile in the four-block form
- [ ] Contract test proves single-concern implementation works with no downcast
- [ ] Full gate green + 0 warnings
**Tests**: unit (contract) + integration · **Gate**: build

### T19: Group `CircuitInstance` into five contracted sections
**What**: Reorder the impl into circuit-state / analysis-entry / mixed-signal-seam / live-mutation / construction sections with `// ──` headers + struct-level `//!` contract (design §6b). No behavior change.
**Where**: `core/circuit.rs`
**Depends on**: T18 · **Requirement**: SS-15
**Done when**:
- [ ] Every public method sits under a named responsibility; struct doc names the five jobs
- [ ] Full gate green (behavior unchanged)
**Tests**: integration (parity) · **Gate**: full

### T20: Consolidate the mixed-signal seam
**What**: Ensure the folded seam methods (T6) + `init_digital`/`run_digital_at*`/`accept_and_run_digital`/`rebuild_digital_topology` sit together as the one seam, contract-documented.
**Where**: `core/circuit.rs`
**Depends on**: T19 · **Requirement**: SS-16
**Done when**:
- [ ] Seam is one cohesive section; T2 mixed-signal baseline bit-identical
**Tests**: integration (parity) · **Gate**: full

### T21: Confirm construction stays in the builder
**What**: Verify `CircuitInstance` grows no ad-hoc constructor beyond `from_devices_and_netlist` + documented re-entry (`with_initial_state`, restamp); document the boundary.
**Where**: `core/circuit.rs`, `core/builder.rs`
**Depends on**: T20 · **Requirement**: SS-15
**Done when**:
- [ ] Construction boundary documented; no new constructor added
- [ ] Build gate green (phase close)
**Tests**: none (build gate) · **Gate**: build

### T22: Create `analyses/mod.rs` with shared run config
**What**: New `analyses/` module; move `Context`/`Tolerances`/`Policy` + `init_global` from `solver/mod.rs` into `analyses/mod.rs` (design §2).
**Where**: `crates/piperine-solver/src/analyses/mod.rs` (new), `lib.rs`
**Depends on**: T21 · **Requirement**: SS-01
**Done when**:
- [ ] `analyses` module compiles; `Context`/`Tolerances`/`Policy` re-exported unchanged
- [ ] Full gate green
**Tests**: integration (parity) · **Gate**: full

### T23: Move convergence + config into `analyses/`
**What**: Relocate `solver/convergence.rs` → `analyses/convergence.rs` and `config.rs` → `analyses/config.rs`; fix paths.
**Where**: `analyses/convergence.rs`, `analyses/config.rs`, `lib.rs`
**Depends on**: T22 · **Requirement**: SS-01
**Done when**:
- [ ] Files relocated; imports fixed; full gate green
**Tests**: integration (parity) · **Gate**: full

### T24: Co-locate DC/AC/TF into `analyses/`
**What**: Merge `analysis/{dc,ac,tf}.rs` (data) + `solver/{dc,ac,tf}.rs` (driver) into `analyses/{dc,ac,tf}.rs`, each with documented request-state / driver sections.
**Where**: `analyses/dc.rs`, `analyses/ac.rs`, `analyses/tf.rs`
**Depends on**: T23 · **Requirement**: SS-01/SS-02
**Done when**:
- [ ] Three analyses co-located; sections documented; abi/prelude paths fixed
- [ ] Full gate green
**Tests**: integration (parity) · **Gate**: full

### T25: Co-locate transient/noise/sens/pss into `analyses/`
**What**: Merge the remaining four analyses' data+driver files into `analyses/{transient,noise,sens,pss}.rs`; fold `solver/uic.rs` + `solver/solve.rs` into the analysis/shared module they serve.
**Where**: `analyses/{transient,noise,sens,pss}.rs`
**Depends on**: T24 · **Requirement**: SS-01/SS-02
**Done when**:
- [ ] Four analyses co-located; `uic.rs`/`solve.rs` re-homed; paths fixed
- [ ] Full gate green
**Tests**: integration (parity) · **Gate**: full

### T26: Delete old trees; wire `lib.rs`; layer contracts
**What**: Remove empty `analysis/` + `solver/` dirs; update `lib.rs` mod decls, `abi.rs`, `prelude.rs`; add per-module `//!` responsibility contracts matching design §1.
**Where**: `lib.rs`, `abi.rs`, `prelude.rs`, all `analyses/*`
**Depends on**: T25 · **Requirement**: SS-02/SS-14
**Done when**:
- [ ] No `solver/` or `analysis/` dir; each `analyses/*` + `core/*` carries a `//!` contract
- [ ] Build gate 0 warnings; full gate green (phase close)
**Tests**: integration (parity) · **Gate**: build

### T27: Extract `predict_step` + `attempt_step`
**What**: Pull the predictor-seed/source-update and the Newton candidate solve out of `solve()` into owned methods (design §4).
**Where**: `analyses/transient.rs`
**Depends on**: T26 · **Requirement**: SS-05
**Done when**:
- [ ] Two owned methods; `solve()` calls them; transient parity bit-identical
**Tests**: integration (parity) · **Gate**: full

### T28: Extract `assess_step` + `accept_step`
**What**: Pull LTE + breakpoint-landing assessment and the accept path (commit digital, record, advance history) into owned methods.
**Where**: `analyses/transient.rs`
**Depends on**: T27 · **Requirement**: SS-05
**Done when**:
- [ ] Two owned methods; parity bit-identical; full gate green
**Tests**: integration (parity) · **Gate**: full

### T29: Extract `settle_digital` + `snapshot` + `propose_dt`; slim `solve()`
**What**: Pull the digital settle, the step snapshot, and the next-dt proposal into owned methods; reduce `solve()` to the phase loop.
**Where**: `analyses/transient.rs`
**Depends on**: T28 · **Requirement**: SS-05
**Done when**:
- [ ] `solve()` is a thin loop over named phases; parity bit-identical
**Tests**: integration (parity) · **Gate**: full

### T30: Transient loop parity + size gate
**What**: Verify no transient method exceeds ~60 lines and the full transient suite (breakpoints, UIC, PSS shots, coupled-LC, mixed-signal) is bit-identical.
**Where**: `analyses/transient.rs`
**Depends on**: T29 · **Requirement**: SS-05/SS-06
**Done when**:
- [ ] No method > ~60 lines in the transient driver
- [ ] Build gate 0 warnings; full gate green (phase close)
**Tests**: integration (parity) · **Gate**: build

### T31: Update STATE.md — decisions match reality
**What**: MD-05 → done (`NewtonStrategy`/`StepperStrategy` shipped+wired); add the MD-01 amendment (composed supertraits, C-ABI rationale); add a config-home MD if the user wants one; refresh the Handoff snapshot.
**Where**: `.specs/STATE.md`
**Depends on**: T30 · **Requirement**: SS-13
**Done when**:
- [ ] MD-05 done; MD-01 amendment recorded with date; handoff current
**Tests**: none (docs) · **Gate**: build

### T32: Per-module responsibility contracts audit
**What**: Confirm every solver module (`core/*`, `analyses/*`, `math/*`, `analog/*`, `digital/*`) carries a one-line `//!` contract consistent with design §1; add any missing.
**Where**: `crates/piperine-solver/src/**/*.rs` (module heads)
**Depends on**: T31 · **Requirement**: SS-14
**Done when**:
- [ ] Every module head has a `//!` responsibility line; build gate 0 warnings
**Tests**: none (docs) · **Gate**: build

### T33: Rewrite Part VII §2–§5 (Circuit + Element ABI)
**What**: Rewrite the Circuit-instance and Element-ABI sections to match the built solver: grouped `CircuitInstance` responsibilities, composed supertraits, removed flags/phantom methods, ABI times as `f64` seconds.
**Where**: `docs/spec/part_vii_solver.md`
**Depends on**: T32 · **Requirement**: SS-18
**Done when**:
- [ ] §2–§5 match code; no reference to a removed/phantom construct
**Tests**: none (docs audit) · **Gate**: build

### T34: Audit/rewrite Part VII §8–§16 (algorithms + rules)
**What**: Cross-check every algorithm/contract/failure rule (MNA, DC, transient incl. phase methods + config-homed constants, AC, noise, TF, mixed-signal folded seam, convergence schedules) against the code; correct drift.
**Where**: `docs/spec/part_vii_solver.md`
**Depends on**: T33 · **Requirement**: SS-18
**Done when**:
- [ ] Each §-section verified stated-vs-code; corrections applied
**Tests**: none (docs audit) · **Gate**: build

### T35: Part VII consistency + completeness pass; final gate
**What**: One vocabulary/naming pass matching the code; confirm every public solver contract + analysis algorithm is present. Final full workspace + ngspice gate.
**Where**: `docs/spec/part_vii_solver.md`
**Depends on**: T34 · **Requirement**: SS-18
**Done when**:
- [ ] Part VII internally consistent + complete (finalization gate)
- [ ] `cargo test --workspace` green, 0 warnings; ngspice live
**Tests**: integration (full suite) · **Gate**: build

---

## Phase Execution Map

```
P0 → P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8 → P9

P0:  T1 → T2
P1:  T3 → T4 → T5 → T6
P2:  T7 → T8
P3:  T9 → T10 → T11 → T12
P4:  T13 → T14 → T15 → T16 → T17 → T18
P5:  T19 → T20 → T21
P6:  T22 → T23 → T24 → T25 → T26
P7:  T27 → T28 → T29 → T30
P8:  T31 → T32
P9:  T33 → T34 → T35
```

**Batch packing (~7 tasks/worker, whole phases, sequential):**

| Batch | Phases | Tasks | Count |
| ----- | ------ | ----- | ----- |
| 1 | P0+P1 | T1–T6 | 6 |
| 2 | P2+P3 | T7–T12 | 6 |
| 3 | P4 | T13–T18 | 6 |
| 4 | P5+P6 | T19–T26 | 8 |
| 5 | P7+P8 | T27–T32 | 6 |
| 6 | P9 | T33–T35 | 3 |

6 sequential batch-workers. Verifier runs after T35.

---

## Validation Tables (pre-approval)

### Granularity Check
| Task | Scope | Status |
| ---- | ----- | ------ |
| T1–T2 | one test file each (parity) | ✅ |
| T3,T5,T12 | one flag/doc/toggle change | ✅ |
| T4 | 2 flags + producers + 1 contract test (cohesive) | ✅ |
| T6 | one struct fold | ✅ |
| T7–T8 | alias inline / one re-export | ✅ |
| T9 | one config file | ✅ |
| T10–T11 | one strategy wiring each | ✅ |
| T13–T16 | one trait each (compile chain, noted) | ✅ |
| T17–T18 | one impl-regroup each | ✅ |
| T19–T21 | one section-grouping each | ✅ |
| T22–T26 | one relocation step each | ✅ |
| T27–T29 | 2–3 cohesive method extractions each | ✅ |
| T30,T32 | one audit each | ✅ |
| T31,T33–T35 | one doc target each | ✅ |

### Diagram–Definition Cross-Check
Every `Depends on` is the immediately prior task in its phase (or last task of the prior phase for phase-heads: T3←T2, T7←T6, T9←T8, T13←T12, T19←T18, T22←T21, T27←T26, T31←T30, T33←T32). Diagram arrows match all `Depends on` fields. ✅ No task depends on a later phase.

### Test Co-location Validation
| Task | Layer | Matrix Requires | Task Says | Status |
| ---- | ----- | --------------- | --------- | ------ |
| T1,T2 | analysis parity | integration | integration | ✅ |
| T4,T9,T18 | ABI/contract | unit | unit(+integration) | ✅ |
| T6,T10,T11,T17,T19,T20,T22–T25,T27–T29 | analysis parity | integration | integration | ✅ |
| T3,T5,T7,T8,T12,T21,T26,T30 | structural moves | none (build gate) | none | ✅ |
| T31,T32,T33,T34,T35 | docs | none (build+audit) | none | ✅ |

All ✅ — no violations. Structural/doc tasks legitimately carry `Tests: none` (matrix says "none" for those layers); behavior is guarded by the unchanged suite + P0 parity baselines.

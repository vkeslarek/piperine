# codegen-simplification Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/codegen-simplification/design.md`
**Status**: Draft

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec — confirm before Execute. Guidelines found: `CLAUDE.md` (build/test commands, "zero warnings is the bar", test-placement rule), `AGENTS.md` §"Hard rules" + §"Where the tests are" (MD-28 placement table), `.github/workflows/ci.yml` (`cargo build --workspace`, `cargo test --workspace`), MD-31 (a policy invariant lives in the gate; a guard must be proven able to fail).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `piperine-signal` pure semantics | unit (inline) | All branches; 1:1 to spec ACs; every listed edge case. Matches the crate's existing style — every `src/*.rs` already carries a `#[cfg(test)]` module | `crates/piperine-signal/src/**.rs` (`#[cfg(test)]`) | `cargo test -p piperine-signal` |
| `piperine-codegen` device / kernel / emit | integration | **Zero numeric change** against the T1 baseline is the primary expectation; plus targeted assertions for behavior the existing suite does not pin (CSE call count, construction order) | `crates/piperine-codegen/tests/*.rs`, root `tests/ngspice_validation.rs` | `cargo test -p piperine-codegen`, `cargo test --workspace` |
| `piperine-api` sequencer | unit (inline) | All branches; the `X→1` / `Z→1` / `X→0` / `Z→0` transitions each named explicitly; existing narrow-rule tests rewritten, never deleted | `crates/piperine-api/src/sequencer/*.rs` (`#[cfg(test)]`) | `cargo test -p piperine-api` |
| Cross-crate guards | integration | One witness per delegated family, each **proven able to fail** by injection (MD-31) | root `tests/delegation_guard.rs` | `cargo test --workspace` |
| Cargo manifests / dependency graph | none | build gate only | `crates/*/Cargo.toml` | build gate only |

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | After tasks touching one crate's own tests | `cargo test -p <crate>` |
| Full | After any task touching codegen emission, device construction, or the edge rule | `cargo test --workspace` |
| Build | After phase completion, and for manifest-only tasks | `cargo build --workspace && cargo test --workspace` (CI's own pair; `cargo clippy --workspace --all-targets` additionally must be warning-free) |

**Note on `cargo test`:** bare `cargo test` at the repo root only runs the root package. Every gate uses `--workspace` or `-p`.

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Foundation — prove the dependency edge, capture the baseline

```
T1
```

### Phase 2: Operators

```
T2
```

### Phase 3: Events

```
T3
```

### Phase 4: Limiter (formula + bookkeeping)

```
T4 → T5 → T6
```

### Phase 5: Table

```
T7 → T8
```

### Phase 6: Digital edges (the one behavioral change)

```
T9 → T10 → T11
```

### Phase 7: Guards and cleanup

```
T12 → T13 → T14
```

---

## Task Breakdown

### T1: Add the `piperine-codegen` → `piperine-signal` dependency and capture the zero-change baseline

**What**: Add the dependency, prove Cargo accepts the dev-dependency cycle, and record the baseline the whole feature is measured against.
**Where**: `crates/piperine-codegen/Cargo.toml`
**Depends on**: None
**Reuses**: `crates/piperine-signal/Cargo.toml:9-13` (the existing dev-dep edge)
**Requirement**: CGS-10

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `piperine-signal` is a normal `[dependencies]` entry of `piperine-codegen`
- [x] `cargo build --workspace` succeeds with the cycle in place (`codegen → signal` normal, `signal → codegen` dev) — if Cargo rejects it, STOP and report before writing any other task's code; the fallback is relocating `cross_check_*` out of `piperine-signal/tests/`, not abandoning delegation
- [x] `crates/piperine-signal/tests/purity.rs` still passes (the guard reads `[dependencies]` of *signal*, which is unchanged)
- [x] Baseline recorded in the commit message: total test count, binary count, and the `tests/ngspice_validation.rs` pass count
- [x] Zero rustc warnings
- [x] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: none (manifest layer — build gate only)
**Gate**: build

**Commit**: `build(codegen): depend on piperine-signal ahead of delegation`

---

### T2: Delegate the runtime operators to `signal::ops::Operator`

**What**: Replace codegen's `Operator` enum with a `SlottedOperator` wrapper around the signal type, and rewire the construction and accept sites.
**Where**: `crates/piperine-codegen/src/device/analog/operators.rs`, `crates/piperine-codegen/src/device/analog/instance.rs` (`:207-243` construction, `:1284` accept)
**Depends on**: T1
**Reuses**: `signal::ops::Operator::{delay, slew, transition, integrate, accept}`
**Requirement**: CGS-01, CGS-02

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `piperine-codegen` defines no `enum Operator`; `operators.rs` holds only `SlottedOperator { slot, op }` with `slot()` and `accept()`
- [x] `RuntimeState::{Delay, Slew, Transition, Integrator}` construct the corresponding `signal::ops::Operator` variant with identical parameters
- [x] The `idt` `ic` seeding path still sets the integrator's `value` at construction, before any accept (AC 3)
- [x] `accept_timestep` still mutates operator state **only** on accepted timesteps — no call added to a load path (AC 4)
- [x] `tests/ngspice_validation.rs` numbers identical to T1's baseline
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline (no deletions)

**Tests**: integration
**Gate**: full

**Commit**: `refactor(codegen): delegate runtime operators to piperine-signal`

---

### T3: Delegate the event detectors to `signal::events::EventDetector`

**What**: Map `CompiledTrigger` to `signal::events::Trigger` and let the signal detector make the fire decision; `EventSystem` keeps periods, `@initial` dispatch and the variable-action wiring.
**Where**: `crates/piperine-codegen/src/device/analog/events.rs`
**Depends on**: T1
**Reuses**: `signal::events::{EventDetector, Trigger, CrossDir}`
**Requirement**: CGS-03, CGS-04

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `piperine-codegen` defines no `struct EventDetector`
- [x] `CompiledTrigger::Initial` is short-circuited by `EventSystem` and is **not** given a `signal::events::Trigger` counterpart (AC 7)
- [x] The seeding asymmetry holds: an unseeded `cross` does not fire; an unseeded `above` with a positive value does (AC 6) — asserted by the existing event tests, which must remain green unmodified
- [x] `EventSystem` still owns the per-event timer period table and the `@initial`/fired-event variable-action dispatch (AC 8)
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline

**Tests**: integration
**Gate**: full

**Commit**: `refactor(codegen): delegate analog event detection to piperine-signal`

---

### T4: Add `LIMIT_FNS` and register the limiter symbols with the analog JIT

**What**: A codegen-local symbol table (`MATH_FNS` shape) of `extern "C"` wrappers over `signal::limit::{pnjlim, fetlim, limvds}`, imported into the analog JIT module.
**Where**: `crates/piperine-codegen/src/kernel/analog/limits.rs`, `crates/piperine-codegen/src/kernel/analog/compile.rs` (`:148-164`)
**Depends on**: T1
**Reuses**: `piperine-lang/src/math.rs:8-73` (table + `mod wrappers` shape), `compile.rs:148-164` (import loop; the signature builder already generalizes over `arity`)
**Requirement**: CGS-05 (AC 9)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `LimitFn { name, arity, symbol }` + `const LIMIT_FNS` with three entries (`pnjlim`/4, `fetlim`/3, `limvds`/2), `unsafe impl Sync` as `MathFn` does
- [x] Wrappers are `extern "C"` and call `signal::limit::*` — no formula is retyped (MD-13 rule 5: data table + plain helpers, no macro)
- [x] Symbols registered in the **analog** JIT only; `kernel/digital/compile.rs`'s `MATH_FNS` loops untouched
- [x] Nothing emits a call yet — this task only makes the symbols linkable, so the suite must be bit-identical to T1's baseline
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline

**Tests**: integration
**Gate**: full

**Commit**: `feat(codegen): register piperine-signal limiter symbols with the analog JIT`

---

### T5: Emit `$limit` as a call and delete the inline IR formulas

**What**: Replace the three `emit_*lim` IR bodies with a call to the registered symbol, keeping kind dispatch, argument order, `vold` load and `CseKey::Limit` CSE.
**Where**: `crates/piperine-codegen/src/emit/analog_expr.rs` (`:297-465`)
**Depends on**: T4
**Reuses**: `analog_call_math`'s call+CSE shape (`analog_expr.rs:556-573`), `CseKey::Limit(slot)`
**Requirement**: CGS-05 (AC 10, 11), CGS-06 (AC 12, 13)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `emit_pnjlim`, `emit_fetlim`, `emit_limvds`, `emit_min`, `emit_max` are deleted (lines 331–465); `emit_min`/`emit_max` had no other call sites
- [x] A repeated structurally-equal `$limit` emits **exactly one** call per distinct slot — asserted by a test, not assumed (AC 11; the CSE-regression risk in design §Risks)
- [x] Every emission site delegates, including the parameter-Jacobian kernel and `fn`-inlined bodies (AC 12) — covered by a test exercising a `$limit` inside an inlined `fn`
- [x] `resolve/diff.rs` is **unmodified**; `$limit` stays transparent to differentiation (AC 13)
- [x] The unsupported-kind arm still returns `CodegenError::unsupported` naming the kind (AC 15)
- [x] `tests/ngspice_validation.rs` junction devices (dio, bjt, mos1, jfet) identical to T1's baseline, DC operating points included
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline + the new CSE and inlined-`fn` assertions

**Tests**: integration
**Gate**: full

**Commit**: `refactor(codegen): emit $limit as a call into piperine-signal`

---

### T6: Delegate the device-side limiter bookkeeping to `signal::limit::Limiter`

**What**: Replace codegen's `Limiter` struct with the signal one, keeping `seed()`, the checkpoint packing, and the construction order.
**Where**: `crates/piperine-codegen/src/device/analog/limits.rs`
**Depends on**: T5
**Reuses**: `signal::limit::Limiter::{new, update, active, active_slot, limited}`, `signal::limit::LimitKind`
**Requirement**: CGS-07

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `piperine-codegen` defines no `struct Limiter`; `active` / `active_slots` / `seeds` come from the signal type
- [x] `seed()` stays in codegen (it evaluates `vcrit` through `AnalogKernel` + `SimCtx`) and still runs **after** `@initial` events in `AnalogInstance::new` — the two-step order the file header documents
- [x] `limiting_report` still names the slot that actually clamped (`active_slot`)
- [x] Checkpoint/restore round-trips: `checkpoint_limiter` → `restore_limiter` reproduces the pre-checkpoint limiter state
- [x] `tests/ngspice_validation.rs` identical to T1's baseline
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline

**Tests**: integration
**Gate**: full

**Commit**: `refactor(codegen): delegate limiter bookkeeping to piperine-signal` (`b4f2c4e`)

**SPEC_DEVIATION (resolved, user-approved before implementing)**: the original sketch's
`Limiter::update(vnew, kinds)` recomputes the formula from scratch, but codegen never needed
that (T4/T5 already moved formula computation into the JIT). Fix: `piperine-signal/src/limit.rs`
gained `Limiter::advance_precomputed(vlim, vnew, vold: &mut [f64])` — the rules-2/4 loop
(seed preservation + convergence veto) factored out of `update` so both paths share one
implementation, applied to an already-computed `vlim` over a caller-owned `vold` slice it never
copies — plus two small accessors, `seeds()` and `restore_seeds(seeds, active)`, needed for
checkpoint pack/unpack. `device/analog/limits.rs`'s `Limiter` is now
`{ inner: piperine_signal::limit::Limiter }`; `update()` still gets `vlim`/`vnew` from
`kernel.eval_limit_update`/`eval_limit_vnew` (unchanged) and delegates rules 2+4 to
`inner.advance_precomputed(&vlim, &vnew, &mut state[base..base+nl])`. `kernel.rs` and
`instance.rs` untouched, as decided. 5 new piperine-signal unit tests;
`tests/checkpoint_limiter.rs` and `tests/limiters.rs` (both unmodified) pass unchanged,
including the active-flag+seeds+vold round-trip tests.

---

### T7: Expose validated segment geometry on `signal::behavior::table::Table`

**What**: Add read-only accessors so a second renderer can build from the validated data without re-deriving the rules.
**Where**: `crates/piperine-signal/src/behavior/table.rs`
**Depends on**: T1
**Reuses**: the existing private `slope` and the `with_mode` validation
**Requirement**: CGS-08

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `Table::points(&self) -> (&[f64], &[f64])` and `Table::segment_slope(&self, i: usize) -> f64` are public
- [x] `value` / `derivative` still share the private `segment` lookup — no evaluation logic is duplicated or moved
- [x] Module doc states the accessors exist for **renderers**, not evaluators
- [x] Unit tests: accessors return the validated arrays; `segment_slope(i)` equals `derivative(x)` for an `x` inside segment `i`
- [x] Gate check passes: `cargo test -p piperine-signal`
- [x] Test count: ≥ existing + 2

**Tests**: unit
**Gate**: quick

**Commit**: `feat(signal): expose Table segment geometry for renderers`

---

### T8: Build codegen's `table` `If`-chain from the validated `signal::Table`

**What**: Construct `signal::Table::with_mode` in the lowerer and render the nested-`If` from its segments; map `TableError` onto the existing loud-error path.
**Where**: `crates/piperine-codegen/src/resolve/pom/analog_ops.rs` (`:128-232`)
**Depends on**: T7
**Reuses**: `Table::const_array` (unchanged), `Table::err` (unchanged messages), `signal::Table::{points, segment_slope}`
**Requirement**: CGS-08, CGS-09

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] Validation (strictly-increasing `xs`, length match, `< 2` points, non-`"linear"` mode) happens **only** in `signal::Table`; codegen re-implements none of it
- [x] User-facing error messages are unchanged for every failure mode (AC 17)
- [x] Non-constant `xs`/`ys` still fail loud on the existing `const_array` path, before any `signal::Table` is built (AC 20)
- [x] The lowered expression clamps to the end values outside `[xs[0], xs[n-1]]` (AC 18), including the two-point table
- [x] `resolve/diff.rs` is unmodified; the derivative is still the segment slope read off the same `If` structure (AC 19)
- [x] `tests/host_capability_parity.rs::table_nonlinearity_matches_between_phdl_and_a_host_device` passes unmodified
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline

**Tests**: integration
**Gate**: full

**Commit**: `refactor(codegen): lower table from the validated piperine-signal Table`

---

### T9: Add `signal::logic` — the four-state value and the IEEE 1364 edge rule

**What**: A new pure module owning `Logic`, `EdgeDirection` and `EdgeDirection::fires`, with the semantics codegen currently implements.
**Where**: `crates/piperine-signal/src/logic.rs`, `crates/piperine-signal/src/lib.rs`
**Depends on**: T1
**Reuses**: the rule at `device/digital.rs:286` (the correct one); `MathFn`-free, dependency-free by construction
**Requirement**: CGS-13 (AC 29, 34)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `enum Logic { Zero = 0, One = 1, X = 2, Z = 3 }` with discriminants pinned to `LogicValue`'s and `Quad`'s, documented as a convention with a test
- [x] `EdgeDirection::fires` is a **method** (MD-13 rule 2), implementing: `Rising` ⟺ `before != One && after == One`; `Falling` ⟺ `before != Zero && after == Zero`; `Either` ⟺ `before != after`
- [x] Unit tests cover the full 4×4 transition table for all three directions — `X→1`, `Z→1`, `X→0`, `Z→0` each named explicitly (AC 29)
- [x] `piperine-signal` gains no dependency; `tests/purity.rs` passes (AC 34)
- [x] Gate check passes: `cargo test -p piperine-signal`
- [x] Test count: ≥ existing + 3 (one per direction, each covering its transition table)

**Tests**: unit
**Gate**: quick

**Commit**: `feat(signal): own the four-state edge rule (IEEE 1364)`

---

### T10: Delegate codegen's digital edge decision to `signal::logic`

**What**: Both the sequential and combinational trigger paths call `EdgeDirection::fires` over converted `Quad` values.
**Where**: `crates/piperine-codegen/src/device/digital.rs` (`:286` seq, `:329` comb, `:19-35` `Quad`)
**Depends on**: T9
**Reuses**: `Quad::to_logic`'s `_ => X` folding; `signal::logic::{Logic, EdgeDirection}`
**Requirement**: CGS-13 (AC 30)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] Neither path contains an inline edge comparison; both call `fires`
- [x] `EdgeKind → EdgeDirection` mapping lives beside the `Quad` helpers
- [x] A watch value outside `0..=3` still folds to `X` and still fires `Rising` on a transition to `1` — asserted, since it is the one case where "identical to today" needs an argument
- [x] `tests/ngspice_validation.rs` and the mixed-signal suites identical to T1's baseline — codegen's behavior is **unchanged** by this task
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline + 1

**Tests**: integration
**Gate**: full

**Commit**: `refactor(codegen): delegate digital edge detection to piperine-signal`

---

### T11: Fix the host edge rule by delegating `EdgeWait` to `signal::logic`

**What**: Delete `EdgeDirection::matches`, delegate `EdgeWait::observe`, re-export `EdgeDirection` under its existing name, and rewrite the tests that encoded the narrow rule.
**Where**: `crates/piperine-api/src/sequencer/wait.rs`, `crates/piperine-api/src/sequencer/mod.rs:40`
**Depends on**: T10
**Reuses**: `signal::logic::EdgeDirection::fires`; the existing `baseline: Option<LogicValue>` arming rule
**Requirement**: CGS-13 (AC 31), CGS-14 (AC 32, 33)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `EdgeDirection::matches` is deleted; `observe` converts `LogicValue → Logic` and calls `fires`
- [x] `EdgeDirection` is re-exported under its existing name and path, so `pub use wait::EdgeDirection` and the Python bindings compile unchanged
- [x] A wait armed with no prior settled sample still does not fire on its first observation (AC 33) — the arming rule stays host-side and is **not** pushed into `fires`
- [x] `rising_matches_zero_to_one_only` is **rewritten** (not deleted) to the IEEE rule, and a new test asserts `X → 1` satisfies a `Rising` wait (AC 32)
- [x] Falling's `X→0` and `Z→0` cases each have a test
- [x] This is the feature's **only** intended behavior change; any other suite difference is a defect (AC 21)
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline + 3

**Tests**: unit + integration
**Gate**: full

**Commit**: `fix(api): host edge waits follow IEEE 1364, matching PHDL @(posedge)`

---

### T12: Add `tests/delegation_guard.rs` — five witnesses, each proven able to fail

**What**: A registry+exhaustiveness source-walk asserting no delegated family has a second copy, in either crate.
**Where**: root `tests/delegation_guard.rs`
**Depends on**: T11
**Reuses**: `tests/suite_hygiene.rs`'s source-walking pattern; MD-31's registry+exhaustiveness shape
**Requirement**: CGS-11 (AC 22, 23)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] A `const FAMILIES` registry with five entries (operators, events, limiter, table, digital edges), each naming its forbidden identifiers and the source roots to walk
- [x] The digital-edges entry walks **both** `crates/piperine-codegen/src` and `crates/piperine-api/src`
- [x] Failure names the family and the offending `file:line`
- [x] **Each of the five witnesses is proven able to fail**: inject the violation, watch it fail, revert (MD-31 — a guard that has not been proven to fail is decoration). The commit message records that this was done for all five
- [x] A `//!` scope header per `suite_hygiene.rs`'s requirement
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥ T1 baseline + 5

**Tests**: integration
**Gate**: full

**Commit**: `test: guard the piperine-signal delegation against a second copy`

---

### T13: Delete the four `cross_check_*` fixtures

**What**: Remove the interim anti-drift fixtures whose two implementations no longer exist.
**Where**: `crates/piperine-signal/tests/cross_check_{operators,events,limiter,table}.rs`
**Depends on**: T12
**Reuses**: nothing — this is the deletion those files' own headers ask for
**Requirement**: CGS-11 (AC 24, 25)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] All four targets deleted
- [x] `tests/host_capability_parity.rs` is **untouched** (it compares two bindings of one implementation, per its own header)
- [x] `crates/piperine-signal/tests/purity.rs` still passes
- [x] `piperine-signal`'s `[dev-dependencies]` on `piperine-codegen`/`piperine-solver`/`piperine-lang` are dropped **only if** no remaining test in the crate uses them — otherwise left in place and the reason recorded
- [x] `tests/suite_hygiene.rs` passes (no dangling registrations)
- [x] Gate check passes: `cargo build --workspace && cargo test --workspace`
- [x] Test count: T1 baseline minus exactly the deleted fixtures' tests, plus everything added since — accounted for explicitly in the commit message

**Tests**: integration
**Gate**: build

**Commit**: `test(signal): drop the cross-check fixtures, delegation landed`

---

### T14: Measure and report the shrinkage

**What**: Verify the five per-file caps and the net-deletion floor; report actuals.
**Where**: `.specs/features/codegen-simplification/` (report), no source change expected
**Depends on**: T13
**Reuses**: spec AC 26's cap table
**Requirement**: CGS-12

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `wc -l` recorded for `device/analog/operators.rs` (≤ 100), `device/analog/events.rs` (≤ 140), `device/analog/limits.rs` (≤ 130), `emit/analog_expr.rs` (≤ 500), `resolve/pom/analog_ops.rs` (≤ 330)
- [x] `piperine-codegen` net line delta measured against the 16 670 baseline; **≥ 300 removed** (AC 27) — **MISSED: 196 removed, see shrinkage.md**
- [x] Any missed cap is reported with the actual number and the reason, and renegotiated in the open — **never** met by moving code to another file (AC 28) — see `.specs/features/codegen-simplification/shrinkage.md`
- [x] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: none (measurement task; no code layer created)
**Gate**: build

**Commit**: `docs(codegen-simplification): record measured shrinkage`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7

Phase 1:  T1
Phase 2:  T2
Phase 3:  T3
Phase 4:  T4 ──→ T5 ──→ T6
Phase 5:  T7 ──→ T8
Phase 6:  T9 ──→ T10 ──→ T11
Phase 7:  T12 ──→ T13 ──→ T14
```

Execution is strictly sequential. 14 tasks → packs into **2 batches** on the ~7-task budget (Phases 1–4 = 6 tasks; Phases 5–7 = 8 tasks), so batch sub-agents will be offered at Execute.

---

## Task Granularity Check

| Task | Scope | Status |
|---|---|---|
| T1: dependency + baseline | 1 manifest | ✅ Granular |
| T2: operators delegation | 1 type + its call sites in 1 file | ✅ Granular |
| T3: events delegation | 1 file | ✅ Granular |
| T4: `LIMIT_FNS` + registration | 1 table + 1 loop, 2 cohesive files | ✅ Granular |
| T5: `$limit` emits a call | 1 function + deletions in 1 file | ✅ Granular |
| T6: limiter bookkeeping | 1 file | ✅ Granular |
| T7: `Table` accessors | 2 methods, 1 file | ✅ Granular |
| T8: `table` lowering | 1 function | ✅ Granular |
| T9: `signal::logic` | 1 module | ✅ Granular |
| T10: codegen edge delegation | 2 call sites, 1 file | ✅ Granular |
| T11: host edge fix | 1 function + its tests | ✅ Granular |
| T12: delegation guard | 1 test target | ✅ Granular |
| T13: fixture deletion | 4 file deletions, one concern | ✅ Granular |
| T14: measurement | 1 report | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
|---|---|---|---|
| T1 | None | (phase 1 root) | ✅ Match |
| T2 | T1 | Phase 2 after Phase 1 | ✅ Match |
| T3 | T1 | Phase 3 after Phase 2 (⊇ T1) | ✅ Match |
| T4 | T1 | Phase 4 after Phase 3 (⊇ T1) | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | T5 | T5 → T6 | ✅ Match |
| T7 | T1 | Phase 5 after Phase 4 (⊇ T1) | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T1 | Phase 6 after Phase 5 (⊇ T1) | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | T10 | T10 → T11 | ✅ Match |
| T12 | T11 | Phase 7 after Phase 6 | ✅ Match |
| T13 | T12 | T12 → T13 | ✅ Match |
| T14 | T13 | T13 → T14 | ✅ Match |

No task depends on a later-phase task. T3, T4, T7 and T9 declare `T1` only; the phase ordering is stricter than their true dependency, which is intentional — the sequencing exists so a numeric regression localizes to one family (design §Approach).

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
|---|---|---|---|---|
| T1 | Cargo manifest | none | none | ✅ OK |
| T2 | codegen device | integration | integration | ✅ OK |
| T3 | codegen device | integration | integration | ✅ OK |
| T4 | codegen kernel | integration | integration | ✅ OK |
| T5 | codegen emit | integration | integration | ✅ OK |
| T6 | codegen device | integration | integration | ✅ OK |
| T7 | signal pure semantics | unit | unit | ✅ OK |
| T8 | codegen resolve | integration | integration | ✅ OK |
| T9 | signal pure semantics | unit | unit | ✅ OK |
| T10 | codegen device | integration | integration | ✅ OK |
| T11 | api sequencer + cross-crate | unit (+ integration) | unit + integration | ✅ OK |
| T12 | cross-crate guard | integration | integration | ✅ OK |
| T13 | test targets (deletion) | integration | integration | ✅ OK |
| T14 | none (measurement) | none | none | ✅ OK |

No violations. Note on T2–T6, T8, T10: their coverage expectation is **zero numeric change against the T1 baseline**, satisfied by existing suites plus the named targeted assertions — this is not `Tests: none` in disguise, and each task lists the specific new assertion where the existing suite does not already pin the behavior.

---

## Tools

No MCP servers are configured for this project, and no skill applies to these tasks beyond `tlc-spec-driven` itself. Every task is `MCP: NONE / Skill: NONE`. Say so if you want a different tool posture before Execute.

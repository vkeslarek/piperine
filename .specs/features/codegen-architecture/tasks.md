# Codegen Architecture Refactor Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Spec**: `.specs/features/codegen-architecture/spec.md`
**Design**: `.specs/features/codegen-architecture/design.md`
**Status**: All 18 tasks complete (T1–T18). Dispatching feature-level Verifier next.

**Refactor invariant (every task):** zero functional change. The existing test
suite passes **unchanged** — no test weakened, skipped, added, or rewritten to
fit the new shape. The suite is the safety net; behavior is byte-for-byte
preserved. Every commit builds the whole workspace with **zero warnings**.

---

## Test Coverage Matrix

> Generated from codebase + guidelines. Guidelines found: `CLAUDE.md`
> (§Build and test, §Tests of record), `.specs/STATE.md` (MD-13 idiom rules).
> **This is a behavior-preserving refactor** — the coverage expectation is *the
> existing suite stays green unchanged*, NOT new tests. Writing a new test to
> "cover" a moved item would be test-mirroring the implementation (forbidden).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Codegen kernel/emit/resolve/flatten (moved or split) | regression (existing) | The existing codegen tests pass **unchanged** — value-for-value identical outputs | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Device / circuit (capability + responsibility split) | regression (existing) | Existing device/circuit + solver + ngspice tests pass unchanged (stamping equivalence) | `crates/piperine-codegen/tests/*.rs`, root `tests/*.rs`, `crates/piperine-solver/tests/*.rs` | `cargo test --workspace` |
| Cross-crate call sites (dropping `ir` alias, moved pub items) | regression (build) | Workspace builds; no broken import | (all importers) | `cargo build --workspace` |
| Docs (CLAUDE.md, STATE.md, ROADMAP) | none | build gate only | `CLAUDE.md`, `.specs/STATE.md`, `ROADMAP.md` | build gate |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | Task touching only `piperine-codegen` internals | `cargo test -p piperine-codegen` |
| Full | Task moving public items / touching device stamping / cross-crate | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |
| Build | Docs-only tasks | `cargo build --workspace` (zero warnings) |

> **Zero rustc warnings is a hard gate on every task** (CLAUDE.md).

---

## Execution Plan

Phases ordered, run sequentially; tasks within a phase run in order.

### Phase 1: Crate-root contracts + `resolve` rename (foundation)
```
T1 → T2
```
### Phase 2: `emit` module (from `codegen/`) — split the emission machinery
```
T3 → T4 → T5 → T6
```
### Phase 3: `flatten` + `kernel` module moves (dissolve `jit/`)
```
T7 → T8 → T9
```
### Phase 4: `AnalogKernel` capability decomposition
```
T10
```
### Phase 5: `device/analog` split by capability (`Stamps` trait)
```
T11 → T12 → T13
```
### Phase 6: `device/circuit` split by responsibility
```
T14 → T15 → T16
```
### Phase 7: façade + conventions
```
T17 → T18
```

---

## Task Breakdown

### T1: `error.rs` crate-root — home `CodegenError`, fix messages

**What**: Create `crates/piperine-codegen/src/error.rs`, move `CodegenError`
out of `jit/mod.rs`, fix the stale `ModuleNotFound` message ("in IrProgram" →
the real module lookup), audit for provably-redundant variants; update every
in-crate reference.
**Where**: `src/error.rs` (new), `src/jit/mod.rs`, all `CodegenError` importers
**Depends on**: None
**Reuses**: the current enum verbatim
**Requirement**: CGA-07
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `CodegenError` lives in `error.rs`; `lib.rs` re-exports it.
- [x] `rg 'IrProgram'` — the stale `ModuleNotFound` message no longer contains it (fixed). Note: 3 unrelated pre-existing doc-comment mentions of the deleted `IrProgram` *type* survive in `device/mod.rs`, `device/circuit.rs`, `lower/pom/mod.rs` (+ a test) — these describe its historical absence, are outside T1's `CodegenError`-importer scope, and were left untouched per the surgical-changes rule. Flagged as a deviation, not fixed here.
- [x] Full gate passes: `cargo build --workspace` (zero warnings) + `cargo test --workspace`.
- [x] Test count unchanged (no silent deletions) — 582 passed, 0 failed.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): home CodegenError in error.rs, fix stale messages (CGA-07)` — `cdc4201`
**Status**: ✅ Complete

---

### T2: Rename `lower/` → `resolve/`, drop `ir` alias

**What**: Rename the module directory `lower/` → `resolve/`, remove
`pub use lower as ir`, and update every call site (`crate::ir::…`,
`piperine_codegen::ir::…`, `lower::…`, `as ir`) across the workspace to
`resolve::…`.
**Where**: `src/resolve/` (from `src/lower/`), `src/lib.rs`, all importers (in-crate + sibling crates)
**Depends on**: T1
**Reuses**: all `lower/` contents unchanged (rename only)
**Requirement**: CGA-01, CGA-02
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `src/lower/` gone; `src/resolve/` present with identical contents.
- [x] `rg 'as ir\b|::ir::|mod lower'` returns nothing in the workspace.
- [x] Full gate passes (build zero-warning + workspace tests).
- [x] Test count unchanged — 582 passed, 0 failed.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): rename lower→resolve, drop legacy ir alias (CGA-01,02)` — `63ed66b`
**Status**: ✅ Complete

---

### T3: Create `emit/` from `codegen/`; move `SimCtx` + `Codegen` trait

**What**: Rename `codegen/` → `emit/`; move `SimCtx` from `jit/mod.rs` →
`emit/abi.rs`; rename `codegen/trait_.rs` (the `Codegen` trait) →
`emit/digital_expr.rs`. Update `lib.rs`/importers.
**Where**: `src/emit/` (from `src/codegen/`), `src/emit/abi.rs`, `src/emit/digital_expr.rs`, `src/jit/mod.rs`, `src/lib.rs`
**Depends on**: T2
**Reuses**: `codegen/` contents; `SimCtx`/`Codegen` verbatim
**Requirement**: CGA-01, CGA-08
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `src/codegen/` gone; `src/emit/` present; `SimCtx` in `emit/abi.rs`; `Codegen` in `emit/digital_expr.rs`.
- [x] `rg 'mod codegen|trait_'` returns nothing in the crate.
- [x] Full gate passes.
- [x] Test count unchanged — 582 passed, 0 failed.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): emit module + SimCtx/Codegen relocation (CGA-01,08)` — `9aac07c`
**Status**: ✅ Complete

---

### T4: Split `builder.rs` → `builder` + `resolver` + `cse`

**What**: Split `emit/builder.rs` (1293 LOC): keep `Builder`+`Tape` in
`emit/builder.rs`; extract `Resolver`/`Typed`/`DigTy` → `emit/resolver.rs`;
extract `CseKey`/`SimField`/`expr_structural_eq` → `emit/cse.rs`. Every helper
keeps a trait/struct owner (MD-13 r2).
**Where**: `src/emit/builder.rs`, `src/emit/resolver.rs` (new), `src/emit/cse.rs` (new)
**Depends on**: T3
**Reuses**: existing types verbatim, regrouped
**Requirement**: CGA-05, CGA-06
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `Resolver`/`Typed`/`DigTy` in `resolver.rs`; CSE in `cse.rs`.
- [x] No module-scope loose `fn` introduced (`expr_structural_eq`/`bin_tag` stay free functions in `cse.rs`, matching their pre-existing shape — pure data-keyed helpers, not owned by a struct; unchanged from before the split, no new loose fns added).
- [x] Quick gate passes: `cargo test -p piperine-codegen`.
- [x] Test count unchanged.
**Tests**: regression · **Gate**: quick
**Commit**: `refactor(codegen): split emit builder into builder/resolver/cse (CGA-05,06)` — `5040c64`
**Status**: ✅ Complete

---

### T5: Extract statement emission → `emit/stmt.rs`

**What**: Move `Builder::emit_stmt`, `emit_guarded_block`, `emit_if_branch` into
`emit/stmt.rs` (as `Builder` impl methods).
**Where**: `src/emit/stmt.rs` (new), `src/emit/builder.rs`
**Depends on**: T4
**Reuses**: the methods verbatim
**Requirement**: CGA-05
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] Statement-emission methods live in `emit/stmt.rs`; `builder.rs` no longer holds them.
- [x] Quick gate passes.
- [x] Test count unchanged.
**Tests**: regression · **Gate**: quick
**Commit**: `refactor(codegen): move statement emission to emit/stmt.rs (CGA-05)` — `8872369`
**Status**: ✅ Complete

Note: also moved `emit_assign`/`store_var`/`store_net`/`emit_match`/`pattern_flag` — private helpers called exclusively from `emit_stmt`'s dispatch chain and nowhere else (verified via `rg`). Splitting only the 3 named pub methods while leaving their sole private callees behind would have fragmented one statement-emission unit across two files, contradicting the file's own "Owns: statement-level emission" boundary. Pure relocation, no behavior change.

---

### T6: Split the 594-line `emit_analog` → category helpers

**What**: In `emit/analog_expr.rs` (from `codegen/analog_emit.rs`), decompose
`Builder::emit_analog` into private per-category helpers (arithmetic, `V`/`I`
access, call/syscall, conditional, literal) dispatched by a lean top matcher —
**no emission reordering** (byte-identical IR).
**Where**: `src/emit/analog_expr.rs` (from `src/codegen/analog_emit.rs`)
**Depends on**: T5
**Reuses**: the existing match arms, regrouped into methods
**Requirement**: CGA-05
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `emit_analog` is a lean dispatcher; each category is its own `Builder` method. (Already true going in — `emit_analog` was a ~90-line top matcher dispatching to `emit_analog_branch`/`emit_state_load`/`emit_analog_syscall`/`emit_analog_unary`/`emit_analog_binary`/`emit_analog_block_value`/`$limit` helpers; no further split needed. This task's remaining work was renaming the file to its design-mapped home, `emit/analog_expr.rs`.)
- [x] `analog_jit.rs` value-for-value tests pass unchanged (no numeric drift). Note: `crates/piperine-codegen/tests/analog_jit.rs` is `#![cfg(any())]`-disabled crate-wide ("pending rewrite for POM Expr/Stmt", pre-existing, unrelated to this refactor) — 0 tests run from that file both before and after. Numeric-drift coverage instead comes from the value-for-value tests that DO run: `disto_jit.rs`, `digital_jit.rs`, `silent_bugs.rs`, `codegen_ir.rs`, root `ngspice_validation.rs` — all pass unchanged.
- [x] Quick gate passes; full-workspace gate also re-verified (582 passed, 0 failed).
- [x] Test count unchanged.
**Tests**: regression · **Gate**: quick
**Commit**: `refactor(codegen): split emit_analog into category helpers (CGA-05)` — `d1380c2`
**Status**: ✅ Complete

---

### T7: Promote `flatten/` stage (from `jit/flatten.rs`)

**What**: Move `jit/flatten.rs` → `flatten/analog.rs` (+ `flatten/mod.rs`);
update paths.
**Where**: `src/flatten/analog.rs`, `src/flatten/mod.rs` (new), `src/lib.rs`
**Depends on**: T3
**Reuses**: `jit/flatten.rs` verbatim
**Requirement**: CGA-01
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `flatten/` module present; flattener importable from its stage path.
- [x] Quick gate passes.
- [x] Test count unchanged.
**Tests**: regression · **Gate**: quick
**Commit**: `refactor(codegen): promote flatten stage out of jit (CGA-01)` — `7351c94`
**Status**: ✅ Complete

---

### T8: Move `jit/digital/` → `kernel/digital/`, delete `jit/`

**What**: Move `jit/digital/` (compile/network/abi/layout) under
`kernel/digital/`; remove the now-empty `jit/` module (its `CodegenError` and
`SimCtx` already relocated).
**Where**: `src/kernel/digital/` (from `src/jit/digital/`), `src/jit/` (removed), `src/lib.rs`
**Depends on**: T3, T7
**Reuses**: `jit/digital/` verbatim
**Requirement**: CGA-01
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `src/jit/` gone; `DigitalKernel` under `kernel/digital/`.
- [x] `rg 'mod jit'` returns nothing.
- [x] Quick gate passes.
- [x] Test count unchanged.
**Tests**: regression · **Gate**: quick
**Commit**: `refactor(codegen): move digital+analog kernels under kernel/, delete jit module (CGA-01)` — `e688a47`
**Status**: ✅ Complete

Deviation: T9's "Where"/task text names `src/jit/analog.rs` as T9's source, but T8's own done-when
requires `src/jit/` gone entirely (and `rg 'mod jit'` empty) — the two task definitions conflict on
when `jit/analog.rs` moves. Resolved per the orchestrator's explicit framing ("dissolving jit/ is
your job in T8"): T8 relocates *both* `jit/digital/` and `jit/analog.rs` (the latter unsplit, as a
single `kernel/analog.rs` file), fully deleting `jit/`. T9 then performs the struct/compile split
starting from `kernel/analog.rs` (its actual predecessor file) rather than literally `jit/analog.rs`
as the stale task text states. No functional impact — this only affects which task performs which
mechanical file move.

---

### T9: Move `AnalogKernel` → `kernel/analog/` (struct + compile split)

**What**: Move `jit/analog.rs` → `kernel/analog/mod.rs` (struct + `eval_*`
surface) and `kernel/analog/compile.rs` (the long `compile` routine).
**No** capability regrouping yet (that is T10) — pure relocation + compile split.
**Where**: `src/kernel/analog/mod.rs`, `src/kernel/analog/compile.rs` (from `src/jit/analog.rs`)
**Depends on**: T8
**Reuses**: `jit/analog.rs` verbatim
**Requirement**: CGA-01
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `AnalogKernel` under `kernel/analog/`; compile routine in `compile.rs`.
- [x] Quick gate passes (also re-verified full workspace gate).
- [x] Test count unchanged — 99 test-result blocks, 0 failed.
**Tests**: regression · **Gate**: quick
**Commit**: `refactor(codegen): relocate AnalogKernel to kernel/analog, split compile (CGA-01)` — `4faed3d`
**Status**: ✅ Complete

---

### T10: Decompose `AnalogKernel` into capability sub-structs + `AnalogCapability`

**What**: Introduce the `AnalogCapability` trait and per-capability structs
(`Reactive`, `Forces`, `Limits`, `Noise`, `AcStim`) in
`kernel/analog/{reactive,forces,limits,noise,ac_stim}.rs`; regroup
`AnalogKernel` fields into `core: AnalogCore` + `Option<Cap>`; rewrite each
`has_*()` as `self.<cap>.is_some()`/emptiness. `eval_*` delegates to the present
capability (empty path = today's `None` branch, identical behavior).
**Where**: `src/kernel/analog/mod.rs`, `src/kernel/analog/{reactive,forces,limits,noise,ac_stim}.rs` (new), `src/kernel/analog/compile.rs`
**Depends on**: T9
**Reuses**: `AnalogFn`, existing field data + eval logic (regrouped, not rewritten)
**Requirement**: CGA-04
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `AnalogKernel` = `core` + `Option<Capability>` per optional capability; `has_*` bools gone (presence checks).
- [x] One file per capability under `kernel/analog/`.
- [x] `analog_jit.rs` (cfg'd off, pre-existing — see T6) + full workspace suite pass unchanged (99 test-result blocks, 0 failed).
- [x] Full gate passes (zero warnings + `cargo test --workspace` green, re-verified in a clean single run after ruling out a concurrent-test-run race).
- [x] Test count unchanged.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): AnalogKernel capability sub-structs behind Option (CGA-04)` — `afe550e`
**Status**: ✅ Complete

Deviation (documented in the commit body): `ac_idt_jacobian` stays its own top-level
`Option<AnalogFn>` rather than nesting inside `Reactive` as design.md's illustrative sketch
showed. `device/analog.rs` checks `has_reactive()` and `has_ac_idt()` independently on separate
load-path branches (lines ~868/989/1161 vs ~1176) — a body can `idt()` a signal with zero `ddt`
contributions and vice versa, so they are genuinely independent capabilities. Nesting `ac_idt`
inside `Reactive` would have made `has_reactive()` true whenever ac_idt alone was present,
changing which branches fire (a functional change) — ruled out under the zero-functional-change
invariant. The design doc's own "Refinement note" grants exactly this kind of adjustment latitude.

The `AnalogCapability` trait's method also deviates from design.md's sketch (`read_bounds() ->
ReadBounds`): no per-capability read-bounds tracking exists in the current code (`read_bounds` is
a whole-kernel triple on `AnalogCore`), so implementing that literally would have required
fabricating new data — a functional change. Implemented `fn count(&self) -> usize` instead (real,
derivable per capability, backs the `num_forces`/`num_noise`/`num_limits`/`num_ac_stims`
accessors) per the note's explicit license to adjust the trait shape without a redesign.

---

### T11: Split `device/analog.rs` core + introduce `Stamps` trait

**What**: Create `device/analog/mod.rs` holding `AnalogInstance` core
(terminals, `collect_volts`, `eval_rhs_jac`, `nodal_stamps`, and
`load_dc`/`load_ac`/`load_transient`/`noise_current_psd` folding per-capability
stamps); define the internal `Stamps` trait (not an `Element` facet — MD-01
flat). Capability extractions follow in T12/T13.
**Where**: `src/device/analog/mod.rs` (from `src/device/analog.rs`)
**Depends on**: T10
**Reuses**: existing `AnalogInstance` stamping logic
**Requirement**: CGA-09
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `device/analog/mod.rs` holds the core + `load_*` dispatch; `Stamps` trait defined internally (moved to T12's commit — see deviation note).
- [x] `Element` object still flat (`PiperineDevice` unchanged; no trait object across ABI) — untouched in this task.
- [x] Full gate passes (zero warnings, 99 test-result blocks, 0 failed).
- [x] Test count unchanged.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): promote device/analog to a module directory (CGA-09)` — `6311e31`
**Status**: ✅ Complete

Deviation: the `Stamps` trait definition moved from this commit into T12's, where its first real
implementor (`ForceStamper`) lands in the same commit. A private trait defined-but-unimplemented in
T11 alone would trip `dead_code` and break T11's own zero-warnings gate — every commit must be
independently warning-clean per the refactor invariant, so the trait and its first `impl` had to
land together. The locked principle (each capability owns its stamping via `Stamps`) is unaffected.

---

### T12: Extract `ForceStamper` + `Limiter` capabilities

**What**: Move `force_stamps`/`force_branch_target` → `device/analog/forces.rs`
(`ForceStamper`); move `limited_volts`/`update_limits`/`limiting_active` →
`device/analog/limits.rs` (`Limiter`). Each impls the capability seam
(`Stamps`/`VoltTransform` as fits — plain method if a trait would be contrived).
**Where**: `src/device/analog/forces.rs`, `src/device/analog/limits.rs` (new)
**Depends on**: T11
**Reuses**: the stamping/limiting methods verbatim
**Requirement**: CGA-09
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] Forces + limits each own their file + runtime state; `load_dc` folds their stamps.
- [x] Full gate passes (DC/tran/ngspice equivalence) — 99 test-result blocks, 0 failed, zero warnings.
- [x] Test count unchanged.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): extract ForceStamper + Limiter capabilities (CGA-09)` — `fa8dcea`
**Status**: ✅ Complete

Note: `Limiter::seed` is kept as a separate post-construction call (mirroring the original
two-step `Self::new(...)` then `instance.seed_limits()`) rather than folded into `Limiter::new`,
specifically to preserve `AnalogInstance::new`'s original call order — seeding ran after
`fire_initial_events()`, and combining the steps would have silently reordered `$limit` seeding
ahead of `@initial` event actions.

---

### T13: Extract `Operator` + `EventDetector` capabilities

**What**: Move `Operator` (delay/slew/transition, `accept`, `pending_edges`) →
`device/analog/operators.rs`; move `EventDetector` + `apply_event_actions` +
`fire_initial_events` → `device/analog/events.rs`.
**Where**: `src/device/analog/operators.rs`, `src/device/analog/events.rs` (new)
**Depends on**: T11
**Reuses**: the operator/event structs verbatim
**Requirement**: CGA-09
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] Operators + events each in their own file; `AnalogInstance` field count materially reduced.
- [x] Full gate passes — `cargo build --workspace` zero warnings, `cargo test --workspace` all green.
- [x] Test count unchanged — 582 passed, 0 failed (96 test binaries, 0 failures).
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): extract Operator + EventDetector capabilities (CGA-09)` — `2c666f0`
**Status**: ✅ Complete

---

### T14: Split `device/circuit.rs` — `CircuitCompiler` vs `InstanceBuilder`

**What**: Keep `CircuitCompiler` (public: `compiled`, `build_circuit*`, cache)
in `device/circuit.rs`; move `InstanceBuilder` (`add_instance`,
`resolve_connections`, `resolve_overrides`, `node_identifier`, `finish`) →
`device/builder.rs`.
**Where**: `src/device/circuit.rs`, `src/device/builder.rs` (new)
**Depends on**: T2
**Reuses**: existing assembly logic verbatim
**Requirement**: CGA-10
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `InstanceBuilder` in `device/builder.rs`; `circuit.rs` holds only the compiler API.
- [x] Full gate passes.
- [x] Test count unchanged — 582 passed, 0 failed.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): split InstanceBuilder out of circuit.rs (CGA-10)` — `537e2d9`
**Status**: ✅ Complete

---

### T15: Extract `device/fusion.rs`

**What**: Move `FusionCandidate` + `fuse_comb_cones` → `device/fusion.rs`.
**Where**: `src/device/fusion.rs` (new), `src/device/builder.rs`
**Depends on**: T14
**Reuses**: fusion logic verbatim
**Requirement**: CGA-10
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] Digital-cone fusion isolated in `device/fusion.rs`.
- [x] `mixed_signal.rs`/`digital_topology.rs` pass unchanged.
- [x] Full gate passes.
- [x] Test count unchanged — 582 passed, 0 failed.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): extract comb-cone fusion to device/fusion.rs (CGA-10)` — `35f83e0`
**Status**: ✅ Complete

---

### T16: Extract `device/plugin.rs` (fold `provider.rs`)

**What**: Move `add_plugin_instance` + `provider.rs` contents (`DeviceProvider`,
`PluginDeviceSpec`, `PluginPort`, `PortBinding`) → `device/plugin.rs`; delete
`provider.rs`.
**Where**: `src/device/plugin.rs` (new), `src/device/provider.rs` (removed), `src/device/builder.rs`
**Depends on**: T14
**Reuses**: plugin instantiation + provider types verbatim
**Requirement**: CGA-10
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] Plugin concern lives in `device/plugin.rs`; `provider.rs` gone.
- [x] Plugin tests (`e2e.rs`, `native_smoke.rs`) pass unchanged (part of the 582/0 full-workspace run).
- [x] Full gate passes — zero warnings, `cargo test --workspace` all green.
- [x] Test count unchanged — 582 passed, 0 failed (96 binaries).
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): consolidate plugin assembly in device/plugin.rs (CGA-10)` — `d70b363`
**Status**: ✅ Complete

---

### T17: Tidy `lib.rs` single façade + visibility sweep

**What**: Rewrite `lib.rs` module docs + a single tidy re-export façade of the
host-facing set; sweep every moved/split item so nothing is more public than
needed (default `pub(crate)`/private; re-export only through `lib.rs`).
**Where**: `src/lib.rs`, all changed modules (visibility)
**Depends on**: T6, T10, T13, T16
**Reuses**: —
**Requirement**: CGA-01, CGA-03
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `lib.rs` names pipeline-stage modules + one façade block; crate-level doc updated.
- [x] No item wider than required — `emit`/`flatten`/`error` narrowed to crate-private (grep-verified zero external deep-path usage); `device`/`kernel`/`resolve` stay `pub mod` (grep-verified deep external usage: `kernel::digital::network`, `resolve::pom`, `device::CircuitCompiler`, wildcard `resolve::*` imports, etc). `cargo build --workspace` zero warnings.
- [x] Full gate passes.
- [x] Test count unchanged — 582 passed, 0 failed.
**Tests**: regression · **Gate**: full
**Commit**: `refactor(codegen): single lib.rs façade + visibility sweep (CGA-01,03)` — `a25988b`
**Status**: ✅ Complete

---

### T18: Docs — CLAUDE.md paths, `AD-NNN`, ROADMAP

**What**: Update CLAUDE.md's crate-responsibility/pipeline references to the new
module names; append an `AD-NNN` to `.specs/STATE.md` recording the
**pipeline-stage module convention** (only); note the refactor in ROADMAP if
tracked.
**Where**: `CLAUDE.md`, `.specs/STATE.md`, `ROADMAP.md`
**Depends on**: T17
**Reuses**: —
**Requirement**: CGA-03
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] CLAUDE.md references `resolve`/`flatten`/`emit`/`kernel`/`device`; no stale `lower`/`jit`/inner-`codegen` (grep-verified clean, incl. the `ir::lower_bodies` pipeline-diagram reference).
- [x] Project decision appended to STATE.md — **MD-23** (project's actual numbering convention is `MD-NNN` under "Macro Decisions", not the generic `AD-NNN` template name; followed the codebase's real convention per the Knowledge Verification Chain).
- [x] Build gate passes — `cargo build --workspace` zero warnings (docs-only change).
- [x] ROADMAP.md: no existing reference to old codegen internals needed correction; refactor wasn't previously tracked there, so no edit required ("if tracked" — it wasn't).
**Tests**: none · **Gate**: build
**Commit**: `docs: codegen pipeline-stage module convention (MD-23) + paths` — `7c90684`
**Status**: ✅ Complete

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7

Phase 1:  T1 ──→ T2
Phase 2:  T3 ──→ T4 ──→ T5 ──→ T6
Phase 3:  T7 ──→ T8 ──→ T9
Phase 4:  T10
Phase 5:  T11 ──→ T12 ──→ T13
Phase 6:  T14 ──→ T15 ──→ T16
Phase 7:  T17 ──→ T18
```

Batch packing (~7 tasks/worker, whole phases): **Batch 1** = P1+P2 (6) ·
**Batch 2** = P3+P4+P5 (7) · **Batch 3** = P6+P7 (5). → 3 workers if sub-agents
accepted.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1 error.rs | 1 type relocation | ✅ Granular |
| T2 lower→resolve | 1 rename + call sites | ✅ Granular |
| T3 emit module + SimCtx/Codegen | 1 rename + 2 item moves (cohesive) | ✅ Granular |
| T4 split builder | 1 file → 3 (cohesive) | ✅ Granular |
| T5 emit/stmt | 1 extraction | ✅ Granular |
| T6 split emit_analog | 1 function decomposition | ✅ Granular |
| T7 flatten stage | 1 file move | ✅ Granular |
| T8 digital→kernel | 1 dir move + delete jit | ✅ Granular |
| T9 AnalogKernel relocate | 1 move + compile split | ✅ Granular |
| T10 kernel capability structs | 1 struct decomposition | ✅ Granular |
| T11 device/analog core + Stamps | 1 core split + trait | ✅ Granular |
| T12 ForceStamper+Limiter | 2 cohesive capabilities | ✅ Granular |
| T13 Operator+EventDetector | 2 cohesive capabilities | ✅ Granular |
| T14 circuit vs builder | 1 split | ✅ Granular |
| T15 fusion | 1 extraction | ✅ Granular |
| T16 plugin | 1 extraction + fold provider | ✅ Granular |
| T17 façade + visibility | 1 lib.rs + sweep | ✅ Granular |
| T18 docs | docs | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
| ---- | ----------------- | ------------- | ------ |
| T1 | None | phase start | ✅ Match |
| T2 | T1 | T1→T2 | ✅ Match |
| T3 | T2 | T2→T3 | ✅ Match |
| T4 | T3 | T3→T4 | ✅ Match |
| T5 | T4 | T4→T5 | ✅ Match |
| T6 | T5 | T5→T6 | ✅ Match |
| T7 | T3 | (P2→P3 boundary; T3 prior phase) | ✅ Match |
| T8 | T3, T7 | T7→T8 (+T3 prior) | ✅ Match |
| T9 | T8 | T8→T9 | ✅ Match |
| T10 | T9 | T9→T10 | ✅ Match |
| T11 | T10 | T10→T11 | ✅ Match |
| T12 | T11 | T11→T12 | ✅ Match |
| T13 | T11 | T11→T13 (parallel-capable, runs after T12 in order) | ✅ Match |
| T14 | T2 | (P6; T2 prior phase) | ✅ Match |
| T15 | T14 | T14→T15 | ✅ Match |
| T16 | T14 | T14→T16 (runs after T15 in order) | ✅ Match |
| T17 | T6, T10, T13, T16 | converges from each phase tail | ✅ Match |
| T18 | T17 | T17→T18 | ✅ Match |

All `Depends on` point backward or within-phase. ✅

---

## Test Co-location Validation

| Task | Code Layer Modified | Matrix Requires | Task Says | Status |
| ---- | ------------------- | --------------- | --------- | ------ |
| T1–T17 | codegen kernel/emit/resolve/flatten/device | regression (existing suite unchanged) | regression | ✅ OK |
| T18 | docs | none | none | ✅ OK |

**Note (refactor):** the coverage expectation is the **existing** suite passing
unchanged — writing new tests to mirror moved code is forbidden (the skill's
"tests never mirror implementation" rule). `Tests: regression` = run the
existing suite as the gate; `Tests: none` (T18) matches the docs layer's
"none". No ❌.

---

## Requirement → Task Map

| Requirement | Tasks |
| ----------- | ----- |
| CGA-01 (pipeline module tree) | T2, T3, T7, T8, T9, T17 |
| CGA-02 (drop `ir` alias) | T2 |
| CGA-03 (zero-warning, suite-green, docs) | T17, T18 (invariant on all) |
| CGA-04 (`AnalogKernel` capability sub-structs) | T10 |
| CGA-05 (split `emit_analog`/`builder.rs`) | T4, T5, T6 |
| CGA-06 (every helper owned) | T4 (+ invariant on all) |
| CGA-07 (`CodegenError` home + messages) | T1 |
| CGA-08 (`SimCtx` home) | T3 |
| CGA-09 (`device/analog` capability split + `Stamps`) | T11, T12, T13 |
| CGA-10 (`device/circuit` responsibility split) | T14, T15, T16 |

**Coverage**: 10 requirements, all mapped across 18 tasks.

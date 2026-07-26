# Hierarchy Flattening Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Spec**: `.specs/features/hierarchy-flattening/spec.md`
**Design**: `.specs/features/hierarchy-flattening/design.md`
**Status**: Tasks drafted — not started.
**Scope**: MVP = gap 1 (flatten pass). Gaps 2 (const-arg-into-behavior) and 3
(array-net expansion) are fail-loud, not built.
**LOCKED invariant**: non-destructive — `Design::modules` never mutated (see
CLAUDE.md "UNBREAKABLE RULE"). Every task that touches the pass carries a
`modules`-untouched assertion.

---

## Test Coverage Matrix

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Elaboration pass (`FlattenHierarchy`, driver, `inline`, remap) | unit | Every AC + every fail-loud edge (cycle, dangling net, array-net); **non-destructive assertion** (`modules` deep-equal before/after) | `crates/piperine-lang/src/elab/lower/` `#[cfg(test)]` + `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| `Design` model (`flat_modules`, `flat_module`, serde skip) | unit | Accessor fallback; serde round-trips authored hierarchy only | `crates/piperine-lang/src/pom/design.rs` inline + `tests/pom_serde.rs` | `cargo test -p piperine-lang pom_serde` |
| Codegen root indirection | unit | A mid-level module builds; `circuit.rs:389` unreachable for valid input | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Host override retarget | integration | 2-level designs unchanged; flat-label override works | root `tests/compile_once_sweep.rs`, `tests/session.rs` | `cargo test -p piperine` |
| ngspice cross-check (`urc`) | integration | `urc` matches ngspice at ≥3 `lump` values | root `tests/ngspice_validation.rs` (+`tests/ngspice/`) | `cargo test -p piperine ngspice` |
| Monomorph regression guard | integration | `compile_count` deltas per shape; zero recompile on non-structural sweep | root `tests/compile_once_sweep.rs` | `cargo test -p piperine compile_once` |
| Docs (ROADMAP/STATE) | none | build gate only | `ROADMAP.md`, `.specs/STATE.md` | build gate |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task touching one crate | `cargo test -p <crate>` |
| Full | After a task crossing crates (codegen wiring, overrides, urc) | `cargo test --workspace` |
| Build | After phase completion / docs-only tasks | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

---

## Execution Plan

Phases ordered, run sequentially; tasks within a phase run in order.

- **Phase 1** — non-destructive flatten infrastructure (the pass, in isolation).
- **Phase 2** — codegen + host consume the flat form.
- **Phase 3** — `urc` end-to-end proof + monomorph regression guards.
- **Phase 4** — docs.

---

## Task Breakdown

### T1: `Design::flat_modules` side map + accessor

**What**: Add the `flat_modules: HashMap<String, Module>` field, the
`flat_module(name)` accessor (fallback to `modules`), and `#[serde(skip)]`.
**Where**: `crates/piperine-lang/src/pom/design.rs`
**Depends on**: None
**Reuses**: existing `modules` map shape; `behaviors` `#[serde(skip)]` precedent
**Requirement**: FLAT-01 (infra)
**Done when**:
- `flat_module(name)` returns `flat_modules[name]` else `modules[name]`.
- `pom_serde` round-trip carries authored hierarchy only (flat map skipped).
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): Design::flat_modules side map + flat_module accessor`

---

### T2: `FlattenHierarchy` pass — driver + leaf test + cycle guard (non-destructive)

**What**: The memoized bottom-up driver: `is_leaf`, per-module clone-and-splice,
`in_progress` cycle guard; writes **only** `flat_modules`. Inlining body stubbed
to T3 (a non-leaf child with no ports/wires is enough to exercise the driver).
**Where**: `crates/piperine-lang/src/elab/lower/` (new `flatten.rs`)
**Depends on**: T1
**Reuses**: `Module`/`Instance` POM; `mono_cache`/`RefCell` memo precedent
**Requirement**: FLAT-01
**Done when**:
- Flat form of a leaf-only module equals its authored form.
- A module instantiating a non-leaf child yields a flat form with leaf
  instances only.
- **Non-destructive assertion**: `Design::modules` deep-equal before/after.
- Recursive instantiation fails loud naming the cycle.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): FlattenHierarchy driver + cycle guard (FLAT-01)`

---

### T3: `inline()` net remapping — rename map, path-prefixed labels, fail-loud

**What**: Build `ρ` (child port → parent NetRef; child wire → fresh
`inst.name().w`), splice sub-instances/connections through `remap`; path-prefixed
labels; fail loud on a dangling net and on an indexed NetRef (gap-3 boundary).
**Where**: `crates/piperine-lang/src/elab/lower/flatten.rs`
**Depends on**: T2
**Reuses**: `NetRef`, `is_ground`
**Requirement**: FLAT-02, FLAT-03
**Done when**:
- Child port binds to the parent net the instance connected; child wire lifts to
  `x.w`.
- Two instances of the same child produce collision-free `x.*` / `y.*` labels;
  nesting composes (`x.seg0.rc`).
- Dangling net (neither port nor wire) fails loud naming net + child.
- Indexed `NetRef` (`node[i]`) fails loud as gap-3 deferred.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): flatten inline net remapping + fail-loud guards (FLAT-02,03)`

---

### T4: Wire `FlattenHierarchy` into `PASSES`

**What**: Append `&FlattenHierarchy` after `&Typecheck` in `PASSES`.
**Where**: `crates/piperine-lang/src/elab/lower/passes.rs`
**Depends on**: T3
**Reuses**: `ElabPass` trait
**Requirement**: FLAT-01
**Done when**:
- After `parse_and_elaborate`, `flat_modules` is populated for every module.
- Existing `parse_elab`/`elab` suites stay green (additive pass).
**Tests**: unit · **Gate**: full
**Commit**: `feat(lang): run FlattenHierarchy pass after Typecheck (FLAT-01)`

---

### T5: Codegen reads the root from `flat_module`

**What**: `InstanceBuilder` root (and root `module`/`compiled` lookups) read
`design.flat_module(root)`; leaf children stay from `modules`. Keep
`circuit.rs:389` as a defensive assertion.
**Where**: `crates/piperine-codegen/src/device/circuit.rs`
**Depends on**: T4
**Reuses**: existing build path (unchanged below the root lookup)
**Requirement**: FLAT-01
**Done when**:
- A mid-level module (instantiates a sub-module) builds and simulates.
- `circuit.rs:389` "nested hierarchy" error is unreachable for valid input
  (a test that previously errored now builds).
**Tests**: unit · **Gate**: full
**Commit**: `feat(codegen): consume flat_module(root); mid-level modules build (FLAT-01)`

---

### T6: `with_overrides_applied` retargets to `flat_modules[root]`

**What**: Patch the flat form for host overrides (flat-label host contract).
**Where**: `crates/piperine-lang/src/pom/design.rs`
**Depends on**: T5
**Reuses**: existing override loop; flat-string label match
**Requirement**: FLAT-03
**Done when**:
- 2-level designs: `flat_module(root) == authored` → existing
  `compile_once_sweep`/`session` overrides unchanged (no regression).
- An override on a flattened instance label (`x.seg0`) resolves and restamps.
**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): overrides target the flat netlist (FLAT-03)`

---

### T7: Author `urc.phdl` (pure-structural `mod urc[N]`)

**What**: `urc` as `mod urc[lump]` + `StructuralFor` over `lump` leaf RC
segments, series-connected via generated scalar wires (no array nets → avoids
gap 3; no `N` in a mid-level analog body → avoids gap 2).
**Where**: `crates/piperine-lang/headers/spice/tline.phdl` (or a new `urc.phdl`)
**Depends on**: T5
**Reuses**: existing `res`/`cap` leaf models; `StructuralFor`, `mod [N]` monomorph
**Requirement**: FLAT-04
**Done when**:
- `urc[N]` elaborates for N ∈ {2,5,10}; `flat_modules[urc__N]` has the expected
  leaf count; builds without hitting `circuit.rs:389`.
**Tests**: unit (elaborates + builds) · **Gate**: quick
**Commit**: `feat(spice): urc lumped RC line via generate + flatten (FLAT-04)`

---

### T8: ngspice cross-check `urc` at ≥3 `lump` values

**What**: `.op`/`.tran` cross-check vs ngspice's `urc` at lump = 2, 5, 10.
**Where**: root `tests/ngspice_validation.rs` (+ `tests/ngspice/urc_*.{cir,phdl}`)
**Depends on**: T7
**Reuses**: `ngspice_validation.rs` harness pattern
**Requirement**: FLAT-04
**Done when**:
- Each `lump` matches ngspice within the established tolerance.
**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): urc parity at lump 2/5/10 (FLAT-04)`

---

### T9: Monomorph regression guards (compile_count)

**What**: Assert `urc[3]` + `urc[7]` compile 2 distinct kernels; same shape
shares one; non-structural sweep adds zero compiles.
**Where**: root `tests/compile_once_sweep.rs`
**Depends on**: T7
**Reuses**: `AnalogKernel::compile_count` pattern
**Requirement**: FLAT-05, FLAT-06, FLAT-07
**Done when**:
- `compile_count` delta = 2 for `{urc[3], urc[7]}` build.
- Two `urc[3]` with different `r`/`c` → 1 shared kernel + restamp.
- 20-point non-structural sweep → delta stays constant (zero extra compiles).
**Tests**: integration · **Gate**: full
**Commit**: `test: monomorph per-shape kernel + restamp guards (FLAT-05,06,07)`

---

### T10: Docs — unblock `urc`, record the pass

**What**: ROADMAP `urc` → done/unblocked; `.specs/STATE.md` decision entry for
the non-destructive flatten + the LOCKED POM-navigability rule.
**Where**: `ROADMAP.md`, `.specs/STATE.md`
**Depends on**: T8, T9
**Reuses**: —
**Requirement**: FLAT-01..07 (closure)
**Done when**:
- ROADMAP `urc` row reflects shipped; STATE records the pass + rule as a
  numbered decision.
**Tests**: none · **Gate**: build
**Commit**: `docs: urc shipped; record non-destructive flatten + POM rule`

---

## Requirement → Task Map

| Requirement | Tasks |
| ----------- | ----- |
| FLAT-01 (flatten pass, flat netlist) | T1, T2, T4, T5 |
| FLAT-02 (net/port binding, remap, fail-loud) | T3 |
| FLAT-03 (collision-free labels, host contract) | T3, T6 |
| FLAT-04 (urc ngspice proof) | T7, T8 |
| FLAT-05 (distinct kernels per shape) | T9 |
| FLAT-06 (shared kernel restamp) | T9 |
| FLAT-07 (sweep, zero recompile) | T9 |

**Coverage**: 7 requirements, all mapped. Non-destructive invariant asserted in
T2 and carried through T4–T6.

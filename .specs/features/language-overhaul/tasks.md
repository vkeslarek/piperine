# language-overhaul Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/language-overhaul/design.md` (Approved)
**Spec**: `.specs/features/language-overhaul/spec.md`
**Status**: Draft — awaiting approval

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec — confirm before Execute.
> **Guidelines found:** `CLAUDE.md` ("Build and test", "Where the tests are"), `AGENTS.md` (hard rules), `.specs/STATE.md` MD-13 (Rust idiom, no macros, no loose functions), MD-28 (test placement), MD-31 (invariants live in gates), MD-38 (parity measured), `tests/suite_hygiene.rs` (the enforcing guard: no switched-off tests, no `#[ignore]`, every target carries a `//!` scope header, no file-scope lint suppression, no dead-architecture identifiers).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| IR core / verifier / printer / diff / interpreter (`piperine-ir`) | unit inline + integration | All branches; 1:1 to spec ACs; every listed edge case has a test | `crates/piperine-ir/src/**` (`#[cfg(test)] mod tests`) + `crates/piperine-ir/tests/*.rs` | `cargo test -p piperine-ir` |
| Intrinsic catalog entries | unit inline | Every entry: signature, state shape, derivative rule, legal domains asserted | `crates/piperine-ir/src/catalog/**` | `cargo test -p piperine-ir` |
| Codegen lowering + backends | integration, grouped by concern | All branches; 1:1 to spec ACs; every named error path asserted | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Differential harness (interpreter ≡ JIT) | integration, cross-crate | Every IR fixture runs under both evaluators and must agree | root `tests/*.rs` | `cargo test --workspace` |
| Parity oracle (ngspice) | integration, cross-crate | **Unmodified.** Existing tolerance contracts; matrix registry unchanged (MD-38) | root `tests/ngspice_*.rs` | `cargo test --workspace` |
| Policy gates (concept budget, purity, no-catch-all) | integration, source-walking | Registry-driven, never a hardcoded name list (pattern: `tests/delegation_guard.rs`) | `crates/piperine-ir/tests/`, root `tests/` | `cargo test --workspace` |
| PHDL surface additions | integration + examples gate | Each construct: compiles, simulates, and appears in `examples/` where user-facing | `crates/piperine-lang/tests/`, `tests/run_examples.rs` | `cargo test --workspace` |

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | After tasks touching one crate only | `cargo test -p <crate>` |
| Full | After tasks crossing crates or touching the differential harness | `cargo test --workspace` |
| Build | After phase completion, or config/rename-only tasks | `cargo build --workspace` (**zero warnings is the bar**, CLAUDE.md) then `cargo test --workspace` |

**Note:** `cargo test` bare at the repo root only runs the root package — always
pass `--workspace` (CLAUDE.md).

---

## Execution Plan

Phases run sequentially; tasks within a phase run in order.

### Phase 1: IR core (the five concepts)
```
T1 → T2 → T3 → T4 → T5 → T6 → T7
```

### Phase 2: IR services (print, catalog, diff, interpret)
```
T8 → T9 → T10 → T11 → T12 → T13
```

### Phase 3: The oracle and the gates — P1 closes here
```
T14 → T15 → T16 → T17
```

### Phase 4: Backends over the IR
```
T18 → T19 → T20 → T21 → T22
```

### Phase 5: Completeness fixtures — P2 closes here
```
T23 → T24 → T25
```

### Phase 6: POM → IR lowering
```
T26 → T27 → T28 → T29 → T30 → T31
```

### Phase 7: Switchover and deletion — P3 closes here
```
T32 → T33 → T34 → T35
```

### Phase 8: PHDL surface — P4 closes here
```
T36 → T37 → T38 → T39 → T40 → T41 → T42
```

---

## Task Breakdown

### T1: Rename `piperine-signal` to `piperine-ir`

**What**: Mechanical crate rename across the workspace; purity guard inherited unchanged.
**Where**: `crates/piperine-signal/` → `crates/piperine-ir/`, `Cargo.toml` (workspace + dependents)
**Depends on**: None
**Reuses**: the whole crate, including `tests/purity.rs`
**Requirement**: LOV-09
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Crate directory, package name, and every dependent import updated
- [ ] `tests/purity.rs` passes unchanged (empty in-tree dependency list)
- [ ] `.specs/STATE.md` MD-40 referenced in the crate's `lib.rs` header
- [ ] Gate passes: `cargo build --workspace` at zero warnings, `cargo test --workspace`
- [ ] Test count: no change (rename only, no silent deletions)

**Tests**: none (rename; existing suite is the gate) · **Gate**: build
**Commit**: `refactor(ir): rename piperine-signal to piperine-ir (MD-40)`

---

### T2: IR value arena — `Ty`, `ValueId`, `Module` storage

**What**: Arena-backed value storage with distinct newtype ids and the two-class-plus-array type (`Real | Bits(u32) | Array(Box<Ty>, u32)`).
**Where**: `crates/piperine-ir/src/core/value.rs`, `core/mod.rs`
**Depends on**: T1
**Reuses**: interning idea from `piperine-codegen/src/resolve/symbols.rs`
**Requirement**: LOV-01
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Ty` has exactly three variants; no `Int`, no `Logic4`, no `Vec` type
- [ ] Ids are distinct newtypes; no `usize` in the public API
- [ ] Unit tests: type construction, array nesting, id distinctness
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): value arena and the two-class type`

---

### T3: IR nets — nature-generic `node` and `signal`

**What**: `NetKind { Node { across: NatureId, through: NatureId }, Signal { ty: Ty } }` plus the nature table. **No `V`/`I` anywhere.**
**Where**: `crates/piperine-ir/src/core/net.rs`
**Depends on**: T2
**Reuses**: `NatureId`/`NatureInfo`/`NatureKind` semantics from `resolve/symbols.rs`
**Requirement**: LOV-13, LOV-14
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A conservative net carries two named natures; the IR never names a volt
- [ ] `Signal` carries no conservation law and no KCL row
- [ ] Unit test: a thermal discipline (`temp`/`pwr`) and an electrical one construct identically
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): nature-generic nets`

---

### T4: IR cells — one declared writer, depth in the type

**What**: `Cell { ty, init, writer: BlockId, name }`. Writer is a **field**, so two writers is a construction error.
**Where**: `crates/piperine-ir/src/core/cell.rs`
**Depends on**: T2
**Reuses**: —
**Requirement**: LOV-03
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Cell` has no `depth` modifier — array depth lives in `Ty::Array`
- [ ] Unit tests: scalar cell = register, array cell = memory; writer required at construction
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): cells with a single declared writer`

---

### T5: IR blocks and ops — domain, trigger, the op set

**What**: `Block { domain, trigger, ops }` and the `Op` enum including the six net accesses, cell read/write, structured `If`/`Match`.
**Where**: `crates/piperine-ir/src/core/block.rs`, `core/op.rs`
**Depends on**: T3, T4
**Reuses**: trigger kinds from `resolve/stmt.rs` (`AnalogEvent`, `DigitalEvent`, `EdgeKind`, `CrossDir`)
**Requirement**: LOV-01, LOV-07
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Six distinct net-access ops exist; `read_across` and `read_through` are separate variants
- [ ] `If`/`Match` carry inline op lists — control stays structured, no flattening
- [ ] Trigger covers `Always`, `Edge`, `Cross`, `Timer`, `Initial`
- [ ] Op order within a block is explicit and preserved
- [ ] Unit tests: block construction per domain; ordering preserved
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): blocks, triggers, and the op set`

---

### T6: Signal references as chunk lists

**What**: `SigRef(Vec<Chunk>)`, `Chunk = Slice { value, lo, hi } | Const(ConstId)`. Slice and concat are representation, **not ops**.
**Where**: `crates/piperine-ir/src/core/sigref.rs`
**Depends on**: T2
**Reuses**: RTLIL `SigSpec` model (`ideal.md` §3)
**Requirement**: LOV-27
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No `slice`/`concat`/`extract`/`insert` op exists in the `Op` enum
- [ ] Unit tests: a 32-bit ref built from three chunks reports width 32; constants embed
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): chunk-list signal references`

---

### T7: IR verifier

**What**: Reject malformed IR: writer count ≠ 1, width mismatch, chunk out of bounds, nature access on a `Signal`, undeclared block output.
**Where**: `crates/piperine-ir/src/verify.rs`
**Depends on**: T5, T6
**Reuses**: —
**Requirement**: LOV-03, LOV-04, LOV-14
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Each check has a named error naming the offending entity
- [ ] Integration tests: one rejection fixture per check, each asserting the error names the entity
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit + integration · **Gate**: quick
**Commit**: `feat(ir): verifier`

---

### T8: IR printer — stable and diffable

**What**: Textual output, deterministic ordering, `\user` vs `$generated` name provenance.
**Where**: `crates/piperine-ir/src/print.rs`
**Depends on**: T7
**Reuses**: RTLIL identifier convention (`ideal.md` §3)
**Requirement**: LOV-02, LOV-06, LOV-08
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Printing the same module twice is byte-identical
- [ ] Generated names are visually distinct from authored ones
- [ ] Integration test: repeat-print equality on a non-trivial module
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(ir): stable textual printer`

---

### T9: IR parser — round-trip

**What**: Parse the printed form back into an equivalent module, so hand-written IR fixtures need no front-end.
**Where**: `crates/piperine-ir/src/parse.rs`
**Depends on**: T8
**Reuses**: —
**Requirement**: LOV-02
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `parse(print(m))` is structurally equal to `m` for every fixture
- [ ] A malformed text input is a named error, never a panic
- [ ] Integration tests: round-trip property over ≥5 fixtures + malformed-input cases
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(ir): textual parser with round-trip`

---

### T10: Intrinsic catalog — trait and data table

**What**: `Intrinsic` trait (signature, state shape, derivative rule, breakpoints, legal domains) plus the entry table. Table + trait, **never a macro** (MD-13 rule 5).
**Where**: `crates/piperine-ir/src/catalog/mod.rs`
**Depends on**: T5
**Reuses**: existing `piperine-ir` (ex-signal) families
**Requirement**: LOV-12
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Trait defined; no `macro_rules!` anywhere in the crate
- [ ] Unit tests: every registered entry answers all five questions
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): intrinsic catalog contract`

---

### T11: Migrate the existing signal families into catalog entries

**What**: Runtime operators, event detectors, junction limiter, `table`, `Behavior`/`Measure`/`Wave`, digital edge logic become catalog entries in place.
**Where**: `crates/piperine-ir/src/catalog/`, existing family modules
**Depends on**: T10
**Reuses**: all of it — **move, do not rewrite** (their rules were learned from failures, `ideal.md` §3 / MD-36 lineage)
**Requirement**: LOV-12
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every family reachable through the catalog; existing family tests pass unchanged
- [ ] The four undeclared operators (`idtmod`, `absdelay`, `table`, `ac_stim`) have entries, closing `GAPS.md` B6
- [ ] `LaplaceKind`/`ZKind` confirmed dead and deleted, or given entries (MD-35 forbids dead identifiers)
- [ ] Gate passes: `cargo test -p piperine-ir`
- [ ] Test count: ≥ pre-migration count (no silent deletions)

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): runtime semantics become catalog entries`

---

### T12: Port symbolic differentiation to an IR pass

**What**: `d(value, wrt) -> ValueId` over structured control — `If`/`Match` differentiate arm-wise.
**Where**: `crates/piperine-ir/src/diff.rs`
**Depends on**: T10
**Reuses**: `piperine-codegen/src/resolve/diff.rs` — **port, do not rewrite** (design §7 risk)
**Requirement**: LOV-15
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every rule in the source `diff.rs` has a corresponding IR test
- [ ] Structured control differentiates arm-wise; a nested `If` is covered
- [ ] Derivatives cross-checked against finite differences (reuse the `verify_derivatives` FD oracle already in the crate)
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(ir): symbolic differentiation as an IR pass`

---

### T13: Reference interpreter

**What**: Direct IR walker — evaluates blocks, advances cells, honours triggers. Slow and obvious by design.
**Where**: `crates/piperine-ir/src/interp.rs`
**Depends on**: T11, T12
**Reuses**: catalog entries for intrinsic evaluation
**Requirement**: LOV-10
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Evaluates all op variants; a cell can be inspected after each step
- [ ] Structured `If`/`Match` honoured; triggers fire at declared instants
- [ ] Integration tests: hand-written modules with hand-computed expected values
- [ ] Gate passes: `cargo test -p piperine-ir`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(ir): reference interpreter`

---

### T14: Differential harness — interpreter vs evaluator

**What**: Harness that runs one IR fixture under two evaluators and asserts agreement within a declared tolerance.
**Where**: `crates/piperine-ir/tests/differential.rs` + shared support module
**Depends on**: T13
**Reuses**: pattern from `tests/support/ngspice.rs` (tolerance contract in one place)
**Requirement**: LOV-11
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Harness takes a fixture + two evaluators and reports the first disagreeing output/cell by name
- [x] Tolerance is declared per fixture in exactly one place
- [x] Gate passes: `cargo test -p piperine-ir`

**Tests**: integration · **Gate**: quick
**Commit**: `test(ir): differential harness`

**Note (batch execution, Phase 3):** no JIT backend exists yet at this point
in the plan (Phase 4/T18+ builds it) — spec AC6's literal "interpreter ≡
JIT" claim cannot be earned here. The harness is generic (`Evaluator`
trait + `compare`), proven against TWO evaluators: the reference
interpreter (T13) and a second, INDEPENDENTLY-WRITTEN `CheckedEvaluator`
(dense-array store, per-op-category dispatch, written from the IR's own
docs, kept in `tests/` not `src/` so it is never mistaken for a shipped
backend). See `tests/support/harness.rs`'s module doc for the full
reasoning. T18 rewires this same `compare` function against the real
analog backend to close the literal AC6 claim. Extending Bits arithmetic
in `interp.rs::eval_pure` (previously Real-only) was required to make the
digital counter fixture evaluable and is folded into this commit.

---

### T15: First fixtures — RC divider and 4-bit counter

**What**: Two hand-written IR modules exercising the analog and digital paths, run under the interpreter with golden values.
**Where**: `crates/piperine-ir/tests/fixtures/`
**Depends on**: T14
**Reuses**: T9's parser so fixtures are text
**Requirement**: LOV-02, LOV-11
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] RC fixture: hand-computed DC point matched
- [x] Counter fixture: hand-computed sequence over ≥8 cycles matched
- [x] Both round-trip through print/parse
- [x] Gate passes: `cargo test -p piperine-ir`

**Tests**: integration · **Gate**: quick
**Commit**: `test(ir): RC and counter fixtures`

**Note (batch execution, Phase 3):** the RC fixture is a purely resistive
divider (no `ddt`/reactive companion) — T13's interpreter is handed node
values externally rather than solving KCL, so the fixture supplies an
already-balanced operating point and checks the Ohm's-law contributions
against a hand computation; reactive dynamics land with the analog
backend (T20). The counter is 4-bit, driven 20 rising edges (> the ≥8
minimum), checked against `k % 16` after every edge, not just the final
value.

---

### T16: Rejection fixtures for the four seam defects

**What**: One fixture per historical defect proving the construct is now rejected loudly.
**Where**: `crates/piperine-ir/tests/seam_defects.rs`
**Depends on**: T15
**Reuses**: T7's verifier
**Requirement**: LOV-03, LOV-05, LOV-13, LOV-14
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] **A0d**: a cell written by two blocks is rejected, naming both
- [x] **A0b**: a `Signal` handed to analog stamping is rejected before any matrix
- [x] **A0c**: a write with no matching net-access op fails by name (no catch-all path exists to fall into)
- [x] **A0**: `read_across` and `read_through` on one node pair yield **different** values — asserted against a hand-computed current and voltage
- [x] Gate passes: `cargo test -p piperine-ir`

**Tests**: integration · **Gate**: quick
**Commit**: `test(ir): the four seam defects are unrepresentable`

**Note (batch execution, Phase 3):** A0d's fixture required a real
verifier extension — `Cell.writer` is a single FIELD (T4), so a SECOND
*claimed* writer cannot be constructed at all, but nothing before this
task stopped a rogue `CellWrite` from appearing in a block other than the
declared writer. Added `VerifyError::CellWrittenByWrongBlock` +
`Verifier::check_op`'s `CellWrite` arm now compares `site.block` against
`cell.writer`, naming both blocks (`crates/piperine-ir/src/verify.rs`).
Verified this doesn't regress any existing CellWrite fixture (full
`cargo test -p piperine-ir` stayed green throughout). A0c's fixture
targets the wrong-domain-op rejection (`DomainOpMismatch`) as the IR-level
analogue of the historical ground-to-ground fallthrough — PHDL-level
lowering with an actual `parse_dest`-shaped bug doesn't exist until
Phase 6.

---

### T17: Policy gates — concept budget and no-catch-all

**What**: Source-walking guards, registry-driven, no hardcoded name lists.
**Where**: `crates/piperine-ir/tests/policy_gates.rs`
**Depends on**: T16
**Reuses**: `tests/delegation_guard.rs` as the pattern (MD-31)
**Requirement**: LOV-01, LOV-05
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Concept-budget guard fails if a sixth top-level concept appears in `core/`
- [ ] No-catch-all guard fails on a `_ =>` arm in any match over `Op` or net kind
- [ ] Each guard proven able to fail (inject, observe red, revert — MD-31)
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(ir): concept-budget and no-catch-all gates`

---

### T18: Analog backend — residual emission

**What**: IR `region analog` → residual function via Cranelift.
**Where**: `crates/piperine-codegen/src/backend/analog/mod.rs`
**Depends on**: T17
**Reuses**: `emit/` (`Builder`, CSE, `SimCtx`) unchanged
**Requirement**: LOV-16
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `contribute_through`/`force_across` produce residual rows; a force adds a branch unknown
- [ ] Structured control emits branches (no flattening)
- [ ] Differential test: RC fixture agrees with the interpreter
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): analog backend over the IR — residual`

---

### T19: Analog backend — Jacobian via the diff pass

**What**: Jacobian emitted from T12's pass rather than a bespoke path.
**Where**: `crates/piperine-codegen/src/backend/analog/jacobian.rs`
**Depends on**: T18
**Reuses**: `piperine-ir::diff`
**Requirement**: LOV-15, LOV-16
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Jacobian derived once from the IR pass; no second differentiation implementation exists
- [ ] Differential + finite-difference cross-check on a nonlinear fixture
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): Jacobian from the IR diff pass`

---

### T20: Analog backend — reactive state and operators

**What**: `ddt` companion model, operator state slots, `ac_stim`, noise.
**Where**: `crates/piperine-codegen/src/backend/analog/reactive.rs`
**Depends on**: T19
**Reuses**: `IntegrationMethod::coeffs`, existing companion logic
**Requirement**: LOV-16
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `ddt` produces `Q(V)` + companion; operator state advances on accept
- [ ] Differential test on a reactive fixture
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): reactive state and operators over the IR`

---

### T21: Digital backend — combinational and clocked

**What**: IR `region digital` → comb + seq functions.
**Where**: `crates/piperine-codegen/src/backend/digital/mod.rs`
**Depends on**: T20
**Reuses**: `emit/` machinery
**Requirement**: LOV-17
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Always` and `Edge` triggers produce the two recipes
- [ ] Cell writes respect the single declared writer
- [ ] Differential test: counter fixture agrees with the interpreter
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): digital backend over the IR`

---

### T22: Digital backend — memories and width-native ops

**What**: Array cells lower to real memories; a 32-bit add is one op.
**Where**: `crates/piperine-codegen/src/backend/digital/memory.rs`
**Depends on**: T21
**Reuses**: —
**Requirement**: LOV-18, LOV-19, LOV-26
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Memory access ops distinct from declaration; port count inspectable pre-lowering
- [ ] A `bits<32>` add emits one machine-level operation sequence, not 32
- [ ] Read/write to the same address in one evaluation has explicitly defined ordering
- [ ] Differential test on a memory fixture
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): memories and width-native digital ops`

---

### T23: Opamp completeness fixture

**What**: Hand-written IR opamp — nonlinear, reactive, differentiable.
**Where**: `crates/piperine-ir/tests/fixtures/opamp.ir` + test
**Depends on**: T22
**Reuses**: T14 harness
**Requirement**: LOV-16
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Residual + Jacobian produced; DC solution matches a hand-computed operating point
- [ ] Agrees under both evaluators
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(ir): opamp completeness fixture`

---

### T24: Buck completeness fixture

**What**: Hand-written IR buck — analog and digital blocks sampling each other, `timer` and `cross` triggers.
**Where**: `crates/piperine-ir/tests/fixtures/buck.ir` + test
**Depends on**: T23
**Reuses**: T14 harness; the real design in `~/Git/lab-bench-ps` as the reference behaviour
**Requirement**: LOV-17
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] One module holds analog and digital blocks with explicit cross-sampling
- [ ] `timer` and `cross` fire at declared instants; breakpoints declared
- [ ] **A module with both domains sharing a module-level cell is exercised** (the A0d shape, which had zero coverage)
- [ ] Agrees under both evaluators
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(ir): buck completeness fixture`

---

### T25: RV32I completeness fixture

**What**: Hand-written IR core — register file, width-native ALU, indexed writes.
**Where**: `crates/piperine-ir/tests/fixtures/rv32i.ir` + test
**Depends on**: T24
**Reuses**: T14 harness
**Requirement**: LOV-18
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `[bits<32>; 32]` register file read and written by address
- [ ] A short instruction sequence executes to a hand-computed final state
- [ ] Agrees under both evaluators
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(ir): RV32I completeness fixture`

---

### T26: `lower` module skeleton — POM structure to IR

**What**: New `lower` stage: modules, ports, nets, params-as-cells.
**Where**: `crates/piperine-codegen/src/lower/mod.rs`, `structure.rs`
**Depends on**: T25
**Reuses**: `resolve/pom/structure.rs` walk logic
**Requirement**: LOV-01, LOV-24
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A trivial PHDL module lowers to a verifiable IR module
- [ ] Params lower to host-written cells (MD-18 restamp preserved)
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): lower stage — POM structure to IR`

---

### T27: Bundle expansion ahead of the IR

**What**: Expand bundle-typed values before the IR; **delete** the silent `ValueType::Bundle(_) => Type::Real`.
**Where**: `crates/piperine-codegen/src/lower/bundles.rs`
**Depends on**: T26
**Reuses**: `fn_bundle_sigs`, `bundle_bindings`, `lower_bundle_arg`
**Requirement**: LOV-22
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A bundle that cannot be expanded is a named error identifying the bundle and site
- [ ] The silent coercion is gone from the tree (grep-verified)
- [ ] Integration test asserting the named error
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(codegen): bundle expansion before the IR, no silent coercion`

---

### T28: Latch inference emits an explicit cell + `InferredLatch`

**What**: Inference becomes a front-end pass producing a declared cell, and raises the diagnostic Part I §10.3/§16 specifies and nothing implements.
**Where**: `crates/piperine-codegen/src/lower/inference.rs`
**Depends on**: T27
**Reuses**: current latch logic in `kernel/digital/layout.rs`
**Requirement**: LOV-23
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Inferred state appears as a declared cell in the IR — the IR never infers
- [ ] `InferredLatch` raised with the var name (closes `GAPS.md` C2)
- [ ] Integration tests: inferred-latch case + intended-register case
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): latch inference emits a declared cell and raises InferredLatch`

---

### T29: Lower analog bodies

**What**: PHDL analog statements → IR ops, structured control preserved.
**Where**: `crates/piperine-codegen/src/lower/analog.rs`
**Depends on**: T28
**Reuses**: `resolve/pom/expr.rs` resolution; **note M1** — both bodies carry raw AST today, so this is new work, not a port
**Requirement**: LOV-05, LOV-13
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Contributions and forces lower to the correct nature-generic ops
- [ ] **No catch-all arm**: an unlowerable destination is a named error (the A0c mechanism)
- [ ] Integration tests: every named error path asserted
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): lower analog bodies to IR`

---

### T30: Lower digital bodies

**What**: PHDL digital statements → IR ops.
**Where**: `crates/piperine-codegen/src/lower/digital.rs`
**Depends on**: T29
**Reuses**: —
**Requirement**: LOV-05
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Comb and clocked blocks lower with correct triggers
- [ ] A module with both bodies produces two blocks sharing no undeclared state
- [ ] Integration test for the A0d shape at PHDL level
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): lower digital bodies to IR`

---

### T31: Internal-unknown budget counted from the IR

**What**: Derive the unknown count from IR forces; `allocate_unknowns` becomes mechanical.
**Where**: `crates/piperine-codegen/src/lower/unknowns.rs`, `device/`
**Depends on**: T30
**Reuses**: `Element::allocate_unknowns` seam (unchanged ABI)
**Requirement**: LOV-25
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Count derived from the IR matches what the device allocates, asserted
- [ ] A device allocating without declaring `HAS_INTERNAL_UNKNOWNS` (or the reverse) is caught
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): unknown budget derived from the IR`

---

### T32: Wire `CircuitCompiler` to IR-built kernels

**What**: Switch the device path onto the IR backends. `Element` ABI untouched.
**Where**: `crates/piperine-codegen/src/device/`
**Depends on**: T31
**Reuses**: `PiperineDevice`, `CircuitCompiler` unchanged in shape
**Requirement**: LOV-20
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every existing codegen integration test passes on the IR path
- [ ] `Element` trait and `ElementCapabilities` unmodified (grep-verified)
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): device path consumes IR-built kernels`

---

### T33: `headers/spice/` and the parity suite on the IR path

**What**: Prove numerical invisibility of the migration.
**Where**: `headers/spice/` (bounded edits only), `tests/ngspice_*.rs` (**unmodified**)
**Depends on**: T32
**Reuses**: the entire parity oracle (MD-38)
**Requirement**: LOV-20, LOV-21
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every `tests/ngspice_*` assertion passes with tolerance contracts **unmodified**
- [ ] `tests/ngspice_parity_matrix.rs` registry unchanged
- [ ] Any `headers/spice/` edit is listed in the commit body with its reason
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): spice models numerically unchanged through the IR`

---

### T34: Delete `flatten/` and the digital AST path

**What**: Remove the replaced stages. **Deleted, not retained behind a flag.**
**Where**: `crates/piperine-codegen/src/flatten/`, `resolve/`, the AST→Cranelift digital path
**Depends on**: T33
**Reuses**: —
**Requirement**: LOV-24
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `flatten/` gone; `resolve/` gone or reduced to what `lower/` did not absorb
- [ ] No feature flag or dead branch preserves the old path (MD-35: no dead-architecture identifiers)
- [ ] `tests/suite_hygiene.rs` passes
- [ ] Gate passes: `cargo build --workspace` zero warnings, `cargo test --workspace`

**Tests**: integration · **Gate**: build
**Commit**: `refactor(codegen): delete flatten and the digital AST path (MD-41)`

---

### T35: `--emit=ir` on the CLI

**What**: Dump IR after any named pass, diffable.
**Where**: `crates/piperine-cli/src/commands/build.rs`, `check.rs`
**Depends on**: T34
**Reuses**: T8's printer
**Requirement**: LOV-08
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `piperine build --emit=ir` prints; `--emit=ir-after-<pass>` selects a stage
- [ ] Two runs on the same input produce byte-identical output
- [ ] CLI integration test
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(cli): --emit=ir for traceability`

---

### T36: `Bit[N]` lowers to one width-N value

**What**: The surface change that removes 17 wires from a 4-bit adder.
**Where**: `crates/piperine-lang/src/elab/`, `crates/piperine-codegen/src/lower/`
**Depends on**: T35
**Reuses**: T2's `Ty::Bits`
**Requirement**: LOV-26
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `wire b : Bit[8]` produces one IR value of width 8
- [ ] `examples/17_ripple_adder_4bit.phdl` still passes (behaviour preserved)
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): Bit[N] is one width-N value`

---

### T37: Slice and concat as chunk references

**What**: `x[31:25]` and `concat(a,b)` lower to chunk refs with no computation.
**Where**: `crates/piperine-lang/src/parse/`, `elab/`, `codegen/src/lower/`
**Depends on**: T36
**Reuses**: T6's `SigRef`
**Requirement**: LOV-27
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Both forms parse, elaborate, and lower to chunk refs
- [ ] No op is emitted for either
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): slice and concat as chunk references`

---

### T38: Indexed write — the spec's own `SarAdc` compiles

**What**: `code[idx] = 0` in a digital body, replacing `emit/stmt.rs:120`'s `Unsupported`.
**Where**: `crates/piperine-codegen/src/lower/digital.rs`, `crates/piperine-lang/src/parse/`
**Depends on**: T37
**Reuses**: T6, T22
**Requirement**: LOV-28
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The `SarAdc` example printed in Part I §10.3 compiles **and simulates**
- [ ] Added to `examples/` and gated by `tests/run_examples.rs`
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration + examples gate · **Gate**: full
**Commit**: `feat(lang): indexed bit write; the spec's SarAdc compiles`

---

### T39: Multi-bit `match` patterns

**What**: `0b1??0` — already in the grammar (App. B §B.8), unsupported below.
**Where**: `crates/piperine-codegen/src/lower/digital.rs`
**Depends on**: T38
**Reuses**: T6
**Requirement**: LOV-29
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Multi-bit patterns compile to a mask + compare
- [ ] Integration test over all don't-care positions
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): multi-bit match patterns`

---

### T40: `for` in digital bodies

**What**: Unrolled at elaboration or lowered as an IR loop.
**Where**: `crates/piperine-lang/src/elab/`, `codegen/src/lower/digital.rs`
**Depends on**: T39
**Reuses**: existing structural-`for` unrolling
**Requirement**: LOV-30
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `for` compiles in a digital body
- [ ] A non-const bound is a named error, not a silent unroll
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): for in digital bodies`

---

### T41: Ship `UInt`/`SInt` in `headers/`

**What**: Part I §6.1/§6.6 promise them; they exist only in a parse-only test fixture.
**Where**: `crates/piperine-lang/headers/`
**Depends on**: T40
**Reuses**: `crates/piperine-lang/tests/examples/capabilities.phdl` as the starting definition
**Requirement**: LOV-31
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `UInt`/`SInt` resolve from a shipped header, with `concat`
- [ ] `extern_coverage_guard.rs` passes
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): ship UInt/SInt in headers`

---

### T42: PHDL memory declaration → array-typed cell

**What**: A declaration form for a memory, lowering to `Ty::Array`.
**Where**: `crates/piperine-lang/src/parse/`, `elab/`, `codegen/src/lower/`
**Depends on**: T41
**Reuses**: T4, T22
**Requirement**: LOV-32
**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A register file declares and simulates in PHDL
- [ ] An FSM + datapath example lands in `examples/`, gated by `tests/run_examples.rs`
- [ ] Gate passes: `cargo build --workspace` zero warnings, `cargo test --workspace`

**Tests**: integration + examples gate · **Gate**: build
**Commit**: `feat(lang): memory declarations lower to array cells`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8

Phase 1 (IR core):        T1 → T2 → T3 → T4 → T5 → T6 → T7
Phase 2 (IR services):    T8 → T9 → T10 → T11 → T12 → T13
Phase 3 (oracle + gates): T14 → T15 → T16 → T17          ← P1 closes
Phase 4 (backends):       T18 → T19 → T20 → T21 → T22
Phase 5 (completeness):   T23 → T24 → T25                 ← P2 closes
Phase 6 (lowering):       T26 → T27 → T28 → T29 → T30 → T31
Phase 7 (switchover):     T32 → T33 → T34 → T35           ← P3 closes
Phase 8 (surface):        T36 → T37 → T38 → T39 → T40 → T41 → T42  ← P4 closes
```

42 tasks, 8 phases. Packing at ~7 tasks/batch yields **~6 batch workers**.

---

## Task Granularity Check

| Task range | Scope per task | Status |
|---|---|---|
| T1 | 1 crate rename | ✅ Granular |
| T2–T7 | 1 module each (value, net, cell, block, sigref, verify) | ✅ Granular |
| T8–T13 | 1 module each (print, parse, catalog, migration, diff, interp) | ✅ Granular |
| T14–T17 | 1 test target each | ✅ Granular |
| T18–T22 | 1 backend concern each | ✅ Granular |
| T23–T25 | 1 fixture each | ✅ Granular |
| T26–T31 | 1 lowering concern each | ✅ Granular |
| T32–T35 | 1 integration step each | ✅ Granular |
| T36–T42 | 1 language construct each | ✅ Granular |

**T11 note**: migrates several families in one task. Kept whole because it is a
**move** with tests travelling intact — splitting it per family would create
intermediate states where the catalog is half-populated and nothing compiles.
Cohesive, ⚠️→✅.

---

## Diagram-Definition Cross-Check

| Task | Depends on (body) | Diagram | Status |
|---|---|---|---|
| T1 | None | phase head | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | T2 | T2→T3 | ✅ |
| T4 | T2 | T3→T4 *(chain)* | ✅ ordering compatible — T4 needs only T2, runs after T3 |
| T5 | T3, T4 | T4→T5 | ✅ |
| T6 | T2 | T5→T6 *(chain)* | ✅ ordering compatible |
| T7 | T5, T6 | T6→T7 | ✅ |
| T8–T13 | previous in chain (T13: T11,T12) | linear | ✅ |
| T14–T17 | previous in chain | linear | ✅ |
| T18–T22 | previous in chain | linear | ✅ |
| T23–T25 | previous in chain | linear | ✅ |
| T26–T31 | previous in chain | linear | ✅ |
| T32–T35 | previous in chain | linear | ✅ |
| T36–T42 | previous in chain | linear | ✅ |

**Rule check**: no task depends on a later phase. T4/T6 declare narrower
dependencies than the linear chain implies, which is safe (the chain is a
superset). No backward violations.

---

## Test Co-location Validation

| Task | Layer created/modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1 | crate rename (no new layer) | none (build gate) | none | ✅ |
| T2–T6 | IR core | unit | unit | ✅ |
| T7 | IR verifier | unit + integration | unit + integration | ✅ |
| T8, T9 | IR printer/parser | integration | integration | ✅ |
| T10, T11 | catalog | unit | unit | ✅ |
| T12 | diff pass | unit | unit | ✅ |
| T13 | interpreter | integration | integration | ✅ |
| T14–T17 | differential + gates | integration | integration | ✅ |
| T18–T22 | codegen backends | integration | integration | ✅ |
| T23–T25 | fixtures | integration | integration | ✅ |
| T26–T31 | codegen lowering | integration | integration | ✅ |
| T32, T33, T35 | cross-crate | integration | integration | ✅ |
| T34 | deletion | integration (hygiene gate) | integration | ✅ |
| T36–T41 | PHDL surface | integration (+examples for user-facing) | integration / +examples on T38 | ✅ |
| T42 | PHDL surface | integration + examples | integration + examples | ✅ |

No `Tests: none` is justified by "tested elsewhere". T1 is the only `none`, and
the matrix allows it (rename, build gate).

---

## Requirement Coverage

All 32 LOV requirements map to at least one task:

| Requirement | Tasks |
|---|---|
| LOV-01 | T2, T5, T17, T26 |
| LOV-02 | T8, T9, T15 |
| LOV-03 | T4, T7, T16 |
| LOV-04 | T7 |
| LOV-05 | T16, T17, T29, T30 |
| LOV-06 | T8, T35 |
| LOV-07 | T5 |
| LOV-08 | T8, T35 |
| LOV-09 | T1 |
| LOV-10 | T13 |
| LOV-11 | T14, T15 |
| LOV-12 | T10, T11 |
| LOV-13 | T3, T16, T29 |
| LOV-14 | T3, T7, T16 |
| LOV-15 | T12, T19 |
| LOV-16 | T18, T19, T20, T23 |
| LOV-17 | T21, T24 |
| LOV-18 | T22, T25 |
| LOV-19 | T22 |
| LOV-20 | T32, T33 |
| LOV-21 | T33 |
| LOV-22 | T27 |
| LOV-23 | T28 |
| LOV-24 | T26, T34 |
| LOV-25 | T31 |
| LOV-26 | T22, T36 |
| LOV-27 | T6, T37 |
| LOV-28 | T38 |
| LOV-29 | T39 |
| LOV-30 | T40 |
| LOV-31 | T41 |
| LOV-32 | T42 |

**Coverage: 32 total, 32 mapped, 0 unmapped.**

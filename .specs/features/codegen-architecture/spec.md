# Codegen Architecture Refactor Specification

## Problem Statement

`piperine-codegen` (13.3k LOC) has drifted into hard-to-read shape. The module
tree does not map to the compilation pipeline, several god-files mix unrelated
responsibilities, one god-struct holds every analog capability at once, and a
few crate-wide contracts live in the wrong place. None of this is a functional
bug — it is a **readability/architecture** debt that violates the project's own
binding Rust-idiom rules (STATE.md MD-13). Concrete smells (evidence):

1. **`codegen/` vs `jit/` is a false split.** Both compile to Cranelift.
   `codegen/` holds the *emission machinery* (`Builder`, `Codegen` trait,
   `emit_analog`, CSE); `jit/` holds the *products* (`AnalogKernel`,
   `DigitalKernel`) plus the *flatten stage*. The crate is named `codegen` and
   contains a sub-module also named `codegen` — the names do not convey the
   distinction (violates MD-13 rule 4: modules named by system function).
2. **`AnalogKernel` is a god-struct** (~40 fields, ~50 methods) holding every
   analog capability inline — terminals, params, limits (5 fields), forces (4),
   noise (3), charge (3), ac_stim, events, diagnostics, runtime_states — with
   `has_reactive()`/`has_force_*()` bool accessors that duplicate what an
   `Option<Capability>` would express (violates MD-13 rules 1 & 3).
3. **God-functions.** `emit_analog` is a single 594-line function
   (`codegen/analog_emit.rs:25`). `codegen/builder.rs` (1293 LOC) is a grab-bag:
   CSE infra + `Resolver` + `Typed` + `DigTy` + `Builder` + `Tape` + statement
   emission + helpers + `expr_structural_eq` (violates MD-13 rule 3).
4. **Misplaced contracts.** `CodegenError` (crate-wide, used by `device/`,
   `lower/`) lives in `jit/mod.rs`; its `ModuleNotFound` message still says
   "not found in IrProgram" — a type deleted long ago. `SimCtx` (the analog JIT
   ABI struct) is also dumped in `jit/mod.rs`. A legacy `pub use lower as ir`
   alias survives "for call-site continuity" (violates MD-13 rules 1 & 4).

## Goals

- [ ] The module tree mirrors the compilation pipeline stage-for-stage, so a
      glance at the file tree tells the reader where any logic lives (MD-13 r4):
      `POM → resolve → flatten → emit → kernel → device`.
- [ ] Every god-file and god-struct is decomposed along responsibility /
      capability lines (MD-13 r1, r3); bool `has_*` capability flags become
      `Option<Capability>` presence. This includes the `device/` god-files
      (`device/analog.rs` by capability, `device/circuit.rs` by
      responsibility) — **in scope, not deferred** — using **capability
      traits** as the decomposition seam (internal to codegen; the solver
      `Element` object stays flat, MD-01 upheld).
- [ ] Crate-wide contracts live where they belong: `CodegenError` at a crate
      root error module (messages corrected), `SimCtx` with the emit/ABI layer;
      the legacy `ir` alias is removed.
- [ ] **Zero functional change.** The entire existing test suite passes
      unchanged — no test is weakened, skipped, or rewritten to fit the new
      shape. Behavior is byte-for-byte preserved.

## Out of Scope

| Item | Reason |
| ---- | ------ |
| Any behavior/functional change | This is a pure readability/architecture refactor; outputs must be identical. |
| New codegen features (gap fills, new operators) | Tracked elsewhere (ROADMAP); mixing them in would obscure the refactor diff. |
| Performance optimization | Not a goal; only accept a change if it is behavior-neutral. Perf work is separate. |
| `piperine-solver`/`piperine-lang` internals | Refactor is scoped to `piperine-codegen`; cross-crate call-site fixups only as forced by removing the `ir` alias / moving public items. |
| A two-tier `prelude`/`abi` surface (MD-17 style) | **Resolved (user):** codegen has one deliverable → a single tidy `lib.rs` façade, not the solver's two-tier split. |
| New `Element` facets / capability trait objects across the ABI | Capability traits are **internal** to codegen (kernel/instance); the solver `Element` object stays flat (MD-01). |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Refactor is safe under the existing test suite | The codegen + host + solver tests (analog_jit, digital_jit, codegen_ir, from_ir, silent_bugs, ngspice_validation, session, …) are a strong behavioral net | CLAUDE.md "Tests of record"; the suite already gates every construct | y (agent, from CLAUDE.md) |
| Target module names | `resolve` / `flatten` / `emit` / `kernel` / `device` (one per pipeline stage) | Each name states the system function it performs (MD-13 r4) | n (**Design-phase user review**) |
| `AnalogKernel` decomposition | Capability sub-structs behind `Option` (`reactive`, `forces`, `limits`, `noise`, `ac_stim`), lean `core` for the always-present residual/Jacobian/terminals | Option-presence *is* the capability contract (MD-13 r1); kills the `has_*` bool flags | n (**Design-phase user review**) |
| No macros introduced | Data tables + trait/struct methods only | MD-13 r5 | y (locked) |
| Every moved fn keeps an owner | No loose `pub(crate) fn` at module scope; helpers become trait/struct methods | MD-13 r2 | y (locked) |

**Open questions (for Design/user):** the exact target module names, the
`AnalogKernel` capability grouping boundaries, and whether `piperine-codegen`
adopts a disciplined public surface (single façade vs today's scattered
`pub use`). All resolved at Design; the spec fixes *what* must improve.

---

## User Stories

### P1: Pipeline-shaped module tree ⭐ MVP

**User Story**: As a codegen maintainer, I want the module tree to mirror the
compilation pipeline, so that finding "where does X happen" is a glance at the
file tree, not a three-file trace.

**Acceptance Criteria**:

1. WHEN a reader opens `crates/piperine-codegen/src/` THEN the top-level modules
   SHALL name pipeline stages (`resolve`, `flatten`, `emit`, `kernel`,
   `device`) — no module named after a language construct or a vague bucket
   (`jit`, an inner `codegen`), per MD-13 r4.
2. WHEN the `resolve` (formerly `lower`) module is referenced THEN the legacy
   `pub use lower as ir` alias SHALL be gone and all call sites use the real
   path.
3. WHEN the crate builds THEN `cargo build --workspace` SHALL emit **zero**
   warnings and the full test suite SHALL pass unchanged.

**Independent Test**: `cargo test --workspace` green pre- and post-refactor with
the identical set of tests; `rg 'as ir\b'`/`mod jit`/inner `mod codegen` returns
nothing in the crate.

---

### P2: Decomposed god-struct & god-functions

**User Story**: As a codegen maintainer, I want `AnalogKernel` and the big
emission functions split along capability/responsibility lines, so each concern
reads in isolation.

**Acceptance Criteria**:

1. WHEN `AnalogKernel` is inspected THEN its optional analog capabilities
   (reactive/charge, forces, limits, noise, ac_stim) SHALL be grouped into
   named sub-structs held as `Option<_>`; a `has_<cap>()` query SHALL be
   `self.<cap>.is_some()`, not a separately-stored bool.
2. WHEN `emit_analog` and `builder.rs` are inspected THEN each SHALL be split so
   no single function exceeds a readable size and each file owns one
   responsibility (emission of one expr category; `Builder` vs `Resolver` vs
   statement emission vs CSE).
3. WHEN any helper is moved THEN it SHALL retain a trait/struct owner — no new
   module-level loose `fn` (MD-13 r2).
4. WHEN `device/analog.rs` is inspected THEN `AnalogInstance` SHALL be split so
   each analog capability (forces, limits, operators, events) owns its runtime
   state and MNA stamping (a `Stamps`-style trait or capability struct method),
   and `load_dc`/`load_ac`/`load_transient` SHALL fold per-capability stamps
   rather than inline every capability in one method.
5. WHEN `device/circuit.rs` is inspected THEN it SHALL be split by
   responsibility — `CircuitCompiler` (public build API), `InstanceBuilder`
   (assembly), fusion, and plugin instantiation each in their own file.

**Independent Test**: The analog kernel tests (`analog_jit.rs`) and the
device/circuit tests (`codegen_ir.rs`, `from_ir.rs`, `silent_bugs.rs`,
`session.rs`, `ngspice_validation.rs`) pass unchanged; `AnalogKernel`/
`AnalogInstance` field counts drop materially; no `pub fn`/`pub(crate) fn` at
module scope in the changed files.

---

### P3: Contracts in the right place

**User Story**: As a codegen maintainer, I want crate-wide contracts located by
responsibility, so their ownership is obvious.

**Acceptance Criteria**:

1. WHEN `CodegenError` is referenced THEN it SHALL live in a crate-root error
   module (not inside a pipeline-stage module), and every variant message SHALL
   be accurate (no reference to the deleted `IrProgram`).
2. WHEN `SimCtx` is referenced THEN it SHALL live with the emission/ABI layer it
   belongs to, not in a module `mod.rs` acting as a dumping ground.

**Independent Test**: `CodegenError`/`SimCtx` resolve from their new homes; the
"IrProgram" string is absent; tests green.

---

## Edge Cases

- WHEN the refactor moves a `pub` item that a sibling crate imports THEN the
  cross-crate call site SHALL be updated in the same commit — the workspace
  never builds broken between commits.
- WHEN a capability sub-struct is `None` THEN every stamping/eval path that
  previously checked a `has_*` bool SHALL behave identically (same branches
  taken) — verified by the unchanged device/solver tests.
- WHEN a file split changes item visibility THEN no item SHALL become *more*
  public than needed (default to `pub(crate)`/private; only re-export what the
  crate's façade requires).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| CGA-01 | P1 Pipeline module tree | Design | Pending |
| CGA-02 | P1 Drop `ir` alias, real paths | Design | Pending |
| CGA-03 | P1 Zero-warning, suite-green invariant | Design | Pending |
| CGA-04 | P2 `AnalogKernel` capability sub-structs | Design | Pending |
| CGA-05 | P2 Split `emit_analog` / `builder.rs` | Design | Pending |
| CGA-06 | P2 Every helper keeps an owner (MD-13 r2) | Design | Pending |
| CGA-07 | P3 `CodegenError` home + correct messages | Design | Pending |
| CGA-08 | P3 `SimCtx` home | Design | Pending |
| CGA-09 | P2 `device/analog.rs` split by capability (+ `Stamps` trait) | Design | Pending |
| CGA-10 | P2 `device/circuit.rs` split by responsibility | Design | Pending |

**ID format:** `CGA-[NUMBER]`

**Coverage:** 10 total, all designed (`design.md`) and mapped to tasks
(`tasks.md`, T1–T18). Not started.

---

## Success Criteria

- [ ] `cargo test --workspace` green with the **identical** test set (no test
      weakened/removed); `cargo build --workspace` zero warnings.
- [ ] Top-level module tree names pipeline stages; `jit`/inner-`codegen`/`ir`
      alias gone.
- [ ] `AnalogKernel` capabilities are `Option` sub-structs; no `has_*` bool
      duplication; no god-function over a readable size threshold.
- [ ] `CodegenError`/`SimCtx` relocated; stale `IrProgram` message gone.
- [ ] Every changed file honors MD-13 (contracts-first, no loose fns, clean,
      system-function names, no macros).

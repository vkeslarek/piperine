# Hierarchy Flattening Specification

> **Was** `codegen-parametric-devices`. Renamed because the original spec
> framed the problem as *parametric monomorphization* — but that half
> **already exists** and works (see "Already Solved" below). The
> genuinely-missing capability is **hierarchy flattening**: inlining a
> mid-level module's sub-instances into the parent's flat netlist so codegen
> ever sees them. Surfaced during the BSIM-models port (multi-segment / NQS
> structure); `urc` is the smaller ngspice-validatable proof.

## Problem Statement

Codegen accepts only a **two-level** netlist: a top module that directly
instantiates **leaf** devices (modules with an analog/digital body and *no*
sub-instances). `InstanceBuilder::add_instance` (`circuit.rs:389`) hard-errors
the moment an instantiated child itself instantiates further modules:

```rust
if !child.instances().is_empty() {
    return Err(CodegenError::unsupported(format!(
        "nested hierarchy: `{}` instantiates further modules — flatten during elaboration",
        child.name()
    )));
}
```

There is **no elaboration pass that flattens hierarchy** — nothing inlines a
nested module's instances, wires, and connections into its parent. So any
*mid-level* module — one that both instantiates devices **and** is itself
instantiated — is unbuildable. This blocks every ladder/distributed device
(`urc`'s `lump=N` segments, BSIM NQS segments): a `urc` module necessarily
instantiates N sub-elements, so dropping it into a circuit makes a 3-level
hierarchy (`top → urc → segments`) that codegen rejects.

The **structural-monomorphization** blocker the original spec worried about is
**not** the issue — that half works (see "Already Solved"). But flatten is not
the *only* remaining gap: `p1-solver-complete` T16 (the urc attempt) found
**three** distinct codegen gaps. Flatten is the headline; the other two bite
depending on how `urc` is authored.

## Already Solved (do not re-implement)

| Capability | Where | Status |
| ---------- | ----- | ------ |
| Generic modules `mod Foo[N]`, N a `nat` const — **structure** (ports/wires/instances) | `elab/lower/mono.rs` `monomorphize()` | Works — mangles `Foo__3`/`Foo__5`, caches in `mono_cache`, emits a distinct `Module` per const-arg tuple. |
| Per-shape distinct kernels (monomorphization) | Falls out for free: codegen's `kernels: HashMap<String, _>` keyed by module **name** already gives `Foo__3`/`Foo__5` distinct kernels — no cache-key change needed. | Works. |
| Structural `for`/`if` unrolling in a module body | `elab/lower/module.rs` (`StructuralFor` const-unrolled, `StructuralIf` const-folded), instances triggering on-demand monomorph | Works — produces the sub-instances in the parent `Module`. |
| Non-structural param restamp (compile-once sweep) | `Design::with_overrides_applied` — flat, already-monomorphized netlist | Works. |

**Do not change the codegen cache key** (the original P2 design) — monomorph
already happens at elaboration via name-mangling; codegen stays name-keyed.

## The Three Real Gaps (T16 findings — verified 2026-07-20)

1. **Hierarchy flattening (headline).** Codegen (`circuit.rs:389`) rejects a
   submodule that itself instantiates devices. No elaboration pass inlines a
   nested module's instances/wires/connections into its parent. **Required by
   every `urc` authoring route.**
2. **Const-args not substituted into analog behaviors.** `AttachBehaviors`
   (`elab/lower/passes.rs:160-176`) attaches the base analog behavior to a
   monomorphized module by prefix-match but pushes `behavior.clone()`
   **unsubstituted** — a generic module's `analog` body still sees `N` as
   undefined. `Stmt::subst_const` exists (`ast.rs:584`, already used for
   `StructuralFor` unrolling) but is **not** applied per monomorphized
   variant. **Bites only if `urc`'s own `analog` body references `N`** (e.g.
   scaling a contribution by segment count without sub-instances); a
   pure-structural `urc` (for-loop of leaf segments, no own analog block)
   avoids it.
3. **Array wires are not flat analog nets.** `wire node : Electrical[N+1]`
   stays one array-typed net; `node[i]` in `V()`/`I()` cannot resolve (no
   `node[0]` in the node table). No array-net → flat-net expansion in the
   flattener. **Bites only the array-net authoring route**; a `StructuralFor`
   that generates individually-named wires + connections avoids it.

The cleanest `urc` route (pure-structural `for` over leaf segments with
generated per-index wires) needs **only gap 1**. Gaps 2 and 3 are logged so a
future device that needs `N` in-behavior, or array-net syntax, has them
tracked — but the MVP targets gap 1 alone.

## Goals

- [ ] An elaboration pass flattens hierarchy: a mid-level module's instances,
      wires, and connections are inlined into the top module's flat netlist,
      recursively (outer-to-inner), so codegen only ever sees leaf devices.
- [ ] Combined with the *existing* `mod Foo[N]` + `StructuralFor`, a `urc[N]`
      authored as N series sub-elements builds and simulates. `N` comes from
      the existing monomorphization path — this feature adds no new param
      machinery.
- [ ] `urc` (ngspice lumped RC line, `lump=N`) ships as the end-to-end proof:
      an N-segment ladder, N via `urc[N]` monomorphization, ngspice-validated
      at ≥3 distinct `lump` values.
- [ ] Flattening preserves the flat-netlist contract hosts already rely on:
      instance labels in `root_module` stay addressable for overrides
      (`Design::with_overrides_applied`), name collisions across inlined
      sub-instances resolved deterministically (path-prefixed labels).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| **Parametric monomorphization machinery** | Already exists (`mono.rs`) — this feature *consumes* it, does not rebuild it. The original P2 "change the codegen cache key" is explicitly **rejected**: monomorph is done at elaboration by name-mangling. |
| Runtime-variable segment count (N changes during simulation) | MD-18: elaboration fixes devices; N is fixed at build time like every structural parameter. |
| BSIM's dynamic self-heating thermal node | Separate feature — `self-heating` (authorable today from existing primitives, not a capability gap): a *fixed* extra internal node, orthogonal to hierarchy flattening. See that feature's spec. |
| `urc`'s lossy-line variant (LTRA, distributed R+G+C) | ROADMAP logs LTRA as backlog beyond urc's lumped case. |
| Compile-time optimization for very large N (thousands of segments) | Correctness first; a follow-up perf pass if N in practice gets large, not blocking. |
| Multi-dimensional generate (nested loops, 2D sub-instance arrays) | No device in the ngspice-46/BSIM backlog needs it; single-dimension covers `urc` and BSIM NQS. Revisit per concrete need. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | --------------- | --------- | ---------- |
| Where flattening runs | An elaboration pass, after monomorphization + `StructuralFor` unrolling, before the POM `Design` is handed to codegen | Codegen must receive an already-flat netlist (`circuit.rs:389` contract, host flat-netlist contract in `design.rs:441`); monomorph/unroll must precede it so the pass sees concrete instances | y (agent, from code) |
| Inlined-instance naming | Path-prefixed labels (`urc0.seg1`, or `urc0__seg1`) — deterministic, collision-free, still a flat bare label in `root_module` | Preserves the host override contract (`with_overrides_applied` addresses flat labels); a hierarchical path would break it | n (needs Design-phase confirmation of separator + override-path compatibility) |
| Net/wire inlining | A sub-module's internal wires become fresh top-level nets (path-prefixed); its ports bind to the parent's connected nets | Standard netlist flattening; ports are the only cross-boundary identity | y (agent default) |
| Monomorphization reuse | The flatten pass consumes `mono_cache` output as-is; two `urc[3]` share the monomorphized `urc__3` **before** flattening, then each inlines its own labeled copy | Keeps monomorph = one compile per shape (already true), flatten = per-instance label expansion | y (agent, from mono.rs) |
| Generate syntax | The **existing** `StructuralFor` over instances (`for (i in 0..N) { seg[i]: Segment(...); }`) — no new grammar; `[N]` monomorph supplies N | Both halves already parse/elaborate; the only missing link is the flatten that lets the resulting mid-level module be instantiated | y (code — StructuralFor + mono both exist) |

**Open questions:** the inlined-label separator and its exact
override-path compatibility (Design-phase). Everything structural is resolved:
the feature is a single flatten pass over already-monomorphized, already
`for`-unrolled modules.

---

## User Stories

### P1: Hierarchy flattening ⭐ MVP

**User Story**: As a PHDL device author, I want a mid-level module (one that
instantiates other modules) to be usable inside a larger circuit, so I can
compose devices hierarchically — `urc` as N series segments, BSIM as body +
NQS segments — instead of hand-inlining every primitive into one flat module.

**Why P1**: The core missing capability. Monomorphization and `StructuralFor`
already exist; without the flatten, their output (a mid-level module full of
sub-instances) is unbuildable — codegen hard-errors at `circuit.rs:389`.

**Acceptance Criteria**:

1. WHEN the top module instantiates a mid-level module (one whose body
   contains sub-instances) THEN an elaboration pass SHALL inline that
   sub-module's instances, wires, and connections into the flat netlist,
   recursively, so the `Design` handed to `CircuitCompiler` contains only
   leaf devices — `circuit.rs:389`'s "nested hierarchy" error SHALL become
   unreachable for well-formed input.
2. WHEN a sub-instance's port connects to a parent net (and its internal wire
   connects only within the sub-module) THEN flattening SHALL bind the port to
   the parent's net and lift the internal wire to a fresh, collision-free
   top-level net (path-prefixed label).
3. WHEN two instances of the same sub-module coexist THEN their inlined labels
   SHALL be distinct and deterministic (path-prefixed), and each SHALL remain
   addressable as a flat bare label for host overrides
   (`Design::with_overrides_applied`) — no regression to the flat-netlist
   host contract.
4. WHEN `urc` is authored as `urc[N]` (existing monomorphization) with a
   `StructuralFor` over `N` series segments, instantiated in a circuit, and
   simulated at a `.op`/`.tran` point THEN the result SHALL match ngspice's
   `urc` device within the existing cross-check tolerance
   (`tests/ngspice_validation.rs` pattern).

**Independent Test**: Author `urc.phdl` as `urc[N]` + `StructuralFor` segments;
run a DC op point at 3 `lump` values (2, 5, 10); confirm each matches ngspice's
`urc` at that `lump`. A second test asserts a plain (non-parametric) two-level
mid-level module also flattens and builds.

---

### P2: Monomorphization already holds per-shape (regression guard)

**User Story**: As the compiler, I want two `urc` instances with different
`N` to produce distinct kernels and two with the same `N` to share one — this
already falls out of name-mangled monomorphization; this story only guards it
against regression from the new flatten pass.

**Why P2**: Not new work — `Foo__3`/`Foo__5` already compile distinctly (codegen
keyed by name), and `with_overrides_applied` already restamps non-structural
params. The flatten pass must not break either.

**Acceptance Criteria**:

1. WHEN a circuit instantiates `urc[3]` and `urc[7]` THEN codegen SHALL compile
   two distinct kernels (`urc__3`, `urc__7`) — verified via
   `AnalogKernel::compile_count` delta of 2 (existing `compile_count` pattern
   in `tests/compile_once_sweep.rs`).
2. WHEN a circuit instantiates `urc[3]` twice with different **non-structural**
   params (`r`, `c`) THEN codegen SHALL compile **one** shared `urc__3` kernel
   and restamp both — no regression to compile-once.
3. WHEN a sweep varies a non-structural param across many points on a built
   circuit THEN it SHALL restamp the existing kernel with zero additional
   compiles — no regression to `tests/compile_once_sweep.rs`/`dc_host_proof.rs`.

**Independent Test**: Build a circuit with `urc[3]` and `urc[7]`; assert
`compile_count` delta is exactly 2; sweep `r` on one across 20 points; assert
the delta stays 2.

---

## Edge Cases

- WHEN `N` resolves to 0 THEN monomorphization/elaboration SHALL fail loud
  (an empty ladder has no defensible meaning; ngspice's `urc` requires
  `lump >= 1`).
- WHEN `N` is negative or non-const THEN elaboration SHALL fail loud (an
  invalid structural parameter — already the const-eval path's behavior).
- WHEN a mid-level module nests another mid-level module (3+ levels) THEN
  flattening SHALL recurse outer-to-inner; if the recursive case is not yet
  implemented it SHALL fail loud, never mis-flatten.
- WHEN inlined sub-instance labels would collide (same sub-module twice) THEN
  path-prefixing SHALL disambiguate deterministically — never a silent
  overwrite of one instance by another.
- WHEN a sub-module port is left unconnected in the parent THEN flattening
  SHALL preserve the existing unconnected-port diagnostic — no new silent
  dangling net.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| FLAT-01 | P1 Hierarchy flattening (inline pass) | Tasks | Designed |
| FLAT-02 | P1 Hierarchy flattening (net/port binding) | Tasks | Designed |
| FLAT-03 | P1 Hierarchy flattening (label collision + host contract) | Tasks | Designed |
| FLAT-04 | P1 Hierarchy flattening (urc ngspice proof) | Tasks | Designed |
| FLAT-05 | P2 Monomorph regression guard (distinct kernels) | Tasks | Designed |
| FLAT-06 | P2 Monomorph regression guard (shared kernel restamp) | Tasks | Designed |
| FLAT-07 | P2 Monomorph regression guard (sweep, zero recompile) | Tasks | Designed |

**ID format:** `FLAT-[NUMBER]`

**Coverage:** 7 total, all designed (`design.md`) and mapped to tasks
(`tasks.md`, T1–T10). Not started.

---

## Success Criteria

- [ ] A mid-level module (instantiates sub-modules) builds and simulates —
      `circuit.rs:389` "nested hierarchy" error unreachable for valid input.
- [ ] `urc` ships in PHDL as `urc[N]` + `StructuralFor`, ngspice-validated at
      ≥3 distinct `lump` values.
- [ ] Non-const/zero/negative `N` fails loud (no silent fallback/truncation).
- [ ] No regression: monomorphization still yields correct per-shape kernel
      counts; compile-once sweep/live-session tests stay green; host override
      flat-label contract intact.
- [ ] `cargo test --workspace` green; zero rustc warnings.

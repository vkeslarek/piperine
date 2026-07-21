# Solver ABI — Library Surface + Element ABI Completion

**Merges:** `solver-library-abi` + `solver-osdi-abi-completion` (both standalone
specs deleted when this feature lands — this file is their successor).
**Implements:** MD-06 (init_global as Once), MD-11 (OSDI as checklist),
MD-12 (ABI vs policy classification), MD-13 (Rust idiom rules 2+4).
**Codebase baseline (2026-07-16):** branch `feature/plugin-architecture`,
392 workspace tests green, zero warnings. All file/line references below were
verified against this baseline.

## Problem Statement

Two related holes, one on each side of the `Element` ABI:

1. **Host side (library surface).** A host must assemble `CircuitInstance`
   manually (no builder), every internal module is `pub mod` in `lib.rs`
   (downstream crates import 25+ internal paths), and `digital/scheduler.rs`
   (403 lines) mixes three concepts (`DigitalTopology`, `DigitalState`,
   evaluation loops) in one file. `DcAnalysisResult::as_iv` still couples the
   analysis layer to `Netlist`.
2. **Element side (ABI completion).** The `Element` trait has introspection
   (`list_params`/`set_param` with `Invalidation`, `QueryDescriptor`,
   `TerminalDescriptor`) but no lifecycle hooks (setup/destroy), no
   internal-unknown allocation seam (the `HAS_INTERNAL_UNKNOWNS` capability is
   declared but there is no API through which an element can *take* that
   seam), terminal descriptors lack discipline + sign convention, noise
   sources are anonymous (total PSD only), and there is no stamp-capability
   declaration (linear / charge / analytic-Jacobian).

## Goals

- [ ] Two-tier public surface: `prelude` (host: build → run → read) and `abi`
      (device author: implement `Element`); every other module `pub(crate)`.
- [ ] `Solver` entry point whose `build()` triggers `Context::init_global`
      (MD-06) and hands out all five analyses.
- [ ] `CircuitBuilder`: safe, discoverable circuit assembly (no manual
      `Netlist::connect_node`).
- [ ] `digital/scheduler.rs` split by system function into `topology.rs`,
      `state.rs`, `scheduler.rs` (MD-13 rule 4), no free functions (rule 2).
- [ ] `DcAnalysisResult::as_iv` takes `&CircuitInstance`, not `&Netlist`.
- [ ] Element lifecycle: `setup` (once, before first analysis) and `destroy`
      (on circuit drop).
- [ ] Internal-unknown allocation API (`allocate_unknowns` + allocator),
      called before the matrix shape freezes.
- [ ] `TerminalDescriptor` gains `discipline` and `sign` (OSDI sign
      convention).
- [ ] Named noise sources: `Noise` carries `name` + `NoiseKind`; the noise
      analysis reports per-source contributions, not only the total PSD.
- [ ] Stamp-capability declaration: `LINEAR`, `STAMPS_CHARGE`,
      `ANALYTIC_JACOBIAN` capability flags (declared + documented; consumed by
      the perf follow-up).

## Out of Scope

| Feature | Reason |
|---------|--------|
| Model/instance separation (`ModelHandle` vs `ElementInstance`) | No in-repo consumer (piperine-osdi is external); follow-up when the OSDI plugin needs it — user decision 2026-07-16 |
| Commit/rollback lifecycle hooks (`checkpoint_state`/`rollback_state`/`commit_state`) | Separate feature `solver-commit-rollback` |
| Solver consumption of `LINEAR`/`STAMPS_CHARGE`/`ANALYTIC_JACOBIAN` (bypass fast paths, single-LU) | Perf follow-up; this feature only declares + documents the flags |
| `NewtonStrategy`/`StepperStrategy` extraction | `solver-strategy-composition` |
| Temperature protocol beyond existing `set_temperature` + `Invalidation::Temperature` | Already adequate; no new requirement surfaced |
| Formal limiting API | Delivered 2026-07-16 as `ConvergenceHint` (solver-convergence-performance CP-12) |
| Breakpoint notifications | Already delivered (`Element::next_breakpoints`, TR-BDF2 engine) |
| Opvar catalog | Already delivered (`QueryDescriptor` + `list_queries`) |
| Parameter invalidation rules | Already delivered (`Invalidation` enum on `set_param`) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|-----------------------|----------------|-----------|------------|
| Merged feature name | `solver-abi` | Covers both sides of the ABI | y (user, 2026-07-16) |
| Public surface shape | Two tiers: `prelude` (host) + `abi` (device author) | Host and device author are different audiences; codegen/bench import device-author types (`Stamp`, `CircularArrayBuffer2`, `EvalCtx`, analysis states) that don't belong in a host prelude | y (user, 2026-07-16) |
| OSDI scope | Everything except model/instance separation | ModelHandle has no in-repo consumer; speculative API | y (user, 2026-07-16) |
| `add_node` return type | `AnalogReference` (not bare `NodeIdentifier`) | Elements are constructed from `AnalogReference`s; returning the reference makes the builder usable without a second lookup | y (design review) |
| `Solver::build()` vs existing per-analysis `init_global` calls | Keep the (idempotent, `Once`-guarded) calls inside `DcSolver::new` etc.; `Solver::build()` adds the MD-06 entry point | Removing constructor calls would break direct `circuit.dc(ctx)` usage (bench path); `Once` makes double-call free | y (design review) |
| Evaluation loops after the scheduler split | Stay methods on `DigitalState`, defined in a second `impl DigitalState` block inside `scheduler.rs` | Satisfies both MD-13 rule 2 (no free fns) and rule 4 (scheduler.rs holds only evaluation) without changing any call site | y (design review) |
| `destroy` wiring | `impl Drop for CircuitInstance` calls `destroy` on every element | Deterministic teardown for FFI-backed elements (OSDI wrappers) without asking hosts to remember a call | y (design review) |
| New `TerminalDescriptor` fields breaking construction sites | Acceptable — construction sites are crate-internal + tests; provide `TerminalDescriptor::new(name, domain, direction)` with defaults (`required: true`, `discipline: None`, `sign: IntoTerminal`) | Struct-literal sites migrate to the constructor | y (design review) |
| Per-source noise reporting granularity | Per (element name, source name): integrated contribution over the sweep + per-frequency PSD available | Matches "not only total PSD" without redesigning the sweep loop | y (design review) |

**Open questions:** none — all resolved or logged above.

### Implicit-requirement dimensions sweep (Large)

| Dimension | Resolution |
|-----------|------------|
| Input validation & bounds | ABI-07 AC4 (duplicate node name), ABI-09 AC3 (allocation after freeze = programming error, panics with message) |
| Failure / partial-failure states | ABI-01 AC4 (`setup` error aborts analysis construction, propagates `Error`) |
| Idempotency / retry / duplicate handling | ABI-01 AC2 (`init_global` once), ABI-07 AC2 (`add_ground` idempotent), ABI-08 AC2 (`setup` once per element) |
| Auth boundaries & rate limits | N/A because this is an in-process library ABI — no callers to authenticate |
| Concurrency / ordering | ABI-08 AC2 (setup ordering: before first assemble); `init_global` already `Once`-guarded. N/A beyond that: solver is single-threaded per instance |
| Data lifecycle / expiry | ABI-08 AC4 (`destroy` on drop, exactly once) |
| Observability | N/A because no new runtime loops are added — existing `SolverStats`/`tracing` unchanged |
| External-dependency failure | N/A because no external services; FFI teardown covered by `destroy` |
| State-transition integrity | ABI-08 AC1-AC2 (element lifecycle order: construct → allocate_unknowns → setup → loads → destroy); ABI-09 AC3 (no allocation after freeze) |

---

## User Stories

### P1: Two-tier public surface (`prelude` + `abi`) ⭐ MVP

**User Story**: As a host library user I want `piperine_solver::prelude::*` to
be everything I need to build, run, and read; as a device author I want
`piperine_solver::abi::*` to be everything I need to implement `Element` —
without either audience importing internal module paths.

**Why P1**: Every other story exports through these two modules; the privacy
flip is the load-bearing change with the largest downstream blast radius —
land it first, everything else is additive.

**Acceptance Criteria**:

1. WHEN `lib.rs` is inspected THEN the only `pub mod` items SHALL be
   `prelude` and `abi`; every other module SHALL be `pub(crate) mod`; the
   crate root SHALL re-export `pub use prelude::*;`.
2. WHEN a host writes only `use piperine_solver::prelude::*;` THEN it SHALL
   compile code that: builds a circuit via `CircuitBuilder`, constructs
   `Solver`, runs `dc`/`tran`/`ac`/`noise`/`tf`, and reads results
   (`DcAnalysisResult::get_net`, `TransientAnalysisResult`, `SolverStats`,
   `Net`, `Error`).
3. WHEN a device author writes only `use piperine_solver::abi::*;` THEN it
   SHALL compile an `Element` implementation that stamps
   (`Stamp`, `AnalogReference`), reads solution history
   (`CircularArrayBuffer2`, `DcAnalysisState`, `TransientAnalysisState`,
   `TransientAnalysisContext`, `AcAnalysisContext`), participates in digital
   evaluation (`EvalCtx`, `EventSink`, `DigitalPorts`, `DigitalNet`,
   `LogicValue`, `DigitalEvent`), reports noise (`Noise`), and exposes
   introspection (`ParamDescriptor`, `QueryDescriptor`, `TerminalDescriptor`,
   `Invalidation`, `Value`).
4. WHEN `piperine-codegen`, `piperine-bench`, `piperine-python`, and the
   solver's own `tests/` are grepped for `piperine_solver::` THEN zero
   matches SHALL reference `core::`, `math::`, `digital::`, `analog::`,
   `analysis::`, `solver::`, `result::`, or `error::` paths — only
   `piperine_solver::prelude`, `piperine_solver::abi`, or bare
   `piperine_solver::{Item}` (crate-root re-exports).
5. WHEN `cargo build --workspace` runs THEN it SHALL emit zero warnings.

**Independent Test**: A new `tests/prelude_surface.rs` in piperine-solver
imports ONLY `prelude::*` and runs the full host flow; a new test element in
`tests/abi_surface.rs` imports ONLY `abi::*` and implements `Element`.

---

### P1: `Solver` entry point (`build()` triggers `init_global`) ⭐ MVP

**User Story**: As a host library user, I want `Solver::new(circuit).build()`
to initialize tracing/faer exactly once and hand me all five analyses, so I
never call `Context::init_global()` manually.

**Acceptance Criteria**:

1. WHEN `Solver::new(circuit).build()` is called for the first time in a
   process THEN `Context::init_global()` SHALL have run (tracing + faer
   parallelism initialized) exactly once.
2. WHEN a second `Solver` is built in the same process THEN `init_global`'s
   body SHALL NOT run again (guarded by the existing `std::sync::Once`) and
   the call SHALL NOT panic.
3. WHEN `solver.dc()` / `solver.tran()` / `solver.ac(sweep)` /
   `solver.noise(opts)` / `solver.tf(opts)` are called THEN each SHALL return
   the corresponding analysis solver (`DcSolver`, `TransientSolver`,
   `AcSolver`, `NoiseSolver`, `TransferFunctionSolver`) configured with the
   `Solver`'s `Context` and `Policy`.
4. WHEN an element's `setup` returns an error during analysis construction
   THEN the analysis constructor SHALL return that `Error` (fail loud, no
   partial setup).

**Independent Test**: Build two `Solver`s from two circuits in one test
process; run DC on both; assert the second build does not panic and both
results are correct (voltage-divider expected values, tolerance ±1e-9).

---

### P2: `digital/scheduler.rs` split by system function

**User Story**: As a solver developer, I want `DigitalTopology`,
`DigitalState`, and the evaluation loops in separate files so I find each
concept at a glance (MD-13 rule 4).

**Acceptance Criteria**:

1. WHEN `digital/topology.rs` is opened THEN it SHALL contain exactly
   `DigitalTopology` + its `build` method (moved verbatim from
   `scheduler.rs:19-99`).
2. WHEN `digital/state.rs` is opened THEN it SHALL contain the `Checkpoint`
   struct and `DigitalState` with its data/lifecycle methods (`new`,
   `with_labels`, `set_label`, `label_or_default`, `schedule`,
   `peek_next_event_time`, `checkpoint`, `rollback`, `commit` — moved
   verbatim from `scheduler.rs:9-17,101-183`).
3. WHEN `digital/scheduler.rs` is opened THEN it SHALL contain only a second
   `impl DigitalState` block with `evaluate_until_stable` and
   `evaluate_dag_ordered` (no free functions — MD-13 rule 2) and SHALL be
   under 250 lines.
4. WHEN `cargo test -p piperine-solver` runs after the split THEN every
   existing test SHALL pass without modification to test bodies (import
   adjustments only).

**Independent Test**: `tests/digital_topology.rs` and `tests/mixed_signal.rs`
pass unchanged (assertions untouched).

---

### P2: `as_iv` decoupled from `Netlist`

**User Story**: As a solver developer, I want the analysis layer to not
depend on `Netlist` so results are purely about values.

**Acceptance Criteria**:

1. WHEN `DcAnalysisResult::as_iv` is called THEN its signature SHALL be
   `pub fn as_iv(&self, circuit: &CircuitInstance) -> Vec<InitialValue<AnalogReference, f64>>`
   and it SHALL read `circuit.netlist()` internally.
2. WHEN `analysis/dc.rs` is grepped for `Netlist` THEN the only permitted
   match SHALL be inside `as_iv`'s implementation via
   `circuit.netlist()` — no `use crate::analog::...Netlist` import of the
   type in a public signature.
3. WHEN the transient driver computes initial conditions THEN
   `compute_initial_conditions` SHALL pass `&*self.system.circuit` (the
   circuit, not the netlist) and produce identical `iv` vectors (existing
   tran tests pass, results ±1e-9).

---

### P2: `CircuitBuilder`

**User Story**: As a host library user, I want a builder for circuit assembly
so I never call `Netlist::connect_node` manually.

**Acceptance Criteria**:

1. WHEN `CircuitBuilder::new("top")` is called THEN it SHALL return a builder
   with an empty netlist, zero elements, zero digital nets, and no ground.
2. WHEN `.ground()` is called THEN it SHALL return the ground
   `AnalogReference`; a second call SHALL return an equal reference without
   allocating (idempotent).
3. WHEN `.node("out")` is called THEN it SHALL allocate one analog node and
   return its `AnalogReference`; WHEN `.node("out")` is called a second time
   with the same name THEN it SHALL return the same reference without
   allocating a second unknown (lookup, not duplicate).
4. WHEN `.digital_net(Some("clk"))` is called THEN it SHALL return a
   `DigitalNet` whose index increments per call and whose label is
   registered.
5. WHEN `.element(boxed_element)` is called THEN the builder SHALL store it
   in insertion order.
6. WHEN `.build()` is called THEN it SHALL return a `CircuitInstance` on
   which: `allocate_unknowns` has run for every element (ABI-09),
   `rebuild_digital_topology()` has run, `init_digital()` has run, and the
   digital net count equals the number of `.digital_net` calls.
7. WHEN `.build()` is called without `.ground()` THEN it SHALL succeed (pure
   digital circuits have no ground).

**Independent Test**: Build a two-resistor divider (V source element + 2
resistors from the test-double set), run DC through `Solver`, assert the
midpoint voltage ±1e-9.

---

### P2: Element lifecycle — `setup` / `destroy`

**User Story**: As a device author (including the external OSDI wrapper), I
want explicit setup and teardown hooks so FFI-backed models can bind
resources before the first load and release them deterministically.

**Acceptance Criteria**:

1. WHEN the first analysis solver is constructed over a `CircuitInstance`
   THEN `setup(&mut self, ctx: &Context) -> Result<()>` SHALL have been
   called exactly once on every element, before the first `assemble` (i.e.
   before the Newton dry-run in `NewtonRaphsonSolver::new`).
2. WHEN a second analysis is constructed over the same `CircuitInstance`
   THEN `setup` SHALL NOT be called again (once per circuit lifetime).
3. WHEN any element's `setup` returns `Err` THEN the analysis constructor
   SHALL return that error and no `assemble` SHALL run.
4. WHEN the `CircuitInstance` is dropped THEN `destroy(&mut self)` SHALL be
   called exactly once on every element.
5. WHEN an element does not override the hooks THEN behavior SHALL be
   identical to today (defaults: `setup` returns `Ok(())`, `destroy` no-op).

**Independent Test**: Test element with call counters; construct DC then AC
over one circuit → `setup_calls == 1`; drop circuit → `destroy_calls == 1`.

---

### P2: Internal-unknown allocation API

**User Story**: As a device author, I want to request internal MNA unknowns
(auxiliary branch currents, hidden states) through a formal seam before the
matrix shape freezes, instead of hand-wiring them into the netlist.

**Acceptance Criteria**:

1. WHEN `CircuitBuilder::build()` runs THEN it SHALL call
   `allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>)` exactly
   once per element, in insertion order, before constructing the
   `CircuitInstance`.
2. WHEN an element calls `alloc.branch("v1", "flow")` THEN the allocator
   SHALL register a branch unknown in the netlist and return its
   `AnalogReference`; the MNA size (`netlist.max_index()`) SHALL grow by
   exactly 1.
3. WHEN an element allocated unknowns THEN its `capabilities()` SHALL
   contain `HAS_INTERNAL_UNKNOWNS` — asserted by `build()`, which SHALL
   return an `Error` (domain `SolverDomain::Circuit` or existing closest
   domain) naming the offending element if the flag is missing.
4. WHEN an element does not override `allocate_unknowns` THEN nothing SHALL
   change (default no-op; existing manual assembly paths untouched).

**Independent Test**: Test element allocating one branch; build via
`CircuitBuilder`; assert matrix size = nodes + 1 and DC solves with the
branch row present.

---

### P2: Rich terminal descriptors

**User Story**: As a host/tooling author, I want terminal metadata to carry
discipline and sign convention so external-model wrappers (OSDI) can map
terminals without special-casing.

**Acceptance Criteria**:

1. WHEN `TerminalDescriptor` is inspected THEN it SHALL carry
   `discipline: Option<String>` (e.g. `"electrical"`) and
   `sign: SignConvention` where
   `enum SignConvention { IntoTerminal, OutOfTerminal }`.
2. WHEN `TerminalDescriptor::new(name, domain, direction)` is called THEN it
   SHALL default `required: true`, `discipline: None`,
   `sign: SignConvention::IntoTerminal` (the OSDI convention: positive
   current flows into the terminal).
3. WHEN existing construction sites are migrated to the constructor THEN
   `cargo test --workspace` SHALL stay green.

---

### P2: Named noise sources + per-source reporting

**User Story**: As a circuit designer, I want to know which noise source
dominates the output, not just the total PSD.

**Acceptance Criteria**:

1. WHEN `Noise` is inspected THEN it SHALL carry
   `name: Option<String>` and `kind: NoiseKind` where
   `enum NoiseKind { Thermal, Shot, Flicker, Other }`; a
   `Noise::new(terminals, value)` constructor SHALL default
   `name: None, kind: NoiseKind::Other` so existing emitters compile with
   one-line changes.
2. WHEN a noise analysis completes THEN `NoiseAnalysisResult` SHALL expose
   `contributions()` returning, per `(element_name, source_name)` pair, that
   source's integrated output-referred contribution over the sweep, where
   unnamed sources use the source's index as `source_name` (`"0"`, `"1"`, …).
3. WHEN the per-source contributions are summed at any frequency THEN the sum
   SHALL equal the total output PSD at that frequency within reltol 1e-9
   (conservation check).
4. WHEN existing noise tests (`08_johnson_noise` example, bench noise tests)
   run THEN total PSD values SHALL be identical to baseline (±1e-12).

---

### P3: Stamp-capability declaration

**User Story**: As a solver developer, I want elements to declare stamp
properties (linear, charge, analytic Jacobian) so follow-up optimizations
(bypass fast path, single-LU linear solve) can plan without probing.

**Acceptance Criteria**:

1. WHEN `ElementCapabilities` is inspected THEN it SHALL define three new
   flags with doc comments: `LINEAR` (stamps are solution-independent),
   `STAMPS_CHARGE` (contributes a charge/`ddt` part), `ANALYTIC_JACOBIAN`
   (Jacobian is exact, not finite-difference).
2. WHEN the JIT-compiled PHDL device (`PiperineDevice`) declares capabilities
   THEN it SHALL set `ANALYTIC_JACOBIAN` always (symbolic differentiation)
   and `STAMPS_CHARGE` when the kernel has a `Q(V)` part.
3. WHEN no solver code consults the three flags THEN that SHALL be documented
   on the flags themselves ("declaration consumed by solver-performance
   follow-up") — declaration-only is the scoped deliverable (MD-11:
   checklist, not behavior).

---

## Edge Cases

- WHEN `CircuitBuilder::build()` is called with zero elements THEN it SHALL
  return a valid empty `CircuitInstance` (DC on it yields an empty result,
  no panic).
- WHEN `.node("gnd")` is called (a ground-family name) THEN the builder SHALL
  route it to the ground reference, consistent with the netlist's
  gnd-family handling (`gnd/GND/vss/VSS`).
- WHEN an element's `destroy` panics during drop THEN the drop impl SHALL NOT
  abort the process beyond normal panic semantics (no `catch_unwind`
  machinery — document that `destroy` must not panic).
- WHEN the scheduler split lands THEN `digital/mod.rs` re-exports SHALL keep
  `DigitalState`, `DigitalTopology`, `DigitalEvent`, `DigitalNet`,
  `LogicValue` importable from `crate::digital` (internal paths unchanged
  inside the crate).
- WHEN `allocate_unknowns` is (wrongly) called after `build()` froze the
  matrix THEN there SHALL be no API path to do so — `UnknownAllocator` is
  constructed only inside `build()` and not exported in `prelude`/`abi`
  (compile-time prevention, not runtime check).

---

## Requirement Traceability

| ID | Story | Supersedes | Status |
|----|-------|-----------|--------|
| ABI-01 | P1 Solver entry point AC1-AC4 | LIB-01 | In Tasks (T14) |
| ABI-02 | P1 two-tier AC2 — prelude host-complete | LIB-02 | In Tasks (T6) |
| ABI-03 | P1 two-tier AC3 — abi device-author-complete | (new) | In Tasks (T2) |
| ABI-04 | P1 two-tier AC1, AC4 — internals pub(crate), downstream migrated | LIB-02 | In Tasks (T3,T4,T5,T6) |
| ABI-05 | P2 scheduler split AC1-AC4 | LIB-03 | In Tasks (T1) |
| ABI-06 | P2 as_iv AC1-AC3 | LIB-04 | In Tasks (T7) |
| ABI-07 | P2 CircuitBuilder AC1-AC7 | LIB-05 | In Tasks (T9) |
| ABI-08 | P2 lifecycle AC1-AC5 | OSDI-01 (partial) | In Tasks (T8) |
| ABI-09 | P2 internal unknowns AC1-AC4 | OSDI-01 (partial) | In Tasks (T9) |
| ABI-10 | P2 terminal descriptors AC1-AC3 | OSDI-01 (partial) | In Tasks (T10) |
| ABI-11 | P2 noise metadata AC1-AC4 | OSDI-03 | In Tasks (T11,T12) |
| ABI-12 | P3 stamp capabilities AC1-AC3 | OSDI-01 (partial) | In Tasks (T13) |
| ABI-13 | — build zero warnings + `cargo test --workspace` green + 21/21 examples | LIB-06 / OSDI-04 | In Tasks (T15) |

Superseded IDs from the two merged specs: LIB-01..06, OSDI-01..04.
OSDI-02 (invalidation rules) was already delivered pre-merge (`Invalidation`
enum) — carried as done, no task.

**Coverage:** 13 total, 13 mapped to tasks ✅ (T1–T15, see `tasks.md`)

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

---

## Success Criteria

- [ ] `tests/prelude_surface.rs` compiles + passes importing only
      `piperine_solver::prelude::*`.
- [ ] `tests/abi_surface.rs` compiles + passes importing only
      `piperine_solver::abi::*`.
- [ ] `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-codegen crates/piperine-bench crates/piperine-python crates/piperine-solver/tests`
      returns zero matches.
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace`
      green (≥ 392 tests, plus the new surface/lifecycle/noise tests);
      21/21 `examples/*.py` pass via `piperine run` (rebuild the ROOT
      binary: `cargo build -p piperine`).
- [ ] Noise conservation: per-source contributions sum to total PSD (±1e-9
      reltol) on the Johnson-noise example.

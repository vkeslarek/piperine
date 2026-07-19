# solver-simplification Specification

Simplify `crates/piperine-solver` — code **and** mental model — so the whole
solver reads as a layered specification of *what* it does, with uniform
contracts, minimal special cases, and clear per-layer responsibilities. This is
a **behavior-preserving** refactor: every existing analysis result stays
bit-identical; the deliverable is a smaller, more obvious architecture, not new
capability.

**Sequencing:** stands alone on `feature/bench-removal`; no dependency on other
pending solver specs. Where it overlaps `solver-strategy-composition` and
`solver-library-abi` (both planning-only), it *closes* their open items rather
than deferring — see Requirement Traceability.

## Problem Statement

The solver is correct and well-tested (509 green, Verifier-PASS on
`p1-solver-complete`) but has accreted structure that taxes the reader:

- **Two files per analysis.** `analysis/<x>.rs` (per-analysis state/options/
  result) and `solver/<x>.rs` (the driver) share names, so understanding one
  analysis end-to-end means opening two parallel trees. The boundary between
  them is real (data contract vs. driver) but under-named and under-documented.
- **One 40-method flat `Element` trait** mixes four unrelated concerns — analog
  loading, digital evaluation, introspection, integration feedback — behind one
  wall of methods. A reader cannot see the concerns; they must know which
  methods group.
- **A 310-line `transient::solve()`** (`solver/transient.rs:394–705`) carries
  the whole time-loop: predictor, breakpoint merge, LTE, accept/reject,
  snapshot, digital settle — one method, no sub-structure.
- **Hidden constants.** Homotopy schedule tunables live as inline literals
  inside `GminStepping`/`SourceStepping` (`0.1`, `1.3`, `3.0`, `/8.0`, `200`,
  `300`, `knee_gmin=1e-6`) despite `PlanLimits` existing precisely to be "one
  place to look for the solver's hidden constants." Trace toggles hide in
  `PIPERINE_TRACE_*` env vars.
- **Write-only / dangling contract surface.** `ElementCapabilities::LINEAR`
  (never produced, never read), `ANALYTIC_JACOBIAN`/`STAMPS_CHARGE` (produced by
  codegen + asserted in tests, but **no solver consumer**), and a
  `SUPPORTS_ROLLBACK` doc block promising `checkpoint_state`/`rollback_state`/
  `commit_state` methods that **do not exist on the trait**. Each is a point to
  track with nothing on the other end.
- **Stale project memory.** `STATE.md` MD-05 records `NewtonStrategy`/
  `StepperStrategy` as "pending"; both shipped and are wired (`DampedNewton`,
  `PiController`). The map disagrees with the territory.

Each item is a place where the architecture is harder to hold in the head than
the work it does. None is a bug; all are simplification debt.

## Goals

- [ ] **One place per analysis.** A reader opens exactly one module to see an
      analysis's request shape, its driver, and its result. The data-vs-driver
      layer boundary is either dissolved or renamed to function-named modules
      that state what each layer *contains* and *must not contain*.
- [ ] **`Element` reads as composed concerns.** The single object is preserved
      (no downcast, MD-01 spirit), but its surface is decomposed into
      concern-scoped supertraits so each concern is separately legible.
- [ ] **No method over ~60 lines in a driver** without named sub-steps; the
      transient time-loop is decomposed into named phases.
- [ ] **One config home.** Every solver tunable (tolerances, plan limits,
      homotopy schedules, trace flags) is reachable through a typed config
      surface — zero behavior-affecting magic literals inside strategy/driver
      bodies.
- [ ] **Every contract has both ends.** Every capability flag has a producer
      *and* a consumer, or it is gone; every documented method exists.
- [ ] **Cohesive circuit object.** `CircuitInstance`'s surface is grouped into
      named responsibilities; the mixed-signal seam has one home; the dead
      `math/unit.rs` `f64`-alias module is gone.
- [ ] **Map matches territory.** `STATE.md` decisions reflect shipped reality;
      each module carries a one-line responsibility contract.
- [ ] **One canonical source.** `docs/spec/part_vii_solver.md` is rewritten to
      match the built solver exactly — every algorithm and contract, internally
      consistent, no phantom constructs (the finalization deliverable).
- [ ] Full suite stays green, zero warnings, ngspice live — **bit-identical
      numerics** on every existing analysis.

## Out of Scope

| Item | Reason |
|------|--------|
| New analyses, operators, or models | Behavior-preserving refactor only |
| Numeric-behavior changes (tolerances, schedules retuned) | Constants move to config at their *current* values; retuning is a separate perf feature |
| Solver performance work (bypass/predictor extensions) | `solver-performance` owns it; this feature only *removes* its dead flag reservations |
| Codegen / plugin internal restructure | Only the cross-crate `Element` surface changes; producers adapt at the seam |
| OSDI ABI completion, commit/rollback lifecycle | `solver-osdi-abi-completion` / `solver-commit-rollback` own these; this feature deletes the *phantom* rollback doc, not the future work |

## Assumptions & Open Questions

| Decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Execution depth | Specify + Design, then stop for user approval before Tasks/Execute | 8.8k-LOC refactor; user sees the whole plan first | **y (user, 2026-07-18)** |
| `Element` shape | Decompose into composed supertraits (`Element: AnalogDevice + DigitalDevice + Introspect + …`); one object, capabilities still gate, no downcast | Each concern legible separately without a facet/downcast split; downcast would block a future C-style ABI | **y (user, 2026-07-18)** — **MD-01 amendment**, locked |
| Module layout | **Scheme B** — co-locate each analysis (data + driver) in one file under `analyses/`, split as documented in-file sections; shared machinery in `analyses/convergence.rs` | User: "sem mazes, colocalização fica mais organizado"; avoids the `contracts/` name | **y (user, 2026-07-18)** |
| Dead capability flags | **Remove** `LINEAR`, `ANALYTIC_JACOBIAN`, `STAMPS_CHARGE` + codegen producers + test asserts; delete the phantom `SUPPORTS_ROLLBACK` method doc | ROADMAP Performance section is delivered; no planned consumer for these flags | **y (user, 2026-07-18)** — "weigh against roadmap; if no use, remove" → no use found |
| `SignalBridge` empty struct | **Fold** its two methods into `CircuitInstance` as the named mixed-signal-seam methods; delete the zero-field struct | It is accept-hook→queue plumbing, not a mixed-signal device path; any Element is natively mixed-signal (MD-01), so a hollow "future home" struct earns nothing | **y (user, 2026-07-18)** |
| `math/unit.rs` | **Remove** the module; the 22 aliases are all `= f64` with zero type safety (a remnant of the abandoned typed-units ambition); inline `f64` at the ~38 use-sites, including the `Second` ABI surface | Dead abstraction: module + import noise for no compile-time benefit | **y (user, 2026-07-18)** |
| `CircuitInstance` contract | **Split** the 24-method god-object into cohesive responsibilities (§ design): construction stays in `CircuitBuilder`; the runtime keeps circuit-state accessors; the analysis-factory methods and the mixed-signal seam get clear homes | Its contract is "all over the place" (user); one struct spans 5 concerns | **y (user, 2026-07-18)** |
| Discrimination for a refactor | The existing 509-test behavioral suite is the invariance oracle; add targeted **contract tests** (every flag has a consumer; every config value reaches its schedule; layer-boundary assertions) | Mutation testing a pure refactor is low-signal; behavioral parity + contract assertions are the real gate | n (design) |

**Open questions:** none — all resolved. `SignalBridge` fold and module scheme
are locked (above).

**Implicit-dimension sweep:** input validation — unchanged (same loud errors);
failure states — unchanged (`SolverDomain`); idempotency/auth/concurrency —
N/A (single-threaded, pure); observability — trace toggles move from env vars
into typed config (net improvement); data lifecycle — N/A; external-dep —
ngspice harness unchanged; state-transition — the refactor must preserve every
analysis's state machine exactly (proven by parity).

## User Stories

### SS-A: One module per analysis ⭐

**Story:** As a solver maintainer, I want to read one analysis end-to-end in one
place, so I can reason about it without cross-referencing a parallel tree.

**Acceptance Criteria:**

1. WHEN a maintainer opens the module for any analysis (dc, ac, tran, noise,
   tf, sens, pss) THEN its request types, driver, and result types SHALL be
   reachable from that one module (either co-located or via a documented,
   function-named layer split — never two same-named files in parallel dirs
   with an unstated boundary).
2. WHEN the module tree is glanced at (MD-13 rule 4) THEN every directory name
   SHALL state a **system function**, not a language construct; no directory is
   named `contracts`, `traits`, `models`, or `utils`.
3. WHEN the split is retained THEN each layer SHALL carry a one-line
   responsibility contract naming what it **contains** and what it **must not
   contain**, and no type SHALL violate its layer's contract.

### SS-B: Element as composed concerns ⭐

**Acceptance Criteria:**

1. WHEN `Element`'s surface is read THEN it SHALL be composed of
   concern-scoped supertraits (analog device behavior, digital device behavior,
   introspection, integration feedback) such that each concern is a separate,
   independently-documented trait, `Element` being their conjunction.
2. WHEN the solver dispatches to any element THEN it SHALL do so through one
   object with **no downcast and no `Any`** (MD-01 spirit preserved); capability
   flags still gate which concerns run.
3. WHEN the workspace builds THEN every existing element (codegen devices,
   plugins, test doubles) SHALL implement the composed surface with no behavior
   change; the full suite stays green.

### SS-C: Decomposed transient time-loop ⭐

**Acceptance Criteria:**

1. WHEN `transient::solve()` is read THEN the time-loop SHALL be expressed as
   named phases (e.g. predict → propose-dt → step → assess-lte → accept/reject
   → snapshot → settle-digital), each an owned method, with no single method
   over ~60 lines carrying the whole loop.
2. WHEN the transient suite runs (incl. breakpoints, UIC, PSS shots, coupled-LC,
   mixed-signal) THEN results SHALL be bit-identical to pre-refactor.

### SS-D: One config home, zero magic literals ⭐

**Acceptance Criteria:**

1. WHEN a homotopy strategy (`GminStepping`, `SourceStepping`) or the
   `PiController` runs THEN every behavior-affecting numeric (initial/decade
   step, back-off factors, iteration caps, `knee_gmin`, reject divisor, growth
   factor, clamps) SHALL be read from a typed config struct with named,
   documented fields — not a literal in the method body.
2. WHEN a host wants to tune a schedule or enable tracing THEN it SHALL do so
   through the typed config surface; `PIPERINE_TRACE_*` env-var toggles SHALL be
   replaced by (or backed by) a typed trace/verbosity config.
3. WHEN the config defaults are applied THEN every value SHALL equal today's
   literal exactly (parity: numerics unchanged).

### SS-E: Every contract has both ends ⭐

**Acceptance Criteria:**

1. WHEN `ElementCapabilities` is enumerated THEN every flag SHALL have both a
   producer and a solver consumer; `LINEAR`, `ANALYTIC_JACOBIAN`, and
   `STAMPS_CHARGE` (no consumer) SHALL be removed along with their codegen
   producers and test assertions.
2. WHEN `Element`'s docs reference a method THEN that method SHALL exist; the
   `SUPPORTS_ROLLBACK` block referencing non-existent `checkpoint_state`/
   `rollback_state`/`commit_state` SHALL be corrected (flag + doc removed, or
   the flag documented as reserved with no method promise — design decides,
   defaulting to removal per the roadmap check).
3. WHEN the `SignalBridge` empty struct is reviewed THEN it SHALL either gain
   real owned state or be folded back into its caller — no zero-field
   "future home" struct remains.

### SS-G: `CircuitInstance` contract, cohesive ⭐

**Story:** As a maintainer, I want the runtime circuit object to have one clear
job, so I do not meet construction, seven analysis factories, the mixed-signal
seam, and param-mutation in one 24-method wall.

**Acceptance Criteria:**

1. WHEN `CircuitInstance` is read THEN its surface SHALL be grouped into named
   responsibilities — (a) circuit-state accessors (`netlist`, `nets`,
   `capabilities`, device access), (b) analysis entry points, (c) the
   mixed-signal seam (digital init/settle/run), (d) live param mutation — each
   with a stated contract; no method SHALL belong to none of them.
2. WHEN the mixed-signal seam runs THEN the folded `SignalBridge` methods SHALL
   live here as named seam methods (SS-12), with the analog→digital plumbing in
   one place, not a hollow struct.
3. WHEN construction is done THEN it SHALL remain the `CircuitBuilder`'s job;
   `CircuitInstance` SHALL NOT grow ad-hoc constructors beyond the builder's
   output and documented re-entry (`with_initial_state`, restamp).
4. WHEN the analysis factories are called THEN behavior SHALL be unchanged
   (same solvers, same results); this is a surface-organization change, not a
   semantics change.

### SS-H: Remove the dead units module ⭐

**Acceptance Criteria:**

1. WHEN the workspace is grepped THEN `math/unit.rs` SHALL be gone and its
   `f64` aliases (`Volt`, `Ohm`, `Second`, `Siemens`, …) SHALL be replaced by
   `f64` at every use-site, including the `abi` re-export of `Second`.
2. WHEN the build runs THEN it SHALL be green with zero warnings and no
   remaining reference to `crate::math::unit`.
3. WHEN numerics run THEN results SHALL be bit-identical (aliases were `f64`;
   this is a pure name removal).

### SS-I: Part VII is the canonical solver source ⭐ (finalization)

**Story:** As anyone extending, modifying, or verifying the solver, I want
`docs/spec/part_vii_solver.md` to be the one complete, consistent, beautiful
source of everything the solver does — every algorithm and every contract — so
the code and the spec never disagree again.

**Acceptance Criteria:**

1. WHEN Part VII is read after this feature THEN it SHALL describe the solver as
   actually built at feature close: the composed-supertrait `Element` surface
   (SS-B), the `analyses/` module layout and layer contracts (SS-A), the config
   home (SS-D), the folded mixed-signal seam (SS-G), and the removal of the
   dead flags/units/methods (SS-E, SS-H) — with **no reference to a removed or
   phantom construct**.
2. WHEN each §-section (Circuit, Element ABI, DC, transient, AC, noise, TF,
   mixed-signal, convergence) is cross-checked against the code THEN every
   algorithm, contract, and failure rule stated SHALL match the implementation
   (a stated-vs-code audit, one section at a time).
3. WHEN a reader wants the full picture THEN Part VII SHALL be internally
   consistent (one vocabulary, one naming scheme matching the code) and
   complete (every public solver contract and every analysis algorithm present)
   — the finalization gate of this feature.

### SS-F: Map matches territory ⭐

**Acceptance Criteria:**

1. WHEN `STATE.md` is read THEN MD-05 SHALL reflect that `NewtonStrategy`/
   `StepperStrategy` are shipped and wired; any MD amended by this feature
   (MD-01 sub-trait decomposition, MD-07 already amended) SHALL be updated with
   an amendment line + date.
2. WHEN each solver module is opened THEN its top SHALL carry a one-line
   responsibility contract (module-level `//!` doc) consistent with the layer
   table in `design.md`.

## Edge Cases

- WHEN an element implements only one concern (pure resistor = analog only,
  pure gate = digital only) THEN the composed-trait surface SHALL still let it
  leave the other concerns at their defaults (no forced empty impls beyond
  today's).
- WHEN a plugin or OSDI wrapper (external crate) implements `Element` THEN the
  decomposition SHALL not force a source break beyond re-grouping impl blocks
  (design keeps a single `impl Element` ergonomic path or documents the split).
- WHEN config defaults are used THEN behavior SHALL be indistinguishable from
  the pre-refactor literals (parity tests pin representative circuits).

## Requirement Traceability

| Requirement ID | Story | Closes / relates | Status |
|---|---|---|---|
| SS-01 | SS-A one module per analysis | — | Planned |
| SS-02 | SS-A function-named modules + layer contracts | MD-13 rule 4 | Planned |
| SS-03 | SS-B composed supertraits, no downcast | MD-01 (amend) | Planned |
| SS-04 | SS-B all elements implement composed surface, green | — | Planned |
| SS-05 | SS-C transient loop decomposed into named phases | `solver-strategy-composition` | Planned |
| SS-06 | SS-C transient parity bit-identical | — | Planned |
| SS-07 | SS-D homotopy/stepper constants → typed config | rule 6; `PlanLimits` extension | Planned |
| SS-08 | SS-D trace toggles → typed config | — | Planned |
| SS-09 | SS-D config-default parity | — | Planned |
| SS-10 | SS-E dead flags removed (+ codegen + tests) | — | Planned |
| SS-11 | SS-E phantom rollback doc corrected | `solver-commit-rollback` | Planned |
| SS-12 | SS-E SignalBridge folded or filled | — | Planned |
| SS-13 | SS-F STATE.md MD-05 + amendments current | — | Planned |
| SS-14 | SS-F per-module responsibility contracts | MD-13 rules 3+4 | Planned |
| SS-15 | SS-G CircuitInstance grouped into named responsibilities | `solver-library-abi` | Planned |
| SS-16 | SS-G mixed-signal seam folded in, behavior unchanged | — | Planned |
| SS-17 | SS-H math/unit.rs removed, f64 inlined, parity | — | Planned |
| SS-18 | SS-I Part VII rewritten as canonical, code-consistent source | — | Planned |

## Success Criteria

- [ ] `cargo test --workspace` green, zero warnings; ngspice harness live — same
      counts as baseline (509 + any new contract tests), no analysis regressed.
- [ ] Representative parity circuits (divider op, clipper tran, coupled-LC,
      mixed-signal divider, diode DC) produce numerics bit-identical to the
      pre-refactor baseline.
- [ ] Every `ElementCapabilities` flag has a producer and a consumer.
- [ ] No behavior-affecting numeric literal remains inside a homotopy/stepper
      method body.
- [ ] Module tree glance-test: each dir/file name states a system function;
      each carries a one-line responsibility contract.
- [ ] `CircuitInstance` surface grouped by responsibility; `math/unit.rs` gone;
      `SignalBridge` folded.
- [ ] `STATE.md` decisions match shipped reality.
- [ ] `docs/spec/part_vii_solver.md` audited section-by-section against the
      code — canonical, complete, consistent.

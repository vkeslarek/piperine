# Element ABI Maturity Specification

> **Supersedes** `solver-commit-rollback` (stub, 4 ACs) and `solver-osdi-abi-
> completion` (stale goals — internal-unknown allocation shipped, model/instance
> rejected). Implements ROADMAP P2 "Element ABI maturity checklist" in full.

**Implements:** MD-01 (one Element ABI), MD-11 (OSDI as checklist), MD-12 (ABI
vs policy classification).

## Problem Statement

The `Element` trait is the single composed device contract
(`AnalogDevice + DigitalDevice + Introspect`), but the surface is incomplete in
ways that block a correct mixed-signal simulator and a clean external-device
authoring story:

1. **Correctness bug (rollback):** rejected transient steps restore the analog
   solution vector and the digital net array, but **not** device-internal state
   mutated during the failed attempt — the limiter (`active`, `seeds`, vold
   slots) and digital register banks (`vars_int`, `vars_real`, `prev_watch`)
   are left dirty. The solver calls no device hook on the reject path.
2. **Declared-but-unused seams:** `set_temperature` exists but is never called;
   `TerminalDescriptor` is rich but JIT devices return empty; `QueryDescriptor`
   exists but no JIT device exposes opvars; `Invalidation::Temperature` is
   declared but nothing drives it.
3. **Missing capability declarations:** old Jacobian/linear/charge flags were
   removed with no replacement; `.disto` silently skips devices without
   high-order kernels; there is no model type-id/version; the kernel's named
   catalogs don't reach the ABI.
4. **Ad-hoc event model:** four event sources (digital queue, analog
   breakpoints, scheduled live sets, `$bound_step`) are merged by hand in
   `predict_step` — no unified typed queue.
5. **Incomplete strategy composition:** `NewtonStrategy` + `HomotopyStrategy`
   are folded into `ConvergencePlan`; `StepperStrategy` is owned separately by
   the transient driver and its rejection logic is inline.

Every item below is V1 — the user's position is that a complete solver needs
the full maturity surface.

## Goals

- [ ] Rollback hooks: checkpoint/restore pair on `Element`, driven by the solver
      around every candidate step (transient reject + DC homotopy retry)
- [ ] Formal limiting API: replace `limiting_active: bool` +
      `convergence_hint: Option<…>` with a structured `LimitingReport`
- [ ] Lifecycle contract: one ordered chart per analysis, documented + enforced
      by an executable contract test
- [ ] Temperature protocol: nominal/instance/delta separation, `set_temperature`
      wired, invalidation rules honored
- [ ] Jacobian/stamp capability declaration: element advertises analytic/numeric/
      absent; analyses fail loud when they need what's missing
- [ ] Terminal/opvar catalogs bridged from kernel to ABI: JIT-compiled devices
      populate `list_terminals`/`read_opvars`/`list_queries` from kernel data
- [ ] Save/probe selection: devices declare observables + cost; host requests a
      subset; recording is per-observable, not all-or-nothing
- [ ] Unified event model: one typed queue for digital events, analog crossings,
      timers, `$bound_step` hints
- [ ] Strategy composition completed: `StepperStrategy` folded into
      `ConvergencePlan`; transient rejection is plan-composed, not inline
- [ ] Introspect leftovers: model descriptor (type id/version); kernel named
      catalogs surfaced through the ABI

## Out of Scope

Explicitly excluded — documented to prevent scope creep and re-litigation.

| Feature | Reason |
|---------|--------|
| **Noise metadata / per-source reporting** | **Fully delivered** — `Noise { name, kind }`, `NoiseContribution` with per-source PSD, conservation test at `noise.rs:437-502`. Not a gap. |
| **Parameter invalidation core** | **Delivered** — wired through `CircuitInstance::set_element_param`, DC restamp, transient `apply_scheduled_sets`, `.sens` rebuild gate, Python host auto-rebuild. Minor gap: generic sweep/optimizer consumers outside `set_element_param` — tracked as a minor follow-up, not a blocker. |
| **NewtonStrategy fold into ConvergencePlan** | **Done** — `ConvergencePlan::newton` field, `DampedNewton` wired. Only the Stepper half is open (Story 9). |
| **Internal-unknown allocation (`HAS_INTERNAL_UNKNOWNS`)** | **Delivered** — `allocate_unknowns` + builder check shipped via `solver-abi` feature. PHDL devices expand `wire` → anonymous nets; plugin Elements allocate through the standard path. |
| **Model/instance separation** | **Rejected** (user 2026-07-16) — SPICE concept; Piperine is HDL-centric (module = model). The OSDI wrapper handles model/instance internally. |
| **Commit/rollback for analog accept-gated state** | Operators, event detectors, `last_volts` are mutated only in `accept_timestep` — naturally safe. No checkpoint needed. |
| **Full Monte-Carlo / large-scale sweep harness** | The invalidation plumbing is done; a generic harness that threads `set_element_param` is a P6 (optimizer) consumer. |

---

## Assumptions & Open Questions

Every ambiguity is resolved or recorded here — nothing is left silently unclear.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Rollback hook shape | Symmetric `checkpoint_state`/`restore_state` pair on `Element`; solver calls before each attempt, `restore_state` on reject | Follows the existing `digital_hidden_snapshot`/`restore` pattern; general (any state, not just digital); opt-in (default None = zero cost) | y (user) |
| Checkpoint carrier type | Opaque owned `ElementCheckpoint` (newtype over `Vec<u8>` or `(Vec<i64>, Vec<f64>)`) — same shape as `digital_hidden_snapshot` carrier | Devices pack whatever they need; solver is type-agnostic | n (Design) |
| Multiple rejects in a row | Checkpoint is taken once before each attempt; restore called per reject. The "last accepted" checkpoint is the source of truth. | Matches current `DigitalState::checkpoint` (one-deep) semantics | y (code — `Checkpoint` is one-deep `Option`) |
| PSS re-entry vs per-step rollback | `digital_hidden_snapshot`/`restore` (PSS recording) stays as-is; the new `checkpoint_state`/`restore_state` is the per-step mechanism. If the carrier shapes are compatible, unify in Design. | PSS recording runs every step (into `TransientStep`); rollback runs on reject only. Different lifecycles — separate until proven composable. | n (Design) |
| DC homotopy retry rollback | `checkpoint_state` called before each homotopy attempt; `restore_state` on strategy-fallthrough. Same hook as transient. | DC Newton failures dirty the limiter too — same correctness concern. | n (Design) |
| Formal limiting API replaces both | `LimitingReport { proposed, limited, limiter_name, reason }` replaces `limiting_active: bool` AND `convergence_hint: Option<ConvergenceHint>` | User confirmed: formalize alongside rollback | y (user) |
| `.disto` fail-loud for absent kernels | When `.disto` runs and NO device contributes (all return `None`), the analysis SHALL warn or fail with a named diagnostic — not silently return zero HD2/HD3 | "Fail loud" is the project bar; today `.disto` silently `continue`s on `None` (`disto.rs:375`) | n (Design — warn vs fail) |
| Jacobian capability granularity | A new `ElementCapabilities` bit or a descriptor enum (`Analytic`/`Numeric`/`None`) — not the old per-analysis flags | Old `ANALYTIC_JACOBIAN`/`LINEAR`/`STAMPS_CHARGE` were removed; Design picks the shape | n (Design) |
| Unified event queue type | A single typed `EventQueue` with entries `{ kind, target, time, priority, source, rollback_behavior }` replacing the four ad-hoc sources | The four sources are merged by hand in `predict_step:734-782`; a typed queue is the clean contract | n (Design) |
| Save/probe observable catalog | Devices declare `ObservableDescriptor { name, kind, cost }`; host passes a `ProbeSelection` to `TransientAnalysisOptions` | Today `record_device_state: bool` is global all-or-nothing | n (Design) |
| Temperature protocol shape | `set_temperature(t_nominal, t_instance)` or keep single-arg + add `dtemp` to instance params; `Invalidation::Temperature` drives recompute constants → restamp | The single `set_temperature(f64)` is never called; needs wiring + per-instance delta | n (Design) |
| Lifecycle chart location | `docs/spec/part_vii_solver.md` updated + a contract test in `piperine-solver/tests/` | Spec Part VII is the normative home; the test enforces it | y (user — doc + test) |

**Open questions:** the items marked `n (Design)` above. These are design-shape
decisions (type signatures, enum variants, fail-vs-warn thresholds) that the
Design phase resolves — they do not change WHAT the spec requires, only HOW.

---

## User Stories

### P1: Rollback lifecycle — checkpoint/restore ⭐ MVP

**User Story**: As a mixed-signal simulator, when a transient step or DC homotopy
attempt is rejected, I restore every stateful device participant to the last
accepted checkpoint — not only the solution vector and digital net array.

**Why P1**: This is a proven correctness bug. The limiter (`active`, `seeds`,
vold slots) and digital register banks (`vars_int`, `vars_real`, `prev_watch`)
are mutated during rejected attempts and never rewound. Accept-gated state
(operators, event detectors, `last_volts`) is naturally safe and excluded.

**Acceptance Criteria**:

1. WHEN the transient solver calls `attempt_step` THEN it SHALL call
   `Element::checkpoint_state()` on every element that declared
   `SUPPORTS_ROLLBACK`, storing the returned checkpoint before the digital
   settle + Newton solve begin.
2. WHEN a transient step is rejected (LTE failure via `reject_lte_step` or
   non-convergence via `reject_step`) THEN the solver SHALL call
   `Element::restore_state(&checkpoint)` on every checkpointed element BEFORE
   the retry attempt.
3. WHEN a transient step is accepted THEN the solver SHALL discard the
   checkpoint (no restore is called); `accept_timestep` advances accept-gated
   state as today.
4. WHEN a limiter (pnjlim/fetlim) is active during a rejected Newton iteration
   THEN `restore_state` SHALL rewind the limiter's `active` flag, `seeds`, and
   vold slots to the last accepted step's values — the retry starts from clean
   limiter state.
5. WHEN digital `seq_phase` commits register writes during a rejected settle
   THEN `restore_state` SHALL rewind `vars_int`, `vars_real`, and `prev_watch`
   to the pre-settle values — the retry's `seq_phase` starts from clean
   registers.
6. WHEN an element has no mutable non-accept-gated state THEN
   `checkpoint_state` SHALL return `None` (default) and the solver SHALL skip
   the checkpoint/restore pair entirely (zero cost).
7. WHEN a DC homotopy strategy falls through (failed attempt → next strategy)
   THEN the DC solver SHALL call `restore_state` on checkpointed elements
   before retrying with the next homotopy parameter.
8. WHEN multiple rejects occur in sequence THEN each attempt takes a fresh
   checkpoint and each reject restores from it — no checkpoint stacking, no
   stale restore.

**Independent Test**: A circuit with a pn-junction device (exercises pnjlim)
and a clocked digital register, forced through repeated step rejections by a
tight LTE tolerance. After a rejected-and-retried step, assert: (a) the
limiter's `active` flag matches the pre-attempt state, (b) the digital
register bank matches the pre-settle state, (c) the accepted solution after
retry is bit-identical to a circuit that never rejected.

---

### P2: Formal limiting API

**User Story**: As a device author, I report structured limiting feedback
(proposed value, limited value, limiter name, reason) so the solver can steer
Newton and the host can report diagnostics — instead of a bare boolean.

**Why P2**: The limiter state is being checkpointed (Story P1); formalizing the
API in the same feature avoids rework. The current `limiting_active: bool` +
`convergence_hint: Option<ConvergenceHint>` is too thin for diagnostics and
splits one concept across two methods.

**Acceptance Criteria**:

1. WHEN a device limiter clamps a value THEN the device SHALL return a
   `LimitingReport { proposed, limited, limiter_name, reason }` from the load
   method (or a dedicated `limiting_report()` accessor), replacing both
   `limiting_active()` and `convergence_hint()`.
2. WHEN the solver tests convergence THEN it SHALL read `LimitingReport.limited`
   and apply it to the Newton guess (current `convergence_hint` behavior,
   generalized to the structured report).
3. WHEN no limiter is active THEN the report SHALL be `None` (zero cost — the
   default).
4. WHEN a host queries device diagnostics THEN it SHALL be able to read the
   last `LimitingReport` (limiter name + reason) for UI/error reporting.
5. WHEN `LimitingReport` replaces `limiting_active`/`convergence_hint` THEN
   every existing call site (DC driver, transient driver, bypass suppression)
   SHALL use the new API — no dead methods left behind.

**Independent Test**: A MOSFET DC sweep through the subthreshold region
(exercises fetlim). Assert: the `LimitingReport` carries the limiter name
(`"fetlim"`), the proposed vs limited `vds`/`vgs`, and the solver applies the
limited value. A purely linear device returns `None`.

---

### P3: Lifecycle contract — ordered chart + algorithm flow per analysis

**User Story**: As an external device author (OSDI plugin, co-sim wrapper), I
know exactly when each lifecycle hook fires and what algorithm each analysis
driver runs, so I can write correct device code without reading the solver
source.

**Why P3**: The hooks exist individually (`setup` → `allocate_unknowns` →
`set_temperature` → `update` → `load_*` → `accept_timestep` → `checkpoint`/
`restore` → `destroy`) but there is no single ordered contract per analysis.
An external author doesn't know if `set_temperature` fires before or after
`setup`, or whether `checkpoint_state` is called in DC. Equally, the algorithm
flow per analysis (Newton loop structure, TR-BDF2 two-phase, shooting method,
central-difference sensitivity) is implicit in the driver source — an external
author who needs to understand WHY a hook fires (e.g., "the TR phase converges
but BDF2 fails, so `restore_state` is called between phases") has no reference.

**Acceptance Criteria**:

1. WHEN a device author reads `docs/spec/part_vii_solver.md` THEN they SHALL
   find, for each analysis (DC, AC, tran, noise, PSS, `.sens`), a table listing
   the ordered sequence of `Element`/`AnalogDevice`/`DigitalDevice` hooks with
   their preconditions and postconditions.
2. WHEN a device author reads the same section THEN they SHALL find, for each
   analysis, a structured algorithm description covering: the driver's main loop
   structure (Newton/homotopy/shooting/FD), the phases within one iteration
   (e.g., TR-BDF2's two-phase solve, PSS's predict-shoot-assess), the
   convergence/rejection criteria, and where each lifecycle hook sits in that
   flow — so the author understands WHY a hook fires, not just WHEN.
3. WHEN the lifecycle contract test runs THEN it SHALL instrument a test Element
   (recording every hook call with its timestamp) and assert the hook ordering
   matches the documented chart for each analysis.
4. WHEN the rollback hooks (Story P1) are added THEN the chart + algorithm
   description SHALL include them: `checkpoint_state` before attempt,
   `restore_state` on reject, discard on accept — and the algorithm flow SHALL
   show where in the iteration the reject can occur (e.g., between TR and BDF2
   phases, after LTE assessment).
5. WHEN the temperature protocol (Story P4) is wired THEN the chart SHALL
   include `set_temperature` in its correct position relative to `setup` and
   `load_*`.

**Independent Test**: The contract test itself — a test Element that panics if
hooks fire out of documented order, run once per analysis type. A second
assertion: the algorithm description for each analysis is present and
non-empty (a documentation completeness check).

---

### P4: Temperature protocol

**User Story**: As a designer running a temperature sweep, each device
recomputes its temperature-dependent constants at the right temperature, and the
solver knows whether a temperature change means recompute-constants, restamp, or
rebuild.

**Why P4**: `set_temperature` exists (`element.rs:152`) but is **never called**
anywhere. `Invalidation::Temperature` is declared but nothing drives it. There
is no per-instance temperature (only a global `Tolerances.temperature`). A
temperature sweep today works only because stdlib models read `$temperature` at
eval time, not through the ABI seam.

**Acceptance Criteria**:

1. WHEN the solver builds a circuit THEN it SHALL call
   `set_temperature(t_nominal)` on every analog element during `setup`, after
   `allocate_unknowns` and before the first `load_*`.
2. WHEN a temperature sweep point changes the temperature THEN the solver SHALL
   call `set_temperature(t_new)` on every element and honor the returned
   `Invalidation::Temperature` (recompute constants → restamp).
3. WHEN an instance has a per-instance delta temperature (`dtemp`) THEN the
   effective temperature SHALL be `t_nominal + dtemp_instance`, and
   `set_temperature` SHALL receive the effective value.
4. WHEN `set_temperature` causes `Invalidation::Rebuild` (structural change —
   rare) THEN the solver SHALL fail loud if a rebuild is not possible in the
   active analysis (same rule as parameter invalidation).

**Independent Test**: The existing temperature-sweep proof (diode forward drop
shifts ≈ −2 mV/°C) SHALL now route through `set_temperature` instead of
`$temperature`-at-eval-time. A test device asserts `set_temperature` was called
with the correct effective temperature before the first load.

---

### P5: Jacobian/stamp capability declaration

**User Story**: As an analysis driver, I check whether a device provides the
Jacobian/derivative order I need before running, and fail loud if it doesn't —
not silently produce zero results.

**Why P5**: The old `ANALYTIC_JACOBIAN`/`LINEAR`/`STAMPS_CHARGE` flags were
removed (solver-simplification batch 1) with no replacement. `.disto` silently
`continue`s on `None` (`disto.rs:375`) — a fully linear circuit yields zero
HD2/HD3 with no diagnostic. A future plugin with finite-difference Jacobians
has no way to declare it.

**Acceptance Criteria**:

1. WHEN an element is built THEN it SHALL declare its derivative capability
   (analytic Jacobian available, disto2 available, disto3 available — or none)
   through a capability descriptor — either new `ElementCapabilities` bits or a
   typed enum on the introspection surface.
2. WHEN `.disto` runs and NO device declares disto2/disto3 capability THEN the
   analysis SHALL emit a named diagnostic (warn or fail — Design decides) — not
   silently return zero.
3. WHEN a device provides numeric-only Jacobians (finite difference) THEN it
   SHALL declare so, and analyses that require analytic derivatives (`.disto`)
   SHALL fail loud with a named error.
4. WHEN every in-tree JIT device compiles THEN it SHALL declare analytic
   capability (symbolic differentiation always produces one) — no regression.

**Independent Test**: A purely linear circuit (resistors + VCVS) running
`.disto` SHALL produce a named diagnostic. A circuit with one nonlinear device
(MOSFET) SHALL run `.disto` normally. A test plugin with numeric-only Jacobian
SHALL fail loud on `.disto`.

---

### P6: Terminal descriptors + opvar catalog — kernel→ABI bridge

**User Story**: As a host (LSP, CLI, Python), I query a JIT-compiled device's
terminals and operating-point variables through the standard `Introspect`
surface and get real data — not empty lists.

**Why P6**: `TerminalDescriptor` is rich (`name`, `domain`, `direction`,
`required`, `discipline`, `sign`) and `QueryDescriptor` carries `kind`/`unit`/
`description`, but `PiperineDevice` overrides neither `list_terminals` nor
`read_opvars`/`list_queries` — every JIT device exposes empty catalogs. The
kernel (`AnalogKernel::terminals()`, `DigitalKernel::inputs()/outputs()`,
param names, state/var slot counts) has the data one layer down.

**Acceptance Criteria**:

1. WHEN a JIT-compiled analog device is introspected THEN `list_terminals()`
   SHALL return one `TerminalDescriptor` per kernel terminal, populated from
   `AnalogKernel::terminals()` + the symbol table (names, not just `NodeId`
   indices).
2. WHEN a JIT-compiled digital device is introspected THEN `list_terminals()`
   SHALL return one `TerminalDescriptor` per input/output, populated from
   `DigitalKernel::inputs()`/`outputs()`.
3. WHEN a terminal is an internal/auxiliary node (non-port `wire`) THEN its
   `TerminalDescriptor` SHALL mark it as internal (new field — Design picks the
   variant: a `kind: TerminalKind { External, Internal, Auxiliary }` field or
   equivalent).
4. WHEN a device's operating point is queried THEN `read_opvars()` SHALL return
   the runtime-computed opvars (e.g., `gm`, `vbe`) — not an empty list. The
   codegen SHALL compile an opvar-evaluation path (or document why it is
   deferred).
5. WHEN `list_queries()` is called THEN it SHALL return the declared query
   catalog (names, kinds, units) — not a derived scan of an empty `read_opvars`.

**Independent Test**: A MOSFET test circuit after DC operating-point analysis.
Assert: `list_terminals()` returns drain/gate/source/body (external) + any
internal nodes; `read_opvars()` returns at least `gm` and `vbe` (or whatever
the kernel declares). Before this feature, both are empty.

---

### P7: Save/probe selection

**User Story**: As a host running a long transient, I record only the specific
device observables I asked for (e.g., `Trace.i` on one branch), not every
device's full state/var bank every step.

**Why P7**: Today `record_device_state: bool` is global and all-or-nothing.
When on, `collect_device_banks()` clones every device's full `(state, vars)`
bank into every `TransientStep` — O(devices × steps) memory, even if the host
wants one branch current on one device.

**Acceptance Criteria**:

1. WHEN a device declares observables THEN it SHALL expose an
   `ObservableDescriptor { name, kind, cost }` catalog (what it can provide +
   the recording cost).
2. WHEN the host sets up a transient THEN `TransientAnalysisOptions` SHALL
   accept a `ProbeSelection` (per-device list of requested observables) — when
   empty, no device-state recording (current default-off behavior).
3. WHEN `collect_device_banks` runs THEN it SHALL record only the requested
   observables for each device — not the full bank.
4. WHEN a host requests an observable a device doesn't declare THEN the solver
   SHALL fail loud with a named error (not silently omit it).

**Independent Test**: A 100-step transient with 10 devices. `ProbeSelection`
requests one observable on one device. Assert: only that device's requested
data is in `TransientStep::device_state`; the other 9 devices are absent.
Memory usage is O(1 device), not O(10).

---

### P8: Unified event model

**User Story**: As a mixed-signal solver, every time-discontinuity source
(digital event, analog crossing, timer, `$bound_step` hint) flows through one
typed queue — not four ad-hoc containers merged by hand.

**Why P8**: `predict_step` (`transient.rs:734-782`) manually merges four
sources: `DigitalState::event_queue` (BinaryHeap), `next_breakpoints()` (polled
Vec), `SetQueue` (Vec), `bound_step_hint()` (single f64). Adding a new event
kind (e.g., an analog crossing detector) means modifying `predict_step`, not
pushing into a queue. The rollback behavior differs per source (digital queue
rolls back; breakpoints are stateless; `$bound_step` is advisory).

**Acceptance Criteria**:

1. WHEN the transient solver enters `predict_step` THEN it SHALL read from a
   single `EventQueue` that holds entries `{ kind, target, time, priority,
   source, rollback_behavior }` covering all four current sources.
2. WHEN a digital event is emitted THEN it SHALL enter the unified queue with
   `kind = Digital`, its rollback behavior = restore-on-reject (current
   `DigitalState::rollback` semantics).
3. WHEN an analog source declares a breakpoint (`next_breakpoints`) THEN the
   times SHALL enter the unified queue with `kind = Breakpoint`, rollback =
   stateless (re-polled next step).
4. WHEN `$bound_step` is reported THEN it SHALL enter the queue with
   `kind = StepHint`, priority = advisory (soft floor).
5. WHEN an analog crossing is detected (new capability — A2D comparator without
   the digital scheduler) THEN it SHALL enter the queue with
   `kind = Crossing`, carrying the crossing time + value.
6. WHEN a step is rejected THEN the queue SHALL honor each entry's
   `rollback_behavior` — digital events restored, breakpoints re-polled,
   crossing events discarded (re-detected next attempt).

**Independent Test**: A mixed-signal circuit with a digital clock (digital
events), a pulse source (breakpoints), and a `$bound_step` hint. Assert: all
three land in one queue, `predict_step` picks the earliest, and a rejected step
restores the digital events but re-polls the breakpoints.

---

### P9: Stepper strategy composition

**User Story**: As a solver developer, transient step rejection and dt proposal
are composed through `ConvergencePlan` (like Newton damping and homotopy
already are), not inline in the transient driver.

**Why P9**: `NewtonStrategy` + `HomotopyStrategy` are folded into
`ConvergencePlan`; `StepperStrategy` is owned separately by `TransientSolver`
and its rejection logic (`reject_step`/`reject_lte_step`/`propose_dt`) is inline
phase methods. Completing the fold makes the transient strategy surface uniform.

**Acceptance Criteria**:

1. WHEN `ConvergencePlan` is constructed for a transient analysis THEN it SHALL
   own the `StepperStrategy` alongside `NewtonStrategy` and `HomotopyStrategy`.
2. WHEN a step is rejected THEN the rejection decision + dt update SHALL route
   through the plan's stepper, not through inline `TransientSolver` methods.
3. WHEN the existing PI controller (`PiController`) runs THEN its behavior SHALL
   be unchanged (regression: bit-identical step sequence on the parity
   baselines).
4. WHEN a custom stepper is plugged in (test double) THEN it SHALL receive the
   reject/accept callbacks through the plan — not by reimplementing the
   transient loop.

**Independent Test**: The parity baselines (`parity_baseline.rs`) remain
bit-identical. A test double `StepperStrategy` that halves dt on every reject
produces a different (but deterministic) step sequence through the plan.

---

### P10: Introspect leftovers — model descriptor + kernel catalogs

**User Story**: As a host, I read a device's model identity (type id, version)
and its real named terminal/opvar catalogs through the introspection surface —
not positional indices.

**Why P10**: There is no model type-id/version descriptor (only `name()` as a
string). The kernel carries rich named catalogs (`AnalogKernel::param_names()`,
terminal `NodeId`s, state/var/force slot counts) but the Element ABI surfaces
none of it — `PiperineDevice` doesn't bridge kernel data to `Introspect`
(except `list_params`/`get_param`/`set_param`). Story P6 bridges terminals +
opvars; this story covers the remaining catalog surface.

**Acceptance Criteria**:

1. WHEN an element is introspected THEN it SHALL expose a `ModelDescriptor`
   carrying type id (e.g., `"mos"`, `"diode"`) and version (e.g., `"3.1"`) —
   not just the instance name.
2. WHEN a kernel has named state slots (runtime banks, force terminals, noise
   terminals) THEN the Element ABI SHALL surface them as named catalogs, not
   positional indices.
3. WHEN the host queries "what can this device report?" THEN the introspection
   surface SHALL return the union of: terminals (P6), opvars (P6), params
   (done), state slots, force/noise terminal names — all named.

**Independent Test**: A MOSFET after DC. Assert: `ModelDescriptor` reads
`{ type: "mos", version: "3" }` (or whatever the kernel declares); the state
slot catalog names the limiter slots; the noise terminal catalog names the
thermal/shot noise terminal pairs.

---

## Edge Cases

- WHEN an element declares `SUPPORTS_ROLLBACK` but `checkpoint_state` returns
  `None` THEN the solver SHALL treat it as stateless (no restore on reject).
- WHEN a checkpoint is taken but the element is destroyed before the step
  resolves (e.g., live-param rebuild mid-step) THEN the checkpoint SHALL be
  discarded — no use-after-free.
- WHEN `.disto` runs on a circuit where SOME devices have disto2 but none have
  disto3 THEN the disto2 results SHALL be valid and disto3 SHALL emit the named
  diagnostic (not fail the whole analysis).
- WHEN a temperature sweep hits a device that doesn't override
  `set_temperature` (default no-op) THEN the sweep SHALL proceed — the device
  reads `$temperature` at eval time as today (backward compatible).
- WHEN the unified event queue is empty (no events of any kind) THEN
  `predict_step` SHALL fall back to the PI-proposed dt — no busy-loop.
- WHEN a `ProbeSelection` requests an observable on a device that has
  `record_device_state = false` globally THEN the per-device selection SHALL
  win (the global bool becomes "record everything" shorthand, not a gate).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| ABI-01 | P1 Rollback: checkpoint before attempt | Design | Pending |
| ABI-02 | P1 Rollback: restore on transient reject | Design | Pending |
| ABI-03 | P1 Rollback: discard on accept | Design | Pending |
| ABI-04 | P1 Rollback: limiter state rewound | Design | Pending |
| ABI-05 | P1 Rollback: digital registers rewound | Design | Pending |
| ABI-06 | P1 Rollback: default None = zero cost | Design | Pending |
| ABI-07 | P1 Rollback: DC homotopy retry | Design | Pending |
| ABI-08 | P1 Rollback: multiple rejects, fresh checkpoint each | Design | Pending |
| ABI-09 | P2 Limiting: LimitingReport struct | Design | Pending |
| ABI-10 | P2 Limiting: solver applies limited value | Design | Pending |
| ABI-11 | P2 Limiting: None when inactive | Design | Pending |
| ABI-12 | P2 Limiting: host-readable diagnostics | Design | Pending |
| ABI-13 | P2 Limiting: no dead methods | Design | Pending |
| ABI-14 | P3 Lifecycle: documented hook chart per analysis | Design | Pending |
| ABI-15 | P3 Lifecycle: algorithm flow description per analysis | Design | Pending |
| ABI-16 | P3 Lifecycle: executable contract test | Design | Pending |
| ABI-17 | P3 Lifecycle: rollback hooks in chart + algorithm | Design | Pending |
| ABI-18 | P3 Lifecycle: temperature in chart | Design | Pending |
| ABI-19 | P4 Temperature: set_temperature in setup | Design | Pending |
| ABI-20 | P4 Temperature: sweep drives invalidation | Design | Pending |
| ABI-21 | P4 Temperature: per-instance dtemp | Design | Pending |
| ABI-22 | P4 Temperature: Rebuild fail-loud | Design | Pending |
| ABI-23 | P5 Jacobian: capability descriptor | Design | Pending |
| ABI-24 | P5 Jacobian: .disto fail-loud when absent | Design | Pending |
| ABI-25 | P5 Jacobian: numeric-only declared + gated | Design | Pending |
| ABI-26 | P5 Jacobian: JIT devices declare analytic | Design | Pending |
| ABI-27 | P6 Terminals: analog kernel→ABI bridge | Design | Pending |
| ABI-28 | P6 Terminals: digital kernel→ABI bridge | Design | Pending |
| ABI-29 | P6 Terminals: internal/auxiliary kind | Design | Pending |
| ABI-30 | P6 Opvars: read_opvars populated for JIT | Design | Pending |
| ABI-31 | P6 Opvars: list_queries catalog | Design | Pending |
| ABI-32 | P7 Save/probe: observable catalog | Design | Pending |
| ABI-33 | P7 Save/probe: ProbeSelection in options | Design | Pending |
| ABI-34 | P7 Save/probe: per-observable recording | Design | Pending |
| ABI-35 | P7 Save/probe: fail-loud on unknown observable | Design | Pending |
| ABI-36 | P8 Events: unified queue type | Design | Pending |
| ABI-37 | P8 Events: digital events in unified queue | Design | Pending |
| ABI-38 | P8 Events: breakpoints in unified queue | Design | Pending |
| ABI-39 | P8 Events: $bound_step in unified queue | Design | Pending |
| ABI-40 | P8 Events: analog crossings in unified queue | Design | Pending |
| ABI-41 | P8 Events: per-entry rollback behavior | Design | Pending |
| ABI-42 | P9 Strategy: StepperStrategy in ConvergencePlan | Design | Pending |
| ABI-43 | P9 Strategy: rejection routes through plan | Design | Pending |
| ABI-44 | P9 Strategy: parity baselines bit-identical | Design | Pending |
| ABI-45 | P9 Strategy: custom stepper via plan | Design | Pending |
| ABI-46 | P10 Introspect: ModelDescriptor (type id/version) | Design | Pending |
| ABI-47 | P10 Introspect: named state/force/noise catalogs | Design | Pending |
| ABI-48 | P10 Introspect: unified "what can I report" query | Design | Pending |

**ID format:** `ABI-[NUMBER]`

**Coverage:** 48 total, 0 mapped to tasks yet (Design pending).

---

## Success Criteria

- [ ] A rejected transient step leaves zero dirty device-internal state — the
      limiter, digital registers, and edge-detection memory are restored to the
      last accepted checkpoint (ABI-01..08).
- [ ] The limiting surface is a single structured report, not a boolean + an
      optional hint (ABI-09..13).
- [ ] An external device author can read the lifecycle chart + algorithm flow
      and know exactly when every hook fires and why, enforced by a contract
      test (ABI-14..18).
- [ ] Temperature sweeps route through the ABI seam (`set_temperature`), not
      only through `$temperature`-at-eval-time (ABI-19..22).
- [ ] `.disto` on a circuit without nonlinear devices emits a named diagnostic,
      not zero (ABI-23..26).
- [ ] A JIT-compiled device's terminals and opvars are visible through the
      standard introspection surface (ABI-27..31).
- [ ] Recording is per-observable, not all-or-nothing (ABI-32..35).
- [ ] All event sources flow through one typed queue (ABI-36..41).
- [ ] `StepperStrategy` is plan-composed; parity baselines bit-identical
      (ABI-42..45).
- [ ] A device's model identity and named catalogs are introspectable
      (ABI-46..48).
- [ ] `cargo test --workspace` green; zero rustc warnings.

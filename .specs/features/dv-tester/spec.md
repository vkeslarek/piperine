# dv-tester Specification — the host-language device library

**Surface design:** `ideal.md` in this directory — read it first. It carries the
level taxonomy, the four level-1 families, the cost model, and the decision log
(§12). This document turns that surface into requirements.
**Vision:** `.specs/features/design-verification/ideal.md` §4 and §9 items 10–15.
Decisions D1–D16 are binding; this spec cites them.
**Depends on:** `dv-core` — `validation_reports()` is how a tester speaks, and
margins are what `cover` reuses.
**Siblings:** `dv-gradients` (delivered). **Follow-on:** `codegen-simplification`
— this feature *extracts* into `piperine-signal`; that one *delegates* codegen to
it (`ideal.md` §12.16).
**Scope tier:** Large/Complex.

> **Rewritten 2026-07-31** against `ideal.md`'s third revision. The previous
> version specified a narrow "programmatic tester" and was marked DEFERRED
> because the framing had outgrown it. The scope is now the full host-language
> device library, of which a tester is one use.

## Problem Statement

There are four ways to get a device into a Piperine circuit and none of them lets
an author write one where they already are (`ideal.md` §0): PHDL requires the
behavior to be expressible in PHDL; a plugin requires a separate Rust crate,
`dlopen`, and a matched toolchain (`plugin/src/backend/native.rs`); OSDI requires
Verilog-A and a compiler. `manifest.rs` shows Python plugins are *scripted* —
CLI entries and hooks, never devices. **There is no in-process, no-build-step
device authoring at all.**

Testers feel this most: a functional test is imperative and temporal — drive,
wait, sample, compare — and expressing that as a PHDL `digital` FSM means
hand-coding a state machine for what is naturally a straight-line program.
Driving stimulus from Python today means stepping the simulation from outside,
with no way for a host-side program to participate in the time loop at its own
rate. But the gap is general, and designing it as "tester plumbing" is what
produced two rejected drafts.

Meanwhile the mechanism half already exists and is unreachable: `Element::next_breakpoints`
(`core/element.rs`) lets an element declare its own wake-ups and the transient
driver lands on them; `SUPPORTS_ROLLBACK` checkpoint/restore is live; the
`EventQueue` already carries digital edges, breakpoints, step hints and
crossings. Nothing is wired to a host. And codegen holds correct, tested,
**already pure** implementations of the runtime operators, the event detectors,
`table` and the junction limiter — reachable only from PHDL.

## Goals

- [ ] **`piperine-signal`**, a pure crate (`ideal.md` §3.1: no MNA, no `Element`,
      no JIT, no in-tree dependency) holding four families: `Wave` (value from
      time), `Behavior` (value from circuit state, with closed-form Jacobian),
      events (crossing detection), `Measure` (streaming reductions).
- [ ] **Extraction, not invention** — the runtime operators, event detectors,
      `table` and the limiter move from codegen with their semantics and tests
      intact (§3.9).
- [ ] **`HostElement`** + a blanket `Element` adapter, so the solver sees no
      difference between a host device and any other.
- [x] **A `Sequencer`**: one `wait` verb over three wake sources (time, digital
      edge, analog crossing), written as a coroutine.
- [x] **`@device` gains a host binding mode** — a host device is declared in PHDL
      and instantiated like any module (§8.6/§8.7).
- [x] **Capability parity**: a host-written device reaches the same vocabulary a
      PHDL device does, provably.
- [x] Python and Rust as one surface, with one deliberate asymmetry: the level-1
      catalog is closed in Python, open in Rust (§3.7).
- [x] `cover` bins with host-side accumulation and their own posture (D9, D10).
- [x] Monitors for the residue a tester cannot reach.

## Out of Scope

| Feature | Reason / owner |
|---|---|
| `tol`, `constraint`, margins, postures, the findings channel itself | `dv-core` |
| Gradients, optimizer, Monte Carlo, centering | `dv-gradients` (delivered) |
| **Rewiring codegen to call `piperine-signal`** | `codegen-simplification` — this feature extracts and pays the interim cost (cross-check fixtures); that one deletes codegen's copies (§12.16) |
| **Unifying `Measure` with `dv-core`'s `constraint`** | Deliberately two engines for now (§8.1); revisited by `codegen-simplification` |
| **Exact (tier-3) crossings** | Needs `EventKind::Crossing` promoted to `Exact` + reject-and-retry — a scoped solver feature. This ships tier 2 with the error published (§3.4) |
| **Tester in AC** | Known gap (D13) — refused loudly |
| **RNM / `behavioral` body kind** | Dropped from V1 (D16); this feature is what replaced it |
| Distribution of Python host devices | They stay script-local; a Rust one graduates to a plugin (§8.7) |
| An in-language sequence syntax (`##`-style) for monitors | Inadmissible until usage proves the generator form inadequate |
| Coverage closure *tooling* (dashboards, merge UI) | Data model and merge semantics are in scope; UI is not |

---

## Assumptions & Decisions

All resolved; `ideal.md` §12 is the full log with rationale.

| Decision | Choice | Source |
|---|---|---|
| Level taxonomy | 0 = raw solver ABI, 1 = `piperine-signal`, 2 = host library; they compose in one device | §1 |
| Crate name | `piperine-signal` | §12.1 |
| Level-1 extensibility | Closed catalog in Python, open trait in Rust — for `Behavior` **and** `Measure` | §12.2 |
| Digital gate/wake rhythm | **Settled events only** — a mid-delta-cycle value can be a DAG-evaluation-order artifact, not a signal | §12.3 |
| Relationship to `#[pip::device]` | One concept (`@device`), two binding modes (plugin / host); **no new macro** | §12.4 |
| How a host device enters a circuit | **Declared in PHDL**, instantiated like any module | §12.5 |
| Distribution | A Rust host device **graduates** to a plugin with no PHDL change; Python stays script-local | §12.6 |
| Digital edge waits | **In V1** | §12.7 |
| `install` semantics | Install-relative time; discontinuity registered, never smoothed; continuity explicit | §12.8 |
| `Measure` lifecycle | `arm` = fresh accumulator; accepted points only; read-before-sample is loud; `disarm` freezes | §12.9 |
| DC/OP with a sequencer | Coroutine does not run at DC; each `Wave` kind declares its own DC value (SPICE convention) | §12.10 |
| `set_param` mid-transient | Restamp-class fine; rebuild-class loud | §12.11 |
| Ordering at one instant | Authored POM instantiation order; same-net drive conflict is loud regardless | §12.12 |
| Rollback | Explicit `advance` is the guarantee — level 2 never observes a rejected step; levels 0/1 participate in them | D11, §7 |
| Side effects | Allowed; reproducibility is the author's responsibility | D12 |
| Peak resolution | Accepted-point reduction + published error bound; `refine=True` opt-in | §3.6 |

**Open (none blocking):** the `refine=True` termination rule (§12.13), whether
protocol helpers ship or stay exemplary (§12.14).

---

## User Stories

### P1: `piperine-signal` — the pure mechanism crate ⭐ MVP

**User Story**: As every other layer, I want one crate that knows what signals
*mean* — values, derivatives, events, reductions — without knowing anything about
matrices or compilers.

**Why P1**: Levels 0 and 2 are both thin over it; capability parity is impossible
without it.

**Acceptance Criteria**:

1. WHEN `piperine-signal` is built THEN it SHALL depend on **no other crate in
   the workspace**, and SHALL contain no reference to `Element`, to any MNA or
   matrix type, or to Cranelift. Its purity is the crate boundary's own test.
2. WHEN the runtime operators are extracted THEN `delay`, `slew`, `transition`
   and `idt`/`idtmod` SHALL preserve every rule catalogued in §3.9 A — including
   `transition`'s **re-anchor on retarget**, its **first-observation seeding**,
   its **zero-ramp** handling, and its `pending_edges` contract — and SHALL be
   mutated **only on accepted timesteps**.
3. WHEN the event detectors are extracted THEN `cross`, `above` and `timer` SHALL
   preserve the seeding asymmetry of §3.9 B: an unseeded `cross` never fires; an
   unseeded `above` **does** fire on its first sample.
4. WHEN `table` is provided THEN it SHALL lower to a **pure piecewise function**
   whose derivative is the segment slope — no hand-written Jacobian — and SHALL
   refuse loudly on non-constant, unequal-length, fewer-than-two-point, or
   non-strictly-increasing breakpoints.
5. WHEN the limiter is extracted THEN all five pnjlim rules of §3.9 D SHALL
   survive, including the **`vcrit` seed**, the **seed-preservation** rule, the
   **convergence veto with its tolerance**, and the **move-the-minus-node** rule.
6. WHEN any extracted family is exercised THEN its behavior SHALL be identical to
   codegen's copy, proven by a **per-family cross-check fixture** (one fixture,
   both paths, asserted equal) — the holding measure until
   `codegen-simplification` deletes the copy.

**Independent Test**: the crate builds with an empty in-tree dependency list; the
relocated operator/detector/limiter tests pass unchanged; one cross-check fixture
per family asserts codegen and crate agree.

---

### P1: `Wave` and `Behavior` — declared drive ⭐ MVP

**User Story**: As a device author, I want to declare what my device drives —
whether it depends on time alone or on the circuit — and pay **no** host call for
it.

**Why P1**: This is what makes a host device fast enough to be worth writing, and
`Behavior` is what makes a reactive device (a comparator, a PWM) expressible at
all.

**Acceptance Criteria**:

1. WHEN a `Wave` descriptor other than `from_fn` is installed THEN it SHALL be
   evaluated natively with **zero** host calls, and `Wave.sampled(t, v)` SHALL
   accept an arbitrary numpy-computed shape at the same zero cost.
2. WHEN a `Wave` has discontinuities THEN it SHALL publish its own breakpoints so
   the integrator lands on each edge rather than interpolating across it.
3. WHEN a `Behavior` is installed THEN it SHALL contribute **cross Jacobian
   entries** (`∂f/∂vᵢ`) from a closed form, reacting at every Newton iteration
   with **zero** host calls — the acceptance case being a hysteretic PWM
   controller reading two terminals (`ideal.md` example D).
4. WHEN a discontinuous `Behavior` kind (`switch`, `hysteresis`) is used THEN it
   SHALL register a crossing so the integrator sees the edge.
5. WHEN a new `BehaviorKind` or `MeasureKind` is written in **Rust** THEN it
   SHALL compose exactly like a shipped one; in **Python** the catalog SHALL be
   closed, with level 0 as the documented escape hatch.
6. WHEN `verify_derivatives` is run on a `BehaviorKind` THEN it SHALL compare the
   analytic Jacobian against a central finite difference and fail loudly on
   disagreement.

**Independent Test**: a PWM fixture whose host-call count inside Newton is
asserted **zero**; a `Behavior` with a deliberately wrong Jacobian is caught by
`verify_derivatives`.

---

### P1: `Measure` — reductions at the solver's rate ⭐ MVP

**User Story**: As a test author, I want peak, ripple, settling time and
overshoot — the things that live *between* my breakpoints — and I want to know
how much to trust them.

**Why P1**: "Insert a signal and test the reaction" is meaningless without it;
`read_v()` at test breakpoints cannot see an overshoot.

**Acceptance Criteria**:

1. WHEN a `Measure` is armed THEN it SHALL be a streaming accumulator holding
   O(1) state, fed one `(t, v)` per **accepted** point, never storing a waveform.
2. WHEN the same reduction is applied to a stored `Waveform` post-run and to a
   live transient online THEN both SHALL produce identical results — the crate's
   own regression test.
3. WHEN a `Measure` reports a value THEN it SHALL travel with an **error bound**
   derived from the local LTE, and an `expect` against a measure SHALL compare
   against the **bound**, not the point estimate alone.
4. WHEN `refine=True` is requested THEN the measure SHALL drive step refinement
   near the extremum until the bound meets the requested tolerance, and SHALL
   **terminate at the step floor with a report** rather than stalling.
5. WHEN a `Measure` is gated THEN time windows, analog gates (crossing
   predicates), and digital gates SHALL all be supported; a digital gate SHALL
   observe **settled** events only.
6. WHEN `.value` is read before any accepted point has fed the measure — or when
   a gate window never opened — THEN it SHALL fail **loud** naming the reason,
   never returning `NaN`, zero, or the nominal.
7. WHEN a measure is armed twice THEN the two SHALL be independent handles;
   WHEN one is disarmed THEN its value SHALL freeze and stay readable.

**Independent Test**: an overshoot fixture where the true peak falls between
accepted points — assert the reported bound contains the analytic peak, assert
`refine=True` narrows it, assert reading an unfed measure is loud.

---

### P1: `HostElement` and the `@device` host binding ⭐ MVP

**User Story**: As a device author, I want to write a device in Python or Rust,
declare it in PHDL like any other, and have the solver not care.

**Why P1**: This is the gap in §0; everything else is a library on top of it.

**Acceptance Criteria**:

1. WHEN a type implements `HostElement` THEN a blanket adapter SHALL present it
   to the solver as an ordinary `Element` — no new solver concept, no downcast
   (MD-01).
2. WHEN a host device is used THEN it SHALL be **declared in PHDL** with
   `@device` and instantiated like any module — composing with hierarchy, with
   parameters, and instantiable more than once.
3. WHEN the same `@device` declaration is bound to a **plugin** implementation
   instead of a host one THEN the PHDL SHALL NOT change — the binding mode is
   inferred from where the implementation is registered.
4. WHEN a Rust host device is promoted to a plugin THEN the circuit using it
   SHALL be unaffected; WHEN a Python host device is asked to be distributed THEN
   the limitation SHALL be documented, not discovered.
5. WHEN a Python device raises mid-solve THEN the bridge SHALL map it to a typed
   error carrying the simulation time and stop the analysis cleanly.
6. WHEN a host device writes a parameter mid-transient THEN a restamp-class write
   SHALL go through the live-parameter path and a rebuild-class write SHALL fail
   loud (MD-18).
7. WHEN a level-1 mechanism vetoes convergence (the limiter's rule, §3.9 D4) THEN
   the adapter SHALL surface a `limiting_report`-shaped signal, not only stamps.

**Independent Test**: a resistor written three ways — PHDL, Rust host, Python
host — instantiated twice each in a hierarchy, producing identical operating
points.

---

### P1: The `Sequencer` and one `wait` ⭐ MVP

**User Story**: As a test author, I want to write a test as a sequence of actions
in time — the way I think about bring-up — and to wait on a clock edge without
leaving that straight line.

**Acceptance Criteria**:

1. WHEN a test is written as a coroutine THEN `yield t.advance(dt)` SHALL return
   control to the solver, which runs to the breakpoint and resumes the program
   there, with imperative state living in the coroutine frame.
2. WHEN the solver reaches a sequencer breakpoint and **accepts** the step THEN
   the host SHALL be resumed exactly once for it; WHEN a step containing one is
   **rejected** THEN the host SHALL NOT be resumed (D11).
3. WHEN `yield t.wait(...)` is used THEN it SHALL accept three wake sources —
   a **time** advance, a **digital edge**, and an **analog crossing** — mapping
   onto the `EventQueue`'s existing `Digital`/`Breakpoint`/`Crossing` kinds.
4. WHEN a digital edge wait is armed THEN the scheduler SHALL resume the waiter
   only after the net has **settled**, never mid-delta-cycle.
5. WHEN several sequencers are instantiated THEN their breakpoint sets SHALL
   union and each SHALL be resumed independently at its own instants, in
   **authored POM instantiation order** at a shared instant.
6. WHEN `t.expect(cond, msg)` fails THEN it SHALL emit an `Error` finding through
   `dv-core`'s `validation_reports()` with time and instance provenance, honoring
   the active posture; `t.warn(...)` SHALL emit a `Warning` that never aborts.
7. WHEN a sequencer also stamps (level 0) or installs `Wave`/`Behavior`/`Measure`
   (level 1) THEN that SHALL be legal in the same class — the levels are
   coordinates, not exclusive modes.
8. WHEN `SimHooks` is considered THEN it SHALL NOT be the mechanism — it is
   coarse-grained with no per-step site.

**Independent Test**: a recording sequencer asserting it is resumed exactly at
its declared instants and never on a rejected step; two sequencers interleaving;
a clock-synchronized burst test (`ideal.md` example F).

---

### P1: Drive without a host crossing inside Newton ⭐ MVP

**User Story**: As a test author, I want to drive any stimulus imperatively
without the simulation calling back into Python inside a Newton iteration.

**Why P1**: This is what made RNM unnecessary (D16).

**Acceptance Criteria**:

1. WHEN `t.install(net, wave)` is called THEN the level-1 descriptor SHALL be
   installed and stamped natively until replaced — with **zero** host crossings
   inside any Newton iteration, asserted by a counted test.
2. WHEN a wave is installed mid-run THEN its time origin SHALL be the **install
   instant**, not absolute zero.
3. WHEN an installed wave replaces an active one THEN it SHALL take effect
   immediately and its discontinuity SHALL be **registered as a breakpoint** —
   never silently smoothed. Continuity SHALL be expressible explicitly.
4. WHEN a drive is installed THEN its impedance SHALL be declared; an ideal force
   SHALL be available but spelled explicitly and never defaulted.
5. WHEN a tester drives an **internal** net THEN it SHALL be allowed and SHALL
   appear in the run's findings, so a passing test that forces a node is visibly
   doing so (D14).
6. WHEN a digital net is driven THEN it SHALL go through the digital `EventSink`
   and SHALL NOT touch the MNA.
7. WHEN an **AC** analysis is requested with a sequencer present THEN it SHALL
   fail loud (D13). WHEN **DC/OP** is requested THEN the coroutine SHALL NOT run
   and each installed `Wave` SHALL contribute its declared DC value (§12.10).

**Independent Test**: a ramp into an RC compared against the analytic segment
response; host calls between breakpoints counted and asserted zero; an install
mid-run asserted to be install-relative and to register its edge; AC asserted
loud.

---

### P1: Capability parity, proven ⭐ MVP

**User Story**: As a device author, I want the host language to be a real place
to write a device — not a subset that silently lacks what PHDL has.

**Why P1**: It is the difference between a library and a toy, and it is the
success criterion `codegen-simplification` inherits.

**Acceptance Criteria**:

1. WHEN a device is written in PHDL and the same device is written as a host
   device THEN both SHALL produce identical results on the same fixture.
2. WHEN parity is claimed THEN it SHALL be proven for one device per level-1
   family: a `transition`-driven source, a pnjlim junction, a `table`
   nonlinearity, and a `cross`-triggered element.
3. WHEN a PHDL capability is **not** reachable from the host THEN it SHALL be
   named in the documentation — a scoped promise, never an eroding one.
4. WHEN the host surfaces are enumerated THEN Python and Rust SHALL match in
   names, argument order and semantics, enforced by a `host_parity.rs`-style
   target; the closed-Python/open-Rust catalog asymmetry SHALL be the only
   documented divergence.

**Independent Test**: the four twinned devices, each asserted identical across
authoring paths.

---

### P2: Monitors — the residue a tester cannot reach

**User Story**: As a block author, I want an obligation watched continuously, or
one that ships with my block rather than with someone's test.

**Acceptance Criteria**:

1. WHEN a monitor is written THEN it SHALL be an ordinary `digital` module with
   no new grammar, reporting through `validation_reports()`.
2. WHEN a monitor fires THEN it SHALL do so at the digital scheduler's **settled**
   events, never mid-delta-cycle.
3. WHEN a monitor is instantiated in an array or parameterized THEN it SHALL
   behave as any module does.
4. WHEN an obligation could have been checked by a tester at its own rhythm THEN
   the documentation SHALL prefer the tester.

**Independent Test**: a protocol monitor inside a hierarchy detecting a violation
the top-level tester cannot see, reporting with instance provenance.

---

### P3: `cover` — bins, accumulation, and closure data

**User Story**: As a verification lead, I want to know which operating regions my
regression actually exercised.

**Acceptance Criteria**:

1. WHEN `cover <name> : <expr> bins [lo:step:hi];` or `… in {…};` appears in a
   `constraint` block THEN it SHALL reuse `dv-core`'s pointwise path with a
   bin-mapper — a hit is a counter increment, cheaper than a margin.
2. WHEN `cover=off` THEN no coverage kernel SHALL be called; coverage SHALL have
   its **own** posture so an optimizer's inner loop cannot pollute the database
   (D9).
3. WHEN a `cover cross` is declared THEN its joint bin count SHALL be checked at
   **elaboration** against a `Context`-raisable cap and refused loudly (D10).
4. WHEN multiple runs complete THEN their counters SHALL merge on the host and
   the merged data SHALL identify empty bins.
5. WHEN coverage is reported THEN it SHALL be host state persisting across runs
   and seeds.

**Independent Test**: a sweep filling a known subset of bins; empty bins named; a
second run merges rather than replaces; an oversized cross refused at elaboration.

---

## Edge Cases

- `advance(dt)` past the analysis stop time → clamped, and the sequencer is told
  the run ended rather than silently never resumed.
- `advance(0)` → legal (act again at the same instant) but bounded: a limited
  number of zero-advance resumes per instant, then loud.
- A coroutine that never yields → loud, not an infinite hang.
- A coroutine raising an exception → surfaced as a test failure with the
  simulation time, analysis stopped cleanly.
- Reading a net that does not exist → loud at the first call, listing candidates.
- Two host devices driving the same net, or a host device and a design source →
  **loud conflict**, never last-writer-wins, regardless of instantiation order.
- A `Measure` read before any accepted point, or whose gate never opened → loud.
- A `Behavior` whose analytic Jacobian disagrees with finite differences →
  caught by `verify_derivatives`, not by a failed convergence weeks later.
- A `Wave.from_fn` callback raising → same typed-error path as a device method.
- A host device present with `checks=off` → its `expect` findings still matter;
  the posture governs `require` margins, so the interaction is defined
  explicitly.
- A bulk Python device (thousands of instances) → allowed and documented as a bad
  idea with the Rust route named; never silently prevented.
- `cover` with zero accepted points → all bins empty and reported as
  not-exercised, never a vacuous 100%.

---

## Requirement Traceability

| ID | Requirement | Status |
|---|---|---|
| DVT-01 | `piperine-signal` is pure — no in-tree dependency, no `Element`/matrix/JIT | Done (T1, `2a114e8`) |
| DVT-02 | Runtime operators extracted with every §3.9 A rule intact | Done (T2, `1447cde`) |
| DVT-03 | Event detectors extracted with the `cross`/`above` seeding asymmetry | Done (T3, `35a13ba`) |
| DVT-04 | `table` as a pure piecewise function; derivative is the segment slope | Done (T4, `afda1af`) |
| DVT-05 | Limiter extracted with all five pnjlim rules | Done (T5, `4ed9569`) |
| DVT-06 | Per-family cross-check fixture vs codegen's copy | Done (T6, `996045e`) |
| DVT-07 | `Wave` descriptors evaluated natively, zero host calls; `sampled` arbitrary | Done (T7, `52b131c`) |
| DVT-08 | `Wave` publishes its own breakpoints | Done (T7, `52b131c`) |
| DVT-09 | `Behavior` contributes cross-Jacobian entries; PWM fixture, zero host calls | Done (T8, `be11a05`) |
| DVT-10 | Discontinuous `Behavior` registers a crossing | Done (T8, T10, `be11a05`, `419aa55`) |
| DVT-11 | Catalog closed in Python, open in Rust (`Behavior` + `Measure`) | Done — Rust half (T8, T11 `be11a05`, `564f88f`); Python half (T28, `ea6be69`) |
| DVT-12 | `verify_derivatives` FD oracle catches a wrong Jacobian | Done (T9, `dda2c96`) |
| DVT-13 | `Measure` is an O(1) streaming accumulator on accepted points | Done (T11, `564f88f`) |
| DVT-14 | Online and post-run reductions agree | Done (T13, `3bbdc6c`) |
| DVT-15 | Every measure reports an error bound; `expect` compares against it | Done (T11, `564f88f`) |
| DVT-16 | `refine=True` narrows the bound and terminates at the floor with a report | Done (T11, `564f88f`) |
| DVT-17 | Gating: time, analog, digital (settled only) | Done (T12, `e6c15d4`) |
| DVT-18 | Read-before-sample / never-gated is loud | Done (T13, `3bbdc6c`) |
| DVT-19 | `HostElement` + blanket `Element` adapter, no new solver concept | Done (T14, T15, `fa4f78d`, `b603132`) |
| DVT-20 | Host device declared in PHDL via `@device`, composes with hierarchy | Done (T17, `b93cbc5`) |
| DVT-21 | Same declaration serves plugin and host binding; Rust device graduates | Done (T17, `b93cbc5`) |
| DVT-22 | Python exception mid-solve → typed error with time, clean stop | Done (T27, `5e1f1eb`) |
| DVT-23 | `set_param` mid-transient: restamp fine, rebuild loud | Done (T14, `fa4f78d`) |
| DVT-24 | Level-1 convergence veto surfaced as a limiting report | Done (T16, `962c1a7`) |
| DVT-25 | Coroutine resumed exactly once per accepted breakpoint, never on rejection | Done (T19, `4aa4ef2`) |
| DVT-26 | One `wait`, three wake sources, mapped to existing `EventKind`s | Done (T19, T23, T24, `4aa4ef2`, `81147be`, `8ff01af`) |
| DVT-27 | Digital edge wait resumes only after settling | Done (T23, `81147be`) |
| DVT-28 | Multiple sequencers union breakpoints; authored POM order at a tie | Done (T26, `cf1c791`) |
| DVT-29 | `expect`/`warn` into the findings channel, posture-honoring | Done (T22, `880ee8c`) |
| DVT-30 | Levels compose in one class | Done (T18, `6b4c04b`) |
| DVT-31 | Zero host crossings inside Newton, counted | Done (T21, `a6a2319`, `abe5049`) |
| DVT-32 | `install` is install-relative; replacement registers its edge | Done (T20, `b04f58b`) |
| DVT-33 | Declared impedance; explicit ideal force | Done (T21, `a6a2319`) |
| DVT-34 | Internal-net drive allowed and always reported (D14) | Done (T21, `a6a2319`) |
| DVT-35 | Digital drive via `EventSink`, never the MNA | Done (T21, `a6a2319`) |
| DVT-36 | AC loud; DC runs no coroutine and uses per-kind `Wave` DC values | Done (T25, `3f5b26c`) |
| DVT-37 | Capability parity proven on four twinned devices | Done (T30, `5c73083`) |
| DVT-38 | Python/Rust surface parity enforced by a guard | Done (T29, `32f6016`) |
| DVT-39 | Monitors as ordinary `digital` modules firing at settled events | Done (T31, `ea6ec22`) |
| DVT-40 | `cover` bins reuse the pointwise path + bin-mapper | Done (T33, `be43d3b`) |
| DVT-41 | `cover=on\|off` own posture (D9) | Done (T33, `be43d3b`) |
| DVT-42 | Cross bin cap loud at elaboration, raisable (D10) | Done (T32, `35873da`) |
| DVT-43 | Host merge across runs; empty-bin identification | Done (T34, `00aa576`) |
| DVT-44 | cross-cutting: the fail-loud catalog (Edge Cases) | Done (T26, `cf1c791`) |
| DVT-45 | `docs/spec/` updated: Part VII, Part VIII + appendix C, Part I/II + appendix B (`cover`), appendix A (worked example) | Done (T35, T36, T37, `1c7049a`, `b3b9242`, `3dc8018`) |

**Coverage:** 45 total, **45 mapped** to T1–T37 (`tasks.md` Check 4).

---

## Success Criteria

- [ ] A **hysteretic PWM controller reading two terminals** runs with **zero**
      host calls inside Newton — the case that rejected revision 2's design.
- [ ] A **load-transient test** installs a step, measures droop and recovery with
      published error bounds, and **adapts within the same run** based on what it
      measured (`ideal.md` example E).
- [ ] A **UART-echo test** written as a coroutine drives and checks bytes over
      thousands of clock edges, findings carrying real simulation times, **zero**
      host calls inside any Newton iteration.
- [x] The **same device written in PHDL and in the host language produces
      identical results**, for one device per level-1 family.
- [ ] A test that **forces an internal node still passes, and the findings say it
      forced one**.
- [ ] The same test logic in Rust and Python produces identical findings.
- [ ] `piperine-signal` builds with **no in-tree dependency**, and each extracted
      family agrees with codegen's copy on a cross-check fixture.
- [ ] Every measurement the tool cannot make exactly **reports its bound**; every
      gradient-shaped guess it cannot make **fails loud** — no plausible wrong
      numbers, including AC (D13) and unrefined peaks.
- [x] `cover` over a sweep names its empty bins and merges across two runs; an
      oversized cross is refused at elaboration.
- [ ] `docs/spec/` carries the tester API normatively, plus a worked example.
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green.

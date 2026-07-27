# dv-tester Specification — the programmatic tester and verification at scale

**Vision:** `.specs/features/design-verification/ideal.md` §4 and §9 items 10–15.
Decisions D1–D16 are binding; this spec cites them.
**Surface design:** `ideal.md` in this directory — the four primitive kinds, the
three-layer library, the two host faces, and the implementability review.
**Depends on:** `dv-core` (the `validation_reports()` channel is how a tester
speaks; margins are what `cover` reuses).
**Sibling:** `dv-gradients`.
**Scope tier:** Large/Complex. **ROADMAP:** feeds the new P8.

> ## ⚠️ STATUS: DEFERRED (2026-07-27)
>
> **This spec is on hold and must not be broken into tasks yet.** The framing
> changed after it was written: `ideal.md` in this directory now designs a
> **host-language device library over the `Element` ABI** — Python or Rust, same
> contract, spanning "place this stamp" to "ramp a sigmoid" to "burst on the clock
> edge" — of which a tester is one use. This spec still describes the narrower
> "programmatic tester", so its scope, requirements, and success criteria are all
> undersized relative to the accepted design.
>
> **Rewrite it against `ideal.md` before planning tasks.** What changes:
> the three composing access levels (raw stamping / `Wave` descriptors /
> sequencing) rather than one sequencer; `HostElement` and its blanket `Element`
> adapter; the published cost model per level; the GIL cost of the Python tier;
> where D11's no-rollback guarantee applies (sequencing only — a level-0 device
> participates in rejected steps); and the six open questions in `ideal.md` §9,
> led by the relationship to the existing `#[pip::device]` spelling.
>
> **Sequencing decision:** `dv-core` and `dv-gradients` are planned and go first.
> Building them will settle several of the open questions by contact — especially
> how the findings channel behaves in practice, which this feature depends on.

The third mechanism, and the one that adds **no grammar**: a tester is an element
that advances time and reacts imperatively in Python (or Rust). Adopting it is
what let RNM leave V1 (D16) and shrank in-language verification to what only the
kernel can do.

## Problem Statement

There is no way to test the digital side from inside a simulation. A `constraint`
covers invariants at the solver's rhythm, but a functional test is imperative and
temporal — drive, wait, sample, compare, repeat — and expressing that as a PHDL
`digital` FSM means hand-coding a state machine for what is naturally a
straight-line program. Meanwhile driving stimulus from Python today means
stepping the simulation from outside, and there is no way for a host-side test to
participate in the time loop at its own rate. The pieces are already in the ABI:
`Element::next_breakpoints` (`crates/piperine-solver/src/core/element.rs:187`)
lets an element declare its own wake-up times and the transient driver already
lands on them. Nothing is wired to a host.

## Goals

- [ ] A sequencer `Element`: `next_breakpoints` + a host resume callback, so the
      **test** chooses its evaluation instants (§4.1).
- [ ] Host API: `advance(dt)`, `drive_voltage`/`drive_current`/`ramp` with
      declared impedance, digital `drive`, `read_voltage`/`read_port`,
      `expect`/`warn` into `dv-core`'s findings channel.
- [ ] Analog drive with **no host crossing inside Newton**: the host installs
      stamp *segments*, the element stamps them between breakpoints (§4.2).
- [ ] Generator ergonomics in Python; one contract, Rust parity (D15).
- [ ] Rollback-free by construction: explicit `advance` is the guarantee (D11).
- [ ] `cover` bins with host-side accumulation and its own posture (D9, D10).
- [ ] Monitors for the residue a tester cannot reach: continuous obligations at
      the solver's rhythm, or checks that must ship with a block.

## Out of Scope

| Feature | Reason / owner |
|---|---|
| `tol`, `constraint`, margins, postures, the findings channel itself | `dv-core` |
| Gradients, optimizer, Monte Carlo, centering | `dv-gradients` |
| **Tester in AC** | Known gap (D13) — refused loudly, not solved here |
| **RNM / `behavioral` body kind** | Dropped from V1 (D16); the tester is what replaced it |
| Implementing the declared `resolve` kinds | MD-24 declared-language debt, own ROADMAP item |
| An in-language sequence syntax (`##`-style) for monitors | Inadmissible until usage proves the generator form inadequate (§10.2) |
| Coverage closure *tooling* (dashboards, merge UI) | Host/CI work beyond this feature; the data model and merge semantics are in scope |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Confirmed? |
|---|---|---|
| Rollback | Explicit `advance(dt)` inserts a breakpoint and asserts nothing the tester observes changes inside the window; the tester resumes only at accepted points. No rollback protocol | **y** (D11) |
| Accepted cost of D11 | Landing on every tester breakpoint constrains the solver's timestep, so a fine-grained tester runs slower — a known inefficiency | **y** (D11) |
| Side effects | Allowed — file reads, golden models, logging. Reproducibility is the tester author's responsibility, not policed | **y** (D12) |
| Tester in AC | Fails loud; DC treatment of installed segments also unspecified | **y** (D13) |
| Hierarchy reach | Read internal nets freely; driving an internal net is allowed but always reported in the run's findings | **y** (D14) |
| Language | One `Element` contract driven by a host resume callback; Python and Rust both implement it | **y** (D15) |
| `cover` posture | `cover=on\|off` on `Context`, separate from `checks=` | **y** (D9) |
| Cross-coverage bin cap | Loud at elaboration against a `Context`-raisable default | **y** (D10) |
| Analog drive impedance | Declared; an ideal force is available but never the default | **y** (§4.2) |
| Digital drive path | Through the digital `EventSink` — never the MNA | n (agent) |
| One tester per design or many | Many, each with its own breakpoint set; the solver unions them | n (agent) |

**Open questions:** none blocking. Two items to revisit with usage: whether the
generator form needs sugar, and whether the monitor residue justifies a sequence
syntax (§10.2).

---

## User Stories

### P1: The sequencer element ⭐ MVP

**User Story**: As the solver, I want a tester to be an ordinary element that
declares its own wake-up times, so nothing about the transient driver changes.

**Why P1**: The seam everything else in this spec hangs on.

**Acceptance Criteria**:

1. WHEN a tester is instantiated THEN it SHALL be an `Element` declaring its
   wake-up times through the existing `next_breakpoints(from, horizon)`
   (`core/element.rs:187`), and the transient driver SHALL land on them with no
   change to its own contract.
2. WHEN the solver reaches a tester breakpoint and accepts the step THEN the
   host callback SHALL be resumed exactly once for that breakpoint.
3. WHEN a step containing a tester breakpoint is **rejected** THEN the host SHALL
   NOT be resumed for it — the tester observes accepted points only (D11).
4. WHEN several testers are instantiated THEN their breakpoint sets SHALL union,
   and each SHALL be resumed independently at its own instants.
5. WHEN a tester is present THEN `SimHooks` SHALL NOT be involved — it is
   coarse-grained (`transform_design`/`before_lower`/`after_solve`) and has no
   per-step site.
6. WHEN a tester's program returns THEN the analysis SHALL continue to its
   configured stop time unless the tester requests otherwise.

**Independent Test**: a recording tester asserting it is resumed exactly at its
declared instants, never on a rejected step, and that two testers interleave
correctly.

---

### P1: Host tester API and the generator model ⭐ MVP

**User Story**: As a test author, I want to write a test as a sequence of actions
in time — the way I think about bring-up — in Python.

**Why P1**: The whole point; this is the functional-verification path.

**Acceptance Criteria**:

1. WHEN a Python test is written as a generator THEN `yield t.advance(dt)` SHALL
   return control to the solver, which runs to the breakpoint and resumes the
   function there, with imperative state living in the generator frame.
2. WHEN `t.expect(cond, msg)` fails THEN it SHALL emit an `Error` finding through
   `dv-core`'s `validation_reports()` channel with time and instance provenance —
   not a print — and SHALL honor the active posture (`strict` aborts, `collect`
   records).
3. WHEN `t.warn(...)` is called THEN it SHALL emit a `Warning` finding, which
   never aborts.
4. WHEN `t.read_voltage(net)` / `t.read_port(name)` are called at a breakpoint
   THEN they SHALL return the accepted-point value.
5. WHEN a tester drives a digital net THEN it SHALL go through the digital
   `EventSink` and SHALL NOT touch the MNA.
6. WHEN the same test logic is written in Rust THEN it SHALL use the same element
   contract and produce identical findings (D15); Python's generator form is
   ergonomics over one shared contract, not a second mechanism.
7. WHEN a tester performs side effects (reads a vector file, consults a golden
   model) THEN that SHALL be allowed, and the documentation SHALL state that
   reproducibility becomes the author's responsibility — it is the one link a
   Monte Carlo `(seed, index)` does not cover (D12).

**Independent Test**: a UART-echo-shaped fixture driving and checking bytes;
assert findings carry the right times, that `strict` aborts on the first
mismatch, that `collect` records all of them, and that a Rust twin produces
identical findings.

---

### P1: Analog drive without a host crossing inside Newton ⭐ MVP

**User Story**: As a test author, I want to drive any analog stimulus
imperatively — steps, ramps, arbitrary piecewise shapes — without the simulation
calling back into Python inside a Newton iteration.

**Why P1**: This is what made RNM unnecessary (D16); without analog drive the
tester is a digital-only tool.

**Acceptance Criteria**:

1. WHEN the host calls `t.drive_voltage(net, v, rout=…)` or
   `t.ramp(net, from, to, dur)` THEN it SHALL install a stamp **segment**, and
   the element SHALL stamp from that description until the next breakpoint — with
   **no** host crossing inside any Newton iteration.
2. WHEN a drive is installed THEN its impedance SHALL be declared; an ideal force
   SHALL be available but spelled explicitly and never defaulted (§4.2).
3. WHEN a discontinuous drive (an ideal step) is installed THEN its breakpoint
   SHALL be registered so the integrator sees the edge instead of interpolating
   across it.
4. WHEN a tester drives an **internal** net THEN the drive SHALL be allowed and
   SHALL appear in the run's findings, so a passing test that forces a node is
   visibly doing so (D14).
5. WHEN a tester is instantiated and an **AC** analysis is requested THEN the
   request SHALL fail loud (D13) — never silently ignore or freeze the tester.
6. WHEN a tester is instantiated and a **DC/OP** analysis is requested THEN the
   behavior SHALL be defined and documented; if left unspecified in this
   iteration it SHALL fail loud rather than guess.

**Independent Test**: a ramp into an RC — assert the node follows the segment
analytically, assert zero host calls occur between breakpoints (counted), assert
an ideal step registers its breakpoint, and assert AC with a tester is loud.

---

### P2: Monitors — the residue a tester cannot reach

**User Story**: As a block author, I want an obligation that must be watched
continuously, or that should ship with my block rather than with someone's test,
expressed as an ordinary module.

**Why P2**: Most protocol checking happens at clock edges — the tester's rhythm —
so this is the narrow remainder, and the tester must exist first to know what
remains.

**Acceptance Criteria**:

1. WHEN a monitor is written THEN it SHALL be an ordinary `digital` module
   (registers, `match`) with no new grammar, reporting through
   `validation_reports()`.
2. WHEN a monitor fires THEN it SHALL do so at the digital scheduler's **accepted
   events** — after event settling, not mid-delta-cycle.
3. WHEN a monitor is instantiated in an array or parameterized THEN it SHALL
   behave as any module does — that composability is the reason not to import an
   assertion language.
4. WHEN a monitor's obligation could have been checked by a tester at its own
   rhythm THEN documentation SHALL prefer the tester — the monitor is for the
   continuous or ship-with-the-block cases.

**Independent Test**: a protocol monitor inside a hierarchy detecting a violation
the top-level tester cannot see, reporting with instance provenance.

---

### P3: `cover` — bins, accumulation, and closure data

**User Story**: As a verification lead, I want to know which operating regions my
regression actually exercised.

**Acceptance Criteria**:

1. WHEN `cover <name> : <expr> bins [lo:step:hi];` or
   `cover <name> : <expr> in {…};` appears in a `constraint` block THEN it SHALL
   reuse the pointwise evaluation path from `dv-core` with a bin-mapper — a bin
   hit is a counter increment, cheaper per point than a margin (no signed
   distance, no argmin).
2. WHEN `cover=off` (D9) THEN no coverage kernel SHALL be called; coverage SHALL
   have its **own** posture rather than riding `checks=`, so an optimizer's 10³
   inner-loop iterates cannot pollute the coverage database.
3. WHEN a `cover cross` is declared THEN its joint bin count SHALL be checked at
   **elaboration** against a `Context`-raisable cap and refused loudly if it
   exceeds it (D10) — bin edges are literal, so the product is known statically.
4. WHEN multiple runs complete THEN their counters SHALL merge on the host, and
   the merged data SHALL identify empty bins.
5. WHEN coverage is reported THEN it SHALL be host state that persists across
   runs and seeds — unlike a margin, which belongs to one analysis.

**Independent Test**: a sweep that fills a known subset of bins; assert the empty
bins are named, assert a second run merges rather than replaces, assert an
oversized cross is refused at elaboration.

---

## Edge Cases

- A tester whose `advance(dt)` would step past the analysis stop time → clamped
  at the stop time, and the tester is told the run ended rather than silently
  never resumed.
- `advance(0)` → legal (act again at the same instant) but must not livelock; a
  bounded number of zero-advance resumes per instant, then loud.
- A tester that never yields → loud, not an infinite hang.
- A tester reading a net that does not exist → loud at the first call, listing
  candidates.
- A tester driving the same net as another tester or a design source → loud
  conflict, not a silent last-writer-wins.
- A tester present with `checks=off` → its `expect` findings still matter; the
  posture governs `require` margins, so the interaction must be defined
  explicitly rather than inherited by accident.
- A generator raising an exception mid-test → surfaced as a test failure with the
  simulation time at which it happened, and the analysis stopped cleanly.
- `cover` with zero accepted points (analysis never ran) → all bins empty and
  reported as not-exercised, never a vacuous 100%.

---

## Requirement Traceability

| ID | Story | Status |
|---|---|---|
| DVT-01 | Sequencer element via `next_breakpoints` | Pending |
| DVT-02 | Host resumed exactly once per accepted breakpoint | Pending |
| DVT-03 | Never resumed on a rejected step (D11) | Pending |
| DVT-04 | Multiple testers union their breakpoints | Pending |
| DVT-05 | Generator model: `advance` yields, state in the frame | Pending |
| DVT-06 | `expect`/`warn` into the findings channel, posture-honoring | Pending |
| DVT-07 | `read_voltage`/`read_port` at accepted points | Pending |
| DVT-08 | Digital drive via `EventSink`, never the MNA | Pending |
| DVT-09 | Rust parity on one contract (D15) | Pending |
| DVT-10 | Side effects allowed; reproducibility documented as the author's (D12) | Pending |
| DVT-11 | Analog drive by installed segments; zero host calls inside Newton | Pending |
| DVT-12 | Declared impedance; explicit ideal force | Pending |
| DVT-13 | Discontinuity registers its breakpoint | Pending |
| DVT-14 | Internal-net drive allowed and always reported (D14) | Pending |
| DVT-15 | Tester + AC fails loud (D13); DC defined or loud | Pending |
| DVT-16 | Monitors as ordinary `digital` modules on the channel | Pending |
| DVT-17 | Monitor fires at settled digital events | Pending |
| DVT-18 | `cover` bins reuse the pointwise path + bin-mapper | Pending |
| DVT-19 | `cover=on\|off` own posture (D9) | Pending |
| DVT-20 | Cross bin cap loud at elaboration, raisable (D10) | Pending |
| DVT-21 | Host merge across runs; empty-bin identification | Pending |
| DVT-22 | cross-cutting: fail-loud catalog (Edge Cases) | Pending |
| DVT-23 | `docs/spec/` updated: Part VIII + appendix C (the tester API), Part VII (element contract), Part I/II + appendix B (`cover`), appendix A (worked example) | Pending |

**Coverage:** 23 total, 0 mapped (tasks phase not started).

---

## Success Criteria

- [ ] A UART-echo test written as a Python generator drives and checks bytes over
      thousands of cycles, with findings carrying real simulation times, and
      **zero** host calls occurring inside any Newton iteration.
- [ ] A tester ramps an analog supply and checks an analog threshold — proving the
      analog-drive path that made RNM unnecessary (D16).
- [ ] A test that forces an internal node still passes, and the run's findings
      say it forced one.
- [ ] The same test logic in Rust produces identical findings.
- [ ] `cover` over a sweep names its empty bins and merges across two runs; an
      oversized cross is refused at elaboration.
- [ ] Instantiating a tester and asking for AC fails loud.
- [ ] `docs/spec/` carries the full tester API normatively, plus a worked
      example — this surface is learned from examples, not from grammar tables.
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green.

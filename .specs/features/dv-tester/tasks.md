# dv-tester Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and
follow its Execute flow and Critical Rules.** Do not search for skill files by
filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Spec**: `.specs/features/dv-tester/spec.md` (DVT-01..45)
**Surface design**: `.specs/features/dv-tester/ideal.md` — **read first**; the
level taxonomy, the four level-1 families, the cost model, and the decision log
(§12) are all there. Every task below cites it.
**Design**: `.specs/features/dv-tester/design.md` — where things live.
**Vision**: `.specs/features/design-verification/ideal.md` §4 (D1–D16 binding)
**Depends on**: `dv-core` — `validation_reports()` is the findings channel;
`dv-gradients` is delivered and not a dependency.
**Follow-on**: `.specs/features/codegen-simplification/` — this feature
**extracts** into `piperine-signal`; that one **delegates** codegen to it. The
cross-check fixtures of T6 are this feature's obligation to that one.
**Status**: Phase 4 complete — Phase 5 next

## Progress log

| Batch | Phase | Tasks | Commits | Result |
|---|---|---|---|---|
| 1 | Phase 1 | T1–T6 | T1 `2a114e8`, T2 `1447cde`, T3 `35a13ba`, T4 `afda1af`, T5 `4ed9569`, T6 `996045e` | All green: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (0 failures), `cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` (0 warnings) |
| 2 | Phase 2 | T7–T13 | T7 `52b131c`, T8 `be11a05`, T9 `dda2c96`, T10 `419aa55`, T11 `564f88f`, T12 `e6c15d4`, T13 `3bbdc6c` | All green: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (0 failures), `cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` (0 warnings) |
| 3 | Phase 3 | T14–T18 | T14 `fa4f78d`, T15 `b603132`, T16 `962c1a7`, T17 `b93cbc5`, T18 `6b4c04b` | All green: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (0 failures), `cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` (0 warnings). SPEC_DEVIATION: `HostAdapter<T>` local newtype anchors the blanket `Element` impl (Rust orphan rule); `piperine-signal`'s `Wave`/`Hysteresis`/`BehaviorKind` gained `Send + Sync` (Rc→Arc, Cell→AtomicBool, a supertrait) so a `Behavior`/`Wave`-holding `HostElement` can satisfy `Element`'s unconditional `Send + Sync` bound — pure marker-trait/representation changes, no logic change, all of `piperine-signal`'s own tests unchanged and green. |
| 4 | Phase 4 | T19–T26 | T19 `4aa4ef2`, T20 `b04f58b`, T21 `a6a2319`/`abe5049`, T22 `880ee8c`, T23 `81147be`, T24 `8ff01af`, T25 `3f5b26c`, T26 `cf1c791` | All green: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (0 failures), `cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` (0 warnings). `piperine-api/src/sequencer/` split into `driver.rs`/`control.rs`/`findings.rs`/`install.rs`/`drive.rs`/`wait.rs`/`crossing.rs`/`conflicts.rs` (mod.rs stays declarations-only, MD-34). SPEC_DEVIATION: `HostElement` gained three narrow additive hooks beyond T14's original mirror — `drain_digital_drives` (T21), `validation_reports` (T22), `on_digital_settled` (T23) — each independently defaulted, extending the documented "mirror grows when a task gives a method bearing" rule rather than widening `accept_timestep`'s signature. T23 is the one task touching `piperine-solver`: `Element::on_digital_settled`, a defaulted no-op dispatched from `CircuitInstance::accept_and_run_digital` strictly after digital settle, not a new `EventKind` variant (mechanically verified). T25's AC refusal (D13) needed a small `Session`/`HostDeviceRegistry` extension (`register_sequencer`/`host_registry`) rather than a solver capability bit, since neither downcasting (MD-01) nor a second solver touch was available. |

---

## Test Coverage Matrix

> Generated from codebase sampling and project guidelines — confirm before Execute.
> Guidelines found: `CLAUDE.md` (fail-loud rule, "zero warnings is the bar", the
> named numeric oracles, MD-28 placement), `AGENTS.md` (MD-13), `.specs/STATE.md`
> (MD-31: a policy invariant lives in the gate), `tests/suite_hygiene.rs`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `piperine-signal` state machines | **unit, inline** (MD-28 r1) | every rule in `ideal.md` §3.9 has a named test; extracted tests move unchanged | `#[cfg(test)] mod tests` in the same file | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-signal` |
| `piperine-signal` crate boundary | integration | **purity asserted mechanically**, not by inspection (MD-31) | `crates/piperine-signal/tests/*.rs` | same as above |
| Extraction ↔ codegen agreement | integration | one cross-check fixture **per family**, values asserted equal | `crates/piperine-signal/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| `piperine-api` ABI + sequencer | integration | every policy: happy path, loud refusal, counted host calls | `crates/piperine-api/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-api` |
| `piperine-solver` subscriber | integration | settled-only proven; rejection path proven | `crates/piperine-solver/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-solver` |
| `piperine-python` bindings | integration | every Python-visible name; parity with Rust | `crates/piperine-python/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-python` |
| Host surface / cross-crate (MD-28 r2) | integration | capability parity; twinned devices end to end | `tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| `docs/spec/` (markdown) | none | build gate only | `docs/spec/*.md` | build gate only |

**Numeric oracles for this feature specifically:**

| Claim | Oracle |
|---|---|
| Extraction changed nothing | the relocated unit tests, unchanged, **plus** `tests/ngspice_validation.rs` (30 tests) still green — the limiter and operators feed it |
| Crate agrees with codegen's copy | four cross-check fixtures (one per family), values asserted equal, not "both ran" |
| A `Behavior` Jacobian is right | `verify_derivatives` — central finite difference, the same oracle pattern `dv-gradients` used against `.sens` |
| `Measure` is trustworthy | an overshoot fixture with an **analytically known** peak between accepted points: the reported bound must contain it |
| Crossing tier 2 converges | an analytically known crossing instant; the published error must shrink toward the step floor |
| Zero host calls inside Newton | a counted call test — the count is asserted, never inspected |
| Capability parity | the same device written both ways, results asserted **identical** |

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | Task touching exactly one crate's internals | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p <crate>` |
| Full | Task touching the host surface, Python, or more than one crate | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| Build | Phase completion, doc-only tasks | `CARGO_PROFILE_DEV_DEBUG=0 cargo build --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo clippy --workspace --all-targets -- -D warnings && CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` |

Same three binding rules as `dv-core`/`dv-gradients`: `CARGO_PROFILE_DEV_DEBUG=0`
always (the workspace test build is ~63 GB without it), the doc gate excludes
`piperine-python`/`piperine-cli` (pre-existing `numpy` rustdoc ICE), and
`--workspace` always.

---

## Execution Plan

### Phase 1: The crate, and the extraction
```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Level 1's new families
```
T7 → T8 → T9 → T10 → T11 → T12 → T13
```

### Phase 3: The ABI seam
```
T14 → T15 → T16 → T17 → T18
```

### Phase 4: The sequencer
```
T19 → T20 → T21 → T22 → T23 → T24 → T25 → T26
```

### Phase 5: Python and parity
```
T27 → T28 → T29 → T30
```

### Phase 6: Monitors and coverage
```
T31 → T32 → T33 → T34
```

### Phase 7: The formal spec
```
T35 → T36 → T37
```

**Natural split point:** Phase 6 (`cover`) is a different subject riding along.
If this feature needs to be cut, cut there — Phases 1–5 and 7 deliver the device
library and its documentation as a coherent whole.

---

## Task Breakdown

### T1: `piperine-signal` — the crate, and its purity guard

**What**: Create the crate and the mechanical proof that it stays pure.
**Where**: `crates/piperine-signal/` (new), workspace `Cargo.toml`
**Depends on**: None
**Reuses**: the workspace's existing crate conventions (edition 2024, `//!`
headers, MD-13)
**Requirement**: DVT-01
**Critical**: purity is the crate boundary's entire justification (`ideal.md`
§3.1). A guard that can be satisfied by inspection is not a guard — MD-31 says a
policy invariant lives in the gate.

**Done when**:
- [x] the crate builds, is in the workspace, and its `//!` header states the purity contract verbatim
- [x] its in-tree dependency list is **empty** — asserted by a test that reads the manifest, not by reading it
- [x] a test asserts no `Element`, MNA/matrix, or Cranelift symbol appears anywhere in the crate's sources
- [x] the guard is proven able to fail: add a workspace dependency, watch it fail, revert (MD-31)
- [x] `CLAUDE.md`'s crate-responsibilities table gains its row

**Tests**: integration · **Gate**: quick
**Commit**: `feat(signal): add the piperine-signal crate with a purity guard`

---

### T2: Extract the runtime operators

**What**: Move `delay`/`slew`/`transition`/`idt`/`idtmod` into the crate with
every semantic rule intact.
**Where**: `crates/piperine-signal/src/ops.rs`, from
`crates/piperine-codegen/src/device/analog/operators.rs`
**Depends on**: T1
**Reuses**: the source file's existing tests — they move with it
**Requirement**: DVT-02
**Critical**: `ideal.md` §3.9 A catalogues the rules. `transition`'s re-anchor,
its first-observation seeding, and its zero-ramp handling were each learned from
a failure; rewriting from the operator's *name* would silently drop them.

**Done when**:
- [x] `delay` keeps the history-ring rule: drop samples older than `t − td` **but keep one at or before it**, interpolate linearly with a clamped fraction, fall back to the input on empty history
- [x] `slew` keeps the asymmetric rate limit (`rise` climbing, `fall` falling), `dt` floored at zero
- [x] `transition` keeps **all four**: single pending ramp, re-anchor from the current output on retarget, first-observation settling (no ramp from zero), zero-ramp instantaneous past the delay edge with no divide-by-zero
- [x] `transition::pending_edges` still declares edges **only while a ramp is pending** — settled or unseeded contributes none
- [x] `idt`/`idtmod` keep the implicit-Euler accumulation and the modulus wrap, with the in-step `dt·x` term left to the consumer
- [x] every relocated test passes **unchanged**; codegen still compiles against its own copy (untouched this task)

**Tests**: unit, inline · **Gate**: quick
**Commit**: `feat(signal): extract the runtime operators from codegen`

---

### T3: Extract the event detectors

**What**: Move `cross`/`above`/`timer` detection into the crate.
**Where**: `crates/piperine-signal/src/events.rs`, from
`crates/piperine-codegen/src/device/analog/events.rs`
**Depends on**: T2
**Reuses**: the source's `EventDetector`
**Requirement**: DVT-03
**Critical**: the seeding asymmetry is a real semantic difference, not an
inconsistency to tidy away (`ideal.md` §3.9 B).

**Done when**:
- [x] `cross` fires on a sign change between accepted samples, per direction (rising/falling/either)
- [x] an **unseeded `cross` never fires**; an **unseeded `above` does** fire on its first sample — both asserted by name
- [x] `timer` keeps the `next_fire` advance loop and the phase rule (first fire at `phase` when positive, else at `period`)
- [x] `timer` still contributes static breakpoints while crossings still do not — the distinction is preserved and commented as the tier-1/tier-2 boundary (`ideal.md` §3.4)
- [x] relocated tests pass unchanged

**Tests**: unit, inline · **Gate**: quick
**Commit**: `feat(signal): extract the event detectors from codegen`

---

### T4: Extract `table`

**What**: Move the 1-D lookup, keeping it a **pure piecewise function**.
**Where**: `crates/piperine-signal/src/behavior/table.rs`, from
`crates/piperine-codegen/src/resolve/pom/analog_ops.rs`
**Depends on**: T3
**Reuses**: the existing validation rules and error messages
**Requirement**: DVT-04
**Critical**: the design rule to preserve is that no hand-written derivative
exists — the Jacobian is the segment slope and therefore cannot be wrong
(`ideal.md` §3.9 C).

**Done when**:
- [x] `table(x, xs, ys)` evaluates as linear interpolation with **end clamp**
- [x] its derivative is the segment slope, obtained from the piecewise form rather than written by hand
- [x] every validation stays loud: non-constant arrays, unequal lengths, fewer than two points, non-strictly-increasing `xs`, and any interpolation mode but `"linear"`
- [x] the general principle is recorded in the module header: a `Behavior` expressible as a pure piecewise function **must** be, because its derivative is then free

**Tests**: unit, inline · **Gate**: quick
**Commit**: `feat(signal): extract the table lookup from codegen`

---

### T5: Extract the junction limiter

**What**: Move pnjlim/fetlim with all five convergence rules.
**Where**: `crates/piperine-signal/src/limit.rs`, from
`crates/piperine-codegen/src/device/analog/limits.rs`
**Depends on**: T4
**Reuses**: the source's `Limiter`
**Requirement**: DVT-05
**Critical**: the least guessable knowledge in the tree (`ideal.md` §3.9 D), and
what a stiff `Behavior` (high-gain comparator, sharp switch) will need.

**Done when**:
- [x] slots seed with **`vcrit`**, a function of params/temperature only, never node voltages
- [x] the **seed-preservation** rule survives: skip the update while a slot is still at its seed and the new value is lower
- [x] each iteration limits against the previous **limited** value
- [x] the **convergence veto** survives with its tolerance (`1e-6 + 1e-4·|vnew|`), so post-limiting jitter does not veto forever
- [x] the **move-the-minus-node** rule survives, keeping a shared plus node fixed
- [x] `tests/ngspice_validation.rs` stays green — it is the oracle that proves junction devices still converge

**Tests**: unit, inline · **Gate**: full (the ngspice oracle is cross-crate)
**Commit**: `feat(signal): extract the junction limiter from codegen`

---

### T6: Cross-check fixtures — the drift guard

**What**: One fixture per extracted family, exercising codegen's copy and the
crate's, asserting equal values.
**Where**: `crates/piperine-signal/tests/`
**Depends on**: T5
**Reuses**: existing PHDL fixtures that already exercise each operator
**Requirement**: DVT-06
**Critical**: this is the **obligation this feature owes
`codegen-simplification`** (`design.md` §C1). Between the two features each
family exists twice; without these fixtures the copies drift and the follow-on
becomes "reconcile two behaviors" instead of "delete a copy".

**Done when**:
- [x] four fixtures — operators, detectors, `table`, limiter — each running both paths on the same inputs
- [x] values are asserted **equal**, not merely both-finite or both-ran
- [x] each fixture's header states it is deleted by `codegen-simplification`, naming that feature
- [x] a deliberate divergence injected into one path is caught (guard proven able to fail, MD-31)

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `test(signal): cross-check extracted families against codegen`

---

### T7: `Wave` — value from time

**What**: The descriptor catalog, native evaluation, self-declared breakpoints,
and each kind's DC value.
**Where**: `crates/piperine-signal/src/wave.rs`
**Depends on**: T6
**Reuses**: `transition::pending_edges`'s "publish your own discontinuities"
contract (T2) — one interface, not a special case per kind
**Requirement**: DVT-07, DVT-08
**Care**: `Wave.from_fn` is the one kind that costs a host call. It is available,
priced, and never the default (`ideal.md` §3.2).

**Done when**:
- [x] the catalog ships: `const`/`ramp`/`pulse`/`sin`/`exp`/`sigmoid`/`pwl`/`sampled`/`from_fn` plus `sum`/`scaled`/`shifted`
- [x] every kind but `from_fn` evaluates natively with **zero** host calls, asserted by count
- [x] `sampled` accepts an arbitrary precomputed array at the same zero cost
- [x] each kind publishes its own discontinuities through the shared `pending_edges`-shaped interface
- [x] each kind declares its **DC value** per `ideal.md` §12.10 (SPICE convention: `const`→value, `ramp`→start, `pulse`→initial level, `sin`→offset, `pwl`/`sampled`→first point, `sigmoid`→`lo`, `from_fn`→evaluated at `t = 0`)

**Tests**: unit, inline · **Gate**: quick
**Commit**: `feat(signal): add the Wave descriptor catalog`

---

### T8: `Behavior` — value from circuit state

**What**: The reactive family: closed-form value **and** closed-form Jacobian, so
a device reacts at every Newton iteration for zero host calls.
**Where**: `crates/piperine-signal/src/behavior/`
**Depends on**: T7
**Reuses**: T4's `table` as a member of the family and as the design precedent
**Requirement**: DVT-09, DVT-10, DVT-11 (Rust half)
**Critical**: this is the family whose absence made "a PWM comparator reading two
terminals" unanswerable (`ideal.md` §3.3). The acceptance case is that device.

**Done when**:
- [x] the catalog ships: `comparator`/`hysteresis`/`switch`/`limiter`/`vcvs`/`vccs`/`vco`/`table` plus `sum`/`scaled`/`composed`
- [x] each kind contributes **cross Jacobian entries** `∂f/∂vᵢ` from a closed form
- [x] smooth is the default (`comparator` is `tanh`-shaped, differentiable); a genuinely discontinuous kind registers a crossing instead
- [x] a hysteretic PWM controller reading two terminals runs with **zero** host calls inside Newton — counted, not inspected
- [x] `BehaviorKind` is a public trait: a new kind written in Rust composes exactly like a shipped one

**Tests**: unit, inline + integration · **Gate**: quick
**Commit**: `feat(signal): add the Behavior family with closed-form Jacobians`

---

### T9: `verify_derivatives` — the FD oracle

**What**: The guard on every hand-written Jacobian.
**Where**: `crates/piperine-signal/src/verify.rs`
**Depends on**: T8
**Reuses**: the central-finite-difference oracle pattern `dv-gradients` used
against `.sens`
**Requirement**: DVT-12
**Care**: a wrong analytic derivative does not crash — it converges slowly or to
the wrong place. This is the only thing between a new `BehaviorKind` and a
plausible wrong answer.

**Done when**:
- [x] `verify_derivatives(kind, points)` compares analytic against central FD over a sample grid
- [x] every shipped `Behavior` kind passes it, run as a test
- [x] a **deliberately wrong** Jacobian is caught — the guard proven able to fail
- [x] the failure names the kind, the input point, and both values

**Tests**: unit, inline · **Gate**: quick
**Commit**: `feat(signal): add the finite-difference derivative oracle`

---

### T10: Crossing detection, tier 2

**What**: Predict the crossing instant, bound the step toward it, iterate, and
**publish the residual error**.
**Where**: `crates/piperine-signal/src/events.rs`
**Depends on**: T9
**Reuses**: T3's detectors (tier 1); the `$bound_step` seam, which already exists
end to end (`ideal.md` §3.9 F)
**Requirement**: DVT-10
**Critical**: the audit found PHDL's own `@cross` fires late by up to a timestep
and says nothing (`ideal.md` §3.4). Tier 2 needs **no solver change**; tier 3 is
a separate solver feature and is out of scope here.

**Done when**:
- [x] a bracketed sign change yields an interpolated instant **with an error bound**
- [x] the detector emits a step bound that walks the next step toward the predicted instant
- [x] against an analytically known crossing, the published error shrinks toward the step floor
- [x] refinement **terminates at the step floor and reports** rather than stalling
- [x] the module header states plainly that this is tier 2, what tier 1 (today's PHDL `@cross`) does, and what tier 3 would require

**Tests**: unit, inline + integration · **Gate**: quick
**Commit**: `feat(signal): predict and bound analog crossings with an error bound`

---

### T11: `Measure` — streaming reductions with error bounds

**What**: The accumulators, and the honesty rule that makes them usable.
**Where**: `crates/piperine-signal/src/measure/`
**Depends on**: T10
**Reuses**: T10's crossing machinery for `cross`/`settle`/`delay`
**Requirement**: DVT-13, DVT-15, DVT-16
**Critical**: a peak reduced over accepted points **misses the true peak** between
them. For overshoot — the archetypal analog check — that is a plausible wrong
number (`ideal.md` §3.6).

**Done when**:
- [x] the catalog ships: `peak`/`min`/`average`/`rms`/`ripple`/`integral`/`cross`/`settle`/`delay`/`count`
- [x] each is an **O(1) streaming accumulator** fed one `(t, v)` per accepted point — never storing a waveform
- [x] every value travels with an **error bound** derived from the local LTE
- [x] `refine=True` drives step refinement until the bound meets a requested tolerance, and **terminates at the step floor with a report**
- [x] `MeasureKind` is a public trait — extensible in Rust, exactly as `Behavior` is
- [x] an overshoot fixture whose analytic peak falls **between** accepted points: the reported bound contains it, and `refine=True` narrows it

**Tests**: unit, inline + integration · **Gate**: quick
**Commit**: `feat(signal): add streaming measures with published error bounds`

---

### T12: `Measure` gating

**What**: Windows by time, by analog signal, and by digital net.
**Where**: `crates/piperine-signal/src/measure/`
**Depends on**: T11
**Reuses**: T10's crossing predicates for analog gates
**Requirement**: DVT-17
**Critical**: a digital gate observes **settled** events only — and the reason is
correctness, not taste: a mid-delta-cycle value can be an artifact of DAG
evaluation order rather than a signal the circuit had (`ideal.md` §3.5).

**Done when**:
- [x] `window=(t0, t1)` gates by time
- [x] an analog gate (`above(net, level)`) opens and closes on a crossing predicate
- [x] a digital gate opens and closes on **settled** digital events only — never mid-delta-cycle, asserted by a test that would catch an unsettled read
- [x] a measure whose window opens more than once accumulates across all open intervals, not just the first
- [x] the settled-only rationale is in the module header, stated as the DAG-artifact argument

**Tests**: unit, inline · **Gate**: quick
**Commit**: `feat(signal): gate measures by time, analog level, and digital net`

---

### T13: `Measure` lifecycle, and the online/post-run identity

**What**: Arm, read, disarm — and the proof that the same code reduces a stored
waveform and a live transient identically.
**Where**: `crates/piperine-signal/src/measure/`, `crates/piperine-signal/tests/`
**Depends on**: T12
**Reuses**: `piperine-api`'s `Waveform` shape as the post-run input (by value —
the crate must not depend on it; the test constructs samples directly)
**Requirement**: DVT-14, DVT-18

**Done when**:
- [x] `arm` creates a **fresh** accumulator; arming twice yields two independent handles
- [x] reading a value before any sample has fed it fails **loud** naming the reason — never `NaN`, zero, or a nominal
- [x] a gated measure whose window never opened fails loud as "never sampled"
- [x] `disarm` freezes the value, which stays readable afterwards
- [x] the same reduction over the same sample sequence gives identical results whether streamed live or replayed post-run — asserted on values

**Tests**: unit, inline + integration · **Gate**: build (phase completion)
**Commit**: `feat(signal): define the measure lifecycle and prove replay identity`

---

### T14: `HostElement` and the blanket `Element` adapter

**What**: The trait, and the adapter that makes the solver unable to tell a host
device from any other.
**Where**: `crates/piperine-api/src/device/`
**Depends on**: T13
**Reuses**: `Element`'s defaulted shape (`solver/core/element.rs`) — `HostElement`
mirrors it method for method
**Requirement**: DVT-19, DVT-23
**Critical**: MD-01 — one ABI, no downcast. No solver code may learn that host
devices exist.

**Done when**:
- [x] `HostElement` has two required methods (`name`, `capabilities`) and defaults the rest, mirroring `Element`
- [x] a blanket `impl<T: HostElement> Element for HostAdapter<T>` presents it to the solver (a local newtype anchor — the orphan rule forbids `impl<T> Element for T` directly across crates; see adapter.rs's SPEC_DEVIATION)
- [x] a trivial Rust host device (a resistor) stamps into a real circuit and solves
- [x] a host device's `set_param` mid-transient goes through the live-parameter path for restamp-class and fails **loud** for rebuild-class (MD-18) — inherited for free: `Session::set` already dispatches through `Introspect::set_param` generically
- [x] no `Any`, no downcast, no new solver-side concept — asserted by the absence of solver changes in this task's diff

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add HostElement and its blanket Element adapter`

---

### T15: `StampSink`, `Ctx`, `StateView`

**What**: The safe wrappers — the real work of exposing matrix rows and solution
values across an FFI boundary.
**Where**: `crates/piperine-api/src/device/`
**Depends on**: T14
**Reuses**: `Stamp` (`solver/src/math/linear.rs`); `load_dc`'s sink pattern
**Requirement**: DVT-19
**Care**: this is the soundness surface. A raw pointer handed to Python is the
failure this task exists to prevent.

**Done when**:
- [x] the sink exposes only operations (`g(row, col, v)`, `i(row, v)`) — never a `&mut` to the matrix (a branch row is just another `AnalogReference`, so `g`/`i` cover `branch(...)` too)
- [x] an out-of-range index is a **loud typed error**, never undefined behavior
- [x] views are borrow-scoped to one call; nothing a device holds can outlive the step
- [x] no allocation per call on the stamping path — asserted by shape, not by benchmark

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add index-validated stamp and state views`

---

### T16: Level 1 into stamps

**What**: The adapter that turns a `Wave`/`Behavior` into MNA rows, feeds
`Measure` accumulators from accepted points, and surfaces the limiter's veto.
**Where**: `crates/piperine-api/src/device/`
**Depends on**: T15
**Reuses**: T7/T8/T11's evaluation; T15's sink
**Requirement**: DVT-24
**Critical**: a level-1 mechanism can **veto convergence** (the limiter's rule,
`ideal.md` §3.9 D4). Swallowing that signal produces a device that converges to a
non-solution.

**Done when**:
- [x] an installed `Wave` stamps as a source with its declared impedance and its breakpoints reach `next_breakpoints`
- [x] an installed `Behavior` stamps value **and** cross-Jacobian entries every Newton iteration, with zero host calls — counted
- [x] `Measure` accumulators are fed **only on accepted steps**, proven by a rejected-step test that contributes nothing
- [x] a limiter's still-clamping state surfaces as a `limiting_report`, gating convergence exactly as a PHDL device's does

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): stamp level-1 descriptors and surface the limiting veto`

---

### T17: `@device` gains a host binding mode

**What**: A PHDL-declared `@device` module resolves to an in-process
implementation.
**Where**: `crates/piperine-api/src/`, `crates/piperine-lang/` (resolution only)
**Depends on**: T16
**Reuses**: the existing `@device` attribute (`headers/device_port.phdl`, MD-24)
and the plugin resolution path it already feeds
**Requirement**: DVT-20, DVT-21
**Critical**: **one concept, two binding modes** (`ideal.md` §8.7). The PHDL does
not change between them; the mode is inferred from where the implementation is
registered. **No new macro** (MD-13 r5).

**Done when**:
- [x] a host device is declared in PHDL with `@device` and instantiated like any module — in a hierarchy, with parameters, **more than once**
- [x] circuit build resolves an `@device` module to a plugin binding **or** a host registration, failing loud naming both when neither is found
- [x] the same PHDL declaration serves either mode unchanged — proven by binding one module both ways in one test
- [x] a Rust host device registers without a macro; the registration is an explicit call
- [x] the graduation path (host → plugin, no PHDL change) is documented, and the Python limitation (cannot graduate) is stated

**Tests**: integration · **Gate**: full
**Commit**: `feat(api): resolve @device modules to in-process host implementations`

---

### T18: A Rust host device, end to end

**What**: The phase's proof: a non-trivial Rust host device composing all three
levels.
**Where**: `crates/piperine-api/tests/`
**Depends on**: T17
**Reuses**: everything in Phases 1–3
**Requirement**: DVT-30

**Done when**:
- [x] one class stamps directly (level 0), installs a `Wave` and a `Behavior` (level 1) — the composition `ideal.md` example G describes
- [x] the hysteretic PWM of `ideal.md` example D runs end to end in a real circuit
- [x] a rejected timestep leaves level-1 operator state untouched while the level-0 half participates normally (`ideal.md` §7's table, proven per row)
- [x] host-call count inside Newton is **zero** for the level-1 parts, counted

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `test(api): prove a Rust host device composing all three levels`

---

### T19: The coroutine driver and time wakes

**What**: `advance(dt)`, resumed strictly after acceptance.
**Where**: `crates/piperine-api/src/sequencer/`
**Depends on**: T18
**Reuses**: `Element::next_breakpoints`; T14's `HostElement`
**Requirement**: DVT-25, DVT-26 (time source)
**Critical**: the resume site must be **after** step acceptance, never inside the
accept/reject decision (D11, `design.md` §C4). A sequencer resumed during a step
later rejected breaks the model with no clean repair.

**Done when**:
- [x] a `Sequencer` is a `HostElement` whose `next_breakpoints` comes from its pending wake
- [x] the host is resumed **exactly once** per accepted breakpoint
- [x] a rejected step containing a sequencer breakpoint resumes **nothing** — proven by a recording sequencer
- [x] `advance(0)` is legal but bounded: a limited number of zero-advance resumes per instant, then loud
- [x] a coroutine that never yields is loud, not an infinite hang
- [x] `advance` past the stop time clamps and tells the sequencer the run ended
- [x] `SimHooks` is **not** involved — asserted by its absence from the path

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add the sequencer coroutine driver with time wakes`

---

### T20: `install` semantics

**What**: Install-relative time, and discontinuities that are registered rather
than smoothed.
**Where**: `crates/piperine-api/src/sequencer/`
**Depends on**: T19
**Reuses**: T7's `Wave`; T16's stamping
**Requirement**: DVT-32
**Critical**: `ideal.md` §4.2 — absolute time would make every mid-run install
require arithmetic the author eventually gets wrong; silent smoothing would make
a result impossible to explain. Both rules are counter-intuitive enough to need
a test each.

**Done when**:
- [x] a wave installed at `t_i` has its time origin at `t_i` — a ramp installed mid-run ramps over `[t_i, t_i + dur]`, asserted numerically
- [x] replacing an active wave takes effect **immediately** and registers its discontinuity as an `Exact` breakpoint
- [x] the replacement is **not** re-anchored from the current value — `const(100e-3)` over `const(10e-3)` steps, because that is what was asked for
- [x] continuity is expressible explicitly, and the documented idiom is shown in a test

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): define install-relative wave semantics`

---

### T21: Drive — analog, digital, and the reach rule

**What**: Driving with declared impedance, digital through the event sink, and
internal-net forcing that always reports itself.
**Where**: `crates/piperine-api/src/sequencer/`
**Depends on**: T20
**Reuses**: the digital `EventSink`; `dv-core`'s findings channel
**Requirement**: DVT-31, DVT-33, DVT-34, DVT-35
**Critical**: D14 — reading internal nets is free, driving one is allowed **but
always reported**, so a passing test that forces a node is visibly doing so.

**Done when**:
- [x] `drive_voltage`/`drive_current` require a declared impedance; an ideal force is available but spelled explicitly and never defaulted
- [x] a digital drive goes through the `EventSink` and **never** touches the MNA — asserted
- [x] driving an internal net is allowed and **appears in the run's findings**
- [x] host calls between breakpoints are **counted and asserted zero** — the invariant the whole design exists to protect
- [x] a ramp into an RC follows the analytic segment response

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add analog and digital drive with declared impedance`

---

### T22: `expect` / `warn` into the findings channel

**What**: The sequencer speaks through `dv-core`, not through prints.
**Where**: `crates/piperine-api/src/sequencer/`
**Depends on**: T21
**Reuses**: `dv-core`'s `validation_reports()` and posture handling
**Requirement**: DVT-29

**Done when**:
- [x] `expect(cond, msg)` failing emits an `Error` finding with **time and instance provenance** — not a print
- [x] `warn(...)` emits a `Warning` that never aborts
- [x] posture is honored: `strict` aborts on the first failure, `collect` records all of them
- [x] an `expect` against a `Measure` compares against the **error bound**, not the point estimate (`ideal.md` §4.3)
- [x] a coroutine raising an exception surfaces as a test failure carrying the simulation time, with the analysis stopped cleanly
- [x] the interaction with `checks=off` is defined explicitly rather than inherited by accident

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): report sequencer findings through the dv-core channel`

---

### T23: The dynamic digital subscriber, and edge waits

**What**: The one solver change — a subscriber list consulted after net settling
— and `wait(edge(...))` on top of it.
**Where**: `crates/piperine-solver/src/`, `crates/piperine-api/src/sequencer/`
**Depends on**: T22
**Reuses**: `EventKind::Digital` (already `priority: Exact`); the existing settle
path
**Requirement**: DVT-26 (digital source), DVT-27
**Critical**: **after settling only** (`ideal.md` §3.5/§12.3) — a mid-delta-cycle
value can be a DAG-evaluation-order artifact, so waking on one gives results that
move when an unrelated edit reorders the graph. Not a new event kind and not a
new scheduling concept: a registration point plus a post-settle dispatch.

**Done when**:
- [x] a waiter registers on a digital net at runtime and is resumed on the matching edge
- [x] resumption happens **only after settling** — proven by a fixture whose mid-delta-cycle value differs from its settled one
- [x] the mechanism is a subscriber list, not a new `EventKind` — asserted by the absence of a new variant
- [x] a rejected step does not resume a waiter (D11 holds for edge waits too)
- [x] the seam's other plausible consumers are named in its `//!` header (`design.md` §C5), so a future reader knows it is not tester-only

**Tests**: integration · **Gate**: full
**Commit**: `feat(solver): add a post-settle digital subscriber for edge waits`

---

### T24: Analog crossing waits

**What**: The third wake source, on T10's tier-2 machinery.
**Where**: `crates/piperine-api/src/sequencer/`
**Depends on**: T23
**Reuses**: T10's crossing prediction and error bound
**Requirement**: DVT-26 (analog source)

**Done when**:
- [x] `wait(above(net, level))` resumes at the predicted crossing, within the published bound
- [x] the reported wake instant carries its error bound — the sequencer never claims an exactness it does not have
- [x] all three wake forms compose in one coroutine, and the mapping to `Digital`/`Breakpoint`/`Crossing` is asserted
- [x] a crossing that never occurs before the stop time ends the run cleanly, telling the sequencer

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): wait on analog crossings with a reported bound`

---

### T25: DC, AC, and the analyses a sequencer changes

**What**: What happens when a sequencer is present and the analysis is not
transient.
**Where**: `crates/piperine-api/src/`
**Depends on**: T24
**Reuses**: T7's per-kind DC values
**Requirement**: DVT-36
**Critical**: D13 defers AC **loudly**. DC is defined rather than guessed
(`ideal.md` §12.10).

**Done when**:
- [x] an **AC** analysis with a sequencer present fails loud naming the gap (D13) — never silently freezes it
- [x] at **DC/OP** the coroutine does not run; `run()` begins with the transient
- [x] each installed `Wave` contributes its declared DC value at the operating point, per kind
- [x] a soft-start example's DC bias matches the documented convention, asserted numerically

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): define DC behavior and refuse AC for sequencers`

---

### T26: Many devices at one instant — ordering and conflicts

**What**: Determinism where it is legitimate, loudness where it is not.
**Where**: `crates/piperine-api/src/`
**Depends on**: T25
**Reuses**: the authored POM instantiation order (`ideal.md` §12.12)
**Requirement**: DVT-28, DVT-44

**Done when**:
- [x] several sequencers union their breakpoints and are each resumed independently at their own instants
- [x] at a shared instant they act in **authored POM instantiation order** — deterministic and visible to the designer, asserted
- [x] two devices driving the **same net** is a **loud conflict** regardless of order — never last-writer-wins
- [x] the feature's fail-loud catalog is complete and each entry has a test: unknown net (listing candidates), driver conflict, non-yielding coroutine, rebuild-class `set_param`, unfed measure, AC with a sequencer

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): order host devices and refuse drive conflicts`

---

### T27: The PyO3 bridge

**What**: Duck-typed method dispatch, one exception catch point, and the GIL cost
stated.
**Where**: `crates/piperine-python/src/`
**Depends on**: T26
**Reuses**: the existing embedded-Python host isolation
**Requirement**: DVT-22

**Done when**:
- [ ] a Python class subclassing `pip.Device` is dispatched on **method presence** — it implements only what it needs
- [ ] a Python exception mid-solve maps to a **typed error carrying the simulation time**, and the analysis stops cleanly
- [ ] the GIL cost is documented as a property of the Python tier, with level 1 named as the way to avoid it
- [ ] a Python device and a Rust device coexist in one circuit

**Tests**: integration · **Gate**: quick
**Commit**: `feat(python): bridge host devices into the embedded interpreter`

---

### T28: The Python surface

**What**: `pip.Device`, `pip.Wave`, `pip.Behavior`, `pip.Measure`,
`pip.Sequencer`, `@pip.device`.
**Where**: `crates/piperine-python/src/`, the pure-Python facade
**Depends on**: T27
**Reuses**: T7/T8/T11's catalogs, exposed as closed enumerations
**Requirement**: DVT-11 (Python half)
**Critical**: the catalog is **closed** in Python — the documented escape hatch is
level 0, not an extension point (`ideal.md` §3.7).

**Done when**:
- [ ] every level-1 catalog entry is reachable from Python with **identical spelling** to Rust
- [ ] the catalog is closed: no Python-side registration of a new `Behavior`/`Measure` kind, and attempting it fails with a message naming level 0 and the Rust route
- [ ] `@pip.device` registers a Python host device; the generator form of `Sequencer.run` works
- [ ] `ideal.md`'s examples A–G all run as written

**Tests**: integration · **Gate**: quick
**Commit**: `feat(python): expose the host device library surface`

---

### T29: Host parity guard

**What**: The mechanical proof that the two hosts are one surface.
**Where**: `tests/host_parity.rs`
**Depends on**: T28
**Reuses**: `host_parity.rs`'s existing enumeration mechanism
**Requirement**: DVT-38

**Done when**:
- [ ] every name added by this feature exists on both hosts with identical spelling and shape
- [ ] one fixture through both hosts asserts identical findings **and** identical measure values with identical bounds
- [ ] the closed-Python/open-Rust catalog asymmetry is the **only** divergence, and it is declared in the guard rather than silently tolerated
- [ ] the guard is proven able to fail: add a Rust-only name, watch it fail, revert

**Tests**: integration · **Gate**: full
**Commit**: `test(host): enforce parity across the device library surface`

---

### T30: Capability parity, proven

**What**: The feature's hard success criterion — the same device written both
ways gives the same answer.
**Where**: `tests/`
**Depends on**: T29
**Reuses**: existing PHDL fixtures as the reference half
**Requirement**: DVT-37
**Critical**: this is the criterion `codegen-simplification` inherits
(`ideal.md` §2 of that feature). Without it "capability parity" is a claim.

**Done when**:
- [ ] four twinned devices — a `transition`-driven source, a pnjlim junction, a `table` nonlinearity, a `cross`-triggered element — each written in PHDL **and** as a host device
- [ ] each pair produces **identical** results on the same fixture, asserted on values
- [ ] any PHDL capability **not** reachable from the host is named in the documentation — a scoped promise, not an eroding one
- [ ] the test's header states that `codegen-simplification` extends this set rather than replacing it

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `test(host): prove capability parity between PHDL and host devices`

---

### T31: Monitors

**What**: The residue a tester cannot reach — continuous obligations, or checks
that ship with a block.
**Where**: no new code; `crates/piperine-api/tests/` or root `tests/`
**Depends on**: T30
**Reuses**: ordinary `digital` modules + `dv-core`'s channel
**Requirement**: DVT-39

**Done when**:
- [ ] a monitor is an ordinary `digital` module with **no new grammar**, reporting through `validation_reports()`
- [ ] it fires at the scheduler's **settled** events, never mid-delta-cycle
- [ ] it composes in an array and under parameterization, as any module does
- [ ] a hierarchical fixture: a monitor detects a violation the top-level tester cannot see, reporting with instance provenance
- [ ] documentation prefers the tester where the obligation could be checked at the tester's rhythm

**Tests**: integration · **Gate**: quick
**Commit**: `test(api): prove monitors as ordinary digital modules`

---

### T32: `cover` — grammar, POM, and the elaboration cap

**What**: The statement kind, and the bin-count refusal that happens before
anything allocates.
**Where**: `crates/piperine-lang/src/parse/`, `pom/`, `elab/`
**Depends on**: T31
**Reuses**: the `constraint` block's existing parse path
**Requirement**: DVT-42
**Care**: `crates/piperine-lang/src/parse/` is hand-written recursive descent —
changes ripple (CLAUDE.md).

**Done when**:
- [ ] `cover <name> : <expr> bins [lo:step:hi];` and `… in {…};` parse inside a `constraint` block
- [ ] `cover` joins the reserved-word list
- [ ] a `cover cross`'s joint bin count is checked at **elaboration** against a `Context`-raisable cap and refused loudly (D10) — bin edges are literal, so the product is known statically
- [ ] the refusal names the declared count and the cap

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): add the cover statement and its elaboration bin cap`

---

### T33: `cover` — bin mapping and its own posture

**What**: The counter increment, and the posture that keeps an optimizer out of
the coverage database.
**Where**: `crates/piperine-codegen/src/kernel/analog/`, `crates/piperine-solver/src/analyses/`
**Depends on**: T32
**Reuses**: `dv-core`'s pointwise evaluation path
**Requirement**: DVT-40, DVT-41
**Critical**: D9 — riding `checks=` would let `dv-gradients`' optimizer pollute
coverage with 10³ inner-loop iterates nobody meant as verification runs.

**Done when**:
- [ ] a bin hit reuses the pointwise path with a bin-mapper — a counter increment, with no signed distance and no argmin
- [ ] `cover=on|off` is a posture on `Context`, **separate** from `checks=`
- [ ] with `cover=off` no coverage kernel is called — asserted by count, not by output
- [ ] an optimizer loop with `cover=off` leaves the database untouched

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): map cover bins with their own posture`

---

### T34: `cover` — merge and closure data

**What**: The part whose value is accumulation across a whole regression.
**Where**: `crates/piperine-api/src/`
**Depends on**: T33
**Reuses**: the host result objects
**Requirement**: DVT-43

**Done when**:
- [ ] counters merge across runs and seeds on the host, rather than replacing
- [ ] merged data **identifies empty bins** by name
- [ ] coverage persists across runs — unlike a margin, which belongs to one analysis
- [ ] a run with zero accepted points reports all bins as not-exercised, never a vacuous 100%
- [ ] a sweep filling a known subset: the empty bins are named, and a second run merges

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): merge coverage across runs and name empty bins`

---

### T35: `docs/spec/` — the solver contract

**What**: The ABI delta, the subscriber, and the crossing-fidelity correction.
**Where**: `docs/spec/part_vii_solver.md`
**Depends on**: T34
**Reuses**: Part VII's numbered-section structure
**Requirement**: DVT-45
**Critical**: today's `@cross` documentation describes behavior more exact than
what the code does. This task **corrects** it — a gap the spec names is a
decision; a gap it omits is a bug report waiting.

**Done when**:
- [ ] `HostElement` and the blanket `Element` adapter documented
- [ ] the dynamic digital subscriber documented **with its settled-only rule and the DAG-artifact reason**
- [ ] resume-strictly-after-acceptance (D11) documented as a contract, not an implementation note
- [ ] drive segments and the declared-impedance requirement documented
- [ ] the **three crossing fidelity tiers** documented, stating plainly that today's `@cross` is tier 1 (fires at the first accepted point after, unreported) — correcting the existing text
- [ ] the AC refusal (D13) documented as a stated limitation with its reason

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document the host element ABI and crossing tiers`

---

### T36: `docs/spec/` — the host library

**What**: The largest documentation surface in the dv- family.
**Where**: `docs/spec/part_viii_host_api.md`, `docs/spec/appendix_c_host_surface.md`
**Depends on**: T35
**Reuses**: the host-API Part's structure
**Requirement**: DVT-45

**Done when**:
- [ ] the three levels documented **with their costs**, as the table that lets an author choose
- [ ] `Wave`, `Behavior`, `Measure` catalogs documented, including per-kind DC values
- [ ] the `Sequencer`, its one `wait` with three wake sources, and `install` semantics documented
- [ ] the measure lifecycle and **error bounds** documented, including that `expect` compares against the bound
- [ ] the closed-Python/open-Rust asymmetry documented as a decision with its reason
- [ ] the reach rules (D14) and posture interaction documented

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document the host device library surface`

---

### T37: `docs/spec/` — language, grammar, and the worked examples

**What**: `@device`'s host mode, `cover`'s grammar, and the examples this feature
will actually be learned from.
**Where**: `docs/spec/part_i_language.md`, `appendix_b_grammar.md`,
`part_ii_elaboration.md`, `appendix_a_worked_examples.md`
**Depends on**: T36
**Reuses**: the existing appendix structure
**Requirement**: DVT-45

**Done when**:
- [ ] `@device`'s **host binding mode** documented in Part I, with the graduation path and the Python limitation
- [ ] `cover` documented as a `constraint`-block statement kind and added to the reserved words
- [ ] `CoverStmt` added to the grammar appendix; bin resolution and the cross cap added to Part II
- [ ] appendix A gains **two** worked examples: a host device and a tester
- [ ] `CLAUDE.md`'s crate table carries `piperine-signal` (from T1) and the entry is accurate at close

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document @device host binding, cover, and worked examples`

---

## Validation Tables

### Check 1: Task granularity

Each task is one deliverable: one crate, one extracted family, one descriptor
catalog, one trait, one adapter, one wake source, one posture, or one pair of
spec documents. T23 spans solver and api because the subscriber and its consumer
are one mechanism proven end to end — still a single concept. T26 collects the
fail-loud catalog because a catalogue split across tasks is a catalogue nobody
completes.

### Check 2: Diagram ↔ `Depends on` cross-check

| Task | Diagram predecessor | `Depends on` | ✅ |
|---|---|---|---|
| T1 | — | None | ✅ |
| T2–T6 | T1…T5 | previous | ✅ |
| T7 | T6 (phase order) | T6 | ✅ |
| T8–T13 | T7…T12 | previous | ✅ |
| T14 | T13 (phase order) | T13 | ✅ |
| T15–T18 | T14…T17 | previous | ✅ |
| T19 | T18 (phase order) | T18 | ✅ |
| T20–T26 | T19…T25 | previous | ✅ |
| T27 | T26 (phase order) | T26 | ✅ |
| T28–T30 | T27…T29 | previous | ✅ |
| T31 | T30 (phase order) | T30 | ✅ |
| T32–T34 | T31…T33 | previous | ✅ |
| T35 | T34 (phase order) | T34 | ✅ |
| T36–T37 | T35, T36 | previous | ✅ |

The chain is strictly linear: every phase consumes the previous one's product.
The one place that could parallelize (Phase 6's `cover`, which touches nothing
Phases 1–5 built) is deliberately sequenced last because it is the split point.

### Check 3: Test co-location

| Task | Layer touched | Matrix requires | Task's `Tests` | ✅ |
|---|---|---|---|---|
| T1, T6 | crate boundary | integration | integration | ✅ |
| T2–T5, T7 | `piperine-signal` state machines | unit, inline | unit, inline | ✅ |
| T8–T13 | `piperine-signal` + its proofs | unit + integration | both | ✅ |
| T14–T18 | `piperine-api` ABI | integration | integration | ✅ |
| T19–T22, T24–T26 | `piperine-api` sequencer | integration | integration | ✅ |
| T23 | solver + api | integration | integration | ✅ |
| T27, T28 | `piperine-python` | integration | integration | ✅ |
| T29, T30, T31 | cross-crate host surface | integration | integration | ✅ |
| T32 | `piperine-lang` | integration | integration | ✅ |
| T33 | codegen + solver | integration | integration | ✅ |
| T34 | `piperine-api` | integration | integration | ✅ |
| T35–T37 | `docs/spec/` markdown | none (build gate) | none | ✅ |

### Check 4: Requirement coverage

| Requirement | Task | | Requirement | Task |
|---|---|---|---|---|
| DVT-01 | T1 | | DVT-24 | T16 |
| DVT-02 | T2 | | DVT-25 | T19 |
| DVT-03 | T3 | | DVT-26 | T19, T23, T24 |
| DVT-04 | T4 | | DVT-27 | T23 |
| DVT-05 | T5 | | DVT-28 | T26 |
| DVT-06 | T6 | | DVT-29 | T22 |
| DVT-07 | T7 | | DVT-30 | T18 |
| DVT-08 | T7 | | DVT-31 | T21 |
| DVT-09 | T8 | | DVT-32 | T20 |
| DVT-10 | T8, T10 | | DVT-33 | T21 |
| DVT-11 | T8, T28 | | DVT-34 | T21 |
| DVT-12 | T9 | | DVT-35 | T21 |
| DVT-13 | T11 | | DVT-36 | T25 |
| DVT-14 | T13 | | DVT-37 | T30 |
| DVT-15 | T11 | | DVT-38 | T29 |
| DVT-16 | T11 | | DVT-39 | T31 |
| DVT-17 | T12 | | DVT-40 | T33 |
| DVT-18 | T13 | | DVT-41 | T33 |
| DVT-19 | T14, T15 | | DVT-42 | T32 |
| DVT-20 | T17 | | DVT-43 | T34 |
| DVT-21 | T17 | | DVT-44 | T26 |
| DVT-22 | T27 | | DVT-45 | T35, T36, T37 |
| DVT-23 | T14 | | | |

All 45 mapped.

---

## Tools

**MCP**: none required. **Skill**: `tlc-spec-driven` (mandatory, per the Execution
Protocol above).

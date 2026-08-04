# Part IX — Runtime Semantics (piperine-signal)

This Part defines the runtime-semantic contract computed by `piperine-signal`:
the family of behaviors — runtime operators, event detection, the junction
limiter, table interpolation, closed-form behavior kinds, streaming measures,
time-domain waveforms, derivative verification, and four-state digital edge
logic — that used to have two implementations (one inside `piperine-codegen`'s
JIT-compiled devices, one inside a hand-written host device) and now have
exactly one. A compiled PHDL block (Part VII §5, via `piperine-codegen` — Part
X) and a host-written device (Part VIII) both compute these semantics by
calling into this crate; neither hand-rolls its own copy.

`piperine-signal` **computes values, derivatives, event predicates, and
running reductions. It never stamps, never sees a matrix, never sees an
`Element`, and depends on no other crate in the tree.** That purity is
mechanically guarded, not merely documented — the crate's own dependency list
is asserted empty by a permanent regression test, so a future change that
tries to pull in a matrix type, a solver type, or a codegen type fails the
build with a named test, not a silent architectural drift.

Everything in this Part is **pure computation**: given the same inputs (and,
for stateful kinds, the same prior state), every function and method here
returns the same output. Nothing here allocates an MNA unknown, writes a
stamp, or reads simulation time except as an explicit argument. Time,
values, and error bounds crossing this boundary are plain `f64`, matching
Part VII's convention (§3.3) — there is no typed-units layer here either.

## Contents

- §1 Purity contract and position in the pipeline
- §2 Runtime operators
- §3 Event detectors
- §4 The junction limiter
- §5 Table interpolation
- §6 Behavior — closed-form value and Jacobian
- §7 Measure — streaming reductions
- §8 Wave — value-from-time descriptors
- §9 Derivative verification
- §10 Digital edge logic

---

## §1 Purity contract and position in the pipeline

`piperine-signal` sits below both the PHDL compilation pipeline (Part X) and
the host device surface (Part VIII), and above neither: it has no upstream
dependency on either. Both consume it as a library.

```text
piperine-codegen ──┐
                    ├──► piperine-signal (pure: no matrix, no Element, no in-tree deps)
piperine-api  ──────┘
    (host devices)
```

**The purity rule, stated normatively:**

1. A type or function in this crate SHALL NOT accept, return, or hold a
   reference to a solver ABI type (an `Element`, a stamp, a matrix row/column
   index, an `AnalogReference`/`DigitalNet`).
2. A type or function in this crate SHALL NOT depend on simulation time
   except as an explicit `f64` argument supplied by the caller.
3. The crate's `[dependencies]` in its own manifest SHALL remain empty of
   every other crate in this workspace. A permanent guard test inspects the
   manifest directly and fails by name if that ever changes — enforcing the
   rule mechanically (in the spirit of Part VII §16's registry-plus-
   exhaustiveness pattern) rather than trusting review.

**Consequence — capability parity is structural, not incidental.** Because
neither `piperine-codegen` nor `piperine-api`'s host device layer can see
inside the other, and both call the *same* functions here for the *same*
family, a device written in PHDL and a device written by hand in the host
language that both use (for example) `transition` or the junction limiter are
guaranteed to compute identical values for identical inputs — not because the
two authors were careful, but because there is only one implementation to
call.

**What belongs here, and what does not.** A family belongs in this crate when
it is a pure function of its own inputs and (for stateful kinds) its own
prior state — nothing more. A family that needs matrix structure (device
loading and MNA stamping, Part VII §8), a compiled kernel (JIT emission, Part
X), or a live `Element` (checkpoint/restore, digital net snapshots) does not
move here even if its *formula* is simple; only the pure part moves, and the
family's other half stays exactly where it was. Two examples the crate
deliberately does not own: a noise power-spectral-density formula that is
three lines of arithmetic still lives in the JIT because it has no state and
no reuse pressure of its own; the ddt companion-model coefficients live in
the solver's integration method (Part VII §10.3) because they are a property
of the *integrator*, not of any one device.

---

## §2 Runtime operators

Five operators — `delay`, `slew`, `transition`, `idt`, `idtmod` — each
allocate one runtime state slot and are advanced **once per accepted
timestep**, never inside a Newton iteration and never on a step later
rejected. This is the operators' central invariant:

> **A rejected timestep SHALL leave every operator's state completely
> unchanged.** The caller is responsible for calling the advance method only
> after a timestep is accepted (Part VII §10.2's `accept_step` phase, via
> `Element::accept_timestep`); the operator has no way to enforce this on its
> own, since it has no notion of "accepted" or "rejected" — only of being
> asked to advance or not.

Each operator is constructed fresh (with its own initial state) and then
advanced by feeding it `(time, input)` pairs in monotonically non-decreasing
time order.

| Operator | Construction | State | Advance semantics |
|---|---|---|---|
| `delay(x, td)` | empty history | a `(time, value)` ring buffer | Push the new sample; linearly interpolate the value at `time − td` from the two bracketing retained samples, falling back to the oldest retained sample if `time − td` precedes it, or to the raw input if history is still empty. History strictly older than the target is dropped, but one sample at or before the target is always retained for interpolation. |
| `slew(x, rise, fall)` | output `0`, time `0` | current output value, last-advance time | The output moves toward the input by at most `rise·Δt` (rising) or `fall·Δt` (falling), where `Δt` is floored at `0` (a non-monotonic time input produces zero motion, never negative-rate motion). |
| `transition(x, td, rise, fall)` | unseeded | `start`, `target`, `t_change`, `seeded` | See below — the re-anchor rule. |
| `idt(x, ic)` | `value = ic`, time `0` | running integral, last-advance time | Implicit-Euler accumulation: `value += Δt·x` at each advance, `Δt` floored at `0`. |
| `idtmod(x, ic, modulus)` | `value = ic`, time `0` | same as `idt`, plus a modulus | Same accumulation, then wrapped into `[0, modulus)` when `modulus > 0`. |

### 2.1 `idt`'s initial-condition rule

The initial condition `ic` seeds the accumulator's `value` **at
construction**, before any advance. The first advance call after
construction (even one at `time = 0` producing `Δt = 0`) therefore returns
exactly `ic`, unaffected by that call's input — the initial condition is a
starting value, not a first sample folded into the accumulation.

### 2.2 `transition`'s seeding and re-anchor rule

`transition` is a **single pending ramp**: its state is `(start, target,
t_change)` plus a `seeded` flag, not a queue of pending changes.

**Seeding.** The first advance after construction is the seed: it commits
`start = target = input` at the seed time and returns `input` unchanged —
there is no ramp on the very first observation, matching a DC operating-point
seed. This is deliberately unlike `delay`'s history-based fallback and unlike
`idt`'s constructor-time `ic` seeding; each operator states its own seed
convention rather than sharing one.

**Steady state.** While the fed input equals the current `target`, the
operator has nothing new to ramp toward and returns the already-settled
output.

**A new input value (re-anchor).** When the fed input differs from the
current `target`:

1. The operator first evaluates its own ramp function at the current time
   using the *old* `(start, target, t_change)` triple, and commits that value
   as the new `start`.
2. `target` becomes the new input; `t_change` becomes the current time.

This means a new input arriving mid-ramp **re-anchors from wherever the
output currently is**, not from the ramp's original starting point — a
retarget partway up a rise produces a new ramp beginning at the
partially-completed value, never a discontinuous jump back to the old
`start`.

**The ramp function itself**, evaluated at any time `t`:

```text
t0 = t_change + max(delay, 0)
ramp = rise   if target >= start
       fall   otherwise
if t <= t0:                     output = start
elif ramp <= 0 or t >= t0+ramp: output = target
else:                           output = start + (target - start) * (t - t0) / ramp
```

A `rise`/`fall` of exactly `0` produces an instantaneous step at `t0` — the
delay edge still lands as a discontinuity, but there is no ramp duration to
divide by (the implementation SHALL NOT divide by zero in this case; the
branch above returns `target` directly whenever `ramp <= 0`).

### 2.3 Declared breakpoints

`transition` publishes its own future discontinuities so the integrator can
land on them exactly rather than stepping past them and interpolating across a
kink. The publishing method is `pending_edges(from, end)` — this crate's own
name, since it knows nothing of elements or breakpoints; the device layer
forwards what it returns to the solver as `Element::next_breakpoints` (Part VII
§15.9). `Wave` publishes through the identical shape (§8.4). Given a query
window `(from, end]`:

- A settled operator (unseeded, or `start == target`) contributes nothing —
  it has no pending edge.
- A ramping operator contributes `t0` (the delay-edge instant, where the ramp
  begins) when `t0` falls in `(from, end]`, and `t0 + ramp` (where the ramp
  completes) when that falls in `(from, end]` — including the case `ramp =
  0`, which contributes only the single instantaneous-step instant.

`delay`, `slew`, `idt`, and `idtmod` declare no breakpoints of their own:
their outputs are continuous functions of a continuous input (subject to the
input's own discontinuities, which are the input's responsibility to
declare, not the operator's).

---

## §3 Event detectors

Three trigger kinds — `Above`, `Cross(direction)`, `Timer` — share one
detector shape: remember what was observed at the previous accepted sample
(or the next scheduled fire time, for a timer), and decide whether the
*current* accepted sample fires.

### 3.1 The seeding asymmetry

**This asymmetry is deliberate, not an inconsistency, and any future change
to either detector must preserve it explicitly.**

- **`cross` never fires on an unseeded (first) observation.** A sign change
  is a comparison between two samples; with only one sample there is nothing
  to compare against, so the first call to the detector only *arms* it
  (records the value as `prev`) and returns `false` regardless of that
  value's sign.
- **`above` DOES fire on its first observation if the value is positive.**
  An unseeded `above` detector's internal "was it below zero last time"
  state defaults to `true` (as if the previous sample had been at or below
  zero), so a first sample that is already positive counts as a rise past
  zero and fires immediately.

Both detectors' `Cross`/`Above` fire tests, stated precisely:

| Trigger | Fires when (given previous value `prev`, current value `v`) | Unseeded behavior |
|---|---|---|
| `Above` | `rose && v > 0`, where `rose = (prev <= 0)` once seeded, `rose = true` while unseeded | Fires on the first sample iff `v > 0` |
| `Cross(Rising)` | `seeded && prev <= 0 && v > 0` | Never fires |
| `Cross(Falling)` | `seeded && prev >= 0 && v < 0` | Never fires |
| `Cross(Either)` | either of the two rows above | Never fires |

After every call, the detector records `v` as the new `prev` and marks itself
seeded, regardless of whether it fired.

### 3.2 Timer detectors

A timer detector tracks one `next_fire` instant, advanced by whole periods.
Given a period `p` and the current time `t`:

- The detector fires when `p > 0` and `t >= next_fire`.
- On firing, `next_fire` advances by whole multiples of `p` until it again
  exceeds `t` — a call that arrives long after the previous one (a large
  time jump) still lands on the correct period boundary rather than firing
  once per skipped period.
- The detector's *first* fire instant is supplied at construction: it is the
  timer's declared `phase` when `phase > 0`, otherwise the `period` itself —
  this resolution is the caller's responsibility (the detector only stores
  whatever first-fire instant it is given).

### 3.3 The tier-1/tier-2 boundary and static breakpoints

A timer's fire instants are known **a priori** — they depend only on
parameters, never on the solution — so a timer detector declares its future
fire instants as static breakpoints the integrator can land on exactly,
within a queried window `(from, end]`.

A crossing (`Above`/`Cross`) is **solution-dependent**: whether and when it
fires depends on the analog solve, so it cannot be declared in advance. This
is the tier-1/tier-2 boundary:

- **Tier 1** (this section's `EventDetector`) detects reactively — it fires
  at the first accepted sample after the sign change has already happened,
  contributing no static breakpoints. This is what PHDL's `@cross`/`@above`
  compile to.
- **Tier 2** (§3.4 below) *predicts* the crossing instant from the
  bracketing pair of accepted samples and publishes an honest error bound,
  without requiring any solver change — it does not retroactively make tier
  1 exact, it gives a caller (the host sequencer, in practice) a tool to
  narrow toward the true instant using the existing step-bound seam.
- **Tier 3** (exact — promoting the crossing to a hard breakpoint the
  integrator retries onto) is out of scope for this crate; it would require
  a solver-side change to reject-and-retry at a predicted instant.

### 3.4 Tier-2 crossing prediction

Given a bracketing pair of accepted samples `(t0, v0)` and `(t1, v1)` whose
sign change matches a requested direction, the **secant prediction** is:

```text
t_cross = t0 + (t1 - t0) · (-v0) / (v1 - v0)
```

reported together with an **error bound** equal to the bracket width `|t1 −
t0|` — the prediction never claims a precision it has not earned. A pair that
does not actually bracket a crossing of the requested direction (including
the degenerate case `v0 == v1`) produces no prediction.

The prediction carries a derived quantity, the **next-step bound**: the
distance from a given "from" time to the predicted crossing instant, floored
at `0` (once the caller has stepped past the prediction, no further capping
is proposed).

**Refinement.** A caller with access to a sampling function may narrow the
bracket by bisection: repeatedly probe the sample at the bracket's midpoint,
keep whichever half still straddles the sign change, and stop once the
bracket width reaches a caller-supplied floor (or an iteration budget is
exhausted). The refinement:

- SHALL report a final error bound no smaller than the actual bracket width
  achieved, floored at the caller's `step_floor` — it never claims a
  precision the loop did not reach.
- SHALL terminate at the step floor rather than continuing to consume the
  full iteration budget once the floor is reached.
- Uses bisection (not a repeated secant step) specifically because
  bisection's convergence is agnostic to the sampled function's curvature —
  a single secant step is exact only for a linear bracket.

This is the pure-math shape of what a real driver does by capping its next
timestep via the step-bound seam and re-detecting; this crate performs the
arithmetic, a caller with access to the solver performs the actual stepping.

---

## §4 The junction limiter

The junction limiter is ngspice's Newton-convergence aid for stiff
exponential and MOSFET-style nonlinearities (`devsup.c`'s `DEVpnjlim`/
`DEVfetlim`/`DEVlimvds`), reproduced with matching numeric behavior. It has
two parts: three **pure formulas** (this section, §4.1) and a **per-slot
bookkeeping type** (§4.2) that decides, from the formula's output, whether a
junction is still moving and therefore whether Newton convergence should be
vetoed for this iteration.

### 4.1 The three limiter formulas

Each formula computes a **limited** value `vlim` from the raw new value
`vnew`, the previous limited value `vold`, and (for `pnjlim`) a critical
voltage `vcrit`.

**`pnjlim(vnew, vold, vte, vcrit)`** — the pn-junction voltage limiter.

```text
dv = vnew - vold
if not (vnew > vcrit and |dv| > 2·vte):
    return vnew                                  # small step: pass through
arg = 1 + dv/vte
posval = vold + vte·ln(arg)   if arg > 0 else vcrit
negval = vte·ln(vnew/vte)
return posval if vold > 0 else negval
```

*Edge cases, normative:*

- **A small step (`|dv| <= 2·vte` or `vnew <= vcrit`) is not limited at
  all** — the formula returns `vnew` unchanged. This is the common case once
  a junction has converged near its operating point.
- **`arg <= 0`** (an excessively large negative step) clamps `posval` to
  `vcrit` rather than evaluating `ln` of a non-positive argument. The
  branchless-IR rendering of this formula (historically used in the
  compiled kernel) computes `ln` unconditionally and discards the result via
  a select when `arg <= 0`; because IEEE 754 permits computing `ln` of a
  non-positive value without trapping (it produces `NaN` or `−∞`, values
  that are simply never read on that branch), the two renderings — branch
  and branchless-select — produce identical observable results. This is the
  one place in this formula where "the same value" needs the argument above
  rather than a bare assertion.
- **`vte == 0`** produces `dv/vte = ±∞` in the guard test in both renderings
  identically, and the subsequent branch selection (`vnew > vcrit` combined
  with an infinite `|dv|`) resolves the same way regardless of rendering —
  there is no divergence introduced by this degenerate input.

**`fetlim(vnew, vold, vto)`** — the MOSFET gate-voltage limiter. A
region-selecting formula (on: `vold >= vto`; high: `vold >= vto + 3.5`) whose
four quadrant cases (on/off × high/mid) each clamp the step size relative to
`vold` differently, always producing a **finite** result across the entire
real-valued domain of `(vnew, vold)` — there is no input pair for which
`fetlim` is undefined.

**`limvds(vnew, vold)`** — the MOSFET drain-source voltage limiter. A
two-region formula (`vold >= 3.5` vs. below) that limits the step's growth
(`3·vold + 2` cap when rising, in the high region) or its magnitude (`[−0.5,
4]` clamp, in the low region), also finite over its entire domain.

*Rationale.* These three formulas exist because a stiff nonlinearity (a
forward-biased pn junction's exponential, a MOSFET's gate/drain-source
transition) can make an unconstrained Newton step overshoot wildly — the
limiter constrains each iteration's *proposed* value before it becomes the
next linearization point, without changing what the converged solution
actually is.

### 4.2 The limiter's five bookkeeping rules

Per-`$limit`-slot state — whether the slot is still actively clamping, and
its own seed voltage — obeys five rules that must survive any future
refactor of the bookkeeping type intact:

1. **Seed with `vcrit`, not zero.** Every slot's initial (`vold`) value is
   its own critical voltage `vcrit` — parameters/temperature only, **never**
   a node voltage — so a junction opens near its expected turn-on point
   rather than from `0 V` (matching ngspice's `MODEINITJCT` mode).
2. **Preserve the seed until first bias.** While a slot's stored value is
   still (within floating-point tolerance) equal to its seed, and the
   newly-computed limited value would be *lower* than the seed, the update
   is **skipped** — the slot's stored value stays at the seed. Without this
   rule, `pnjlim(0, vcrit) = 0` on the opening iterations (before the
   junction is genuinely biased) would discard the seed and let the node
   float toward the supply rail instead of opening near turn-on.
3. **Limit against the previous *limited* value, not the previous raw
   value.** Each accepted iteration's limited output becomes the next
   iteration's `vold` input to the formula — this iterative anchoring
   (rather than always limiting against the raw prior branch voltage) is
   what makes a stiff exponential converge instead of overshooting and
   stalling.
4. **Limiting vetoes Newton convergence, with a tolerance.** A slot counts
   as "still clamping" — and must veto the outer Newton loop's convergence
   test — exactly when the limited value differs from the iteration's raw
   (unlimited) value by more than `1e-6 + 1e-4·|raw|`. This tolerance exists
   specifically so that post-limiting numerical jitter (the raw and limited
   values agreeing to within ordinary Newton noise, once limiting has
   effectively stopped mattering) does not veto convergence forever.
5. **Move the minus node, not the plus, when linearizing.** When a limited
   branch value is realized as node voltages for the next linearization
   point, the branch's minus-side node is displaced (holding the plus-side
   node fixed) whenever the minus side is a real node — this keeps a shared
   plus node (for example, a bipolar transistor's internal base node, shared
   across multiple limited branches) from being pulled by every limiter
   independently. Only when the minus side is ground does the plus-side node
   move instead.

**Aggregation.** The per-instance "is any slot still limiting" flag is the
logical OR of every slot's individual clamping state (rule 4); the
first-clamping slot is nameable (for diagnostic reporting) as the lowest
slot index currently active.

**Two calling shapes.** The bookkeeping accepts either of two inputs for a
given advance:

- The formula computes `vlim` itself from `(vnew, kinds)`, where `kinds`
  supplies each slot's formula selection (`pnjlim`/`fetlim`/`limvds`) and its
  non-node parameter (`vte` for `pnjlim`, `vto` for `fetlim`, none for
  `limvds`) — the natural shape for a caller with no other source of
  `vlim`.
- A caller that has **already computed** `vlim` by some other means (for
  example, a compiled kernel that calls the same three formulas directly as
  linked functions) supplies the precomputed `vlim` together with the raw
  `vnew`, and only rules 2 and 4 are applied — this avoids recomputing the
  same formula twice while still sharing exactly one implementation of the
  seed-preservation and convergence-veto rules. In this calling shape the
  per-slot `vold` storage may be owned by the caller (for example, inside a
  larger state array that is checkpointed as a unit) rather than by the
  bookkeeping type itself; only the active-flag and seed bookkeeping is
  owned internally.

Both shapes execute the identical rule-2/rule-4 logic — there is exactly one
implementation of those two rules regardless of which calling shape a
consumer uses.

---

## §5 Table interpolation

`table(x, xs, ys)` is a **pure piecewise function**: 1-D linear interpolation
between `(xs[i], ys[i])` control points, clamped flat beyond both ends (never
extrapolated).

### 5.1 Validation

A table is validated at construction, before any evaluation is possible.
Construction SHALL fail (loud, never silently substituting a default) when
any of:

| Condition | Failure |
|---|---|
| Fewer than two `(x, y)` points | too few points |
| `xs` and `ys` have different lengths | length mismatch |
| `xs` is not strictly increasing (`xs[i] < xs[i+1]` for every consecutive pair) | not strictly increasing |
| The requested interpolation mode is anything other than `"linear"` | unsupported mode |

`"linear"` is, at present, the only supported interpolation mode; a table
requesting any other mode fails construction naming the requested mode.

### 5.2 Evaluation

For `x` strictly between `xs[0]` and `xs[n-1]`, the table locates the
enclosing segment `i` such that `xs[i] <= x < xs[i+1]` and evaluates:

```text
value(x)      = ys[i] + (x - xs[i]) · slope_i
derivative(x) = slope_i
where slope_i = (ys[i+1] - ys[i]) / (xs[i+1] - xs[i])
```

For `x <= xs[0]`, `value(x) = ys[0]` (flat clamp at the low end); for `x >=
xs[n-1]`, `value(x) = ys[n-1]` (flat clamp at the high end). In both clamp
regions, `derivative(x) = 0` — a flat extrapolation has no slope.

**The segment lookup is shared.** `value` and `derivative` locate the
enclosing segment through the same lookup — there is exactly one piece of
code that decides which segment (or which clamp region) a given `x` falls
into, and both the value formula and the derivative formula read from it.
This is a deliberate design invariant, not an accident of the current
implementation:

> A `Behavior` (§6) that can be expressed as a pure piecewise function
> **must** be, because its derivative then falls directly out of the same
> piecewise structure the value uses — read off, never separately
> hand-derived — and therefore cannot silently disagree with the value it is
> the derivative of. Hand-written Jacobians (§6) are reserved for kinds that
> genuinely carry state a pure piecewise structure cannot express.

### 5.3 Renderer accessors

Beyond `value`/`derivative`, a table exposes its validated `(xs, ys)` arrays
and its per-segment slope as read-only accessors, for the express purpose of
letting a **second renderer** — one that must build an equivalent
representation of the same piecewise structure at compile time, rather than
evaluate it at runtime — walk the already-validated segment geometry without
re-deriving or re-checking any of the validation rules in §5.1. These
accessors are not a second evaluation path: `value`/`derivative` remain the
only way to evaluate a table at a specific `x`; the accessors exist for a
consumer building a structurally equivalent representation (for example, a
nested conditional expression whose branches are the same segments), not for
evaluating one.

---

## §6 Behavior — closed-form value and Jacobian

A `Behavior` is `f(v_1, …, v_n)`: a value that depends on the current
circuit solution — the analog "reads its inputs every Newton iteration and
reacts" family, as distinct from `Wave` (§8), which never reads the
solution. Every behavior kind exposes both:

- **`value(inputs)`** — the behavior's output for the given ordered input
  slice.
- **`jacobian(inputs, out)`** — the partial derivative of the output with
  respect to each input, written into `out` (same length as `inputs`).

This closed-form-Jacobian shape is what lets a behavior contribute exact
cross-Jacobian entries to the Newton system every iteration, at zero
evaluation cost beyond the formula itself — there is no finite-difference
probing at solve time (§9 exists precisely so that finite differences are
used *once*, at authoring/verification time, to check the closed form, never
at every Newton iteration to compute it).

### 6.1 Smooth is the default; hard edges are opt-in

Most shipped kinds are smooth (infinitely differentiable) and therefore need
nothing beyond `value`/`jacobian`. A genuinely discontinuous kind additionally
implements a third, optional method:

- **`crossings(inputs)`** — reports a pending discontinuity as `(value,
  threshold, direction)` when the kind's output is about to (or has just)
  flipped, so an integrator can be steered to land on the edge rather than
  interpolate across it (Part VII §15.9). The default implementation reports
  none — every smooth kind is exempt from declaring crossings.

A discontinuous kind's `jacobian` is `0` in every region away from its own
edge (a stepped output has no local slope except exactly at the step) — it
does not attempt to approximate the edge with a steep-but-smooth Jacobian;
the edge is handled structurally, through `crossings`, not through the
derivative.

### 6.2 The shipped catalog

| Kind | Inputs | `value` | Smooth? |
|---|---|---|---|
| `comparator(a, b, lo, hi, gain)` | `[a, b]` | `lo + (hi−lo)/2 · (1 + tanh(gain·(a−b)))` | smooth |
| `hysteresis(a, b, vth_hi, vth_lo)` | `[a, b]` | a two-state Schmitt trigger: `1` once `a−b` rises past `vth_hi`, `0` once it falls past `vth_lo`, holding between | discontinuous |
| `switch(ctrl, ron, roff, vth)` | `[ctrl]` | `1/ron` while `ctrl >= vth`, else `1/roff` | discontinuous |
| `limiter(x, lo, hi)` | `[x]` | `clamp(x, lo, hi)` | continuous, kinked at the two corners (no crossing registered — see below) |
| `vcvs(a, b, gain)` | `[a, b]` | `gain·(a−b)` | smooth (linear) |
| `vccs(a, b, gm)` | `[a, b]` | `gm·(a−b)` — same closed form as `vcvs`; the two differ only in which downstream row a consumer feeds them into, not in this crate | smooth (linear) |
| `vco(ctrl, f0, kvco)` | `[ctrl]` | `f0 + kvco·ctrl` — the *instantaneous frequency*, not a phase or waveform (composing with `idt`, §2, turns a frequency into an oscillating phase) | smooth (linear) |
| `table(x, xs, ys)` | `[x]` | `Table::value`/`Table::derivative` (§5) exactly | continuous, kinked at breakpoints (no crossing registered — same convention as `limiter`) |
| `sum(a, b)` | shared with both operands | `a(inputs) + b(inputs)` | composition — smooth iff both operands are |
| `scaled(inner, k)` | shared with `inner` | `k · inner(inputs)` | composition |
| `composed(f, g)` | shared with `g`; `f` sees only `g`'s scalar output | `f(g(inputs))` — the general shape for a controller whose control signal is itself computed (e.g. a hysteretic PWM: `switch` driven by `hysteresis`, both authored purely) | composition |

**`limiter` and `table` are continuous but kinked** (a corner, not a jump):
neither registers a `crossings` entry, because there is no discontinuous
*value* for the integrator to land on — only a slope discontinuity, which
does not by itself demand a breakpoint the way a value jump does.

**`hysteresis`'s one bit of memory.** Unlike every other shipped kind,
`hysteresis` carries state: which side of its band it is currently on. This
is exactly the "value from circuit state" a comparator-with-memory is — it
is not a `Wave` (which never reads the solution) and it is not stateless (it
cannot answer `value` without knowing which side it was last on). Reading
`value` may itself flip the internal state (crossing a threshold on the way
in) and the just-updated state is what the following `crossings` call
reports against.

**`crossings` propagation through composition.** `sum`/`scaled`/`composed`
each forward `crossings` from whichever operand(s) declare one (for `sum`,
whichever operand reports first; for `composed`, `f`'s crossing over `g`'s
scalar output takes priority over `g`'s own crossing) — a composition of a
discontinuous kind with anything else does not silently drop the
discontinuity.

---

## §7 Measure — streaming reductions

A `Measure` reduces a stream of accepted `(t, v)` samples — fed exactly once
per **accepted** solution point, never for a rejected step, which is filtered
out by the caller before this crate ever sees it — into one running value,
using **O(1) memory regardless of how many samples have been fed**: every
shipped kind is a fixed-size structure with no growing collection. This is
what makes a measure viable both against a stored waveform (post-run
reduction) and inline during a live transient (online reduction) — the same
accumulator, fed the same way, produces the same answer either way.

### 7.1 The `MeasureKind` contract

Every kind implements three operations:

- **`update(t, v, lte)`** — fold in one accepted sample, together with the
  **local truncation error** the integrator estimated at that point.
- **`value()`** — the reduction's current value.
- **`error_bound()`** — how far `value()` could still be off.

**Error bounds are derived from the local LTE, not asserted as zero.** A
reduction computed only from accepted points can, in principle, miss the
true extremum that occurred *between* two accepted points — the caller's
integrator only guarantees accuracy at the points it accepted, not at every
instant. Every shipped kind therefore folds the `lte` it is fed into a
running bound, so a coarsely-sampled run reports an honestly wide bound
rather than a falsely precise number.

### 7.2 The shipped catalog

| Kind | Reduces to | Error-bound source |
|---|---|---|
| `peak(x)` | running maximum | the LTE observed at the sample that set the current maximum |
| `min(x)` | running minimum | symmetric to `peak` |
| `average(x)` | time-weighted (trapezoidal) mean | the LTE-weighted trapezoidal area, normalized by elapsed time |
| `rms(x)` | trapezoidal mean-of-squares, square-rooted | the mean-of-squares bound, linearized through the square root |
| `ripple(x)` | `peak − min` | the sum of both extrema's bounds |
| `integral(x)` | plain trapezoidal area (no time normalization) | the LTE-weighted trapezoidal area |
| `cross(x, level, dir)` | the predicted instant of a level crossing, via the tier-2 crossing predictor (§3.4) applied to `x − level` | the tier-2 bracket-width bound |
| `settle(x, target, tol)` | the first instant `x` enters `[target−tol, target+tol]` and never subsequently leaves | `0` once settled (the instant is exactly the accepted sample that first entered the band); the last-seen time while not yet settled, otherwise |
| `count(pred)` | how many fed samples were nonzero (the predicate itself is a caller concern — this kind counts nonzero feeds) | exact — always `0` |
| `delay(a, b, level, dir)` | propagation delay between two channels, both crossing `level` in direction `dir` — see below | `0` (both crossing instants come from exact tier-2 predictions) |

**`delay` is the one two-channel exception.** Every other kind's contract is
"one accepted `(t, v)` per feed"; propagation delay is inherently a
two-signal quantity (when does `a` cross, when does `b` cross), so it cannot
be expressed through the single-value `update` signature. `delay` therefore
ships with its own two-argument feed method, matching the catalog's
*spelling* without pretending to satisfy the single-channel trait — widening
the trait itself to accommodate one two-channel kind was rejected as a worse
trade than one honestly differently-shaped kind.

### 7.3 Peak refinement

`peak` (and, symmetrically, any kind built on it) supports iterative
refinement toward a caller-supplied tolerance, given access to a sampling
function: it re-probes near the current maximum at a half-width derived from
the *current* error bound (not a fixed halving schedule — a bound that
already reflects how far off the estimate could be is the distance actually
worth probing at), folding each new sample back into the accumulator. Like
tier-2 crossing refinement (§3.4), this:

- Terminates at a caller-supplied step floor rather than exhausting the full
  iteration budget once the floor is reached.
- Reports honestly whether the requested tolerance was actually met — never
  claiming success past what the floor allowed.
- Never widens the bound; each refinement step can only tighten it or leave
  it unchanged.

### 7.4 Gating

A measure may be **gated**: it accumulates only while a gate condition is
open. Three gate kinds:

| Gate | Opens when |
|---|---|
| `Time { t0, t1 }` | `t0 <= t <= t1`, judged from the measured sample's own time — no separate feed needed |
| `Analog { level }` | the gate-controlling analog signal is above `level` (closed at or below); the crossing instant is available via the tier-2 predictor |
| `Digital` | the gate net's last **settled** value is high |

**The digital gate accepts only settled state — this is a correctness rule,
not a preference.** A digital network settles by propagating through an
evaluation graph (Part VII §4.3's delta-cycle algorithm); a value observed
*partway* through that propagation can be an artifact of evaluation order
rather than a value the circuit ever genuinely held, and gating on such a
value would produce a result that silently changes if an unrelated edit
reorders the evaluation graph — wrong, but stable-looking until it moves.
Consequently the digital gate's only input is "the settled value" (a single
boolean, supplied once per settled net update); there is no mid-cycle
variant to call by mistake, because only the caller above this crate (the
scheduler) knows when settling actually happened, and this crate does not
— and must not — guess at it.

A gate that opens more than once over a run keeps accumulating across
**every** open interval on the same underlying accumulator — closing and
reopening the gate does not reset the measure.

### 7.5 Lifecycle: arm, read, disarm

A measure is a **handle**, not a name in a global registry — arming the same
kind of measure twice produces two independent accumulators with no shared
state.

- **`arm(kind)`** creates a fresh, ungated accumulator starting at the arm
  instant. **`arm_gated(kind, gate)`** creates one additionally gated per
  §7.4.
- **`feed(t, v, lte)`** folds in one accepted sample — a no-op once the
  measure has been disarmed, or while a gate is present and currently
  closed.
- **`disarm()`** freezes the accumulator: `feed` becomes a no-op from that
  point on, so the reported value stops moving but stays readable — a
  disarmed measure does not become unreadable, only inert.
- **`value()`/`error_bound()`** SHALL fail loud — never `NaN`, never `0.0`,
  never a nominal placeholder — when no accepted point has yet reached the
  accumulator. This applies identically whether nothing was ever fed at all,
  or a gate simply never opened during the run: from the accumulator's point
  of view these are the same fact (zero accepted points reached it), so they
  are reported with the same "never sampled" outcome rather than two
  different plausible-looking failure modes.

---

## §8 Wave — value-from-time descriptors

A `Wave` is `v(t)`: a value that depends **only on time**, with no coupling
to circuit state — the independent-source family, as distinct from
`Behavior` (§6), which reads the solution. Because a `Wave` never reads
anything but `t`, its contribution to a Jacobian is trivially zero; it
fills a right-hand-side term, not a cross-coupled row.

### 8.1 The shipped catalog

| Kind | Parameters | Shape |
|---|---|---|
| `const(v)` | value | constant |
| `ramp(v0, v1, dur)` | start, end, duration | linear from `v0` at `t=0` to `v1` at `t=dur`; holds `v0` before, `v1` after |
| `pulse(lo, hi, delay, rise, width, fall, period)` | seven ngspice-`PULSE`-matching parameters | `lo` until `delay`; ramps to `hi` over `rise`; holds `hi` for `width`; ramps back over `fall`; repeats every `period` when `period > 0`, else stays at `lo` |
| `sin(offset, amp, freq, phase, damping)` | five | `offset + amp·sin(2π·freq·t + phase)·exp(−damping·t)`, `phase` in radians |
| `exp(v0, v1, tau, delay)` | four | `v0` until `delay`, then a single-stage exponential approach toward `v1` with time constant `tau` |
| `sigmoid(lo, hi, t0, tau)` | four | a logistic S-curve: `lo + (hi−lo)/(1+exp(−(t−t0)/tau))` |
| `pwl(points)` | a sorted `(t, v)` list, authored inline | piecewise-linear, clamped flat beyond both ends (the same convention as `table`, §5, but on the time axis instead of an input value) |
| `sampled(t_array, v_array)` | a precomputed array pair | the identical shape as `pwl`, for data supplied as an already-computed array rather than authored inline — the two variants exist to name the authoring path, not to differ in evaluation |
| `from_fn(f)` | a caller-supplied callback `f(t) -> f64` | whatever `f` computes |
| `sum(a, b)` | two `Wave`s | `a(t) + b(t)` |
| `scaled(w, k)` | a `Wave` and a scale factor | `k · w(t)` |
| `shifted(w, dt)` | a `Wave` and a time shift | `w(t − dt)` |

### 8.2 `from_fn` is the one non-zero-cost kind

Every kind but `from_fn` (including composition kinds built from
zero-cost operands) evaluates natively — no callback is invoked, and no
kind other than `from_fn` even *holds* a callback field, so the cost
distinction is structural, not merely a documented convention. `from_fn`
exists specifically for a caller-supplied function and costs exactly one
call per evaluation — available, but never the default and never free.

### 8.3 DC value

When a sequencer or coroutine driver is not running (a DC/operating-point
analysis has no time axis to evaluate `v(t)` against), each `Wave` kind
states its own **DC value**, following the same convention SPICE sources use
— this is *not* generically `value(0.0)`, because that would be wrong for
several kinds:

| Kind | DC value | Why not `value(0.0)` |
|---|---|---|
| `const` | its value | (trivially the same) |
| `ramp` | `v0` (start) | — |
| `pulse` | `lo` (initial level) | — |
| `sin` | `offset` | `value(0.0)` includes the phase-shifted sine term unless `phase = 0` |
| `exp` | `v0` (initial level) | — |
| `sigmoid` | `lo` | `value(0.0)` may already be partway up the curve if `t0 > 0` |
| `pwl` / `sampled` | the first authored point's value | the axis need not start at `t = 0` |
| `from_fn` | `f(0.0)` — evaluated, at the cost of one call, paid once at the operating point rather than per Newton iteration | (this is the one kind for which "evaluate at zero" is the only available convention) |
| `sum` / `scaled` / `shifted` | the same combination of the operands' own **declared** DC values | re-evaluating the composed `value` at `t=0` would silently reduce to the generic (wrong, for the reasons above) convention for any non-trivial operand |

### 8.4 Declared breakpoints

Symmetric with `transition`'s breakpoint declaration (§2.3), each `Wave`
kind whose discontinuities are knowable in advance (because its parameters
are literals, not run-dependent) publishes them through the same
`pending_edges(from, end)` shape:

| Kind | Publishes |
|---|---|
| `const` | nothing |
| `ramp` | `0` and `dur`, when `v0 != v1` |
| `pulse` | all four corner instants (`delay`, `delay+rise`, `delay+rise+width`, `delay+rise+width+fall`) per cycle, repeated at every period offset whose cycle could touch the query window |
| `sin` / `sigmoid` | nothing — continuous (infinitely differentiable), so there is no a-priori corner to land on |
| `exp` | `delay` |
| `pwl` / `sampled` | every authored point's time coordinate |
| `from_fn` | nothing — arbitrary and host-defined, so no discontinuity is knowable a priori |
| `sum` | the union of both operands' edges |
| `scaled` | the inner wave's edges, unchanged |
| `shifted` | the inner wave's edges, each shifted by `dt` |

A queried window `(from, end]` is **half-open on the low end, closed on the
high end**: an edge landing exactly on `from` is excluded, an edge landing
exactly on `end` is included.

---

## §9 Derivative verification

`verify_derivatives` is the finite-difference oracle every hand-written
`BehaviorKind::jacobian` (§6) is checked against before it ships. A wrong
analytic derivative does not crash the solver — it converges slowly, or
converges to the wrong operating point, silently. This function is what
stands between a newly-authored behavior kind and exactly that failure mode.

**Method.** For every input dimension of every supplied test point, compare
the kind's own `jacobian` against a **central finite difference**:

```text
fd_i = (value(point with input i nudged by +h) − value(point with input i nudged by −h)) / (2h)
```

with `h` scaled to the magnitude of the input coordinate itself (never a
fixed absolute step, which would be either too coarse or too fine depending
on the input's scale). Central finite difference is exact to `O(h²)` —
independent of however the analytic Jacobian being checked was actually
derived, which is what makes it a trustworthy oracle rather than a repetition
of the same reasoning.

**Failure reporting.** The comparison uses a combined absolute-plus-relative
tolerance. The function returns the **first** disagreement found — not every
disagreement — because a single named point (which input, which sample, both
the analytic and finite-difference values) is sufficient to reproduce and fix
the defect; collecting every disagreement was deliberately not built, to
avoid duplicating a "collect all findings" posture that belongs to the
validation-findings channel (Part VII §3.5), not to this crate.

**Known false positives, stated so they are never mistaken for bugs.** A
sample point at or extremely near a genuinely discontinuous kind's own edge
(§6.1) — `hysteresis` at its threshold, `switch` at its threshold — will
disagree with finite difference by construction: the finite-difference probe
straddles the jump and reports a large or undefined slope, while the
analytic Jacobian correctly reports zero away from the edge. This is not a
defect in either the kind or the oracle; it is why every kind's own
verification samples are chosen away from its known discontinuities. The
same caveat applies to `limiter` and `table` at their exact corner/breakpoint
coordinates, where the derivative has a genuine kink (a jump in slope, not in
value).

---

## §10 Digital edge logic

Four-state digital logic (`0`, `1`, `X`, `Z` — matching the meanings defined
in Part VII §7) and the edge-trigger predicate (`Rising`/`Falling`/`Either`)
that both PHDL's compiled `@(posedge …)`/`@(negedge …)` blocks and a host
sequencer's `wait(edge(…))` calls resolve through — one shared rule, so a
host test waiting on a net's edge fires at **exactly** the same transitions a
PHDL block driven by the identical net would.

### 10.1 The edge rule (IEEE 1364)

Given a **before** value and an **after** value (the net's value at the
previous and current settled sample, respectively):

| Direction | Fires when |
|---|---|
| `Rising` | `before != 1 && after == 1` |
| `Falling` | `before != 0 && after == 0` |
| `Either` | `before != after` |

**The full 4×4 transition table for `Rising`, stated explicitly because it
is the exact rule a real defect once got wrong:**

| before \ after | `0` | `1` | `X` | `Z` |
|---|---|---|---|---|
| **`0`** | no fire | **fires** | no fire | no fire |
| **`1`** | no fire | no fire (already `1`) | no fire | no fire |
| **`X`** | no fire | **fires** | no fire | no fire |
| **`Z`** | no fire | **fires** | no fire | no fire |

`X→1` and `Z→1` **fire** a `Rising` wait, exactly the same as the canonical
`0→1` transition — a net that starts at its power-up `X` state and settles
directly to `1` (skipping an observable `0` in between, which is common for
a net with no explicit reset drive) still satisfies a `Rising` edge wait. The
symmetric table holds for `Falling` (`X→0` and `Z→0` both fire, alongside the
canonical `1→0`), and `Either` fires on any value change whatsoever,
including a change between two non-`0`/`1` values (`X→Z` fires `Either`; it
fires neither `Rising` nor `Falling`, since neither endpoint is `0` or `1`).

**Why this precision matters normatively, not just pedagogically.** A
digital net's power-up state is `X`. A host test synchronizing to "the first
rising edge of a clock" that only recognized the canonical `0→1` transition
would silently miss a clock whose first observed transition was `X→1` —
stalling for a full cycle with no error reported. This is not a hypothetical
edge case; it is the exact scenario that motivated stating this rule as its
own normative section rather than leaving it implicit in prose.

### 10.2 Ordering independence

The rule is stated purely in terms of `(before, after)` — it has no notion of
*which* value preceded which chronologically beyond that ordering, no
dependency on how many samples came before "before," and no smoothing or
hysteresis of its own. A caller that wants "no fire on the very first
observation, because there is no genuine 'before' yet" (arming) implements
that as its own sequencing concern on top of this rule — this rule always
answers the question "does `(before, after)` constitute an edge of this
direction," never "has enough history accumulated to ask that question yet."
That arming question is answered differently by different callers (a
sequencer's wait needs it; a compiled `@(posedge)` block, which is seeded by
the net's actual `X` power-up state as its first "before," does not need a
separate arming concept at all) and therefore belongs above this rule, not
inside it.

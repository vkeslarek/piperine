# dv-tester IDEAL — a host-language device library over the `Element` ABI

**Status:** surface design document — **refined and decided, 2026-07-31.** Every
question that blocked level-1 or level-2 design is now answered (§12); `spec.md`
and `design.md` are rewritten against this document. Third revision: the level
taxonomy was corrected, level 1 became a crate of its own, and a codegen audit
(§3.4, §3.9) replaced two guesses with verified facts — crossings land late and
nothing can force a retry at a computed instant; and four families the crate needs
already exist in codegen as pure code, so level 1 starts by extracting rather than
inventing.
**What this is:** a library for **writing devices in the host language** — Python
or Rust, same contract — spanning from "place this stamp" to "ramp a sigmoid here"
to "switch when these two nodes cross" to "drive a burst on the clock edge". A
tester is one kind of device you write with it, not the thing itself.

**What revision 1 got wrong.** It designed a test sequencer and then restricted the
surface — two segment kinds, no waveform synthesis — in the name of
implementability. That closes off exactly the interesting cases. The correct
principle is the opposite: **expose the whole ABI, make the cheap path convenient,
and let the author choose the granularity of host calls.**

**What revision 2 got wrong.** Its three "levels" conflated *abstraction* with
*rate*: it called raw-stamping-from-Python "level 0" and waveform descriptors
"level 1", which mixes who-writes-it with where-it-runs. Under that framing a
question as ordinary as "a PWM comparator that reads two terminals" had no home —
`Wave` is a function of time alone and cannot express it, and the only remaining
answer was a per-Newton-iteration host call. Revision 3 fixes the axis (§1) and
adds the two families that were missing: `Behavior` (§3.2) and `Measure` (§3.4).

---

## 0. Why this is a real gap

There are four ways to get a device into a Piperine circuit today, and none of
them lets you write one where you already are:

| Path | Authoring cost | Speed | Distribution |
|---|---|---|---|
| **PHDL** (`analog`/`digital` body) | declarative; must be expressible in PHDL | JIT — fastest | source |
| **Plugin cdylib** (`#[pip::device]`) | a Rust crate, `dlopen`, host and plugin built by the same toolchain (`backend/native.rs`) | native | binary artifact |
| **OSDI** (external `piperine-osdi`) | Verilog-A + a compiler | native | `.osdi` |
| **Host device library** ← this | a class in the script you are already writing | native mechanism + host-call at the rate *you* choose | a module |

Device plugins are **native-only** — `crates/piperine-plugin/src/backend/native.rs`
requires a compiled shared library, and `manifest.rs` shows Python plugins are
*scripted* (CLI entries and hooks), never devices. So there is no way to write a
device in Python at all, and no way to write one in Rust without building a
separate crate with a matched toolchain.

That is the gap: **no in-process, no-build-step device authoring.** Testers need
it most urgently, which is why it lives in this feature — but the capability is
general, and designing it as "tester plumbing" is what produced the bad first
draft.

---

## 1. Three levels — the corrected taxonomy

The axis is **where the code runs**, not how abstract it looks. Each level has one
home and one cost, and they are stated per level rather than discovered:

| Level | What it is | Home | Cost |
|---|---|---|---|
| **0** | the raw solver ABI — `Element`, stamps, unknowns | `piperine-solver` | one host call per **Newton iteration** if written in Python; free in Rust |
| **1** | native signal mechanism — `Wave`, `Behavior`, events, `Measure` | **`piperine-signal`** (new crate, §3) | **zero** host calls; closed-form value + derivative |
| **2** | the host library — declares level 1, sequences time, checks | `piperine-api` + `piperine-python` | one host call per **test breakpoint** |

The three **compose in one device**: a class may stamp directly (level 0), install
a `Wave` and a `Behavior` (level 1), and sequence time (level 2), all at once.
They are coordinates, not a ladder you pick a rung of.

The rule that separates level 2 from level 1 is the same one that separates a
tester from a `constraint` (`design.md` §C1): **who chooses the instants**. When
the solver dictates the rhythm (every accepted point, every Newton iteration), the
work must be native — that is level 1. When the *test* dictates its own rhythm
(10⁴–10⁵ wake-ups), the host is affordable — that is level 2.

---

## 2. Level 0 — the raw solver ABI is already the right shape

`crates/piperine-solver/src/core/element.rs` has ~40 methods of which **two are
required** (`name`, `capabilities`) and the rest are defaulted. That is exactly
the shape a host library wants: implement what your device does, inherit nothing
for what it does not.

Grouped by what an author actually reaches for:

| Group | Methods | Needed by |
|---|---|---|
| Identity | `name`, `capabilities` | everything |
| Analog stamping | `load_dc`, `load_ac`, `load_transient`, `allocate_unknowns` | sources, resistors, behavioral analog |
| Time control | `next_breakpoints`, `bound_step_hint`, `suggest_transient_step`, `accept_timestep` | sequencers, breakpointed sources |
| Digital | `boundary`, `init`, `evaluate`, `seq_phase`, `comb_phase`, `has_input_on` | logic, monitors, protocol drivers |
| State | `update`, `initial_conditions`, `set_temperature` | anything with memory |
| Introspection | `read_opvars`, `list_params`, `get_param`, `set_param`, `list_terminals`, `model_descriptor`, `list_observables` | anything a host should be able to query |
| Validation | `validation_reports` (from `dv-core`) | checkers, testers, self-testing models |
| Noise / disto | `noise_current_psd`, `load_disto2/3` | noise and distortion models |

Level 0 is the **escape hatch, and the reason nothing is ever a wall**. If a device
needs something level 1 never anticipated, this always works — at the stated
price.

```python
class Memristor(pip.Device):
    caps = pip.ANALOG | pip.LOADS_DC | pip.LOADS_TRAN

    def __init__(self, p, n, ron=100.0, roff=16e3):
        self.p, self.n, self.ron, self.roff, self.x = p, n, ron, roff, 0.0

    def load_dc(self, ctx, m):
        g = 1.0 / (self.ron * self.x + self.roff * (1.0 - self.x))
        m.g(self.p, self.p, +g);  m.g(self.p, self.n, -g)     # conductance stamps
        m.g(self.n, self.p, -g);  m.g(self.n, self.n, +g)

    def load_transient(self, ctx, m):
        self.load_dc(ctx, m)

    def update(self, state, ctx):                              # integrate state
        v = ctx.v(self.p) - ctx.v(self.n)
        self.x = min(1.0, max(0.0, self.x + ctx.dt * v * 1e4))

    def read_opvars(self):
        return [("x", self.x), ("r", self.ron * self.x + self.roff * (1 - self.x))]
```

`m` is the stamp sink: `m.g(row, col, value)` for conductance, `m.i(row, value)`
for current RHS, `m.branch(...)` for a branch unknown. Nothing is hidden.

---

## 3. Level 1 — `piperine-signal`, the native mechanism crate

Level 1 is **not** a module inside the solver and **not** a module inside codegen.
It is a crate of its own, because its four families are useful in places neither
of those reaches: post-processing a recorded `Waveform`, a CLI that renders a
stimulus shape, a plotting path, a bench script that reduces a trace — none of
which involve an MNA matrix or a JIT.

### 3.1 Purity — what the crate does *not* know

This is the constraint that makes the crate reusable, and it is load-bearing:

> **`piperine-signal` computes values, derivatives, event predicates and running
> reductions. It never stamps, never sees a matrix, never sees an `Element`, and
> depends on no other crate in the tree.**

The consumer turns a value + derivative into stamps (the host-device adapter in
`piperine-api`; codegen later, §12.16). The consumer decides what to do with a
detected crossing (request a breakpoint, refine a step). The consumer feeds
accepted points into a `Measure` — or replays a stored `Waveform` into the same
accumulator and gets the same answer.

Dependency direction, therefore:

```
piperine-api    ─┐
piperine-codegen─┼──▶ piperine-signal ──▶ (nothing)
(later, §12.16)  ┘
```

`piperine-solver` does **not** depend on it. Level 1 sits above level 0.

### 3.2 `Wave` — value from time

Unchanged from revision 2, and still the right shape for the one-input case:
`v(t)`, no coupling to any other node, so its Jacobian contribution is trivial
(it fills an RHS, it is an independent source).

```
Wave.const(v)                      Wave.ramp(v0, v1, dur)
Wave.pulse(lo, hi, delay, rise, width, fall, period)
Wave.sin(offset, amp, freq, phase, damping)
Wave.exp(v0, v1, tau, delay)       Wave.sigmoid(lo, hi, t0, tau)
Wave.pwl([(t, v), …])              Wave.sampled(t_array, v_array)   ← numpy
Wave.from_fn(lambda t: …)          ← arbitrary, host call per evaluation
Wave.sum(a, b)  Wave.scaled(w, k)  Wave.shifted(w, dt)   ← composition
```

Three ways to get an arbitrary shape, and the author picks knowingly:

1. `Wave.sigmoid(...)` — a built-in descriptor. **Zero host calls.**
2. `Wave.sampled(t, v)` — precompute in numpy, evaluated as native PWL.
   **Zero host calls**, arbitrary shape. This is the right answer far more often
   than people expect, and it is why the library does not need to be restrictive
   to be fast.
3. `Wave.from_fn(f)` — a host callback, evaluated whenever the solver asks.
   Arbitrary and slow. Available, priced, and never the default.

A `Wave`'s discontinuities are known a priori (its parameters are literals), so it
publishes its own breakpoints — the integrator sees an edge rather than
interpolating across it.

### 3.3 `Behavior` — value from circuit state

The family revision 2 was missing. A `Behavior` is `f(v₁ … vₙ)`: it depends on the
solution, so it contributes **cross Jacobian entries** (`∂f/∂vᵢ`), not just an
RHS. That is the whole technical difference from `Wave`, and it is why it could
not have been a `Wave` variant.

Each kind ships as native Rust with a **closed-form value and a closed-form
derivative** — which is what buys zero host calls inside Newton while still
reacting to the circuit at every iteration:

```
Behavior.comparator(a, b, lo, hi, gain)     Behavior.hysteresis(a, b, vth_hi, vth_lo)
Behavior.switch(ctrl, ron, roff, vth)       Behavior.limiter(x, lo, hi)
Behavior.vcvs(a, b, gain)                   Behavior.vccs(a, b, gm)
Behavior.vco(ctrl, f0, kvco)                Behavior.table(x, xs, ys)   ← PWL in voltage
Behavior.sum(a, b)  Behavior.scaled(b, k)   Behavior.composed(f, g)
```

This is the catalog that answers the PWM question: a hysteretic controller is
`Behavior.hysteresis` driving `Behavior.switch`, both stamped natively, both
reacting at Newton rate, with no Python in the loop at any point.

**Smooth is the default, hard edges are opt-in.** `comparator(gain=…)` is a
`tanh`-shaped transition: differentiable, convergence-friendly, and closer to a
real comparator than an ideal step. A genuinely discontinuous kind (`switch`,
`hysteresis`) registers a crossing (§3.4) so the integrator lands on the edge
instead of interpolating across it — and pays for it in timestep.

### 3.4 Events — crossing detection

`Behavior.hysteresis`, `Behavior.switch`, and `Measure.cross` all need the same
primitive underneath: **given a predicate over the solution, find the instant it
flips.**

The crate's part is pure and small: hold the predicate, sample it at the points it
is fed, and when a sign change brackets an interval, interpolate the crossing
instant and report it with an error bound. What the consumer does with that
instant — register a breakpoint, refine the step, force a retry — belongs to the
consumer, because only the consumer knows the solver.

#### What the tree actually does today (verified 2026-07-31)

Two facts, both checked in the code, and they replace the guess this section
carried in revision 3:

**1. `SUPPORTS_ROLLBACK` is fully live.** The "phantom doc" that
`solver-simplification` removed was later *implemented* by `element-abi-maturity`.
`Element::checkpoint_state() -> Option<ElementCheckpoint>` /
`restore_state(&ElementCheckpoint)` exist (`core/element.rs:542-555`); the
transient driver snapshots every device before **every** candidate attempt
(`transient.rs::attempt_step` → `snapshot_device_checkpoints`) and restores on
both rejection paths (`reject_lte_step`, `reject_step` →
`restore_device_checkpoints`). DC homotopy retries use the same seam. Default
`None` = stateless = zero cost; codegen declares the bit when a device owns a
`$limit` limiter or stateful digital registers (`device/element.rs:662`).

**2. But crossings are *not* exact — and PHDL's own `cross()` is not either.**
This is the finding that matters. `device/analog/events.rs`'s `EventDetector`
compares the **previous accepted** trigger value against the current one
(`prev <= 0.0 && value > 0.0`), and its `next_breakpoints` states outright:
*"Non-timer events (crossings) are detected reactively and contribute no static
breakpoints here."* The solver agrees — `EventKind::Crossing` carries
`priority: Advisory` with the comment *"the integrator notes the time but does not
necessarily land on it exactly."*

So a PHDL `@cross` fires at the **first accepted point after** the crossing, late
by up to one full timestep, with no interpolation and no refinement. There is no
"reject and retry at `t_cross`" anywhere in the tree.

#### Three fidelity tiers, and what each costs

The pieces to do better already exist; they have simply never been composed.

| Tier | Mechanism | Solver change | Error |
|---|---|---|---|
| **1 — reactive** (today's `cross()`) | detect the sign change at the next accepted point | none | up to one timestep, **unreported** |
| **2 — predicted + bounded** | extrapolate `t_cross` from the trajectory, cap the step via `bound_step_hint`/`$bound_step` (or declare an `Exact` breakpoint at the prediction), re-detect, iterate | **none** — both seams exist | converges to the step floor; **reportable** |
| **3 — exact** | promote `EventKind::Crossing` from `Advisory` to `Exact` and add reject-and-retry-at-`t*` | yes, a real solver feature | machine precision |

**Level 1 ships tier 2, with the error published** (§3.6's rule applied to
instants rather than amplitudes). It needs no solver change, it is strictly better
than what PHDL has today, and it never returns a bare plausible instant. Tier 3 is
a solver feature with its own spec, and `EventKind::Crossing` + `EventPriority`
are already the right shape to receive it — the promotion is a small diff on
scaffolding that exists.

A useful side effect: implementing tier 2 in the crate gives PHDL's `@cross` a
better implementation to adopt later (§12.16), because the improvement lives in
shared pure code rather than in one lowering path.

### 3.5 `Measure` — reductions at the solver's rate

The other family revision 2 was missing, and the one that makes "insert a signal
and test the reaction" mean anything. `t.read_v()` at level 2 observes **only the
test's own breakpoints**; peak, ripple, settling and overshoot live *between*
them.

A `Measure` is a **streaming accumulator**: fed one `(t, v)` per accepted point,
holding O(1) state, never storing a waveform.

```
Measure.peak(x)        Measure.min(x)         Measure.average(x)
Measure.rms(x)         Measure.ripple(x)      Measure.integral(x)
Measure.cross(x, level, dir)        → the instant of a crossing
Measure.settle(x, target, tol)      → when it enters the band and stays
Measure.delay(a, b, level)          → propagation between two signals
Measure.count(pred)                 → how many times a predicate fired
```

Because the accumulator is pure, the **same code** reduces a stored `Waveform`
post-run and a live transient online, and both must agree — which is the crate's
own regression test.

**Gating (V1, §8.3).** Every measure takes an optional window:

```python
Measure.ripple("out", window=(1e-3, 2e-3))          # by time
Measure.peak("out", gate=pip.above("en", 0.9))      # by analog signal
Measure.peak("out", gate=pip.digital_high("en"))    # by digital net
```

A time window is trivial. An **analog** gate is a crossing predicate (§3.4) — same
rhythm as the accepted points. A **digital** gate has its own rhythm: it opens and
closes on the digital scheduler's events, and it observes **settled** events only,
never mid-delta-cycle — the same rule `dv-core`'s monitors follow.

The reason settled-only is a correctness rule and not a preference (user,
2026-07-31): **a mid-delta-cycle value can be an artifact of DAG evaluation
order, not a signal the circuit ever had.** The digital network settles by
propagating through an evaluation graph; an intermediate value observed partway
through is an implementation detail of the traversal. Gating (or firing, or
measuring) on one would produce a result that changes when an unrelated edit
reorders the graph — a wrong answer that looks stable until it moves. Only
settled events are physically meaningful, so only settled events are observable.

### 3.6 Resolution and the error bound — the accuracy decision

`Measure.peak` over accepted points **misses the true peak** when it falls between
two of them. For overshoot measurement — the archetypal analog check — that is a
plausible wrong number, exactly what this project's fail-loud rule exists to
prevent. So:

| Mode | What it does | Cost |
|---|---|---|
| **default** | reduces over accepted points and **reports its own error bound** (`peak ∈ [v, v+ε]`, ε from the local LTE) | free |
| `refine=True` | asks for step refinement near the extremum (via the step-hint path) until ε is under a requested tolerance | simulation time |

The default is honest rather than exact; the exact answer is available and its
price is explicit. A `Measure` whose bound is wider than the assertion it feeds
should say so at the assertion, not silently pass — an `expect` against a measure
compares **against the bound**, not the point estimate.

### 3.7 Closed in Python, extensible in Rust

The kinds in §3.2–§3.5 are native Rust — closed-form value and derivative,
hand-written. That fixes the extension story:

- **Python: the catalog is closed.** `pip.Behavior.*` and `pip.Measure.*` are what
  ships. A Python author who needs something else drops to level 0 (raw stamps,
  priced per Newton iteration) — the escape hatch is always there.
- **Rust: the catalog is open.** Implement the trait, get a new kind at native
  speed with no plugin and no build ceremony:

```rust
pub trait BehaviorKind {
    fn value(&self, inputs: &[f64]) -> f64;
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]);   // ∂f/∂inputs[i]
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> { None }
}

pub trait MeasureKind {
    fn update(&mut self, t: f64, v: f64, lte: f64);
    fn value(&self) -> f64;
    fn error_bound(&self) -> f64;
}
```

`Measure` follows the same rule as `Behavior` by symmetry — closed in Python, open
in Rust (§12.3 confirms).

### 3.8 Derivative verification — the FD oracle

A hand-written `jacobian` is a silent-failure surface: a wrong derivative does not
crash, it converges slowly or converges to the wrong place. The crate ships the
check:

```rust
signal::verify_derivatives(&my_behavior, &sample_points)  // analytic vs central FD
```

Same oracle pattern `dv-gradients` used to validate the adjoint against `.sens`.
Cheap, and it is the only thing standing between a new `BehaviorKind` and a
plausible wrong answer.

### 3.9 The rules already in the tree — what level 1 inherits

`piperine-signal` is not a greenfield invention. Codegen already implements most
of these semantics, correctly and with tests, in four files that are **already
pure** — no MNA, no `Element`, no JIT, just `f64` and `VecDeque`. That purity is
what makes the extraction mechanical rather than a rewrite, and it is the
strongest evidence the crate boundary is drawn in the right place.

Audited 2026-07-31. Each rule below is battle-tested behavior that must survive
into the crate; losing any of them means re-learning it from a bug report.

#### A. Runtime operators — `device/analog/operators.rs`

**The invariant that governs all four: mutated once per *accepted* timestep,
never inside Newton.** This is §7's rule, already implemented — a rejected step
leaves every operator untouched. Level 1 inherits it as a type-level property, not
a convention.

| Operator | Rules that must survive |
|---|---|
| `delay(x, td)` | `(time, value)` history ring; drop samples strictly older than `t − td` **but keep one at or before it** — that sample is the left bracket for interpolation. Linear interpolation between brackets with a clamped fraction. Empty history → return the input. |
| `slew(x, rise, fall)` | rate limiter: climbing `delta.min(rise·dt)`, falling `delta.max(−fall·dt)`. `dt` floored at 0. |
| `transition(x, td, tr, tf)` | the richest, and the one whose rules are least guessable: **(a) single pending ramp** `(start, target, t_change)`; **(b) re-anchor on retarget** — a new input mid-ramp sets `start` to the *current output* and `t_change` to now, so the walk never jumps; **(c) seeding** — the first observation (the DC-op seed) settles *at* the input rather than ramping from zero; **(d) zero ramp** steps instantaneously past the delay edge with no divide-by-zero; **(e) `pending_edges`** declares its own breakpoints (`t_change+td`, `+ramp`) **only while a ramp is pending** — settled or unseeded contributes none. |
| `idt` / `idtmod` | implicit-Euler accumulator `value += dt·x`; modulus wrap `value −= m·floor(value/m)`. The **in-step** `dt·x` term is added by the *consumer*, not the accumulator — `value` is the integral up to the last accepted step. That split is what keeps the operator pure. |

Rule (e) generalizes into a level-1 contract worth naming: **a stateful signal
element publishes its own future discontinuities.** `Wave` does it from literals,
`transition` does it from pending state, `timer` does it from its period. That is
one interface (`pending_edges(from, end) -> impl Iterator<Item = f64>`), not three
special cases.

#### B. Event detection — `device/analog/events.rs`

| Detector | Rules that must survive |
|---|---|
| `cross(x, dir)` | sign change between accepted points: rising `prev ≤ 0 ∧ v > 0`, falling `prev ≥ 0 ∧ v < 0`, `Either` = both. **Seeding:** an unseeded detector never fires — the first observation only arms it. |
| `above(x)` | `rose ∧ v > 0`, where `rose` is `prev ≤ 0` when seeded and **`true` when not**. Deliberately asymmetric to `cross`: `above` *does* fire on its first sample. Two detectors, two seeding rules — a genuine semantic difference, not an inconsistency to tidy away. |
| `timer(period, phase)` | `next_fire` advanced by `while next_fire ≤ t { next_fire += period }`. First fire at `phase` when `phase > 0`, else at `period` — which is what lets one source declare its rise and its fall with two phased timers. **Unlike crossings, timers do declare static breakpoints** (`next_breakpoints`), because their instants are known a priori. |

That last column is the whole tier-1/tier-2 distinction of §3.4 in one sentence:
*known-a-priori instants get exact landings; solution-dependent instants get
detected late.* Level 1's job is to move crossings from the second class toward
the first.

#### C. `table(x, xs, ys)` — `resolve/pom/analog_ops.rs`

Already exactly `Behavior.table`, and its design note is the rule to preserve:
**lowered to a pure piecewise expression (nested selects), no state — so the
Jacobian falls out as the segment slope.** No hand-written derivative, no
`verify_derivatives` burden. Validation rules: `xs`/`ys` constant real arrays,
equal length, ≥ 2 points, `xs` **strictly increasing**, linear interpolation with
**end clamp**, `"linear"` the only mode — each of them loud.

The general principle: **a `Behavior` that can be expressed as a pure piecewise
function should be, because its derivative is then free and cannot be wrong.**
Reserve hand-written Jacobians for kinds that genuinely need state.

#### D. `$limit` — pnjlim/fetlim — `device/analog/limits.rs`

The convergence machinery, and the least guessable knowledge in the tree. It
matters to level 1 because a stiff `Behavior` — a high-gain `comparator`, a
`switch` with a sharp threshold, anything exponential — hits exactly the failure
these rules exist to prevent.

1. **Seed with `vcrit`, not zero.** Each limit slot starts at its critical
   voltage (a function of params/temperature only, never node voltages) so a
   junction opens near turn-on rather than at 0 V (ngspice `MODEINITJCT`).
2. **Preserve the seed until first bias.** `pnjlim(0, vcrit) = 0` on the opening
   iterations would discard the seed and let the node float to the supply — so
   the update is skipped while the slot is still at its seed and the new value is
   lower.
3. **Limit against the previous *limited* value.** Each iteration stores its
   limited voltage for the next one to limit against. The comment in the file is
   the justification, verbatim: *"This is what makes junction devices converge —
   without it a stiff exponential overshoots and stalls."*
4. **Limiting vetoes convergence, with a tolerance.** A slot is "still clamping"
   iff the limited value differs from the raw branch voltage by more than
   `1e-6 + 1e-4·|vnew|`; while any slot is clamping, Newton must not declare
   convergence — a clamped junction can momentarily satisfy KCL at a
   non-solution voltage. The tolerance exists so post-limiting Newton jitter does
   **not** veto forever.
5. **Move the minus node, not the plus.** When rewriting node voltages to the
   limited branch value, displace the minus node if it is a real node — this
   keeps a *shared* plus node (a BJT's internal base) fixed. Only fall back to
   moving the plus node when the minus is ground.

Level 1 wants a `Limiter` of its own for the same reason PHDL devices have one,
and rule 4 has an ABI consequence worth stating: **a level-1 mechanism can veto
convergence**, which means the host-device adapter must surface a
`limiting_report`-shaped signal, not just stamps.

#### E. Contribution split rules — `flatten/analog.rs`

Less directly reusable (they are lowering rules, not runtime rules), but two are
real semantics that a level-1 author will hit:

- **`ddt` in a potential force must integrate a branch current** (`V = R·I +
  L·ddt(I(p,n))`), over the force's *own* branch. `ddt` of a different branch, or
  mixing `R·I` with `ddt` in one force, is a loud error — not a penalty
  conductance quietly inserted.
- **`ddt`/`ac_stim` markers are illegal inside a `var` body.** They belong to a
  contribution. Loud.

#### F. `$bound_step` — `kernel/analog/kernel.rs` + `analyses/events.rs`

The advisory step cap: the minimum over the body's `$bound_step` expressions,
evaluated at the last accepted voltages, surfaced through
`Element::bound_step_hint()` into an `EventEntry::step_hint`
(`priority: Advisory`, `rollback: Discard`). **This is the seam tier 2 of §3.4
rides on**, and it already exists end to end. A level-1 `Behavior` or `Measure`
computes a hint; the adapter emits it.

#### G. Noise shape — `kernel/analog/noise.rs`

`S(f) = psd · (1/f)^exponent`, exponent `0` for white, per-source terminals.
One line, but it is the definition `Measure`'s spectral relatives would share.

#### What this means for sequencing

The extraction targets are four files, all already pure:

| Codegen file | Becomes | Reuse direction |
|---|---|---|
| `device/analog/operators.rs` | `signal::ops` (`Delay`/`Slew`/`Transition`/`Integrate`) | codegen delegates later (§12.16) |
| `device/analog/events.rs` (`EventDetector`) | `signal::events` | same; level 1 adds tiers 2/3 |
| `resolve/pom/analog_ops.rs` (`Table`) | `signal::Behavior::table` | same |
| `device/analog/limits.rs` (`Limiter`) | `signal::limit` | same |

Which reframes §8.1's cost. Level 1 is not "a second engine beside codegen's" for
these four — it is **codegen's own semantics, lifted into a crate where a host
device can also reach them**. The duplication risk §8.1 names applies to
`Measure`-vs-`constraint` (genuinely two implementations); it does **not** apply
to A–D, where the honest plan is extract-then-delegate. Writing them fresh in the
crate while codegen keeps its copies is the outcome to avoid, and the mitigation
is the same cross-check fixture, applied per operator.

---

## 4. Level 2 — the host library

A coroutine over simulated time. This is where a *tester* lives:

```python
class LinkTest(pip.Sequencer):
    def run(self, t):
        t.drive("rst", 1);                yield t.advance(100e-9)
        t.drive("rst", 0)
        t.install("vdd", pip.Wave.ramp(0, 1.8, 1e-6))
        yield t.advance(2e-6)

        for _ in range(64):
            yield t.wait(pip.edge("clk", pip.RISING))
            t.drive("tx", next(self.vectors))              # a burst, per edge

        t.expect(t.read_v("vout") > 1.2, f"vout = {t.read_v('vout')}")
```

The generator form resolves the tension between the two obvious alternatives: a
bare `while True: get(); advance()` loop forces a host crossing at every instant
and burns a breakpoint whether or not anything happened; a pure
`on_change(callback)` form scatters sequential logic across closure state. A
coroutine is sequential on the outside — `yield t.advance(dt)` reads as a
straight-line program — while `yield t.wait(edge(...))` expresses event-driven
waiting **without leaving that straight line**.

A `Sequencer` **is** a `Device`: the same ABI object with `next_breakpoints` wired
to the coroutine's pending wake. It can therefore also stamp (level 0) and install
`Wave`/`Behavior`/`Measure` (level 1) in the same class.

### 4.1 One `wait`, three wake sources

`yield t.wait(...)` is the only blocking verb, and it has three forms. Two of
the three ride event kinds the solver's `EventQueue` already carries
(`analyses/events.rs`); the third — the analog crossing wait — is resumed by
the sequencer's own tier-2 prediction and polling, not by an `EventKind`:

| The test writes | Wakes on | Existing `EventKind` | Landing |
|---|---|---|---|
| `t.advance(dt)` | a time the test named | `Breakpoint` | `Exact` |
| `t.wait(edge("clk", RISING))` | a digital net settling to a new value | `Digital` | `Exact` |
| `t.wait(above("vout", 1.2))` | an analog crossing | none — resumed via tier-2 prediction + `accept_timestep`/`StateView` polling, not `EventKind::Crossing` | tier 2 (§3.4), error reported |

That is not a coincidence to be pleased about for the two forms that do ride an
`EventKind`; it is the argument that those two need no new scheduling concept.
The crossing wait is honestly a different mechanism: `EventKind::Crossing` is
what PHDL's `@cross` reads (tier 1, unreported), while `Sequencer::wait(above(
...))` predicts the crossing instant from the bracketing accepted pair and
polls for it every accepted step — it never touches `EventQueue`/
`EventKind::Crossing` at all (Part VII §20.5 distinguishes the two paths
precisely). The sequencer still needs only **one new seam** beyond that: a
dynamic subscriber. Time breakpoints already flow through `next_breakpoints`;
a digital edge wait needs the scheduler to consult registered waiters after
settling a net (never before — §3.5), and the analog wait rides §3.4's tier-2
machinery directly, without an `EventKind`.

**Digital edge waits ship in V1** (user, 2026-07-31). Revision 3 had them
deferred as "can land second"; they are the whole point of the sequencer for
protocol work (example F), and deferring them would ship a tester that cannot
synchronize to a clock — which is most functional verification. The cost is a
real scheduler change (§10) rather than a library-only feature, and that is
accepted.

### 4.2 `install` — time is relative, discontinuities are honest

A `Wave` installed mid-run needs two rules, and both are the kind that bite
silently if left unstated:

1. **A wave's time origin is its install instant, not absolute zero.**
   `Wave.ramp(a, b, dur)` installed at `t = 5 µs` ramps over `[5 µs, 5 µs + dur]`.
   Absolute time would force every mid-run install to do arithmetic against the
   current instant — arithmetic the author will eventually get wrong, in a way
   that looks like a circuit bug.
2. **Replacing a wave takes effect immediately, and its discontinuity is
   registered — it is never silently smoothed.** If the author installs
   `Wave.const(100e-3)` over `Wave.const(10e-3)`, they asked for a step and they
   get a step; the install instant becomes a breakpoint so the integrator lands
   on the edge instead of interpolating across it.

Rule 2 revises the hint this document's discussion started with (re-anchoring
from the current value, the way `transition` does). `transition`'s re-anchor is
correct *for `transition`*, whose entire job is smoothing. A `Wave` is a literal
drive shape, and silently smoothing it would be exactly the kind of helpful
magic that makes a result impossible to explain.

**Continuity is available, explicitly**, which is the right trade:

```python
now = t.read_v("vdd")
t.install("vdd", pip.Wave.ramp(now, 1.8, 100e-6))   # continuous, because you said so
```

### 4.3 `Measure` lifecycle

`t.arm(...)` installs a level-1 accumulator; `.value` reads it at a test
breakpoint. That is the difference between testing a reaction and performing an
autopsy:

```python
droop = t.arm(pip.Measure.min("out"))
t.install("out", pip.Wave.ramp(10e-3, 100e-3, 1e-9))
yield t.advance(50e-6)
if droop.value < nominal * 0.9:
    ...                       # the test adapts, in the same run
```

The lifecycle, stated because "arm and read" leaves four real questions open:

- **`arm` creates a fresh accumulator starting at the arm instant.** Arming the
  same measure twice yields two independent measures — a measure is a handle, not
  a name in a global table.
- **It accumulates on accepted points only** (§7). A rejected step contributes
  nothing; this is a property of where the crate is fed, not a rule the author
  must remember.
- **Reading `.value` before any accepted point has fed it is loud** — not `NaN`,
  not zero, not the nominal. A measure with no samples has no value, and
  returning a plausible one is the failure mode this project exists to refuse.
  The same applies to a gated measure whose window never opened: *"never
  sampled"*, named, not `0.0`.
- **`disarm()` freezes it.** After disarming (or after its gate window closes for
  good), `.value` stays readable and stops moving — so a measure scoped to one
  phase of a test can be read in a later phase.

`.value` always travels with `.error_bound` (§3.6); an `expect` against a measure
compares against the bound, never the point estimate alone.

---

## 5. The cost model, stated per level

Exposing this lets an author make the trade themselves; hiding it behind
restrictions was revision 1's mistake.

| What you write | Level | Host calls | Per | Practical ceiling |
|---|---|---|---|---|
| `Wave.*` descriptor (except `from_fn`) | 1 | **0** | — | unlimited |
| `Wave.sampled(numpy)` | 1 | **0** | — | unlimited |
| `Behavior.*` (catalog or Rust-authored) | 1 | **0** | — | unlimited |
| `Measure.*` accumulation | 1 | **0** | — | unlimited |
| `Sequencer` wake | 2 | 1 | test breakpoint | ~10⁵ per run is free (~1 µs each) |
| `Wave.from_fn` | 1→host | 1 | evaluation | ~10⁵–10⁶ total |
| `load_dc`/`load_transient` in Python | 0 | 1 | **Newton iteration** | ~10⁵–10⁶ total |
| `update` in Python | 0 | 1 | accepted step | ~10⁵–10⁶ |
| any of the above in Rust | — | native | — | no ceiling |

The rules that follow — guidance, not prohibition:

- **A boundary device is cheap; a bulk device is not.** A tester or a stimulus
  source is a handful of elements. Ten thousand Python memristors is a bad idea,
  and the docs should say so rather than prevent it.
- **Precompute beats callback.** `Wave.sampled` with numpy is free in the loop.
- **Reach for level 1 before level 0.** The catalog exists so the common reactive
  cases never pay per-iteration.
- **Rust is the answer for bulk, and for extending the catalog.** Same contract,
  no per-call cost — which is what makes parity worth having.
- **The clock is still better off in the design.** Arithmetic, not policy: a
  sequencer-driven 1 GHz clock over 1 ms is 2 × 10⁶ forced solver landings.
  Synchronizing to an existing clock (`t.wait(edge("clk"))`) costs nothing extra.

---

## 6. Rust/Python parity — one contract, two bindings

```rust
pub trait HostElement {
    fn name(&self) -> &str;
    fn capabilities(&self) -> ElementCapabilities;
    // everything else defaulted, mirroring `Element` itself
    fn load_dc(&mut self, _ctx: &Ctx, _m: &mut StampSink) {}
    fn update(&mut self, _state: &StateView, _ctx: &Ctx) {}
    fn next_breakpoints(&self, _from: f64, _horizon: f64) -> Vec<f64> { vec![] }
    /* … */
}
```

- **Rust** implements the trait directly; a blanket adapter turns any
  `HostElement` into an `Element`, so the solver sees no difference.
- **Python** subclasses `pip.Device`, defining only the methods it needs; the
  bridge dispatches on presence. Method names, argument order, and semantics are
  **identical** to the Rust trait.
- **The one deliberate asymmetry** is §3.7: the level-1 catalog is closed in
  Python and open in Rust. This is not a parity failure — the *surface* is
  identical, and the extension point is exactly where native code is required
  anyway.
- **`Sequencer`** is a `HostElement` whose `next_breakpoints` comes from its
  coroutine. Python uses a generator; Rust implements `resume` as an explicit
  state machine (no stable generators). That asymmetry is real and stated.
- Parity is enforced the way the project already enforces it: a
  `host_parity.rs`-style target enumerating the surface on both sides.

---

## 7. Where the guarantees apply — and where they do not

D11 (no rollback protocol, because explicit `advance` is the guarantee) is a
**level-2 property**. It does not extend downward:

| Level | Rejected steps | Rule |
|---|---|---|
| 2 (sequencer) | never observed — resumed only after acceptance | D11 holds; no rollback logic in the test |
| 1 (`Wave`/`Behavior`/`Measure`) | evaluated during them, like any native mechanism | the crate is pure — re-evaluating is free and side-effect-free. A `Measure` accumulates **only on acceptance** |
| 0 (raw stamping device) | **participates in them** like any element | the author handles `accept_timestep` and mutates state only on acceptance, exactly as a PHDL device must |

The level-0 row must be loud in the documentation: a device that integrates state
in `load_transient` instead of `accept_timestep`/`update` is subtly wrong in a way
that only appears under timestep rejection — the same trap PHDL authors already
face, now reachable from Python.

---

## 8. Recorded decisions and their costs

### 8.1 `Measure` is a **separate engine** from `dv-core`'s `constraint`

`dv-core` already reduces signals over time with argmin tracking, for `constraint`
margins. `Measure` does the same class of work. They are **not** the same engine,
by decision (user, 2026-07-31): `dv-core`'s path is JIT-lowered from PHDL, and
rebuilding it as a host-constructible engine is a large rewrite for little
*current* gain.

The cost is real and named: **two definitions of "peak" now exist in the tree**,
and they will drift silently. Three cheap mitigations, none of which requires
shared code:

1. **One written semantics.** What peak/settle/cross mean at a rejected step, how
   ties break, whether values interpolate — specified once, cited by both.
2. **A cross-check test.** One fixture reduced by both paths, asserted to agree —
   the same oracle discipline `dv-gradients` used against `.sens`.
3. **A shared output channel.** `Measure`-fed `expect`s report through `dv-core`'s
   `validation_reports()`, so posture (`strict`/`collect`), provenance and time
   stamping are shared even though the evaluators are not.

Convergence path, recorded rather than promised: once level 1 exists and is
proven, codegen may adopt it (§12.16) — shrinking codegen and giving `constraint`
the same mechanism. That is a later feature, not a prerequisite.

### 8.2 Peak resolution: honest by default, exact on request

§3.6. Default reduces over accepted points and publishes its error bound;
`refine=True` buys exactness with simulation time. Never a bare plausible number.

### 8.3 Gating ships in V1

Time windows, analog gates (crossing predicates) and digital gates (settled events
only). §3.5.

### 8.4 Level 1 is a crate, not a module

§3. Its consumers are more than the solver and codegen, its purity is what makes
that true, and it depends on nothing in the tree.

### 8.5 Extraction before invention

§3.9. Codegen already owns correct, tested implementations of the runtime
operators, the event detectors, `table`, and the junction limiter — all in files
that are **already pure**. Level 1 lifts those rather than reinventing them, and
the reuse direction that §8.1 recorded as a vague "maybe later" is, for these four
families, the concrete plan: extract, then delegate (§12.16).

Two consequences worth stating because they are easy to lose:

- **The crate's first commit is mostly a move, not a design.** Its correctness
  bar on day one is "the existing tests still pass, relocated" — a far stronger
  starting position than a greenfield signal library.
- **Rules nobody would re-derive survive.** The `transition` re-anchor, the
  `above`-vs-`cross` seeding asymmetry, the five pnjlim rules — each was learned
  from a failure. Rewriting from the operator's *name* would silently drop them.

### 8.6 A host device is declared in PHDL, like every other device

**Decision (user, 2026-07-31):** a host device is **declared in PHDL and
instantiated like any module**, not attached to a compiled session from the host.

The rejected alternative — hand the session a Python object — is genuinely
simpler to build and is the wrong shape for the designer. A device that exists
only inside one script cannot be instantiated twice, cannot sit inside a
hierarchy, cannot be parameterized by the instantiating module, and cannot ship
as a part. Declaring it in PHDL gives all four for free, because they are
properties of *being a module*, and the designer never learns a second way to
place a device on a schematic.

The cost is a declaration: the module's ports and params are written in PHDL,
and only its *behavior* comes from the host. That cost is already paid by the
existing plugin path — which is the whole of §8.7.

### 8.7 One device concept, two binding modes

`@device` already exists (`headers/device_port.phdl`, MD-24): a PHDL module
that declares its ports and params and says its implementation comes from
outside PHDL. That is exactly what a host device is. So there is **one concept,
not two**, and the tiers differ only in *where the implementation is bound*:

| Binding mode | Implementation lives in | Bound at | Distribution |
|---|---|---|---|
| **plugin** (exists) | a dlopen'd cdylib, `#[pip::device]` + `inventory` | plugin load, TOFU-pinned (MD-30) | a release artifact |
| **host** (this feature) | a class/impl in the running process | session build | the script itself |

Both converge on `Element` through the blanket adapter (§6) — the solver cannot
tell them apart, which is what makes "one concept" true at the level that
matters rather than only in the documentation.

**No new macro.** A Rust host device implements `HostElement` and is registered
explicitly; it does not reuse `#[pip::device]`, whose `inventory` collection is
a plugin-binary mechanism with no meaning in-process. MD-13 rule 5 (no macros)
makes not adding one the correct answer rather than a compromise. Python's
`@pip.device` decorator performs the same registration in the idiom that
language already uses for it — the *concept* is identical, the spelling follows
each language's grain.

**The consequence worth designing for: promotion.** A Rust host device can
become a plugin device with **no change to the PHDL** — same `@device`
declaration, same trait impl, the implementation simply moves from a script into
a distributed binary. Prototype in-process, promote when it stabilizes, and the
circuits that use it never notice. That path also answers "can a host device be
distributed" (§12): not *as* a host device — it graduates. A **Python** host
device cannot graduate (there is no Python cdylib), so it stays script-local:
fine for testers, limiting for models, and stated rather than discovered.

---

## 9. Examples spanning the range

**A — a resistor in twelve lines** (level 0; the "hello world" proving the ABI is
genuinely exposed):

```python
class R(pip.Device):
    caps = pip.ANALOG | pip.LOADS_DC | pip.LOADS_AC | pip.LOADS_TRAN
    def __init__(self, p, n, r): self.p, self.n, self.g = p, n, 1.0 / r
    def load_dc(self, ctx, m):
        m.g(self.p, self.p, +self.g); m.g(self.p, self.n, -self.g)
        m.g(self.n, self.p, -self.g); m.g(self.n, self.n, +self.g)
    load_ac = load_transient = load_dc
```

**B — a sigmoid supply ramp** (level 1, zero host calls):

```python
class SoftStart(pip.Device):
    def build(self, b):
        b.vsource("vdd", "gnd", rout=0.05,
                  wave=pip.Wave.sigmoid(lo=0.0, hi=1.8, t0=200e-6, tau=40e-6))
```

**C — an arbitrary measured waveform** (level 1 via numpy, still zero host calls):

```python
t_us, v = np.loadtxt("captured_supply.csv", unpack=True)
class Replay(pip.Device):
    def build(self, b):
        b.vsource("vdd", "gnd", wave=pip.Wave.sampled(t_us * 1e-6, v), rout=0.05)
```

**D — a hysteretic PWM controller** (level 1 `Behavior`; the case revision 2 had
no answer for). Reacts to two terminals at every Newton iteration, with **zero**
host calls:

```python
class HystPWM(pip.Device):
    def build(self, b):
        err = pip.Behavior.hysteresis("vout", "vref", vth_hi=5e-3, vth_lo=-5e-3)
        b.switch("sw", "gnd", ctrl=err, ron=50e-3, roff=1e6)
```

**E — load transient, measured and reacted to** (level 2 orchestrating level 1;
the archetype for "insert a signal, test the reaction"):

```python
class LoadTransient(pip.Sequencer):
    def build(self, b):
        b.vsource("vdd", "gnd", wave=pip.Wave.const(1.8), rout=0.05)
        b.isource("out", "gnd", wave=pip.Wave.const(10e-3))       # nominal load

    def run(self, t):
        yield t.advance(100e-6)                                    # settle
        nominal = t.read_v("out")

        droop = t.arm(pip.Measure.min("out"))
        recov = t.arm(pip.Measure.settle("out", nominal, tol=0.01 * nominal))

        t.install("out", pip.Wave.ramp(10e-3, 100e-3, 1e-9))        # load step
        yield t.advance(50e-6)

        t.expect(droop.value > nominal * 0.95, f"droop {droop.value:.4f}")
        t.expect(recov.value < 20e-6,          f"recovery {recov.value*1e6:.1f} us")

        for i_load in (150e-3, 200e-3, 250e-3):                     # and adapts
            d = t.arm(pip.Measure.min("out"))
            t.install("out", pip.Wave.const(i_load))
            yield t.advance(20e-6)
            if d.value < nominal * 0.9:
                t.note(f"regulation lost at {i_load*1e3:.0f} mA"); break
```

**F — burst on the clock edge with an analog check** (level 2 + level 1):

```python
class BurstTest(pip.Sequencer):
    def run(self, t):
        yield t.advance(1e-6)
        for burst in range(8):
            for _ in range(16):
                yield t.wait(pip.edge("clk", pip.RISING));  t.drive("data", 1)
                yield t.wait(pip.edge("clk", pip.FALLING)); t.drive("data", 0)
            t.install("vbias", pip.Wave.ramp(0.4, 0.4 + 0.05 * burst, 100e-9))
            peak = t.arm(pip.Measure.peak("vpeak"))
            yield t.advance(1e-6)
            t.expect(peak.value < 1.9, f"burst {burst}: vpeak {peak.value:.3f}")
```

**G — a device that both stamps and sequences** (the composition revision 1 made
impossible):

```python
class ActiveLoadSweep(pip.Sequencer):
    caps = pip.ANALOG | pip.LOADS_DC | pip.LOADS_TRAN
    def __init__(self, p, n): self.p, self.n, self.g = p, n, 1e-3
    def load_dc(self, ctx, m):                      # its own conductance
        m.g(self.p, self.p, +self.g); m.g(self.n, self.n, +self.g)
        m.g(self.p, self.n, -self.g); m.g(self.n, self.p, -self.g)
    def run(self, t):                               # …swept over time
        for g in (1e-3, 5e-3, 2e-2):
            self.g = g
            yield t.advance(10e-6)
            t.note(f"g={g}: vout={t.read_v('out'):.4f}")
```

---

## 10. Implementability

Honest sizing — larger than revision 2, because two families and a crate were
added:

**E** = extracted from codegen (§3.9 — already written, already tested, moves
mostly intact). **N** = new code.

| Piece | Crate | | Estimate |
|---|---|---|---|
| Runtime ops (`delay`/`slew`/`transition`/`idt`) | `piperine-signal` | **E** | ~250 |
| Event detectors (`cross`/`above`/`timer`) | `piperine-signal` | **E** | ~150 |
| `Behavior::table` (pure piecewise, free derivative) | `piperine-signal` | **E** | ~120 |
| Limiter (pnjlim/fetlim, the five rules) | `piperine-signal` | **E** | ~200 |
| Crossing tier 2: prediction + bounds + error report | `piperine-signal` | **N** | ~250 |
| `Wave` descriptors + evaluation + breakpoints | `piperine-signal` | **N** | ~350 |
| `Behavior` kinds: value + hand-written Jacobians (~8) | `piperine-signal` | **N** | ~400 |
| `Measure` accumulators + gating + error bounds | `piperine-signal` | **N** | ~400 |
| `verify_derivatives` FD oracle | `piperine-signal` | **N** | ~100 |
| `HostElement` trait + blanket `Element` adapter | `piperine-api` | **N** | ~400 |
| `StampSink` / `Ctx` / `StateView` safe wrappers | `piperine-api` | **N** | ~350 |
| Level-1 → stamp adapter (incl. `limiting_report` surfacing) | `piperine-api` | **N** | ~350 |
| `@device` host binding mode (PHDL declaration → in-process impl) | `piperine-api` + `lang` | **N** | ~250 |
| Dynamic digital subscriber (edge waits, §4.1) | `piperine-solver` | **N** | ~200 |
| Sequencer (coroutine driver, wakes, findings) | `piperine-api` | **N** | ~350 |
| PyO3 bridge (dispatch, exceptions, GIL) | `piperine-python` | **N** | ~450 |
| Python surface (`pip.Device`/`Wave`/`Behavior`/`Measure`/`Sequencer`) | `piperine-python` | **N** | ~350 (Py) |
| Protocol helpers (uart/spi/vectors) | — | **N** | ~300 (Py) |

≈ 3 700 lines of Rust and ~650 of Python, of which ~2 200 is the new crate — but
**~720 of that crate is extraction, not invention**, and it arrives with its
existing tests. Codegen's delegation step (§12.16) is *not* counted here; it is
scheduled separately because it touches correctness-critical files.

Nothing in the JIT or the solver core: the blanket adapter means the solver keeps
seeing `Element` and nothing else.

### What could go wrong

| Risk | Containment |
|---|---|
| **Crossings land late** — no retry-at-`t*` exists (verified) | §3.4 — ship tier 2 (predict + `bound_step`, error published) on seams that already exist; tier 3 is a scoped solver feature, not a blocker |
| Extraction leaves codegen with a stale copy of A–D | §3.9 / §12.16 — the delegation step is scheduled, not assumed; a per-operator cross-check fixture holds the line until it lands |
| A hand-written `Behavior` Jacobian is wrong | §3.8 — FD oracle shipped with the crate; a new kind is not done until it passes |
| Two definitions of "peak" drift apart | §8.1 — one written semantics, one cross-check fixture, shared findings channel |
| `Measure` returns a plausible wrong overshoot | §3.6 — error bound published by default; `expect` compares against the bound |
| Exposing the stamp matrix to Python unsoundly | The sink is a narrow, index-validated API — never a raw pointer; invalid indices are loud |
| **The GIL**: a Python device holds it during the solve | Documented as a property of the Python tier; level 1 sidesteps it entirely, which is the main reason the catalog exists |
| Python exceptions mid-solve | One catch point in the bridge; typed error carrying simulation time; the analysis stops cleanly |
| Authors integrating state in `load_*` instead of `accept_timestep` | §7 is documentation *and* an example |
| Dynamic digital sensitivity (edge waits) needs scheduler surgery | **In scope** (§4.1, user decision) — the seam is a subscriber list consulted *after* net settling, not a new event kind: `EventKind::Digital` already carries the edge. Sequence it after time wakes, so A–E and G are provable before F |
| The solver gains a subscriber list that only the tester uses | It is the same mechanism a future in-language monitor or a live-plotting host would want; a one-consumer seam is a smell, and this one has three plausible consumers |
| Per-call cost surprises a user who wrote a bulk Python device | §5 is published, with the Rust escape route named |
| A third device path fragments the story | §0's table: four paths, four trade-offs, one ABI underneath |
| A new crate that turns out to have one consumer | §3.1's purity is the test — if it cannot reduce a stored `Waveform` without a solver, it was built wrong |

---

## 11. Deliberately out

| Not included | Why |
|---|---|
| A test-runner framework | `piperine test` and `*_tb.py` already exist |
| Randomization / constrained-random over parameters | `dv-gradients`' Monte Carlo owns statistical sampling |
| Scoreboards, transaction layers | Python data structures already express these; ship examples, not a framework |
| Distribution/packaging of host devices | Plugin territory (`piperine-plugin`); §12.6 |
| Circuit-level assertions (SOA, spec limits) | `dv-core`'s `constraint` blocks — different rhythm, different home |
| Replacing `dv-core`'s constraint engine with level 1 | §8.1 — a later feature, deliberately not a prerequisite |

---

## 12. Open questions

**Resolved by the 2026-07-31 audit (§3.4, §3.9) — kept for the record:**

- ~~*Can an element force a retry at a computed instant?*~~ **No**, and PHDL's own
  `@cross` does not either — it fires at the first accepted point after the
  crossing, unreported. `SUPPORTS_ROLLBACK` *is* live (checkpoint/restore around
  every attempt), but nothing lets an element name the retry instant. Level 1
  ships tier 2 (predict + bound, error published); tier 3 is a solver feature on
  scaffolding that already has the right shape.
- ~~*Is level 1 a rewrite of what codegen has?*~~ **No** for the four operator/
  event/table/limit families — codegen's implementations are already pure and the
  plan is extract-then-delegate (§3.9). It remains a genuine second
  implementation only for `Measure`-vs-`constraint` (§8.1).

**Decided 2026-07-31 (user, except where noted) — the design log:**

| # | Question | Decision |
|---|---|---|
| 1 | Crate name | **`piperine-signal`** (§3) |
| 2 | Is `Measure` Rust-extensible too? | **Yes, Rust only** — same rule as `Behavior`: closed catalog in Python, open trait in Rust (§3.7) |
| 3 | Digital gate rhythm | **Settled events only.** Rationale upgraded from taste to correctness: a mid-delta-cycle value can be a DAG-evaluation-order artifact rather than a signal the circuit had (§3.5) |
| 4 | Relationship to `#[pip::device]` | **One concept, two binding modes** — `@device` in PHDL for both; plugin binds from a cdylib, host binds in-process; no new macro (agent decision, delegated; §8.7) |
| 5 | How a host device enters a circuit | **Declared in PHDL**, instantiated like any module (§8.6) |
| 6 | Can a host device be distributed? | **It graduates rather than distributes** — a Rust host device becomes a plugin device with no PHDL change; a Python one stays script-local (§8.7) |
| 7 | Digital edge waits in V1? | **Yes** — a tester that cannot synchronize to a clock is not a tester (§4.1) |
| 8 | `install` semantics | Time is **install-relative**; a replacement takes effect immediately with its discontinuity **registered, never smoothed**; continuity is explicit (§4.2) |
| 9 | `Measure` lifecycle | `arm` creates a fresh accumulator; accepted points only; reading before any sample is **loud**; `disarm` freezes (§4.3) |

**Decided by the agent, flagged for override if any reads wrong:**

10. **DC/OP with a sequencer attached.** The coroutine does **not** run at DC —
    it sequences time, and DC has none; `run()` begins with the transient. Each
    `Wave` kind declares **its own DC value**, following SPICE's convention so a
    reader arriving from that tool is not surprised: `const`→its value,
    `ramp`→start, `pulse`→initial level, `sin`→offset, `pwl`/`sampled`→first
    point, `sigmoid`→`lo`, `from_fn`→evaluated at `t = 0`. AC stays loud (D13).
11. **`set_param` from a host device mid-transient.** Restamp-class writes go
    through the existing live-parameter path and are fine; rebuild-class is
    **loud**, never a silent mid-analysis re-elaboration (MD-18).
12. **Ordering among host devices at one instant.** The **authored POM
    instantiation order** — which §8.6 makes visible and controllable, since host
    devices are now declared modules rather than objects handed to a session.
    Note this governs *independent* actions only: two devices **driving the same
    net** is a loud conflict regardless of order, not a last-writer-wins race.

**Genuinely still open (none blocking):**

13. **How does `refine=True` ask for step refinement?** `bound_step_hint` is the
    natural seam (§3.9 F) — a `Measure` near an extremum caps the next step. Needs
    a convergence check: a measure that keeps shrinking dt must terminate at the
    step floor and report, not stall.
14. **Are the protocol helpers (uart/spi/vectors) shipped or exemplary?** §11 says
    ship examples, not a framework. Revisit once one real test exists.
15. **Does `Measure` eventually unify with `dv-core`'s `constraint`?** Owned by
    `../codegen-simplification/ideal.md` §6.1, decided there, not here.

**Owned by the follow-on feature, recorded so it is not rediscovered:**

16. **Codegen delegates to `piperine-signal`** — decided (user, 2026-07-31):
    `dv-tester` **extracts**, and a separate feature
    (`../codegen-simplification/ideal.md`) **delegates**, deleting codegen's
    copies. That feature is the next priority after this one. The interim cost is
    this feature's to pay: **per-family cross-check fixtures** (one fixture, both
    paths, asserted equal) authored here and deleted there. Without them the two
    copies drift and the follow-on turns from "delete a copy" into "reconcile two
    behaviors".

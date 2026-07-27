# FOLLOW_UP_IDEAL — verification, optimization, and centering as one declaration

**Status:** vision document, not a spec. Written 2026-07-26 to feed `ROADMAP.md`
P7 (Optimizer) and a future P8 (Design Verification). Nothing here is
implemented; nothing here is committed to. Syntax is illustrative and uses real
PHDL grammar (`mod`/`analog`/`digital`/`bundle`/`discipline`/`param` blocks,
`V(a, b) <- expr` contributions) so the sketches can be read against the tree.

**Scope decision (2026-07-26):** of everything explored in this document,
exactly **two** constructs pass the No-Bloat burden of proof and enter the
grammar:

1. **`tol` on `param`** — statistical variation declared on the parameter it
   perturbs, visible in the POM.
2. **The `constraint` block** — a third body kind whose statements evaluate
   inside the kernel at every accepted solver point.

Everything else — the optimizer, Monte Carlo, centering, coverage closure,
high-sigma methods — is host code, stdlib, or explicitly deferred. The rest of
this document argues why those two, and only those two, deserve layer 0.

**Expertise pass (2026-07-27).** Every checkable claim below was verified
against the tree and is cited by `file:line`. Five inconsistencies were resolved
into design text — the pointwise/reduced split (§2.1a), evaluation points
(§2.3a), contextual keywords (§2.3b), absent-by-default SOA limits (§2.5), and
the validation channel's shape and capability bit (§2.6) — and two new forks
were added to §9 (adjoint DC-vs-AC ordering, where reduced metrics are
computed). Three claims turned out **stronger** than written: the RNM resolve
kinds are already in the formal grammar, `AnalogDevice` already exists as the
home for `param_stamps`, and hierarchical instance-port refs already work.

---

## 0. The thesis

Every commercial AMS flow treats verification, optimization, and centering as
**three separate tools** bolted onto a simulator: a checker deck, an optimizer
cockpit, and a Monte Carlo wrapper. Each re-declares the same knowledge — what
"working" means for this circuit — in its own dialect. The designer writes the
SOA limit in the checker, the objective in the optimizer, and the yield target
in the MC setup, and nothing enforces that the three agree.

**The claim of this document: those three things are one declaration.**

A constraint says "this quantity must stay on this side of this level". Read it
three ways and you get all three tools:

| Read the same constraint as… | You get |
|---|---|
| a predicate checked over the trajectory | **verification** (SOA, ERC, spec compliance) |
| a feasibility boundary with a signed distance | **optimization** (constrained sizing) |
| a distance measured in units of statistical spread | **design centering** (yield) |

Piperine can make this the *native* form because of three assets no
SPICE-derived flow has:

1. **Symbolic differentiation already in the compiler.** `resolve/diff.rs`
   exists to emit the Jacobian. Pointed at parameters instead of unknowns, the
   same machinery yields ∂(anything)/∂(any parameter) — gradient-based sizing
   instead of black-box search. This is the single biggest lever in the document.
2. **Compile once, restamp (MD-18).** Already proven by `urc_compile_count.rs`
   and `compile_once_sweep.rs`. An optimization loop and a Monte Carlo run *are*
   parameter sweeps over an already-JITed circuit. The per-iteration cost is
   already near the floor.
3. **Analog and digital in one process, no tool boundary.** `06_flash_adc.phdl`
   already reads `V(vin, gnd)` inside a `digital` body. The A/D boundary is an
   internal detail, not an integration between two simulators.

Everything below is an attempt to spend those three assets well — and to spend
*nothing else*.

---

## 1. Language addition one: `tol` on `param`

Statistical variation is part of a parameter's declaration, not a separate
deck. It attaches to the parameter it perturbs, survives monomorphization, and
is readable from the POM.

```phdl
mod Mos1(...) {
    param vto : Real = 0.7  tol gauss(sigma = 0.005 / sqrt(w * l));  // mismatch: per-instance
    param u0  : Real = 600.0 tol gauss(sigma_rel = 0.02) global;     // process: shared
    param tox : Real = 4e-9   tol uniform(rel = 0.02);               // ±2% uniform
}
```

- `tol <dist>(args)` follows the default. Distributions are **declared names**,
  not grammar: `gauss(sigma | sigma_rel)`, `uniform(half | rel)`, and friends
  live as `extern` declarations in a `headers/statistics.phdl`, per MD-24.
  Adding a distribution never touches the parser.
- Without `global`, the variation is **mismatch** — independent per instance.
  With `global`, it is **process** — one draw shared by every instance. That
  distinction is the whole game in analog yield, and putting it next to the
  parameter beats a separate mismatch deck.
- The distribution is **inert by default**. A nominal `op()`/`tran()`/`ac()`
  uses the declared value and is fully deterministic; a sim that secretly
  resamples parameters would be a trap. Sampling happens only when the host
  asks for it (`monte_carlo`, `center`), under an explicit seed. The POM
  carries the distribution unconditionally — `Param.distribution()` — so the
  host can enumerate the statistical knobs without simulating anything.
- A tolerance on a parameter does not change its `Invalidation` class: a
  sampled draw is an ordinary restamp write through the MD-18 path.
- Distribution arguments are elaboration-constant expressions and may
  reference sibling params (`sigma = 0.005 / sqrt(w * l)`) — they evaluate in
  the module's param scope, like any parameter default.
- `tol` applies to `Real` params. A statistical clause on a discrete param
  (`Integer`/`Natural`) is rejected loud — discrete variation is a different
  mechanism (§3.4's discrete-knob discussion), not a silent rounding.

**Rejected sugar.** A percent literal (`tol 5%`) was considered and rejected:
`%` is already the modulo operator in `MulExpr`, and a second lexer meaning
for it is exactly the kind of surface growth No-Bloat exists to stop. The
distribution form covers the intent: `tol uniform(rel = 0.05)`.

**Why this passes No-Bloat.** The alternative is declaring variation in the
host — which re-declares every parameter name in Python, breaks the moment a
model refactors, and cannot ship with a foundry PDK written in PHDL. Variation
attached to the parameter composes with hierarchy for free. This is the same
argument as §2.5 for constraints: knowledge that belongs to the *model* must
live where the model lives.

---

## 2. Language addition two: the `constraint` block

A third body kind alongside `analog` and `digital`, attached to a module by
name, exactly as they are.

```phdl
mod Amplifier(input vin : Electrical, output vout : Electrical,
              inout vdd : Electrical, inout gnd : Electrical) {
    param w1 : Real = 2e-6;
    param w2 : Real = 4e-6;
    param ibias : Real = 10e-6;
}

constraint Amplifier {
    // ── hard limits: violation means the circuit is wrong, not off-spec ──
    require vds_m1     : abs(V(m1.d, m1.s)) <= 1.98;
    require headroom   : V(vout, gnd) >= 0.2 && V(vdd, vout) >= 0.2;
    require no_fwd_bulk: V(m1.b, m1.s) <= 0.3;

    // ── named scalars the host can read and the optimizer can use ──
    var gain_db  = 20.0 * log10(abs(ac_gain(vout, vin)));
    var ugbw     = ac_unity_gain_freq(vout, vin);
    var phase_m  = ac_phase_margin(vout, vin);
    var power    = abs(I(vdd)) * V(vdd, gnd);

    // ── spec: soft constraints with a scale, giving normalized margins ──
    target gain_db >= 60.0  tol 3.0;
    target ugbw    >= 10e6  tol 1e6;
    target phase_m >= 60.0  tol 5.0;
}
```

There is no `minimize` in the block. The objective is **host-side**: it is the
intent of one optimization run, not a truth about the model, and optimization
scripts typically run once. The block declares the *vocabulary* (`var`s,
`target`s); Python picks the objective per run — `ota.optimize(objective=
"power", …)`. An objective naming an unknown `var` fails loud at the host
boundary.

### 2.1 The three statement kinds

| Statement | Meaning | Margin | Violation |
|---|---|---|---|
| `require <name> : <pred>;` | invariant that must hold everywhere it is evaluated | unnormalized signed distance | **loud failure** in strict posture — analysis errors, reports instance + time + value |
| `target <expr> <cmp> <level> [tol <scale>];` | specification the design should meet | signed distance ÷ `tol` | recorded, not fatal; drives optimization and centering |
| `var <name> : <Type> = <expr>;` | named scalar extracted from the analysis | — | — |

`require` is for physics and reliability: SOA, oxide field, junction polarity,
supply headroom, current density. `target` is for specification, where "worse"
is a gradient, not an error.

A `var` in a constraint block is the same keyword with its ordinary meaning —
bind a computed value to a name — under two block-specific rules: single
assignment, and exported by name to the host (it is how `r.margins`, the
objective reference, and coverage expressions address computed scalars). No
new vocabulary: `var` already exists in every `fn`-body grammar in PHDL, and
it keeps its **mandatory type annotation** there (`var code : Real = 0.0;` in
`06_flash_adc.phdl`, one of 930 `var` sites in the tree, every one annotated).
A constraint block does not get inference the rest of the language lacks —
that would be a parser special case bought for four saved characters.

### 2.1a Two classes of measured quantity — the distinction that decides lowering

`var` and `target` expressions split into two kinds, and **conflating them is
the single easiest way to make this feature unimplementable**:

| Class | Example | Exists at | Lowers to |
|---|---|---|---|
| **Pointwise** | `abs(I(vdd))`, `V(vout, gnd)`, `m1.region` | every accepted solver point | a margin kernel, evaluated per point (§6) |
| **Reduced** | `ac_unity_gain_freq(vout, vip)`, `ac_phase_margin(…)`, settling time, overshoot | only after a whole sweep is known | a post-analysis reduction over the collected points |

Unity-gain frequency is not a property of a frequency point; it is a property
of the *sweep*. No per-point kernel can compute it, because at any single
frequency the answer does not yet exist. The same is true of phase margin,
settling time, overshoot instant, and every "where did the curve do X" metric.

Three consequences that must hold in the design:

1. **Each declared helper carries its class.** `headers/constraints.phdl`
   declares `ac_gain` as pointwise and `ac_unity_gain_freq` as reduced (§6's
   MD-24 note). The class is part of the declaration, not folklore.
2. **A `require` cannot read a reduced quantity inside a pointwise scope.**
   `@ tran { require settle : ac_phase_margin(…) >= 60.0; }` is a loud
   elaboration error, not a kernel that returns garbage per step. A reduced
   quantity is checkable only *once*, against the completed analysis.
3. **Margins therefore come in two shapes too.** A pointwise margin has an
   argmin (time or frequency + instance); a reduced margin has one value per
   analysis and no argmin. The host surface must say which it is rather than
   reporting a fake `t = 0`.

Gradients inherit the split: `∂m/∂p` for a pointwise margin is the adjoint at
the argmin point (§3.4's non-smoothness caveat); for a reduced margin it is
the derivative of the reduction, which exists only if the reduction itself is
differentiable — a unity-gain *frequency* differentiates through the implicit
function theorem on `|gain(f)| = 1`, while an event-detected settling time
mostly does not. Fail loud on the ones that do not (§3.4).

A `var` or `target` whose expression calls an analysis-specific helper —
`ac_gain` exists only in AC — is defined only in that analysis; reading it
from another analysis is a loud "not defined in this analysis", never a stale
or zero value.

**Why `require` is not `$assert`.** `$assert` is a diagnostic: it prints. A
`require` produces a *value* — the signed margin — with provenance (instance,
time, worst point), readable by the host, differentiable by the compiler, and
consumable by three different tools. An assert that a margin read as "−0.03"
is a print statement; the same declaration as a `require` is the objective
function of a centering run. Same surface idea, different type.

### 2.2 Margins — the one mechanism behind everything

Every comparison lowers to a **signed margin function**:

```
a <= b          →   m = (b − a) / scale
a >= b          →   m = (a − b) / scale
a in [l,u]      →   m = min(u − a, a − l) / scale
```

with `scale = 1` when no `tol` is given. (The range form spells `in`, keeping
the window vocabulary of §2.4 free of double duty.) Two roles share the
spelling `tol`: on a `param` it declares a distribution (§1); on a `target` it
declares the normalization scale. The rhyme is deliberate — both express "how
much slack exists" — but a spec may rename the target's to `scale` if the
collision reads badly. The invariant that makes the whole
design work:

> **`m ≥ 0` ⟺ satisfied.** `m` is a real number the compiler can differentiate.

That single convention is what collapses three tools into one declaration:

- **Verification** evaluates `m` at every accepted solver point and fails
  `require`s where `m < 0` (strict posture).
- **Optimization** treats `{m_i ≥ 0}` as the feasible set and has
  `∂m_i/∂p` from the compiler (§3).
- **Centering** maximizes `min_i (m_i / σ_i)` where `σ_i` is the spread of
  `m_i` under the declared `tol` statistics — worst-case-distance centering,
  and the normalized margin is exactly the quantity it needs.

Nothing else in this document is as important as that table. Get margins right
and the rest is engineering.

### 2.3 Check posture: strict, collect, off

A `constraint` block is **evaluated at runtime, per accepted solver point** —
and it is **skippable**, because the right posture depends on what the run is
for:

| Posture | Behavior | Who uses it |
|---|---|---|
| `strict` (default) | `require` margin < 0 → loud analysis failure naming instance + time + value | verification runs, `piperine test` |
| `collect` | margins recorded, never fatal | **the optimizer's inner loop** — an infeasible iterate is a margin to climb out of, not an exception to catch; Monte Carlo — a failing sample is a yield event, not an abort |
| `off` | constraint kernels not called; zero cost | performance-critical sweeps by a host that already knows the design is legal |

The posture is a solver/host config knob (`pip.Solver(checks=…)`), not a
language construct; it joins the canonical `Solver`/`SolverConfig` field set
on both hosts (HOST-20 parity). The default stays strict: silent pass is
never the surprise a user gets by default.

### 2.3a Which points count — the definition that keeps strict posture honest

"Per accepted solver point" is not precise enough to implement, and the
imprecise reading produces false violations on circuits that are fine. A
margin is evaluated at, and only at:

| Analysis | Evaluated at | **Not** at |
|---|---|---|
| DC / OP | the **final converged solution** | any homotopy stage — gmin stepping and source stepping each converge at intermediate stages that are *non-physical by construction* (gmin adds shunt conductance; source stepping scales every source down). A headroom or SOA `require` checked at a gmin stage fails on a solve that is converging correctly. |
| Transient | each **accepted** timestep | rejected steps (the LTE/rollback path — `SUPPORTS_ROLLBACK`, `accept_timestep`); a rejected step's state is discarded and must never emit a finding |
| Transient at `t = 0` | the operating point, if one was computed | the UIC / `@initial` state — a user-forced initial condition may legitimately sit outside SOA before the circuit has settled. Skip `t = 0` under UIC, or the first honest transient a new user runs reports a violation they did not cause. |
| AC / noise | each frequency point (pointwise margins); once per sweep (reduced, §2.1a) | the DC operating point solve underneath it, which the DC row already covers |
| Sweep / `sweep_grid` | each swept point, reduced to a worst-across-sweep margin with the **swept coordinate** as part of the argmin | — |

The sweep row is not a detail: `sweep`/`sweep_grid` over a compiled session is
the loop that optimization, corner runs, and Monte Carlo are all built from
(§4). "Worst margin over the sweep, and at which point" is the quantity those
three consume, so it is a first-class result, not something the host
reconstructs by scraping per-point results.

This is also why the postures matter more than they first appear: an optimizer
walking through infeasible iterates would otherwise abort on its own search
path. `collect` is not a convenience — it is the only posture in which
gradient descent through an infeasible region is expressible.

### 2.3b Keywords: only `constraint` can be reserved

Measured against the corpus, not assumed. Occurrences as identifiers across
`headers/`, `examples/`, and `crates/piperine-lang/tests/`:

| Word | Hits | Verdict |
|---|---|---|
| `constraint` | 0 | safe to reserve at the parser level |
| `tol`, `require`, `cover`, `global` | 0 | free today, but see below |
| `target` | **7** | **cannot be reserved** |
| `in` | 55 | already reserved (`part_i_language.md:233`), used by comprehensions — `a in [l, u]` reuses it in a new position |

`target` is used as an ordinary variable in
`crates/piperine-lang/tests/examples/ring_oscillator.phdl:5` and
`oscillator.phdl:16` — `var target : Real = -gain * V(a, gnd);` — and CLAUDE.md
marks `tests/fixtures*` a **frozen corpus**. Reserving it at the parser level
breaks two frozen fixtures for a keyword.

So all five new words are **contextual keywords**, reserved only where the
grammar expects them: `require`/`target`/`var`/`cover` at statement position
inside a `constraint` body, `tol` after a `param` default or a `target`
comparison, `global` after a `tol` clause. PHDL already has this mechanism and
already documents it — the resolve kinds `tri`/`or`/`and`/`sum`/`avg`/`max`/`min`
are contextual inside a `discipline` body (`part_i_language.md:238`). Nothing
new is invented; the precedent is followed.

`constraint` is the one exception worth reserving properly, because it opens a
top-level item and a contextual read there buys nothing.

### 2.4 Scoping: the existing event block selects *when* a constraint holds

A `require`'s applicability uses **the event-block syntax the language already
has** — `@ EventSpec [when (cond)] { … }`, the same production as in `analog`
and `digital` bodies — with the block holding constraint statements instead of
behavior statements:

```phdl
constraint PowerStage {
    @ dc {
        require bias_sane  : V(mid, gnd) > 0.4 && V(mid, gnd) < 1.4;
    }
    @ tran {
        require soa_switch : abs(V(sw.d, sw.s)) <= 40.0;
    }
    @ ac {
        require no_peaking : abs(ac_gain(vout, vin)) <= 4.0;
    }

    // a 5 µs window after the enable crossing
    @ tran(after = cross(V(en, gnd), rise), dur = 5e-6) {
        require settle : abs(V(vout, gnd) - 1.8) <= 0.018;
    }

    // ignore the startup transient
    @ tran(after = 1e-6) {
        require soa_steady : abs(V(sw.d, sw.s)) <= 40.0;
    }

    // composition: holds in both analyses
    @ (dc | tran) {
        require supply_sane : V(vdd, gnd) >= 1.62;
    }
}
```

The design:

- **One event syntax everywhere.** The grammar addition is one production:
  `ConstraintStmt` grows `ScopeBlock`, which *is* the existing `EventBlock`
  with a constraint-statement body. `dc`/`tran`/`ac`/… register as event
  terms in the existing `EventRegistry` — the same registry that already
  resolves `cross`/`above`/`timer` through `headers/operators.phdl`. Blocks
  nest; `when (cond)` level-gates exactly as it does in behavior bodies.
- **`after =` and `dur =` are the whole window language.** `EventTerm` grows
  named arguments, mirroring how `timer(period, phase)` already carries a
  second argument. `after` takes a time (`1e-6`) or an event term
  (`cross(V(en, gnd), rise)`) — the window starts at that time or at the
  first fire; `dur` closes it. A bare event term (`@ cross(x)`) denotes its
  fire instants; an analysis term denotes every accepted point of that
  analysis.
- **Scopes are sets of evaluation points and compose as sets.** `|` is union
  (already the event-OR in the grammar), `&` intersection, `not` complement —
  boolean algebra over windows, in the same `EventSpec` production. A
  statement reduces over its enclosing set: `m_worst = min_t m(t)`.
- **An empty scope is reported, never silently vacuous.** A window that
  selects no points (the crossing never fired, the analysis never ran) is a
  *result* — "not exercised" — surfaced to the host, so a misspelled trigger
  can't masquerade as a pass. This is the coverage instinct applied to
  constraints.
- **A top-level `require` (no enclosing block) holds in every analysis.**
  (Whether that is the right default is open question 2.)

The host gets `m_worst` plus the argmin — time and instance — because "gain
is 3 dB low" and "gain is 3 dB low *at 1.2 µs on instance u3.m1*" differ in
usefulness by an order of magnitude.

### 2.5 Reuse: constraints belong to the model, not to the testbench

The payoff appears when a constraint ships with a *device* rather than with a
circuit. `headers/spice/mos.phdl` grows:

```phdl
constraint Mos1 {
    require vgs_ox : abs(V(g, s)) <= vgs_max;
    require vds_ox : abs(V(d, s)) <= vds_max;
    require bulk_bd: V(b, d) <= 0.3;
    require bulk_bs: V(b, s) <= 0.3;
}
```

with `vgs_max`/`vds_max` as ordinary model-card parameters in `bundle
Mos1Model`. Now **every** MOSFET in **every** design carries its own SOA
rules, and a foundry PDK expressed in PHDL ships enforceable reliability
limits instead of a PDF appendix. A designer who never writes a `constraint`
block still gets SOA checking on every device they instantiate.

**The limits must default to absent, and the reason is not caution.** The
built-in `headers/spice/` models are **ngspice-faithful** — that is their stated
contract, and ngspice has no SOA limits at all. Inventing a `vds_max` for
`Mos1` would (a) fabricate a number no source supports, (b) break the
faithfulness claim the ngspice cross-check suite exists to defend, and (c) with
`strict` as the default posture, trip the entire example gallery on limits its
author never wrote.

So `vgs_max`/`vds_max` default to unset (`Real?`, or `+inf`), and a `require`
reading an unset limit is **inert** — not skipped by posture, but vacuously
satisfied because no limit was declared. A foundry deck that sets them gets
enforcement on every instance; the built-in models stay faithful and the
gallery stays green *by construction*, not by choosing a lenient default well.

The distinction matters for the default posture argument: what should be quiet
by default is an **undeclared limit**, never the check itself. Keep `strict` as
the default; make absence of data mean absence of a claim.

This is the strongest single argument for putting constraints *in the
language* rather than in the host: a constraint attached to a model composes
with hierarchy automatically, and a constraint written in a Python testbench
does not.

**Hierarchical reach: instance ports yes, instance internals no.** Reading a
child instance's *port* net is existing capability, not a new one —
`crates/piperine-lang/tests/examples/sar_adc.phdl:29` already writes
`I(dac.out, gnd) <+ cload * ddt(V(dac.out, gnd));`. So `V(m1.d, m1.s)` in a
parent's constraint block resolves the nets the parent itself connected, and
`constraint Mos1` reading its own `V(d, s)` is plainer still.

An instance's **internal** node is a different matter: `Mos1` creates an
internal drain node when `rd > 0`, and a parent reaching it (`m1.di`) would
cross the encapsulation boundary, depend on whether an optional resistance was
given, and break under monomorphization. That should be a loud error, not a
quiet resolution — and the two cases must be distinguished explicitly, because
"resolve through the authored instance tree" reads as though it covered both.

### 2.6 The validation channel: one ABI hook under everything

A `require` compiled from PHDL is not the only thing that can have something
to say about correctness. Three sources exist, and they must converge:

1. **Constraint kernels** (PHDL `require`s) — margins crossing zero.
2. **Monitor modules** — ordinary `digital` (or mixed) modules evaluating
   sequential properties (the SVA story, §8 item 11).
3. **Plugin/OSDI devices** — model-embedded checks, which is exactly how
   industry foundry decks emit SOA warnings today. A Verilog-A model wrapped
   behind the `Element` ABI has no PHDL constraint block; without an ABI
   channel, its checks are second-class.

The answer is one hook on the `Element` ABI, shaped after the existing
`limiting_report()` precedent (a structured per-step report the solver polls
and acts on):

```
validation_reports() -> Option<ValidationReport {
    findings: [ValidationFinding { severity: Warning | Error,
                                   label, message, value, time, instance_path }]
}>
```

Cross-cutting on `Element` (like `accept_timestep`), not `AnalogDevice`-only —
a digital monitor element reports through the same channel. Polled at the points
§2.3a defines, and in `off` posture not polled at all.

Two shape decisions, both taken from what the tree already does:

- **`Option<Report>`, not `Vec<Finding>`.** The precedent is
  `limiting_report() -> Option<LimitingReport>` (`core/element.rs:173`) — the
  same "structured per-step report the solver polls and acts on" role. `None`
  is the common case and costs nothing to return.
- **Polling is gated by a capability bit, `EMITS_VALIDATION`.** A defaulted
  method that returns `None` still costs a virtual call *per element, per
  accepted point* — on a 10 k-element circuit over 10⁶ timesteps that is 10¹⁰
  calls to learn nothing. `EMITS_NOISE` already establishes the pattern: the
  solver asks only the elements that declare they have something to say.
  `ElementCapabilities` is a `u32` whose highest used bit is
  `NUMERIC_JACOBIAN = 1 << 14`, so there is room.

"Costs nothing when unused" has to mean *not called*, not *returns empty*.

The layering is clean:

> **Margins are values; findings are events.** A `require`'s margin crossing
> zero *generates* a finding (severity `Error`). The margin channel is
> continuous and differentiable — it feeds the optimizer. The finding channel
> is discrete and postural — in `strict`, an `Error` finding fails the
> analysis loud, naming instance, time, and label; in `collect`, it lands on
> the result object (`r.violations`); in `off`, nobody listens.

Consequences that fall out for free:

- **Device self-testing covers every element origin.** A `Mos1` constraint
  block, a monitor FSM, and an OSDI foundry model all report into the same
  channel with the same provenance — SOA checking no longer depends on the
  device being PHDL-compiled.
- **Digital acceptance testing is the same channel.** A monitor module's
  sequential failure is a finding with time and instance, emitted from the
  digital scheduler's accepted points — no `ok`-net indirection, no special
  assertion engine.
- **Warnings become representable.** The `Warning` severity exists in the
  channel whether or not Tier-1 PHDL grows a `warn name : pred;` statement
  kind (a cheap later addition — the channel already carries it, and OSDI
  models will use it from day one).
- **Testers are devices, not a framework.** An ATE-style test program (Tier 3,
  item 15) is an ordinary element that drives nets, wakes on its own
  breakpoints, and reports through this channel — acceptance suites compose
  with hierarchy instead of living in a separate testbench dialect.

### 2.7 Where it does *not* go

| In PHDL | In Python |
|---|---|
| what "correct" and "in-spec" mean for this circuit | stimulus, test orchestration |
| SOA, ERC, spec limits, objectives | which corners to run, how many MC samples |
| anything evaluated per timestep inside the kernel | regression, coverage closure, reporting |

The dividing line is not taste, it is cost: a constraint is evaluated at every
accepted solver point, and crossing FFI per timestep to ask Python whether a
voltage is legal would dominate the simulation. Constraints compile into the
kernel. Everything that runs once per analysis lives in the host.

---

## 3. Gradients: differentiated before the solver, delivered through one ABI bit

The asset that makes Piperine's optimization story different from everyone
else's. Two questions were open here; both now have a proposed answer.

### 3.1 Where derivatives are born: a pre-solver differentiation layer

`resolve/diff.rs` differentiates residual contributions with respect to
*unknowns* to build the Jacobian. Sizing needs derivatives with respect to
*parameters*. Same symbolic pass, different differentiation variable — so the
answer to "do we need a layer before the solver?" is **yes, and it already
half-exists**: at compile time, alongside the residual and Jacobian kernels,
codegen emits a third kernel computing `∂F/∂p` stamps for every differentiable
parameter — where *differentiable* means `Real`-typed and restamp-class, the
same admissibility rule `.sens` already enforces (Part VII §17). A gradient
requested through a rebuild-class or discrete parameter fails loud, exactly as
a `.sens` request does today. This is MD-18-safe by construction — the kernel
is emitted once; an optimizer iteration restamps values, never re-elaborates.

**One kernel with a parameter index, never one kernel per parameter.** This is
not a style preference — it is the lesson of a bug found in this codebase on
2026-07-26 and now open in `ROADMAP.md` P1: the `.disto` 2nd/3rd-derivative
kernels emit one JIT function per *ordered* controlling-branch combination, and
on MOS2/MOS3 that count overruns Cranelift with `TryFromIntError`, making those
devices uncompilable. A `Mos1` model card carries ~60 parameters. A `∂F/∂p`
design that emits a function per parameter walks into the identical wall, with
the identical symptom, one crate over. Emit one kernel taking a parameter index.

The classical consumption method is **adjoint sensitivity** (Director &
Rohrer, 1969): for a scalar output, one extra linear solve yields the gradient
with respect to *all* parameters at once:

```
∂f/∂p  =  −λᵀ (∂F/∂p),      where  Jᵀ λ = ∂f/∂x
```

`J` is the Jacobian the solver already factors. The adjoint solve shape —
transpose, apply an excitation, solve once — is **already in the tree**:
`analyses/noise.rs:240` `solve_adjoint_system` transposes the system and solves
with a unit excitation at the output, per frequency (Part VII §12).

**But that existing adjoint is complex and per-frequency — it is the AC shape,
not the DC one.** This inverts the sequencing the rest of this document assumed.
A DC adjoint needs a *real* transpose solve, which is a sibling code path, not
the one noise already walks. Two consequences:

1. "Reuse the noise solve shape" is accurate for **AC** sensitivity and
   optimistic for **DC**. Say which one is being reused.
2. The metrics a designer actually optimizes — gain, bandwidth, phase margin —
   are AC metrics anyway (§7). If AC adjoint is the closer of the two, a
   DC-only first delivery buys the harder half and postpones the useful half.
   Whether Tier 1 lands DC-first (simpler algebra, `.sens` available as a
   cross-check oracle) or AC-first (closer to existing code, immediately useful
   metrics) is open question 6 — a real fork, not a detail.

The existing `.sens` driver (`analyses/sens.rs:2`: central finite difference
over the restamp path, already refusing `Invalidation::Rebuild`) is the
cross-check oracle either way: a DC adjoint gradient must match it within the
finite difference's own error, which makes DC the easier thing to *verify* even
if it is the further thing to *build*.

### 3.2 The delivery contract: one method, one capability bit

The solver never sees PHDL. It sees elements. So the one thing the ABI must
answer is: *how does an element hand over ∂F/∂p?*

- `AnalogDevice` grows one method: `param_stamps(param_handle, &mut sink)` —
  the element's residual differentiated w.r.t. one of its declared parameters,
  written as ordinary stamps at the current operating point. (`AnalogDevice`
  does exist and is the right home: `abi.rs`, implemented for `PiperineDevice`
  at `device/element.rs:139`. CLAUDE.md's "one ABI: `Element`" summary reads as
  though it were gone.)
- **Stamps go into a caller-provided sink, not a returned `Vec`.** `Stamp`
  already exists (`math/linear.rs:12`, generic over `AsIndex`/`Scalar`), and
  this call sits in the adjoint's inner loop over parameters — returning a fresh
  allocation per parameter per solve is the wrong shape for a hot path when
  `load_dc` already demonstrates the sink pattern.
- `ElementCapabilities` grows `HAS_SENSITIVITY`, alongside the existing
  `HAS_DISTO2 = 1 << 12`/`HAS_DISTO3 = 1 << 13` precedent: disto already proved
  that "this element carries extra derivative kernels" is a
  capability-bit-shaped fact.

For a JIT-compiled PHDL device, the bit is set and the method calls the §3.1
kernel. For a plugin or OSDI device, the bit is clear — and the driver falls
back to **stamp perturbation**: call `load_dc` twice at the *same* operating
point with the parameter perturbed through the ordinary restamp path, and
difference the stamps. That is two load calls per parameter — **no re-solve**
— so the adjoint economy (one Jᵀ solve for all parameters) survives intact;
only the ∂F/∂p term becomes approximate. Absence of the bit degrades accuracy
and speed, never correctness, and never silently: the driver reports which
elements used the fallback.

That is the whole ABI change. One method, one bit, one documented fallback.

### 3.3 Host surface

```python
m = piperine.load("amp.phdl").module("Amplifier")

# DC-defined quantities come off a DC result
r = m.op()
r.margins["headroom"]              # 0.34  — signed, normalized
r.sensitivity("idd", "w1")         # A per metre, one adjoint solve
r.gradient("idd")                  # {"w1": ..., "w2": ..., "ibias": ...}

# AC-defined quantities come off an AC result — reading `gain_db` from `op()`
# is a loud "not defined in this analysis", per §2.1
a = m.ac(piperine.AcConfig(start=1e3, stop=1e9, points=201))
a.margins["gain_db"]               # pointwise metric, per-frequency argmin
a.margins["ugbw"]                  # reduced metric, one value, no argmin (§2.1a)
a.sensitivity("gain_db", "w1")     # needs the AC adjoint (§3.1, open question 6)
```

Each result object serves the analysis that produced it. A metric is readable
where it is defined and loud everywhere else — which is why the two blocks
above cannot be collapsed into one, however much shorter that would read.

The existing `.sens` (central finite difference, two full DC re-solves per
parameter) stays as the always-available baseline; the adjoint path is the
fast exact route for differentiable elements. Same host surface, two engines
underneath — and the difference is reported, not hidden.

### 3.4 Honest difficulties

- **Transient adjoint runs backwards in time** and needs the forward
  trajectory stored (or checkpointed). Memory cost is real and must be a
  design input, not a surprise.
- **`min` over time and over corners is non-smooth.** The worst-case margin
  has a kink where the argmin switches. Options: differentiate at the located
  argmin (correct almost everywhere, standard practice), or use a softmin with
  a declared sharpness. Both are defensible; the choice must be explicit and
  documented, because silently smoothing a constraint changes what the
  optimizer believes.
- **Discrete parameters are not differentiable.** Device multipliers, topology
  choices, and segment counts need a hybrid: gradient inside a discrete shell.
- **Not every metric is differentiable through the solver.** A metric defined
  by event detection (settling time, overshoot instant) has derivative
  information only through the event condition. Fail loud on the ones that
  cannot be differentiated rather than returning a plausible wrong number.

---

## 4. Optimization, centering, Monte Carlo — host-side, on the two additions

Nothing in this section touches the grammar. All of it is Python (and Rust
parity) driving the compiled session. Reached via `Module` or `Session`, these
methods compile once and hold the session internally: a 10³-sample Monte Carlo
is 10³ restamps on one JIT, never 10³ elaborations (MD-18).

```python
mc = m.monte_carlo(n=500, seed=7)    # samples every `tol` in the design
mc.yield_()                          # fraction with all margins >= 0
mc.worst("headroom")                 # the failing sample, reproducible by seed
mc.sigma("gain_db")                  # spread of the metric

c = m.center(over={"w1": (1e-6, 1e-4), "w2": (1e-6, 1e-4)},
             corners=["tt", "ss", "ff"], temp=[-40, 27, 125])
c.wcd                                # worst-case distance, in sigmas
c.params                             # the centered sizing
```

**Design centering, stated precisely:** maximize `min_i (m_i / σ_i)` over the
design parameters, where `m_i` is constraint *i*'s margin and `σ_i` its spread
under the declared `tol` statistics. Equivalently: push the nominal design as
far from every feasibility boundary as the variation-weighted geometry allows.
With adjoint gradients on `m_i`, this becomes a tractable smooth-ish program
instead of a Monte-Carlo-in-the-loop brute force. **This is the single most
valuable thing in this document for a real analog designer**, and it falls out
of the margin convention plus the gradients almost for free.

High-sigma (memory-bitcell territory, 5–6σ) needs importance sampling —
statistical blockade or scaled-sigma sampling — because plain Monte Carlo
needs 10⁸ samples to see a 6σ failure. That is a well-understood family of
methods and belongs in the host, on top of the restamp loop, not in the
language.

---

## 5. What industry does today — and whether this covers it

An honest read of the production AMS verification landscape, so the two
additions are judged against what designers actually use, not against a
caricature.

| Capability | Industry answer | Where this document lands |
|---|---|---|
| Unified A/D simulation | Spectre X / AMS Designer (Cadence), PrimeSim (Synopsys), Symphony Pro (Siemens) — unified kernels or tight co-simulation | Already Piperine's base: one process, one `Element` ABI, A2D/D2A native (§0, asset 3) |
| SOA / reliability checks | Model-embedded checks in foundry Verilog-A decks (`$strobe`-style **warnings**), simulator assert cards; RelXpert / MOSRA (aging: HCI/BTI), Legato | `constraint` on the model (§2.5) — same placement as the foundry deck, but margins are **first-class values**, not print warnings. Aging deferred (Tier 3) |
| Assertions on analog behavior | SVA + Verilog-AMS event checks (`cross`/`above`), PSL; sequential properties live in the digital domain; host-side scoreboards | Scoped `require` windows (§2.4) cover the analog subset; sequential protocol properties are Tier 3 as **monitor modules on the validation channel** (§2.6) — no assertion language |
| Coverage | SV covergroups over real/RNM signals, vManager / MDV closure dashboards | `cover` bins (Tier 3); closure reporting is host work, matching where vManager lives |
| Real-number modeling | `wreal` / SV real vars, DMS ports; the industry's main AMS speedup — with **silent ideality** at every real→electrical boundary | Deferred (Tier 3) but with the declared-impedance bridge as the explicit fix for the industry's known accuracy leak |
| Corners / Monte Carlo orchestration | ADE Assembler / Maestro, PrimeWave | Host sweeps on the compiled session (already shipped: `sweep`/`sweep_grid`) + `monte_carlo` reading `tol` declarations |
| High-sigma yield (3–6σ) | Solido (ML-guided importance sampling — the production reference), scaled-sigma, statistical blockade | Host methods over the restamp loop (§4); no language surface needed |
| Optimization | ADE Optimizer, ASO.ai (AI/black-box), **MunEDA WiCkeD** — the production reference for gradient-based sizing and worst-case-distance centering | §4's `center` *is* WCD centering; the difference is gradients are analytic and free, not finite-differenced |
| Differentiable simulation | **Absent commercially.** Academic differentiable SPICE (JAX-based inverse design) and photonic adjoint design (Lumerical-style) prove the value; no production SPICE exposes it | §3 — the defensible differentiator |

**Verdict.** The two additions cover the industry's core needs — SOA on every
device, spec margins, corner/statistical flow, sizing, centering — with the
same *placement* the industry converged on (checks ship with models;
orchestration lives in the cockpit), but with three upgrades no incumbent can
match without reopening their numerical kernels: (1) one declaration feeds all
three tools, (2) analytic gradients, (3) margins as typed values with
provenance instead of warning text. The gaps against industry — SVA-sequential
assertions, coverage closure tooling, aging, RNM — are real, all host- or
Tier-3-shaped, and none of them blocks the Tier-1 core.

---

## 6. How the two additions map onto the architecture that exists

```
param tol  ──────────────────────────────────────────────►
   parse/ParamDecl grammar  →  POM Param.distribution()   (authored, walkable)
   →  inert at solve; host MC samples → restamp writes (MD-18, nothing new)

constraint block
   │  parse/            new body kind, sibling of `analog`/`digital`
   ▼
POM  Module::constraints           ← authored structure, per the UNBREAKABLE RULE
   │  elab/
   ▼
resolve/  margin expressions, interned ids
   │  resolve/diff.rs differentiates them w.r.t. params (§3.1)
   ▼
flatten/  → emit/  one kernel function per margin, CSE'd with the residual
   │                + the ∂F/∂p param-Jacobian kernel (§3.1)
   ▼
kernel/analog/constraints.rs       new capability sub-struct behind Option
   ▼
device/analog/constraints.rs       evaluated per accepted step, like limits.rs
   ▼
solver  reports margins + argmin (time, instance); HAS_SENSITIVITY gates
        param_stamps (§3.2); validation_reports() (§2.6) is the universal
        findings channel (constraint kernels, monitors, OSDI all emit here)
   ▼
piperine-api  margins/sensitivities on results; optimize/center/monte_carlo
```

Every arrow lands on a file or pattern that already exists. The new capability
sub-struct behind `Option` is exactly how `forces.rs`/`limits.rs`/
`operators.rs`/`events.rs` are already organized — a circuit with no
constraints pays nothing.

**Five existing rules constrain the design, and all five are satisfiable:**

- **POM navigability (UNBREAKABLE).** Constraints and `tol` are authored
  structure and attach to the module/param as written. A hierarchical
  `require` referencing `u3.m1.d` resolves through the authored instance tree,
  never through `flat_modules`.
- **MD-18 (compile once).** Constraints, margin kernels, and ∂F/∂p kernels are
  emitted at compile time. An optimizer iteration changes parameter *values*
  and restamps — it must never re-elaborate. `compile_once_sweep.rs` is the
  guard.
- **MD-24 (declared surface).** New referenceable names get textual
  declarations: `headers/statistics.phdl` for distributions (`gauss`,
  `uniform`), `headers/constraints.phdl` for the margin helpers and scope
  terms (`ac_gain`, `ac_phase_margin`, `dc`/`tran`/`ac` as event terms, …) as
  `extern fn`/`extern operator`. No Rust-side registry of magic names;
  `extern_coverage_guard.rs` extends. Each helper's declaration also carries
  its **class** — pointwise or reduced (§2.1a) — because that decides whether it
  can be lowered into a per-point kernel at all. `abs`/`log10`/`sqrt` are already
  declared (`headers/math.phdl:23-27`), so the sketches type-check today.
  `m1.region` wants a declared `enum Region { Cutoff, Triode, Saturation }` —
  PHDL already has enums in the prelude (`prelude.phdl:13`
  `enum Scale { Lin, Dec, Oct }`), so the region opvar is an ordinary enum-valued
  observable, not integer codes.
- **Fail loud.** A constraint referencing an unknown signal, a
  non-differentiable metric asked for a gradient, a `tol` distribution with
  unknown name: compile or analysis errors, never a silent `0.0`.
- **No-Bloat.** The entire language delta is one clause on `ParamDecl` and one
  body kind. Both were argued in §1 and §2 against the host-side alternative
  and survived.

---

## 7. A complete worked example

One file, one Python driver, all three tools reading one declaration.

```phdl
use piperine::disciplines;
use spice::mos;

mod Ota(input vip : Electrical, input vin : Electrical,
        output vout : Electrical, inout vdd : Electrical, inout gnd : Electrical) {
    param w_in   : Real = 4e-6  tol gauss(sigma_rel = 0.01);
    param w_load : Real = 8e-6  tol gauss(sigma_rel = 0.01);
    param ibias  : Real = 20e-6;
    param l      : Real = 180e-9;

    m1 : Mos1(.d = n1, .g = vip, .s = tail, .b = gnd) { .w = w_in, .l = l };
    m2 : Mos1(.d = vout, .g = vin, .s = tail, .b = gnd) { .w = w_in, .l = l };
    // … load, bias, compensation …
}

constraint Ota {
    // physics — inherited SOA from Mos1's own constraint block also applies
    @ dc {
        require all_saturated : m1.region == saturation && m2.region == saturation;
        require headroom      : V(vout, gnd) >= 0.25 && V(vdd, vout) >= 0.25;
    }
    @ tran {
        require soa_out       : abs(V(vout, gnd)) <= 1.98;
    }

    // pointwise, DC — a per-point kernel (§2.1a)
    var idd     : Real = abs(I(vdd));

    // pointwise, AC — evaluated at each frequency point
    var gain_db : Real = 20.0 * log10(abs(ac_gain(vout, vip)));

    // reduced, AC — properties of the whole sweep, not of any one point
    var ugbw    : Real = ac_unity_gain_freq(vout, vip);
    var pm      : Real = ac_phase_margin(vout, vip);

    target gain_db >= 65.0 tol 3.0;
    target ugbw    >= 20e6 tol 2e6;
    target pm      >= 60.0 tol 5.0;
}
```

```python
import piperine

ota = piperine.load("ota.phdl").module("Ota")

# 1 ─ verification: strict posture. DC requires and DC vars off the DC result.
r = ota.op()
assert r.requires_ok, r.first_violation      # instance, value, margin
print(r.margins["headroom"], r.margins["idd"])

# AC targets are read where they are defined — `r.margins["gain_db"]` would be
# a loud "not defined in this analysis" (§2.1).
a = ota.ac(piperine.AcConfig(start=1e3, stop=1e9, points=201))
print(a.margins["gain_db"], a.margins["ugbw"], a.margins["pm"])

# 2 ─ optimization: gradients, not guesses (constraints run in collect posture)
opt = ota.optimize(
    objective="idd",            # a DC var — differentiable through the DC adjoint
    over={"w_in": (1e-6, 40e-6), "w_load": (1e-6, 40e-6), "ibias": (5e-6, 200e-6)},
    method="gradient",          # adjoint sensitivities from the compiler
    corners=["tt", "ss", "ff"],
)
print(opt.params, opt.iterations)            # ~10^1-10^2 sims, not 10^3-10^5

# The AC targets constrain that search. Gradients through them need the AC
# adjoint (§3.1) — with DC-only sensitivity they are constraints the optimizer
# can *evaluate* but not differentiate, so it finite-differences those or
# treats them as feasibility filters. This is exactly what open question 6 decides.

# 3 ─ centering: push away from every boundary, in sigmas
c = ota.center(over=opt.bounds, corners=["tt", "ss", "ff"], temp=[-40, 27, 125])
print(f"worst-case distance: {c.wcd:.2f} sigma")

# 4 ─ confirm with statistics; the failing sample is reproducible
mc = ota.monte_carlo(n=1000, seed=7, params=c.params)
print(f"yield {mc.yield_():.4f}   worst: {mc.worst('headroom')}")
```

One `constraint` block, two `tol` clauses. Verified, optimized, centered, and
yield-confirmed, with no restatement of intent anywhere in the flow.

The example is deliberately honest about the seam it sits on: an OTA's
*specification* is mostly AC, its cheapest *gradient* is DC, and the two
classes of `var` (§2.1a) are not interchangeable. Any version of this snippet
that reads `gain_db` off `op()` is describing a language that cannot be built.

---

## 8. Sequencing

Ordered by value ÷ effort, with the dependency structure made explicit.

**Tier 1 — the foundation (V1 candidate)**

1. `tol` on `param`: grammar clause, POM `Param.distribution()`,
   `headers/statistics.phdl`. Inert at solve; readable by the host.
2. `constraint` block: grammar, POM, `require`/`var`/`target`,
   margin lowering, the three postures. No temporal windows yet.
3. Margin evaluation in the kernel (`kernel/analog/constraints.rs` +
   `device/analog/constraints.rs`) + the `validation_reports()` ABI hook,
   with instance and time provenance.
4. `constraint` blocks on the `headers/spice/` models → **SOA on every device
   in every design, for free.** Highest industrial value per line of code here.
   (This pulls the operating-region opvar — `m1.region` — forward into the same
   headers work; an introspection addition, not a language change.)
5. The ∂F/∂p kernel (**one kernel, parameter-indexed** — §3.1's `.disto`
   lesson) + `param_stamps` into a caller sink + `HAS_SENSITIVITY`; one adjoint
   sensitivity driver; stamp-perturbation fallback for elements without the bit.
   Whether that driver is DC or AC first is open question 6 — the noise adjoint
   this reuses is the AC shape, not the DC one.

**Tier 2 — optimization and centering (V1.5)**

6. Host optimizer driving the restamp loop, gradient-based, with a black-box
   fallback (BO/CMA-ES) for the non-differentiable parts.
7. Monte Carlo with reproducible seeds over `tol` declarations.
8. Design centering (`min_i m_i/σ_i`), the natural consequence of 1 + 5 + 7.
9. Scoped `require` windows (event blocks in constraint bodies, `after =`/
   `dur =` event-term args, the `|`/`&`/`not` window algebra), reusing
   `EventRegistry` and the existing `EventBlock` production.

**Tier 3 — verification scale (V2)**

Every Tier-3 item below is anchored in the margin machinery, and the audit of
what each one *actually* costs in language surface is the surprising part:
across the whole vision, the total grammar delta is the `tol` clause, the
`constraint` block, and — maybe — one body kind.

10. **Operating-region coverage.** One new statement kind inside the
    `constraint` block, not a new construct:
    `cover input_cm : V(vip, gnd) bins [0.4:0.1:1.4];` and
    `cover region_m1 : m1.region in {cutoff, triode, saturation};`
    Kernel side it is *the same per-point expression evaluation as a margin*
    plus a bin-mapper — no new machinery. Cross coverage (`cover cross
    temp_x_region`) is a pair of expressions binned jointly. Accumulation,
    merging across runs/seeds, and closure reporting ("which bins are empty,
    which stimulus would fill them") are host state and host work — the
    vManager role, living where vManager lives: outside the language. The
    operating-region opvar (`m1.region`) it samples already landed with
    Tier 1 item 4.
11. **Sequential (SVA-shaped) properties = monitor modules on the validation
    channel.** Do not import SVA. A sequential property is a small ordinary
    `digital` monitor module — registers, `match` on state — that reports
    failures through `validation_reports()` (§2.6): a finding with time and
    instance provenance, emitted from the digital scheduler's accepted points.
    No `ok`-net plumbing, no assertion language, no special engine — PHDL's
    `digital` grammar already expresses FSMs, hierarchy already composes them,
    and the validation channel already turns a detection into a typed failure.
    A dedicated sequence syntax (`##`-style) is admissible later **only** if
    real usage shows monitor modules are too verbose — evidence first, syntax
    second.
12. **RNM: cheaper than it looks, because the grammar slot already exists.**
    Verified: `docs/spec/part_i_language.md:477` carries the production
    `ResolveDecl ::= "resolve" ("tri"|"or"|"and"|"sum"|"avg"|"max"|"min") ";"`,
    and `:484` assigns `sum`/`avg`/`max`/`min` specifically to `Real` storage
    nets. Those kinds are *already* contextual keywords (`:238`). The resolution
    vocabulary is in the spec and unimplemented — so this item is finishing
    declared language, not growing it. Three pieces: (a) implement those declared resolve kinds in
    the digital/event kernel; (b) an event-scheduled body for all-storage-real
    modules (inferred, or an explicit `behavioral` kind — still the one open
    grammar question, see §9); (c) **bridging needs no keyword at all**: the
    declared-impedance bridge is an ordinary module whose `analog` body states
    the impedance in code —
    `I(out, gnd) <- (V(out, gnd) - code * lsb) / rout;` *is* the declaration.
    What the industry hides inside wreal, PHDL writes as one explicit line.
13. **High-sigma importance sampling** (statistical blockade, scaled-sigma):
    host methods over the restamp loop. No language surface.
14. **Aging/reliability (HCI/NBTI):** host-side parameter drift over a
    declared lifetime; the model ships aging coefficients as ordinary params
    and the host restamps them along a stress curve. No language surface —
    the `tol` clause already proved "variation metadata on the parameter" is
    the right shape if a future drift clause is ever wanted.
15. **Tester (ATE-style) devices.** A library over the `Element` ABI for
    imperative test-programs-as-devices: declare ports, then sequence
    `advance_clock(dt)` (breakpoints/`timer` — the device declares its own
    wake-up times), `read_port` (`EvalCtx`), `write_port` (`EventSink` —
    digital drive never touches the MNA by construction), `read_voltage`
    (`SAMPLES_ANALOG`), and `warn(...)`/`fail(...)` (`validation_reports`,
    §2.6 — the third consumer of the channel, after constraint kernels and
    monitor modules). Analog drive uses declared-impedance stamps or
    storage-real nets — never a silent ideal source. Acceptance testers for
    protocol specs (USB-phy-style suites), protocol checkers, and
    stimulus+check pods ship as ordinary devices that compose with hierarchy
    and run under the same posture rules as everything else. No language
    surface: a plugin/host library.

**Explicitly not planned:** analog formal/reachability (research-scale, does
not survive contact with a real netlist), RL/GNN sizing (dominated by the
gradient path for continuous sizing; revisit only for topology choices).

The dependency spine is short: **`tol` (1) and margins (2–3) unlock SOA (4);
margins plus gradients (5) unlock optimization (6); those plus sampling (7)
unlock centering (8).** Item 4 is the cheapest real win and item 8 is the
biggest one, and both stand on the same convention from §2.2.

---

## 9. Open questions

An expertise pass on 2026-07-27 closed five earlier inconsistencies in place
(they are now design text, not questions): the two classes of measured quantity
(§2.1a), which points a margin is evaluated at (§2.3a), contextual keywords
against the frozen corpus (§2.3b), SOA limits defaulting to absent (§2.5), and
the validation channel's capability bit and `Option` shape (§2.6). What follows
is what genuinely still needs a decision.

1. **Non-smooth worst-case: argmin differentiation or declared softmin?** Must
   be explicit either way — silently smoothing a constraint changes what the
   optimizer believes about feasibility.
2. **Do `require`s hold in *every* analysis by default, or only where
   scoped?** Default-on is safer and noisier; default-off is quieter and lets a
   real violation hide. §2.3a removes the worst false-positive sources
   (homotopy stages, rejected steps, UIC at `t = 0`), which makes default-on
   more livable than it first looked — but the scoping default is still a call.
3. **Are margins result rows or their own channel?** Margins are per-point
   scalars with provenance, so a `MarginsResult` beside the nine host types is
   the leading sketch. §2.1a adds a wrinkle: pointwise and reduced margins have
   *different* shapes (argmin vs no argmin), so whatever carries them must
   represent both without faking a `t = 0` for the reduced ones.
4. **How do constraints compose through monomorphization?** A `require` on
   `Mos1` must instantiate per device instance, and `urc__5`-style variants
   must not multiply the *declaration*. The UNBREAKABLE RULE says the authored
   form stays walkable; the evaluation form is a codegen side artifact. The
   sketch is clear; the POM representation is not yet designed.
5. **RNM bodies: explicit `behavioral` kind, or inferred from all-storage-real
   nets?** (Tier 3, item 12b.) Explicit makes a 100–1000× performance cliff
   visible in source; inferred is friendlier and less grammar. This is the
   only remaining place in the whole vision where the grammar might still
   grow.
6. **DC adjoint first, or AC adjoint first?** (New — §3.1.) The adjoint machinery
   that already exists (`analyses/noise.rs:240`) is complex and per-frequency:
   it is the **AC** shape. DC needs a real transpose solve, a sibling path. But
   DC has `.sens` (`analyses/sens.rs:2`) as a finite-difference oracle to
   verify against, and AC does not. So: build the harder-to-reach half first
   because it is verifiable, or the closer half first because gain/bandwidth/
   phase-margin are what designers actually optimize? This changes what Tier 1
   delivers and is the sharpest remaining fork.
7. **Does a reduced metric belong in the kernel path at all?** (New — §2.1a.)
   A sweep-reduced quantity could be computed in the solver as a post-analysis
   pass, or handed to the host as raw points plus a declared reduction. The
   host route is simpler and keeps the kernel pointwise-only; the solver route
   keeps the margin surface uniform and makes reduced margins available to a
   Rust host without reimplementation. Related to question 3.

---

## 10. Why this is worth doing

The honest competitive read: commercial AMS tools have better device models,
better layout integration, and decades of foundry qualification. Piperine will
not win on model breadth.

What they cannot easily do is make the simulator *differentiable* and make
"what correct means" a **single first-class declaration** shared by
verification, optimization, and centering. Their simulators are closed
numerical kernels with optimizer cockpits bolted on; the gradient information
is not there to expose. Piperine's compiler already computes symbolic
derivatives as a matter of course.

That is a narrow but real advantage, and §2.2's margin convention is what
turns it from a numerical curiosity into a design methodology. Two grammar
additions buy it. Everything else is engineering we already know how to do.

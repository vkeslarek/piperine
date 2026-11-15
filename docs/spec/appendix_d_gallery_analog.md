# Appendix D. Gallery: sources, clocks and continuous time

This appendix is **informative**. It adds no rule. Each model is traced against the normative
parts, and the trace is the content: where the time points land, which instants are jumps, and
what every analysis sees.

**Layout of each entry.** Purpose, code, then a *Semantics walkthrough* with these rows where
they apply: drives, events, the operating point and `(0, ·)`, transient instants, time points,
`Dc`, `Ac`, `Noise`, unknown values.

**Models referenced from the normative parts.**

| Model | Defined in | Declaration assumed here |
| --- | --- | --- |
| `Resistor` | §8 | `module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; tc1: f64 = 0.0; tnom: f64 = 300.15 where tnom > 0.0; }` |
| `Capacitor` | §11 | `module Capacitor(p: Electrical, n: Electrical) { c: f64; }` with `i(p, n) +> ddt(c * v(p, n));` |
| `Switch` | §11 | `module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}` |
| `idtmod`, `above`, `transition`, `normal` | §22.1, §22.2 | as written there |

> **Resolution.** §11 gives `Capacitor` an `analog` block and no `module` declaration. The
> galleries assume the declaration in the table: two `Electrical` ports and one required param `c`.

**Guarded instants.** By §17.2.2 a reaction counts at a scheduled time only if its event fires
at `(t, 0)`. A `clk` commit to low cannot fire `clk.rise()`, so a falling edge that reaches
`analog` only through a `rise()` reaction is not a time point. Claims that rest on this are
marked "(guarded instant)".

## D.1 Piecewise-linear source

**Purpose.** A potential source that follows straight segments through `N` points. Each corner
is a carrier change that `analog` reads, so the solver lands on it (§17.3).

```rust
module Pwl<const N: usize>(p: Electrical, n: Electrical) where N >= 1 {
    points: [(f64, f64); N];          // (time, value), times strictly increasing
    signal seg: Bit<u32> = 0;
}

digital<N> for Pwl<N> {
    for k in 1..N { on at(points[k].0) { seg <- k as u32; } }
}

analog<N> for Pwl<N> {
    let k = seg.known_or(0) as usize;
    let y = if k + 1 >= N { points[N - 1].1 } else {
        let ((t0, y0), (t1, y1)) = (points[k], points[k + 1]);
        y0 + (y1 - y0) * ((now() - t0) / (t1 - t0)).clamp(0.0, 1.0)
    };
    v(p, n) +> y;
}
```

> **Resolution.** `where N >= 1` is added (§8.1, const-generic constraint). With `N == 0` the
> expression `points[N - 1]` has no meaning, and the source model left that case unstated.

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | `Pwl` holds the one drive of `seg` (§5.2): its initial value and the commits of `N − 1` generated reactions. Branch `(p, n)` gets a potential contribution in every solve (§11 rule 2). |
| Structure | `N` is structural. `points` is not (§8.1.2): generation reads `N` only, and `points[k].0` is an event argument. The host may `set` `points` during a run. |
| Events | `N − 1` instances of `at`, one per unrolled reaction (§8.3.4). Each fires once at its tick (§13 rule 4, §15). |
| `(0, 0)` | `seg ← 0` is committed and `seg.changed()` fires (§13 rule 6). No reaction listens. |
| Operating point | `now()` is 0 (§16.2). With `points[0].0 == 0`, `y = points[0].1`. A first time above 0 clamps to the same value. |
| Corner `t_k` | `(t_k, 0)`: `at` fires, the reaction commits `seg`. `(t_k, 1)`: `seg ← k` applies, `analog` reads it, and the jump is solved (§15 step 3.4). |
| The jump | Left value with `seg = k − 1`: the clamp is 1, so `y = y_k`. Right value with `seg = k`: the clamp is 0, so `y = y_k`. The source is continuous; only its slope changes. Tick rounding (§15) can leave a difference of at most the slope times half a tick, which the jump absorbs. |
| After the last corner | `seg = N − 1`, so `k + 1 >= N` and `y` stays at `points[N − 1].1`. |
| Time points | Every corner (§17.2.2: its reaction changes a carrier that `analog` reads). No other instant. |
| `Dc` | `at` never fires (§16.1.3). `seg` stays 0 and `now()` is 0: the source holds its value at time 0. |
| `Ac` | The contribution has no dependence on a probe and no `ac_stim`. The branch is a potential source of zero small-signal value: a short. |
| `Noise` | No `noise` instance: no contribution. |
| Unknown | `seg` is always known. §3.3.5 still requires `known_or` (G.29). |
| Host `set` of `points` | A jump (§9). Each `at` is re-armed; a corner moved to a time already past does not fire again (§13 rule 4). |
| Failure modes | Two equal times make `t1 − t0` zero. The resulting non-finite value fails the solve (§17.1). |

## D.2 Clock with jitter

**Purpose.** A free-running clock whose half periods carry Gaussian jitter. It is digital only;
its cost to the solver depends on who reads `clk`.

```rust
module ClockGen(out clk: Bit = false) {
    period: f64 where period > 0.0;
    jitter: f64 = 0.0 where jitter < period / 10.0;    // keeps every half period positive in practice
    seed: u64 = 1;
    signal rng: Bit<u64> = seed;
}

digital for ClockGen {
    on clk.changed() {                                  // the first time at (0, 0): its initial value
        let (j, next) = normal(rng, 0.0, jitter);
        rng <- next;
        clk <- !clk after period / 2.0 + j;
    }
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | `ClockGen` drives `clk` (its `out` port) and `rng`, each by commits only (§5.2). |
| `(0, 0)` | `clk ← false` and `rng ← seed` are committed. Both are first commits, so `clk.changed()` fires (§13 rule 6). The reaction reads `clk = false` and the seed (§12.1.1). It commits `rng` for `(0, 1)` and `clk ← true` for `period / 2 + j`. |
| `(0, 1)` | `rng` applies. `rng.changed()` fires and nothing listens. The instant goes quiet. |
| Each edge `t_e` | `(t_e, 0)`: the delayed commit applies and `clk.changed()` fires. The reaction draws the next `j` and schedules the opposite level. `(t_e, 1)`: `rng` applies. |
| Transport | One statement schedules every edge. Each new time is later than the pending one, so nothing is removed (§12.1.4). |
| Tick rounding | Each edge is rounded to the nearest tick (§15). Jitter below the time resolution is lost. |
| Negative delay | A draw with `period / 2 + j <= 0` is a next-microstep commit (§12.1). The `where` bound makes that a five-sigma event; it does not exclude it. A negative `jitter` passes the bound and is the author's error. |
| Time points | None of its own. An edge is a time point only when a reaction or `analog` block elsewhere depends on `clk` (§17.2.2). |
| `Dc`, operating point of `Ac`/`Noise` | The reaction runs once at `(0, 0)`. Its delayed commit never comes due and is dropped when the analysis ends (§16.1.3). `clk` stays low. |
| `Dc` sweep | Initial values are committed at the first point only (§16.1.2). The reaction does not run again. |
| Small-signal sweep | No instants (§16.2). The module has no `analog` block and contributes nothing. |
| Continued transient | The pending edge stays in the queue (§16.4.1). The clock keeps ticking with the same `rng` sequence. |
| Unknown | `clk` and `rng` are always known. The delay is known, so no commit is `Unknown` (§3.3.3). |
| Gradients | Edge times move with `period` by their own derivative (§23.3.3). |

## D.3 Voltage-controlled oscillator

**Purpose.** A VCO whose phase is an integral wrapped to one cycle. The edges are two rising
crossings half a cycle apart, never the wrap itself.

```rust
module Vco(vctl: Electrical, out clk: Bit = false) {
    f0: f64 = 1G;
    kvco: f64 = 100M;
    let ph = idtmod(f0 + kvco * v(vctl), 0.0, 1.0, -0.5);    // phase in [-0.5, 0.5)
}

digital for Vco {
    on above(ph + 0.25) { clk <- true; }      // above: ph starts at 0, already inside the high half
    on cross(ph - 0.25) { clk <- false; }
}

analog for Vco { bound_step(0.05 / f0); }
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Instances | `ph` names one `idtmod` instance, placed by the body `let` (§8.2, §8.3.3). Inside it: one `idt` on a `State` node and two `cross` instances. `Vco` adds one `above` and one `cross`. |
| Branches | `Vco` contributes to no branch. `vctl` is probed only, so the module draws no flow from it (§11 rule 4). |
| Arguments | `ic = 0.0`, `modulus = 1.0`, `offset = −0.5` (§3.2). Then `lo = −0.5` and `start = −0.5 + 0.5.rem_euclid(1.0) = 0.0`. |
| Operating point | `idt` has an `ic` and `analysis(Static)` holds, so its node is held at `base.known_or(start) = 0.0` (§8.3). `ph = 0`. |
| `(0, 0)` | `clk ← false` is committed. `above(ph + 0.25)` fires through its `init() && e > 0.0` arm, since `0.25 > 0` (§22.2). `cross(ph − 0.25)` cannot fire on a first solution (§16). The reaction commits `clk <- true`. |
| `(0, 1)` | `clk ← true` applies. `clk.rise()` fires for readers of `clk`. |
| Rising edge | When `ph` rises through −0.25 the `cross` inside `above` is located (§17.3). The instant is rounded up to a tick and is a time point. |
| Falling edge | When `ph` rises through +0.25, `cross(ph − 0.25)` is located the same way. |
| Wrap, microstep `n` | `ph` rises through 0.5: `cross(acc − lo − modulus)` fires. The reaction commits `base <- acc − 1.0` and `seat <- true`. |
| Wrap, `n + 1` | Both apply. `idt` reads `reset` true and forces its node to `base`: a jump forced by a potential constraint (§15.2.2). `seat.rise()` fires and commits `seat <- false`. |
| Wrap, `n + 2` | `seat` falls. A second jump returns `idt` to its integrating row, with the state continuous at `base` (§15.2.1). |
| Edges at the wrap | `ph + 0.25` falls from 0.75 to about −0.25, and `ph − 0.25` falls too. `cross` is compared across the jump (§13 rule 5) and fires on rises only. Neither edge reaction runs. |
| Re-seat margin | The crossing is rounded up, so `acc > 0.5` there (§15). The seated value is above `lo`, so `cross(lo − acc)` does not fire in that instant. |
| Time points | Three located crossings per cycle: two edges and the wrap. Only the wrap is a jump (§17.2 table). `bound_step` caps every step at a twentieth of the nominal period. |
| `Dc` | Same as the operating point: `ph = 0`, `clk` goes high at `(0, 1)`. `bound_step` does nothing (§16.2). |
| `Ac` | Under the sweep's flags `idt` is `1/jω` (§11.1): `ph = kvco · v(vctl) / jω`. `clk` is a discrete carrier and has no response. The edge-time modulation is reached only by a periodic analysis (§16.3, G.30). |
| `Noise` | No `noise` instance. |
| Unknown | `base` starts unknown and is read through `known_or(start)`; `seat` through `known_or(false)` (§3.3.5). |
| Gradients | Each edge time moves as a located event: `dt*/dp = −(∂e/∂p)/(de/dt)` (§23.3.2). |

**Operating range.** The trace assumes `f0 + kvco · v(vctl) > 0`. With a negative frequency the
phase falls and wraps under the bottom, from −0.5 to about +0.5. Both edge expressions then
rise across that jump and both reactions run in one microstep. `clk <- false` is later in source
order and wins (§12.1.2), so `clk` stays low. A model that must run backwards writes its edges
for both directions.

## D.4 DAC with finite edges

**Purpose.** A code-to-voltage source whose output moves along straight edges. `transition`
(§22.2) ramps a piecewise-constant value; a code change in mid-ramp starts from the current
output.

```rust
module DacRamp(in code: Bit<u8>, vout: Electrical) {
    vref: f64 = 1.0; tedge: f64 = 100p;
    let ramp = transition(vref * (code as f64) / 256.0, 0.0, tedge, tedge);
}
analog for DacRamp { v(vout) +> ramp; }
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| The first argument | `x` is an `in` port of `transition`. The expression bound to it is an anonymous definition (§8.1): discrete, recomputed when `code` changes, and outside §3.3.5. It reads no probe (§8.2.5). |
| The other arguments | `td`, `tr`, `tf` are value inputs of type `Option<f64>`; each `f64` argument becomes `Some` (§3.2). |
| Drives | The anonymous definition is the parent's drive of `x`. `transition` drives its own `xd`, `y0`, `y1`, `t0`, `dt`. `DacRamp` contributes potential to `(vout, gnd)`. |
| `(0, 0)`, `code` known | `x` settles to `vref · code / 256`; `x.changed()` fires (§13 rule 6). `xd = x after 0` makes its first commit for `(0, 1)` (§8.2.4). The reaction `on x.changed() && analysis(Static)` commits `y0 <- x; y1 <- x`. |
| Operating point | The result is `x.known_or(0.0)` while `analysis(Static)` holds. The output equals the code's level, with no ramp. |
| `(0, 1)` | `xd`, `y0`, `y1` apply. `xd.changed()` fires, but its reaction is guarded by `!analysis(Static)` and does not run. `analog` reads carriers that changed, so the equilibrium is solved again and gives the same answer (§16 step 4). |
| First step | `analysis(Static)` is false and `dt = 0`, so the result is `y1`, which equals `x`. The release is not a discontinuity. |
| Code change at `(t, n)` | `x` is recomputed in microstep `n` (§8.2.1). `xd` follows at `n + 1`. Its reaction commits `y0 <- y`, `y1 <- to`, `t0 <- now()`, `dt <- tr` or `tf`, read at that instant (§8.2, `let`). |
| Near corner, `(t, n + 2)` | The four carriers apply; `analog` reads them: a jump. With `now() == t0` the clamp is 0 and the result is `y0`, the old output. The value is continuous. |
| Far corner, `t + tedge` | `at(t0 + dt)` fires and commits `dt <- 0.0`. One microstep later the result is `y1`. The clamp was 1, so the value is continuous again. |
| Mid-ramp change | The reaction reads the current ramp value as `y`, so the new edge leaves from there. `t0` changes, which re-arms `at` to the new far corner (§13 rule 4). |
| Equal levels | A change to the level already held gives `dt <- 0.0`: no ramp and no far corner. |
| Time points | Both corners of every edge (§17.2.2). The ramp between them is a function of `now()` with no corner (§17.3). |
| `Dc` sweep | Each code change runs `on x.changed() && analysis(Static)`. The output follows the code at every point (§16.1). |
| `Ac` | The sweep runs with `analysis(Static)` false and `dt = 0`: the result is the constant `y1`. The branch is a potential source of zero small-signal value. |
| `Noise` | No `noise` instance. |
| Unknown `code`, transient | `code as f64` is all-or-nothing, so `x` is unknown (§3.3.2). `xd.known()` is `None` and the reaction commits nothing: the output holds. |
| Unknown `code`, equilibrium | The result is `x.known_or(0.0)`, so the output is 0.0 in that solve, whatever was held before. A code that was never known gives 0.0 in every solve. |

## D.5 Sampling at a clock edge: delta-sigma modulator

**Purpose.** A first-order modulator: a continuous-time integrator, a one-bit quantizer sampled
by a reaction, and feedback through a discrete carrier.

```rust
module DeltaSigma1(vin: Electrical, in clk: Bit, out bit: Bit = false) {
    r: f64 = 100k; c: f64 = 1p; vref: f64 = 1.0;
    signal x: Electrical = 0.0;
}

analog for DeltaSigma1 {
    let fb = if bit.high() { vref } else { -vref };
    i(x) +> ddt(c * v(x)) - (v(vin) - fb) / r;
}

digital for DeltaSigma1 {
    on clk.rise() { bit <- v(x) > 0.0; }
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | One branch, `(x, gnd)`, with a flow contribution. `DeltaSigma1` drives `bit` by commits. `vin` is probed only and carries no flow of this module. |
| The equation | `x` has this one branch, so KCL gives `c · dv(x)/dt = (v(vin) − fb) / r`. |
| No DC path | In an equilibrium solve `ddt` is 0 (§16.2). The row of `x` then does not contain `v(x)`. |
| Operating point of `Tran` | `x` has `= 0.0`, so it is held at 0 V by a source to `gnd` (§16). The hold's flow balances `(v(vin) − fb) / r`. `fb = −vref`, since `bit` starts false. |
| `(0, 0)` | `bit ← false` is committed. If `clk` starts high, `clk.rise()` fires (§13 rule 6): the reaction reads `v(x) = 0` and commits `false`, which changes nothing. |
| First step | The hold is removed. `c · v(x)` is a `ddt` state, so the step starts from 0 V with no jump (§16, §15.2.1). |
| Rising edge, `(t, 0)` | `t` is a time point: the reaction probes (§12.1.5, §17.2.2). The left value is solved first (§15 step 1). The reaction reads it and commits `bit`. |
| Rising edge, `(t, 1)` | If `bit` changed, `analog` reads the change: a jump (§15 step 3.4). `c · v(x)` stays (§15.2.1). The flow term changes by `2 · vref / r`. |
| Restart | The state's derivative differs across the jump, so the integrator restarts for it (§15.2.6). |
| Unchanged `bit` | A commit of the value already held changes nothing. No jump; the instant is still a time point. |
| Falling edge | No reaction fires and `analog` does not read `clk`. Not a time point (guarded instant). |
| `Dc` | Nothing is held outside a transient's operating point. The solve is singular and names `x` (§16). |
| `Ac`, `Noise` | Their operating point is an equilibrium solve with nothing held: they fail the same way. This also holds when run at a transient's last instant (§16.4.2). The loop's response needs a periodic analysis (§16.3, G.30). |
| Unknown | The probe is known, so `bit` is always known. `bit.high()` satisfies §3.3.5. |
| Gradients | `bit` is not real: it carries no sensitivity and acts through event times only (§23.3.4). |

## D.6 Switched-capacitor integrator

**Purpose.** A stray-insensitive integrator built from structure only: a two-phase clock made of
two delayed definitions, four resistive switches, two capacitors and an amplifier.

```rust
module RSwitch(a: Electrical, b: Electrical, in ctl: Bit) { ron: f64 = 100.0; roff: f64 = 1G; }
analog for RSwitch { i(a, b) +> v(a, b) / if ctl.high() { ron } else { roff }; }

module OpAmpIdeal(inp: Electrical, inn: Electrical, vout: Electrical) { a0: f64 = 1e5; }
analog for OpAmpIdeal { v(vout) +> a0 * v(inp, inn); }

/// Non-overlapping two-phase clock: pure structure.
module TwoPhase(in clk: Bit, out phi1: Bit = false, out phi2: Bit = false) {
    gap: f64 = 50p;
    phi1 = clk after if clk { gap } else { 0.0 };      // rises gap after clk, falls with it
    phi2 = !clk after if clk { 0.0 } else { gap };     // the mirror
}

/// Stray-insensitive SC integrator: structure only.
module ScIntegrator(vin: Electrical, vout: Electrical, in clk: Bit) {
    cs: f64 = 1p; cf: f64 = 4p;
    signal phi1: Bit;
    signal phi2: Bit;
    signal x: Electrical;
    signal y: Electrical;
    signal vg: Electrical;
    let ck = TwoPhase(clk, phi1, phi2);
    let s1 = RSwitch(vin, x, phi1);
    let s2 = RSwitch(x, gnd, phi2);
    let s3 = RSwitch(y, gnd, phi1);
    let s4 = RSwitch(y, vg, phi2);
    let csmp = Capacitor(x, y) { c: cs };
    let cfb = Capacitor(vg, vout) { c: cf };
    let amp = OpAmpIdeal(gnd, vg, vout);
}
```

> **Resolution.** `ron` is written `100.0`. The source model had the integer literal `100` as
> the default of an `f64` param. §3.2.3 allows one implicit conversion, `T` to `Option<T>`, so
> an integer literal does not become an `f64`.

**Semantics walkthrough: `TwoPhase`.**

| Aspect | Trace |
| --- | --- |
| Drives | Each `out` port has one drive: a delayed definition. Its `= false` is the first commit of that drive (§5.2). `ScIntegrator` declares `phi1` and `phi2` with no initial value; the driver supplies it once. |
| Meaning | Each definition is a commit made whenever its expression changes, first at `(0, 0)` (§8.2.4). The delay is read with the new value. |
| `(0, 0)`, `clk` low | `phi1 <- false after 0` applies at `(0, 1)` and changes nothing. `phi2 <- true after gap` is scheduled. |
| `clk` rises at `t` | `phi2 <- false after 0` applies at `(t, 1)`. `phi1 <- true after gap` applies at `t + gap`. |
| `clk` falls at `t'` | `phi1 <- false after 0` applies at `(t', 1)`. `phi2 <- true after gap` applies at `t' + gap`. |
| Non-overlap | Both phases are low for `gap` after every `clk` edge. They are never high together. |
| Short pulse | If `clk` falls before `gap` has elapsed, the fall is made for now. It removes the pending rise, which the same statement made (§12.1.4). `phi1` never sticks high. |
| Where time does not advance | The `after gap` commits never come due (§16.1.3). With `clk` low at the first point, both phases stay low through the analysis (G.32). |
| Unknown `clk` | The delay is unknown, so the statement commits `Unknown` at the next microstep (§3.3.3). |

**Semantics walkthrough: `ScIntegrator`.**

| Aspect | Trace |
| --- | --- |
| Branches | Every branch belongs to a child. `OpAmpIdeal` holds the only potential branch, `(vout, gnd)`. `gnd` is passed as a port connection (§6 rule 1). |
| Operating point | Both phases are low, so the four switches are `roff`; the capacitors carry no flow. `y` and `vg` reach `gnd` through `s3` and `s4`: 0 V. `vout = −a0 · v(vg) = 0`. `x` sits at `v(vin) / 2` between `s1` and `s2`. No node floats, which is what a finite `roff` buys. |
| Sampling phase | `phi1` high: `x` follows `vin` and `y` is grounded. `cs` charges to `v(vin)` with time constant `ron · cs` per switch, 0.1 ns. |
| Transfer phase | `phi2` high: `x` is grounded and `y` joins the virtual ground `vg`. The charge `cs · v(vin)` moves onto `cf`. |
| Result per period | Charge at the `vg` cut is conserved: `vout[n] = vout[n − 1] + (cs / cf) · vin[n]`. The integrator is non-inverting with gain 0.25. |
| Time points | Four per period, one at each phase edge (§17.2.2). Each is a jump: an `RSwitch` conductance read by `analog` changes. |
| The jumps | A resistive switch forces nothing. Both capacitor states are continuous (§15.2.1) and nothing is forced. Only the flows step. |
| `Dc` | Solvable, by the operating point above. It is the all-switches-off circuit, not the sampled system. |
| `Ac` | Linearized with both phases low (§11.1). The sampled transfer function needs a periodic analysis (§16.3). |
| `Noise` | `RSwitch` and `OpAmpIdeal` place no `noise` instance. |
| Unknown | A phase that is unknown reads as low through `high()` (§22.1): the switch is off. |

**Variant with ideal switches.** With `Switch` (§11) in place of `RSwitch`, each phase edge is a
jump in which a potential constraint forces capacitor states to move, and charge is conserved
over each cut (§15.2.2, F.5). That variant has no finite off-resistance. With both phases low,
`x` has no DC path, so the variant needs `= value` on its internal nodes to start a transient,
and its `Dc` is singular (§16).

## D.7 Guidance

Each guideline restates a consequence of a normative rule.

| Guideline | Follows from |
| --- | --- |
| Read a discrete carrier in `analog` only when the analog behavior depends on it. Each change is then a time point and a jump. | §15.2, §17.2 |
| Say what unknown means electrically, with `known_or` or `high()`. | §3.3.5 |
| Prefer known times (`at`, `after`) to crossings. A scheduled time costs no location. | §17.2, §17.3 |
| Sample at a clock edge with a reaction, not with `cross` on the clock's analog image. | §12.1.5 |
| Make the edges of a phase with offset crossings, not with the wrap of `idtmod`. | D.3, §13 rule 5 |
| Give edges a duration (`transition`) when the edge rate matters to the analog side. | D.4, §22.2 |
| Make every corner of a `now()` waveform a carrier change. | §17.3, §9 case D |
| Give a node with no DC path an `= value`, and expect its `Dc` to fail. | §16 |
| Guard a reaction with `s.prev().is_known()` when the first commit must not count as an edge. | §13 rule 6, §12.1 |

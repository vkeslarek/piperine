# dv-tester IDEAL — a host-language device library over the `Element` ABI

**Status:** surface design document, rewritten 2026-07-27 after the first draft
was rejected for being too narrow.
**What this is:** a library for **writing devices in the host language** — Python
or Rust, same contract — spanning from "place this stamp" to "ramp a sigmoid here"
to "drive a burst on the clock edge". A tester is one kind of device you write
with it, not the thing itself.

**What the first draft got wrong.** It designed a test sequencer and then
restricted the surface — two segment kinds, no waveform synthesis — in the name of
implementability. That closes off exactly the interesting cases: an author who
wants a sigmoid, or who wants to stamp something the library never anticipated,
hits a wall and has no escape hatch. The correct principle is the opposite:
**expose the whole ABI, make the cheap path convenient, and let the author choose
the granularity of host calls.** Restriction is not the same as simplicity.

---

## 0. Why this is a real gap

There are four ways to get a device into a Piperine circuit today, and none of
them lets you write one where you already are:

| Path | Authoring cost | Speed | Distribution |
|---|---|---|---|
| **PHDL** (`analog`/`digital` body) | declarative; must be expressible in PHDL | JIT — fastest | source |
| **Plugin cdylib** (`#[pip::device]`) | a Rust crate, `dlopen`, host and plugin built by the same toolchain (`backend/native.rs`) | native | binary artifact |
| **OSDI** (external `piperine-osdi`) | Verilog-A + a compiler | native | `.osdi` |
| **Host device library** ← this | a class in the script you are already writing | host-call bound, or native in Rust | a module |

Device plugins are **native-only** — `crates/piperine-plugin/src/backend/native.rs`
requires a compiled shared library, and `manifest.rs` shows Python plugins are
*scripted* (CLI entries and hooks), never devices. So there is no way to write a
device in Python at all, and no way to write one in Rust without building a
separate crate with a matched toolchain.

That is the gap: **no in-process, no-build-step device authoring.** Testers need
it most urgently, which is why it lives in this feature — but the capability is
general, and designing it as "tester plumbing" is what produced the bad first draft.

---

## 1. The `Element` surface is already the right shape

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

The library's job is to make this surface reachable from Python and Rust, safely,
without pretending it is smaller than it is.

---

## 2. Three access levels that **compose**

Not layers you choose between — coordinates in one device. A device may stamp
directly, install a waveform, and sequence time, all at once.

### Level 0 — raw: you place the stamps

The escape hatch, and the reason nothing is ever a wall.

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
for current RHS, `m.branch(...)` for a branch unknown. Nothing is hidden. If your
device needs something the library never anticipated, this level always works.

### Level 1 — behavioral: you declare, the library stamps

For the common cases, with the **cheap path being the default**:

```python
class Supply(pip.Device):
    def build(self, b):
        b.vsource("vdd", "gnd", wave=pip.Wave.ramp(0.0, 1.8, 1e-3), rout=0.05)

class Stim(pip.Device):
    def build(self, b):
        b.vsource("in", "gnd", wave=pip.Wave.sigmoid(lo=0, hi=1.2, t0=1e-6, tau=50e-9))
        b.isource("bias", "gnd", wave=pip.Wave.const(20e-6))
```

`pip.Wave` is a **descriptor**, evaluated natively with no host call in the solve
loop. The shipped set covers what an author reaches for:

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
2. `Wave.sampled(t, v)` — precompute in numpy, stamped as native PWL.
   **Zero host calls**, arbitrary shape. This is the right answer far more often
   than people expect, and it is the reason the library does not need to be
   restrictive to be fast.
3. `Wave.from_fn(f)` — a host callback, evaluated whenever the solver asks.
   Arbitrary and slow. Available, priced, and never the default.

### Level 2 — sequencing: you advance time and react

A coroutine over simulated time. This is where a *tester* lives, and where
"drive a burst on the clock edge" is written:

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

A `Sequencer` **is** a `Device` — it is the same ABI object with `next_breakpoints`
wired to the coroutine's pending wake. Which means a sequencer can also stamp
(level 0) and install waveforms (level 1) in the same class, because those are
just methods it inherits.

---

## 3. The cost model, stated per level

The first draft's mistake was hiding this behind restrictions. Exposing it lets an
author make the trade themselves.

| What you write | Host calls | Per | Practical ceiling |
|---|---|---|---|
| `Wave.*` descriptor (except `from_fn`) | **0** | — | unlimited; native stamping |
| `Wave.sampled(numpy)` | **0** | — | unlimited; native PWL |
| `Sequencer` wake | 1 | tester breakpoint | ~10⁵ per run is free (~1 µs each) |
| `load_dc`/`load_transient` in Python | 1 | **Newton iteration** | ~10⁵–10⁶ total before it dominates |
| `Wave.from_fn` | 1 | evaluation | same as above |
| `update` in Python | 1 | accepted step | ~10⁵–10⁶ |
| the same in Rust | native | — | no ceiling |

The rules that follow, and they are guidance rather than prohibition:

- **A boundary device is cheap; a bulk device is not.** A tester or a stimulus
  source is a handful of elements. Ten thousand Python memristors in one circuit
  is a bad idea, and the library should say so in its docs rather than prevent it.
- **Precompute beats callback.** `Wave.sampled` with numpy is almost always
  available and is free in the loop.
- **Rust is the answer for bulk.** Same contract, no per-call cost — which is what
  makes parity worth having rather than symmetric decoration.
- **The clock is still better off in the design.** Not a rule of the library, an
  arithmetic fact: a sequencer-driven 1 GHz clock over 1 ms is 2 × 10⁶ forced
  solver landings. Synchronizing to an existing clock (`t.wait(edge("clk"))`) costs
  nothing extra. The library makes both possible and documents the difference.

---

## 4. Rust/Python parity — one contract, two bindings

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
  **identical** to the Rust trait — the same document describes both.
- **`Sequencer`** is a `HostElement` whose `next_breakpoints` comes from its
  coroutine. Python uses a generator; Rust implements `resume` as an explicit state
  machine (no stable generators). That asymmetry is real and is stated, not hidden.
- Parity is enforced the way the project already enforces it: a
  `host_parity.rs`-style target that enumerates the surface on both sides.

---

## 5. Where the guarantees apply — and where they do not

D11 (no rollback protocol, because explicit `advance` is the guarantee) is a
**sequencing-level** property. It does not extend to level 0:

| Level | Rejected steps | Rule |
|---|---|---|
| Level 2 (sequencer) | never observed — resumed only after acceptance | D11 holds; no rollback logic in the test |
| Level 0/1 (stamping device) | **participates in them** like any element | the author handles `accept_timestep` and mutates state only on acceptance, exactly as a PHDL device must |

This distinction must be loud in the documentation. A device that integrates state
in `load_transient` instead of `accept_timestep`/`update` will be subtly wrong in
a way that only shows up with timestep rejection — the same trap PHDL device
authors already face, now reachable from Python.

---

## 6. Examples spanning the range

**A — a resistor in twelve lines** (level 0; the "hello world" that proves the ABI
is genuinely exposed):

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

**D — burst on the clock edge with an analog check** (level 2 + level 1 together):

```python
class BurstTest(pip.Sequencer):
    def run(self, t):
        yield t.advance(1e-6)
        for burst in range(8):
            for _ in range(16):
                yield t.wait(pip.edge("clk", pip.RISING))
                t.drive("data", 1)
                yield t.wait(pip.edge("clk", pip.FALLING))
                t.drive("data", 0)
            t.install("vbias", pip.Wave.ramp(0.4, 0.4 + 0.05 * burst, 100e-9))
            yield t.advance(1e-6)
            t.expect(t.read_v("vpeak") < 1.9, f"burst {burst}: vpeak too high")
```

**E — a device that both stamps and sequences** (the composition that the first
draft made impossible):

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

## 7. Implementability

Honest sizing — larger than the first draft claimed, because the capability is
larger:

| Piece | Language | Estimate | Notes |
|---|---|---|---|
| `HostElement` trait + blanket `Element` adapter | Rust | ~400 | mirrors `element.rs`'s defaults |
| `StampSink` / `Ctx` / `StateView` safe wrappers | Rust | ~350 | **the real work**: exposing matrix and solution references safely across FFI |
| `Wave` descriptors + native evaluation | Rust | ~400 | ~10 kinds + composition |
| Sequencer (coroutine driver, wakes, findings) | Rust | ~350 | breakpoints exist; edge waits need dynamic digital sensitivity |
| PyO3 bridge (dispatch, exceptions, GIL) | Rust | ~450 | duck-typed method presence |
| Python surface (`pip.Device`, `pip.Wave`, `pip.Sequencer`) | Python | ~300 | thin |
| Protocol helpers (uart/spi/vectors) | Python | ~300 | user-extensible, shipped as examples |

≈2 000 lines of Rust and ~600 of Python. Nothing in the JIT, codegen, or solver
core: the blanket adapter means the solver keeps seeing `Element` and nothing else.

### What could go wrong

| Risk | Containment |
|---|---|
| Exposing the stamp matrix to Python unsoundly (dangling references, aliasing) | The sink is a narrow, index-validated API — never a raw pointer; invalid indices are loud, not undefined |
| **The GIL**: a Python device holds it during the solve, serializing anything the solver might parallelize | Documented as a property of the Python tier; Rust devices have no such cost, which is a concrete reason parity exists |
| Python exceptions mid-solve | One catch point in the bridge; mapped to a typed error carrying simulation time; the analysis stops cleanly |
| Authors integrating state in `load_*` instead of `accept_timestep` | §5 is documentation *and* an example; the trap already exists for PHDL authors, so it is a known shape |
| Dynamic digital sensitivity (edge waits) needs scheduler surgery | Time wakes alone make examples A, B, C and E work; edge waits can land second |
| Per-call cost surprises a user who wrote a bulk Python device | §3 is published, with the Rust escape route named |
| A third device path fragments the story | §0's table is the answer: four paths, four different authoring/speed/distribution trade-offs, one ABI underneath |

---

## 8. Deliberately out

| Not included | Why |
|---|---|
| A test-runner framework | `piperine test` and `*_tb.py` already exist |
| Randomization / constrained-random over parameters | `dv-gradients`' Monte Carlo owns statistical sampling |
| Scoreboards, transaction layers | Python data structures already express these; ship examples, not a framework |
| Distribution/packaging of host devices | Plugin territory (`piperine-plugin`); an open question is whether a host device can *become* a plugin contribution (§9) |
| Circuit-level assertions (SOA, spec limits) | `dv-core`'s `constraint` blocks — different rhythm, different home |

---

## 9. Open questions

1. **Naming, and the relationship to `#[pip::device]`.** The plugin macro already
   owns that spelling for compiled Rust devices. Options: one concept with two
   backends (a Python `@pip.device` and a Rust `#[pip::device]` that mean the same
   thing at different tiers), or distinct names to keep the tiers legible. Leaning
   toward one concept, because "how do I write a device" should have one answer
   whose *tier* is a detail — but that is a real decision about the plugin story,
   not just a name.
2. **How does a host device enter a circuit?** Attached to a compiled session from
   the host (simple, no PHDL, not reusable as a component), or declared in PHDL and
   instantiated like any module (composes with hierarchy, needs a declaration —
   possibly the existing `@device` attribute with a new backend). The second is
   what would let an acceptance suite ship as a part.
3. **Can a host device be distributed?** If yes, it is a plugin contribution and
   `piperine-plugin`'s trust/permissions model applies. If no, it is
   script-local — which is fine for testers and limiting for models.
4. **DC/OP with a sequencer attached.** D13 defers AC loudly. DC could treat
   installed waveforms as their `t = 0` value, or refuse. Treating them as bias is
   what an author will expect after writing example B.
5. **`set_param` from a host device mid-transient.** Restamp-class is the
   live-parameter path and fine; rebuild-class would force re-elaboration
   mid-analysis and must be loud.
6. **Ordering among multiple host devices at one instant.** Instantiation order,
   documented — not left to chance.

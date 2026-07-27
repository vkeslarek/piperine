# dv-tester IDEAL — designing the tester library surface for implementability

**Status:** surface design document. Companion to `spec.md`/`design.md` in this
directory; the shared vision is `../design-verification/ideal.md` §4.
**Purpose:** the tester is the feature most likely to be *used* rather than read
about, and the one whose API is hardest to change once written. This document
designs that surface for two properties at once: **easy to write a test with**,
and **easy to implement**. Where those conflict, it says so.

---

## 0. What "easily implementable" means here, concretely

Four commitments. Every design choice below is justified by one of them.

1. **The ABI surface is four operation kinds.** Everything a test does reduces to:
   let time pass, install a drive, read a value, report a finding. Only the first
   one yields. If a proposed feature needs a fifth kind, it needs a very good
   reason.
2. **No waveform synthesis in the tester.** The tester sequences and checks; the
   *design* generates shapes. `spice::sources` already has PULSE/SIN/PWL, and the
   live-parameter path already lets a host change their parameters between steps.
   A tester that grew its own sine generator would be reimplementing the design
   language inside the host.
3. **The tester never adds a breakpoint the circuit does not already have** —
   unless the test genuinely acts at that instant. Breakpoints constrain the
   solver's timestep (D11's accepted cost), so the design must make synchronizing
   to *existing* events the easy path and generating new ones the deliberate one.
4. **Nothing touches the JIT, codegen, or the solver core.** The tester is an
   `Element` plus a host bridge. If the implementation starts editing
   `emit/analog_expr.rs`, the design is wrong.

---

## 1. The primitive core

### 1.1 The one yielding operation: time

```python
yield t.advance(dt)            # let dt of simulated time pass
yield t.wait_until(time)       # absolute instant
yield t.wait_for(edge("clk"))  # resume on the next matching digital edge
yield t.finish()               # this program is done; the analysis continues
```

Only these yield. **Everything else is an immediate call** on the handle, because
it needs no simulated time to happen. That split is what keeps the request
protocol tiny:

```
Request  ::=  Advance(dt)  |  WaitUntil(t)  |  WaitEvent(spec)  |  Finish
```

Four variants. The element translates a `Request` into a wake registration and
returns control to the solver. Nothing else crosses the boundary as a request.

### 1.2 Install a drive (immediate)

```python
t.drive(net, value)                       # digital net → EventSink, never the MNA
t.drive_v(net, volts, rout=50.0)          # analog: Norton source, declared impedance
t.drive_i(net, amps)                      # analog: current source
t.ramp_v(net, to, dur, rout=50.0)         # linear segment from the present value
t.release(net)                            # stop driving; the net returns to the circuit
t.force_v(net, volts)                     # ideal voltage force — explicit, never default
t.set(label, param, value)                # restamp a design parameter (live-param path)
```

**Only two analog segment kinds exist: constant and linear ramp.** That is the
whole waveform vocabulary, and commitment #2 is why. Anything richer is a design
source whose parameters the tester writes with `t.set(...)` — which reuses the
already-delivered live-parameter restamp path instead of inventing a second way
to change a stimulus.

`force_v` exists because forcing a node is a legitimate bring-up tool, and it is
spelled differently from `drive_v` so that reading a test tells you which one is
happening. Per D14, forcing an internal node lands in the run's findings.

### 1.3 Read a value (immediate)

```python
v = t.read_v(net)              # accepted-point node voltage
i = t.read_i(branch)           # branch current
b = t.read(net)                # digital logic value
x = t.opvar(label, name)       # device operating-point variable
```

Reads see the **accepted solution at the current instant**. A read is only valid
while the program is resumed; the element does not cache values across a wake.

### 1.4 Report a finding (immediate)

```python
t.expect(cond, msg)            # Error finding if cond is false
t.warn(cond, msg)              # Warning finding if cond is false
t.fail(msg)                    # unconditional Error
t.note(msg)                    # informational, never a failure
```

These go into `dv-core`'s `validation_reports()` channel with the current
simulation time and the tester's instance path. **Tester findings are always
collected, regardless of the `checks=` posture** — `checks=` governs `require`
margins, and a test whose expectations were silenced by a solver knob would be a
trap. What the posture does govern is whether an `Error` *aborts* the analysis or
merely records.

---

## 2. Wake sources — and the one that is deferred

Three ways a tester can be woken, with very different implementation costs:

| Wake | Mechanism | Status |
|---|---|---|
| **Time** | `Element::next_breakpoints` (`core/element.rs:187`) | **exists** — the transient driver already lands on declared breakpoints |
| **Digital edge** | element sensitivity through the digital scheduler (`DEPENDS_ON_DIGITAL`) | **exists as a mechanism**; needs a dynamic-sensitivity path so the *host* can say what it is waiting for |
| **Analog threshold** | a dynamically registered `cross`/`above` watch | **deferred** — PHDL's event machinery compiles static event conditions; letting a host register one at run time is a new capability |

For analog thresholds in v1, the documented idiom is an explicit poll:

```python
while t.read_v("vout") < 0.9:
    yield t.advance(10e-9)      # the test chooses its own resolution
```

This is honest about its cost — the test is buying breakpoints at 10 ns — and it
keeps v1 from needing dynamic analog event registration. A native
`wait_for(cross("vout", 0.9))` is the obvious follow-up, and the surface above is
designed so adding it changes nothing else: it is one more `WaitEvent` spec.

### 2.1 Do not generate the clock in the tester

The single most important performance rule in this design, and it falls out of
commitment #3.

A tester that drives its own clock installs two breakpoints per cycle. At 100 MHz
over 10 µs that is 2 000 forced solver landings — fine. At 1 GHz over 1 ms it is
2 × 10⁶ — not fine, and the slowdown will be blamed on the tester rather than on
the choice.

So the clock belongs in the **design** — an ordinary `digital` oscillator or a
`spice::sources` pulse — and the tester **synchronizes** to it:

```python
for _ in range(1000):
    yield t.wait_for(edge("clk", rising))
    t.drive("d", next(vectors))
```

The tester now adds breakpoints only where it acts. Generating a clock stays
possible (`t.drive` in a loop) and is the deliberate choice, not the default one.

---

## 3. The layered library

Three layers, and only the bottom one is Rust.

```
Layer 3 — protocol helpers, pure Python, user-extensible
          uart_send/uart_recv, spi_xfer, i2c_*, apply_vectors(file), clock_sync
          ── shipped as a Python module; users write their own the same way
                          │  built only from layer 2
Layer 2 — conveniences, pure Python
          ramp_v (composes drive_v), pulse, settle_to(value, tol, timeout),
          expect_within(t, cond), for_each_edge(...)
                          │  built only from layer 1
Layer 1 — the primitives of §1, Rust + the host bridge
          Advance/WaitUntil/WaitEvent/Finish · drive/read/report
```

The layering is a delivery plan as much as a structure: **layer 1 is the whole
Rust deliverable.** Layers 2 and 3 are Python that a user could have written, which
is the test of whether layer 1 is complete. If a protocol helper cannot be written
in Python over layer 1, layer 1 is missing something — and that is a much better
signal than reviewing the API in the abstract.

---

## 4. The element side — what must actually be tracked

Deliberately small, because commitment #4 says this is the only new runtime object:

```
TesterElement {
    program        : HostProgram,          // opaque handle; §5
    pending_wake   : Wake,                 // Time(t) | Event(spec) | Done
    drives         : Map<NetHandle, Segment>,
    findings       : Vec<ValidationFinding>,   // drained by validation_reports()
    net_cache      : Map<String, NetHandle>,   // resolved on first use, then cached
}

Segment ::= Const { v }  |  Ramp { v0, v1, t0, dt }
```

Element responsibilities, in full:

1. `next_breakpoints(from, horizon)` → the pending time wake, if it is a time wake.
2. Stamp every active `Segment` on `load_dc`/`load_transient` — evaluating a
   segment is arithmetic on `(t, v0, v1, t0, dt)`, **with no host call**.
3. After the solver **accepts** a step at the pending wake, resume the program,
   apply whatever immediate calls it makes, and store the next `Request`.
4. Drain findings through `validation_reports()`, gated by `EMITS_VALIDATION`.
5. Declare capabilities: `ANALOG` if it drives or reads analog, `DIGITAL` if
   digital, `SAMPLES_ANALOG` for analog reads, `EMITS_VALIDATION` always.

That is the entire behavioral surface. There is no protocol knowledge, no
waveform library, no scheduling policy — the element is a dumb executor of
requests, which is exactly why it is implementable.

**The one ordering rule that must not be got wrong:** resume happens *after* step
acceptance, never inside the accept/reject decision. A tester resumed during a
step that is later rejected breaks D11's guarantee with no clean repair.

---

## 5. Two host faces, one contract

The contract is: **resume the program, get back a `Request`.**

```
trait HostProgram {
    fn resume(&mut self, ctx: &mut TesterCtx) -> Result<Request, TestError>;
}
```

### 5.1 Python — a generator

The generator protocol *is* this contract. `resume` calls `gen.send(None)`; the
yielded object is the `Request`; immediate calls happened as ordinary method calls
on `t` before the yield.

```python
def uart_echo(t):
    t.drive("rst", 1)
    yield t.advance(100e-9)
    t.drive("rst", 0)
    t.ramp_v("vdd", 1.8, 1e-6)
    yield t.advance(2e-6)

    for byte in (0x55, 0xAA):
        yield from uart_send(t, "tx", byte, baud=1e6)
        got = yield from uart_recv(t, "rx", baud=1e6)
        t.expect(got == byte, f"echo {got:#x} != {byte:#x}")
```

`yield from` composing sub-protocols is the property that makes layer 3 possible
without any framework: a protocol helper is just a generator that yields requests.

A Python exception becomes a `TestError` carrying the simulation time, reported as
an `Error` finding, and the analysis stops cleanly — the generator is closed so
`finally` blocks run.

### 5.2 Rust — an explicit state machine

Rust has no stable generators, so the same contract is implemented by hand:

```rust
enum Phase { Reset, PowerUp, Send(u8), Check(u8), Done }

impl HostProgram for UartEcho {
    fn resume(&mut self, ctx: &mut TesterCtx) -> Result<Request, TestError> {
        match self.phase {
            Phase::Reset   => { ctx.drive("rst", 1); self.phase = Phase::PowerUp;
                                Ok(Request::Advance(100e-9)) }
            Phase::PowerUp => { ctx.drive("rst", 0); ctx.ramp_v("vdd", 1.8, 1e-6);
                                self.phase = Phase::Send(0x55);
                                Ok(Request::Advance(2e-6)) }
            /* … */
            Phase::Done    => Ok(Request::Finish),
        }
    }
}
```

**This is admittedly clumsier, and that is accepted** (D15): Python is the
intended face and Rust exists for parity and for hosts that cannot embed Python.
Being explicit about the asymmetry is better than pretending an ergonomic Rust
story exists — and it is why layers 2 and 3 are specified as Python.

---

## 6. Worked examples, increasing in difficulty

**A — analog bring-up, no digital at all.** Proves the drive path that made RNM
unnecessary:

```python
def supply_ramp(t):
    t.ramp_v("vdd", 1.8, 100e-6, rout=0.1)
    yield t.advance(100e-6)
    t.expect(abs(t.read_v("vref") - 0.9) < 0.01, f"vref = {t.read_v('vref')}")
    for v in (1.62, 1.8, 1.98):                 # corners of the supply
        t.drive_v("vdd", v, rout=0.1)
        yield t.advance(10e-6)
        t.expect(abs(t.read_v("vref") - 0.9) < 0.02, f"vref at vdd={v}")
```

**B — vectors from a file, synchronized to the design's clock.** D12 in action:

```python
def apply_vectors(t, path):
    with open(path) as f:                        # side effects are allowed
        for line in f:
            stim, expected = line.split()
            yield t.wait_for(edge("clk", rising))
            t.drive("d", int(stim, 2))
            yield t.wait_for(edge("clk", falling))
            t.expect(t.read("q") == int(expected, 2), f"vector {line.strip()}")
```

**C — a mixed-signal loop: digital control reacting to an analog measurement.**
The case that is awkward in every other formalism:

```python
def servo_bringup(t):
    code = 0
    for _ in range(64):                          # successive approximation by hand
        t.drive("dac_code", code)
        yield t.advance(1e-6)                    # let the analog settle
        if t.read_v("vout") < 1.0:
            code += 1
        else:
            break
    t.expect(code < 64, "servo never reached 1.0 V")
    t.note(f"settled at code {code}")
```

**D — isolating a block by forcing an internal node.** Visible in the findings by
construction (D14):

```python
def isolate_stage2(t):
    t.force_v("u_stage1.out", 0.9)               # reported in r.violations
    yield t.advance(1e-6)
    t.expect(t.read_v("u_stage2.out") > 1.2, "stage 2 gain with forced input")
```

---

## 7. Implementability review

Rough sizing, so the claim is checkable rather than asserted:

| Piece | Language | Estimate | Depends on |
|---|---|---|---|
| `TesterElement` (state, breakpoints, resume, findings) | Rust | ~350 lines | `dv-core`'s findings channel |
| `Request` + wake translation | Rust | ~100 | `next_breakpoints`, digital sensitivity |
| Segment stamping (Const, Ramp) | Rust | ~150 | existing source-stamping patterns |
| Net/branch resolution + cache | Rust | ~100 | the existing probe/trace resolution |
| PyO3 bridge (generator drive, exception mapping) | Rust | ~200 | — |
| `HostProgram` trait + Rust example | Rust | ~80 | — |
| Layers 2 and 3 | **Python** | ~300 | layer 1 only |

≈1 000 lines of Rust and ~300 of Python, and **none of it in the JIT, codegen, or
the solver core** (commitment #4). The two genuinely new mechanisms are the
resume callback and dynamic digital sensitivity; everything else composes existing
machinery.

### What could still go wrong

| Risk | Why it is contained |
|---|---|
| Dynamic digital sensitivity turns out to need scheduler surgery | Time wakes alone make the feature useful (examples A, C and D need no edge wait); edge waits can land second |
| Resume-vs-accept ordering is subtly wrong | It is a single call site, and the acceptance test is a recording tester on a fixture with rejected steps |
| Breakpoint density makes tests slow | §2.1 makes clock-synchronization the easy path; the cost is documented (D11), not hidden |
| Python exceptions across FFI leak or abort badly | One catch point in the bridge; generator `close()` so `finally` runs |
| Net-name resolution differs from the probe surface | Reuse that resolution rather than writing a second one; a mismatch here would be a user-visible inconsistency |
| Analog thresholds needed sooner than expected | The polling idiom (§2) works today and the surface accepts a native wait without other changes |

---

## 8. Deliberately not in the library

| Not included | Why | Where it goes instead |
|---|---|---|
| Waveform synthesis (sine, exponential, arbitrary PWL) | Commitment #2 | design sources + `t.set(...)` |
| Clock generation as the default pattern | Commitment #3 / §2.1 | a `digital` oscillator in the design |
| A test-runner framework (discovery, fixtures, reporting) | `piperine test` and `*_tb.py` already exist | the CLI |
| Randomization / constrained-random | Python has `random`; constrained-random over *parameters* is `dv-gradients`' Monte Carlo | user code / `dv-gradients` |
| Scoreboards, transaction layers | UVM-shaped abstractions that Python data structures already express | user code, layer 3 |
| Protocol IP beyond a couple of reference helpers | Layer 3 is a starting library, not a catalog | users and plugins |
| Assertions about the *circuit* (SOA, spec limits) | Those are `constraint` blocks — different rhythm, different home | `dv-core` |

---

## 9. Open questions

1. **How does a tester get instantiated?** Two shapes: a PHDL module the design
   instantiates (composes with hierarchy, needs a declaration), or a host-side
   attachment to a compiled session (no PHDL at all, cannot be reused as a
   component). The host-side attachment is simpler and matches "no grammar";
   the PHDL instantiation is what makes an acceptance suite shippable as a part.
   **Leaning: host attachment for v1**, with the PHDL form as the follow-up once
   there is a real reusable suite to justify it.
2. **DC and OP with a tester attached.** D13 defers AC loudly. DC could treat
   installed segments as a bias, or refuse. Refusing is safer; treating them as
   bias is what a user will expect after writing example A. Undecided.
3. **`t.set()` and MD-18.** Writing a design parameter mid-test is the
   live-parameter path, which is restamp-class — but a rebuild-class parameter
   would force re-elaboration mid-transient, which cannot be allowed. Almost
   certainly: rebuild-class `set` from a tester is a loud error.
4. **Timeouts.** `settle_to(value, tol, timeout)` needs a failure mode when the
   condition never holds. A missing timeout turns a bug into a hang, so layer 2
   should probably require one rather than defaulting to forever.
5. **Multiple testers observing each other.** Two testers driving different nets
   is fine. Two testers where one reads what the other drives, at the same
   instant, has an ordering question. Simplest answer: resume order within one
   instant is instantiation order, documented — not left to chance.

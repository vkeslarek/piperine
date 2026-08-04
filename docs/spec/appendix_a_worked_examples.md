# Appendix A — Worked Examples

Each example stresses a corner of the model. Together they cover analog, digital,
and mixed-signal design, plus the host surfaces that drive them (Part VIII).

## A.1 Core library (excerpt)

The foundational devices — all behavior, no elaboration-time structure beyond params.

```phdl
discipline Electrical { potential v : Real (unit = "V", abstol = 1e-6);
                        flow      i : Real (unit = "A", abstol = 1e-12); }

mod Resistor  ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1k; }
analog Resistor  { I(p, n) <+ V(p, n) / r; }

mod Capacitor ( inout p : Electrical, inout n : Electrical ) { param c : Real = 1n; }
analog Capacitor { I(p, n) <+ c * ddt(V(p, n)); }

mod VSource ( inout p : Electrical, inout n : Electrical ) { param dc : Real = 0.0; }
analog VSource { V(p, n) <- dc; }

fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }

mod Diode ( inout a : Electrical, inout c : Electrical ) {
    param is_sat : Real = 1e-14; param temp : Real = 300.0;
}
analog Diode { I(a, c) <+ is_sat * (exp(V(a, c) / thermal_voltage(temp)) - 1.0); }

mod Comparator ( input vp : Electrical, input vn : Electrical, output out : Bit );
digital Comparator { out <- (V(vp) > V(vn)); }

mod BitToVoltage ( input d : Bit, inout a : Electrical ) {
    param vlow : Real = 0.0; param vhigh : Real = 1.8;
}
analog BitToVoltage { if (d == 1) { V(a) <- vhigh; } else { V(a) <- vlow; } }
```

## A.2 Worked architectures

### A.2.1 Parametric N-bit SAR ADC

Analog + digital in one module set. Named children (`dac`, `comp`) are loaded by the
parent through its own `analog` block (KCL accumulation of parasitic load).

```phdl
enum SarState : Bit[2] { Idle, Convert, Done }

mod Dac[N] ( input code : Bit[N], inout out : Electrical, inout gnd : Electrical ) {
    param vref : Real = 1.8;
}
analog Dac {
    var acc = 0.0;
    for i in 0..N {
        if (code[i] == 1) {
            acc = acc + vref * pow(2.0, Real::from(i)) / pow(2.0, Real::from(N));
        }
    }
    V(out, gnd) <- acc;
}

mod SarAdc[N] ( input clk : Bit, input start : Bit, input vin : Electrical,
                inout gnd : Electrical, output result : Bit[N], output done : Bit ) {
    wire dout : Electrical;  wire cmp : Bit;
    var state : SarState = Idle;  var code : Bit[N] = 0;  var idx : Natural = 0;
    param cload : Real = 50f;
    dac  : Dac[N]     ( code, dout, gnd );
    comp : Comparator ( vin, dout, cmp );
}
analog SarAdc { I(dac.out, gnd) <+ cload * ddt(V(dac.out, gnd)); }
digital SarAdc {
    result <- code;  done <- (state == Done);
    @ posedge(clk) {
        match state {
            Idle    => { if (start == 1) { state = Convert; idx = N-1; code = 0; code[N-1] = 1; } }
            Convert => { if (cmp == 0) { code[idx] = 0; }
                         if (idx == 0) { state = Done; } else { idx = idx - 1; code[idx-1] = 1; } }
            Done    => { if (start == 0) { state = Idle; } }
        }
    }
}
```

### A.2.2 Electrothermal

Two disciplines (`Electrical` and `Thermal`) coupled inside one device. No converter is
needed because no single net crosses a discipline boundary — the coupling is inside the
`analog` block.

```phdl
discipline Thermal { potential temp : Real (unit = "K", abstol = 1e-4);
                     flow pwr      : Real (unit = "W", abstol = 1e-9); }
mod HeatedResistor ( inout p : Electrical, inout n : Electrical, inout th : Thermal ) {
    param r0 : Real = 1k; param t0 : Real = 300.0; param tc : Real = 0.004;
}
analog HeatedResistor {
    var rt = r0 * (1.0 + tc * (Temp(th) - t0));
    I(p, n) <+ V(p, n) / rt;
    Pwr(th) <+ V(p, n) * V(p, n) / rt;
}
```

### A.2.3 LC oscillator

An analog initial condition with no stable DC operating point. The `@ initial` event
seeds the voltage; the oscillation is self-sustaining.

```phdl
mod LcTank ( inout p : Electrical, inout n : Electrical ) {
    param l : Real = 1u; param c : Real = 1n;
}
analog LcTank {
    I(p, n) <+ c * ddt(V(p, n)) + idt(V(p, n)) / l;
    @ initial { V(p, n) <- 1.0; }
}
```

### A.2.4 SR latch

Bistability as event-held state. The register `st` is updated on either `posedge(s)` or
`posedge(r)`.

```phdl
mod SrLatch ( input s : Bit, input r : Bit, output q : Bit ) { var st : Bit = 0; }
digital SrLatch {
    q <- st;
    @ (posedge(s) | posedge(r)) { if (s == 1) { st = 1; } else { st = 0; } }
}
```

### A.2.5 Ideal op-amp

Finite-gain VCVS. The gain is large-but-finite (1M) so the system is non-singular.

```phdl
mod OpAmp ( input inp : Electrical, input inn : Electrical, inout out : Electrical ) {
    param gain : Real = 1M;
}
analog OpAmp { V(out) <- gain * V(inp, inn); }
```

### A.2.6 Tri-state bus

Resolved multi-driver via `Quad` storage with `resolve tri`. Drivers output `0qZ`
(high-impedance) when disabled.

```phdl
discipline DataLine { storage Quad; resolve tri; }
mod Driver[N] ( input en : Bit, input val : Logic[N], inout bus : DataLine[N] );
digital Driver { if (en == 1) { bus <- val; } else { bus <- [0qZ; N]; } }
```

### A.2.7 Synchronizer

Two clock domains. The register chain (`m`, `n`) is a pipeline — within the clocked
block, reads see the pre-edge value.

```phdl
mod Synchronizer ( input d : Bit, input clk_b : Bit, output q : Bit ) {
    var m : Bit = 0; var n : Bit = 0;
}
digital Synchronizer {
    q <- n;
    @ posedge(clk_b) { m = d; n = m; }
}
```

### A.2.8 First-order delta-sigma

A closed loop crossing the analog/digital boundary twice. The register `q` is the unit
delay that makes the loop well-posed — without it, there would be a zero-delay algebraic
loop.

```phdl
mod DeltaSigma ( input vin : Electrical, inout gnd : Ground, input clk : Bit, output dout : Bit ) {
    param c : Real = 1p; param r : Real = 1k; param vref : Real = 1.0;
    wire intg : Electrical;  var q : Bit = 0;
}
analog DeltaSigma {
    var vfb = if (q == 1) { vref } else { -vref };
    I(intg, gnd) <+ c * ddt(V(intg, gnd));
    I(intg, gnd) <+ (vfb - V(vin)) / r;
}
digital DeltaSigma {
    dout <- q;
    @ posedge(clk) { q = (V(intg) > 0.0); }
}
```

### A.2.9 Ring oscillator

Feedback is the `analog` mechanism itself — a finite-bandwidth ODE with no stable DC
point on an odd ring. (The same topology in `digital` would be a zero-delay loop with
no fixed point — an error.)

```phdl
mod Inverter ( input a : Electrical, inout y : Electrical, inout gnd : Ground ) {
    param gain : Real = 10.0; param c : Real = 1f; param r : Real = 1k;
}
analog Inverter {
    var target = -gain * V(a, gnd);
    I(y, gnd) <+ c * ddt(V(y, gnd)) + (V(y, gnd) - target) / r;
}

mod RingOsc[N] ( inout gnd : Ground ) {                     // N odd
    wire node : Electrical[N];
    for i in 0..N { Inverter ( node[i], node[(i + 1) % N], gnd ); }
}
```

### A.2.10 RC ladder with per-tap parasitics

Named-instance arrays; the parent reaches each tap via `name[i].port`. Layout intent
rides on an attribute (`@route(shield = true)`).

```phdl
mod Ladder[N] ( inout bus : Electrical, inout gnd : Ground ) {
    param r : Real = 1k; param cpar : Real = 5f;
    wire tap : Electrical[N];
    for i in 0..N {
        rseg[i] : Resistor ( bus, tap[i] ) { .r = r };
        @route(shield = true) rgnd[i] : Resistor ( tap[i], gnd ) { .r = r };
    }
}
analog Ladder {
    for i in 0..N { I(rseg[i].n, gnd) <+ cpar * ddt(V(rseg[i].n, gnd)); }
}
```

### A.2.11 Generic pipelined accumulator

Generics + register inference. The accumulator updates on the clock edge when `en` is
asserted; `sum` reflects the accumulated value combinationally.

```phdl
mod Accumulator[W] ( input clk : Bit, input en : Bit, input x : UInt[W], output sum : UInt[W] ) {
    var acc : UInt[W] = 0;
}
digital Accumulator {
    sum <- acc;
    @ posedge(clk) when (en) { acc = acc + x; }
}
```

## A.3 Worked testbenches (host API)

The interpreted `bench` block was removed — testbenches are Python files
(Part VIII). One runnable twin exists per `.phdl` circuit in `examples/`;
a minimal one:

```python
# switch_open_tb.py
import piperine

m = piperine.load("switch.phdl").module("SwitchOpenTest")
r = m.op()
assert r.v("signal") < 1e-6, "open switch pulls the node to ground"
```

Run with `piperine test` (discovers `*_tb.py`) or `piperine run switch_open_tb.py`.

## A.4 Host devices and testers (dv-tester)

The host device library (Part VII §20, Part VIII §17) writes a device *in*
the host language rather than in PHDL. Both examples below reflect the
actually-delivered surface — `crates/piperine-api/src/device/`,
`crates/piperine-python/src/sequencer_bridge.rs`/`signal_bridge.rs` — not
`ideal.md`'s aspirational sketches (`t.arm`/`t.install`/`b.vsource` are not
literally what shipped; see Part VIII §17.5's stated Rust/Python
asymmetry).

### A.4.1 A host device — Rust, level 0 + level 1 composed

The declaration is ordinary PHDL — `@device` with a plugin binding and a
host registration both able to satisfy it (Part I §10.7):

```phdl
discipline Electrical { potential v : Real; flow i : Real; }

@device(plugin = "none", type = "SoftStartLoad")
mod SoftStartLoad ( inout p : Electrical, inout n : Electrical ) {
    param r : Real = 1e3;
}
```

The implementation is a Rust `HostElement`: a resistive load stamped
directly (level 0) in series with a `Wave`-driven soft-start ramp installed
once at construction (level 1, zero host calls per Newton iteration):

```rust
use piperine_api::{Ctx, HostAdapter, HostDeviceRegistry, HostElement, StampSink, WaveStamp};
use piperine_codegen::device::PortBinding;
use piperine_signal::wave::Wave;
use piperine_solver::abi::{AnalogReference, ElementCapabilities};

struct SoftStartLoad {
    p: AnalogReference,
    n: AnalogReference,
    g: f64,                 // level 0: the resistive load, r's conductance
    ramp: WaveStamp,         // level 1: a soft-start Wave in series with p/n
}

impl HostElement for SoftStartLoad {
    fn name(&self) -> &str { "SoftStartLoad" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
    fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        sink.g(&self.p, &self.p, self.g);
        sink.g(&self.p, &self.n, -self.g);
        sink.g(&self.n, &self.p, -self.g);
        sink.g(&self.n, &self.n, self.g);
        self.ramp.stamp(ctx, sink);           // native, zero host calls
    }
}

fn analog_node(binding: &PortBinding) -> AnalogReference {
    match binding {
        PortBinding::Analog(r) => r.clone(),
        PortBinding::Digital(_) => panic!("SoftStartLoad has no digital ports"),
    }
}

let registry = HostDeviceRegistry::new();
registry.register("SoftStartLoad", |spec| {
    let p = analog_node(&spec.ports[0]);
    let n = analog_node(&spec.ports[1]);
    Ok(Box::new(HostAdapter(SoftStartLoad {
        p: p.clone(), n: n.clone(),
        g: 1.0 / 1e3,
        ramp: WaveStamp::new(Wave::sigmoid(0.0, 1.8, 200e-6, 40e-6), p, n, 0.05),
    })))
});
```

`SessionBuilder::host_registry(Rc::new(registry))` (Appendix C §2) wires it; the same
`@device` declaration would resolve to a plugin binding instead if one were
wired with `SessionBuilder::provider(...)` — the PHDL never changes between
the two modes (Part I §10.7's graduation path).

### A.4.2 A tester — Python `Sequencer`

A `pip.Sequencer` subclass is the coroutine-shaped tester itself —
`run(self, t)` is a generator, `yield t.advance(dt)`/`yield
t.wait(pip.edge(...))` are the wake forms, `t.expect`/`t.warn` report
findings. `@pip.device(type_id)` declares which `@device(plugin = "none",
type = type_id)` PHDL instance it binds to, exactly as a plain host device
does (Part I §10.7):

```python
# regulator_tb.py
import piperine as pip

PHDL = """
discipline Electrical { potential v : Real; flow i : Real; }

mod VSource ( inout p : Electrical, inout n : Electrical ) { param dc : Real = 0.0; }
analog VSource { V(p, n) <- dc; }

mod Regulator ( inout vin : Electrical, inout vout : Electrical, inout gnd : Electrical );
analog Regulator { V(vout, gnd) <+ min(V(vin, gnd), 1.8); }

@device(plugin = "none", type = "RegulatorTest")
mod RegulatorTest ( inout vout : Electrical, inout gnd : Electrical );

mod Top () {
    wire vin : Electrical; wire vout : Electrical; wire gnd : Electrical;
    src : VSource ( vin, gnd ) { .dc = 3.3 };
    reg : Regulator ( vin, vout, gnd );
    tst : RegulatorTest ( vout, gnd );
}
"""

@pip.device('RegulatorTest')
class RegulatorTest(pip.Sequencer):
    """`t` (`SeqCtl`) exposes advance/wait/expect/warn today (Part VIII
    §17.5) — no `t.read_v` yet, so a pure-observer device reads its own
    input through `accept_timestep`, the same pattern `ideal.md` example G
    composes level 0 (observation) with level 2 (sequencing)."""

    caps = pip.ANALOG

    def __init__(self, ports, params):
        self.p, self.n = ports
        self.last_vout = None

    def load_dc(self, ctx, m):
        pass  # a pure observer: no stamps of its own

    def accept_timestep(self, state, t):
        self.last_vout = state.v(self.p) - state.v(self.n)

    def run(self, t):
        yield t.advance(50e-6)                      # let the operating point settle
        t.expect(self.last_vout is not None, "never observed vout")
        t.expect(abs(self.last_vout - 1.8) < 1e-3,
                  f"regulator output {self.last_vout:.4f} V, want 1.8 V")
        yield t.advance(50e-6)

# `Module.compile()`'s ordinary path does not yet auto-wire `@pip.device`
# registrations (that is `piperine_python::registration::
# register_python_devices` + `Session::builder(..).provider(registry)`,
# `crates/piperine-python/tests/host_device_library.rs`'s established
# idiom) — a Python tester is driven from a script that builds its own
# `HostDeviceRegistry` today, not yet a one-line `pip.load(...).compile()`
# call. This is the honest state of the wiring, not the aspirational one.
```

A failing `t.expect` records an `Error` finding with the resume time and
the sequencer's own instance path — never a print — and the coroutine
keeps running past it (posture `collect`) or aborts there (posture
`strict`, Part VIII §8/§17.7).

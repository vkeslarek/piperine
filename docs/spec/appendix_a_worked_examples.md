# Appendix A — Worked Examples

Each example stresses a corner of the model. Together they cover analog, digital,
and mixed-signal design, plus the host surfaces that drive them (Part VIII). Every
snippet below is real Verilog-AMS, checked against the actual sources under
`examples/*.vams` (Piperine's runnable example gallery, one Python twin per
circuit, gated end-to-end by `tests/run_examples.rs`) — several sections are
verbatim excerpts; the rest are minimal compositions built from the same
constructs those files use, so nothing here relies on a construct outside the
supported LRM subset.

## A.1 Core library (excerpt)

The foundational device patterns — all behavior, no elaboration-time structure
beyond `parameter`s. Each piece below is drawn from a real example
(`01_voltage_divider.vams`, `03_diode_clipper.vams`, `06_flash_adc.vams`).

```verilogams
`include "disciplines.vams"

module VoltageSource (inout electrical p, inout electrical n);
    parameter real voltage = 5.0;
    analog begin
        V(p, n) <+ voltage;
    end
endmodule

module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule

module Capacitor (inout electrical p, inout electrical n);
    parameter real c = 1e-9;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
    end
endmodule

// Series-resistor-into-diode clipper: the exponential nonlinearity that
// exercises the Newton solve (03_diode_clipper.vams).
module Diode (inout electrical a, inout electrical c);
    parameter real isat = 1e-14;
    parameter real vt = 0.02585;
    analog begin
        I(a, c) <+ isat * (exp(V(a, c) / vt) - 1.0);
    end
endmodule

// Comparator: a digital block reading an analog node through the A2D
// bridge — `always @(*)` over a `V(...)` read (06_flash_adc.vams's idiom).
module Comparator (input electrical vp, input electrical vn, output logic out);
    always @(*) begin
        out = V(vp, vn) > 0.0;
    end
endmodule

// D2A bridge: a digital `logic` net driving an analog level from inside an
// `analog` block (23_mixed_signal_adc.vams's idiom).
module BitToVoltage (input logic d, inout electrical a, inout electrical gnd);
    parameter real vlow = 0.0;
    parameter real vhigh = 1.8;
    analog begin
        V(a, gnd) <+ (d == 1) ? vhigh : vlow;
    end
endmodule
```

## A.2 Worked architectures

### A.2.1 Multi-level flash ADC (analog + digital in one circuit)

A bank of comparators (digital, reading node voltages through the A2D bridge)
feeds thermometer-to-binary logic, and an analog reconstruction DAC lets a
host testbench verify the transfer staircase end-to-end — real syntax,
excerpted from `examples/06_flash_adc.vams` (a `mod Dac[N]`/generic
successive-approximation state machine, as an earlier PHDL-era draft of this
appendix sketched, used generics and an `enum` state register that have no
Verilog-AMS equivalent in Piperine's supported subset; this is the real,
buildable substitute):

```verilogams
`include "disciplines.vams"

module FlashAdc (input electrical vin, inout electrical gnd, inout electrical rec);
    integer q1;
    integer q0;
    real code;
    always @(*) begin
        q1 = V(vin, gnd) > 2.0;
        q0 = (V(vin, gnd) > 3.0) || ((V(vin, gnd) > 1.0) && !(V(vin, gnd) > 2.0));
        code = 0.0;
        if (q1 == 1) code = code + 2.0;
        if (q0 == 1) code = code + 1.0;
    end
    analog begin
        V(rec, gnd) <+ code;
    end
endmodule

module AdcBoard ();
    electrical gnd;
    electrical vin;
    electrical rec;
    VoltageSource #(.voltage(0.5)) src (.p(vin), .n(gnd));
    FlashAdc adc (.vin(vin), .gnd(gnd), .rec(rec));
endmodule
```

### A.2.2 Electrothermal

Two disciplines (`electrical` and `thermal`, both from the Annex D
`disciplines.vams` standard library) coupled inside one device — a bang-bang
thermostat: a thermal RC plant heated by a controller whose `@(above(...))`
events implement the hysteresis comparator (`examples/07_thermostat.vams`,
verbatim):

```verilogams
`include "disciplines.vams"

// Heat capacity + leak to ambient.
module ThermalPlant (inout electrical temp, inout electrical gnd);
    parameter real tamb = 20.0;
    parameter real rth = 10.0;     // C per W
    parameter real cth = 1e-3;     // tau = rth*cth = 10 ms
    analog begin
        I(temp, gnd) <+ cth * ddt(V(temp, gnd));
        I(temp, gnd) <+ (V(temp, gnd) - tamb) / rth;
    end
endmodule

// The controller injects heat while `heater` is on; the events implement
// the hysteresis comparator. `heater` is persistent analog state
// (module-scope `real`): seeded once at `initial_step`, then flipped only
// by the `above` events.
module Controller (inout electrical temp, inout electrical gnd);
    parameter real t_high = 25.0;
    parameter real t_low = 22.0;
    parameter real power = 1.0;    // W -> +10 C above ambient when always-on
    real heater;
    analog begin
        @(initial_step) heater = 1.0;
        I(temp, gnd) <+ -heater * power;
        @(above(V(temp, gnd) - t_high)) heater = 0.0;
        @(above(t_low - V(temp, gnd))) heater = 1.0;
    end
endmodule

module Thermostat ();
    electrical gnd;
    electrical temp;
    ThermalPlant plant (.temp(temp), .gnd(gnd));
    Controller    ctrl  (.temp(temp), .gnd(gnd));
endmodule
```

(This example encodes temperature on an `electrical` node — 1 V = 1 °C — rather
than a genuine cross-branch `thermal` net; that is the real file's own
modeling choice, not a language limitation. `disciplines.vams` does ship a
standard `thermal` discipline with `Temp`/`Pwr` access functions for designs
that want a real conservative thermal branch.)

### A.2.3 Single-pole op-amp macromodel (analog feedback, non-singular loop)

A finite-gain macromodel (`gm` into `rp`‖`cp`, ideal output buffer) wired as a
unity follower — `examples/12_opamp_follower.vams`, verbatim:

```verilogams
`include "disciplines.vams"

// A0 = gm*rp = 1e4, pole fp = 1/(2*pi*rp*cp) = 10 Hz, GBW = A0*fp = 100 kHz.
// The compensation node is a pin so the testbench can preset it.
module OpAmp (input electrical inp, input electrical inn,
              inout electrical pole, inout electrical out, inout electrical gnd);
    parameter real gm = 1e-3;
    parameter real rp = 1e7;
    parameter real cp = 1.5915e-9;
    analog begin
        I(pole, gnd) <+ -gm * (V(inp, gnd) - V(inn, gnd));
        I(pole, gnd) <+ V(pole, gnd) / rp;
        I(pole, gnd) <+ cp * ddt(V(pole, gnd));
        V(out, gnd) <+ V(pole, gnd);
    end
endmodule

module Follower ();
    electrical gnd;
    electrical vin;
    electrical comp;
    electrical out;
    VoltageSource #(.voltage(1.0)) src (.p(vin), .n(gnd));
    OpAmp amp (.inp(vin), .inn(out), .pole(comp), .out(out), .gnd(gnd));
endmodule
```

### A.2.4 D flip-flop chain (register state held across module boundaries)

A shift register built from four separate `Dff` module instances, clocked by
an analog square wave crossing the A2D bridge — `examples/21_shift_register_tran.vams`
(excerpt; the full file also has the clock/high-rail sources feeding the
comparators):

```verilogams
// Rising-edge D flip-flop (canonical 1364 idiom: no intermediate state, the
// new q is latched directly from d at each posedge).
module Dff (input logic clk, input logic d, output logic q);
    always @(posedge clk) q <= d;
endmodule

module ShiftReg4 (input logic clk, input logic din,
                  output logic q0, output logic q1, output logic q2, output logic q3);
    Dff f0 (.clk(clk), .d(din), .q(q0));
    Dff f1 (.clk(clk), .d(q0),  .q(q1));
    Dff f2 (.clk(clk), .d(q1),  .q(q2));
    Dff f3 (.clk(clk), .d(q2),  .q(q3));
endmodule
```

### A.2.5 Ripple-carry adder (structural composition across module boundaries)

Four 1-bit `FullAdder` instances chained by their carry — the thing that
matters across module boundaries: the carry propagates `fa0 -> fa1 -> fa2 ->
fa3`, and this is also the correctness oracle for the fused digital-network
JIT (`piperine-codegen/docs/DIGITAL_JIT.md`). `examples/17_ripple_adder_4bit.vams`,
excerpted (the full file also declares the `BitDriver` stageable inputs):

```verilogams
// 1-bit full adder: sum = a XOR b XOR cin, cout = majority(a, b, cin).
module FullAdder (input logic a, input logic b, input logic cin,
                  output logic sum, output logic cout);
    always @(*) begin
        sum <= (a != b) != cin;
        cout <= (a && b) || (cin && (a != b));
    end
endmodule

module Adder4 (input logic a0, a1, a2, a3, input logic b0, b1, b2, b3,
              output logic s0, s1, s2, s3, output logic cout);
    logic zero, c0, c1, c2;
    FullAdder fa0 (.a(a0), .b(b0), .cin(zero), .sum(s0), .cout(c0));
    FullAdder fa1 (.a(a1), .b(b1), .cin(c0),  .sum(s1), .cout(c1));
    FullAdder fa2 (.a(a2), .b(b2), .cin(c1),  .sum(s2), .cout(c2));
    FullAdder fa3 (.a(a3), .b(b3), .cin(c2),  .sum(s3), .cout(cout));
endmodule
```

### A.2.6 Structural generation (`generate`/`genvar`)

A `generate for` loop instantiates a bank of matched resistors as the bottom
leg of a divider — `examples/11_resistor_string.vams`, verbatim (this is the
real replacement for an earlier PHDL-era draft's `mod Ladder[N]` generic —
Verilog-AMS expresses repeated structure with `generate`/`genvar` over a
`parameter integer`, not a generic module):

```verilogams
`include "disciplines.vams"

module ParallelBank ();
    electrical gnd;
    electrical top;
    electrical mid;
    genvar i;
    VoltageSource src (.p(top), .n(gnd));
    Resistor r_top (.p(top), .n(mid));
    generate
        for (i = 0; i < 4; i = i + 1) begin : bank
            Resistor r (.p(mid), .n(gnd));
        end
    endgenerate
endmodule
```

### A.2.7 Mixed-signal transient: both bridges in one circuit

A ramp voltage source drives a 3-comparator level ADC; the digital block reads
the analog ramp through the A2D bridge and the analog block reads the digital
bits back through the D2A bridge, reconstructing a monotonic staircase — one
circuit, both bridges, both directions. `examples/23_mixed_signal_adc.vams`,
verbatim:

```verilogams
`include "disciplines.vams"

module RampSource (inout electrical p, inout electrical n);
    parameter real slope = 4.0e5;   // V/s  (0.4 V/us -> 0..4 V over 10 us)
    analog begin
        V(p, n) <+ slope * $abstime;
    end
endmodule

module LevelAdc (input electrical vin, inout electrical gnd,
                 output logic d2, output logic d1, output logic d0,
                 inout electrical rec);
    real v;
    always @(*) begin
        d0 = V(vin, gnd) > 1.0;
        d1 = V(vin, gnd) > 2.0;
        d2 = V(vin, gnd) > 3.0;
    end
    analog begin
        v = 0.0;
        if (d0 == 1) v = v + 1.0;
        if (d1 == 1) v = v + 1.0;
        if (d2 == 1) v = v + 1.0;
        V(rec, gnd) <+ v;
    end
endmodule

module AdcDemo ();
    electrical gnd;
    electrical vin;
    electrical rec;
    logic d2, d1, d0;
    RampSource #(.slope(4.0e5)) src (.p(vin), .n(gnd));
    LevelAdc adc (.vin(vin), .gnd(gnd), .d2(d2), .d1(d1), .d0(d0), .rec(rec));
endmodule
```

## A.3 Worked testbenches (host API)

There is no in-language bench block — testbenches are Python files (Part
VIII). One runnable twin exists per `.vams` circuit in `examples/`; a minimal
one:

```python
# 01_voltage_divider.py (excerpt)
import piperine

design = piperine.load("01_voltage_divider.vams")
board = design.module("DividerBoard")
op = board.op()
assert abs(op.v("mid") - 2.0) < 1e-6, "divider ratio is R2/(R1+R2)"
```

Run with `piperine test` (discovers `*_tb.py`/the example gallery's own
Python twins) or `piperine run 01_voltage_divider.py`.

## A.4 Host devices and testers (dv-tester)

The host device library (Part VII §20, Part VIII §17) writes a device *in*
the host language rather than in Verilog-AMS. Both examples below reflect the
actually-delivered surface — `crates/piperine-api/src/device/`,
`crates/piperine-python/src/sequencer_bridge.rs`/`signal_bridge.rs` — not
`ideal.md`'s aspirational sketches (`t.arm`/`t.install`/`b.vsource` are not
literally what shipped; see Part VIII §17.5's stated Rust/Python
asymmetry).

### A.4.1 A host device — Rust, level 0 + level 1 composed

The declaration is ordinary Verilog-AMS — a `(* device = …, type = … *)`
attribute-instance module with a plugin binding and a host registration both
able to satisfy it (Part I §10.7):

```verilogams
`include "disciplines.vams"

(* device = "none", type = "SoftStartLoad" *)
module SoftStartLoad (inout electrical p, inout electrical n);
    parameter real r = 1e3;
endmodule
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

`SessionBuilder::host_registry(Rc::new(registry))` (Appendix C §2) wires it; the
same `device` attribute would resolve to a plugin binding instead if one
were wired with `SessionBuilder::provider(...)` — the Verilog-AMS source
never changes between the two modes (Part I §10.7's graduation path).

### A.4.2 A tester — Python `Sequencer`

A `pip.Sequencer` subclass is the coroutine-shaped tester itself —
`run(self, t)` is a generator, `yield t.advance(dt)`/`yield
t.wait(pip.edge(...))` are the wake forms, `t.expect`/`t.warn` report
findings. `@pip.device(type_id)` declares which `(* device = "none", type =
type_id *)` instance it binds to, exactly as a plain host device does (Part I
§10.7):

```python
# regulator_tb.py
import piperine as pip

REGULATOR_VAMS = """
`include "disciplines.vams"

module VSource (inout electrical p, inout electrical n);
    parameter real dc = 0.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule

module Regulator (inout electrical vin, inout electrical vout, inout electrical gnd);
    analog begin
        V(vout, gnd) <+ min(V(vin, gnd), 1.8);
    end
endmodule

(* device = "none", type = "RegulatorTest" *)
module RegulatorTest (inout electrical vout, inout electrical gnd);
endmodule

module Top ();
    electrical vin, vout, gnd;
    VSource #(.dc(3.3)) src (.p(vin), .n(gnd));
    Regulator reg (.vin(vin), .vout(vout), .gnd(gnd));
    RegulatorTest tst (.vout(vout), .gnd(gnd));
endmodule
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

# Appendix A — Worked Examples

Each example stresses a corner of the model. Together they cover analog, digital,
mixed-signal, and the interpreted context.

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
    for i in 0..N { if (code[i] == 1) { acc = acc + vref * pow(2.0, real(i)) / pow(2.0, real(N)); } }
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

## A.3 Worked benches (interpreted context)

### A.3.1 Open-circuit test

DC operating-point test: the switch is open, so current should be near zero. Then the
switch is closed (override staged) and current should flow.

```phdl
mod SwitchOpenTest() {
    wire signal : Electrical;  wire gnd : Ground;
    sw     : Switch ( /* ctrl, p, n */ );
    source : VoltageSource ( /* p, n */ ) { .dc = 5.0 };
    resistor : Resistor ( /* p, n */ );
}
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(source.p, gnd) > 4.9, "source must be near 5V");
        $assert(r.i(resistor.p, resistor.n) < 1e-8, "open switch => ~0 current");
    }
    fn test_closed_circuit() {
        sw.ctrl = 1.0;
        var r = $op();
        $assert(r.i(resistor.p, resistor.n) > 4e-6, "closed switch => current flows");
    }
}
```

### A.3.2 Transient with a warm-corner config

Transient analysis at 358.15 K (85°C). The `solver` field of `TranConfig` carries the
temperature override.

```phdl
bench OscTest {
    fn test_frequency() {
        var r = $tran(TranConfig {
            .stop = 1e-3, .step = 1e-7,
            .solver = Solver { .temperature = 358.15 }
        });
        var out = r.v(out, gnd);
        $assert(out.peak_to_peak() > 1.0, "oscillation should start");
    }
}
```

### A.3.3 AC bandwidth

AC sweep from 1 Hz to 1 GHz, 100 points per decade. The result is a
`Waveform<Complex>`; `.db()` converts to dB, `.at(f)` reads at a specific frequency.

```phdl
bench FilterTest {
    fn test_bandwidth() {
        var r = $ac(AcConfig { .fstart = 1.0, .fstop = 1e9, .points = 100 });
        $assert(r.v(out).db().at(1e3) > -3.0, "passband flat at 1 kHz");
    }
}
```

### A.3.4 Sweep — a `for`, not a task

DC gain vs. load resistance, swept over four values. The curve is a `Vec<(Real, Real)>`
emitted as CSV.

```phdl
bench AmpSweep {
    fn dc_gain_vs_load() {
        var curve : Vec<(Real, Real)> = [];
        for rl in [1e3, 1e4, 1e5, 1e6] {
            load.resistance = rl;
            var r = $op();
            curve.push((rl, r.v(out) / r.v(in_)));
        }
        $write("gain_vs_load.csv", curve);
    }
}
```

### A.3.5 Closure loop (tune_bias)

Measure → adjust → re-run, in a bounded loop. If the bias converges (error < 1mV), the
fn returns; otherwise it errors after 20 iterations.

```phdl
bench BiasTrim {
    fn tune_bias() {
        for i in 0..20 {
            var r = $op();
            var err = r.v(out) - 1.0;
            if (abs(err) < 1e-3) { return; }
            bias.trim = bias.trim - 0.1 * err;
        }
        $error("bias did not converge");
    }
}
```

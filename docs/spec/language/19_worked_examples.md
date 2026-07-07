## 12. Worked examples

**12.1 Open-circuit test.**

```phdl
mod SwitchOpenTest() {
    wire gnd : Electrical;
    wire signal : Electrical;
    wire vsrc : Electrical;
    sw       : Switch        ( .a = signal, .b = gnd ) { .ctrl = 0.0 };
    source   : VoltageSource ( .p = vsrc, .n = gnd ) { .voltage = 5.0 };
    resistor : Resistor      ( .p = vsrc, .n = signal ) { .resistance = 1e6 };
}
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(vsrc, gnd) > 4.9, "voltage source should be active");
        $assert(r.i(resistor.p, resistor.n) < 1e-8, "no current with the switch open");
    }
    fn test_closed_circuit() {
        sw.ctrl = 1.0;
        var r = $op();
        $assert(r.i(resistor.p, resistor.n) > 4e-6, "current should flow when closed");
    }
}
```

(Ports bind positionally/named in the instance's `(...)` list; params bind in a trailing `{...}`
block — Part I §7.3. A `wire` declares one net per statement; there is no comma-separated form.
Numeric comparisons use a tolerance, not exact equality, following a solved `Real` — SPEC
Part I's `!=`/`==` are exact, and `Real`-valued voltages are never exactly a target value.)

**12.2 Transient with a warm-corner config.**

```phdl
bench OscTest {
    fn test_frequency() {
        var r = $tran(TranConfig { .stop = 1e-3, .step = 1e-7,
                                   .solver = Solver { .temperature = 358.15 } });
        var out = r.v(out, gnd);
        $assert(out.peak_to_peak() > 1.0, "oscillation should start");
    }
}
```

**12.3 AC bandwidth via a library FFT-free magnitude read.**

```phdl
bench FilterTest {
    fn test_bandwidth() {
        var r = $ac(AcConfig { .fstart = 1.0, .fstop = 1e9, .points = 100 });
        $assert(r.v(out).db().at(1e3) > -3.0, "passband flat at 1 kHz");
    }
}
```

**12.4 Sweep — a `for`, not a task.**

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

---


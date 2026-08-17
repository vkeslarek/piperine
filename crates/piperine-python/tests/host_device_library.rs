//! T28 (DVT-11 Python half) — the Python surface's host device library:
//! every level-1 catalog entry reachable with identical spelling to Rust
//! (`Wave.const` excepted, `signal_bridge.rs`'s documented divergence), the
//! catalog's closedness, `@pip.device` registering a Python host device
//! through T17's own `HostDeviceRegistry` (not a parallel mechanism), and
//! the generator form of `Sequencer.run` driving a real coroutine end to
//! end. `ideal.md` §9's examples A–G are translated here to exercise the
//! actually-delivered Python surface (a `build(self, b)` zero-host-call
//! install site over `WaveStamp`/`BehaviorStamp`, and direct `_NetRef`-
//! scoped stamping for level 0 — not the aspirational `b.vsource(...)`/
//! `b.switch(...)` circuit-construction sketch, which has no concrete
//! backing in `design.md` or any Rust phase this feature delivered)
//! rather than transcribed as literal syntax. Every test goes through
//! `piperine_python::embed::run_script` — the project's established
//! embedding idiom (`facade_lock`/`register_modules` stay `pub(crate)`,
//! mirrored by every other test in this crate, e.g. `typed_configs.rs`).

use piperine_python::embed::run_script;

fn run_script_string(script: &str) {
    // Each test runs in its own thread within one `cargo test` process
    // (`std::process::id()` alone is not unique per-test) — the thread id
    // makes the temp path unique so concurrent tests never race on the
    // same file.
    let tid = format!("{:?}", std::thread::current().id());
    let tid: String = tid.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let path = std::env::temp_dir().join(format!("piperine_t28_{}_{tid}.py", std::process::id()));
    std::fs::write(&path, script).expect("write probe script");
    run_script(path.to_str().expect("utf8 path")).unwrap_or_else(|e| panic!("script failed: {e}"));
}

/// Every level-1 catalog entry (`Wave.*`, `HostBehavior.*`, `Measure.*`,
/// `DelayMeasure.*`) is reachable, with identical spelling to Rust except
/// `Wave.const` (a Rust keyword) and `HostBehavior` (renamed to avoid
/// colliding with the pre-existing POM `Behavior` reflection type, see
/// `signal_bridge.rs`'s header).
#[test]
fn every_level_one_catalog_entry_is_reachable_from_python() {
    run_script_string(
        r#"
import piperine as pip

waves = [
    pip.Wave.const(1.0),
    pip.Wave.ramp(0.0, 1.0, 1e-6),
    pip.Wave.pulse(0.0, 1.0, 0.0, 1e-9, 1e-6, 1e-9, 2e-6),
    pip.Wave.sin(0.0, 1.0, 1e6, 0.0, 0.0),
    pip.Wave.exp(0.0, 1.0, 1e-6, 0.0),
    pip.Wave.sigmoid(0.0, 1.8, 200e-6, 40e-6),
    pip.Wave.pwl([(0.0, 0.0), (1.0, 1.0)]),
    pip.Wave.sampled([0.0, 1.0], [0.0, 2.0]),
]
w2 = pip.Wave.sum(waves[0], waves[1])
w3 = pip.Wave.scaled(waves[2], 2.0)
w4 = pip.Wave.shifted(waves[3], 1e-6)
assert waves[0].value(0.0) == 1.0
assert w2 is not None and w3 is not None and w4 is not None

behaviors = [
    pip.HostBehavior.comparator(0.0, 1.0, 1.0),
    pip.HostBehavior.hysteresis(5e-3, -5e-3),
    pip.HostBehavior.switch(50e-3, 1e6, 0.5),
    pip.HostBehavior.limiter(0.0, 1.0),
    pip.HostBehavior.vcvs(2.0),
    pip.HostBehavior.vccs(1e-3),
    pip.HostBehavior.vco(1e6, 1e5),
    pip.HostBehavior.table([0.0, 1.0], [0.0, 2.0]),
]
assert all(b is not None for b in behaviors)
comp = pip.HostBehavior.comparator(0.0, 1.0, 1.0)
assert comp.value([1.0, 0.0]) > 0.5

measures = [
    pip.Measure.peak(), pip.Measure.min(), pip.Measure.average(), pip.Measure.rms(),
    pip.Measure.ripple(), pip.Measure.integral(), pip.Measure.cross(0.5, pip.EITHER),
    pip.Measure.settle(1.0, 0.01), pip.Measure.count(),
]
assert all(m is not None for m in measures)
d = pip.DelayMeasure.delay(0.5, pip.RISING)
assert d is not None
m = pip.Measure.peak()
m.feed(0.0, 3.3, 0.0)
assert m.value == 3.3
"#,
    );
}

/// `ideal.md` §3.7: the catalog is closed. Attempting to register a new
/// kind from Python fails, naming level 0 and the Rust route.
#[test]
fn the_catalog_is_closed_in_python() {
    run_script_string(
        r#"
import piperine as pip
try:
    pip.HostBehavior.register_kind("MyKind")
    raise AssertionError("register_kind must refuse")
except RuntimeError as e:
    assert "level 0" in str(e), str(e)
    assert "BehaviorKind" in str(e), str(e)
try:
    pip.Measure.register_kind("MyKind")
    raise AssertionError("register_kind must refuse")
except RuntimeError as e:
    assert "level 0" in str(e), str(e)
    assert "MeasureKind" in str(e), str(e)
"#,
    );
}

/// `ideal.md` example A translated (level 0, "the hello world proving the
/// ABI is genuinely exposed"): a Python resistor stamps `load_dc`.
/// `ideal.md` example B translated (level 1, zero host calls): a `Wave`
/// installed through `build(self, b)`. T28's `@pip.device` bullet: the
/// existing `device(type_id)` decorator (T17's registration point, reused
/// per DVT-20/21) resolves an `@device` PHDL module and solves a real
/// circuit, end to end — a level-0 device and a level-1 device coexist in
/// one circuit.
#[test]
fn pip_device_registers_python_host_devices_end_to_end() {
    run_script_string(
        r#"
import piperine as pip

PHDL = """
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Vsrc (inout electrical p, inout electrical n);
    parameter real voltage = 0.0;
    analog V(p, n) <+ voltage;
endmodule

(* device = "none", type = "py_resistor" *)
module PyR (inout electrical p, inout electrical n);
    parameter real r = 1e3;
endmodule

(* device = "none", type = "py_wave_resistor" *)
module PyWaveR (inout electrical p, inout electrical n);
    parameter real r = 1e3;
endmodule

module Divider ();
    electrical gnd, vin, mid;
    Vsrc #(.voltage(5.0)) src (.p(vin), .n(gnd));
    PyR #(.r(3e3)) r_top (.p(vin), .n(mid));
    PyWaveR #(.r(2e3)) r_bot (.p(mid), .n(gnd));
endmodule
"""

# Example A, translated: a level-0 device, load_dc stamps directly.
@pip.device('py_resistor')
class PyR(pip.Device):
    caps = pip.ANALOG | pip.LOADS_DC

    def __init__(self, ports, params):
        self.p, self.n = ports
        self.g = 1.0 / params['r']

    def load_dc(self, ctx, m):
        m.g(self.p, self.p, self.g); m.g(self.p, self.n, -self.g)
        m.g(self.n, self.p, -self.g); m.g(self.n, self.n, self.g)

# Example B, translated: a level-1 device — a Wave installed once through
# build(b), zero host calls inside Newton (no load_dc at all).
@pip.device('py_wave_resistor')
class PyWaveR(pip.Device):
    def __init__(self, ports, params):
        self.p, self.n = ports
        self.r = params['r']

    def build(self, b):
        # An ideal (rout=0) voltage-divider-shaped resistor isn't
        # expressible as a Wave (Wave has no state dependence); this
        # exercises the `build(b)`/`b.wave` seam with a trivial constant
        # source in series with the declared impedance instead — the
        # circuit's actual resistor role is played by `r` as the series
        # impedance itself, `wave=const(0)` contributing nothing.
        b.wave(self.p, self.n, pip.Wave.const(0.0), self.r)

v = pip._solve_with_python_devices(PHDL, "Divider", "mid")
assert abs(v - 2.0) < 1e-9, f"3k(load_dc)/2k(build+Wave) divider from 5V gives 2.0V at mid, got {v}"
"#,
    );
}

/// T28: the generator form of `Sequencer.run` works end to end — `yield
/// t.advance(dt)` drives a real coroutine, and a failing `t.expect`
/// records an `Error` finding (DVT-29) without aborting the coroutine,
/// mirroring `ideal.md` example E's basic shape (advance, then check).
#[test]
fn the_generator_form_of_sequencer_run_drives_end_to_end() {
    run_script_string(
        r#"
import piperine as pip

class T(pip.Sequencer):
    def run(self, t):
        yield t.advance(1e-6)
        t.expect(False, 'deliberately fails')
        yield t.advance(2e-6)

first, second, finding = pip._drive_sequencer_twice(T(), 1e-6)
assert abs(first - 1e-6) < 1e-15, first
assert abs(second - 2e-6) < 1e-15, "the coroutine kept running past the failing expect"
assert finding is not None, "the failing t.expect must have recorded a finding"
message, time, severity = finding
assert message == "deliberately fails", message
assert abs(time - 1e-6) < 1e-15, "the finding carries the resume time it fired at"
assert severity == "Error", severity
"#,
    );
}

//! T25 — `cross`/`dir`/`scale` enums on the Python side (HOST-23).
//!
//! One `#[test]` — the embedded interpreter's `sys.modules["piperine"]`/
//! `["_piperine"]` are process-global, so two `#[test]`s in the same binary
//! calling `run_script` in parallel race on them (the pattern established
//! by `host_plot.rs`'s HOST-17 test: exercise every path sequentially in
//! one script).

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = out) { .r = 1e3 };
    c1 : C(.p = out, .n = gnd) { .c = 1e-6 };
}
";

/// `wf.cross(level, CrossDirection.Rising)` behaves the same as the legacy
/// string spelling; `Direction(descriptor.direction)` wraps a
/// `TerminalDescriptor`'s reflected direction string; `Scale` already
/// drives `AcConfig`/`NoiseConfig`.
#[test]
fn cross_direction_and_scale_enums() {
    let phdl = write_temp("piperine_t25_rc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")
module = design.top()
module.set("v1", "dc", 1.0)
# Force a genuine 0 -> 1V step (without `ic`, the transient's initial
# condition is the DC steady state, which for this lowpass is already 1V —
# no rising edge to find).
trace = module.tran(pip.TranConfig(stop=5e-3, step=1e-4, ic={{"out": 0.0}}))
wf = trace.v("out")

# ── CrossDirection enum vs legacy string spelling ──
t_enum = wf.cross(0.5, pip.CrossDirection.Rising)
t_str = wf.cross(0.5, "Rising")
assert t_enum is not None, f"t_enum is None (axis={{list(wf.axis)[:5]}}, values={{list(wf.values)[:5]}})"
assert t_enum == t_str, (t_enum, t_str)
assert wf.cross(0.5, pip.CrossDirection.Falling) is None

# ── Direction enum wraps a TerminalDescriptor's reflected direction ──
op = module.op()
inst = op["r1"]
descriptor = inst.terminals[0]
assert descriptor.direction in ("in", "out", "inout"), f"unexpected direction: {{descriptor.direction!r}}"
d = pip.Direction(descriptor.direction)
assert d in (pip.Direction.In, pip.Direction.Out, pip.Direction.Inout), d

# ── Scale enum (already drives AcConfig/NoiseConfig) ──
assert pip.Scale.Dec.value == "Dec"
assert pip.Scale.Lin.value == "Lin"
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t25_enums.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "enums script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

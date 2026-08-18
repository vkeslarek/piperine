//! The Python margin + findings surface (dv-core T24, DVC-18): the
//! bindings delegate to `piperine_api` — `margins()`, `violations()`,
//! `requires_ok()`, `first_violation()` on every analysis result, the
//! `_MarginsResult` channel reads, and the loud failure modes (unknown
//! name → `KeyError` listing candidates). Values come from the same
//! solver the Rust host drives — nothing is recomputed in Python.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

fn run(name: &str, script: &str) {
    let script_path = write_temp(name, script);
    let result = run_script(&script_path);
    let _ = std::fs::remove_file(&script_path);
    assert!(result.is_ok(), "python script must pass: {:?}", result.err());
}

/// The divider: one violated `require` (margin −0.1).
/// Self-contained (inline natures/discipline) so it elaborates under
/// `SourceMap::dummy()` regardless of the runner's CWD.
const DIVIDER: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    parameter real acmag = 1.0;
    analog begin
        V(p, n) <+ dc + ac_stim(acmag, 0.0);
    end
endmodule

module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule

module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule

module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// DVC-18: the Python result objects read the margins channel and the
/// findings with the same values the solver recorded — the wrapper is a
/// thin forward over `piperine_api`, not a reimplementation.
#[test]
fn margins_and_violations_read_through_python() {
    let phdl = write_temp("piperine_dv_divider.vams", DIVIDER);
    let script = format!(
        r#"
import piperine

design = piperine.load({phdl:?})
session = design.compile()
solver = piperine.Solver(checks="collect")
op = session.op(piperine.OpConfig(solver=solver))

margins = op.margins()
assert len(margins) == 1 and not margins.is_empty()
entry = margins.get("head")
assert entry.label == "head"
assert entry.instance_path == "dut1"
assert entry.kind == "require"
assert entry.shape == "pointwise"
assert abs(entry.scalar() - (0.4 - 0.5)) < 1e-12, entry.scalar()
assert entry.argmin() == 0.0
assert not entry.is_satisfied()
assert margins.entries()[0].label == "head"

assert not op.requires_ok()
assert op.first_violation().label == "head"
assert margins.first_violation().label == "head"

v = op.violations()
assert len(v) == 1
assert v[0].severity == "error"
assert v[0].label == "head"
assert v[0].instance_path == "dut1"
assert abs(v[0].value - (0.4 - 0.5)) < 1e-12, v[0].value
assert v[0].time == 0.0

# The trace reads the same channel with time argmins.
trace = session.tran(piperine.TranConfig(stop=1e-6, step=1e-7, solver=solver))
tm = trace.margins().get("head")
assert tm.shape == "pointwise" and tm.argmin() is not None
assert not trace.requires_ok()
assert len(trace.violations()) > 0
assert all(f.severity == "error" and f.label == "head" for f in trace.violations())

# The AC trace carries the same four methods.
ac = session.ac(piperine.AcConfig(fstart=1.0, fstop=1e3, points=3, solver=solver))
assert ac.margins().get("head").shape == "pointwise"
assert not ac.requires_ok()
assert ac.first_violation().label == "head"
assert len(ac.violations()) > 0
"#,
        phdl = phdl
    );
    run("piperine_dv_margins_surface.py", &script);
    let _ = std::fs::remove_file(phdl);
}

/// DVC-18 AC3 + DVC-08 AC6: the loud reads fail identically through
/// Python — an unknown name is a `KeyError` listing the candidates; a
/// reduced `var` read outside AC is "not defined in this analysis" and
/// reads fine on the AC result itself.
#[test]
fn loud_margin_reads_fail_identically_through_python() {
    let phdl = write_temp("piperine_dv_divider2.vams", DIVIDER);
    let script = format!(
        r#"
import piperine

design = piperine.load({phdl:?})
session = design.compile()
solver = piperine.Solver(checks="collect")
op = session.op(piperine.OpConfig(solver=solver))

try:
    op.margins().get("hed")
    raise AssertionError("an unknown name must fail loud")
except KeyError as e:
    msg = str(e)
    assert "unknown constraint" in msg and "dut1.head" in msg, msg
"#,
        phdl = phdl
    );
    run("piperine_dv_loud_unknown.py", &script);
    let _ = std::fs::remove_file(phdl);
}

/// A gain stage with a reduced `var` — defined on AC results only.
/// Self-contained (inline natures/discipline) so it elaborates under
/// `SourceMap::dummy()` regardless of the runner's CWD.
const STAGE: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Vac (inout electrical p, inout electrical n);
    parameter real acmag = 1.0;
    analog begin
        V(p, n) <+ ac_stim(acmag, 0.0);
    end
endmodule

module Stage (inout electrical inp, inout electrical out, inout electrical gnd);
    parameter real gm = 0.1;
    parameter real r = 1e3;
    parameter real c = 1e-6;
    analog begin
        I(gnd, out) <+ gm * V(inp, gnd);
        I(out, gnd) <+ V(out, gnd) / r;
        I(out, gnd) <+ c * ddt(V(out, gnd));
    end
    constraint begin
        var ugbw = ac_unity_gain_freq(V(out, gnd));
    end
endmodule

module Top ();
    electrical gnd, in, out;
    Vac v1 (.p(in), .n(gnd));
    Stage x1 (.inp(in), .out(out), .gnd(gnd));
endmodule
"#;

/// DVC-08 AC6 / D7: the live Python session computes the reduced
/// reductions on AC (identical host pass the Rust session runs) and a
/// read from an analysis that does not define the `var` fails loud with
/// "not defined in this analysis".
#[test]
fn reduced_vars_are_defined_on_ac_only_through_python() {
    let phdl = write_temp("piperine_dv_stage.vams", STAGE);
    let script = format!(
        r#"
import piperine

design = piperine.load({phdl:?})
session = design.compile()
solver = piperine.Solver(checks="collect")
op = session.op(piperine.OpConfig(solver=solver))

try:
    op.margins().get("ugbw")
    raise AssertionError("a reduced var read outside AC must fail loud")
except RuntimeError as e:
    assert "not defined in this analysis" in str(e), str(e)

ac = session.ac(piperine.AcConfig(fstart=1.0, fstop=1e6, points=21, solver=solver))
ugbw = ac.margins().get("ugbw")
assert ugbw.kind == "var"
assert ugbw.shape == "reduced"
assert ugbw.argmin() is None, "a reduced margin has no argmin"
assert ugbw.scalar() > 1e4, ugbw.scalar()
"#,
        phdl = phdl
    );
    run("piperine_dv_reduced_vars.py", &script);
    let _ = std::fs::remove_file(phdl);
}

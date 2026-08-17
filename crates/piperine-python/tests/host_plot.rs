//! host-library T19 (HOST-17): `wf.plot()`/`pip.plot(...)`/`pip.bode(...)`,
//! matplotlib-guarded. One `#[test]` — `run_script` shares the
//! process-global interpreter (same convention as `live_facade.rs`), so
//! both the "matplotlib present" and "matplotlib absent" scenarios run
//! sequentially inside one script rather than racing across parallel tests.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

/// RC low-pass fixture: a DC step through `r1` into `c1`, with an AC
/// stimulus so `s.ac(...)` produces a `ComplexWaveform` for the Bode case.
const RC_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module V (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    parameter real acmag = 1.0;
    analog V(p, n) <+ dc + ac_stim(acmag, 0.0);
endmodule

module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module C (inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog I(p, n) <+ c * ddt(V(p, n));
endmodule

module Top ();
    electrical gnd, vin, out;
    V v1 (.p(vin), .n(gnd));
    R #(.r(1e3)) r1 (.p(vin), .n(out));
    C #(.c(1e-6)) c1 (.p(out), .n(gnd));
endmodule
"#;

/// HOST-17 AC4: with matplotlib present, `wf.plot()`/`pip.plot(waveform)`/
/// `pip.plot({label: waveform})`/`cw.plot()`/`pip.bode(cw)` all render (each
/// returns a `matplotlib.figure.Figure`, no exception); with matplotlib
/// forced absent (`sys.modules["matplotlib"] = None`, the documented CPython
/// halted-import mechanism), every entry point raises a clear `ImportError`
/// naming matplotlib and the install instruction — no hard dependency, no
/// silent no-op.
#[test]
fn plot_and_bode_render_with_matplotlib_and_fail_loud_without_it() {
    let phdl = write_temp("piperine_host_plot_rc.phdl", RC_PHDL);
    let script = format!(
        r#"import piperine

design = piperine.load("{phdl}")
s = design.compile()
trace = s.tran(piperine.TranConfig(stop=5e-3, step=1e-4))
wf = trace.v("out")
ac = s.ac(piperine.AcConfig(fstart=1.0, fstop=1e6, points=5))
cw = ac.v("out")

# ── matplotlib present: every entry point renders a Figure ──────────────
fig1 = wf.plot()
assert type(fig1).__name__ == "Figure", type(fig1).__name__

fig2 = piperine.plot(wf)
assert type(fig2).__name__ == "Figure", type(fig2).__name__

fig3 = piperine.plot({{"out": wf}})
assert type(fig3).__name__ == "Figure", type(fig3).__name__

fig4 = cw.plot()
assert type(fig4).__name__ == "Figure", type(fig4).__name__

fig5 = piperine.bode(cw)
assert type(fig5).__name__ == "Figure", type(fig5).__name__

# ── matplotlib absent: every entry point fails loud with an install hint ─
import sys
sys.modules["matplotlib"] = None
try:
    wf.plot()
    raise AssertionError("wf.plot() without matplotlib must raise ImportError")
except ImportError as e:
    msg = str(e).lower()
    assert "matplotlib" in msg and "install" in msg, msg

try:
    piperine.plot(wf)
    raise AssertionError("pip.plot() without matplotlib must raise ImportError")
except ImportError as e:
    msg = str(e).lower()
    assert "matplotlib" in msg and "install" in msg, msg

try:
    piperine.bode(cw)
    raise AssertionError("pip.bode() without matplotlib must raise ImportError")
except ImportError as e:
    msg = str(e).lower()
    assert "matplotlib" in msg and "install" in msg, msg

del sys.modules["matplotlib"]
"#
    );
    let script_path = write_temp("piperine_host_plot_script.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "plot/bode script must pass: {:?}", result.err());

    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

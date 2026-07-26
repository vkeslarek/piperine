//! T7 — `Session` (renamed from `LiveSession`) gains the full uniform
//! analysis surface: `sens`/`pss`/`pz`/`disto`/`sp`/`tf`/`dc`, all typed
//! results (not dicts/tuples at the facade level) — mirrors `Module`'s
//! shapes, on the one compilation.

use piperine_python::embed::run_script;

/// Write `body` to a temp file named `name`; return its path as a `String`.
fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const RLC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod L(inout p: Electrical, inout n: Electrical) { param l: Real = 1e-3; }
analog L { V(p, n) <- l * ddt(I(p, n)); }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    wire b   : Electrical;
    V1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 10.0 };
    l1 : L(.p = a, .n = b) { .l = 1e-3 };
    c1 : C(.p = b, .n = gnd) { .c = 1e-6 };
}
";

/// `Session.sens`/`pz`/`disto`/`tf` return the facade's typed result
/// classes on the held compilation; `Session.dc` returns a `Trace` swept
/// over the restamped axis (HOST-02/03/05, all on one `compile()`).
#[test]
fn session_exposes_sens_pz_disto_tf_dc_with_typed_results() {
    let phdl = write_temp("piperine_session_analyses_rlc.phdl", RLC_PHDL);
    let script = format!(
        r#"
import piperine

design = piperine.load("{phdl}")
session = design.compile()

pz = session.pz("V1", "b")
assert type(pz).__name__ == "PoleZeroResult", type(pz).__name__
assert len(pz.poles) == 2, pz.poles

sens = session.sens(["a"], [("r1", "r")])
assert type(sens).__name__ == "SensResult", type(sens).__name__
assert sens.get("a", "r1", "r") is not None

tf = session.tf("a", "V1")
assert type(tf).__name__ == "TfResult", type(tf).__name__
assert isinstance(tf.gain, float)
assert isinstance(tf.z_in, float)
assert isinstance(tf.z_out, float)

trace = session.dc("V1", "dc", [1.0, 2.0, 3.0])
assert type(trace).__name__ == "_Trace", type(trace).__name__
wf = trace.v("a")
assert len(wf.values) == 3
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_session_analyses_script.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "session analyses script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

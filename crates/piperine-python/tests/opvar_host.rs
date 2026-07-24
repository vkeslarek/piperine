//! HOST-07 (host-library T10): `op["x1"].opvar("gm")`/`.opvars()` on the
//! Python facade — the same introspection door `tests/opvar_host.rs` proves
//! on the Rust host, over the identical fixture shape (uniform surface,
//! MD-22).

use piperine_python::embed::run_script;

/// Divider whose resistors compute an opvar `g = 1/r` in their analog body
/// (mirrors the root `tests/opvar_host.rs` fixture and
/// `piperine-codegen/tests/opvar_bridge.rs`).
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
    var g : Real = 0.0;
}
analog Resistor {
    g = 1.0 / r;
    I(p, n) <+ g * V(p, n);
}

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

#[test]
fn opvar_and_opvars_are_readable_from_python() {
    let divider_path = write_temp("piperine_opvar_host_divider.phdl", DIVIDER_PHDL);
    let script = format!(
        r#"import piperine

design = piperine.load("{divider_path}")
divider = design.module("Divider")
op = divider.op()

view = op["r_top"]
g = view.opvar("g")
assert abs(g - 1.0 / 3e3) < 1e-9, g

vars_ = view.opvars()
assert len(vars_) == 1, vars_
assert vars_[0][0] == "g", vars_
assert abs(vars_[0][1] - 1.0 / 3e3) < 1e-9, vars_

# An unknown opvar fails loud, never None/NaN.
try:
    view.opvar("bogus")
    raise AssertionError("expected opvar('bogus') to raise")
except Exception as e:
    assert "bogus" in str(e), str(e)

# A device with no declared opvars (VoltageSource) returns an empty list.
src_view = op["src"]
assert src_view.opvars() == []
"#
    );
    let script_path = write_temp("piperine_opvar_host_script.py", &script);
    run_script(&script_path).expect("opvar host script runs");
}

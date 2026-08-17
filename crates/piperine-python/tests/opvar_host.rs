//! HOST-07 (host-library T10): `op["x1"].opvar("gm")`/`.opvars()` on the
//! Python facade — the same introspection door `tests/opvar_host.rs` proves
//! on the Rust host, over the identical fixture shape (uniform surface,
//! MD-22).

use piperine_python::embed::run_script;

/// Divider whose resistors compute an opvar `g = 1/r` in their analog body
/// (mirrors the root `tests/opvar_host.rs` fixture and
/// `piperine-codegen/tests/opvar_bridge.rs`).
const DIVIDER_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module VoltageSource (inout electrical p, inout electrical n);
    parameter real voltage = 0.0;
    analog V(p, n) <+ voltage;
endmodule

module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    real g;
    analog begin
        g = 1.0 / r;
        I(p, n) <+ g * V(p, n);
    end
endmodule

module Divider ();
    electrical gnd, vin, mid;
    VoltageSource #(.voltage(5.0)) src (.p(vin), .n(gnd));
    Resistor #(.r(3e3)) r_top (.p(vin), .n(mid));
    Resistor #(.r(2e3)) r_bot (.p(mid), .n(gnd));
endmodule
"#;

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

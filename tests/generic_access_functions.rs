//! LRM §4.4/§5.5.1: `potential`/`flow` are the generic access functions —
//! always potential/flow respectively, regardless of which discipline is
//! in scope — and a custom discipline's own declared `access` name (e.g.
//! `thermal`'s `Temp`/`Pwr`, §3.6.1.2) resolves the same way.
//!
//! SPEC_DEVIATION.md 2.4 (pre-fix): `flatten/analog.rs`'s destination
//! classifier only recognized the literal `"V"` as a potential source;
//! every other access-function name (`"potential"` included) fell through
//! to `Flow` — `potential(p, gnd) <+ 5.0;` silently became a phantom
//! current source. A custom discipline's `access` name (parsed as a plain
//! `Call`, not the dedicated `AccessFunction` AST shape `V`/`I`/
//! `potential`/`flow` get) was not recognized as a contribution
//! destination at all — a hard `CodegenError`, not a wrong value.
//!
//! LRM-COVERS: 4.4, 5.5.1

use std::path::PathBuf;

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.vams"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A tiny series resistor to `gnd` so a `Flow` (current) source produces
/// an observably different voltage than a `Potential` (voltage) source
/// would — the discriminating setup spec AC10 calls for. Source and load
/// are separate instances (separate implicit branches) so a `Potential`
/// source never conflicts with the load's own `Flow` contribution on the
/// same branch (LRM §5.4.2).
const RESISTOR_LOAD_TEMPLATE: &str = "\
module Src(inout electrical p, inout electrical gnd);
    analog {{FN}}(p, gnd) <+ {{VAL}};
endmodule

module Load(inout electrical p, inout electrical gnd);
    parameter real r = 1e-3;
    analog I(p, gnd) <+ V(p, gnd) / r;
endmodule

module Top();
    electrical p, gnd;
    Src  s (p, gnd);
    Load l (p, gnd);
endmodule
";

fn solve(fn_name: &str, val: f64) -> f64 {
    let src = RESISTOR_LOAD_TEMPLATE.replace("{{FN}}", fn_name).replace("{{VAL}}", &val.to_string());
    let design = piperine_lang::parse_and_elaborate(&src, &headers_source_map())
        .unwrap_or_else(|e| panic!("{fn_name} circuit elaborates: {e}"));
    let mut session = Session::compile(&design, "Top").unwrap_or_else(|e| panic!("{fn_name} circuit compiles: {e}"));
    let op = session.op(&SolverConfig::default(), None).unwrap_or_else(|e| panic!("{fn_name} circuit solves: {e}"));
    op.v("p").expect("v(p)")
}

/// spec AC10: `potential(p, gnd) <+ 5.0;` produces a 5 V source at the OP
/// — a current source into a 1 mΩ load would read ~5 mV instead, so this
/// discriminates the pre-fix bug directly.
#[test]
fn potential_access_function_produces_a_voltage_source() {
    let v = solve("potential", 5.0);
    // Solver reltol/vntol margin on an ideal source, not a residual bug —
    // still two orders of magnitude tighter than the ~5 mV a phantom
    // current source into the 1 mΩ load would read (the discriminator).
    assert!((v - 5.0).abs() < 1e-2, "potential(p, gnd) <+ 5.0 must force V(p) = 5.0, got {v}");
}

/// spec AC10: `flow(p, gnd) <+ 1.0;` produces 1 A — through the 1 mΩ load,
/// KCL at `p` (source pushes 1 A out of `p`, the resistor must source it
/// back in) gives V(p) = -I·R = -1.0 · 1e-3.
#[test]
fn flow_access_function_produces_a_current_source() {
    let v = solve("flow", 1.0);
    let expected = -1.0 * 1e-3;
    assert!((v - expected).abs() < 1e-9, "flow(p, gnd) <+ 1.0 must force I(p) = 1.0 (V(p) = {expected}), got {v}");
}

/// wave-1 regression (LRMC-C1-08/09): a custom discipline's own declared
/// `access` name still resolves — `thermal`'s potential nature declares
/// `access = Temp` (`headers/disciplines.vams`). Pre-fix this was a hard
/// `CodegenError` (not an access function), not just a wrong value.
#[test]
fn custom_discipline_access_name_still_resolves() {
    // headers/disciplines.vams's own `thermal` discipline (Annex D.1),
    // declared inline here so the test doesn't depend on how the fixture's
    // SourceMap resolves `\`include`.
    let src = "\
nature Temperature;
    units = \"K\";
    abstol = 1e-4;
    access = Temp;
endnature

nature Power;
    units = \"W\";
    abstol = 1e-9;
    access = Pwr;
endnature

discipline thermal;
    potential Temperature;
    flow Power;
enddiscipline

module Src(inout thermal p, inout thermal gnd);
    analog Temp(p, gnd) <+ 300.0;
endmodule

module Top();
    thermal p, gnd;
    Src  s (p, gnd);
endmodule
";
    let design = piperine_lang::parse_and_elaborate(src, &headers_source_map())
        .expect("thermal circuit elaborates");
    let mut session = Session::compile(&design, "Top").expect("thermal circuit compiles");
    let op = session.op(&SolverConfig::default(), None).expect("thermal circuit solves");
    let v = op.v("p").expect("v(p)");
    assert!((v - 300.0).abs() < 1.0, "Temp(p, gnd) <+ 300.0 must force the potential to 300.0, got {v}");
}

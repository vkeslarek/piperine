//! §6.4.3 paramset output variables (T28, LRMW3-19) — the compiled-device
//! half. `crates/piperine-lang/tests/paramset_output_variables.rs` proves
//! the POM half (`Instance::paramset_output_vars`); this file proves the
//! formula is actually *evaluated* against the compiled device's own live
//! opvar readings, through the same introspection path
//! `circuit_from_design.rs`'s `jit_device_exposes_params_through_
//! introspection` already exercises for ordinary params.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::Design;
use piperine_codegen::resolve::LoweredBody;
use piperine_solver::prelude::CircuitInstance;
use piperine_codegen::CircuitCompiler;

fn from_ir(design: &Design, bodies: &HashMap<String, LoweredBody>, top: &str) -> Result<CircuitInstance, String> {
    let mut c = CircuitCompiler::new(design, bodies);
    c.build_circuit(top).map_err(|e| e.to_string())
}

const NPN: &str = "
    module npn (inout electrical c, inout electrical b, inout electrical e);
        parameter real is = 1.0e-16;
        real gm;
        real cpi;
        real cmu;
        analog begin
            gm = is * 2.0;
            cpi = 1.0e-12;
            cmu = 1.0e-13;
            I(c, e) <+ gm * V(b, e);
        end
    endmodule
";

/// AC1/AC4 (T28 Done-when #1/#4): a described paramset output variable's
/// formula, referencing module output variables via `.gm`/`.cpi`/`.cmu`,
/// evaluates to the LRM's own worked-example computation
/// (`ft = gm / (2*pi*(cpi + cmu))`) against the compiled device's actual
/// opvar values — not merely stored, but computable.
#[test]
fn described_paramset_variable_evaluates_against_live_opvars() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            (* desc=\"cut-off frequency\" *) real ft;
            ft = .gm/(2*3.14159265*(.cpi + .cmu));
        endparamset
        module top (inout electrical c, inout electrical b, inout electrical e);
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("parse+elab");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut ci = from_ir(&design, &bodies, "top").expect("from_ir compiles top");
    let dev = &mut ci.all_devices_mut()[0];

    let opvars: HashMap<String, f64> = dev.read_opvars().into_iter().collect();
    let gm = *opvars.get("gm").expect("gm reported");
    let cpi = *opvars.get("cpi").expect("cpi reported");
    let cmu = *opvars.get("cmu").expect("cmu reported");
    let expected = gm / (2.0 * std::f64::consts::PI * (cpi + cmu));

    let ft = *opvars.get("ft").expect("ft reported (paramset output variable)");
    assert!((ft - expected).abs() < 1e-6 * expected.abs().max(1.0), "ft={ft}, expected={expected}");

    // AC2: `ft` replaces npn's own report — exactly one entry, and it
    // also appears in the declared query catalog.
    assert_eq!(dev.read_opvars().iter().filter(|(n, _)| n == "ft").count(), 1);
    assert!(dev.list_queries().iter().any(|q| q.name == "ft"), "ft listed in the query catalog");
}

/// AC3 (T28 Done-when #3): an undescribed paramset variable hides the
/// module's own same-named output variable — `npn` here declares
/// `(* desc="transit frequency" *) real ft;` (so it would otherwise be
/// reported), but the paramset's bare `real ft;` removes it from both
/// `read_opvars` and `list_queries` without replacing it.
#[test]
fn undescribed_paramset_variable_hides_the_reported_value() {
    let npn_with_ft = "
        module npn2 (inout electrical c, inout electrical b, inout electrical e);
            parameter real is = 1.0e-16;
            (* desc=\"transit frequency\" *) real ft;
            analog begin
                ft = 1.0e9;
                I(c, e) <+ is * V(b, e);
            end
        endmodule
    ";
    let src = format!(
        "{npn_with_ft}
        paramset smnpn2 (npn2);
            real ft;
        endparamset
        module top (inout electrical c, inout electrical b, inout electrical e);
            smnpn2 q1 (c, b, e);
        endmodule
        "
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("parse+elab");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut ci = from_ir(&design, &bodies, "top").expect("from_ir compiles top");
    let dev = &mut ci.all_devices_mut()[0];

    assert!(
        !dev.read_opvars().iter().any(|(n, _)| n == "ft"),
        "ft must not be reported once an undescribed paramset variable hides it"
    );
    assert!(
        !dev.list_queries().iter().any(|q| q.name == "ft"),
        "ft must not appear in the query catalog either"
    );
}

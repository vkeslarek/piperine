//! T22 addendum (module structure lowering, VAMS-04): module-level `var`
//! declarations (LRM Annex A.2.1.3 `module_or_generate_item_declaration`'s
//! `integer_declaration`/`real_declaration`/`time_declaration`/
//! `realtime_declaration` alternatives) lower into `Module::vars()`.
//!
//! Before this fix, `elab/lower/vams.rs`'s `ModuleItem::Var(_) => {}` arm
//! silently skipped every module-level `var` — no `.vams` source could
//! declare working register state (T33's progress-log entry). Fixing the
//! elaboration alone was not sufficient: `piperine-vams`'s own
//! `parse_module_item` never dispatched `integer`/`real`/`time`/`realtime`
//! into `ModuleItem::Var` either (it fell through to the net-declaration
//! fallback, misreading the keyword as a discipline name) — both gaps are
//! fixed together here.

use piperine_lang::pom::ValueType;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// A minimal digital discipline (LRM §3.6.2, `domain discrete`) so a
/// digital port declares its discipline explicitly rather than relying on
/// Annex F connectivity resolution, which is orthogonal to this file's
/// concern (var lowering).
const LOGIC: &str = "
    discipline logic;
        domain discrete;
    enddiscipline
";

#[test]
fn integer_var_lowers_into_module_vars() {
    let d = elab(&format!(
        "{LOGIC}
        module counter (input logic clk, output logic q);
            integer count;
        endmodule
        "
    ));
    let m = d.module("counter").expect("counter elaborated");
    assert_eq!(m.vars().len(), 1);
    let v = m.vars().iter().find(|v| v.name() == "count").expect("var count");
    assert_eq!(*v.value_type(), ValueType::Integer);
    // VAMS's own `VarDecl` carries no initializer expression (unlike
    // `parameter`, which is a mandatory default) — see the
    // `var_declarator_to_var` doc comment in `elab/lower/vams.rs`.
    assert!(v.init().is_none());
}

#[test]
fn real_var_lowers_with_real_type() {
    let d = elab(&format!(
        "{LOGIC}
        module accum (input logic clk);
            real total;
        endmodule
        "
    ));
    let m = d.module("accum").expect("accum elaborated");
    let v = m.vars().iter().find(|v| v.name() == "total").expect("var total");
    assert_eq!(*v.value_type(), ValueType::Real);
}

#[test]
fn multiple_names_in_one_var_declaration_all_lower() {
    let d = elab(&format!(
        "{LOGIC}
        module m2 (input logic clk);
            integer a, b, c;
        endmodule
        "
    ));
    let m = d.module("m2").expect("m2 elaborated");
    assert_eq!(m.vars().len(), 3);
    for name in ["a", "b", "c"] {
        assert!(m.vars().iter().any(|v| v.name() == name), "missing var {name}");
    }
}

#[test]
fn digital_register_updated_in_clocked_block_lowers_end_to_end() {
    // The VAMS-06 vertical-slice shape: a `var` written inside a clocked
    // `always @(posedge clk)` block — the digital-counter half T33 needed.
    let d = elab(&format!(
        "{LOGIC}
        module counter (input logic clk, output logic q);
            integer count;
            always @(posedge clk) count = count + 1;
        endmodule
        "
    ));
    let m = d.module("counter").expect("counter elaborated");
    assert_eq!(m.vars().len(), 1);
    assert_eq!(m.vars()[0].name(), "count");
    assert_eq!(m.behaviors().len(), 1);
}

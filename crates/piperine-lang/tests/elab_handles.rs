//! Scope: handle functions at elaboration (generics GN-11, GN-12, GN-29, part I §3.1.5): a call of a function with a
//! handle parameter is inlined where it stands, so no IR function exists for it and its body is written out over the
//! carrier or node the caller passed.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::Ir;
use support::Elab;

/// The IR of a design whose `analog` block of `Top` is `statement`, after `items`.
struct HandleDesigns;

impl HandleDesigns {
    /// The elaborated `Top`, which must pass `Ir::check`.
    fn analog(items: &str, statement: &str) -> Ir {
        Elab::ir(&format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; signal t: Thermal; }}\nanalog for Top {{ {statement} }}\n"), "Top")
    }
}

#[test]
fn a_handle_call_lowers_to_its_written_out_body() {
    // GN-11 AC 1, GN-29 / D11: a handle function is no IR function; the call is its body over the node passed: the
    // probe is a probe of `a`, and `diff(a, gnd)` of `through` reads the same through two levels of handles
    let items = "fn level(x: Electrical) -> f64 { v(x) }\nfn through(x: Electrical) -> f64 { level(x) * 2.0 }\nimpl Thermal { fn celsius(self) -> f64 { temp(self) - 273.15 } }\nfn generic<N: Nature>(n: N) -> f64 { potential(n) }";
    let ir = HandleDesigns::analog(items, "v(o) +> through(a) + t.celsius() + generic(a) + generic(t);");
    assert_eq!(ir.functions().count(), 0, "no IR function for a handle function: {:?}", ir.functions().map(|f| f.name().to_string()).collect::<Vec<_>>());
    let text = ir.to_string();
    assert!(text.contains("probe"), "the body is written out: {text}");
}

#[test]
fn a_recursion_through_a_handle_function_is_e0126() {
    // GN-11 / D6: a handle function is inlined at its call, so a recursion through one cannot be expanded: `E0126` naming
    // the function, never an IR recursion
    let items = "fn again(x: Electrical, n: u8) -> f64 { if n == 0 { v(x) } else { again(x, n - 1) } }";
    let src = format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; }}\nanalog for Top {{ v(o) +> again(a, 3); }}\n");
    let found = Elab::run(&src, "Top").err().expect("refused");
    assert!(found.iter().any(|d| d.diagnostic.code == "E0126" && d.diagnostic.message.contains("`again`")), "{:?}", found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
}

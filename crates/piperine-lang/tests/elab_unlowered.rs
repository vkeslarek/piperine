//! Scope: the forms the type checker accepts (or refuses on its own) and M1 elaboration does not
//! lower (ELAB-06): each is `P0001` naming the form, at the span of the form, with no `Ir`.
//! One negative source per row of the table in `elab/unlowered.rs`.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_lang::elab::Elaborator;
use support::Elab;

/// One negative source: the form's name as the message must say it, and the text the span covers.
struct Row {
    form: &'static str,
    src: &'static str,
    names: &'static str,
    at: &'static str,
}

impl Row {
    fn check(&self) {
        let err = Elab::run(self.src, "Top").expect_err(self.form);
        assert_eq!(Elab::codes(&err), ["P0001"], "{}: {:?}", self.form, err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        let message = &err[0].diagnostic.message;
        assert!(message.contains(self.names), "{}: `{message}` does not name `{}`", self.form, self.names);
        let span = err[0].diagnostic.primary.0;
        assert_eq!(&self.src[span.start..span.end], self.at, "{}", self.form);
    }
}

#[test]
fn a_site_or_contribution_under_if_is_p0001() {
    // analog-structure T16 and T21 lifted every refusal this test asserted: a contribution under an
    // `if` or in a nested block (T16), a `limit` or `ac_stim` under an `if` or on the right of `&&` or
    // `||` (T21). Kept under its name; each form lowers, a guarded `limit` carrying its `enabled`
    // (AS-06 AC 1), and a `limit` or `ac_stim` that is not guarded carries none.
    for (form, body, enabled) in [
        ("limit under if", "let x = if v(p) > 0.0 { limit(v(p), |a, b| a) } else { 0.0 };", true),
        ("limit right of &&", "let c = v(p) > 0.0 && limit(v(p), |a, b| a) > 0.0;", true),
        ("limit right of ||", "let c = v(p) > 0.0 || limit(v(p), |a, b| a) > 0.0;", true),
        ("ac_stim under if", "let x = if v(p) > 0.0 { ac_stim(1.0) } else { 0.0 };", false),
        ("ac_stim right of &&", "let c = v(p) > 0.0 && ac_stim(1.0) > 0.0;", false),
        ("ac_stim right of ||", "let c = v(p) > 0.0 || ac_stim(1.0) > 0.0;", false),
    ] {
        let src = format!("module M(p: Electrical, n: Electrical) {{ }}\nanalog for M {{ {body} }}\nmodule Top() {{ signal s: Electrical; let m = M {{ }}(s, gnd); }}");
        let ir = Elab::ir(&src, "Top");
        let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert_eq!(m.limits().iter().all(|site| site.enabled.is_some()) && !m.limits().is_empty(), enabled, "{form}");
        assert_eq!(m.limits().len() + m.stims().len(), 1, "{form}: one site");
    }
}

#[test]
fn ddt_off_the_top_of_a_flow_contribution_is_p0001() {
    // analog-structure T20 lifted both refusals this test asserted (a `ddt` that is not a whole
    // addend, a `ddt` with a tolerance): each is a derivative site now (AS-07), one per call, its
    // rate read as `Expr::Derivative`. Kept under its name; it asserts the lowering.
    for (form, body) in [
        ("ddt in a product", "i(p, n) +> 2.0 * ddt(v(p, n));"),
        ("ddt in a product of a potential contribution", "v(p, n) +> 2.0 * ddt(v(p));"),
        ("ddt bound to a name", "let q = ddt(v(p)); i(p, n) +> q;"),
        ("ddt with a tolerance", "i(p, n) +> ddt(v(p), 1e-6);"),
    ] {
        let ir = Elab::ir(DdtSource::of(body), "Top");
        let body = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert_eq!(body.derivatives().len(), 1, "{form}: one site");
    }
}

/// A source with `body` as the analog block of a two-node device.
struct DdtSource;

impl DdtSource {
    fn of(body: &str) -> &'static str {
        // Leaked: a test-only literal of 'static lifetime for the `Row` table.
        Box::leak(format!("module M(p: Electrical, n: Electrical) {{ }}\nanalog for M {{ {body} }}\nmodule Top() {{ signal s: Electrical; let m = M {{ }}(s, gnd); }}").into_boxed_str())
    }
}

#[test]
fn m_dtemp_or_extern_at_a_placement_is_p0001() {
    // SPEC_DEVIATION (ELAB-06 AC 3): the front end resolves a placement's field against the
    // module's params only, so `m` and `dtemp` are `E0320` there, before elaboration runs; the
    // model is still rejected at the field with no `Ir`, but the code is not `P0001`. The `P0001`
    // arm in `children.rs` stays for the day the POM resolves implicit params (M2).
    for field in ["m: 2.0", "dtemp: 5.0"] {
        let src = format!("module R(p: Electrical, n: Electrical) {{ r: f64 = 1.0; }}\nmodule Top() {{ signal s: Electrical; let r = R {{ r: 1.0, {field} }}(s, gnd); }}");
        let design = Elab::design(&src);
        let err = Elaborator::new(&design).run("Top").expect_err(field);
        assert_eq!(Elab::codes(&err), ["E0320"], "{field}");
        let span = err[0].diagnostic.primary.0;
        assert_eq!(&src[span.start..span.end], field.split(':').next().unwrap_or(field), "{field}");
    }
    for row in [
        Row {
            form: "extern module",
            src: "#[link(\"x.osdi\", model = \"x\")]\nextern \"osdi\" module X(a: Electrical) { }\nmodule Top() { signal s: Electrical; let x = X(s); }",
            names: "`extern` module",
            // The type checker (`types/`) reports it, at the declaration of the module, not at the
            // placement; ELAB-06 AC 3 asks for the placement (reported as a spec gap).
            at: "#[link(\"x.osdi\", model = \"x\")]\nextern \"osdi\" module X(a: Electrical) { }",
        },
    ] {
        row.check();
    }
}

#[test]
fn a_for_loop_in_a_digital_block_is_p0001() {
    // A `digital` block is lowered from msl-digital T30 on; a form still refused inside one is the
    // `for` loop of DIG-07 AC 1.
    let row = Row {
        form: "for loop in a digital block",
        src: "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { for k in 0..3 { } } }",
        names: "`for` loop",
        at: "for k in 0..3 { }",
    };
    row.check();
}

#[test]
fn a_diagnostic_means_no_ir() {
    // Two refusals in one design: still `Err`, every diagnostic a `P0001`, never a partial `Ir`.
    let src = "module Top(in clk: Bit, out q: Bit) { }\ndigital for Top { on clk.rise() { q[0] <- true; } on clk.fall() { q[0] <- false; } }";
    let err = Elab::run(src, "Top").expect_err("refused");
    assert!(err.len() >= 2, "{:?}", Elab::codes(&err));
    assert!(Elab::codes(&err).iter().all(|c| c == "P0001"));
}

#[test]
fn a_site_on_the_left_of_a_short_circuit_still_lowers() {
    // The left operand always runs, so a site there is unconditional (ELAB-06, AC 1).
    for op in ["&&", "||"] {
        let src = format!("module M(p: Electrical, n: Electrical) {{ }}\nanalog for M {{ let c = limit(v(p), |a, b| a) > 0.0 {op} v(p) > 0.0; let d = ac_stim(1.0) > 0.0 {op} v(p) > 0.0; }}\nmodule Top() {{ signal s: Electrical; let m = M {{ }}(s, gnd); }}");
        let ir = Elab::ir(&src, "Top");
        let text = ir.to_string();
        assert!(text.contains("limit L0") && text.contains("stim"), "{op}: {text}");
    }
}

#[test]
fn a_contribution_in_the_analog_blocks_own_list_still_lowers() {
    let src = "module M(p: Electrical, n: Electrical) { }\nanalog for M { i(p, n) +> 1.0; }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }";
    Elab::run(src, "Top").expect("a top-level contribution lowers");
}

#[test]
fn a_form_the_checker_accepts_for_msl_digital_is_p0001_until_it_is_lowered() {
    // The type checker accepts the digital forms and the flow read of a potential branch
    // (msl-digital T9 to T16); the structure passes that lower them come later. Until then every one
    // is refused by name, never dropped and never lowered as a different circuit.
    for (form, src, names) in [
        // A carrier is lowered from msl-digital T28 on: the form still refused in its place is the
        // resolved carrier of DIG-07 AC 1.
        ("a nature with resolve", "module Top(c: Wire) { }", vec!["resolved nature `Wire`"]),
        // A definition is lowered from msl-digital T29 on: the form still refused in its place is
        // the element commit of DIG-07 AC 1.
        ("an element commit", "module Top(in clk: Bit, out q: Bit) { }\ndigital for Top { on clk.rise() { q[0] <- true; } }", vec!["element or field commit"]),
        // A `digital` block is lowered from msl-digital T30 on: the form still refused in its place
        // is `settle` of DIG-07 AC 1.
        ("settle", "module Top() { }\ndigital for Top { on settle { } }", vec!["`settle`"]),
        // A module with a result is hoisted from msl-digital T33 on: the form still refused in its
        // place is `others()` of DIG-07 AC 1.
        ("others()", "module Top(in c: Bit) { let x = c.others(); }", vec!["method `others`"]),
        // The flow read of a potential branch is lowered from msl-digital T35 on: the form still
        // refused in its place is the template of DIG-07 AC 1.
        ("a template", "module Top(a: Electrical) { let t = Resistor { r: 1.0 }; }\nmodule Resistor(p: Electrical, n: Electrical) { r: f64; }", vec!["module template"]),
    ] {
        let err = Elab::run(src, "Top").expect_err(form);
        assert!(Elab::codes(&err).iter().all(|c| c == "P0001"), "{form}: {:?}", err.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        for name in names {
            assert!(err.iter().any(|d| d.diagnostic.message.contains(name)), "{form}: no diagnostic names `{name}`: {:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        }
    }
}

#[test]
fn a_value_input_of_a_module_without_a_result_lowers_hoisted() {
    // analog-structure T27 (AS-08 AC 1): the row `a value input` of the table above is lifted: a
    // module with a value input is hoisted like one with a result. `Amp` has no body of its own; its
    // `analog` block joins `Top`'s under the placement's name, and the body records it (`hoisted`).
    let src = "module Amp(x: f64, p: Electrical) { }\nanalog for Amp { v(p) +> x; }\nmodule Top() { signal s: Electrical; let a = Amp(1.0, s); }";
    let ir = Elab::ir(src, "Top");
    assert!(ir.bodies().all(|b| b.name() != "Amp"), "a hoisted module gets no body");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("the body Top");
    assert_eq!(top.contributions().len(), 1, "the callee's contribution joins the caller's body");
    assert_eq!(top.hoisted().iter().map(|h| (h.name.as_str(), h.module.as_str())).collect::<Vec<_>>(), [("a", "Amp")]);
}

#[test]
fn a_contribution_under_control_flow_lowers_where_it_was_written() {
    // analog-structure T16 (AS-01 AC 1, AC 3): a contribution under an `if` or in a plain block
    // nested in the `analog` block lowers, in place, as a `Contribute` inside the arm or the
    // block; the old refusals of these three forms were rows of `a_site_or_contribution_under_if_is_p0001`.
    for (form, body) in [
        ("contribution under if", "if v(p) > 0.0 { i(p, n) +> 1.0; }"),
        ("contribution in a bare block", "{ i(p, n) +> 1.0; }"),
        ("contribution in a let block", "let x = { i(p, n) +> 1.0; 2.0 };"),
    ] {
        let src = format!("module M(p: Electrical, n: Electrical) {{ }}\nanalog for M {{ {body} }}\nmodule Top() {{ signal s: Electrical; let m = M {{ }}(s, gnd); }}");
        let ir = Elab::ir(&src, "Top");
        let body = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert_eq!(body.contributions().len(), 1, "{form}");
    }
}

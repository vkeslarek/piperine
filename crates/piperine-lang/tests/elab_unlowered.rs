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
    // A `limit` or `ac_stim` in a `fn` or a closure is `E0450` from the front end (spec rule),
    // before elaboration; under an `if` of the analog block it reaches this table.
    for row in [
        Row {
            form: "contribution under if",
            src: "module M(p: Electrical, n: Electrical) { }\nanalog for M { if v(p) > 0.0 { i(p, n) +> 1.0; } }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }",
            names: "a contribution under an `if` or in a closure",
            at: "i(p, n) +> 1.0;",
        },
        Row {
            form: "limit under if",
            src: "module M(p: Electrical, n: Electrical) { }\nanalog for M { let x = if v(p) > 0.0 { limit(v(p), |a, b| a) } else { 0.0 }; }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }",
            names: "a `limit` under an `if` or in a closure",
            at: "limit(v(p), |a, b| a)",
        },
        Row {
            form: "ac_stim under if",
            src: "module M(p: Electrical, n: Electrical) { }\nanalog for M { let x = if v(p) > 0.0 { ac_stim(1.0) } else { 0.0 }; }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }",
            names: "an `ac_stim` under an `if` or in a closure",
            at: "ac_stim(1.0)",
        },
    ] {
        row.check();
    }
}

#[test]
fn ddt_off_the_top_of_a_flow_contribution_is_p0001() {
    for row in [
        Row { form: "ddt in a product", src: &DdtSource::of("i(p, n) +> 2.0 * ddt(v(p, n));"), names: "a `ddt` that is not a whole addend of a flow contribution", at: "ddt(v(p, n))" },
        Row { form: "ddt in a potential contribution", src: &DdtSource::of("v(p, n) +> ddt(v(p));"), names: "a `ddt` that is not a whole addend of a flow contribution", at: "ddt(v(p))" },
        Row { form: "ddt bound to a name", src: &DdtSource::of("let q = ddt(v(p)); i(p, n) +> q;"), names: "a `ddt` that is not a whole addend of a flow contribution", at: "ddt(v(p))" },
        Row { form: "ddt with a tolerance", src: &DdtSource::of("i(p, n) +> ddt(v(p), 1e-6);"), names: "a `ddt` with a tolerance", at: "ddt(v(p), 1e-6)" },
    ] {
        row.check();
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
fn a_digital_block_is_p0001() {
    let row = Row {
        form: "digital block",
        src: "module Top() { }\ndigital for Top { }",
        names: "`digital` block",
        at: "digital for Top { }",
    };
    row.check();
}

#[test]
fn a_diagnostic_means_no_ir() {
    // Two refusals in one design: still `Err`, every diagnostic a `P0001`, never a partial `Ir`.
    let src = "module M(p: Electrical, n: Electrical) { }\nanalog for M { i(p, n) +> 2.0 * ddt(v(p, n)); if v(p) > 0.0 { i(p, n) +> 1.0; } }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }";
    let err = Elab::run(src, "Top").expect_err("refused");
    assert!(err.len() >= 2, "{:?}", Elab::codes(&err));
    assert!(Elab::codes(&err).iter().all(|c| c == "P0001"));
}

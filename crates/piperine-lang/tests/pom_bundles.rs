//! Declaration rules of a bundle (feature `resolved-nets-bundles`, BN-01; spec part II §7): a
//! `struct` of carriers is a bundle (rule 1), and what the POM refuses at the declaration, with
//! no types and no const evaluation, is `E0260`, `E0261`, `E0262` and `E0264`, each at the field
//! or the position it names. The source uses the real standard library for `Bit`, `Wire` and
//! `Electrical`.

use piperine_lang::pom::{Design, DesignDiagnostic};

/// Builds the declaration rules' test designs.
struct Bundles;

impl Bundles {
    /// A design of `src` over the embedded standard library.
    fn design(src: &str) -> Design {
        Design::builder().source("top.msl", src).build()
    }

    /// The diagnostics of `design` with `code`, each as the authored text of its primary span.
    fn spans(design: &Design, code: &str) -> Vec<String> {
        design
            .diagnostics()
            .iter()
            .filter(|d| d.diagnostic.code == code)
            .map(|d| Bundles::primary_text(design, d).to_string())
            .collect()
    }

    /// The authored text of the primary span of a diagnostic.
    fn primary_text<'a>(design: &'a Design, diagnostic: &DesignDiagnostic) -> &'a str {
        let file = &design.files()[diagnostic.file.index()];
        &file.text()[diagnostic.diagnostic.span().range()]
    }

    /// The codes of every diagnostic of the design.
    fn codes(design: &Design) -> Vec<String> {
        design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
    }
}

#[test]
fn a_struct_of_carriers_is_a_bundle() {
    // §7 rule 1: carrier fields, arrays of carriers and bundles make a bundle, and no rule of this
    // stage refuses it; a nested bundle is a field too
    let design = Bundles::design(
        "struct Inner { x: Electrical }\nstruct Stream { a: Electrical, b: [Electrical; 4], c: Inner }\nmodule Top() { }\n",
    );
    assert_eq!(Bundles::codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
}

#[test]
fn a_struct_mixing_carrier_and_value_fields_is_e0260() {
    // §7 rule 1: a struct that mixes carrier fields and value fields is an error naming one field
    // of each kind; a struct of values only is a plain struct and is no concern of this rule
    let design = Bundles::design("struct Mixed { hi: Electrical, gain: f64 }\nstruct Plain { gain: f64, offset: f64 }\nmodule Top() { }\n");
    assert_eq!(Bundles::spans(&design, "E0260"), ["gain"], "{:?}", design.diagnostics());
    let message = &design.diagnostics().iter().find(|d| d.diagnostic.code == "E0260").expect("E0260").diagnostic.message;
    assert!(message.contains("`hi`") && message.contains("`gain`"), "names a field of each kind: {message}");
}

#[test]
fn one_driver_fields_carry_a_direction_and_no_other_field_does() {
    // §7 rule 2: `Bit` is a one-driver nature and takes `in` or `out`; `Wire` (resolved) and
    // `Electrical` (conservative) take none
    let design = Bundles::design("struct Stream { out valid: Bit, in ready: Bit, bus: Wire, node: Electrical }\nmodule Top() { }\n");
    assert_eq!(Bundles::codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
}

#[test]
fn a_misplaced_or_missing_field_direction_is_e0261() {
    // §7 rule 2: a one-driver field with no direction, and a field of any other nature with one,
    // is an error at the field's name
    let design = Bundles::design("struct Stream { valid: Bit, out bus: Wire, in node: Electrical, ok: Wire, out good: Bit }\nmodule Top() { }\n");
    assert_eq!(Bundles::spans(&design, "E0261"), ["valid", "bus", "node"], "{:?}", design.diagnostics());
}

#[test]
fn flip_out_and_nested_directions_are_e0262() {
    // §7 rules 5 and 6: a direction on a nested bundle field, `out` on a bundle port and `flip` on
    // a type that is no bundle are errors; `flip` on a bundle, `in` on a bundle port and a plain
    // nested bundle are accepted
    let design = Bundles::design(
        "struct Inner { out x: Bit }\n\
         struct Outer { in nested: Inner, reversed: flip Inner, plain: Inner, out data: Bit }\n\
         module Top(out tx: Inner, rx: flip Inner, in obs: Inner, bad: flip Bit) { }\n",
    );
    assert_eq!(Bundles::spans(&design, "E0262"), ["nested", "tx", "flip Bit"], "{:?}", design.diagnostics());
}

#[test]
fn a_bundle_declared_as_a_value_is_e0264() {
    // §7 rule 9: a bundle is no param type, no type argument of a nature and no function result;
    // rule 10: a bundle signal or port has no initial value. Each case is one error at the position.
    let cases: [(&str, &str, &str); 5] = [
        ("a param type", "module Top() { k: Stream; }", "k"),
        ("a nature's type argument", "module Top(p: Bit<Stream>) { }", "Stream"),
        ("a function result", "fn make() -> Stream { }\nmodule Top() { }", "Stream"),
        ("a signal's initial value", "module Top(s: Stream) { signal t: Stream = s; }", "s"),
        ("a port's initial value", "module Top(p: Stream = 0) { }", "0"),
    ];
    for (what, body, expected_span) in cases {
        let design = Bundles::design(&format!("struct Stream {{ out valid: Bit }}\n{body}\n"));
        assert_eq!(Bundles::spans(&design, "E0264"), [expected_span], "{what}: {:?}", design.diagnostics());
    }
    let plain = Bundles::design("struct Stream { out valid: Bit }\nmodule Top(p: Stream, in q: Stream) { signal t: Stream; }\n");
    assert_eq!(Bundles::codes(&plain), Vec::<String>::new(), "a bundle port and signal without a value are accepted: {:?}", plain.diagnostics());
}

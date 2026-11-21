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

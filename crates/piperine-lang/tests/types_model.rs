//! The whole-model check of `piperine_lang::types` (S3, GN-08): `Types::check_model` types every
//! module, function, behavior block, `const` and `impl` function of every crate but `std`,
//! reached from a top or not, and each error has the code it has when reached. `Types::check`
//! keeps the reach of one top (`types_subset.rs::unreached_items_are_not_reported`).

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{CrateGraph, Design, DesignDiagnostic};
use piperine_lang::types::Types;

/// The natures every source below places in the open.
const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// One unreached item of each kind, each with one error of its own.
const ONE_ERROR_PER_KIND: &str = "fn broken(x: f64) -> bool { x + 1.0 }\n\
module Wrong() { limit: f64 = true; }\n\
module Behaving(a: E) { }\n\
analog for Behaving { let flag: bool = 1.0; }\n\
const LIMIT: bool = 1.0;\n\
struct Holder { a: f64 }\n\
impl Holder { fn make() -> bool { 1.0 } }\n";

/// The diagnostics of the model check of `src` after `HEAD`, with `src`.
fn model_errors(src: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = format!("{HEAD}{src}");
    let design = support::build(&src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    let found = Types::check_model(&design).expect_err("expected diagnostics");
    (src, found)
}

/// The text each diagnostic points at, in source order.
fn snippets<'a>(src: &'a str, found: &[DesignDiagnostic]) -> Vec<&'a str> {
    found.iter().map(|d| &src[d.diagnostic.span().range()]).collect()
}

fn codes_of(found: &[DesignDiagnostic]) -> Vec<String> {
    found.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

#[test]
fn every_item_kind_of_every_crate_is_checked_unreached() {
    // the root crate holds one error per item kind; a dependency crate holds one more
    let mut graph = CrateGraph::new();
    let root = graph.add_crate("app", "app/lib.msl").unwrap();
    let dependency = graph.add_crate("util", "util/lib.msl").unwrap();
    graph.add_dependency(root, "util", dependency).unwrap();
    let app = format!("{HEAD}{ONE_ERROR_PER_KIND}");
    let util = "pub fn from_a_dependency(x: f64) -> bool { x }\n";
    let design = Design::builder().std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", "").source("app/lib.msl", app.as_str()).source("util/lib.msl", util).crates(graph).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let found = Types::check_model(&design).expect_err("expected diagnostics");
    assert_eq!(found.len(), 6, "five in the root crate, one in the dependency: {found:?}");
    let in_root: Vec<&DesignDiagnostic> = found.iter().filter(|d| d.file == found[0].file).collect();
    assert_eq!(in_root.len(), 5, "{found:?}");
    let named: Vec<&str> = in_root.iter().map(|d| &app[d.diagnostic.span().range()]).collect();
    for item in ["x + 1.0", "true", "1.0"] {
        assert!(named.contains(&item), "`{item}` is not pointed at: {named:?}");
    }
    assert!(found.iter().any(|d| d.file != found[0].file), "the dependency crate is checked: {found:?}");
}

#[test]
fn an_unreached_function_with_an_error_fails_the_model_check() {
    // GN-08 AC 3: `broken` is called by nothing, and the model check names it
    let (src, found) = model_errors("fn broken(x: f64) -> bool { x + 1.0 }\nmodule Top() { }");
    assert_eq!(snippets(&src, &found), ["x + 1.0"]);
    assert_eq!(codes_of(&found), ["E0180"]);
}

#[test]
fn an_unplaced_module_calling_ddt_from_a_function_is_e0350() {
    // GN-08 AC 4: nothing places `Lonely`, and its function places the core module `ddt`
    let (src, found) = model_errors("fn rate(x: f64) -> f64 { ddt(x) }\nmodule Lonely(a: E) { }\nanalog for Lonely { let y = rate(v(a)); }");
    assert_eq!(codes_of(&found), ["E0350"]);
    assert_eq!(snippets(&src, &found), ["ddt(x)"]);
}

#[test]
fn an_unreached_item_has_the_code_it_has_when_reached() {
    // the same function, once unreached under the model check and once reached from `Top`
    let function = "fn broken(x: f64) -> bool { x + 1.0 }\n";
    let (src, unreached) = model_errors(&format!("{function}module Top() {{ }}"));
    let reached_src = format!("{HEAD}{function}module Top() {{ }}\nanalog for Top {{ let b = broken(1.0); }}");
    let design = support::build(&reached_src);
    let reached = Types::check(&design, "Top").expect_err("reached, it fails");
    assert_eq!(codes_of(&unreached), codes_of(&reached));
    assert_eq!(snippets(&src, &unreached), snippets(&reached_src, &reached));
}

#[test]
fn the_standard_library_is_left_to_its_own_check() {
    // an error in a `std` item that nothing reaches is not the project's (D2); the same error in
    // the project is
    let std_error = "pub fn broken(x: f64) -> bool { x + 1.0 }\n";
    let design = Design::builder().std_source("lib.msl", format!("pub mod prelude;\n{std_error}")).std_source("prelude.msl", "").source("top.msl", format!("{HEAD}module Top() {{ }}")).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    assert!(Types::check_model(&design).is_ok());
    let (_, found) = model_errors(&format!("{std_error}module Top() {{ }}"));
    assert_eq!(codes_of(&found), ["E0180"]);
}

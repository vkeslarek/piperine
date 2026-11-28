//! The whole-model check of `piperine_lang::types` (S3, GN-08): `Types::check_model` types every
//! module, function, behavior block, `const` and `impl` function of every crate but `std`,
//! reached from a top or not, and each error has the code it has when reached. `Types::check`
//! keeps the reach of one top (`types_subset.rs::unreached_items_are_not_reported`).



use piperine_lang::pom::{CrateGraph, Design, DesignDiagnostic};
use piperine_lang::types::Types;
use piperine_lang_fixtures::generic_sources::GenericSources;
use piperine_lang_fixtures::pom::PomSource;
use piperine_lang_fixtures::checked::CheckedSource;

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

/// What a model-check test reads from a source: the diagnostics of the check and the text they point at.
struct ModelCheck;

impl ModelCheck {
    /// The diagnostics of the model check of `src` after `HEAD`, with `src`.
    fn errors(src: &str) -> (String, Vec<DesignDiagnostic>) {
        let src = format!("{HEAD}{src}");
        let design = PomSource::build(&src);
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", PomSource::codes(&design));
        let found = Types::check_model(&design).expect_err("expected diagnostics");
        (src, found)
    }

    /// The text each diagnostic points at, in source order.
    fn snippets<'a>(src: &'a str, found: &[DesignDiagnostic]) -> Vec<&'a str> {
        found.iter().map(|d| &src[d.diagnostic.span().range()]).collect()
    }
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
    assert!(design.diagnostics().is_empty(), "{:?}", PomSource::codes(&design));
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
    let (src, found) = ModelCheck::errors("fn broken(x: f64) -> bool { x + 1.0 }\nmodule Top() { }");
    assert_eq!(ModelCheck::snippets(&src, &found), ["x + 1.0"]);
    assert_eq!(CheckedSource::codes(&found), ["E0180"]);
}

#[test]
fn an_unplaced_module_calling_ddt_from_a_function_is_e0350() {
    // GN-08 AC 4: nothing places `Lonely`, and its function places the core module `ddt`
    let (src, found) = ModelCheck::errors("fn rate(x: f64) -> f64 { ddt(x) }\nmodule Lonely(a: E) { }\nanalog for Lonely { let y = rate(v(a)); }");
    assert_eq!(CheckedSource::codes(&found), ["E0350"]);
    assert_eq!(ModelCheck::snippets(&src, &found), ["ddt(x)"]);
}

#[test]
fn an_unreached_item_has_the_code_it_has_when_reached() {
    // the same function, once unreached under the model check and once reached from `Top`
    let function = "fn broken(x: f64) -> bool { x + 1.0 }\n";
    let (src, unreached) = ModelCheck::errors(&format!("{function}module Top() {{ }}"));
    let reached_src = format!("{HEAD}{function}module Top() {{ }}\nanalog for Top {{ let b = broken(1.0); }}");
    let design = PomSource::build(&reached_src);
    let reached = Types::check(&design, "Top").expect_err("reached, it fails");
    assert_eq!(CheckedSource::codes(&unreached), CheckedSource::codes(&reached));
    assert_eq!(ModelCheck::snippets(&src, &unreached), ModelCheck::snippets(&reached_src, &reached));
}

#[test]
fn the_standard_library_is_checked_whole_with_no_instantiation() {
    // GN-27 AC 3: every item of `std` (the embedded library, nothing placed) type-checks with no error
    let design = Design::builder().source("top.msl", format!("{HEAD}module Top() {{ }}")).build();
    assert!(design.diagnostics().is_empty(), "{:?}", PomSource::codes(&design));
    let found = Types::check_model(&design).err().unwrap_or_default();
    assert!(found.is_empty(), "{:?}", CheckedSource::codes(&found));
    // and an error in a `std` item that nothing reaches is reported like the same error in the project (D2 retired)
    let std_error = "pub fn broken(x: f64) -> bool { x + 1.0 }\n";
    let design = Design::builder().std_source("lib.msl", format!("pub mod prelude;\n{std_error}")).std_source("prelude.msl", "").source("top.msl", format!("{HEAD}module Top() {{ }}")).build();
    assert!(design.diagnostics().is_empty(), "{:?}", PomSource::codes(&design));
    let in_std = Types::check_model(&design).expect_err("an unreached `std` error is now found");
    let (_, in_project) = ModelCheck::errors(&format!("{std_error}module Top() {{ }}"));
    assert_eq!(CheckedSource::codes(&in_std), CheckedSource::codes(&in_project));
}

#[test]
fn an_unreached_generic_module_with_an_error_fails_the_model_check() {
    // GN-13 AC 1, S3: a generic module nothing places is typed once at its definition, so an error in its body is
    // reported where it is written, with the code it has when reached, never as `P0001`
    let broken = "module Hold<T: Repr + Default>(in d: Bit<T>) { }\nanalog<T> for Hold<T> { let flag: bool = 1.0; }\nmodule Top() { }\n";
    let found = GenericSources::standard_found(broken);
    assert_eq!(GenericSources::codes_of(&found), ["E0180"], "{found:?}");
    let clean = "module Hold<T: Repr + Default>(in d: Bit<T>) { }\nmodule Top() { }\n";
    assert!(GenericSources::standard_found(clean).is_empty(), "a clean generic module passes the model check");
}

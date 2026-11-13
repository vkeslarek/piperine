//! Scope: the undocumented-until-now legacy `` `include`` alias fallback in
//! `vams_include.rs::SourceMapIncludeResolver` (T53, `lrm-conformance-wave3`,
//! PSW3-05/B7) — `discipline.h`/`disciplines.h`/`constants.h`, the
//! Cadence-era names real-world Verilog-A sources still `` `include``,
//! aliasing Annex D's own `disciplines.vams`/`constants.vams`. Before this
//! task the mechanism existed in the tree with no test at all (spec.md's B7
//! finding); this file proves it resolves, proves its precedence (a
//! project's own `discipline.h` wins over the alias), and proves an unknown
//! legacy name still fails loud rather than silently matching something.
//!

use piperine_lang::{IncludeResolver, SourceMap, SourceMapIncludeResolver};

#[test]
fn discipline_h_resolves_to_annex_d_disciplines_vams() {
    let resolver = SourceMapIncludeResolver::new(&SourceMap::dummy());
    let alias = resolver.resolve("discipline.h").expect("discipline.h resolves via the legacy alias");
    let canonical = resolver.resolve("disciplines.vams").expect("disciplines.vams resolves directly");
    assert_eq!(alias, canonical, "discipline.h must resolve to exactly Annex D's disciplines.vams content");
}

#[test]
fn disciplines_h_resolves_to_annex_d_disciplines_vams() {
    let resolver = SourceMapIncludeResolver::new(&SourceMap::dummy());
    let alias = resolver.resolve("disciplines.h").expect("disciplines.h resolves via the legacy alias");
    let canonical = resolver.resolve("disciplines.vams").expect("disciplines.vams resolves directly");
    assert_eq!(alias, canonical, "disciplines.h must resolve to exactly Annex D's disciplines.vams content");
}

#[test]
fn constants_h_resolves_to_annex_d_constants_vams() {
    let resolver = SourceMapIncludeResolver::new(&SourceMap::dummy());
    let alias = resolver.resolve("constants.h").expect("constants.h resolves via the legacy alias");
    let canonical = resolver.resolve("constants.vams").expect("constants.vams resolves directly");
    assert_eq!(alias, canonical, "constants.h must resolve to exactly Annex D's constants.vams content");
}

#[test]
fn a_projects_own_discipline_h_wins_over_the_legacy_alias() {
    let dir = std::env::temp_dir().join("piperine_vams_include_legacy_alias_precedence");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let sentinel = "// PROJECT-OWNED discipline.h — not the Annex D alias target\n";
    std::fs::write(dir.join("discipline.h"), sentinel).expect("write project's own discipline.h");

    let mut map = SourceMap::new(dir.clone());
    map.add_namespace("piperine", "headers");
    map.add_namespace("spice", "headers/spice");
    let resolver = SourceMapIncludeResolver::new(&map);

    let resolved = resolver.resolve("discipline.h").expect("discipline.h resolves");
    assert_eq!(
        resolved, sentinel,
        "a project's own discipline.h must win over the legacy alias to disciplines.vams"
    );

    let _ = std::fs::remove_file(dir.join("discipline.h"));
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn an_unknown_legacy_style_name_still_fails_loud() {
    let resolver = SourceMapIncludeResolver::new(&SourceMap::dummy());
    assert!(
        resolver.resolve("not_a_real_legacy_header.h").is_none(),
        "an unrecognized `.h` name must not silently resolve to anything"
    );
}

/// The alias only ever kicks in as a last-resort fallback, never ahead of
/// the literal-name search across every namespace/root candidate — proven
/// directly by the precedence test above, restated here as an integration
/// check that a real `` `include "discipline.h" `` inside a `.vams` source
/// actually parses and elaborates through the preprocessor end to end (not
/// just the resolver in isolation).
#[test]
fn include_discipline_h_elaborates_a_real_module() {
    let src = "`include \"discipline.h\"
module Top (inout electrical p, inout electrical n);
    analog V(p, n) <+ 0.0;
endmodule
";
    piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaboration failed via the discipline.h alias: {e:?}"));
}

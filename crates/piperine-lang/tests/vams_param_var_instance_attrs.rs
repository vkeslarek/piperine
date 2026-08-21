//! T15/LRMW2-11 (§3.4.3 parameters, §3.2.1 variables): `(* ... *)`
//! attribute instances on parameters and variables now survive into the
//! POM through the same schema-checked path `lower_port_attrs`/
//! `lower_net_attrs` already run for ports/nets. Before this fix every
//! `Param`/`Var::attributes` field was hardcoded to `Vec::new()` at
//! `elab/lower/vams.rs` — the attrs were parsed, then silently dropped
//! before the POM was ever built.
//!
//! Instance attrs are fixed at the same code sites (`Instance::attributes`
//! no longer hardcodes `Vec::new()` either — see `lower_instance`/
//! `lower_paramset_instance` in `elab/lower/vams.rs`), but no source-text
//! test exercises that path here: `(* ... *) ModuleName label(...);`
//! cannot parse *at all* today, attributed or not — a pre-existing,
//! separately-scoped bug in `grammar/item.rs::parse_module_item`'s
//! `looks_like_instantiation` heuristic (its `peek_at(1)`/`peek_at(2)`
//! lookahead offsets are relative to the cursor *before* `kw_after_attrs`
//! virtually skips the leading attrs, so they land inside the attr-list
//! tokens instead of past them). That is a `grammar/item.rs` fix, one of
//! CLAUDE.md's "files not to edit casually" and outside this task's
//! stated `Where` (`elab/lower/vams.rs` only) — left as a follow-on gap,
//! not fixed here (Wave-2 standing rule 1).
//!
//! LRM-COVERS: 3.4.3, 3.2.1

use piperine_lang::{parse_and_elaborate, parse_and_elaborate_seeded, SourceMap};

fn seed_meta(ctx: &mut piperine_lang::elab::registry::ElabContext) {
    ctx.register_schema("meta");
}

const PARAM_SRC: &str = "
module HostR (inout electrical p, inout electrical n);
    (* meta = \"vth\" *) parameter real vth = 0.7;
endmodule
";

const VAR_SRC: &str = "
module HostR (inout electrical p, inout electrical n);
    (* meta = \"state\" *) real acc;
    analog acc = 0.0;
endmodule
";

const PARAM_UNKNOWN_SRC: &str = "
module HostR (inout electrical p, inout electrical n);
    (* frobnicate = \"x\" *) parameter real vth = 0.7;
endmodule
";

const VAR_UNKNOWN_SRC: &str = "
module HostR (inout electrical p, inout electrical n);
    (* frobnicate = \"x\" *) real acc;
    analog acc = 0.0;
endmodule
";

/// A parameter's `(* meta = "vth" *)` attribute reaches its `Param` in the
/// POM, carrying its own const-evaluated value.
#[test]
fn parameter_attribute_survives_into_the_pom() {
    let design = parse_and_elaborate_seeded(PARAM_SRC, &SourceMap::dummy(), seed_meta)
        .expect("elaborates with `meta` seeded");
    let m = design.module("HostR").expect("module HostR");
    let p = m.params().iter().find(|p| p.name() == "vth").expect("param vth");
    let attr = p.attributes().iter().find(|a| a.schema() == "meta").expect("meta attribute present");
    assert_eq!(attr.field("meta").unwrap().to_string(), "\"vth\"");
}

/// A variable's `(* meta = "state" *)` attribute reaches its `Var` in the
/// POM.
#[test]
fn var_attribute_survives_into_the_pom() {
    let design = parse_and_elaborate_seeded(VAR_SRC, &SourceMap::dummy(), seed_meta)
        .expect("elaborates with `meta` seeded");
    let m = design.module("HostR").expect("module HostR");
    let v = m.vars().iter().find(|v| v.name() == "acc").expect("var acc");
    let attr = v.attributes().iter().find(|a| a.schema() == "meta").expect("meta attribute present");
    assert_eq!(attr.field("meta").unwrap().to_string(), "\"state\"");
}

/// An unregistered attribute schema on a parameter still fails loud, typed
/// as `ElabErrorKind::UnknownAttrSchema` (E2022) naming the attribute —
/// MD-24's "no silent fallback" carried into the new lowering path.
/// Asserted by the miette diagnostic code, the same convention
/// `vams_device_port_attrs.rs`'s own `UnknownAttrSchema` negative tests
/// use — this error carries a span, so it renders through
/// `resolve_elab_error`'s `.with_source_code` path, which wraps it behind
/// an opaque `dyn Diagnostic` `downcast_ref::<ElabError>` cannot see
/// through (verified: `err.downcast_ref::<ElabError>()` returns `None`
/// here even though it succeeds for an unspanned `ElabError`) — the
/// diagnostic code is the typed-kind check available on this path.
#[test]
fn unknown_attribute_schema_on_a_parameter_fails_loud_by_typed_kind() {
    let report = parse_and_elaborate(PARAM_UNKNOWN_SRC, &SourceMap::dummy())
        .expect_err("an unregistered schema must fail loud");
    let rendered = format!("{report:?}");
    assert!(rendered.contains("E2022"), "{rendered}");
    assert!(rendered.contains("frobnicate"), "{rendered}");
}

/// Same negative check on a variable declaration.
#[test]
fn unknown_attribute_schema_on_a_var_fails_loud_by_typed_kind() {
    let report = parse_and_elaborate(VAR_UNKNOWN_SRC, &SourceMap::dummy())
        .expect_err("an unregistered schema must fail loud");
    let rendered = format!("{report:?}");
    assert!(rendered.contains("E2022"), "{rendered}");
    assert!(rendered.contains("frobnicate"), "{rendered}");
}

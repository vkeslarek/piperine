//! Scope: proving the retired POM/value/error variants named in
//! `lrm-conformance-wave3` spec.md's B2/B3/B4 are genuinely unreachable
//! from Verilog-AMS source (T50, PSW3-02). Two proof techniques, matched to
//! what each family is:
//!
//! - `pom::node::Kind::{Enum,Bundle,Capability}` and `value::Value::
//!   {EnumVariant,Tuple,Record}`, plus nine `ElabErrorKind` variants
//!   (`ContribInModBody`/`ForceInModBody` — no `mod` body in VAMS;
//!   `UnknownBundle`/`BundleFieldUnknown`/`BundleParamDefault`/
//!   `BundleFieldNoDefault`/`BundleParamNameCollision` — no bundle-typed
//!   `param` in VAMS; `NonExhaustiveMatch` — no `match` statement in VAMS;
//!   `PrivateItem` — no package/`pub` declaration in VAMS): these have
//!   **zero construction sites** anywhere in the crate outside their own
//!   `enum` definitions, proven mechanically by scanning the crate's own
//!   sources the same way `tests/errors.rs::error_module_has_no_free_string_catch_all`
//!   proves its own constraint. A variant with no construction site cannot
//!   be produced by *any* input, Verilog-AMS or otherwise.
//! - A live elaboration + full-graph traversal over a fixture that
//!   exercises every `Node` kind (module, instance, port, param, wire,
//!   behavior, attribute, constraint) confirms the walk never yields
//!   `Kind::Enum`/`Bundle`/`Capability` — reinforcing the source-scan proof
//!   with a runtime one for the one family (`Kind`) that has a live
//!   traversal to run.
//!
//! **Correction found during this task**: spec.md's B3 row lists `Value::
//! List` alongside `Record`/`Tuple`/`EnumVariant` as a "retired-front-end
//! survivor". That is not accurate — `List` **is** reachable from real
//! Verilog-AMS: the `'{...}` array assignment-pattern extension lowers to
//! it (`elab::const_eval::ConstEnv::eval_array`, called from
//! `elab::lower::vams`'s array-`parameter` default lowering, confirmed by
//! `list_is_reachable_via_array_assignment_pattern` below). `List` is
//! therefore excluded from this file's "prove unreachable" claims; T55
//! corrects `docs/spec/part_iv_reflection_selector.md` §5 to drop it from
//! the retired-variant list.
//!
//! SPEC-COVERS: part_iv#5, part_iv#10

use piperine_lang::pom::node::{Kind, Node};
use piperine_lang::pom::traits::Kinded;
use piperine_lang::value::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

/// Every `.rs` file under `crates/piperine-lang/src/`, as (relative path,
/// contents) — a minimal scoped version of `tests/support/sources.rs`'s
/// walker (that one lives in the root package and isn't reachable from this
/// crate's own `tests/`).
fn crate_sources() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    assert!(out.len() > 20, "the walk found only {} sources under src/ — layout moved?", out.len());
    out
}

fn collect(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.push((rel, text));
            }
        }
    }
}

/// Strip `//` line comments so a construction/match-site scan can't be
/// fooled by a doc comment naming the variant (this file's own header, and
/// the "reserved" doc lines added on each retired variant, both do) —
/// mirrors `tests/errors.rs`'s `code_only` filter.
fn code_only(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Count qualified occurrences (`Type::Variant`) of `needle` across every
/// `.rs` file under `src/` **except** `defining_file` (the file that
/// declares the enum, e.g. `value.rs`/`pom/error.rs`) — that file's own
/// exhaustive `match`/`Debug`/`PartialEq`/serde impls necessarily spell the
/// qualified path in pattern position, which is not a construction site and
/// would otherwise produce false positives. A real construction OR
/// pattern-match reference from *any other* module is what "reachable"
/// means here: either shape requires some other code path to have obtained
/// or produced a value of that variant.
fn occurrences_outside_defining_file(defining_file: &str, needle: &str) -> usize {
    crate_sources()
        .into_iter()
        .filter(|(path, _)| path != defining_file)
        .map(|(_, text)| code_only(&text).matches(needle).count())
        .sum()
}

#[test]
fn value_retired_variants_have_no_reference_outside_their_defining_file() {
    for (variant, needle) in [
        ("EnumVariant", "Value::EnumVariant"),
        ("Tuple", "Value::Tuple("),
        ("Record", "Value::Record"),
    ] {
        let n = occurrences_outside_defining_file("value.rs", needle);
        assert_eq!(
            n, 0,
            "Value::{variant} is referenced outside value.rs (construction or match) — it is \
             not actually unreachable; found {n} occurrence(s) of `{needle}`"
        );
    }
    // Sanity check: List — proven live in the next test — must show up
    // outside value.rs, or this scan technique isn't discriminating.
    let list_refs = occurrences_outside_defining_file("value.rs", "Value::List");
    assert!(
        list_refs > 0,
        "sanity check failed: Value::List (known live via '{{...}}') shows zero references \
         outside value.rs — the scan technique itself is broken, not proving anything"
    );
}

#[test]
fn list_is_reachable_via_array_assignment_pattern() {
    // The correction this file documents: unlike its EnumVariant/Tuple/
    // Record neighbors, `Value::List` genuinely is constructed by real
    // elaboration — proven live here, not just by source scan, so the
    // correction is backed by an actual fixture rather than assertion.
    let src = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
discipline electrical;
    potential Voltage;
enddiscipline

module Top ();
    parameter real coeffs [0:2] = '{1.0, 2.0, 3.0};
endmodule
"#;
    let design = parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"));
    let module = design.module("Top").expect("module Top");
    let param = module.param("coeffs").expect("param coeffs");
    let default = param.default().expect("array parameter has a default");
    assert!(
        matches!(default, Value::List(_)),
        "an array `parameter` default with a `'{{...}}` initializer must lower to \
         Value::List — got {default:?} instead"
    );
}

#[test]
fn elab_error_kind_retired_variants_have_no_reference_outside_their_defining_file() {
    let defining_file = "pom/error.rs";
    for (variant, needle) in [
        ("ContribInModBody", "ElabErrorKind::ContribInModBody"),
        ("ForceInModBody", "ElabErrorKind::ForceInModBody"),
        ("UnknownBundle", "ElabErrorKind::UnknownBundle"),
        ("BundleFieldUnknown", "ElabErrorKind::BundleFieldUnknown"),
        ("BundleParamDefault", "ElabErrorKind::BundleParamDefault"),
        ("BundleFieldNoDefault", "ElabErrorKind::BundleFieldNoDefault"),
        ("BundleParamNameCollision", "ElabErrorKind::BundleParamNameCollision"),
        ("NonExhaustiveMatch", "ElabErrorKind::NonExhaustiveMatch"),
        ("PrivateItem", "ElabErrorKind::PrivateItem"),
    ] {
        let n = occurrences_outside_defining_file(defining_file, needle);
        assert_eq!(
            n, 0,
            "ElabErrorKind::{variant} is referenced outside pom/error.rs (construction or \
             match) — it is not actually unreachable; found {n} occurrence(s) of `{needle}`"
        );
    }
    // Sanity check: a variant that genuinely IS constructed elsewhere
    // (UnknownAttrSchema, wired through elab::lower::vams's attribute-schema
    // check) must show at least one reference outside pom/error.rs, or this
    // whole file's proof method is vacuous.
    let live_count = occurrences_outside_defining_file(defining_file, "ElabErrorKind::UnknownAttrSchema");
    assert!(
        live_count > 0,
        "sanity check failed: a known-live variant (UnknownAttrSchema) shows zero references \
         outside pom/error.rs — the scan technique itself is broken, not proving anything"
    );
}

/// A fixture that instantiates every live `Node` kind the selector can
/// traverse to (module, instance, port, param, wire, behavior, attribute,
/// constraint) — a full-graph walk over it must never yield
/// `Kind::Enum`/`Bundle`/`Capability`, because no `Node` variant exists for
/// them (confirmed structurally: `pom::node::Node` has no `Enum`/`Bundle`/
/// `Capability` arm at all, only the source-scanned `ElabErrorKind`/`Value`
/// families needed the text-scan proof above).
const FULL_GRAPH_FIXTURE: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Leaf (inout electrical p, inout electrical n);
    (* desc="leaf resistance" *) parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Top (inout electrical g, inout electrical s);
    parameter real vgs_max = 1.8;
    electrical mid;
    Leaf #(.r(100.0)) la (.p(g), .n(mid));
    constraint begin
        require vgs_ox : abs(V(g, s)) <= vgs_max;
    end
endmodule
"#;

fn collect_kinds<'a>(design: &'a piperine_lang::Design, node: Node<'a>, out: &mut Vec<Kind>) {
    out.push(node.kind());
    for axis in ["inst", "net", "port", "param", "behavior", "attr", "constraint"] {
        let path = format!("{axis}::*");
        if let Ok(sel) = node.select(design, &path) {
            for child in sel.iter() {
                collect_kinds(design, *child, out);
            }
        }
    }
}

#[test]
fn full_graph_traversal_never_yields_a_retired_kind() {
    let mut design = parse_and_elaborate(FULL_GRAPH_FIXTURE, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"));
    design.set_top("Top");
    let top = design.top().expect("top module set");

    let mut kinds = Vec::new();
    collect_kinds(&design, Node::Module(top), &mut kinds);

    assert!(kinds.len() > 1, "sanity check: traversal must visit more than the root module");
    for retired in [Kind::Enum, Kind::Bundle, Kind::Capability] {
        assert!(
            !kinds.contains(&retired),
            "full-graph traversal produced {retired}, which is claimed unreachable from \
             Verilog-AMS"
        );
    }
    // And the traversal does exercise the live kinds this fixture
    // authored, so the walk itself is doing real work, not vacuously
    // passing over an empty graph.
    for live in [Kind::Instance, Kind::Port, Kind::Param, Kind::Behavior, Kind::Attribute, Kind::Constraint] {
        assert!(kinds.contains(&live), "traversal should have reached {live} from the fixture");
    }
}

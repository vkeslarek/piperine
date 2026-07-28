//! dv-core T28 (DVC-19) — analysis-scoped event blocks: `@ dc { … }` /
//! `@ tran` / `@ ac` / `@ (dc | tran)` inside a `constraint` body, parsed
//! through the existing `EventSpec` production and resolved against the
//! existing `EventRegistry`. A scoped `require` records its scope's
//! analyses on its `StmtClass`; `dc`/`tran`/`ac` resolve as event kinds
//! (not a new registry); a top-level `require` holds everywhere; a scoped
//! require whose analysis never ran reports "not exercised" (never a
//! silent vacuous pass). The transient-startup-vs-DC acceptance test lives
//! in `crates/piperine-api/tests/scoped_require_scope.rs`.

use piperine_lang::parse::ast::{ConstraintStmt, EventSpec, Item};
use piperine_lang::parse_str;
use piperine_lang::pom::QuantityClass;

fn constraint_of(src: &str) -> piperine_lang::parse::ast::ConstraintDecl {
    let ast = parse_str(src).expect("fixture must parse");
    for item in ast.items {
        if let Item::ConstraintDecl(c) = item {
            return c;
        }
    }
    panic!("no constraint block found in fixture");
}

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    piperine_lang::parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &piperine_lang::SourceMap::dummy())
}

fn elab_err(src: &str) -> String {
    elaborate(src).map(|_| ()).expect_err("fixture must fail elaboration").to_string()
}

const BASE: &str = "\
mod M(inout p : Electrical, inout gnd : Electrical) {
    param vmax : Real = 3.3;
}
analog M { V(p, gnd) <+ vmax; }
";

/// DVC-19: the four scope forms parse — `@ dc`, `@ tran`, `@ ac`, and the
/// `|` composition `@ (dc | tran)`. The bodies nest authored constraint
/// statements, recursively.
#[test]
fn analysis_scope_event_blocks_parse() {
    let c = constraint_of(&format!(
        "{BASE}\n\
         constraint M {{
             @ dc {{ require dc_ok : V(p, gnd) <= vmax; }}
             @ tran {{ require tran_ok : V(p, gnd) <= vmax; }}
             @ ac {{ require ac_ok : V(p, gnd) <= vmax; }}
             @ (dc | tran) {{ require dc_or_tran : V(p, gnd) <= vmax; }}
         }}"
    ));
    let event_blocks: Vec<_> = c
        .body
        .iter()
        .map(|s| match s {
            ConstraintStmt::Event { spec, body, .. } => (spec.clone(), body.len()),
            other => panic!("expected an event block, got {other:?}"),
        })
        .collect();
    assert_eq!(event_blocks.len(), 4, "one @ block per analysis scope form");
    // `@ dc { … }` — a bare Named term, no args.
    let EventSpec::Named { name, args } = &event_blocks[0].0 else {
        panic!("`@ dc` parses to a Named term");
    };
    assert_eq!(name, "dc");
    assert!(args.is_empty(), "`@ dc` carries no args");
    assert_eq!(event_blocks[0].1, 1, "one statement in the body");
    // `@ (dc | tran) { … }` — Or of two bare Named terms.
    let EventSpec::Or(specs) = &event_blocks[3].0 else {
        panic!("`@ (dc | tran)` parses to an Or");
    };
    assert_eq!(specs.len(), 2, "two terms in the union");
}

/// DVC-19: a top-level `require` and a scoped one coexist; the scoped one
/// records its scope's analysis on its `StmtClass.analyses` (union of
/// helper analyses and enclosing scope analyses), the top-level one
/// records `[]` (holds everywhere, subject to D2's `Context` default).
#[test]
fn scoped_require_carries_its_analyses_top_level_is_unscoped() {
    let design = elaborate(&format!(
        "{BASE}\n\
         constraint M {{
             require everywhere : V(p, gnd) <= vmax;
             @ dc {{ require dc_only : V(p, gnd) <= vmax; }}
             @ (dc | tran) {{ require dc_or_tran : V(p, gnd) <= vmax; }}
         }}"
    ))
    .expect("fixture must elaborate");
    let block = &design.module("M").unwrap().constraints()[0];

    let everywhere = block.class_of("everywhere").expect("top-level require classified");
    assert_eq!(everywhere.class, QuantityClass::Pointwise);
    assert!(everywhere.analyses.is_empty(), "top-level require holds everywhere (D2)");

    let dc_only = block.class_of("dc_only").expect("@ dc require classified");
    assert_eq!(dc_only.analyses.as_slice(), &["dc".to_string()], "scoped to dc only");

    let dc_or_tran = block.class_of("dc_or_tran").expect("@ (dc | tran) require classified");
    assert_eq!(
        dc_or_tran.analyses.as_slice(),
        &["dc".to_string(), "tran".to_string()],
        "`|` composes the analyses set"
    );
}

/// DVC-19: `dc`/`tran`/`ac` resolve as event terms through the existing
/// `EventRegistry` — an unknown name or a non-analysis-scope event
/// (cross/above/timer/posedge) fails loud naming the term.
#[test]
fn non_analysis_event_kinds_in_a_constraint_event_block_fail_loud() {
    // Unknown name.
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ nope {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(msg.contains("unknown event `nope`"), "must name the unknown event: {msg}");
    assert!(msg.contains("dc") && msg.contains("tran") && msg.contains("ac"), "must list the legal kinds: {msg}");

    // A runtime event name from the existing registry (cross) — analysis
    // scoping accepts the analysis kinds only (T29 owns temporal windows).
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ cross(V(p, gnd)) {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(msg.contains("not an analysis-scope marker"), "must refuse a runtime event: {msg}");

    // `initial` / `final` are not analysis kinds.
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ initial {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(msg.contains("`initial`/`final`"), "must refuse the lifecycle markers: {msg}");

    // An analysis-scope event with arguments.
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ dc(V(p, gnd)) {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(msg.contains("`dc` takes no arguments"), "analysis kinds are bare names: {msg}");
}

/// DVC-19: `dc`/`tran`/`ac` are runtime-event-irrelevant — used in a
/// behavior body's `@ … { … }` (analog/digital), they fail loud as
/// "analysis-scope marker, not a runtime event".
#[test]
fn analysis_scope_events_are_rejected_in_behavior_bodies() {
    let src = "\
mod M(inout p : Electrical, inout gnd : Electrical) { param vmax : Real = 3.3; }\n\
analog M { @ dc { V(p, gnd) <+ vmax; } }\n";
    let msg = piperine_lang::parse_and_elaborate(
        &format!("use piperine::disciplines;\n{src}"),
        &piperine_lang::SourceMap::dummy(),
    )
    .expect_err("must fail elaboration")
    .to_string();
    assert!(
        msg.contains("analysis-scope marker") && msg.contains("runtime event"),
        "must refuse dc/tran/ac in a behavior body: {msg}"
    );
}

/// DVC-19: nested event blocks compose — `@ dc { @ tran { … } }` produces
/// a require whose analyses set is the union of both scopes. (A fixture
/// for the algebra; whether nesting is *useful* is the user's call.)
#[test]
fn nested_event_blocks_compose_their_analyses() {
    let design = elaborate(&format!(
        "{BASE}\n\
         constraint M {{
             @ dc {{
                 @ tran {{ require dc_then_tran : V(p, gnd) <= vmax; }}
             }}
         }}"
    ))
    .expect("fixture must elaborate");
    let block = &design.module("M").unwrap().constraints()[0];
    let nested = block.class_of("dc_then_tran").expect("nested require classified");
    assert_eq!(
        nested.analyses.as_slice(),
        &["dc".to_string(), "tran".to_string()],
        "inner scope unions with outer scope's analyses"
    );
}

/// DVC-04 AC6 (recursion): a `require` and a `var` may not share a label
/// across the whole tree — even across an event block boundary (a top-
/// level `var x` and `@ dc { var x : … }` clash).
#[test]
fn duplicate_labels_spanning_event_blocks_fail_loud() {
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{
             var x : Real = 1.0;
             @ dc {{ var x : Real = 2.0; }}
         }}"
    ));
    assert!(msg.contains("duplicate statement label `x`"), "must name the duplicate: {msg}");
}

/// DVC-19: a `var` declared in an outer scope is visible to a `require`
/// inside an inner event block — the scope chain is preserved, exactly as
/// a top-level statement sees the vars before it.
#[test]
fn vars_declared_outside_an_event_block_are_visible_inside() {
    let design = elaborate(&format!(
        "{BASE}\n\
         constraint M {{
             var headroom : Real = vmax - V(p, gnd);
             @ dc {{ require positive : headroom >= 0.0; }}
         }}"
    ))
    .expect("fixture must elaborate");
    let block = &design.module("M").unwrap().constraints()[0];
    let positive = block.class_of("positive").expect("scoped require classified");
    assert_eq!(positive.analyses.as_slice(), &["dc".to_string()]);
}

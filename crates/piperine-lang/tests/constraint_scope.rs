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
    let EventSpec::Named { name, args, window } = &event_blocks[0].0 else {
        panic!("`@ dc` parses to a Named term");
    };
    assert_eq!(name, "dc");
    assert!(args.is_empty(), "`@ dc` carries no args");
    assert!(window.is_none(), "`@ dc` carries no window");
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

    // An analysis-scope event with positional arguments. T29 renamed the
    // message to "positional arguments" — `after =`/`dur =` window args
    // are now legal on an analysis-scope term (DVC-20).
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ dc(V(p, gnd)) {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(msg.contains("`dc` takes no positional arguments"), "analysis kinds take no positional args (only `after=`/`dur=` window args, T29): {msg}");
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

// ────────────────────── dv-core T29 (DVC-20) ──────────────────────────────

/// DVC-20: the four window forms parse — `@ tran(after = …, dur = …)`,
/// `after`-only, `dur`-only, and bare `tran` (no window). The window
/// attaches to the `Named` term, mirroring how `timer(period, phase)`
/// already carries a second argument.
#[test]
fn window_named_args_parse_on_an_analysis_scope_term() {
    let c = constraint_of(&format!(
        "{BASE}\n\
         constraint M {{
             @ tran(after = 5e-6, dur = 1e-6) {{ require a : V(p, gnd) <= vmax; }}
             @ tran(after = 5e-6) {{ require b : V(p, gnd) <= vmax; }}
             @ tran(dur = 1e-6) {{ require c : V(p, gnd) <= vmax; }}
             @ tran {{ require d : V(p, gnd) <= vmax; }}
         }}"
    ));
    let windows: Vec<_> = c
        .body
        .iter()
        .map(|s| match s {
            ConstraintStmt::Event { spec, .. } => match spec {
                EventSpec::Named { window, .. } => window.as_ref().map(|w| (w.after.is_some(), w.dur.is_some())),
                other => panic!("expected a Named term, got {other:?}"),
            },
            other => panic!("expected an event block, got {other:?}"),
        })
        .collect();
    assert_eq!(windows.len(), 4);
    assert_eq!(windows[0], Some((true, true)), "after + dur");
    assert_eq!(windows[1], Some((true, false)), "after only");
    assert_eq!(windows[2], Some((false, true)), "dur only");
    assert_eq!(windows[3], None, "no window");
}

/// DVC-20: `&` parses as `EventSpec::And`, `not` as `EventSpec::Not`. The
/// three operators may mix with the existing `|` (`Or`); precedence is
/// `|` < `&` < `not` (loosest to tightest).
#[test]
fn intersection_and_complement_parse_as_new_event_spec_variants() {
    use piperine_lang::parse::ast::EventSpec;
    let c = constraint_of(&format!(
        "{BASE}\n\
         constraint M {{
             @ (dc & tran) {{ require a : V(p, gnd) <= vmax; }}
             @ not dc {{ require b : V(p, gnd) <= vmax; }}
             @ (dc | tran & not ac) {{ require c : V(p, gnd) <= vmax; }}
         }}"
    ));
    let specs: Vec<&EventSpec> = c
        .body
        .iter()
        .map(|s| match s {
            ConstraintStmt::Event { spec, .. } => spec,
            other => panic!("expected an event block, got {other:?}"),
        })
        .collect();
    assert!(matches!(specs[0], EventSpec::And(_)), "`&` parses to And");
    assert!(matches!(specs[1], EventSpec::Not(_)), "`not` parses to Not");
    // The third spec mixes the three operators: an Or whose right side is
    // `tran & not ac` (an And of `tran` and `Not(ac)`).
    let EventSpec::Or(branches) = specs[2] else {
        panic!("expected an Or at the top level: {specs:?}", );
    };
    assert_eq!(branches.len(), 2, "two branches at the `|` level");
    assert!(matches!(branches[1], EventSpec::And(_)), "right branch is an And");
}

/// DVC-20 AC1: a window's `after`/`dur` evaluate as Real elaboration
/// constants. They land on every leaf's `StmtClass.window` (here only the
/// `require`, since the block has one leaf) as a `ResolvedWindow`.
#[test]
fn window_after_and_dur_resolve_onto_the_leaf_class() {
    let design = elaborate(&format!(
        "{BASE}\n\
         constraint M {{
             @ tran(after = 5e-6, dur = 1e-6) {{ require settle : V(p, gnd) <= vmax; }}
         }}"
    ))
    .expect("fixture must elaborate");
    let block = &design.module("M").unwrap().constraints()[0];
    let settle = block.class_of("settle").expect("windowed require classified");
    assert_eq!(settle.analyses.as_slice(), &["tran".to_string()]);
    let window = settle.window.expect("the window resolves onto the leaf");
    assert!((window.after - 5e-6).abs() < 1e-15, "after = 5e-6: got {}", window.after);
    assert_eq!(window.dur, Some(1e-6), "dur = 1e-6");
    // The window's `contains` predicate — the runtime filter.
    assert!(!window.contains(0.0), "before the window");
    assert!(window.contains(5e-6), "at the open boundary");
    assert!(window.contains(6e-6), "inside");
    assert!(window.contains(6e-6), "at the close boundary");
}

/// DVC-20 AC2: `&` is set intersection. `dc & tran` is empty (the two
/// analyses are disjoint), so the inner require carries the empty
/// analyses set and is "not exercised" at runtime — never a silent
/// vacuous pass.
#[test]
fn and_intersects_the_analyses_set() {
    let design = elaborate(&format!(
        "{BASE}\n\
         constraint M {{
             @ (dc & tran) {{ require both : V(p, gnd) <= vmax; }}
         }}"
    ))
    .expect("fixture must elaborate");
    let block = &design.module("M").unwrap().constraints()[0];
    let both = block.class_of("both").expect("intersection require classified");
    assert!(both.analyses.is_empty(), "`dc & tran` intersects to empty: got {:?}", both.analyses);
}

/// DVC-20 AC2: `not` is set complement against the universe `{dc, tran,
/// ac}`. `not dc` resolves to `{tran, ac}`.
#[test]
fn not_complements_the_analyses_set() {
    let design = elaborate(&format!(
        "{BASE}\n\
         constraint M {{
             @ not dc {{ require not_dc : V(p, gnd) <= vmax; }}
         }}"
    ))
    .expect("fixture must elaborate");
    let block = &design.module("M").unwrap().constraints()[0];
    let not_dc = block.class_of("not_dc").expect("complement require classified");
    assert_eq!(
        not_dc.analyses.as_slice(),
        &["ac".to_string(), "tran".to_string()],
        "`not dc` complements against {{dc, tran, ac}} → {{ac, tran}}"
    );
}

/// DVC-20 AC1: a misspelled window arg fails loud at parse — only
/// `after`/`dur` are legal names. The error names the offender.
#[test]
fn misspelled_window_arg_fails_loud_at_parse() {
    let src = format!(
        "{BASE}\n\
         constraint M {{ @ tran(affter = 5e-6) {{ require x : V(p, gnd) <= vmax; }} }}"
    );
    piperine_lang::parse_str(&src).expect_err("must fail — `affter` is not a legal window arg");
}

/// DVC-20 AC1: a non-Real `after` fails loud at elaboration naming the
/// offending kind. Event-term `after` (e.g. `cross(...)`) is recorded as
/// "not yet supported" by T29's runtime scope.
#[test]
fn non_real_window_bound_fails_loud_at_elaboration() {
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ tran(after = \"oops\", dur = 1e-6) {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(
        msg.contains("window `after`") && msg.contains("Real"),
        "must name the offending bound and the expected type: {msg}"
    );
}

/// DVC-20: a negative `dur` fails loud — the window's duration is
/// non-negative by construction.
#[test]
fn negative_window_dur_fails_loud() {
    let msg = elab_err(&format!(
        "{BASE}\n\
         constraint M {{ @ tran(after = 5e-6, dur = -1e-6) {{ require x : V(p, gnd) <= vmax; }} }}"
    ));
    assert!(
        msg.contains("dur") && msg.contains("non-negative"),
        "must name `dur` and the non-negative rule: {msg}"
    );
}

/// DVC-20: a window on a behavior-body `@` block fails loud — windows
/// are constraint-only (a runtime body has no evaluation-point set to
/// filter). The trigger is a runtime event (`cross`); the window is the
/// offending piece, not the trigger itself.
#[test]
fn window_on_a_behavior_body_event_block_fails_loud() {
    let src = "\
mod M(inout p : Electrical, inout gnd : Electrical) { param vmax : Real = 3.3; }\n\
analog M { @ cross(V(p, gnd), after = 5e-6, dur = 1e-6) { V(p, gnd) <+ vmax; } }\n";
    let msg = piperine_lang::parse_and_elaborate(
        &format!("use piperine::disciplines;\n{src}"),
        &piperine_lang::SourceMap::dummy(),
    )
    .expect_err("must fail elaboration")
    .to_string();
    assert!(
        msg.contains("temporal window") && msg.contains("constraint"),
        "must refuse a window in a behavior body: {msg}"
    );
}

/// DVC-20: `&` / `not` on a behavior-body `@` block fail loud — the
/// algebra over evaluation-point sets is constraint-only.
#[test]
fn algebra_on_a_behavior_body_event_block_fails_loud() {
    let src = "\
mod M(inout p : Electrical, inout gnd : Electrical) { param vmax : Real = 3.3; }\n\
analog M { @ (posedge(clk) & initial) { V(p, gnd) <+ vmax; } }\n\
mod C(inout clk : Electrical) {}\n";
    let msg = piperine_lang::parse_and_elaborate(
        &format!("use piperine::disciplines;\n{src}"),
        &piperine_lang::SourceMap::dummy(),
    )
    .expect_err("must fail elaboration")
    .to_string();
    assert!(
        msg.contains("intersection") && msg.contains("constraint"),
        "must refuse `&` in a behavior body: {msg}"
    );
}

//! LRM-COVERS: 6.7, 6.2.1
//!
//! LRMW2-42 (`lrm-conformance-wave2`, T50): §6.7 Syntax 6-9
//! (`hierarchical_identifier ::= [ $root . ] { identifier '.' } identifier`)
//! and §6.2.1's `$root` disambiguation marker. Before this task,
//! `grammar/expr.rs::parse_primary` had no `.`-postfix handling on a bare
//! identifier at all (`u1.u2.net` parsed `u1` then choked on the trailing
//! `.u2.net`), and `$root` lexed as an ordinary `Tok::SystemIdent`, parsed
//! into a zero-arg `$root()` system-function call rather than a
//! hierarchical-path root. This file covers the grammar/POM layer only —
//! codegen's *resolution* of a parsed `Expr::Hier` against
//! `Design::flat_modules` is `crates/piperine-codegen/tests/
//! hierarchical_ref_resolution.rs`; the simulated end-to-end proof is root
//! `tests/hierarchical_ref_e2e.rs`.

use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

const FIXTURE: &str = r#"
    module leaf (inout electrical p, inout electrical gnd);
        analog V(p, gnd) <+ 3.0;
    endmodule
    module top (inout electrical gnd);
        electrical outnode1, outnode2;
        leaf u1 (outnode1, gnd);
        analog begin
            V(outnode1) <+ V(u1.p, gnd);
            V(outnode2) <+ V($root.top.u1.p, gnd);
        end
    endmodule
"#;

fn contribution_value<'a>(stmts: &'a [Stmt], target_pos: &str) -> &'a Expr {
    for s in stmts {
        if let Stmt::Contribution { target: Expr::AccessFunction { pos, .. }, value, .. } = s
            && let Expr::Ident { name, .. } = pos.as_ref()
            && name == target_pos
        {
            return value;
        }
        if let Stmt::Block { stmts, .. } = s {
            return contribution_value(stmts, target_pos);
        }
    }
    panic!("no contribution to `{target_pos}` found");
}

/// §6.7: an un-rooted `identifier '.' identifier` parses as
/// `Expr::Hier { root: false, path }`, reaching the POM unchanged
/// (elaboration performs no rewriting of `Hier`, unlike `PortAccess` —
/// resolution against `Design::flat_modules` is a codegen-only concern,
/// T50's own design constraint).
#[test]
fn local_dotted_path_parses_as_hier_with_root_false() {
    let d = elab(FIXTURE);
    let m = d.module("top").expect("top elaborated");
    let analog = m.behaviors().iter().find(|b| b.is_analog()).expect("an analog behavior");
    let value = contribution_value(analog.body(), "outnode1");
    let inner = match value {
        Expr::AccessFunction { pos, .. } => pos.as_ref(),
        other => panic!("expected V(...) wrapping the Hier arg, got {other:?}"),
    };
    match inner {
        Expr::Hier { root, path, .. } => {
            assert!(!root, "`u1.p` (no `$root.` prefix) must parse with root: false");
            assert_eq!(path, &vec!["u1".to_string(), "p".to_string()]);
        }
        other => panic!("expected Expr::Hier for `u1.p`, got {other:?}"),
    }
}

/// §6.2.1: a `$root.`-prefixed path parses as `Expr::Hier { root: true,
/// path }`, distinguishing `$root` (a hierarchical-path root marker) from
/// every other `$name` system-function call — `$root` alone with no
/// trailing `.` (never used this way per the grammar) still falls through
/// to an ordinary (unresolvable) system-function call rather than being
/// silently special-cased.
#[test]
fn root_prefixed_path_parses_as_hier_with_root_true() {
    let d = elab(FIXTURE);
    let m = d.module("top").expect("top elaborated");
    let analog = m.behaviors().iter().find(|b| b.is_analog()).expect("an analog behavior");
    let value = contribution_value(analog.body(), "outnode2");
    let inner = match value {
        Expr::AccessFunction { pos, .. } => pos.as_ref(),
        other => panic!("expected V(...) wrapping the Hier arg, got {other:?}"),
    };
    match inner {
        Expr::Hier { root, path, .. } => {
            assert!(*root, "`$root.top.u1.p` must parse with root: true");
            assert_eq!(path, &vec!["top".to_string(), "u1".to_string(), "p".to_string()]);
        }
        other => panic!("expected Expr::Hier for `$root.top.u1.p`, got {other:?}"),
    }
}

/// MD-25: parsing/elaborating a `Hier` reference must not touch the
/// authored instance/hierarchy structure at all — `Design::modules` still
/// shows exactly the one authored instance, unchanged by the presence of a
/// hierarchical reference to its port.
#[test]
fn hier_reference_does_not_alter_authored_pom_structure() {
    let d = elab(FIXTURE);
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances.len(), 1, "top must still authored-instantiate exactly one instance");
    assert_eq!(top.instances[0].module, "leaf");
    assert_eq!(top.instances[0].label.as_deref(), Some("u1"));
}

/// A bare `$root` with nothing following (never legal per §6.7 Syntax 6-9's
/// `[ $root . ]` — the `.` is part of the production, not optional once
/// `$root` is used) is disambiguated at parse time by what follows it:
/// with no trailing `.`, `parse_primary` never takes the `Hier` branch at
/// all — it parses as an ordinary `$root(...)` system-function call, same
/// as any other unrecognized `$name`, rather than being silently accepted
/// as a hierarchical path with an empty tail.
#[test]
fn bare_root_with_no_trailing_dot_is_not_treated_as_a_hier_path() {
    let src = r#"
        module top (inout electrical a);
            analog V(a) <+ $root;
        endmodule
    "#;
    let d = elab(src);
    let m = d.module("top").expect("top elaborated");
    let analog = m.behaviors().iter().find(|b| b.is_analog()).expect("an analog behavior");
    let value = contribution_value(analog.body(), "a");
    assert!(
        matches!(value, Expr::SystemFunctionCall { name, .. } if name == "root"),
        "bare `$root` (no trailing `.`) must parse as an ordinary SystemFunctionCall, not Hier; got {value:?}"
    );
}

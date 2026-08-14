//! Ch.10 compiler-directive state (design.md P0.3) — LRMC-C10-01/02's
//! parse half: `` `default_discipline ``/`` `default_transition `` write
//! `DirectiveState` instead of raising `UndefinedMacro`, with §10.2/§10.3's
//! own set/override/clear scope semantics.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// Before this fix, any unrecognized `` `name `` (including these two
/// ch.10 directives) fell through to `handle_macro_invocation`/
/// `UndefinedMacro`, so elaboration itself would have failed with that
/// error. This is the regression guard for that bug class: elaboration
/// must now succeed (`elab` panics with the error text if it doesn't).
#[test]
fn default_discipline_no_longer_raises_undefined_macro() {
    let _ = elab("`default_discipline ddiscrete\nmodule m(); endmodule\n");
}

#[test]
fn default_transition_no_longer_raises_undefined_macro() {
    let _ = elab("`default_transition 1e-9\nmodule m(); endmodule\n");
}

/// LRM §10.2's own worked example form (`` `default_discipline ddiscrete ``)
/// — the directive sets `DirectiveState.default_discipline`.
#[test]
fn default_discipline_sets_the_named_discipline() {
    let d = elab("`default_discipline ddiscrete\nmodule m(); endmodule\n");
    assert_eq!(d.directives().default_discipline.as_deref(), Some("ddiscrete"));
}

/// A second `` `default_discipline `` directive overrides the first — LRM
/// §10.2: in force "until ... another `` `default_discipline `` directive
/// ... is found in the subsequent text."
#[test]
fn a_second_default_discipline_directive_overrides_the_first() {
    let d = elab(
        "`default_discipline ddiscrete\n`default_discipline logic\nmodule m(); endmodule\n",
    );
    assert_eq!(d.directives().default_discipline.as_deref(), Some("logic"));
}

/// A bare `` `default_discipline `` (no name) clears the default — LRM
/// §10.2: "if this directive is used without a discipline name,
/// discipline resolution will not use a default discipline ... after this
/// directive is encountered."
#[test]
fn a_bare_default_discipline_directive_clears_the_default() {
    let d = elab(
        "`default_discipline ddiscrete\n`default_discipline\nmodule m(); endmodule\n",
    );
    assert_eq!(d.directives().default_discipline, None);
}

/// No directive at all: the default stays unset.
#[test]
fn no_default_discipline_directive_leaves_it_unset() {
    let d = elab("module m(); endmodule\n");
    assert_eq!(d.directives().default_discipline, None);
}

/// LRM §10.3's own semantics: `transition_time` sets both the default rise
/// and fall time.
#[test]
fn default_transition_sets_rise_and_fall_to_the_same_value() {
    let d = elab("`default_transition 2.5e-9\nmodule m(); endmodule\n");
    assert_eq!(d.directives().default_transition, Some((2.5e-9, 2.5e-9)));
}

/// A second `` `default_transition `` directive overrides the first — LRM
/// §10.3: "the default rise and fall times ... are derived from the
/// transition_time value of the directive which immediately precedes."
#[test]
fn a_second_default_transition_directive_overrides_the_first() {
    let d = elab("`default_transition 1e-9\n`default_transition 4e-9\nmodule m(); endmodule\n");
    assert_eq!(d.directives().default_transition, Some((4e-9, 4e-9)));
}

/// No directive at all: `default_transition` stays unset (LRM §10.3: "If a
/// `` `default_transition `` directive is not used ... transition_time is
/// controlled by the simulator").
#[test]
fn no_default_transition_directive_leaves_it_unset() {
    let d = elab("module m(); endmodule\n");
    assert_eq!(d.directives().default_transition, None);
}

/// A malformed `default_transition` argument fails loud with a typed error
/// naming the directive, instead of silently defaulting to some value.
#[test]
fn malformed_default_transition_argument_fails_loud() {
    let msg = elab_err("`default_transition not_a_number\nmodule m(); endmodule\n");
    assert!(
        msg.to_lowercase().contains("default_transition"),
        "error must name the malformed directive, got: {msg}"
    );
}

/// A negative-exponent scientific-notation form parses correctly (a
/// realistic `transition_time`, e.g. picoseconds).
#[test]
fn default_transition_accepts_scientific_notation() {
    let d = elab("`default_transition 100e-12\nmodule m(); endmodule\n");
    assert_eq!(d.directives().default_transition, Some((100e-12, 100e-12)));
}

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

// ── T52: 1364-inherited directives (LRMC-C10-05..09) ─────────────────────

/// None of celldefine/endcelldefine/line/pragma/default_nettype/
/// unconnected_drive/nounconnected_drive raise `UndefinedMacro` anymore.
#[test]
fn all_1364_inherited_directives_no_longer_raise_undefined_macro() {
    let _ = elab(
        "`celldefine\n\
         module m(); endmodule\n\
         `endcelldefine\n\
         `line 100 \"other.v\" 0\n\
         `pragma foo bar=1\n\
         `default_nettype none\n\
         `unconnected_drive pull1\n\
         `nounconnected_drive\n\
         `default_nettype wire\n",
    );
}

#[test]
fn celldefine_endcelldefine_records_the_enclosed_span() {
    let d = elab("`celldefine\nmodule m(); endmodule\n`endcelldefine\n`default_nettype wire\n");
    assert_eq!(d.directives().celldefine_spans.len(), 1);
}

#[test]
fn default_nettype_none_makes_an_undeclared_net_an_error() {
    let msg = elab_err(
        "`default_nettype none\n\
         module leaf (inout electrical p, inout electrical n); endmodule\n\
         module top (); leaf u1 (undeclared_net, undeclared_net); endmodule\n",
    );
    assert!(msg.contains("undeclared_net"), "must name the offending net: {msg}");
    assert!(msg.to_lowercase().contains("default_nettype"), "must cite the directive: {msg}");
}

/// The same source with the default (implicit `wire`) `default_nettype`
/// elaborates fine — proves the `none` case above is the directive's
/// effect, not some unrelated regression.
#[test]
fn without_default_nettype_none_an_undeclared_net_is_still_implicit() {
    let d = elab(
        "module leaf (inout electrical p, inout electrical n); endmodule\n\
         module top (); leaf u1 (undeclared_net, undeclared_net); endmodule\n",
    );
    assert!(d.module("top").is_some());
}

#[test]
fn unconnected_drive_sets_pull0_or_pull1() {
    use piperine_lang::directives::Drive;
    let d = elab("`unconnected_drive pull0\nmodule m(); endmodule\n");
    assert_eq!(d.directives().unconnected_drive, Some(Drive::Pull0));

    let d = elab("`unconnected_drive pull1\nmodule m(); endmodule\n");
    assert_eq!(d.directives().unconnected_drive, Some(Drive::Pull1));
}

#[test]
fn nounconnected_drive_clears_it() {
    let d = elab("`unconnected_drive pull1\n`nounconnected_drive\nmodule m(); endmodule\n");
    assert_eq!(d.directives().unconnected_drive, None);
}

/// `` `line `` remaps the line number reported for source after it —
/// tested directly against `DirectiveState::line_for_offset` (the
/// preprocessor's own remapping helper) rather than end-to-end error
/// text, since that keeps the assertion tied to the directive's actual
/// recorded state instead of a diagnostic renderer's formatting.
#[test]
fn line_directive_remaps_reported_line_numbers() {
    use piperine_lang::directives::DirectiveState;
    use piperine_lang::preprocessor::Preprocessor;

    let src = "`line 100 \"other.v\" 0\nsecond_line\nthird_line\n";
    let mut pp = Preprocessor::new();
    let expanded = pp.run(src).expect("preprocesses");
    let directives: &DirectiveState = pp.directives();

    // Offset of "second_line" in the *expanded* text (the directive itself
    // is consumed, not passed through) should read back as line 100; the
    // following "third_line" should read back as line 101.
    let second_offset = expanded.find("second_line").unwrap();
    let third_offset = expanded.find("third_line").unwrap();
    assert_eq!(directives.line_for_offset(&expanded, second_offset), 100);
    assert_eq!(directives.line_for_offset(&expanded, third_offset), 101);
}

/// Without any `` `line `` directive, `line_for_offset` falls back to a
/// plain 1-based physical line count.
#[test]
fn line_for_offset_without_a_line_directive_is_the_physical_line() {
    use piperine_lang::directives::DirectiveState;
    let d = DirectiveState::default();
    let src = "one\ntwo\nthree\n";
    assert_eq!(d.line_for_offset(src, 0), 1);
    assert_eq!(d.line_for_offset(src, src.find("two").unwrap()), 2);
    assert_eq!(d.line_for_offset(src, src.find("three").unwrap()), 3);
}

// ── T53: `timescale`/`resetall` real behavior (LRMC-C10-10) ─────────────

/// Before this fix, `` `timescale`` text was pushed through untouched by
/// the preprocessor and lexed as an orphan `Tok::Directive` nothing in
/// the grammar consumed, so a file containing it failed to parse.
#[test]
fn a_file_containing_timescale_parses() {
    let d = elab("`timescale 1ns/1ps\nmodule m(); endmodule\n");
    assert!(d.module("m").is_some());
}

#[test]
fn timescale_1ns_1ps_records_the_expected_exponents() {
    use piperine_lang::directives::Timescale;
    let d = elab("`timescale 1ns/1ps\nmodule m(); endmodule\n");
    assert_eq!(d.directives().timescale, Some(Timescale { unit_exponent: -9, precision_exponent: -12 }));
}

/// LRM §6991's own worked value (`` `timescale 1ns / 100ps``) — with
/// spaces around the slash, which the scanner must tolerate.
#[test]
fn timescale_tolerates_spaces_around_the_slash() {
    use piperine_lang::directives::Timescale;
    let d = elab("`timescale 1ns / 100ps\nmodule m(); endmodule\n");
    assert_eq!(d.directives().timescale, Some(Timescale { unit_exponent: -9, precision_exponent: -10 }));
}

/// `` `timescale ``'s recorded state feeds `%t`'s default rendering
/// (before any explicit `$timeformat` call) via `TimeFormat::from_timescale`.
#[test]
fn timescale_feeds_the_default_time_format() {
    use piperine_lang::directives::Timescale;
    use piperine_lang::eval::tasks::{TimeFormat, format_time};

    let ts = Timescale { unit_exponent: -9, precision_exponent: -12 };
    let fmt = TimeFormat::from_timescale(&ts);
    assert_eq!(fmt.suffix, "ns");
    assert_eq!(fmt.precision, 3, "unit ns (-9) to precision ps (-12) is 3 fractional digits");
    assert_eq!(format_time(2.5e-9, &fmt), "2.500ns");
}

/// `` `resetall `` clears `default_discipline`/`default_nettype`/
/// `unconnected_drive` (§10.2's own explicit "in addition to resetall"
/// clause), and — like `timescale` — leaves no orphan `Tok::Directive`
/// behind for the grammar to choke on.
#[test]
fn resetall_clears_directive_state_and_parses() {
    use piperine_lang::directives::NetTypeDefault;
    let d = elab(
        "`default_discipline ddiscrete\n\
         `default_nettype none\n\
         `unconnected_drive pull1\n\
         `resetall\n\
         module m(); endmodule\n",
    );
    assert_eq!(d.directives().default_discipline, None);
    assert_eq!(d.directives().default_nettype, NetTypeDefault::Wire);
    assert_eq!(d.directives().unconnected_drive, None);
}

// ── T54: §10.5 predefined macros (LRMC-C10-03) ───────────────────────────

/// The LRM's own §10.5 portability idiom (Example, `not_gate`, adapted so
/// the branches are numerically distinguishable): the `` `ifdef
/// __VAMS_ENABLE__`` branch, with its `from [1:100]` range clause, is
/// taken rather than the plain, unconstrained `` `else`` branch — proven
/// by a default deliberately *outside* that range failing elaboration
/// with a range-violation error. If the `else` branch (no `from` clause)
/// ran instead, this would elaborate cleanly.
#[test]
fn vams_enable_ifdef_takes_the_ams_branch() {
    let msg = elab_err(
        "module not_gate (input in, output out);\n\
         `ifdef __VAMS_ENABLE__\n\
         parameter integer del = 200 from [1:100];\n\
         `else\n\
         parameter integer del = 200;\n\
         `endif\n\
         endmodule\n",
    );
    assert!(msg.contains("del"), "must name the out-of-range param: {msg}");
}

#[test]
fn vams_compact_modeling_ifdef_takes_the_compact_branch() {
    let msg = elab_err(
        "module m ();\n\
         `ifdef __VAMS_COMPACT_MODELING__\n\
         parameter integer compact = 200 from [1:100];\n\
         `else\n\
         parameter integer compact = 200;\n\
         `endif\n\
         endmodule\n",
    );
    assert!(msg.contains("compact"), "must name the out-of-range param: {msg}");
}

/// §10.5: "the `undef` compiler directive shall have no effect on
/// predefined Verilog-AMS macros" — an attempted `` `undef __VAMS_ENABLE__``
/// does not clear it; the `ifdef` branch (and its range enforcement) is
/// still taken afterward.
#[test]
fn undef_has_no_effect_on_predefined_macros() {
    let msg = elab_err(
        "`undef __VAMS_ENABLE__\n\
         module not_gate (input in, output out);\n\
         `ifdef __VAMS_ENABLE__\n\
         parameter integer del = 200 from [1:100];\n\
         `else\n\
         parameter integer del = 200;\n\
         `endif\n\
         endmodule\n",
    );
    assert!(msg.contains("del"), "undef must not have cleared __VAMS_ENABLE__: {msg}");
}

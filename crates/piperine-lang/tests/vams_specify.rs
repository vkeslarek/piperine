//! LRM-COVERS: A.7.1, A.7.2, A.7.3, A.7.4, A.7.5, A.7.5.1, A.7.5.2, A.7.5.3
//! `specify` blocks (T15/T16/T17, LRMW3-06/LRMW3-07): real grammar
//! productions for `specparam` declarations and simple path declarations
//! (parallel `=>` and full `*>`, 1/2/3/6/12 delay values, T15), the eight
//! in-scope system timing checks (T16), and named loud-error recognition
//! for the forms Piperine does not lower — edge-sensitive/state-dependent
//! paths, `PATHPULSE$`, `pulsestyle_*`, `showcancelled`/`noshowcancelled`
//! (T17) — replacing the old token-skipping stub. T15/T16/T17's own tests
//! read the parsed AST directly (`Parser`); T17's fail-loud tests also run
//! the full `parse_and_elaborate` pipeline, since the loud error fires at
//! elaboration (`ElabErrorKind::UnsupportedConstruct`), not at parse time.
//! POM lowering of the *supported* forms is T18's target.

use piperine_lang::ast::{Item, ModuleItem, SpecifyBlock, TimingCheckKind};
use piperine_lang::{Parser, SourceMap};

fn parse_specify(module_src: &str) -> SpecifyBlock {
    let full = format!(
        "module top(a, y, clk, d);\n\
         inout a, y, clk, d;\n\
         {module_src}\n\
         endmodule\n"
    );
    let file = Parser::new().parse(&full).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    for item in &file.items {
        if let Item::Module(m) = item {
            for mi in &m.items {
                if let ModuleItem::Specify(sb) = mi {
                    return sb.clone();
                }
            }
        }
    }
    panic!("no specify block found in parsed module");
}

#[test]
fn parallel_path_with_two_delays_parses() {
    let sb = parse_specify("specify\n(a => y) = (1.0, 1.0);\nendspecify");
    assert_eq!(sb.paths.len(), 1);
    let p = &sb.paths[0];
    assert!(!p.full, "=> is the parallel form");
    assert_eq!(p.inputs, vec!["a".to_string()]);
    assert_eq!(p.outputs, vec!["y".to_string()]);
    assert_eq!(p.delays.len(), 2);
}

#[test]
fn full_connection_path_parses_input_and_output_lists() {
    let sb = parse_specify("specify\n(a, clk *> y, d) = 1.5;\nendspecify");
    assert_eq!(sb.paths.len(), 1);
    let p = &sb.paths[0];
    assert!(p.full, "*> is the full-connection form");
    assert_eq!(p.inputs, vec!["a".to_string(), "clk".to_string()]);
    assert_eq!(p.outputs, vec!["y".to_string(), "d".to_string()]);
    assert_eq!(p.delays.len(), 1);
}

#[test]
fn specparam_inside_specify_block_parses() {
    let sb = parse_specify("specify\nspecparam tr = 1.0;\nendspecify");
    assert_eq!(sb.specparams.len(), 1);
    assert_eq!(sb.specparams[0].names[0].0, "tr");
}

#[test]
fn specify_block_is_no_longer_an_unread_span() {
    // Every item recognized by the grammar lands on a typed list — the
    // old `items: vec![]` shape (a raw skipped span) is gone.
    let sb = parse_specify(
        "specify\n\
         specparam tr = 1.0;\n\
         (a => y) = (1.0, 1.0);\n\
         endspecify",
    );
    assert_eq!(sb.specparams.len() + sb.paths.len(), 2, "every specify_item lands on a typed list");
}

#[test]
fn one_and_six_delay_value_forms_parse() {
    let sb1 = parse_specify("specify\n(a => y) = 1.0;\nendspecify");
    assert_eq!(sb1.paths[0].delays.len(), 1);

    let sb6 = parse_specify("specify\n(a => y) = (1.0, 1.0, 1.0, 1.0, 1.0, 1.0);\nendspecify");
    assert_eq!(sb6.paths[0].delays.len(), 6);
}

#[test]
fn wrong_delay_count_fails_loud() {
    let full = "module top(a, y);\ninout a, y;\nspecify\n(a => y) = (1.0, 1.0, 1.0, 1.0);\nendspecify\nendmodule\n";
    let err = Parser::new().parse(full).expect_err("4 delays is not a valid A.7.4 count");
    let msg = err.to_string();
    assert!(msg.contains("A.7.4"), "error cites the production: {msg}");
}

#[test]
fn multiple_inputs_before_parallel_arrow_fails_loud() {
    let full = "module top(a, b, y);\ninout a, b, y;\nspecify\n(a, b => y) = 1.0;\nendspecify\nendmodule\n";
    let err = Parser::new().parse(full).expect_err("parallel form takes exactly one input");
    let msg = err.to_string();
    assert!(msg.contains("A.7.2"), "error cites the production: {msg}");
}

// ─────────────────────────────── T16 ───────────────────────────────────

fn parse_specify_err(module_src: &str) -> String {
    let full = format!(
        "module top(a, y, clk, d);\n\
         inout a, y, clk, d;\n\
         {module_src}\n\
         endmodule\n"
    );
    Parser::new().parse(&full).expect_err("expected a parse error").to_string()
}

#[test]
fn setup_timing_check_parses_data_reference_and_limit() {
    let sb = parse_specify("specify\n$setup(d, posedge clk, 1.0);\nendspecify");
    assert_eq!(sb.checks.len(), 1);
    let c = &sb.checks[0];
    assert_eq!(c.kind, TimingCheckKind::Setup);
    assert_eq!(c.data_event.as_ref().unwrap().terminal, "d");
    assert!(c.data_event.as_ref().unwrap().edge.is_none());
    assert_eq!(c.reference_event.terminal, "clk");
    assert!(c.reference_event.edge.is_some(), "posedge qualifies the reference event");
    assert_eq!(c.limits.len(), 1);
}

#[test]
fn hold_timing_check_parses_reference_then_data() {
    let sb = parse_specify("specify\n$hold(posedge clk, d, 0.5);\nendspecify");
    assert_eq!(sb.checks.len(), 1);
    let c = &sb.checks[0];
    assert_eq!(c.kind, TimingCheckKind::Hold);
    assert_eq!(c.reference_event.terminal, "clk");
    assert_eq!(c.data_event.as_ref().unwrap().terminal, "d");
}

#[test]
fn setuphold_timing_check_parses_two_limits() {
    let sb = parse_specify("specify\n$setuphold(posedge clk, d, 1.0, 0.5);\nendspecify");
    let c = &sb.checks[0];
    assert_eq!(c.kind, TimingCheckKind::SetupHold);
    assert_eq!(c.limits.len(), 2);
}

#[test]
fn width_and_period_checks_require_a_controlled_reference_event() {
    let sb = parse_specify("specify\n$width(posedge clk, 2.0);\n$period(posedge clk, 4.0);\nendspecify");
    assert_eq!(sb.checks.len(), 2);
    assert_eq!(sb.checks[0].kind, TimingCheckKind::Width);
    assert!(sb.checks[0].data_event.is_none());
    assert_eq!(sb.checks[1].kind, TimingCheckKind::Period);
}

#[test]
fn width_check_missing_mandatory_edge_fails_loud() {
    let msg = parse_specify_err("specify\n$width(clk, 2.0);\nendspecify");
    assert!(msg.contains("A.7.5.3"), "error cites controlled_timing_check_event's production: {msg}");
}

#[test]
fn recovery_removal_and_skew_checks_parse() {
    let sb = parse_specify(
        "specify\n\
         $recovery(posedge clk, d, 1.0);\n\
         $removal(posedge clk, d, 1.0);\n\
         $skew(posedge clk, d, 1.0);\n\
         endspecify",
    );
    assert_eq!(sb.checks.len(), 3);
    assert_eq!(sb.checks[0].kind, TimingCheckKind::Recovery);
    assert_eq!(sb.checks[1].kind, TimingCheckKind::Removal);
    assert_eq!(sb.checks[2].kind, TimingCheckKind::Skew);
}

#[test]
fn optional_notifier_argument_parses() {
    let sb = parse_specify("reg notify_reg;\nspecify\n$setup(d, posedge clk, 1.0, notify_reg);\nendspecify");
    assert_eq!(sb.checks[0].notifier.as_deref(), Some("notify_reg"));
}

#[test]
fn width_check_optional_threshold_before_notifier_parses() {
    let sb = parse_specify("reg notify_reg;\nspecify\n$width(posedge clk, 2.0, 1.0, notify_reg);\nendspecify");
    assert_eq!(sb.checks[0].notifier.as_deref(), Some("notify_reg"));
}

#[test]
fn unknown_dollar_construct_inside_specify_fails_loud_naming_it() {
    let msg = parse_specify_err("specify\n$nochange(posedge clk, d, 0, 0);\nendspecify");
    assert!(msg.contains("nochange"), "error names the unrecognized construct: {msg}");
    assert!(msg.contains("A.7.5.1"), "error cites the production: {msg}");
}

// ─────────────────────────────── T17 ───────────────────────────────────

fn elab_err(module_src: &str) -> String {
    let full = format!(
        "module top(a, y, clk, d);\n\
         inout a, y, clk, d;\n\
         {module_src}\n\
         endmodule\n"
    );
    piperine_lang::parse_and_elaborate(&full, &SourceMap::dummy())
        .expect_err("expected an elaboration error")
        .to_string()
}

#[test]
fn edge_sensitive_path_is_recognized_and_named() {
    let sb = parse_specify("specify\n(posedge a => (y : d)) = 1.0;\nendspecify");
    assert_eq!(sb.paths.len(), 0, "not lowered into an ordinary path");
    assert_eq!(sb.unsupported.len(), 1);
    assert_eq!(sb.unsupported[0].construct, "edge-sensitive path declaration");
    assert_eq!(sb.unsupported[0].clause, "A.7.2");
}

#[test]
fn edge_sensitive_path_fails_loud_at_elaboration() {
    let msg = elab_err("specify\n(posedge a => (y : d)) = 1.0;\nendspecify");
    assert!(msg.contains("edge-sensitive path declaration"), "{msg}");
    assert!(msg.contains("A.7.2"), "{msg}");
}

#[test]
fn state_dependent_path_is_recognized_and_named() {
    let sb = parse_specify("specify\nif (a) (a => y) = 1.0;\nendspecify");
    assert_eq!(sb.unsupported.len(), 1);
    assert_eq!(sb.unsupported[0].construct, "state-dependent path declaration");
    assert_eq!(sb.unsupported[0].clause, "A.7.2");
}

#[test]
fn state_dependent_path_fails_loud_at_elaboration() {
    let msg = elab_err("specify\nif (a) (a => y) = 1.0;\nendspecify");
    assert!(msg.contains("state-dependent path declaration"), "{msg}");
    assert!(msg.contains("A.7.2"), "{msg}");
}

#[test]
fn ifnone_path_is_recognized_and_named() {
    let sb = parse_specify("specify\nifnone (a => y) = 1.0;\nendspecify");
    assert_eq!(sb.unsupported[0].construct, "state-dependent path declaration");
}

#[test]
fn pathpulse_specparam_is_recognized_and_named() {
    let sb = parse_specify("specify\nspecparam PATHPULSE$a$y = (1.0, 2.0);\nendspecify");
    assert_eq!(sb.specparams.len(), 0, "not lowered as an ordinary specparam");
    assert_eq!(sb.unsupported.len(), 1);
    assert_eq!(sb.unsupported[0].construct, "PATHPULSE$ pulse-control specparam");
    assert_eq!(sb.unsupported[0].clause, "A.2.4");
}

#[test]
fn pathpulse_specparam_fails_loud_at_elaboration() {
    let msg = elab_err("specify\nspecparam PATHPULSE$a$y = (1.0, 2.0);\nendspecify");
    assert!(msg.contains("PATHPULSE$"), "{msg}");
    assert!(msg.contains("A.2.4"), "{msg}");
}

#[test]
fn pulsestyle_declaration_is_recognized_and_named() {
    let sb = parse_specify("specify\npulsestyle_onevent y;\nendspecify");
    assert_eq!(sb.unsupported[0].construct, "pulsestyle declaration");
    assert_eq!(sb.unsupported[0].clause, "A.7.1");
}

#[test]
fn pulsestyle_declaration_fails_loud_at_elaboration() {
    let msg = elab_err("specify\npulsestyle_onevent y;\nendspecify");
    assert!(msg.contains("pulsestyle declaration"), "{msg}");
    assert!(msg.contains("A.7.1"), "{msg}");
}

#[test]
fn showcancelled_declaration_is_recognized_and_named() {
    let sb = parse_specify("specify\nshowcancelled y;\nendspecify");
    assert_eq!(sb.unsupported[0].construct, "showcancelled declaration");
    assert_eq!(sb.unsupported[0].clause, "A.7.1");
}

#[test]
fn showcancelled_declaration_fails_loud_at_elaboration() {
    let msg = elab_err("specify\nshowcancelled y;\nendspecify");
    assert!(msg.contains("showcancelled declaration"), "{msg}");
    assert!(msg.contains("A.7.1"), "{msg}");
}

#[test]
fn noshowcancelled_declaration_is_recognized_and_named() {
    let sb = parse_specify("specify\nnoshowcancelled y;\nendspecify");
    assert_eq!(sb.unsupported[0].construct, "showcancelled declaration");
}

#[test]
fn every_recognized_specify_item_is_typed_not_an_unread_span() {
    // Mixed block: every construct present must land on exactly one typed
    // list — nothing is silently dropped and nothing double-counts.
    let sb = parse_specify(
        "specify\n\
         specparam tr = 1.0;\n\
         (a => y) = 1.0;\n\
         $setup(d, posedge clk, 1.0);\n\
         if (a) (a => y) = 1.0;\n\
         showcancelled y;\n\
         endspecify",
    );
    assert_eq!(sb.specparams.len(), 1);
    assert_eq!(sb.paths.len(), 1);
    assert_eq!(sb.checks.len(), 1);
    assert_eq!(sb.unsupported.len(), 2);
}

#[test]
fn a_supported_specify_block_still_elaborates_cleanly() {
    // Guards against T17's check firing on the happy path — a block with
    // no unsupported forms elaborates past the specify arm (T18 will make
    // it observable on the POM; for now this just proves no false-positive
    // rejection).
    let full = "module top(a, y);\ninout electrical a, y;\nspecify\n(a => y) = 1.0;\nendspecify\nendmodule\n";
    piperine_lang::parse_and_elaborate(full, &SourceMap::dummy())
        .expect("a supported specify block elaborates without error");
}

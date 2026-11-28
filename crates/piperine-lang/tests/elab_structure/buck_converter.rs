//! Scope: the buck-converter example (`examples/buck`) elaborated from source (DIG-14, DIG-15).
//! Each module `ir-rust` T29 transcribed by hand prints as its fixture
//! (`crates/piperine-ir/tests/golden/buck/*.txt`, never edited) except the differences named in
//! this file, one constant each; `Buck` elaborates with no diagnostic, an empty `check()` and the
//! structural print pinned in `tests/fixtures/elab/buck.txt`.


use piperine_ir::Ir;
use piperine_lang_fixtures::buck::BuckSource;
use piperine_lang_fixtures::elaboration::SourceElaboration;

/// Associated test helpers of `buck_converter`, grouped on one fixture type.
struct BuckConverterFixture;

impl BuckConverterFixture {
    /// `examples/buck` as a one-crate graph over the embedded standard library.
    fn buck_design() -> piperine_lang::pom::Design {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck");
        let mut graph = piperine_lang::pom::CrateGraph::new();
        graph.add_crate("buck", dir.join("src/lib.msl")).expect("a valid crate name");
        piperine_lang::pom::Design::load_crates(graph)
    }

    fn buck_ir() -> Ir {
        let design = BuckConverterFixture::buck_design();
        assert!(design.diagnostics().is_empty(), "{:?}", SourceElaboration::codes(design.diagnostics()));
        piperine_lang::elab::Elaborator::new(&design).run("Buck").unwrap_or_else(|e| panic!("{:?}", SourceElaboration::codes(&e)))
    }

    fn fixture(name: &str) -> String {
        BuckSource::golden(name)
    }

    /// The lines of the item of a print whose header line is `header` (`body Osc`, `fn name(..`): the
    /// header and the indented lines under it.
    fn item(print: &str, header: &str) -> Vec<String> {
        let mut lines = print.lines().skip_while(|l| !l.starts_with(header));
        let first = lines.next().unwrap_or_else(|| panic!("no `{header}` in:\n{print}"));
        std::iter::once(first).chain(lines.take_while(|l| l.starts_with("  "))).map(str::to_string).collect()
    }

    /// The first difference listed in `tasks.md` T37 (ruling 2, A7): the fixtures guard a `rise` with
    /// a read of `c`; the source gives the stdlib's form over the previous value (`!c.prev() && c`, D9).
    fn stdlib_rise_guard(carrier: &str) -> (String, String) {
        (format!("guard {carrier}"), format!("guard ((!{carrier}.prev()) && {carrier})"))
    }

    /// Replace `from` by `to` in `lines`, which must hold `from` exactly once: a listed difference is
    /// one place, never a blanket rewrite.
    fn replace_once(lines: &[String], (from, to): &(String, String)) -> Vec<String> {
        let hits = lines.iter().filter(|l| l.contains(from.as_str())).count();
        assert_eq!(hits, 1, "the listed difference `{from}` must occur in exactly one line:\n{}", lines.join("\n"));
        lines.iter().map(|l| l.replacen(from.as_str(), to, 1)).collect()
    }

    fn pair(difference: (&str, &str)) -> (String, String) {
        (difference.0.to_string(), difference.1.to_string())
    }

    fn assert_same(case: &str, expected: &[String], actual: &[String]) {
        assert_eq!(expected, actual, "{case}: the elaborated print differs from the fixture\n--- fixture\n{}\n--- elaborated\n{}", expected.join("\n"), actual.join("\n"));
    }

    /// Remove each listed line from `lines`, asserting it is present exactly once.
    fn without(lines: &[String], extras: &[Extra]) -> Vec<String> {
        let mut out = lines.to_vec();
        for Extra(line) in extras {
            let hits = out.iter().filter(|l| l.as_str() == *line).count();
            assert_eq!(hits, 1, "the listed item `{line}` must be present exactly once:\n{}", lines.join("\n"));
            out.retain(|l| l != line);
        }
        out
    }
}

/// The second: `Pid` folds `DUTY_MAX as i32` where the fixture writes the cast (DIG-08 AC 2).
const PID_FOLD: (&str, &str) = ("(2880u12 as i32)", "2880i32");

/// The one difference of `GateDriver`: its delayed definitions give no value to its `out` ports, so each writes its
/// initial value (explicit-unknowns T30, §3.3.4) and the fixture, the earlier transcription, has none.
const GATE_DRIVER_INITIAL_VALUES: [(&str, &str); 2] = [("carrier hs_gate: bool out", "carrier hs_gate: bool out init false"), ("carrier ls_gate: bool out", "carrier ls_gate: bool out init false")];

#[test]
fn six_slice_bodies_equal_their_hand_built_fixtures() {
    // DIG-14 AC 1, C128. Osc prints exactly and GateDriver by its two initial values; the others differ by the rise guard
    // (SetpointReg `wr`, RefRamp `tick`, Adc `sample`, Pid `tick`) and Pid also by the fold.
    let ir = BuckConverterFixture::buck_ir();
    let print = ir.print_structural();
    let pid = vec![BuckConverterFixture::stdlib_rise_guard("tick"), BuckConverterFixture::pair(PID_FOLD)];
    let cases: [(&str, &str, Vec<(String, String)>); 6] = [
        ("osc", "Osc", vec![]),
        ("gate_driver", "GateDriver", GATE_DRIVER_INITIAL_VALUES.map(BuckConverterFixture::pair).to_vec()),
        ("setpoint_reg", "SetpointReg", vec![BuckConverterFixture::stdlib_rise_guard("wr")]),
        ("ref_ramp", "RefRamp", vec![BuckConverterFixture::stdlib_rise_guard("tick")]),
        ("adc", "Adc", vec![BuckConverterFixture::stdlib_rise_guard("sample")]),
        ("pid", "Pid", pid),
    ];
    for (file, body, differences) in cases {
        let header = format!("body {body}");
        let mut expected = BuckConverterFixture::item(&BuckConverterFixture::fixture(file), &header);
        for difference in &differences {
            expected = BuckConverterFixture::replace_once(&expected, difference);
        }
        BuckConverterFixture::assert_same(body, &expected, &BuckConverterFixture::item(&print, &header));
    }
}

/// An item the source has and the fixture has not, a line of the body print.
struct Extra(&'static str);

#[test]
fn dpwm_and_a2d_differ_only_as_listed() {
    // DIG-14 AC 2, C129.
    let ir = BuckConverterFixture::buck_ir();
    let print = ir.print_structural();

    // Dpwm adds the `Never` monitor the source places (child, its carrier, the drive of that
    // carrier); its reaction equals the fixture's, the rise guard of `clk` apart.
    let dpwm_extras = [
        Extra("  carrier no_shoot_through.bad: bool signal"),
        Extra("  child no_shoot_through: Never  nodes ()  params ()  carriers (clk, no_shoot_through.bad)"),
        Extra("  drive no_shoot_through.bad = (hs & ls)"),
    ];
    let elaborated = BuckConverterFixture::item(&print, "body Dpwm");
    let expected = BuckConverterFixture::replace_once(&BuckConverterFixture::item(&BuckConverterFixture::fixture("dpwm"), "body Dpwm"), &BuckConverterFixture::stdlib_rise_guard("clk"));
    BuckConverterFixture::assert_same("Dpwm", &expected, &BuckConverterFixture::without(&elaborated, &dpwm_extras));
    // Its two functions are the fixture's.
    BuckConverterFixture::assert_same("dithered_width", &BuckConverterFixture::item(&BuckConverterFixture::fixture("dpwm"), "fn dithered_width"), &BuckConverterFixture::item(&print, "fn dithered_width"));
    BuckConverterFixture::assert_same("rev3", &BuckConverterFixture::item(&BuckConverterFixture::fixture("dpwm"), "fn rev3"), &BuckConverterFixture::item(&print, "fn rev3"));

    // A2D (the fixture `comparator`) adds the `init` of each `above`: two signals and an event for
    // each, numbered after the cross, so the second cross is E2 where the fixture has E1. Each
    // `above` is one reaction on both its events (FIX-13: `cross(e) || (init() && e > 0.0)` is a
    // head split per disjunct), its body under the `fired` guard, where the fixture has one
    // reaction per cross and no `init`.
    let a2d_extras = [
        Extra("  carrier above$0.init$0.up: bool signal init true"),
        Extra("  carrier above$1.init$0.up: bool signal init true"),
        Extra("  event E1 changed above$0.init$0.up"),
        Extra("  event E3 changed above$1.init$0.up"),
        Extra("  reaction R0 on (E0, E1)"),
        Extra("    if (fired(E0) || (fired(E1) && (((probe(a, gnd) - vth) - (hyst / 2.0)) > 0.0))) { d <- true } else { () }"),
        Extra("  reaction R1 on (E2, E3)"),
        Extra("    if (fired(E2) || (fired(E3) && (((vth - (hyst / 2.0)) - probe(a, gnd)) > 0.0))) { d <- false } else { () }"),
    ];
    let fixture_reactions = [Extra("  reaction R0 on (E0)"), Extra("    d <- true"), Extra("  reaction R1 on (E1)"), Extra("    d <- false")];
    let mut survivors = BuckConverterFixture::without(&BuckConverterFixture::item(&print, "body A2D"), &a2d_extras);
    survivors = survivors.iter().map(|l| if l.starts_with("  event E2 cross") { l.replacen("  event E2 cross", "  event E1 cross", 1) } else { l.clone() }).collect();
    BuckConverterFixture::assert_same("A2D", &BuckConverterFixture::without(&BuckConverterFixture::item(&BuckConverterFixture::fixture("comparator"), "body A2D"), &fixture_reactions), &survivors);
}

#[test]
fn the_buck_elaborates_with_no_diagnostic_and_a_sound_ir() {
    // DIG-15 AC 1, C143: 22 bodies, the 20 modules of the package and the stdlib's `A2D` and
    // `Never`; `transition`, `above` and `init` are hoisted and have none.
    let design = BuckConverterFixture::buck_design();
    assert!(design.diagnostics().is_empty(), "{:?}", SourceElaboration::codes(design.diagnostics()));
    let ir = piperine_lang::elab::Elaborator::new(&design).run("Buck").unwrap_or_else(|e| panic!("{:?}", SourceElaboration::codes(&e)));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let mut names: Vec<&str> = ir.bodies().map(|b| b.name()).collect();
    names.sort_unstable();
    assert_eq!(names.len(), 22, "{names:?}");
    for hoisted in ["transition", "above", "init"] {
        assert!(!names.contains(&hoisted), "{hoisted} is hoisted and has no body");
    }
    for stdlib in ["A2D", "Never"] {
        assert!(names.contains(&stdlib), "{stdlib}");
    }
}

#[test]
fn the_buck_ir_is_pinned() {
    // DIG-15 AC 2, C144.
    let path = format!("{}/tests/fixtures/elab/buck.txt", env!("CARGO_MANIFEST_DIR"));
    let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(BuckConverterFixture::buck_ir().print_structural(), golden, "the elaborated Buck differs from tests/fixtures/elab/buck.txt");
}

//! Scope: the buck-converter example (`examples/buck`) elaborated from source (DIG-14).
//! Each module `ir-rust` T29 transcribed by hand prints as its fixture
//! (`crates/piperine-ir/tests/golden/buck/*.txt`, never edited) except the differences named in
//! this file, one constant each.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::Ir;
use support::Elab;

/// `examples/buck` as a one-crate graph over the embedded standard library.
fn buck_design() -> piperine_lang::pom::Design {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck");
    let mut graph = piperine_lang::pom::CrateGraph::new();
    graph.add_crate("buck", dir.join("src/lib.msl")).expect("a valid crate name");
    piperine_lang::pom::Design::load_crates(graph)
}

fn buck_ir() -> Ir {
    let design = buck_design();
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    piperine_lang::elab::Elaborator::new(&design).run("Buck").unwrap_or_else(|e| panic!("{:?}", Elab::codes(&e)))
}

fn fixture(name: &str) -> String {
    let path = format!("{}/../piperine-ir/tests/golden/buck/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The lines of the item of a print whose header line is `header` (`body Osc`, `fn name(..`): the
/// header and the indented lines under it.
fn item(print: &str, header: &str) -> Vec<String> {
    let mut lines = print.lines().skip_while(|l| !l.starts_with(header));
    let first = lines.next().unwrap_or_else(|| panic!("no `{header}` in:\n{print}"));
    std::iter::once(first).chain(lines.take_while(|l| l.starts_with("  "))).map(str::to_string).collect()
}

/// The first difference listed in `tasks.md` T37 (ruling 2, A7): the fixtures guard a `rise` with
/// `c.high()`; the source gives the stdlib's two-condition form over the previous value.
fn stdlib_rise_guard(carrier: &str) -> (String, String) {
    (
        format!("guard {carrier}.high()"),
        format!("guard ((!{carrier}.prev().high()) & ((!{carrier}.is_known()) | {carrier}.known_or(false)))"),
    )
}

/// The second: `Pid` folds `DUTY_MAX as i32` where the fixture writes the cast (DIG-08 AC 2).
const PID_FOLD: (&str, &str) = ("(2880u12 as i32)", "2880i32");

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

#[test]
fn six_slice_bodies_equal_their_hand_built_fixtures() {
    // DIG-14 AC 1, C128. Osc and GateDriver print exactly; the others differ by the rise guard
    // (SetpointReg `wr`, RefRamp `tick`, Adc `sample`, Pid `tick`) and Pid also by the fold.
    let ir = buck_ir();
    let print = ir.print_structural();
    let cases: [(&str, &str, Vec<(String, String)>); 6] = [
        ("osc", "Osc", vec![]),
        ("gate_driver", "GateDriver", vec![]),
        ("setpoint_reg", "SetpointReg", vec![stdlib_rise_guard("wr")]),
        ("ref_ramp", "RefRamp", vec![stdlib_rise_guard("tick")]),
        ("adc", "Adc", vec![stdlib_rise_guard("sample")]),
        ("pid", "Pid", vec![stdlib_rise_guard("tick"), pair(PID_FOLD)]),
    ];
    for (file, body, differences) in cases {
        let header = format!("body {body}");
        let mut expected = item(&fixture(file), &header);
        for difference in &differences {
            expected = replace_once(&expected, difference);
        }
        assert_same(body, &expected, &item(&print, &header));
    }
}

/// An item the source has and the fixture has not, a line of the body print.
struct Extra(&'static str);

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

#[test]
fn dpwm_and_a2d_differ_only_as_listed() {
    // DIG-14 AC 2, C129.
    let ir = buck_ir();
    let print = ir.print_structural();

    // Dpwm adds the `Never` monitor the source places (child, its carrier, the drive of that
    // carrier); its reaction equals the fixture's, the rise guard of `clk` apart.
    let dpwm_extras = [
        Extra("  carrier no_shoot_through.bad: bool signal"),
        Extra("  child no_shoot_through: Never  nodes ()  params ()  carriers (clk, no_shoot_through.bad)"),
        Extra("  drive no_shoot_through.bad = (hs & ls)"),
    ];
    let elaborated = item(&print, "body Dpwm");
    let expected = replace_once(&item(&fixture("dpwm"), "body Dpwm"), &stdlib_rise_guard("clk"));
    assert_same("Dpwm", &expected, &without(&elaborated, &dpwm_extras));
    // Its two functions are the fixture's, exactly.
    for header in ["fn \"dithered_width~01\"", "fn \"rev3~1\""] {
        assert_same(header, &item(&fixture("dpwm"), header), &item(&print, header));
    }

    // A2D (the fixture `comparator`) adds the `init` of each `above`: two signals and an event
    // and reaction for each. Its events and reactions are numbered after those, so the two
    // survivors are renamed: E2 is the fixture's E1, R2 its R1.
    let a2d_extras = [
        Extra("  carrier above$0.init$0.up: bool signal init true"),
        Extra("  carrier above$1.init$0.up: bool signal init true"),
        Extra("  event E1 changed above$0.init$0.up"),
        Extra("  event E3 changed above$1.init$0.up"),
        Extra("  reaction R1 on (E1)"),
        Extra("    if (((probe(a, gnd) - vth) - (hyst / 2.0)) > 0.0) { d <- true } else { () }"),
        Extra("  reaction R3 on (E3)"),
        Extra("    if (((vth - (hyst / 2.0)) - probe(a, gnd)) > 0.0) { d <- false } else { () }"),
    ];
    let renames = [("  event E2 cross", "  event E1 cross"), ("  reaction R2 on (E2)", "  reaction R1 on (E1)")];
    let mut survivors = without(&item(&print, "body A2D"), &a2d_extras);
    for (from, to) in renames {
        assert_eq!(survivors.iter().filter(|l| l.starts_with(from)).count(), 1, "{from}");
        survivors = survivors.iter().map(|l| if l.starts_with(from) { l.replacen(from, to, 1) } else { l.clone() }).collect();
    }
    assert_same("A2D", &item(&fixture("comparator"), "body A2D"), &survivors);
}

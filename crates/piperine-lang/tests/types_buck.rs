//! The subset after `msl-digital`, and the buck (DIG-07): every form of DIG-07 AC 1 that is still
//! outside is `P0001` naming the construct, and `Buck` of `examples/buck`, loaded from disk as
//! `buck_example.rs` loads it, type-checks with no diagnostic.

use std::path::PathBuf;

use piperine_lang::pom::{CrateGraph, Design, DesignDiagnostic};
use piperine_lang::types::Types;

/// One construct of DIG-07 AC 1: its name, a source that reaches it from `Top` and the words the
/// diagnostic must contain. The sources are over the embedded standard library and keep the
/// construct where it matters: most are written in a reaction body.
struct Row {
    construct: &'static str,
    source: &'static str,
    names: &'static str,
}

const NODE: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

const ROWS: &[Row] = &[
    // `an element commit` and `a field commit` left this table when composite-values T76 typed them
    // (`types_carrier_arrays::element_and_field_targets_type_their_levels`).
    // `an array type` (`signal s: [Bit; 4]`) left this table when composite-values T74 typed it
    // (`types_carrier_arrays::arrays_of_carriers_type_as_signals_and_ports`).
    // `a user trait and impl` left this table when generics T25 typed methods (`types_traits::methods_associated_functions_and_constants_type_check`).
    // `a generic module` left this table when generics T71 typed it (`types_generic_modules`).
    // `a generation for`, `a generation if` and `a generation in a digital block` left this table when generics T77 and T80 typed
    // and lowered generation (`types_generation`).
    Row { construct: "a generic function", source: "fn g<T>(x: T) -> T { x }\nmodule Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { let h = g; } }", names: "generic function" },
    Row { construct: "a nature with resolve", source: "module Top(c: Wire) { }", names: "resolved nature `Wire`" },
    Row { construct: "a bundle", source: "struct Bn { out a: Bit }\nmodule Top(p: Bn) { }", names: "`struct` item used as a type" },
    // `others()` on a one-driver carrier left this table when resolved-nets T3 typed it (`types_natures::others_on_a_one_driver_carrier_is_e0224`).
    // `settle` left this table when unknown-semantics-events T5 typed it (`types_events::settle_is_an_event_in_every_event_position`).
    // `delay`, `noise`, `ddx` and `simparam` left this table when analog-operators T22 and T23 typed them (`types_analog_operators`).
    // `bound_step`, `freq()` and `analysis(Dc | Transient | Ac | Noise)` left this table when
    // FIX-09 to FIX-11 typed them (`types_analog::freq_analysis_kinds_and_bound_step_type`).
    // a template left this table when generics T84 typed it as a value (`types_templates::a_template_is_a_value_of_its_modules_type`).
    Row { construct: "an extern module", source: "#[link(\"x.osdi\", model = \"x\")]\nextern \"osdi\" module X(a: Electrical) { }\nmodule Top(a: Electrical) { let x = X(a); }", names: "`extern` module" },
];

fn check(row: &Row) -> (String, Vec<DesignDiagnostic>) {
    let src = format!("{NODE}{}", row.source);
    let design = Design::builder().source("top.msl", &src).build();
    assert!(design.diagnostics().is_empty(), "{}: the source must parse and resolve: {:?}\n{src}", row.construct, design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    match Types::check(&design, "Top") {
        Ok(_) => panic!("{}: expected a P0001, the check passed\n{src}", row.construct),
        Err(d) => (src, d),
    }
}

#[test]
fn what_stays_outside_is_p0001_by_name() {
    // DIG-07 AC 1: each construct is one `P0001` that names it, at the construct.
    for row in ROWS {
        let (src, d) = check(row);
        assert_eq!(d.len(), 1, "{}: one diagnostic per construct, got {:?}\n{src}", row.construct, d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        assert_eq!(d[0].diagnostic.code, "P0001", "{}: {}", row.construct, d[0].diagnostic.message);
        assert!(d[0].diagnostic.message.contains(row.names), "{}: `{}` does not name `{}`", row.construct, d[0].diagnostic.message, row.names);
        let span = d[0].diagnostic.span();
        assert!(span.end > span.start && span.end <= src.len(), "{}: a span of the construct", row.construct);
    }
}

/// `examples/buck` as a one-crate graph named by its manifest, over the embedded standard library.
fn buck() -> Design {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck");
    let manifest = std::fs::read_to_string(dir.join("Piperine.toml")).expect("examples/buck/Piperine.toml");
    let name = manifest
        .lines()
        .map(str::trim)
        .skip_while(|l| *l != "[package]")
        .find_map(|l| l.strip_prefix("name").and_then(|r| r.trim_start().strip_prefix('=')).map(|v| v.trim().trim_matches('"').to_string()))
        .expect("a [package] name in Piperine.toml");
    let mut graph = CrateGraph::new();
    graph.add_crate(&name, dir.join("src/lib.msl")).expect("a valid crate name");
    Design::load_crates(graph)
}

#[test]
fn the_buck_type_checks_with_no_diagnostic() {
    // DIG-07 AC 3: `Buck` of the flagship package, every module, function and constant it reaches,
    // the stdlib items among them, checks with no diagnostic.
    // The inductor reads the flow of a branch it drives with a potential (`v(p, n) +> ddt(l * i(p, n))`),
    // which is a value; the flow of a branch the module does not drive so is outside the subset.
    let rl = format!("{NODE}module Top(p: E, n: E) {{ l: f64 = 1u; }}\nanalog for Top {{ v(p, n) +> ddt(l * i(p, n)) + 2.0 * i(p, n); }}");
    let design = Design::builder().source("top.msl", &rl).build();
    assert!(Types::check(&design, "Top").is_ok(), "the flow of a potential branch is a value");
    let other = format!("{NODE}module Top(p: E, n: E) {{ }}\nanalog for Top {{ v(p, n) +> 2.0 * i(p, gnd); }}");
    let design = Design::builder().source("top.msl", &other).build();
    let d = Types::check(&design, "Top").expect_err("another branch");
    assert_eq!((d[0].diagnostic.code.as_ref(), d[0].diagnostic.message.contains("flow probe")), ("P0001", true));
    let design = buck();
    assert!(design.diagnostics().is_empty(), "the package must parse and resolve: {:?}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    match Types::check(&design, "Buck") {
        Ok(types) => {
            let names: Vec<String> = types.reached().iter().filter_map(|d| design.decl_data(*d).name.as_ref().map(|n| n.text.clone())).collect();
            for module in ["Buck", "Controller", "PowerStage", "Sensing", "Supervisor", "Dpwm", "Pid", "RefRamp", "SetpointReg", "Adc", "PowerSwitch", "GateDriver", "Load", "A2D", "transition", "above", "init", "Never", "log", "require"] {
                assert!(names.iter().any(|n| n == module), "`{module}` is reached from Buck: {names:?}");
            }
        }
        Err(d) => {
            let files = design.files();
            let lines: Vec<String> = d
                .iter()
                .map(|d| {
                    let file = &files[d.file.0 as usize];
                    let span = d.diagnostic.span();
                    let line = file.text()[..span.start.min(file.text().len())].lines().count().max(1);
                    format!("{}:{line}: {} {} `{}`", file.name(), d.diagnostic.code, d.diagnostic.message, file.text().get(span.range()).unwrap_or("?"))
                })
                .collect();
            panic!("Buck does not type-check:\n{}", lines.join("\n"));
        }
    }
}

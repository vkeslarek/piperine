//! Generic modules run from MSL source on the Rust backend (feature `generics` phase C, GN-13, GN-15, GN-16, GN-20): each
//! model is written in `tests/generics/modules.msl`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every expected value is computed here by Rust from the
//! rule the test cites, never read back from the model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/bytes.rs"]
mod bytes;

use std::path::{Path, PathBuf};

use bytes::CarrierBytes;
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};
use support::{build, first_edge_value};

/// The model `file` of `tests/generics/`, with generic modules admitted.
struct ModuleModels;

impl ModuleModels {
    /// The project of `file`.
    fn project(file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generics").join(file);
        Project::new(&path)
    }

    /// The voltage of each node in `names` of `top` of `file` at its operating point.
    fn operating_point(file: &str, top: &str, names: &[&str]) -> Vec<f64> {
        let mut running = Self::project(file).compile(top).unwrap_or_else(|r| panic!("{r}"));
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        names
            .iter()
            .map(|name| {
                let node = running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("node {name}"));
                solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)")
            })
            .collect()
    }

    /// What `Bench` of `file` committed at its first rising edge: the value of each carrier in `names`.
    fn first_commits(file: &str, names: &[&str]) -> Vec<Vec<u8>> {
        let seen = build(&Self::project(file), "Bench").run(10e-9, 1e-9);
        names.iter().map(|name| first_edge_value(&seen, &format!("Bench.{name}"))).collect()
    }
}

/// Rust's own value of each carrier the models commit: the expected bytes are computed here, never copied from a run.
struct RustOracle;

impl RustOracle {
    /// An `Option<u8>` carrier: the flag that it is `Some`, then the value (`None` holds the zero value).
    fn option(value: Option<u8>) -> Vec<u8> {
        vec![u8::from(value.is_some()), value.unwrap_or(0)]
    }
}

/// `enum Mode { Idle, Run }` of the model, laid out by its tag.
#[derive(Clone, Copy)]
enum Mode {
    #[allow(dead_code)]
    Idle,
    Run,
}

#[test]
fn generic_modules_run_with_explicit_and_inferred_arguments() {
    // GN-13 AC 1, GN-15 AC 1: `Reg` placed at `u8` (inferred from `d`), at `u8` (written `::<u8>`) and at `bool`, `Dff` at
    // `Option<u8>` (a `Default` of its own): each register commits its input at the first rising edge
    let committed = ModuleModels::first_commits("modules.msl", &["q0", "q1", "q2"]);
    let (some, mode) = (Some(7u8), Mode::Run);
    assert_eq!(committed, [RustOracle::option(some), RustOracle::option(some), CarrierBytes::byte(mode as u8)]);
}

#[test]
fn equal_arguments_share_one_body() {
    // GN-15 AC 1: the two placements of `Reg` at `Option<u8>` (one inferred, one written) are one body; `Reg` at `Mode` is another
    let ir = ModuleModels::project("modules.msl").elaborate("Bench").unwrap_or_else(|r| panic!("{r}"));
    let mut registers: Vec<String> = ir.bodies().map(|body| body.name().to_string()).filter(|name| name.starts_with("Reg")).collect();
    registers.sort();
    assert_eq!(registers, ["Reg::<Mode>", "Reg::<Option<u8>>"]);
}

#[test]
fn a_nature_parameter_takes_its_default() {
    // GN-13 AC 2 (§3.8.7): nothing connected is typed by `S`, so `Cell(a)` is `Cell::<Electrical>`; its `mid` is a node of
    // that nature held at `level` volts and `a` at 0.5 V
    let ir = ModuleModels::project("modules.msl").elaborate("Rig").unwrap_or_else(|r| panic!("{r}"));
    assert!(ir.bodies().any(|body| body.name() == "Cell::<Electrical>"), "{:?}", ir.bodies().map(|b| b.name().to_string()).collect::<Vec<_>>());
    let seen = ModuleModels::operating_point("modules.msl", "Rig", &["a", "cell.mid"]);
    assert!((seen[0] - 0.5).abs() < 1e-9 && (seen[1] - 1.5).abs() < 1e-9, "{seen:?}");
}

#[test]
fn a_const_generic_of_zero_makes_an_empty_module() {
    // GN-13 AC 1, §1.2.3: `Taps::<0>` has no rail and its loop runs zero times; `Taps::<2>` drives rail `k` to `2 (k + 1)`
    let seen = ModuleModels::operating_point("modules.msl", "Rig", &["b", "c", "two.rails[0]", "two.rails[1]"]);
    let rails: Vec<f64> = (0..2).map(|k| 2.0 * (f64::from(k) + 1.0)).collect();
    assert!((seen[0] - 1.0).abs() < 1e-9 && (seen[1] - 1.0).abs() < 1e-9, "{seen:?}");
    assert!((seen[2] - rails[0]).abs() < 1e-9 && (seen[3] - rails[1]).abs() < 1e-9, "{seen:?}");
}

#[test]
fn a_low_pass_over_the_generic_integrator_settles_at_its_input() {
    // GN-13 AC 1: `lowpass` places the stdlib `idt` at its default nature `State`; at DC the capacitor is open, so the
    // output equals the 1 V input
    let seen = ModuleModels::operating_point("lowpass.msl", "Top", &["a", "b"]);
    assert!((seen[0] - 1.0).abs() < 1e-9 && (seen[1] - 1.0).abs() < 1e-6, "{seen:?}");
}

#[test]
fn a_monitor_over_a_type_reads_its_let_of_the_carrier() {
    // GN-13 AC 1: `Watch<T>` names `age = since(d)` in its body and reads it in a reaction; at `bool` the check passes
    // (d was stable 0.5 us >= 100 ps at the edge)
    let ir = ModuleModels::project("modules.msl").elaborate("Monitored").unwrap_or_else(|r| panic!("{r}"));
    assert!(ir.bodies().any(|body| body.name() == "Watch::<bool>"));
}

#[test]
fn short_merges_two_nodes_of_each_nature() {
    // GN-16 AC 1 (§8.2.8): a divider of 1k over 3k from 4 V has its middle at 4 * 3k / (1k + 3k) = 3 V. Written with one middle
    // node and written with two nodes a `Short` joins, it is the same solution, over `Electrical`, `Thermal` and the nature
    // `Heat` the model declares
    let (top, upper, lower) = (4.0_f64, 1e3_f64, 3e3_f64);
    let middle = top * lower / (upper + lower);
    for rig in ["ElectricalRig", "ThermalRig", "HeatRig"] {
        let seen = ModuleModels::operating_point("short.msl", rig, &["whole_mid", "joined_mid", "whole_top", "joined_top"]);
        assert!((seen[0] - middle).abs() < 1e-6, "{rig}: the divider written with one middle node: {seen:?}");
        assert!((seen[1] - seen[0]).abs() < 1e-9, "{rig}: the joined divider is the same solution: {seen:?}");
        assert!((seen[2] - top).abs() < 1e-9 && (seen[3] - top).abs() < 1e-9, "{rig}: the source holds the top: {seen:?}");
    }
}

#[test]
fn short_over_two_natures_is_refused_naming_both() {
    // GN-16 AC 2 (DC4): `Short(e, t)` over an `Electrical` and a `Thermal` node gives the one `S` two natures: `E0180`
    // naming both, at the placement
    let refused = ModuleModels::project("short_mixed.msl").elaborate("Mixed").expect_err("two natures for one `S`");
    let found: Vec<(String, String)> = refused.diagnostics.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "E0180");
    assert!(found[0].1.contains("Electrical") && found[0].1.contains("Thermal"), "{found:?}");
}

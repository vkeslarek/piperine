//! Scope: the buck-converter example package (`examples/buck/`), Piperine's flagship
//! mixed-signal prototype. Nothing executes MSL yet, so the bar is the front end's: the package
//! is a well-formed package (manifest plus `src/lib.msl`), every file of it lexes, parses and
//! resolves through the POM against the embedded `std` with zero diagnostics, the built design
//! passes its invariant check, no member access stays unresolved, and the hierarchy reads back
//! as written: `Buck` places the supply, power stage, load, measurement and controller, and the
//! package leans on the `std` items it is meant to showcase instead of re-implementing them.

use std::path::{Path, PathBuf};

use piperine_lang::pom::{CrateGraph, Design, ExprKind, Reference, Reflect, Resolution};

/// Associated test helpers of `buck_example`, grouped on one fixture type.
struct BuckExampleFixture;

impl BuckExampleFixture {
    fn package_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck")
    }

    /// The `name` of the `[package]` table of `Piperine.toml`, read line by line: `piperine-lang` takes
    /// the crate graph as data and does not parse manifests (that is `piperine-project`'s job).
    fn package_name(manifest: &str) -> Option<String> {
        let mut in_package = false;
        for line in manifest.lines().map(str::trim) {
            if line.starts_with('[') {
                in_package = line == "[package]";
            } else if in_package && let Some(rest) = line.strip_prefix("name") {
                let value = rest.trim_start().strip_prefix('=')?.trim();
                return Some(value.trim_matches('"').to_string());
            }
        }
        None
    }

    /// Every `.msl` file under `dir`, recursively.
    fn msl_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("readable source directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                BuckExampleFixture::msl_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "msl") {
                out.push(path);
            }
        }
    }

    /// The package built from disk as a one-crate graph named by its manifest, over the embedded std.
    fn buck() -> Design {
        let dir = BuckExampleFixture::package_dir();
        let manifest = std::fs::read_to_string(dir.join("Piperine.toml")).expect("examples/buck/Piperine.toml");
        let name = BuckExampleFixture::package_name(&manifest).expect("a [package] name in Piperine.toml");
        let mut graph = CrateGraph::new();
        graph.add_crate(&name, dir.join("src/lib.msl")).expect("a valid crate name");
        Design::load_crates(graph)
    }
}

#[test]
fn the_package_is_a_package() {
    let dir = BuckExampleFixture::package_dir();
    let manifest = std::fs::read_to_string(dir.join("Piperine.toml")).unwrap();
    assert_eq!(BuckExampleFixture::package_name(&manifest).as_deref(), Some("buck"));
    assert!(dir.join("src/lib.msl").is_file(), "the crate root is src/lib.msl (E0175)");
    assert!(dir.join("SPEC.md").is_file(), "the specification the model cites");
}

#[test]
fn the_package_passes_check() {
    let problems = BuckExampleFixture::buck().check();
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn every_source_file_is_loaded() {
    let design = BuckExampleFixture::buck();
    let mut on_disk = Vec::new();
    BuckExampleFixture::msl_files(&BuckExampleFixture::package_dir().join("src"), &mut on_disk);
    let root = design.root_crate();
    assert_eq!(root.name(), "buck");
    assert_eq!(root.files().len(), on_disk.len(), "a file under src/ that no `mod` names: {on_disk:?}");
}

#[test]
fn no_member_stays_unresolved() {
    let design = BuckExampleFixture::buck();
    let mut unresolved = Vec::new();
    for decl in design.root_crate().decls() {
        for expr in decl.exprs() {
            if matches!(expr.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }) && expr.resolution() == Resolution::Unresolved {
                unresolved.push(expr.text().to_string());
            }
        }
    }
    // `data.min(..)` is a method of the value the carrier `data` reads as (§5.1.1); the POM leaves it `Unresolved`
    // (STD-06 AC 4) and the type checker finds it
    let methods_of_a_carried_value = ["data.min(SETPOINT_MAX)"];
    unresolved.retain(|text| !methods_of_a_carried_value.contains(&text.as_str()));
    assert!(unresolved.is_empty(), "{unresolved:?}");
}

#[test]
fn the_hierarchy_reads_back_as_written() {
    let design = BuckExampleFixture::buck();
    let placed = |module: &str| -> Vec<(String, String)> {
        let m = design.module(module).unwrap_or_else(|| panic!("module {module}"));
        m.instances()
            .into_iter()
            .filter_map(|i| Some((i.name()?.to_string(), i.module()?.name()?.to_string())))
            .collect()
    };
    let pairs = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
    assert_eq!(
        placed("Buck"),
        pairs(&[("osc", "Osc"), ("supply", "Supply"), ("power", "PowerStage"), ("load", "Load"), ("meas", "Sensing"), ("ctl", "Controller")])
    );
    assert_eq!(
        placed("Controller"),
        pairs(&[("reg", "SetpointReg"), ("ramp", "RefRamp"), ("pid", "Pid"), ("sup", "Supervisor"), ("pwm", "Dpwm")])
    );
    assert_eq!(
        placed("PowerStage"),
        pairs(&[("drv", "GateDriver"), ("hs", "PowerSwitch"), ("ls", "PowerSwitch"), ("ind", "Inductor"), ("shunt", "CurrentSense"), ("c_out", "Capacitor"), ("c_in", "Capacitor")])
    );
    let buck = design.module("Buck").unwrap();
    let ports: Vec<_> = buck.ports().iter().filter_map(|p| p.name()).map(str::to_string).collect();
    assert_eq!(ports, ["en", "sp_wr", "sp_data", "load_step", "inject", "pgood", "fault", "mode"]);
    // Every behavioral leaf has its blocks in its own file: the analog stage is analog, the
    // controller digital, the ADC the one sampled crossing.
    for (module, analog, digital) in
        [("PowerSwitch", 1, 0), ("Inductor", 1, 0), ("Adc", 0, 1), ("Dpwm", 0, 1), ("Pid", 0, 1), ("Supervisor", 0, 1), ("Controller", 0, 0)]
    {
        let m = design.module(module).unwrap();
        assert_eq!((m.analog_blocks().len(), m.digital_blocks().len()), (analog, digital), "{module}");
    }
}

#[test]
fn the_package_uses_the_std_items_it_showcases() {
    let design = BuckExampleFixture::buck();
    let mut missing = Vec::new();
    for name in ["carriers::Electrical", "carriers::Bit", "convert::A2D", "monitor::Never", "analog::transition", "io::log", "values::impl Bit.rise"] {
        let referenced = design.std_crate().find(name).is_some_and(|item| {
            item.references().iter().any(|r| match r {
                Reference::Path(p) => p.enclosing().krate().is_root(),
                Reference::Member(e) => e.enclosing().krate().is_root(),
                Reference::Pattern(p) => p.enclosing().krate().is_root(),
            })
        });
        if !referenced {
            missing.push(name);
        }
    }
    assert!(missing.is_empty(), "std items the package does not reference: {missing:?}");
}

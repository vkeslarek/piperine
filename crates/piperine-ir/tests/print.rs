//! Scope: the one-way printer of `piperine-ir` (IR-04). A golden text per device body and one for
//! the divider, compared byte for byte; the text is deterministic; unequal IRs print different
//! texts; no function builds an `Ir` from text. `PIPERINE_UPDATE_GOLDEN=1` rewrites the goldens.

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::Devices;
use piperine_ir::{BodyId, InstanceId, Ir, IrBuilder};

/// One golden case: a name, the IR it prints.
struct Case {
    name: &'static str,
    ir: Ir,
}

struct Cases;

impl Cases {
    /// One device body, placed alone as the design.
    fn device(
        name: &'static str,
        make: fn(&mut IrBuilder) -> BodyId,
        line: u32,
        params: Vec<f64>,
    ) -> Case {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let top: InstanceId = Devices::alone(&mut b, body, name, line, params);
        Case { name, ir: b.finish(top) }
    }

    fn all() -> Vec<Case> {
        let mut b = IrBuilder::new();
        let top = Devices::divider(&mut b);
        vec![
            Cases::device("resistor", Devices::resistor, 5, vec![1000.0]),
            Cases::device("capacitor", Devices::capacitor, 11, vec![1e-12]),
            Cases::device("vsource", Devices::vsource, 17, vec![10.0, 0.0]),
            Cases::device("isource", Devices::isource, 24, vec![1e-3, 0.0]),
            Cases::device("diode", Devices::diode, 31, vec![1e-14, 1.0, 0.0]),
            Case { name: "divider", ir: b.finish(top) },
        ]
    }

    fn golden_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.txt"))
    }
}

#[test]
fn printed_text_shows_every_item() {
    for case in Cases::all() {
        let text = case.ir.to_string();
        let path = Cases::golden_path(case.name);
        if std::env::var_os("PIPERINE_UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &text).expect("write a golden");
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(text, golden, "golden {} differs", case.name);
        // Every body, function, instance and node of the IR appears in the text.
        for body in case.ir.bodies() {
            assert!(text.contains(&format!("body {}  @", body.name())), "{}: body {}", case.name, body.name());
        }
        for function in case.ir.functions() {
            assert!(text.contains(&format!("fn {}(", function.name())), "{}: fn {}", case.name, function.name());
        }
        for instance in case.ir.instances() {
            assert!(text.contains(&format!("instance {}: ", instance.path())), "{}: {}", case.name, instance.path());
        }
        for node in case.ir.nodes() {
            assert!(text.contains(&format!("node {}: ", node.path())), "{}: {}", case.name, node.path());
        }
    }
}

#[test]
fn printing_twice_is_byte_identical() {
    for case in Cases::all() {
        assert_eq!(case.ir.to_string(), case.ir.to_string(), "{}", case.name);
    }
    // Two builds of the same sequence print the same text as well.
    let build = || {
        let mut b = IrBuilder::new();
        let top = Devices::divider(&mut b);
        b.finish(top)
    };
    assert_eq!(build().to_string(), build().to_string());
}

#[test]
fn no_parser_of_the_text_exists() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&dir).expect("read src") {
        let path = entry.expect("dir entry").path();
        let text = std::fs::read_to_string(&path).expect("read a source");
        for needle in ["FromStr", "fn parse", "fn from_text", "fn from_str"] {
            assert!(!text.contains(needle), "{} holds `{needle}`", path.display());
        }
    }
}

#[test]
fn unequal_irs_print_different_texts() {
    let with = |r: f64| {
        let mut b = IrBuilder::new();
        let body = Devices::resistor(&mut b);
        let top = Devices::alone(&mut b, body, "resistor", 5, vec![r]);
        b.finish(top)
    };
    let (a, b) = (with(1000.0), with(1001.0));
    assert_ne!(a, b);
    assert_ne!(a.to_string(), b.to_string());
    // Each pair of distinct golden cases differs too.
    let cases = Cases::all();
    for (i, x) in cases.iter().enumerate() {
        for y in &cases[i + 1..] {
            assert_ne!(x.ir.to_string(), y.ir.to_string(), "{} vs {}", x.name, y.name);
        }
    }
}

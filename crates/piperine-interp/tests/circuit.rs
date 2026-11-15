//! `Interpreter::new` and the hand-built circuits (feature `ir-interp`, INT-01): an `Ir` that
//! fails `check()` is rejected with its entries; the crate spawns no process, reads no file and
//! holds no `unsafe`; the four M1 circuits are well formed.

use std::path::PathBuf;
use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{IrBuilder, NodeRef};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/circuits.rs"]
mod circuits;

use circuits::Circuits;
use devices::Devices;

#[test]
fn an_ir_that_fails_check_is_rejected() {
    // A resistor instance with no param bound and a node binding missing breaks `check()`.
    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![]);
    let ir = b.finish(top);
    let expected = ir.check();
    assert!(!expected.is_empty(), "the fixture must fail check()");
    let rejected = Interpreter::new(Arc::new(ir)).expect_err("a broken IR is rejected");
    assert_eq!(rejected.entries, expected);
    assert!(rejected.entries.iter().all(|e| !e.what.is_empty() && !e.origin.file.is_empty()));
}

#[test]
fn a_checked_ir_is_accepted() {
    for ir in [Circuits::divider(), Circuits::diode(), Circuits::rc_lowpass(), Circuits::rc_held()] {
        assert_eq!(ir.check(), vec![]);
        assert!(Interpreter::new(Arc::new(ir)).is_ok());
    }
}

#[test]
fn the_held_circuit_holds_out_at_zero() {
    let ir = Circuits::rc_held();
    let out = ir.node_at("RcHeld.out").expect("node out");
    assert_eq!(out.init().map(|r| r.get()), Some(0.0));
    let r1 = ir.instance_at("RcHeld.r1").expect("r1");
    assert_eq!(r1.binding(1), Some(NodeRef::Node(out.id())));
}

#[test]
fn the_crate_spawns_nothing_and_reads_no_file() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src];
    let mut seen = 0;
    while let Some(path) = files.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("read src dir") {
                files.push(entry.expect("dir entry").path());
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            seen += 1;
            let text = std::fs::read_to_string(&path).expect("read a source");
            for needle in ["std::process", "std::fs", "unsafe"] {
                assert!(!text.contains(needle), "{} contains `{needle}`", path.display());
            }
        }
    }
    assert!(seen > 0, "no source scanned");
}

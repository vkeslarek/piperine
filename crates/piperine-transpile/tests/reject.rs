//! `Transpiler::new` on an `Ir` that fails `check()` (feature `ir-rust`, RS-01 AC 5, check C5):
//! no transpiler and so no source, and the error holds the check's entries, each naming an item
//! and its origin.

use std::sync::Arc;

use piperine_ir::IrBuilder;
use piperine_transpile::{Error, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;

use devices::Devices;

#[test]
fn an_ir_that_fails_check_gives_no_source() {
    // A resistor instance with no param bound breaks `check()`.
    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![]);
    let ir = b.finish(top);
    let expected = ir.check();
    assert!(!expected.is_empty(), "the fixture must fail check()");

    match Transpiler::new(Arc::new(ir)) {
        Ok(_) => panic!("a broken IR gives a transpiler, and with it a source"),
        Err(Error::Rejected(entries)) => {
            assert_eq!(entries, expected);
            assert!(entries.iter().all(|e| !e.item.is_empty() && !e.what.is_empty() && !e.origin.file.is_empty()));
        }
        Err(other) => panic!("expected the check's entries, got {other}"),
    }
}

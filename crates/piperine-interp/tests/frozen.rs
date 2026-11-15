//! The interpreter is frozen at the M1 analog subset (R1, spec RS-09 AC 6): an `Ir` that holds a
//! body with a carrier, a drive or a reaction is `Rejected` by `Interpreter::new`, one entry
//! naming each such body; an analog body beside them is not named.

use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{BodySpec, InstanceSpec, IrBuilder, Origin};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use devices::Devices;
use digital::Buck;

#[test]
fn a_digital_body_is_rejected_by_name() {
    let mut b = IrBuilder::new();
    Devices::resistor(&mut b);
    Buck::osc(&mut b); // a carrier and a reaction
    Buck::gate_driver(&mut b); // carriers and drives
    Buck::switch(&mut b); // an `analog` block reading a carrier
    let top_body = b.body(BodySpec {
        name: "Top".into(),
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
        params: vec![],
        nodes: vec![],
        branches: vec![],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: None,
        children: vec![],
    });
    let top = b.instance(InstanceSpec {
        path: "Top".into(),
        name: "Top".into(),
        body: top_body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
    });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the design itself is well formed");

    let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a digital design was accepted") };
    let named: Vec<(&str, &str, u32)> = rejected.entries.iter().map(|e| (e.item.as_str(), e.origin.file.as_str(), e.origin.line)).collect();
    assert_eq!(named, vec![("body Osc", "dpwm.msl", 7), ("body GateDriver", "power.msl", 74), ("body Switch", "switch.msl", 1)]);
    for entry in &rejected.entries {
        assert!(entry.what.contains(&entry.item[5..]), "{entry:?}");
        assert!(entry.what.contains("carriers, drives or reactions"), "{entry:?}");
    }
}

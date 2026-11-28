//! Scope: every hand-built body of the IR fixtures crate is well-formed. Each body is built alone under a
//! `TopDesign` top, and `TopDesign::finish` holds the design to `Ir::check`, so a fixture that stops matching the IR's
//! rules fails here rather than in the first consumer that happens to use it.

use piperine_ir::{BodyId, IrBuilder};
use piperine_ir_fixtures::devices::Devices;
use piperine_ir_fixtures::digital::Buck;
use piperine_ir_fixtures::mixed::TopDesign;

/// One body builder of the crate, by the name its tests know it by.
struct BodyBuilder {
    name: &'static str,
    build: fn(&mut IrBuilder) -> BodyId,
}

impl BodyBuilder {
    /// Every body builder of `Devices` and `Buck`.
    fn all() -> Vec<BodyBuilder> {
        vec![
            BodyBuilder { name: "resistor", build: Devices::resistor },
            BodyBuilder { name: "capacitor", build: Devices::capacitor },
            BodyBuilder { name: "vsource", build: Devices::vsource },
            BodyBuilder { name: "isource", build: Devices::isource },
            BodyBuilder { name: "diode", build: Devices::diode },
            BodyBuilder { name: "everything", build: Devices::everything },
            BodyBuilder { name: "osc", build: Buck::osc },
            BodyBuilder { name: "ref_ramp", build: Buck::ref_ramp },
            BodyBuilder { name: "adc", build: Buck::adc },
            BodyBuilder { name: "comparator", build: Buck::comparator },
            BodyBuilder { name: "dff", build: Buck::dff },
            BodyBuilder { name: "setpoint_reg", build: Buck::setpoint_reg },
            BodyBuilder { name: "dpwm", build: Buck::dpwm },
            BodyBuilder { name: "pid", build: Buck::pid },
            BodyBuilder { name: "gate_driver", build: Buck::gate_driver },
            BodyBuilder { name: "switch", build: Buck::switch },
        ]
    }
}

#[test]
fn every_body_passes_check_under_a_top() {
    for body in BodyBuilder::all() {
        let mut wire = TopDesign::new();
        (body.build)(&mut wire.b);
        let design = wire.finish();
        assert_eq!(design.check(), vec![], "{} passes Ir::check", body.name);
    }
}

#[test]
fn the_divider_design_and_the_functions_pass_check() {
    let mut builder = IrBuilder::new();
    let divider = Devices::divider(&mut builder);
    let design = builder.finish(divider);
    assert_eq!(design.check(), vec![], "the divider design passes Ir::check");
    let mut wire = TopDesign::new();
    let rev3 = Buck::rev3(&mut wire.b);
    Buck::dithered_width(&mut wire.b, rev3);
    assert_eq!(wire.finish().check(), vec![], "the functions pass Ir::check");
}

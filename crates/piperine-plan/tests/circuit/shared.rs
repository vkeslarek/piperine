//! What the two backends share (feature `ir-rust`, RS-05 AC 2, check C28): `BodyPlan` is one
//! type, the interpreter's re-export of this crate's, and the shape a plan gives a body is the
//! shape the interpreter's module serves, entry by entry; `Netlist::circuit` binds each instance
//! to the module of its body as whatever implements `Modules` supplies it, and nothing else.

use std::any::TypeId;
use std::sync::Arc;

use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_interp::Interpreter;
use piperine_ir::{BodyId, Ir};
use piperine_plan::{BodyPlan, Modules, Netlist};
use piperine_solver::prelude::{Context, NodeIdentifier};

use piperine_ir_fixtures::circuits;
#[allow(dead_code)]
#[path = "../../../piperine-interp/tests/interpreter/models.rs"]
mod models;

use circuits::HandCircuits;
use models::{Models, View};

#[test]
fn the_interpreter_and_the_plan_crate_share_one_body_plan() {
    // one type: the interpreter re-exports this crate's plan, it does not keep a second one
    assert_eq!(TypeId::of::<piperine_interp::BodyPlan>(), TypeId::of::<BodyPlan>());

    // and that plan is what the interpreter serves: for each of the five M1 bodies, the shape
    // planned here equals the module's shape entry by entry
    let m = Models::all();
    let mut compared = Vec::new();
    for body in m.interp.ir().bodies() {
        let Some(module) = m.interp.module(body.id()) else { continue };
        let plan = BodyPlan::build(body).expect("the body plans");
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create `{}`: {e}", body.name()));
        let (served, planned) = (View { shape: handle.shape() }, View { shape: plan.shape().shape() });
        let name = body.name();
        assert_eq!(served.pin_names(), planned.pin_names(), "{name}: pins");
        assert_eq!(served.terms(), planned.terms(), "{name}: terms");
        assert_eq!(served.inputs(), planned.inputs(), "{name}: inputs");
        assert_eq!(served.function_names(), planned.function_names(), "{name}: functions");
        for f in planned.function_names() {
            assert_eq!(served.function(f), planned.function(f), "{name}: function `{f}`");
        }
        // the function table runs parallel to the shape's functions
        assert_eq!(plan.functions().len(), planned.function_names().len(), "{name}: function table");
        compared.push(name.to_string());
    }
    compared.sort();
    assert_eq!(compared, ["Capacitor", "Diode", "Isource", "Resistor", "Vsource"]);
}

/// A supplier other than the interpreter: the modules of another interpreter over the same
/// design, withholding the bodies named in `withheld`.
struct Supplier {
    from: Interpreter,
    withheld: &'static [&'static str],
}

impl Modules for Supplier {
    fn module(&self, body: BodyId) -> Option<&'static Module> {
        let name = self.from.ir().body(body).name();
        if self.withheld.contains(&name) { None } else { self.from.module(body) }
    }
}

fn instances(ir: &Ir, supplier: &Supplier) -> (Vec<String>, piperine_solver::prelude::CircuitInstance) {
    let circuit = Netlist::new(ir).circuit(supplier).unwrap_or_else(|e| panic!("circuit: {e}"));
    let mut paths = circuit.bound().names.instances.clone();
    paths.sort();
    (paths, circuit)
}

#[test]
fn the_netlist_binds_whatever_supplies_the_modules() {
    let ir = HandCircuits::divider();
    let supplier = |withheld| Supplier { from: Interpreter::new(Arc::new(ir.clone())).expect("a checked IR"), withheld };

    // every module supplied: the three instances are bound and the divider reads 7.5 V
    // (10 V across 1 k and 3 k)
    let (paths, mut circuit) = instances(&ir, &supplier(&[]));
    assert_eq!(paths, ["Divider.r1", "Divider.r2", "Divider.v1"]);
    let mid = NodeIdentifier::Anonymous(ir.node_at("Divider.mid").expect("mid").id().index());
    let v = circuit.dc(Context::default()).expect("dc").solve().expect("solve").get_node(&mid).expect("v(mid)");
    assert!((v - 7.5).abs() < 1e-9, "v(mid) = {v}");

    // a body the supplier does not supply gets no device: only the source is bound
    let (paths, _) = instances(&ir, &supplier(&["Resistor"]));
    assert_eq!(paths, ["Divider.v1"]);
}

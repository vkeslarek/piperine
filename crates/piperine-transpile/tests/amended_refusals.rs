//! Scope: the forms the IR amendments of msl-digital added, before the printer lowers them
//! (T40 onward: `now()` and `analysis(Static)` since T40, timers since T42, emit sites since
//! T44, flow reads since T46): a body that holds one is `Error::Unsupported` naming the form,
//! the body and the origin, and no text is printed. Each lowering task deletes the case it
//! lowers.

use std::sync::Arc;

use piperine_ir::{CarrierRole, Constant, Event, EventSource, Expr, InstanceSpec, Ir, IrBuilder, NetSpec, ValueType};
use piperine_transpile::{Error, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use digital::{Buck, Dx};

const F: &str = "amended.msl";

/// The refusal of a design.
struct Case;

impl Case {
    /// What printing `ir` gives: the refusal.
    fn refused(ir: Ir) -> Error {
        assert_eq!(ir.check(), vec![]);
        let transpiler = Transpiler::new(Arc::new(ir)).expect("the design passes check()");
        match transpiler.source() {
            Err(e) => e,
            Ok(text) => panic!("a form with no lowering printed:\n{text}"),
        }
    }
}

#[test]
fn the_previous_value_of_a_carrier_is_refused_by_name() {
    // `on q.changed() where q.prev().is_known() { q <- true; }`, the guard at line 2.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Edge", F, 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 20));
    let mut x = Dx::new(&mut b, F, 2, 24);
    let o = x.origin();
    let prev = x.b.expr(Expr::Previous(0), ValueType::Flagged(Box::new(ValueType::Bool)), o);
    let known = x.is_known(prev);
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: Some(known), origin: Buck::origin(F, 2, 8) });
    let mut x = Dx::new(&mut b, F, 2, 50);
    let yes = x.boolean(true);
    let commit = x.commit(0, yes, None);
    Buck::reaction(&mut b, body, vec![changed], vec![commit], vec![], F, 2);
    let at = b.next_instance_id();
    let net = b.net(NetSpec { path: "Top.q".into(), ty: ValueType::Bool, declared_by: at, init: None, origin: Buck::origin(F, 1, 20) });
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![], origin: Buck::origin(F, 1, 1) });
    b.bind_nets(top, vec![net]);
    assert_eq!(
        Case::refused(b.finish(top)),
        Error::Unsupported { form: "the previous value of a carrier".into(), path: "body Edge e0".into(), origin: Buck::origin(F, 2, 24) }
    );
}

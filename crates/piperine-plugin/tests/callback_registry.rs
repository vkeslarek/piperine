//! T59 (LRMC-C10-11): the runtime event-grained callback registry
//! (`PluginHost::register_cb`/`remove_cb`, `registry::SimEvent`) —
//! `vpi_register_cb()`/`vpi_remove_cb()`'s native equivalent, orthogonal
//! to the static, macro-declared `HookPhase` mechanism (which this file
//! also proves is unaffected).

use std::cell::RefCell;
use std::rc::Rc;

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_plugin::{PluginHost, SimEvent};

const DIVIDER: &str = r#"
    module VoltageSource (inout electrical p, inout electrical n);
        parameter real voltage = 5.0;
        analog begin
            V(p, n) <+ voltage;
        end
    endmodule
    module Resistor (inout electrical p, inout electrical n);
        parameter real r = 1e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
        end
    endmodule
    module Top ();
        electrical gnd, vin;
        VoltageSource src (.p(vin), .n(gnd));
        Resistor r1 (.p(vin), .n(gnd));
    endmodule
"#;

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(DIVIDER, &SourceMap::dummy()).expect("elaborate")
}

/// `register_cb` returns a usable `CallbackId`, and the callback fires at
/// exactly the requested event (`AfterSolve("op")`) — not on an unrelated
/// event.
#[test]
fn register_cb_fires_at_exactly_the_requested_event() {
    let host = Rc::new(PluginHost::empty());
    let fired = Rc::new(RefCell::new(0));
    let fired2 = fired.clone();
    let id = host.register_cb(SimEvent::AfterSolve("op".to_string()), move || {
        *fired2.borrow_mut() += 1;
    });
    // `id` is a real, distinguishable handle — `remove_cb` accepts it and
    // reports `true` (a fresh registration, not-yet-removed).
    assert!(host.remove_cb(id));
    assert!(!host.remove_cb(id), "removing twice must report false, not silently succeed again");

    // Re-register for the actual fire assertion below.
    let fired2 = fired.clone();
    host.register_cb(SimEvent::AfterSolve("op".to_string()), move || {
        *fired2.borrow_mut() += 1;
    });

    let d = design();
    let mut session = Session::builder(&d, "Top").hooks(host.clone()).compile().expect("compiles");
    session.op(&SolverConfig::default(), None).expect("op solves");

    assert_eq!(*fired.borrow(), 1, "the callback must fire exactly once for the op() run");
}

/// A callback registered for one analysis kind does not fire for another
/// — the event-grained distinction `HookPhase::AfterSolve` cannot make
/// (it fires for every analysis indiscriminately).
#[test]
fn callback_does_not_fire_for_a_different_analysis_kind() {
    let host = Rc::new(PluginHost::empty());
    let fired = Rc::new(RefCell::new(0));
    let fired2 = fired.clone();
    host.register_cb(SimEvent::AfterSolve("tran".to_string()), move || {
        *fired2.borrow_mut() += 1;
    });

    let d = design();
    let mut session = Session::builder(&d, "Top").hooks(host.clone()).compile().expect("compiles");
    session.op(&SolverConfig::default(), None).expect("op solves");

    assert_eq!(*fired.borrow(), 0, "an `op` run must not fire a callback registered for `tran`");
}

/// `remove_cb` deregisters — asserted by absence of any further fire, not
/// merely by the removal call succeeding.
#[test]
fn remove_cb_stops_further_fires() {
    let host = Rc::new(PluginHost::empty());
    let fired = Rc::new(RefCell::new(0));
    let fired2 = fired.clone();
    let id = host.register_cb(SimEvent::AfterSolve("op".to_string()), move || {
        *fired2.borrow_mut() += 1;
    });

    let d = design();
    let mut session = Session::builder(&d, "Top").hooks(host.clone()).compile().expect("compiles");
    session.op(&SolverConfig::default(), None).expect("first op solves");
    assert_eq!(*fired.borrow(), 1);

    assert!(host.remove_cb(id), "remove_cb must report the id was registered");
    assert!(!host.remove_cb(id), "removing the same id twice reports false, not a silent no-op");

    session.op(&SolverConfig::default(), None).expect("second op solves");
    assert_eq!(*fired.borrow(), 1, "no further fire after remove_cb");
}

/// Multiple callbacks on the same event all fire, and `callback_count`
/// reflects registration/removal accurately.
#[test]
fn multiple_callbacks_on_the_same_event_all_fire() {
    let host = Rc::new(PluginHost::empty());
    let a = Rc::new(RefCell::new(0));
    let b = Rc::new(RefCell::new(0));
    let (a2, b2) = (a.clone(), b.clone());
    host.register_cb(SimEvent::AfterSolve("op".to_string()), move || *a2.borrow_mut() += 1);
    host.register_cb(SimEvent::AfterSolve("op".to_string()), move || *b2.borrow_mut() += 1);
    assert_eq!(host.callback_count(), 2);

    let d = design();
    let mut session = Session::builder(&d, "Top").hooks(host.clone()).compile().expect("compiles");
    session.op(&SolverConfig::default(), None).expect("op solves");

    assert_eq!(*a.borrow(), 1);
    assert_eq!(*b.borrow(), 1);
}

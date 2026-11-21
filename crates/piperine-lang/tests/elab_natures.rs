//! Natures of every kind in `piperine_lang::elab` (feature `resolved-nets`, RN-08): the potential
//! drivers of a signal-flow node (§6 rule 2 and its Resolution), `E0242` naming the node and each
//! driver, and a node any of whose natures has `flow` being conservative and not counted. Sources
//! are written over the embedded standard library.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

/// A module that drives its `VSignal` port with a potential, and one that reads it.
const BLOCKS: &str = "module Source(o: VSignal) { }\nanalog for Source { v(o) +> 1.0; }\n\
                      module Reader(a: VSignal) { }\nanalog for Reader { let x = v(a); }\n\
                      module Load(p: Electrical) { }\nanalog for Load { let x = v(p); }\n";

#[test]
fn a_signal_flow_node_with_no_or_two_drivers_is_e0242() {
    // §6 Resolution of rule 2: a node every nature of which lacks `flow` takes a potential
    // contribution to `(n, gnd)` from exactly one instance. None, or two, is `E0242` naming the
    // node and each driver; one is fine.
    let one = format!("{BLOCKS}module Top() {{ signal s: VSignal; let a = Source(s); let r = Reader(s); }}");
    Elab::ir(&one, "Top");

    let none = format!("{BLOCKS}module Top() {{ signal s: VSignal; let r = Reader(s); }}");
    let diagnostics = Elab::run(&none, "Top").expect_err("no driver");
    assert_eq!(Elab::codes(&diagnostics), ["E0242"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("`Top.s`") && message.contains("0 potential driver") && message.contains("none"), "{message}");

    let two = format!("{BLOCKS}module Top() {{ signal s: VSignal; let a = Source(s); let b = Source(s); }}");
    let diagnostics = Elab::run(&two, "Top").expect_err("two drivers");
    assert_eq!(Elab::codes(&diagnostics), ["E0242"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("`Top.s`") && message.contains("2 potential driver") && message.contains("Top.a") && message.contains("Top.b"), "{message}");
}

#[test]
fn a_node_meeting_a_nature_with_flow_is_not_counted() {
    // §4 rule 13 Resolution, §6 Resolution of rule 2: a node where an `Electrical` port meets a
    // `VSignal` port is conservative; it is not counted, so no driver and two drivers are both
    // accepted here.
    let none = format!("{BLOCKS}module Top() {{ signal s: VSignal; let l = Load(s); let r = Reader(s); }}");
    Elab::ir(&none, "Top");
    let two = format!("{BLOCKS}module Top() {{ signal s: VSignal; let l = Load(s); let a = Source(s); let b = Source(s); }}");
    Elab::ir(&two, "Top");
}

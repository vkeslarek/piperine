//! Bundles elaborated (feature `resolved-nets-bundles`, BN-02, BN-03; spec part II §7 rules 7, 14 and 15): a bundle port
//! or signal lowers to one carrier or node per leaf field named by its dotted path, a connection of a bundle joins each
//! field to the field of the same name, and the one-driver rule is judged field by field after the views.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{CarrierRole, Ir};
use support::Elab;

/// What a test reads of an elaborated design.
struct Read<'a> {
    ir: &'a Ir,
}

impl Read<'_> {
    /// The `(name, role)` of each carrier of the body `name`, in order.
    fn carriers(&self, name: &str) -> Vec<(String, CarrierRole)> {
        let body = self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`:\n{}", self.ir.print_structural()));
        body.carriers().iter().map(|c| (c.name.clone(), c.role.clone())).collect()
    }

    /// The names of the nodes of the body `name`, in order.
    fn nodes(&self, name: &str) -> Vec<String> {
        let body = self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`"));
        body.nodes().iter().map(|n| n.name.clone()).collect()
    }
}

#[test]
fn a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path() {
    // §7 rule 14: a bundle has no semantics of its own, each field is the carrier or node it is, named by its path
    let src = "struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }\n\
               struct Pair { p: Electrical, n: Electrical }\n\
               struct Link { out up: Bit, pair: Pair }\n\
               module Top(tx: Stream, rx: flip Stream, in s: Stream, l: Link) {\n\
                   signal go: Bit = false;\n\
                   tx.valid = go; tx.data = 1; rx.ready = go; l.up = go;\n\
               }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    let names: Vec<&str> = carriers.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(names, ["tx.valid", "tx.ready", "tx.data", "rx.valid", "rx.ready", "rx.data", "s.valid", "s.ready", "s.data", "l.up", "go"], "ports in order, each bundle as its fields, then the signals");
    let role = |name: &str| carriers.iter().find(|c| c.0 == name).map(|c| c.1.clone()).unwrap_or_else(|| panic!("no carrier `{name}`"));
    // the views: the producer's `out` fields are `Out`, its `in` field `In`; the consumer's are the reverse; an observer reads all
    assert_eq!((role("tx.valid"), role("tx.ready")), (CarrierRole::Out, CarrierRole::In));
    assert_eq!((role("rx.valid"), role("rx.ready")), (CarrierRole::In, CarrierRole::Out));
    assert_eq!((role("s.valid"), role("s.ready"), role("s.data")), (CarrierRole::In, CarrierRole::In, CarrierRole::In));
    // a nested bundle's fields are named through it, and a continuous field is a node of the body
    assert_eq!(read.nodes("Top"), ["l.pair.p", "l.pair.n"]);
}

#[test]
fn a_bundle_signal_declares_every_carrier_and_drives_none_it_does_not_write() {
    // §7 rule 7: `signal a: Stream;` declares every carrier of the bundle; the declaring module drives none of its
    // one-driver fields unless it writes them
    let src = "struct Stream { out valid: Bit<Logic>, in ready: Bit<Logic> }\n\
               struct Bus { out v: Bit<Logic>, w: Electrical }\n\
               module Top(in clk: Bit) { signal a: Stream; signal b: Bus; }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    assert_eq!(carriers.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["clk", "a.valid", "a.ready", "b.v"]);
    assert!(carriers[1..].iter().all(|c| c.1 == CarrierRole::Signal), "a signal's fields are signals of the body: {carriers:?}");
    assert_eq!(read.nodes("Top"), ["b.w"], "the continuous field of a signal is a node");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert!(body.drives().is_empty(), "the declaring module drives no field it does not write");
}

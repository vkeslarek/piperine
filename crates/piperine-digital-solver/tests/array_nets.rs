//! Array nets (composite-values CV-23 to CV-27, part II §5.1.8, abi.md §3.1 P4a): an array of carriers is one net
//! whose store holds only the elements written, an element never written reads the array's initial value and its
//! `PREVIOUS` is that value, and a rewind restores the map. T67 adds windows and element commits to this target.

use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::store::Store;

/// The arrays this target builds: one net of one-byte elements, `count` of them.
struct Arrays;

impl Arrays {
    /// A store holding one array net `lines` of `count` one-byte elements whose initial bytes are `init`.
    fn store(count: u64, init: &[u8]) -> (Store, NetId) {
        let spec = NetSpec { init: init.to_vec(), ..NetSpec::array("lines", 1, 1, count) };
        (Store::new(&[spec]), NetId(0))
    }
}

#[test]
fn a_sparse_array_stores_only_written_elements_and_rewinds() {
    // CV-27 AC 1, §5.1.8: `1 << 32` elements and 1 000 distinct writes hold 1 000 entries, not 2^32.
    let (mut store, net) = Arrays::store(1 << 32, &[0]);
    for k in 0..1000u64 {
        let element = k * 4_000_000;
        assert!(store.write_element(net, element, 0, &[1]), "element {element} changes from its initial 0");
    }
    assert_eq!(store.array(net).expect("an array net").written_len(), 1000, "storage is what was written");
    assert_eq!(store.element_value(net, 4_000_000), [1]);
    assert_eq!(store.element_value(net, 4_000_001), [0], "a neighbour never written reads the initial value");
    // a write of the value an element already holds is no change and stores nothing
    assert!(!store.write_element(net, 7, 0, &[0]));
    assert_eq!(store.array(net).expect("an array net").written_len(), 1000);

    // R6: a rewind restores the map.
    let mark = store.mark();
    store.begin_microstep();
    assert!(store.write_element(net, 7, 0, &[1]), "a new element");
    assert!(store.write_element(net, 4_000_000, 0, &[0]), "an existing element changes back");
    assert_eq!(store.array(net).expect("an array net").written_len(), 1001);
    store.rewind(&mark);
    assert_eq!(store.array(net).expect("an array net").written_len(), 1000, "the new element left the map");
    assert_eq!(store.element_value(net, 7), [0]);
    assert_eq!(store.element_value(net, 4_000_000), [1], "the changed element has its value back");
    assert_eq!(store.element_previous(net, 4_000_000), [0], "and the previous it had");
    store.release(mark);
}

#[test]
fn an_unwritten_element_reads_its_initial_value_and_previous() {
    // §5.1.8: an element never written reads the array's initial value for that element, and `prev()` is the same.
    // `[x; K]`: one element's bytes for every element.
    let (store, net) = Arrays::store(8, &[5]);
    for element in [0, 3, 7] {
        assert_eq!(store.element_value(net, element), [5], "element {element} of [5; 8]");
        assert_eq!(store.element_previous(net, element), [5], "its previous is the same");
        assert!(!store.element_changed(net, element), "nothing was committed");
    }
    // `[a, b, c]`: every element's bytes.
    let (each, net) = Arrays::store(3, &[1, 2, 3]);
    assert_eq!([0, 1, 2].map(|element| each.element_value(net, element)[0]), [1, 2, 3]);
    // no init: all zero bytes (abi.md §3.1 P3)
    let (zero, net) = Arrays::store(4, &[]);
    assert_eq!(zero.element_value(net, 2), [0]);

    // part IV §13 rule 6: the initial commit is a change of every element, in the microstep it lands in only.
    let (mut store, net) = Arrays::store(8, &[5]);
    store.stamp_initial_array(net);
    store.begin_microstep();
    assert!(store.element_changed(net, 3), "every element changes in the initial microstep");
    assert_eq!(store.array(net).expect("an array net").written_len(), 0, "and none is materialized");
    store.begin_microstep();
    assert!(!store.element_changed(net, 3), "the next microstep it is quiet again");
    // a written element changes in its own microstep and keeps the old value as its previous
    store.begin_microstep();
    assert!(store.write_element(net, 3, 0, &[9]));
    assert!(store.element_changed(net, 3));
    assert!(!store.element_changed(net, 4), "a neighbour did not change");
    assert_eq!((store.element_value(net, 3), store.element_previous(net, 3)), (&[9u8][..], &[5u8][..]));
}

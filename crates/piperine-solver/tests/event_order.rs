//! Digital event queue order (SR-05): events pop by `time`, then by `seq`.
//! Events are built through `QueueSink` and re-stamped, so the test names no
//! field the ordering does not use.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_solver::abi::{DigitalEvent, DigitalNet, EventSink, LogicValue, QueueSink};

/// One event produced by the real sink, to be re-stamped by the caller.
fn template() -> DigitalEvent {
    let mut queue = BinaryHeap::new();
    let mut seq = 0u64;
    QueueSink::new(&mut queue, 0.0, 0, &mut seq).emit(DigitalNet(0), LogicValue::One, 0.0);
    queue.pop().expect("the sink pushed one event").0
}

fn stamped(time: f64, seq: u64) -> DigitalEvent {
    DigitalEvent { time, seq, ..template() }
}

#[test]
fn same_time_events_order_by_seq() {
    let mut queue = BinaryHeap::new();
    for seq in [2u64, 0, 1] {
        queue.push(Reverse(stamped(5.0, seq)));
    }
    let popped: Vec<u64> = std::iter::from_fn(|| queue.pop().map(|Reverse(e)| e.seq)).collect();
    assert_eq!(popped, vec![0, 1, 2], "one time: ascending seq");

    let mut queue = BinaryHeap::new();
    queue.push(Reverse(stamped(2.0, 9)));
    queue.push(Reverse(stamped(1.0, 100)));
    let first = queue.pop().expect("two events queued").0;
    assert_eq!((first.time, first.seq), (1.0, 100), "an earlier time pops before a lower seq");
}

//! The commit queue of the ABI engine (feature `solver-abi-digital`, SAD-09;
//! abi.md §6 C2 and C3): pending commits by due instant, the order commits to
//! one instant apply in, and transport removal.

use piperine_digital_solver::net::NetId;
use piperine_digital_solver::queue::{Pending, Queue, Writer};
use piperine_digital_solver::tick::Tick;

const N: NetId = NetId(0);
const M: NetId = NetId(1);

fn by(instance: u32, reaction: u32) -> Writer {
    Writer::Reaction { instance, reaction }
}

/// A commit of one byte `v` at `offset` of net `N`, written by statement
/// `statement` of reaction 0 of instance 0, made at (`made`, 0), due at (`due`, 0).
fn commit(statement: u32, offset: u32, v: u8, made: i64, due: i64) -> Pending {
    Pending {
        net: N,
        element: None,
        offset,
        bytes: vec![v],
        writer: by(0, 0),
        statement,
        made: (Tick(made), 0),
        call: 0,
        due: (Tick(due), 0),
    }
}

fn values(p: &[Pending]) -> Vec<u8> {
    p.iter().map(|c| c.bytes[0]).collect()
}

#[test]
fn commits_apply_in_made_statement_call_order() {
    // C2: (instant made, statement index, call order), pushed shuffled; all due at 10.
    let mut q = Queue::new();
    // Statements differ or the writers do, or C3 would remove the earlier push.
    let mk = |v, reaction, statement, made, call| Pending {
        call,
        writer: by(0, reaction),
        ..commit(statement, 0, v, made, 10)
    };
    q.push(mk(5, 2, 0, 3, 0), false); // made 3: last although statement 0
    q.push(mk(2, 0, 0, 2, 1), false); // made 2, statement 0, call 1
    q.push(mk(1, 1, 0, 2, 0), false); // made 2, statement 0, call 0 (another reaction)
    q.push(mk(3, 0, 2, 2, 0), false); // made 2, statement 2
    q.push(mk(4, 0, 1, 2, 0), false); // made 2, statement 1
    let due = q.take_due((Tick(10), 0));
    assert_eq!(values(&due), [1, 2, 4, 3, 5], "made, then statement, then call; the last (5) is the value");
    assert!(q.is_empty());
}

#[test]
fn transport_removes_later_commits_of_the_same_statement_only() {
    // C3
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 5), false); // earlier due: stays
    q.push(commit(0, 0, 2, 0, 20), false); // later due, same statement and range: goes
    q.push(commit(0, 0, 3, 0, 10), false); // due exactly at the new one's instant: goes
    q.push(Pending { statement: 1, ..commit(1, 0, 4, 0, 20) }, false); // another statement: stays
    q.push(Pending { writer: by(0, 1), ..commit(0, 0, 5, 0, 20) }, false); // same index, another reaction: stays
    q.push(commit(0, 0, 9, 1, 10), false); // the new commit
    let mut got = Vec::new();
    while let Some(at) = q.next_due() {
        got.extend(values(&q.take_due(at)));
    }
    assert_eq!(got, [1, 9, 5, 4], "at 20: statement 0 of reaction 1 before statement 1");
}

#[test]
fn replace_removes_every_pending_commit_of_the_statement() {
    // C3: a REPLACE statement removes all of them, earlier ones included, whatever the range.
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 5), false);
    q.push(commit(0, 3, 2, 0, 20), false);
    q.push(commit(1, 0, 3, 0, 7), false); // another statement
    q.push(commit(0, 0, 9, 1, 12), true);
    let mut got = Vec::new();
    while let Some(at) = q.next_due() {
        got.extend(values(&q.take_due(at)));
    }
    assert_eq!(got, [3, 9]);
}

#[test]
fn disjoint_byte_ranges_of_one_statement_do_not_remove_each_other() {
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 20), false); // byte 0
    q.push(commit(0, 1, 2, 0, 20), false); // byte 1: disjoint from the next
    q.push(commit(0, 2, 3, 1, 5), false); // byte 2, earlier due
    assert_eq!(q.len(), 3);
    // a two-byte commit over bytes 1..3 overlaps byte 1 only (byte 2 is due earlier)
    q.push(Pending { bytes: vec![7, 8], ..commit(0, 1, 0, 2, 10) }, false);
    let mut got = Vec::new();
    while let Some(at) = q.next_due() {
        got.extend(q.take_due(at).iter().map(|c| (c.offset, c.bytes.clone())));
    }
    assert_eq!(got, [(2, vec![3]), (1, vec![7, 8]), (0, vec![1])]);
}

#[test]
fn a_commit_of_another_net_is_not_transported() {
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 20), false);
    q.push(Pending { net: M, ..commit(0, 0, 2, 1, 10) }, false);
    assert_eq!(q.len(), 2, "same statement, different net: nothing removed");
}

#[test]
fn a_host_commit_is_never_transported() {
    let mut q = Queue::new();
    let host = |v, due| Pending { writer: Writer::Host, ..commit(0, 0, v, 0, due) };
    q.push(host(1, 20), false);
    q.push(host(2, 10), false);
    assert_eq!(q.len(), 2);
}

#[test]
fn next_due_is_the_earliest_instant() {
    let mut q = Queue::new();
    assert_eq!(q.next_due(), None);
    q.push(commit(0, 0, 1, 0, 30), false);
    q.push(Pending { statement: 1, due: (Tick(10), 2), ..commit(1, 0, 2, 0, 10) }, false);
    q.push(Pending { statement: 2, due: (Tick(10), 1), ..commit(2, 0, 3, 0, 10) }, false);
    assert_eq!(q.next_due(), Some((Tick(10), 1)), "by tick, then microstep");
    assert_eq!(values(&q.take_due((Tick(10), 1))), [3]);
    assert_eq!(q.next_due(), Some((Tick(10), 2)));
    assert_eq!(q.take_due((Tick(11), 0)), [], "nothing due there");
    let c = q.clone();
    assert_eq!(c, q, "a queue clones for the checkpoint");
}

/// HYG-14 AC 2: a rewind undoes pushes, the removals a push or a cancel makes (C3) and the
/// commits taken, so the queue equals the copy taken at the mark, order and all; marks nest.
#[test]
fn a_queue_rewind_restores_its_pending_commits() {
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 5), false);
    q.push(commit(0, 0, 2, 0, 20), false);
    q.push(commit(1, 0, 3, 0, 20), false);
    q.push(Pending { net: M, ..commit(0, 0, 4, 0, 20) }, false);
    let outer_copy = q.clone();
    let outer = q.mark();
    assert_eq!(q.journal_len(), 0);
    // a push that transports away the later commits of its statement (C3), and inserts
    q.push(commit(0, 0, 9, 1, 10), false);
    assert_eq!(values(&q.due_at((Tick(20), 0)).cloned().collect::<Vec<_>>()), [3, 4], "the statement-0 commit at 20 went");
    let inner_copy = q.clone();
    let inner = q.mark();
    // a take, a cancel and a REPLACE push inside the inner mark
    let taken = q.take_due((Tick(5), 0));
    assert_eq!(values(&taken), [1]);
    q.cancel(&commit(1, 0, 0, 0, 0));
    q.push(commit(0, 0, 7, 2, 30), true);
    assert_ne!(q, inner_copy);
    q.rewind(&inner);
    assert_eq!(q, inner_copy, "the inner rewind equals the copy at the inner mark");
    q.release(inner);
    q.rewind(&outer);
    assert_eq!(q, outer_copy, "the outer rewind equals the copy at the outer mark: the removed commit is back");
    q.release(outer);
    assert_eq!(q.journal_len(), 0, "the outermost release empties the log");
    q.push(commit(0, 0, 8, 3, 40), false);
    assert_eq!(q.journal_len(), 0, "no mark open: nothing is journaled");
}

/// The value of every commit due at `instant`, in C2 order.
fn due_at(q: &mut Queue, instant: i64) -> Vec<u8> {
    values(&q.take_due((Tick(instant), 0)))
}

#[test]
fn a_plain_commit_removes_the_pending_it_transports() {
    // abi.md C3 with one kind of commit: the pending commits of its statement that are due at or
    // after it and share a byte with it go; earlier due, another statement and another range stay.
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 5), false); // earlier due: untouched
    q.push(commit(0, 0, 3, 0, 10), false); // due at the new one's instant: removed
    q.push(commit(0, 0, 2, 0, 20), false); // later due, same statement and range: removed
    q.push(Pending { statement: 1, ..commit(1, 0, 4, 0, 20) }, false); // another statement: untouched
    q.push(commit(0, 0, 9, 1, 10), false);
    assert_eq!(q.len(), 3, "two removed, the new one inserted");
    assert_eq!(due_at(&mut q, 5), [1]);
    assert_eq!(due_at(&mut q, 10), [9]);
    assert_eq!(due_at(&mut q, 20), [4]);
}

#[test]
fn a_rewind_restores_a_removal() {
    // HYG-14: a rejected step leaves no trace, a transport removal included
    let mut q = Queue::new();
    q.push(commit(0, 0, 1, 0, 20), false);
    q.push(commit(0, 4, 2, 0, 20), false);
    let before = q.clone();
    let mark = q.mark();
    q.push(commit(0, 0, 9, 1, 10), false);
    q.push(commit(0, 4, 8, 1, 12), false);
    assert_ne!(q, before);
    q.rewind(&mark);
    assert_eq!(q, before, "the queue equals the copy taken at the mark: the removed back, the pushed gone");
    assert_eq!(due_at(&mut q, 20), [1, 2]);
}

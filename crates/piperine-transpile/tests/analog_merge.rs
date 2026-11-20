//! An `if` on a value that may be unknown inside an `analog` block (feature
//! `unknown-semantics`, MSL §3.3.3, US-04 AC 10): the block is built by hand, compiled, and its
//! terms function is called through the ABI. `analog` reads no carrier outside the flag reads
//! (§3.3.5), and the checker refuses `Unknown` there, so the IR is the one way to reach the
//! printer with a condition that may be unknown (the literal `Unknown`, which the IR allows).
//! Expected currents are written from the merge rule, never from the printed source: an unknown
//! condition runs both arms and merges them (equal values stay known, anything else is unknown),
//! and `known_or` gives its default for an unknown merge.
//!
//! ```text
//! module Gate(p: Electrical, n: Electrical) { a: f64; b: f64; }
//! analog for Gate { i(p, n) +> v(p, n) * (if Unknown { a } else { b }).known_or(1.0); }
//! ```
#![cfg(unix)]

use piperine_abi::caller::Handle;
use piperine_abi::raw::{Cell, List, cell};
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, IrBuilder, NodeRole, Operator, QuantityRole, Stmt, Terminal,
    ValueType,
};
use piperine_plan::Modules;
use piperine_transpile::{Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/alone.rs"]
mod alone;

use alone::Alone;
use digital::{Buck, Dx};

const F: &str = "gate.msl";

/// The `Gate` body above, `c` its one carrier.
fn gate(b: &mut IrBuilder) -> BodyId {
    let el = Buck::electrical(b);
    let node = |name: &str, col: u32| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) };
    let mut x = Dx::new(b, F, 2, 30);
    let volts = x.probe(Terminal::Node(0), Terminal::Node(1));
    let c = x.unknown(digital::Ty::flagged(ValueType::Bool));
    let (a, other) = (x.param(0, ValueType::F64), x.param(1, ValueType::F64));
    let chosen = x.if_(c, a, other, digital::Ty::flagged(ValueType::F64));
    let fallback = x.real(1.0);
    let conductance = x.known_or(chosen, fallback, ValueType::F64);
    let value = x.bin(BinaryOp::Mul, volts, conductance, ValueType::F64);
    let stmt = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
    let body = b.body(BodySpec {
        params: vec![Buck::param("a", F, 1, None, None), Buck::param("b", F, 1, None, None)],
        nodes: vec![node("p", 20), node("n", 34)],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![stmt]),
        ..Buck::spec("Gate", F, 1)
    });
    body
}

/// A body with one carrier and nothing to run: its digital form is what gives the design the
/// runtime that has `rt::Flagged`, which a merge of an unknown condition needs (an `analog`-only
/// design has `real.rs` alone, and the source never reaches the printer with an unknown there).
fn idle(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Idle", F, 9));
    b.carrier(body, Carrier { name: "c".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: Buck::origin(F, 9, 5) });
    body
}

fn items<T: Copy>(list: &List<'_, T>) -> Vec<T> {
    (0..list.len()).map(|i| *list.get(i).expect("a readable element")).collect()
}

/// The current the gate carries at 2 V when its params are `a` and `b`.
fn current(a: f64, b: f64) -> f64 {
    let mut ir = IrBuilder::new();
    let body = gate(&mut ir);
    idle(&mut ir);
    let ir = Alone::finish(ir);
    let cache = tempfile::tempdir().expect("a temp dir");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let compiled = Transpiler::new(std::sync::Arc::new(ir)).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
    let module = Modules::module(&compiled, body).expect("the gate's module");
    let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create: {e}"));
    let shape = handle.shape();
    let k = items(&shape.functions).iter().position(|f| f.name.as_str() == "terms").expect("terms");
    let function = items(&shape.functions)[k];
    let reads: Vec<Vec<u8>> = items(&function.reads)
        .iter()
        .map(|c: &Cell| {
            assert_eq!(c.kind, cell::PIN, "a gate reads pins");
            let value = match items(&shape.pins)[c.index as usize].name.as_str() {
                "p" => 2.0,
                "n" => 0.0,
                "a" => a,
                _ => b,
            };
            value.to_le_bytes().to_vec()
        })
        .collect();
    let reads: Vec<&[u8]> = reads.iter().map(Vec::as_slice).collect();
    let partials = function.partials.len();
    let mut out = [0u8; 8];
    let mut jac = vec![0.0; partials];
    handle.eval(k as u32, &reads, &mut [&mut out[..]], &vec![1; partials], &mut jac).unwrap_or_else(|e| panic!("{e}"));
    f64::from_le_bytes(out)
}

#[test]
fn an_unknown_if_in_an_analog_block_merges() {
    // the condition is unknown, so both arms run and merge: equal arms are that value, known, and
    // arms that differ are unknown, which `known_or` gives the default 1.0
    assert_eq!(current(3.0, 3.0), 2.0 * 3.0);
    assert_eq!(current(3.0, 5.0), 2.0 * 1.0);
}

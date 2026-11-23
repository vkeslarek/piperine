//! An `if` on a carrier inside an `analog` block (MSL §3.3.5, §3.3.6): the block is built by hand,
//! compiled, and its terms function is called through the ABI. The carrier is read like any
//! value, so the `if` runs the one arm its value names. Expected currents are written from that
//! rule, never from the printed source.
//!
//! ```text
//! module Gate(p: Electrical, n: Electrical) { a: f64; b: f64; signal en: Bit = false; }
//! analog for Gate { i(p, n) +> v(p, n) * (if en { a } else { b }); }
//! ```
#![cfg(unix)]

use piperine_abi::caller::Handle;
use piperine_abi::raw::{Cell, List, cell};
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Constant, IrBuilder, NodeRole, Operator, QuantityRole, Stmt, Terminal,
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

/// The `Gate` body above.
fn gate(b: &mut IrBuilder) -> BodyId {
    let el = Buck::electrical(b);
    let node = |name: &str, col: u32| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) };
    let mut x = Dx::new(b, F, 2, 30);
    let volts = x.probe(Terminal::Node(0), Terminal::Node(1));
    let en = x.carrier(0, ValueType::Bool);
    let (a, other) = (x.param(0, ValueType::F64), x.param(1, ValueType::F64));
    let conductance = x.if_(en, a, other, ValueType::F64);
    let value = x.bin(BinaryOp::Mul, volts, conductance, ValueType::F64);
    let stmt = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
    let body = b.body(BodySpec {
        params: vec![Buck::param("a", F, 1, None, None), Buck::param("b", F, 1, None, None)],
        nodes: vec![node("p", 20), node("n", 34)],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![stmt]),
        ..Buck::spec("Gate", F, 1)
    });
    b.carrier(body, Carrier { name: "en".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: Some(Constant::Bool(false)), origin: Buck::origin(F, 1, 50) });
    body
}

fn items<T: Copy>(list: &List<'_, T>) -> Vec<T> {
    (0..list.len()).map(|i| *list.get(i).expect("a readable element")).collect()
}

/// The current the gate carries at 2 V when its params are `a` and `b` and `en` is `enabled`.
fn current(a: f64, b: f64, enabled: bool) -> f64 {
    let mut ir = IrBuilder::new();
    let body = gate(&mut ir);
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
                "b" => b,
                _ => return vec![u8::from(enabled), 1],
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
fn an_if_in_an_analog_block_runs_the_arm_its_carrier_names() {
    // the condition is the value of `en`: its `then` arm is `a`, its `else` arm is `b`
    assert_eq!(current(3.0, 5.0, true), 2.0 * 3.0);
    assert_eq!(current(3.0, 5.0, false), 2.0 * 5.0);
}

//! Phase 1.5 — IR → DigitalInterpreter, TDD style.

use piperine_lang::DigitalInterpreter;
use piperine_lang::ir_digital_to_interp;
use piperine_lang::ppr_to_ir;
use piperine_lang::parse_and_elaborate;
use piperine_solver::digital::{DigitalNet, LogicValue};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

const DFF_SRC: &str = "
discipline Bit {}
mod DFF (input clk: Bit, input D: Bit, output Q: Bit) {}
digital DFF {
    @ posedge(clk) {
        Q <- D;
    }
}
";

const BUF_SRC: &str = "
discipline Bit {}
mod Buf (input A: Bit, output Y: Bit) {}
digital Buf {
    @ change(A) {
        Y <- A;
    }
}
";

#[test]
fn ir_digital_dff_posedge_captures_d() {
    let elab = parse_and_elaborate(DFF_SRC).expect("parse_and_elaborate DFF");
    let ir = ppr_to_ir(&elab);
    let mut interp: DigitalInterpreter =
        ir_digital_to_interp(&ir, "DFF").expect("interp compiles");
    let mut port_net = HashMap::new();
    port_net.insert("clk".to_string(), DigitalNet(0));
    port_net.insert("D".to_string(),   DigitalNet(1));
    port_net.insert("Q".to_string(),   DigitalNet(2));
    interp.set_port_nets(port_net);

    let mut queue: BinaryHeap<Reverse<piperine_solver::digital::DigitalEvent>> =
        BinaryHeap::new();
    interp.init(&mut queue);

    let mut nets = vec![LogicValue::Zero, LogicValue::One, LogicValue::X];
    nets[0] = LogicValue::One;
    interp.eval(1e-9, &nets, &mut queue);

    assert_eq!(queue.len(), 1);
    let Reverse(ev) = queue.pop().unwrap();
    assert_eq!(ev.net, DigitalNet(2));
    assert_eq!(ev.value, LogicValue::One, "Q = D = 1");
}

#[test]
fn ir_digital_buf_change_follows_input() {
    let elab = parse_and_elaborate(BUF_SRC).expect("parse_and_elaborate Buf");
    let ir = ppr_to_ir(&elab);
    let mut interp: DigitalInterpreter =
        ir_digital_to_interp(&ir, "Buf").expect("interp compiles");
    let mut port_net = HashMap::new();
    port_net.insert("A".to_string(), DigitalNet(0));
    port_net.insert("Y".to_string(), DigitalNet(1));
    interp.set_port_nets(port_net);

    let mut queue: BinaryHeap<Reverse<piperine_solver::digital::DigitalEvent>> =
        BinaryHeap::new();
    interp.init(&mut queue);

    let nets = vec![LogicValue::One, LogicValue::X];
    interp.eval(0.0, &nets, &mut queue);

    assert_eq!(queue.len(), 1);
    let Reverse(ev) = queue.pop().unwrap();
    assert_eq!(ev.net, DigitalNet(1));
    assert_eq!(ev.value, LogicValue::One);
}

#[test]
fn ir_digital_returns_err_for_module_without_digital_body() {
    let body = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    ";
    let elab = parse_and_elaborate(body).expect("parse_and_elaborate");
    let ir = ppr_to_ir(&elab);
    assert!(ir_digital_to_interp(&ir, "R").is_err());
}

use piperine_common::Command;
use piperine_common::spice::*;
use piperine_coordinator::pool::{PoolConfig, ProcessPool};

fn main() {
    let pool = ProcessPool::spawn(PoolConfig {
        size: 2,
        ..Default::default()
    }).expect("spawn workers");
    eprintln!("coordinator: {} workers running\n", pool.len());

    let w0 = pool.handle(0);

    let circ = Netlist::new("RC filter")
        .push(Resistor::new("R1", 1, 2, 1e3))
        .push(Capacitor::new("C1", 2, Node::Ground, 1e-6))
        .push(VoltageDc::new("V1", 1, Node::Ground, 5.0));

    w0.cmd.send(Command::from(circ)).unwrap();
    eprintln!("load: {:?}", w0.resp.recv().unwrap());

    w0.cmd.send(Command::from(Analysis::Op)).unwrap();
    eprintln!("op: {:?}", w0.resp.recv().unwrap());

    w0.cmd.send(Command::GetVecData { name: "1".into() }).unwrap();
    eprintln!("v(1): {:?}", w0.resp.recv().unwrap());

    for i in 0..pool.len() {
        let _ = pool.handle(i).cmd.send(Command::Shutdown);
        let _ = pool.handle(i).resp.recv();
    }
    drop(pool);
}

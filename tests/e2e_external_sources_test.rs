//! Test external sources with bilateral IPC.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use piperine_core::prelude::*;
use piperine_core::engine::SimulationEngine;
use piperine_ngspice::protocol::*;

fn write_msg<T: serde::Serialize>(w: &mut impl Write, msg: &T) {
    let bytes = bincode::serialize(msg).unwrap();
    w.write_all(&(bytes.len() as u32).to_le_bytes()).unwrap();
    w.write_all(&bytes).unwrap();
    w.flush().unwrap();
}

fn read_msg<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> T {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).unwrap();
    bincode::deserialize(&buf).unwrap()
}

/// Test bilateral IPC: worker sends ExternalSourceRequest, we respond.
#[test]
fn external_source_bilateral_ipc() {
    let exe = env!("CARGO_BIN_EXE_piperine");
    let mut child = Command::new(exe)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut w = child.stdin.take().unwrap();
    let r = child.stdout.take().unwrap();

    // Reader thread: reads all messages from worker
    let (msg_tx, msg_rx) = mpsc::channel::<WorkerToMain>();
    thread::spawn(move || {
        let mut r = r;
        loop {
            match (|| -> Option<WorkerToMain> {
                let mut len_buf = [0u8; 4];
                r.read_exact(&mut len_buf).ok()?;
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                r.read_exact(&mut buf).ok()?;
                bincode::deserialize(&buf).ok()
            })() {
                Some(msg) => { if msg_tx.send(msg).is_err() { break; } }
                None => break,
            }
        }
    });

    // Ready
    let msg = msg_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(msg, WorkerToMain::Ready));

    let ckt = Circuit::new("External IPC Test")
        .v_external("sensor", "sig", GND)
        .resistor("1", "sig", "out", "1k")
        .resistor("2", "out", GND, "1k");

    let tran = TranAnalysis::new(1e-4, 1e-3);

    write_msg(&mut w, &MainToWorker::RunSimulation {
        netlist_lines: ckt.to_netlist_lines(),
        control_commands: tran.to_control_commands(),
        has_external_sources: true,
    });

    let mut callback_count = 0u64;
    loop {
        let msg = msg_rx.recv_timeout(Duration::from_secs(30))
            .expect("timeout waiting for worker message");

        match msg {
            WorkerToMain::ExternalSourceRequest { request_id, .. } => {
                callback_count += 1;
                write_msg(&mut w, &MainToWorker::ExternalSourceValue {
                    request_id,
                    value: 5.0,
                });
            }
            WorkerToMain::SimulationComplete { plots, .. } => {
                println!("External source: {callback_count} callbacks, {} plots", plots.len());
                assert!(callback_count > 0);
                assert!(!plots.is_empty());
                break;
            }
            WorkerToMain::Error { message } => panic!("error: {message}"),
            _ => {}
        }
    }

    write_msg(&mut w, &MainToWorker::Shutdown);
    child.wait().unwrap();
}

/// Test external sources via the high-level NgspiceEngine.
#[test]
fn external_source_via_engine() {
    let exe = env!("CARGO_BIN_EXE_piperine");
    let engine = piperine_pool::NgspiceEngine::with_exe(exe, 1)
        .expect("failed to create engine");

    let ckt = Circuit::new("Engine External Test")
        .v_external("sensor", "sig", GND)
        .resistor("1", "sig", "out", "1k")
        .resistor("2", "out", GND, "1k");

    let tran = TranAnalysis::new(1e-4, 1e-3);

    let result = engine.run_with_external_sources(&ckt, &tran,
        &|_name: &str, _time: f64| -> f64 { 3.3 }
    ).expect("external source simulation failed");

    assert!(!result.plots.is_empty());
    println!("Engine external: {} plots", result.plots.len());
}

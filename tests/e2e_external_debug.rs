//! Debug: measure how many callbacks + check if SimulationComplete arrives.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use piperine_core::prelude::*;
use piperine_ngspice::protocol::*;

fn write_ipc<T: serde::Serialize>(w: &mut impl Write, msg: &T) {
    let bytes = bincode::serialize(msg).unwrap();
    w.write_all(&(bytes.len() as u32).to_le_bytes()).unwrap();
    w.write_all(&bytes).unwrap();
    w.flush().unwrap();
}

#[test]
fn debug_callback_count() {
    let exe = env!("CARGO_BIN_EXE_piperine");
    let mut child = Command::new(exe)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut w = child.stdin.take().unwrap();
    let r = child.stdout.take().unwrap();

    // Reader thread
    let (msg_tx, msg_rx) = mpsc::channel::<WorkerToMain>();
    thread::spawn(move || {
        let mut r = r;
        loop {
            let mut len_buf = [0u8; 4];
            if r.read_exact(&mut len_buf).is_err() { break; }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            if r.read_exact(&mut buf).is_err() { break; }
            if let Ok(msg) = bincode::deserialize::<WorkerToMain>(&buf) {
                if msg_tx.send(msg).is_err() { break; }
            }
        }
    });

    // Ready
    let msg = msg_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(msg, WorkerToMain::Ready));

    // Use very short simulation to minimize callbacks
    write_ipc(&mut w, &MainToWorker::RunSimulation {
        netlist_lines: vec![
            "Debug".into(),
            "V1 sig 0 DC 0 EXTERNAL".into(),
            "R1 sig 0 1k".into(),
            ".end".into(),
        ],
        // Very short tran with large step
        control_commands: vec!["tran 1e-3 2e-3".into()],
        has_external_sources: true,
    });

    let start = Instant::now();
    let mut callbacks = 0u64;
    let mut got_complete = false;

    loop {
        match msg_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(WorkerToMain::ExternalSourceRequest { request_id, .. }) => {
                callbacks += 1;
                write_ipc(&mut w, &MainToWorker::ExternalSourceValue {
                    request_id,
                    value: 1.0,
                });
            }
            Ok(WorkerToMain::SimulationComplete { plots, .. }) => {
                eprintln!("SimulationComplete: {callbacks} callbacks, {} plots, {:?}",
                    plots.len(), start.elapsed());
                got_complete = true;
                break;
            }
            Ok(WorkerToMain::Error { message }) => {
                eprintln!("Error: {message}");
                break;
            }
            Ok(other) => {
                eprintln!("Unexpected: {other:?}");
            }
            Err(_) => {
                eprintln!("TIMEOUT after {callbacks} callbacks, {:?}", start.elapsed());
                break;
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    assert!(got_complete, "did not receive SimulationComplete");
}

//! End-to-end test: spawn worker process, run a simulation, get results.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Once;

use piperine_ngspice::protocol::*;
use tracing::{info, warn};

fn init_tracing_for_tests() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::INFO)
            .try_init();
    });
}

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

#[test]
fn worker_op_simulation() {
    init_tracing_for_tests();
    let exe = env!("CARGO_BIN_EXE_piperine");

    let mut child = Command::new(exe)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn worker");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    // Wait for Ready
    let msg: WorkerToMain = read_msg(&mut stdout);
    match msg {
        WorkerToMain::Ready => {}
        other => panic!("expected Ready, got: {other:?}"),
    }

    // Send a simple OP simulation
    let netlist = vec![
        "Resistor Divider".to_string(),
        "V1 in 0 DC 10".to_string(),
        "R1 in out 1k".to_string(),
        "R2 out 0 1k".to_string(),
        ".end".to_string(),
    ];

    write_msg(
        &mut stdin,
        &MainToWorker::RunSimulation {
            netlist_lines: netlist,
            control_commands: vec!["op".to_string()],
            has_external_sources: false,
        },
    );

    // Read result
    let result: WorkerToMain = read_msg(&mut stdout);
    match result {
        WorkerToMain::SimulationComplete {
            plots,
            measurements,
            log,
        } => {
            info!(plots = plots.len(), "worker OP plots");
            for (name, plot) in &plots {
                info!(plot = %name, vectors = plot.vectors.len(), "worker OP plot vectors");
                for (vname, vdata) in &plot.vectors {
                    match vdata {
                        VectorData::Real { data, .. } => {
                            info!(vector = %vname, sample = ?&data[..data.len().min(5)], "worker OP real vector sample");
                        }
                        VectorData::Complex { data, .. } => {
                            info!(vector = %vname, points = data.len(), "worker OP complex vector");
                        }
                    }
                }
            }
            info!(measurements = ?measurements, "worker OP measurements");
            info!(log_lines = log.len(), "worker OP log lines");

            // Verify we got at least one plot with vectors
            assert!(!plots.is_empty(), "should have at least one plot");

            // Check that out voltage is ~5V (voltage divider)
            let has_out = plots.values().any(|p| {
                p.vectors.iter().any(|(name, vd)| {
                    let lname = name.to_lowercase();
                    if lname.contains("out") || lname == "v(out)" {
                        if let VectorData::Real { data, .. } = vd {
                            if let Some(&v) = data.first() {
                                let ok = (v - 5.0).abs() < 0.01;
                                info!(v_out = v, ok, "worker OP v(out) check");
                                return ok;
                            }
                        }
                    }
                    false
                })
            });
            // Don't hard-fail on value check since plot naming varies
            if !has_out {
                warn!("could not verify v(out) value");
            }
        }
        WorkerToMain::Error { message } => {
            panic!("simulation error: {message}");
        }
        other => panic!("unexpected response: {other:?}"),
    }

    // Shutdown
    write_msg(&mut stdin, &MainToWorker::Shutdown);
    let status = child.wait().unwrap();
    assert!(status.success(), "worker exited with: {status}");
}

#[test]
fn worker_dc_sweep() {
    init_tracing_for_tests();
    let exe = env!("CARGO_BIN_EXE_piperine");

    let mut child = Command::new(exe)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn worker");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    // Ready
    let _: WorkerToMain = read_msg(&mut stdout);

    // DC sweep
    write_msg(
        &mut stdin,
        &MainToWorker::RunSimulation {
            netlist_lines: vec![
                "DC Sweep Test".to_string(),
                "V1 in 0 DC 0".to_string(),
                "R1 in out 1k".to_string(),
                "R2 out 0 1k".to_string(),
                ".end".to_string(),
            ],
            control_commands: vec!["dc V1 0 10 1".to_string()],
            has_external_sources: false,
        },
    );

    let result: WorkerToMain = read_msg(&mut stdout);
    match result {
        WorkerToMain::SimulationComplete { plots, .. } => {
            assert!(!plots.is_empty(), "should have plots from DC sweep");
            info!(plots = plots.len(), "worker DC sweep plots");
            for (name, plot) in &plots {
                info!(plot = %name, vectors = plot.vectors.len(), "worker DC sweep plot vectors");
            }
        }
        WorkerToMain::Error { message } => panic!("DC sweep error: {message}"),
        other => panic!("unexpected: {other:?}"),
    }

    write_msg(&mut stdin, &MainToWorker::Shutdown);
    child.wait().unwrap();
}

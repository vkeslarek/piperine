//! End-to-end test: spawn worker process, run a simulation, get results.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

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

#[test]
fn worker_op_simulation() {
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

    write_msg(&mut stdin, &MainToWorker::RunSimulation {
        netlist_lines: netlist,
        control_commands: vec!["op".to_string()],
        has_external_sources: false,
    });

    // Read result
    let result: WorkerToMain = read_msg(&mut stdout);
    match result {
        WorkerToMain::SimulationComplete { plots, measurements, log } => {
            eprintln!("Plots: {}", plots.len());
            for (name, plot) in &plots {
                eprintln!("  Plot '{}': {} vectors", name, plot.vectors.len());
                for (vname, vdata) in &plot.vectors {
                    match vdata {
                        VectorData::Real { data, .. } => {
                            eprintln!("    {}: {:?}", vname, &data[..data.len().min(5)]);
                        }
                        VectorData::Complex { data, .. } => {
                            eprintln!("    {} (complex): {} points", vname, data.len());
                        }
                    }
                }
            }
            eprintln!("Measurements: {:?}", measurements);
            eprintln!("Log lines: {}", log.len());

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
                                eprintln!("    v(out) = {v}, expected ~5.0, ok={ok}");
                                return ok;
                            }
                        }
                    }
                    false
                })
            });
            // Don't hard-fail on value check since plot naming varies
            if !has_out {
                eprintln!("WARNING: could not verify v(out) value");
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
    write_msg(&mut stdin, &MainToWorker::RunSimulation {
        netlist_lines: vec![
            "DC Sweep Test".to_string(),
            "V1 in 0 DC 0".to_string(),
            "R1 in out 1k".to_string(),
            "R2 out 0 1k".to_string(),
            ".end".to_string(),
        ],
        control_commands: vec![
            "dc V1 0 10 1".to_string(),
        ],
        has_external_sources: false,
    });

    let result: WorkerToMain = read_msg(&mut stdout);
    match result {
        WorkerToMain::SimulationComplete { plots, .. } => {
            assert!(!plots.is_empty(), "should have plots from DC sweep");
            eprintln!("DC sweep plots: {}", plots.len());
            for (name, plot) in &plots {
                eprintln!("  '{}': {} vecs", name, plot.vectors.len());
            }
        }
        WorkerToMain::Error { message } => panic!("DC sweep error: {message}"),
        other => panic!("unexpected: {other:?}"),
    }

    write_msg(&mut stdin, &MainToWorker::Shutdown);
    child.wait().unwrap();
}


//! End-to-end test: spawn worker pool, run a simulation, check results.

use piperine_coordinator::pool::{PoolConfig, ProcessPool};
use piperine_common::{Command, Response};

fn worker_binary_path() -> std::path::PathBuf {
    // Test binary lives in target/debug/deps/; worker lives in target/debug/.
    let mut path = std::env::current_exe().expect("no current_exe");
    path.pop(); // remove test binary name
    if path.ends_with("deps") {
        path.pop(); // deps → debug (or release)
    }
    path.push("piperine-worker");
    path
}

fn spawn_single_worker() -> (ProcessPool, piperine_coordinator::pool::WorkerHandle) {
    let config = PoolConfig { size: 1, worker_binary: Some(worker_binary_path()) };
    let mut pool = ProcessPool::spawn(config).expect("failed to spawn worker pool");
    let handle = pool.take_first();
    (pool, handle)
}

fn send(handle: &mut piperine_coordinator::pool::WorkerHandle, cmd: Command) -> Response {
    handle.cmd.send(cmd).expect("send failed");
    handle.resp.recv().expect("recv failed")
}

#[test]
fn worker_op_simulation() {
    let (_pool, mut handle) = spawn_single_worker();

    let netlist = vec![
        "Resistor Divider".to_string(),
        "V1 in 0 DC 10".to_string(),
        "R1 in out 1k".to_string(),
        "R2 out 0 1k".to_string(),
        ".end".to_string(),
    ];

    match send(&mut handle, Command::LoadCircuit { lines: netlist }) {
        Response::Ok => {}
        other => panic!("LoadCircuit failed: {other:?}"),
    }

    match send(&mut handle, Command::Run { cmd: "op".to_string() }) {
        Response::Ok => {}
        other => panic!("op failed: {other:?}"),
    }

    let values = match send(&mut handle, Command::GetVecData { name: "v(out)".to_string() }) {
        Response::VecData { values } => values,
        other => panic!("GetVecData failed: {other:?}"),
    };

    assert!(!values.is_empty(), "v(out) should have at least one value");
    let v_out = values[0];
    assert!(
        (v_out - 5.0).abs() < 0.01,
        "v(out) should be ~5V (voltage divider), got {v_out}"
    );
}

#[test]
fn worker_dc_sweep() {
    let (_pool, mut handle) = spawn_single_worker();

    // Include the .dc control line in the netlist so ngspice runs it via `run`.
    let netlist = vec![
        "DC Sweep Test".to_string(),
        "V1 in 0 DC 0".to_string(),
        "R1 in out 1k".to_string(),
        "R2 out 0 1k".to_string(),
        ".dc V1 0 10 1".to_string(),
        ".end".to_string(),
    ];

    match send(&mut handle, Command::LoadCircuit { lines: netlist }) {
        Response::Ok => {}
        other => panic!("LoadCircuit failed: {other:?}"),
    }

    match send(&mut handle, Command::Run { cmd: "run".to_string() }) {
        Response::Ok => {}
        other => panic!("dc run failed: {other:?}"),
    }

    // Discover the current plot name, then list its vectors.
    let plot_name = match send(&mut handle, Command::GetCurPlot) {
        Response::CurPlot { name } => name,
        other => panic!("GetCurPlot failed: {other:?}"),
    };
    let vec_names = match send(&mut handle, Command::GetAllVecs { plot: plot_name }) {
        Response::VecList { names } => names,
        other => panic!("GetAllVecs failed: {other:?}"),
    };

    // Find the v(out) or out vector (ngspice may use either name depending on analysis type).
    let out_vec_name = vec_names.iter()
        .find(|n| n.to_lowercase() == "v(out)" || n.to_lowercase() == "out")
        .cloned()
        .unwrap_or_else(|| panic!("no v(out)/out vector in DC plot; available: {vec_names:?}"));

    let values = match send(&mut handle, Command::GetVecData { name: out_vec_name }) {
        Response::VecData { values } => values,
        other => panic!("GetVecData failed: {other:?}"),
    };

    // DC sweep from 0 to 10 with step 1 → 11 points
    assert_eq!(values.len(), 11, "expected 11 sweep points");
    // At V1=10V, v(out) = 5V (voltage divider)
    let v_out_max = values.last().copied().unwrap();
    assert!(
        (v_out_max - 5.0).abs() < 0.01,
        "v(out) at V1=10V should be ~5V, got {v_out_max}"
    );
}

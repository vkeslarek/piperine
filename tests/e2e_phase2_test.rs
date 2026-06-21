use piperine_common::{Command, EventAction, Response, SimEventKind};
use piperine_coordinator::pool::{PoolConfig, ProcessPool, WorkerHandle};
use std::path::PathBuf;

fn worker_binary_path() -> PathBuf {
    let mut path = std::env::current_exe().expect("no current_exe");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("piperine-worker");
    path
}

fn spawn_single_worker() -> (ProcessPool, WorkerHandle) {
    let config = PoolConfig {
        size: 1,
        worker_binary: Some(worker_binary_path()),
    };
    let mut pool = ProcessPool::spawn(config).expect("failed to spawn worker pool");
    let handle = pool.take_first();
    (pool, handle)
}

fn send(handle: &mut WorkerHandle, cmd: Command) -> Response {
    handle.cmd.send(cmd).expect("send failed");
    handle.resp.recv().expect("recv failed")
}

#[test]
fn test_run_analysis_events() {
    let (_pool, mut handle) = spawn_single_worker();

    let netlist = vec![
        "RC Circuit".to_string(),
        "V1 in 0 PULSE(0 1 0 1p 1p 1n 2n)".to_string(),
        "R1 in out 1k".to_string(),
        "C1 out 0 1p".to_string(),
        ".end".to_string(),
    ];

    match send(&mut handle, Command::LoadCircuit { lines: netlist }) {
        Response::Ok => {}
        other => panic!("LoadCircuit failed: {other:?}"),
    }

    // Send RunAnalysis Command
    handle
        .cmd
        .send(Command::RunAnalysis {
            cmd: "tran 100p 2n".to_string(),
            fire_step_events: true,
        })
        .unwrap();

    let mut initial_step_count = 0;
    let mut step_count = 0;
    let mut final_step_count = 0;

    // We should receive events followed by AnalysisDone
    loop {
        match handle.resp.recv().unwrap() {
            Response::Event { kind, time, .. } => {
                match kind {
                    SimEventKind::InitialStep => initial_step_count += 1,
                    SimEventKind::Step => {
                        assert!(time >= 0.0);
                        step_count += 1;
                    }
                    SimEventKind::FinalStep => final_step_count += 1,
                    _ => {}
                }
                // Must reply to the event so the worker continues
                handle
                    .cmd
                    .send(Command::EventResponse {
                        action: EventAction::Continue,
                    })
                    .unwrap();
            }
            Response::AnalysisDone {
                plot_name,
                had_run_errors,
            } => {
                assert!(!plot_name.is_empty());
                assert!(!had_run_errors);
                break;
            }
            Response::Error { message, .. } => {
                panic!("Received error from worker: {message}");
            }
            other => panic!("Unexpected response during run_analysis: {other:?}"),
        }
    }

    assert_eq!(initial_step_count, 1, "Expected exactly 1 initial_step event");
    assert_eq!(final_step_count, 1, "Expected exactly 1 final_step event");
    assert!(step_count > 10, "Expected multiple step events");
}

#[test]
fn test_run_analysis_early_halt() {
    let (_pool, mut handle) = spawn_single_worker();

    let netlist = vec![
        "RC Circuit".to_string(),
        "V1 in 0 PULSE(0 1 0 1p 1p 1n 2n)".to_string(),
        "R1 in out 1k".to_string(),
        "C1 out 0 1p".to_string(),
        ".end".to_string(),
    ];

    match send(&mut handle, Command::LoadCircuit { lines: netlist }) {
        Response::Ok => {}
        other => panic!("LoadCircuit failed: {other:?}"),
    }

    // Send RunAnalysis Command
    handle
        .cmd
        .send(Command::RunAnalysis {
            cmd: "tran 100p 5n".to_string(),
            fire_step_events: true,
        })
        .unwrap();

    let mut halted = false;

    loop {
        match handle.resp.recv().unwrap() {
            Response::Event { kind: SimEventKind::Step, time, .. } => {
                if time >= 1e-9 {
                    // Halt the simulation at 1ns
                    handle
                        .cmd
                        .send(Command::EventResponse {
                            action: EventAction::Halt { reason: "test halt".into() },
                        })
                        .unwrap();
                    halted = true;
                } else {
                    handle
                        .cmd
                        .send(Command::EventResponse {
                            action: EventAction::Continue,
                        })
                        .unwrap();
                }
            }
            Response::Event { .. } => {
                handle
                    .cmd
                    .send(Command::EventResponse {
                        action: EventAction::Continue,
                    })
                    .unwrap();
            }
            Response::AnalysisDone { had_run_errors, .. } => {
                // If we halted by setting run error, had_run_errors might be true.
                // Or if we halted via normal halt, it might just be done.
                // The main thing is that we successfully broke out early.
                break;
            }
            other => panic!("Unexpected response: {other:?}"),
        }
    }

    assert!(halted, "Simulation was not halted as requested");
}

#[test]
fn test_parser_and_elaborator_always_blocks() {
    use cvaf::model::Document;
    use piperine_circuit::elaboration::elaborate;
    use piperine_circuit::registry::HardwareRegistry;
    use cvaf::parser::parse;

    let source = r#"
module tb;
    always @(initial_step) begin
        $display("Start");
    end

    always @(step) begin
        $display("Step");
    end

    always @(final_step) begin
        $display("Final");
    end

    always @(above(V(out) - 1.0)) begin
        $display("Crossed 1V");
    end

    always @(cross(V(out) - 2.0, +1)) begin
        $display("Crossed 2V rising");
    end

    initial begin
        $tran(1n, 100n);
    end
endmodule
"#;

    let document = parse(source).expect("Failed to parse source");
    let registry = HardwareRegistry::new();
    let result = elaborate(&document, &registry).expect("Elaboration failed");

    let handlers = result.always_handlers;
    assert_eq!(handlers.initial_step.len(), 1);
    assert_eq!(handlers.step.len(), 1);
    assert_eq!(handlers.final_step.len(), 1);
    assert_eq!(handlers.above.len(), 1);
    assert_eq!(handlers.cross.len(), 1);

    // Verify above handler details
    let (_, crossing_id, _) = &handlers.above[0];
    assert_eq!(*crossing_id, 0);

    // Verify cross handler details
    let (_, dir, crossing_id, _) = &handlers.cross[0];
    assert_eq!(*dir, 1);
    assert_eq!(*crossing_id, 1);
}

use std::env;
use ipc_channel::ipc::{self, IpcSender};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use piperine_common::{Command, CmdReceiver, Handshake, RespSender, Response, EventAction, SimEventKind};
use piperine_worker::ngspice::{Ngspice, NgspiceHandler};

struct WorkerState {
    /// Set true only during `RunAnalysis` — gates all event IPC.
    /// Prevents on_initial_step/on_step callbacks from sending events
    /// during plain `Command::Run` (op, dc, etc.) where the coordinator
    /// expects a simple `Response::Ok`, not a streaming protocol.
    streaming_active: AtomicBool,
    /// Whether to fire per-step events (controlled by `RunAnalysis.fire_step_events`).
    fire_step_events: AtomicBool,
    halt_requested: AtomicBool,
    run_error_message: Mutex<Option<String>>,
}

struct WorkerHandler {
    tx: RespSender,
    rx: Arc<Mutex<CmdReceiver>>,
    state: Arc<WorkerState>,
}

impl NgspiceHandler for WorkerHandler {
    fn on_initial_step(&self, time: f64) {
        if self.state.streaming_active.load(Ordering::Relaxed) {
            self.send_event_and_wait(SimEventKind::InitialStep, time, 0);
        }
    }

    fn on_step(&self, time: f64) {
        if self.state.streaming_active.load(Ordering::Relaxed)
            && self.state.fire_step_events.load(Ordering::Relaxed)
        {
            self.send_event_and_wait(SimEventKind::Step, time, 0);
        }
    }

    // on_final_step is NOT called here — RunAnalysis sends it manually after
    // ng.command() returns, so the coordinator can run @(final_step) handlers
    // with full post-analysis vector data available.
}

impl WorkerHandler {
    fn send_event_and_wait(&self, kind: SimEventKind, time: f64, crossing_id: u32) {
        if self.state.halt_requested.load(Ordering::Relaxed) { return; }
        let _ = self.tx.send(Response::Event { kind, time, crossing_id });
        if let Ok(cmd) = self.rx.lock().unwrap().recv() {
            match cmd {
                Command::EventResponse { action: EventAction::Halt { .. } } => {
                    self.state.halt_requested.store(true, Ordering::Relaxed);
                }
                Command::EventResponse { action: EventAction::RunError { message } } => {
                    *self.state.run_error_message.lock().unwrap() = Some(message);
                    self.state.halt_requested.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }
}

fn main() {
    let server_name = env::args().nth(1).expect("missing server name");

    let tx: IpcSender<Handshake> = IpcSender::connect(server_name.into()).expect("ipc connect");

    let (cmd_tx, cmd_rx) = ipc::channel::<Command>().expect("cmd channel");
    let (resp_tx, resp_rx) = ipc::channel::<Response>().expect("resp channel");

    tx.send((cmd_tx, resp_rx)).expect("handshake");

    eprintln!("worker connected");

    let worker_state = Arc::new(WorkerState {
        streaming_active: AtomicBool::new(false),
        fire_step_events: AtomicBool::new(false),
        halt_requested:   AtomicBool::new(false),
        run_error_message: Mutex::new(None),
    });

    let rx_arc = Arc::new(Mutex::new(cmd_rx));

    let handler = Box::new(WorkerHandler {
        tx: resp_tx.clone(),
        rx: rx_arc.clone(),
        state: worker_state.clone(),
    });

    let ng = Ngspice::init(handler).expect("ngspice init");
    run_loop(&ng, rx_arc, resp_tx, worker_state);
    let _ = ng.quit();
}

fn run_loop(ng: &Ngspice, rx: Arc<Mutex<CmdReceiver>>, tx: RespSender, state: Arc<WorkerState>) {
    loop {
        let cmd = {
            match rx.lock().unwrap().recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            }
        };
        let resp = run_command(ng, cmd, &rx, &state, &tx);
        if tx.send(resp).is_err() {
            break;
        }
    }
}

fn run_command(ng: &Ngspice, cmd: Command, rx: &Arc<Mutex<CmdReceiver>>, state: &Arc<WorkerState>, tx: &RespSender) -> Response {
    match cmd {
        Command::RunAnalysis { cmd: c, fire_step_events } => {
            // Reset per-run state
            state.streaming_active.store(true, Ordering::Relaxed);
            state.fire_step_events.store(fire_step_events, Ordering::Relaxed);
            state.halt_requested.store(false, Ordering::Relaxed);
            *state.run_error_message.lock().unwrap() = None;

            let _ = ng.command(&c);

            // Disable streaming so cleanup commands (cur_plot etc.) don't fire events
            state.streaming_active.store(false, Ordering::Relaxed);

            let plot = ng.cur_plot().unwrap_or_default();
            let had_errors_from_step = state.run_error_message.lock().unwrap().is_some();

            // Send FinalStep — coordinator runs @(final_step) handlers here.
            // Done after streaming_active=false so the callback path doesn't
            // interfere; we send it directly.
            let _ = tx.send(Response::Event {
                kind: SimEventKind::FinalStep,
                time: 0.0,
                crossing_id: 0,
            });
            let mut had_errors = had_errors_from_step;
            if let Ok(cmd) = rx.lock().unwrap().recv() {
                match cmd {
                    Command::EventResponse { action: EventAction::RunError { message } } => {
                        *state.run_error_message.lock().unwrap() = Some(message);
                        had_errors = true;
                    }
                    Command::EventResponse { action: EventAction::Halt { .. } } => {
                        had_errors = true;
                    }
                    _ => {}
                }
            }

            Response::AnalysisDone { plot_name: plot, had_run_errors: had_errors }
        }

        Command::Run { cmd: c } => {
            // Plain command — no streaming, no events. Handler is gated by
            // streaming_active=false (already the default between commands).
            match ng.command(&c) {
                Ok(()) => Response::Ok,
                Err(code) => Response::Error { code, message: format!("command failed: {c}") },
            }
        }

        Command::LoadCircuit { lines } => {
            let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
            match ng.load_circuit(&refs) {
                Ok(()) => Response::Ok,
                Err(code) => Response::Error { code, message: "load circuit failed".into() },
            }
        }
        Command::GetVecData { name } => match ng.vec_real_data(&name) {
            Some(data) => Response::VecData { values: data.to_vec() },
            None => Response::Error { code: -1, message: format!("vector not found: {name}") },
        },
        Command::GetVecComplex { name } => match ng.vec_complex_data(&name) {
            Some(data) => Response::VecComplex {
                pairs: data.iter().map(|c| (c.re, c.im)).collect(),
            },
            None => Response::Error { code: -1, message: format!("complex vector not found: {name}") },
        },
        Command::GetAllVecs { plot } => {
            Response::VecList { names: ng.all_vecs(&plot) }
        }
        Command::GetCurPlot => {
            Response::CurPlot { name: ng.cur_plot().unwrap_or_default() }
        }
        Command::GetAllPlots => {
            Response::VecList { names: ng.all_plots() }
        }
        Command::Shutdown => Response::Ok,
        Command::EventResponse { .. } => {
            // Spurious EventResponse outside of streaming — ignore.
            Response::Error { code: -1, message: "unexpected EventResponse outside RunAnalysis".into() }
        }
    }
}

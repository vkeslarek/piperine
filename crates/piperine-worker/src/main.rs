use std::env;
use ipc_channel::ipc::{self, IpcSender};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use piperine_common::{Command, CmdReceiver, Handshake, RespSender, Response, EventAction, SimEventKind};
use piperine_worker::ngspice::{Ngspice, NgspiceHandler};

/// Shared flag: coordinator told us to halt the current run.
struct WorkerState {
    halt_requested: AtomicBool,
    run_error_message: Mutex<Option<String>>,
}

struct WorkerHandler {
    tx: RespSender,
    rx: Arc<Mutex<CmdReceiver>>,   // mutable recv, lock per callback
    state: Arc<WorkerState>,
    fire_step: bool,
}

impl NgspiceHandler for WorkerHandler {
    fn on_initial_step(&self, time: f64) {
        self.send_event_and_wait(SimEventKind::InitialStep, time, 0);
    }

    fn on_step(&self, time: f64) {
        if self.fire_step {
            self.send_event_and_wait(SimEventKind::Step, time, 0);
        }
    }

    fn on_final_step(&self, time: f64) {
        self.send_event_and_wait(SimEventKind::FinalStep, time, 0);
    }
}

impl WorkerHandler {
    fn send_event_and_wait(&self, kind: SimEventKind, time: f64, crossing_id: u32) {
        if self.state.halt_requested.load(Ordering::Relaxed) { return; }
        let _ = self.tx.send(Response::Event { kind, time, crossing_id });
        // Block until coordinator responds
        if let Ok(cmd) = self.rx.lock().unwrap().recv() {
            match cmd {
                Command::EventResponse { action: EventAction::Halt { reason: _ } } => {
                    self.state.halt_requested.store(true, Ordering::Relaxed);
                    // ngSpice_Command("halt") will stop the current analysis
                    // (called from run_command after the step callback returns)
                }
                Command::EventResponse { action: EventAction::RunError { message } } => {
                    *self.state.run_error_message.lock().unwrap() = Some(message);
                    self.state.halt_requested.store(true, Ordering::Relaxed);
                }
                _ => {}  // Continue
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
        halt_requested: AtomicBool::new(false),
        run_error_message: Mutex::new(None),
    });

    let rx_arc = Arc::new(Mutex::new(cmd_rx));

    let handler = Box::new(WorkerHandler {
        tx: resp_tx.clone(),
        rx: rx_arc.clone(),
        state: worker_state.clone(),
        fire_step: true, // we will recreate handler per run in the future, for now true
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
        let resp = run_command(ng, cmd, &state, &tx);
        if tx.send(resp).is_err() {
            break;
        }
    }
}

fn run_command(ng: &Ngspice, cmd: Command, state: &Arc<WorkerState>, tx: &RespSender) -> Response {
    match cmd {
        Command::RunAnalysis { cmd: c, fire_step_events: _ } => {
            state.halt_requested.store(false, Ordering::Relaxed);
            *state.run_error_message.lock().unwrap() = None;
            let _ = ng.command(&c);
            let plot = ng.cur_plot().unwrap_or_default();
            let had_errors = state.run_error_message.lock().unwrap().is_some();
            Response::AnalysisDone { plot_name: plot, had_run_errors: had_errors }
        }
        Command::Run { cmd: c } => match ng.command(&c) {
            Ok(()) => Response::Ok,
            Err(code) => Response::Error { code, message: format!("command failed: {c}") },
        },
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
            Response::Error { code: -1, message: "Unexpected EventResponse".into() }
        }
    }
}

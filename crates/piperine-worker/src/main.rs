use std::env;

use ipc_channel::ipc::{self, IpcSender};
use piperine_common::{Command, CmdReceiver, Handshake, RespSender, Response};
use piperine_worker::ngspice::{Ngspice, NgspiceHandler};

struct WorkerHandler;
impl NgspiceHandler for WorkerHandler {}

fn main() {
    let server_name = env::args().nth(1).expect("missing server name");

    let tx: IpcSender<Handshake> = IpcSender::connect(server_name.into()).expect("ipc connect");

    let (cmd_tx, cmd_rx) = ipc::channel::<Command>().expect("cmd channel");
    let (resp_tx, resp_rx) = ipc::channel::<Response>().expect("resp channel");

    tx.send((cmd_tx, resp_rx)).expect("handshake");

    eprintln!("worker connected");

    let ng = Ngspice::init(Box::new(WorkerHandler)).expect("ngspice init");
    run_loop(&ng, cmd_rx, resp_tx);
    let _ = ng.quit();
}

fn run_loop(ng: &Ngspice, rx: CmdReceiver, tx: RespSender) {
    loop {
        let cmd = match rx.recv() {
            Ok(cmd) => cmd,
            Err(_) => break,
        };
        let resp = run_command(ng, cmd);
        if tx.send(resp).is_err() {
            break;
        }
    }
}

fn run_command(ng: &Ngspice, cmd: Command) -> Response {
    match cmd {
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
    }
}

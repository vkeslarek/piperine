pub mod spice;

use ipc_channel::ipc::{IpcReceiver, IpcSender};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Run { cmd: String },
    LoadCircuit { lines: Vec<String> },
    GetVecData { name: String },
    GetAllVecs { plot: String },
    GetCurPlot,
    GetAllPlots,
    Shutdown,

    // ── NEW ──
    /// Start an analysis that will stream Event responses.
    /// Worker fires Response::Event for each @(step) etc., then Response::AnalysisDone.
    RunAnalysis { cmd: String, fire_step_events: bool },

    /// Coordinator's response to a Response::Event from the worker.
    /// Sent synchronously — worker blocks on this before continuing the callback.
    EventResponse { action: EventAction },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EventAction {
    Continue,
    Halt { reason: String },
    RunError { message: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Ok,
    Error { code: i32, message: String },
    VecData { values: Vec<f64> },
    VecList { names: Vec<String> },
    CurPlot { name: String },

    // ── NEW ──
    /// Worker fires this for each @(step)/@(initial_step)/@(final_step)/above() crossing.
    /// Coordinator must reply with Command::EventResponse before worker continues.
    Event { kind: SimEventKind, time: f64, crossing_id: u32 },

    /// Analysis complete. All vectors now readable via GetVecData.
    AnalysisDone { plot_name: String, had_run_errors: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SimEventKind {
    InitialStep,
    Step,
    FinalStep,
    AboveCrossing,   // above(expr) threshold crossed positive→negative
}

pub type CmdSender = IpcSender<Command>;
pub type CmdReceiver = IpcReceiver<Command>;
pub type RespSender = IpcSender<Response>;
pub type RespReceiver = IpcReceiver<Response>;

pub type Handshake = (CmdSender, RespReceiver);

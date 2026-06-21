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
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Ok,
    Error { code: i32, message: String },
    VecData { values: Vec<f64> },
    VecList { names: Vec<String> },
    CurPlot { name: String },
}

pub type CmdSender = IpcSender<Command>;
pub type CmdReceiver = IpcReceiver<Command>;
pub type RespSender = IpcSender<Response>;
pub type RespReceiver = IpcReceiver<Response>;

pub type Handshake = (CmdSender, RespReceiver);

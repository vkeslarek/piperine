use piperine_common::{Command, Response, CmdSender, RespReceiver};
use piperine_interpreter::{SimulatorBackend, InterpreterError};

/// A `SimulatorBackend` backed by a process-isolated ngspice worker.
/// Communicates via IPC channels established by `piperine-coordinator`.
pub struct NgspiceBackend {
    command_sender:   CmdSender,
    response_receiver: RespReceiver,
}

impl NgspiceBackend {
    pub fn new(command_sender: CmdSender, response_receiver: RespReceiver) -> Self {
        Self { command_sender, response_receiver }
    }

    fn send(&mut self, command: Command) -> Result<Response, InterpreterError> {
        self.command_sender
            .send(command)
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))?;
        self.response_receiver
            .recv()
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))
    }
}

impl SimulatorBackend for NgspiceBackend {
    fn load_circuit(&mut self, lines: &[String]) -> Result<(), InterpreterError> {
        match self.send(Command::LoadCircuit { lines: lines.to_vec() })? {
            Response::Ok => Ok(()),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }

    fn run_command(&mut self, command: &str) -> Result<(), InterpreterError> {
        match self.send(Command::Run { cmd: command.to_string() })? {
            Response::Ok => Ok(()),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }

    fn get_vector(&mut self, name: &str) -> Result<Vec<f64>, InterpreterError> {
        match self.send(Command::GetVecData { name: name.to_string() })? {
            Response::VecData { values } => Ok(values),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }
}

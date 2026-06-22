use piperine_common::{Command, Response, CmdSender, RespReceiver, EventAction};
use piperine_interpreter::{SimulatorBackend, InterpreterError, AnalysisEvent};

/// A `SimulatorBackend` backed by a process-isolated ngspice worker.
/// Communicates via IPC channels established by `piperine-coordinator`.
pub struct NgspiceBackend {
    command_sender:    CmdSender,
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
        self.recv()
    }

    fn recv(&mut self) -> Result<Response, InterpreterError> {
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

    fn list_vectors(&mut self, plot_name: &str) -> Result<Vec<String>, InterpreterError> {
        match self.send(Command::GetAllVecs { plot: plot_name.to_string() })? {
            Response::VecList { names } => Ok(names),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }

    fn start_analysis(&mut self, cmd: &str, fire_step: bool) -> Result<(), InterpreterError> {
        self.command_sender
            .send(Command::RunAnalysis { cmd: cmd.to_string(), fire_step_events: fire_step })
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))
        // No response yet — caller drives the loop via poll_analysis.
    }

    fn poll_analysis(&mut self) -> Result<AnalysisEvent, InterpreterError> {
        loop {
            match self.recv()? {
                Response::Event { kind, time, crossing_id } => {
                    return Ok(AnalysisEvent::Event { kind, time, crossing_id });
                }
                Response::AnalysisDone { plot_name, had_run_errors } => {
                    return Ok(AnalysisEvent::Done { plot_name, had_run_errors });
                }
                Response::Error { message, .. } => {
                    return Err(InterpreterError::SimulatorError(message));
                }
                _ => {
                    // Ignore unexpected responses (e.g., stale Ok from prior command).
                }
            }
        }
    }

    fn respond_to_analysis_event(&mut self, action: EventAction) -> Result<(), InterpreterError> {
        self.command_sender
            .send(Command::EventResponse { action })
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))
    }
}

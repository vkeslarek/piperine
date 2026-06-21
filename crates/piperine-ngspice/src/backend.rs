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
        self.recv()
    }

    fn recv(&mut self) -> Result<Response, InterpreterError> {
        self.response_receiver
            .recv()
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))
    }

    fn get_all_vecs(&mut self, plot: &str) -> Result<Vec<String>, InterpreterError> {
        match self.send(Command::GetAllVecs { plot: plot.to_string() })? {
            Response::VecList { names } => Ok(names),
            Response::Error { message, .. } => Err(InterpreterError::SimulatorError(message)),
            other => Err(InterpreterError::SimulatorError(format!("unexpected response: {other:?}"))),
        }
    }

    fn recv_all_vecs(&mut self, plot: &str) -> Result<std::collections::HashMap<String, piperine_interpreter::value::VectorData>, InterpreterError> {
        let names = self.get_all_vecs(plot)?;
        let mut map = std::collections::HashMap::new();
        for name in names {
            match self.send(Command::GetVecData { name: name.clone() })? {
                Response::VecData { values } => {
                    map.insert(name, piperine_interpreter::value::VectorData::Real(values));
                }
                _ => {}
            }
        }
        Ok(map)
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

    fn run_analysis(
        &mut self,
        cmd: &str,
        handlers: &piperine_circuit::elaboration::AlwaysHandlerSet,
        interp_ctx: &mut dyn piperine_interpreter::InterpreterCallbacks,
        fire_step: bool,
    ) -> Result<piperine_interpreter::value::AnalysisResult, InterpreterError> {
        self.command_sender
            .send(Command::RunAnalysis { cmd: cmd.to_string(), fire_step_events: fire_step })
            .map_err(|e| InterpreterError::SimulatorError(e.to_string()))?;

        let mut had_run_errors = false;
        let mut plot_name = String::new();

        loop {
            match self.recv()? {
                Response::AnalysisDone { plot_name: p, had_run_errors: e } => {
                    plot_name = p;
                    had_run_errors = e;
                    break;
                }
                Response::Event { kind, time, crossing_id } => {
                    let action = interp_ctx.fire_event(kind, time, crossing_id, handlers);
                    self.command_sender
                        .send(Command::EventResponse { action })
                        .map_err(|e| InterpreterError::SimulatorError(e.to_string()))?;
                }
                Response::Error { message, .. } => {
                    return Err(InterpreterError::SimulatorError(message));
                }
                _ => {}
            }
        }

        // Pull all vectors after the run
        let vecs = self.recv_all_vecs(&plot_name)?;
        Ok(piperine_interpreter::value::AnalysisResult {
            kind: piperine_interpreter::value::AnalysisKind::Tran, // Assume Tran for now, actually we should parse cmd or pass kind
            plot_name,
            vectors: vecs,
            run_errors: Vec::new(),
        })
    }
}

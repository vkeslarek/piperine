use piperine_interpreter::{SystemTask, SimulatorBackend, Value, InterpreterError};

// ── $op() ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct OperatingPointTask;

impl SystemTask for OperatingPointTask {
    fn name(&self) -> &str { "op" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if !arguments.is_empty() {
            return Err(InterpreterError::TypeError {
                expected: "0 arguments".into(),
                got: format!("{} arguments", arguments.len()),
            });
        }
        simulator.run_command("op")?;
        Ok(None)
    }
}

// ── $tran(step, stop) ────────────────────────────────────────────────────────
//
// NOTE: ngspice shared library does not execute analysis commands (dc, tran, ac)
// issued via ngSpice_Command at runtime. Analysis type must be declared in the
// netlist as a control line (e.g., `.tran 1n 1u`). $tran() runs the already-
// declared transient analysis by sending `run`.

#[derive(Debug)]
pub struct TransientTask;

impl SystemTask for TransientTask {
    fn name(&self) -> &str { "tran" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if !arguments.is_empty() {
            return Err(InterpreterError::TypeError {
                expected: "0 arguments — transient analysis is declared in the netlist via .tran".into(),
                got: format!("{} arguments", arguments.len()),
            });
        }
        simulator.run_command("run")?;
        Ok(None)
    }
}

// ── $V("node") ───────────────────────────────────────────────────────────────

/// Returns the node voltage after an analysis. Result is a `real`.
#[derive(Debug)]
pub struct VoltageTask;

impl SystemTask for VoltageTask {
    fn name(&self) -> &str { "V" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let node = arguments.first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| InterpreterError::TypeError {
                expected: "string node name".into(),
                got: arguments.first().map(|v| v.type_name()).unwrap_or("nothing").into(),
            })?
            .to_string();
        let vector = simulator.get_vector(&format!("v({node})"))?;
        let last_value = vector.last().copied().ok_or_else(|| {
            InterpreterError::SimulatorError(format!("vector v({node}) is empty after analysis"))
        })?;
        Ok(Some(Value::Real(last_value)))
    }
}

// ── $I("branch") ─────────────────────────────────────────────────────────────

/// Returns the branch current after an analysis. Result is a `real`.
#[derive(Debug)]
pub struct CurrentTask;

impl SystemTask for CurrentTask {
    fn name(&self) -> &str { "I" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let branch = arguments.first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| InterpreterError::TypeError {
                expected: "string branch name (e.g., \"v1\")".into(),
                got: arguments.first().map(|v| v.type_name()).unwrap_or("nothing").into(),
            })?
            .to_string();
        // ngspice exposes voltage-source current as `v1#branch` in the raw vector table.
        // The `i(v1)` alias works in interactive ngspice but not always via the shared-lib API.
        let vector = simulator.get_vector(&format!("i({branch})"))
            .or_else(|_| simulator.get_vector(&format!("{branch}#branch")))?;
        let last_value = vector.last().copied().ok_or_else(|| {
            InterpreterError::SimulatorError(format!("vector i({branch}) is empty after analysis"))
        })?;
        Ok(Some(Value::Real(last_value)))
    }
}

// ── $display(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct DisplayTask;

impl SystemTask for DisplayTask {
    fn name(&self) -> &str { "display" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output = if arguments.is_empty() {
            String::new()
        } else {
            let format_string = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
                expected: "string format".into(),
                got: arguments[0].type_name().into(),
            })?.to_string();
            format_display_string(&format_string, &arguments[1..])
        };
        simulator.print(&output);
        Ok(None)
    }
}

/// Minimal `$display` format string processor.
/// Supported: `%g` `%f` `%d` `%s` `%0d` `%%` and literal text.
fn format_display_string(format: &str, arguments: &[Value]) -> String {
    let mut output = String::new();
    let mut chars = format.chars().peekable();
    let mut argument_index = 0;

    while let Some(character) = chars.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        // Consume optional width digits (e.g., `%0d`)
        while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            chars.next();
        }
        match chars.next() {
            Some('%')      => output.push('%'),
            Some('g') => {
                let value = arguments.get(argument_index).and_then(|v| v.as_f64()).unwrap_or(0.0);
                output.push_str(&format!("{value}"));
                argument_index += 1;
            }
            Some('f') => {
                let value = arguments.get(argument_index).and_then(|v| v.as_f64()).unwrap_or(0.0);
                output.push_str(&format!("{value:.6}"));
                argument_index += 1;
            }
            Some('d') => {
                let value = arguments.get(argument_index).and_then(|v| v.as_integer()).unwrap_or(0);
                output.push_str(&format!("{value}"));
                argument_index += 1;
            }
            Some('s') => {
                let value = arguments.get(argument_index).map(|v| v.to_string()).unwrap_or_default();
                output.push_str(&value);
                argument_index += 1;
            }
            Some(other) => { output.push('%'); output.push(other); }
            None        => { output.push('%'); }
        }
    }
    output
}

// ── $run_error(fmt, args...) ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct RunErrorTask;

impl SystemTask for RunErrorTask {
    fn name(&self) -> &str { "run_error" }

    fn call(
        &self,
        arguments: Vec<Value>,
        _simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError> {
        let msg = if arguments.is_empty() {
            "run failed".into()
        } else {
            let format_string = arguments[0].as_str().unwrap_or_default();
            format_display_string(&format_string, &arguments[1..])
        };
        Err(InterpreterError::RunFailed { message: msg })
    }
}

// ── $fatal([exit_code,] fmt, args...) ────────────────────────────────────────

#[derive(Debug)]
pub struct FatalTask;

impl SystemTask for FatalTask {
    fn name(&self) -> &str { "fatal" }

    fn call(
        &self,
        arguments: Vec<Value>,
        _simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError> {
        let mut exit_code = 1;
        let mut fmt_idx = 0;
        
        if !arguments.is_empty() && matches!(arguments[0], Value::Integer(_)) {
            exit_code = arguments[0].as_integer().unwrap() as u32;
            fmt_idx = 1;
        }

        let msg = if fmt_idx < arguments.len() {
            let format_string = arguments[fmt_idx].as_str().unwrap_or_default();
            format_display_string(&format_string, &arguments[(fmt_idx + 1)..])
        } else {
            "fatal error".into()
        };

        Err(InterpreterError::Fatal { message: msg, exit_code })
    }
}

// ── $warning(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct WarningTask;

impl SystemTask for WarningTask {
    fn name(&self) -> &str { "warning" }

    fn call(
        &self,
        arguments: Vec<Value>,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError> {
        let msg = if arguments.is_empty() {
            "warning".into()
        } else {
            let format_string = arguments[0].as_str().unwrap_or_default();
            format_display_string(&format_string, &arguments[1..])
        };
        simulator.print(&format!("WARNING: {}", msg));
        Ok(None)
    }
}

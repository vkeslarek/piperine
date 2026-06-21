use crate::Command;

use super::analysis::Analysis;
use super::spice_line::SpiceLine;

#[derive(Debug, Clone)]
pub struct Netlist {
    title: String,
    lines: Vec<String>,
}

impl Netlist {
    pub fn new(title: impl Into<String>) -> Self {
        Netlist { title: title.into(), lines: Vec::new() }
    }

    pub fn push(mut self, line: impl SpiceLine) -> Self {
        self.lines.push(line.spice_line());
        self
    }

    pub fn analyze(mut self, a: Analysis) -> Self {
        self.lines.push(a.deck_line());
        self
    }

    pub fn into_lines(self) -> Vec<String> {
        let mut out = vec![self.title];
        out.extend(self.lines);
        out.push(".end".into());
        out
    }
}

impl From<Netlist> for Command {
    fn from(net: Netlist) -> Command {
        Command::LoadCircuit { lines: net.into_lines() }
    }
}

impl From<Analysis> for Command {
    fn from(a: Analysis) -> Command {
        Command::Run { cmd: a.cmd_line() }
    }
}

//! `Exit`: how a command ends (spec "Exit codes": Cargo's and clap's convention).

/// The outcome of a command, as the process reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The command did what it was asked: 0.
    Done,
    /// The design, the run or a test failed: 1.
    Failed,
    /// The command line is wrong: 2.
    Usage,
}

impl Exit {
    /// The process exit code.
    pub fn code(&self) -> i32 {
        match self {
            Exit::Done => 0,
            Exit::Failed => 1,
            Exit::Usage => 2,
        }
    }
}


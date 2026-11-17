//! Solver error taxonomy: `SolverDomain` + the loud-error types every layer returns.
use thiserror::Error;

/// Where an error originated. Replaces the free string titles (`"DC"`, `"TF"`,
/// `"Noise"`, …) so a typo is a compile error and a tool can route diagnostics
/// by domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverDomain {
    Dc,
    Ac,
    Transient,
    Noise,
    Tf,
    Digital,
    Bridge,
    Newton,
    Linear,
    SpaceMatrix,
    Device,
    Sens,
    Pss,
    Pz,
    Sp,
}

impl SolverDomain {
    pub const fn as_str(self) -> &'static str {
        match self {
            SolverDomain::Dc => "DC",
            SolverDomain::Ac => "AC",
            SolverDomain::Transient => "Transient",
            SolverDomain::Noise => "Noise",
            SolverDomain::Tf => "TF",
            SolverDomain::Digital => "Digital",
            SolverDomain::Bridge => "Bridge",
            SolverDomain::Newton => "Newton",
            SolverDomain::Linear => "Linear",
            SolverDomain::SpaceMatrix => "SpaceMatrix",
            SolverDomain::Device => "Device",
            SolverDomain::Sens => "Sens",
            SolverDomain::Pss => "PSS",
            SolverDomain::Pz => "PZ",
            SolverDomain::Sp => "SP",
        }
    }
}

impl std::fmt::Display for SolverDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The run-failure codes of Part V (§17.1): each names one way a run fails loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCode {
    /// The Jacobian is singular, or the linear solve returned a non-finite value.
    E0503,
    /// A transient step fails at the smallest step where it reads a jump.
    E0504,
    /// An equilibrium solve does not converge after Newton and every homotopy.
    E0505,
    /// A non-finite contribution or stamp.
    E0507,
    /// A transient step fails at the smallest step.
    E0508,
}

impl RunCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            RunCode::E0503 => "E0503",
            RunCode::E0504 => "E0504",
            RunCode::E0505 => "E0505",
            RunCode::E0507 => "E0507",
            RunCode::E0508 => "E0508",
        }
    }
}

impl std::fmt::Display for RunCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A run failure with its spec code: when it happened (`time`), which unknowns
/// it concerns (`unknowns`, matrix indices) and the same unknowns by name
/// (`names`, filled by the owner that knows the pins and branches).
#[derive(Debug, Clone, PartialEq)]
pub struct RunError {
    pub code: RunCode,
    pub domain: SolverDomain,
    pub detail: String,
    pub time: Option<f64>,
    pub unknowns: Vec<usize>,
    pub names: Vec<String>,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}: {}", self.domain, self.code, self.detail)?;
        if let Some(time) = self.time {
            write!(f, " at t = {time:e} s")?;
        }
        if !self.names.is_empty() {
            write!(f, ": {}", self.names.join(", "))
        } else if !self.unknowns.is_empty() {
            let indices: Vec<String> = self.unknowns.iter().map(|u| format!("#{u}")).collect();
            write!(f, ": unknowns {}", indices.join(", "))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Run(RunError),

    #[error("{domain}: {detail}")]
    Simple {
        domain: SolverDomain,
        detail: String,
    },

    #[error("{domain}: {detail}\nCaused by: {cause}")]
    WithCause {
        domain: SolverDomain,
        detail: String,
        #[source]
        cause: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl Error {
    pub fn wrap(domain: SolverDomain, detail: impl Into<String>, cause: Error) -> Error {
        Error::WithCause {
            domain,
            detail: detail.into(),
            cause: Box::new(cause),
        }
    }

    /// A run failure carrying a Part V code.
    pub fn run(code: RunCode, domain: SolverDomain, detail: impl Into<String>) -> Error {
        Error::Run(RunError {
            code,
            domain,
            detail: detail.into(),
            time: None,
            unknowns: Vec::new(),
            names: Vec::new(),
        })
    }

    /// The run code, when this is a run failure.
    pub fn code(&self) -> Option<RunCode> {
        match self {
            Error::Run(run) => Some(run.code),
            _ => None,
        }
    }

    /// Stamp the time of the failure on a run error; other errors pass through.
    pub fn with_time(mut self, time: f64) -> Error {
        if let Error::Run(run) = &mut self {
            run.time = Some(time);
        }
        self
    }

    /// Record the unknowns (matrix indices) a run error concerns.
    pub fn with_unknowns(mut self, unknowns: Vec<usize>) -> Error {
        if let Error::Run(run) = &mut self {
            run.unknowns = unknowns;
        }
        self
    }

    /// Record the names of the unknowns a run error concerns.
    pub fn with_names(mut self, names: Vec<String>) -> Error {
        if let Error::Run(run) = &mut self {
            run.names = names;
        }
        self
    }

    pub fn simple(domain: SolverDomain, description: impl Into<String>) -> Error {
        Error::Simple {
            domain,
            detail: description.into(),
        }
    }

    pub fn cause(
        domain: SolverDomain,
        description: impl Into<String>,
        cause: Box<dyn std::error::Error + Send + Sync>,
    ) -> Error {
        Error::WithCause {
            domain,
            detail: description.into(),
            cause,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_error_displays_its_code_time_and_names() {
        let err = Error::run(RunCode::E0503, SolverDomain::Dc, "the solve is singular")
            .with_unknowns(vec![0, 1])
            .with_names(vec!["flow of Top.v1 (b0)".into(), "flow of Top.v2 (b0)".into()]);
        assert_eq!(
            err.to_string(),
            "DC: E0503: the solve is singular: flow of Top.v1 (b0), flow of Top.v2 (b0)"
        );
        let timed = Error::run(RunCode::E0508, SolverDomain::Transient, "step fails at the floor")
            .with_time(1.5e-6)
            .with_names(vec!["node of Top.n1.p".into()]);
        assert_eq!(
            timed.to_string(),
            "Transient: E0508: step fails at the floor at t = 1.5e-6 s: node of Top.n1.p"
        );
        assert_eq!(timed.code(), Some(RunCode::E0508));
    }

    #[test]
    fn a_run_error_without_names_shows_its_unknowns() {
        let err = Error::run(RunCode::E0505, SolverDomain::Dc, "no equilibrium")
            .with_unknowns(vec![3, 7]);
        assert_eq!(err.to_string(), "DC: E0505: no equilibrium: unknowns #3, #7");
        assert_eq!(Error::simple(SolverDomain::Dc, "x").code(), None);
    }
}

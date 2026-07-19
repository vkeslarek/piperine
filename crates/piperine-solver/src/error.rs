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
    Element,
    Sens,
    Pss,
    Pz,
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
            SolverDomain::Element => "Element",
            SolverDomain::Sens => "Sens",
            SolverDomain::Pss => "PSS",
            SolverDomain::Pz => "PZ",
        }
    }
}

impl std::fmt::Display for SolverDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum Error {
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

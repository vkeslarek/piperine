use faer::sparse::linalg::LuError;
use faer::sparse::{CreationError, FaerError};

#[derive(Debug, Clone)]
pub enum Problem {
    FaerCreationProblem(CreationError),
    FaerLuError(LuError),
    FaerGenericError(FaerError),
    AcAnalysisNotEnabledForComponent { name: String },
}

#[derive(Debug)]
pub struct Error {
    pub title: String,
    pub detail: String,
    pub problems: Vec<Problem>,
}

impl Error {
    pub fn wrap(self, problem: Problem) -> Self {
        let Error {
            title,
            detail,
            mut problems,
        } = self;

        problems.push(problem);

        Self {
            title,
            detail,
            problems,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

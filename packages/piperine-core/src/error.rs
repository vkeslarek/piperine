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
pub struct ErrorDetail {
    pub title: String,
    pub detail: String,
    pub problems: Vec<Problem>,
}

impl ErrorDetail {
    pub(crate) fn simple(title: &str, description: &str) -> ErrorDetail {
        ErrorDetail {
            title: title.to_string(),
            detail: description.to_string(),
            problems: vec![],
        }
    }
}

impl ErrorDetail {
    pub fn wrap(self, problem: Problem) -> Self {
        let ErrorDetail {
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

pub type Result<T> = std::result::Result<T, ErrorDetail>;

use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Error {
    pub title: String,
    pub detail: String,
    pub cause: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}\nCaused by: {}",
            self.title,
            self.detail,
            match &self.cause {
                Some(cause) => format!("{}", cause),
                None => "None".to_string(),
            }
        )
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn wrap(title: impl Into<String>, detail: impl Into<String>, cause: Error) -> Error {
        Error {
            title: title.into(),
            detail: detail.into(),
            cause: Some(Box::new(cause)),
        }
    }

    pub fn simple(title: impl Into<String>, description: impl Into<String>) -> Error {
        Error {
            title: title.into(),
            detail: description.into(),
            cause: None,
        }
    }

    pub fn cause(
        title: impl Into<String>,
        description: impl Into<String>,
        cause: Box<dyn std::error::Error + Send + Sync>,
    ) -> Error {
        Error {
            title: title.into(),
            detail: description.into(),
            cause: Some(cause),
        }
    }
}

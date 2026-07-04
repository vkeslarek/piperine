use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{title}: {detail}")]
    Simple { title: String, detail: String },

    #[error("{title}: {detail}\nCaused by: {cause}")]
    WithCause {
        title: String,
        detail: String,
        #[source]
        cause: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl Error {
    pub fn wrap(title: impl Into<String>, detail: impl Into<String>, cause: Error) -> Error {
        Error::WithCause {
            title: title.into(),
            detail: detail.into(),
            cause: Box::new(cause),
        }
    }

    pub fn simple(title: impl Into<String>, description: impl Into<String>) -> Error {
        Error::Simple {
            title: title.into(),
            detail: description.into(),
        }
    }

    pub fn cause(
        title: impl Into<String>,
        description: impl Into<String>,
        cause: Box<dyn std::error::Error + Send + Sync>,
    ) -> Error {
        Error::WithCause {
            title: title.into(),
            detail: description.into(),
            cause,
        }
    }
}

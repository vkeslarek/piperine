use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug, Clone)]
pub enum ParseError {
    #[error("Unexpected end of file")]
    #[diagnostic(code(piperine::parse::eof))]
    UnexpectedEof {
        #[label("parser reached the end of this file here")]
        span: SourceSpan,
    },
    
    #[error("Unexpected token")]
    #[diagnostic(code(piperine::parse::unexpected_tok))]
    UnexpectedTok {
        #[label("found this instead of expected token")]
        span: SourceSpan,
        expected: String,
    },
    
    #[error("{message}")]
    #[diagnostic(code(piperine::parse::generic))]
    Generic {
        message: String,
        #[label("here")]
        span: SourceSpan,
    },

    #[error("{message}")]
    #[diagnostic(code(piperine::parse::legacy))]
    Legacy {
        message: String,
    },
}

impl ParseError {
    pub fn byte_offset(&self) -> Option<usize> {
        match self {
            ParseError::UnexpectedEof { span } => Some(span.offset()),
            ParseError::UnexpectedTok { span, .. } => Some(span.offset()),
            ParseError::Generic { span, .. } => Some(span.offset()),
            ParseError::Legacy { .. } => None,
        }
    }
}

impl From<String> for ParseError {
    fn from(message: String) -> Self {
        ParseError::Legacy { message }
    }
}

impl From<&str> for ParseError {
    fn from(message: &str) -> Self {
        ParseError::Legacy { message: message.into() }
    }
}

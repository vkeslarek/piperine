//! The one diagnostic type of the MSL front end.

use std::borrow::Cow;
use std::fmt;

use miette::{GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource};

use serde::{Deserialize, Serialize};

use super::span::Span;

/// The help of the `E0161` for `Clone`, `Copy` and `clone` (§1.4.6): every value is copied where it is used.
pub const COPY_HELP: &str = "every value is copied; delete the name";

/// The help of the `E0161` for `Unknown` and the flag methods (§3.3): a value is always known, and what
/// the flag modelled has three explicit forms.
pub const UNKNOWN_HELP: &str = "no value is unknown: give the carrier an initial value, hold an absence in an `Option<T>`, or use `Logic` for the four-state level";

/// An MSL error: a spec code (`E09xx`, `E0121`, ...), a one-line message, a
/// primary label at the offending span, optional secondary labels and an
/// optional help line naming the correct form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: Cow<'static, str>,
    pub message: String,
    /// The offending span and the text of its label.
    pub primary: (Span, String),
    pub secondary: Vec<(Span, String)>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>, span: Span, label: impl Into<String>) -> Self {
        Diagnostic {
            code: Cow::Borrowed(code),
            message: message.into(),
            primary: (span, label.into()),
            secondary: Vec::new(),
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_label(mut self, span: Span, label: impl Into<String>) -> Self {
        self.secondary.push((span, label.into()));
        self
    }

    pub fn span(&self) -> Span {
        self.primary.0
    }

    /// Render this diagnostic as a `miette` graphical report (no color) against `src`,
    /// named `name` in the report header.
    pub fn render(&self, name: &str, src: &str) -> String {
        let report = miette::Report::new(self.clone())
            .with_source_code(NamedSource::new(name, src.to_string()));
        let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor());
        let mut out = String::new();
        match handler.render_report(&mut out, report.as_ref()) {
            Ok(()) => out,
            Err(_) => format!("error[{}]: {}", self.code, self.message),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Diagnostic {}

impl miette::Diagnostic for Diagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.code.clone()))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help.as_ref().map(|h| Box::new(h) as Box<dyn fmt::Display>)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let primary = LabeledSpan::new_primary_with_span(Some(self.primary.1.clone()), self.primary.0);
        let rest = self
            .secondary
            .iter()
            .map(|(s, l)| LabeledSpan::new_with_span(Some(l.clone()), *s));
        Some(Box::new(std::iter::once(primary).chain(rest)))
    }
}

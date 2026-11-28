//! `Report`: a failure as the text a user reads on standard error (CLI-02 AC 6, CLI-04 AC 4).
//! Every diagnostic is a `miette` graphical report with its code, file, line and source
//! excerpt (never colored: the handler of `piperine-lang` is the no-color one, so a report is
//! the same bytes on a terminal and in a file), then one `error:` line naming the stage.

use std::io::{self, Write};

use piperine_api::Failure;

/// A failure laid out for a terminal.
pub struct Report;

impl Report {
    /// The reports of `failure`, then `error: <the failure's own message>`; a message such as
    /// `parse failed with 2 diagnostic(s)` names the stage. Package errors and a malformed run
    /// table are rendered against the manifest they point into; the language's diagnostics
    /// against the file of the design that holds them.
    pub fn write(failure: &Failure, err: &mut dyn Write) -> io::Result<()> {
        match failure {
            Failure::Package(errors) => {
                for error in errors {
                    writeln!(err, "{}", error.render())?;
                }
            }
            Failure::Diagnostics { design, found, .. } => {
                let files = design.files();
                for d in found {
                    match files.get(d.file.index()) {
                        Some(file) => writeln!(err, "{}", d.diagnostic.render(file.name(), file.text()))?,
                        None => writeln!(err, "error[{}]: {}", d.diagnostic.code, d.diagnostic.message)?,
                    }
                }
            }
            Failure::Request { diagnostic, file, text } => {
                let name = file.file_name().map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned());
                writeln!(err, "{}", diagnostic.render(&name, text))?;
            }
            Failure::NoRun { .. } | Failure::Host { .. } | Failure::Backend(_) | Failure::Solver { .. } | Failure::Io { .. } => {}
        }
        writeln!(err, "error: {failure}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_lang::pom::Design;

    /// Associated test helpers of `diagnostic`, grouped on one fixture type.
    struct DiagnosticFixture;

    impl DiagnosticFixture {
        fn report(failure: &Failure) -> String {
            let mut err = Vec::new();
            Report::write(failure, &mut err).expect("a vector takes writes");
            String::from_utf8(err).expect("text")
        }
    }

    #[test]
    fn a_diagnostic_has_code_file_line_and_excerpt() {
        // CLI-04 AC 4: every diagnostic with its code, file, line and source excerpt, then the
        // stage. `Nope` is no module (E0401); it stands at line 2, column 12 of `top.msl`.
        let design = Design::builder().source("top.msl", "struct Ok {}\nanalog for Nope { }\n").build();
        let found = design.diagnostics().to_vec();
        assert_eq!(found.len(), 1, "{found:?}");
        let text = DiagnosticFixture::report(&Failure::Diagnostics { stage: "parse", design: Box::new(design), found });
        assert!(text.contains("E0401"), "the code:\n{text}");
        assert!(text.contains("top.msl:2:12"), "the file, line and column:\n{text}");
        assert!(text.contains("analog for Nope { }"), "the excerpt:\n{text}");
        assert!(text.ends_with("error: parse failed with 1 diagnostic(s)\n"), "the stage line:\n{text}");
        assert!(!text.contains('\u{1b}'), "no color escape:\n{text}");
        // a failure with no excerpt is its message on one `error:` line
        let host = Failure::Host { code: "P0011", message: "no top".into() };
        assert_eq!(DiagnosticFixture::report(&host), "error: P0011: no top\n");
    }
}

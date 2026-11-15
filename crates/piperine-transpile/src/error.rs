//! Every way a transpilation, a build or a load fails (design "Data Models"), each naming what
//! the user acts on: the IR item, the form, the compiler, the directory or the file.

use std::fmt;
use std::path::PathBuf;

use piperine_ir::{Broken, Origin};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The `Ir` fails `check()`: its entries, each naming an item and its origin.
    Rejected(Vec<Broken>),
    /// A form with no Rust lowering, found while printing.
    Unsupported { form: String, path: String, origin: Origin },
    /// No compiler where `PIPERINE_RUSTC` or `PATH` says.
    NoCompiler { looked_for: String },
    CompilerTooOld { found: String, required: &'static str },
    /// The compiler refused generated source: a transpiler defect, the source kept on disk.
    CompileFailed { stderr: String, source: PathBuf },
    Cache { dir: PathBuf, cause: String },
    Load { file: PathBuf, cause: String },
    /// A loaded module's shape differs from the plan's: a transpiler defect.
    ShapeMismatch { body: String, entry: String },
    Setting { name: &'static str, value: String, valid: &'static str },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = |o: &Origin| format!("{}:{}:{}", o.file, o.line, o.col);
        match self {
            Error::Rejected(entries) => {
                write!(f, "the IR fails check() with {} entries", entries.len())?;
                for e in entries {
                    write!(f, "\n  {} @ {}: {}", e.item, at(&e.origin), e.what)?;
                }
                Ok(())
            }
            Error::Unsupported { form, path, origin } => {
                write!(f, "`{form}` has no Rust lowering yet: {path} @ {}", at(origin))
            }
            Error::NoCompiler { looked_for } => write!(
                f,
                "the compiled backend needs `rustc` and none was found (looked for {looked_for}); install a Rust \
                 toolchain or set PIPERINE_RUSTC; the interpreter backend needs no compiler"
            ),
            Error::CompilerTooOld { found, required } => {
                write!(f, "`rustc` {found} is too old: the compiled backend needs {required} or newer")
            }
            Error::CompileFailed { stderr, source } => {
                write!(f, "rustc refused the generated source {} (a transpiler defect):\n{stderr}", source.display())
            }
            Error::Cache { dir, cause } => write!(f, "the cache directory {} cannot be used: {cause}", dir.display()),
            Error::Load { file, cause } => write!(f, "the library {} cannot be loaded: {cause}", file.display()),
            Error::ShapeMismatch { body, entry } => {
                write!(f, "the compiled `{body}` declares {entry} unlike its plan (a transpiler defect)")
            }
            Error::Setting { name, value, valid } => write!(f, "{name} = `{value}` is not valid: {valid}"),
        }
    }
}

impl std::error::Error for Error {}

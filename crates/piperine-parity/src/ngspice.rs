//! Locating and running ngspice. The golden side of a parity test is ngspice run at test time;
//! its absence fails loud unless `PIPERINE_NGSPICE=skip` says the comparison is off.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::golden::Golden;
use crate::scratch::ScratchDir;

/// The name of the environment variable that picks the binary or turns the comparison off.
const VARIABLE: &str = "PIPERINE_NGSPICE";

/// What the lookup decided.
#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    Binary(PathBuf),
    Skip,
}

/// What a run gave: ngspice's output, or nothing because the comparison is disabled.
#[derive(Debug)]
pub enum Reference {
    Golden(Golden),
    Skipped,
}

/// A located ngspice (or the decision to skip it).
#[derive(Clone, Debug)]
pub struct Ngspice {
    lookup: Lookup,
}

impl Ngspice {
    pub fn new(lookup: Lookup) -> Self {
        Ngspice { lookup }
    }

    /// The decision from the variable's value and the `PATH` value, both given: a path in the
    /// variable wins, the literal `skip` disables the comparison, otherwise `ngspice` is searched
    /// on the `PATH`. An error names what was looked for and the opt-out.
    pub fn locate(variable: Option<&str>, path: Option<&OsStr>) -> Result<Lookup, String> {
        match variable.filter(|value| !value.is_empty()) {
            Some("skip") => return Ok(Lookup::Skip),
            Some(given) => {
                return if Path::new(given).is_file() {
                    Ok(Lookup::Binary(PathBuf::from(given)))
                } else {
                    Err(format!("{VARIABLE}={given} names no file; set it to an ngspice binary, unset it, or set {VARIABLE}=skip to disable the comparison"))
                };
            }
            None => {}
        }
        let searched = path.map(OsString::from).unwrap_or_default();
        for directory in std::env::split_paths(&searched) {
            let candidate = directory.join("ngspice");
            if candidate.is_file() {
                return Ok(Lookup::Binary(candidate));
            }
        }
        Err(format!(
            "ngspice not found: looked at {VARIABLE} (unset) and for `ngspice` on PATH ({}); install ngspice or set {VARIABLE}=skip to disable the comparison",
            searched.to_string_lossy()
        ))
    }

    /// The lookup from the process environment. Panics with the lookup's message: it serves tests only.
    pub fn from_environment() -> Self {
        let variable = std::env::var(VARIABLE).ok();
        let path = std::env::var_os("PATH");
        match Self::locate(variable.as_deref(), path.as_deref()) {
            Ok(lookup) => Ngspice { lookup },
            Err(message) => panic!("{message}"),
        }
    }

    /// The line a skipped fixture prints.
    pub fn skip_line(fixture: &str) -> String {
        format!("SKIP {fixture}: ngspice comparison disabled by {VARIABLE}=skip")
    }

    /// The `.cir` of a fixture, as an absolute path.
    pub fn fixture_path(fixture: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice").join(format!("{fixture}.cir")).canonicalize().unwrap_or_else(|error| {
            panic!("{fixture}: no tests/ngspice/{fixture}.cir ({error})");
        })
    }

    /// Runs `ngspice -b <fixture>.cir` in a new scratch directory. Panics, naming the fixture and
    /// ngspice's captured output, when ngspice cannot start or exits non-zero.
    pub fn run(&self, fixture: &str) -> Reference {
        let binary = match &self.lookup {
            Lookup::Skip => {
                println!("{}", Self::skip_line(fixture));
                return Reference::Skipped;
            }
            Lookup::Binary(binary) => binary,
        };
        let scratch = ScratchDir::create(fixture);
        let output = Command::new(binary)
            .arg("-b")
            .arg(Self::fixture_path(fixture))
            .current_dir(scratch.path())
            .output()
            .unwrap_or_else(|error| panic!("{fixture}: cannot run {}: {error}", binary.display()));
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            panic!("{fixture}: ngspice exited with {}\n--- ngspice stdout ---\n{stdout}\n--- ngspice stderr ---\n{stderr}", output.status);
        }
        Reference::Golden(Golden::new(fixture, stdout, stderr, Self::version(binary), scratch))
    }

    fn version(binary: &Path) -> String {
        let output = Command::new(binary)
            .arg("--version")
            .output()
            .unwrap_or_else(|error| panic!("cannot run {} --version: {error}", binary.display()));
        String::from_utf8_lossy(&output.stdout).lines().map(str::trim).find(|line| line.contains("ngspice")).unwrap_or("ngspice (version line not found)").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory_with_ngspice() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("ngspice"), "").expect("write a stand-in binary");
        directory
    }

    #[test]
    fn the_binary_comes_from_the_variable_or_the_path() {
        let on_path = directory_with_ngspice();
        let elsewhere = directory_with_ngspice();
        let named = elsewhere.path().join("ngspice");
        let path = std::env::join_paths([on_path.path()]).expect("join paths");

        let from_variable = Ngspice::locate(named.to_str(), Some(&path));
        assert_eq!(from_variable, Ok(Lookup::Binary(named.clone())), "a path in the variable wins over PATH");
        let from_path = Ngspice::locate(None, Some(&path));
        assert_eq!(from_path, Ok(Lookup::Binary(on_path.path().join("ngspice"))));
        let from_empty = Ngspice::locate(Some(""), Some(&path));
        assert_eq!(from_empty, from_path, "an empty variable is unset");
        let missing = on_path.path().join("absent");
        assert!(Ngspice::locate(missing.to_str(), Some(&path)).is_err(), "a variable naming no file fails loud");
    }

    #[test]
    fn a_missing_binary_fails_naming_the_opt_out() {
        let empty = tempfile::tempdir().expect("temp dir");
        let path = std::env::join_paths([empty.path()]).expect("join paths");
        let message = Ngspice::locate(None, Some(&path)).expect_err("no binary on PATH");
        assert!(message.contains("PIPERINE_NGSPICE"), "{message}");
        assert!(message.contains("PIPERINE_NGSPICE=skip"), "{message}");
        assert!(message.contains("ngspice"), "{message}");
        assert!(message.contains(&*path.to_string_lossy()), "names what it searched: {message}");
        assert!(Ngspice::locate(None, None).is_err(), "no PATH at all is a miss too");
    }

    #[test]
    fn skip_passes_and_prints_one_line() {
        assert_eq!(Ngspice::locate(Some("skip"), None), Ok(Lookup::Skip), "skip wins even with no PATH");
        let line = Ngspice::skip_line("divider");
        assert_eq!(line, "SKIP divider: ngspice comparison disabled by PIPERINE_NGSPICE=skip");
        assert_eq!(line.lines().count(), 1);
        assert!(matches!(Ngspice::new(Lookup::Skip).run("divider"), Reference::Skipped));
    }
}

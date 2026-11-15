//! Finding the compiler and running it (design "Rustc"; RS-03, RS-Q4). `rustc` is run directly
//! and `cargo` never; the flags are fixed so the cache key can name them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::Error;
use crate::settings::Settings;

/// A located compiler of an accepted version.
#[derive(Debug)]
pub struct Rustc {
    program: PathBuf,
    version_text: String,
    builds: AtomicU32,
}

impl Rustc {
    /// The first release with edition 2024 (RS-Q4, RS-03 AC 4).
    pub const MINIMUM: &'static str = "1.85";

    /// The flags of every build (RS-Q4): edition 2024, optimized, line tables kept so a
    /// backtrace names the generated source, lints capped so a newer compiler never breaks a
    /// user's build. They are part of the cache key.
    pub const FLAGS: &'static [&'static str] =
        &["--edition", "2024", "-C", "opt-level=2", "-C", "debuginfo=1", "--cap-lints", "warn"];

    /// The compiler `settings` names (`PIPERINE_RUSTC`), else `rustc` on `PATH`, asked for
    /// `-vV`. A compiler that cannot be run is `Error::NoCompiler`; one below [`Rustc::MINIMUM`]
    /// is `Error::CompilerTooOld`.
    pub fn locate(settings: &Settings) -> Result<Rustc, Error> {
        let (program, looked_for) = match &settings.rustc {
            Some(path) => (path.clone(), format!("PIPERINE_RUSTC={}", path.display())),
            None => (PathBuf::from("rustc"), "`rustc` on PATH".to_string()),
        };
        let output = Command::new(&program)
            .arg("-vV")
            .output()
            .map_err(|e| Error::NoCompiler { looked_for: format!("{looked_for}: {e}") })?;
        if !output.status.success() {
            return Err(Error::NoCompiler { looked_for: format!("{looked_for}: `-vV` exited with {}", output.status) });
        }
        let version_text = String::from_utf8_lossy(&output.stdout).into_owned();
        let release = version_text
            .lines()
            .find_map(|l| l.strip_prefix("release: "))
            .map(str::trim)
            .unwrap_or("unknown")
            .to_string();
        if !Rustc::accepts(&release) {
            return Err(Error::CompilerTooOld { found: release, required: Rustc::MINIMUM });
        }
        Ok(Rustc { program, version_text, builds: AtomicU32::new(0) })
    }

    /// Whether `release` (`1.98.0`, `1.86.0-nightly`) is at least [`Rustc::MINIMUM`].
    fn accepts(release: &str) -> bool {
        let mut parts = release.split('.').map(|p| p.split('-').next().unwrap_or("").parse::<u32>());
        let (Some(Ok(major)), Some(Ok(minor))) = (parts.next(), parts.next()) else { return false };
        (major, minor) >= (1, 85)
    }

    /// The output of `rustc -vV`, whole: it is part of the cache key.
    pub fn version_text(&self) -> &str {
        &self.version_text
    }

    /// Builds the `piperine_abi` library from `src` (its `lib.rs`) into `out`; the compiler's
    /// standard error, which holds any warning, on success.
    pub fn rlib(&self, src: &Path, out: &Path) -> Result<String, Error> {
        self.build(src, out, &["--crate-type", "rlib", "--crate-name", "piperine_abi"], None)
    }

    /// Builds the design `src` into the `cdylib` `out`, linking the `rlib` built by
    /// [`Rustc::rlib`] as `piperine_abi`; the compiler's standard error on success.
    pub fn cdylib(&self, src: &Path, extern_abi: &Path, out: &Path) -> Result<String, Error> {
        self.build(src, out, &["--crate-type", "cdylib", "--crate-name", "design"], Some(extern_abi))
    }

    /// How many builds this compiler has run; `-vV` is not one.
    pub fn invocations(&self) -> u32 {
        self.builds.load(Ordering::SeqCst)
    }

    fn build(&self, src: &Path, out: &Path, kind: &[&str], extern_abi: Option<&Path>) -> Result<String, Error> {
        self.builds.fetch_add(1, Ordering::SeqCst);
        let mut command = Command::new(&self.program);
        command.args(Rustc::FLAGS).args(kind);
        if let Some(rlib) = extern_abi {
            command.arg("--extern").arg(format!("piperine_abi={}", rlib.display()));
            if let Some(dir) = rlib.parent() {
                command.arg("-L").arg(format!("dependency={}", dir.display()));
            }
        }
        command.arg(src).arg("-o").arg(out);
        let output = command
            .output()
            .map_err(|e| Error::NoCompiler { looked_for: format!("{}: {e}", self.program.display()) })?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            Ok(stderr)
        } else {
            Err(Error::CompileFailed { stderr, source: src.to_path_buf() })
        }
    }
}

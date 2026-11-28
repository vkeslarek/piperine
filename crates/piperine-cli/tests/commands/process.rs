//! The `piperine` binary run as a child process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use piperine_parity::compile_cache::CompileCache;

use super::project_directories::ProjectDirectory;

/// The `piperine` binary run as a child process, and what it printed read back as text.
pub(crate) struct PiperineProcess;

impl PiperineProcess {
    /// `piperine <args>` in `dir`.
    pub(crate) fn piperine_in(dir: &Path, args: &[&str]) -> Output {
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", CompileCache::shared().path()).env_remove("PIPERINE_BACKEND").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(env!("CARGO_BIN_EXE_piperine")).display()))
    }

    pub(crate) fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    /// The standard output of a command that must have exited 0.
    pub(crate) fn stdout_of(out: &Output) -> String {
        assert_eq!(out.status.code(), Some(0), "stderr:\n{}", PiperineProcess::text(&out.stderr));
        PiperineProcess::text(&out.stdout)
    }

    /// The cells of a line, split at runs of spaces.
    pub(crate) fn cells(line: &str) -> Vec<&str> {
        line.split_whitespace().collect()
    }

    /// `piperine <args>` in `dir`, on the cache `cache`.
    pub(crate) fn piperine_cached(dir: &Path, cache: &Path, args: &[&str]) -> Output {
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", cache).env_remove("PIPERINE_BACKEND").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(env!("CARGO_BIN_EXE_piperine")).display()))
    }

    pub(crate) fn piperine_on_example(args: &[&str]) -> Output {
        PiperineProcess::piperine_cached(&ProjectDirectory::rc(), CompileCache::shared().path(), args)
    }

    /// A run that failed with `code`: nothing on standard output, a message on standard error.
    pub(crate) fn failed(out: &Output, code: i32) -> String {
        let err = PiperineProcess::text(&out.stderr);
        assert_eq!(out.status.code(), Some(code), "stdout:\n{}\nstderr:\n{err}", PiperineProcess::text(&out.stdout));
        err
    }

    /// `piperine <args>` in `dir`, on the cache `cache`, with a compiler path `rustc` when given.
    pub(crate) fn piperine_with(dir: &Path, cache: &Path, rustc: Option<&str>, args: &[&str]) -> Output {
        let mut command = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine")));
        command.args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", cache).env_remove("PIPERINE_BACKEND").env_remove("PIPERINE_RUSTC");
        if let Some(rustc) = rustc {
            command.env("PIPERINE_RUSTC", rustc);
        }
        command.output().unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(env!("CARGO_BIN_EXE_piperine")).display()))
    }

    pub(crate) fn piperine_shared(dir: &Path, args: &[&str]) -> Output {
        PiperineProcess::piperine_with(dir, CompileCache::shared().path(), None, args)
    }

    /// `piperine <args>` in `dir` with `PIPERINE_BACKEND` set to `backend`.
    pub(crate) fn piperine_on(dir: &Path, backend: &str, args: &[&str]) -> Output {
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", CompileCache::shared().path()).env("PIPERINE_BACKEND", backend).env_remove("PIPERINE_RUSTC").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(env!("CARGO_BIN_EXE_piperine")).display()))
    }

    /// The lines of a standard output.
    pub(crate) fn lines_of(out: &Output) -> Vec<String> {
        PiperineProcess::text(&out.stdout).lines().map(str::to_string).collect()
    }

    // ---- new (CLI-17) ---------------------------------------------------------------------------

    pub(crate) fn piperine(args: &[&str]) -> Output {
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(args).output().unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(env!("CARGO_BIN_EXE_piperine")).display()))
    }
}

//! A design built and loaded (design "Transpiler and Compiled"): the library the solver binds
//! its modules from, the source kept beside it, and what the build cost.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use piperine_ir::Ir;

use crate::cache::Entry;
use crate::load::Loaded;

/// What one `Transpiler::compile` cost (RS-08).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Printing the IR as Rust.
    pub transpile: Duration,
    /// Running `rustc` (the ABI `rlib` when it was missing, and the design's `cdylib`); zero on
    /// a cache hit.
    pub compile: Duration,
    /// Checking and opening the library.
    pub load: Duration,
    /// Builds `rustc` ran for this compile: 0 on a cache hit.
    pub rustc_invocations: u32,
    /// Whether the entry was already in the cache and loaded as it was.
    pub cache_hit: bool,
    /// The compiler's standard error for the design's build: its warnings. Empty on a cache
    /// hit, where nothing was built.
    pub warnings: String,
}

/// A design compiled, cached and loaded.
#[derive(Debug)]
pub struct Compiled {
    pub(crate) ir: Arc<Ir>,
    pub(crate) entry: Entry,
    pub(crate) loaded: Loaded,
    pub(crate) report: Report,
}

impl Compiled {
    /// What this build cost.
    pub fn report(&self) -> &Report {
        &self.report
    }

    /// The generated Rust (`design.rs`), kept on disk beside the library (RS-02, R3): read it
    /// to see what runs.
    pub fn source_path(&self) -> PathBuf {
        self.entry.source()
    }

    /// The cache entry: its source, runtime source, library and manifest.
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// The loaded library.
    pub fn loaded(&self) -> &Loaded {
        &self.loaded
    }

    /// The design this was built from.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }
}

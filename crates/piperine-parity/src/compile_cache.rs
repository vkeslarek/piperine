//! The one compile cache of a test run: every test of every target of one `cargo test` invocation builds
//! its generated libraries into the same directory under the target directory, so a design that two
//! targets compile is compiled once, and no directory is left in the temporary directory.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use piperine_transpile::Settings;

/// The cache directory of this test run, found or made on first use.
pub struct CompileCache {
    path: PathBuf,
}

impl CompileCache {
    /// The name of the directory, under the target directory, that holds the caches of the runs.
    const DIRECTORY: &'static str = "piperine-compile-cache";

    /// The cache of this run: `<target>/piperine-compile-cache/<process of cargo>`. A test binary's parent is
    /// the `cargo test` that started it, so every target of one invocation shares the directory; the
    /// directories of runs whose `cargo` is gone are removed when a run makes its own.
    pub fn shared() -> &'static CompileCache {
        static SHARED: OnceLock<CompileCache> = OnceLock::new();
        SHARED.get_or_init(|| {
            let base = Self::target_directory().join(Self::DIRECTORY);
            Self::remove_finished_runs(&base);
            let path = base.join(std::os::unix::process::parent_id().to_string());
            std::fs::create_dir_all(&path).unwrap_or_else(|error| panic!("the compile cache {}: {error}", path.display()));
            CompileCache { path }
        })
    }

    /// The cache directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The compiler settings of a run that builds into this cache with the `rustc` on `PATH`.
    pub fn settings(&self) -> Settings {
        Settings { rustc: None, cache: self.path.clone() }
    }

    /// The target directory the running test binary was built into: the directory above `debug/deps`.
    fn target_directory() -> PathBuf {
        let executable = std::env::current_exe().expect("the path of the running test binary");
        executable.ancestors().nth(3).unwrap_or_else(|| panic!("{} is not inside a target directory", executable.display())).to_path_buf()
    }

    /// Removes the directories of `base` that name no running process: a finished run's, or one whose
    /// name is not a process number at all. A running process's directory stays.
    pub fn remove_finished_runs(base: &Path) {
        let Ok(entries) = std::fs::read_dir(base) else { return };
        for entry in entries.flatten() {
            let running = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()).is_some_and(|process| Path::new("/proc").join(process.to_string()).exists());
            if !running {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

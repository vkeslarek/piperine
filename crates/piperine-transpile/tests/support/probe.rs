//! A printed IR function compiled with the runtime and run: the program `rustc` builds from the
//! function texts and a `main`, written into a temp dir beside the runtime files that need no ABI
//! (`real.rs`, `int.rs`, `flag.rs`, `control.rs`). Shared by the printer tests of control flow.

use std::path::Path;
use std::process::Command;

use piperine_transpile::Rustc;
use piperine_transpile::rt::{CONTROL, FLAG, INT, REAL};

/// The `rt.rs` of a probe: the runtime without `commits.rs`, which needs the ABI crate.
const ROOT: &str = "\
pub mod control;
pub mod flag;
pub mod int;
pub mod real;

#[allow(unused_imports)]
pub use control::{Depth, Index, Outcome, Panic, Step, Trips, Unrolled};
#[allow(unused_imports)]
pub use flag::{Flagged, LevelWords};
#[allow(unused_imports)]
pub use int::{Int, UInt, Unwrap};
#[allow(unused_imports)]
pub use real::{Dual, Real};
";

/// One program to build and run.
pub struct Probe;

impl Probe {
    /// What the program made of `functions` (printed IR functions) and `main` (the text of a
    /// `fn main()`) writes to standard output; a build that fails or a run that fails is a panic
    /// of the test naming the compiler's or the program's standard error.
    pub fn run(functions: &[String], main: &str) -> String {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("rt")).expect("rt dir");
        for (path, text) in [("rt.rs", ROOT), ("rt/real.rs", REAL), ("rt/int.rs", INT), ("rt/flag.rs", FLAG), ("rt/control.rs", CONTROL)] {
            std::fs::write(root.join(path), text).expect("a runtime file");
        }
        let program = format!("mod rt;\n#[allow(unused_imports)]\nuse rt::Real;\n\n{}\n\n{main}\n", functions.join("\n\n"));
        std::fs::write(root.join("main.rs"), program).expect("main.rs");
        let compiler = std::env::var_os("PIPERINE_RUSTC").unwrap_or_else(|| "rustc".into());
        let built = Command::new(compiler)
            .args(Rustc::FLAGS)
            .args(["--crate-type", "bin", "--crate-name", "probe"])
            .arg(root.join("main.rs"))
            .arg("-o")
            .arg(root.join("probe"))
            .output()
            .expect("rustc runs");
        assert!(built.status.success(), "rustc failed:\n{}\n--- source ---\n{}", String::from_utf8_lossy(&built.stderr), Self::source(root));
        let ran = Command::new(root.join("probe")).output().expect("the program runs");
        assert!(ran.status.success(), "the program failed: {}", String::from_utf8_lossy(&ran.stderr));
        String::from_utf8_lossy(&ran.stdout).into_owned()
    }

    /// The program's text, for a failure message.
    fn source(root: &Path) -> String {
        std::fs::read_to_string(root.join("main.rs")).unwrap_or_default()
    }
}

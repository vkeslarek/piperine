//! A one-module package the harness tests plant on disk, so the stimulus and the runs are proven
//! on a design small enough to build in seconds and known in full.

use std::path::Path;

use tempfile::TempDir;

/// The package's one source: a source and a resistor with a param, under a top with one discrete
/// `in` port.
const LIBRARY: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\npub module Top(in level: Bit<u8>) {\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\n";

/// The manifest: two identical 2 us transients, `short` and `twin`, printing the port and the
/// node every 0.5 us.
const MANIFEST: &str = "[package]\nname = \"planted\"\nversion = \"0.1.0\"\n\n[run.short]\ntop = \"Top\"\nanalysis = \"transient\"\nstop = 2e-6\nmax_step = 1e-7\nprint = [\"Top.level\", \"Top.a\"]\nprint_step = 5e-7\n\n[run.twin]\ntop = \"Top\"\nanalysis = \"transient\"\nstop = 2e-6\nmax_step = 1e-7\nprint = [\"Top.level\", \"Top.a\"]\nprint_step = 5e-7\n";

/// The planted package, deleted with this value.
pub struct PlantedPackage {
    /// The temporary directory holding the package; removed with the value.
    dir: TempDir,
}

impl PlantedPackage {
    /// Writes the package into a fresh temporary directory.
    pub fn write() -> PlantedPackage {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir_all(dir.path().join("src")).expect("the source directory");
        std::fs::write(dir.path().join("Piperine.toml"), MANIFEST).expect("the manifest");
        std::fs::write(dir.path().join("src/lib.msl"), LIBRARY).expect("the library");
        PlantedPackage { dir }
    }

    /// The package's directory.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

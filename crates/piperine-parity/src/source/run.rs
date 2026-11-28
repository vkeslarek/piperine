//! The run of one model file of an MSL source target (the `source_*` targets that run a model file:
//! `source_values`, `source_structure`): read from the target's own directory, elaborated, compiled on the
//! transpiled backend and run on the solver, then read back as commits, node voltages, a fault or
//! records.

use std::path::{Path, PathBuf};

use piperine_api::{Record, Streams};
use crate::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier, Solver, TransientAnalysisOptions};

use crate::buck_fixture::BuckFixture;

/// The model files of one directory of `tests/`.
pub struct SourceRun {
    /// The directory, relative to the crate root: `tests/control`.
    directory: &'static str,
}

impl SourceRun {
    /// The runs of the model files in `directory` of the crate.
    pub const fn in_directory(directory: &'static str) -> SourceRun {
        SourceRun { directory }
    }

    /// The lone file `file` of the directory, as a project.
    pub fn project(&self, file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join(self.directory).join(file);
        Project::new(&path)
    }

    /// What `Bench` of `file` committed at its first rising edge (2.5 ns of a 200 MHz oscillator): the
    /// value of each carrier in `names` (a commit of the value it started at changes nothing).
    pub fn first_commits(&self, file: &str, names: &[&str]) -> Vec<Vec<u8>> {
        let seen = BuckFixture::build(&self.project(file), "Bench").run(10e-9, 1e-9);
        names
            .iter()
            .map(|name| BuckFixture::first_edge_value(&seen, &format!("Bench.{name}")))
            .collect()
    }

    /// The commits of `names` of `file`, each compared with the bytes Rust gives.
    pub fn assert_commits(&self, file: &str, names: &[&str], wanted: &[Vec<u8>]) {
        let committed = self.first_commits(file, names);
        for (k, name) in names.iter().enumerate() {
            assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
        }
    }

    /// The operating point of `top` of `file`: the voltage of each node in `names`.
    pub fn operating_point(&self, file: &str, top: &str, names: &[&str]) -> Vec<f64> {
        let mut running = self.project(file).compile(top).unwrap_or_else(|r| panic!("{r}"));
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        names
            .iter()
            .map(|name| {
                let node = running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("node {name}"));
                solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)")
            })
            .collect()
    }

    /// The diagnostics with which the front end refuses `top` of `file`: the compile is expected to fail.
    pub fn refusal(&self, file: &str, top: &str) -> String {
        match self.project(file).compile(top) {
            Ok(_) => panic!("`{top}` of `{file}` was expected to be refused"),
            Err(refused) => refused.to_string(),
        }
    }

    /// The message of the fault the transient of `top` of `file` runs into: the run is expected to fail.
    pub fn the_fault(&self, file: &str, top: &str) -> String {
        let built = BuckFixture::build(&self.project(file), top);
        let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
        let mut options = TransientAnalysisOptions::new(10e-9, 1e-9);
        options.dt_max = 1e-9;
        let mut solver = Solver::new(circuit).with_context(Context::default()).with_tran_opts(options).build();
        match solver.tran().and_then(|mut t| t.solve()) {
            Ok(_) => panic!("the run was expected to fault"),
            Err(e) => e.to_string(),
        }
    }

    /// Every record of the 4 ns transient of `top` of `file`, decoded as the host decodes it.
    pub fn records_of(&self, file: &str, top: &str) -> Vec<Record> {
        let built = BuckFixture::build(&self.project(file), top);
        let streams = Streams::of(built.ir.clone());
        let seen = built.run(4e-9, 0.5e-9);
        seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect()
    }
}

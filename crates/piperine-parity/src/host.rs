//! A twin run the way a user runs it (host-ii HII-03 to HII-07): through the host library's
//! `Session`, with the run keys of `piperine run` (`temp`, `input`, `output`, `wrt`, `period`, ...),
//! so a parity target holds what the run keys do against ngspice and not only what the solver
//! gives a hand-built context.

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Overrides, Project, Session, Table};
use piperine_transpile::Settings;

use crate::backend::Backend;
use crate::bench::Bench;
use crate::compile_cache::CompileCache;
use crate::sinks::Ignore;

/// One run of a twin through a `Session`.
pub struct HostRun {
    twin: String,
    backend: Backend,
    overrides: Overrides,
}

impl HostRun {
    /// A run of `tests/e2e/twins/<twin>.msl` that takes `analysis`, on `backend`.
    pub fn new(twin: &str, analysis: &str, backend: Backend) -> HostRun {
        HostRun { twin: twin.to_string(), backend, overrides: Overrides { analysis: Some(analysis.to_string()), ..Overrides::default() } }
    }

    /// The run keys, given by `set` on the request.
    pub fn keys(mut self, set: impl FnOnce(&mut Overrides)) -> HostRun {
        set(&mut self.overrides);
        self
    }

    /// The table the run gives; a failure panics naming the twin, the backend and the failure.
    pub fn table(self) -> Table {
        let fail = |stage: &str, what: &dyn std::fmt::Display| -> ! { panic!("{}: {stage} on {}: {what}", self.twin, self.backend.name()) };
        let project = Project::file(&Bench::twin_path(&self.twin)).unwrap_or_else(|f| fail("project", &f));
        let overrides = Overrides { backend: Some(self.backend.name().to_string()), ..self.overrides.clone() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| fail("request", &f));
        let mut session = Session::open(&project, &spec, &HostRun::settings()).unwrap_or_else(|f| fail("open", &f));
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| fail("run", &f))
    }

    /// The compiler settings of every run of this process: one temporary cache, so a test run
    /// never writes the user's own cache.
    pub fn settings() -> Settings {
        let dir = CompileCache::shared();
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    /// The real number of `cell`; any other cell is a failure of the table's shape.
    pub fn real(cell: &Cell) -> f64 {
        match cell {
            Cell::Real(value) => *value,
            other => panic!("a real number, found {other:?}"),
        }
    }

    /// The row of `table` whose first cell is the text `key`.
    pub fn row<'t>(table: &'t Table, key: &str) -> &'t [Cell] {
        table.rows.iter().find(|row| row[0] == Cell::Text(key.to_string())).unwrap_or_else(|| panic!("no row `{key}` in {table:?}")).as_slice()
    }
}

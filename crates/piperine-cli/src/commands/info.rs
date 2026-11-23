//! `InfoCommand`: `piperine info [<run>]` (HII-11). Elaborates the run's top, prints its name
//! and backend, then the tables `params`, `ports` and `streams`; nothing is built or simulated.
//! A design that does not elaborate gets the diagnostics `piperine check` prints, exit 1.

use std::io::Write;
use std::path::{Path, PathBuf};

use piperine_api::{Cell, Failure, Metadata, Overrides, Project, Session, Table};
use piperine_transpile::Settings;

use crate::cli::Out;
use crate::commands::run::{RunCommand, Stop};
use crate::commands::Locate;
use crate::exit::Exit;
use crate::render::diagnostic::Report;
use crate::render::table::TextTable;

/// `piperine info`: the project and the run to describe (or a lone `.msl` file).
pub struct InfoCommand {
    pub project: Option<PathBuf>,
    pub target: Option<String>,
}

impl InfoCommand {
    /// Exit 0 with the header and the three tables, 1 for a design that does not elaborate, 2
    /// for a run that cannot be chosen.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        match self.attempt(out) {
            Ok(()) => Exit::Done,
            Err(Stop::Failed(failure)) => {
                let _ = Report::write(&failure, err);
                Exit::Failed
            }
            Err(Stop::Usage(failure)) => {
                let _ = Report::write(&failure, err);
                Exit::Usage
            }
            Err(Stop::Closed(error)) => {
                let _ = writeln!(err, "error: cannot write to standard output: {error}");
                Exit::Failed
            }
        }
    }

    fn attempt(&self, out: &Out) -> Result<(), Stop> {
        let lone = self.target.as_deref().map(Path::new).filter(|p| p.extension().is_some_and(|e| e == "msl"));
        let project = match lone {
            Some(file) => Project::file(file)?,
            None => Locate::package(self.project.as_deref())?,
        };
        // a lone file has no run table: an operating point names the top as the run would
        let overrides = if lone.is_some() { Overrides { analysis: Some("op".into()), ..Overrides::default() } } else { Overrides::default() };
        // with no run named, the first run of the manifest: the runs of a package share a design
        let first = project.runs().first().map(|run| run.name.clone());
        let name = if lone.is_some() { None } else { self.target.clone().or(first) };
        let spec = project.request(name.as_deref(), &overrides)?;
        let backend = RunCommand::backend(None, &spec)?;
        let (top, metadata) = Session::describe(&project, &spec, &|| Settings::from_environment().map_err(|error| Failure::Backend(error.to_string())))?;
        let mut out = out.borrow_mut();
        writeln!(out, "info {}: top {top}, backend {}", spec.name, backend.name())?;
        for (title, table) in [("params", Self::params(&metadata)), ("ports", Self::ports(&metadata)), ("streams", Self::streams(&metadata))] {
            writeln!(out)?;
            writeln!(out, "{title}")?;
            TextTable::write(&table, &mut *out)?;
        }
        out.flush()?;
        Ok(())
    }

    fn text(value: &str) -> Cell {
        Cell::Text(if value.is_empty() { "-".to_string() } else { value.to_string() })
    }

    fn params(metadata: &Metadata) -> Table {
        let columns = ["path", "type", "value", "where", "structural"].map(String::from).to_vec();
        let rows = metadata
            .params
            .iter()
            .map(|p| vec![Cell::Text(p.path.clone()), Cell::Text(p.ty.clone()), p.value.clone(), Self::text(p.check.as_deref().unwrap_or("")), Cell::Bool(p.structural)])
            .collect();
        Table { columns, rows }
    }

    fn ports(metadata: &Metadata) -> Table {
        let columns = ["name", "direction", "type"].map(String::from).to_vec();
        let rows = metadata.ports.iter().map(|p| vec![Cell::Text(p.name.clone()), Cell::Text(p.direction.into()), Cell::Text(p.ty.clone())]).collect();
        Table { columns, rows }
    }

    fn streams(metadata: &Metadata) -> Table {
        let columns = ["stream", "fields"].map(String::from).to_vec();
        let rows = metadata
            .streams
            .iter()
            .map(|s| {
                let fields: Vec<String> = s.fields.iter().map(|(name, ty)| format!("{name}: {ty}")).collect();
                vec![Cell::Text(s.path.clone()), Self::text(&fields.join(", "))]
            })
            .collect();
        Table { columns, rows }
    }
}

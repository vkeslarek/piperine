//! `Cli`: the command tree, built from data with `clap`'s builder interface (no derive), its
//! parse into one [`Invocation`], and its execution.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use clap::builder::PathBufValueParser;
use clap::{Arg, ArgAction, Command};

use crate::exit::Exit;

/// One argument of a command: its id, whether it is a positional, a switch or a valued flag,
/// and its help line.
struct ArgRow {
    id: &'static str,
    kind: ArgKind,
    help: &'static str,
}

enum ArgKind {
    /// A positional argument; `many` takes any number of values.
    Positional { required: bool, many: bool },
    /// `--<id>`, no value.
    Switch,
}

/// One command: its name, its help line and its arguments.
struct CommandRow {
    name: &'static str,
    about: &'static str,
    args: &'static [ArgRow],
}

/// The seven commands (CLI-01 AC 5), in the order `--help` lists them.
const COMMANDS: &[CommandRow] = &[
    CommandRow {
        name: "check",
        about: "Parse, resolve, type-check and elaborate the project's runs, without a backend",
        args: &[ArgRow { id: "target", kind: ArgKind::Positional { required: false, many: false }, help: "a lone .msl file to check" }],
    },
    CommandRow {
        name: "build",
        about: "Compile the top of every run on its backend",
        args: &[
            ArgRow { id: "runs", kind: ArgKind::Positional { required: false, many: true }, help: "the runs to build; all when none is named" },
            ArgRow { id: "verbose", kind: ArgKind::Switch, help: "print the path of the generated Rust source" },
        ],
    },
    CommandRow {
        name: "run",
        about: "Run one run of the project, or a lone .msl file",
        args: &[ArgRow { id: "target", kind: ArgKind::Positional { required: false, many: false }, help: "a run name, or a .msl file" }],
    },
    CommandRow {
        name: "test",
        about: "Run the project's runs and judge them by their checks",
        args: &[ArgRow { id: "runs", kind: ArgKind::Positional { required: false, many: true }, help: "the runs to test; all when none is named" }],
    },
    CommandRow {
        name: "new",
        about: "Create a project: the RC low-pass filter",
        args: &[ArgRow { id: "name", kind: ArgKind::Positional { required: true, many: false }, help: "the package name, and the directory created" }],
    },
    CommandRow {
        name: "clean",
        about: "Remove the project's target directory",
        args: &[ArgRow { id: "cache", kind: ArgKind::Switch, help: "also remove the cache of compiled designs" }],
    },
    CommandRow {
        name: "fmt",
        about: "Format the project's .msl files, keeping every comment",
        args: &[
            ArgRow { id: "files", kind: ArgKind::Positional { required: false, many: true }, help: "the files to format; every file under src/ and benches/ when none is named" },
            ArgRow { id: "check", kind: ArgKind::Switch, help: "write nothing; fail if a file would change" },
        ],
    },
];

/// The flag every command but `new` takes: the package directory, searched nowhere else.
const PROJECT: &str = "project";

/// What the command line asked for: one command and its arguments. `project` is the directory
/// given with `--project`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Check { project: Option<PathBuf>, target: Option<PathBuf> },
    Build { project: Option<PathBuf>, runs: Vec<String>, verbose: bool },
    Run { project: Option<PathBuf>, target: Option<String> },
    Test { project: Option<PathBuf>, runs: Vec<String> },
    New { name: String },
    Clean { project: Option<PathBuf>, cache: bool },
    Fmt { project: Option<PathBuf>, files: Vec<PathBuf>, check: bool },
}

impl Invocation {
    /// The command's name, as a user types it.
    pub fn name(&self) -> &'static str {
        match self {
            Invocation::Check { .. } => "check",
            Invocation::Build { .. } => "build",
            Invocation::Run { .. } => "run",
            Invocation::Test { .. } => "test",
            Invocation::New { .. } => "new",
            Invocation::Clean { .. } => "clean",
            Invocation::Fmt { .. } => "fmt",
        }
    }
}

/// The command line.
pub struct Cli;

impl Cli {
    /// The command tree: `piperine` with the seven commands of [`COMMANDS`].
    pub fn command() -> Command {
        let mut root = Command::new("piperine")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Run MSL projects: check, build, run, test, new, clean and fmt")
            .subcommand_required(true)
            .arg_required_else_help(true)
            .disable_help_subcommand(true);
        for row in COMMANDS {
            let mut command = Command::new(row.name).about(row.about);
            if row.name != "new" {
                command = command.arg(Arg::new(PROJECT).long(PROJECT).value_name("DIR").value_parser(PathBufValueParser::new()).help("the package directory; no search upward"));
            }
            for arg in row.args {
                command = command.arg(Cli::arg(arg));
            }
            root = root.subcommand(command);
        }
        root
    }

    fn arg(row: &ArgRow) -> Arg {
        let arg = Arg::new(row.id).help(row.help);
        match row.kind {
            ArgKind::Positional { required, many } => {
                let arg = arg.required(required);
                if many { arg.action(ArgAction::Append).num_args(0..) } else { arg.action(ArgAction::Set) }
            }
            ArgKind::Switch => arg.long(row.id).action(ArgAction::SetTrue),
        }
    }

    /// The invocation `args` spell (the first is the program name), or `clap`'s error: a usage
    /// error, or the text of `--help` or `--version`.
    pub fn parse<I, T>(args: I) -> Result<Invocation, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let matches = Cli::command().try_get_matches_from(args)?;
        let Some((name, m)) = matches.subcommand() else {
            return Err(Cli::command().error(clap::error::ErrorKind::MissingSubcommand, "a command is required"));
        };
        let project = || m.get_one::<PathBuf>(PROJECT).cloned();
        let text = |id: &str| m.get_one::<String>(id).cloned();
        let texts = |id: &str| -> Vec<String> { m.get_many::<String>(id).map(|v| v.cloned().collect()).unwrap_or_default() };
        let paths = |id: &str| texts(id).into_iter().map(PathBuf::from).collect();
        let flag = |id: &str| m.get_flag(id);
        Ok(match name {
            "check" => Invocation::Check { project: project(), target: text("target").map(PathBuf::from) },
            "build" => Invocation::Build { project: project(), runs: texts("runs"), verbose: flag("verbose") },
            "run" => Invocation::Run { project: project(), target: text("target") },
            "test" => Invocation::Test { project: project(), runs: texts("runs") },
            "new" => Invocation::New { name: text("name").unwrap_or_default() },
            "clean" => Invocation::Clean { project: project(), cache: flag("cache") },
            "fmt" => Invocation::Fmt { project: project(), files: paths("files"), check: flag("check") },
            other => return Err(Cli::command().error(clap::error::ErrorKind::InvalidSubcommand, format!("unknown command `{other}`"))),
        })
    }

    /// Runs `invocation`, writing data to `out` and diagnostics to `err`. A command whose task
    /// has not landed says so and fails.
    pub fn execute(invocation: &Invocation, _out: &mut dyn Write, err: &mut dyn Write) -> Exit {
        // a closed standard error leaves nothing to tell; the command fails either way
        let _ = writeln!(err, "{}: not built yet", invocation.name());
        Exit::Failed
    }
}


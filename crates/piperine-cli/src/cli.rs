//! `Cli`: the command tree, built from data with `clap`'s builder interface (no derive), its
//! parse into one [`Invocation`], and its execution.

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use clap::builder::PathBufValueParser;
use clap::{Arg, ArgAction, Command};

use piperine_api::Overrides;

use crate::commands::build::BuildCommand;
use crate::commands::check::CheckCommand;
use crate::commands::clean::CleanCommand;
use crate::commands::fmt::FmtCommand;
use crate::commands::info::InfoCommand;
use crate::commands::new::NewCommand;
use crate::commands::run::RunCommand;
use crate::commands::test::TestCommand;
use crate::exit::Exit;

/// Where a command writes its data. Shared, because the records of a run are written by the
/// solver's sink while the command goes on to write the table.
pub type Out = Rc<RefCell<dyn Write>>;

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
    /// `--<id> <value>`, one text; `value` names the placeholder of the usage line.
    Text { value: &'static str },
    /// `--<id> <value>`, a path.
    Path { value: &'static str },
    /// `--<id> <value>`, a real number.
    Real { value: &'static str },
    /// `--<id> <value>`, a whole number.
    Count { value: &'static str },
    /// `--<id> <value>`, a real number checked when the run is built, so a value that is no number
    /// is a `P0010` naming the flag, not a usage error.
    Checked { value: &'static str },
    /// `--<id> <value>`, given any number of times.
    Texts { value: &'static str },
    /// `--<id> <path>=<real>`, given any number of times.
    Param,
    /// `--<id> <path>=<value>`, given any number of times: a real, or text the param's type reads (PI-01 to PI-03).
    Given,
    /// `--<id> <name>=<value>`, given any number of times; the value is checked when the run is
    /// built, so a value that is no number is a `P0010` naming the entry, not a usage error.
    Setting,
}

/// The parsers of the typed values of a flag.
struct Values;

impl Values {
    /// A real number.
    fn real(text: &str) -> Result<f64, String> {
        text.parse::<f64>().map_err(|_| format!("`{text}` is not a number"))
    }

    /// A whole number.
    fn count(text: &str) -> Result<u32, String> {
        text.parse::<u32>().map_err(|_| format!("`{text}` is not a whole number"))
    }

    /// The path and the real value of a `<path>=<real>` flag's `text` (`--nodeset`), split at the last `=`; a flag is 1 or 0.
    fn path_real(text: &str) -> Result<(String, f64), String> {
        let (path, value) = Values::path_and_value(text)?;
        let real = match value.as_str() {
            "true" => 1.0,
            "false" => 0.0,
            other => Values::real(other)?,
        };
        Ok((path, real))
    }

    /// The path and the text of `--param`'s `text`, split at the last `=`: the text is kept whole, for the param's type to read
    /// (an integer literal in a radix, a variant name, `None`, a number, a flag; PI-01 to PI-03, D24).
    fn param(text: &str) -> Result<(String, String), String> {
        Values::path_and_value(text)
    }

    /// `<path>=<value>` split at the last `=`; a text with no `=` or no path is refused.
    fn path_and_value(text: &str) -> Result<(String, String), String> {
        let (path, value) = text.rsplit_once('=').ok_or_else(|| format!("`{text}` is not <path>=<value>"))?;
        if path.is_empty() {
            return Err(format!("`{text}` names no path"));
        }
        Ok((path.to_string(), value.to_string()))
    }

    /// The name and the text of `--simparam`'s `text`, split at the first `=`.
    fn setting(text: &str) -> Result<(String, String), String> {
        let (name, value) = text.split_once('=').ok_or_else(|| format!("`{text}` is not <name>=<value>"))?;
        if name.is_empty() {
            return Err(format!("`{text}` names no session parameter"));
        }
        Ok((name.to_string(), value.to_string()))
    }
}

/// One command: its name, its help line and its arguments.
struct CommandRow {
    name: &'static str,
    about: &'static str,
    args: &'static [ArgRow],
}

/// The eight commands (CLI-01 AC 5, and `info`), in the order `--help` lists them.
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
        args: &[
            ArgRow { id: "target", kind: ArgKind::Positional { required: false, many: false }, help: "a run name, or a .msl file" },
            ArgRow { id: "bench", kind: ArgKind::Path { value: "FILE" }, help: "the bench file; the root crate is this file" },
            ArgRow { id: "top", kind: ArgKind::Text { value: "MODULE" }, help: "the module without ports to elaborate" },
            ArgRow { id: "analysis", kind: ArgKind::Text { value: "ANALYSIS" }, help: "op, dc, ac, noise, transient, tf, pz, sens or pss" },
            ArgRow { id: "stop", kind: ArgKind::Real { value: "SECONDS" }, help: "transient: the end time" },
            ArgRow { id: "max-step", kind: ArgKind::Real { value: "SECONDS" }, help: "transient: the largest step" },
            ArgRow { id: "print-step", kind: ArgKind::Real { value: "SECONDS" }, help: "transient: the spacing of the printed rows" },
            ArgRow { id: "sweep", kind: ArgKind::Text { value: "PATH" }, help: "dc: the source to sweep" },
            ArgRow { id: "from", kind: ArgKind::Real { value: "VALUE" }, help: "the first value of a sweep, or the first time kept" },
            ArgRow { id: "to", kind: ArgKind::Real { value: "VALUE" }, help: "the last value of a sweep" },
            ArgRow { id: "step", kind: ArgKind::Real { value: "VALUE" }, help: "dc: the sweep step" },
            ArgRow { id: "per-decade", kind: ArgKind::Count { value: "N" }, help: "ac, noise: points per decade" },
            ArgRow { id: "points", kind: ArgKind::Count { value: "N" }, help: "ac, noise: points in all" },
            ArgRow { id: "print", kind: ArgKind::Texts { value: "SIGNAL" }, help: "a signal to print (repeatable)" },
            ArgRow { id: "stream", kind: ArgKind::Texts { value: "STREAM" }, help: "a record stream to print (repeatable)" },
            ArgRow { id: "param", kind: ArgKind::Given, help: "set a parameter before the run: <path>=<value> (repeatable)" },
            ArgRow { id: "backend", kind: ArgKind::Text { value: "BACKEND" }, help: "the backend: rust or interp" },
            ArgRow { id: "csv", kind: ArgKind::Path { value: "FILE" }, help: "write the table as CSV to this file" },
            ArgRow { id: "records", kind: ArgKind::Path { value: "FILE" }, help: "write the records as JSON Lines to this file" },
            ArgRow { id: "plot", kind: ArgKind::Path { value: "FILE" }, help: "write the plot as SVG to this file" },
            ArgRow { id: "all-checks", kind: ArgKind::Switch, help: "print the passing checks too; a failing check always prints" },
            ArgRow { id: "reltol", kind: ArgKind::Real { value: "VALUE" }, help: "the relative tolerance of the solver" },
            ArgRow { id: "abstol", kind: ArgKind::Real { value: "AMPERES" }, help: "the absolute tolerance of a flow" },
            ArgRow { id: "vntol", kind: ArgKind::Real { value: "VOLTS" }, help: "the absolute tolerance of a potential" },
            ArgRow { id: "trtol", kind: ArgKind::Real { value: "VALUE" }, help: "the factor on the truncation error a transient accepts" },
            ArgRow { id: "temp", kind: ArgKind::Real { value: "CELSIUS" }, help: "the temperature of the run, in degrees Celsius" },
            ArgRow { id: "tnom", kind: ArgKind::Checked { value: "CELSIUS" }, help: "the nominal temperature of the models' parameters, in degrees Celsius (default 27)" },
            ArgRow { id: "resolution", kind: ArgKind::Real { value: "SECONDS" }, help: "the tick of the digital engine, in seconds" },
            ArgRow { id: "maxrounds", kind: ArgKind::Count { value: "N" }, help: "the bound on the rounds of an operating point" },
            ArgRow { id: "maxmicro", kind: ArgKind::Count { value: "N" }, help: "the bound on the microsteps of one instant" },
            ArgRow { id: "max-steps", kind: ArgKind::Count { value: "N" }, help: "the budget of attempts a transient may make (default 2000000)" },
            ArgRow { id: "nodeset", kind: ArgKind::Param, help: "seed an operating point's first iterate: <node>=<value> (repeatable)" },
            ArgRow { id: "simparam", kind: ArgKind::Setting, help: "set a session parameter a design reads with simparam(): <name>=<value> (repeatable)" },
            ArgRow { id: "input", kind: ArgKind::Text { value: "SOURCE" }, help: "tf, pz: the voltage source that excites; noise: the instance the noise is referred to" },
            ArgRow { id: "output", kind: ArgKind::Text { value: "NODE" }, help: "tf, pz, noise: the node that is measured (noise: also v(a, b) or i(instance))" },
            ArgRow { id: "wrt", kind: ArgKind::Texts { value: "PARAM" }, help: "sens: a param to differentiate by (repeatable)" },
            ArgRow { id: "period", kind: ArgKind::Real { value: "SECONDS" }, help: "pss: the period of the drive" },
            ArgRow { id: "tstab", kind: ArgKind::Real { value: "SECONDS" }, help: "pss: the length of the transient that comes before the orbit (default 0)" },
        ],
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
    CommandRow {
        name: "info",
        about: "List the params, ports and record streams of a run's design, simulating nothing",
        args: &[ArgRow { id: "target", kind: ArgKind::Positional { required: false, many: false }, help: "the run to describe, or a lone .msl file; the only run when none is named" }],
    },
];

/// The flag every command but `new` takes: the package directory, searched nowhere else.
const PROJECT: &str = "project";

/// What the command line asked for: one command and its arguments. `project` is the directory
/// given with `--project`.
#[derive(Debug, Clone, PartialEq)]
pub enum Invocation {
    Check { project: Option<PathBuf>, target: Option<PathBuf> },
    Build { project: Option<PathBuf>, runs: Vec<String>, verbose: bool },
    /// `refused` is the `P0010` of a flag value that is no number, raised when the command runs.
    Run { project: Option<PathBuf>, target: Option<String>, overrides: Overrides, refused: Option<String> },
    Test { project: Option<PathBuf>, runs: Vec<String> },
    New { name: String },
    Clean { project: Option<PathBuf>, cache: bool },
    Fmt { project: Option<PathBuf>, files: Vec<PathBuf>, check: bool },
    Info { project: Option<PathBuf>, target: Option<String> },
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
            Invocation::Info { .. } => "info",
        }
    }
}

/// The command line.
pub struct Cli;

impl Cli {
    /// The command tree: `piperine` with the eight commands of [`COMMANDS`].
    pub fn command() -> Command {
        let mut root = Command::new("piperine")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Run MSL projects: check, build, run, test, new, clean, fmt and info")
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
            ArgKind::Text { value } => arg.long(row.id).value_name(value).action(ArgAction::Set),
            ArgKind::Path { value } => arg.long(row.id).value_name(value).value_parser(PathBufValueParser::new()).action(ArgAction::Set),
            ArgKind::Real { value } => arg.long(row.id).value_name(value).value_parser(Values::real).allow_negative_numbers(true).action(ArgAction::Set),
            ArgKind::Checked { value } => arg.long(row.id).value_name(value).allow_negative_numbers(true).action(ArgAction::Set),
            ArgKind::Count { value } => arg.long(row.id).value_name(value).value_parser(Values::count).action(ArgAction::Set),
            ArgKind::Texts { value } => arg.long(row.id).value_name(value).action(ArgAction::Append),
            ArgKind::Param => arg.long(row.id).value_name("PATH=VALUE").value_parser(Values::path_real).action(ArgAction::Append),
            ArgKind::Given => arg.long(row.id).value_name("PATH=VALUE").value_parser(Values::param).action(ArgAction::Append),
            ArgKind::Setting => arg.long(row.id).value_name("NAME=VALUE").value_parser(Values::setting).action(ArgAction::Append),
        }
    }

    /// The settings the flags of `run` gave, one field per key of a run table.
    fn overrides(m: &clap::ArgMatches) -> Overrides {
        let text = |id: &str| m.get_one::<String>(id).cloned();
        let path = |id: &str| m.get_one::<PathBuf>(id).cloned();
        let real = |id: &str| m.get_one::<f64>(id).copied();
        let count = |id: &str| m.get_one::<u32>(id).copied();
        let texts = |id: &str| -> Vec<String> { m.get_many::<String>(id).map(|v| v.cloned().collect()).unwrap_or_default() };
        Overrides {
            bench: path("bench"),
            top: text("top"),
            analysis: text("analysis"),
            stop: real("stop"),
            max_step: real("max-step"),
            print_step: real("print-step"),
            sweep: text("sweep"),
            from: real("from"),
            to: real("to"),
            step: real("step"),
            per_decade: count("per-decade"),
            points: count("points"),
            print: texts("print"),
            streams: texts("stream"),
            params: Vec::new(),
            param_texts: Cli::given_params(m),
            backend: text("backend"),
            csv: path("csv"),
            records: path("records"),
            plot: path("plot"),
            all_checks: m.get_flag("all-checks").then_some(true),
            reltol: real("reltol"),
            abstol: real("abstol"),
            vntol: real("vntol"),
            trtol: real("trtol"),
            temp: real("temp"),
            tnom: text("tnom").and_then(|value| value.parse().ok()),
            resolution: real("resolution"),
            maxrounds: count("maxrounds"),
            maxmicro: count("maxmicro"),
            max_steps: count("max-steps"),
            input: text("input"),
            output: text("output"),
            wrt: texts("wrt"),
            period: real("period"),
            tstab: real("tstab"),
            nodeset: m.get_many::<(String, f64)>("nodeset").map(|v| v.cloned().collect()).unwrap_or_default(),
            simparam: Cli::simparam_entries(m).0,
        }
    }

    /// The fault (`P0010`) of the first flag of `m` checked when the run is built whose value is no
    /// number, naming it: a `--simparam` entry, else `--tnom`.
    fn refusal(m: &clap::ArgMatches) -> Option<String> {
        Cli::simparam_entries(m).1.or_else(|| {
            let value = m.get_one::<String>("tnom")?;
            Values::real(value).err().map(|why| format!("`--tnom {value}`: {why}"))
        })
    }

    /// The `--param` entries of `m`, in the order given.
    fn given_params(m: &clap::ArgMatches) -> Vec<(String, String)> {
        m.get_many::<(String, String)>("param").map(|v| v.cloned().collect()).unwrap_or_default()
    }

    /// The `--simparam` entries of `m` whose value is a number, in order, and the fault of the
    /// first whose value is none (`P0010`, naming the entry).
    fn simparam_entries(m: &clap::ArgMatches) -> (Vec<(String, f64)>, Option<String>) {
        let mut readable = Vec::new();
        let mut refused = None;
        for (name, text) in m.get_many::<(String, String)>("simparam").into_iter().flatten() {
            match Values::real(text) {
                Ok(value) => readable.push((name.clone(), value)),
                Err(why) => {
                    refused.get_or_insert_with(|| format!("`--simparam {name}={text}`: {why}"));
                }
            }
        }
        (readable, refused)
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
            "run" => Invocation::Run { project: project(), target: text("target"), overrides: Cli::overrides(m), refused: Cli::refusal(m) },
            "test" => Invocation::Test { project: project(), runs: texts("runs") },
            "new" => Invocation::New { name: text("name").unwrap_or_default() },
            "clean" => Invocation::Clean { project: project(), cache: flag("cache") },
            "fmt" => Invocation::Fmt { project: project(), files: paths("files"), check: flag("check") },
            "info" => Invocation::Info { project: project(), target: text("target") },
            other => return Err(Cli::command().error(clap::error::ErrorKind::InvalidSubcommand, format!("unknown command `{other}`"))),
        })
    }

    /// Runs `invocation`, writing data to `out` and diagnostics to `err`.
    pub fn execute(invocation: &Invocation, out: &Out, err: &mut dyn Write) -> Exit {
        match invocation {
            Invocation::Build { project, runs, verbose } => BuildCommand { project: project.clone(), runs: runs.clone(), verbose: *verbose }.execute(out, err),
            Invocation::Clean { project, cache } => CleanCommand { project: project.clone(), cache: *cache }.execute(out, err),
            Invocation::Check { project, target } => CheckCommand { project: project.clone(), target: target.clone() }.execute(out, err),
            Invocation::New { name } => NewCommand { name: name.clone() }.execute(out, err),
            Invocation::Test { project, runs } => TestCommand { project: project.clone(), runs: runs.clone() }.execute(out, err),
            Invocation::Run { project, target, overrides, refused } => RunCommand { project: project.clone(), target: target.clone(), overrides: overrides.clone(), refused: refused.clone() }.execute(out, err),
            Invocation::Info { project, target } => InfoCommand { project: project.clone(), target: target.clone() }.execute(out, err),
            Invocation::Fmt { project, files, check } => FmtCommand { project: project.clone(), files: files.clone(), check: *check }.execute(out, err),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_built() {
        // CLI-01 AC 5: each of the eight commands runs its own code. Given a project directory
        // that does not exist (and `new` a name the manifest rule refuses, so nothing is
        // written), each fails with its own error, exit 1, and none says it is not built.
        let nowhere = std::env::temp_dir().join(format!("piperine-cli-no-such-project-{}", std::process::id()));
        assert!(!nowhere.exists());
        let nowhere = nowhere.to_string_lossy().into_owned();
        let names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
        assert_eq!(names, ["check", "build", "run", "test", "new", "clean", "fmt", "info"]);
        for name in names {
            let args: Vec<&str> = if name == "new" { vec!["piperine", name, "a/b"] } else { vec!["piperine", name, "--project", &nowhere] };
            let invocation = Cli::parse(&args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
            assert_eq!(invocation.name(), name);
            let out: Out = Rc::new(RefCell::new(Vec::<u8>::new()));
            let mut err = Vec::new();
            let exit = Cli::execute(&invocation, &out, &mut err);
            let err = String::from_utf8_lossy(&err);
            assert_eq!(exit, Exit::Failed, "{name}: {err}");
            assert!(err.starts_with("error") || err.contains("\nerror: "), "{name} reports its own error:\n{err}");
            assert!(!err.contains("not built yet"), "{name}: {err}");
        }
    }

    #[test]
    fn every_run_key_has_its_flag() {
        // CLI-03 AC 1 and 2: each key of a run table is a flag of `run`, spelled with dashes
        // (`--print` and `--stream`, `--param` for the three list keys), and no flag besides
        // those and `--project` exists (`--help` is clap's, added when the command is built)
        let flags = [
            "bench", "top", "analysis", "stop", "max-step", "print-step", "sweep", "from", "to", "step", "per-decade", "points", "print", "stream", "param", "backend", "csv", "records", "plot", "all-checks",
            "reltol", "abstol", "vntol", "trtol", "temp", "resolution", "maxrounds", "maxmicro", "max-steps", "nodeset", "input", "output", "wrt", "period", "tstab", "tnom", "simparam",
        ];
        let command = Cli::command();
        let run = command.find_subcommand("run").expect("a `run` command");
        let mut have: Vec<String> = run.get_arguments().filter_map(|a| a.get_long().map(str::to_string)).collect();
        have.sort();
        let mut want: Vec<String> = flags.iter().chain(&["project"]).map(|f| f.to_string()).collect();
        want.sort();
        assert_eq!(have, want);
        // each flag reaches its field, as the spelled-out value (repeatable flags in order)
        let args = [
            "piperine", "run", "x", "--bench", "b.msl", "--top", "Top", "--analysis", "transient", "--stop", "1e-3", "--max-step", "1e-6", "--print-step", "1e-5", "--sweep", "v1.dc", "--from", "-1", "--to", "5",
            "--step", "0.5", "--per-decade", "10", "--points", "20", "--print", "a", "--print", "b", "--stream", "s1", "--stream", "s2", "--param", "r1.r=2000", "--param", "c1.c=1e-9", "--backend", "interp",
            "--csv", "o.csv", "--records", "r.jsonl", "--plot", "p.svg", "--all-checks",
            "--reltol", "1e-4", "--abstol", "1e-13", "--vntol", "2e-6", "--trtol", "5", "--temp", "-40", "--tnom", "50", "--resolution", "1e-12", "--maxrounds", "30", "--maxmicro", "500", "--max-steps", "4000000", "--nodeset", "Top.a=1.5",
            "--nodeset", "Top.b=0", "--input", "v1", "--output", "Top.out", "--wrt", "r1.r", "--wrt", "r2.r", "--period", "1e-3", "--tstab", "5e-3",
            "--simparam", "gain=2", "--simparam", "bias=0.5",
        ];
        let Ok(Invocation::Run { target, overrides, .. }) = Cli::parse(args) else { panic!("the flags parse as a run") };
        assert_eq!(target.as_deref(), Some("x"));
        let want = Overrides {
            bench: Some("b.msl".into()),
            top: Some("Top".into()),
            analysis: Some("transient".into()),
            stop: Some(1e-3),
            max_step: Some(1e-6),
            print_step: Some(1e-5),
            sweep: Some("v1.dc".into()),
            from: Some(-1.0),
            to: Some(5.0),
            step: Some(0.5),
            per_decade: Some(10),
            points: Some(20),
            print: vec!["a".into(), "b".into()],
            streams: vec!["s1".into(), "s2".into()],
            params: vec![],
            param_texts: vec![("r1.r".into(), "2000".into()), ("c1.c".into(), "1e-9".into())],
            backend: Some("interp".into()),
            csv: Some("o.csv".into()),
            records: Some("r.jsonl".into()),
            plot: Some("p.svg".into()),
            all_checks: Some(true),
            reltol: Some(1e-4),
            abstol: Some(1e-13),
            vntol: Some(2e-6),
            trtol: Some(5.0),
            temp: Some(-40.0),
            tnom: Some(50.0),
            resolution: Some(1e-12),
            maxrounds: Some(30),
            maxmicro: Some(500),
            max_steps: Some(4_000_000),
            input: Some("v1".into()),
            output: Some("Top.out".into()),
            wrt: vec!["r1.r".into(), "r2.r".into()],
            period: Some(1e-3),
            tstab: Some(5e-3),
            nodeset: vec![("Top.a".into(), 1.5), ("Top.b".into(), 0.0)],
            simparam: vec![("gain".into(), 2.0), ("bias".into(), 0.5)],
        };
        assert_eq!(overrides, want);
        // a flag not given leaves its field alone, and a bad value is a usage error
        let Ok(Invocation::Run { overrides, .. }) = Cli::parse(["piperine", "run"]) else { panic!("no flag parses") };
        assert_eq!(overrides, Overrides::default());
        for bad in [&["--stop", "soon"][..], &["--points", "1.5"], &["--param", "r1.r"], &["--param", "=3"], &["--maxrounds", "1.5"], &["--temp", "hot"]] {
            let args: Vec<&str> = ["piperine", "run"].iter().chain(bad).copied().collect();
            assert!(Cli::parse(args).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_param_is_kept_as_text_for_its_type() {
        // PI-01 to PI-03 (D24): `--param` keeps every value as the text it was written, a number included: the param's type reads
        // it when the session opens (an integer literal in a radix, a variant name, `None`, a number, a flag).
        let args = ["piperine", "run", "--param", "a.x=1e-9", "--param", "a.n=0x1F", "--param", "a.md=Slow", "--param", "a.rs=None", "--param", "a.f=true"];
        let Ok(Invocation::Run { overrides, .. }) = Cli::parse(args) else { panic!("the params parse") };
        assert!(overrides.params.is_empty(), "no `--param` is read as a real before its type is known");
        let texts: Vec<(&str, &str)> = overrides.param_texts.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
        assert_eq!(texts, [("a.x", "1e-9"), ("a.n", "0x1F"), ("a.md", "Slow"), ("a.rs", "None"), ("a.f", "true")]);
    }

    #[test]
    fn a_nodeset_flag_without_equals_exits_2() {
        // HII-01 AC 2: `--nodeset` is `<node>=<value>`, as `--param` is; text with no `=`, no node
        // or no number is a usage error, exit 2
        for bad in ["Top.a", "=1.0", "Top.a=high"] {
            let error = Cli::parse(["piperine", "run", "--nodeset", bad]).err().unwrap_or_else(|| panic!("`{bad}` parses"));
            assert_eq!(error.exit_code(), 2, "{bad}");
            assert!(error.to_string().contains("--nodeset"), "{bad}: {error}");
        }
        assert!(Cli::parse(["piperine", "run", "--nodeset", "Top.a=1.0"]).is_ok());
    }
}

use clap::{Parser, Subcommand};

pub mod commands;

#[derive(Parser, Debug)]
#[command(name = "piperine")]
#[command(version)]
#[command(about = "The Piperine Verilog-AMS toolchain", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Parse and validate a Verilog-AMS file or project
    Check {
        /// The file to check
        file: Option<String>,
    },
    /// Format Verilog-AMS code
    Fmt {
        /// The file to format
        file: Option<String>,
    },
    /// Elaborate and build the design
    Build {
        /// The file to build
        file: Option<String>,
    },
    /// Run a simulation explicitly, a Python script, or an interactive REPL
    Run {
        /// The file to run: `foo.py` (Python
        /// script), or `foo.phdl` (loaded into the interactive REPL with `-i`)
        entry: Option<String>,
        /// The file containing the entry point (defaults to src/main.phdl)
        #[arg(long, short)]
        file: Option<String>,
        /// Start an interactive Python REPL with `import piperine` ready.
        /// With a `.phdl` arg, pre-loads it as `design`.
        #[arg(short = 'i', long)]
        interactive: bool,
    },
    /// Run the project's Python testbenches (`*_tb.py`)
    Test {
        /// List discovered testbenches instead of running them
        #[arg(long, short)]
        list: bool,
        /// A single testbench file to run; defaults to every `*_tb.py` under the project
        file: Option<String>,
    },
    /// Create a new piperine project
    New {
        /// Project name
        name: Option<String>,
    },
    /// Clean build artifacts
    Clean,
    /// Add a new dependency to Piperine.toml
    Add {
        /// Name of the package
        name: String,
        /// Git URL
        #[arg(long)]
        git: Option<String>,
        /// Version mapping (maps to release/v<version>)
        #[arg(long)]
        version: Option<String>,
        /// Specific git branch
        #[arg(long)]
        branch: Option<String>,
        /// Specific git revision
        #[arg(long)]
        rev: Option<String>,
        /// Local path
        #[arg(long)]
        path: Option<String>,
    },
    /// Remove a dependency from Piperine.toml
    Remove {
        /// Name of the package
        name: String,
    },
    /// Display dependency tree
    Tree,
    /// Inspect loaded plugins (SPEC Part VI)
    Plugin {
        #[command(subcommand)]
        cmd: PluginCmd,
    },
    /// A plugin-contributed script: `piperine <script> [args...]`
    /// (SPEC Part VI §10) — dispatched when no builtin command matches.
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum PluginCmd {
    /// List loaded plugins and their contributions
    List,
}

pub fn execute() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => {
            commands::check::execute(file);
        }
        Commands::Fmt { file } => {
            commands::fmt::execute(file);
        }
        Commands::Build { file } => {
            commands::build::execute(file);
        }
        Commands::Run {
            entry,
            file,
            interactive,
        } => {
            commands::run::execute(entry, file, interactive);
        }
        Commands::Test { list, file } => {
            commands::test::execute(list, file);
        }
        Commands::New { name } => {
            commands::new::execute(name);
        }
        Commands::Clean => {
            commands::clean::execute();
        }
        Commands::Add {
            name,
            git,
            version,
            branch,
            rev,
            path,
        } => {
            commands::add::execute(name, git, version, branch, rev, path);
        }
        Commands::Remove { name } => {
            commands::remove::execute(name);
        }
        Commands::Tree => {
            commands::tree::execute();
        }
        Commands::Plugin { cmd } => match cmd {
            PluginCmd::List => commands::plugin::list(),
        },
        Commands::External(args) => {
            commands::plugin::script(args);
        }
    }
}

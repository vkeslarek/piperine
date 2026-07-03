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
    /// Run a simulation
    Run {
        /// The file to run
        file: Option<String>,
    },
    /// Run `bench` entry points (SPEC_BENCH.md)
    Test {
        /// The file to test; defaults to every `.phdl` under `src/`
        file: Option<String>,
    },
    /// Create a new piperine project
    New {
        /// Project name
        name: Option<String>,
    },
    /// Clean build artifacts
    Clean,
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
        Commands::Run { file } => {
            commands::run::execute(file);
        }
        Commands::Test { file } => {
            commands::test::execute(file);
        }
        Commands::New { name } => {
            commands::new::execute(name);
        }
        Commands::Clean => {
            commands::clean::execute();
        }
    }
}

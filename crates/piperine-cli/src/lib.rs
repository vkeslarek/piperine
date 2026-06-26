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
    /// Parse and validate a Verilog-AMS file
    Check {
        /// The file to check
        file: String,
    },
    /// Format Verilog-AMS code
    Fmt {
        /// The file to format
        file: String,
    },
    /// Elaborate and build the design
    Build {
        /// The file to build
        file: String,
    },
    /// Run a simulation
    Run {
        /// The file to run
        file: String,
    },
    /// Run automated testbenches
    Test {
        /// Directory containing tests
        #[arg(default_value = "tests")]
        dir: String,
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
        Commands::Test { dir } => {
            commands::test::execute(dir);
        }
        Commands::New { name } => {
            commands::new::execute(name);
        }
        Commands::Clean => {
            commands::clean::execute();
        }
    }
}

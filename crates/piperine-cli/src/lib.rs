use clap::{Parser, Subcommand};

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
        name: String,
    },
    /// Clean build artifacts
    Clean,
}

pub fn execute() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => {
            println!("Checking file: {}", file);
            // TODO: call parser
        }
        Commands::Fmt { file } => {
            println!("Formatting file: {}", file);
            // TODO: call formatter
        }
        Commands::Build { file } => {
            println!("Building file: {}", file);
            // TODO: call compiler/elaborator
        }
        Commands::Run { file } => {
            println!("Running simulation for: {}", file);
            // TODO: call simulator
        }
        Commands::Test { dir } => {
            println!("Running tests in: {}", dir);
            // TODO: test runner
        }
        Commands::New { name } => {
            println!("Creating new project: {}", name);
            // TODO: project scaffolding
        }
        Commands::Clean => {
            println!("Cleaning target directory...");
            // TODO: remove artifacts
        }
    }
}

//! # piperine-cli
//!
//! The command line: a shell over `piperine-api` (MD-20). It parses the arguments with `clap`'s
//! builder interface, calls the host library, lays out the text a user reads and returns an
//! exit code. Every rule that decides what a user sees lives in `piperine-api`.
//!
//! - [`cli`]: the command tree, its parse into one [`cli::Invocation`] and its execution.
//! - [`exit`]: the three exit codes.

pub mod cli;
pub mod exit;

pub use cli::{Cli, Invocation};
pub use exit::Exit;

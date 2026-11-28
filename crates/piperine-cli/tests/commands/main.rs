//! The command line of the `piperine` binary, run as a child process: the usage and its errors, `piperine run` and its output options, the four analyses added by the host library, `piperine info`, the project commands (`check`, `build`, `test`, `new`, `clean`, `fmt`) and the integrated package. Each former target is a module of this one.

mod analyses;
mod commands;
mod info;
mod integrated;
mod options;
mod process;
mod project_directories;
mod run;
mod usage;

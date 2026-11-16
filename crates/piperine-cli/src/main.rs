//! The `piperine` binary: parse the command line, run the command, exit with its code.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use piperine_cli::cli::Out;
use piperine_cli::Cli;

fn main() {
    let code = match Cli::parse(std::env::args_os()) {
        Ok(invocation) => {
            let out: Out = Rc::new(RefCell::new(std::io::stdout()));
            let exit = Cli::execute(&invocation, &out, &mut std::io::stderr().lock());
            exit.code()
        }
        Err(error) => {
            // `--help` and `--version` are clap "errors" printed on standard output with code 0;
            // a usage error goes to standard error with code 2
            let _ = error.print();
            let _ = std::io::stdout().flush();
            error.exit_code()
        }
    };
    std::process::exit(code);
}

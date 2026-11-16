//! The `piperine` binary: parse the command line, run the command, exit with its code.

use std::io::Write;

use piperine_cli::Cli;

fn main() {
    let code = match Cli::parse(std::env::args_os()) {
        Ok(invocation) => {
            let (stdout, stderr) = (std::io::stdout(), std::io::stderr());
            let exit = Cli::execute(&invocation, &mut stdout.lock(), &mut stderr.lock());
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

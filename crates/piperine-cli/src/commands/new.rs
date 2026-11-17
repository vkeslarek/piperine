//! `NewCommand`: `piperine new <name>` (CLI-17). Writes the RC low-pass project: the four
//! files of `examples/rc`, embedded in the binary, with the package name replaced. Nothing is
//! written when the name is refused or the directory exists.

use std::io::Write;
use std::path::Path;

use piperine_lang::pom::is_crate_name;

use crate::cli::Out;
use crate::exit::Exit;

/// The name `examples/rc` carries, replaced in its manifest and in the bench's `use`.
const TEMPLATE_NAME: &str = "rc";

/// The files of `examples/rc`, by path under the project.
const TEMPLATE: [(&str, &str); 4] = [
    ("Piperine.toml", include_str!("../../../../examples/rc/Piperine.toml")),
    ("src/lib.msl", include_str!("../../../../examples/rc/src/lib.msl")),
    ("benches/step.msl", include_str!("../../../../examples/rc/benches/step.msl")),
    (".gitignore", include_str!("../../../../examples/rc/.gitignore")),
];

/// `piperine new`: the package name, which is also the directory created.
pub struct NewCommand {
    pub name: String,
}

impl NewCommand {
    /// Exit 0 with `created <name>`; exit 1, writing nothing, for a name the manifest rule
    /// forbids or a directory that exists.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        match self.create() {
            Ok(()) => {
                let mut out = out.borrow_mut();
                match writeln!(out, "created {}", self.name).and_then(|()| out.flush()) {
                    Ok(()) => Exit::Done,
                    Err(error) => {
                        let _ = writeln!(err, "error: cannot write to standard output: {error}");
                        Exit::Failed
                    }
                }
            }
            Err(message) => {
                // a closed standard error leaves nothing to tell; the command fails either way
                let _ = writeln!(err, "error: {message}");
                Exit::Failed
            }
        }
    }

    fn create(&self) -> Result<(), String> {
        if !is_crate_name(&self.name) {
            let message = format!("`{}` is not a package name: a package name is one identifier (letters, digits and `_`, not starting with a digit), not a keyword and not `std`", self.name);
            return Err(message);
        }
        let dir = Path::new(&self.name);
        if dir.exists() {
            return Err(format!("`{}` already exists: `new` never writes into an existing path", self.name));
        }
        for (path, text) in TEMPLATE {
            let file = dir.join(path);
            let io = |error: std::io::Error| format!("{}: {error}", file.display());
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            let text = match path {
                "Piperine.toml" => text.replacen(&format!("name = \"{TEMPLATE_NAME}\""), &format!("name = \"{}\"", self.name), 1),
                "benches/step.msl" => text.replacen(&format!("use {TEMPLATE_NAME}::"), &format!("use {}::", self.name), 1),
                _ => text.to_string(),
            };
            std::fs::write(&file, text).map_err(io)?;
        }
        Ok(())
    }
}

//! A `cargo test` command as the proof lines of `.specs/features/` write it.

/// The pieces of one `cargo test ...` command that name what it runs.
pub struct CargoTestCommand {
    tokens: Vec<String>,
    cargo_at: usize,
    arguments_end: usize,
}

impl CargoTestCommand {
    /// Parses a command line; `None` when it holds no `cargo test`.
    pub fn parse(command: &str) -> Option<CargoTestCommand> {
        let tokens: Vec<String> = command.split_whitespace().map(str::to_string).collect();
        let cargo_at = tokens.windows(2).position(|pair| pair[0] == "cargo" && pair[1] == "test")?;
        let arguments_end = tokens[cargo_at + 2..].iter().position(|t| matches!(t.as_str(), "--" | "|" | "&&" | ";" | "||")).map_or(tokens.len(), |at| at + cargo_at + 2);
        Some(CargoTestCommand { tokens, cargo_at, arguments_end })
    }

    fn arguments(&self) -> &[String] {
        &self.tokens[self.cargo_at + 2..self.arguments_end]
    }

    fn value_after(&self, flags: &[&str]) -> Option<(usize, &str)> {
        let arguments = self.arguments();
        let at = arguments.iter().position(|a| flags.contains(&a.as_str()))?;
        arguments.get(at + 1).map(|value| (self.cargo_at + 2 + at + 1, value.as_str()))
    }

    /// The package the command runs: `-p`, or the root package `piperine` when it names none.
    /// `None` for a `--workspace` or `--all` run.
    pub fn package(&self) -> Option<&str> {
        if self.arguments().iter().any(|a| a == "--workspace" || a == "--all") {
            return None;
        }
        Some(self.value_after(&["-p", "--package"]).map_or("piperine", |(_, value)| value))
    }

    /// The integration-test target named by `--test`, when there is one.
    pub fn target(&self) -> Option<&str> {
        self.value_after(&["--test"]).map(|(_, value)| value)
    }

    /// Every integration-test target named by a `--test`, in order.
    pub fn targets(&self) -> Vec<&str> {
        let arguments = self.arguments();
        arguments.iter().enumerate().filter(|(_, a)| a.as_str() == "--test").filter_map(|(at, _)| arguments.get(at + 1)).map(String::as_str).collect()
    }

    /// The command with its `-p` and `--test` replaced and, when `module` is given, its test filter
    /// prefixed with `module::` (added when the command had none).
    pub fn retargeted(&self, package: &str, target: &str, module: Option<&str>) -> String {
        let mut tokens = self.tokens.clone();
        if let Some((at, _)) = self.value_after(&["-p", "--package"]) {
            tokens[at] = package.to_string();
        }
        let Some((target_at, _)) = self.value_after(&["--test"]) else { return tokens.join(" ") };
        tokens[target_at] = target.to_string();
        if let Some(module) = module {
            let filter_at = target_at + 1;
            let has_filter = filter_at < self.arguments_end && !tokens[filter_at].starts_with('-');
            if has_filter {
                tokens[filter_at] = format!("{module}::{}", tokens[filter_at]);
            } else {
                tokens.insert(filter_at, format!("{module}::"));
            }
        }
        tokens.join(" ")
    }

    /// The command with its `-p` value replaced.
    pub fn with_package(&self, package: &str) -> String {
        let mut tokens = self.tokens.clone();
        if let Some((at, _)) = self.value_after(&["-p", "--package"]) {
            tokens[at] = package.to_string();
        }
        tokens.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_names_its_package_and_target() {
        let command = CargoTestCommand::parse("env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test source_io -- --nocapture").expect("a cargo test");
        assert_eq!((command.package(), command.target()), (Some("piperine-parity"), Some("source_io")));
        let root = CargoTestCommand::parse("cargo test --test host_ii").expect("a cargo test");
        assert_eq!((root.package(), root.target()), (Some("piperine"), Some("host_ii")));
        assert_eq!(CargoTestCommand::parse("cargo test --workspace").expect("a cargo test").package(), None);
        assert!(CargoTestCommand::parse("cargo build -p x").is_none());
    }

    #[test]
    fn retargeting_prefixes_the_filter_with_the_module() {
        let plain = CargoTestCommand::parse("cargo test -p a --test old").expect("a cargo test");
        assert_eq!(plain.retargeted("a", "merged", Some("old")), "cargo test -p a --test merged old::");
        let filtered = CargoTestCommand::parse("cargo test -p a --test old inventory_guard").expect("a cargo test");
        assert_eq!(filtered.retargeted("b", "merged", Some("old")), "cargo test -p b --test merged old::inventory_guard");
    }
}

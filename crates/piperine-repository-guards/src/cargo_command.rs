//! The `cargo` commands the proof and build lines of `.specs/features/` write: any command line's
//! package flags, and a `cargo test` command's package and targets. A command may sit inside a
//! command substitution (`o=$(cargo test -p a --test t 2>&1); ...`).

/// How one whitespace-separated token of a shell command line is read.
struct ShellToken;

impl ShellToken {
    /// The shell operators that end a command.
    const OPERATORS: [&'static str; 5] = ["--", "|", "&&", ";", "||"];

    /// The token without an opening command substitution or subshell before it (`o=$(cargo` is
    /// `cargo`), without the closing `)` and `;` after it, and without its quotes (`'parity_*'`).
    fn word(token: &str) -> &str {
        let opened = if token.starts_with('(') || token.contains("$(") { token.rsplit('(').next().unwrap_or(token) } else { token };
        opened.trim_end_matches([')', ';']).trim_matches(['"', '\''])
    }

    /// Whether the token is `cargo`, opened by a substitution or not.
    fn is_cargo(token: &str) -> bool {
        Self::word(token) == "cargo"
    }

    /// Whether the token is a redirection (`2>&1`, `>out`), which is no argument.
    fn is_redirection(token: &str) -> bool {
        token.contains('>') || token.contains('<')
    }

    /// Whether the token ends the command's arguments before itself: an operator or a redirection.
    fn ends_before(token: &str) -> bool {
        Self::OPERATORS.contains(&token) || Self::is_redirection(token)
    }

    /// Whether the token is the command's last: it closes a substitution or ends with `;`.
    fn closes(token: &str) -> bool {
        token.ends_with(')') || token.ends_with(';')
    }

    /// The index one past the last argument of the command whose arguments start at `start`.
    fn arguments_end(tokens: &[String], start: usize) -> usize {
        for (at, token) in tokens.iter().enumerate().skip(start) {
            if Self::ends_before(token) {
                return at;
            }
            if Self::closes(token) {
                return at + 1;
            }
        }
        tokens.len()
    }

    /// The token with its word replaced by `new`, its surrounding punctuation kept.
    fn with_word(token: &str, new: &str) -> String {
        token.replacen(Self::word(token), new, 1)
    }
}

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
        let cargo_at = tokens.windows(2).position(|pair| ShellToken::is_cargo(&pair[0]) && pair[1] == "test")?;
        let arguments_end = ShellToken::arguments_end(&tokens, cargo_at + 2);
        Some(CargoTestCommand { tokens, cargo_at, arguments_end })
    }

    /// The tokens after `cargo test`, up to the first `--`, shell operator or redirection, or to the
    /// end of the substitution the command sits in.
    fn arguments(&self) -> &[String] {
        &self.tokens[self.cargo_at + 2..self.arguments_end]
    }

    /// The value after the first of `flags` among the arguments, with its token index.
    fn value_after(&self, flags: &[&str]) -> Option<(usize, &str)> {
        let arguments = self.arguments();
        let at = arguments.iter().position(|a| flags.contains(&a.as_str()))?;
        arguments.get(at + 1).map(|value| (self.cargo_at + 2 + at + 1, ShellToken::word(value)))
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
        arguments.iter().enumerate().filter(|(_, a)| a.as_str() == "--test").filter_map(|(at, _)| arguments.get(at + 1)).map(|value| ShellToken::word(value)).collect()
    }

    /// Whether a `--test` value names the target `name`: equal to it, or a pattern cargo expands
    /// (`*` any run of characters, `?` one character) that matches it.
    pub fn names_target(pattern: &str, name: &str) -> bool {
        let pattern: Vec<char> = pattern.chars().collect();
        let name: Vec<char> = name.chars().collect();
        // matched[j]: the pattern read so far matches the first j characters of the name
        let mut matched = vec![false; name.len() + 1];
        matched[0] = true;
        for &symbol in &pattern {
            let mut next = vec![false; name.len() + 1];
            for at in 0..=name.len() {
                next[at] = match symbol {
                    '*' => matched[at] || (at > 0 && next[at - 1]),
                    '?' => at > 0 && matched[at - 1],
                    literal => at > 0 && matched[at - 1] && name[at - 1] == literal,
                };
            }
            matched = next;
        }
        matched[name.len()]
    }

    /// The command after the target `from_target` became the module `module` of `to_target` in
    /// `to_package`: the `-p` and the `--test` replaced, a repeated `--test` dropped, and, when the
    /// command named that one target alone, its test filter prefixed with `module::` (added when
    /// there was none) so the run stays as small as it was.
    pub fn moved(&self, from_target: &str, to_package: &str, to_target: &str, module: &str) -> String {
        let alone = self.targets().len() == 1;
        let mut tokens = self.tokens.clone();
        if let Some((at, _)) = self.value_after(&["-p", "--package"]) {
            tokens[at] = ShellToken::with_word(&tokens[at], to_package);
        }
        let mut seen = false;
        let mut at = self.cargo_at + 2;
        let mut kept: Vec<String> = Vec::new();
        while at < tokens.len() {
            let is_flag = at < self.arguments_end && tokens[at] == "--test" && at + 1 < tokens.len();
            if !is_flag {
                kept.push(tokens[at].clone());
                at += 1;
                continue;
            }
            let value = if ShellToken::word(&tokens[at + 1]) == from_target { ShellToken::with_word(&tokens[at + 1], to_target) } else { tokens[at + 1].clone() };
            if !kept.windows(2).any(|pair| pair[0] == "--test" && ShellToken::word(&pair[1]) == ShellToken::word(&value)) {
                kept.push("--test".to_string());
                kept.push(value.clone());
            }
            if alone && !seen {
                let filter_at = at + 2;
                let has_filter = filter_at < self.arguments_end && !tokens[filter_at].starts_with('-');
                kept.push(if has_filter { format!("{module}::{}", tokens[filter_at]) } else { format!("{module}::") });
                at += usize::from(has_filter);
                seen = true;
            }
            at += 2;
        }
        let mut result: Vec<String> = tokens[..self.cargo_at + 2].to_vec();
        result.extend(kept);
        result.join(" ")
    }
}

/// A command line as a `**Build**:` line writes it, read for the packages its `cargo` commands
/// name: every `-p`/`--package` of every `cargo` subcommand (`build`, `test`, `run`, ...), each
/// command's flags read up to its `--` or the next shell operator.
pub struct CargoCommandLine {
    tokens: Vec<String>,
}

impl CargoCommandLine {
    /// Splits a command line into its tokens.
    pub fn parse(command: &str) -> CargoCommandLine {
        CargoCommandLine { tokens: command.split_whitespace().map(str::to_string).collect() }
    }

    /// The token index of every package a `cargo` command names, with the package, in order:
    /// the value after `-p` or `--package`, or the text after `--package=`.
    fn package_positions(&self) -> Vec<(usize, &str)> {
        let mut positions = Vec::new();
        let mut in_cargo_flags = false;
        for (at, token) in self.tokens.iter().enumerate() {
            if ShellToken::is_cargo(token) {
                in_cargo_flags = true;
                continue;
            }
            if ShellToken::ends_before(token) {
                in_cargo_flags = false;
                continue;
            }
            if in_cargo_flags && at > 0 && matches!(self.tokens[at - 1].as_str(), "-p" | "--package") {
                positions.push((at, ShellToken::word(token)));
            } else if in_cargo_flags && let Some(package) = token.strip_prefix("--package=") {
                positions.push((at, ShellToken::word(package)));
            }
            if ShellToken::closes(token) {
                in_cargo_flags = false;
            }
        }
        positions
    }

    /// Every package the command line's `cargo` commands name, in order.
    pub fn packages(&self) -> Vec<&str> {
        self.package_positions().into_iter().map(|(_, package)| package).collect()
    }

    /// The command line with every package flag naming `old` naming `new`; `None` when no flag
    /// names `old`.
    pub fn with_package_renamed(&self, old: &str, new: &str) -> Option<String> {
        let renamed: Vec<usize> = self.package_positions().into_iter().filter(|(_, package)| *package == old).map(|(at, _)| at).collect();
        if renamed.is_empty() {
            return None;
        }
        let mut tokens = self.tokens.clone();
        for at in renamed {
            tokens[at] = match tokens[at].strip_prefix("--package=") {
                Some(value) => format!("--package={}", ShellToken::with_word(value, new)),
                None => ShellToken::with_word(&tokens[at], new),
            };
        }
        Some(tokens.join(" "))
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
    fn a_move_prefixes_the_filter_with_the_module_and_merges_repeated_targets() {
        let plain = CargoTestCommand::parse("cargo test -p a --test old").expect("a cargo test");
        assert_eq!(plain.moved("old", "a", "merged", "old"), "cargo test -p a --test merged old::");
        let filtered = CargoTestCommand::parse("cargo test -p a --test old inventory_guard").expect("a cargo test");
        assert_eq!(filtered.moved("old", "b", "merged", "old"), "cargo test -p b --test merged old::inventory_guard");
        let several = CargoTestCommand::parse("cargo test -p a --test crate_boundaries --test workspace_shape").expect("a cargo test");
        let once = several.moved("crate_boundaries", "g", "boundaries", "crate_boundaries");
        assert_eq!(once, "cargo test -p g --test boundaries --test workspace_shape");
        let twice = CargoTestCommand::parse(&once).expect("a cargo test").moved("workspace_shape", "g", "boundaries", "workspace_shape");
        assert_eq!(twice, "cargo test -p g --test boundaries");
    }

    #[test]
    fn a_cargo_test_inside_a_substitution_is_read_and_moved() {
        let proof = "o=$(cargo test -p a --test old 2>&1); for t in x; do printf '%s' \"$o\" | grep -q \"test $t ... ok\"; done";
        let command = CargoTestCommand::parse(proof).expect("a cargo test");
        assert_eq!((command.package(), command.targets()), (Some("a"), vec!["old"]));
        assert_eq!(command.moved("old", "b", "merged", "old"), proof.replace("-p a --test old", "-p b --test merged old::"));
        let closed = CargoTestCommand::parse("x=$( cargo test -p a --test old)").expect("a cargo test");
        assert_eq!((closed.package(), closed.targets()), (Some("a"), vec!["old"]));
        assert_eq!(CargoCommandLine::parse("o=$(cargo build -p a); cargo run -p b)").packages(), vec!["a", "b"]);
        assert_eq!(CargoCommandLine::parse("o=$(cargo test -p old)").with_package_renamed("old", "new").as_deref(), Some("o=$(cargo test -p new)"));
    }

    #[test]
    fn a_quoted_pattern_names_every_target_it_matches() {
        let command = CargoTestCommand::parse("env -u X cargo test -p a --test 'parity_*'").expect("a cargo test");
        assert_eq!(command.targets(), vec!["parity_*"]);
        assert!(CargoTestCommand::names_target("parity_*", "parity_dc"));
        assert!(CargoTestCommand::names_target("parity_?c", "parity_dc"));
        assert!(!CargoTestCommand::names_target("parity_*", "twins"));
        assert!(CargoTestCommand::names_target("twins", "twins"));
    }

    #[test]
    fn a_redirection_is_no_test_filter() {
        let command = CargoTestCommand::parse("cargo test -p a --test old 2>&1 | tail -3").expect("a cargo test");
        assert_eq!(command.moved("old", "a", "merged", "old"), "cargo test -p a --test merged old:: 2>&1 | tail -3");
    }

    #[test]
    fn a_command_line_names_the_packages_of_every_cargo_command() {
        let line = CargoCommandLine::parse("cargo build -p a -p b --all-targets 2>&1 | tail -3 && cargo run -p c -- -p not_a_package && cargo test --package=d");
        assert_eq!(line.packages(), vec!["a", "b", "c", "d"]);
        assert!(CargoCommandLine::parse("grep -p x").packages().is_empty());
    }

    #[test]
    fn a_rename_changes_every_package_flag_that_names_the_old_crate() {
        let line = CargoCommandLine::parse("cargo build -p old -p other && cargo test --package=old --test t");
        assert_eq!(line.with_package_renamed("old", "new").as_deref(), Some("cargo build -p new -p other && cargo test --package=new --test t"));
        assert_eq!(line.with_package_renamed("missing", "new"), None);
    }
}

//! The binary's command line (feature `msl-cli`, T5; checks C2, C5, C7 and C8, CLI-01 AC 2, 5,
//! 6 and 7): the real `piperine` binary is run as a child process; its name, the commands
//! `--help` lists, `--version`, and the exit code 2 of a usage error.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary cargo built for this target.
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_piperine"))
}

fn piperine(args: &[&str]) -> Output {
    Command::new(binary()).args(args).output().unwrap_or_else(|e| panic!("cannot run {}: {e}", binary().display()))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_binary_is_named_piperine() {
    // CLI-01 AC 2: the binary target of `piperine-cli` is `piperine`
    let path = binary();
    assert_eq!(path.file_stem().and_then(|s| s.to_str()), Some("piperine"), "{}", path.display());
    assert!(path.is_file(), "{} is not built", path.display());
}

/// The command names under the `Commands:` heading of a `--help` text.
fn commands_listed(help: &str) -> Vec<String> {
    let section = help.split("Commands:").nth(1).unwrap_or_else(|| panic!("no `Commands:` heading in\n{help}"));
    section
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect()
}

#[test]
fn help_lists_exactly_the_eight_commands() {
    // CLI-01 AC 5 and HII-11: exactly `check`, `build`, `run`, `test`, `new`, `clean`, `fmt` and `info`, exit 0
    let out = piperine(&["--help"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let mut listed = commands_listed(&text(&out.stdout));
    listed.sort();
    let mut want = ["check", "build", "run", "test", "new", "clean", "fmt", "info"].map(str::to_string).to_vec();
    want.sort();
    assert_eq!(listed, want, "{}", text(&out.stdout));
}

/// `version = "..."` of `[workspace.package]` in the root manifest.
fn workspace_version() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let manifest = std::fs::read_to_string(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    let section = manifest.split("[workspace.package]").nth(1).expect("a [workspace.package] table");
    let line = section.lines().find(|l| l.trim_start().starts_with("version")).expect("a workspace version");
    line.split('"').nth(1).expect("a quoted version").to_string()
}

#[test]
fn version_prints_the_workspace_version() {
    // CLI-01 AC 6: `piperine` and the workspace version, exit 0
    let out = piperine(&["--version"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), format!("piperine {}\n", workspace_version()));
}

#[test]
fn help_lists_the_commands_and_version_prints_the_version() {
    // the Book's cli~3cd92481: `piperine --help` lists the commands and `piperine --version` prints the version
    let help = piperine(&["--help"]);
    assert_eq!(help.status.code(), Some(0), "{}", text(&help.stderr));
    assert_eq!(commands_listed(&text(&help.stdout)).len(), 8, "{}", text(&help.stdout));
    let version = piperine(&["--version"]);
    assert_eq!(version.status.code(), Some(0), "{}", text(&version.stderr));
    assert_eq!(text(&version.stdout), format!("piperine {}\n", workspace_version()));
}

#[test]
fn an_unknown_command_or_flag_exits_2() {
    // CLI-01 AC 7: a command or a flag the system does not define prints the usage error on
    // standard error, nothing on standard output, and exits 2
    for args in [&["frobnicate"][..], &["--frobnicate"], &["run", "--frobnicate"]] {
        let out = piperine(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out.stderr));
        let err = text(&out.stderr);
        assert!(err.starts_with("error:") && err.contains("Usage:"), "{args:?}: the usage error on standard error:\n{err}");
        assert!(out.stdout.is_empty(), "{args:?}: nothing on standard output: {}", text(&out.stdout));
    }
}

//! Where the parity corpus lives: `tests/e2e/`, one directory per scenario, each holding twins
//! (`<name>.msl` beside its ngspice netlist `<name>.cir`, the command a user types in
//! `<name>.command` and the output it prints in `<name>.stdout`) and the libraries they load.
//! This is the one place that knows that layout.

use std::path::{Path, PathBuf};

/// The corpus: the scenario directories under one root.
#[derive(Clone, Debug)]
pub struct Corpus {
    root: PathBuf,
}

impl Corpus {
    /// The corpus of this repository, `tests/e2e/`.
    pub fn of_repository() -> Corpus {
        Corpus::at(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e"))
    }

    /// A corpus at another root (a test plants one).
    pub fn at(root: &Path) -> Corpus {
        Corpus { root: root.canonicalize().unwrap_or_else(|error| panic!("{}: no corpus ({error})", root.display())) }
    }

    /// Every twin of the corpus (an `.msl` beside a `.cir` of its name), by name.
    pub fn twins(&self) -> Vec<Twin> {
        let mut twins = Vec::new();
        for directory in self.scenario_directories() {
            for name in Self::names_in(&directory, "msl") {
                if directory.join(format!("{name}.cir")).is_file() {
                    twins.push(Twin { name, directory: directory.clone() });
                }
            }
        }
        twins.sort_by(|left, right| left.name.cmp(&right.name));
        twins
    }

    /// The one twin of that name. Panics naming the name when no scenario has it, and naming both
    /// directories when two do.
    pub fn twin(&self, name: &str) -> Twin {
        let mut found: Vec<Twin> = self.twins().into_iter().filter(|twin| twin.name == name).collect();
        match found.len() {
            1 => found.remove(0),
            0 => panic!("{name}: no twin of that name (an `{name}.msl` beside an `{name}.cir`) under {}", self.root.display()),
            _ => panic!("{name}: a twin of that name is in two scenarios, {} and {}", found[0].directory.display(), found[1].directory.display()),
        }
    }

    /// The one `<name>.cir` of the corpus, a twin's or an ngspice-only fixture's, as an absolute
    /// path. Panics like [`twin`](Self::twin).
    pub fn netlist(&self, name: &str) -> PathBuf {
        let mut found: Vec<PathBuf> = self.scenario_directories().into_iter().map(|directory| directory.join(format!("{name}.cir"))).filter(|path| path.is_file()).collect();
        match found.len() {
            1 => found.remove(0),
            0 => panic!("{name}: no {name}.cir under {}", self.root.display()),
            _ => panic!("{name}: a netlist of that name is in two scenarios, {} and {}", found[0].display(), found[1].display()),
        }
    }

    /// The subdirectories of the root, by name.
    fn scenario_directories(&self) -> Vec<PathBuf> {
        let entries = std::fs::read_dir(&self.root).unwrap_or_else(|error| panic!("{}: cannot read the corpus ({error})", self.root.display()));
        let mut directories: Vec<PathBuf> = entries.map(|entry| entry.unwrap_or_else(|error| panic!("{}: cannot read an entry ({error})", self.root.display())).path()).filter(|path| path.is_dir()).collect();
        directories.sort();
        directories
    }

    /// The stems of the files of `directory` with `extension`.
    fn names_in(directory: &Path, extension: &str) -> Vec<String> {
        let entries = std::fs::read_dir(directory).unwrap_or_else(|error| panic!("{}: cannot read the scenario ({error})", directory.display()));
        entries
            .map(|entry| entry.unwrap_or_else(|error| panic!("{}: cannot read an entry ({error})", directory.display())).path())
            .filter(|path| path.extension().is_some_and(|found| found == extension))
            .filter_map(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
            .collect()
    }
}

/// One twin: the files of a scenario that carry its name.
#[derive(Clone, Debug)]
pub struct Twin {
    name: String,
    directory: PathBuf,
}

impl Twin {
    /// The twin's name, the stem of every file of it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The scenario directory the twin lives in, where its command runs.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The Piperine model, `<name>.msl`.
    pub fn model(&self) -> PathBuf {
        self.directory.join(format!("{}.msl", self.name))
    }

    /// The ngspice netlist, `<name>.cir`.
    pub fn netlist(&self) -> PathBuf {
        self.directory.join(format!("{}.cir", self.name))
    }

    /// The command a user types from the scenario directory, `<name>.command`.
    pub fn command(&self) -> PathBuf {
        self.directory.join(format!("{}.command", self.name))
    }

    /// The standard output the command prints after its header line, `<name>.stdout`.
    pub fn expected_stdout(&self) -> PathBuf {
        self.directory.join(format!("{}.stdout", self.name))
    }

    /// The arguments of [`command`](Self::command) after the program name `piperine`, split the
    /// way a shell splits a line: at blanks, except inside single quotes. Panics naming the file
    /// when it cannot be read or does not start with `piperine`.
    pub fn command_arguments(&self) -> Vec<String> {
        let path = self.command();
        let line = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: cannot read the command ({error})", path.display()));
        let mut words = Self::split_line(line.trim());
        if words.is_empty() || words.remove(0) != "piperine" {
            panic!("{}: a command starts with `piperine`", path.display());
        }
        words
    }

    /// `line` split at blanks outside single quotes; the quotes themselves are dropped.
    fn split_line(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut quoted = false;
        for character in line.chars() {
            match character {
                '\'' => quoted = !quoted,
                blank if blank.is_whitespace() && !quoted => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                other => word.push(other),
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Associated test helpers of `corpus`, grouped on one fixture type.
    struct CorpusFixture;

    impl CorpusFixture {
        /// A corpus of the scenario directories `layout` names, each with the files it lists.
        fn planted(layout: &[(&str, &[&str])]) -> (tempfile::TempDir, Corpus) {
            let root = tempfile::tempdir().expect("a temporary directory");
            for (scenario, files) in layout {
                let directory = root.path().join(scenario);
                std::fs::create_dir(&directory).expect("a scenario directory");
                for file in *files {
                    std::fs::write(directory.join(file), "").expect("a planted file");
                }
            }
            let corpus = Corpus::at(root.path());
            (root, corpus)
        }
    }

    #[test]
    fn a_twin_is_found_by_name() {
        let (_root, corpus) = CorpusFixture::planted(&[("twins", &["alpha.msl", "alpha.cir", "devices.msl", "lonely.cir"])]);
        let twin = corpus.twin("alpha");
        assert_eq!(twin.name(), "alpha");
        assert!(twin.directory().ends_with("twins"));
        assert!(twin.model().ends_with("twins/alpha.msl") && twin.model().is_file());
        assert!(twin.netlist().ends_with("twins/alpha.cir") && twin.netlist().is_file());
        assert!(twin.command().ends_with("twins/alpha.command"));
        assert!(twin.expected_stdout().ends_with("twins/alpha.stdout"));
        let names: Vec<String> = corpus.twins().iter().map(|twin| twin.name().to_string()).collect();
        assert_eq!(names, vec!["alpha".to_string()], "a library and a netlist with no model are no twins");
        assert!(corpus.netlist("lonely").ends_with("twins/lonely.cir"), "a netlist with no model is still found");
    }

    #[test]
    fn a_command_is_read_as_the_arguments_after_piperine() {
        let (_root, corpus) = CorpusFixture::planted(&[("twins", &["alpha.msl", "alpha.cir"])]);
        let twin = corpus.twin("alpha");
        std::fs::write(twin.command(), "piperine run alpha.msl --from -1 --print 'i(v1)'\n").expect("a planted command");
        assert_eq!(twin.command_arguments(), vec!["run", "alpha.msl", "--from", "-1", "--print", "i(v1)"]);
    }

    #[test]
    #[should_panic(expected = "gamma: no twin of that name")]
    fn a_missing_twin_names_itself() {
        let (_root, corpus) = CorpusFixture::planted(&[("twins", &["alpha.msl", "alpha.cir"])]);
        corpus.twin("gamma");
    }

    #[test]
    #[should_panic(expected = "a twin of that name is in two scenarios")]
    fn a_name_in_two_scenarios_is_refused() {
        let (_root, corpus) = CorpusFixture::planted(&[("one", &["alpha.msl", "alpha.cir"]), ("two", &["alpha.msl", "alpha.cir"])]);
        corpus.twin("alpha");
    }
}

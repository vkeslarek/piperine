use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SourceMap {
    /// Mapping of base namespaces to their filesystem paths.
    /// E.g. "piperine" -> "/path/to/stdlib/headers", or a package name from
    /// `Piperine.toml` -> that package's `src/` directory.
    pub namespaces: HashMap<String, PathBuf>,
    /// The root path for unqualified `use` statements.
    /// E.g. `use capabilities;` -> `<root_path>/capabilities.phdl`
    pub root_path: PathBuf,
    /// An optional prelude path to inject prelude contents automatically.
    pub prelude_path: Option<PathBuf>,
}

impl SourceMap {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            namespaces: HashMap::new(),
            root_path,
            prelude_path: None,
        }
    }

    pub fn with_prelude(mut self, prelude_path: PathBuf) -> Self {
        self.prelude_path = Some(prelude_path);
        self
    }

    pub fn add_namespace(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) {
        self.namespaces.insert(name.into(), path.into());
    }

    /// A dummy source map for use in tests.
    pub fn dummy() -> Self {
        let mut map = Self::new("headers".into());
        map = map.with_prelude("headers/prelude.phdl".into());
        map.add_namespace("piperine", "headers");
        map
    }
}

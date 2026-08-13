use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SourceMap {
    /// Mapping of base namespaces to their filesystem paths, also used as
    /// the `` `include`` search path (VAMS-16, `vams_include::
    /// SourceMapIncludeResolver`). E.g. "piperine" -> "/path/to/stdlib/
    /// headers", or a package name from `Piperine.toml` -> that package's
    /// `src/` directory.
    pub namespaces: HashMap<String, PathBuf>,
    /// The root path `` `include`` falls back to when no namespace
    /// resolves a path.
    pub root_path: PathBuf,
    /// An optional prelude path to inject prelude contents automatically.
    /// Unused since T21/MD-45 (PHDL's `use`-decl resolver, the only reader
    /// of this field, was deleted with the PHDL parser) — kept as a stable
    /// field rather than a breaking removal; nothing sets it anymore.
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

    /// A dummy source map for use in tests. `` `include "disciplines.vams"``
    /// etc. resolve against the "piperine" namespace below (T40, VAMS-09).
    pub fn dummy() -> Self {
        let mut map = Self::new("headers".into());
        map.add_namespace("piperine", "headers");
        map.add_namespace("spice", "headers/spice");
        map
    }
}

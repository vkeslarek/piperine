//! A `std`-like crate: a standard library written in a test, over which a model is built and checked.

use piperine_lang::pom::Design;
use piperine_lang::types::Types;

/// A standard library of two files, `lib.msl` and `prelude.msl`, plus the namespaces a test adds, and the root crate
/// that uses it.
pub struct StdCrate {
    /// The items the prelude of the standard library declares.
    prelude: String,
    /// The namespaces of the standard library beside its prelude, each as its name and the text of its file.
    namespaces: Vec<(String, String)>,
    /// The text of the root crate.
    root: String,
}

impl StdCrate {
    /// A standard library whose prelude is `prelude`, and a root crate `root`.
    pub fn declaring(prelude: &str, root: &str) -> Self {
        Self { prelude: prelude.to_string(), namespaces: Vec::new(), root: root.to_string() }
    }

    /// The same crates, the standard library also declaring the `pub` namespace `name` of the items `text` (its file
    /// `<name>.msl`, named by a `pub mod` of the root of the standard library).
    pub fn with_namespace(mut self, name: &str, text: &str) -> Self {
        self.namespaces.push((name.to_string(), text.to_string()));
        self
    }

    /// The design of the two crates.
    pub fn design(&self) -> Design {
        let library: String = self.namespaces.iter().map(|(name, _)| format!("pub mod {name};\n")).chain(["pub mod prelude;\n".to_string()]).collect();
        let builder = Design::builder().std_source("lib.msl", library).std_source("prelude.msl", self.prelude.clone());
        let builder = self.namespaces.iter().fold(builder, |builder, (name, text)| builder.std_source(format!("{name}.msl"), text.clone()));
        builder.source("top.msl", self.root.clone()).build()
    }

    /// The `(code, message)` of every diagnostic of the design, then of the check of its module `Top`; empty when both pass.
    pub fn findings(&self) -> Vec<(String, String)> {
        let design = self.design();
        let mut found: Vec<(String, String)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
        if found.is_empty()
            && let Err(refused) = Types::check(&design, "Top")
        {
            found.extend(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())));
        }
        found
    }

    /// The codes of [`StdCrate::findings`], in order.
    pub fn codes(&self) -> Vec<String> {
        self.findings().into_iter().map(|(code, _)| code).collect()
    }
}

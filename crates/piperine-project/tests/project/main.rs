//! `piperine-project`: the manifest of a package, `path` dependencies into a crate graph, git dependencies checked out under `target/deps`, the lockfile that records their commits, and the entry surface (the project root is the nearest `Piperine.toml`). Each former target is a module of this one.

mod git;
mod lock;
mod manifest;
mod paths;
mod project;
mod support;

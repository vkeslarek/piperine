//! Boundaries: the structure of the workspace as guards. `crate_boundaries` holds the three-crate
//! solver split, `workspace_shape` the manifests, the instruction files and the root package, and
//! `dependency_graph` the arrows of `CLAUDE.md` "Pipeline", and `inventory_types` the types the
//! inventory found written in two crates.

mod crate_boundaries;
mod dependency_graph;
mod inventory_types;
mod workspace_shape;
